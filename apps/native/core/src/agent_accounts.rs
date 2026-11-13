//! Named Codex and Claude Code accounts (Electron `main/agent-accounts.ts` and `shared/agent-accounts.ts`). Each added
//! account is a name, an opaque id and a home folder of its own under Locust's data. Its sign-in is the agent's own
//! (`claude auth login`, `codex login`) run with that home, and a run on that account gets only that home in its
//! environment. Only names and ids live here: credential files belong to the agent's CLI, and nothing here reads,
//! copies or prints one.

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FOLDER: &str = "agent-accounts";
const LIST: &str = "accounts.json";

pub const UNREADABLE: &str = "The saved account list could not be read. No account was changed.";
pub const NAME_IT: &str = "Give the account a name of up to 80 characters.";
pub const NOT_SUPPORTED: &str = "This agent does not support another account.";
pub const CHOOSE_ONE: &str = "Choose an account from Settings. Nothing was started.";
pub const REMOVED: &str = "That account was removed. Choose another account in Settings. Nothing was started.";
pub const MOVED: &str = "The account folder has moved. Nothing was removed or started.";
pub const MOVED_ADD: &str = "The account folder has moved. Nothing was added.";
pub const BUSY_REMOVE: &str = "This agent is busy. Stop its runs and sign-in before removing the account.";
pub const BUSY_LOGIN: &str = "This agent is busy. Finish its runs before signing in again.";
pub const NO_SIGN_IN: &str = "The agent could not open its sign-in. Your other accounts are unchanged.";
pub const OTHER_AGENT: &str = "That account belongs to another agent. Nothing was started.";
pub const IN_PROGRESS: &str = "This account is being removed or signed in. Finish that first. Nothing was started.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn parse(runtime: &str) -> Option<Self> {
        match runtime {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
    /// The agent's own documented sign-in.
    pub fn login_args(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["auth", "login"],
            Self::Codex => &["login"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountState {
    New,
    Waiting,
    Ready,
    Failed,
}

impl AccountState {
    fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Waiting => "waiting",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "new" => Self::New,
            "waiting" => Self::Waiting,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub agent: Agent,
    pub name: String,
    pub state: AccountState,
}

pub fn is_account_id(id: &str) -> bool {
    id.len() == 40 && id.starts_with("account_") && id[8..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The usage and limits key: each account's are its own.
pub fn usage_key(runtime: &str, account: Option<&str>) -> String {
    account.map_or_else(|| runtime.to_owned(), |a| format!("{runtime}:{a}"))
}

/// The account a route names (`...-account:account_<32 hex>`), if any.
pub fn account_from_route(route: &str) -> Option<&str> {
    let (_, id) = route.rsplit_once("-account:")?;
    is_account_id(id).then_some(id)
}

/// What a run or sign-in on this account gets: its home, and none of the person's own keys.
pub fn env_for(agent: Agent, home: &Path) -> Vec<(&'static str, String)> {
    let home = home.to_string_lossy().into_owned();
    match agent {
        Agent::Claude => vec![("CLAUDE_CONFIG_DIR", home), ("ANTHROPIC_API_KEY", String::new()), ("ANTHROPIC_AUTH_TOKEN", String::new()), ("CLAUDE_CODE_OAUTH_TOKEN", String::new())],
        Agent::Codex => vec![
            ("CODEX_HOME", home.clone()),
            ("CODEX_SQLITE_HOME", home),
            ("OPENAI_API_KEY", String::new()),
            ("CODEX_API_KEY", String::new()),
            ("OPENAI_ACCESS_TOKEN", String::new()),
            ("CODEX_ACCESS_TOKEN", String::new()),
        ],
    }
}

/// Written readable by this user alone (Electron writes the list with mode 0600).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

fn name_of(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 80 || value.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return Err(NAME_IT.into());
    }
    Ok(trimmed.to_owned())
}

/// A sign-in in progress: how to stop it.
pub type Cancel = Box<dyn FnOnce() + Send>;
/// Starts the agent's sign-in with this home and environment; `done` is told once whether it worked.
pub type Login = dyn Fn(Agent, &Path, &[(&'static str, String)], Box<dyn FnOnce(bool) + Send>) -> Result<Cancel, String> + Send + Sync;

#[derive(Default)]
struct Live {
    /// Sign-ins under way, by account, each with its own number so a stale answer changes nothing.
    pending: HashMap<String, (u64, Option<Cancel>)>,
    next: u64,
    removing: HashSet<String>,
}

pub struct Accounts {
    root: PathBuf,
    login: Arc<Login>,
    refusal: Box<dyn Fn() -> Option<String> + Send + Sync>,
    in_use: Box<dyn Fn(Agent, &str) -> bool + Send + Sync>,
    live: Arc<Mutex<Live>>,
    /// One change at a time (Electron's serial queue).
    serial: Arc<Mutex<()>>,
}

impl Accounts {
    pub fn new(
        data: &Path,
        login: Arc<Login>,
        refusal: Box<dyn Fn() -> Option<String> + Send + Sync>,
        in_use: Box<dyn Fn(Agent, &str) -> bool + Send + Sync>,
    ) -> Self {
        Self { root: data.join(FOLDER), login, refusal, in_use, live: Arc::default(), serial: Arc::default() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn home_for(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    fn load_from(root: &Path, live: &Live) -> Result<Vec<Account>, String> {
        let text = match std::fs::read_to_string(root.join(LIST)) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(UNREADABLE.into()),
        };
        let value: Value = serde_json::from_str(&text).map_err(|_| UNREADABLE.to_owned())?;
        let list = value.as_array().ok_or(UNREADABLE)?;
        list.iter()
            .map(|a| {
                let id = a["id"].as_str().filter(|id| is_account_id(id)).ok_or(UNREADABLE)?;
                let agent = a["runtime"].as_str().and_then(Agent::parse).ok_or(UNREADABLE)?;
                let name = a["name"].as_str().ok_or(UNREADABLE)?;
                let mut state = a["state"].as_str().and_then(AccountState::parse).ok_or(UNREADABLE)?;
                // A sign-in this process is not running was interrupted.
                if state == AccountState::Waiting && !live.pending.contains_key(id) {
                    state = AccountState::Failed;
                }
                Ok(Account { id: id.to_owned(), agent, name: name.to_owned(), state })
            })
            .collect()
    }

    fn load(&self) -> Result<Vec<Account>, String> {
        let live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        Self::load_from(&self.root, &live)
    }

    fn save_to(root: &Path, accounts: &[Account]) -> Result<(), String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let list: Vec<Value> = accounts.iter().map(|a| json!({"id": a.id, "runtime": a.agent.as_str(), "name": a.name, "state": a.state.as_str()})).collect();
        let temp = root.join(format!("{LIST}.tmp"));
        write_private(&temp, Value::Array(list).to_string().as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, root.join(LIST)).map_err(|e| e.to_string())
    }

    fn set_state(root: &Path, live: &Live, id: &str, state: AccountState, only_if: Option<AccountState>) {
        if let Ok(mut accounts) = Self::load_from(root, live) {
            for a in accounts.iter_mut().filter(|a| a.id == id && only_if.is_none_or(|s| a.state == s)) {
                a.state = state;
            }
            let _ = Self::save_to(root, &accounts);
        }
    }

    /// The account folder, refused when it or the folder holding it is a link or has moved.
    fn owned_home(&self, id: &str) -> Result<PathBuf, String> {
        let home = self.home_for(id);
        let link = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(true);
        if link(&self.root) || link(&home) {
            return Err(MOVED.into());
        }
        let real_root = std::fs::canonicalize(&self.root).map_err(|_| MOVED.to_owned())?;
        let real_home = std::fs::canonicalize(&home).map_err(|_| MOVED.to_owned())?;
        if real_home.parent() != Some(real_root.as_path()) {
            return Err(MOVED.into());
        }
        Ok(home)
    }

    fn allowed(&self) -> Result<(), String> {
        (self.refusal)().map_or(Ok(()), Err)
    }

    fn get(&self, id: &str) -> Result<Account, String> {
        if !is_account_id(id) {
            return Err(CHOOSE_ONE.into());
        }
        self.load()?.into_iter().find(|a| a.id == id).ok_or_else(|| REMOVED.to_owned())
    }

    pub fn list(&self) -> Result<Vec<Account>, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        self.load()
    }

    pub fn add(&self, runtime: &str, name: &str) -> Result<Account, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        self.allowed()?;
        let agent = Agent::parse(runtime).ok_or(NOT_SUPPORTED)?;
        let mut accounts = self.load()?;
        let id = format!("account_{}", crate::native::uuid_v4().map_err(|e| e.to_string())?.replace('-', ""));
        let account = Account { id, agent, name: name_of(name)?, state: AccountState::New };
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        if std::fs::symlink_metadata(&self.root).map(|m| m.file_type().is_symlink()).unwrap_or(true) {
            return Err(MOVED_ADD.into());
        }
        let home = self.home_for(&account.id);
        std::fs::create_dir(&home).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        accounts.push(account.clone());
        Self::save_to(&self.root, &accounts)?;
        Ok(account)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        self.allowed()?;
        let account = self.get(id)?;
        let next = name_of(name)?;
        let mut accounts = self.load()?;
        for a in accounts.iter_mut().filter(|a| a.id == account.id) {
            a.name = next.clone();
        }
        Self::save_to(&self.root, &accounts)
    }

    /// Removal is allowed whatever the policy says: it only takes something away.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let account = self.get(id)?;
        self.live.lock().unwrap_or_else(|e| e.into_inner()).removing.insert(account.id.clone());
        let result = (|| {
            let pending = self.live.lock().unwrap_or_else(|e| e.into_inner()).pending.contains_key(&account.id);
            if pending || (self.in_use)(account.agent, &account.id) {
                return Err(BUSY_REMOVE.to_owned());
            }
            let home = self.owned_home(&account.id)?;
            // The removal is saved before the folder goes: a failed delete leaves an orphan, never a dangling account.
            let accounts: Vec<Account> = self.load()?.into_iter().filter(|a| a.id != account.id).collect();
            Self::save_to(&self.root, &accounts)?;
            let _ = std::fs::remove_dir_all(home);
            Ok(())
        })();
        self.live.lock().unwrap_or_else(|e| e.into_inner()).removing.remove(&account.id);
        result
    }

    pub fn login(&self, id: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        self.allowed()?;
        let account = self.get(id)?;
        if self.live.lock().unwrap_or_else(|e| e.into_inner()).pending.contains_key(&account.id) {
            return Ok(());
        }
        if (self.in_use)(account.agent, &account.id) {
            return Err(BUSY_LOGIN.into());
        }
        let home = self.owned_home(&account.id)?;
        let job = {
            let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
            live.next += 1;
            let job = live.next;
            live.pending.insert(account.id.clone(), (job, None));
            Self::set_state(&self.root, &live, &account.id, AccountState::Waiting, None);
            job
        };
        let (live, root, account_id) = (self.live.clone(), self.root.clone(), account.id.clone());
        let done: Box<dyn FnOnce(bool) + Send> = Box::new(move |ok| {
            let mut live = live.lock().unwrap_or_else(|e| e.into_inner());
            if live.pending.get(&account_id).map(|(j, _)| *j) != Some(job) {
                return;
            }
            live.pending.remove(&account_id);
            Self::set_state(&root, &live, &account_id, if ok { AccountState::Ready } else { AccountState::Failed }, None);
        });
        match (self.login)(account.agent, &home, &env_for(account.agent, &home), done) {
            Ok(cancel) => {
                let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
                if let Some((j, slot)) = live.pending.get_mut(&account.id)
                    && *j == job
                {
                    *slot = Some(cancel);
                }
                Ok(())
            }
            Err(_) => {
                let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
                live.pending.remove(&account.id);
                Self::set_state(&self.root, &live, &account.id, AccountState::Failed, None);
                Err(NO_SIGN_IN.into())
            }
        }
    }

    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let account = self.get(id)?;
        let cancel = {
            let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
            let cancel = live.pending.remove(&account.id).and_then(|(_, c)| c);
            Self::set_state(&self.root, &live, &account.id, AccountState::Failed, Some(AccountState::Waiting));
            cancel
        };
        if let Some(cancel) = cancel {
            cancel();
        }
        Ok(())
    }

    /// The environment a run on `account` adds to its launch: nothing without an account, so the launch stays
    /// exactly as it was.
    pub fn launch_env(&self, runtime: &str, account: Option<&str>) -> Result<Vec<(&'static str, String)>, String> {
        let Some(account) = account else { return Ok(vec![]) };
        self.allowed()?;
        let found = self.get(account)?;
        if found.agent.as_str() != runtime {
            return Err(OTHER_AGENT.into());
        }
        let home = self.owned_home(&found.id)?;
        let live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        if live.removing.contains(&found.id) || live.pending.contains_key(&found.id) {
            return Err(IN_PROGRESS.into());
        }
        Ok(env_for(found.agent, &home))
    }

    /// Locust is quitting: every sign-in under way stops.
    pub fn dispose(&self) {
        let jobs: Vec<Cancel> = self.live.lock().unwrap_or_else(|e| e.into_inner()).pending.drain().filter_map(|(_, (_, c))| c).collect();
        for cancel in jobs {
            cancel();
        }
    }
}

static SHARED: Mutex<Option<HashMap<PathBuf, Arc<Accounts>>>> = Mutex::new(None);

/// Shares one live `Accounts` handle per data folder so runs and Settings see the same pending sign-ins and removals.
pub fn register(accounts: Arc<Accounts>) -> Arc<Accounts> {
    if let Some(data) = accounts.root().parent() {
        let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(HashMap::new).insert(data.to_path_buf(), accounts.clone());
    }
    accounts
}

pub fn registered(data: &Path) -> Option<Arc<Accounts>> {
    let guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().and_then(|m| m.get(data).cloned())
}

pub fn shared(
    data: &Path,
    login: Arc<Login>,
    refusal: Box<dyn Fn() -> Option<String> + Send + Sync>,
    in_use: Box<dyn Fn(Agent, &str) -> bool + Send + Sync>,
) -> Arc<Accounts> {
    let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.entry(data.to_path_buf())
        .or_insert_with(|| Arc::new(Accounts::new(data, login, refusal, in_use)))
        .clone()
}

/// The agent's own sign-in, run for real: hidden, with its home, every line of its output discarded, and only whether
/// it worked reported (Electron's `startAccountLogin`).
pub fn start_login(program: &Path, agent: Agent, home: &Path, env: &[(&'static str, String)], done: Box<dyn FnOnce(bool) + Send>) -> Result<Cancel, String> {
    use std::process::{Command, Stdio};
    let mut command = Command::new(program);
    command.args(agent.login_args()).current_dir(home).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    crate::runtime::process::own_group(&mut command);
    let mut child = command.spawn().map_err(|_| NO_SIGN_IN.to_owned())?;
    let pid = child.id();
    std::thread::spawn(move || {
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        done(ok);
    });
    Ok(Box::new(move || crate::runtime::process::kill_process(pid)))
}

#[cfg(test)]
#[path = "agent_accounts_tests.rs"]
mod tests;
