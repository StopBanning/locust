//! Settings › App › "The Claude app" (Electron `LocustMcpSettings.tsx` 0.740 and its handlers in `main/index.ts`):
//! Locust in the Claude app's tools in one press. Add turns the door on and writes the one entry; Update points a stale
//! entry here; Restart Claude closes and reopens it; Remove takes the entry out. Shown only where the Claude app is
//! installed, and on a managed computer with other apps off, only to remove an entry made before. The file work and
//! the words are `locust_core::claude_app`; this draws the card and runs its calls off the window's thread.

use super::*;
use locust_core::claude_app::{self, Connection, Connector, Entry};
use std::{path::PathBuf, sync::{Arc, Mutex}};

#[derive(Default)]
pub(super) struct Card {
    pub connection: Option<Connection>,
    /// What the last press said (Electron's `claudeSaid`).
    pub said: String,
    pub busy: bool,
    /// Other apps are off by the organization's policy.
    pub managed: bool,
}

pub(super) enum Answer {
    Read(Connection),
    Pressed(Connection, Option<String>),
}

static CONNECTOR: Mutex<Option<Arc<Connector>>> = Mutex::new(None);

/// Turns the door on for Add; tests answer for it themselves.
type DoorOn = dyn Fn() -> Option<locust_core::door::State> + Send + Sync;
static DOOR_ON: Mutex<Option<Arc<DoorOn>>> = Mutex::new(None);

fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// This machine's Claude app, pointed at this program's door bridge.
fn real_connector() -> Option<Connector> {
    let store = locust_core::store::Store::local().ok()?;
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)?;
    let exe = std::env::current_exe().ok()?.display().to_string();
    let connection = store.root().join(locust_core::door::CONNECTION_FILE).display().to_string();
    let mac = platform() == "darwin";
    Some(Connector {
        file: claude_app::config_file(platform(), std::env::var("APPDATA").ok().as_deref(), &home),
        entry: Entry { command: exe, args: vec!["--door-bridge".into(), connection] },
        marks_file: store.root().join("claude-app.json"),
        started_at: Box::new(move || if mac { claude_app::mac_started_at() } else { None }),
        relaunch: mac.then(|| Box::new(claude_app::mac_relaunch) as Box<dyn Fn() -> Result<(), String> + Send + Sync>),
        now: Box::new(|| chrono::Utc::now().timestamp_millis()),
    })
}

/// Tests never touch the real Claude app's settings: without a stand-in of their own, the card reads nothing.
fn connector() -> Option<Arc<Connector>> {
    let mut held = CONNECTOR.lock().unwrap_or_else(|e| e.into_inner());
    if cfg!(test) {
        return held.clone();
    }
    if held.is_none() {
        *held = real_connector().map(Arc::new);
    }
    held.clone()
}

#[cfg(test)]
pub(super) fn use_connector(connector: Connector, door_on: Arc<DoorOn>) {
    *CONNECTOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(connector));
    *DOOR_ON.lock().unwrap_or_else(|e| e.into_inner()) = Some(door_on);
}

fn door_on() -> Option<locust_core::door::State> {
    if cfg!(test) {
        let held = DOOR_ON.lock().unwrap_or_else(|e| e.into_inner()).clone();
        return held.and_then(|on| on());
    }
    crate::door_now(locust_core::engine_api::DoorChange::Enabled(true))
}

/// Read where Locust stands in the Claude app, as Settings opens.
pub(super) fn request(s: &mut State) {
    s.claude_app.managed = locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "otherApps").is_some();
    let Some(connector) = connector() else { return };
    if s.claude_app_job.is_pending() {
        return;
    }
    s.claude_app_job.request(connector, |c| Answer::Read(c.status()));
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(answer) = s.claude_app_job.take() else { return false };
    match answer {
        Answer::Read(connection) => s.claude_app.connection = Some(connection),
        Answer::Pressed(connection, said) => {
            s.claude_app.connection = Some(connection);
            s.claude_app.said = said.unwrap_or_default();
            s.claude_app.busy = false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum Press {
    Add,
    Remove,
    Restart,
}

fn press(s: &mut State, what: Press) {
    if s.claude_app.busy || (s.claude_app.managed && matches!(what, Press::Add)) {
        return;
    }
    let Some(connector) = connector() else { return };
    s.claude_app.said.clear();
    if matches!(what, Press::Add) {
        // Add turns the door on first; when it cannot, the Claude app is not changed.
        let state = door_on();
        let on = state.as_ref().is_some_and(|d| d.settings.enabled);
        let message = state.as_ref().and_then(|d| d.message.clone());
        if let Some(state) = state {
            s.settings_panel.door = Some(state);
        }
        if !on {
            s.claude_app.said = message.unwrap_or_else(|| "The local server could not be turned on, so the Claude app was not changed.".into());
            return;
        }
    }
    s.claude_app.busy = true;
    s.claude_app_job.request((connector, what), |(c, what)| {
        let done = match what {
            Press::Add => c.add(),
            Press::Remove => c.remove(),
            Press::Restart => c.restart(),
        };
        Answer::Pressed(c.status(), done.err())
    });
}

fn button(l: &crate::look::controls::Look, label: &str, key: &str, boxed: bool, disabled: bool) -> El {
    use crate::look::controls::{self, Tone};
    let b = if boxed { controls::button(l, label, Tone::Neutral, disabled) } else { controls::ghost_button(l, label) };
    b.key(key.to_owned())
}

/// The card, under the door's switches; nothing where the Claude app is not installed.
pub(super) fn section(s: &State, p: &Palette) -> Option<El> {
    let card = &s.claude_app;
    let connection = card.connection.as_ref()?;
    let added = matches!(connection, Connection::Added { .. });
    if matches!(connection, Connection::NoApp) || (card.managed && !added) {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut words = col()
        .grow(1.0)
        .gap(4.0)
        .child(text("The Claude app").font(l.ty.control).color(l.c("text-primary")))
        .child(text(claude_app::connection_words(connection)).font(l.ty.body).color(l.c("text-secondary")).wrap());
    if !card.said.is_empty() {
        words = words.child(text(card.said.clone()).font(l.ty.body).color(l.c("amber-text")).wrap().key("claude-app-said"));
    }
    let restart = matches!(connection, Connection::Added { restart: true, .. } | Connection::NotAdded { restart: true });
    let mut acts = row().gap(8.0).center_y();
    let locked = card.managed || card.busy;
    if matches!(connection, Connection::NotAdded { .. }) {
        acts = acts.child(button(&l, "Add to the Claude app", "claude-app-add", true, locked).on_press(|s: &mut State| press(s, Press::Add)));
    }
    if matches!(connection, Connection::Added { stale: true, .. }) {
        acts = acts.child(button(&l, "Update", "claude-app-update", true, locked).on_press(|s: &mut State| press(s, Press::Add)));
    }
    if restart {
        acts = acts.child(button(&l, "Restart Claude", "claude-app-restart", true, card.busy).on_press(|s: &mut State| press(s, Press::Restart)));
    }
    if added {
        acts = acts.child(button(&l, "Remove", "claude-app-remove", false, card.busy).on_press(|s: &mut State| press(s, Press::Remove)));
    }
    Some(row().w_full().gap(16.0).padding(10.0).center_y().key("claude-app-card").label("The Claude app").child(words).child(acts))
}
