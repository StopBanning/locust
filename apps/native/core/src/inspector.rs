//! Reply route, provenance, permissions, artifacts, receipt, and recovery inspector
//! (Electron `renderer/src/components/Inspector.tsx`, `shared/may-do.ts`, `renderer/src/status.ts`,
//! `renderer/src/cost.ts`, `renderer/src/missionView.ts`; PARITY row 26).

use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MayDoRow {
    pub verdict: &'static str, // "allow" | "deny"
    pub wide: bool,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MayDo {
    pub rows: Vec<MayDoRow>,
    pub rest: String,
}

pub fn runtime_display_name(runtime: &str) -> &str {
    match runtime {
        "codex" => "Codex",
        "claude" => "Claude Code",
        "cursor" => "Cursor Agent",
        "opencode" => "OpenCode",
        "copilot" => "Copilot CLI",
        "muse" => "Muse Code",
        "kilo" => "Kilo",
        "antigravity" => "Antigravity",
        "gemini" => "Gemini CLI",
        "acp" => "Custom agent",
        other => other,
    }
}

/// Exact port of `apps/desktop/src/renderer/src/status.ts` `sandboxPhrase`.
pub fn sandbox_phrase(sandbox: Option<&str>) -> &'static str {
    match sandbox {
        Some("full-access") => "may edit anything on this machine",
        Some("workspace-write") => "may edit the workspace",
        _ => "read-only",
    }
}

/// Exact port of `apps/desktop/src/renderer/src/status.ts` `accountPhrase`.
pub fn account_phrase(runtime: &str, model: &str, resolved_route_id: &str) -> String {
    if resolved_route_id == "antigravity:hub" {
        return "through the Antigravity app".into();
    }
    if model.to_ascii_lowercase().ends_with("-free") {
        return "none — a free model".into();
    }
    format!("your {} sign-in", runtime_display_name(runtime))
}

/// Exact port of `apps/desktop/src/renderer/src/status.ts` `shortMissionId`.
pub fn short_mission_id(mission_id: &str) -> String {
    let bare = mission_id.strip_prefix("mission_").unwrap_or(mission_id);
    bare.chars().take(8).collect()
}

/// Exact port of `apps/desktop/src/renderer/src/status.ts` `checkpointLabel`.
pub fn checkpoint_label(epoch: u64) -> String {
    format!("ck_{epoch}")
}

/// Exact port of `apps/desktop/src/renderer/src/status.ts` `ledgerVerificationLabel`.
pub fn ledger_verification_label(integrity_issue_count: usize) -> &'static str {
    if integrity_issue_count == 0 {
        "record readable"
    } else {
        "record incomplete"
    }
}

/// Exact port of `apps/desktop/src/renderer/src/components/Inspector.tsx` `noArtifactsLine`.
pub fn no_artifacts_line(sandbox: Option<&str>, running: bool, watched: bool) -> &'static str {
    if running {
        return "Files this reply changes appear here as it works.";
    }
    if sandbox == Some("read-only") {
        return "This reply could only read, so it changed no files.";
    }
    if watched {
        return "This reply changed no files.";
    }
    "No changed files were reported. A file a command wrote may not be listed here."
}

fn allow(text: impl Into<String>) -> MayDoRow {
    MayDoRow {
        verdict: "allow",
        wide: false,
        text: text.into(),
    }
}

fn allow_wide(text: impl Into<String>) -> MayDoRow {
    MayDoRow {
        verdict: "allow",
        wide: true,
        text: text.into(),
    }
}

fn deny(text: impl Into<String>) -> MayDoRow {
    MayDoRow {
        verdict: "deny",
        wide: false,
        text: text.into(),
    }
}

/// Exact port of `apps/desktop/src/shared/may-do.ts` `whatItMayDo`.
pub fn what_it_may_do(runtime: &str, sandbox: &str, mode: Option<&str>) -> MayDo {
    let read = allow("read the files in this folder");
    let no_changes = deny("changing any file");
    let changes_here = allow("change files in this folder");
    let changes_anywhere =
        allow_wide("change files anywhere your account can reach, in this folder and outside it");
    let any_command = allow_wide("run any command your account can run, and reach any network");
    let no_commands = deny("running commands");
    let open_commands = allow_wide("run commands, which can reach anything your account can");
    let the_web = allow_wide("search the web and open web pages");
    let claude_skills = allow(
        "use this folder's skills, yours when Settings lends them, and the ones you kept from GitHub, with only the tools listed here",
    );
    let not_outside = deny("opening files outside this folder, other than by running a command");
    let web_only_in_auto = |rt: &str| {
        deny(format!(
            "searching the web or opening web pages, which {} allows here only in Auto",
            runtime_display_name(rt)
        ))
    };

    let rest = format!(
        "Anything not listed is left to {}'s own settings.",
        runtime_display_name(runtime)
    );
    let files = match sandbox {
        "read-only" => no_changes,
        "workspace-write" => changes_here.clone(),
        _ => changes_anywhere,
    };
    let rows_with = |own: Vec<MayDoRow>| -> MayDo {
        let mut rows = vec![read.clone(), files.clone()];
        rows.extend(own);
        MayDo {
            rows,
            rest: rest.clone(),
        }
    };
    let asks = mode == Some("approve-each") && sandbox == "workspace-write";

    match runtime {
        "codex" => {
            if sandbox == "full-access" {
                return rows_with(vec![any_command, the_web]);
            }
            if asks {
                return MayDo {
                    rows: vec![
                        read,
                        allow("change files in this folder, once you approve each change"),
                        allow("run commands in Codex's sandbox, asking you before any that does more than read"),
                        allow_wide("search the web and open web pages, without asking"),
                    ],
                    rest,
                };
            }
            let cmd = if sandbox == "read-only" {
                allow("run commands in Codex's sandbox, which lets them read and change nothing")
            } else {
                allow("run commands in Codex's sandbox, which keeps their changes in this folder")
            };
            let mut own = vec![cmd, the_web];
            if sandbox == "read-only" {
                own.push(deny(
                    "starting the MCP servers and plugins your Codex runs as programs on this computer, or its own browser and computer use",
                ));
            }
            rows_with(own)
        }
        "claude" => {
            if sandbox == "full-access" {
                return rows_with(vec![any_command, the_web]);
            }
            rows_with(vec![
                if sandbox == "read-only" {
                    no_commands
                } else {
                    open_commands
                },
                allow_wide("search the web and open web pages, once you approve each one"),
                claude_skills,
            ])
        }
        "opencode" => {
            if sandbox == "full-access" {
                return rows_with(vec![any_command]);
            }
            if asks {
                return MayDo {
                    rows: vec![
                        read,
                        allow("change files in this folder, once you approve each change"),
                        allow_wide("run commands and open web pages, once you approve each one"),
                        allow_wide("search the web"),
                        allow_wide("open files outside this folder, once you approve it"),
                        allow_wide("use tools from your own MCP servers, once you approve each one"),
                    ],
                    rest,
                };
            }
            if sandbox == "read-only" {
                return rows_with(vec![
                    no_commands,
                    the_web,
                    not_outside,
                    deny("using tools from your own MCP servers"),
                ]);
            }
            rows_with(vec![open_commands, the_web, not_outside])
        }
        "copilot" => {
            if sandbox == "full-access" {
                return rows_with(vec![any_command, the_web]);
            }
            if sandbox == "read-only" {
                return rows_with(vec![
                    no_commands,
                    the_web,
                    deny("starting the MCP servers in your Copilot settings"),
                ]);
            }
            rows_with(vec![open_commands, the_web, not_outside])
        }
        "acp" => MayDo {
            rows: vec![
                read,
                allow("change files and run commands when it asks and you approve; which actions it asks about is the agent's own choice"),
            ],
            rest,
        },
        "kilo" => {
            if sandbox == "full-access" {
                return rows_with(vec![any_command, the_web]);
            }
            if sandbox == "read-only" {
                return rows_with(vec![no_commands, the_web]);
            }
            rows_with(vec![open_commands, the_web, not_outside])
        }
        "muse" => {
            if sandbox == "read-only" {
                return rows_with(vec![no_commands]);
            }
            if sandbox == "full-access" {
                return rows_with(vec![any_command, the_web]);
            }
            MayDo {
                rows: vec![
                    read,
                    changes_here,
                    allow("run commands inside Muse's sandbox"),
                    the_web,
                ],
                rest,
            }
        }
        "cursor" => {
            if sandbox == "read-only" {
                return rows_with(vec![
                    web_only_in_auto(runtime),
                    deny("starting the MCP servers this folder names"),
                ]);
            }
            if sandbox == "full-access" {
                rows_with(vec![any_command, the_web])
            } else {
                rows_with(vec![web_only_in_auto(runtime)])
            }
        }
        "antigravity" => {
            if sandbox == "full-access" {
                rows_with(vec![any_command, the_web])
            } else {
                rows_with(vec![web_only_in_auto(runtime)])
            }
        }
        _ => rows_with(vec![]),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalTone {
    Muted,
    Green,
    Amber,
    Red,
    Live,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalRow {
    pub key: String,
    pub name: String,
    pub meta: String,
    pub tone: SignalTone,
}

fn clock_of(iso: &str) -> String {
    if let Some(t) = iso.find('T')
        && iso.len() >= t + 9
    {
        return iso[t + 1..t + 9].to_owned();
    }
    iso.to_owned()
}

fn tool_name(payload: &Value) -> String {
    let kind = payload["toolKind"].as_str().unwrap_or("tool");
    let raw = payload["title"]
        .as_str()
        .or_else(|| payload["command"].as_str())
        .or_else(|| payload["toolName"].as_str())
        .unwrap_or(kind);
    let first = raw.lines().next().unwrap_or(raw).trim();
    if first.chars().count() > 64 {
        format!("{}…", first.chars().take(63).collect::<String>())
    } else if first.is_empty() {
        kind.to_owned()
    } else {
        first.to_owned()
    }
}

pub fn build_signal_rail(events: &[Value], running: bool) -> Vec<SignalRow> {
    let mut settled_by: HashMap<String, &Value> = HashMap::new();
    let mut started: HashSet<String> = HashSet::new();
    for event in events {
        let ty = event["type"].as_str().unwrap_or("");
        if ty == "tool.completed" || ty == "tool.failed" {
            if let Some(id) = event["payload"]["itemId"].as_str() {
                settled_by.insert(id.to_owned(), event);
            }
        } else if ty == "tool.started"
            && let Some(id) = event["payload"]["itemId"].as_str()
        {
            started.insert(id.to_owned());
        }
    }

    let mut rows = Vec::new();
    for (idx, event) in events.iter().enumerate() {
        let ty = event["type"].as_str().unwrap_or("");
        let key = event["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("ev-{idx}"));
        let clock = clock_of(event["occurredAt"].as_str().unwrap_or(""));
        let payload = &event["payload"];

        match ty {
            "run.started" => rows.push(SignalRow {
                key,
                name: format!(
                    "Started on {}",
                    runtime_display_name(event["sourceAdapter"].as_str().unwrap_or("runtime"))
                ),
                meta: clock,
                tone: SignalTone::Muted,
            }),
            "tool.started" => {
                if payload["helper"].is_object() || payload["helperId"].is_string() {
                    continue;
                }
                let item_id = payload["itemId"].as_str().unwrap_or("");
                let name = tool_name(payload);
                if let Some(settled) = settled_by.get(item_id) {
                    let st = settled["type"].as_str().unwrap_or("");
                    if st == "tool.failed" {
                        let status = settled["payload"]["status"].as_str();
                        let label = if status == Some("refused") {
                            "refused"
                        } else {
                            "failed"
                        };
                        rows.push(SignalRow {
                            key,
                            name,
                            meta: format!("{clock} · {label}"),
                            tone: SignalTone::Red,
                        });
                    } else {
                        let code = settled["payload"]["exitCode"].as_i64();
                        let meta = match code {
                            None | Some(0) => format!("{clock} · done"),
                            Some(c) => format!("{clock} · exit code {c}"),
                        };
                        rows.push(SignalRow {
                            key,
                            name,
                            meta,
                            tone: SignalTone::Muted,
                        });
                    }
                } else {
                    rows.push(SignalRow {
                        key,
                        name,
                        meta: format!(
                            "{clock} · {}",
                            if running { "running" } else { "did not finish" }
                        ),
                        tone: if running {
                            SignalTone::Live
                        } else {
                            SignalTone::Muted
                        },
                    });
                }
            }
            "tool.completed" | "tool.failed" => {
                if payload["helper"].is_object() || payload["helperId"].is_string() {
                    continue;
                }
                let item_id = payload["itemId"].as_str().unwrap_or("");
                if started.contains(item_id) {
                    continue;
                }
                let name = tool_name(payload);
                if ty == "tool.failed" {
                    let label = if payload["status"].as_str() == Some("refused") {
                        "refused"
                    } else {
                        "failed"
                    };
                    rows.push(SignalRow {
                        key,
                        name,
                        meta: format!("{clock} · {label}"),
                        tone: SignalTone::Red,
                    });
                } else {
                    let code = payload["exitCode"].as_i64();
                    let meta = match code {
                        None | Some(0) => format!("{clock} · done"),
                        Some(c) => format!("{clock} · exit code {c}"),
                    };
                    rows.push(SignalRow {
                        key,
                        name,
                        meta,
                        tone: SignalTone::Muted,
                    });
                }
            }
            "step.failed" => {
                let kind = payload["stepKind"].as_str().unwrap_or("step");
                rows.push(SignalRow {
                    key,
                    name: "A step failed".into(),
                    meta: format!("{clock} · {kind}"),
                    tone: SignalTone::Red,
                });
            }
            "approval.requested" => rows.push(SignalRow {
                key,
                name: "Asked for approval".into(),
                meta: clock,
                tone: SignalTone::Amber,
            }),
            "approval.resolved" => rows.push(SignalRow {
                key,
                name: "Approval answered".into(),
                meta: clock,
                tone: SignalTone::Muted,
            }),
            "runtime.error" => {
                let msg = payload["message"].as_str().unwrap_or("Runtime error");
                rows.push(SignalRow {
                    key,
                    name: msg.to_owned(),
                    meta: clock,
                    tone: SignalTone::Red,
                });
            }
            "run.completed" => rows.push(SignalRow {
                key,
                name: "Reply finished".into(),
                meta: clock,
                tone: SignalTone::Green,
            }),
            "run.failed" => {
                let msg = payload["message"]
                    .as_str()
                    .or_else(|| payload["reason"].as_str())
                    .unwrap_or("Run failed");
                rows.push(SignalRow {
                    key,
                    name: msg.to_owned(),
                    meta: clock,
                    tone: SignalTone::Red,
                });
            }
            "run.cancelled" => rows.push(SignalRow {
                key,
                name: "Run stopped".into(),
                meta: clock,
                tone: SignalTone::Amber,
            }),
            _ => {}
        }
    }
    rows
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRow {
    pub path: String,
    pub shown: String,
    pub status: Option<String>,
}

fn relative_path(path: &str, workspace: Option<&str>) -> String {
    let norm = path.replace('\\', "/");
    if let Some(ws) = workspace {
        let ws_norm = ws.replace('\\', "/");
        let ws_trim = ws_norm.trim_end_matches('/');
        if let Some(rest) = norm.strip_prefix(&format!("{ws_trim}/")) {
            return rest.to_owned();
        }
    }
    norm
}

pub fn produced_files(events: &[Value], workspace_path: Option<&str>) -> Vec<ArtifactRow> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for event in events {
        let ty = event["type"].as_str().unwrap_or("");
        if ty != "tool.completed" {
            continue;
        }
        let payload = &event["payload"];
        if let Some(changes) = payload["changes"].as_array() {
            for ch in changes {
                if let Some(path) = ch["path"].as_str() {
                    let shown = relative_path(path, workspace_path);
                    let key = shown.trim_start_matches("./").to_ascii_lowercase();
                    if seen.insert(key) {
                        out.push(ArtifactRow {
                            path: path.to_owned(),
                            shown,
                            status: ch["kind"]
                                .as_str()
                                .or_else(|| ch["status"].as_str())
                                .map(str::to_owned),
                        });
                    }
                }
            }
        }
    }
    out
}

pub fn cost_label_and_line(events: &[Value], running: bool) -> (&'static str, String) {
    let mut usd: Option<f64> = None;
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    for event in events {
        let usage = &event["payload"]["usage"];
        if usage.is_object() {
            if let Some(c) = usage["costUsd"].as_f64() {
                usd = Some(usd.unwrap_or(0.0) + c);
            }
            if let Some(i) = usage["inputTokens"].as_u64() {
                input_tokens = input_tokens.max(i);
            }
            if let Some(o) = usage["outputTokens"].as_u64() {
                output_tokens = output_tokens.max(o);
            }
        }
    }
    if let Some(cost) = usd {
        return ("Cost", format!("${cost:.4}"));
    }
    if input_tokens > 0 || output_tokens > 0 {
        return (
            "Usage",
            format!("{input_tokens} in / {output_tokens} out tokens"),
        );
    }
    (
        "Usage",
        if running {
            "reported when the run ends".into()
        } else {
            "not reported by the runtime".into()
        },
    )
}

#[derive(Clone, Debug)]
pub struct TurnInput {
    pub mission_id: String,
    pub prompt: String,
    pub runtime: String,
    pub cli_version: Option<String>,
    pub model: String,
    pub resolved_route_id: String,
    pub sandbox: String,
    pub mode: Option<String>,
    pub continues_reason: Option<String>,
    pub started_by_kind: Option<String>,
    pub phase: String,
    pub running: bool,
    pub events: Vec<Value>,
    pub checkpoints_count: usize,
    pub last_checkpoint_epoch: Option<u64>,
    pub integrity_issue_count: usize,
    pub recovery: Option<crate::recovery::Offer>,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub mission_id: String,
    pub prompt: String,
    pub runtime: String,
    pub runtime_label: String,
    pub cli_version: Option<String>,
    pub model: String,
    pub account: String,
    pub sandbox: String,
    pub sandbox_label: &'static str,
    pub phase: String,
    pub running: bool,
    pub event_count: usize,
    pub checkpoints_count: usize,
    pub last_checkpoint_epoch: Option<u64>,
    pub integrity_issue_count: usize,
    pub ledger_label: &'static str,
    pub cost_label: &'static str,
    pub cost_line: String,
    pub may_do: Option<MayDo>,
    pub signal_rail: Vec<SignalRow>,
    pub artifacts: Vec<ArtifactRow>,
    pub recovery_note: Option<String>,
}

pub fn build_run_report(
    input: &TurnInput,
    next_turn: Option<&TurnInput>,
    workspace_path: Option<&str>,
) -> RunReport {
    let runtime_label = if let Some(v) = input.cli_version.as_deref().filter(|s| !s.is_empty()) {
        format!("{} {v}", runtime_display_name(&input.runtime))
    } else {
        runtime_display_name(&input.runtime).to_owned()
    };
    let sandbox_opt = (!input.sandbox.is_empty()).then_some(input.sandbox.as_str());
    let may_do = (!input.runtime.is_empty() && !input.sandbox.is_empty())
        .then(|| what_it_may_do(&input.runtime, &input.sandbox, input.mode.as_deref()));
    let (cost_label, cost_line) = cost_label_and_line(&input.events, input.running);

    let recovery_note = if let Some(next) = next_turn {
        if next.continues_reason.as_deref() == Some("route-switch")
            || next.started_by_kind.as_deref() == Some("carry-on")
        {
            Some(format!(
                "Carried on to {} ({}) in {}.",
                runtime_display_name(&next.runtime),
                next.model,
                short_mission_id(&next.mission_id)
            ))
        } else {
            None
        }
    } else {
        match &input.recovery {
            Some(crate::recovery::Offer::Resume { epoch, note, .. }) => {
                Some(format!("Can resume from {} ({note}).", checkpoint_label(*epoch)))
            }
            Some(crate::recovery::Offer::Refused(reason)) => Some(reason.clone()),
            None => None,
        }
    };

    RunReport {
        mission_id: input.mission_id.clone(),
        prompt: input.prompt.clone(),
        runtime: input.runtime.clone(),
        runtime_label,
        cli_version: input.cli_version.clone(),
        model: input.model.clone(),
        account: account_phrase(&input.runtime, &input.model, &input.resolved_route_id),
        sandbox: input.sandbox.clone(),
        sandbox_label: sandbox_phrase(sandbox_opt),
        phase: input.phase.clone(),
        running: input.running,
        event_count: input.events.len(),
        checkpoints_count: input.checkpoints_count,
        last_checkpoint_epoch: input.last_checkpoint_epoch,
        integrity_issue_count: input.integrity_issue_count,
        ledger_label: ledger_verification_label(input.integrity_issue_count),
        cost_label,
        cost_line,
        may_do,
        signal_rail: build_signal_rail(&input.events, input.running),
        artifacts: produced_files(&input.events, workspace_path),
        recovery_note,
    }
}

pub fn build_conversation_reports(
    turns: &[TurnInput],
    workspace_path: Option<&str>,
) -> Vec<RunReport> {
    turns
        .iter()
        .enumerate()
        .map(|(i, turn)| build_run_report(turn, turns.get(i + 1), workspace_path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn speaks_in_locusts_words_not_the_plumbings() {
        for sandbox in ["read-only", "workspace-write", "full-access"] {
            let may = what_it_may_do("cursor", sandbox, None);
            let joined: String = may
                .rows
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!joined.contains("host-selected"));
            assert!(!joined.contains("argv"));
            assert!(!joined.contains("app-server"));
            assert!(!joined.contains("workspace folder"));
        }
    }

    #[test]
    fn says_why_artifacts_are_empty_for_this_run() {
        assert_eq!(
            no_artifacts_line(Some("workspace-write"), false, true),
            "This reply changed no files."
        );
        assert_eq!(
            no_artifacts_line(Some("full-access"), false, true),
            "This reply changed no files."
        );
        assert_eq!(
            no_artifacts_line(Some("workspace-write"), false, false),
            "No changed files were reported. A file a command wrote may not be listed here."
        );
        assert_eq!(
            no_artifacts_line(Some("read-only"), false, false),
            "This reply could only read, so it changed no files."
        );
        assert_eq!(
            no_artifacts_line(Some("workspace-write"), true, false),
            "Files this reply changes appear here as it works."
        );
    }

    #[test]
    fn never_denies_what_auto_allows() {
        let auto = what_it_may_do("cursor", "full-access", None);
        assert!(auto.rows.iter().all(|r| r.verdict != "deny"));
        assert!(auto
            .rows
            .iter()
            .any(|r| r.text.contains("anywhere your account can reach")));
    }

    #[test]
    fn says_what_the_header_says_from_the_same_function() {
        assert_eq!(
            sandbox_phrase(Some("full-access")),
            "may edit anything on this machine"
        );
        assert_eq!(
            sandbox_phrase(Some("workspace-write")),
            "may edit the workspace"
        );
        assert_eq!(sandbox_phrase(Some("read-only")), "read-only");
    }

    #[test]
    fn still_denies_what_workspace_write_cannot_do() {
        let scoped = what_it_may_do("opencode", "workspace-write", None);
        assert!(scoped
            .rows
            .iter()
            .any(|r| r.verdict == "deny" && r.text.contains("opening files outside this folder")));
        assert!(scoped
            .rows
            .iter()
            .any(|r| r.verdict == "allow" && r.text.contains("change files in this folder")));
    }

    #[test]
    fn says_opencode_may_search_the_web_and_no_runtime_has_false_network_rule() {
        let oc = what_it_may_do("opencode", "workspace-write", None);
        assert!(oc
            .rows
            .iter()
            .any(|r| r.text.contains("search the web and open web pages")));
        for runtime in [
            "codex",
            "claude",
            "cursor",
            "opencode",
            "copilot",
            "muse",
            "antigravity",
        ] {
            for sandbox in ["read-only", "workspace-write"] {
                let may = what_it_may_do(runtime, sandbox, None);
                assert!(may
                    .rows
                    .iter()
                    .all(|r| !r.text.contains("beyond the model's own")));
            }
        }
    }

    #[test]
    fn leaves_the_rest_to_the_runtimes_own_settings_by_name() {
        let agy = what_it_may_do("antigravity", "workspace-write", None);
        assert_eq!(
            agy.rest,
            "Anything not listed is left to Antigravity's own settings."
        );
    }

    #[test]
    fn says_approve_each_asks_rather_than_drawing_edit() {
        let asking = what_it_may_do("opencode", "workspace-write", Some("approve-each"));
        assert!(asking
            .rows
            .iter()
            .any(|r| r.text.contains("once you approve each change")));
        assert!(asking
            .rows
            .iter()
            .all(|r| !r.text.contains("opening files outside this folder, other than")));
    }

    #[test]
    fn still_refuses_every_write_for_read_only() {
        let ro = what_it_may_do("cursor", "read-only", None);
        assert!(ro
            .rows
            .iter()
            .any(|r| r.verdict == "deny" && r.text.contains("changing any file")));
    }

    #[test]
    fn builds_two_turn_report_with_route_provenance_and_recovery() {
        let t1 = TurnInput {
            mission_id: "mission_111122223333".into(),
            prompt: "First ask".into(),
            runtime: "codex".into(),
            cli_version: Some("0.153.0".into()),
            model: "gpt-6-astra".into(),
            resolved_route_id: "codex-account:default".into(),
            sandbox: "read-only".into(),
            mode: Some("ask".into()),
            continues_reason: None,
            started_by_kind: None,
            phase: "failed".into(),
            running: false,
            events: vec![
                json!({"id":"e1","type":"run.started","sourceAdapter":"codex","occurredAt":"2026-11-12T10:00:00.000Z","payload":{}}),
                json!({"id":"e2","type":"run.failed","sourceAdapter":"codex","occurredAt":"2026-11-12T10:00:05.000Z","payload":{"message":"Quota reached"}}),
            ],
            checkpoints_count: 1,
            last_checkpoint_epoch: Some(1),
            integrity_issue_count: 0,
            recovery: None,
        };
        let t2 = TurnInput {
            mission_id: "mission_444455556666".into(),
            prompt: "Carry on".into(),
            runtime: "claude".into(),
            cli_version: None,
            model: "claude-sonnet-4-6".into(),
            resolved_route_id: "claude-account:default".into(),
            sandbox: "workspace-write".into(),
            mode: Some("accept-edits".into()),
            continues_reason: Some("route-switch".into()),
            started_by_kind: Some("carry-on".into()),
            phase: "completed".into(),
            running: false,
            events: vec![
                json!({"id":"e3","type":"run.started","sourceAdapter":"claude","occurredAt":"2026-11-12T10:01:00.000Z","payload":{}}),
                json!({"id":"e4","type":"run.completed","sourceAdapter":"claude","occurredAt":"2026-11-12T10:01:10.000Z","payload":{"usage":{"costUsd":0.0125}}}),
            ],
            checkpoints_count: 1,
            last_checkpoint_epoch: Some(1),
            integrity_issue_count: 0,
            recovery: None,
        };

        let reports = build_conversation_reports(&[t1, t2], None);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].runtime_label, "Codex 0.153.0");
        assert!(reports[0]
            .recovery_note
            .as_deref()
            .unwrap()
            .contains("Carried on to Claude Code (claude-sonnet-4-6) in 44445555."));
        assert_eq!(reports[1].cost_label, "Cost");
        assert_eq!(reports[1].cost_line, "$0.0125");
        assert_eq!(reports[1].ledger_label, "record readable");
    }
}
