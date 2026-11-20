//! Electron's `an-opencode-teammate-is-guarded-too.test.ts` and `an-opencode-2-teammate-keeps-the-command-guard.test.ts`:
//! every OpenCode run is told to load Locust's command guard, and the plugin Locust writes, loaded in node as OpenCode
//! loads it, refuses what the guard refuses and lets the rest through.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!("locust-oc-guard-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("native")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    (Temp(root), store)
}

const MODES: [Mode; 5] = [Mode::Ask, Mode::AcceptEdits, Mode::ApproveEach, Mode::Plan, Mode::Auto];

/// Load the written plugin in node, call its hook(s) with each (tool, command), and say what each did.
fn hooks_say(plugin: &std::path::Path, v2: bool, calls: &[(&str, &str)]) -> Vec<String> {
    let script = if v2 {
        r#"const m = await import(process.argv[1]); const hooks = new Map();
await m.default.setup({directory: process.cwd(), tool: {hook: async (n, f) => hooks.set('tool.' + n, f)}, shell: {hook: async (n, f) => hooks.set('shell.' + n, f)}});
const out = [[...hooks.keys()].join(',')];
for (const [tool, command] of JSON.parse(process.argv[2])) {
  const said = async (f) => { try { await f(); return 'ok'; } catch (e) { return 'refused: ' + e.message; } };
  const byTool = await said(() => hooks.get('tool.execute.before')({tool, input: {command}}));
  const byShell = await said(() => hooks.get('shell.create.before')(tool === 'shell' ? {command} : {}));
  out.push(byTool === byShell ? byTool : 'tool ' + byTool + ' / shell ' + byShell);
}
console.log(JSON.stringify(out));"#
    } else {
        r#"const m = await import(process.argv[1]); const before = (await m.LocustCommandGuard({directory: process.cwd()}))['tool.execute.before'];
const out = [];
for (const [tool, command] of JSON.parse(process.argv[2])) {
  try { await before({tool}, {args: tool === 'bash' ? {command} : {filePath: command}}); out.push('ok'); } catch (e) { out.push('refused: ' + e.message); }
}
console.log(JSON.stringify(out));"#
    };
    let entry = if v2 { plugin.join("index.js") } else { plugin.to_path_buf() };
    let url = format!("file://{}", entry.display());
    let calls = serde_json::to_string(calls).unwrap();
    let out = std::process::Command::new("node").args(["--input-type=module", "-e", script, "--", &url, &calls]).output().expect("node");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

// ---- an-opencode-teammate-is-guarded-too.test.ts ----

// case: is named on the run route and on the server, Approve each included, in every mode
#[test]
fn the_guard_is_named_in_every_mode_on_both_protocols_and_nothing_without_one() {
    let (_t, store) = store();
    let v1 = write_guard(&store, false).unwrap();
    let v2 = write_guard(&store, true).unwrap();
    let url = v1.to_string_lossy().into_owned();
    for mode in MODES {
        assert_eq!(legacy_config(mode, Some(&url))["plugin"], json!([url]), "{mode:?}");
        assert_eq!(acp_config(mode, &v2)["plugins"][0], json!(file_url(&v2)), "{mode:?}");
    }
    assert!(legacy_config(Mode::AcceptEdits, None).get("plugin").is_none());
}

// case: refuses, with the guard’s own reason, what the guard refuses, and lets the rest through
#[test]
fn the_v1_plugin_refuses_what_the_guard_refuses_and_lets_the_rest_through() {
    let (_t, store) = store();
    let plugin = write_guard(&store, false).unwrap();
    let said = hooks_say(&plugin, false, &[("bash", "taskkill /F /IM msedge.exe"), ("bash", "npm test"), ("edit", "taskkill.txt")]);
    assert!(said[0].starts_with("refused: "), "{said:?}");
    let ctx = crate::command_guard::GuardContext { fix_copy: false, started_pids: Vec::new() };
    let reason = crate::command_guard::stopped_because("taskkill /F /IM msedge.exe", &ctx).expect("the guard's own reason");
    assert!(said[0].contains(reason.split('.').next().unwrap()), "the plugin says the guard's reason: {said:?} / {reason}");
    assert_eq!(&said[1..], ["ok", "ok"], "only commands are its business");
}

// ---- an-opencode-2-teammate-keeps-the-command-guard.test.ts ----

// case: writes a separate V2 entrypoint while leaving the V1 plugin unchanged
#[test]
fn a_v2_entrypoint_is_its_own_and_the_v1_plugin_is_unchanged() {
    let (_t, store) = store();
    let v1 = write_guard(&store, false).unwrap();
    let before = std::fs::read_to_string(&v1).unwrap();
    let v2 = write_guard(&store, true).unwrap();
    assert_ne!(v2.join("index.js"), v1);
    assert_eq!(std::fs::read_to_string(&v1).unwrap(), before);
    let package: Value = serde_json::from_str(&std::fs::read_to_string(v2.join("package.json")).unwrap()).unwrap();
    assert_eq!((package["type"].as_str(), package["main"].as_str()), (Some("module"), Some("./index.js")));
}

#[test]
fn the_v2_plugin_registers_both_hooks_and_refuses_on_each() {
    let (_t, store) = store();
    let v2 = write_guard(&store, true).unwrap();
    let said = hooks_say(&v2, true, &[("shell", "taskkill /F /IM msedge.exe"), ("shell", "npm test"), ("edit", "taskkill.txt")]);
    assert_eq!(said[0], "tool.execute.before,shell.create.before");
    assert!(said[1].starts_with("refused: "), "{said:?}");
    assert_eq!(&said[2..], ["ok", "ok"]);
}
