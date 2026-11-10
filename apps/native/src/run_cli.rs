//! Headless --run entry point; no window is constructed.
use crate::{
    ledger_write::LedgerWriter,
    runtime::{Runtime, Turn, antigravity::Antigravity, claude::Claude, codex::Codex, copilot::Copilot, cursor::Cursor, muse::Muse},
    store::Store,
};
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::Relaxed)
}
pub fn run(args: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
    let runtime_name = args.first().map(String::as_str).unwrap_or("");
    if args.len() < 2 || !matches!(runtime_name, "codex" | "claude" | "acp" | "opencode" | "kilo" | "cursor" | "copilot" | "antigravity" | "muse") {
        return Err(
            "Usage: locust-native --run <codex|claude|acp|opencode|kilo|cursor|copilot|antigravity|muse> <prompt> [--folder <dir>] [--model <id>]"
                .into(),
        );
    }
    let mut turn = Turn {
        prompt: args[1].clone(),
        folder: std::env::current_dir()?,
        model: None,
        effort: None,
        resume: None,
        mode: crate::approval::Mode::Ask,
        approvals: None,
        mission_id: "synthetic".into(),
        run_id: "run_synthetic".into(),
    };
    let mut i = 2;
    while i < args.len() {
        let v = args.get(i + 1).ok_or("Missing option value")?;
        match args[i].as_str() {
            "--folder" => turn.folder = PathBuf::from(v),
            "--model" => turn.model = Some(v.clone()),
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 2;
    }
    turn.folder = turn.folder.canonicalize()?;
    if !turn.folder.is_dir() {
        return Err("--folder must name a directory".into());
    }
    let store = Store::local()?;
    let id = format!(
        "mission_native_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::Relaxed))?;
    match runtime_name {
        "codex" => {
            let mut command = crate::runtime::codex::command()?;
            command.current_dir(&turn.folder);
            let mut writer = LedgerWriter::create(
                &store,
                &id,
                &turn.prompt,
                turn.model.as_deref().unwrap_or("account-default"),
                &command.get_program().to_string_lossy(),
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Codex::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "claude" => {
            let mut command = crate::runtime::claude::command(&turn.folder, turn.model.as_deref())?;
            let args: Vec<String> = command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let mut writer = LedgerWriter::create_runtime(
                &store,
                &id,
                &turn.prompt,
                turn.model.as_deref().unwrap_or("account-default"),
                &command.get_program().to_string_lossy(),
                "claude",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Claude::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "acp" => {
            let mut command = crate::runtime::acp::command(store.electron(), &turn.folder, turn.model.as_deref())?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "acp", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::acp::Acp::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "opencode" => {
            let mut command = crate::runtime::opencode::command(&turn.folder, turn.mode, &store)?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "opencode", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::opencode::OpenCode::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "kilo" => {
            let mut command = crate::runtime::kilo::command(&turn.folder, &store)?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let model = crate::runtime::kilo::model_for_run(turn.model.as_deref());
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, &model, &command.get_program().to_string_lossy(), "kilo", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::kilo::Kilo::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "cursor" => {
            let mut command = crate::runtime::cursor::command(&turn.folder, turn.mode, turn.model.as_deref(), turn.resume.as_deref())?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "cursor", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Cursor::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "copilot" => {
            let mut command = crate::runtime::copilot::command(&turn.folder, &turn.prompt, turn.mode, turn.model.as_deref(), turn.effort.as_deref(), turn.resume.as_deref(), Some(&id))?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "copilot", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Copilot::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "antigravity" => {
            let mut command = crate::runtime::antigravity::command(&turn.folder, turn.mode, turn.model.as_deref(), turn.effort.as_deref(), turn.resume.as_deref())?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "antigravity", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Antigravity::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "muse" => {
            let mut command = crate::runtime::muse::command(&turn.folder, turn.mode, turn.model.as_deref(), turn.effort.as_deref(), turn.resume.as_deref())?;
            let prompt_file = crate::runtime::muse::prepare_prompt_file(&mut command, &turn.prompt, &turn.folder)?;
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(&store, &id, &turn.prompt, turn.model.as_deref().unwrap_or("account-default"), &command.get_program().to_string_lossy(), "muse", &args, None)?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Muse::spawn_with_prompt_file(&mut command, Some(prompt_file)) {
                Ok(r) => r,
                Err(e) => {record_start_failure(&mut writer, &e)?;return Err(e.into());}
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        _ => unreachable!(),
    }
}

fn drive<R: Runtime>(
    runtime: &mut R,
    writer: &mut LedgerWriter,
    turn: &Turn,
) -> Result<i32, Box<dyn std::error::Error>> {
    if let Err(e) = runtime.start(turn) {
        if interrupted() {
            let _ = runtime.stop();
        }
        record_start_failure(writer, &e)?;
        return Err(e.into());
    }
    let mut printed = std::collections::HashMap::<String, String>::new();
    loop {
        if interrupted() {
            runtime.stop()?;
        }
        let Some(event) = runtime.next_event()? else {
            continue;
        };
        writer.append(&event.kind, event.payload.clone(), event.thread.as_deref())?;
        if event.kind == "message.delta" {
            let id = event.payload["itemId"].as_str().unwrap_or("");
            let text = event.payload["text"].as_str().unwrap_or("");
            let held = printed.entry(id.into()).or_default();
            if event.payload["operation"] == "append" {
                print!("{text}");
                held.push_str(text);
            } else {
                if let Some(rest) = text.strip_prefix(held.as_str()) {
                    print!("{rest}");
                }
                *held = text.into();
            }
            io::stdout().flush()?;
        }
        match event.kind.as_str() {
            "run.completed" => {
                println!();
                return Ok(0);
            }
            "run.failed" | "run.cancelled" => {
                eprintln!(
                    "{}",
                    event.payload["message"].as_str().unwrap_or("Turn stopped")
                );
                return Ok(1);
            }
            "adapter.diagnostic" => {
                eprintln!("{}", event.payload["message"].as_str().unwrap_or(""))
            }
            _ => {}
        }
    }
}

fn record_start_failure(writer: &mut LedgerWriter, error: &io::Error) -> io::Result<()> {
    let mut normal = crate::runtime::events::Normalizer::default();
    let event = normal.finish(interrupted(), &error.to_string());
    writer.append(&event.kind, json!(event.payload), event.thread.as_deref())?;
    Ok(())
}
