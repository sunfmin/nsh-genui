//! The shell's `host.call("generate", …)`: asks DeepSeek for a module, checks it, and asks again with the
//! error when it does not pass (at most `MAX_ATTEMPTS` times). The reply streams to the shell as it comes.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use nsh_engine::{HostCall, HostChannel};
use serde_json::{Value, json};

use crate::deepseek::{Client, Delta};
use crate::{build, keychain};

const MAX_ATTEMPTS: usize = 3;

/// The contract of generated modules, then NSH's own TSX guide.
static SYSTEM: LazyLock<String> = LazyLock::new(|| {
    format!("{}\n\n{}", include_str!("../prompts/contract.md"), include_str!("../prompts/nsh-tsx-guide.md"))
});

/// Answers one call; runs on a thread of its own.
pub fn handle(call: HostCall, host: HostChannel) {
    let data = call.data().clone();
    let result = run(&data, &host);
    if let Err(e) = &result {
        log::warn!(target: "genui", "generate failed: {e}");
    }
    call.reply(result);
}

fn run(data: &Value, host: &HostChannel) -> Result<Value, String> {
    let instruction = data["instruction"].as_str().unwrap_or("").trim();
    if instruction.is_empty() {
        return Err("需求是空的".into());
    }
    // Development: `GENUI_REPLAY=<file>` answers with the file's text instead of DeepSeek's.
    if let Ok(path) = std::env::var("GENUI_REPLAY") {
        let reply = std::fs::read_to_string(&path).map_err(|e| format!("GENUI_REPLAY {path}: {e}"))?;
        let (tsx, summary) = build::extract(&reply)?;
        let js = build::generated(&tsx)?;
        return Ok(json!({ "tsx": tsx, "js": js, "summary": summary, "model": "replay", "seconds": 0.0,
            "attempts": 1, "promptTokens": 0, "completionTokens": 0 }));
    }
    let key = keychain::get()?.ok_or("还没有设置 DeepSeek API key：点左下角「设置」填写")?;
    let client = Client::new(key, data["model"].as_str().unwrap_or(""));
    let mut messages = vec![
        json!({ "role": "system", "content": &*SYSTEM }),
        json!({ "role": "user", "content": user_message(instruction, data["base"].as_str(), data["error"].as_str()) }),
    ];

    let started = Instant::now();
    let (mut prompt_tokens, mut completion_tokens) = (0, 0);
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let _ = host.send("reset", Value::Null);
        status(
            host,
            if attempt == 1 {
                format!("{} 正在写代码", client.model)
            } else {
                format!("第 {attempt} 次：按检查结果修正")
            },
        );
        let mut stream = Stream::new(host);
        let reply = client.chat(&messages, |d| stream.push(d))?;
        stream.flush();
        prompt_tokens += reply.prompt_tokens;
        completion_tokens += reply.completion_tokens;

        let checked = build::extract(&reply.content)
            .and_then(|(tsx, summary)| build::generated(&tsx).map(|js| (tsx, summary, js)));
        match checked {
            Ok((tsx, summary, js)) => {
                let seconds = started.elapsed().as_secs_f64();
                log::info!(target: "genui", "generated in {seconds:.1}s, {attempt} attempt(s), {completion_tokens} completion tokens");
                return Ok(json!({
                    "tsx": tsx,
                    "js": js,
                    "summary": summary,
                    "model": client.model,
                    "seconds": seconds,
                    "attempts": attempt,
                    "promptTokens": prompt_tokens,
                    "completionTokens": completion_tokens,
                }));
            }
            Err(e) => {
                log::warn!(target: "genui", "attempt {attempt} did not pass: {e}");
                messages.push(json!({ "role": "assistant", "content": reply.content }));
                messages.push(json!({
                    "role": "user",
                    "content": format!("这个模块没有通过宿主的检查：\n\n{e}\n\n请修正，并输出完整的模块。"),
                }));
                last_error = e;
            }
        }
    }
    Err(format!("试了 {MAX_ATTEMPTS} 次仍没有通过检查：\n{last_error}"))
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

fn status(host: &HostChannel, text: String) {
    let _ = host.send("status", json!({ "text": text }));
}

/// Hands the reply to the shell in pieces at most every 60 ms, so a fast stream is a few events a frame.
struct Stream<'a> {
    host: &'a HostChannel,
    pending: String,
    sent: Instant,
    reasoning: usize,
}

impl<'a> Stream<'a> {
    const EVERY: Duration = Duration::from_millis(60);

    fn new(host: &'a HostChannel) -> Self {
        Stream { host, pending: String::new(), sent: Instant::now(), reasoning: 0 }
    }

    fn push(&mut self, delta: Delta) {
        match delta {
            Delta::Content(text) => self.pending.push_str(text),
            Delta::Reasoning(text) => self.reasoning += text.chars().count(),
        }
        if self.sent.elapsed() >= Self::EVERY {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.sent = Instant::now();
        if self.reasoning > 0 && self.pending.is_empty() {
            status(self.host, format!("思考中（{} 字）", self.reasoning));
        }
        if !self.pending.is_empty() {
            let text = std::mem::take(&mut self.pending);
            let _ = self.host.send("delta", json!({ "text": text }));
        }
    }
}
