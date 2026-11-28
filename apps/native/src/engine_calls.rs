//! Every place the window still reaches the engine's turn state directly instead of through
//! `locust_core::engine_api` (plan E1), pinned by file.
//!
//! The running turns and the waiting cards live in statics in `core/src/native.rs`. A window, Quick Ask or `--ask`
//! that reaches them directly sees only its own process. E3 puts one engine process behind `engine_api`, so each direct
//! use left here is a place that would still see only itself. The counts are exact: a new use fails this test, and so
//! does a removed one until its count here is lowered, so the list can only shrink on purpose.

#[test]
fn direct_uses_of_the_engines_turn_state_only_shrink() {
    // Built at run time so this file does not count itself.
    let needles: Vec<String> = [
        "send", "send_from", "stop", "owns", "owned_key", "running_ids", "waiting_approvals", "waiting_approvals_for",
        "decide_approval", "carry", "shutdown_in_background", "shutdown_in_background_for", "wait_for_shutdown",
        "approvals_channel",
    ]
    .iter()
    .map(|f| ["native", "::", f, "("].concat())
    .collect();
    let allowed: &[(&str, usize)] = &[
        // A test that answers two cards on the shared channel at once (the Board's review test).
        ("src/ui/home_panel.rs", 1),
        // A test that raises a real card on the shared channel, to see a face wait on the person (teammate_faces.rs).
        ("src/ui/teammate_faces.rs", 1),
        // A test that raises a real card on the shared channel, to see a browser given the window handed it (phone_section.rs).
        ("src/ui/phone_section.rs", 1),
        // Two tests that raise real cards on the shared channel, to verify active-thread approval and question routing.
        ("src/ui/approval_active_thread_tests.rs", 2),
        // Two tests that register a teammate on the shared channel, to check saved-rule offers on secret vs ordinary commands.
        ("src/ui/approval_secret_rule_tests.rs", 1),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = std::collections::BTreeMap::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                // Not `other_native::stop(`: the match must start a path segment.
                let n: usize = needles
                    .iter()
                    .map(|needle| {
                        text.match_indices(needle.as_str())
                            .filter(|(i, _)| !text[..*i].ends_with('_') && !text[..*i].ends_with(char::is_alphanumeric))
                            .count()
                    })
                    .sum();
                if n > 0 {
                    let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                    found.insert(rel, n);
                }
            }
        }
    }
    let expected: std::collections::BTreeMap<String, usize> =
        allowed.iter().map(|(f, n)| (f.to_string(), *n)).collect();
    assert_eq!(
        found, expected,
        "the window's direct uses of the engine's turn state changed. Go through locust_core::engine_api::api() \
         instead, so one engine process can own every turn. If you removed one, lower its count here."
    );
}

/// `engine_api::install` and `browser_tools::install` choose for the whole process. `cargo test` (CI and the release
/// gate) runs every test of a crate in one process, so a test that calls either changes every test after it: on 28 Nov
/// one test's fake engine made the window suite wait for a run that never ended. Only the app itself calls them in
/// place; a test that needs one runs in a process of its own (it reruns itself with `current_exe()`, as
/// core/src/runtime/copilot_browser_grid_tests.rs does).
#[test]
fn a_process_wide_install_in_a_test_runs_in_that_tests_own_process() {
    // Built at run time so this file does not count itself.
    let needles: Vec<String> = ["engine_api", "browser_tools"].iter().map(|m| [m, "::install("].concat()).collect();
    // The app's own calls: the window's start chooses its engine, and its first browser poll installs the tools
    // (a test sets `browser.initialized` first, or attaches with `browser_panel::attach_in_tests`).
    let app: &[&str] = &["src/main.rs", "src/ui/browser_panel.rs"];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut shared = Vec::new();
    let mut stack = vec![root.join("src"), root.join("core").join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                let installs = needles.iter().any(|n| text.contains(n.as_str()));
                if installs && !app.contains(&rel.as_str()) && !text.contains("current_exe()") {
                    shared.push(rel);
                }
            }
        }
    }
    shared.sort();
    assert!(
        shared.is_empty(),
        "these files install the engine or the browser tools for the whole test process: {shared:?}. Run that test \
         in a process of its own (rerun it with std::env::current_exe() and an env var, as \
         core/src/runtime/copilot_browser_grid_tests.rs does)."
    );
}
