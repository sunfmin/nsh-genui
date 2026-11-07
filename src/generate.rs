//! The model writes a module with tools of its own: `check` compiles it and runs it as the sandbox would
//! (headless: renders, presses its buttons) and tells it what failed, as often as it wants; `submit` hands in
//! the result, which is checked again and only accepted without errors. The loop ends with an accepted module,
//! or after [`MAX_TURNS`] requests. Knows nothing of the app: the key and where progress goes are the caller's
//! (the app's `src/calls.rs`, the integration tests).

use std::sync::LazyLock;
use std::time::Instant;

use serde_json::{Value, json};

use crate::build;
use crate::deepseek::{Client, Delta, ToolCall};
use crate::verify::{self, Report};

/// Requests to the model at most, each answering the tool results before it.
pub const MAX_TURNS: usize = 16;
/// Texts of the screen a check reports at most.
const TEXTS: usize = 80;

/// The contract of generated modules and its tools, the names NSH's modules export (from the runtime itself),
/// then NSH's own guides: TSX, and the charts of `nsh/base/plot`.
static SYSTEM: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{}\n\n{}\n\n{}\n\n# The charts: nsh/base/plot\n\n{}",
        include_str!("../prompts/contract.md"),
        build::exports_reference(),
        include_str!("../prompts/nsh-tsx-guide.md"),
        include_str!("../prompts/nsh-plot-guide.md"),
    )
});

static TOOLS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    let module = json!({ "type": "string", "description": "The whole module: TSX source with `export default function App()`." });
    vec![
        json!({ "type": "function", "function": {
            "name": "check",
            "description": "Compiles the module and runs it as the sandbox will, headless: it renders, then each \
                enabled button is pressed in turn. Returns `ok`, the errors (compile errors, imports NSH does not \
                export, exceptions while rendering or pressing), the texts on screen and the buttons pressed. \
                Call it as often as you need; fix what it reports and check again.",
            "parameters": { "type": "object", "properties": { "tsx": module }, "required": ["tsx"] },
        }}),
        json!({ "type": "function", "function": {
            "name": "submit",
            "description": "Hands in the finished module. It is checked again: accepted only when the check passes, \
                otherwise you get the errors and must fix and submit again.",
            "parameters": { "type": "object", "properties": {
                "tsx": module,
                "summary": { "type": "string", "description": "One short sentence: what the UI is, in the user's language." },
            }, "required": ["tsx", "summary"] },
        }}),
    ]
});

/// What to generate.
#[derive(Debug, Default, Clone)]
pub struct Request {
    pub instruction: String,
    /// The current module, to change ("edit") or to fix (with `error`).
    pub base: Option<String>,
    /// What the current module failed with at run time.
    pub error: Option<String>,
}

impl Request {
    /// From the shell's call data: `{ instruction, base?, error? }`.
    pub fn from_json(data: &Value) -> Request {
        let text = |k: &str| data[k].as_str().map(str::to_owned).filter(|s| !s.trim().is_empty());
        Request { instruction: text("instruction").unwrap_or_default(), base: text("base"), error: text("error") }
    }
}

/// What happens while generating.
pub enum Progress<'a> {
    /// A request to the model starts (1-based).
    Turn(usize),
    /// A piece of the model's text.
    Delta(&'a str),
    /// The model's reasoning so far, in characters.
    Thinking(usize),
    /// The model is writing a tool call: its arguments so far, in characters.
    Writing(usize),
    /// A module the model checks or submits, before its result.
    Draft(&'a str),
    /// A check's or a submission's result.
    Checked { tool: &'a str, number: usize, errors: &'a [String] },
}

/// A module that passed the checks.
#[derive(Debug, Clone)]
pub struct Generation {
    pub tsx: String,
    /// What a sandbox runs (`build::generated`).
    pub js: String,
    pub summary: String,
    pub model: String,
    pub seconds: f64,
    /// Requests to the model.
    pub turns: usize,
    /// Checks the model ran (`check` and `submit`).
    pub checks: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// The errors of the checks that failed, in order.
    pub failed: Vec<Vec<String>>,
    /// The accepted module's headless run.
    pub report: Report,
}

impl Generation {
    /// The shell's `Generation` (`shell/App.tsx`).
    pub fn to_json(&self) -> Value {
        json!({
            "tsx": self.tsx, "js": self.js, "summary": self.summary, "model": self.model,
            "seconds": self.seconds, "attempts": self.checks, "turns": self.turns,
            "promptTokens": self.prompt_tokens, "completionTokens": self.completion_tokens,
        })
    }
}

/// A module compiled and run headless.
pub struct Checked {
    /// The compiled module, when it compiled.
    pub js: Option<String>,
    pub report: Report,
}

impl Checked {
    pub fn errors(&self) -> &[String] {
        &self.report.errors
    }

    pub fn ok(&self) -> bool {
        self.js.is_some() && self.report.errors.is_empty()
    }

    /// The tool's answer.
    fn to_json(&self) -> Value {
        let r = &self.report;
        json!({
            "ok": self.ok(),
            "compiled": self.js.is_some(),
            "errors": r.errors,
            "texts": r.texts.iter().take(TEXTS).collect::<Vec<_>>(),
            "buttons": r.buttons,
            "pressed": r.pressed,
        })
    }
}

/// Compiles `tsx` and runs it headless (`build::generated`, `verify::render`).
pub fn check(tsx: &str) -> Checked {
    match build::generated(tsx) {
        Ok(js) => {
            let mut report = verify::render(&js);
            if report.errors.is_empty() && report.texts.is_empty() {
                report.errors.push("the module shows no text at all".into());
            }
            Checked { js: Some(js), report }
        }
        Err(e) => Checked { js: None, report: Report { errors: vec![e], ..Report::default() } },
    }
}

pub fn generate(client: &Client, request: &Request, progress: &mut dyn FnMut(Progress)) -> Result<Generation, String> {
    let instruction = request.instruction.trim();
    if instruction.is_empty() {
        return Err("需求是空的".into());
    }
    let mut messages = vec![
        json!({ "role": "system", "content": &*SYSTEM }),
        json!({ "role": "user", "content": user_message(instruction, request.base.as_deref(), request.error.as_deref()) }),
    ];

    let started = Instant::now();
    let (mut prompt_tokens, mut completion_tokens) = (0, 0);
    let mut checks = 0;
    let mut failed: Vec<Vec<String>> = Vec::new();
    for turn in 1..=MAX_TURNS {
        progress(Progress::Turn(turn));
        let (mut reasoning, mut writing) = (0, 0);
        let reply = client.chat_with_tools(&messages, &TOOLS, |d| match d {
            Delta::Content(text) => progress(Progress::Delta(text)),
            Delta::Reasoning(text) => {
                reasoning += text.chars().count();
                progress(Progress::Thinking(reasoning));
            }
            Delta::ToolArguments(text) => {
                writing += text.chars().count();
                progress(Progress::Writing(writing));
            }
        })?;
        prompt_tokens += reply.prompt_tokens;
        completion_tokens += reply.completion_tokens;
        messages.push(reply.message());

        // An answer in text instead of a call: a module in it counts as submitted, else the model is reminded.
        let mut calls = reply.tool_calls.clone();
        if calls.is_empty() {
            match build::extract(&reply.content) {
                Ok((tsx, summary)) => calls.push(ToolCall {
                    id: String::new(),
                    name: "submit".into(),
                    arguments: json!({ "tsx": tsx, "summary": summary }).to_string(),
                }),
                Err(_) => {
                    messages.push(json!({ "role": "user", "content": "用 check 检查模块，通过后用 submit 提交。" }));
                    continue;
                }
            }
        }

        for call in calls {
            let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
            let tsx = args["tsx"].as_str().unwrap_or("");
            let answer = match call.name.as_str() {
                "check" | "submit" if !tsx.trim().is_empty() => {
                    progress(Progress::Draft(tsx));
                    checks += 1;
                    let checked = check(tsx);
                    progress(Progress::Checked { tool: &call.name, number: checks, errors: checked.errors() });
                    if !checked.ok() {
                        failed.push(checked.errors().to_vec());
                    } else if call.name == "submit" {
                        return Ok(Generation {
                            tsx: tsx.to_owned(),
                            js: checked.js.unwrap_or_default(),
                            summary: args["summary"].as_str().unwrap_or("").to_owned(),
                            model: client.model.clone(),
                            seconds: started.elapsed().as_secs_f64(),
                            turns: turn,
                            checks,
                            prompt_tokens,
                            completion_tokens,
                            failed,
                            report: checked.report,
                        });
                    }
                    checked.to_json()
                }
                "check" | "submit" => json!({ "ok": false, "errors": ["`tsx` is missing or empty"] }),
                other => json!({ "ok": false, "errors": [format!("there is no tool `{other}`: use check or submit")] }),
            };
            if !call.id.is_empty() {
                messages.push(json!({ "role": "tool", "tool_call_id": call.id, "content": answer.to_string() }));
            } else {
                messages.push(json!({ "role": "user", "content": format!("submit 的结果：{answer}") }));
            }
        }
    }
    let last = failed.last().map(|e| e.join("\n")).unwrap_or_default();
    Err(format!("{MAX_TURNS} 轮后仍没有通过检查的模块（检查了 {checks} 次）。最后一次的错误：\n{last}"))
}

/// A generation from a saved reply instead of DeepSeek's (development: `GENUI_REPLAY`).
pub fn replay(reply: &str) -> Result<Generation, String> {
    let (tsx, summary) = build::extract(reply)?;
    let js = build::generated(&tsx)?;
    Ok(Generation {
        tsx,
        js,
        summary,
        model: "replay".into(),
        seconds: 0.0,
        turns: 0,
        checks: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        failed: Vec::new(),
        report: Report::default(),
    })
}

fn user_message(instruction: &str, base: Option<&str>, error: Option<&str>) -> String {
    match (base, error) {
        (Some(base), Some(error)) => format!(
            "当前模块：\n```tsx\n{base}\n```\n\n它运行时出错：\n{error}\n\n原始需求：{instruction}\n\n请修复。"
        ),
        (Some(base), None) => format!("当前模块：\n```tsx\n{base}\n```\n\n修改要求：{instruction}"),
        _ => instruction.to_owned(),
    }
}
