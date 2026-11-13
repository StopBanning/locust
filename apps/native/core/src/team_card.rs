//! Team image import/export with routines (`shared/team-card.ts`, `main/team-card-routines.ts`).

use crate::people;
use crate::png_text::{read_text_chunk, render_team_card_png, with_text_chunk};
use crate::routine_file::{
    RoutineFile, parse_routine_file, paths_as_inputs, routine_file_text, routine_to_file,
};
use crate::routine_inputs::{RoutineInput, kept_inputs_value, parse_inputs_array};
use crate::routines;
use crate::store::Store;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::Path;

pub const TEAM_CARD_KEYWORD: &str = "locust-team";
pub const MAX_TEAM_CARD_TEAMMATES: usize = 12;
pub const MAX_TEAM_CARD_ROUTINES: usize = 24;
pub const MAX_TEAM_CARD_FILE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct TeamCardRoutine {
    pub teammate: usize,
    pub routine: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamCardImportResult {
    pub added: Vec<String>,
    pub skipped: Vec<String>,
    pub instructed: Vec<String>,
    pub routines: Vec<String>,
    pub routines_left: Vec<String>,
}

fn is_own_model(model: &str) -> bool {
    let Some(rest) = model
        .get(..4)
        .filter(|p| p.eq_ignore_ascii_case("own-"))
        .and_then(|_| model.get(4..))
    else {
        return false;
    };
    let Some(slash) = rest.find('/') else {
        return false;
    };
    slash > 0 && rest[..slash].bytes().all(|b| b.is_ascii_alphanumeric())
}

fn role_label_of(mate: &Value) -> Option<String> {
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
}

/// What a card carries for these teammates (strips ids, folders, limits, worktree, own models).
pub fn team_card_of(teammates: &[Value]) -> Value {
    let entries: Vec<Value> = teammates
        .iter()
        .take(MAX_TEAM_CARD_TEAMMATES)
        .map(|t| {
            let mut obj = Map::new();
            obj.insert("name".into(), t["name"].clone());
            obj.insert("role".into(), t["role"].clone());
            if let Some(rt) = t.get("roleTitle").filter(|v| !v.is_null()) {
                obj.insert("roleTitle".into(), rt.clone());
            }
            obj.insert("hue".into(), t["hue"].clone());
            if let Some(av) = t.get("avatar").filter(|v| v.is_object()) {
                obj.insert("avatar".into(), av.clone());
            }
            if let Some(route) = t.get("route").filter(|v| v.is_object())
                && !route["model"].as_str().is_some_and(is_own_model)
                && route["runtime"].as_str().is_some_and(|s| !s.is_empty())
                && route["mode"].as_str().is_some_and(|s| !s.is_empty())
            {
                let mut r = Map::new();
                r.insert("runtime".into(), route["runtime"].clone());
                r.insert("model".into(), route["model"].clone());
                r.insert("mode".into(), route["mode"].clone());
                if let Some(eff) = route.get("effort").filter(|v| !v.is_null()) {
                    r.insert("effort".into(), eff.clone());
                }
                obj.insert("route".into(), Value::Object(r));
            }
            if let Some(instr) = t["instructions"].as_str().filter(|s| !s.trim().is_empty()) {
                obj.insert("instructions".into(), json!(instr));
            }
            if let Some(starters) = t["starters"].as_array().filter(|a| !a.is_empty()) {
                obj.insert("starters".into(), Value::Array(starters.clone()));
            }
            Value::Object(obj)
        })
        .collect();
    json!({
        "schema": 1,
        "teammates": entries
    })
}

/// The team's routines on its card (`main/team-card-routines.ts`).
pub fn team_card_routines(
    team: &[Value],
    routines: &[Value],
    workspace_id: Option<&str>,
    connector_names: &[String],
) -> Vec<TeamCardRoutine> {
    let carried: Vec<&Value> = team.iter().take(MAX_TEAM_CARD_TEAMMATES).collect();
    let mut out = Vec::new();
    for routine in routines {
        if out.len() >= MAX_TEAM_CARD_ROUTINES {
            break;
        }
        let tm_id = routine["teammateId"].as_str().unwrap_or("");
        let Some(place) = carried.iter().position(|m| m["teammateId"] == tm_id) else {
            continue;
        };
        if let Some(r_ws) = routine["workspaceId"].as_str()
            && Some(r_ws) != workspace_id
        {
            continue;
        }
        let steps: Vec<String> = routine["steps"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let inputs: Vec<RoutineInput> = routine
            .get("inputs")
            .and_then(parse_inputs_array)
            .unwrap_or_default();
        let (kept_steps, kept_inputs) = paths_as_inputs(&steps, &inputs).unwrap_or((steps, inputs));
        let role_of = |id: &str| team.iter().find(|e| e["teammateId"] == id).and_then(role_label_of);
        let file = routine_to_file(routine, &kept_steps, &kept_inputs, &role_of, connector_names);
        if parse_routine_file(&routine_file_text(&file)).is_ok() {
            out.push(TeamCardRoutine {
                teammate: place,
                routine: file.to_json(),
            });
        }
    }
    out
}

/// A card's routine as a `RoutineFile`, read as an imported `.locust-routine.json` is.
pub fn card_routine_file(entry: &TeamCardRoutine) -> Option<RoutineFile> {
    let text = serde_json::to_string(&entry.routine).ok()?;
    parse_routine_file(&text).ok()
}

/// The card's routines, as far as they can be read.
pub fn read_team_card_routines(value: &Value, teammates: usize) -> Vec<TeamCardRoutine> {
    let Some(routines) = value.get("routines").and_then(Value::as_array) else {
        return Vec::new();
    };
    let cap = teammates.min(MAX_TEAM_CARD_TEAMMATES);
    routines
        .iter()
        .take(MAX_TEAM_CARD_ROUTINES)
        .filter_map(|entry| {
            let obj = entry.as_object()?;
            let tm = obj.get("teammate")?;
            let idx = tm.as_u64()? as usize;
            if idx >= cap {
                return None;
            }
            Some(TeamCardRoutine {
                teammate: idx,
                routine: obj.get("routine").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

/// The card's teammates, as far as they can be read.
/// Safety rule (`team-card.ts:113`): a teammate read from someone's team card never arrives in Auto.
pub fn read_team_card(value: &Value) -> Option<Vec<Value>> {
    let obj = value.as_object()?;
    if obj.get("schema").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let arr = obj.get("teammates")?.as_array()?;
    let out = arr
        .iter()
        .take(MAX_TEAM_CARD_TEAMMATES)
        .filter_map(|entry| {
            let mut cloned = entry.as_object()?.clone();
            if let Some(route) = cloned.get("route").and_then(Value::as_object) {
                let mut safe_route = route.clone();
                if safe_route.get("mode").and_then(Value::as_str) == Some("auto") {
                    safe_route.insert("mode".into(), json!("accept-edits"));
                }
                cloned.insert("route".into(), Value::Object(safe_route));
            }
            Some(Value::Object(cloned))
        })
        .collect();
    Some(out)
}

/// A name not already on the roster: "Wren", else "Wren 2", "Wren 3".
pub fn free_name(name: &str, taken: &HashSet<String>) -> String {
    let lower: HashSet<String> = taken.iter().map(|s| s.to_lowercase()).collect();
    if !lower.contains(&name.to_lowercase()) {
        return name.to_owned();
    }
    for n in 2..100 {
        let candidate = format!("{name} {n}");
        if !lower.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("{name} {}", crate::invariants::unique_nanos() % 1000)
}

/// Export the current team and its routines in `workspace_id` into a PNG buffer.
pub fn export_team_card(
    store: &Store,
    routine_store: Option<&Store>,
    workspace_id: Option<&str>,
    connector_names: &[String],
) -> Result<Vec<u8>, String> {
    let team_file = people::team(store)?;
    let team = team_file["teammates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let routines_list = routine_store
        .and_then(|rs| routines::list(rs).ok())
        .and_then(|v| v["routines"].as_array().cloned())
        .unwrap_or_default();
    let carried = team_card_routines(&team, &routines_list, workspace_id, connector_names);
    let mut card = team_card_of(&team);
    if !carried.is_empty() {
        let routines_json: Vec<Value> = carried
            .into_iter()
            .map(|c| json!({"teammate": c.teammate, "routine": c.routine}))
            .collect();
        card["routines"] = Value::Array(routines_json);
    }
    let png = render_team_card_png(&team);
    with_text_chunk(&png, TEAM_CARD_KEYWORD, &card.to_string())
}

/// Import a team and its routines from PNG bytes.
pub fn import_team_card_bytes(
    store: &Store,
    routine_store: &Store,
    workspace_id: Option<&str>,
    png_bytes: &[u8],
) -> Result<TeamCardImportResult, String> {
    let text = read_text_chunk(png_bytes, TEAM_CARD_KEYWORD)
        .ok_or_else(|| "That image is not a Locust team card: it carries no team.".to_string())?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| "That image could not be read as a Locust team card.".to_string())?;
    let entries = read_team_card(&parsed)
        .ok_or_else(|| "That image is not a Locust team card: it carries no team.".to_string())?;

    let existing_team = people::team(store)?;
    let mut taken: HashSet<String> = existing_team["teammates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["name"].as_str().map(ToOwned::to_owned))
        .collect();

    let mut result = TeamCardImportResult::default();
    let mut made: Vec<Option<(String, String, Option<Value>)>> = Vec::with_capacity(entries.len());

    for entry in &entries {
        let given = entry["name"].as_str().unwrap_or("").trim();
        let name = free_name(given, &taken);
        let mut save_entry = json!({
            "name": name,
            "hue": entry.get("hue").and_then(Value::as_str).unwrap_or("lime"),
            "role": entry.get("role").and_then(Value::as_str).unwrap_or("Custom"),
            "instructions": entry.get("instructions").and_then(Value::as_str).unwrap_or(""),
        });
        if let Some(rt) = entry.get("roleTitle").filter(|v| !v.is_null()) {
            save_entry["roleTitle"] = rt.clone();
        }
        if let Some(av) = entry.get("avatar").filter(|v| v.is_object()) {
            save_entry["avatar"] = av.clone();
        }
        if let Some(st) = entry.get("starters").filter(|v| v.is_array()) {
            save_entry["starters"] = st.clone();
        }
        let route = entry
            .get("route")
            .filter(|r| {
                r.is_object()
                    && r["runtime"].as_str().is_some_and(|s| !s.is_empty())
                    && r["mode"].as_str().is_some_and(|s| !s.is_empty())
            })
            .cloned();
        if let Some(r) = &route {
            save_entry["route"] = r.clone();
        }
        match people::save_teammate_id(store, save_entry) {
            Ok(tm_id) => {
                taken.insert(name.clone());
                result.added.push(name.clone());
                if entry["instructions"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty())
                {
                    result.instructed.push(name.clone());
                }
                made.push(Some((tm_id, name, route)));
            }
            Err(_) => {
                result.skipped.push(if given.is_empty() {
                    "A teammate with no name".into()
                } else {
                    given.to_owned()
                });
                made.push(None);
            }
        }
    }

    for entry in read_team_card_routines(&parsed, entries.len()) {
        let file = card_routine_file(&entry);
        let owner = made.get(entry.teammate).and_then(|o| o.as_ref());
        let name = file
            .as_ref()
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "A routine".into());
        let (Some(file), Some((tm_id, owner_name, owner_route))) = (file, owner) else {
            result.routines_left.push(format!("{name} (it did not read)"));
            continue;
        };
        if workspace_id.is_none() {
            result
                .routines_left
                .push(format!("{name} (open a project folder for it first)"));
            continue;
        }
        if owner_route.is_none() {
            result
                .routines_left
                .push(format!("{name} ({owner_name} has no model yet)"));
            continue;
        }
        let mut r_entry = json!({
            "name": file.name,
            "steps": file.steps,
            "inputs": kept_inputs_value(&file.inputs),
            "teammateId": tm_id,
            "mode": "ask",
        });
        if file.hand_offs.iter().any(|h| h.ask_first == Some(true)) {
            let hos: Vec<Value> = file
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
            r_entry["handOffs"] = Value::Array(hos);
        }
        match routines::save(routine_store, r_entry) {
            Ok(()) => result.routines.push(file.name),
            Err(_) => result
                .routines_left
                .push(format!("{name} (it could not be saved)")),
        }
    }

    Ok(result)
}

pub fn import_team_card_file(
    store: &Store,
    routine_store: &Store,
    workspace_id: Option<&str>,
    path: &Path,
) -> Result<TeamCardImportResult, String> {
    let meta = std::fs::metadata(path)
        .map_err(|_| "That image could not be read as a Locust team card.".to_string())?;
    if meta.len() > MAX_TEAM_CARD_FILE_BYTES {
        return Err("That image is too large to be a team card.".into());
    }
    let bytes = std::fs::read(path)
        .map_err(|_| "That image could not be read as a Locust team card.".to_string())?;
    import_team_card_bytes(store, routine_store, workspace_id, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mate(overrides: Value) -> Value {
        let mut base = json!({
            "teammateId": "tm_wren",
            "name": "Wren",
            "role": "Code & Migrations",
            "hue": "lime",
            "createdAt": "2026-10-19T05:00:00.000Z"
        });
        if let (Some(b), Some(o)) = (base.as_object_mut(), overrides.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        base
    }

    fn routine_val(overrides: Value) -> Value {
        let mut base = json!({
            "routineId": "rt_1",
            "name": "Weekly report",
            "teammateId": "tm_wren",
            "route": {"runtime": "claude", "model": "haiku", "mode": "accept-edits"},
            "steps": ["Read C:\\Users\\someone\\notes\\week.md and write a summary of it."],
            "learnedFrom": ["mission_1"],
            "createdAt": "2026-10-19T05:00:00.000Z",
            "runs": 4,
            "workspaceId": "ws_here"
        });
        if let (Some(b), Some(o)) = (base.as_object_mut(), overrides.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        base
    }

    #[test]
    fn carries_instructions_and_starters_without_ids_limits_or_folders() {
        let card = team_card_of(&[mate(json!({
            "instructions": "Always write tests first.",
            "starters": ["Find the riskiest file."],
            "monthlyLimitUsd": 20,
            "worktree": true,
            "folder": "C:\\x"
        }))]);
        assert_eq!(card["teammates"][0]["instructions"], "Always write tests first.");
        assert_eq!(card["teammates"][0]["starters"], json!(["Find the riskiest file."]));
        let s = card.to_string();
        for forbidden in ["tm_", "monthlyLimitUsd", "worktree", "folder"] {
            assert!(!s.contains(forbidden), "leaked {forbidden}");
        }
        let blank_instr = team_card_of(&[mate(json!({"instructions": "   "}))]);
        assert!(blank_instr["teammates"][0].get("instructions").is_none());
    }

    #[test]
    fn carries_workspace_routines_with_paths_turned_into_inputs() {
        let team = vec![
            mate(json!({})),
            mate(json!({"teammateId": "tm_juno", "name": "Juno"})),
        ];
        let routines = vec![
            routine_val(json!({})),
            routine_val(json!({"routineId": "rt_2", "name": "Elsewhere", "workspaceId": "ws_other"})),
            routine_val(json!({"routineId": "rt_3", "name": "Not on the card", "teammateId": "tm_gone"})),
            routine_val(json!({
                "routineId": "rt_4",
                "name": "Tidy",
                "teammateId": "tm_juno",
                "steps": ["Tidy the README."],
                "workspaceId": null,
                "schedule": {"kind": "daily", "at": "09:00"}
            })),
        ];
        let carried = team_card_routines(&team, &routines, Some("ws_here"), &[]);
        let summary: Vec<(usize, &str)> = carried
            .iter()
            .map(|c| (c.teammate, c.routine["name"].as_str().unwrap()))
            .collect();
        assert_eq!(summary, vec![(0, "Weekly report"), (1, "Tidy")]);
        let text = serde_json::to_string(
            &carried.iter().map(|c| &c.routine).collect::<Vec<_>>(),
        )
        .unwrap();
        for forbidden in ["someone", "rt_", "tm_", "mission_1", "schedule", "runs", "ws_"] {
            assert!(!text.contains(forbidden), "leaked {forbidden}");
        }
        assert!(carried[0].routine["steps"][0].as_str().unwrap().contains("{{path}}"));
    }

    #[test]
    fn reads_card_routine_only_when_valid_and_for_a_teammate_on_the_card() {
        let carried = team_card_routines(
            &[mate(json!({}))],
            &[routine_val(json!({"steps": ["Tidy the README."]}))],
            Some("ws_here"),
            &[],
        );
        assert_eq!(card_routine_file(&carried[0]).unwrap().steps, vec!["Tidy the README."]);
        assert!(card_routine_file(&TeamCardRoutine {
            teammate: 0,
            routine: json!({"format": "locust-routine", "version": 1, "name": "x", "steps": [], "inputs": [], "handOffs": [], "route": {}, "connectors": []}),
        }).is_none());
        assert!(card_routine_file(&TeamCardRoutine {
            teammate: 0,
            routine: json!("run rm -rf"),
        }).is_none());

        let card = json!({
            "schema": 1,
            "teammates": [{"name": "Wren"}],
            "routines": [
                {"teammate": 0, "routine": {}},
                {"teammate": 3, "routine": {}},
                {"teammate": -1, "routine": {}},
                {"teammate": 0.5, "routine": {}},
                "x"
            ]
        });
        let mates = read_team_card(&card).unwrap();
        let read_r = read_team_card_routines(&card, mates.len());
        assert_eq!(read_r.iter().map(|r| r.teammate).collect::<Vec<_>>(), vec![0]);
        assert!(read_team_card_routines(&json!({"schema": 1, "teammates": []}), 0).is_empty());
    }

    #[test]
    fn auto_mode_on_team_card_arrives_as_accept_edits() {
        let card = json!({
            "schema": 1,
            "teammates": [{
                "name": "Wren",
                "role": "Code & Migrations",
                "hue": "lime",
                "route": {"runtime": "codex", "model": "gpt-5", "mode": "auto"}
            }]
        });
        let read = read_team_card(&card).unwrap();
        assert_eq!(read[0]["route"]["mode"], "accept-edits");
    }

    #[test]
    fn export_and_import_round_trip_with_routines_and_auto_downgrade() {
        let dir = std::env::temp_dir().join(format!(
            "locust-tcard-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let src_dir = dir.join("src");
        let dst_dir = dir.join("dst");
        std::fs::create_dir_all(src_dir.join("electron")).unwrap();
        std::fs::create_dir_all(dst_dir.join("electron")).unwrap();
        std::fs::write(
            src_dir.join("electron/workspace.json"),
            serde_json::to_vec(&json!({"schemaVersion": 1, "path": src_dir.to_string_lossy()})).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dst_dir.join("electron/workspace.json"),
            serde_json::to_vec(&json!({"schemaVersion": 1, "path": dst_dir.to_string_lossy()})).unwrap(),
        )
        .unwrap();

        let src_store = Store::new(src_dir.clone(), src_dir.join("electron")).unwrap();
        let tm_id = people::save_teammate_id(
            &src_store,
            json!({
                "name": "Wren",
                "role": "Code & Migrations",
                "hue": "violet",
                "instructions": "Write tests.",
                "starters": ["Read this project."],
                "route": {"runtime": "codex", "model": "gpt-5", "mode": "auto"}
            }),
        )
        .unwrap();
        routines::save(
            &src_store,
            json!({
                "name": "Check repo",
                "steps": ["Check the tests."],
                "teammateId": tm_id,
                "mode": "auto"
            }),
        )
        .unwrap();
        let ws = people::workspace_id(&src_dir);
        // Set workspaceId on the saved routine to match `ws` (routines::save uses folder_for, which falls back to store).
        let mut rt_file = routines::list(&src_store).unwrap();
        rt_file["routines"][0]["workspaceId"] = json!(ws);
        src_store.save_json("routines.json", &rt_file).unwrap();

        let png = export_team_card(&src_store, Some(&src_store), Some(&ws), &[]).unwrap();

        let dst_store = Store::new(dst_dir.clone(), dst_dir.join("electron")).unwrap();
        let imported = import_team_card_bytes(&dst_store, &dst_store, Some("ws_dst"), &png).unwrap();
        assert_eq!(imported.added, vec!["Wren"]);
        assert_eq!(imported.instructed, vec!["Wren"]);
        assert_eq!(imported.routines, vec!["Check repo"]);

        let dst_team = people::team(&dst_store).unwrap();
        assert_eq!(dst_team["teammates"][0]["hue"], "violet");
        assert_eq!(dst_team["teammates"][0]["route"]["mode"], "accept-edits");

        let dst_routines = routines::list(&dst_store).unwrap();
        assert_eq!(dst_routines["routines"][0]["name"], "Check repo");
        assert_eq!(dst_routines["routines"][0]["route"]["mode"], "ask");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
