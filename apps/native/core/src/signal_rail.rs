//! Signal rail sentences from Electron's missionView, independent of the window.
use crate::inspector::{SignalRow, SignalTone};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
fn words(s: &str) -> String {
    s.replace(['-', '_'], " ").trim().into()
}
fn label(s: &str, limit: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        flat
    } else {
        format!("{}…", flat.chars().take(limit - 1).collect::<String>())
    }
}
/// skillCalled (missionView.ts, 0.679): the skill a Skill call ran, as the person would name it, or None for any other
/// tool ("" when it named none). Locust hands a folder's skills over as the plugin `project` and the person's own as
/// `personal` (skills.rs), so that prefix is Locust's and goes; anyone else's plugin keeps its name.
pub fn skill_called(tool: &str, target: &str) -> Option<String> {
    if !tool.trim().eq_ignore_ascii_case("skill") {
        return None;
    }
    let target = target.trim();
    Some(target.strip_prefix("project:").or_else(|| target.strip_prefix("personal:")).unwrap_or(target).to_owned())
}

/// skillWords: "Used the review skill", or the live line's "Using the review skill"; "Used a skill" when none named.
pub fn skill_words(skill: &str, live: bool) -> String {
    let verb = if live { "Using" } else { "Used" };
    if skill.is_empty() {
        format!("{verb} a skill")
    } else {
        format!("{verb} the {} skill", label(skill, 40))
    }
}

pub fn tool_name(p: &Value) -> String {
    let name = p["name"].as_str().unwrap_or("");
    let kind = p["toolKind"].as_str().unwrap_or("");
    let target = p["command"]
        .as_str()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let shown = !target.is_empty();
    let file = target
        .split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .next_back()
        .unwrap_or(&target);
    let split = crate::invariants::regex(r"([a-z0-9])([A-Z])")
        .replace_all(name, "$1 $2")
        .to_lowercase();
    let parts = split
        .split(|c: char| !c.is_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    let has = |w: &str| parts.contains(&w);
    let text = if crate::mission_export::is_shell_tool(name, Some(kind))
    {
        if shown {
            format!("Ran {}", crate::shell_words::shell_command_text(&target))
        } else {
            "Ran a command".into()
        }
    } else if let Some(skill) = skill_called(name, if target == name { "" } else { &target }) {
        skill_words(&skill, false)
    } else if name == "file_change" || crate::mission_export::edit_tool_name(name) {
        if shown {
            format!("Changed {file}")
        } else {
            "Changed a file".into()
        }
    } else if has("websearch") || (has("web") && has("search")) {
        if shown && crate::invariants::regex(r"(?i)^https?://\S+$").is_match(&target) {
            format!("Opened {target}")
        } else if shown {
            format!("Searched the web for {target}")
        } else {
            "Searched the web".into()
        }
    } else if ["search", "grep", "find", "glob"].iter().any(|w| has(w)) {
        if shown {
            format!("Searched for {target}")
        } else {
            "Searched".into()
        }
    } else if ["read", "view", "cat", "open"].iter().any(|w| has(w)) {
        if shown {
            format!("Read {file}")
        } else {
            "Read a file".into()
        }
    } else if has("fetch") || has("browse") {
        if shown {
            format!("Opened {target}")
        } else {
            "Opened a page".into()
        }
    } else if crate::invariants::regex(r"(?i)^(image_?generation|image_?gen|generate_?image)$")
        .is_match(name.trim())
    {
        "Made an image".into()
    } else if name.trim().eq_ignore_ascii_case("invalid") {
        "Made a tool call that did not work".into()
    } else if let Some(c) =
        crate::invariants::regex(r"(?i)^locust_git_(status|diff|log|show|blame|branches)$")
            .captures(name.trim())
    {
        format!(
            "Ran git {}",
            if c[1].eq_ignore_ascii_case("branches") {
                "branch".into()
            } else {
                c[1].to_lowercase()
            }
        )
    } else if shown {
        format!("{name} · {target}")
    } else {
        name.into()
    };
    label(&text, 72)
}
pub fn build(events: &[Value], running: bool) -> Vec<SignalRow> {
    let settled: HashMap<_, _> = events
        .iter()
        .filter(|e| matches!(e["type"].as_str(), Some("tool.completed" | "tool.failed")))
        .map(|e| (e["payload"]["itemId"].as_str().unwrap_or(""), e))
        .collect();
    let started: HashSet<_> = events
        .iter()
        .filter(|e| e["type"] == "tool.started")
        .map(|e| e["payload"]["itemId"].as_str().unwrap_or(""))
        .collect();
    let mut rows = Vec::new();
    for (i, e) in events.iter().enumerate() {
        let p = &e["payload"];
        let ty = e["type"].as_str().unwrap_or("");
        let s = |k: &str| p[k].as_str().unwrap_or("");
        let clock = crate::time_marker::clock_of(e["occurredAt"].as_str().unwrap_or(""));
        let mut meta = clock.clone();
        let mut tone = SignalTone::Muted;
        let name = match ty {
            "run.started" => format!(
                "Started on {}",
                crate::inspector::runtime_display_name(e["sourceAdapter"].as_str().unwrap_or(""))
            ),
            "tool.started" | "tool.completed" | "tool.failed" => {
                if p["parentItemId"].as_str().is_some_and(|id| !id.is_empty()) {
                    continue;
                }
                let id = s("itemId");
                if ty != "tool.started" && started.contains(id) {
                    continue;
                }
                let end = if ty == "tool.started" {
                    settled.get(id).copied()
                } else {
                    Some(e)
                };
                let mut name = tool_name(p);
                if let Some(end) = end {
                    let q = &end["payload"];
                    let status = q["status"].as_str().unwrap_or("");
                    if matches!(status, "refused" | "declined") && name.starts_with("Ran ") {
                        name = format!("Did not run {}", &name[4..]);
                    }
                    let outcome = if end["type"] == "tool.failed" {
                        tone = SignalTone::Red;
                        if status == "refused" {
                            "refused".into()
                        } else {
                            "failed".into()
                        }
                    } else {
                        match q["exitCode"].as_i64() {
                            None | Some(0) => "done".into(),
                            Some(c) => format!("exit code {c}"),
                        }
                    };
                    meta = format!("{clock} · {outcome}");
                } else {
                    meta = format!(
                        "{clock} · {}",
                        if running { "running" } else { "did not finish" }
                    );
                    if running {
                        tone = SignalTone::Live;
                    }
                }
                name
            }
            "step.failed" => {
                tone = SignalTone::Red;
                meta = format!("{clock} · {}", words(s("stepKind")));
                "A step failed".into()
            }
            "step.completed" if words(s("stepKind")).contains("reasoning") => "Thought".into(),
            "plan.updated" => {
                tone = SignalTone::Violet;
                let steps = crate::ledger::read_plan(&p["plan"]);
                let done = steps.iter().filter(|s| s.state == "done").count();
                format!(
                    "{}{}",
                    if steps.is_empty() {
                        "Plan updated".into()
                    } else {
                        format!("Plan: {done} of {} done", steps.len())
                    },
                    if p["final"] == true { " · final" } else { "" }
                )
            }
            "route.limit_detected" => {
                tone = SignalTone::Amber;
                meta = label(&format!("{clock} · {}", s("message")), 96);
                format!("Usage limit · {}", words(s("kind")))
            }
            "adapter.diagnostic" => {
                if s("code").ends_with(".usage_window") || s("code") == "host.start-timing" {
                    continue;
                }
                tone = if s("level") == "error" {
                    SignalTone::Red
                } else {
                    SignalTone::Amber
                };
                meta = format!("{clock} · {}", s("level"));
                label(
                    if s("message").trim().is_empty() {
                        "A note from the runtime"
                    } else {
                        s("message")
                    },
                    96,
                )
            }
            "run.completed" => {
                tone = SignalTone::Blue;
                "Finished".into()
            }
            "run.cancelled" => {
                tone = SignalTone::Amber;
                meta = format!("{clock} · by you");
                "Stopped".into()
            }
            "run.failed" => {
                tone = SignalTone::Red;
                meta = label(&format!("{clock} · {}", s("message")), 96);
                format!("Failed · {}", words(s("kind")))
            }
            _ => continue,
        };
        rows.push(SignalRow {
            key: e["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("ev-{i}")),
            name,
            meta,
            tone,
        });
    }
    rows.reverse();
    rows
}
#[cfg(test)]
#[path = "signal_rail_tests.rs"]
mod tests;
