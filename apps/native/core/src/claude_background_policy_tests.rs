//! Use the manager's real start boundary and the existing recording Claude CLI stand-in.
use super::*;
use crate::{
    org_policy::{self, PolicyState},
    store::Store,
};

struct TestHome(PathBuf);
impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct TestPolicy;
impl TestPolicy {
    fn set(value: Value) -> Self {
        org_policy::use_policy_on_this_thread(Some(PolicyState {
            path: "/test/organization-policy.json".into(),
            present: true,
            policy: org_policy::parse_policy(&value, &mut |_| {}).unwrap(),
            error: None,
        }));
        Self
    }
}
impl Drop for TestPolicy {
    fn drop(&mut self) {
        org_policy::use_policy_on_this_thread(None);
    }
}

fn fixture(tag: &str) -> (TestHome, Store, PathBuf) {
    let home = TestHome(
        std::env::temp_dir().join(format!("locust-bg-policy-{tag}-{}", std::process::id())),
    );
    fs::create_dir_all(home.0.join("electron/mission-ledger")).unwrap();
    let store = Store::new(home.0.join("native"), home.0.join("electron")).unwrap();
    let calls = home.0.join("calls.jsonl");
    (home, store, calls)
}

fn manager(store: &Store, calls: &Path) -> BackgroundRuns<crate::native::NativeBackgroundHost> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/fixtures/claude-bg.py");
    crate::native::background_runs_for_with_program(
        store,
        "python3",
        vec![script.to_string_lossy().into_owned()],
        vec![(
            "CLAUDE_BG_CALLS_FILE".into(),
            calls.to_string_lossy().into_owned(),
        )],
    )
}

fn request(home: &TestHome) -> StartBackgroundRequest {
    StartBackgroundRequest {
        prompt: "Review the work.".into(),
        folder: home.0.to_string_lossy().into_owned(),
        sandbox: BackgroundSandbox::ReadOnly,
        ask_each: false,
        conversation: None,
        teammate_id: None,
        model: Some("haiku".into()),
        effort: None,
        account: None,
    }
}

fn refused(result: StartBackgroundOutcome) -> String {
    match result {
        StartBackgroundOutcome::Refused {
            message,
            needs_trust: false,
        } => message,
        other => panic!("expected a policy refusal before launch, got {other:?}"),
    }
}

#[test]
fn background_start_checks_unmetered_policy_before_launch_or_record() {
    let (home, store, calls) = fixture("unmetered");
    let _policy = TestPolicy::set(json!({"version": 1, "monthlyLimitUsd": 50}));
    let mut real = manager(&store, &calls);
    let why = refused(real.start(request(&home)));
    assert!(
        why.contains("This launch cannot report spending to Locust"),
        "{why}"
    );
    assert!(!calls.exists(), "the Claude process was not launched");
    assert!(!store.root().join("claude-background.json").exists());
}

#[test]
fn background_start_checks_the_model_it_will_actually_pass_to_claude() {
    let (home, store, calls) = fixture("model");
    let _policy = TestPolicy::set(
        json!({"version": 1, "allowedModels": {"claude": ["haiku", "haiku bad"]}, "highestMode": "plan"}),
    );
    let mut real = manager(&store, &calls);
    let mut job = request(&home);
    job.model = Some("haiku bad".into()); // a policy pattern is not necessarily an accepted CLI model value
    let why = refused(real.start(job));
    assert!(
        why.contains("account default") && why.contains("allowedModels"),
        "{why}"
    );
    assert!(!calls.exists());

    let started = real.start(request(&home));
    assert!(
        matches!(started, StartBackgroundOutcome::Started(_)),
        "{started:?}"
    );
    let written = fs::read_to_string(&calls).unwrap();
    let args: Vec<String> = serde_json::from_str(written.lines().next().unwrap()).unwrap();
    assert!(args.windows(2).any(|words| words == ["--model", "haiku"]));
    assert!(
        args.windows(2)
            .any(|words| words == ["--permission-mode", "plan"])
    );
}

#[test]
fn background_start_checks_the_named_teammates_personal_spending_when_unmanaged() {
    let (home, store, calls) = fixture("spend");
    let _engine = crate::native::hooks::install(store.clone(), None);
    let _policy = TestPolicy::set(json!({"version": 1}));
    store.save_json("teammates.json", &json!({
        "schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "monthlyLimitUsd": 5}],
        "missionOwners": {"mission_spent": "tm_wren"}, "settings": {}
    })).unwrap();
    let now = chrono::Local::now().to_rfc3339();
    let head = json!({"schemaVersion": 27, "recordType": "mission.created",
        "metadata":{"missionId":"mission_spent","runtime":"claude","model":"haiku","createdAt":now}});
    let done = json!({"schemaVersion": 27, "recordType": "mission.event",
        "event":{"type":"run.completed","occurredAt":now,"sequence":1,"payload":{"usage":{"usd":5.02}}}});
    fs::write(
        store.electron().join("mission-ledger/mission_spent.jsonl"),
        format!("{head}\n{done}\n"),
    )
    .unwrap();

    let mut job = request(&home);
    job.teammate_id = Some("tm_wren".into());
    let why = refused(manager(&store, &calls).start(job));
    assert!(
        why.contains("Wren has reached this month's limit") && why.contains("$5.02"),
        "{why}"
    );
    assert!(!calls.exists());
    assert!(!store.root().join("claude-background.json").exists());
}
