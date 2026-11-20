//! the-composer-counts-what-it-is-about.test.tsx (5 cases), on the placeholder's own rule. Partial: Rust's discovery
//! (boot_view::path_discovery_sweep) only finds agents on PATH and never probes one that hangs, so the window always
//! passes `installed_silent: 0`; counting only the installed ones (case 3) has nothing to count yet.
use super::*;

fn input(installed_silent: usize) -> PlaceholderInput<'static> {
    PlaceholderInput {
        working_now: false,
        working_name: "",
        queued: false,
        route_can_run: true,
        selected_ready: false,
        selected_display: None,
        teammate_name: None,
        discovery_loading: false,
        discovery_error: false,
        ready_elsewhere: None,
        auth_required: false,
        installed_silent,
        checking: false,
        nothing_installed: false,
    }
}

// case: says one in the singular
#[test]
fn says_one_in_the_singular() {
    let said = placeholder(input(1));
    assert!(said.contains("An AI agent is installed but not answering"), "{said}");
    assert!(!said.chars().next().unwrap().is_ascii_digit());
}

// case: counts five as five, not as one
#[test]
fn counts_five_as_five() {
    let said = placeholder(input(5));
    assert!(said.contains("5 AI agents are installed but not answering"), "{said}");
    assert!(!said.contains("An AI agent is installed"));
}

// case: does not complain about hung agents when one of them works
#[test]
fn a_working_agent_is_named_instead_of_the_hung_ones() {
    let said = placeholder(PlaceholderInput { ready_elsewhere: Some("OpenCode"), ..input(2) });
    assert!(!said.contains("not answering"), "{said}");
    assert!(said.contains("OpenCode is ready"), "{said}");
}

// case: says nothing of the kind while discovery is still asking
#[test]
fn nothing_of_the_kind_while_discovery_asks() {
    let said = placeholder(PlaceholderInput { discovery_loading: true, ..input(3) });
    assert!(!said.contains("not answering"), "{said}");
}
