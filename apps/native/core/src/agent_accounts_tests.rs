//! Electron's an-added-account-has-only-its-own-home.test.ts (the account store's cases) and
//! an-account-sign-in-belongs-to-its-agent.test.ts, against scratch folders and a sign-in that the test answers.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Done = Box<dyn FnOnce(bool) + Send>;

#[derive(Default)]
struct Sign {
    started: Mutex<Vec<(Agent, PathBuf, Vec<(&'static str, String)>)>>,
    dones: Mutex<Vec<Done>>,
    cancelled: Arc<AtomicBool>,
    fails: bool,
}

fn scratch() -> Scratch {
    let root = std::env::temp_dir().join(format!("locust-accounts-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    Scratch(root)
}

fn accounts_with(data: &Path, sign: Arc<Sign>, refusal: Arc<Mutex<Option<String>>>, busy: Arc<AtomicBool>) -> Accounts {
    let login: Arc<Login> = Arc::new(move |agent: Agent, home: &Path, env: &[(&'static str, String)], done: Done| {
        if sign.fails {
            return Err("spawn failed with a token-shaped secret".into());
        }
        sign.started.lock().unwrap().push((agent, home.to_path_buf(), env.to_vec()));
        sign.dones.lock().unwrap().push(done);
        let cancelled = sign.cancelled.clone();
        Ok(Box::new(move || cancelled.store(true, Ordering::SeqCst)) as Cancel)
    });
    Accounts::new(data, login, Box::new(move || refusal.lock().unwrap().clone()), Box::new(move |_, _| busy.load(Ordering::SeqCst)))
}

fn accounts(data: &Path) -> (Accounts, Arc<Sign>) {
    let sign = Arc::new(Sign::default());
    (accounts_with(data, sign.clone(), Arc::default(), Arc::default()), sign)
}

#[test]
fn keeps_the_exact_launch_and_environment_when_no_account_is_chosen() {
    let s = scratch();
    let (accounts, _) = accounts(&s.0);
    assert_eq!(accounts.launch_env("claude", None).unwrap(), vec![]);
    assert_eq!(accounts.launch_env("codex", None).unwrap(), vec![]);
}

#[test]
fn gives_each_supported_agent_and_account_a_separate_home_under_locust_data() {
    let s = scratch();
    let (accounts, _) = accounts(&s.0);
    let work = accounts.add("claude", "  Work  ").unwrap();
    let home = accounts.add("claude", "Home").unwrap();
    let codex = accounts.add("codex", "Team").unwrap();
    assert_eq!(work.name, "Work");
    assert!(is_account_id(&work.id) && is_account_id(&codex.id));
    let dirs: HashSet<PathBuf> = [&work, &home, &codex].iter().map(|a| accounts.home_for(&a.id)).collect();
    assert_eq!(dirs.len(), 3);
    for dir in &dirs {
        assert!(dir.is_dir() && dir.starts_with(s.0.join(FOLDER)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }
    let claude_env = accounts.launch_env("claude", Some(&work.id)).unwrap();
    assert!(claude_env.contains(&("CLAUDE_CONFIG_DIR", accounts.home_for(&work.id).to_string_lossy().into_owned())));
    assert!(claude_env.contains(&("ANTHROPIC_API_KEY", String::new())) && claude_env.contains(&("CLAUDE_CODE_OAUTH_TOKEN", String::new())));
    let codex_env = accounts.launch_env("codex", Some(&codex.id)).unwrap();
    assert!(codex_env.contains(&("CODEX_HOME", accounts.home_for(&codex.id).to_string_lossy().into_owned())));
    assert!(codex_env.contains(&("OPENAI_API_KEY", String::new())));
    assert_eq!(accounts.launch_env("codex", Some(&work.id)), Err(OTHER_AGENT.into()));
    assert_eq!(accounts.add("opencode", "x").unwrap_err(), NOT_SUPPORTED);
    for bad in ["", "   ", &"x".repeat(81), "line\nbreak"] {
        assert_eq!(accounts.add("claude", bad).unwrap_err(), NAME_IT);
    }
    // The list survives a restart, names and states only.
    let list = std::fs::read_to_string(s.0.join(FOLDER).join(LIST)).unwrap();
    assert!(list.contains("\"Work\"") && !list.contains("CLAUDE_CONFIG_DIR"));
    let (again, _) = super::tests::accounts(&s.0);
    assert_eq!(again.list().unwrap().len(), 3);
}

#[test]
fn never_reads_copies_or_prints_a_credential_file_while_managing_accounts() {
    let s = scratch();
    let (accounts, _) = accounts(&s.0);
    let a = accounts.add("claude", "Work").unwrap();
    let secret = accounts.home_for(&a.id).join(".credentials.json");
    std::fs::write(&secret, "SECRET-TOKEN-VALUE").unwrap();
    accounts.rename(&a.id, "Renamed").unwrap();
    let listed = format!("{:?}", accounts.list().unwrap());
    assert!(!listed.contains("SECRET-TOKEN-VALUE"));
    let saved = std::fs::read_to_string(s.0.join(FOLDER).join(LIST)).unwrap();
    assert!(!saved.contains("SECRET-TOKEN-VALUE") && saved.contains("Renamed"));
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "SECRET-TOKEN-VALUE");
}

#[test]
fn removes_only_the_home_locust_made_and_refuses_a_link_pointing_elsewhere() {
    let s = scratch();
    let (accounts, _) = accounts(&s.0);
    let a = accounts.add("codex", "Work").unwrap();
    let b = accounts.add("codex", "Other").unwrap();
    accounts.remove(&a.id).unwrap();
    assert!(!accounts.home_for(&a.id).exists());
    assert_eq!(accounts.list().unwrap().iter().map(|x| x.id.clone()).collect::<Vec<_>>(), vec![b.id.clone()]);
    assert_eq!(accounts.launch_env("codex", Some(&a.id)), Err(REMOVED.into()));
    // A home swapped for a link to somewhere else: nothing there is touched.
    let elsewhere = s.0.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("keep.txt"), "mine").unwrap();
    std::fs::remove_dir_all(accounts.home_for(&b.id)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, accounts.home_for(&b.id)).unwrap();
    assert_eq!(accounts.remove(&b.id), Err(MOVED.into()));
    assert_eq!(accounts.launch_env("codex", Some(&b.id)), Err(MOVED.into()));
    assert!(elsewhere.join("keep.txt").exists());
    assert_eq!(accounts.list().unwrap().len(), 1);
}

#[test]
fn tracks_waiting_success_cancellation_and_interrupted_sign_ins() {
    let s = scratch();
    let (accounts, sign) = accounts(&s.0);
    let a = accounts.add("claude", "Work").unwrap();
    accounts.login(&a.id).unwrap();
    let state = |acc: &Accounts| acc.list().unwrap()[0].state;
    assert_eq!(state(&accounts), AccountState::Waiting);
    let (agent, home, env) = sign.started.lock().unwrap()[0].clone();
    assert_eq!((agent, home.clone()), (Agent::Claude, accounts.home_for(&a.id)));
    assert!(env.contains(&("CLAUDE_CONFIG_DIR", home.to_string_lossy().into_owned())));
    // Pressing again while it waits starts nothing more; a run cannot use the account until it ends.
    accounts.login(&a.id).unwrap();
    assert_eq!(sign.started.lock().unwrap().len(), 1);
    assert_eq!(accounts.launch_env("claude", Some(&a.id)), Err(IN_PROGRESS.into()));
    (sign.dones.lock().unwrap().remove(0))(true);
    assert_eq!(state(&accounts), AccountState::Ready);
    assert!(accounts.launch_env("claude", Some(&a.id)).is_ok());

    // Cancelled: stopped, failed, and its late answer changes nothing.
    accounts.login(&a.id).unwrap();
    accounts.cancel(&a.id).unwrap();
    assert!(sign.cancelled.load(Ordering::SeqCst));
    assert_eq!(state(&accounts), AccountState::Failed);
    (sign.dones.lock().unwrap().remove(0))(true);
    assert_eq!(state(&accounts), AccountState::Failed);

    // Interrupted: a sign-in left waiting when Locust ended reads as failed.
    accounts.login(&a.id).unwrap();
    let (later, _) = super::tests::accounts(&s.0);
    assert_eq!(state(&later), AccountState::Failed);
}

#[test]
fn a_sign_in_that_cannot_start_says_so_without_the_agents_own_words() {
    let s = scratch();
    let sign = Arc::new(Sign { fails: true, ..Default::default() });
    let accounts = accounts_with(&s.0, sign, Arc::default(), Arc::default());
    let a = accounts.add("codex", "Work").unwrap();
    assert_eq!(accounts.login(&a.id), Err(NO_SIGN_IN.into()));
    assert_eq!(accounts.list().unwrap()[0].state, AccountState::Failed);
}

#[test]
fn the_accounts_policy_holds_adding_signing_in_and_launching_and_still_allows_removal() {
    let s = scratch();
    let refusal = Arc::new(Mutex::new(None));
    let accounts = accounts_with(&s.0, Arc::default(), refusal.clone(), Arc::default());
    let a = accounts.add("claude", "Work").unwrap();
    *refusal.lock().unwrap() = Some("Added accounts are off.".into());
    assert_eq!(accounts.add("claude", "Another").unwrap_err(), "Added accounts are off.");
    assert_eq!(accounts.login(&a.id), Err("Added accounts are off.".into()));
    assert_eq!(accounts.launch_env("claude", Some(&a.id)), Err("Added accounts are off.".into()));
    assert_eq!(accounts.rename(&a.id, "x"), Err("Added accounts are off.".into()));
    // Without an account nothing is refused.
    assert_eq!(accounts.launch_env("claude", None), Ok(vec![]));
    accounts.remove(&a.id).unwrap();
}

#[test]
fn refuses_removal_or_a_new_sign_in_while_the_account_is_in_use() {
    let s = scratch();
    let busy = Arc::new(AtomicBool::new(false));
    let accounts = accounts_with(&s.0, Arc::default(), Arc::default(), busy.clone());
    let a = accounts.add("codex", "Work").unwrap();
    busy.store(true, Ordering::SeqCst);
    assert_eq!(accounts.remove(&a.id), Err(BUSY_REMOVE.into()));
    assert_eq!(accounts.login(&a.id), Err(BUSY_LOGIN.into()));
    assert!(accounts.home_for(&a.id).exists());
}

#[test]
fn keeps_usage_separate_for_each_account_and_reads_the_account_a_route_names() {
    let id = "account_0123456789abcdef0123456789abcdef";
    assert_eq!(usage_key("codex", None), "codex");
    assert_eq!(usage_key("codex", Some(id)), format!("codex:{id}"));
    assert_eq!(account_from_route(&format!("codex-gpt-5-account:{id}")), Some(id));
    assert_eq!(account_from_route("codex-gpt-5"), None);
    assert_eq!(account_from_route("codex-account:account_xyz"), None);
    assert!(!is_account_id("account_0123456789ABCDEF0123456789abcdef"));
}

#[test]
fn an_unreadable_list_changes_no_account() {
    let s = scratch();
    std::fs::create_dir_all(s.0.join(FOLDER)).unwrap();
    std::fs::write(s.0.join(FOLDER).join(LIST), "{not a list").unwrap();
    let (accounts, _) = accounts(&s.0);
    assert_eq!(accounts.list(), Err(UNREADABLE.into()));
    assert_eq!(accounts.add("claude", "Work").unwrap_err(), UNREADABLE);
    assert_eq!(std::fs::read_to_string(s.0.join(FOLDER).join(LIST)).unwrap(), "{not a list");
}

// Electron's an-account-sign-in-belongs-to-its-agent.test.ts, with a stand-in program that records how it was run.
#[test]
fn runs_each_documented_login_hidden_with_its_home_and_reports_only_whether_it_worked() {
    let s = scratch();
    let program = s.0.join("agent");
    std::fs::write(&program, "#!/bin/sh\necho \"$@|$CODEX_HOME|$CLAUDE_CONFIG_DIR|$(pwd -P)\" > \"$RECORD\"\necho 'secret token on stdout'\nexit ${EXIT_WITH:-0}\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let home = s.0.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let home = std::fs::canonicalize(home).unwrap();
    for (agent, exit, expect) in [(Agent::Claude, "0", "auth login"), (Agent::Codex, "0", "login"), (Agent::Codex, "1", "login")] {
        let record = s.0.join("record.txt");
        let mut env = env_for(agent, &home);
        env.push(("RECORD", record.to_string_lossy().into_owned()));
        env.push(("EXIT_WITH", exit.to_owned()));
        let (tx, rx) = std::sync::mpsc::channel();
        let _stop = start_login(&program, agent, &home, &env, Box::new(move |ok| tx.send(ok).unwrap())).unwrap();
        let ok = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(ok, exit == "0");
        assert!(rx.recv_timeout(std::time::Duration::from_millis(50)).is_err(), "told more than once");
        let seen = std::fs::read_to_string(&record).unwrap();
        let fields: Vec<&str> = seen.trim().split('|').collect();
        assert_eq!(fields[0], expect);
        let home_var = if agent == Agent::Codex { fields[1] } else { fields[2] };
        assert_eq!(home_var, home.to_string_lossy());
        assert_eq!(fields[3], home.to_string_lossy());
    }
}

#[test]
fn for_new_moves_on_first_sign_in_and_respects_use_for_new_and_ready_state() {
    let s = scratch();
    let (accounts, sign) = accounts(&s.0);
    let max = accounts.add("claude", "max").unwrap();
    assert_eq!(accounts.for_new("claude").unwrap(), None);
    // Failed sign-in leaves for_new unset.
    accounts.login(&max.id).unwrap();
    (sign.dones.lock().unwrap().remove(0))(false);
    assert_eq!(accounts.for_new("claude").unwrap(), None);
    // First successful sign-in claims for_new.
    accounts.login(&max.id).unwrap();
    (sign.dones.lock().unwrap().remove(0))(true);
    assert_eq!(accounts.for_new("claude").unwrap().as_deref(), Some(max.id.as_str()));
    // Second signed-in account does not steal for_new.
    let personal = accounts.add("claude", "Personal").unwrap();
    accounts.login(&personal.id).unwrap();
    (sign.dones.lock().unwrap().remove(0))(true);
    assert_eq!(accounts.for_new("claude").unwrap().as_deref(), Some(max.id.as_str()));
    // use_for_new switches to Personal, or back to None (This Mac's sign-in).
    accounts.use_for_new("claude", Some(&personal.id)).unwrap();
    assert_eq!(accounts.for_new("claude").unwrap().as_deref(), Some(personal.id.as_str()));
    // Unrelated agent choice stays independent; wrong agent or unsigned account refused.
    let codex = accounts.add("codex", "Plus").unwrap();
    accounts.login(&codex.id).unwrap();
    (sign.dones.lock().unwrap().remove(0))(true);
    let fresh = accounts.add("claude", "Fresh").unwrap();
    assert_eq!(
        accounts.use_for_new("claude", Some(&fresh.id)).unwrap_err(),
        "Sign in to Fresh first. New conversations are unchanged."
    );
    assert_eq!(accounts.use_for_new("claude", Some(&codex.id)).unwrap_err(), OTHER_AGENT_FOR_NEW);
    assert_eq!(accounts.use_for_new("cursor", None).unwrap_err(), NOT_SUPPORTED);
    // Re-signing in makes it Waiting, so for_new returns None until ready again.
    accounts.login(&personal.id).unwrap();
    assert_eq!(accounts.for_new("claude").unwrap(), None);
    (sign.dones.lock().unwrap().remove(0))(true);
    assert_eq!(accounts.for_new("claude").unwrap().as_deref(), Some(personal.id.as_str()));
    // Clearing claude leaves codex untouched.
    accounts.use_for_new("claude", None).unwrap();
    assert_eq!(accounts.for_new("claude").unwrap(), None);
    assert_eq!(accounts.for_new("codex").unwrap().as_deref(), Some(codex.id.as_str()));
}

// ---- the account at work: a real start, a backup, background work (Electron's last three cases) ----

/// An agent in place of the real one: it writes its arguments and the account keys of its environment as one JSON line
/// to `log`, then does `then` (Python, with `args` bound to its arguments).
fn logging_agent(dir: &Path, name: &str, log: &Path, then: &str) -> (PathBuf, Vec<String>) {
    let script = dir.join(name);
    let body = format!(
        "import json, os, sys\nargs = sys.argv[1:]\nkeys = ['CODEX_HOME', 'CODEX_SQLITE_HOME', 'OPENAI_API_KEY', 'CLAUDE_CONFIG_DIR', 'ANTHROPIC_API_KEY']\nwith open({log:?}, 'a', encoding='utf-8') as f:\n    f.write(json.dumps({{'args': args, 'env': {{k: os.environ.get(k) for k in keys}}}}) + '\\n')\n{then}\n",
        log = log.to_string_lossy(),
    );
    std::fs::write(&script, body).unwrap();
    (PathBuf::from("python3"), vec![script.to_string_lossy().into_owned()])
}

fn logged(log: &Path) -> Vec<Value> {
    std::fs::read_to_string(log).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

/// The machine's own policy back after a test that set none (the engine reads it on the turn's own threads).
struct NoPolicy;
impl NoPolicy {
    fn set() -> NoPolicy {
        crate::org_policy::use_policy(None);
        NoPolicy
    }
}
impl Drop for NoPolicy {
    fn drop(&mut self) {
        crate::org_policy::use_policy(None);
    }
}

/// The engine's store over a scratch folder, with `accounts` as the store's own (what `native::accounts_for` finds).
fn engine_store(s: &Scratch) -> (crate::store::Store, Arc<Accounts>) {
    std::fs::create_dir_all(s.0.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(s.0.join("work")).unwrap();
    let store = crate::store::Store::new(s.0.join("native"), s.0.join("electron")).unwrap();
    let accounts = Accounts::new(store.root(), Arc::new(|_, _, _, _| Ok(Box::new(|| {}) as Cancel)), Box::new(|| None), Box::new(|_, _| false));
    (store.clone(), register(Arc::new(accounts)))
}

#[test]
fn a_real_start_on_an_account_runs_with_its_home_and_a_start_without_one_is_left_as_it_was() {
    // case: dispatches a real mission service command with its account home and leaves a default command unchanged
    let s = scratch();
    let (store, accounts) = engine_store(&s);
    let log = s.0.join("codex.jsonl");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py");
    let command = logging_agent(&s.0, "codex.py", &log, &format!("os.execv(sys.executable, [sys.executable, {fixture:?}, 'success'] + args)"));
    let _engine = crate::native::hooks::install(store.clone(), Some(command));
    let _policy = NoPolicy::set();
    let work = accounts.add("codex", "Work").unwrap();
    let teammate = |account: Option<String>| crate::engine::Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(s.0.join("work").to_string_lossy().into_owned()),
    };
    let start = |account: Option<String>| {
        let begun = crate::native::send(crate::native::Send {
            teammate: teammate(account),
            conversation: None,
            prompt: "Read this folder".into(),
            model: Some("synthetic-model".into()),
            effort: None,
        })
        .unwrap();
        let deadline = std::time::Instant::now() + crate::load_waits::UNDER_LOAD;
        while crate::native::owns(&begun.conversation_id) {
            assert!(std::time::Instant::now() < deadline, "the turn did not end");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let created = std::fs::read_to_string(store.write_path(&begun.conversation_id).unwrap()).unwrap();
        serde_json::from_str::<Value>(created.lines().next().unwrap()).unwrap()["metadata"]["resolvedRouteId"].as_str().unwrap().to_owned()
    };
    let one = start(Some(work.id.clone()));
    let two = start(None);

    let ran = logged(&log);
    assert_eq!(ran.len(), 2, "{ran:?}");
    let home = accounts.home_for(&work.id).to_string_lossy().into_owned();
    assert_eq!(ran[0]["env"]["CODEX_HOME"].as_str(), Some(home.as_str()));
    // The start with no account is the command as it was: nothing set, nothing taken away.
    assert_eq!(ran[1]["env"]["CODEX_HOME"].as_str(), std::env::var("CODEX_HOME").ok().as_deref());
    assert_eq!(ran[1]["env"]["OPENAI_API_KEY"].as_str(), std::env::var("OPENAI_API_KEY").ok().as_deref());
    assert_eq!(account_from_route(&one), Some(work.id.as_str()), "{one}");
    assert!(two.ends_with("-account:default"), "{two}");
}

#[test]
fn keeps_account_homes_out_of_backups_and_refuses_removal_while_that_account_is_in_use() {
    // case: keeps account homes out of backups and refuses removal while that account is in use
    let s = scratch();
    let busy: Arc<Mutex<Option<String>>> = Arc::default();
    let in_use = busy.clone();
    let accounts = Accounts::new(
        &s.0,
        Arc::new(|_, _, _, _| Ok(Box::new(|| {}) as Cancel)),
        Box::new(|| None),
        Box::new(move |_, id| in_use.lock().unwrap().as_deref() == Some(id)),
    );
    let a = accounts.add("codex", "Work").unwrap();
    let b = accounts.add("codex", "Personal").unwrap();
    *busy.lock().unwrap() = Some(a.id.clone());
    assert!(accounts.remove(&a.id).unwrap_err().contains("busy"));
    accounts.remove(&b.id).unwrap();
    assert_eq!(accounts.list().unwrap().into_iter().map(|x| x.id).collect::<Vec<_>>(), vec![a.id.clone()]);

    // A backup of this profile: the conversations go, the account's home and its sign-in stay on this machine.
    std::fs::create_dir_all(s.0.join("mission-ledger")).unwrap();
    std::fs::write(s.0.join("mission-ledger/m1.jsonl"), "{}\n").unwrap();
    std::fs::write(accounts.home_for(&a.id).join("auth.json"), "{\"token\":\"secret\"}").unwrap();
    let parent = s.0.join("backups");
    std::fs::create_dir_all(&parent).unwrap();
    let written = crate::profile_backup::write_backup(&s.0, &parent, "0.0.0", chrono::Local::now()).unwrap();
    let copied = crate::profile_backup::files_under(&written.folder.join("profile"), "");
    assert!(copied.iter().any(|f| f.contains("mission-ledger")), "{copied:?}");
    assert!(!copied.iter().any(|f| f.contains(FOLDER)), "{copied:?}");
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(written.folder.join(crate::profile_backup::BACKUP_MANIFEST)).unwrap()).unwrap();
    assert!(manifest["leftOut"].as_array().unwrap().iter().any(|v| v == FOLDER));
    let why = crate::profile_backup::LEFT_OUT.iter().find(|(k, _)| *k == "agent-accounts").map(|(_, why)| *why).unwrap_or_default();
    assert!(why.contains("never copied"), "{why}");
    assert!(!crate::profile_backup::BACKED_UP_FOLDERS.contains(&"agent-accounts"));
}

#[test]
fn starts_watches_and_stops_background_work_on_the_same_account() {
    // case: starts, watches and stops background work on the same account
    use crate::claude_background::{BackgroundSandbox, StartBackgroundOutcome, StartBackgroundRequest};
    let s = scratch();
    let (store, accounts) = engine_store(&s);
    let _policy = NoPolicy::set();
    let work = accounts.add("claude", "Work").unwrap();
    let log = s.0.join("claude.jsonl");
    let (program, prefix) = logging_agent(
        &s.0,
        "claude.py",
        &log,
        "print('[]' if args[:1] == ['agents'] else '' if args[:1] == ['stop'] else 'backgrounded \u{b7} abcdef12')",
    );
    let folder = s.0.join("work").to_string_lossy().into_owned();
    let mut background = crate::native::background_runs_for_with_program(&store, &program.to_string_lossy(), prefix, Vec::new());
    let started = background.start(StartBackgroundRequest {
        prompt: "Read this".into(),
        folder: folder.clone(),
        sandbox: BackgroundSandbox::ReadOnly,
        ask_each: false,
        conversation: None,
        teammate_id: None,
        model: None,
        effort: None,
        account: Some(work.id.clone()),
    });
    let StartBackgroundOutcome::Started(run) = started else { panic!("not started: {started:?}") };
    assert_eq!(run.account.as_deref(), Some(work.id.as_str()));
    background.poll_once().unwrap();
    assert!(background.stop("abcdef12"));

    let home = accounts.home_for(&work.id).to_string_lossy().into_owned();
    let ran = logged(&log);
    assert_eq!(ran.len(), 3, "{ran:?}");
    for call in &ran {
        assert_eq!(call["env"]["CLAUDE_CONFIG_DIR"].as_str(), Some(home.as_str()), "{call}");
    }
    assert!(ran[0]["args"].as_array().unwrap().iter().any(|a| a == "--bg"), "{}", ran[0]);
    assert_eq!(ran[1]["args"], json!(["agents", "--json", "--all"]));
    assert_eq!(ran[2]["args"], json!(["stop", "abcdef12"]));
}
