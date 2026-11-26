//! Speed 1, row 1: a resting window's reads share one kept connection, so it opens no sockets and the engine starts no
//! threads; a kept connection the engine closed is made again once; a request that changes something is still sent
//! once, on its own connection.
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

#[test]
fn a_kept_connection_the_engine_closed_is_made_again_once() {
    let dir = dir();
    let first = Server::start_with(&dir, Named::new("first")).unwrap();
    let window = Client::new(first.socket().to_path_buf());
    assert_eq!(window.running(), vec!["first".to_string()]);
    assert_eq!(accepted(&first), 1);
    // The engine stops and another starts in its place: the kept connection is closed under the window.
    drop(first);
    let second = Server::start_with(&dir, Named::new("second")).unwrap();
    assert_eq!(window.running(), vec!["second".to_string()], "the read is asked again of the engine that is there");
    assert_eq!(window.running(), vec!["second".to_string()]);
    assert_eq!(accepted(&second), 1, "made again once, then kept");
    // With no engine at all, a read finds nothing and a change says the engine is not running.
    drop(second);
    assert!(window.running().is_empty());
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
                    let mut out = &stream;
                    let _ = out.write_all(format!("{}\n", json!({"ok": ["mission_a"]})).as_bytes());
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
