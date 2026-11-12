//! commands.ts:openCodeModeEnv + desktop opencode-2-launch.ts:openCode2Launch.
use crate::{approval::Mode, store::Store};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};
pub fn legacy_config(mode: Mode, plugin: Option<&str>) -> Value {
    let readonly = matches!(mode, Mode::Ask | Mode::Plan);
    let each = mode == Mode::ApproveEach;
    let mut p = json!({});
    if readonly || each {
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
    p["external_directory"] = json!(if mode == Mode::Auto {
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
    if let Some(plugin) = plugin {
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

/// withOpenCodeSkills: the run's skills folder in `skills.paths` (after any the config had), and that folder readable
/// outside the workspace unless every folder already is.
pub fn with_skills(mut config: Value, dir: Option<&str>) -> Value {
    let Some(dir) = dir.filter(|d| !d.is_empty()) else { return config };
    let mut paths: Vec<Value> = config["skills"]["paths"].as_array().cloned().unwrap_or_default();
    if !paths.iter().any(|p| p == dir) {
        paths.push(json!(dir));
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
    let old = legacy_config(mode, None);
    let mut permissions = vec![
        json!({"action":"*","resource":"*","effect":if mode==Mode::ApproveEach{"ask"}else{"allow"}}),
    ];
    for (k, v) in old["permission"]
        .as_object()
        // Invariant: legacy_config constructs permission as an object.
        .expect("legacy_config constructs permission as an object")
    {
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
    let readonly = matches!(mode, Mode::Ask | Mode::Plan);
    if readonly {
        permissions.push(json!({"action":"subagent","resource":"*","effect":"deny"}));
    }
    let mut config = json!({"default_agent":"locust","agents":{"locust":{"mode":"primary","description":"The current Locust turn","permissions":permissions}},"plugins":[file_url(plugin)]});
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
            acp_config(mode, &plugin).to_string(),
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
            legacy_config(mode, Some(&file_url(&plugin))).to_string(),
        );
        if matches!(mode, Mode::Ask | Mode::Plan) {
            c.env("OPENCODE_PURE", "1");
        }
    }
    Ok(plugin)
}
