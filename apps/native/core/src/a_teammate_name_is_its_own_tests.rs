//! Electron's `apps/desktop/src/main/two-teammates-may-not-share-a-name.test.ts` (A2.18), case by case, through the
//! caller: the window creates and edits a teammate with `save_teammate` (`save_teammate_id` underneath), which keeps teammates.json. A teammate is
//! reached by the name a model writes in `to=`, so a name two teammates share is refused, however it is spelled.
use super::*;

struct Folder(std::path::PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Folder, Store) {
    let path = std::env::temp_dir().join(id("locust-names").unwrap());
    std::fs::create_dir_all(path.join("electron")).unwrap();
    let store = Store::new(path.join("native"), path.join("electron")).unwrap();
    (Folder(path), store)
}

fn wren(name: &str) -> Value {
    json!({"teammateId": "", "name": name, "hue": "lime", "role": "Code & Migrations", "route": {"runtime": "codex", "model": "account-default", "mode": "ask", "effort": "high"}})
}

fn names(store: &Store) -> Vec<String> {
    team(store).unwrap()["teammates"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect()
}

fn saved(store: &Store, id: &str) -> Value {
    team(store).unwrap()["teammates"].as_array().unwrap().iter().find(|t| t["teammateId"] == id).unwrap().clone()
}

#[test]
fn cannot_be_given_to_a_second_teammate_whatever_its_case_or_spaces() {
    let (_folder, store) = store();
    save_teammate_id(&store, wren("Wren")).unwrap();
    for name in ["Wren", "wren", "  WREN  "] {
        assert_eq!(save_teammate(&store, wren(name)).unwrap_err(), "Another teammate is already called Wren. Pick another name.");
    }
    assert_eq!(names(&store), vec!["Wren"]);
}

#[test]
fn cannot_be_taken_by_a_rename_but_a_teammate_keeps_its_own() {
    let (_folder, store) = store();
    let wren_id = save_teammate_id(&store, wren("Wren")).unwrap();
    let booty_id = save_teammate_id(&store, wren("Booty")).unwrap();
    let mut renamed = saved(&store, &booty_id);
    renamed["name"] = json!("WREN");
    assert_eq!(save_teammate(&store, renamed).unwrap_err(), "Another teammate is already called Wren. Pick another name.");
    assert_eq!(saved(&store, &booty_id)["name"], "Booty");
    let mut kept = saved(&store, &wren_id);
    kept["name"] = json!("wren");
    kept["hue"] = json!("blue");
    save_teammate(&store, kept).unwrap();
    assert_eq!(saved(&store, &wren_id)["name"], "wren");
}
