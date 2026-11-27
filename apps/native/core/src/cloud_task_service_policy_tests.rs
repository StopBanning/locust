use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::{CloudStartInput, CloudStartResult, CloudTaskService, Ran, on_this_computer};
use crate::org_policy::{PolicyState, use_policy_on_this_thread};
use crate::store::Store;

struct PolicyFixture;

impl PolicyFixture {
    fn install(value: Value) -> Self {
        use_policy_on_this_thread(Some(PolicyState {
            path: "/organization/policy.json".into(),
            present: true,
            policy: value.as_object().unwrap().clone(),
            error: None,
        }));
        Self
    }
}

impl Drop for PolicyFixture {
    fn drop(&mut self) {
        use_policy_on_this_thread(None);
    }
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (TempDir, Store) {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = TempDir(std::env::temp_dir().join(format!(
        "locust-codex-cloud-policy-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )));
    let store = Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.root()).unwrap();
    (dir, store)
}

fn start(id: &str) -> CloudStartInput {
    CloudStartInput {
        folder: "/project".into(),
        prompt: "Update the greeting.".into(),
        teammate_id: Some(id.into()),
    }
}

// Use the actual production constructor, then replace both runners. No Codex or Git process can run.
fn fake_runners<'a>(store: &Store, calls: &'a RefCell<Vec<String>>) -> CloudTaskService<'a> {
    let mut service: CloudTaskService<'_> = on_this_computer(store);
    service.codex = Box::new(move |args: &[&str], _cwd: &Path| {
        calls.borrow_mut().push(format!("codex {}", args.join(" ")));
        Ok(Ran::new(
            0,
            "https://chatgpt.com/codex/tasks/task_e_abc123\n",
            "",
        ))
    });
    service.git = Box::new(move |args: &[&str], _cwd: &Path| {
        calls.borrow_mut().push(format!("git {}", args.join(" ")));
        Ok(match args.first().copied() {
            Some("remote") => Ran::new(0, "https://github.com/acme/widgets.git\n", ""),
            Some("rev-parse") => Ran::new(0, "main\n", ""),
            Some("rev-list") => Ran::new(0, "0\n", ""),
            _ => Ran::new(0, "", ""),
        })
    });
    service
}

fn refused(result: CloudStartResult, why: &str) {
    let CloudStartResult::Err { message } = result else {
        panic!("expected refusal mentioning {why}, got {result:?}");
    };
    assert!(message.contains(why), "{message}");
}

#[test]
fn codex_cloud_checks_organization_policy_before_any_runner_or_task_record() {
    let (_dir, store) = fixture();
    let calls = RefCell::new(Vec::new());
    let service = fake_runners(&store, &calls);
    for (policy, setting) in [
        (json!({"cloud": false}), "cloud"),
        (json!({"allowedAgents": ["claude"]}), "allowedAgents"),
        (json!({"highestMode": "approve-each"}), "highestMode"),
        (json!({"monthlyLimitUsd": 10.0}), "monthlyLimitUsd"),
    ] {
        let _policy = PolicyFixture::install(policy);
        refused(service.start(start("wren")), setting);
        assert!(calls.borrow().is_empty());
        assert!(!super::store_path_of(&store).exists());
    }
}

#[test]
fn codex_cloud_cannot_claim_a_named_model_the_cloud_does_not_pin() {
    let (_dir, store) = fixture();
    let calls = RefCell::new(Vec::new());
    let service = fake_runners(&store, &calls);
    let _policy = PolicyFixture::install(json!({"allowedModels": {"codex": ["gpt-5.4"]}}));
    refused(service.start(start("wren")), "allowedModels");
    assert!(calls.borrow().is_empty());
}

#[test]
fn codex_cloud_checks_the_selected_teammates_spend_before_the_runner() {
    let (_dir, store) = fixture();
    let _policy = PolicyFixture::install(json!({}));
    std::fs::write(
        store.teammates_json_path().unwrap(),
        json!({
            "schemaVersion": 1,
            "teammates": [
                {"teammateId": "wren", "name": "Wren", "monthlyLimitUsd": 1.0},
                {"teammateId": "robin", "name": "Robin", "monthlyLimitUsd": 1.0}
            ],
            "missionOwners": {"earlier": "wren"}
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    let at = chrono::Local::now().to_rfc3339();
    let created = json!({
        "recordType": "mission.created",
        "metadata": {"runtime": "codex", "model": "gpt-5.4", "createdAt": at}
    });
    let done = json!({"event": {
        "type": "run.completed", "occurredAt": at, "payload": {"usage": {"usd": 1.0}}
    }});
    std::fs::write(
        store.ledger_dir().unwrap().join("earlier.jsonl"),
        format!("{created}\n{done}\n"),
    )
    .unwrap();

    let calls = RefCell::new(Vec::new());
    let service = fake_runners(&store, &calls);
    refused(
        service.start(start("wren")),
        "Wren has reached this month's limit",
    );
    assert!(calls.borrow().is_empty());
    assert!(!super::store_path_of(&store).exists());

    assert!(matches!(
        service.start(start("robin")),
        CloudStartResult::Ok { .. }
    ));
    assert!(
        calls
            .borrow()
            .iter()
            .any(|call| call.starts_with("codex cloud exec "))
    );
}
