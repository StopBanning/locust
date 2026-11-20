//! Where Electron uses `holdsNothingYet` (`index.ts` `workspaceEmpty`, IdleTeammate's `folderEmpty`): a teammate whose
//! folder holds nothing but dot-files offers to make something; the first real file there brings its own starters back.
//! Drawn headless, through the conversation view the person sees.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wren(folder: &std::path::Path) -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates = vec![Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Code & Migrations".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "accept-edits".into(),
        folder: None,
    }];
    s.selected = 0;
    s.conversations.insert("tm_wren".into(), Conversation::default());
    s.people_panel.team = serde_json::json!({"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"}]});
    s.home.folder_path = Some(folder.to_string_lossy().into_owned());
    s
}
fn shown(s: State) -> String {
    let mut rt = rune::Runtime::new(s, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)), (1200.0, 1800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn an_idle_teammate_in_a_folder_of_only_dot_files_offers_to_make_something_until_a_real_file_arrives() {
    let n = std::process::id();
    let folder = std::env::temp_dir().join(format!("locust-idle-empty-{n}"));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join(".locust")).unwrap();
    let _t = Temp(folder.clone());
    std::fs::write(folder.join(".locust").join("memory.md"), "nothing yet").unwrap();
    std::fs::write(folder.join(".DS_Store"), "").unwrap();
    let fresh = "Make a small web page here that shows today's date and a quote, and tell me how to open it.";
    let own = "Read this project and tell me what it does, in one paragraph.";
    let empty = shown(wren(&folder));
    assert!(empty.contains(fresh) && !empty.contains(own), "{empty}");
    std::fs::write(folder.join("index.html"), "<h1>Crumb</h1>").unwrap();
    let held = shown(wren(&folder));
    assert!(held.contains(own) && !held.contains(fresh), "{held}");
    // No folder at all is not an empty one.
    let mut none = wren(&folder);
    none.home.folder_path = None;
    assert!(shown(none).contains(own));
}
