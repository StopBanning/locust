//! COMPARE (Electron `shared/compare.ts` + `main/compare-store.ts`):
//! one ask, two or three models side by side; the person keeps one.

use crate::store::Store;
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAX_COMPARE_SLOTS: usize = 3;
pub const MIN_COMPARE_SLOTS: usize = 2;
pub const COMPARE_SLOTS: [CompareSlotId; 3] = [CompareSlotId::A, CompareSlotId::B, CompareSlotId::C];
pub const MAX_JUDGE_CRITERIA: usize = 1_000;
pub const PANEL_JUDGES: usize = 2;
pub const COMPARE_TREES_DIRECTORY: &str = ".locust/compare";

const SCHEMA_VERSION: u64 = 1;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_COMPARES: usize = 200;
const MAX_PROMPT: usize = 8_000;
const MAX_TURNS_PER_SLOT: usize = 200;
const MAX_RETRIED_PER_SLOT: usize = 50;
const MAX_BROUGHT: usize = 200;
const MAX_JUDGES: usize = 20;
const MAX_RERUNS: usize = 30;
pub const UNREADABLE: &str = "The saved comparisons could not be read. Nothing was changed.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompareSlotId {
    A,
    B,
    C,
}

impl CompareSlotId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
        }
    }

    pub fn upper(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "a" | "A" => Some(Self::A),
            "b" | "B" => Some(Self::B),
            "c" | "C" => Some(Self::C),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangesIn {
    Copy,
    Folder,
}

impl ChangesIn {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "copy" => Some(Self::Copy),
            "folder" => Some(Self::Folder),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareRoute {
    pub runtime: String,
    pub model: String,
    pub account: Option<String>,
    pub effort: Option<String>,
    pub label: Option<String>,
    /// Only `"auto"` is kept; anything else is `None`.
    pub mode: Option<String>,
    pub instructions: Option<String>,
    pub why: Option<String>,
}

impl CompareRoute {
    pub fn new(runtime: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            runtime: runtime.into(),
            model: model.into(),
            account: None,
            effort: None,
            label: None,
            mode: None,
            instructions: None,
            why: None,
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("runtime".into(), json!(self.runtime));
        obj.insert("model".into(), json!(self.model));
        if let Some(v) = &self.account {
            obj.insert("account".into(), json!(v));
        }
        if let Some(v) = &self.effort {
            obj.insert("effort".into(), json!(v));
        }
        if let Some(v) = &self.label {
            obj.insert("label".into(), json!(v));
        }
        if self.mode.as_deref() == Some("auto") {
            obj.insert("mode".into(), json!("auto"));
        }
        if let Some(v) = &self.instructions {
            obj.insert("instructions".into(), json!(v));
        }
        if let Some(v) = &self.why {
            obj.insert("why".into(), json!(v));
        }
        Value::Object(obj)
    }

    fn from_json(val: &Value) -> Option<Self> {
        let obj = val.as_object()?;
        let runtime = text_field(obj.get("runtime"), 40)?;
        let model = text_field(obj.get("model"), 200)?;
        let account = obj
            .get("account")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 80)
            .map(str::to_owned);
        let effort = text_field(obj.get("effort"), 40);
        let label = text_field(obj.get("label"), 120);
        // Special case (compare-store.ts:101): only `mode === 'auto'` is kept.
        let mode = match obj.get("mode").and_then(Value::as_str) {
            Some("auto") => Some("auto".to_owned()),
            _ => None,
        };
        let instructions = obj
            .get("instructions")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 8_000)
            .map(str::to_owned);
        let why = text_field(obj.get("why"), 400);
        Some(Self {
            runtime,
            model,
            account,
            effort,
            label,
            mode,
            instructions,
            why,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareSlot {
    pub slot: CompareSlotId,
    pub route: CompareRoute,
    pub mission_ids: Vec<String>,
    pub refused: Option<String>,
    pub retried: Vec<String>,
    /// Computed by `with_column_folders`; never stored.
    pub folder: Option<String>,
}

impl CompareSlot {
    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("slot".into(), json!(self.slot.as_str()));
        obj.insert("route".into(), self.route.to_json());
        obj.insert("missionIds".into(), json!(self.mission_ids));
        if let Some(v) = &self.refused {
            obj.insert("refused".into(), json!(v));
        }
        if !self.retried.is_empty() {
            obj.insert("retried".into(), json!(self.retried));
        }
        Value::Object(obj)
    }

    fn from_json(val: &Value) -> Option<Self> {
        let obj = val.as_object()?;
        let slot = obj.get("slot").and_then(Value::as_str).and_then(CompareSlotId::parse)?;
        let route = obj.get("route").and_then(CompareRoute::from_json)?;
        let mission_ids = string_list(obj.get("missionIds"), 200, MAX_TURNS_PER_SLOT);
        let refused = text_field(obj.get("refused"), 1_000);
        let retried = string_list(obj.get("retried"), 200, MAX_RETRIED_PER_SLOT);
        Some(Self {
            slot,
            route,
            mission_ids,
            refused,
            retried,
            folder: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareKept {
    pub slot: CompareSlotId,
    pub at: String,
    pub brought: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgePanelMember {
    pub route: CompareRoute,
    pub mission_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareJudge {
    pub route: CompareRoute,
    pub mission_ids: Vec<String>,
    pub criteria: Option<String>,
    pub panel: Vec<JudgePanelMember>,
    pub panel_mission_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compare {
    pub compare_id: String,
    pub teammate_id: Option<String>,
    pub prompt: String,
    pub created_at: String,
    pub slots: Vec<CompareSlot>,
    pub changes: bool,
    pub changes_in: Option<ChangesIn>,
    pub blind: bool,
    pub kept: Option<CompareKept>,
    pub judge: Option<CompareJudge>,
    pub base: Option<String>,
    pub rerun_of: Option<String>,
    pub reruns: Vec<String>,
    pub settled: bool,
    pub point_drafts: Vec<String>,
}

impl Compare {
    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("compareId".into(), json!(self.compare_id));
        if let Some(v) = &self.teammate_id {
            obj.insert("teammateId".into(), json!(v));
        }
        obj.insert("prompt".into(), json!(self.prompt));
        obj.insert("createdAt".into(), json!(self.created_at));
        obj.insert(
            "slots".into(),
            Value::Array(self.slots.iter().map(CompareSlot::to_json).collect()),
        );
        if self.changes {
            obj.insert("changes".into(), json!(true));
            if let Some(ci) = self.changes_in {
                obj.insert("changesIn".into(), json!(ci.as_str()));
            }
        }
        if self.blind {
            obj.insert("blind".into(), json!(true));
        }
        if let Some(kept) = &self.kept {
            let mut k = serde_json::Map::new();
            k.insert("slot".into(), json!(kept.slot.as_str()));
            k.insert("at".into(), json!(kept.at));
            if let Some(brought) = &kept.brought {
                k.insert("brought".into(), json!(brought));
            }
            obj.insert("kept".into(), Value::Object(k));
        }
        if let Some(judge) = &self.judge {
            let mut j = serde_json::Map::new();
            j.insert("route".into(), judge.route.to_json());
            j.insert("missionIds".into(), json!(judge.mission_ids));
            if let Some(c) = &judge.criteria {
                j.insert("criteria".into(), json!(c));
            }
            if !judge.panel.is_empty() {
                j.insert(
                    "panel".into(),
                    Value::Array(
                        judge
                            .panel
                            .iter()
                            .map(|m| json!({"route": m.route.to_json(), "missionId": m.mission_id}))
                            .collect(),
                    ),
                );
            }
            if !judge.panel_mission_ids.is_empty() {
                j.insert("panelMissionIds".into(), json!(judge.panel_mission_ids));
            }
            obj.insert("judge".into(), Value::Object(j));
        }
        if let Some(v) = &self.base {
            obj.insert("base".into(), json!(v));
        }
        if let Some(v) = &self.rerun_of {
            obj.insert("rerunOf".into(), json!(v));
        }
        if !self.reruns.is_empty() {
            obj.insert("reruns".into(), json!(self.reruns));
        }
        if self.settled {
            obj.insert("settled".into(), json!(true));
        }
        if !self.point_drafts.is_empty() {
            obj.insert("pointDrafts".into(), json!(self.point_drafts));
        }
        Value::Object(obj)
    }

    fn from_json(val: &Value) -> Option<Self> {
        let obj = val.as_object()?;
        let compare_id = text_field(obj.get("compareId"), 80)?;
        let teammate_id = match obj.get("teammateId") {
            None | Some(Value::Null) => None,
            Some(v) => Some(text_field(Some(v), 200)?),
        };
        let prompt = text_field(obj.get("prompt"), MAX_PROMPT)?;
        let created_at = text_field(obj.get("createdAt"), 40)?;
        let raw_slots = obj.get("slots")?.as_array()?;
        let slots: Vec<CompareSlot> = raw_slots.iter().filter_map(CompareSlot::from_json).collect();
        if teammate_id.is_none() && slots.iter().any(|s| s.route.instructions.is_some()) {
            return None;
        }
        let unique_slots: HashSet<_> = slots.iter().map(|s| s.slot).collect();
        if !(MIN_COMPARE_SLOTS..=MAX_COMPARE_SLOTS).contains(&slots.len()) || unique_slots.len() != slots.len() {
            return None;
        }
        let changes = obj.get("changes").and_then(Value::as_bool) == Some(true);
        let changes_in = if changes {
            obj.get("changesIn").and_then(Value::as_str).and_then(ChangesIn::parse)
        } else {
            None
        };
        let blind = obj.get("blind").and_then(Value::as_bool) == Some(true);
        let kept = obj.get("kept").and_then(Value::as_object).and_then(|k| {
            let slot = k.get("slot").and_then(Value::as_str).and_then(CompareSlotId::parse)?;
            let at = text_field(k.get("at"), 40)?;
            if !slots.iter().any(|s| s.slot == slot) {
                return None;
            }
            let brought = k.get("brought").and_then(Value::as_array).map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.is_empty() && s.len() <= 500)
                    .take(MAX_BROUGHT)
                    .map(str::to_owned)
                    .collect()
            });
            Some(CompareKept { slot, at, brought })
        });
        let judge = obj.get("judge").and_then(Value::as_object).and_then(|j| {
            let route = j.get("route").and_then(CompareRoute::from_json)?;
            let mission_ids = string_list(j.get("missionIds"), 200, MAX_JUDGES);
            if mission_ids.is_empty() {
                return None;
            }
            let criteria = text_field(j.get("criteria"), MAX_JUDGE_CRITERIA);
            let panel: Vec<JudgePanelMember> = j
                .get("panel")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let m_obj = m.as_object()?;
                            let r = m_obj.get("route").and_then(CompareRoute::from_json)?;
                            let id = text_field(m_obj.get("missionId"), 200)?;
                            Some(JudgePanelMember { route: r, mission_id: id })
                        })
                        .take(PANEL_JUDGES)
                        .collect()
                })
                .unwrap_or_default();
            let panel_mission_ids =
                string_list(j.get("panelMissionIds"), 200, MAX_JUDGES * PANEL_JUDGES);
            Some(CompareJudge {
                route,
                mission_ids,
                criteria,
                panel,
                panel_mission_ids,
            })
        });
        let base = obj
            .get("base")
            .and_then(Value::as_str)
            .filter(|s| is_commit_sha(s))
            .map(str::to_owned);
        let rerun_of = text_field(obj.get("rerunOf"), 80);
        let reruns = string_list(obj.get("reruns"), 80, MAX_RERUNS);
        let settled = obj.get("settled").and_then(Value::as_bool) == Some(true);
        let point_drafts = string_list(obj.get("pointDrafts"), 200, MAX_JUDGES);

        Some(Self {
            compare_id,
            teammate_id,
            prompt,
            created_at,
            slots,
            changes,
            changes_in,
            blind,
            kept,
            judge,
            base,
            rerun_of,
            reruns,
            settled,
            point_drafts,
        })
    }
}

fn text_field(val: Option<&Value>, max: usize) -> Option<String> {
    val.and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= max)
        .map(str::to_owned)
}

fn string_list(val: Option<&Value>, max_item_len: usize, max_items: usize) -> Vec<String> {
    let Some(arr) = val.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut items: Vec<String> = arr
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= max_item_len)
        .map(str::to_owned)
        .collect();
    if items.len() > max_items {
        items.drain(0..items.len() - max_items);
    }
    items
}

pub fn is_commit_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

// ---- Shared helpers (`shared/compare.ts`) --------------------------------------------------------------------------

pub fn compare_slot_key(teammate_id: Option<&str>, compare_id: &str, slot: CompareSlotId) -> String {
    format!("{}#{compare_id}/{}", teammate_id.unwrap_or("nobody"), slot.as_str())
}

pub fn compare_tree_id(compare_id: &str, slot: CompareSlotId) -> String {
    format!("{compare_id}-{}", slot.as_str())
}

pub fn changes_line(files: usize, added: Option<usize>, removed: Option<usize>) -> String {
    if files == 0 {
        return "no changes".into();
    }
    let noun = if files == 1 { "file" } else { "files" };
    match (added, removed) {
        (Some(a), Some(r)) => format!("+{a} \u{2212}{r} in {files} {noun}"),
        _ => format!("{files} {noun} changed"),
    }
}

pub fn compares_gone_with(compares: &[Compare], gone: &HashSet<String>) -> Vec<Compare> {
    compares
        .iter()
        .filter(|c| {
            let mut has_any = false;
            for slot in &c.slots {
                for id in slot.mission_ids.iter().chain(slot.retried.iter()) {
                    has_any = true;
                    if !gone.contains(id) {
                        return false;
                    }
                }
            }
            has_any
        })
        .cloned()
        .collect()
}

pub fn compare_membership(compares: &[Compare]) -> HashMap<String, (String, CompareSlotId)> {
    let mut by_mission = HashMap::new();
    for c in compares {
        for slot in &c.slots {
            for id in slot.retried.iter().chain(slot.mission_ids.iter()) {
                by_mission.insert(id.clone(), (c.compare_id.clone(), slot.slot));
            }
        }
    }
    by_mission
}

pub fn newest_judge_mission_ids(judge: Option<&CompareJudge>) -> Vec<String> {
    let Some(j) = judge else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(last) = j.mission_ids.last() {
        out.push(last.clone());
    }
    for m in &j.panel {
        out.push(m.mission_id.clone());
    }
    out
}

pub fn judge_mission_ids(compares: &[Compare]) -> HashSet<String> {
    let mut out = HashSet::new();
    for c in compares {
        if let Some(j) = &c.judge {
            out.extend(j.mission_ids.iter().cloned());
            out.extend(j.panel_mission_ids.iter().cloned());
        }
        out.extend(c.point_drafts.iter().cloned());
    }
    out
}

pub fn rerun_mission_ids(compares: &[Compare]) -> HashSet<String> {
    let mut out = HashSet::new();
    for c in compares.iter().filter(|c| c.rerun_of.is_some()) {
        for slot in &c.slots {
            out.extend(slot.mission_ids.iter().cloned());
            out.extend(slot.retried.iter().cloned());
        }
        if let Some(j) = &c.judge {
            out.extend(j.mission_ids.iter().cloned());
            out.extend(j.panel_mission_ids.iter().cloned());
        }
    }
    out
}

pub fn compare_needs_copy(_runtime: &str) -> bool {
    false
}

/// Special case (`shared/compare.ts:262`): Antigravity without CLI cannot answer in a comparison.
pub fn compare_refusal_of(runtime: &str, antigravity_cli: bool) -> Option<&'static str> {
    if runtime == "antigravity" && !antigravity_cli {
        return Some(
            "Through its app, Antigravity answers only in the folder it has open, so it cannot answer in a comparison. Install Antigravity CLI and it can.",
        );
    }
    None
}

pub fn has_column_instructions(compare: &Compare) -> bool {
    compare.slots.iter().any(|s| s.route.instructions.is_some())
}

pub fn blind_name(slot: CompareSlotId, setups: bool) -> String {
    format!("{} {}", if setups { "Setup" } else { "Model" }, slot.upper())
}

pub fn versus_label(names: &[&str]) -> String {
    names.join(" vs ")
}

/// Local leaderboard: `runtime:model` -> `(kept, compared)`.
pub fn compare_record(compares: &[Compare]) -> BTreeMap<String, (usize, usize)> {
    let mut record = BTreeMap::new();
    for c in compares {
        let Some(kept) = &c.kept else { continue };
        if has_column_instructions(c) {
            continue;
        }
        for col in &c.slots {
            if col.mission_ids.is_empty() {
                continue;
            }
            let key = format!("{}:{}", col.route.runtime, col.route.model);
            let entry = record.entry(key).or_insert((0, 0));
            entry.1 += 1;
            if col.slot == kept.slot {
                entry.0 += 1;
            }
        }
    }
    record
}

/// The last "keep Answer X" counts; undefined when negated or not in `slots`.
pub fn judge_pick_of(text: &str, slots: &[CompareSlotId]) -> Option<CompareSlotId> {
    let re = crate::invariants::regex(r"(?i)(\bnot\s+|n't\s+)?\bkeep:?[\s*_]*answer\s+([a-c])\b");
    let last = re.captures_iter(text).last()?;
    if last.get(1).is_some() {
        return None;
    }
    let letter = last.get(2)?.as_str();
    let slot = CompareSlotId::parse(letter)?;
    slots.contains(&slot).then_some(slot)
}

pub fn with_column_folders(mut compare: Compare, copy_root: &Path) -> Compare {
    if !compare.changes || compare.changes_in == Some(ChangesIn::Folder) || compare.kept.is_some() || compare.settled {
        return compare;
    }
    for col in &mut compare.slots {
        col.folder = Some(
            copy_root
                .join(compare_tree_id(&compare.compare_id, col.slot))
                .to_string_lossy()
                .into_owned(),
        );
    }
    compare
}

pub fn shuffled_columns<T: Clone>(items: &[T], mut rand_below: impl FnMut(usize) -> usize) -> Vec<T> {
    let mut out = items.to_vec();
    for i in (1..out.len()).rev() {
        let j = rand_below(i + 1) % (i + 1);
        out.swap(i, j);
    }
    out
}

// ---- Store (`main/compare-store.ts`) --------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CreateCompareInput {
    pub teammate_id: Option<String>,
    pub prompt: String,
    pub routes: Vec<CompareRoute>,
    pub changes: bool,
    pub changes_in: Option<ChangesIn>,
    pub blind: bool,
    pub base: Option<String>,
    pub rerun_of: Option<String>,
}

pub struct CompareStore {
    root_dir: PathBuf,
    copy_root: Option<PathBuf>,
}

impl CompareStore {
    pub fn new(root_dir: PathBuf, copy_root: Option<PathBuf>) -> Result<Self, String> {
        if !root_dir.is_absolute() {
            return Err("Compare store directory is invalid".into());
        }
        Ok(Self { root_dir, copy_root })
    }

    pub fn from_store(store: &Store) -> Result<Self, String> {
        let root_dir = store.root().to_path_buf();
        let copy_root = crate::compare_copies::compare_root(store);
        Self::new(root_dir, Some(copy_root))
    }

    fn file_path(&self) -> PathBuf {
        self.root_dir.join("compares.json")
    }

    fn shown(&self, compare: Compare) -> Compare {
        match &self.copy_root {
            Some(root) => with_column_folders(compare, root),
            None => compare,
        }
    }

    fn read_all(&self) -> Result<Vec<Compare>, String> {
        let path = self.file_path();
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(UNREADABLE.into()),
        };
        if raw.len() > MAX_FILE_BYTES {
            return Err(UNREADABLE.into());
        }
        parsed_compare_file(&raw)
    }

    fn write_all(&self, compares: &[Compare]) -> Result<(), String> {
        fs::create_dir_all(&self.root_dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.root_dir, fs::Permissions::from_mode(0o700));
        }
        let body = json!({
            "schemaVersion": SCHEMA_VERSION,
            "compares": compares.iter().map(Compare::to_json).collect::<Vec<_>>(),
        });
        let text = format!(
            "{}\n",
            serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?
        );
        let path = self.file_path();
        let tmp = self.root_dir.join(format!(
            "compares.json.{}.{}.tmp",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
            }
            file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(())
    }

    fn change<F>(&self, compare_id: &str, edit: F) -> Result<Compare, String>
    where
        F: FnOnce(Compare) -> Result<Compare, String>,
    {
        let mut compares = self.read_all()?;
        let pos = compares
            .iter()
            .position(|c| c.compare_id == compare_id)
            .ok_or_else(|| "That comparison is no longer here.".to_string())?;
        let updated = edit(compares[pos].clone())?;
        compares[pos] = updated.clone();
        self.write_all(&compares)?;
        Ok(self.shown(updated))
    }

    pub fn list(&self) -> Result<Vec<Compare>, String> {
        Ok(self.read_all()?.into_iter().map(|c| self.shown(c)).collect())
    }

    pub fn get(&self, compare_id: &str) -> Result<Option<Compare>, String> {
        Ok(self
            .read_all()?
            .into_iter()
            .find(|c| c.compare_id == compare_id)
            .map(|c| self.shown(c)))
    }

    pub fn create(&self, input: CreateCompareInput) -> Result<Compare, String> {
        let prompt = input.prompt.trim().to_owned();
        for route in &input.routes {
            if let Some(inst) = &route.instructions
                && (input.teammate_id.is_none() || inst.trim().is_empty())
            {
                return Err("Choose a teammate and valid column instructions. Nothing was started.".into());
            }
        }
        if prompt.is_empty() || prompt.len() > MAX_PROMPT {
            return Err("There is nothing to compare.".into());
        }
        if !(MIN_COMPARE_SLOTS..=MAX_COMPARE_SLOTS).contains(&input.routes.len()) {
            return Err(format!(
                "A comparison is {MIN_COMPARE_SLOTS} or {MAX_COMPARE_SLOTS} models."
            ));
        }
        let mut compares = self.read_all()?;
        let compare_id = new_compare_id();
        let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let slots = input
            .routes
            .into_iter()
            .enumerate()
            .map(|(i, mut route)| {
                // Special case (compare-store.ts:313): `mode: 'auto'` only when `input.changes === true`.
                if !(route.mode.as_deref() == Some("auto") && input.changes) {
                    route.mode = None;
                }
                route.label = route.label.map(|l| l.chars().take(120).collect());
                route.why = route.why.and_then(|w| {
                    let t = w.trim();
                    (!t.is_empty()).then(|| t.chars().take(400).collect())
                });
                CompareSlot {
                    slot: COMPARE_SLOTS[i],
                    route,
                    mission_ids: Vec::new(),
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                }
            })
            .collect();
        let compare = Compare {
            compare_id,
            teammate_id: input.teammate_id,
            prompt,
            created_at,
            slots,
            changes: input.changes,
            changes_in: if input.changes { input.changes_in } else { None },
            blind: input.blind,
            kept: None,
            judge: None,
            base: input.base.filter(|b| is_commit_sha(b)),
            rerun_of: input.rerun_of,
            reruns: Vec::new(),
            settled: false,
            point_drafts: Vec::new(),
        };
        compares.push(compare.clone());
        if compares.len() > MAX_COMPARES {
            compares.drain(0..compares.len() - MAX_COMPARES);
        }
        self.write_all(&compares)?;
        Ok(self.shown(compare))
    }

    pub fn add_turn(&self, compare_id: &str, slot: CompareSlotId, mission_id: &str) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            let col = c
                .slots
                .iter_mut()
                .find(|s| s.slot == slot)
                .ok_or_else(|| "That comparison has no such column.".to_string())?;
            col.mission_ids.push(mission_id.to_owned());
            if col.mission_ids.len() > MAX_TURNS_PER_SLOT {
                col.mission_ids.drain(0..col.mission_ids.len() - MAX_TURNS_PER_SLOT);
            }
            Ok(c)
        })
    }

    pub fn retry(&self, compare_id: &str, slot: CompareSlotId, mission_id: &str) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            let col = c
                .slots
                .iter_mut()
                .find(|s| s.slot == slot)
                .ok_or_else(|| "That comparison has no such column.".to_string())?;
            col.refused = None;
            if let Some(replaced) = col.mission_ids.pop() {
                col.retried.push(replaced);
                if col.retried.len() > MAX_RETRIED_PER_SLOT {
                    col.retried.drain(0..col.retried.len() - MAX_RETRIED_PER_SLOT);
                }
            }
            col.mission_ids.push(mission_id.to_owned());
            Ok(c)
        })
    }

    pub fn refuse(&self, compare_id: &str, slot: CompareSlotId, why: &str) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            let col = c
                .slots
                .iter_mut()
                .find(|s| s.slot == slot)
                .ok_or_else(|| "That comparison has no such column.".to_string())?;
            col.refused = Some(why.chars().take(1_000).collect());
            Ok(c)
        })
    }

    pub fn keep(
        &self,
        compare_id: &str,
        slot: CompareSlotId,
        brought: Option<Vec<String>>,
    ) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            let col = c
                .slots
                .iter()
                .find(|s| s.slot == slot)
                .ok_or_else(|| "That column has nothing to keep yet.".to_string())?;
            if col.mission_ids.is_empty() {
                return Err("That column has nothing to keep yet.".into());
            }
            c.kept = Some(CompareKept {
                slot,
                at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                brought: brought.map(|b| b.into_iter().take(MAX_BROUGHT).collect()),
            });
            Ok(c)
        })
    }

    pub fn add_column(&self, compare_id: &str, mut route: CompareRoute) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            if c.slots.len() >= MAX_COMPARE_SLOTS {
                return Err(format!("A comparison is at most {MAX_COMPARE_SLOTS} models."));
            }
            if c.slots.iter().any(|col| {
                col.route.runtime == route.runtime
                    && col.route.model == route.model
                    && col.route.account == route.account
            }) {
                return Err("That model is already in this comparison.".into());
            }
            let slot = COMPARE_SLOTS
                .iter()
                .copied()
                .find(|letter| !c.slots.iter().any(|col| col.slot == *letter))
                .ok_or_else(|| format!("A comparison is at most {MAX_COMPARE_SLOTS} models."))?;
            route.mode = None;
            route.label = route.label.map(|l| l.chars().take(120).collect());
            c.slots.push(CompareSlot {
                slot,
                route,
                mission_ids: Vec::new(),
                refused: None,
                retried: Vec::new(),
                folder: None,
            });
            Ok(c)
        })
    }

    pub fn judge(
        &self,
        compare_id: &str,
        route: CompareRoute,
        mission_id: &str,
        criteria: Option<&str>,
        panel: Vec<JudgePanelMember>,
    ) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            let clean_route = |r: CompareRoute| CompareRoute {
                runtime: r.runtime,
                model: r.model,
                account: r.account,
                effort: r.effort,
                label: r.label.map(|l| l.chars().take(120).collect()),
                mode: None,
                instructions: None,
                why: None,
            };
            let members: Vec<JudgePanelMember> = panel
                .into_iter()
                .take(PANEL_JUDGES)
                .map(|m| JudgePanelMember {
                    route: clean_route(m.route),
                    mission_id: m.mission_id,
                })
                .collect();
            let mut mission_ids = c.judge.as_ref().map(|j| j.mission_ids.clone()).unwrap_or_default();
            mission_ids.push(mission_id.to_owned());
            if mission_ids.len() > MAX_JUDGES {
                mission_ids.drain(0..mission_ids.len() - MAX_JUDGES);
            }
            let mut heard = c
                .judge
                .as_ref()
                .map(|j| j.panel_mission_ids.clone())
                .unwrap_or_default();
            for m in &members {
                heard.push(m.mission_id.clone());
            }
            if heard.len() > MAX_JUDGES * PANEL_JUDGES {
                heard.drain(0..heard.len() - MAX_JUDGES * PANEL_JUDGES);
            }
            let criteria = criteria
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(MAX_JUDGE_CRITERIA).collect());
            c.judge = Some(CompareJudge {
                route: clean_route(route),
                mission_ids,
                criteria,
                panel: members,
                panel_mission_ids: heard,
            });
            Ok(c)
        })
    }

    pub fn add_point_draft(&self, compare_id: &str, mission_id: &str) -> Result<Compare, String> {
        self.change(compare_id, |mut c| {
            c.point_drafts.push(mission_id.to_owned());
            if c.point_drafts.len() > MAX_JUDGES {
                c.point_drafts.drain(0..c.point_drafts.len() - MAX_JUDGES);
            }
            Ok(c)
        })
    }

    pub fn remove(&self, compare_id: &str) -> Result<(), String> {
        let mut compares = self.read_all()?;
        compares.retain(|c| c.compare_id != compare_id);
        self.write_all(&compares)
    }
}

pub fn parsed_compare_file(raw: &str) -> Result<Vec<Compare>, String> {
    let val: Value = serde_json::from_str(raw).map_err(|_| UNREADABLE.to_string())?;
    let obj = val.as_object().ok_or_else(|| UNREADABLE.to_string())?;
    if obj.get("schemaVersion").and_then(Value::as_u64) != Some(SCHEMA_VERSION) {
        return Err(UNREADABLE.into());
    }
    let arr = obj
        .get("compares")
        .and_then(Value::as_array)
        .ok_or_else(|| UNREADABLE.to_string())?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for entry in arr {
        if let Some(c) = Compare::from_json(entry)
            && seen.insert(c.compare_id.clone())
        {
            out.push(c);
        }
    }
    if out.len() > MAX_COMPARES {
        out.drain(0..out.len() - MAX_COMPARES);
    }
    Ok(out)
}

fn new_compare_id() -> String {
    let n = crate::invariants::unique_nanos();
    format!("cmp_{n:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "locust-compare-{name}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_judges_pick_is_marked() {
        let slots = [CompareSlotId::A, CompareSlotId::B, CompareSlotId::C];
        let said = "Answer A is a working dodge loop.\n\nAnswer C puts you in a wheat field.\n\nI would keep Answer C, because it plays like a small arcade game.";
        assert_eq!(judge_pick_of(said, &slots), Some(CompareSlotId::C));
        assert_eq!(judge_pick_of("**Keep: Answer B**", &slots), Some(CompareSlotId::B));
        assert_eq!(judge_pick_of("I would **keep Answer A**.", &slots), Some(CompareSlotId::A));
        assert_eq!(
            judge_pick_of("At first I would keep Answer A. On reflection, I would keep Answer B.", &slots),
            Some(CompareSlotId::B)
        );
        assert_eq!(judge_pick_of("I would not keep Answer A.", &slots), None);
        assert_eq!(judge_pick_of("I wouldn't keep Answer B as it is.", &slots), None);
        assert_eq!(
            judge_pick_of("I would keep Answer C.", &[CompareSlotId::A, CompareSlotId::B]),
            None
        );
        assert_eq!(judge_pick_of("They are all fine.", &slots), None);
    }

    #[test]
    fn three_judges_are_kept() {
        let dir = temp_dir("three-judges");
        let store = CompareStore::new(dir.clone(), None).unwrap();
        let route = CompareRoute::new("opencode", "opencode/space-bunny-free");
        let route2 = CompareRoute::new("opencode", "opencode/muse-spark-1.3-contributor-free");
        let made = store
            .create(CreateCompareInput {
                teammate_id: None,
                prompt: "Which CRM?".into(),
                routes: vec![route, route2],
                changes: false,
                changes_in: None,
                blind: false,
                base: None,
                rerun_of: None,
            })
            .unwrap();
        let codex = CompareRoute::new("codex", "account-default");
        let ling = CompareRoute::new("opencode", "opencode/ling-3.1-flash-free");
        let nemo = CompareRoute::new("opencode", "opencode/nemotron-3-ultra-free");

        store
            .judge(
                &made.compare_id,
                codex.clone(),
                "m_j1",
                Some("Names a price"),
                vec![
                    JudgePanelMember { route: ling.clone(), mission_id: "m_p1".into() },
                    JudgePanelMember { route: nemo.clone(), mission_id: "m_p2".into() },
                ],
            )
            .unwrap();
        let judged = store
            .judge(
                &made.compare_id,
                codex.clone(),
                "m_j2",
                Some("Names a price"),
                vec![
                    JudgePanelMember { route: ling, mission_id: "m_p3".into() },
                    JudgePanelMember { route: nemo, mission_id: "m_p4".into() },
                ],
            )
            .unwrap();
        let panel_ids: Vec<&str> = judged
            .judge
            .as_ref()
            .unwrap()
            .panel
            .iter()
            .map(|m| m.mission_id.as_str())
            .collect();
        assert_eq!(panel_ids, vec!["m_p3", "m_p4"]);
        assert_eq!(
            newest_judge_mission_ids(judged.judge.as_ref()),
            vec!["m_j2", "m_p3", "m_p4"]
        );

        let reopened = CompareStore::new(dir.clone(), None)
            .unwrap()
            .get(&made.compare_id)
            .unwrap()
            .unwrap();
        assert_eq!(reopened.judge.as_ref().unwrap().panel, judged.judge.as_ref().unwrap().panel);
        let all_judge_ids = judge_mission_ids(&[reopened]);
        for id in ["m_j1", "m_j2", "m_p1", "m_p2", "m_p3", "m_p4"] {
            assert!(all_judge_ids.contains(id));
        }

        let alone = store
            .judge(&made.compare_id, codex, "m_j3", Some("Names a price"), Vec::new())
            .unwrap();
        assert!(alone.judge.as_ref().unwrap().panel.is_empty());
        assert!(judge_mission_ids(&[alone.clone()]).contains("m_p4"));
        let mut as_rerun = alone;
        as_rerun.rerun_of = Some("cmp_x".into());
        assert!(rerun_mission_ids(&[as_rerun]).contains("m_p1"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_mode_carries_over_only_when_changes_is_true() {
        let dir = temp_dir("auto-mode");
        let store = CompareStore::new(dir.clone(), None).unwrap();
        let mut r1 = CompareRoute::new("codex", "gpt-6");
        r1.mode = Some("auto".into());
        let r2 = CompareRoute::new("claude", "sonnet");

        // Answers-only: `auto` is stripped.
        let answers_only = store
            .create(CreateCompareInput {
                teammate_id: None,
                prompt: "Q".into(),
                routes: vec![r1.clone(), r2.clone()],
                changes: false,
                changes_in: None,
                blind: false,
                base: None,
                rerun_of: None,
            })
            .unwrap();
        assert_eq!(answers_only.slots[0].route.mode, None);

        // Changes: `auto` is kept.
        let with_changes = store
            .create(CreateCompareInput {
                teammate_id: None,
                prompt: "Q".into(),
                routes: vec![r1, r2],
                changes: true,
                changes_in: Some(ChangesIn::Copy),
                blind: false,
                base: None,
                rerun_of: None,
            })
            .unwrap();
        assert_eq!(with_changes.slots[0].route.mode.as_deref(), Some("auto"));

        // Re-reading from disk: non-`auto` modes are dropped.
        let raw = serde_json::json!({
            "schemaVersion": 1,
            "compares": [{
                "compareId": "cmp_read1",
                "prompt": "Q",
                "createdAt": "2026-11-13T00:00:00.000Z",
                "changes": true,
                "slots": [
                    {"slot": "a", "route": {"runtime": "codex", "model": "gpt-6", "mode": "accept-edits"}, "missionIds": []},
                    {"slot": "b", "route": {"runtime": "claude", "model": "sonnet", "mode": "auto"}, "missionIds": []}
                ]
            }]
        });
        let parsed = parsed_compare_file(&raw.to_string()).unwrap();
        assert_eq!(parsed[0].slots[0].route.mode, None);
        assert_eq!(parsed[0].slots[1].route.mode.as_deref(), Some("auto"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_record_counts_decided_comparisons() {
        let mut c1 = Compare {
            compare_id: "cmp_1".into(),
            teammate_id: None,
            prompt: "x".into(),
            created_at: "2026-09-28T12:00:00.000Z".into(),
            slots: vec![
                CompareSlot {
                    slot: CompareSlotId::A,
                    route: CompareRoute::new("opencode", "opencode/mimo-v2.6-flash-free"),
                    mission_ids: vec!["cmp_1_a".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
                CompareSlot {
                    slot: CompareSlotId::B,
                    route: CompareRoute::new("claude", "sonnet"),
                    mission_ids: vec!["cmp_1_b".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
            ],
            changes: false,
            changes_in: None,
            blind: false,
            kept: Some(CompareKept {
                slot: CompareSlotId::B,
                at: "2026-09-28T12:05:00.000Z".into(),
                brought: None,
            }),
            judge: None,
            base: None,
            rerun_of: None,
            reruns: Vec::new(),
            settled: false,
            point_drafts: Vec::new(),
        };
        let mut c2 = c1.clone();
        c2.compare_id = "cmp_2".into();
        c2.kept = Some(CompareKept {
            slot: CompareSlotId::A,
            at: "2026-09-28T12:05:00.000Z".into(),
            brought: None,
        });
        let mut c3 = c1.clone();
        c3.compare_id = "cmp_3".into();
        // Undecided
        c1.kept = None;
        let rec = compare_record(&[c1, c2, c3]);
        assert_eq!(rec.get("claude:sonnet"), Some(&(1, 2)));
        assert_eq!(rec.get("opencode:opencode/mimo-v2.6-flash-free"), Some(&(1, 2)));
    }
}
