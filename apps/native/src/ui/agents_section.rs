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

pub(super) const HEADING: &str = "AI agents & accounts";
/// Electron's lede, minus "Claude Code and Codex can also use accounts you add here": named accounts are not built.
const LEDE: &str = "Each AI agent starts with this Mac's sign-in.";
/// Electron's, with "press its Install" made "run its line": this window has no Install button yet (see `shown_detail`).
const NONE_INSTALLED: &str = "None of these are on this machine yet. The quickest start is OpenCode: run its line below, no account, and its free model runs as soon as it is there. The others below are worth adding when you want a stronger model, and each needs its own sign-in.";
const CHECKING: &str = "Checking each AI agent on this Mac…";

/// Ask every agent again, in the background. Tests never run real agents: they set the results themselves.
pub(super) fn request(s: &mut State) {
    #[cfg(not(test))]
    if !s.settings_panel.agents_job.is_pending() {
        s.settings_panel.agents_job.request((), |()| agents::discover());
    }
    #[cfg(test)]
    let _ = s;
}

/// The background check's answer, when it lands; and an install's latest line and its end.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = match s.settings_panel.agents_job.take() {
        Some(found) => {
            s.settings_panel.agents = Some(found);
            true
        }
        None => false,
    };
    let latest = s.agents_install.line.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if latest != s.agents_install.shown {
        s.agents_install.shown = latest;
        changed = true;
    }
    changed |= poll_updates(s);
    if let Some((id, outcome)) = s.agents_install_job.take() {
        let installed = outcome.is_ok();
        s.agents_install.outcome = Some((id, outcome));
        // Installed: discovery looks again, and the row moves on to Sign in or ready.
        if installed {
            request(s);
        }
        changed = true;
    }
    changed
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
    if s.agents_install_job.is_pending() {
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

/// What a NOT INSTALLED row npm installs offers: Install, npm's latest line while it runs, and how it ended.
fn install_part(s: &State, l: &crate::look::controls::Look, f: &Found, command: String) -> El {
    let mine = s.agents_install.id.as_deref() == Some(f.id.as_str());
    let running = s.agents_install_job.is_pending();
    let mut part = col().gap(6.0);
    let (id, name) = (f.id.clone(), f.display_name.clone());
    let label = if running && mine { "Installing\u{2026}" } else { "Install" };
    part = part.child(crate::look::controls::button(l, label, crate::look::controls::Tone::Neutral, running).key(format!("agents-install-{}", f.id)).label(format!("Install {}", f.display_name)).self_start().on_press(move |s: &mut State| install(s, id.clone(), name.clone())));
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

/// RUNTIME_BAND, then the name: what can be used first, what is planned last (runtimeListOrder).
fn band(tag: &str) -> u8 {
    match tag {
        "READY" => 0,
        "EXPERIMENTAL" => 1,
        "SIGN IN" | "AT LIMIT" => 2,
        "NOT INSTALLED" => 3,
        "PLANNED" => 4,
        _ => 3,
    }
}

pub(super) fn ordered(found: &[Found]) -> Vec<(Found, RowStatus)> {
    let mut rows: Vec<(Found, RowStatus)> = found
        .iter()
        .map(|f| (f.clone(), agents::row_status(f, agents::integration_of(&f.id), false, None)))
        .collect();
    if found.iter().any(|f| f.installed) {
        rows.sort_by(|(a, ra), (b, rb)| {
            band(ra.tag).cmp(&band(rb.tag)).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
        });
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

/// Sign in from a button (runtime-sign-in.ts): a Terminal window in the home folder running the agent's own sign-in
/// command, against the program discovery found. Only the command its facts name is ever run. Check again reads it
/// back once the person has signed in there.
fn sign_in(s: &mut State, id: &str) {
    let Some(line) = agents::sign_in_command(id, false) else { return };
    let words: Vec<String> = line.strip_prefix("run ").unwrap_or(&line).split(',').next().unwrap_or("").split_whitespace().map(str::to_owned).collect();
    let Some((program, args)) = words.split_first() else { return };
    if cfg!(test) {
        s.agents_signed_in.push(format!("{program} {}", args.join(" ")).trim().to_owned());
        return;
    }
    let said = (|| {
        let path = crate::runtime::find_executable(program).map_err(|_| "Locust could not find this agent on the machine. Install it first, then sign in.".to_owned())?;
        let store = s.data.clone().ok_or("Locust could not open its data folder.")?;
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).ok_or("Locust could not find your home folder.")?;
        let script = store.attachments_dir().map_err(|e| e.to_string())?.join(format!("sign-in-{}.command", locust_core::invariants::unique_nanos()));
        locust_core::runtime_terminal::open(&path, args, &home, &script, &store).map_err(|_| "Terminal could not be opened for the sign-in. Run the command shown in a terminal.".to_owned())
    })();
    s.agents_sign_in_said = said.err();
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut body = col()
        .key("agents-section")
        .gap(10.0)
        .child(text(HEADING.to_owned()).font(type_::TITLE).color(p.text))
        .child(text(LEDE.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let Some(found) = &s.settings_panel.agents else {
        return body.child(text(CHECKING.to_owned()).font(l.ty.body).color(l.c("text-muted")));
    };
    if !found.is_empty() && !found.iter().any(|f| f.installed) {
        body = body.child(text(NONE_INSTALLED.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    let mut list = col()
        .gap(0.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-card"));
    for (f, status) in ordered(found) {
        let mut name = row()
            .gap(8.0)
            .center_y()
            .child(text(f.display_name.clone()).font(l.ty.control).color(l.c("text-primary")));
        if let Some(v) = &f.version {
            name = name.child(text(v.clone()).font(l.ty.mono).color(l.c("text-muted")));
        }
        let mut words = col().gap(4.0).grow(1.0).shrink().child(name);
        // "Signed in on this machine" is said once, in the lede: a row speaks only when its state is not the ordinary one.
        if status.tag != "READY" {
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
        if status.tag == "NOT INSTALLED"
            && let Some(command) = agents::install_command(&f.id)
        {
            words = words.child(install_part(s, &l, &f, command));
        }
        if status.tag == "SIGN IN"
            && let Some(line) = agents::sign_in_command(&f.id, false)
        {
            let command = line.strip_prefix("run ").unwrap_or(&line).to_owned();
            let id = f.id.clone();
            words = words
                .child(copy_line(&l, format!("agents-copy-sign-in-{}", f.id), command))
                .child(crate::look::controls::button(&l, "Sign in", crate::look::controls::Tone::Neutral, false).key(format!("agents-sign-in-{}", f.id)).self_start().on_press(move |s: &mut State| sign_in(s, &id)));
        }
        let tag = text(status.tag.to_owned())
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
            .on_press(request),
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
    ]);
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}
