//! Every place the Rust app still calls Electron's door (the door client's `local` constructor), pinned by file.
//!
//! The Rust app is meant to stand alone. Each call below is a path that needs Electron running, and PARITY.md marks
//! the rows that rely on one as not alone. The counts are exact: a new call fails this test, and so does a removed
//! one until its count here is lowered, so the list can only shrink on purpose.

#[test]
fn calls_into_electrons_door_only_shrink() {
    // Built at run time so this file does not count itself.
    let needle = ["Engine", "::", "local()"].concat();
    let allowed: &[(&str, usize)] = &[
        // choose_engine, send (door path), open_models, stop (a turn this process does not own), carry,
        // refresh_recent (fallback), request_reply, open_recent, poll (model list).
        ("src/main.rs", 9),
        // Quick Ask asks Electron's door first, as the window does; it falls back to the native door or engine.
        ("src/quick.rs", 1),
        // The ignored real test of the door client itself.
        ("core/src/engine.rs", 1),
    ];
    // Both crates: the window (src) and the engine (core/src).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = std::collections::BTreeMap::new();
    let mut stack = vec![root.join("src"), root.join("core/src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let n = text.matches(&needle).count();
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
        "calls to Electron's door changed. A new one makes a path need Electron: run it on the native engine instead. \
         If you removed one, lower its count here and check PARITY.md's alone marks."
    );
}
