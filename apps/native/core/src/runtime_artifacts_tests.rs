//! Carries apps/desktop/src/main/runtime-artifacts.test.ts: reading what a person set up in the CLIs themselves. Every
//! home here is a temporary folder, never the real `~/.claude` or `~/.codex`.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn parsed(name: &str, description: Option<&str>) -> Parsed {
    Parsed { name: name.to_owned(), description: description.map(str::to_owned) }
}

struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn bare(label: &str) -> Home {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let home = std::env::temp_dir().join(format!(
        "locust-artifacts-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    Home(home)
}

fn seed() -> Home {
    let home = bare("seed");
    for dir in [".claude/agents", ".claude/commands", ".codex/agents"] {
        std::fs::create_dir_all(home.0.join(dir)).unwrap();
    }
    std::fs::write(home.0.join(".claude/agents/seo-scout.md"), "---\nname: seo-scout\ndescription: Audits visibility\n---\nbody").unwrap();
    std::fs::write(home.0.join(".claude/commands/standup.md"), "---\ndescription: Write the standup\n---\nbody").unwrap();
    std::fs::write(home.0.join(".codex/agents/proposal-writer.toml"), "name = \"proposal-writer\"\ndescription = \"Writes proposals\"\n").unwrap();
    home
}

fn listed(found: &[RuntimeArtifact]) -> Vec<String> {
    let mut all: Vec<String> = found.iter().map(|e| format!("{}/{}/{}", e.runtime, e.kind, e.name)).collect();
    all.sort();
    all
}

// case: reads Claude's YAML fences
#[test]
fn reads_claudes_yaml_fences() {
    assert_eq!(
        parse_artifact("gig-scout.md", "---\nname: gig-scout\ndescription: Scans venues for gigs\ntools: WebFetch\n---\n\nYou are the lead scout."),
        parsed("gig-scout", Some("Scans venues for gigs"))
    );
}

// case: reads Codex's TOML, single-quoted or double
#[test]
fn reads_codexs_toml_single_quoted_or_double() {
    assert_eq!(
        parse_artifact("gig-scout.toml", "name = \"gig-scout\"\ndescription = 'Scans venues for gigs'\n"),
        parsed("gig-scout", Some("Scans venues for gigs"))
    );
    // A quote on one side only is not a quoted value, and a TOML array's trailing comma is not part of it.
    assert_eq!(parse_artifact("a.toml", "name = \"half\ndescription = \"Ends here\",\n"), parsed("\"half", Some("Ends here")));
}

// case: falls back to the filename, because the file existing is the point
#[test]
fn falls_back_to_the_filename_because_the_file_existing_is_the_point() {
    assert_eq!(parse_artifact("standup.md", "Just prose, no front matter."), parsed("standup", None));
    // An empty name is no name.
    assert_eq!(parse_artifact("blank.md", "---\nname: ''\n---\n"), parsed("blank", None));
}

// case: takes a description without a name, which is how commands are written
#[test]
fn takes_a_description_without_a_name_which_is_how_commands_are_written() {
    assert_eq!(parse_artifact("pitch.md", "---\ndescription: Write a pitch\n---\n"), parsed("pitch", Some("Write a pitch")));
}

// case: never reads past the front matter into the instructions
#[test]
fn never_reads_past_the_front_matter_into_the_instructions() {
    let got = parse_artifact("a.md", "---\nname: real\n---\n\nSome prose that happens to contain\nname: not-the-name\n");
    assert_eq!(got.name, "real");
    // Fenced with no name: the body's `name:` line is still not the name.
    let got = parse_artifact("b.md", "---\ndescription: d\n---\n\nname: not-the-name\n");
    assert_eq!(got.name, "b");
    // Only the head of a long file is read, and a cut inside a character does not break it.
    let long = format!("{}\nname: too-far\n", "é".repeat(HEAD_BYTES));
    assert_eq!(parse_artifact("c.md", &long).name, "c");
}

// case: lists agents and commands for the runtimes that are installed
#[test]
fn lists_agents_and_commands_for_the_runtimes_that_are_installed() {
    let home = seed();
    assert_eq!(
        listed(&read_runtime_artifacts(&["claude", "codex"], &home.0)),
        ["claude/agent/seo-scout", "claude/command/standup", "codex/agent/proposal-writer"]
    );
    let found = read_runtime_artifacts(&["claude"], &home.0);
    let scout = found.iter().find(|e| e.name == "seo-scout").unwrap();
    assert_eq!(scout.description.as_deref(), Some("Audits visibility"));
}

#[test]
fn reads_codex_automations_and_cursor_agents_where_those_clis_keep_them() {
    let home = bare("more");
    std::fs::create_dir_all(home.0.join(".codex/automations")).unwrap();
    std::fs::create_dir_all(home.0.join(".cursor/agents")).unwrap();
    std::fs::write(home.0.join(".codex/automations/nightly.JSON"), "{}").unwrap();
    std::fs::write(home.0.join(".cursor/agents/fixer.md"), "x").unwrap();
    assert_eq!(listed(&read_runtime_artifacts(&["codex", "cursor"], &home.0)), ["codex/automation/nightly", "cursor/agent/fixer"]);
}

#[test]
fn reads_forty_of_a_kind_in_name_order_and_no_more() {
    let home = bare("many");
    let dir = home.0.join(".claude/commands");
    std::fs::create_dir_all(&dir).unwrap();
    for i in (0..45).rev() {
        std::fs::write(dir.join(format!("c{i:02}.md")), "x").unwrap();
    }
    let found = read_runtime_artifacts(&["claude"], &home.0);
    assert_eq!(found.len(), 40);
    assert_eq!(found.first().unwrap().name, "c00");
    assert_eq!(found.last().unwrap().name, "c39");
}

// case: says nothing about a runtime that is not on this machine
#[test]
fn says_nothing_about_a_runtime_that_is_not_on_this_machine() {
    let home = seed();
    let found = read_runtime_artifacts(&["codex"], &home.0);
    assert!(!found.is_empty());
    assert!(found.iter().all(|e| e.runtime == "codex"));
}

// case: is empty, not an error, when nobody has set any up
#[test]
fn is_empty_not_an_error_when_nobody_has_set_any_up() {
    let home = bare("bare");
    assert_eq!(read_runtime_artifacts(&["claude", "codex"], &home.0), []);
}

// case: ignores files of the wrong kind
#[test]
fn ignores_files_of_the_wrong_kind() {
    let home = seed();
    std::fs::write(home.0.join(".claude/agents/notes.txt"), "not an agent").unwrap();
    // A folder named like an agent is not a file to read.
    std::fs::create_dir_all(home.0.join(".claude/agents/folder.md")).unwrap();
    let found = read_runtime_artifacts(&["claude"], &home.0);
    assert!(!found.iter().any(|e| e.name == "notes" || e.name == "folder"));
    assert_eq!(found.len(), 2);
}

#[cfg(unix)]
#[test]
fn a_pipe_named_like_an_agent_is_never_opened() {
    // Only a file is read: opening a named pipe would wait for a writer that never comes, and the screen with it.
    let home = seed();
    let pipe = home.0.join(".claude/agents/pipe.md");
    assert!(std::process::Command::new("mkfifo").arg(&pipe).status().unwrap().success());
    let at = home.0.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_runtime_artifacts(&["claude"], &at));
    });
    let found = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("the list came back without waiting on the pipe");
    assert_eq!(listed(&found), ["claude/agent/seo-scout", "claude/command/standup"]);
}

// case: carries the path, so the app can say where to go and change it
#[test]
fn carries_the_path_so_the_app_can_say_where_to_go_and_change_it() {
    let home = seed();
    let found = read_runtime_artifacts(&["claude"], &home.0);
    assert!(found[0].path.to_string_lossy().contains(".claude"));
    assert_eq!(found[0].path, home.0.join(".claude/agents/seo-scout.md"));
}
