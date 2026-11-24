//! Settings › AI agents › "AI agents & accounts" (Electron `Screens.tsx`, the runtimes page): each AI agent this Mac
//! has, its tag, its version, and what to do when it is not ready. The words come from `locust_core::agents`, held to
//! Electron's own answers.
//!
//! A row npm installs carries Install, running the line it shows (runtime-installer.ts); a row to sign in carries Sign
//! in. Each line stays on the row with Copy, to run by hand.
//!
//! Codex and Copilot, which npm installs and never updates, are kept current (runtime-updates.ts, core
//! `agent_updates`): the row says a newer version is out and what it waits for, offers Update, and says how an update
//! ended; a switch below the rows turns updating on its own on or off. All of it runs in jobs off the window's thread:
//! drawing reads memory only.

use super::*;
use locust_core::agent_updates as updates;
use locust_core::agents::{self, Found, RowStatus};
use locust_core::discovery_service;
use locust_core::runtime_order::{RuntimeEntry, keep_what_was_known, runtime_list_order};

pub(super) const HEADING: &str = "AI agents & accounts";
/// Electron's lede, minus "Claude Code and Codex can also use accounts you add here": named accounts are not built.
/// The lede (Screens.tsx), naming this computer as the machine names itself.
fn lede() -> String {
    format!("Each AI agent starts with this {}'s sign-in.", locust_core::machine_words::this_machine().computer)
}
/// Electron's word for word: the row has its Install button now (`install_part`).
const NONE_INSTALLED: &str = "None of these are on this machine yet. The quickest start is OpenCode: press its Install, no account, and its free model runs as soon as it is there. The others below are worth adding when you want a stronger model, and each needs its own sign-in.";
fn checking() -> String {
    format!("Checking each AI agent on this {}…", locust_core::machine_words::this_machine().computer)
}

/// What an Agent Client Protocol agent said it can do, in Electron's words (`agentCapabilities.ts` `LINES`).
const ABILITY_LINES: [(&str, &str); 6] = [
    ("continuesSessions", "Can continue an earlier session"),
    ("images", "Takes pictures in a message"),
    ("audio", "Takes sound in a message"),
    ("embeddedContext", "Takes a file\u{2019}s contents in a message"),
    ("mcpOverHttp", "Connects to connectors over the web (HTTP)"),
    ("mcpOverSse", "Connects to connectors over SSE"),
];

/// `agentCapabilityLines`: each thing the agent said, as a plain yes or no.
pub(super) fn ability_lines(said: &serde_json::Value) -> Vec<String> {
    ABILITY_LINES.iter().map(|(key, label)| format!("{label}: {}", if said[*key] == true { "yes" } else { "no" })).collect()
}

/// `agentCapabilityHeading`: the agent's own word, and when it said it, in whole days since (`Date.parse` reads an
/// ISO time or a bare date, which is midnight UTC).
pub(super) fn ability_heading(name: &str, said: &serde_json::Value, now: chrono::DateTime<chrono::Utc>) -> String {
    let told = said["toldAt"].as_str().and_then(|t| {
        chrono::DateTime::parse_from_rfc3339(t)
            .map(|at| at.with_timezone(&chrono::Utc))
            .ok()
            .or_else(|| chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|at| at.and_utc()))
    });
    let when = match told.map(|at| (now - at).num_milliseconds().div_euclid(86_400_000)) {
        None => "at its last run".to_owned(),
        Some(..=0) => "today".to_owned(),
        Some(1) => "yesterday".to_owned(),
        Some(days) => format!("{days} days ago"),
    };
    format!("What {name} said it can do ({when}):")
}

/// Ask every agent again, in the background. Tests never run real agents: they set the results themselves.
pub(super) fn request(s: &mut State) {
    if let Some(store) = &s.data {
        let from_store = locust_core::limits::limited_runtimes(store);
        if !from_store.is_empty()
            || !cfg!(test)
            || store.root().join("mission-ledger").exists()
            || store.electron().join("mission-ledger").exists()
        {
            s.limited_runtimes = from_store;
        }
    }
    s.limited_runtimes.retain(|_, said| {
        !locust_core::limits::limit_has_reset(said, chrono::Utc::now().timestamp_millis())
    });
    let Some(service) = discovery() else { return };
    if !s.settings_panel.agents_job.is_pending() {
        // A sweep that could not run leaves the list as it was (the service's generic DISCOVERY_FAILED).
        let held = s.settings_panel.agents.clone().unwrap_or_default();
        let first_start = opencode_first_start();
        s.settings_panel.agents_job.request((service, held, first_start), |(service, held, first_start)| {
            // Whether npm can run is asked with the agents, off the window's thread, and remembered (npm_check).
            #[cfg(not(test))]
            locust_core::npm_check::this_machine().present();
            let answer = service.get().unwrap_or(held);
            // OpenCode's first-start set-up, as the window finds it ready, not when a turn starts (index.ts:4771).
            if let Some(first_start) = &first_start {
                locust_core::opencode_first_start::after_discovery(&answer, first_start);
            }
            answer
        });
    }
}

#[cfg(test)]
thread_local! {
    static DISCOVERY_STAND_IN: std::cell::Cell<Option<&'static discovery_service::DiscoveryService>> = const { std::cell::Cell::new(None) };
}

/// A window test's own discovery, on fakes, for this test's thread only. With none, a test asks no agent at all.
#[cfg(test)]
pub(super) fn use_discovery(service: discovery_service::DiscoveryService) -> &'static discovery_service::DiscoveryService {
    let service: &'static discovery_service::DiscoveryService = Box::leak(Box::new(service));
    DISCOVERY_STAND_IN.with(|c| c.set(Some(service)));
    service
}

#[cfg(test)]
thread_local! {
    static FIRST_START_STAND_IN: std::cell::RefCell<Option<std::sync::Arc<locust_core::opencode_first_start::OpenCodeFirstStart>>> = const { std::cell::RefCell::new(None) };
}

/// A window test's own OpenCode set-up, on fakes, for this test's thread only. With none, a test sets nothing up.
#[cfg(test)]
pub(super) fn use_opencode_first_start(first_start: locust_core::opencode_first_start::OpenCodeFirstStart) {
    FIRST_START_STAND_IN.with(|c| *c.borrow_mut() = Some(std::sync::Arc::new(first_start)));
}

/// OpenCode's first-start set-up (opencode-first-start.ts), which a discovery answer may start.
#[cfg(test)]
fn opencode_first_start() -> Option<std::sync::Arc<locust_core::opencode_first_start::OpenCodeFirstStart>> {
    FIRST_START_STAND_IN.with(|c| c.borrow().clone())
}

#[cfg(not(test))]
fn opencode_first_start() -> Option<std::sync::Arc<locust_core::opencode_first_start::OpenCodeFirstStart>> {
    Some(locust_core::opencode_first_start::shared())
}

/// The agents' answer as the window asks for it (runtime-discovery.ts): shared while a sweep runs, reused briefly.
#[cfg(test)]
fn discovery() -> Option<&'static discovery_service::DiscoveryService> {
    DISCOVERY_STAND_IN.with(|c| c.get())
}

#[cfg(not(test))]
fn discovery() -> Option<&'static discovery_service::DiscoveryService> {
    Some(discovery_service::shared())
}

/// Something changed on the machine: the next ask is a new sweep, not the short-lived answer.
fn invalidate() {
    if let Some(service) = discovery() {
        service.invalidate();
    }
}

/// App.tsx `readAgainIfStale` and the late watch: an agent whose late check finished, still drawn as being checked,
/// is read back from the held answer -- once per finish, with no new check, and past giving up, since this answer is
/// exactly the end of that wait.
fn read_late_answers(s: &mut State) -> bool {
    let Some(service) = discovery() else { return false };
    s.settings_panel.agents_finished.extend(service.take_finished());
    let Some(list) = s.settings_panel.agents.as_deref() else { return false };
    let stale = agents::stale_checking(list, &s.settings_panel.agents_finished);
    if stale.is_empty() {
        return false;
    }
    for id in &stale {
        s.settings_panel.agents_finished.remove(id);
    }
    let Some(held) = service.held() else { return false };
    let kept = keep_known(list, held);
    s.settings_panel.agents = Some(kept);
    true
}

/// How long the window waits before asking agents again (App.tsx `RUNTIME_RECHECK_MS`).
pub(super) const RECHECK: std::time::Duration = std::time::Duration::from_secs(15);
/// Sweeps before the window stops asking an agent that will not answer (App.tsx `SWEEPS_BEFORE_GIVING_UP`).
pub(super) const SWEEPS_BEFORE_GIVING_UP: u8 = 4;

/// Check again: ask now, with the give-up budget back in full (App.tsx `resetDiscoveryBudget`).
fn check_again(s: &mut State) {
    s.settings_panel.agent_sweeps = 0;
    s.settings_panel.agents_gave_up = false;
    invalidate();
    request(s);
}

/// An installed agent whose check came back without an answer. One whose check is still RUNNING is not that (App.tsx,
/// 0.639): its own finish brings the answer, and counting it spent the give-up budget on a check that was only slow.
fn still_checking(agents: &[Found]) -> bool {
    agents.iter().any(|f| f.installed && !f.checking && matches!(f.status, agents::Probe::ProbeFailed | agents::Probe::Offline))
}

/// One sweep answered: ask again in a while while an agent is still unanswered, and stop after the budget, so the
/// screen stops saying "checking" and offers a way past an agent that will never answer.
fn after_a_sweep(s: &mut State) {
    let p = &mut s.settings_panel;
    p.agent_sweeps = p.agent_sweeps.saturating_add(1);
    if p.agents_gave_up || !still_checking(p.agents.as_deref().unwrap_or(&[])) {
        return;
    }
    if p.agent_sweeps >= SWEEPS_BEFORE_GIVING_UP {
        p.agents_gave_up = true;
        return;
    }
    #[cfg(not(test))]
    p.agents_recheck.request((), |()| std::thread::sleep(RECHECK));
}

/// The background check's answer, when it lands; and an install's latest line and its end.
pub(super) fn poll(s: &mut State) -> bool {
    if s.settings_panel.agents_recheck.take().is_some() && !s.settings_panel.agents_gave_up {
        request(s);
    }
    let mut changed = match s.settings_panel.agents_job.take() {
        Some(found) => {
            // A re-check does not make the list go backwards (App.tsx: `keepWhatWasKnown(held, response.runtimes)`).
            let kept = keep_known(s.settings_panel.agents.as_deref().unwrap_or(&[]), found);
            s.settings_panel.agents = Some(kept);
            after_a_sweep(s);
            true
        }
        None => false,
    };
    changed |= read_late_answers(s);
    read_artifacts(s);
    if let Some(found) = s.settings_panel.artifacts_job.take() {
        s.settings_panel.artifacts = found;
        changed = true;
    }
    let latest = s.agents_install.line.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if latest != s.agents_install.shown {
        s.agents_install.shown = latest;
        changed = true;
    }
    changed |= poll_updates(s);
    if let Some((id, outcome)) = s.agents_install_job.take() {
        // An install finishing is the one moment npm's answer can have changed: the next sweep asks it again.
        forget_npm();
        let installed = outcome.is_ok();
        s.agents_install.outcome = Some((id, outcome));
        // Installed: discovery looks again, and the row moves on to Sign in or ready.
        if installed {
            invalidate();
            request(s);
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
thread_local! {
    static HOME_STAND_IN: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// A window test's own home folder, for this test's thread only. With none, a test reads no one's CLI folders.
#[cfg(test)]
pub(super) fn use_home(home: std::path::PathBuf) {
    HOME_STAND_IN.with(|c| *c.borrow_mut() = Some(home));
}

#[cfg(test)]
fn home() -> Option<std::path::PathBuf> {
    HOME_STAND_IN.with(|c| c.borrow().clone())
}

#[cfg(not(test))]
fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// What the agents have of their own (App.tsx's `listRuntimeArtifacts`), read again whenever which agents are ready or
/// signed out changes -- not on every sweep -- off the window's thread.
fn read_artifacts(s: &mut State) {
    let Some(found) = s.settings_panel.agents.as_deref() else { return };
    let listed = locust_core::runtime_artifacts::listed_runtimes(found);
    if s.settings_panel.artifacts_for.as_ref() == Some(&listed) || s.settings_panel.artifacts_job.is_pending() {
        return;
    }
    let Some(home) = home() else { return };
    s.settings_panel.artifacts_for = Some(listed.clone());
    s.settings_panel.artifacts_job.request((listed, home), |(listed, home)| {
        let installed: Vec<&str> = listed.iter().map(String::as_str).collect();
        locust_core::runtime_artifacts::read_runtime_artifacts(&installed, &home)
    });
}

/// CliArtifacts.tsx under one agent: folded, with what is inside counted on the fold; open, the note and each one with
/// the tail of where it lives. Nothing when the agent has none.
fn artifacts_part(s: &State, l: &crate::look::controls::Look, f: &Found) -> Option<El> {
    use locust_core::runtime_artifacts::{LISTS_NOT_RUNS, fold_line, path_tail};
    let mine: Vec<&locust_core::runtime_artifacts::RuntimeArtifact> = s.settings_panel.artifacts.iter().filter(|a| a.runtime == f.id).collect();
    if mine.is_empty() {
        return None;
    }
    let open = s.settings_panel.artifacts_open.contains(&f.id);
    let id = f.id.clone();
    let mut part = col().gap(4.0).key(format!("agents-artifacts-{}", f.id)).child(
        text(fold_line(&mine))
            .font(l.ty.mono)
            .color(l.c("text-secondary"))
            .key(format!("agents-artifacts-fold-{}", f.id))
            .role("button")
            .label(format!("{}, {}", fold_line(&mine), if open { "open" } else { "folded" }))
            .self_start()
            .on_press(move |s: &mut State| {
                if !s.settings_panel.artifacts_open.remove(&id) {
                    s.settings_panel.artifacts_open.insert(id.clone());
                }
            }),
    );
    if !open {
        return Some(part);
    }
    part = part.child(text(LISTS_NOT_RUNS.to_owned()).font(l.ty.mono).color(l.c("text-muted")));
    for entry in mine {
        let mut line = row().gap(8.0).center_y().flex_wrap()
            .child(text(entry.kind.to_owned()).font(l.ty.mono).color(l.c("text-muted")))
            .child(text(entry.name.clone()).font(l.ty.body).color(l.c("text-primary")));
        if let Some(description) = &entry.description {
            line = line.child(text(description.clone()).font(l.ty.body).color(l.c("text-secondary")).wrap().shrink());
        }
        part = part.child(line.child(text(path_tail(&entry.path)).font(l.ty.mono).color(l.c("text-muted")).help(entry.path.display().to_string())));
    }
    Some(part)
}

/// What the keeping-current service is doing, as the window last saw it.
pub(super) struct Keeping {
    /// The service: None in a copy that never updates the machine's agents, and before the first poll decides.
    service: Option<std::sync::Arc<updates::Updates>>,
    /// Decided once: whether this launch may change the machine's agents (Electron's `heldHere`).
    decided: bool,
    pub held_here: bool,
    /// The saved file has been read: before that the switch does not know which way it is.
    read: bool,
    pub state: updates::State,
    /// Each kept agent's one line, worded when the state changed, so drawing never asks the clock.
    pub lines: std::collections::BTreeMap<String, String>,
    launched: std::time::Instant,
    last_tick: Option<std::time::Instant>,
}

impl Default for Keeping {
    fn default() -> Self {
        Keeping { service: None, decided: false, held_here: false, read: false, state: updates::State::default(), lines: Default::default(), launched: std::time::Instant::now(), last_tick: None }
    }
}

/// The service's answer to a job.
pub(super) type UpdatesAnswer = updates::State;

#[derive(Clone)]
enum Job {
    /// Read the saved file.
    Read,
    /// Look if a look is due, then update what is behind and not in use.
    Tick,
    /// The person pressed Update on this agent's row.
    Press(String),
    /// The person turned the switch.
    Automatic(bool),
}

#[cfg(test)]
thread_local! {
    static UPDATES_STAND_IN: std::cell::RefCell<Option<std::sync::Arc<updates::Updates>>> = const { std::cell::RefCell::new(None) };
}

/// A window test's own service, on fakes, for this test's thread only: no other window test sees it, and with none the
/// window has no service at all and no process or network is touched.
#[cfg(test)]
pub(super) fn use_updates(service: std::sync::Arc<updates::Updates>) {
    UPDATES_STAND_IN.with(|c| *c.borrow_mut() = Some(service));
}

#[cfg(test)]
fn make_service(_s: &State) -> Option<std::sync::Arc<updates::Updates>> {
    UPDATES_STAND_IN.with(|c| c.borrow().clone())
}

#[cfg(not(test))]
fn make_service(s: &State) -> Option<std::sync::Arc<updates::Updates>> {
    let root = s.data.as_ref()?.root().to_path_buf();
    Some(std::sync::Arc::new(updates::Updates::new(Box::new(updates::Machine::new(&root)))))
}

/// Whether this launch may change the machine's agents (mayUpdateAgents): the installed app only, never a scripted
/// launch. In a test, only with a stand-in service.
fn may_update() -> bool {
    if cfg!(test) {
        return true;
    }
    let args: Vec<String> = std::env::args().collect();
    updates::may_update_agents(&args, &|k| std::env::var(k).ok(), crate::os_hosts::packaged())
}

/// The first look waits until the app has been up a while, never on the way up.
fn first_look_after() -> std::time::Duration {
    if cfg!(test) { std::time::Duration::ZERO } else { std::time::Duration::from_millis(updates::FIRST_LOOK_AFTER_MS) }
}

fn word_lines(k: &mut Keeping) {
    let now = chrono::Local::now();
    k.lines = k.state.agents.iter().filter_map(|v| updates::update_line(Some(v), now).map(|line| (v.runtime.clone(), line))).collect();
}

fn start(s: &mut State, job: Job) {
    let Some(service) = s.agent_updates.service.clone() else { return };
    if s.agent_updates_job.is_pending() {
        return;
    }
    s.agent_updates_job.request((service, job), |(service, job)| match job {
        Job::Read => service.state(),
        Job::Tick => {
            service.tick();
            service.state()
        }
        Job::Press(id) => service.update_now(&id),
        Job::Automatic(on) => service.set_automatic(on),
    });
}

/// The service's answers and its clock: the first look 45 s after launch, then each hour (a look goes to the network only
/// when six hours have passed). Memory only; every read, process and install is in a job.
fn poll_updates(s: &mut State) -> bool {
    if !s.agent_updates.decided {
        s.agent_updates.decided = true;
        if may_update() {
            s.agent_updates.service = make_service(s);
        }
        s.agent_updates.held_here = s.agent_updates.service.is_none();
    }
    let Some(service) = s.agent_updates.service.clone() else { return false };
    let mut changed = false;
    if let Some(state) = s.agent_updates_job.take() {
        s.agent_updates.read = true;
        s.agent_updates.state = state;
        word_lines(&mut s.agent_updates);
        // An agent was updated: the machine is asked again, so the row's version and the models move on.
        if service.take_updated() {
            invalidate();
            request(s);
        }
        changed = true;
    } else if s.agent_updates_job.is_pending() {
        // What the service shows meanwhile ("Updating to ..."), from its memory.
        let seen = service.peek();
        if seen != s.agent_updates.state {
            s.agent_updates.state = seen;
            word_lines(&mut s.agent_updates);
            changed = true;
        }
    }
    if !s.agent_updates_job.is_pending() {
        let k = &s.agent_updates;
        let due = match k.last_tick {
            None => k.launched.elapsed() >= first_look_after(),
            Some(t) => t.elapsed().as_millis() as u64 >= updates::TICK_EVERY_MS,
        };
        if !k.read {
            start(s, Job::Read);
        } else if due {
            s.agent_updates.last_tick = Some(std::time::Instant::now());
            start(s, Job::Tick);
        }
    }
    changed
}

fn press_update(s: &mut State, id: String) {
    start(s, Job::Press(id));
}

fn turn_automatic(s: &mut State) {
    let on = !s.agent_updates.state.automatic;
    start(s, Job::Automatic(on));
}

/// Below the rows: the switch with its sentence, or -- in a copy that is not the installed Locust -- why there is none.
fn keep_current(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    let k = &s.agent_updates;
    if !k.decided {
        return None;
    }
    if k.held_here {
        return Some(text(updates::HELD_HERE_NOTE.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap().key("agents-held-here"));
    }
    if !k.read {
        return None;
    }
    let on = k.state.automatic;
    Some(
        row()
            .w_full()
            .gap(12.0)
            .center_y()
            .child(text(updates::keep_current_note(on).to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap().grow(1.0).shrink().key("agents-keep-current-note"))
            .child(crate::look::controls::switch(l, on, "Update Codex CLI and Copilot CLI on their own", s.agent_updates_job.is_pending()).key("agents-keep-current").on_press(turn_automatic)),
    )
}

/// The agent being installed from its row (Electron's InstallButton state).
#[derive(Default)]
pub(super) struct Installing {
    pub id: Option<String>,
    /// npm's latest line, written by the install's thread.
    pub line: std::sync::Arc<std::sync::Mutex<String>>,
    /// The line as last drawn.
    pub shown: String,
    pub outcome: Option<InstallAnswer>,
}

pub(super) type InstallAnswer = (String, locust_core::agent_install::Outcome);

static STAND_IN: std::sync::Mutex<Option<&'static locust_core::agent_install::Installer>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(super) fn use_installer(installer: &'static locust_core::agent_install::Installer) {
    *STAND_IN.lock().unwrap_or_else(|e| e.into_inner()) = Some(installer);
}

/// Install from the row (runtime-installer.ts): the row's own line, off the window's thread, one at a time. The window's
/// tests never run npm: without a stand-in of their own, nothing is installed.
fn install(s: &mut State, id: String, name: String) {
    // A greyed button still takes a press: while npm cannot be run, Install starts nothing.
    if s.agents_install_job.is_pending() || npm_missing() {
        return;
    }
    let stand_in = *STAND_IN.lock().unwrap_or_else(|e| e.into_inner());
    let installer = match stand_in {
        Some(i) => Some(i),
        None if cfg!(test) => None,
        None => Some(locust_core::agent_install::for_this_machine()),
    };
    let line = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    s.agents_install = Installing { id: Some(id.clone()), line: line.clone(), shown: String::new(), outcome: None };
    s.agents_install_job.request((id, name, line, installer), |(id, name, line, installer)| {
        let Some(installer) = installer else {
            return (id, Err(locust_core::agent_install::Failure { what: "No install runs in a test.".into(), next: "Nothing was changed.".into(), restart: false, command: None }));
        };
        let outcome = installer.install(&id, &name, None, &mut |said| *line.lock().unwrap_or_else(|e| e.into_inner()) = said.to_owned());
        (id, outcome)
    });
}

#[cfg(test)]
thread_local! {
    /// A window test's answer for npm, in place of this machine's: the tests never ask npm.
    static NPM_STAND_IN: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn set_npm_stand_in(present: Option<bool>) {
    NPM_STAND_IN.with(|c| c.set(present));
}

/// Whether npm was asked and cannot be run (Electron's `npmMissing`). Never asked counts as there.
pub(super) fn npm_missing() -> bool {
    #[cfg(test)]
    if let Some(present) = NPM_STAND_IN.with(|c| c.get()) {
        return !present;
    }
    locust_core::npm_check::this_machine().seen() == Some(false)
}

/// Opens a link in the person's browser; never from a test.
fn open_link(url: &'static str) {
    #[cfg(not(test))]
    {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(test)]
    let _ = url;
}

/// Whether Home defers agents that are not installed and not the free-start runtime (FirstLaunch.tsx `deferOthers`).
pub(super) fn home_should_defer(found: &[Found], installing: bool, others_open: bool) -> bool {
    let connected = found.iter().filter(|f| f.ready && agents::integration_of(&f.id) != agents::Integration::Planned).count();
    let checking_any = found.iter().any(|f| f.installed && f.checking);
    connected == 0 && !checking_any && !installing && !others_open
}

/// Whether a row stays visible when Home defers the rest: the free-start runtime or anything already on this machine.
fn home_kept_when_deferred(f: &Found) -> bool {
    f.id == agents::FREE_START_RUNTIME || f.installed
}

/// The short account/free-start note shown on a not-installed Home row (FirstLaunch.tsx `need.detail`).
fn home_need_detail(f: &Found) -> Option<String> {
    if f.installed {
        return None;
    }
    let facts = agents::runtime_install_facts(&f.id)?;
    if let Some(account) = facts.account {
        return Some(format!("needs {account}"));
    }
    match facts.install {
        agents::RuntimeInstall::Npm(_) => Some("no account needed".to_owned()),
        agents::RuntimeInstall::Vendor(url) => Some(format!("from {url}")),
    }
}

/// Panel-level explanation when Node.js is missing, shown regardless of whether another runtime is connected
/// (FirstLaunch.tsx `lc-installnote`: "Node.js is not on this machine, so the Install buttons cannot run." + Get Node.js).
fn nodejs_missing_note(l: &crate::look::controls::Look) -> El {
    row()
        .key("agents-nodejs-missing")
        .gap(8.0)
        .center_y()
        .flex_wrap()
        .child(
            text("Node.js is not on this machine, so the Install buttons cannot run.")
                .font(l.ty.body)
                .color(l.c("amber-text"))
                .wrap(),
        )
        .child(
            text("Get Node.js \u{2197}")
                .font(l.ty.label)
                .color(l.c("text-primary"))
                .px(6.0)
                .py(2.0)
                .radius(4.0)
                .hover(l.c("hover-surface"))
                .key("agents-get-nodejs")
                .role("button")
                .label("Get Node.js")
                .on_press(|_: &mut State| open_link("https://nodejs.org")),
        )
}

/// Drop npm's remembered answer (index.ts `npmSeen = undefined`).
fn forget_npm() {
    #[cfg(test)]
    NPM_STAND_IN.with(|c| c.set(None));
    locust_core::npm_check::this_machine().forget();
}

/// What a NOT INSTALLED row npm installs offers: Install, npm's latest line while it runs, and how it ended. While npm
/// cannot be run, Install is off and the row says why; its line stays there to run by hand.
fn install_part(s: &State, l: &crate::look::controls::Look, f: &Found, command: String) -> El {
    let mine = s.agents_install.id.as_deref() == Some(f.id.as_str());
    let running = s.agents_install_job.is_pending();
    let npm_missing = npm_missing();
    let mut part = col().gap(6.0);
    let (id, name) = (f.id.clone(), f.display_name.clone());
    let label = if running && mine { "Installing\u{2026}" } else { "Install" };
    part = part.child(crate::look::controls::button(l, label, crate::look::controls::Tone::Neutral, running || npm_missing).key(format!("agents-install-{}", f.id)).label(format!("Install {}", f.display_name)).self_start().on_press(move |s: &mut State| install(s, id.clone(), name.clone())));
    if npm_missing {
        part = part.child(text(locust_core::npm_check::NPM_MISSING.to_owned()).font(l.ty.body).color(l.c("amber-text")).wrap().key(format!("agents-install-npm-{}", f.id)));
    }
    if mine && running && !s.agents_install.shown.is_empty() {
        part = part.child(text(s.agents_install.shown.clone()).font(l.ty.mono).color(l.c("text-muted")).wrap_anywhere().key(format!("agents-install-line-{}", f.id)));
    }
    let failed = match &s.agents_install.outcome {
        Some((id, Err(failure))) if mine && id == &f.id && !running => Some(failure),
        _ => None,
    };
    let shown_command = failed.and_then(|f| f.command.clone()).unwrap_or(command);
    if let Some(failure) = failed {
        part = part
            .child(text(failure.what.clone()).font(l.ty.body).color(l.c("amber-text")).wrap().key(format!("agents-install-what-{}", f.id)))
            .child(text(failure.next.clone()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    part.child(copy_line(l, format!("agents-copy-install-{}", f.id), shown_command))
}

/// PublicRuntimeStatus's `status` word for a probe: the two rules read only "probe-failed" and "offline" as no answer.
fn status_word(p: agents::Probe) -> &'static str {
    use agents::Probe::*;
    match p {
        Ready => "ready",
        AuthRequired => "auth-required",
        Unsupported => "unsupported",
        Offline => "offline",
        ProbeFailed => "probe-failed",
        Missing => "not-installed",
        Unknown => "unknown",
    }
}

fn entry(f: &Found) -> RuntimeEntry {
    RuntimeEntry { id: f.id.clone(), display_name: f.display_name.clone(), installed: f.installed, version: f.version.clone(), status: status_word(f.status).to_owned() }
}

/// `keep_what_was_known` over discovery's answers: a runtime whose new answer is silence keeps the row it had, whole;
/// one that answered without a version keeps the version it printed before; every definite answer is taken.
fn keep_known(previous: &[Found], next: Vec<Found>) -> Vec<Found> {
    let before: Vec<RuntimeEntry> = previous.iter().map(entry).collect();
    let after: Vec<RuntimeEntry> = next.iter().map(entry).collect();
    keep_what_was_known(&before, &after)
        .into_iter()
        .zip(next)
        .map(|(kept, new)| {
            if kept.status != status_word(new.status) {
                return previous.iter().find(|p| p.id == kept.id).cloned().unwrap_or(new);
            }
            Found { version: kept.version, ..new }
        })
        .collect()
}

pub(super) fn ordered(found: &[Found], limited: &std::collections::HashMap<String, String>) -> Vec<(Found, RowStatus)> {
    let mut rows: Vec<(Found, RowStatus)> = found
        .iter()
        .map(|f| {
            (
                f.clone(),
                agents::row_status(
                    f,
                    agents::integration_of(&f.id),
                    false,
                    limited.get(&f.id).map(String::as_str),
                ),
            )
        })
        .collect();
    if found.iter().any(|f| f.installed) {
        // What can be used first, what is planned last, by name inside a band (Screens.tsx: runtimeListOrder).
        let tags: std::collections::HashMap<String, &'static str> = rows.iter().map(|(f, r)| (f.id.clone(), r.tag)).collect();
        let entries: Vec<RuntimeEntry> = rows.iter().map(|(f, _)| entry(f)).collect();
        let order = runtime_list_order(&entries, |e| tags.get(&e.id).copied().unwrap_or("NOT INSTALLED").to_owned());
        let mut by_id: std::collections::HashMap<String, (Found, RowStatus)> = rows.into_iter().map(|r| (r.0.id.clone(), r)).collect();
        rows = order.iter().filter_map(|e| by_id.remove(&e.id)).collect();
    } else {
        // Nothing installed: OpenCode, the free start, first.
        rows.sort_by_key(|(f, _)| f.id != agents::FREE_START_RUNTIME);
    }
    rows
}

/// The row's sentence. Electron's NOT INSTALLED sentence says "Press Install" because its row has an Install button
/// that runs npm; this window shows the line instead, so it says to run it. Everything else is Electron's word for word.
fn shown_detail(_f: &Found, status: &RowStatus) -> String {
    // Electron's own words: the row has Install now, so "Press Install" names a button that is there.
    status.detail.clone()
}

/// A line to run, with Copy (Electron's InstallCommand).
fn copy_line(l: &crate::look::controls::Look, key: String, line: String) -> El {
    let copied = line.clone();
    row()
        .gap(8.0)
        .center_y()
        .child(text(line).font(l.ty.mono).color(l.c("text-primary")))
        .child(
            text("Copy")
                .font(l.ty.label)
                .color(l.c("text-secondary"))
                .px(6.0)
                .py(2.0)
                .radius(4.0)
                .hover(l.c("hover-surface"))
                .key(key)
                .role("button")
                .label(format!("Copy: {copied}"))
                .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, copied.clone())),
        )
}

/// Where a row's sign-in button stands (SignInButton). Idle is no entry at all.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SignInRow {
    /// Sign in again was pressed: the row says it can leave the agent signed out, with Open it and Cancel.
    Confirm,
    /// The window is open: the row says to finish there, until Locust's window is back in front.
    Opened { again: bool, at: std::time::Instant },
}

/// Sign in from a button (runtime-sign-in.ts): a Terminal window in the home folder running the agent's own sign-in
/// command, against the program discovery found. Only the command its facts name is ever run. Signing in again runs
/// the CLI's own command for changing accounts (`claude auth login`, `codex login`).
pub(crate) fn sign_in(s: &mut State, id: &str, again: bool) {
    let Some(line) = agents::sign_in_command(id, again) else { return };
    let Some(args) = agents::sign_in_args(id, again) else { return };
    let Some(program) = line.strip_prefix("run ").unwrap_or(&line).split([',', ' ']).next().map(str::to_owned) else { return };
    if cfg!(test) {
        s.agents_signed_in.push(format!("{program} {}", args.join(" ")).trim().to_owned());
        s.agents_sign_in_said = None;
        s.agents_sign_in_rows.insert(id.to_owned(), SignInRow::Opened { again, at: std::time::Instant::now() });
        return;
    }
    let store = s.data.clone();
    let said = open_sign_in(&program, |p| crate::runtime::find_executable(p).ok(), |path| {
        let store = store.ok_or("Locust could not open its data folder.")?;
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).ok_or("Locust could not find your home folder.")?;
        let script = store.attachments_dir().map_err(|e| e.to_string())?.join(format!("sign-in-{}.command", locust_core::invariants::unique_nanos()));
        locust_core::runtime_terminal::open(path, &args, &home, &script, &store).map_err(|_| "Terminal could not be opened for the sign-in. Run the command shown in a terminal.".to_owned())
    });
    match said {
        Ok(_) => {
            s.agents_sign_in_said = None;
            s.agents_sign_in_rows.insert(id.to_owned(), SignInRow::Opened { again, at: std::time::Instant::now() });
        }
        Err(why) => {
            s.agents_sign_in_rows.remove(id);
            s.agents_sign_in_said = Some(why);
        }
    }
}

/// openSignIn's refusal: a program discovery did not find is never guessed at a path; the window opens only against
/// the one found.
fn open_sign_in(program: &str, find: impl Fn(&str) -> Option<std::path::PathBuf>, open: impl FnOnce(&std::path::Path) -> Result<(), String>) -> Result<(), String> {
    let path = find(program).ok_or_else(|| "Locust could not find this agent on the machine. Install it first, then sign in.".to_owned())?;
    open(&path)
}

/// Locust's window is back in front (SignInButton's focus listener, armed a second after the window opened): the
/// sign-in window was dealt with one way or the other, so each opened row offers its button again and every agent is
/// asked again, which turns a row that signed in ready by itself (App.tsx's SIGN_IN_OPENED_EVENT).
pub(super) fn window_focused(s: &mut State, now: std::time::Instant) {
    let before = s.agents_sign_in_rows.len();
    s.agents_sign_in_rows.retain(|_, row| !matches!(row, SignInRow::Opened { at, .. } if now.saturating_duration_since(*at) >= std::time::Duration::from_secs(1)));
    if s.agents_sign_in_rows.len() < before {
        check_again(s);
    }
}

/// A row's Sign in or Sign in again, as SignInButton draws it: the button, the warning with Open it and Cancel, or the
/// line saying to finish in the window that opened.
fn sign_in_part(s: &State, l: &crate::look::controls::Look, f: &Found, again: bool) -> El {
    use crate::look::controls::{Tone, button};
    let id = f.id.clone();
    match s.agents_sign_in_rows.get(&f.id) {
        Some(SignInRow::Opened { again: opened_again, .. }) if *opened_again == again => text(agents::sign_in_opened_line(&f.id, again))
            .font(l.ty.body)
            .color(l.c("text-secondary"))
            .wrap()
            .key(format!("agents-sign-in-opened-{}", f.id)),
        Some(SignInRow::Confirm) if again => {
            let open = id.clone();
            col()
                .gap(6.0)
                .key(format!("agents-sign-in-again-confirm-{}", f.id))
                .role("group")
                .label(format!("Sign in to {} again", f.display_name))
                .child(text(agents::sign_in_again_warning(&f.id, &f.display_name)).font(l.ty.body).color(l.c("text-secondary")).wrap())
                .child(
                    row()
                        .gap(8.0)
                        .child(button(l, "Open it", Tone::Neutral, false).key(format!("agents-sign-in-again-open-{}", f.id)).on_press(move |s: &mut State| sign_in(s, &open, true)))
                        .child(button(l, "Cancel", Tone::Neutral, false).key(format!("agents-sign-in-again-cancel-{}", f.id)).on_press(move |s: &mut State| {
                            s.agents_sign_in_rows.remove(&id);
                        })),
                )
        }
        _ if again => {
            let command = agents::sign_in_command(&f.id, true).unwrap_or_default();
            button(l, "Sign in again", Tone::Neutral, false)
                .key(format!("agents-sign-in-again-{}", f.id))
                .label(format!("Sign in to {} again. Opens a window that runs: {}. Sign in there as this account or another; the agent uses whichever signed in last.", f.display_name, command.strip_prefix("run ").unwrap_or(&command)))
                .self_start()
                .on_press(move |s: &mut State| {
                    s.agents_sign_in_rows.insert(id.clone(), SignInRow::Confirm);
                })
        }
        _ => button(l, "Sign in", Tone::Neutral, false).key(format!("agents-sign-in-{}", f.id)).self_start().on_press(move |s: &mut State| sign_in(s, &id, false)),
    }
}

/// What a row discovery gave up on says (FirstLaunch.tsx's `need.detail`).
const STUCK: &str = "did not answer its version check";

/// The title on that row's Check again, Electron's word for word: its "5 seconds" is Electron's own text (both apps
/// give a check ten, `PROBE_TIMEOUT`), kept so the two say the same.
fn stuck_title(name: &str) -> String {
    format!("{name} is on this machine but did not answer its version check in 5 seconds, four times. Check again asks once more.")
}

/// FirstLaunch.tsx's head note, for the two states an agent not answering is in: being asked, said once with a count
/// of what is on this machine; and given up on, with none of them answering.
fn head_note(checking_any: bool, none_answering: bool, on_this_machine: usize) -> Option<String> {
    if checking_any {
        Some(format!("checking {on_this_machine} on this machine"))
    } else if none_answering {
        Some(format!("{on_this_machine} on this machine \u{b7} none answering"))
    } else {
        None
    }
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    section_inner(s, p, false)
}

/// Home's expanded agents panel (FirstLaunch.tsx): defers unowned, uninstalled agents behind a press while nothing is
/// connected, uses "ready to install" instead of "NOT INSTALLED", and explains missing Node.js under the list.
pub(super) fn home_section(s: &State, p: &Palette) -> El {
    section_inner(s, p, true)
}

fn section_inner(s: &State, p: &Palette, for_home: bool) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut body = col()
        .key("agents-section")
        .gap(10.0)
        .child(text(HEADING.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(text(lede()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let Some(found) = &s.settings_panel.agents else {
        return body.child(text(checking()).font(l.ty.body).color(l.c("text-muted")));
    };
    if !found.is_empty() && !found.iter().any(|f| f.installed) {
        body = body.child(text(NONE_INSTALLED.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    let rows = ordered(found, &s.limited_runtimes);
    let gave_up = s.settings_panel.agents_gave_up;
    // FirstLaunch.tsx: while discovery is still asking, an installed agent not answering is a moment -- the head says it
    // once, with a count -- and after it has given up, a verdict, said on the row with the one repair the app can do.
    let waiting = |f: &Found, status: &RowStatus| f.installed && status.tag == "CHECKING";
    let checking_any = !gave_up && rows.iter().any(|(f, st)| waiting(f, st));
    let stuck_any = gave_up && rows.iter().any(|(f, st)| waiting(f, st));
    let ready_any = rows.iter().any(|(_, st)| st.selectable);
    if let Some(note) = head_note(checking_any, stuck_any && !ready_any, rows.iter().filter(|(f, _)| f.installed).count()) {
        body = body.child(text(note).font(l.ty.label).color(l.c("text-muted")).key("agents-head-note"));
    }
    let defer = for_home && home_should_defer(found, s.agents_install_job.is_pending(), s.home.others_open);
    let (drawn, deferred): (Vec<_>, Vec<_>) = if defer {
        rows.into_iter().partition(|(f, _)| home_kept_when_deferred(f))
    } else {
        (rows, Vec::new())
    };
    let mut list = col()
        .gap(0.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-card"));
    for (f, status) in drawn {
        let stuck = gave_up && waiting(&f, &status);
        let mut name = row()
            .gap(8.0)
            .center_y()
            .child(text(f.display_name.clone()).font(l.ty.control).color(l.c("text-primary")));
        if let Some(v) = &f.version {
            name = name.child(text(v.clone()).font(l.ty.mono).color(l.c("text-muted")));
        }
        let mut words = col().gap(4.0).grow(1.0).shrink().child(name);
        if for_home
            && let Some(need) = home_need_detail(&f)
        {
            words = words.child(text(need).font(l.ty.label).color(l.c("text-secondary")).wrap());
        }
        // "Signed in on this machine" is said once, in the lede: a row speaks only when its state is not the ordinary one.
        // Words are for results: a row still being asked says nothing of its own; one discovery gave up on says what
        // was tried, and offers Check again -- not Install, for what is already there.
        if stuck {
            let name = f.display_name.clone();
            words = words.child(text(STUCK.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap()).child(
                crate::look::controls::button(&l, "Check again", crate::look::controls::Tone::Neutral, false)
                    .key(format!("agents-row-check-again-{}", f.id))
                    .label(stuck_title(&name))
                    .help(stuck_title(&name))
                    .self_start()
                    .on_press(check_again),
            );
        } else if status.tag != "READY" && !waiting(&f, &status) {
            words = words.child(text(shown_detail(&f, &status)).font(l.ty.body).color(l.c("text-secondary")).wrap());
        }
        // Kept current (runtime-updates.ts): a newer version, an update under way, one that landed.
        if let Some(line) = s.agent_updates.lines.get(&f.id) {
            words = words.child(text(line.clone()).font(l.ty.body).color(l.c("text-secondary")).wrap().key(format!("agents-update-line-{}", f.id)));
        }
        // A newer version is out: the download is the person's to start (0.303).
        if updates::offers_update(s.agent_updates.state.view(&f.id)) {
            let id = f.id.clone();
            words = words.child(crate::look::controls::button(&l, "Update", crate::look::controls::Tone::Neutral, s.agent_updates_job.is_pending()).key(format!("agents-update-{}", f.id)).label(format!("Update {}", f.display_name)).self_start().on_press(move |s: &mut State| press_update(s, id.clone())));
        }
        // What an Agent Client Protocol agent told Locust it can do, at its last run (W12): read as the row is drawn,
        // never by asking the CLI again.
        if let Some(said) = locust_core::acp_capabilities::said_by(&f.id) {
            let mut abilities = col()
                .gap(2.0)
                .key(format!("agents-abilities-{}", f.id))
                .child(text(ability_heading(&f.display_name, &said, chrono::Utc::now())).font(l.ty.body).color(l.c("text-secondary")).wrap());
            for line in ability_lines(&said) {
                abilities = abilities.child(text(line).font(l.ty.body).color(l.c("text-secondary")).wrap().pl(12.0));
            }
            words = words.child(abilities);
        }
        if status.tag == "NOT INSTALLED"
            && let Some(command) = agents::install_command(&f.id)
        {
            words = words.child(install_part(s, &l, &f, command));
        }
        if status.tag == "SIGN IN"
            && let Some(line) = agents::sign_in_command(&f.id, false)
        {
            let command = line.strip_prefix("run ").unwrap_or(&line).to_owned();
            words = words.child(copy_line(&l, format!("agents-copy-sign-in-{}", f.id), command)).child(sign_in_part(s, &l, &f, false));
        }
        // Signed in: Sign in again, to change accounts with the CLI's own command (Colin, 2026-10-02).
        if matches!(status.tag, "READY" | "ACTIVE") && f.auth == agents::Auth::Authenticated && agents::sign_in_command(&f.id, true).is_some() {
            words = words.child(sign_in_part(s, &l, &f, true));
        }
        // What the person set up inside this CLI, folded under it, last on the row (Screens.tsx's CliArtifacts).
        if let Some(own) = artifacts_part(s, &l, &f) {
            words = words.child(own);
        }
        let tag_words = if for_home && status.tag == "NOT INSTALLED" { crate::boot_view::READY_TO_INSTALL } else { status.tag };
        let tag = text(tag_words.to_owned())
            .font(l.ty.label)
            .color(if status.selectable { l.c("text-primary") } else { l.c("text-secondary") })
            .px(8.0)
            .py(2.0)
            .radius(999.0)
            .border(1.0, l.c("border-card"))
            // A row with a sentence under its name is taller than the tag: the tag keeps its own height at the top.
            .self_start();
        list = list.child(
            row()
                .key(format!("agents-row-{}", f.id))
                .gap(12.0)
                .padding(12.0)
                .w_full()
                .child(words)
                .child(tag),
        );
    }
    if let Some(said) = &s.agents_sign_in_said {
        body = body.child(text(said.clone()).font(l.ty.body).color(l.c("amber-text")).wrap());
    }
    body = body.child(list);
    if !deferred.is_empty() {
        let flags: Vec<bool> = deferred.iter().map(|(f, _)| agents::install_command(&f.id).is_some()).collect();
        let sentence = crate::boot_view::deferred_others_sentence(&flags);
        body = body.child(
            text(sentence)
                .font(l.ty.label)
                .color(l.c("text-secondary"))
                .px(8.0)
                .py(4.0)
                .radius(6.0)
                .hover(l.c("hover-surface"))
                .key("home-agents-others")
                .role("button")
                .on_press(|s: &mut State| s.home.others_open = true),
        );
    }
    if npm_missing() {
        body = body.child(nodejs_missing_note(&l));
    }
    if let Some(keep) = keep_current(s, &l) {
        body = body.child(keep);
    }
    body.child(
        text("Check again")
            .font(l.ty.label)
            .color(l.c("text-secondary"))
            .px(6.0)
            .py(2.0)
            .radius(4.0)
            .hover(l.c("hover-surface"))
            .key("agents-check-again")
            .role("button")
            .on_press(check_again),
    )
}

/// Settings › AI agents with made-up discovery results, one of each kind of row: for `--snap` frames.
pub(super) fn scene(s: &mut State) {
    use locust_core::agents::{Auth, Probe};
    crate::settings_panel::scene(s);
    s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    let agent = |id: &str, name: &str, version: Option<&str>, auth: Auth, status: Probe| Found {
        id: id.into(),
        display_name: name.into(),
        installed: status != Probe::Missing,
        version: version.map(str::to_owned),
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: id == "antigravity",
        checking: false,
    };
    s.settings_panel.agents = Some(vec![
        agent("codex", "Codex CLI", Some("0.167.2"), Auth::Authenticated, Probe::Ready),
        agent("claude", "Claude Code", Some("2.1.322"), Auth::Unauthenticated, Probe::AuthRequired),
        agent("cursor", "Cursor Agent", Some("2026.11.10-731fb78"), Auth::Authenticated, Probe::Ready),
        agent("antigravity", "Antigravity", Some("1.2.14"), Auth::Authenticated, Probe::Ready),
        agent("opencode", "OpenCode", None, Auth::NotApplicable, Probe::Missing),
        agent("kilo", "Kilo", Some("7.8.18"), Auth::NotApplicable, Probe::Ready),
        agent("muse", "Muse Code", None, Auth::Unknown, Probe::Missing),
        agent("gemini", "Gemini CLI", Some("0.66.0"), Auth::Authenticated, Probe::Ready),
        agent("apple", "Apple Intelligence", None, Auth::NotApplicable, Probe::Ready),
    ]);
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

#[cfg(test)]
#[path = "agents_recheck_tests.rs"]
mod agents_recheck_tests;

#[cfg(test)]
#[path = "agents_probe_tests.rs"]
mod agents_probe_tests;

#[cfg(test)]
#[path = "agents_abilities_tests.rs"]
mod agents_abilities_tests;

#[cfg(test)]
#[path = "agents_opencode_first_start_tests.rs"]
mod agents_opencode_first_start_tests;

#[cfg(test)]
#[path = "agents_artifacts_fold_tests.rs"]
mod agents_artifacts_fold_tests;

#[cfg(test)]
#[path = "signing_in_is_a_button_tests.rs"]
mod signing_in_is_a_button_tests;

#[cfg(test)]
#[path = "npm_missing_install_tests.rs"]
mod npm_missing_install_tests;

#[cfg(test)]
#[path = "install_buttons_say_why_tests.rs"]
mod install_buttons_say_why_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::agents::{Auth, Probe};
    use serde_json::json;

    fn found_agent(id: &str, name: &str) -> Found {
        Found {
            id: id.into(),
            display_name: name.into(),
            installed: true,
            version: Some("1.2.14".into()),
            auth: Auth::Authenticated,
            ready: true,
            status: Probe::Ready,
            unsupported_note: None,
            through_cli: id == "antigravity",
            checking: false,
        }
    }

    #[test]
    fn ordered_marks_a_limited_agent_at_limit_with_its_words_and_places_it_in_band_two() {
        let out = "Antigravity's quota is used up · resets 2099-11-22T05:28:22Z";
        let limited: std::collections::HashMap<String, String> = [("antigravity".to_owned(), out.to_owned())].into_iter().collect();
        let found = vec![
            found_agent("antigravity", "Antigravity"),
            found_agent("codex", "Codex CLI"),
        ];
        let rows = ordered(&found, &limited);
        assert_eq!(rows[0].0.id, "codex");
        assert_eq!(rows[0].1.tag, "READY");
        assert_eq!(rows[1].0.id, "antigravity");
        assert_eq!(rows[1].1.tag, "AT LIMIT");
        assert_eq!(
            rows[1].1.detail,
            format!("Signed in, but the last run on it ended on your account's usage limit: {out}")
        );
    }

    #[test]
    fn section_draws_at_limit_from_the_ledger_and_lifts_it_once_the_reset_passes() {
        let root = std::env::temp_dir().join(format!(
            "locust-agents-section-limits-{}-{}",
            std::process::id(),
            locust_core::invariants::unique_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let ledger = store.root().join("mission-ledger");
        std::fs::create_dir_all(&ledger).unwrap();
        let future_out = "Antigravity's quota is used up · resets 2099-11-22T05:28:22Z";
        let write_out = |message: &str| {
            let lines = [
                json!({
                    "schemaVersion": 27,
                    "recordType": "mission.created",
                    "metadata": {
                        "missionId": "mission_out",
                        "runtime": "antigravity",
                        "resolvedRouteId": "antigravity-account:default"
                    }
                }),
                json!({
                    "schemaVersion": 27,
                    "recordType": "mission.event",
                    "event": {
                        "id": "e0",
                        "sequence": 1,
                        "type": "route.limit_detected",
                        "occurredAt": "2026-11-16T04:29:59.050Z",
                        "sourceAdapter": "antigravity",
                        "payload": { "kind": "quota-exhausted", "message": message }
                    }
                }),
            ];
            std::fs::write(
                ledger.join("mission_out.jsonl"),
                lines.iter().map(|r| format!("{r}\n")).collect::<String>(),
            )
            .unwrap();
        };
        write_out(future_out);

        let mut s = State::default();
        s.data = Some(store);
        s.settings_panel.agents = Some(vec![found_agent("antigravity", "Antigravity")]);
        request(&mut s);

        let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 780.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let texts = |rt: &rune::Runtime<State>| -> String {
            rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
        };
        let drawn = texts(&rt);
        assert!(drawn.contains("AT LIMIT"), "{drawn}");
        assert!(drawn.contains(future_out), "{drawn}");

        // Once the reset time the limit named is in the past, refreshing lifts AT LIMIT back to READY.
        write_out("Antigravity's quota is used up · resets 2020-11-22T05:28:22Z");
        rt.update(|s, _| request(s));
        rt.frame(0.05);
        let drawn = texts(&rt);
        assert!(!drawn.contains("AT LIMIT"), "{drawn}");
        assert!(drawn.contains("READY"), "{drawn}");

        let _ = std::fs::remove_dir_all(&root);
    }

    // `the-frame-pass.test.ts`, drawn: the window's own list, through `ordered` and `poll`.
    fn agent(id: &str, name: &str, version: Option<&str>, auth: Auth, status: Probe) -> Found {
        Found {
            id: id.into(),
            display_name: name.into(),
            installed: status != Probe::Missing,
            version: version.map(Into::into),
            auth,
            ready: status == Probe::Ready,
            status,
            unsupported_note: None,
            // Antigravity through its own CLI is READY; through the app it is EXPERIMENTAL, the band under test.
            through_cli: false,
            checking: false,
        }
    }
    fn window(agents: Vec<Found>) -> rune::Runtime<State> {
        let mut s = State::default();
        s.settings_panel.agents = Some(agents);
        let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }
    /// A re-check lands as discovery's answer does: through the job, then the window's poll.
    fn recheck(rt: &mut rune::Runtime<State>, answer: Vec<Found>) {
        rt.update(move |s, _| s.settings_panel.agents_job.request((), move |()| answer));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut landed = false;
        while !landed && std::time::Instant::now() < until {
            rt.update(|s, _| landed = poll(s));
            if !landed {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        assert!(landed, "the re-check never landed");
        rt.frame(0.05);
    }
    /// The drawn rows, top to bottom.
    fn drawn_order(rt: &rune::Runtime<State>) -> Vec<String> {
        let mut rows: Vec<(f32, String)> = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.key.and_then(|k| k.strip_prefix("agents-row-").map(str::to_owned)).map(|id| (e.rect[1], id)))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        rows.into_iter().map(|(_, id)| id).collect()
    }
    /// Every word drawn inside one agent's row.
    fn row_words(rt: &rune::Runtime<State>, id: &str) -> String {
        let all = rt.elements();
        let key = format!("agents-row-{id}");
        let Some(row) = all.iter().position(|e| e.key.as_deref() == Some(key.as_str())) else { return String::new() };
        let inside = |mut i: usize| loop {
            if i == row {
                return true;
            }
            match all[i].parent {
                Some(p) => i = p,
                None => return false,
            }
        };
        (0..all.len()).filter(|&i| !all[i].exiting && inside(i)).filter_map(|i| all[i].text.clone()).collect::<Vec<_>>().join(" | ")
    }

    // case: bands them ready, experimental, sign-in, not installed, planned
    // case: never leaves something that does not exist above something that does
    #[test]
    fn the_window_draws_what_can_be_used_first_and_what_does_not_exist_last() {
        let rt = window(vec![
            agent("gemini", "Gemini CLI", Some("0.66.0"), Auth::Authenticated, Probe::Ready),
            agent("codex", "Codex CLI", Some("0.167.2"), Auth::Authenticated, Probe::Ready),
            agent("antigravity", "Antigravity", Some("1.2.14"), Auth::Authenticated, Probe::Ready),
            agent("muse", "Muse Code", None, Auth::Unknown, Probe::Missing),
            agent("claude", "Claude Code", Some("2.1.322"), Auth::Unauthenticated, Probe::AuthRequired),
            agent("omniroute", "OmniRoute", Some("1.0"), Auth::Authenticated, Probe::Ready),
        ]);
        assert_eq!(drawn_order(&rt), ["codex", "antigravity", "claude", "muse", "gemini", "omniroute"]);
        let tags: Vec<String> = drawn_order(&rt).iter().map(|id| row_words(&rt, id)).collect();
        for (words, tag) in tags.iter().zip(["READY", "EXPERIMENTAL", "SIGN IN", "NOT INSTALLED", "PLANNED", "PLANNED"]) {
            assert!(words.ends_with(tag), "{words} should carry {tag}");
        }
    }

    // case: is stable between launches, so the list does not shuffle
    #[test]
    fn the_window_draws_one_band_by_name_whatever_order_discovery_answered_in() {
        let ready = |id: &str, name: &str| agent(id, name, Some("1.0"), Auth::Authenticated, Probe::Ready);
        let first = window(vec![ready("cursor", "Cursor Agent"), ready("claude", "Claude Code"), ready("codex", "Codex CLI")]);
        let again = window(vec![ready("codex", "Codex CLI"), ready("cursor", "Cursor Agent"), ready("claude", "Claude Code")]);
        assert_eq!(drawn_order(&first), ["claude", "codex", "cursor"]);
        assert_eq!(drawn_order(&again), drawn_order(&first));
    }

    // case: keeps a known result while the next probe is still out
    // case: keeps a version the new answer simply did not print
    #[test]
    fn a_recheck_still_out_keeps_the_row_it_had_and_a_version_not_printed_again() {
        let mut rt = window(vec![
            agent("claude", "Claude Code", Some("2.1.270"), Auth::Authenticated, Probe::Ready),
            agent("codex", "Codex CLI", Some("0.153.0"), Auth::Authenticated, Probe::Ready),
        ]);
        recheck(&mut rt, vec![agents::pending_found("claude", "Claude Code"), agent("codex", "Codex CLI", None, Auth::Authenticated, Probe::Ready)]);
        let claude = row_words(&rt, "claude");
        // A signed-in row carries Sign in again, to change accounts (SignInButton `again`).
        assert_eq!(claude, "Claude Code | 2.1.270 | Sign in again | READY");
        assert_eq!(row_words(&rt, "codex"), "Codex CLI | 0.153.0 | Sign in again | READY");
    }

    // case: takes every DEFINITE answer at once, including the unwelcome ones
    // case: lets a runtime that was out come back when it answers
    // case: passes through a runtime it has never seen
    #[test]
    fn a_recheck_draws_every_definite_answer_at_once_and_rows_it_never_saw() {
        let mut rt = window(vec![
            agent("claude", "Claude Code", Some("2.1.270"), Auth::Authenticated, Probe::Ready),
            agent("cursor", "Cursor Agent", Some("1.0"), Auth::Authenticated, Probe::Ready),
            agents::pending_found("opencode", "OpenCode"),
        ]);
        recheck(&mut rt, vec![
            agent("claude", "Claude Code", Some("2.1.270"), Auth::Unauthenticated, Probe::AuthRequired),
            agent("cursor", "Cursor Agent", None, Auth::Unknown, Probe::Missing),
            agent("opencode", "OpenCode", Some("1.18.27"), Auth::NotApplicable, Probe::Ready),
            agent("codex", "Codex CLI", Some("0.58.0"), Auth::Authenticated, Probe::Ready),
        ]);
        assert!(row_words(&rt, "claude").ends_with("SIGN IN"), "{}", row_words(&rt, "claude"));
        assert!(row_words(&rt, "cursor").ends_with("NOT INSTALLED"), "{}", row_words(&rt, "cursor"));
        assert!(!row_words(&rt, "cursor").contains("1.0"), "{}", row_words(&rt, "cursor"));
        assert_eq!(row_words(&rt, "opencode"), "OpenCode | 1.18.27 | READY");
        assert_eq!(row_words(&rt, "codex"), "Codex CLI | 0.58.0 | Sign in again | READY");
    }
}
