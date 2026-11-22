//! Electron `apps/desktop/src/main/opencode-2-models-come-from-its-acp-session.test.ts` (20 cases).
use super::*;
use crate::models::{kept_catalog, opencode_models_for_version, read_discovered_checked};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

fn legacy_hints() -> Value {
    json!({
        "aliases": [],
        "efforts": [],
        "models": [{"id": "old/stale-free", "displayName": "Stale", "efforts": ["max"]}]
    })
}

fn runtime(major: u32) -> ProbeRuntime {
    let mut env = BTreeMap::new();
    env.insert("ONLY".into(), "inherited".into());
    ProbeRuntime {
        id: "opencode".into(),
        ready: true,
        major: Some(major),
        executable: Some(ProbeLaunch {
            executable_path: "/bin/opencode".into(),
            prefix_args: vec!["shim".into()],
            env,
        }),
        hints: Some(legacy_hints()),
    }
}

fn sample_session() -> Value {
    json!({
        "sessionId": "catalog-only",
        "configOptions": [
            {
                "id": "model",
                "category": "model",
                "type": "select",
                "currentValue": "opencode/zen-free",
                "options": [
                    { "group": "OpenCode", "options": [{ "value": "opencode/zen-free", "name": "Zen Free" }] },
                    { "value": "provider/model", "name": "A paid model" },
                    { "value": "opencode/zen-free", "name": "Duplicate" }
                ]
            },
            {
                "id": "effort",
                "category": "thought_level",
                "options": [{ "value": "high", "name": "High" }]
            }
        ]
    })
}

type OnRequest = Box<dyn Fn(&Value, &mpsc::Sender<ProbeEvent>) -> bool + Send + Sync>;

struct MockServer {
    messages: Arc<Mutex<Vec<Value>>>,
    kills: Arc<AtomicUsize>,
    tx: mpsc::Sender<ProbeEvent>,
    rx: Option<mpsc::Receiver<ProbeEvent>>,
    answer: Value,
    on_request: Option<OnRequest>,
    fail_write_on: Option<Box<dyn Fn(&Value) -> bool + Send + Sync>>,
    fail_kill: bool,
}

impl MockServer {
    fn new(answer: Value) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            kills: Arc::new(AtomicUsize::new(0)),
            tx,
            rx: Some(rx),
            answer,
            on_request: None,
            fail_write_on: None,
            fail_kill: false,
        }
    }

    fn with_handler(mut self, f: impl Fn(&Value, &mpsc::Sender<ProbeEvent>) -> bool + Send + Sync + 'static) -> Self {
        self.on_request = Some(Box::new(f));
        self
    }

    fn with_fail_write(mut self, f: impl Fn(&Value) -> bool + Send + Sync + 'static) -> Self {
        self.fail_write_on = Some(Box::new(f));
        self
    }

    fn with_fail_kill(mut self) -> Self {
        self.fail_kill = true;
        self
    }

    fn finish(mut self) -> (Box<dyn AcpProbeProcess>, mpsc::Receiver<ProbeEvent>) {
        let rx = self.rx.take().expect("receiver taken once");
        (Box::new(self), rx)
    }
}

impl AcpProbeProcess for MockServer {
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let req: Value = serde_json::from_str(line).expect("valid jsonrpc");
        if self.fail_write_on.as_ref().is_some_and(|f| f(&req)) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"));
        }
        self.messages.lock().unwrap().push(req.clone());
        if req.get("method").and_then(Value::as_str).is_none() {
            return Ok(());
        }
        if let Some(handler) = &self.on_request
            && handler(&req, &self.tx)
        {
            return Ok(());
        }
        let result = if req["method"] == "initialize" {
            json!({"protocolVersion": 1})
        } else {
            self.answer.clone()
        };
        let reply = format!("{}\n", json!({"jsonrpc": "2.0", "id": req["id"], "result": result}));
        let _ = self.tx.send(ProbeEvent::Data(reply.into_bytes()));
        Ok(())
    }

    fn kill(&mut self) -> io::Result<()> {
        self.kills.fetch_add(1, Ordering::SeqCst);
        if self.fail_kill {
            Err(io::Error::other("process already gone"))
        } else {
            Ok(())
        }
    }
}

fn make_scratch() -> io::Result<PathBuf> {
    default_scratch("locust-catalog-unit")
}

// case: opens an empty no-prompt session and lists only its advertised models, including free grouped rows
#[test]
fn opens_an_empty_no_prompt_session_and_lists_only_its_advertised_models_including_free_grouped_rows() {
    let mock = MockServer::new(sample_session());
    let messages = mock.messages.clone();
    let kills = mock.kills.clone();
    let child_slot = Mutex::new(Some(mock.finish()));
    let spawned: Mutex<Vec<(String, Vec<String>, BTreeMap<String, String>, PathBuf)>> = Mutex::new(Vec::new());
    let dir_holder: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    let dir_out = dir_holder.clone();

    let spawn = |exe: &str, args: &[String], env: &BTreeMap<String, String>, cwd: &Path| {
        spawned
            .lock()
            .unwrap()
            .push((exe.to_owned(), args.to_vec(), env.clone(), cwd.to_path_buf()));
        Ok(child_slot.lock().unwrap().take().expect("spawn once"))
    };

    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(move || {
                let d = make_scratch()?;
                *dir_out.lock().unwrap() = Some(d.clone());
                Ok(d)
            })),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );

    let dir = dir_holder.lock().unwrap().clone().expect("scratch dir created");
    let calls = spawned.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/bin/opencode");
    assert_eq!(calls[0].1, vec!["shim", "acp"]);
    assert_eq!(calls[0].2.get("ONLY").map(String::as_str), Some("inherited"));
    assert_eq!(calls[0].3, dir);

    let sent = messages.lock().unwrap();
    let methods: Vec<&str> = sent.iter().filter_map(|m| m["method"].as_str()).collect();
    assert_eq!(methods, vec!["initialize", "session/new"]);
    assert_eq!(
        sent[0]["params"],
        json!({"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}, "terminal": false}})
    );
    assert_eq!(sent[1]["params"], json!({"cwd": dir.to_string_lossy(), "mcpServers": []}));

    let ids: Vec<&str> = models.iter().filter_map(|m| m["id"].as_str()).collect();
    assert_eq!(ids, vec!["opencode/zen-free", "provider/model"]);
    assert_eq!(models[0]["runtime"], "opencode");
    assert_eq!(models[0]["displayName"], "Zen Free");
    assert_eq!(models[0]["description"], "Free · no sign-in");
    assert_eq!(models[1]["description"], "Through OpenCode");
    assert_eq!(kills.load(Ordering::SeqCst), 1);
    assert!(!dir.exists(), "scratch dir must be removed");
}

// case: does not turn the current session effort menu into a promise about every model
#[test]
fn does_not_turn_the_current_session_effort_menu_into_a_promise_about_every_model() {
    let slot = Mutex::new(Some(MockServer::new(sample_session()).finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    let efforts: Vec<&Value> = models.iter().map(|m| &m["supportedEfforts"]).collect();
    assert_eq!(efforts, vec![&json!([]), &json!([])]);
}

// case: rejects unexpected client operations without reading files or approving permission
#[test]
fn rejects_unexpected_client_operations_without_reading_files_or_approving_permission() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        if req["method"] != "session/new" {
            return false;
        }
        let perm = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": "permission", "method": "session/request_permission", "params": {"options": [{"kind": "allow_always"}]}})
        );
        let reply = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": req["id"], "result": sample_session()})
        );
        let _ = tx.send(ProbeEvent::Data(perm.into_bytes()));
        let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
        true
    });
    let messages = mock.messages.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert_eq!(models.len(), 2);
    let sent = messages.lock().unwrap();
    let refusal = sent
        .iter()
        .find(|m| m["id"] == "permission")
        .expect("refusal written");
    assert_eq!(refusal["error"]["code"], -32601);
}

// case: keeps the V1 CLI list unchanged and never offers its stale hints for V2
#[test]
fn keeps_the_v1_cli_list_unchanged_and_never_offers_its_stale_hints_for_v2() {
    let hints = legacy_hints();
    assert_eq!(
        opencode_models_for_version(Some(1), Some(&hints)),
        vec![json!({
            "id": "old/stale-free",
            "runtime": "opencode",
            "displayName": "Stale",
            "description": "Free · no sign-in",
            "supportedEfforts": ["max"]
        })]
    );
    assert!(opencode_models_for_version(Some(2), Some(&hints)).is_empty());
}

// case: makes the catalog select ACP for V2 and caches a complete advertisement
#[test]
fn makes_the_catalog_select_acp_for_v2_and_caches_a_complete_advertisement() {
    let cache = Mutex::new(None);
    let discover_calls = AtomicUsize::new(0);
    let spawn_calls = AtomicUsize::new(0);
    let read = || {
        discover_calls.fetch_add(1, Ordering::SeqCst);
        let runtimes = vec![runtime(2)];
        let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
            spawn_calls.fetch_add(1, Ordering::SeqCst);
            Ok(MockServer::new(sample_session()).finish())
        };
        read_discovered_checked(&runtimes, &spawn, Some(Box::new(make_scratch)), Duration::from_secs(5))
    };
    let first = kept_catalog(&cache, read);
    let ids: Vec<&str> = first.iter().filter_map(|m| m["id"].as_str()).collect();
    assert_eq!(ids, vec!["account-default", "opencode/zen-free", "provider/model"]);
    let second = kept_catalog(&cache, read);
    assert_eq!(second, first);
    assert_eq!(discover_calls.load(Ordering::SeqCst), 1);
    assert_eq!(spawn_calls.load(Ordering::SeqCst), 1);
}

// case: starts independent catalog probes together instead of serialising their deadlines
#[test]
fn starts_independent_catalog_probes_together_instead_of_serialising_their_deadlines() {
    let (codex_started_tx, codex_started_rx) = mpsc::channel();
    let (acp_ready_tx, acp_ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Arc::new(Mutex::new(release_rx));

    let codex_rt = ProbeRuntime {
        id: "codex".into(),
        ..runtime(1)
    };
    let runtimes = vec![runtime(2), codex_rt];

    let spawn = |_exe: &str, args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        if args.iter().any(|a| a == "acp") {
            let acp_ready = acp_ready_tx.clone();
            let release = release_rx.clone();
            let mock = MockServer::new(sample_session()).with_handler(move |req, tx| {
                if req["method"] != "session/new" {
                    return false;
                }
                let _ = acp_ready.send(());
                let tx = tx.clone();
                let id = req["id"].clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    let _ = release.lock().unwrap().recv_timeout(Duration::from_secs(5));
                    let reply = format!(
                        "{}\n",
                        json!({"jsonrpc": "2.0", "id": id, "result": sample_session()})
                    );
                    let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
                });
                true
            });
            Ok(mock.finish())
        } else {
            let _ = codex_started_tx.send(());
            Ok(MockServer::new(json!({"data": [{"id": "codex-model", "displayName": "Codex model"}]})).finish())
        }
    };

    let (listed, incomplete) = std::thread::scope(|s| {
        let probe = s.spawn(|| {
            read_discovered_checked(&runtimes, &spawn, Some(Box::new(make_scratch)), Duration::from_secs(5))
        });
        // Both probes must start concurrently: Codex starts while OpenCode's session/new is held.
        let codex_ok = codex_started_rx.recv_timeout(Duration::from_secs(3)).is_ok();
        let acp_ok = acp_ready_rx.recv_timeout(Duration::from_secs(3)).is_ok();
        let _ = release_tx.send(());
        let res = probe.join().unwrap();
        assert!(codex_ok, "Codex app-server probe must start while OpenCode ACP is still pending");
        assert!(acp_ok, "OpenCode ACP session/new must start concurrently");
        res
    });

    assert!(!incomplete);
    let models = crate::models::catalog(&listed);
    let ids: Vec<&str> = models.iter().filter_map(|m| m["id"].as_str()).collect();
    assert!(ids.contains(&"codex-model"), "{ids:?}");
    assert!(ids.contains(&"opencode/zen-free"), "{ids:?}");
}

// case: keeps V1 discovery-only without opening any ACP process
#[test]
fn keeps_v1_discovery_only_without_opening_any_acp_process() {
    let spawn_calls = AtomicUsize::new(0);
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        spawn_calls.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("V1 must not open ACP"))
    };
    let (listed, incomplete) =
        read_discovered_checked(&[runtime(1)], &spawn, None, Duration::from_secs(5));
    assert!(!incomplete);
    let models = crate::models::catalog(&listed);
    let ids: Vec<&str> = models.iter().filter_map(|m| m["id"].as_str()).collect();
    assert_eq!(ids, vec!["account-default", "old/stale-free"]);
    assert_eq!(spawn_calls.load(Ordering::SeqCst), 0);
}

// case: retries a failed ACP catalog rather than caching a partial list or falling back to stale hints
#[test]
fn retries_a_failed_acp_catalog_rather_than_caching_a_partial_list_or_falling_back_to_stale_hints() {
    let cache = Mutex::new(None);
    let attempts = Arc::new(AtomicUsize::new(0));
    let codex_rt = ProbeRuntime {
        id: "codex".into(),
        ..runtime(1)
    };
    let read = || {
        let attempts = attempts.clone();
        let spawn = move |_exe: &str, args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
            if !args.iter().any(|a| a == "acp") {
                return Ok(MockServer::new(json!({"data": [{"id": "codex-model", "displayName": "Codex"}]})).finish());
            }
            let attempts = attempts.clone();
            let mock = MockServer::new(sample_session()).with_handler(move |req, tx| {
                if req["method"] != "session/new" {
                    return false;
                }
                let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                let line = if n == 1 {
                    format!("{}\n", json!({"jsonrpc": "2.0", "id": req["id"], "error": {"code": -32000}}))
                } else {
                    format!("{}\n", json!({"jsonrpc": "2.0", "id": req["id"], "result": sample_session()}))
                };
                let _ = tx.send(ProbeEvent::Data(line.into_bytes()));
                true
            });
            Ok(mock.finish())
        };
        read_discovered_checked(
            &[runtime(2), codex_rt.clone()],
            &spawn,
            Some(Box::new(make_scratch)),
            Duration::from_secs(5),
        )
    };

    // First attempt: OpenCode V2 fails while Codex succeeds -> partial list is NOT cached and never uses stale V1 hints.
    let first = kept_catalog(&cache, read);
    assert!(!first.iter().any(|m| m["id"] == "old/stale-free"));
    assert!(!first.iter().any(|m| m["id"] == "opencode/zen-free"));
    // Second attempt retries because the partial catalog was marked incomplete.
    let recovered = kept_catalog(&cache, read);
    let ids: Vec<&str> = recovered.iter().filter_map(|m| m["id"].as_str()).collect();
    assert!(!ids.contains(&"old/stale-free"));
    assert!(ids.contains(&"opencode/zen-free"));
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

// case: stops immediately when %s arrives while a request is pending
#[test]
fn stops_immediately_when_exit_or_close_arrives_while_a_request_is_pending() {
    for is_exit in [true, false] {
        let mock = MockServer::new(sample_session()).with_handler(move |_req, tx| {
            let _ = tx.send(if is_exit {
                ProbeEvent::Exited
            } else {
                ProbeEvent::StdoutClosed
            });
            true
        });
        let messages = mock.messages.clone();
        let kills = mock.kills.clone();
        let slot = Mutex::new(Some(mock.finish()));
        let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
            Ok(slot.lock().unwrap().take().unwrap())
        };
        let started = Instant::now();
        let models = read_opencode_acp_models(
            &runtime(2),
            OpenCodeAcpModelsOptions {
                spawn: &spawn,
                scratch: Some(Box::new(make_scratch)),
                runtime: "opencode",
                deadline: Duration::from_secs(10),
            },
        );
        assert!(started.elapsed() < Duration::from_secs(2), "must stop immediately, not wait for deadline");
        assert!(models.is_empty());
        assert_eq!(messages.lock().unwrap().len(), 1);
        assert_eq!(kills.load(Ordering::SeqCst), if is_exit { 0 } else { 1 });
    }
}

// case: accepts a complete session reply in the final output chunk before stdout closes
#[test]
fn accepts_a_complete_session_reply_in_the_final_output_chunk_before_stdout_closes() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        if req["method"] != "session/new" {
            return false;
        }
        let reply = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": req["id"], "result": sample_session()})
        );
        let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
        let _ = tx.send(ProbeEvent::StdoutClosed);
        true
    });
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert_eq!(models.len(), 2);
}

// case: bounds the entire handshake and session read with one deadline
#[test]
fn bounds_the_entire_handshake_and_session_read_with_one_deadline() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        if req["method"] == "initialize" {
            let tx = tx.clone();
            let id = req["id"].clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(120));
                let reply = format!(
                    "{}\n",
                    json!({"jsonrpc": "2.0", "id": id, "result": {"protocolVersion": 1}})
                );
                let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
            });
        }
        true
    });
    let messages = mock.messages.clone();
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let started = Instant::now();
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_millis(200),
        },
    );
    let elapsed = started.elapsed();
    assert!(models.is_empty());
    let methods: Vec<String> = messages
        .lock()
        .unwrap()
        .iter()
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(methods, vec!["initialize", "session/new"]);
    assert!(elapsed < Duration::from_millis(310), "elapsed {elapsed:?} exceeded shared deadline");
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: refuses an unbounded output stream and stops its process
#[test]
fn refuses_an_unbounded_output_stream_and_stops_its_process() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        if req["method"] != "session/new" {
            return false;
        }
        let _ = tx.send(ProbeEvent::Data(vec![b'x'; MAX_OUTPUT + 1]));
        let reply = format!(
            "\n{}\n",
            json!({"jsonrpc": "2.0", "id": req["id"], "result": sample_session()})
        );
        let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
        true
    });
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let started = Instant::now();
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(10),
        },
    );
    assert!(started.elapsed() < Duration::from_secs(3), "must abort immediately when output exceeds cap");
    assert!(models.is_empty());
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: ignores malformed JSON and accepts split frames without corrupting the catalog
#[test]
fn ignores_malformed_json_and_accepts_split_frames_without_corrupting_the_catalog() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        if req["method"] != "session/new" {
            return false;
        }
        let line = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": req["id"], "result": sample_session()})
        );
        let bytes = line.into_bytes();
        let _ = tx.send(ProbeEvent::Data(b"not-json\n".to_vec()));
        let _ = tx.send(ProbeEvent::Data(bytes[..17].to_vec()));
        let _ = tx.send(ProbeEvent::Data(bytes[17..].to_vec()));
        true
    });
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert_eq!(models.len(), 2);
}

// case: cleans up its scratch directory when process launch fails
#[test]
fn cleans_up_its_scratch_directory_when_process_launch_fails() {
    let dir_holder: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    let dir_out = dir_holder.clone();
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Err(io::Error::new(io::ErrorKind::NotFound, "missing binary"))
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(move || {
                let d = make_scratch()?;
                *dir_out.lock().unwrap() = Some(d.clone());
                Ok(d)
            })),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert!(models.is_empty());
    let dir = dir_holder.lock().unwrap().clone().expect("scratch created");
    assert!(!dir.exists());
}

// case: does not open a session on a different ACP protocol version
#[test]
fn does_not_open_a_session_on_a_different_acp_protocol_version() {
    let mock = MockServer::new(sample_session()).with_handler(|req, tx| {
        let reply = format!(
            "{}\n",
            json!({"jsonrpc": "2.0", "id": req["id"], "result": {"protocolVersion": 99}})
        );
        let _ = tx.send(ProbeEvent::Data(reply.into_bytes()));
        true
    });
    let messages = mock.messages.clone();
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert!(models.is_empty());
    let methods: Vec<String> = messages
        .lock()
        .unwrap()
        .iter()
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(methods, vec!["initialize"]);
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: contains failed writes without leaving a pending catalog read
#[test]
fn contains_failed_writes_without_leaving_a_pending_catalog_read() {
    let mock = MockServer::new(sample_session()).with_fail_write(|_| true);
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert!(models.is_empty());
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: does not invent models when a session has no model option
#[test]
fn does_not_invent_models_when_a_session_has_no_model_option() {
    let no_models = json!({
        "sessionId": "no-models",
        "configOptions": [{"id": "mode", "options": [{"value": "build", "name": "Build"}]}]
    });
    let mock = MockServer::new(no_models);
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert!(models.is_empty());
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: cannot launch late when scratch creation outlives the total deadline
#[test]
fn cannot_launch_late_when_scratch_creation_outlives_the_total_deadline() {
    let dir = make_scratch().unwrap();
    let dir_for_scratch = dir.clone();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let spawn_calls = AtomicUsize::new(0);
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        spawn_calls.fetch_add(1, Ordering::SeqCst);
        Ok(MockServer::new(sample_session()).finish())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(move || {
                let _ = release_rx.recv();
                Ok(dir_for_scratch)
            })),
            runtime: "opencode",
            deadline: Duration::from_millis(80),
        },
    );
    assert!(models.is_empty());
    let _ = release_tx.send(());
    let wait_until = Instant::now() + Duration::from_secs(2);
    while dir.exists() && Instant::now() < wait_until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!dir.exists(), "late scratch directory must be cleaned up");
    assert_eq!(spawn_calls.load(Ordering::SeqCst), 0);
}

// case: contains a failed permission-refusal write and stops the process
#[test]
fn contains_a_failed_permission_refusal_write_and_stops_the_process() {
    let mock = MockServer::new(sample_session())
        .with_handler(|req, tx| {
            if req["method"] != "session/new" {
                return false;
            }
            let ask = format!(
                "{}\n",
                json!({"jsonrpc": "2.0", "id": "ask", "method": "fs/read_text_file"})
            );
            let _ = tx.send(ProbeEvent::Data(ask.into_bytes()));
            true
        })
        .with_fail_write(|req| req["id"] == "ask");
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert!(models.is_empty());
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}

// case: keeps a valid advertisement even when terminating its process throws
#[test]
fn keeps_a_valid_advertisement_even_when_terminating_its_process_throws() {
    let mock = MockServer::new(sample_session()).with_fail_kill();
    let kills = mock.kills.clone();
    let slot = Mutex::new(Some(mock.finish()));
    let spawn = |_exe: &str, _args: &[String], _env: &BTreeMap<String, String>, _cwd: &Path| {
        Ok(slot.lock().unwrap().take().unwrap())
    };
    let models = read_opencode_acp_models(
        &runtime(2),
        OpenCodeAcpModelsOptions {
            spawn: &spawn,
            scratch: Some(Box::new(make_scratch)),
            runtime: "opencode",
            deadline: Duration::from_secs(5),
        },
    );
    assert_eq!(models.len(), 2);
    assert_eq!(kills.load(Ordering::SeqCst), 1);
}
