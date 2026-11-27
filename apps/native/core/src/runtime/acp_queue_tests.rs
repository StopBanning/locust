use super::*;
use crate::load_waits::wait_for;

fn start_without_draining(scenario: &str) -> Acp {
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/fixtures/m4-acp.py"
        ))
        .arg(scenario);
    let mut acp = Acp::spawn(&mut command).unwrap();
    acp.start(&Turn {
        prompt: "Synthetic ACP prompt".into(),
        folder: std::env::current_dir().unwrap(),
        model: None,
        effort: None,
        resume: None,
        mode: crate::approval::Mode::ApproveEach,
        approvals: None,
        mission_id: "acp_queue".into(),
        run_id: "run_acp_queue".into(),
    })
    .unwrap();

    // The reader sees the prompt answer only after the entire burst. The
    // engine is busy until then, so it cannot make room in the queue.
    wait_for("ACP prompt answer or queue failure", || {
        acp.answered.load(Ordering::Relaxed) || crate::invariants::lock(&acp.failure).is_some()
    });
    acp
}

fn drain_turn(mut acp: Acp) -> Vec<Event> {
    let mut events = Vec::new();
    wait_for("ACP terminal event", || {
        // Drain a batch before polling again to keep pace with the burst.
        for _ in 0..512 {
            if let Some(event) = acp.next_event().unwrap() {
                let terminal = matches!(event.kind.as_str(), "run.completed" | "run.failed");
                events.push(event);
                if terminal {
                    return true;
                }
            }
        }
        false
    });
    events
}

#[test]
fn acp_burst_waits_in_the_queue_until_the_engine_reads() {
    let events = drain_turn(start_without_draining("burst"));
    let terminal = events.last().unwrap();
    assert_eq!(terminal.kind, "run.completed", "{:?}", terminal.payload);
    assert!(events.iter().any(|event| {
        event.kind == "message.delta"
            && event.payload["final"] == true
            && event.payload["text"] == "Burst complete."
    }));
}

#[test]
fn acp_burst_over_the_queue_bound_fails_without_losing_the_reason() {
    let events = drain_turn(start_without_draining("flood"));
    let terminal = events.last().unwrap();
    assert_eq!(terminal.kind, "run.failed");
    // Electron's sentence (acp-run.ts `lose`), the one Claude Code's and Codex's readers say (SOAK-NOTES row 2a).
    assert_eq!(terminal.payload["message"], OVERFLOW_MESSAGE);
    assert_eq!(terminal.payload["process"]["outputLimitExceeded"], true);
}
