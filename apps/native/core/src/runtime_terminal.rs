//! Runtime escape hatch on macOS, and read-only transcript parsing.
use serde_json::Value;
use std::path::Path;
pub fn resume_args(runtime: &str, id: &str) -> Option<Vec<String>> {
    if id.is_empty()
        || id.len() > 200
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
    {
        return None;
    }
    Some(match runtime {
        "codex" | "muse" => vec!["resume".into(), id.into()],
        "claude" => vec!["--resume".into(), id.into()],
        "copilot" | "cursor" => vec![format!("--resume={id}")],
        "opencode" => vec!["--session".into(), id.into()],
        "antigravity" => vec!["--conversation".into(), id.into()],
        _ => return None,
    })
}
fn quote(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\"'\"'"))
}
pub fn mac_script(file: &Path, args: &[String], folder: &Path) -> String {
    format!(
        "#!/bin/zsh -l\nrm -f -- \"$0\"\ncd {} || exit 1\nexec {}\n",
        quote(&folder.to_string_lossy()),
        std::iter::once(file.to_string_lossy().into_owned())
            .chain(args.iter().cloned())
            .map(|s| quote(&s))
            .collect::<Vec<_>>()
            .join(" ")
    )
}
pub fn open(
    file: &Path,
    args: &[String],
    folder: &Path,
    script: &Path,
    store: &crate::store::Store,
) -> Result<(), String> {
    crate::terminal::check_policy()?;
    if !cfg!(target_os = "macos") {
        return Err(
            "Opening a conversation in a terminal works on Windows, macOS and Linux.".into(),
        );
    }
    use std::io::Write;
    let mut f = store
        .create_new_file_in(
            script.parent().ok_or("Terminal script has no folder")?,
            script
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("Invalid terminal script name")?,
        )
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    f.write_all(mac_script(file, args, folder).as_bytes())
        .map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new("/usr/bin/open");
    cmd.args(["-a", "Terminal"]).arg(script);
    crate::runtime::process::own_group(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Reap only the opener: the external terminal belongs to the person.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
#[derive(Debug, Default, PartialEq)]
pub struct Exchange {
    pub prompt: String,
    pub answer: Option<String>,
    pub started: String,
    pub finished: String,
    pub model: Option<String>,
    pub lines: usize,
    over: bool,
    command: bool,
}
fn words(v: &Value) -> String {
    v.as_str().map(str::to_owned).unwrap_or_else(|| {
        v.as_array()
            .into_iter()
            .flatten()
            .filter(|b| {
                b["type"] == "text" || b["type"] == "input_text" || b["type"] == "output_text"
            })
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
fn typed(s: &str) -> bool {
    !s.trim().is_empty()
        && ![
            "<",
            "[Request interrupted by user",
            "# AGENTS.md instructions",
        ]
        .iter()
        .any(|p| s.trim().starts_with(p))
}
fn close(open: &mut Option<Exchange>, out: &mut Vec<Exchange>) {
    if let Some(e) = open.take()
        && e.over
        && (!e.command || e.answer.is_some())
    {
        out.push(e);
    }
}
pub fn exchanges(runtime: &str, text: &str, after: &str) -> Vec<Exchange> {
    let mut out = vec![];
    let mut open: Option<Exchange> = None;
    let mut model = None;
    for line in text.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(at) = e["timestamp"].as_str() else {
            continue;
        };
        if runtime == "claude" {
            if e["isSidechain"] == true {
                continue;
            }
            let m = &e["message"];
            if e["type"] == "user" {
                let content = &m["content"];
                if content
                    .as_array()
                    .is_some_and(|a| a.iter().any(|b| b["type"] == "tool_result"))
                {
                    if let Some(o) = &mut open {
                        o.lines += 1;
                    }
                    continue;
                }
                let said = words(content);
                if said.trim().starts_with("[Request interrupted by user") {
                    if let Some(o) = &mut open {
                        o.over = true;
                    }
                    continue;
                }
                if ["isMeta", "isCompactSummary", "isVisibleInTranscriptOnly"]
                    .iter()
                    .any(|k| e[k] == true)
                    || e["promptSource"] == "system"
                {
                    continue;
                }
                let tag = |name: &str| -> Option<String> {
                    let a = format!("<{name}>");
                    let b = format!("</{name}>");
                    Some(said.split_once(&a)?.1.split_once(&b)?.0.trim().to_owned())
                };
                let command = tag("command-name").filter(|s| !s.is_empty());
                let prompt = if let Some(c) = &command {
                    format!("{c} {}", tag("command-args").unwrap_or_default())
                        .trim()
                        .to_owned()
                } else {
                    said.trim().into()
                };
                if command.is_none() && !typed(&prompt) {
                    continue;
                }
                if let Some(o) = &mut open {
                    o.over = true;
                }
                close(&mut open, &mut out);
                if e["entrypoint"] == "cli" && at > after {
                    open = Some(Exchange {
                        prompt,
                        started: at.into(),
                        finished: at.into(),
                        lines: 1,
                        command: command.is_some(),
                        ..Exchange::default()
                    });
                }
                continue;
            }
            if let Some(o) = &mut open {
                if e["type"] == "system" && e["subtype"] == "turn_duration" {
                    o.over = true;
                }
                if e["type"] == "assistant" {
                    o.lines += 1;
                    o.finished = at.into();
                    let said = words(&m["content"]);
                    if !said.trim().is_empty() {
                        o.answer = Some(said.trim().into());
                    }
                    if let Some(s) = m["model"].as_str() {
                        o.model = Some(s.into());
                    }
                    if let Some(s) = m["stop_reason"].as_str() {
                        o.over = s != "tool_use";
                    }
                }
            }
        } else if runtime == "codex" {
            if at <= after {
                continue;
            }
            let p = &e["payload"];
            if e["type"] == "turn_context"
                && let Some(m) = p["model"].as_str()
            {
                model = Some(m.to_owned());
            }
            if e["type"] == "event_msg"
                && ["task_complete", "turn_aborted"]
                    .iter()
                    .any(|s| p["type"] == *s)
                && let Some(o) = &mut open
            {
                o.over = true;
            }
            if e["type"] != "response_item" || p["type"] != "message" {
                if let Some(o) = &mut open {
                    o.lines += 1;
                }
                continue;
            }
            let said = words(&p["content"]);
            if p["role"] == "user" && typed(&said) {
                if let Some(o) = &mut open {
                    o.over = true;
                }
                close(&mut open, &mut out);
                open = Some(Exchange {
                    prompt: said.trim().into(),
                    started: at.into(),
                    finished: at.into(),
                    model: model.clone(),
                    lines: 1,
                    ..Exchange::default()
                });
            }
            if p["role"] == "assistant"
                && let Some(o) = &mut open
            {
                o.lines += 1;
                o.finished = at.into();
                if !said.trim().is_empty() {
                    o.answer = Some(said.trim().into());
                }
            }
        }
    }
    close(&mut open, &mut out);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn resume_tokens_and_mac_shell_quoting() {
        assert_eq!(
            resume_args("codex", "ses_1").unwrap(),
            vec!["resume", "ses_1"]
        );
        for s in ["-bad", "x;touch x", "a\nb", "", "$(id)"] {
            assert!(resume_args("claude", s).is_none());
        }
        let script = mac_script(
            Path::new("/a'b/tool"),
            &["resume".into(), "abc".into()],
            Path::new("/a b"),
        );
        assert!(script.contains("cd '/a b' || exit 1"));
        assert!(script.contains("'/a'\"'\"'b/tool'"));
        assert!(script.contains("rm -f -- \"$0\""));
        // Metacharacters and substitutions stay inside single quotes — not shell words.
        let hostile = mac_script(
            Path::new("/bin/echo"),
            &["$(touch /tmp/p9-inject)".into(), "; reboot".into()],
            Path::new("/tmp/a; rm -rf /"),
        );
        assert!(hostile.contains("cd '/tmp/a; rm -rf /' || exit 1"));
        assert!(hostile.contains("'$(touch /tmp/p9-inject)'"));
        assert!(hostile.contains("'; reboot'"));
        assert!(hostile.starts_with("#!/bin/zsh -l\n"));
        assert!(!hostile.contains("exec /bin/echo $(touch"));
    }
    #[test]
    fn codex_holds_partial_turn_and_ignores_injection() {
        let rows = [
            json!({"timestamp":"2","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<context>"}]}}),
            json!({"timestamp":"3","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}),
            json!({"timestamp":"4","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"world"}]}}),
        ];
        let mut text = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(exchanges("codex", &text, "1").is_empty());
        text.push_str("\n{\"timestamp\":\"5\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\"}}");
        let got = exchanges("codex", &text, "1");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].prompt, "hello");
        assert_eq!(got[0].answer.as_deref(), Some("world"));
        assert!(exchanges("codex", &text, "5").is_empty());
    }
    #[test]
    fn claude_cli_only_and_complete_answers() {
        let user = |at: &str, source: &str| json!({"timestamp":at,"type":"user","entrypoint":source,"message":{"content":"hello"}});
        let answer = |at: &str, stop: &str| json!({"timestamp":at,"type":"assistant","message":{"content":[{"type":"text","text":"world"}],"stop_reason":stop}});
        let rows = [
            user("2", "cli"),
            answer("3", "tool_use"),
            answer("4", "end_turn"),
            user("5", "sdk-cli"),
            answer("6", "end_turn"),
        ];
        let text = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let got = exchanges("claude", &text, "1");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].finished, "4");
        assert_eq!(got[0].answer.as_deref(), Some("world"));
    }
}

pub fn mission(id: &str) -> Result<(Value, Vec<Value>), String> {
    let path = crate::native::find_ledger(id).ok_or("That conversation is not in the ledger.")?;
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(64 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let rows: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let metadata = rows
        .first()
        .ok_or("That conversation is not in the ledger.")?["metadata"]
        .clone();
    Ok((metadata, rows))
}
pub fn session(rows: &[Value]) -> Option<String> {
    rows.iter().rev().find_map(|r| {
        r["event"]["runtimeThreadId"]
            .as_str()
            .or_else(|| r["event"]["payload"]["runtimeThreadId"].as_str())
            .map(str::to_owned)
    })
}
fn transcript(runtime: &str, id: &str) -> Option<std::path::PathBuf> {
    if !(8..=80).contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    fn find(root: &Path, suffix: &str, depth: usize) -> Option<std::path::PathBuf> {
        if depth == 0 {
            return None;
        }
        let mut entries = std::fs::read_dir(root)
            .ok()?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
        for e in entries {
            let ty = e.file_type().ok()?;
            if ty.is_symlink() {
                continue;
            }
            if ty.is_file() && e.file_name().to_string_lossy().ends_with(suffix) {
                return Some(e.path());
            }
            if ty.is_dir()
                && let Some(p) = find(&e.path(), suffix, depth - 1)
            {
                return Some(p);
            }
        }
        None
    }
    match runtime {
        "claude" => find(
            &home.join(".claude/projects"),
            &format!("/{id}.jsonl").replace('/', ""),
            2,
        ),
        "codex" => {
            let root = std::env::var_os("CODEX_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".codex"));
            find(&root.join("sessions"), &format!("-{id}.jsonl"), 4)
        }
        _ => None,
    }
}
/// Serialized by the UI worker. Newest-tip and the durable watermark prevent duplicate imports.
pub fn catch_up(anchor: &str) -> Result<usize, String> {
    let store = crate::native::store().ok_or("Native data folder is unavailable.")?;
    let newest = crate::native::real_tip(anchor)?;
    let (meta, rows) = mission(&newest)?;
    if !rows.iter().any(|r| {
        matches!(
            r["event"]["type"].as_str(),
            Some("run.completed" | "run.failed" | "run.cancelled")
        )
    }) {
        return Ok(0);
    }
    let Some(runtime) = meta["runtime"]
        .as_str()
        .filter(|s| ["claude", "codex"].contains(s))
    else {
        return Ok(0);
    };
    let Some(id) = session(&rows) else {
        return Ok(0);
    };
    let Some(path) = transcript(runtime, &id) else {
        return Ok(0);
    };
    let imports = store.seeded_json("terminal-imports.json", serde_json::json!({}))?;
    if !imports.is_object() {
        return Err("Terminal import history could not be read. Nothing was imported.".into());
    }
    let end = rows
        .last()
        .and_then(|r| r["occurredAt"].as_str())
        .or_else(|| meta["createdAt"].as_str())
        .unwrap_or("");
    let after = imports[&id]["through"].as_str().unwrap_or("").max(end);
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(64 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let exchanges = exchanges(runtime, &text, after);
    persist(&store, &meta, &id, newest, imports, exchanges)
}
fn persist(
    store: &crate::store::Store,
    meta: &Value,
    id: &str,
    newest: String,
    mut imports: Value,
    exchanges: Vec<Exchange>,
) -> Result<usize, String> {
    use std::io::Write;
    let runtime = meta["runtime"].as_str().ok_or("Invalid runtime")?;
    let count = exchanges.len();
    let mut previous = newest;
    for (index, e) in exchanges.into_iter().enumerate() {
        let mission = format!(
            "mission_{}",
            crate::native::uuid_v4().map_err(|e| e.to_string())?
        );
        let run = format!("run_{mission}");
        let mut m = meta.clone();
        m.as_object_mut()
            .ok_or("Invalid mission metadata")?
            .remove("command");
        m.as_object_mut()
            .ok_or("Invalid mission metadata")?
            .remove("mode");
        let bound = |s: &str, max: usize| {
            if s.chars().count() > max {
                format!("{}…", s.chars().take(max - 1).collect::<String>())
            } else {
                s.to_owned()
            }
        };
        m["missionId"] = mission.clone().into();
        m["runId"] = run.clone().into();
        m["prompt"] = bound(&e.prompt, 8000).into();
        m["createdAt"] = e.started.clone().into();
        m["continuesFrom"] = serde_json::json!({"missionId":previous,"checkpointEpoch":1,"reason":"follow-up","runtimeThreadId":id});
        m["startedBy"] = serde_json::json!({"kind":"terminal","exchange":index+1});
        let mut records = vec![
            serde_json::json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":e.started,"metadata":m}),
        ];
        let mut events = vec![(
            "run.started",
            e.started.clone(),
            serde_json::json!({"runtimeThreadId":id,"evidence":{"redacted":true,"runtimeEventType":"terminal.transcript"}}),
        )];
        if let Some(answer) = e.answer {
            events.push(("message.delta",e.finished.clone(),serde_json::json!({"itemId":"terminal_answer","operation":"replace","text":bound(&answer,16000),"final":true,"evidence":{"redacted":true,"runtimeEventType":"terminal.transcript"}})));
        }
        events.push(("run.completed",e.finished.clone(),serde_json::json!({"runtimeThreadId":id,"resolvedModel":e.model,"process":{"exitCode":null,"signal":null,"stderr":"","stderrTruncated":false,"recordCount":e.lines,"inputDeliveryFailed":false,"outputLimitExceeded":false,"oversizedRecordsDropped":0,"forcedTerminationAttempted":false,"terminationUnconfirmed":false,"startedAt":e.started,"finishedAt":e.finished}})));
        for (i, (kind, at, payload)) in events.into_iter().enumerate() {
            records.push(serde_json::json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":i+2,"occurredAt":at,"event":{"id":format!("{run}:{}",i+1),"runId":run,"missionId":mission,"sequence":i+1,"occurredAt":at,"sourceAdapter":runtime,"requestedRouteId":meta["requestedRouteId"],"resolvedRouteId":meta["resolvedRouteId"],"runtimeThreadId":id,"type":kind,"payload":payload}}));
        }
        let dir = store.ledger_dir().map_err(|e| e.to_string())?;
        let pending = format!(".{mission}.tmp");
        let mut f = store
            .create_new_file_in(&dir, &pending)
            .map_err(|e| e.to_string())?;
        for record in records {
            writeln!(f, "{record}").map_err(|e| e.to_string())?;
        }
        f.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(
            dir.join(&pending),
            store.write_path(&mission).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        imports[id] = serde_json::json!({"through":e.finished});
        store.save_json("terminal-imports.json", &imports)?;
        previous = mission;
    }
    Ok(count)
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn imported_turn_is_atomic_native_only_and_reader_understands_it() {
        let root =
            std::env::temp_dir().join(format!("runtime-import-{}", crate::native::uuid_v4().unwrap()));
        let electron = root.join("electron");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::write(electron.join("sentinel"), "unchanged").unwrap();
        let store = crate::store::Store::new(root.join("native"), electron.clone()).unwrap();
        let meta = json!({"runtime":"codex","model":"synthetic","requestedRouteId":"codex:synthetic","resolvedRouteId":"codex:synthetic","sandbox":"read-only","workspaceId":"native","cliVersion":null,"executionPolicyVersion":1});
        let e = Exchange {
            prompt: "hello".into(),
            answer: Some("world".into()),
            started: "2026-11-10T00:00:00.000Z".into(),
            finished: "2026-11-10T00:00:01.000Z".into(),
            lines: 3,
            ..Exchange::default()
        };
        assert_eq!(
            persist(
                &store,
                &meta,
                "12345678",
                "mission_previous".into(),
                json!({}),
                vec![e]
            )
            .unwrap(),
            1
        );
        let paths = std::fs::read_dir(store.ledger_dir().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        let parsed = crate::ledger::read_transcript(&paths[0]);
        assert!(
            parsed
                .items
                .iter()
                .any(|i| matches!(i,crate::ledger::Item::Assistant{text,..} if text=="world"))
        );
        let body = std::fs::read_to_string(&paths[0]).unwrap();
        let first: Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
        assert_eq!(first["metadata"]["startedBy"]["kind"], "terminal");
        assert!(first["metadata"].get("command").is_none());
        assert!(first["metadata"].get("mode").is_none());
        assert_eq!(
            std::fs::read_to_string(electron.join("sentinel")).unwrap(),
            "unchanged"
        );
        assert_eq!(std::fs::read_dir(&electron).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
