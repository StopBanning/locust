//! Carries routines-shelf.test.tsx's CliArtifacts case: the fold's count, the note, the tail of the path, and which
//! agents' lists are read.
use super::*;
use crate::agents::{Auth, Found, Probe};

fn artifact(kind: &'static str, path: &str) -> RuntimeArtifact {
    RuntimeArtifact { runtime: "codex".into(), kind, name: "gig-scout".into(), description: None, path: PathBuf::from(path) }
}

#[test]
fn the_fold_counts_each_kind_in_the_order_it_first_appears() {
    let (a, c) = (artifact("agent", "/h/.codex/agents/a.toml"), artifact("command", "/h/.claude/commands/c.md"));
    assert_eq!(inventory_summary(&[&a, &a, &c]), "2 agents \u{b7} 1 command");
    assert_eq!(inventory_summary(&[&c, &a]), "1 command \u{b7} 1 agent");
    assert_eq!(fold_line(&[&a]), "1 agent of its own");
    assert_eq!(fold_line(&[&a, &a, &a, &c]), "3 agents \u{b7} 1 command of its own");
    assert_eq!(LISTS_NOT_RUNS, "Locust lists these, it does not run them");
}

#[test]
fn the_row_shows_the_last_two_parts_of_where_it_lives() {
    assert_eq!(path_tail(Path::new("/Users/me/.codex/agents/gig-scout.toml")), "agents/gig-scout.toml");
    assert_eq!(path_tail(Path::new(r"C:\Users\me\.claude\agents\gig-scout.md")), "agents/gig-scout.md");
}

#[test]
fn ready_and_signed_out_agents_are_read_and_no_other() {
    let agent = |id: &str, status: Probe| Found {
        id: id.into(),
        display_name: id.into(),
        installed: status != Probe::Missing,
        version: None,
        auth: Auth::Unknown,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    };
    let found = [agent("claude", Probe::AuthRequired), agent("codex", Probe::Ready), agent("cursor", Probe::Missing), agent("gemini", Probe::ProbeFailed)];
    assert_eq!(listed_runtimes(&found), ["claude", "codex"]);
}
