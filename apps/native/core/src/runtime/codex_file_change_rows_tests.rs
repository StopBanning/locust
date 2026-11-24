//! Electron's `main/approval-channel.test.ts`, "a fileChange item carries its change into the activity row" (3 cases),
//! on a Codex app server's wire: codex-mission.ts passes every record through `withFileChanges`, and so does this
//! runtime (`approval::with_file_changes`), with the item's changes remembered by id and the run's folder.
use super::*;

/// A fake app server reporting, in a folder `C:\work\pebble`: an approved file change, a declined one, a shell command,
/// a file change it gave no changes for, and a message; then the turn's end.
fn server() -> Command {
    let script = "import json, sys\n\
         def send(m): print(json.dumps(m), flush=True)\n\
         ch = [{'path':'C:\\\\work\\\\pebble\\\\HELLO.txt','kind':{'type':'add'},'diff':'hello from wren\\n'}]\n\
         send({'method':'item/completed','params':{'threadId':'t1','item':{'id':'item_fc','type':'fileChange','changes':ch,'status':'completed'}}})\n\
         send({'method':'item/completed','params':{'threadId':'t1','item':{'id':'item_dc','type':'fileChange','changes':ch,'status':'declined'}}})\n\
         send({'method':'item/completed','params':{'threadId':'t1','item':{'id':'item_sh','type':'commandExecution','command':'ls','status':'completed','exitCode':0}}})\n\
         send({'method':'item/started','params':{'threadId':'t1','item':{'id':'item_other','type':'fileChange'}}})\n\
         send({'method':'item/completed','params':{'threadId':'t1','item':{'id':'m','type':'agentMessage','text':'hi'}}})\n\
         send({'method':'turn/completed','params':{'threadId':'t1','turn':{'id':'turn_1','status':'completed'}}})\n\
         sys.stdin.readline()\n";
    let mut command = Command::new("python3");
    command.args(["-c", script]);
    command
}

/// Every event the run reports before its end.
fn rows() -> Vec<crate::runtime::Event> {
    let mut runtime = Codex::spawn(&mut server()).unwrap();
    runtime.normal.thread = Some("t1".into());
    runtime.run_id = "run_1".into();
    runtime.mission_id = "mission_1".into();
    runtime.folder = "C:\\work\\pebble".into();
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let Some(ev) = runtime.next_event().unwrap() else { continue };
        if matches!(ev.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
            break;
        }
        out.push(ev);
    }
    out
}

fn row<'a>(rows: &'a [crate::runtime::Event], item: &str) -> &'a crate::runtime::Event {
    rows.iter().find(|e| e.payload["itemId"] == item).unwrap_or_else(|| panic!("no row for {item}: {:?}", rows.iter().map(|e| (&e.kind, &e.payload)).collect::<Vec<_>>()))
}

// case: gives a tool row the file's path and a patch, where the normaliser wrote only a count
#[test]
fn an_approved_file_change_row_carries_its_path_and_patch() {
    let rows = rows();
    let fc = row(&rows, "item_fc");
    assert_eq!(fc.kind, "tool.completed");
    assert_eq!(fc.payload["command"], "HELLO.txt");
    assert_eq!(fc.payload["patch"]["added"], 1);
    assert!(fc.payload["patch"]["text"].as_str().unwrap_or("").contains("+hello from wren"), "{:?}", fc.payload);
}

// case: gives a declined fileChange its paths but no patch
#[test]
fn a_declined_file_change_row_names_its_paths_and_carries_no_patch() {
    let rows = rows();
    let dc = row(&rows, "item_dc");
    assert_eq!(dc.kind, "tool.failed");
    assert_eq!(dc.payload["command"], "HELLO.txt");
    assert!(dc.payload.get("patch").is_none(), "{:?}", dc.payload);
}

// case: leaves every other event, and a fileChange it never saw, exactly as it was
#[test]
fn other_rows_and_an_unseen_file_change_are_left_as_they_were() {
    let rows = rows();
    let shell = row(&rows, "item_sh");
    assert_eq!((shell.kind.as_str(), &shell.payload["command"]), ("tool.completed", &json!("ls")));
    assert!(shell.payload.get("patch").is_none());
    let unseen = row(&rows, "item_other");
    assert_eq!(unseen.kind, "tool.started");
    assert!(unseen.payload.get("command").is_none() && unseen.payload.get("patch").is_none(), "{:?}", unseen.payload);
    let message = row(&rows, "m");
    assert_eq!((message.kind.as_str(), &message.payload["text"]), ("message.delta", &json!("hi")));
}
