//! Test for a model of your own (Electron `testOwnEndpoint` in `main/own-models.ts`): ask the endpoint what it serves,
//! then whether the model takes tools, with one request carrying one tool and an answer capped at a single token. The
//! list gets 8 seconds and the one-word request a minute (a model on a CPU answers slowly). What happened is said in
//! words, never left out.
//!
//! The requests go through `Fetch`. This computer's is Apple's `curl`, which reads the whole request (address,
//! headers with the key, body) from its standard input, so the key is never in its arguments.

use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Answered { status: u16, status_text: String, body: String },
    TimedOut,
    Failed(String),
}

pub trait Fetch: Send + Sync {
    fn request(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>, timeout: Duration) -> Outcome;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tested {
    pub ok: bool,
    pub said: String,
    /// Whether the model takes tools, when Test could tell.
    pub tools: Option<bool>,
}

pub const TESTING: &str = "Testing. A model on this computer may take up to a minute to answer.";

fn seconds(d: Duration) -> u64 {
    (d.as_millis() as f64 / 1000.0).round() as u64
}

fn listed_models(base_url: &str, model: &str, key: Option<&str>, fetch: &dyn Fetch, timeout: Duration) -> (bool, String) {
    let mut headers = vec![("anthropic-version", "2023-06-01".to_owned())];
    if let Some(key) = key {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    match fetch.request("GET", &format!("{base_url}/models"), &headers, None, timeout) {
        Outcome::Answered { status: 401 | 403, .. } => (false, "It answered, and refused the key.".into()),
        Outcome::Answered { status, status_text, .. } if !(200..300).contains(&status) => (false, format!("It answered {status} {status_text}.").replace(" .", ".")),
        Outcome::Answered { body, .. } => {
            let ids: Vec<String> = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["data"].as_array().cloned()).unwrap_or_default().iter().filter_map(|e| e["id"].as_str().map(str::to_owned)).collect();
            if ids.is_empty() {
                (true, "It answered. It did not list its models, so the model name could not be checked.".into())
            } else if ids.iter().any(|id| id == model) {
                (true, format!("It answered, and serves {model}."))
            } else {
                (false, format!("It answered, but does not list {model}. It lists {}{}.", ids.iter().take(5).cloned().collect::<Vec<_>>().join(", "), if ids.len() > 5 { ", \u{2026}" } else { "" }))
            }
        }
        Outcome::TimedOut => (false, format!("No answer in {} seconds.", seconds(timeout))),
        Outcome::Failed(why) => (false, format!("It could not be reached: {why}.")),
    }
}

enum Tools {
    Yes,
    No,
    Unknown(String),
}

fn takes_tools(base_url: &str, model: &str, key: Option<&str>, fetch: &dyn Fetch, timeout: Duration) -> Tools {
    let mut headers = vec![("content-type", "application/json".to_owned())];
    if let Some(key) = key {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    let body = json!({"model": model, "messages": [{"role": "user", "content": "Reply with OK."}], "max_tokens": 1,
        "tools": [{"type": "function", "function": {"name": "noop", "description": "Does nothing.", "parameters": {"type": "object", "properties": {}}}}]})
    .to_string();
    match fetch.request("POST", &format!("{base_url}/chat/completions"), &headers, Some(&body), timeout) {
        Outcome::Answered { status, .. } if (200..300).contains(&status) => Tools::Yes,
        Outcome::Answered { status: 400 | 422 | 501, body, .. } if body.to_lowercase().contains("tool") => Tools::No,
        Outcome::Answered { status, status_text, .. } => Tools::Unknown(format!("A one-word request got {} back", format!("{status} {status_text}").trim())),
        Outcome::TimedOut => {
            let n = seconds(timeout);
            Tools::Unknown(format!("It did not answer a one-word request in {n} second{}", if n == 1 { "" } else { "s" }))
        }
        Outcome::Failed(why) => Tools::Unknown(format!("A one-word request to it failed ({why})")),
    }
}

/// Test with Electron's bounds: 8 seconds for the list, a minute for the one-word request.
pub fn test_own_endpoint(base_url: &str, model: &str, key: Option<&str>, fetch: &dyn Fetch) -> Tested {
    test_with(base_url, model, key, fetch, Duration::from_secs(8), Duration::from_secs(60))
}

pub fn test_with(base_url: &str, model: &str, key: Option<&str>, fetch: &dyn Fetch, list_timeout: Duration, chat_timeout: Duration) -> Tested {
    let (ok, said) = listed_models(base_url, model, key, fetch, list_timeout);
    if !ok {
        return Tested { ok, said, tools: None };
    }
    match takes_tools(base_url, model, key, fetch, chat_timeout) {
        Tools::Yes => Tested { ok: true, said: format!("{said} It can use tools."), tools: Some(true) },
        Tools::No => Tested { ok: true, said: format!("{said} It cannot use tools, so it is set to chat only."), tools: Some(false) },
        Tools::Unknown(why) => Tested { ok: false, said: format!("{said} {why}, so Test could not tell whether it can use tools."), tools: None },
    }
}

/// Apple's curl, its request on standard input (`-K -`): the key never reaches its arguments.
pub struct Curl;

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

impl Fetch for Curl {
    fn request(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>, timeout: Duration) -> Outcome {
        use std::io::Write;
        if cfg!(test) {
            return Outcome::Failed("no network in a test".into());
        }
        let mut config = format!("url = {}\nrequest = {}\nmax-time = {}\nsilent\nshow-error\nwrite-out = \"\\n%{{http_code}}\"\n", quoted(url), quoted(method), timeout.as_secs().max(1));
        for (name, value) in headers {
            config.push_str(&format!("header = {}\n", quoted(&format!("{name}: {value}"))));
        }
        if let Some(body) = body {
            config.push_str(&format!("data = {}\n", quoted(body)));
        }
        let child = std::process::Command::new("/usr/bin/curl")
            .args(["-K", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return Outcome::Failed(e.to_string()),
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(config.as_bytes());
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return Outcome::Failed(e.to_string()),
        };
        // curl's own word for a request that ran out of time.
        if out.status.code() == Some(28) {
            return Outcome::TimedOut;
        }
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
        match code.trim().parse::<u16>() {
            Ok(status) if status > 0 => Outcome::Answered { status, status_text: String::new(), body: body.to_owned() },
            _ => Outcome::Failed(String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_owned()),
        }
    }
}

#[cfg(test)]
#[path = "endpoint_test_tests.rs"]
mod tests;
