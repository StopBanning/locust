//! Speed 1, row 1: a resting window's reads share one kept connection, so it opens no sockets and the engine starts no
//! threads; a kept connection the engine closed is made again once; a request that changes something is still sent
//! once, on its own connection. And the reads never wait on the socket: a thread keeps the last answers, an action
//! makes the next read ask the engine, and a changed answer (only a changed one) wakes the window.
use super::*;
use std::sync::atomic::AtomicUsize;

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "locust-kept-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// An engine that names itself in `running` and counts the stops it is asked for.
struct Named {
    name: &'static str,
    stops: AtomicUsize,
}

impl Named {
    fn new(name: &'static str) -> Arc<Named> {
        Arc::new(Named { name, stops: AtomicUsize::new(0) })
    }
}

impl EngineApi for Named {
    fn send(&self, _: Send, _: Origin) -> Result<Started, String> {
        Err("not in this test".into())
    }
    fn stop(&self, _: &str) -> Result<String, String> {
        self.stops.fetch_add(1, Ordering::Relaxed);
        Ok("Stop requested".into())
    }
    fn owns(&self, conversation: &str) -> bool {
        conversation == self.name
    }
    fn running(&self) -> Vec<String> {
        vec![self.name.to_owned()]
    }
    fn waiting(&self) -> Vec<Request> {
        Vec::new()
    }
    fn decide(&self, _: &str, _: Decision, _: Option<String>) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &str, _: Vec<(String, Vec<String>)>) -> Result<(), String> {
        Ok(())
    }
    fn save_rule_and_decide(&self, _: &Request, _: RuleEffect) -> Result<(), String> {
        Ok(())
    }
    fn carry(&self, _: &str, _: &str, _: Option<&str>) -> Result<(String, Option<String>, String), String> {
        Err("not in this test".into())
    }
    fn shut_down(&self) {}
    fn wait_for_shutdown(&self) {}
    fn room_notices(&self) -> Vec<crate::room_tasks::RoomNotice> {
        Vec::new()
    }
}

fn accepted(server: &Server) -> usize {
    server.accepted.load(Ordering::Relaxed)
}

#[test]
fn a_resting_window_s_reads_share_one_connection() {
    let dir = dir();
    let engine = Named::new("mission_a");
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::new(server.socket().to_path_buf());
    // A window's poll at rest asks these every tick; a hundred ticks of them.
    for _ in 0..100 {
        assert_eq!(window.running(), vec!["mission_a".to_string()]);
        assert!(window.waiting().is_empty());
        assert!(window.owns("mission_a") && !window.owns("mission_b"));
        assert!(window.running_checks().is_empty());
        assert!(window.room_notices().is_empty());
    }
    assert!(window.answers());
    assert_eq!(accepted(&server), 1, "every read went over the one kept connection");
    // A change still goes on its own connection, as it always did.
    assert_eq!(window.stop("mission_a").as_deref(), Ok("Stop requested"));
    assert_eq!(accepted(&server), 2);
    assert_eq!(engine.stops.load(Ordering::Relaxed), 1);
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Until `done`, or two seconds.
fn soon(what: &str, mut done: impl FnMut() -> bool) {
    let end = std::time::Instant::now() + Duration::from_secs(2);
    while !done() {
        assert!(std::time::Instant::now() < end, "never: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_kept_connection_the_engine_closed_is_made_again_once() {
    let dir = dir();
    let first = Server::start_with(&dir, Named::new("first")).unwrap();
    let window = Client::with(first.socket().to_path_buf(), None, Duration::from_millis(20));
    assert_eq!(window.running(), vec!["first".to_string()]);
    assert_eq!(accepted(&first), 1);
    // The engine stops and another starts in its place: the kept connection is closed under the window's reader.
    drop(first);
    let second = Server::start_with(&dir, Named::new("second")).unwrap();
    soon("the engine that is there answers", || window.running() == vec!["second".to_string()]);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(window.running(), vec!["second".to_string()]);
    assert_eq!(accepted(&second), 1, "made again once, then kept");
    // With no engine at all, a read finds nothing and a change says the engine is not running.
    drop(second);
    soon("a read finds no engine", || window.running().is_empty());
    assert_eq!(window.stop("x").unwrap_err(), "Locust's engine is not running.");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A socket that answers reads and keeps its connections open, but reads a `stop` and closes without a reply: a reply
/// lost after the engine may have acted. Counts the stops it read.
fn losing_stops(socket: &Path) -> Arc<AtomicUsize> {
    let listener = UnixListener::bind(socket).unwrap();
    let stops = Arc::new(AtomicUsize::new(0));
    let seen = stops.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = seen.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let request: Value = serde_json::from_str(&line).unwrap();
                    line.clear();
                    if request["op"] == "stop" {
                        seen.fetch_add(1, Ordering::Relaxed);
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        return;
                    }
                    let ok = if request["op"] == "reads" {
                        json!({"running": ["mission_a"], "running_checks": [], "waiting": [], "room_notices": []})
                    } else {
                        json!(["mission_a"])
                    };
                    let mut out = &stream;
                    let _ = out.write_all(format!("{}\n", json!({"ok": ok})).as_bytes());
                }
            });
        }
    });
    stops
}

#[test]
fn a_change_whose_reply_is_lost_is_never_sent_again() {
    let dir = dir();
    let socket = dir.join(SOCKET);
    let stops = losing_stops(&socket);
    let window = Client::new(socket);
    assert_eq!(window.running(), vec!["mission_a".to_string()], "a read leaves a kept connection");
    assert!(window.stop("mission_a").is_err(), "no reply is an error, said as such");
    assert_eq!(stops.load(Ordering::Relaxed), 1, "the stop reached the engine once and was not asked again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An engine whose running list the test changes, which can be slow to answer, and which hands over its room notices
/// once each.
struct Shifting {
    running: std::sync::Mutex<Vec<String>>,
    notices: std::sync::Mutex<Vec<crate::room_tasks::RoomNotice>>,
    slow: Duration,
    /// While set, a `running` answer is taken and then held until `release`; `held` counts the answers held.
    hold: std::sync::atomic::AtomicBool,
    release: std::sync::atomic::AtomicBool,
    held: AtomicUsize,
    /// How many times `running` was asked.
    asked: AtomicUsize,
}

impl Shifting {
    fn new(running: &str, slow: Duration) -> Arc<Shifting> {
        Arc::new(Shifting {
            running: std::sync::Mutex::new(vec![running.to_owned()]),
            notices: Default::default(),
            slow,
            hold: Default::default(),
            release: Default::default(),
            held: AtomicUsize::new(0),
            asked: AtomicUsize::new(0),
        })
    }
    fn set(&self, running: &str) {
        *self.running.lock().unwrap() = running.split(' ').map(str::to_owned).collect();
    }
}

impl EngineApi for Shifting {
    fn send(&self, _: Send, _: Origin) -> Result<Started, String> {
        Err("not in this test".into())
    }
    fn stop(&self, _: &str) -> Result<String, String> {
        Ok("Stop requested".into())
    }
    fn owns(&self, conversation: &str) -> bool {
        self.running.lock().unwrap().iter().any(|id| id == conversation)
    }
    fn running(&self) -> Vec<String> {
        let n = self.asked.fetch_add(1, Ordering::Relaxed);
        let mut said = self.running.lock().unwrap().clone();
        // The same turns, in another order every other time, as a list built from a map may come back.
        if n % 2 == 1 {
            said.reverse();
        }
        std::thread::sleep(self.slow);
        if self.hold.load(Ordering::Relaxed) {
            self.held.fetch_add(1, Ordering::Relaxed);
            while !self.release.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        said
    }
    fn waiting(&self) -> Vec<Request> {
        Vec::new()
    }
    fn decide(&self, _: &str, _: Decision, _: Option<String>) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &str, _: Vec<(String, Vec<String>)>) -> Result<(), String> {
        Ok(())
    }
    fn save_rule_and_decide(&self, _: &Request, _: RuleEffect) -> Result<(), String> {
        Ok(())
    }
    fn carry(&self, _: &str, _: &str, _: Option<&str>) -> Result<(String, Option<String>, String), String> {
        Err("not in this test".into())
    }
    fn shut_down(&self) {}
    fn wait_for_shutdown(&self) {}
    fn room_notices(&self) -> Vec<crate::room_tasks::RoomNotice> {
        std::mem::take(&mut *self.notices.lock().unwrap())
    }
}

#[test]
fn an_answer_asked_before_an_action_is_never_kept_after_it() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(300));
    assert_eq!(window.running(), vec!["mission_a".to_string()]);
    // The reader's next answer is taken now (mission_a) and held on its way back while the window stops the turn.
    engine.hold.store(true, Ordering::Relaxed);
    soon("the reader asks", || engine.held.load(Ordering::Relaxed) == 1);
    engine.hold.store(false, Ordering::Relaxed);
    engine.set("mission_b");
    assert_eq!(window.stop("mission_a").as_deref(), Ok("Stop requested"));
    engine.release.store(true, Ordering::Relaxed);
    // The held answer arrives well before the reader asks again (300 ms on).
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(window.running(), vec!["mission_b".to_string()], "the answer from before the stop was not kept");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_never_waits_on_the_socket() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::from_millis(300));
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(50));
    assert_eq!(window.running(), vec!["mission_a".to_string()], "the first read asks the engine");
    // Every read after it is answered from what the reader keeps, though the engine takes 300 ms to say anything.
    for _ in 0..50 {
        let asked = std::time::Instant::now();
        assert!(window.owns("mission_a") && window.waiting().is_empty() && window.running_checks().is_empty());
        assert!(asked.elapsed() < Duration::from_millis(50), "a read waited {:?}", asked.elapsed());
    }
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_action_makes_the_next_read_ask_the_engine() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    // A reader too slow to notice anything in this test: what changes the reads here is the action.
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_secs(60));
    assert_eq!(window.running(), vec!["mission_a".to_string()]);
    engine.set("mission_b");
    assert_eq!(window.running(), vec!["mission_a".to_string()], "kept, not asked again");
    assert_eq!(window.stop("mission_a").as_deref(), Ok("Stop requested"));
    assert_eq!(window.running(), vec!["mission_b".to_string()], "after a stop, the engine is asked");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_answer_wakes_the_window_once_and_an_unchanged_one_never() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(20));
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    *window.inner.wake.lock().unwrap() = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    assert_eq!(window.running(), vec!["mission_a".to_string()]);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(wakes.load(Ordering::Relaxed), 0, "fifteen unchanged answers: no wake");
    engine.set("mission_b");
    soon("the change wakes the window", || wakes.load(Ordering::Relaxed) > 0);
    assert_eq!(window.running(), vec!["mission_b".to_string()], "and the read says it at once");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(wakes.load(Ordering::Relaxed), 1, "one change, one wake");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_room_notice_is_handed_over_once_and_wakes_the_window() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(20));
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    *window.inner.wake.lock().unwrap() = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    assert!(window.room_notices().is_empty());
    let notice = crate::room_tasks::RoomNotice { room_id: "room_1".into(), room_name: "Ops".into(), message: "Task done".into() };
    engine.notices.lock().unwrap().push(notice.clone());
    soon("the notice wakes the window", || wakes.load(Ordering::Relaxed) > 0);
    assert_eq!(window.room_notices(), vec![notice]);
    assert!(window.room_notices().is_empty(), "once");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An engine from before `reads`: it answers each read on its own and says it does not know `reads`.
fn older_engine(socket: &Path) {
    let listener = UnixListener::bind(socket).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let request: Value = serde_json::from_str(&line).unwrap();
                    line.clear();
                    let reply = match request["op"].as_str() {
                        Some("running") => json!({"ok": ["mission_old"]}),
                        Some("running_checks" | "waiting" | "room-notices") => json!({"ok": []}),
                        Some(other) => json!({"error": format!("The engine does not know the request {other}.")}),
                        None => json!({"error": "no op"}),
                    };
                    let mut out = &stream;
                    let _ = out.write_all(format!("{reply}\n").as_bytes());
                }
            });
        }
    });
}

#[test]
fn an_engine_from_before_reads_is_asked_read_by_read() {
    let dir = dir();
    let socket = dir.join(SOCKET);
    older_engine(&socket);
    let window = Client::new(socket);
    assert_eq!(window.running(), vec!["mission_old".to_string()]);
    assert!(window.owns("mission_old") && window.waiting().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_engine_answers_every_read_in_one_request() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let notice = crate::room_tasks::RoomNotice { room_id: "room_1".into(), room_name: "Ops".into(), message: "Task done".into() };
    engine.notices.lock().unwrap().push(notice);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let stream = UnixStream::connect(server.socket()).unwrap();
    let mut out = &stream;
    out.write_all(b"{\"op\":\"reads\"}\n").unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    let said: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(said["ok"]["running"], json!(["mission_a"]), "{line}");
    assert_eq!(said["ok"]["running_checks"], json!([]), "{line}");
    assert_eq!(said["ok"]["waiting"], json!([]), "{line}");
    assert_eq!(said["ok"]["room_notices"][0]["message"], json!("Task done"), "{line}");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_closing_window_s_client_stops_asking_and_never_starts_the_engine_again() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let count = starts.clone();
    let restart: Restart = Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
        Err("not in this test".into())
    });
    let window = Client::with(server.socket().to_path_buf(), Some(restart), Duration::from_millis(20));
    assert_eq!(window.running(), vec!["mission_a".to_string()]);
    window.shut_down();
    window.wait_for_shutdown();
    // A refresh already on its way ends; after it the reader asks nothing more.
    std::thread::sleep(Duration::from_millis(60));
    let asked = engine.asked.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(engine.asked.load(Ordering::Relaxed), asked, "the reader stopped with the window");
    // The engine goes, as it does when asked to shut down: a read finds none, and none is started for a closing window.
    drop(server);
    assert!(window.running().is_empty());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(starts.load(Ordering::Relaxed), 0, "no engine started after the window asked it to shut down");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_reaches_the_wake_the_window_gave() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    // No wake of its own: the client wakes whoever gave `engine_api::on_change` theirs, as the window's setup does.
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(20));
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    crate::engine_api::on_change(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    assert_eq!(window.running(), vec!["mission_a".to_string()]);
    engine.set("mission_b");
    soon("the window's wake hears the change", || wakes.load(Ordering::Relaxed) > 0);
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_turns_in_another_order_never_wake_the_window() {
    let dir = dir();
    let engine = Shifting::new("mission_a", Duration::ZERO);
    engine.set("mission_a mission_b");
    let server = Server::start_with(&dir, engine.clone()).unwrap();
    let window = Client::with(server.socket().to_path_buf(), None, Duration::from_millis(20));
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    *window.inner.wake.lock().unwrap() = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    assert_eq!(window.running().len(), 2);
    std::thread::sleep(Duration::from_millis(300));
    assert!(engine.asked.load(Ordering::Relaxed) > 4, "the reader asked again and again");
    assert_eq!(wakes.load(Ordering::Relaxed), 0, "two turns in turn-about order are no change");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
