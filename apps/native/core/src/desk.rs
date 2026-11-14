//! What Locust in a browser may ask and hear, and how long frames split (`shared/desk.ts` + `shared/desk-calls.ts`).

use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DESK_TAKE_BACK_CHANNEL: &str = "desk:take-back";
pub const DESK_WHERE_CHANNEL: &str = "desk:where";
pub const PHONE_DESK_CHANNEL: &str = "phone:desk";
pub const DESK_COVER_QUERY: &str = "elsewhere";

/// The most characters one part carries, inside the relay's 512 KiB box.
pub const DESK_PART_CHARS: usize = 240_000;
/// The longest frame put back together: an attachment pasted in a browser fits, a flood does not.
pub const DESK_MOST_CHARS: usize = 48 * 1024 * 1024;
/// How long an incomplete multi-part frame is held before eviction.
pub const DESK_DEFAULT_WAIT_MS: u64 = 120_000;

/// What a browser is told of a call that stays on the computer.
pub const WINDOW_ONLY_SAYS: &str = "This works in Locust on your computer.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeskAccess {
    Read,
    Act,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskPart {
    pub v: u8,
    pub kind: &'static str,
    pub id: String,
    pub n: usize,
    pub of: usize,
    pub text: String,
}

pub const READ_CHANNELS: &[&str] = &[
    "acp-agents:list",
    "appearance:get",
    "approval-rules:list",
    "app:changelog",
    "app:info",
    "app:update-check",
    "away:since",
    "background:list",
    "locust:browser:state",
    "calls:list",
    "claude-cloud:check",
    "claude-cloud:environment",
    "claude-cloud:list",
    "cloud-computer:list",
    "cloud-computer:folder",
    "cloud:diff",
    "cloud:folders",
    "cloud:list",
    "cloud:refresh",
    "cloud-turn:list",
    "cloud-turn:patch",
    "cloud:where",
    "compare:changes",
    "compare:changes-refusal",
    "compare:in-place",
    "compare:list",
    "compare:reruns",
    "connectors:list",
    "diagnostics:report",
    "reply:diagram",
    "folder:changes",
    "folder:diff",
    "folder:list",
    "folder:pull-request",
    "folder:repository",
    "locust:github-account",
    "locust:github-cli-versions",
    "github-issues:list",
    "github-issues:ready",
    "github:issue-link",
    "github-issues:open",
    "github-issues:remembered",
    "github:pull-link",
    "groups:list",
    "mission:handoff-preview",
    "keep-running:get",
    "keychain:asking",
    "stay-awake:get",
    "locust-mcp:claude-app",
    "locust-mcp:get",
    "login-item:get",
    "updates:mac-release",
    "memory:list",
    "mission-history:list",
    "mission-history:read",
    "mission:storage",
    "mission:trash:list",
    "models:list",
    "own-commands:expand",
    "own-commands:list",
    "ownModels:list",
    "panel-runs:list",
    "pdf:pages",
    "pets:list",
    "pets:sheet",
    "pets:thumbnail",
    "queued-messages:read",
    "claude-remote-control:get",
    "rooms:list",
    "routines:list",
    "routines:templates",
    "routines:templatePreview",
    "runtime-artifacts:list",
    "runtime:commands",
    "runtime-discovery:get",
    "runtime-discovery:log",
    "runtime:setup",
    "runtime-updates:read",
    "scout:away-status",
    "scout:log",
    "scout:state",
    "session-import:list",
    "skill-library:list",
    "skill-library:preview",
    "talk-shortcut:read",
    "teammates:list",
    "teammate:setup",
    "teammate:skills-list",
    "teammates:spend",
    "teammate:tune-lesson-list",
    "turn-undo:state",
    "usage:read",
    "voice-call:available",
    "voice-call:voices",
    "voice-preview:saved",
    "voice:ready",
    "voice:settings-read",
    "workspace:files",
    "workspace:image",
    "workspace-settings:read",
    "workspace:text",
    "worktrees:land-preview",
    "worktrees:list",
    "worktrees:review",
    "worktrees:turn-diff",
    "conversation:references",
    "mission:claude-context",
];

pub const ACT_CHANNELS: &[&str] = &[
    "workspace-settings:change",
    "workspace:download",
    "appearance:set",
    "app:changelog-seen",
    "calls:save",
    "mission:carry-on",
    "codex-mission:cancel",
    "compare:add-model",
    "compare:ask",
    "compare:draft-points",
    "compare:judge",
    "compare:keep",
    "compare:rerun",
    "compare:rerun-checked",
    "compare:rerun-stop",
    "compare:retry",
    "compare:start",
    "groups:create",
    "groups:instructions",
    "groups:remove",
    "groups:rename",
    "groups:route",
    "memory:add",
    "memory:remove",
    "memory:restore",
    "memory:update",
    "scout:notes",
    "scout:tune",
    "codex-mission:start",
    "folder:commit-draft",
    "folder:switch",
    "groups:assign",
    "mission:delete",
    "mission:hand-off",
    "mission:restore",
    "mission:resume",
    "folder:pull-request-watch",
    "queued-messages:write",
    "rewind:put-back",
    "rooms:create",
    "rooms:post",
    "rooms:remove",
    "rooms:rename",
    "rooms:task",
    "routines:create",
    "routines:pause",
    "routine:recovery",
    "routines:remove",
    "routines:run",
    "routines:update",
    "scout:dismiss",
    "teammates:create",
    "teammates:remove",
    "scout:edit-post",
    "scout:fix-again",
    "scout:fix-apply",
    "scout:fix-skip",
    "scout:look",
    "scout:post",
    "side:ask",
    "teammates:tag",
    "teammates:assign",
    "teammates:pinMission",
    "teammates:renameMission",
    "teammates:settleMission",
    "turn-undo:undo",
    "voice:cancel",
    "voice:transcribe",
    "workspace:paste",
    "worktrees:land",
    "worktrees:resolve",
];

/// Channels that stay in the window on this computer.
pub const WINDOW_ONLY: &[&str] = &[
    "appearance:set-look",
    "appearance:glass-panel",
    "appearance:set-glass-ground",
    "account:status",
    "account:sync-on",
    "account:sync-off",
    "account:sync-now",
    "account:delete",
    "account:bring-here",
    "account:ask",
    "account:ask-stop",
    "account:approve",
    "account:ignore",
    "remote-mcp:get",
    "remote-mcp:set",
    "remote-mcp:remove",
    "remote-mcp:decide",
    "acp-agents:remove",
    "acp-agents:save",
    "acp-agents:test",
    "approval-rules:remove-all",
    "approval-rules:remove",
    "approval-rules:from-card",
    "app:update-install",
    "app-update:lane",
    "background:dismiss",
    "background:open",
    "background:setup",
    "background:start",
    "background:stop",
    "locust:browser:back",
    "locust:browser:bounds",
    "locust:browser:close-panel",
    "locust:browser:close-tab",
    "locust:browser:forward",
    "locust:browser:navigate",
    "locust:browser:new-tab",
    "locust:browser:open-panel",
    "locust:browser:point-cancel",
    "locust:browser:point",
    "locust:browser:reload",
    "locust:browser:select-tab",
    "locust:browser:stop",
    "claude-cloud:apply",
    "claude-cloud:forget",
    "claude-cloud:home",
    "claude-cloud:send",
    "claude-cloud:start",
    "claude-cloud:terminal",
    "cloud:apply",
    "cloud-computer:allow",
    "cloud-computer:remove",
    "cloud-computer:set-up",
    "cloud-computer:sign-in",
    "cloud-computer:sign-in-stop",
    "cloud-computer:sign-out",
    "cloud-computer:stop",
    "cloud-computer:terminal",
    "cloud:start",
    "cloud-turn:apply",
    "cloud-turn:discard",
    "cloud-turn:pull-request",
    "connectors:add",
    "connectors:remove",
    "desk:take-back",
    "desk:where",
    "diagnostics:reveal",
    "diagnostics:feedback",
    "diagnostics:feedback-email",
    "diagnostics:feedback-save",
    "locust:github-install",
    "locust:github-sign-in-cancel",
    "locust:github-sign-in",
    "locust:github-update",
    "keep-running:set",
    "stay-awake:set",
    "locust-link:take",
    "locust-mcp:claude-app-restart",
    "locust-mcp:claude-app-set",
    "locust-mcp:set",
    "locust-mcp:set-conversations",
    "locust-mcp:set-editor-answers",
    "locust-mcp:set-own-mode",
    "login-item:set",
    "memory:clear",
    "mission:prune",
    "missions:save-record",
    "mission:trash:empty",
    "mission:open-in-terminal",
    "shell:open-link",
    "own-commands:allow",
    "own-commands:open",
    "own-commands:run",
    "own-commands:stop",
    "ownModels:add",
    "ownModels:chatOnly",
    "ownModels:remove",
    "ownModels:test",
    "workspace:page-pick-cancel",
    "workspace:page-pick",
    "pets:add",
    "pets:remove",
    "phone:anywhere",
    "phone:desk",
    "phone:forget",
    "phone:new-code",
    "phone:night-approvals",
    "phone:remove",
    "phone:settings",
    "phone:set-enabled",
    "phone:set-mode",
    "phone:github-in",
    "phone:github-out",
    "phone:open-browser",
    "profile:backup",
    "profile:last-restore",
    "profile:pick-folder",
    "profile:restore",
    "profile:restore-preview",
    "claude-remote-control:set",
    "reply:page",
    "routines:export",
    "routines:chooseFolder",
    "routines:import",
    "routines:importPreview",
    "runtime:install",
    "runtime:sign-in",
    "runtime-updates:now",
    "runtime-updates:set",
    "scout:away-set",
    "scout:turn-on",
    "session-import:import",
    "skill-library:install",
    "skill-library:remove",
    "talk-shortcut:pending",
    "talk-shortcut:set",
    "teammates:connectors",
    "teammates:own-browser",
    "teammates:folder",
    "teammate:skills-remove",
    "teammate:skills-use",
    "team-card:add",
    "team-card:save",
    "telegram:new-link",
    "telegram:remove",
    "telegram:settings",
    "telegram:set-token",
    "chat-apps:alerts",
    "discord:new-link",
    "discord:remove",
    "discord:settings",
    "discord:set-token",
    "slack:new-link",
    "slack:remove",
    "slack:settings",
    "slack:set-token",
    "teammate-keys:list",
    "teammate-keys:remove",
    "teammate-keys:set",
    "routine:hook-read",
    "routine:hook-new",
    "mission:terminal-catch-up",
    "terminal:close",
    "terminal:copy",
    "terminal:create",
    "terminal:input",
    "terminal:link",
    "terminal:resize",
    "terminal:shells",
    "teammate:tune-combine",
    "teammate:tune-drafts",
    "teammate:tune-lesson-note",
    "teammate:tune-lesson-remove",
    "voice-call:answer",
    "voice-call:preview",
    "voice-call:set-voice",
    "voice-call:start",
    "voice-call:stop",
    "voice:download",
    "voice:openai-consent",
    "voice-preview:save",
    "voice-preview:trigger",
    "voice:settings-save",
    "folder:walkthrough",
    "workspace:attach",
    "workspace:choose",
    "folder:restore-list",
    "folder:restore-pick",
    "folder:restore-apply",
    "workspace:confirm",
    "workspace:open",
    "workspace:page",
    "workspace:reveal",
    "file:editors",
    "file:open",
    "clipboard:write",
    "workspace:save-copy",
    "workspace-settings:write",
    "worktrees:remove",
];

/// What the computer tells the window and also tells a browser that has it.
pub const DESK_EVENTS: &[&str] = &[
    "appearance:changed",
    "app:update-state",
    "away:since-changed",
    "codex-mission:update",
    "mission-approval:request",
    "mission-approval:withdrawn",
    "folder:pull-request-news",
    "runtime-discovery:event",
    "runtime:install-progress",
    "runtime-updates:changed",
    "scout:away-changed",
    "scout:changed",
    "voice:progress",
];

/// Told the window on this computer only: a sign-in's code, a pairing code, the terminal, this computer's own keys.
pub const WINDOW_ONLY_EVENTS: &[&str] = &[
    "account:changed",
    "remote-mcp:changed",
    "attention:open-mission",
    "locust:browser:event",
    "locust:browser:point-hotkey",
    "locust:github-sign-in-code",
    "locust-link:waiting",
    "own-commands:done",
    "phone:anywhere-changed",
    "runtime:sign-in-closed",
    "talk-shortcut:pressed",
    "telegram:changed",
    "discord:changed",
    "slack:changed",
    "terminal:event",
    "voice-call:event",
];

/// The window's one-way messages -- its badge, its controls, the loading window -- mean nothing from a browser.
pub const WINDOW_ONLY_SENDS: &[&str] = &[
    "window:close",
    "window:minimize",
    "window:toggle-maximize",
    "away:seen",
    "away:summary",
    "attention:needs-you-count",
    "attention:run-finished",
    "splash:done",
];

/// Whether a browser window receives this event channel.
pub fn is_desk_event(channel: &str) -> bool {
    DESK_EVENTS.contains(&channel)
}

/// How a browser may make this call with these arguments, or `None` when it stays in the window on this computer.
pub fn desk_access(channel: &str, args: &[Value]) -> Option<DeskAccess> {
    if READ_CHANNELS.contains(&channel) {
        return Some(DeskAccess::Read);
    }
    if ACT_CHANNELS.contains(&channel) {
        return Some(DeskAccess::Act);
    }
    let first = args.first().and_then(Value::as_object);
    match channel {
        "agent-accounts:call" => {
            if first.and_then(|o| o.get("kind")).and_then(Value::as_str) == Some("list") {
                Some(DeskAccess::Read)
            } else {
                None
            }
        }
        "mission-approval:decide" => {
            let obj = first?;
            if obj.get("decision").and_then(Value::as_str) != Some("approve-always") {
                Some(DeskAccess::Act)
            } else {
                None
            }
        }
        "teammates:update" => {
            let obj = first?;
            if !obj.contains_key("monthlyLimitUsd") {
                Some(DeskAccess::Act)
            } else {
                None
            }
        }
        "scout:set" => {
            let obj = first?;
            if !obj.contains_key("fixes") {
                Some(DeskAccess::Act)
            } else {
                None
            }
        }
        "folder:commit" => {
            if args.get(1).and_then(Value::as_str) == Some("commit") {
                Some(DeskAccess::Act)
            } else {
                None
            }
        }
        "routines:settle" => {
            let decision = first
                .and_then(|o| o.get("decision"))
                .and_then(Value::as_str)?;
            if decision == "keep" || decision == "discard" {
                Some(DeskAccess::Act)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// A frame's text in parts small enough for the relay, each with the frame's id and its place.
pub fn desk_parts(id: &str, text: &str) -> Vec<DeskPart> {
    if text.is_empty() {
        return vec![DeskPart {
            v: 1,
            kind: "desk-part",
            id: id.to_owned(),
            n: 0,
            of: 1,
            text: String::new(),
        }];
    }
    let chunks: Vec<&str> = if text.is_ascii() {
        text.as_bytes()
            .chunks(DESK_PART_CHARS)
            .map(|chunk| std::str::from_utf8(chunk).expect("ascii slice is valid utf-8"))
            .collect()
    } else {
        let mut out = Vec::new();
        let mut start = 0;
        let mut count = 0;
        for (idx, _) in text.char_indices() {
            if count == DESK_PART_CHARS {
                out.push(&text[start..idx]);
                start = idx;
                count = 0;
            }
            count += 1;
        }
        if start < text.len() {
            out.push(&text[start..]);
        }
        out
    };
    let of = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(n, chunk)| DeskPart {
            v: 1,
            kind: "desk-part",
            id: id.to_owned(),
            n,
            of,
            text: chunk.to_owned(),
        })
        .collect()
}

#[derive(Debug)]
struct HeldFrame {
    of: usize,
    parts: Vec<Option<String>>,
    got: usize,
    chars: usize,
    since_ms: u64,
}

/// Parts put back together, a frame at a time: whole text once every part is in, `None` until then.
#[derive(Debug)]
pub struct DeskJoiner {
    most_chars: usize,
    wait_ms: u64,
    open: HashMap<String, HeldFrame>,
}

impl Default for DeskJoiner {
    fn default() -> Self {
        Self::new(DESK_MOST_CHARS, DESK_DEFAULT_WAIT_MS)
    }
}

impl DeskJoiner {
    pub fn new(most_chars: usize, wait_ms: u64) -> Self {
        Self {
            most_chars,
            wait_ms,
            open: HashMap::new(),
        }
    }

    pub fn with_most_chars(most_chars: usize) -> Self {
        Self::new(most_chars, DESK_DEFAULT_WAIT_MS)
    }

    pub fn push(&mut self, part: &DeskPart) -> Option<String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.push_at(part, now_ms)
    }

    pub fn push_at(&mut self, part: &DeskPart, now_ms: u64) -> Option<String> {
        self.open
            .retain(|_, held| now_ms.saturating_sub(held.since_ms) <= self.wait_ms);
        let max_parts = self.most_chars.div_ceil(DESK_PART_CHARS) + 1;
        if part.of < 1 || part.of > max_parts || part.n >= part.of {
            return None;
        }
        if !self.open.contains_key(&part.id) {
            self.open.insert(
                part.id.clone(),
                HeldFrame {
                    of: part.of,
                    parts: vec![None; part.of],
                    got: 0,
                    chars: 0,
                    since_ms: now_ms,
                },
            );
        }
        let held = self.open.get_mut(&part.id)?;
        if held.of != part.of || held.parts[part.n].is_some() {
            return None;
        }
        let part_chars = if part.text.is_ascii() {
            part.text.len()
        } else {
            part.text.chars().count()
        };
        held.parts[part.n] = Some(part.text.clone());
        held.got += 1;
        held.chars += part_chars;
        if held.chars > self.most_chars {
            self.open.remove(&part.id);
            return None;
        }
        if held.got < held.of {
            return None;
        }
        let finished = self.open.remove(&part.id)?;
        let mut joined = String::with_capacity(finished.chars);
        for piece in finished.parts.into_iter().flatten() {
            joined.push_str(&piece);
        }
        Some(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn never_answers_what_grants_signs_in_pairs_spends_installs_or_opens_something_on_the_computer()
    {
        for channel in [
            "approval-rules:from-card",
            "runtime:sign-in",
            "locust:github-sign-in",
            "phone:new-code",
            "phone:desk",
            "ownModels:add",
            "connectors:add",
            "claude-remote-control:set",
            "locust-mcp:set",
            "app:update-install",
            "profile:restore",
            "runtime:install",
            "workspace:choose",
            "shell:open-link",
            "terminal:create",
            "terminal:input",
            DESK_TAKE_BACK_CHANNEL,
            DESK_WHERE_CHANNEL,
            "remote-mcp:decide",
            "remote-mcp:set",
        ] {
            assert_eq!(desk_access(channel, &[]), None, "{channel}");
            assert!(WINDOW_ONLY.contains(&channel), "{channel}");
        }
        assert_eq!(WINDOW_ONLY_SAYS, "This works in Locust on your computer.");
    }

    #[test]
    fn reads_and_acts_as_the_window_does_for_the_rest_the_history_a_new_turn() {
        assert_eq!(
            desk_access("mission-history:list", &[]),
            Some(DeskAccess::Read)
        );
        assert_eq!(
            desk_access("codex-mission:start", &[json!({})]),
            Some(DeskAccess::Act)
        );
    }

    #[test]
    fn answers_allow_once_and_deny_from_a_browser_never_always_reads_the_accounts_never_adds_one() {
        assert_eq!(
            desk_access(
                "mission-approval:decide",
                &[json!({ "approvalId": "a", "decision": "approve-once" })]
            ),
            Some(DeskAccess::Act)
        );
        assert_eq!(
            desk_access(
                "mission-approval:decide",
                &[json!({ "approvalId": "a", "decision": "deny", "reason": "no" })]
            ),
            Some(DeskAccess::Act)
        );
        assert_eq!(
            desk_access(
                "mission-approval:decide",
                &[json!({ "approvalId": "a", "answers": {} })]
            ),
            Some(DeskAccess::Act)
        );
        assert_eq!(
            desk_access(
                "mission-approval:decide",
                &[json!({ "approvalId": "a", "decision": "approve-always" })]
            ),
            None
        );
        assert_eq!(
            desk_access("agent-accounts:call", &[json!({ "kind": "list" })]),
            Some(DeskAccess::Read)
        );
        for action in [
            json!({ "kind": "add", "runtime": "claude", "name": "Work" }),
            json!({ "kind": "login", "id": "account_x" }),
            json!({ "kind": "remove", "id": "account_x" }),
        ] {
            assert_eq!(desk_access("agent-accounts:call", &[action]), None);
        }
    }

    #[test]
    fn tells_a_browser_what_the_window_is_told_but_never_a_sign_in_code_or_a_pairing_code() {
        assert!(is_desk_event("mission-approval:request"));
        for quiet in ["locust:github-sign-in-code", "phone:anywhere-changed"] {
            assert!(!is_desk_event(quiet));
            assert!(WINDOW_ONLY_EVENTS.contains(&quiet));
        }
        assert!(!WINDOW_ONLY_SENDS.is_empty());
    }

    #[test]
    fn carries_a_frame_too_long_for_one_box_in_parts_and_puts_it_back_together_once_whole() {
        let text = "x".repeat(DESK_PART_CHARS * 2 + 17);
        let parts = desk_parts("f1", &text);
        let ns: Vec<usize> = parts.iter().map(|part| part.n).collect();
        assert_eq!(ns, vec![0, 1, 2]);
        let mut join = DeskJoiner::default();
        assert_eq!(join.push(&parts[2]), None);
        assert_eq!(join.push(&parts[0]), None);
        assert_eq!(join.push(&parts[0]), None);
        assert_eq!(join.push(&parts[1]), Some(text));
        // Too much for one frame: let go.
        let mut small = DeskJoiner::with_most_chars(DESK_PART_CHARS);
        assert_eq!(small.push(&parts[0]), None);
        assert_eq!(small.push(&parts[1]), None);
        assert_eq!(small.push(&parts[2]), None);
    }
}
