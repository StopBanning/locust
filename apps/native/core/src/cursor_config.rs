//! A Cursor run leaves the person's own Cursor as it found it (Electron's `main/cursor-default-model.ts`).
//!
//! `cursor-agent --model X` does not choose a model for one run: it saves X as the person's Cursor default in
//! `~/.cursor/cli-config.json`. Only the three model fields are touched (`model`, `selectedModel`,
//! `modelSelectionHistory`). The file also holds the person's Cursor sign-in: it is read and written back whole, never
//! looked at, copied or logged, and the kept copy holds the three model fields alone.
//!
//! Put back only when the default is still the model the run chose; if the person switched models meanwhile, theirs
//! stands. With runs overlapping, the state before the FIRST is what the LAST puts back. What was kept is also written
//! to `kept_file`, so a Locust that quit mid-run puts it back on its next start (`recover`).

use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const FIELDS: [&str; 3] = ["model", "selectedModel", "modelSelectionHistory"];

#[derive(Clone)]
struct Kept {
    fields: Map<String, Value>,
    run_model: String,
}

impl Kept {
    fn to_json(&self) -> String {
        let mut o = Map::new();
        o.insert("fields".into(), Value::Object(self.fields.clone()));
        o.insert("runModel".into(), Value::String(self.run_model.clone()));
        Value::Object(o).to_string()
    }
}

/// The model id a run names, without its bracketed options: `composer-2.5[fast=true]` is `composer-2.5`.
pub fn base_model_id(model: &str) -> String {
    match model.find('[') {
        Some(i) => model[..i].trim().to_owned(),
        None => model.trim().to_owned(),
    }
}

/// Whether the id Cursor saved is the model a run named. Cursor saves the BASE id with the rest as parameters
/// (measured: `composer-2.5-fast` was saved as `composer-2.5` with `fast=true`).
pub fn named_model(run_model: &str, saved_id: Option<&str>) -> bool {
    let Some(saved) = saved_id else { return false };
    let ran = base_model_id(run_model).to_lowercase();
    let saved = saved.to_lowercase();
    ran == saved || ran.starts_with(&format!("{saved}-")) || saved.starts_with(&format!("{ran}-"))
}

fn id_of(value: Option<&Value>) -> Option<String> {
    value?.get("modelId")?.as_str().map(str::to_owned)
}

fn read(path: &PathBuf) -> Option<(String, Map<String, Value>)> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&text).ok()? {
        Value::Object(map) => Some((text, map)),
        _ => None,
    }
}

fn fields_of(config: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for f in FIELDS {
        if let Some(v) = config.get(f) {
            out.insert(f.into(), v.clone());
        }
    }
    out
}

#[derive(Default)]
struct State {
    active: usize,
    kept: Option<Kept>,
}

pub struct CursorDefaultModel {
    file: PathBuf,
    kept_file: PathBuf,
    state: Mutex<State>,
}

impl CursorDefaultModel {
    pub fn new(file: PathBuf, kept_file: PathBuf) -> Self {
        Self { file, kept_file, state: Mutex::new(State::default()) }
    }

    /// Before a Cursor run that names `run_model`.
    pub fn before(&self, run_model: &str) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.active == 0 {
            s.kept = read(&self.file).map(|(_, c)| Kept { fields: fields_of(&c), run_model: run_model.to_owned() });
        } else if let Some(k) = s.kept.as_mut() {
            // A later run's model is what the default will show when the last ends.
            k.run_model = run_model.to_owned();
        }
        if let Some(k) = &s.kept {
            let _ = fs::write(&self.kept_file, k.to_json());
        }
        s.active += 1;
    }

    /// After it ends, however it ended.
    pub fn after(&self) {
        let record = {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            s.active = s.active.saturating_sub(1);
            if s.active > 0 {
                return;
            }
            s.kept.take()
        };
        if let Some(k) = record {
            self.put_back(&k);
            let _ = fs::remove_file(&self.kept_file);
        }
    }

    /// At start: finish what a Locust that quit mid-run could not.
    pub fn recover(&self) {
        {
            let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.active > 0 {
                return;
            }
        }
        let Ok(text) = fs::read_to_string(&self.kept_file) else { return };
        let record = serde_json::from_str::<Value>(&text).ok().and_then(|v| {
            let run_model = v.get("runModel")?.as_str()?.to_owned();
            let fields = v.get("fields").and_then(Value::as_object).cloned().unwrap_or_default();
            Some(Kept { fields, run_model })
        });
        if let Some(k) = record {
            self.put_back(&k);
        }
        let _ = fs::remove_file(&self.kept_file);
    }

    fn put_back(&self, record: &Kept) {
        let Some((text, config)) = read(&self.file) else { return };
        let current = fields_of(&config);
        let same = |f: &str| current.get(f).unwrap_or(&Value::Null) == record.fields.get(f).unwrap_or(&Value::Null);
        if FIELDS.iter().all(|f| same(f)) {
            return;
        }
        let named = named_model(&record.run_model, id_of(current.get("model")).as_deref())
            || named_model(&record.run_model, id_of(current.get("selectedModel")).as_deref());
        if !named {
            return; // the person chose another model themselves: theirs stands
        }
        let mut restored = config.clone();
        for f in FIELDS {
            match record.fields.get(f) {
                Some(v) => {
                    restored.insert(f.into(), v.clone());
                }
                None => {
                    restored.shift_remove(f);
                }
            }
        }
        // Written the way Cursor writes it: its own indent, no final newline unless the file had one.
        let indent = text
            .strip_prefix("{\n")
            .or_else(|| text.strip_prefix("{\r\n"))
            .map(|r| r.chars().take_while(|c| *c == ' ').count())
            .unwrap_or(0);
        let mut body = if indent == 0 {
            Value::Object(restored).to_string()
        } else {
            serde_json::to_string_pretty(&Value::Object(restored))
                .unwrap_or_default()
                .lines()
                .map(|l| {
                    let lead = l.chars().take_while(|c| *c == ' ').count();
                    format!("{}{}", " ".repeat(lead / 2 * indent), &l[lead..])
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        if text.ends_with('\n') {
            body.push('\n');
        }
        let tmp = self.file.with_extension(format!("json.locust-{}.tmp", std::process::id()));
        if fs::write(&tmp, body).is_ok() && fs::rename(&tmp, &self.file).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }
}

#[cfg(test)]
#[path = "cursor_config_tests.rs"]
mod tests;
