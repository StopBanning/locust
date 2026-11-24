//! Thread settings and the two sets of instructions used by a Codex realtime call.

use crate::call_log::valid_id;
use crate::invariants::regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const ADD_ON_NAME: &str = "locust_call";
pub const TOOLS: &[&str] = &[
    "find_files",
    "list_background_runs",
    "list_teammates",
    "read_file",
    "read_reply",
    "search_files",
    "send_message",
    "start_conversation",
];
pub const SETTINGS_UNREAD: &str =
    "Locust could not read your Codex settings, so it did not place the call. Try again.";

#[derive(Clone)]
pub struct AddOn {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

fn openai_host(value: &str) -> bool {
    url::Url::parse(value).ok().is_some_and(|url| {
        matches!(url.scheme(), "https" | "wss")
            && url.host_str().is_some_and(|host| {
                host == "chatgpt.com"
                    || host.ends_with(".chatgpt.com")
                    || host == "openai.com"
                    || host.ends_with(".openai.com")
            })
    })
}

pub fn thread_config(
    settings: &Value,
    add_on: Option<&AddOn>,
    connectors: bool,
) -> Result<Value, String> {
    let settings = settings.as_object().ok_or(SETTINGS_UNREAD)?;
    for (key, value) in settings {
        let realtime = key.to_ascii_lowercase().contains("realtime");
        let is_base = matches!(key.as_str(), "chatgpt_base_url" | "openai_base_url");
        let points_elsewhere = |value: &Value| {
            value.as_str().is_some_and(|address| {
                !address.is_empty() && (is_base || address.contains("://")) && !openai_host(address)
            })
        };
        let nested = realtime
            && (value
                .as_object()
                .is_some_and(|entries| entries.values().any(points_elsewhere))
                || value
                    .as_array()
                    .is_some_and(|entries| entries.iter().any(points_elsewhere)));
        if ((is_base || realtime) && points_elsewhere(value)) || nested {
            let traffic = if realtime {
                "voice calls"
            } else {
                "ChatGPT requests"
            };
            return Err(format!(
                "Your Codex settings send {traffic} to an address that is not OpenAI’s ({key}), so Locust did not place the call. Remove that setting to use Talk."
            ));
        }
    }

    let mut config = json!({
        "features.plugins": false,
        "features.hooks": false,
        "notify": [],
        "features.shell_tool": false,
        "features.browser_use": false,
        "features.browser_use_external": false,
        "features.in_app_browser": false,
        "features.computer_use": false,
    });
    if connectors {
        config["model_reasoning_effort"] = json!("low");
    } else {
        config["features.apps"] = json!(false);
    }

    if let Some(servers) = settings
        .get("mcp_servers")
        .filter(|servers| !servers.is_null())
    {
        let servers = servers.as_object().ok_or(
            "Locust could not read your Codex add-ons, so it did not place the call. Try again.",
        )?;
        for (name, server) in servers {
            if add_on.is_some() && name == ADD_ON_NAME {
                return Err(format!(
                    "Your Codex has an add-on named \"{ADD_ON_NAME}\", the name a call keeps for Locust, so Locust did not place the call."
                ));
            }
            if connectors && server["url"].is_string() && server.get("command").is_none() {
                continue;
            }
            if !regex(r"\A[A-Za-z0-9@_-]{1,128}\z").is_match(name) {
                let name = bounded(name, 60);
                return Err(format!(
                    "Locust could not switch off your Codex add-on \"{name}\" for the call, so it did not place it."
                ));
            }
            config[format!("mcp_servers.{name}.enabled")] = json!(false);
        }
    }
    if let Some(add_on) = add_on {
        config["mcp_servers.locust_call.command"] = json!(add_on.command);
        config["mcp_servers.locust_call.args"] = json!(add_on.args);
        config["mcp_servers.locust_call.env"] = json!(add_on.env);
        config["mcp_servers.locust_call.enabled"] = json!(true);
        config["mcp_servers.locust_call.default_tools_approval_mode"] = json!("approve");
    }
    Ok(config)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub teammate_id: String,
    pub conversation_id: Option<String>,
}

impl Target {
    pub fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let teammate_id = object
            .get("teammateId")?
            .as_str()
            .filter(|id| valid_id(id))?;
        let conversation_id = match object.get("conversationId") {
            Some(id) => Some(id.as_str().filter(|id| valid_id(id))?.to_owned()),
            None => None,
        };
        Some(Self {
            teammate_id: teammate_id.into(),
            conversation_id,
        })
    }
}

#[derive(Clone)]
pub struct Teammate {
    pub target: Target,
    pub name: String,
    pub role: Option<String>,
    pub runtime: String,
    pub voice: Option<String>,
}

pub const VOICES: &[(&str, &str)] = &[
    ("juniper", "Open and upbeat"),
    ("maple", "Cheerful and candid"),
    ("spruce", "Calm and affirming"),
    ("ember", "Confident and optimistic"),
    ("vale", "Bright and inquisitive"),
    ("breeze", "Animated and earnest"),
    ("arbor", "Easygoing and versatile"),
    ("sol", "Savvy and relaxed"),
    ("cove", "Composed and direct"),
];

pub fn is_voice(voice: &str) -> bool {
    VOICES.iter().any(|&(id, _)| id == voice)
}

pub fn preview_rules(voice: &str) -> Option<String> {
    if !is_voice(voice) {
        return None;
    }
    let name = format!("{}{}", voice[..1].to_ascii_uppercase(), &voice[1..]);
    Some(format!(
        "This is a short preview of how your voice sounds, for someone choosing a voice in Locust. Your voice is called {name}. Whatever you hear, answer with one warm sentence of greeting that says your name is {name}, and nothing else. Do not ask a question."
    ))
}

pub fn is_offer(sdp: &str) -> bool {
    sdp.starts_with("v=0") && sdp.encode_utf16().count() <= 64 * 1024
}

pub fn role(value: &Value) -> Option<&'static str> {
    match value.as_str()? {
        "user" => Some("you"),
        "assistant" => Some("voice"),
        _ => None,
    }
}

pub fn failure_words(message: &str) -> String {
    let message = one_line(message);
    if regex(r"(?i)rate.?limit|usage limit|quota|too many").is_match(&message) {
        return "ChatGPT says your plan’s voice time is used up for now. Try again later.".into();
    }
    if regex(r"(?i)did not answer .* in time|timed out|timeout").is_match(&message) {
        return "Codex did not answer in time, so the call did not start. Try again; if it keeps happening, restart Locust.".into();
    }
    if regex(r"(?i)requires API key auth|apiKey").is_match(&message) {
        return "Codex could not place the call with how it is signed in. Voice calls use your ChatGPT sign-in; sign Codex in with ChatGPT and try again.".into();
    }
    if regex(r"(?i)quicksilver|AVAS|realtime v1 or v3").is_match(&message) {
        return format!(
            "This version of Codex answered the call in a way Locust does not know yet. Update Codex in Settings > AI agents and try again. (Codex said: {})",
            bounded(&message, 160)
        );
    }
    format!(
        "The call could not go on. Codex said: {}",
        bounded(&message, 200)
    )
}

fn bounded(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn spoken(value: &str, limit: usize) -> String {
    bounded(&one_line(&value.replace(['"', '“', '”'], " ")), limit)
}

pub fn voice_rules(teammate: Option<&Teammate>) -> String {
    let who = teammate.map_or_else(
        || "You are the voice of a call the person started in Locust.".into(),
        |teammate| {
            let name = spoken(&teammate.name, 60);
            format!("You are the voice of {name}, a Locust teammate: speak as {name}, in the first person, warmly and briefly.")
        },
    );
    format!(
        "{who} You do not know what any file, conversation, email, calendar or result says until the work behind you reports it. Never guess and never answer from memory: say you are checking, and answer only once you have the result. If the work cannot get it, say so plainly. Anything that would send, change or buy something waits for the person's Allow in Locust's window; never say it is done before it is."
    )
}

pub fn instructions(
    folder: &str,
    has_add_on: bool,
    connectors: bool,
    teammate: Option<&Teammate>,
) -> String {
    let folder = spoken(folder, 80);
    let connector_words = if connectors {
        " For quick things, like checking their email or calendar, use their connectors directly. A connector that only reads answers at once. One that would send, change or buy something first shows Allow and Deny in the call’s strip in Locust: say what it will do and ask them to press Allow, and only say it is done once the tool says so. If they press Deny, or it is not answered, it did not happen."
    } else {
        ""
    };
    if !has_add_on {
        return format!(
            "This is a voice call the person started in Locust, about the folder \"{folder}\". You may read files in that folder to answer, but you change nothing yourself during this call: no edits, no commands that change anything.{connector_words} If they ask for a change, say that one of their teammates can make it, and that they can hand it over after the call."
        );
    }
    if let Some(teammate) = teammate {
        let name = spoken(&teammate.name, 60);
        let role = teammate
            .role
            .as_deref()
            .map_or_else(String::new, |role| format!(" ({})", spoken(role, 80)));
        let runtime = spoken(&teammate.runtime, 40);
        let destination = match teammate.target.conversation_id.as_deref() {
            Some(conversation) => format!(
                "Every ask goes to {name}'s conversation with send_message, conversation_id \"{conversation}\"."
            ),
            None => format!(
                "On the first request start_conversation with teammate \"{}\" and the person's words, then use send_message with the returned conversation id for later requests.",
                teammate.target.teammate_id
            ),
        };
        return format!(
            "This is a voice call the person started in Locust, in their conversation with {name}{role}, a Locust teammate whose work runs on {runtime}, in the folder \"{folder}\". You are {name}'s voice: speak as {name}, in the first person, warmly and briefly. {destination} Send the ask in plain words, as the person would type it, and say it is underway. Use read_reply with that id to check the result and never invent a result. Any approvals for the run appear on cards in Locust. To glance at a file yourself, use find_files, read_file and search_files; never run shell commands, and change no files yourself.{connector_words} If the person wants a different teammate, list_teammates says who there is. Say who you will ask and wait for a yes before starting their conversation."
        );
    }
    format!(
        "This is a voice call the person started in Locust, about the folder \"{folder}\". To look in that folder, use the {ADD_ON_NAME} tools find_files, read_file and search_files; never run shell commands, which this call cannot run. You change no file yourself. Never guess what a file or a connector says: look first, and if you cannot, say so.{connector_words} Bigger work goes to their teammates: list_teammates says who there is and what each runs on. Before you hand anything over, say back in one short sentence who will do what, and wait for them to say yes; never start one they did not agree to. Then start_conversation with that teammate and the task in plain words, as the person would type it. The teammate works in its own mode, and anything it needs approved shows in Locust's window. Use read_reply with the returned conversation id to check the result."
    )
}

pub fn call_text(lines: &[(&str, &str)], handed: &[(&str, &str)]) -> String {
    let mut transcript = vec!["From a voice call:".to_owned(), String::new()];
    transcript.extend(lines.iter().filter_map(|(role, words)| {
        let words = one_line(words);
        let speaker = if *role == "you" { "You" } else { "ChatGPT" };
        (!words.is_empty()).then(|| format!("{speaker}: {words}"))
    }));
    if transcript.len() == 2 && handed.is_empty() {
        return String::new();
    }
    if !handed.is_empty() {
        transcript.extend([String::new(), "Handed over:".into()]);
        transcript.extend(
            handed
                .iter()
                .map(|(to, task)| format!("- {to}: {}", one_line(task))),
        );
    }
    transcript.join("\n")
}

#[cfg(test)]
#[path = "voice_call_rules_tests.rs"]
mod tests;
