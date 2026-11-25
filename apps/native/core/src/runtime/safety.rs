//! Runtime launch-argument safety guard (`assertSafeRuntimeCommand` in `packages/runtime-adapters/src/commands.ts`).
//!
//! Refuses permission-bypass, session-sharing, and out-of-workspace flags in every mode; unlocks the narrow
//! `full-access` flags (`--force`, `--allow-all-paths`, `--disable-sandbox`, `--sandbox danger-full-access`,
//! `--permission-mode bypassPermissions`, and Antigravity's `--dangerously-skip-permissions`) ONLY when the caller
//! explicitly passes `Some("full-access")`. An unstated sandbox (`None`) is judged by the strictest reading.

const FULL_ACCESS_ARGUMENTS: &[&str] = &["--force", "--allow-all-paths", "--disable-sandbox"];

const FORBIDDEN_ARGUMENTS: &[&str] = &[
    "--dangerously-bypass-approvals-and-sandbox",
    "--yolo",
    "--dangerously-bypass-hook-trust",
    "--ignore-rules",
    "--dangerously-skip-permissions",
    "--allow-dangerously-skip-permissions",
    "--full-auto",
    "--force",
    "-f",
    "--disable-sandbox",
    "-y",
    "--share",
    "--allow-all-paths",
    "--allow-all-urls",
    "--allow-all",
    "--remote",
    "--remote-export",
    "--enable-memory",
    "--mdns",
];

/// Validate runtime argv before spawning; returns `Err` on any forbidden or over-wide flag for `sandbox`.
pub fn assert_safe_runtime_command(runtime: &str, args: &[String], sandbox: Option<&str>) -> Result<(), String> {
    let full = sandbox == Some("full-access");
    for (index, argument) in args.iter().enumerate() {
        let arg = argument.as_str();
        let unlocked = full
            && (FULL_ACCESS_ARGUMENTS.contains(&arg)
                || (runtime == "antigravity" && arg == "--dangerously-skip-permissions"));
        if FORBIDDEN_ARGUMENTS.contains(&arg) && !unlocked {
            return Err(format!("Forbidden runtime argument: {arg}"));
        }
        if !unlocked
            && FORBIDDEN_ARGUMENTS
                .iter()
                .any(|forbidden| arg.starts_with(&format!("{forbidden}=")))
        {
            return Err(format!("Forbidden runtime argument: {arg}"));
        }
        if arg == "--sandbox=danger-full-access" && !full {
            return Err("Forbidden Codex sandbox: danger-full-access".into());
        }
        if arg.eq_ignore_ascii_case("--permission-mode=bypasspermissions") && !full {
            return Err("Forbidden Claude permission mode: bypassPermissions".into());
        }
        if arg == "--ask-for-approval=never" {
            return Err("Forbidden Codex approval mode: never".into());
        }
        let prev = index.checked_sub(1).and_then(|i| args.get(i)).map(String::as_str);
        if arg == "danger-full-access" && prev == Some("--sandbox") && !full {
            return Err("Forbidden Codex sandbox: danger-full-access".into());
        }
        if arg.eq_ignore_ascii_case("bypasspermissions") && prev == Some("--permission-mode") && !full {
            return Err("Forbidden Claude permission mode: bypassPermissions".into());
        }
        if arg == "never" && prev == Some("--ask-for-approval") {
            return Err("Forbidden Codex approval mode: never".into());
        }
        if arg == "--approval-mode=yolo" {
            return Err("Forbidden Gemini approval mode: yolo".into());
        }
        if arg == "yolo" && prev == Some("--approval-mode") {
            return Err("Forbidden Gemini approval mode: yolo".into());
        }
    }
    Ok(())
}
