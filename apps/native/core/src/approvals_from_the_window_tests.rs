//! Electron's `main/approvals-come-only-from-the-window.test.ts` (A2.16): an approval is answered only by a person,
//! never by anything a teammate writes. In the native app every answer goes through `Channel::decide`, and this keeps
//! the places that call it to: the channel itself (`approval.rs`), the window's two funnels in `native.rs`
//! (`decide_approval`, `answer_approval`), the engine wire the window drives (`engine_process.rs`, `engine_api.rs`),
//! and the window's own presses (`src/ui`, `src/quick.rs`). A new caller (a relay path, a block parser) fails here and
//! has to say why it may. The phone on the home Wi-Fi (`phone_calls.rs`) and the opted-in editor's own turn
//! (`door.rs`, only `Editors::answer`) say why below; Electron's chat-app callers
//! are a networked feature native does not have.

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, with its text before any `#[cfg(test)]` module, test files left out.
fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") || name == "tests.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let cut = ["#[cfg(test)]\nmod ", "#[cfg(all(test"].iter().filter_map(|gate| text.find(gate)).min().unwrap_or(text.len());
        let live = text[..cut].to_owned();
        out.push((path, live));
    }
}

// case: is only the approval holders and the window handler
#[test]
fn only_the_channel_the_windows_funnels_and_a_persons_press_answer_an_approval() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let mut all = Vec::new();
    sources(&root.join("core/src"), &mut all);
    sources(&root.join("src"), &mut all);
    let allowed = |path: &Path| {
        let rel = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        // phone_calls.rs: a phone paired by its code is the person, answering a card as the window's press does
        // (Electron's phoneCalls.decide); a page or a teammate's words never reach it (PHONE-NOTES.md).
        // door.rs: an editor's session answering its own current turn's card, once, only while the person has turned on
        // "Let editors answer cards for their own turns" (locust-cli-sessions.ts `answer`); held below to that one place.
        matches!(rel.as_str(), "core/src/approval.rs" | "core/src/native.rs" | "core/src/engine_process.rs" | "core/src/engine_api.rs" | "src/quick.rs" | "core/src/phone_calls.rs" | "core/src/door.rs")
            || rel.starts_with("src/ui/")
    };
    let callers: Vec<String> = all
        .iter()
        .filter(|(_, text)| text.contains(".decide("))
        .filter(|(path, _)| !allowed(path))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert_eq!(callers, Vec::<String>::new(), "a new caller of decide has to say why it may");

    // In door.rs, the editors' own answer and nothing else: none of the door's tools for other apps answers a card.
    let door = &all.iter().find(|(p, _)| p.ends_with("core/src/door.rs")).unwrap().1;
    let lines: Vec<&str> = door.lines().collect();
    let in_door: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(".decide("))
        .map(|(at, _)| lines[..=at].iter().rev().find_map(|l| l.trim_start().strip_prefix("fn ").and_then(|f| f.split('(').next())).unwrap_or(""))
        .collect();
    assert_eq!(in_door, vec!["answer"], "in door.rs only an editor's session answers its own card");

    // In native.rs, the window's two funnels and nothing else.
    let native = &all.iter().find(|(p, _)| p.ends_with("core/src/native.rs")).unwrap().1;
    let lines: Vec<&str> = native.lines().collect();
    let funnels: Vec<String> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(".decide("))
        .map(|(at, _)| {
            lines[..=at].iter().rev().find_map(|l| l.trim_start().strip_prefix("pub fn ").and_then(|f| f.split('(').next())).unwrap_or("").to_owned()
        })
        .collect();
    assert_eq!(funnels, ["answer_approval", "decide_approval"]);
}
