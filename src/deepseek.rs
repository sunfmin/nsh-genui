//! DeepSeek's chat completions (OpenAI's format), streamed.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde_json::{Value, json};

const ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
const MODELS: &str = "https://api.deepseek.com/models";
/// The models the settings offer (DeepSeek's current ids); the first is the default.
pub const MODEL_IDS: [&str; 2] = ["deepseek-v4-pro", "deepseek-flash"];

pub struct Client {
    agent: ureq::Agent,
    key: String,
    pub model: String,
    pub thinking: Thinking,
}

/// How much the model reasons before it answers (DeepSeek's `thinking` / `reasoning_effort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thinking {
    /// No reasoning: `thinking: { type: "disabled" }`.
    Off,
    /// `reasoning_effort: "low"`.
    Low,
    /// The model's default.
    Default,
}

impl Thinking {
    pub const ALL: [Thinking; 3] = [Thinking::Off, Thinking::Low, Thinking::Default];

    pub fn id(self) -> &'static str {
        match self {
            Thinking::Off => "off",
            Thinking::Low => "low",
            Thinking::Default => "default",
        }
    }

    /// From [`Self::id`]; anything else is `None`.
    pub fn from_id(id: &str) -> Option<Thinking> {
        Thinking::ALL.into_iter().find(|t| t.id() == id)
    }
}

/// A piece of the reply as it streams.
pub enum Delta<'a> {
    /// The model's reasoning (reasoning models), not part of the answer.
    Reasoning(&'a str),
    /// The answer.
    Content(&'a str),
}

pub struct Reply {
    pub content: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl Client {
    /// A client for `model` (one of [`MODEL_IDS`], else the default) with the user's `key`.
    pub fn new(key: String, model: &str) -> Client {
        let model = if MODEL_IDS.contains(&model) { model } else { MODEL_IDS[0] };
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(600)))
            .http_status_as_error(false)
            .build()
            .into();
        Client { agent, key, model: model.to_owned(), thinking: Thinking::Default }
    }

    /// Whether DeepSeek accepts the key: lists the models, which costs nothing.
    pub fn verify(&self) -> Result<(), String> {
        let mut response = self
            .agent
            .get(MODELS)
            .header("Authorization", &format!("Bearer {}", self.key))
            .call()
            .map_err(|e| format!("无法连接 DeepSeek：{e}"))?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            401 | 403 => Err("DeepSeek 拒绝了这个 key（401/403），请检查是否填对".into()),
            code => Err(format!(
                "DeepSeek 返回 {code}：{}",
                response.body_mut().read_to_string().unwrap_or_default()
            )),
        }
    }

    /// Sends `messages` and streams the reply into `on_delta`.
    pub fn chat(&self, messages: &[Value], mut on_delta: impl FnMut(Delta)) -> Result<Reply, String> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "temperature": 0.3,
            // Reasoning counts against it: at 8192 a long reasoning left no room for the answer (measured).
            "max_tokens": 32768,
        });
        match self.thinking {
            Thinking::Off => body["thinking"] = json!({ "type": "disabled" }),
            Thinking::Low => body["reasoning_effort"] = json!("low"),
            Thinking::Default => {}
        }
        let mut response = self
            .agent
            .post(ENDPOINT)
            .header("Authorization", &format!("Bearer {}", self.key))
            .send_json(&body)
            .map_err(|e| format!("DeepSeek request failed: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.body_mut().read_to_string().unwrap_or_default();
            return Err(format!("DeepSeek answered {status}: {text}"));
        }

        let mut reply = Reply { content: String::new(), prompt_tokens: 0, completion_tokens: 0 };
        let (mut finish, mut reasoning) = (String::new(), 0usize);
        let reader = BufReader::new(response.body_mut().as_reader());
        for line in reader.lines() {
            let line = line.map_err(|e| format!("DeepSeek stream broke: {e}"))?;
            let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue };
            if data == "[DONE]" {
                break;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(usage) = event.get("usage").filter(|u| !u.is_null()) {
                reply.prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                reply.completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
            }
            if let Some(reason) = event["choices"][0]["finish_reason"].as_str() {
                finish = reason.to_owned();
            }
            let delta = &event["choices"][0]["delta"];
            if let Some(text) = delta["reasoning_content"].as_str() {
                reasoning += text.chars().count();
                on_delta(Delta::Reasoning(text));
            }
            if let Some(text) = delta["content"].as_str() {
                reply.content.push_str(text);
                on_delta(Delta::Content(text));
            }
        }
        if reply.content.is_empty() {
            return Err(format!(
                "DeepSeek returned an empty answer (finish_reason {finish:?}, {reasoning} characters of reasoning, \
                 {} completion tokens)",
                reply.completion_tokens
            ));
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Network: run with `cargo test -- --ignored`. A wrong key costs nothing.
    #[test]
    #[ignore]
    fn a_wrong_key_is_refused_when_saved() {
        let err = Client::new("sk-0000000000000000".into(), "").verify().unwrap_err();
        assert!(err.contains("401"), "{err}");
    }

    #[test]
    #[ignore]
    fn a_wrong_key_fails_a_chat_with_deepseeks_answer() {
        let messages = [json!({ "role": "user", "content": "hi" })];
        let err = Client::new("sk-0000000000000000".into(), "").chat(&messages, |_| {}).err().unwrap();
        assert!(err.starts_with("DeepSeek answered 401"), "{err}");
    }
}
