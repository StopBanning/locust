//! Settings › AI agents › "Let me start sessions on this computer from claude.ai" (Electron `RemoteControlSetting.tsx`):
//! the switch that owns Claude Code's Remote Control server, the line that says where it stands, and what Claude Code
//! printed, as it printed it. The process is `locust_core::remote_control`'s; the switch is turned off the window's
//! thread, since ending the server waits for it to close.

use super::*;
use locust_core::remote_control::{NEEDS_SIGN_IN, ONLY_THE_START, Phase, REMOTE_CONTROL_LABEL, RealLauncher, RemoteControl, RemoteControlState, says};
use std::{path::PathBuf, sync::{Arc, Mutex}};

static SERVICE: Mutex<Option<RemoteControl>> = Mutex::new(None);

/// This computer's server, in the folder on screen when it starts. Tests never run Claude Code: without a stand-in of
/// their own there is no switch.
fn service(s: &State) -> Option<RemoteControl> {
    let mut held = SERVICE.lock().unwrap_or_else(|e| e.into_inner());
    if cfg!(test) {
        return held.clone();
    }
    if held.is_none() {
        let folder = s.teammates.get(s.selected).and_then(|t| t.folder.clone()).map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir());
        *held = Some(RemoteControl::new(Arc::new(RealLauncher { store: s.data.clone() }), Box::new(move || folder.clone())));
    }
    held.clone()
}

#[cfg(test)]
pub(super) fn use_service(service: RemoteControl) {
    *SERVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(service);
}

/// Locust is quitting: the server ends with it, waited for.
pub(super) fn dispose() {
    let held = SERVICE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(service) = held {
        service.dispose();
    }
}

/// What the server is doing, read while Settings is open (Electron reads it every second).
pub(super) fn poll(s: &mut State) -> bool {
    if !s.settings_panel.open {
        return false;
    }
    let Some(service) = service(s) else { return false };
    let now = service.snapshot();
    if s.remote_control.as_ref() == Some(&now) {
        return false;
    }
    s.remote_control = Some(now);
    true
}

fn toggle(s: &mut State) {
    let Some(service) = service(s) else { return };
    let Some(state) = &s.remote_control else { return };
    if state.phase == Phase::Stopping || managed() {
        return;
    }
    let on = !state.enabled;
    std::thread::spawn(move || {
        service.set_enabled(on);
    });
}

/// Phone access off by the organization's policy turns this off too.
fn managed() -> bool {
    locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone").is_some()
}

pub(super) fn section(s: &State, p: &Palette) -> Option<El> {
    let state: &RemoteControlState = s.remote_control.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let managed = managed();
    let lede = format!("{} {NEEDS_SIGN_IN}", says(state));
    let switch = crate::look::controls::switch(&l, !managed && state.enabled, REMOTE_CONTROL_LABEL, managed || state.phase == Phase::Stopping)
        .key("remote-control-switch")
        .on_press(toggle);
    let mut out = col().w_full().gap(6.0).key("remote-control").child(
        row()
            .w_full()
            .gap(16.0)
            .padding(10.0)
            .center_y()
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(text(REMOTE_CONTROL_LABEL).font(l.ty.control).color(l.c("text-primary")))
                    .child(text(lede).font(l.ty.body).color(l.c("text-secondary")).wrap().role("status")),
            )
            .child(switch),
    );
    // What Claude Code printed has a row of its own, and only when there is some.
    let printed = |label: &str, words: &str, key: &str| {
        col()
            .w_full()
            .gap(4.0)
            .key(key.to_owned())
            .child(text(label.to_owned()).font(type_::LABEL).color(l.c("text-muted")))
            .child(text(words.to_owned()).font(type_::LABEL.mono()).color(l.c("text-primary")).wrap_anywhere())
    };
    let mut output = col().w_full().gap(8.0).px(10.0);
    let mut any = false;
    if !state.stdout.is_empty() {
        output = output.child(printed("Claude Code output", &state.stdout, "remote-control-output"));
        any = true;
    }
    if !state.stderr.is_empty() {
        output = output.child(printed("Claude Code errors", &state.stderr, "remote-control-errors"));
        any = true;
    }
    if let Some(problem) = &state.error {
        output = output.child(text(problem.clone()).font(type_::LABEL.mono()).color(l.c("amber-text")).wrap_anywhere().role("alert").key("remote-control-problem"));
        any = true;
    }
    if state.truncated {
        output = output.child(text(ONLY_THE_START).font(l.ty.body).color(l.c("text-muted")).wrap());
        any = true;
    }
    if any {
        out = out.child(output);
    }
    Some(out)
}
