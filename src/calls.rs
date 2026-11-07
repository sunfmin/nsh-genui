//! The shell's `host.call("generate", …)`: the key from the keychain, the reply streamed to the shell as host
//! events (`reset`, `delta`, `status`), the result or the error as the call's answer.

use std::time::{Duration, Instant};

use nsh_engine::{HostCall, HostChannel};
use nsh_genui::deepseek::Client;
use nsh_genui::generate::{self, Generation, Progress, Request};
use serde_json::{Value, json};

use crate::{dev, keychain};

/// Answers one call; runs on a thread of its own.
pub fn generate(call: HostCall, host: HostChannel) {
    let data = call.data().clone();
    let result = run(&data, &host);
    match &result {
        Ok(g) => dev::event("generated", json!({
            "model": g.model, "seconds": g.seconds, "turns": g.turns, "checks": g.checks,
            "promptTokens": g.prompt_tokens, "completionTokens": g.completion_tokens,
            "summary": g.summary, "tsx": g.tsx,
        })),
        Err(e) => {
            log::warn!(target: "genui", "generate failed: {e}");
            dev::event("failed", json!({ "error": e }));
        }
    }
    call.reply(result.map(|g| g.to_json()));
}

fn run(data: &Value, host: &HostChannel) -> Result<Generation, String> {
    // Development: `GENUI_REPLAY=<file>` answers with the file's text instead of DeepSeek's.
    if let Ok(path) = std::env::var("GENUI_REPLAY") {
        let reply = std::fs::read_to_string(&path).map_err(|e| format!("GENUI_REPLAY {path}: {e}"))?;
        return generate::replay(&reply);
    }
    let key = keychain::get()?.ok_or("还没有设置 DeepSeek API key：点左下角「设置」填写")?;
    let client = Client::new(key, data["model"].as_str().unwrap_or(""));
    let mut stream = Stream::new(host, client.model.clone());
    let result = generate::generate(&client, &Request::from_json(data), &mut |p| stream.push(p));
    stream.flush();
    result
}

/// Hands the reply to the shell in pieces at most every 60 ms, so a fast stream is a few events a frame.
struct Stream<'a> {
    host: &'a HostChannel,
    model: String,
    pending: String,
    reasoning: usize,
    sent: Instant,
}

impl<'a> Stream<'a> {
    const EVERY: Duration = Duration::from_millis(60);

    fn new(host: &'a HostChannel, model: String) -> Self {
        Stream { host, model, pending: String::new(), reasoning: 0, sent: Instant::now() }
    }

    fn push(&mut self, progress: Progress) {
        match progress {
            Progress::Turn(n) => {
                self.flush();
                self.reasoning = 0;
                self.status(format!("{} 第 {n} 轮", self.model));
            }
            Progress::Delta(text) => self.pending.push_str(text),
            Progress::Thinking(chars) => self.reasoning = chars,
            Progress::Writing(chars) => {
                if self.sent.elapsed() >= Self::EVERY {
                    self.status(format!("{} 正在写模块（{chars} 字）", self.model));
                }
            }
            // The module being checked replaces what the code panel shows.
            Progress::Draft(tsx) => {
                self.pending.clear();
                let _ = self.host.send("reset", Value::Null);
                let _ = self.host.send("delta", json!({ "text": tsx }));
            }
            Progress::Checked { tool, number, errors } => {
                let text = if errors.is_empty() {
                    format!("第 {number} 次检查通过（{tool}）")
                } else {
                    format!("第 {number} 次检查：{} 个错误，模型在改", errors.len())
                };
                log::info!(target: "genui", "{text}: {errors:?}");
                dev::event("checked", json!({ "tool": tool, "number": number, "errors": errors }));
                self.status(text);
            }
        }
        if self.sent.elapsed() >= Self::EVERY {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.sent = Instant::now();
        if self.reasoning > 0 && self.pending.is_empty() {
            self.status(format!("思考中（{} 字）", self.reasoning));
        }
        if !self.pending.is_empty() {
            let text = std::mem::take(&mut self.pending);
            let _ = self.host.send("delta", json!({ "text": text }));
        }
    }

    fn status(&self, text: String) {
        let _ = self.host.send("status", json!({ "text": text }));
    }
}
