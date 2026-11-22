//! Cloud computer turn dispatch (`apps/desktop/src/main/codex-mission.ts`):
//! gates cloud-eligible runtimes and models, readies the cloud computer before
//! recording anything, carries `.locust/attachments/` files, briefs the agent
//! with its cloud copy path (`own_folder`), spawns the remote runtime without
//! local guards or local servers, and calls `settled` when the turn ends.

use std::path::Path;

use crate::cloud_turns::{
    CloudAgent, CloudPrepared, CloudSetup, CloudSignIn, CloudStartPhase, carried_path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudTarget {
    pub codespace: String,
    pub dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSpawnCall {
    pub path: String,
    pub args: Vec<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudPrepareRequest {
    pub mission_id: String,
    pub folder: String,
    pub follows: Option<String>,
    pub open_code_version: Option<String>,
    pub agent: Option<(CloudAgent, Option<String>)>,
    pub set_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudDispatchError {
    RuntimeStartFailed {
        message: String,
    },
    CloudSetupNeeded {
        message: String,
        setup: CloudSetup,
    },
    CloudSignInNeeded {
        message: String,
        sign_in: CloudSignIn,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudDispatchInput<'a> {
    pub mission_id: &'a str,
    pub folder: &'a Path,
    pub runtime: &'a str,
    pub cli_version: Option<&'a str>,
    pub model: Option<&'a str>,
    pub prompt: &'a str,
    pub follows: Option<&'a str>,
    pub side: bool,
    pub set_up: bool,
}

pub trait CloudTurnHooks {
    fn prepare(
        &self,
        req: CloudPrepareRequest,
        say: &mut dyn FnMut(CloudStartPhase),
    ) -> CloudPrepared;
    fn carry(&self, mission_id: &str, folder: &Path, files: &[String]);
    fn settled(&self, mission_id: &str);
    fn record_mission(&self, prompt: &str);
    fn brief_hint(&self, own_folder: Option<&str>);
    fn spawn_cloud_claude(&self, target: &CloudTarget, call: CloudSpawnCall);
    fn spawn_cloud_codex(&self, target: &CloudTarget, bin: &str, args: &[String]);
    fn spawn_cloud_opencode(&self, target: &CloudTarget, args: &[String]);
}

/// Extracts `.locust/attachments/...` file paths listed in a message's attachment header.
pub fn prompt_attachment_files(prompt: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_header = false;
    for line in prompt.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Read this file in the workspace before you answer:")
            || trimmed.starts_with("Read these files in the workspace before you answer:")
        {
            in_header = true;
            continue;
        }
        if in_header {
            if let Some(rest) = trimmed.strip_prefix("- ") {
                let candidate = rest.trim();
                if carried_path(candidate) {
                    files.push(candidate.to_string());
                }
            } else if trimmed.is_empty() {
                in_header = false;
            }
        }
    }
    files
}

/// Builds the Claude Code CLI arguments for a cloud turn: never includes local-computer
/// guards (`--settings`, `--mcp-config`, `--permission-prompt-tool`, `--plugin-dir`, `--allowedTools`).
pub fn cloud_claude_args(model: Option<&str>) -> Vec<String> {
    let mut args = vec!["--print".into()];
    if let Some(m) = model
        && !m.is_empty()
        && m != "account-default"
    {
        args.push("--model".into());
        args.push(m.into());
    }
    args
}

/// Dispatches a turn sent with Cloud through the cloud computer seams.
pub fn dispatch_cloud_turn<H: CloudTurnHooks>(
    input: CloudDispatchInput<'_>,
    hooks: &H,
    say: &mut dyn FnMut(CloudStartPhase),
) -> Result<(), CloudDispatchError> {
    if !matches!(input.runtime, "opencode" | "claude" | "codex") {
        return Err(CloudDispatchError::RuntimeStartFailed {
            message: "Your cloud computer runs OpenCode, Claude Code and Codex teammates for now. Pick one of their models, or turn Cloud off.".into(),
        });
    }
    if input.side {
        return Err(CloudDispatchError::RuntimeStartFailed {
            message: "A question on the side is asked on this computer, not on your cloud computer.".into(),
        });
    }
    if input
        .model
        .is_some_and(|m| m.starts_with("own-"))
    {
        return Err(CloudDispatchError::RuntimeStartFailed {
            message: "A model of your own answers from this computer, so it cannot go to your cloud computer. Pick an OpenCode model, or turn Cloud off.".into(),
        });
    }

    let agent = match input.runtime {
        "claude" => Some(CloudAgent::Claude),
        "codex" => Some(CloudAgent::Codex),
        _ => None,
    };

    let prepared = hooks.prepare(
        CloudPrepareRequest {
            mission_id: input.mission_id.to_string(),
            folder: input.folder.to_string_lossy().to_string(),
            follows: input.follows.map(str::to_string),
            open_code_version: input.cli_version.map(str::to_string),
            agent: agent.map(|rt| (rt, input.cli_version.map(str::to_string))),
            set_up: input.set_up,
        },
        say,
    );

    let (dir, codespace, bin) = match prepared {
        CloudPrepared::Ok {
            dir,
            codespace,
            bin,
        } => (dir, codespace, bin),
        CloudPrepared::Err {
            message,
            setup,
            sign_in,
        } => {
            if let Some(sign_in) = sign_in {
                return Err(CloudDispatchError::CloudSignInNeeded { message, sign_in });
            }
            if let Some(setup) = setup {
                return Err(CloudDispatchError::CloudSetupNeeded { message, setup });
            }
            return Err(CloudDispatchError::RuntimeStartFailed { message });
        }
    };

    if agent.is_some() && bin.is_none() {
        return Err(CloudDispatchError::RuntimeStartFailed {
            message: "Your cloud computer did not say where its copy of the agent is. Send again.".into(),
        });
    }

    // Carry the message's own files before the agent starts; nothing sent if none.
    let files = prompt_attachment_files(input.prompt);
    if !files.is_empty() {
        hooks.carry(input.mission_id, input.folder, &files);
    }

    // Brief stands in the cloud copy, never this computer's folder.
    hooks.brief_hint(Some(&dir));
    hooks.record_mission(input.prompt);

    let target = CloudTarget {
        codespace,
        dir: dir.clone(),
    };
    match input.runtime {
        "claude" => {
            let call = CloudSpawnCall {
                path: bin.unwrap_or_default(),
                args: cloud_claude_args(input.model),
                cwd: dir,
            };
            hooks.spawn_cloud_claude(&target, call);
        }
        "codex" => {
            hooks.spawn_cloud_codex(&target, &bin.unwrap_or_default(), &["app-server".into()]);
        }
        "opencode" => {
            hooks.spawn_cloud_opencode(&target, &["serve".into(), "--port".into(), "0".into()]);
        }
        _ => unreachable!(),
    }

    hooks.settled(input.mission_id);
    Ok(())
}

#[cfg(test)]
#[path = "cloud_turn_dispatch_tests.rs"]
mod tests;
