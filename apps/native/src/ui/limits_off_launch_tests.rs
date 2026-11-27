//! speed-2 row 3: setup read every ledger on the window's thread before the first frame, to learn which runtimes are
//! at their usage limit (`load_composer_store` -> `limits::limited_runtimes`). It is now read off that thread
//! (`limits_job`) and set by poll (`take_limits`), so setup returns before the read and the composer learns the limit a
//! moment later. `Store::local` reads the environment, so the load runs in a child copy of this test binary with
//! `LOCUST_NATIVE_DATA` set on that child alone (as engine_default_tests.rs does).
use super::*;

/// A data folder whose native ledger holds one conversation that hit codex's usage limit.
fn data_with_a_limit(tag: &str) -> std::path::PathBuf {
    let data = std::env::temp_dir().join(format!("locust-limits-off-launch-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let ledger = data.join("mission-ledger");
    std::fs::create_dir_all(&ledger).unwrap();
    let records = [
        serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
            "missionId": "mission_1", "runId": "run_1", "prompt": "Look.", "runtime": "codex", "model": "account-default",
            "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "workspaceId": "ws_test",
            "createdAt": "2026-11-27T05:00:00.000Z"}}),
        serde_json::json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"id": "limit-1", "runId": "run_1",
            "missionId": "mission_1", "sequence": 1, "type": "route.limit_detected", "occurredAt": "2026-11-27T05:01:00.000Z",
            "sourceAdapter": "codex", "payload": {"kind": "quota-exhausted", "message": "You have hit your usage limit.",
            "evidence": {"redacted": true}}}}),
    ];
    std::fs::write(ledger.join("mission_1.jsonl"), records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
    data
}

#[test]
fn setup_leaves_the_limits_read_to_a_job_and_poll_sets_what_it_found() {
    let data = data_with_a_limit("child");
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .env("LOCUST_NATIVE_DATA", &data)
        .args([
            "limits_off_launch_tests::load_and_poll_in_a_child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&data);
    let words = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{words}");
    assert!(words.contains("limits after setup: none"), "{words}");
    assert!(words.contains("limits after poll: codex"), "{words}");
}

/// Only ever run by the test above, in its own process.
#[test]
#[ignore = "run by setup_leaves_the_limits_read_to_a_job_and_poll_sets_what_it_found in a child process"]
fn load_and_poll_in_a_child() {
    HEADLESS.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut s = State::default();
    load_composer_store(&mut s);
    assert!(s.data.is_some(), "the store opened");
    // The ledger holds a limit, so had setup read it on this thread it would be here already.
    let after_setup = s.limited_runtimes.keys().cloned().collect::<Vec<_>>().join(",");
    println!("limits after setup: {}", if after_setup.is_empty() { "none" } else { &after_setup });
    let until = std::time::Instant::now() + Duration::from_secs(20);
    let mut tx = Tx::default();
    while !s.limited_runtimes.contains_key("codex") && std::time::Instant::now() < until {
        poll(&mut s, 0.0, &mut tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("limits after poll: {}", s.limited_runtimes.keys().cloned().collect::<Vec<_>>().join(","));
}
