//! Headless --run entry point; no window is constructed.
use crate::{
    ledger_write::LedgerWriter,
    runtime::{
        Runtime, Turn, antigravity::Antigravity, claude::Claude, codex::Codex, copilot::Copilot,
        cursor::Cursor, muse::Muse,
    },
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
    if args.len() < 2
        || !matches!(
            runtime_name,
            "codex"
                | "claude"
                | "acp"
                | "opencode"
                | "kilo"
                | "cursor"
                | "copilot"
                | "antigravity"
                | "muse"
        )
    {
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
            "--model" => {
                let model = v.trim();
                turn.model = (!model.is_empty() && model != "account-default").then(|| model.to_owned());
            }
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 2;
    }
    // Like Electron's CLI door, check the machine policy before probing any executable or writing a mission.
    let policy = crate::org_policy::current();
    check_start(&policy, runtime_name, &turn)?;
    turn.folder = turn.folder.canonicalize()?;
    if !turn.folder.is_dir() {
        return Err("--folder must name a directory".into());
    }
    let store = Store::local()?;
    check_spend(&policy, &store)?;
    let id = format!(
        "mission_native_{}_{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    );
    ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::Relaxed))?;
    match runtime_name {
        "codex" => {
            let mut command = crate::runtime::codex::command()?;
            command.current_dir(&turn.folder);
            let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let mut writer = LedgerWriter::create_runtime(
                &store,
                &id,
                &turn.prompt,
                turn.model.as_deref().unwrap_or("account-default"),
                &command.get_program().to_string_lossy(),
                "codex",
                &args,
                None,
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
            let mut command = crate::runtime::acp::command(
                store.electron(),
                &turn.folder,
                turn.model.as_deref(),
            )?;
            let require_model = check_acp_selection(&policy, &command)?;
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
                "acp",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::acp::Acp::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            if require_model {
                runtime.set_profile(crate::runtime::acp::Profile {
                    strict: true,
                    ..Default::default()
                });
            }
            drive(&mut runtime, &mut writer, &turn)
        }
        "opencode" => {
            let mut command = crate::runtime::opencode::command(&turn.folder, turn.mode, &store)?;
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
                "opencode",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::opencode::OpenCode::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "kilo" => {
            let (home, _) = crate::runtime::kilo::conversation_home(&store, &id)?;
            let mut command = crate::runtime::kilo::command(&turn.folder, &store, &home)?;
            let args: Vec<String> = command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let model = crate::runtime::kilo::model_for_run(turn.model.as_deref());
            let mut writer = LedgerWriter::create_runtime(
                &store,
                &id,
                &turn.prompt,
                &model,
                &command.get_program().to_string_lossy(),
                "kilo",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match crate::runtime::kilo::Kilo::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "cursor" => {
            let mut command = crate::runtime::cursor::command(
                &turn.folder,
                turn.mode,
                turn.model.as_deref(),
                turn.resume.as_deref(),
                None,
            )?;
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
                "cursor",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Cursor::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "copilot" => {
            let mut command = crate::runtime::copilot::command(
                &turn.folder,
                &turn.prompt,
                turn.mode,
                turn.model.as_deref(),
                turn.effort.as_deref(),
                turn.resume.as_deref(),
                Some(&id),
                &[],
                None,
            )?;
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
                "copilot",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Copilot::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "antigravity" => {
            let mut command = crate::runtime::antigravity::command(
                &turn.folder,
                turn.mode,
                turn.model.as_deref(),
                turn.effort.as_deref(),
                turn.resume.as_deref(),
                &[],
                None,
            )?;
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
                "antigravity",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Antigravity::spawn(&mut command) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        "muse" => {
            let mut command = crate::runtime::muse::command(
                &turn.folder,
                turn.mode,
                turn.model.as_deref(),
                turn.effort.as_deref(),
                turn.resume.as_deref(),
            )?;
            let prompt_file =
                crate::runtime::muse::prepare_prompt_file(&mut command, &turn.prompt, &store)?;
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
                "muse",
                &args,
                None,
            )?;
            eprintln!("Ledger: {}", writer.path.display());
            let mut runtime = match Muse::spawn_with_prompt_file(&mut command, Some(prompt_file)) {
                Ok(r) => r,
                Err(e) => {
                    record_start_failure(&mut writer, &e)?;
                    return Err(e.into());
                }
            };
            drive(&mut runtime, &mut writer, &turn)
        }
        _ => unreachable!(),
    }
}

/// Unlike a normal mission, this CLI launches its runtime directly. Judge the model that the driver
/// will ask for. A default or an OpenCode id that the legacy driver would ignore is not a named model.
fn check_start(
    policy: &crate::org_policy::PolicyState,
    runtime: &str,
    turn: &Turn,
) -> Result<(), String> {
    if let Some(why) = crate::org_policy::feature_refusal(Some(policy), "otherApps") {
        return Err(why);
    }
    let asked = turn.model.as_deref();
    let model = match runtime {
        "kilo" => Some(crate::runtime::kilo::model_for_run(asked)),
        "antigravity" => crate::runtime::antigravity::agy_route(asked, turn.effort.as_deref()).0,
        "opencode" => asked.filter(|id| id.split_once('/').is_some_and(|(provider, model)| {
            !provider.is_empty() && !model.is_empty()
        })).map(str::to_owned),
        _ => asked.map(str::to_owned),
    };
    // --run is local, starts in Ask, and has no saved-account selector.
    crate::org_policy::turn_refusal(Some(policy), runtime, model.as_deref(), turn.mode.as_str(), false, false)
        .map_or(Ok(()), Err)?;
    // A saved ACP command need not expose model selection. Its label alone cannot establish
    // what will be billed, so the standalone CLI does not run it in a free-only drive.
    if crate::model_lists::free_routes_only_now()
        && (runtime == "acp" || !crate::scout::host::is_free_route(runtime, model.as_deref()))
    {
        return Err(crate::model_lists::FREE_ONLY_REFUSAL.into());
    }
    Ok(())
}

/// A saved ACP route is not a model selection unless the configured agent declared a selector.
fn check_acp_selection(
    policy: &crate::org_policy::PolicyState,
    command: &std::process::Command,
) -> Result<bool, String> {
    if crate::org_policy::model_allowed(Some(policy), "acp", None) {
        return Ok(false);
    }
    let has = |key| command.get_envs().any(|(name, value)| name == key && value.is_some());
    if has("LOCUST_NATIVE_ACP_MODEL_OPTION") && has("LOCUST_NATIVE_ACP_MODEL_VALUE") {
        // With a finite list, require the live session to acknowledge the value before the prompt.
        return Ok(true);
    }
    Err(crate::org_policy::policy_refusal(
        policy,
        "allowedModels",
        "The acp model account default is not allowed. Pick an allowed, explicit model.",
    ))
}

/// An unowned CLI run counts the whole month's receipts, just as an unowned Electron start does.
/// The headless driver writes its own events into that same native ledger.
fn check_spend(policy: &crate::org_policy::PolicyState, store: &Store) -> Result<(), String> {
    let Some(limit) = crate::org_policy::monthly_limit(Some(policy), None) else {
        return Ok(());
    };
    let unreadable = || crate::org_policy::policy_refusal(policy, "monthlyLimitUsd",
        "The monthly spending limit could not be checked. Ask your administrator to restore access to the spending record.");
    let month = crate::spend::month_of(&chrono::Local::now().to_rfc3339()).ok_or_else(unreadable)?;
    let turns = crate::usage_ledger::usage_turns_readable(store).map_err(|_| unreadable())?;
    let spent = crate::spend::spent_by(&turns, &month, |_| true);
    if crate::spend::limit_reached(Some(&spent), limit) {
        return Err(crate::org_policy::policy_refusal(policy, "monthlyLimitUsd",
            &format!("Locust has reached this month's effective limit of {limit:.2} USD. It starts again next month.")));
    }
    Ok(())
}

fn drive<R: Runtime>(
    runtime: &mut R,
    writer: &mut LedgerWriter,
    turn: &Turn,
) -> Result<i32, Box<dyn std::error::Error>> {
    let store = Store::local()?;
    let checkpoint = crate::checkpoints::Turn::begin(&store, &turn.folder);
    let result = drive_checkpointed(runtime, writer, turn);
    if let Err(error) = checkpoint.finish(&store, &turn.mission_id) {
        eprintln!("Undo checkpoint was not kept: {error}");
    }
    result
}

fn drive_checkpointed<R: Runtime>(
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

#[cfg(all(test, unix))]
#[path = "run_cli_policy_tests.rs"]
mod policy_tests;

#[cfg(test)]
#[path = "run_cli_free_route_tests.rs"]
mod free_route_tests;
