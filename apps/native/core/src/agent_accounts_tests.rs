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
