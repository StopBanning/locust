//! Host-owned file receipts, folder selections, import previews and exports (`main/routine-io.ts`).

use crate::people;
use crate::routine_file::{
    FlaggedPath, MAX_ROUTINE_FILE_BYTES, RoutineFile, absolute_paths_in, parse_routine_file,
    paths_as_inputs, routine_export_text, routine_file_name, routine_to_file,
};
use crate::routine_inputs::{RoutineInput, RoutineInputKind, parse_inputs_array, resolve_values};
use crate::routine_templates::{
    read_shipped_template, template_changes_files, template_on_another_agent,
};
use crate::routines;
use crate::store::Store;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

const MAX_FOLDERS: usize = 100;
const MAX_PREVIEWS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewOrigin {
    File,
    Template,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectorPresence {
    pub name: String,
    pub present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineImportPreview {
    pub token: String,
    pub name: String,
    pub steps: Vec<String>,
    pub inputs: Vec<RoutineInput>,
    pub hand_off_roles: Vec<Option<String>>,
    pub hand_off_checks: Vec<bool>,
    pub hand_off_asks: Option<Vec<bool>>,
    pub changes_files: bool,
    pub hand_off_on_another_agent: Option<Vec<bool>>,
    pub runtime: Option<String>,
    pub connectors: Vec<ConnectorPresence>,
}

struct HeldPreview {
    file: RoutineFile,
    workspace: Option<String>,
    from: PreviewOrigin,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExportOutcome {
    AbsolutePaths(Vec<FlaggedPath>),
    Ready {
        text: String,
        markdown: bool,
        file_name: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportPaths {
    Input,
    Keep,
}

#[derive(Default)]
pub struct RoutineIO {
    folders: Vec<String>,
    previews: HashMap<String, HeldPreview>,
    preview_order: Vec<String>,
}

impl RoutineIO {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_folder(&mut self, path: &str) {
        if let Some(pos) = self.folders.iter().position(|p| p == path) {
            self.folders.remove(pos);
        }
        self.folders.push(path.to_owned());
        if self.folders.len() > MAX_FOLDERS {
            self.folders.remove(0);
        }
    }

    pub fn folder_was_chosen(&self, path: &str) -> bool {
        self.folders.iter().any(|p| p == path)
    }

    pub fn chosen_folders(&self) -> HashSet<String> {
        self.folders.iter().cloned().collect()
    }

    /// Validate input values for a routine run, enforcing the folder-picker rule.
    pub fn validate_run_inputs(
        &self,
        routine: &Value,
        given: Option<&Value>,
    ) -> Result<Option<BTreeMap<String, String>>, String> {
        let inputs: Vec<RoutineInput> = routine
            .get("inputs")
            .and_then(parse_inputs_array)
            .unwrap_or_default();
        let settled = resolve_values(&inputs, given)?;
        for input in &inputs {
            if input.kind == RoutineInputKind::Folder
                && let Some(path) = settled.get(&input.key)
                && !path.is_empty()
                && !self.folder_was_chosen(path)
            {
                return Err(format!(
                    "Choose \"{}\" with the folder picker before running.",
                    input.label
                ));
            }
        }
        if given.is_none() {
            Ok(None)
        } else {
            Ok(Some(settled))
        }
    }

    fn preview_of(
        &mut self,
        file: RoutineFile,
        from: PreviewOrigin,
        workspace: Option<&str>,
        connector_names: &[String],
        changes_files: bool,
        on_another_agent: Option<&[bool]>,
    ) -> RoutineImportPreview {
        let token = crate::native::uuid_v4()
            .unwrap_or_else(|_| format!("tok_{}", crate::invariants::unique_nanos()));
        if self.previews.len() >= MAX_PREVIEWS
            && let Some(oldest) = self.preview_order.first().cloned()
        {
            self.preview_order.remove(0);
            self.previews.remove(&oldest);
        }
        self.preview_order.push(token.clone());
        self.previews.insert(
            token.clone(),
            HeldPreview {
                file: file.clone(),
                workspace: workspace.map(ToOwned::to_owned),
                from,
            },
        );
        let present_set: HashSet<String> = connector_names
            .iter()
            .map(|n| n.to_lowercase())
            .collect();
        let has_asks = file.hand_offs.iter().any(|h| h.ask_first == Some(true));
        RoutineImportPreview {
            token,
            name: file.name,
            steps: file.steps,
            inputs: file.inputs,
            hand_off_roles: file.hand_offs.iter().map(|h| h.role.clone()).collect(),
            hand_off_checks: file
                .hand_offs
                .iter()
                .map(|h| h.check == Some(true))
                .collect(),
            hand_off_asks: has_asks.then(|| {
                file.hand_offs
                    .iter()
                    .map(|h| h.ask_first == Some(true))
                    .collect()
            }),
            changes_files,
            hand_off_on_another_agent: on_another_agent.map(|s| s.to_vec()),
            runtime: file.route.runtime,
            connectors: file
                .connectors
                .iter()
                .map(|name| ConnectorPresence {
                    name: name.clone(),
                    present: present_set.contains(&name.to_lowercase()),
                })
                .collect(),
        }
    }

    pub fn preview_template(
        &mut self,
        id: &str,
        workspace: Option<&str>,
        connector_names: &[String],
    ) -> Result<RoutineImportPreview, String> {
        let file = read_shipped_template(id)?;
        Ok(self.preview_of(
            file,
            PreviewOrigin::Template,
            workspace,
            connector_names,
            template_changes_files(id),
            template_on_another_agent(id),
        ))
    }

    pub fn preview_file_text(
        &mut self,
        text: &str,
        workspace: Option<&str>,
        connector_names: &[String],
    ) -> Result<RoutineImportPreview, String> {
        let file = parse_routine_file(text)?;
        Ok(self.preview_of(
            file,
            PreviewOrigin::File,
            workspace,
            connector_names,
            false,
            None,
        ))
    }

    pub fn preview_file_path(
        &mut self,
        path: &Path,
        workspace: Option<&str>,
        connector_names: &[String],
    ) -> Result<RoutineImportPreview, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() as usize > MAX_ROUTINE_FILE_BYTES {
            return Err("That file is too large to be a routine.".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.preview_file_text(&text, workspace, connector_names)
    }

    /// Consume a preview token and save the routine into `store` (Ask mode, no clock schedule).
    pub fn import_preview(
        &mut self,
        store: &Store,
        token: &str,
        teammate_id: &str,
        current_workspace: Option<&str>,
    ) -> Result<Value, String> {
        let Some(held) = self.previews.get(token) else {
            return Err("Open the routine again before importing it.".into());
        };
        if held.workspace.as_deref() != current_workspace || held.workspace.is_none() {
            return Err(match held.from {
                PreviewOrigin::Template => {
                    "Choose the folder your teammates work in first: a routine runs in the folder it is added in."
                        .into()
                }
                PreviewOrigin::File => {
                    "Open the project folder and preview the file there before importing it.".into()
                }
            });
        }
        let team = people::team(store)?;
        let mate_exists = team["teammates"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|t| t["teammateId"] == teammate_id));
        if !mate_exists {
            return Err("Choose a teammate who is still on this team.".into());
        }
        let held = self.previews.remove(token).ok_or_else(|| {
            "This preview was already imported. Open the routine file again to import another copy."
                .to_string()
        })?;
        self.preview_order.retain(|t| t != token);

        let mut entry = json!({
            "name": held.file.name,
            "steps": held.file.steps,
            "inputs": crate::routine_inputs::kept_inputs_value(&held.file.inputs),
            "teammateId": teammate_id,
            "mode": "ask",
        });
        if let Some(sched) = &held.file.schedule {
            entry["schedule"] = json!({"kind": sched.kind});
        }
        if held.file.hand_offs.iter().any(|h| h.ask_first == Some(true)) {
            let hos: Vec<Value> = held
                .file
                .hand_offs
                .iter()
                .map(|h| {
                    if h.ask_first == Some(true) {
                        json!({"askFirst": true})
                    } else {
                        json!({})
                    }
                })
                .collect();
            entry["handOffs"] = Value::Array(hos);
        }
        routines::save(store, entry)?;
        let saved = routines::list(store)?;
        let last = saved["routines"]
            .as_array()
            .and_then(|arr| arr.last())
            .cloned()
            .unwrap_or(Value::Null);
        Ok(last)
    }

    pub fn prepare_export(
        &self,
        routine: &Value,
        paths: Option<ExportPaths>,
        team: &[Value],
        connector_names: &[String],
    ) -> Result<ExportOutcome, String> {
        let steps: Vec<String> = routine["steps"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let flagged = absolute_paths_in(&steps);
        if !flagged.is_empty() && paths.is_none() {
            return Ok(ExportOutcome::AbsolutePaths(flagged));
        }
        let inputs: Vec<RoutineInput> = routine
            .get("inputs")
            .and_then(parse_inputs_array)
            .unwrap_or_default();
        let (use_steps, use_inputs) = match paths {
            Some(ExportPaths::Input) => paths_as_inputs(&steps, &inputs)?,
            _ => (steps, inputs),
        };
        let role_of = |id: &str| -> Option<String> {
            let mate = team.iter().find(|m| m["teammateId"] == id)?;
            let role = mate["role"].as_str()?;
            if role == "Custom" {
                mate["roleTitle"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .or_else(|| Some("Custom".into()))
            } else {
                Some(role.to_owned())
            }
        };
        let file = routine_to_file(routine, &use_steps, &use_inputs, &role_of, connector_names);
        let (text, markdown) = routine_export_text(&file);
        parse_routine_file(&text)?;
        let file_name = routine_file_name(routine["name"].as_str().unwrap_or("routine"), markdown);
        Ok(ExportOutcome::Ready {
            text,
            markdown,
            file_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routine_file::routine_file_text;

    fn test_store() -> (Store, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "locust-rio-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::new(dir.clone(), dir.join("electron")).unwrap();
        people::save_teammate(
            &store,
            json!({
                "name": "One",
                "role": "Custom",
                "roleTitle": "Helper",
                "instructions": "",
                "folder": dir.to_string_lossy(),
                "route": {"runtime": "codex", "model": "gpt-5", "mode": "auto"}
            }),
        )
        .unwrap();
        (store, dir)
    }

    #[test]
    fn refuses_typed_folder_paths_until_picker_selected_them() {
        let (store, dir) = test_store();
        let team = people::team(&store).unwrap();
        let tm_id = team["teammates"][0]["teammateId"].as_str().unwrap();
        let routine = json!({
            "name": "Folder check",
            "steps": ["Read {{folder}}"],
            "inputs": [{"key": "folder", "label": "Folder", "kind": "folder", "required": true}],
            "teammateId": tm_id,
        });
        let mut io = RoutineIO::new();
        let dir_str = dir.to_string_lossy().into_owned();
        let err = io
            .validate_run_inputs(&routine, Some(&json!({"folder": dir_str})))
            .unwrap_err();
        assert!(err.contains("folder picker"), "unexpected err: {err}");
        assert!(io.validate_run_inputs(&routine, None).is_err());

        io.register_folder(&dir_str);
        assert!(io.folder_was_chosen(&dir_str));
        let ok = io
            .validate_run_inputs(&routine, Some(&json!({"folder": dir_str})))
            .unwrap()
            .unwrap();
        assert_eq!(ok["folder"], dir_str);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_previews_and_imports_once_in_ask_mode_requiring_workspace() {
        let (store, dir) = test_store();
        let team = people::team(&store).unwrap();
        let tm_id = team["teammates"][0]["teammateId"].as_str().unwrap().to_owned();
        let mut io = RoutineIO::new();

        // Missing workspace refuses with template-specific words (not mentioning "file").
        let preview_no_ws = io.preview_template("explain-this-project", None, &[]).unwrap();
        let err = io
            .import_preview(&store, &preview_no_ws.token, &tm_id, None)
            .unwrap_err();
        assert!(err.contains("Choose the folder your teammates work in first"));
        assert!(!err.to_lowercase().contains("file"));

        // With workspace, imports once in ask mode with no schedule.
        let preview = io
            .preview_template("draft-a-reply", Some("ws_test"), &[])
            .unwrap();
        assert_eq!(preview.name, "Draft a reply");
        assert!(preview.connectors.is_empty());
        let imported = io
            .import_preview(&store, &preview.token, &tm_id, Some("ws_test"))
            .unwrap();
        assert_eq!(imported["name"], "Draft a reply");
        assert_eq!(imported["route"]["mode"], "ask");
        assert_eq!(imported["runs"], 0);
        assert!(imported.get("schedule").is_none());

        // Second import with same token fails.
        assert!(
            io.import_preview(&store, &preview.token, &tm_id, Some("ws_test"))
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_preview_connectors_and_export_path_flagging() {
        let (store, dir) = test_store();
        let team = people::team(&store).unwrap();
        let mates = team["teammates"].as_array().unwrap().clone();
        let tm_id = mates[0]["teammateId"].as_str().unwrap().to_owned();
        let mut io = RoutineIO::new();

        let routine = json!({
            "name": "Notes",
            "teammateId": tm_id,
            "steps": ["Read C:\\work\\notes."],
            "inputs": [],
            "route": {"runtime": "codex", "mode": "ask"}
        });
        let ExportOutcome::AbsolutePaths(flagged) =
            io.prepare_export(&routine, None, &mates, &[]).unwrap()
        else {
            panic!("expected flagged paths");
        };
        assert_eq!(flagged.len(), 1);

        let ExportOutcome::Ready {
            text,
            markdown,
            file_name,
        } = io
            .prepare_export(&routine, Some(ExportPaths::Input), &mates, &[])
            .unwrap()
        else {
            panic!("expected ready export");
        };
        assert!(markdown);
        assert_eq!(file_name, "Notes.routine.md");
        assert!(text.contains("Read {{path}}."));

        // Preview file with connectors
        let file = RoutineFile {
            format: "locust-routine".into(),
            version: 1,
            name: "With conn".into(),
            schedule: None,
            steps: vec!["Use present and missing.".into()],
            inputs: vec![],
            hand_offs: vec![Default::default()],
            route: Default::default(),
            connectors: vec!["present".into(), "missing".into()],
        };
        let prev = io
            .preview_file_text(&routine_file_text(&file), Some("ws_test"), &["present".into()])
            .unwrap();
        assert_eq!(
            prev.connectors,
            vec![
                ConnectorPresence { name: "present".into(), present: true },
                ConnectorPresence { name: "missing".into(), present: false },
            ]
        );
        assert!(io.import_preview(&store, &prev.token, &tm_id, Some("ws_other")).is_err());
        assert!(io.import_preview(&store, "invented", &tm_id, Some("ws_test")).is_err());
        assert!(io.import_preview(&store, &prev.token, "tm_gone", Some("ws_test")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
