//! The one line that says a new version is here (`UpdateBanner` in renderer `components/Screens.tsx`,
//! `an-update-says-it-reopens-by-itself`). It sits above the composer while no turn runs, offers the install, and never
//! installs on its own: the update service refuses while a mission is running, and the refusal is said beside it.
use super::*;
use locust_core::updates::{AppUpdateResponse, RESTART_AND_INSTALL, update_banner_line};

/// Keeps the window's copy of where the update stands in step with the app's update service (Electron's
/// `onAppUpdateState`), so a version that finished downloading in the background shows here without opening Settings.
pub(super) fn poll(s: &mut State) -> bool {
    let Some(svc) = s.settings_panel.update_service.as_ref() else {
        return false;
    };
    let now = svc.state();
    if now == s.settings_panel.update {
        return false;
    }
    s.settings_panel.update = now;
    true
}

fn install(s: &mut State) {
    s.settings_panel.update_refusal = None;
    let Some(svc) = s.settings_panel.update_service.clone() else {
        return;
    };
    match svc.install() {
        AppUpdateResponse::Ok(next) => {
            s.settings_panel.update = next;
            s.settings_panel.update_installing = true;
        }
        AppUpdateResponse::Err(err) => s.settings_panel.update_refusal = Some(err.message),
    }
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    if turn_running(s) {
        return None;
    }
    let installing = s.settings_panel.update_installing;
    let line = update_banner_line(Some(&s.settings_panel.update), installing)?;
    let l = crate::look::controls::Look::new(p.dark);
    let mut words = row()
        .grow(1.0)
        .shrink()
        .gap(4.0)
        .flex_wrap()
        .child(text(line).key("update-banner-line").font(type_::LABEL).color(p.text).wrap());
    if let Some(refusal) = &s.settings_panel.update_refusal {
        words = words.child(text(refusal.clone()).key("update-banner-refusal").font(type_::LABEL).color(p.amber).wrap());
    }
    let press = crate::look::controls::button(&l, RESTART_AND_INSTALL, crate::look::controls::Tone::Neutral, installing)
        .key("update-banner-install")
        .role("button")
        .on_press(|s: &mut State| {
            if !s.settings_panel.update_installing {
                install(s);
            }
        });
    Some(
        row()
            .key("update-banner")
            .role("status")
            .w_full()
            .gap(12.0)
            .px(10.0)
            .py(8.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .center_y()
            .child(words)
            .child(press),
    )
}

#[cfg(test)]
#[path = "update_banner_tests.rs"]
mod update_banner_tests;
