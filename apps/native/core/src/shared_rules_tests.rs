// Carries three of apps/desktop/src/shared's rule files (Electron 4ce2f37a), the cases Rust has:
// the-about-you-line-is-taught-once (people::brief), a-first-run-names-no-terminal and kilo-installs-in-one-click
// (agents::install_sentence, install_command, runtime_install_facts). Each file's renderer case reads Electron's own
// .tsx source and has no Rust form (see the rows).
use crate::agents::{RuntimeInstall, install_command, install_sentence, runtime_install_facts};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn brief(note: Option<&str>) -> String {
    let root = std::env::temp_dir().join(format!("locust-about-you-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let _t = Temp(root.clone());
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    if let Some(note) = note {
        crate::people::settings(&store, note, None).unwrap();
    }
    let folder = root.join("pebble");
    std::fs::create_dir_all(&folder).unwrap();
    crate::people::brief(&store, "nobody", &folder, "Tidy the notes.").unwrap()
}
fn taught(text: &str) -> usize {
    text.lines().filter(|l| l.starts_with("about you ::")).count()
}

// --- the-about-you-line-is-taught-once ---

#[test]
fn the_about_you_line_is_taught_by_the_notes_section_alone_when_there_is_a_note() {
    let brief = brief(Some("Keep answers short."));
    assert_eq!(taught(&brief), 1, "{brief}");
    assert!(brief.contains("about you :: Prefers a one-line summary before any detail."));
}

#[test]
fn the_about_you_line_is_still_taught_by_the_memory_block_when_there_is_no_note_yet() {
    let brief = brief(None);
    assert_eq!(taught(&brief), 1, "{brief}");
    assert!(!brief.contains("About the person you work for"));
}

// --- a-first-run-names-no-terminal ---

// case: says Install, then Sign in, for every npm agent that needs an account
#[test]
fn a_first_run_says_install_then_sign_in_for_every_npm_agent_that_needs_an_account() {
    for (id, name) in [("claude", "Claude Code"), ("codex", "Codex CLI"), ("copilot", "Copilot CLI")] {
        let said = install_sentence(id, name);
        assert!(said.contains("Press Install, then Sign in."), "{said}");
        let says_run = said.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "run");
        assert!(!says_run, "{said}");
    }
    assert!(!install_sentence("opencode", "OpenCode").contains("--"));
}

// --- kilo-installs-in-one-click ---

// case: is one npm install of its CLI package, signed into only for a paid model
#[test]
fn kilo_is_one_npm_install_of_its_cli_package_signed_into_only_for_a_paid_model() {
    let facts = runtime_install_facts("kilo").unwrap();
    assert!(matches!(facts.install, RuntimeInstall::Npm("@kilocode/cli")));
    assert_eq!(install_command("kilo").as_deref(), Some("npm install -g @kilocode/cli"));
    assert!(facts.sign_in_optional);
}
