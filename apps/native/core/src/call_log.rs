//! The transcript ledger companion, in Electron's calls.json format. Mission ledgers keep the turns themselves.

use crate::{invariants::lock, store::Store};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::io;
use std::sync::Mutex;

const FILE: &str = "calls.json";
const MAX_KEPT_CALLS: usize = 500;
static WRITES: Mutex<()> = Mutex::new(());

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn id(value: &Value) -> bool {
    value.as_str().is_some_and(valid_id)
}

fn time(value: &Value) -> bool {
    value.as_f64().is_some_and(|at| at.is_finite() && at > 0.0)
}

fn text(value: &Value, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.encode_utf16().count() <= max)
}

fn valid_array(value: &Value, max: usize, valid: impl Fn(&Value) -> bool) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.len() <= max && items.iter().all(valid))
}

fn valid_turn(turn: &Value) -> bool {
    id(&turn["missionId"]) && time(&turn["at"])
}

pub fn is_saved_call(call: &Value) -> bool {
    id(&call["callId"])
        && id(&call["teammateId"])
        && text(&call["name"], 80)
        && call.get("afterMissionId").is_none_or(id)
        && time(&call["startedAt"])
        && call.get("endedAt").is_none_or(time)
        && valid_array(&call["lines"], 400, |line| {
            matches!(line["role"].as_str(), Some("you" | "voice"))
                && text(&line["text"], 2_000)
                && time(&line["at"])
        })
        && valid_array(&call["turns"], 40, valid_turn)
        && call.get("handed").is_none_or(|handed| {
            valid_array(handed, 40, |handoff| {
                valid_turn(handoff)
                    && id(&handoff["teammateId"])
                    && text(&handoff["name"], 80)
                    && text(&handoff["task"], 400)
            })
        })
}

fn read(store: &Store) -> Result<Vec<Value>, String> {
    let path = store
        .check(&store.root().join(FILE))
        .map_err(|e| e.to_string())?;
    let body = match std::fs::read(path) {
        Ok(body) => body,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let value: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let calls = value["calls"]
        .as_array()
        .ok_or("calls.json holds no list of calls")?;
    Ok(calls
        .iter()
        .filter(|call| is_saved_call(call))
        .cloned()
        .collect())
}

pub fn save(store: &Store, call: &Value) -> Result<(), String> {
    if !is_saved_call(call) {
        return Err("The transcript does not read as a saved call.".into());
    }
    let _write = lock(&WRITES);
    // A missing file starts empty. An unreadable file must survive the save attempt.
    let mut calls = read(store)?;
    calls.retain(|old| old["callId"] != call["callId"]);
    calls.push(call.clone());
    calls.sort_by(|a, b| at(a, "startedAt").total_cmp(&at(b, "startedAt")));
    let discard = calls.len().saturating_sub(MAX_KEPT_CALLS);
    calls.drain(..discard);
    store.save_json(FILE, &json!({ "calls": calls }))
}

pub fn list(store: &Store, mission_ids: &[&str]) -> Vec<Value> {
    if mission_ids.len() > 1_000 || !mission_ids.iter().all(|id| valid_id(id)) {
        return Vec::new();
    }
    let here: HashSet<_> = mission_ids.iter().copied().collect();
    read(store)
        .unwrap_or_default()
        .into_iter()
        .filter(|call| {
            in_thread(&call["afterMissionId"], &here)
                || items(call, "turns")
                    .iter()
                    .any(|turn| in_thread(&turn["missionId"], &here))
        })
        .collect()
}

fn at(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or_default()
}

fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}

fn in_thread(id: &Value, here: &HashSet<&str>) -> bool {
    id.as_str().is_some_and(|id| here.contains(id))
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place<'a> {
    Before(&'a str),
    After(&'a str),
    End,
}

pub struct Piece<'a> {
    pub call: &'a Value,
    pub lines: Vec<&'a Value>,
    pub first: bool,
    pub last: bool,
}

pub fn placements<'a>(
    calls: &'a [Value],
    mission_ids: &[&str],
) -> BTreeMap<Place<'a>, Vec<Piece<'a>>> {
    let here: HashSet<_> = mission_ids.iter().copied().collect();
    let mut calls: Vec<_> = calls.iter().filter(|call| is_saved_call(call)).collect();
    calls.sort_by(|a, b| at(a, "startedAt").total_cmp(&at(b, "startedAt")));
    let mut placed = BTreeMap::<_, Vec<_>>::new();
    for call in calls {
        let mut turns: Vec<_> = items(call, "turns")
            .iter()
            .filter_map(|turn| {
                let id = turn["missionId"].as_str().filter(|id| here.contains(id))?;
                Some((id, at(turn, "at")))
            })
            .collect();
        turns.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut lines: Vec<_> = items(call, "lines").iter().collect();
        lines.sort_by(|a, b| at(a, "at").total_cmp(&at(b, "at")));
        let Some(&(first_id, start)) = turns.first() else {
            let place = call["afterMissionId"]
                .as_str()
                .filter(|id| here.contains(id))
                .map_or(Place::End, Place::After);
            placed.entry(place).or_default().push(Piece {
                call,
                lines,
                first: true,
                last: true,
            });
            continue;
        };
        let mut remaining = lines.into_iter().peekable();
        let before = lines_before(&mut remaining, start);
        placed
            .entry(Place::Before(first_id))
            .or_default()
            .push(Piece {
                call,
                lines: before,
                first: true,
                last: false,
            });
        for (index, &(id, _)) in turns.iter().enumerate() {
            let next = turns.get(index + 1);
            let lines = match next {
                Some(&(_, end)) => lines_before(&mut remaining, end),
                None => remaining.by_ref().collect(),
            };
            placed.entry(Place::After(id)).or_default().push(Piece {
                call,
                lines,
                first: false,
                last: next.is_none(),
            });
        }
    }
    placed
}

fn lines_before<'a>(
    remaining: &mut std::iter::Peekable<impl Iterator<Item = &'a Value>>,
    end: f64,
) -> Vec<&'a Value> {
    let mut lines = Vec::new();
    while let Some(line) = remaining.next_if(|line| at(line, "at") < end) {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
#[path = "call_log_tests.rs"]
mod tests;
