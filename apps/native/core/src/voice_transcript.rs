//! Words and completed hand-offs collected during one call, then written to the transcript ledger companion.

use crate::{call_log, store::Store};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    You,
    Voice,
}

impl Speaker {
    pub fn from_wire(role: &str) -> Option<Self> {
        match role {
            "you" => Some(Self::You),
            "voice" => Some(Self::Voice),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Voice => "voice",
        }
    }
}

#[derive(Clone)]
struct Partial {
    text: String,
    at: u64,
}

pub struct Handoff {
    pub mission_id: String,
    pub teammate_id: String,
    pub name: String,
    pub task: String,
    pub at: u64,
}

pub struct Transcript {
    call_id: String,
    teammate_id: String,
    name: String,
    after: Option<String>,
    started: u64,
    lines: Vec<Value>,
    turns: Vec<Value>,
    handed: Vec<Value>,
    you: Option<Partial>,
    voice: Option<Partial>,
}

fn fit(text: &str, limit: usize) -> &str {
    let mut used = 0;
    for (at, c) in text.char_indices() {
        used += c.len_utf16();
        if used > limit {
            return &text[..at];
        }
    }
    text
}

impl Transcript {
    pub fn new(teammate_id: String, name: String, after: Option<String>, started: u64) -> Self {
        let name = fit(name.trim(), 80);
        Self {
            call_id: format!("call_{}", crate::invariants::unique_nanos()),
            teammate_id,
            name: if name.is_empty() {
                "Teammate".into()
            } else {
                name.into()
            },
            after: after.filter(|id| call_log::valid_id(id)),
            started,
            lines: Vec::new(),
            turns: Vec::new(),
            handed: Vec::new(),
            you: None,
            voice: None,
        }
    }

    fn partial(&mut self, speaker: Speaker) -> &mut Option<Partial> {
        match speaker {
            Speaker::You => &mut self.you,
            Speaker::Voice => &mut self.voice,
        }
    }

    pub fn delta(&mut self, speaker: Speaker, delta: &str, at: u64) {
        if delta.is_empty() {
            return;
        }
        let partial = self.partial(speaker).get_or_insert_with(|| Partial {
            text: String::new(),
            at,
        });
        let remaining = 2_000usize.saturating_sub(partial.text.encode_utf16().count());
        partial.text.push_str(fit(delta, remaining));
    }

    pub fn said(&mut self, speaker: Speaker, text: &str, at: u64) {
        let partial = self.partial(speaker).take();
        let at = partial.as_ref().map(|partial| partial.at).filter(|at| *at != 0).unwrap_or(at);
        let text = if text.trim().is_empty() {
            partial.as_ref().map_or("", |partial| partial.text.as_str())
        } else {
            text
        };
        let text = fit(text.trim(), 2_000);
        if !text.is_empty() {
            if self.lines.len() == 400 {
                self.lines.remove(0);
            }
            self.lines
                .push(json!({"role":speaker.name(),"text":text,"at":at}));
        }
    }

    pub fn record_turn(&mut self, mission_id: &str, at: u64) {
        if self.turns.len() < 40
            && call_log::valid_id(mission_id)
            && !self
                .turns
                .iter()
                .any(|turn| turn["missionId"] == mission_id)
        {
            self.turns.push(json!({"missionId":mission_id,"at":at}));
        }
    }

    pub fn record_handoff(
        &mut self,
        mission_id: &str,
        teammate_id: &str,
        name: &str,
        task: &str,
        at: u64,
    ) {
        let task = fit(task.trim(), 400);
        if self.handed.len() == 40
            || task.is_empty()
            || !call_log::valid_id(mission_id)
            || !call_log::valid_id(teammate_id)
            || self
                .handed
                .iter()
                .any(|hand| hand["missionId"] == mission_id)
        {
            return;
        }
        let name = fit(name.trim(), 80);
        let name = if name.is_empty() { "Teammate" } else { name };
        self.handed.push(json!({"missionId":mission_id,"teammateId":teammate_id,"name":name,"task":task,"at":at}));
        if teammate_id == self.teammate_id {
            self.record_turn(mission_id, at);
        }
    }

    pub fn snapshot(&self) -> Value {
        let mut out = json!({
            "callId": self.call_id, "teammateId": self.teammate_id, "name": self.name,
            "startedAt": self.started, "lines": self.lines, "turns": self.turns
        });
        if let Some(after) = &self.after {
            out["afterMissionId"] = json!(after);
        }
        if !self.handed.is_empty() {
            out["handed"] = json!(self.handed);
        }
        out
    }

    /// The open thread's view. The partial words never go into a saved call as partial fields.
    pub fn live_snapshot(&self) -> Value {
        let mut out = self.snapshot();
        let words = |line: &Option<Partial>| line.as_ref().map_or_else(String::new, |line| line.text.trim().to_owned());
        out["partial"] = json!({"you": words(&self.you), "voice": words(&self.voice)});
        out
    }

    pub fn finish(mut self, at: u64) -> Value {
        self.said(Speaker::You, "", at);
        self.said(Speaker::Voice, "", at);
        let mut out = self.snapshot();
        out["endedAt"] = json!(at);
        out
    }

    pub fn save(self, store: &Store, at: u64) -> Result<Value, String> {
        let call = self.finish(at);
        call_log::save(store, &call)?;
        Ok(call)
    }
}

#[cfg(test)]
#[path = "voice_transcript_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "b09_transcript_tests.rs"]
mod b09_transcript_tests;
