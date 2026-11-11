//! Claude's permission-prompt-tool route, with a private Unix socket instead of
//! Electron's token-authenticated HTTP hop. No Node/Electron dependency.
use crate::approval::Channel;
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::fd::AsRawFd,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub struct Bridge {
    dir: PathBuf,
    stopped: Arc<AtomicBool>,
    channel: Arc<Channel>,
    mission: String,
}
impl Bridge {
    pub fn prepare(
        command: &mut Command,
        channel: Arc<Channel>,
        mission: &str,
        run: &str,
        cwd: &str,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        // Short enough for the socket path limit even under an app's long TMPDIR (invariants::socket_dir).
        let dir = crate::invariants::socket_dir("lcp")?;
        let socket = dir.join("host");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let config = dir.join("mcp.json");
        let executable = bridge_executable()?;
        fs::write(&config, json!({"mcpServers":{"locust":{"command":executable,"args":["--permission-bridge",socket]}}}).to_string())?;
        command.args([
            "--mcp-config",
            config
                .to_str()
                .ok_or_else(|| io::Error::other("Permission config path is not UTF-8"))?,
            "--permission-prompt-tool",
            "mcp__locust__approve",
        ]);
        let stopped = Arc::new(AtomicBool::new(false));
        let end = stopped.clone();
        let ch = channel.clone();
        let mid = mission.to_owned();
        let rid = run.to_owned();
        let folder = cwd.to_owned();
        std::thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let ch = ch.clone();
                        let mid = mid.clone();
                        let rid = rid.clone();
                        let folder = folder.clone();
                        let end = end.clone();
                        let cancel = cancel.clone();
                        std::thread::spawn(move || {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                            let mut line = String::new();
                            let read = BufReader::new(&stream).take(262145).read_line(&mut line);
                            let answer = (|| {
                                read.ok()?;
                                if line.len() > 262144 { return None; }
                                let v: Value = serde_json::from_str(&line).ok()?;
                                let name = v["tool_name"].as_str()?;
                                let deadline = std::time::Instant::now() + Duration::from_secs(600);
                                let cancelled = || end.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline || peer_closed(&stream);
                                let answer = ch.ask_claude(&rid, &mid, v["tool_use_id"].as_str().unwrap_or("permission"), name, &v["input"], &folder, &cancelled).ok()?;
                                if cancelled() { None } else { Some(answer) }
                            })().unwrap_or_else(|| json!({"behavior":"deny","message":"Locust could not ask you: the run ended or the request was invalid."}));
                            let _ = writeln!(stream, "{answer}");
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            dir,
            stopped,
            channel,
            mission: mission.into(),
        })
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.channel.release(&self.mission);
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn bridge_executable() -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    // Unit tests run in libtest, whose main cannot serve MCP. Live ignored
    // tests use the adjacent release binary built by `cargo build --release`.
    #[cfg(any(test, feature = "test-hooks"))]
    let executable = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| io::Error::other("Missing release directory"))?
        .join("locust-native");
    Ok(executable)
}

fn peer_closed(stream: &UnixStream) -> bool {
    let mut byte = 0u8;
    let read = unsafe {
        libc::recv(
            stream.as_raw_fd(),
            (&mut byte as *mut u8).cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    read == 0 || (read < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
}

pub fn serve(socket: &str) -> io::Result<()> {
    for line in io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line?)?;
        if v.get("id").is_none() {
            continue;
        }
        let result = match v["method"].as_str().unwrap_or("") {
            "initialize" => {
                json!({"protocolVersion":v["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"locust","version":"1"}})
            }
            "tools/list" => {
                json!({"tools":[{"name":"approve","description":"Ask the person using Locust whether this tool call may proceed.","inputSchema":{"type":"object","properties":{"tool_name":{"type":"string"},"input":{"type":"object"},"tool_use_id":{"type":"string"}},"required":["tool_name"]}}]})
            }
            "tools/call" => {
                let answer = (|| -> io::Result<Value> {
                    if v["params"]["name"] != "approve" { return Err(io::Error::other("Unknown tool")); }
                    let mut stream = UnixStream::connect(socket)?;
                    stream.set_read_timeout(Some(Duration::from_secs(600)))?;
                    writeln!(stream, "{}", v["params"]["arguments"])?;
                    let mut answer = String::new(); BufReader::new(stream).take(262145).read_line(&mut answer)?;
                    Ok(serde_json::from_str(&answer)?)
                })().unwrap_or_else(|_| json!({"behavior":"deny","message":"Locust could not ask you: the window is not reachable."}));
                json!({"content":[{"type":"text","text":answer.to_string()}]})
            }
            _ => json!({}),
        };
        println!("{}", json!({"jsonrpc":"2.0","id":v["id"],"result":result}));
        io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::{Answer, Decision};
    #[test]
    fn permission_tool_routes_decisions_and_refuses_after_end() {
        let channel = Channel::new();
        let mut cmd = Command::new("claude");
        let bridge = Bridge::prepare(
            &mut cmd,
            channel.clone(),
            "bridge_mission",
            "run_bridge_mission",
            "/project",
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(cmd.get_args().any(|a| a == "mcp__locust__approve"));
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&fs::metadata(&bridge.dir).unwrap().permissions()) & 0o777,
            0o700
        );
        for decision in [Decision::Deny, Decision::ApproveOnce] {
            let mut stream = UnixStream::connect(bridge.dir.join("host")).unwrap();
            writeln!(stream, "{}", json!({"tool_name":"Write","input":{"file_path":"/project/hello.txt","content":"hello\n"}})).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while channel.waiting().is_empty() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            let request = channel.waiting()[0].clone();
            channel
                .decide(Answer::Decision {
                    approval_id: request.approval_id,
                    decision: decision.clone(),
                    reason: Some("No writes".into()),
                })
                .unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            let v: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                v["behavior"],
                if decision == Decision::Deny {
                    "deny"
                } else {
                    "allow"
                }
            );
            if decision == Decision::ApproveOnce {
                assert_eq!(v["updatedInput"]["content"], "hello\n");
            }
        }
        channel.release("bridge_mission");
        let mut stream = UnixStream::connect(bridge.dir.join("host")).unwrap();
        writeln!(stream, "{}", json!({"tool_name":"Write","input":{}})).unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&line).unwrap()["behavior"],
            "deny"
        );
        assert!(channel.waiting().is_empty());
        let dir = bridge.dir.clone();
        drop(bridge);
        assert!(!dir.exists());
    }
}
