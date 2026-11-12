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
