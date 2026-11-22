//! Antigravity IDE language server host discovery & agent API (`apps/desktop/src/main/antigravity-host.ts`).

use crate::runtime::antigravity_projects::{normalize_workspace, parse_projects};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PROBE_TTL: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityHost {
    pub executable_path: String,
    pub version: Option<String>,
    pub address: String,
    pub csrf_token: String,
    pub projects: HashMap<String, String>,
    pub ports: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedServerCommandLine {
    pub csrf_token: String,
    pub version: Option<String>,
}

/// The CSRF token and version Antigravity put on its own server's command line (`parseServerCommandLine`).
pub fn parse_server_command_line(command_line: &str) -> Option<ParsedServerCommandLine> {
    if !crate::invariants::regex(r"--override_ide_name antigravity\b").is_match(command_line) {
        return None;
    }
    let csrf_token = crate::invariants::regex(r"(?i)--csrf_token\s+([0-9a-f-]{36})")
        .captures(command_line)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())?;
    let version = crate::invariants::regex(r"--override_ide_version\s+(\S+)")
        .captures(command_line)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned());
    Some(ParsedServerCommandLine { csrf_token, version })
}

/// Both spellings of Antigravity's Windows install folder (`antigravityExecutableCandidates`).
pub fn antigravity_executable_candidates(local_app_data: &str) -> Vec<String> {
    let sep = if local_app_data.contains('\\') || cfg!(windows) {
        "\\"
    } else {
        "/"
    };
    let base = local_app_data.trim_end_matches(['\\', '/']);
    vec![
        format!("{base}{sep}Programs{sep}antigravity{sep}resources{sep}bin{sep}language_server.exe"),
        format!("{base}{sep}Programs{sep}Antigravity{sep}resources{sep}bin{sep}language_server.exe"),
    ]
}

pub fn antigravity_data_directory(home: &str) -> String {
    let sep = if home.contains('\\') || cfg!(windows) {
        "\\"
    } else {
        "/"
    };
    let base = home.trim_end_matches(['\\', '/']);
    format!("{base}{sep}.gemini{sep}antigravity")
}

pub fn transcript_path_for(home: &str, conversation_id: &str) -> String {
    let sep = if home.contains('\\') || cfg!(windows) {
        "\\"
    } else {
        "/"
    };
    let data = antigravity_data_directory(home);
    format!("{data}{sep}brain{sep}{conversation_id}{sep}.system_generated{sep}logs{sep}transcript.jsonl")
}

/// The project id Antigravity gave the mission workspace, or `None` when it has not opened that folder.
pub fn project_id_for(host: &AntigravityHost, workspace_path: &str) -> Option<String> {
    let key = normalize_workspace(workspace_path);
    host.projects.get(&key).cloned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewConversationInput {
    pub project_id: String,
    pub model: String,
    pub title: String,
    pub prompt: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessageInput {
    pub project_id: String,
    pub conversation_id: String,
    pub content: String,
}

pub trait AgentApi: Send + Sync {
    fn new_conversation(&self, input: NewConversationInput) -> Result<String, String>;
    fn send_message(&self, input: SendMessageInput) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    pub command_line: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliRunResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDiscoveryRecord {
    pub id: String,
    pub availability: String,
    pub readiness: String,
}

pub type ListProcessesFn = Arc<dyn Fn() -> Vec<ProcessEntry> + Send + Sync>;
pub type ListeningPortsFn = Arc<dyn Fn(u32) -> Vec<u16> + Send + Sync>;
pub type RunCliFn = Arc<dyn Fn(&str, &[String], &HashMap<String, String>) -> CliRunResult + Send + Sync>;
pub type ExistsFn = Arc<dyn Fn(&str) -> bool + Send + Sync>;
pub type ReadBytesFn = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

pub struct AntigravityProbeOptions {
    pub platform: String,
    pub local_app_data: String,
    pub home: String,
    pub list_processes: ListProcessesFn,
    pub listening_ports: ListeningPortsFn,
    pub run: RunCliFn,
    pub exists: Option<ExistsFn>,
    pub read_bytes: Option<ReadBytesFn>,
}

struct CachedProbe {
    at: Instant,
    value: Option<AntigravityHost>,
}

pub struct AntigravityHostProbe {
    options: AntigravityProbeOptions,
    cached: Mutex<Option<CachedProbe>>,
}

impl AntigravityHostProbe {
    pub fn new(options: AntigravityProbeOptions) -> Self {
        Self {
            options,
            cached: Mutex::new(None),
        }
    }

    fn path_exists(&self, path: &str) -> bool {
        if let Some(f) = &self.options.exists {
            f(path)
        } else {
            Path::new(path).exists()
        }
    }

    fn read_file_bytes(&self, path: &str) -> Option<Vec<u8>> {
        if let Some(f) = &self.options.read_bytes {
            f(path)
        } else {
            std::fs::read(path).ok()
        }
    }

    pub fn probe(&self) -> Option<AntigravityHost> {
        {
            let guard = crate::invariants::lock(&self.cached);
            if let Some(cached) = guard.as_ref()
                && cached.at.elapsed() < PROBE_TTL
            {
                return cached.value.clone();
            }
        }
        if self.options.platform != "win32" {
            return None;
        }
        let executable_path = antigravity_executable_candidates(&self.options.local_app_data)
            .into_iter()
            .find(|candidate| self.path_exists(candidate))?;

        let servers: Vec<(u32, ParsedServerCommandLine)> = (self.options.list_processes)()
            .into_iter()
            .filter_map(|entry| {
                parse_server_command_line(&entry.command_line).map(|parsed| (entry.pid, parsed))
            })
            .collect();

        let mut found = None;
        'outer: for (pid, parsed) in servers {
            let ports = (self.options.listening_ports)(pid);
            let args = vec![
                "agentapi".to_owned(),
                "get-conversation-metadata".to_owned(),
                "00000000-0000-0000-0000-000000000000".to_owned(),
            ];
            for &port in &ports {
                let address = format!("localhost:{port}");
                let mut env = HashMap::new();
                env.insert("ANTIGRAVITY_LS_ADDRESS".into(), address.clone());
                env.insert("ANTIGRAVITY_CSRF_TOKEN".into(), parsed.csrf_token.clone());
                let result = (self.options.run)(&executable_path, &args, &env);
                let combined = format!("{}\n{}", result.stdout, result.stderr);
                if crate::invariants::regex(r"(?i)connection error|wsarecv|is not set")
                    .is_match(&combined)
                {
                    continue;
                }
                let sep = if self.options.home.contains('\\') || cfg!(windows) {
                    "\\"
                } else {
                    "/"
                };
                let pb_path = format!(
                    "{}{sep}agyhub_summaries_proto.pb",
                    antigravity_data_directory(&self.options.home)
                );
                let projects = self
                    .read_file_bytes(&pb_path)
                    .map(|bytes| parse_projects(&bytes))
                    .unwrap_or_default();
                found = Some(AntigravityHost {
                    executable_path: executable_path.clone(),
                    version: parsed.version.clone(),
                    address,
                    csrf_token: parsed.csrf_token.clone(),
                    projects,
                    ports: ports.clone(),
                });
                break 'outer;
            }
        }
        *crate::invariants::lock(&self.cached) = Some(CachedProbe {
            at: Instant::now(),
            value: found.clone(),
        });
        found
    }

    pub fn discovery_record(&self) -> HostDiscoveryRecord {
        let installed = self.options.platform == "win32"
            && antigravity_executable_candidates(&self.options.local_app_data)
                .iter()
                .any(|c| self.path_exists(c));
        if !installed {
            return HostDiscoveryRecord {
                id: "antigravity".into(),
                availability: "unavailable".into(),
                readiness: "unknown".into(),
            };
        }
        match self.probe() {
            Some(_) => HostDiscoveryRecord {
                id: "antigravity".into(),
                availability: "available".into(),
                readiness: "ready".into(),
            },
            None => HostDiscoveryRecord {
                id: "antigravity".into(),
                availability: "available".into(),
                readiness: "unhealthy".into(),
            },
        }
    }
}

/// Parses `language_server agentapi` JSON stdout.
pub fn parse_agent_api_output(stdout: &str, stderr: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(stdout).map_err(|_| {
        let snippet = if !stderr.is_empty() {
            stderr.chars().take(200).collect::<String>()
        } else {
            stdout.chars().take(200).collect::<String>()
        };
        format!("Antigravity's agent API did not answer with JSON: {snippet}")
    })?;
    if let Some(error) = parsed.get("error").and_then(Value::as_str) {
        return Err(error.to_owned());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVER_CMD: &str = "C:\\Users\\dev\\AppData\\Local\\Programs\\antigravity\\resources\\bin\\language_server.exe --standalone --override_ide_name antigravity --subclient_type hub --override_ide_version 2.11.0 --https_server_port 0 --csrf_token 60843f52-6d41-4d97-9b31-53157a780b5e --app_data_dir antigravity";

    #[test]
    fn host_probe_parses_antigravity_only_and_reports_unavailable_when_missing() {
        assert_eq!(
            parse_server_command_line(SERVER_CMD),
            Some(ParsedServerCommandLine {
                csrf_token: "60843f52-6d41-4d97-9b31-53157a780b5e".into(),
                version: Some("2.11.0".into()),
            })
        );
        assert_eq!(
            parse_server_command_line(&SERVER_CMD.replace("--override_ide_name antigravity", "--override_ide_name windsurf")),
            None
        );
        let probe = AntigravityHostProbe::new(AntigravityProbeOptions {
            platform: "win32".into(),
            local_app_data: "C:\\lad".into(),
            home: "C:\\Users\\dev".into(),
            list_processes: Arc::new(Vec::new),
            listening_ports: Arc::new(|_| Vec::new()),
            run: Arc::new(|_, _, _| CliRunResult {
                stdout: String::new(),
                stderr: String::new(),
                code: Some(0),
            }),
            exists: Some(Arc::new(|_| false)),
            read_bytes: None,
        });
        let record = probe.discovery_record();
        assert_eq!(record.id, "antigravity");
        assert_eq!(record.availability, "unavailable");
    }
}
