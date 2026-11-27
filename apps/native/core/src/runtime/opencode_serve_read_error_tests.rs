use super::*;
use std::io::Cursor;

const SESSION: &str = "ses_dropped";
const FAILURE: &str = "stream transport failed";

struct Broken;
impl Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other(FAILURE))
    }
}

fn stream(records: &[Value]) -> Cursor<Vec<u8>> {
    Cursor::new(
        records
            .iter()
            .map(|v| format!("data: {v}\n\n"))
            .collect::<String>()
            .into_bytes(),
    )
}

fn part(text: &str) -> Value {
    json!({"type":"message.part.updated","properties":{"sessionID":SESSION,
        "part":{"id":"part_1","type":"text","messageID":"msg_answer","text":text}}})
}

// case: keeps an explicit stream read error as a failure
#[test]
fn a_stream_read_error_keeps_its_reason_and_the_reply_already_read() {
    let mut serve = Serve::new(Command::new("opencode"), Arc::new(AtomicBool::new(false)));
    serve.session = SESSION.into();
    serve.prompted = true;
    // No account, server or HTTP recovery is needed. The fixture's child represents the owned server;
    // the real next_event must stop it on failure rather than leave it for Drop.
    serve.process = Some(Process::spawn(Command::new("sh").args(["-c", "exec sleep 60"])).unwrap());
    let (tx, rx) = event_channel();
    serve.rx = Some(rx);
    let sid = Mutex::new(Some(SESSION.to_owned()));
    let open = AtomicBool::new(false);
    let overflow = AtomicBool::new(false);
    let assistant = json!({"type":"message.updated","properties":{"sessionID":SESSION,
        "info":{"id":"msg_answer","role":"assistant"}}});
    read_events(
        stream(&[assistant, part("Hello")]),
        &tx,
        &open,
        &sid,
        &overflow,
    );
    let mut events: Vec<_> = (0..2).filter_map(|_| serve.next_event().unwrap()).collect();
    assert_eq!(serve.parts["part_1"].sent, "Hello");
    // Hold the paced emission even on a loaded CI machine.
    serve.parts.get_mut("part_1").unwrap().last = Some(Instant::now() + Duration::from_secs(30));
    read_events(
        BufReader::new(stream(&[part("Hello there")]).chain(Broken)),
        &tx,
        &open,
        &sid,
        &overflow,
    );
    drop(tx);
    serve.stream_ended.store(true, Ordering::Release);
    // Normal EOF without an API would attempt recovery and end with a different reason.
    for _ in 0..8 {
        if let Some(event) = serve.next_event().unwrap() {
            events.push(event);
        }
        if serve.ended && serve.queued.is_empty() {
            break;
        }
    }
    let end = events.last().expect("the run settled");
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["process"]["exitCode"], 1);
    assert_eq!(end.payload["process"]["stderr"], FAILURE, "{}", end.payload);
    assert_eq!(end.payload["message"], FAILURE, "{}", end.payload);
    assert!(!serve.overflow.load(Ordering::Relaxed));
    let reply: String = events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .map(|e| e.payload["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        reply, "Hello there",
        "the two paced fragments must reach the ledger before failure"
    );
    assert!(
        serve.recovery.is_none(),
        "a read failure must not be treated as a recoverable EOF"
    );
    assert!(
        serve.process.as_mut().unwrap().exited().unwrap().is_some(),
        "the owned server is still alive"
    );
    assert!(
        serve.next_event().unwrap().is_none(),
        "there is only one terminal event"
    );
}

// Lane check of B40: the reader sends its last item before it marks the stream ended, so a watchdog that finds the
// stream ended must read that item first. A read error still waiting is the run's failure, not a clean EOF, and
// starts no recovery; with nothing waiting, a closed stream does start one.
#[test]
fn an_error_still_queued_when_the_stream_ends_is_the_failure_and_starts_no_recovery() {
    let mut serve = Serve::new(Command::new("opencode"), Arc::new(AtomicBool::new(false)));
    serve.session = SESSION.into();
    serve.prompted = true;
    let (tx, rx) = event_channel();
    serve.rx = Some(rx);
    tx.send(Err(io::Error::other(FAILURE))).unwrap();
    serve.stream_ended.store(true, Ordering::Release);
    assert!(!serve.begin_eof_recovery(), "a queued read error was taken for a clean end of the stream");
    assert!(serve.ended, "the queued read error was never read");
    assert!(serve.recovery.is_none(), "a read failure began EOF recovery");
    let end = serve.queued.back().expect("the run settled");
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["message"], FAILURE, "{}", end.payload);

    let mut clean = Serve::new(Command::new("opencode"), Arc::new(AtomicBool::new(false)));
    clean.session = SESSION.into();
    clean.prompted = true;
    let (tx, rx) = event_channel();
    clean.rx = Some(rx);
    drop(tx);
    clean.stream_ended.store(true, Ordering::Release);
    assert!(clean.begin_eof_recovery(), "a clean end of the stream began no recovery");
    assert!(!clean.ended && clean.recovery.is_some());
}
