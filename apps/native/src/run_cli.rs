//! Headless --run entry point; no window is constructed.
use crate::{
    ledger_write::LedgerWriter,
    runtime::{Runtime, Turn, codex::Codex},
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
    if args.len() < 2 || args[0] != "codex" {
        return Err(
            "Usage: locust-native --run codex <prompt> [--folder <dir>] [--model <id>]".into(),
        );
    }
    let mut turn = Turn {
        prompt: args[1].clone(),
        folder: std::env::current_dir()?,
        model: None,
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
    let mut command = crate::runtime::codex::command()?;
    command.current_dir(&turn.folder);
    let id = format!(
        "mission_native_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let mut writer = LedgerWriter::create(
        &store,
        &id,
        &turn.prompt,
        turn.model.as_deref().unwrap_or("account-default"),
        &command.get_program().to_string_lossy(),
    )?;
    eprintln!("Ledger: {}", writer.path.display());
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::Relaxed))?;
    let mut runtime = match Codex::spawn(&mut command) {
        Ok(r) => r,
        Err(e) => {
            record_start_failure(&mut writer, &e)?;
            return Err(e.into());
        }
    };
    if let Err(e) = runtime.start(&turn) {
        if interrupted() {
            let _ = runtime.stop();
        }
        record_start_failure(&mut writer, &e)?;
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
