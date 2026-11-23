//! LIVE2 #13: a new Claude turn with no teammate (such as a door start_conversation) runs on the signed-in
//! `forNew` account with its `CLAUDE_CONFIG_DIR`, while a reply in an existing default-account conversation stays on
//! This Mac's sign-in and an explicit account override is respected.
use crate::agent_accounts::{AccountState, Accounts, FOLDER};
use crate::native::{Origin, Send, accounts_for, hooks, nobody, owns, send_from, set_open_folder};
use crate::store::Store;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-live2-13-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos(),
        ));
        fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    fn store(&self) -> Store {
        Store::new(self.0.join("native"), self.0.join("electron")).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_env_wrapper(dir: &Path, log: &Path) -> Option<(PathBuf, Vec<String>)> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts/fixtures/m3-claude.py");
    let script = dir.join("wrap_claude.py");
    let py = r#"import json, os, sys
log_path, fixture_path = sys.argv[1:3]
keys = ['CLAUDE_CONFIG_DIR', 'ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'CLAUDE_CODE_OAUTH_TOKEN']
with open(log_path, 'a', encoding='utf-8') as f:
    f.write(json.dumps({k: os.environ.get(k) for k in keys}) + '\n')
os.execv(sys.executable, [sys.executable, fixture_path, 'success'] + sys.argv[3:])
"#;
    fs::write(&script, py).unwrap();
    Some((
        PathBuf::from("python3"),
        vec![
            script.to_string_lossy().into_owned(),
            log.to_string_lossy().into_owned(),
            fixture.to_string_lossy().into_owned(),
        ],
    ))
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn ledger_created(store: &Store, id: &str) -> Value {
    let path = store.ledger_dir().unwrap().join(format!("{id}.jsonl"));
    let first = fs::read_to_string(path).unwrap().lines().next().unwrap().to_owned();
    serde_json::from_str(&first).unwrap()
}

#[test]
fn new_claude_turn_without_teammate_uses_the_for_new_signed_in_account() {
    let t = Temp::new();
    let store = t.store();
    let env_log = t.0.join("claude_env.jsonl");
    let _guard = hooks::install(store.clone(), write_env_wrapper(&t.0, &env_log));
    set_open_folder(&store, &t.0.join("work")).unwrap();

    // Seed an Electron-format accounts.json with a ready Claude account marked forNew: true.
    let account_id = format!("account_{}", "8".repeat(32));
    let accounts_root = store.root().join(FOLDER);
    let account_home = accounts_root.join(&account_id);
    fs::create_dir_all(&account_home).unwrap();
    fs::write(
        accounts_root.join("accounts.json"),
        json!([{
            "id": account_id,
            "runtime": "claude",
            "name": "freev2",
            "state": "ready",
            "forNew": true
        }])
        .to_string(),
    )
    .unwrap();

    // Start a brand-new Claude turn with no teammate (as door start_conversation does).
    let started = send_from(
        Send {
            teammate: nobody(Some("claude".into()), Some("opus".into())),
            conversation: None,
            prompt: "Reply with just the word ok.".into(),
            model: Some("opus".into()),
            effort: None,
        },
        Origin {
            started_by: Some(json!({"kind": "app", "name": "test"})),
            ..Default::default()
        },
    )
    .unwrap();
    wait_until("claude turn finish", || !owns(&started.conversation_id));

    let created = ledger_created(&store, &started.conversation_id);
    assert_eq!(
        created["metadata"]["resolvedRouteId"].as_str(),
        Some(format!("claude:opus-account:{account_id}").as_str()),
        "a new Claude turn with no teammate must run on the signed-in forNew account"
    );
    let logged: Vec<Value> = fs::read_to_string(&env_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(logged.len(), 1);
    assert_eq!(
        logged[0]["CLAUDE_CONFIG_DIR"].as_str(),
        Some(account_home.to_string_lossy().as_ref())
    );
    assert_eq!(logged[0]["ANTHROPIC_API_KEY"].as_str(), Some(""));
    assert_eq!(logged[0]["ANTHROPIC_AUTH_TOKEN"].as_str(), Some(""));
    assert_eq!(logged[0]["CLAUDE_CODE_OAUTH_TOKEN"].as_str(), Some(""));

    // An explicit None override (This Mac's sign-in) must still run without CLAUDE_CONFIG_DIR,
    // and a reply in that conversation must stay on This Mac's sign-in rather than switching to forNew.
    let mac = send_from(
        Send {
            teammate: nobody(Some("claude".into()), Some("opus".into())),
            conversation: None,
            prompt: "On this Mac".into(),
            model: Some("opus".into()),
            effort: None,
        },
        Origin {
            account: Some(None),
            ..Default::default()
        },
    )
    .unwrap();
    wait_until("mac turn finish", || !owns(&mac.conversation_id));
    let mac_created = ledger_created(&store, &mac.conversation_id);
    assert_eq!(mac_created["metadata"]["resolvedRouteId"].as_str(), Some("claude:opus-account:default"));

    let reply = send_from(
        Send {
            teammate: nobody(Some("claude".into()), Some("opus".into())),
            conversation: Some(mac.conversation_id.clone()),
            prompt: "Follow up on this Mac".into(),
            model: Some("opus".into()),
            effort: None,
        },
        Origin::default(),
    )
    .unwrap();
    wait_until("mac reply finish", || !owns(&reply.conversation_id));
    let reply_tip = crate::native::real_tip(&reply.conversation_id).unwrap();
    let reply_created = ledger_created(&store, &reply_tip);
    assert_eq!(
        reply_created["metadata"]["resolvedRouteId"].as_str(),
        Some("claude:opus-account:default"),
        "a reply in a default-account conversation must stay on default, not move to forNew"
    );
    let logged: Vec<Value> = fs::read_to_string(&env_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(logged.len(), 3);
    assert!(logged[1]["CLAUDE_CONFIG_DIR"].is_null());
    assert!(logged[2]["CLAUDE_CONFIG_DIR"].is_null());

    // Verify accounts_for sees the ready account too.
    let listed = accounts_for(&store).list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, AccountState::Ready);
    let _ = (Accounts::new, Mutex::new(()), Arc::new(()));
}

/// Electron 0.876.1 (`codex-mission.ts`): the for-new account is never used for a rewind. An edit of the first message
/// starts afresh with no conversation, so only the rewind itself tells the engine it is the same conversation, which
/// stays on the account it ran on (23 Nov merge check: a break removing the rewind guard survived).
#[test]
fn an_edited_first_message_stays_on_the_account_its_conversation_ran_on() {
    let t = Temp::new();
    let store = t.store();
    let env_log = t.0.join("claude_env.jsonl");
    let _guard = hooks::install(store.clone(), write_env_wrapper(&t.0, &env_log));
    set_open_folder(&store, &t.0.join("work")).unwrap();
    let account_id = format!("account_{}", "7".repeat(32));
    let accounts_root = store.root().join(FOLDER);
    fs::create_dir_all(accounts_root.join(&account_id)).unwrap();
    fs::write(
        accounts_root.join("accounts.json"),
        json!([{"id": account_id, "runtime": "claude", "name": "work", "state": "ready", "forNew": true}]).to_string(),
    )
    .unwrap();

    let first = send_from(
        Send { teammate: nobody(Some("claude".into()), Some("opus".into())), conversation: None, prompt: "On this Mac".into(), model: Some("opus".into()), effort: None },
        Origin { account: Some(None), ..Default::default() },
    )
    .unwrap();
    wait_until("first turn finish", || !owns(&first.conversation_id));
    assert_eq!(ledger_created(&store, &first.conversation_id)["metadata"]["resolvedRouteId"].as_str(), Some("claude:opus-account:default"));

    let edited = send_from(
        Send { teammate: nobody(Some("claude".into()), Some("opus".into())), conversation: None, prompt: "On this Mac, edited".into(), model: Some("opus".into()), effort: None },
        Origin {
            rewind: Some(crate::native::Rewind { tip: first.conversation_id.clone(), before: None, set_aside: vec![first.conversation_id.clone()] }),
            ..Default::default()
        },
    )
    .unwrap();
    wait_until("edited turn finish", || !owns(&edited.conversation_id));
    let tip = crate::native::real_tip(&edited.conversation_id).unwrap_or(edited.conversation_id.clone());
    assert_ne!(
        ledger_created(&store, &tip)["metadata"]["resolvedRouteId"].as_str(),
        Some(format!("claude:opus-account:{account_id}").as_str()),
        "an edit must not move the conversation onto the for-new account"
    );
}
