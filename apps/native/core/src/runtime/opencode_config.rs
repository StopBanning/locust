//! commands.ts:openCodeModeEnv + desktop opencode-2-launch.ts:openCode2Launch.
use crate::{approval::Mode, store::Store};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};
pub fn legacy_config(mode: Mode, plugin: Option<&str>) -> Value {
    legacy_config_with_seal(mode, plugin, false)
}

/// OpenCode legacy config for Ask/Plan with every tool refused (`OPENCODE_SEALED_CONFIG`).
pub fn legacy_sealed_config(plugin: Option<&str>) -> Value {
    legacy_config_with_seal(Mode::Ask, plugin, true)
}

fn legacy_config_with_seal(mode: Mode, plugin: Option<&str>, sealed: bool) -> Value {
    let readonly = matches!(mode, Mode::Ask | Mode::Plan);
    let each = mode == Mode::ApproveEach;
    let mut p = json!({});
    if sealed && readonly {
        p = json!({"*": "ask", "external_directory": "deny"});
    } else if readonly || each {
        p = json!({"*":"ask","read":{"*":"allow","*.env":if each{"ask"}else{"deny"},"*.env.*":if each{"ask"}else{"deny"},"*.env.example":"allow"},"glob":"allow","grep":"allow","list":"allow","lsp":"allow","task":"allow","todowrite":"allow","todoread":"allow","skill":"allow"});
        for k in ["webfetch", "websearch", "codesearch"] {
            p[k] = json!(if each && k == "webfetch" {
                "ask"
            } else {
                "allow"
            });
        }
        // Approve each asks for `edit` alone, as commands.ts writes it; read-only refuses every way to write.
        for k in if each { &["edit"][..] } else { &["edit", "write", "patch"][..] } {
            p[*k] = json!(if each { "ask" } else { "deny" });
        }
        p["bash"] = json!("ask");
    }
    p["external_directory"] = json!(if sealed && readonly {
        "deny"
    } else if mode == Mode::Auto {
        "allow"
    } else if each {
        "ask"
    } else {
        "deny"
    });
    let mut config = json!({"permission":p,"tools":{"question":false}});
    if readonly {
        config["experimental"] = json!({"continue_loop_on_deny":true});
    }
    if !(sealed && readonly)
        && let Some(plugin) = plugin
    {
        config["plugin"] = json!([plugin]);
    }
    config
}
/// openCodeModeEnv's reference folders: in a read-only run, each folder (and what is under it) readable outside the
/// workspace, after a default denial. Other modes leave the config alone: an extra root would also be writable.
pub fn with_references(mut config: Value, mode: Mode, references: &[&str]) -> Value {
    if !matches!(mode, Mode::Ask | Mode::Plan) || references.is_empty() {
        return config;
    }
    let old = config["permission"]["external_directory"].clone();
    let mut allowed = serde_json::Map::new();
    allowed.insert("*".into(), json!("deny"));
    if let Some(o) = old.as_object() {
        allowed.extend(o.clone());
    }
    for folder in references {
        let path = folder.replace('\\', "/").trim_end_matches('/').to_owned();
        let windows = path.len() > 2 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/';
        let spellings = if windows { vec![path.clone(), path.replace('/', "\\")] } else { vec![path.clone()] };
        for spelling in spellings {
            let sep = if spelling.contains('\\') { "\\" } else { "/" };
            allowed.insert(spelling.clone(), json!("allow"));
            allowed.insert(format!("{spelling}{sep}**"), json!("allow"));
            allowed.insert(format!("{spelling}{sep}*"), json!("allow"));
        }
    }
    config["permission"]["external_directory"] = Value::Object(allowed);
    config
}

/// commands.ts:gitDirPattern: the parent repository's `.git`, spelled as the root is spelled (backslashes on Windows).
pub fn git_dir_pattern(repository_root: &str) -> String {
    let b = repository_root.as_bytes();
    let windows = (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
        || repository_root.contains('\\');
    if windows {
        format!("{repository_root}\\.git\\*")
    } else {
        format!("{}/.git/*", repository_root.trim_end_matches('/'))
    }
}

/// commands.ts:opencodeWorktreeConfig, applied to a run's config: a worktree run may reach its parent repository's
/// `.git` and nothing else outside its folder. Auto reaches past both, and a config whose permissions are not an object
/// (OpenCode 2's list) is left as it was. Reference folders already allowed stay allowed beside it.
pub fn with_parent_git(mut config: Value, mode: Mode, repository_root: &str) -> Value {
    if mode == Mode::Auto || !config["permission"].is_object() {
        return config;
    }
    let pattern = git_dir_pattern(repository_root);
    match config["permission"]["external_directory"].as_object_mut() {
        Some(allowed) => {
            allowed.insert(pattern, json!("allow"));
            allowed.entry("*").or_insert(json!("deny"));
        }
        None => {
            let mut allowed = serde_json::Map::new();
            allowed.insert(pattern, json!("allow"));
            allowed.insert("*".into(), json!("deny"));
            config["permission"]["external_directory"] = Value::Object(allowed);
        }
    }
    config
}

/// commands.ts:withOpenCodeGitReads: Locust's read-only git tool (`git_reads.rs`) as the run's `locust_git` MCP server,
/// started in this run's folder, and its tools allowed after every other rule.
pub fn with_git_reads(mut config: Value, command: &[String], folder: &str) -> Value {
    if !config["mcp"].is_object() {
        config["mcp"] = json!({});
    }
    config["mcp"]["locust_git"] = json!({
        "type": "local",
        "command": command,
        "environment": { "LOCUST_GIT_FOLDER": folder },
        "enabled": true,
    });
    if config["permission"].is_object() {
        config["permission"]["locust_git_*"] = json!("allow");
    }
    config
}

/// What openCodeModeEnv adds for one run after its mode's config: the parent `.git` for a run in a worktree, and
/// Locust's git tool for a run with no shell (Ask, Plan). A sealed run gets neither.
pub fn for_run(config: Value, mode: Mode, sealed: bool, repository_root: Option<&str>, git_reads: Option<&[String]>, folder: &str) -> Value {
    if sealed {
        return config;
    }
    let config = match repository_root {
        Some(root) => with_parent_git(config, mode, root),
        None => config,
    };
    match git_reads {
        Some(command) if matches!(mode, Mode::Ask | Mode::Plan) => with_git_reads(config, command, folder),
        _ => config,
    }
}

/// A run's OpenCode 1 config, rewritten in place: only when it carries one whose permissions are an object.
pub fn rewrite_config(command: &mut Command, change: impl FnOnce(Value) -> Value) {
    let config = command
        .get_envs()
        .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, v)| v.and_then(|v| v.to_str()).and_then(|v| serde_json::from_str::<Value>(v).ok()));
    if let Some(config) = config.filter(|c| c["permission"].is_object()) {
        command.env("OPENCODE_CONFIG_CONTENT", change(config).to_string());
    }
}

/// withOpenCodeSkills: `skills.paths` is the person's own paths (`opencode_skill_paths`: OpenCode keeps only the last
/// config's list, and this run's config is last), then any the config had, then the run's skills folder, each once and
/// strings only; the rest of `skills` (its `urls`) kept. The folder is readable outside the workspace unless every
/// folder already is. No folder, the config as it was: the person's paths are only carried beside one.
pub fn with_skills(mut config: Value, dir: Option<&str>, person: &[String]) -> Value {
    let Some(dir) = dir.filter(|d| !d.is_empty()) else { return config };
    let old = config["skills"]["paths"].as_array().cloned().unwrap_or_default();
    let mut paths: Vec<Value> = Vec::new();
    for path in person.iter().map(String::as_str).chain(old.iter().filter_map(Value::as_str)).chain([dir]) {
        if !paths.iter().any(|p| p == path) {
            paths.push(json!(path));
        }
    }
    if !config["skills"].is_object() {
        config["skills"] = json!({});
    }
    config["skills"]["paths"] = Value::Array(paths);
    let external = config["permission"]["external_directory"].clone();
    if config["permission"].is_object() && external != "allow" {
        let mut allowed = match external.as_object() {
            Some(o) => o.clone(),
            None => {
                let mut m = serde_json::Map::new();
                m.insert("*".into(), if external.is_null() { json!("ask") } else { external.clone() });
                m
            }
        };
        let path = dir.replace('\\', "/").trim_end_matches('/').to_owned();
        let windows = path.len() > 2 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/';
        let spellings = if windows { vec![path.clone(), path.replace('/', "\\")] } else { vec![path.clone()] };
        for spelling in spellings {
            let sep = if spelling.contains('\\') { "\\" } else { "/" };
            allowed.insert(spelling.clone(), json!("allow"));
            allowed.insert(format!("{spelling}{sep}**"), json!("allow"));
        }
        config["permission"]["external_directory"] = Value::Object(allowed);
    }
    config
}

pub fn acp_config(mode: Mode, plugin: &Path) -> Value {
    acp_config_with_seal(mode, plugin, false)
}

/// OpenCode 2 (ACP) config with every tool refused (`ask` for `*`, `deny` for question/subagent/integrations).
pub fn acp_sealed_config(plugin: &Path) -> Value {
    acp_config_with_seal(Mode::Ask, plugin, true)
}

fn acp_config_with_seal(mode: Mode, plugin: &Path, sealed: bool) -> Value {
    acp_from_legacy(&legacy_config_with_seal(mode, None, sealed), mode == Mode::ApproveEach, matches!(mode, Mode::Ask | Mode::Plan), sealed, plugin)
}

/// The key a V2 run's config carries while it is still in V1's shape: what the last step needs to write V2's.
const V2_PENDING: &str = "locust_opencode_2";

/// opencode-2-launch.ts: V2's config is written from V1's WHOLE config for the run, after everything a run adds to it
/// (references, the parent `.git` of a worktree, skills, a model of your own). So a V2 run's command carries V1's shape
/// with `V2_PENDING` until `finish_v2`, at spawn, writes V2's from it.
fn pending_v2(mut legacy: Value, mode: Mode, plugin: &Path, sealed: bool) -> Value {
    legacy[V2_PENDING] = json!({"plugin": plugin.to_string_lossy(), "approveEach": mode == Mode::ApproveEach,
        "readOnly": matches!(mode, Mode::Ask | Mode::Plan), "sealed": sealed});
    legacy
}

/// The last step before an OpenCode 2 process starts: its config, still in V1's shape, written as V2's.
pub fn finish_v2(command: &mut Command) {
    let config = command
        .get_envs()
        .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, v)| v.and_then(|v| v.to_str()).and_then(|v| serde_json::from_str::<Value>(v).ok()));
    let Some(config) = config else { return };
    let Some(pending) = config.get(V2_PENDING).cloned() else { return };
    let flag = |k: &str| pending[k].as_bool().unwrap_or(false);
    let plugin = PathBuf::from(pending["plugin"].as_str().unwrap_or_default());
    let native = acp_from_legacy(&config, flag("approveEach"), flag("readOnly"), flag("sealed"), &plugin);
    command.env("OPENCODE_CONFIG_CONTENT", native.to_string());
}

/// `openCode2Launch`: V1's permissions as V2's ordered rules (renamed actions, each path's default first: V2's last
/// match wins), V2's own catch-all first and never V1's (0.821), no question, no subagent in Ask, and everything
/// refused last for a chat-only model of your own (V1's `tools: {"*": false}`). Its providers and skills folder carried.
fn acp_from_legacy(old: &Value, approve_each: bool, readonly: bool, sealed: bool, plugin: &Path) -> Value {
    let mut permissions = vec![
        json!({"action":"*","resource":"*","effect":if approve_each || (sealed && readonly) {"ask"} else {"allow"}}),
    ];
    for (k, v) in old["permission"].as_object().into_iter().flatten() {
        if k == "*" {
            continue;
        }
        let action = match k.as_str() {
            "bash" => "shell",
            "write" | "patch" => "edit",
            "task" => "subagent",
            _ => k,
        };
        if v.is_string() {
            permissions.push(json!({"action":action,"resource":"*","effect":v}));
        } else if let Some(m) = v.as_object() {
            for (resource, effect) in m
                .iter()
                .filter(|(k, _)| k.as_str() == "*")
                .chain(m.iter().filter(|(k, _)| k.as_str() != "*"))
            {
                permissions.push(json!({"action":action,"resource":resource,"effect":effect}));
            }
        }
    }
    permissions.push(json!({"action":"question","resource":"*","effect":"deny"}));
    if readonly {
        permissions.push(json!({"action":"subagent","resource":"*","effect":"deny"}));
    }
    if old["tools"]["*"] == json!(false) {
        permissions.push(json!({"action":"*","resource":"*","effect":"deny"}));
    }
    let mut config = json!({"default_agent":"locust","agents":{"locust":{"mode":"primary","description":"The current Locust turn","permissions":permissions}},"plugins":[file_url(plugin)]});
    // The run's own skills folder, the last of V1's paths (with_skills puts it there).
    if let Some(dir) = old["skills"]["paths"].as_array().and_then(|p| p.last()) {
        config["skills"] = json!([dir]);
    }
    // OpenCode's V2 normalizer takes V1's provider declarations; the run's own stays per-run.
    if !old["provider"].is_null() {
        config["provider"] = old["provider"].clone();
    }
    if readonly {
        config["policies"] = json!([{"action":"integration.use","resource":"plugin:*","effect":"deny"},{"action":"integration.use","resource":format!("plugin:{}",plugin.display()),"effect":"allow"}]);
    }
    config
}
pub fn file_url(p: &Path) -> String {
    let mut s = String::from("file://");
    for b in p.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}
/// Run-owned directory, created only through Store's descriptor-based no-follow writer.
pub fn write_guard(store: &Store, v2: bool) -> io::Result<PathBuf> {
    let root = store
        .ledger_dir()?
        .parent()
        .ok_or_else(|| io::Error::other("Native ledger has no parent directory"))?
        .to_path_buf();
    let id = format!(
        "opencode-guard-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    );
    let dir = store.check(&root.join(id))?;
    store
        .create_new_file_in(&dir, "guard.mjs")?
        .write_all(include_bytes!("resources/locust-command-guard.mjs"))?;
    let shared = r#"import { stoppedBecause, lookAtProcesses } from './guard.mjs';
const check = (command,cwd,workdir) => {
 if (typeof command !== 'string') return;
 const look = () => {const seen=lookAtProcesses(); return seen === undefined ? undefined : {...seen, agent:process.pid, above:[process.pid,...seen.above]};};
 const reason=stoppedBecause(command,look,[cwd,workdir,process.cwd()]);
 if (reason !== undefined) throw new Error(reason);
};
"#;
    let plugin = if v2 {
        format!(
            "{shared}\nexport default {{id:'locust.command-guard',async setup(ctx){{const folder=ctx?.directory??ctx?.project?.directory??ctx?.worktree;await ctx.tool.hook('execute.before',e=>check(e?.tool==='shell'||e?.tool==='bash'?e.input?.command:undefined,folder,e?.input?.workdir));await ctx.shell.hook('create.before',e=>check(e?.command,folder,e?.cwd));}}}}\n"
        )
    } else {
        format!(
            "{shared}\nexport const LocustCommandGuard = async (ctx) => ({{'tool.execute.before':async(input,output)=>{{if(input?.tool==='bash')check(output?.args?.command,ctx?.directory,output?.args?.workdir);}}}});\n"
        )
    };
    store
        .create_new_file_in(&dir, "index.js")?
        .write_all(plugin.as_bytes())?;
    store.create_new_file_in(&dir,"package.json")?.write_all(br#"{"name":"locust-command-guard","private":true,"type":"module","main":"./index.js","exports":"./index.js"}"#)?;
    store.create_new_file_in(&dir.join("config"), ".keep")?;
    Ok(if v2 { dir } else { dir.join("index.js") })
}
pub fn configure(
    c: &mut Command,
    mode: Mode,
    store: &Store,
    v2: bool,
) -> io::Result<PathBuf> {
    configure_with_seal(c, mode, store, v2, false)
}

pub fn configure_with_seal(
    c: &mut Command,
    mode: Mode,
    store: &Store,
    v2: bool,
    sealed: bool,
) -> io::Result<PathBuf> {
    let home = store
        .ledger_dir()?
        .parent()
        .ok_or_else(|| io::Error::other("Native ledger has no parent directory"))?
        .join("opencode-home");
    store.create_new_file_in(
        &home,
        &format!(
            ".run-{}",
            crate::invariants::unique_nanos()
        ),
    )?;
    for key in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
    ] {
        c.env(key, &home);
    }
    let plugin = write_guard(store, v2)?;
    c.env("LOCUST_GUARD_PARENT", std::process::id().to_string());
    if v2 {
        c.env(
            "OPENCODE_CONFIG_CONTENT",
            pending_v2(legacy_config_with_seal(mode, None, sealed), mode, &plugin, sealed).to_string(),
        );
        if matches!(mode, Mode::Ask | Mode::Plan) {
            c.env("OPENCODE_CONFIG_PROJECT_DISABLE", "1")
                .env("OPENCODE_DISABLE_CLAUDE_CODE", "1")
                .env("OPENCODE_CONFIG", "")
                .env("OPENCODE_CONFIG_DIR", plugin.join("config"));
        }
    } else {
        c.env(
            "OPENCODE_CONFIG_CONTENT",
            legacy_config_with_seal(mode, Some(&file_url(&plugin)), sealed).to_string(),
        );
        if matches!(mode, Mode::Ask | Mode::Plan) {
            c.env("OPENCODE_PURE", "1");
        }
    }
    Ok(plugin)
}

#[cfg(test)]
#[path = "opencode_guard_tests.rs"]
mod opencode_guard_tests;

#[cfg(test)]
#[path = "opencode_2_rules_tests.rs"]
mod opencode_2_rules_tests;

#[cfg(test)]
#[path = "opencode_question_tool_tests.rs"]
mod opencode_question_tool_tests;
