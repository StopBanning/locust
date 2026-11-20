//! A call the person declined, end to end on a Codex app server (Electron's codex-mission.ts `accept`, which passes
//! every record through `withDeclines`): Codex reports the declined command as a failed script, and Locust says it as
//! declined. The cases are in `approval_declined_tests.rs`; this is the wire they ride.
use super::*;

/// A fake app server: asks to run `git status` for `item`, reads Locust's answer, then reports the item failed (as
/// Codex 0.157 does for a declined command), another item failed on its own, and the turn's end.
fn declining_server(item: &str) -> Command {
    let script = format!(
        "import json, sys\n\
         def send(m): print(json.dumps(m), flush=True)\n\
         send({{'id':9,'method':'item/commandExecution/requestApproval','params':{{'threadId':'t1','itemId':'{item}','command':'git status','cwd':'/tmp'}}}})\n\
         answer = json.loads(sys.stdin.readline())\n\
         sys.stderr.write(json.dumps(answer))\n\
         for i in ['{item}', 'item_other']:\n\
         \x20   send({{'method':'item/completed','params':{{'threadId':'t1','item':{{'id':i,'type':'commandExecution','command':'git status','status':'failed','exitCode':1,'aggregatedOutput':'Rejected(\"approval request failed\")'}}}}}})\n\
         send({{'method':'turn/completed','params':{{'threadId':'t1','turn':{{'id':'turn_1','status':'completed'}}}}}})\n\
         sys.stdin.readline()\n"
    );
    let mut command = Command::new("python3");
    command.args(["-c", &script]);
    command
}

/// Every tool row the run reports, by item id, with its status, the person answering `decision` to the one card.
fn statuses_after(decision: crate::approval::Decision) -> Vec<(String, String, String)> {
    let mut runtime = Codex::spawn(&mut declining_server("item_1")).unwrap();
    let channel = Channel::new();
    runtime.normal.thread = Some("t1".into());
    runtime.mode = Mode::ApproveEach;
    runtime.run_id = "run_1".into();
    runtime.mission_id = "mission_1".into();
    runtime.approvals = Some(channel.clone());
    let ch = channel.clone();
    let decider = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(card) = ch.waiting().first() {
                ch.decide(crate::approval::Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: None }).unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let mut rows = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let Some(ev) = runtime.next_event().unwrap() else { continue };
        if ev.kind.starts_with("tool.") {
            rows.push((ev.payload["itemId"].as_str().unwrap_or("").to_owned(), ev.kind.clone(), ev.payload["status"].as_str().unwrap_or("").to_owned()));
        }
        if matches!(ev.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
            break;
        }
    }
    decider.join().unwrap();
    rows
}

#[test]
fn a_command_the_person_declined_reads_as_declined_when_codex_says_it_failed() {
    let rows = statuses_after(crate::approval::Decision::Deny);
    assert_eq!(
        rows,
        vec![
            ("item_1".into(), "tool.failed".into(), "declined".into()),
            ("item_other".into(), "tool.failed".into(), "failed".into()),
        ]
    );
}

#[test]
fn a_command_the_person_approved_that_fails_still_reads_as_failed() {
    let rows = statuses_after(crate::approval::Decision::ApproveOnce);
    assert_eq!(
        rows,
        vec![
            ("item_1".into(), "tool.failed".into(), "failed".into()),
            ("item_other".into(), "tool.failed".into(), "failed".into()),
        ]
    );
}
