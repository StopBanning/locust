//! Electron's `groups-unreadable-is-a-line.test.tsx`, case by case, on the sidebar the window draws from a real
//! profile: a file that would not read is a sentence above the conversations, never an empty list, and an empty
//! folder and a broken file never look alike.
use super::*;

/// A profile with one conversation in it; `files` are written into this profile's own folder before it is read.
fn sidebar_from(files: &[(&str, &str)]) -> (String, State) {
    let temp = std::env::temp_dir().join(format!("locust-sb-unreadable-line-{}-{}", std::process::id(), files.len() + files.iter().map(|(n, b)| n.len() + b.len()).sum::<usize>()));
    let _ = std::fs::remove_dir_all(&temp);
    let (electron, native) = (temp.join("electron"), temp.join("native"));
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::create_dir_all(&native).unwrap();
    let store = crate::store::Store::new(native.clone(), electron).unwrap();
    for (name, body) in files {
        std::fs::write(native.join(name), body).unwrap();
    }
    let mut s = State::default();
    s.org = org::load_org(&store);
    s.data = Some(store);
    s.recent.push(ConversationRow {
        id: "mission_1".into(),
        title: "Control renamed conversation".into(),
        teammate: String::new(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: "2026-09-14T05:00:00.000Z".into(),
    });
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let words = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    let _ = std::fs::remove_dir_all(&temp);
    (words, rt.state)
}

const GROUPED: &str = r#"{"schemaVersion":1,"groups":[{"groupId":"grp_1","name":"State control","createdAt":"2026-09-14T05:00:00.000Z"}],"members":{"mission_1":{"groupId":"grp_1","at":"2026-09-14T05:00:00.000Z"}}}"#;

#[test]
fn is_said_in_the_sidebar_above_the_conversations_it_could_not_sort() {
    // case: is said in the sidebar, above the conversations it could not sort
    let (words, _) = sidebar_from(&[("groups.json", "{broken")]);
    assert!(words.contains("Groups could not be read — everything is listed ungrouped."), "{words}");
    assert!(words.contains("Nothing is saved over the file until it reads again."), "{words}");
    // The conversation is still there to work with, under no group heading it cannot vouch for.
    assert!(words.contains("Control renamed conversation"), "{words}");
    assert!(!words.contains("Ungrouped"), "{words}");
}

#[test]
fn is_not_said_when_there_are_simply_no_groups() {
    // case: is not said when there are simply no groups
    let (words, _) = sidebar_from(&[]);
    assert!(!words.contains("Groups could not be read"), "{words}");
    assert!(!words.contains("could not be read"), "{words}");
    assert!(words.contains("Control renamed conversation"), "{words}");
}

#[test]
fn names_every_file_that_would_not_read_in_one_sentence_roster_first() {
    // case: names every file that would not read, in one sentence, roster first
    let (words, s) = sidebar_from(&[("routines.json", "{broken"), ("teammates.json", "{broken")]);
    assert!(s.org.routines_unreadable && s.org.teammates_org_unreadable);
    assert!(words.contains("Teammates and routines could not be read."), "{words}");
    assert!(words.contains("Nothing is saved over them until they read again."), "{words}");
    assert!(words.contains("Control renamed conversation"), "{words}");
}

#[test]
fn is_not_said_when_the_groups_read_fine() {
    // case: is not said when the groups read fine
    let (words, _) = sidebar_from(&[("groups.json", GROUPED)]);
    assert!(!words.contains("Groups could not be read"), "{words}");
    // The group is drawn, its name as a section heading (in capitals, as the sidebar's headings are).
    assert!(words.to_lowercase().contains("state control"), "{words}");
}
