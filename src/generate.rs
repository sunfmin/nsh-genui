//! Asks DeepSeek for a module, checks it, and asks again with the error when it does not pass (at most
//! [`MAX_ATTEMPTS`] times). Knows nothing of the app: where the key comes from and where progress goes are the
//! caller's (the app's `src/calls.rs`, the integration tests).

use std::sync::LazyLock;
use std::time::Instant;

use serde_json::{Value, json};

use crate::build;
use crate::deepseek::{Client, Delta};

pub const MAX_ATTEMPTS: usize = 3;

/// The contract of generated modules, the names NSH's modules export (from the runtime itself), then NSH's own
/// guides: TSX, and the charts of `nsh/base/plot`.
static SYSTEM: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{}\n\n{}\n\n{}\n\n# The charts: nsh/base/plot\n\n{}",
        include_str!("../prompts/contract.md"),
        build::exports_reference(),
        include_str!("../prompts/nsh-tsx-guide.md"),
        include_str!("../prompts/nsh-plot-guide.md"),
    )
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
    /// An attempt starts (1-based); the reply streams after it.
    Attempt(usize),
    /// A piece of the reply.
    Delta(&'a str),
    /// The model's reasoning so far, in characters.
    Thinking(usize),
    /// An attempt's module did not pass the checks.
    Rejected { attempt: usize, error: &'a str },
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
    pub attempts: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Why the earlier attempts did not pass.
    pub rejected: Vec<String>,
}

impl Generation {
    /// The shell's `Generation` (`shell/App.tsx`).
    pub fn to_json(&self) -> Value {
        json!({
            "tsx": self.tsx, "js": self.js, "summary": self.summary, "model": self.model,
            "seconds": self.seconds, "attempts": self.attempts,
            "promptTokens": self.prompt_tokens, "completionTokens": self.completion_tokens,
        })
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
    let mut rejected = Vec::new();
    for attempt in 1..=MAX_ATTEMPTS {
        progress(Attempt(attempt));
        let mut reasoning = 0;
        let reply = client.chat(&messages, |d| match d {
            Delta::Content(text) => progress(Progress::Delta(text)),
            Delta::Reasoning(text) => {
                reasoning += text.chars().count();
                progress(Progress::Thinking(reasoning));
            }
        })?;
        prompt_tokens += reply.prompt_tokens;
        completion_tokens += reply.completion_tokens;

        let checked = build::extract(&reply.content)
            .and_then(|(tsx, summary)| build::generated(&tsx).map(|js| (tsx, summary, js)));
        match checked {
            Ok((tsx, summary, js)) => {
                return Ok(Generation {
                    tsx,
                    js,
                    summary,
                    model: client.model.clone(),
                    seconds: started.elapsed().as_secs_f64(),
                    attempts: attempt,
                    prompt_tokens,
                    completion_tokens,
                    rejected,
                });
            }
            Err(e) => {
                progress(Progress::Rejected { attempt, error: &e });
                messages.push(json!({ "role": "assistant", "content": reply.content }));
                messages.push(json!({
                    "role": "user",
                    "content": format!("这个模块没有通过宿主的检查：\n\n{e}\n\n请修正，并输出完整的模块。"),
                }));
                rejected.push(e);
            }
        }
    }
    Err(format!("试了 {MAX_ATTEMPTS} 次仍没有通过检查：\n{}", rejected.last().map(String::as_str).unwrap_or("")))
}

use Progress::Attempt;

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
        attempts: 1,
        prompt_tokens: 0,
        completion_tokens: 0,
        rejected: Vec::new(),
    })
}

fn user_message(instruction: &str, base: Option<&str>, error: Option<&str>) -> String {
    match (base, error) {
        (Some(base), Some(error)) => format!(
            "当前模块：\n```tsx\n{base}\n```\n\n它运行时出错：\n{error}\n\n原始需求：{instruction}\n\n请修复，输出完整模块。"
        ),
        (Some(base), None) => format!("当前模块：\n```tsx\n{base}\n```\n\n修改要求：{instruction}"),
        _ => instruction.to_owned(),
    }
}
