//! Cutover 1, row 1: the window's own engine is the default, even while Electron's door is up.
//! Made-up door only: a local listener that records whether it was asked, in the test runner's temporary HOME.
//! The engine choice reads the environment, so it runs in a child copy of this test binary with the variables set on
//! that child alone: no other test in this process ever sees them (`cargo test` runs tests as threads).

use super::*;

/// A door that is "up" (a connection file pointing at a listening port) and records each time it is asked.
fn door_that_counts() -> (std::path::PathBuf, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            drop(stream);
        }
    });
    let home = std::path::PathBuf::from(std::env::var_os("HOME").expect("the test runner's HOME"));
    let connection = home.join(format!("engine-default-door-{port}.json"));
    std::fs::write(
        &connection,
        serde_json::json!({ "url": format!("http://127.0.0.1:{port}/tools/call"), "token": "a".repeat(64) }).to_string(),
    )
    .unwrap();
    (connection, asked)
}

/// Runs `choose_engine_in_a_child` in a child copy of this binary; returns what it printed and whether it passed.
fn choose_in_a_child(connection: &std::path::Path, engine: Option<&str>) -> (bool, String) {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args(["engine_default_tests::choose_engine_in_a_child", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env("LOCUST_MCP_CONNECTION", connection)
        .env_remove("LOCUST_NATIVE_ENGINE");
    if let Some(value) = engine {
        child.env("LOCUST_NATIVE_ENGINE", value);
    }
    let out = child.output().unwrap();
    let words = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.success(), words)
}

#[test]
fn the_window_runs_on_its_own_engine_while_electrons_door_is_up() {
    let (connection, asked) = door_that_counts();
    let (passed, words) = choose_in_a_child(&connection, None);
    assert!(passed, "{words}");
    assert!(words.contains("chose Native"), "{words}");
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0, "Electron's door must not be asked by default");

    // LOCUST_NATIVE_ENGINE=0 is the one way back to Electron's engine: the door is asked first.
    let (passed, words) = choose_in_a_child(&connection, Some("0"));
    assert!(passed, "{words}");
    assert!(asked.load(std::sync::atomic::Ordering::SeqCst) >= 1, "LOCUST_NATIVE_ENGINE=0 asks Electron's door: {words}");
}

/// Only ever run by the test above, in its own process.
#[test]
#[ignore = "run by the_window_runs_on_its_own_engine_while_electrons_door_is_up in a child process"]
fn choose_engine_in_a_child() {
    HEADLESS.store(true, std::sync::atomic::Ordering::Relaxed);
    match choose_engine() {
        Ok((kind, _)) => println!("chose {kind:?}"),
        Err(words) => println!("no engine: {words}"),
    }
}
