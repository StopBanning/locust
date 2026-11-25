//! What a teammate's face and row say, as Electron works it out (`faceState.ts`, `status.ts` `teammateStatusView`,
//! `runtimeReach`, `stateInWords`, `facePresenceFor`).
//!
//! One teammate, one state, in this order: blocked (nothing can run), waiting on you, whatever their live run is
//! doing, a hop for a run that just finished, a glance at a message that just arrived, then idle. The live run's part
//! is read off its own events (`live_activity_of`), the same events the thread's working line reads.

use crate::agents::Probe;
use crate::bot_rig::{face_label, live_activity_of, teammate_activity, FaceActivity, LiveActivity, LiveRuntimeEvent};
use serde_json::Value;
use std::path::Path;

/// A finished run's hop stays on the face this long (faceState.ts `DONE_HOP_MS`).
pub const DONE_HOP_MS: u64 = 1_800;

/// One normalized event (`{type, payload}`), as `live_activity_of` reads it. A helper's own tool calls carry their
/// parent's item (`byHelper`, shared/tool-kinds.ts).
pub fn live_event_of(event: &Value) -> LiveRuntimeEvent {
    let payload = &event["payload"];
    let text = |key: &str| payload[key].as_str().unwrap_or("").to_owned();
    match event["type"].as_str().unwrap_or("") {
        "message.delta" => LiveRuntimeEvent::MessageDelta { final_chunk: payload["final"] == true },
        "step.started" => LiveRuntimeEvent::StepStarted { step_kind: text("stepKind") },
        "step.completed" => LiveRuntimeEvent::StepCompleted { step_kind: text("stepKind") },
        "step.failed" => LiveRuntimeEvent::StepFailed { step_kind: text("stepKind") },
        "tool.started" => LiveRuntimeEvent::ToolStarted {
            item_id: text("itemId"),
            name: text("name"),
            by_helper: payload["parentItemId"].as_str().is_some_and(|p| !p.is_empty()),
        },
        "tool.completed" => LiveRuntimeEvent::ToolCompleted { item_id: text("itemId") },
        "tool.failed" => LiveRuntimeEvent::ToolFailed { item_id: text("itemId") },
        _ => LiveRuntimeEvent::Other,
    }
}

/// What a live run is doing, from its ledger file's events. None when the file cannot be read.
pub fn live_activity_in_ledger(path: &Path) -> Option<LiveActivity> {
    let body = std::fs::read_to_string(path).ok()?;
    let events: Vec<LiveRuntimeEvent> = body
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|record| record.get("event").map(live_event_of))
        .collect();
    Some(live_activity_of(&events, true))
}

/// `conversationList.ts` `liveRunLastAt` (0.755): when a live run last did something -- its newest event,
/// otherwise when it was sent, otherwise the restored ledger's `lastUpdatedAt`.
pub fn live_run_last_at<'a>(
    event_times: &'a [&'a str],
    started_at_iso: Option<&'a str>,
    restored_last_updated_at: Option<&'a str>,
) -> Option<&'a str> {
    event_times.last().copied().or(started_at_iso).or(restored_last_updated_at)
}

/// When a live run in `path` last did something, from its ledger events' `occurredAt` timestamps.
pub fn live_last_at_in_ledger(
    path: &Path,
    started_at_iso: Option<&str>,
    restored_last_updated_at: Option<&str>,
) -> Option<String> {
    let body = std::fs::read_to_string(path).ok().unwrap_or_default();
    let owned: Vec<String> = body
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|record| record["event"]["occurredAt"].as_str().map(str::to_owned))
        .collect();
    let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    live_run_last_at(&refs, started_at_iso, restored_last_updated_at).map(str::to_owned)
}

/// One agent as discovery found it, for the face (the part of PublicRuntimeStatus these rules read).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFacts {
    pub id: String,
    pub ready: bool,
    /// What its check said (`status`).
    pub status: Probe,
    pub installed: bool,
    /// Installed, and its check has not finished.
    pub checking: bool,
}

/// status.ts `runtimeIsUsable`: needs both the flag and the probe.
pub fn runtime_is_usable(runtime: &RuntimeFacts) -> bool {
    runtime.ready && runtime.status == Probe::Ready
}

/// status.ts `runtimeReach`'s answer. Each half is None when it is not said.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeReach {
    pub any_usable: Option<bool>,
    pub any_installed: Option<bool>,
}

/// status.ts `runtimeReach`: nothing until discovery has answered; then whether any agent is usable (not said as
/// "none" while a check is still running) and whether any is installed.
pub fn runtime_reach(runtimes: &[RuntimeFacts], known: bool) -> RuntimeReach {
    if !known {
        return RuntimeReach::default();
    }
    let usable = runtimes.iter().any(runtime_is_usable);
    // A partial sweep can prove that one works, but cannot prove none work yet.
    let pending = runtimes.iter().any(|r| r.checking);
    RuntimeReach { any_usable: (usable || !pending).then_some(usable), any_installed: Some(runtimes.iter().any(|r| r.installed)) }
}

/// status.ts `TeammateStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeammateStatus {
    Blocked,
    ApprovalNeeded,
    Working,
    Idle,
}

/// The colour a status is drawn in (`lc-tone-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Red,
    Amber,
    Live,
    Muted,
}

/// status.ts `TeammateStatusView`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusView {
    pub status: TeammateStatus,
    pub activity: FaceActivity,
    pub label: String,
    pub tone: Tone,
    pub pulse: bool,
}

/// What `teammate_status_view` reads (its `input`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusInputs<'a> {
    /// The runtime the teammate's own work is on (`runtimeOfTeammate`); None when they have none.
    pub own_runtime: Option<&'a RuntimeFacts>,
    pub reach: RuntimeReach,
    /// A run of theirs is running (`hasRunningMission`).
    pub running: bool,
    /// Cards waiting on the person from them.
    pub pending_approvals: usize,
    pub role_label: &'a str,
    /// What their live run is doing, when its events were read.
    pub live: Option<LiveActivity>,
    pub recently_done: bool,
    pub recently_received: bool,
}

/// status.ts `teammateStatusView`.
pub fn teammate_status_view(input: &StatusInputs<'_>) -> StatusView {
    // A pending check proves neither a missing tool nor a sign-in wall.
    let own_blocked = input.own_runtime.is_some_and(|r| !r.checking && !runtime_is_usable(r));
    let activity = teammate_activity(
        input.reach.any_usable == Some(false) || own_blocked,
        input.pending_approvals > 0,
        if input.running { input.live.unwrap_or(LiveActivity::Working) } else { LiveActivity::Idle },
        input.recently_done,
        input.recently_received,
    );
    let view = |status, label: String, tone, pulse| StatusView { status, activity, label, tone, pulse };
    // Nobody can work when nothing is usable; WHY decides what the row says (no agent at all is not a sign-in).
    if input.reach.any_usable == Some(false) {
        let label = if input.reach.any_installed == Some(false) { "No AI agent installed" } else { "Sign-in needed" };
        return view(TeammateStatus::Blocked, label.into(), Tone::Red, false);
    }
    if let Some(runtime) = input.own_runtime.filter(|_| own_blocked) {
        // Absent is not unsigned, and not answering is not signed out (L18).
        let not_answering = runtime.installed && matches!(runtime.status, Probe::ProbeFailed | Probe::Offline);
        let label = if !runtime.installed {
            "AI agent not installed"
        } else if runtime.status == Probe::Unsupported {
            "This AI agent version is not supported yet"
        } else if not_answering {
            "AI agent not answering — checking again"
        } else {
            "Sign-in needed"
        };
        return view(TeammateStatus::Blocked, label.into(), if not_answering { Tone::Amber } else { Tone::Red }, false);
    }
    // Copy follows state: the word beside the face is the face's own word.
    let said = format!("{} · {}", input.role_label, face_label(activity));
    if input.pending_approvals > 0 {
        return view(TeammateStatus::ApprovalNeeded, said, Tone::Amber, false);
    }
    if input.running {
        return view(TeammateStatus::Working, said, Tone::Live, true);
    }
    view(TeammateStatus::Idle, said, Tone::Muted, false)
}

/// What a face's own state comes to (the activity half of `teammate_status_view`).
pub fn teammate_face(input: &StatusInputs<'_>) -> FaceActivity {
    teammate_status_view(input).activity
}

/// status.ts `stateInWords`: what the teammate is doing, or why they are blocked.
pub fn state_in_words(view: &StatusView) -> String {
    if view.status == TeammateStatus::Blocked { view.label.clone() } else { face_label(view.activity).to_owned() }
}

/// status.ts `facePresenceFor`: the dot on the face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Working,
    Approval,
    Blocked,
    None,
}

pub fn face_presence_for(status: TeammateStatus) -> Presence {
    match status {
        TeammateStatus::Working => Presence::Working,
        TeammateStatus::ApprovalNeeded => Presence::Approval,
        TeammateStatus::Blocked => Presence::Blocked,
        TeammateStatus::Idle => Presence::None,
    }
}

/// `needsYou.ts` `waitingByTeammate` (0.564): how many things each teammate waits on the person for -- paused
/// approvals AND questions they stopped on. Memory suggestions, Scout cards, waiting routines, and items no
/// teammate owns are left out.
pub fn waiting_by_teammate(items: &[crate::needs_you::NeedsYouItem]) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for item in items {
        let teammate_id = match item {
            crate::needs_you::NeedsYouItem::Approval { teammate_id, .. }
            | crate::needs_you::NeedsYouItem::Decision { teammate_id, .. } => teammate_id.as_deref(),
            _ => None,
        };
        if let Some(id) = teammate_id {
            *counts.entry(id.to_owned()).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
#[path = "face_status_tests.rs"]
mod tests;
