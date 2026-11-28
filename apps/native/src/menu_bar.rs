//! The menu bar icon (Electron `main/index.ts`: `trayIcon`, `ensureTray`, `toTray`; the-menu-bar-uses-a-template).
//!
//! Everything here is the same on every computer and runs in tests anywhere: which picture the icon uses and whether
//! macOS tints it as a template, the menu's items and words, what each item does, and when the menu is rebuilt. The
//! icon on the screen is this computer's (`os_hosts.rs`): an `NSStatusItem` on a Mac, Rune's tray elsewhere.

use locust_core::quit_guard::{tray_line, tray_menu_labels};

/// Electron's `resources/trayTemplate.png`: a 16-point black-and-clear picture that macOS tints for a light or dark
/// menu bar. Shipped inside the program rather than beside it.
pub const TEMPLATE_PNG: &[u8] = include_bytes!("../assets/tray/trayTemplate.png");
/// Its Retina representation, `trayTemplate@2x.png` (32 pixels for the same 16 points).
pub const TEMPLATE_PNG_2X: &[u8] = include_bytes!("../assets/tray/trayTemplate@2x.png");
/// The old coloured icon the tray uses off a Mac, `resources/icon-32.png`.
pub const ICON_32_PNG: &[u8] = include_bytes!("../assets/tray/icon-32.png");

/// The picture the icon is made from (`trayIcon`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrayImage {
    /// Electron's file name for it.
    pub file: &'static str,
    pub png: &'static [u8],
    /// The @2x representation, kept with the 1x one: resizing would throw the Retina picture away.
    pub retina: Option<&'static [u8]>,
    /// Marked as a template, so macOS tints it (only ever on a Mac).
    pub template: bool,
}

/// `trayIcon`, for `platform` as Node names it (`darwin`, `linux`, `win32`): on a Mac the template image with its @2x
/// representation, marked as a template; elsewhere the old icon, never resized. Windows takes the 32-pixel PNG
/// rather than Electron's `icon.ico`, because Rune's tray takes pixels and the system scales them.
pub fn tray_image(platform: &str) -> TrayImage {
    if platform == "darwin" {
        return TrayImage { file: "trayTemplate.png", png: TEMPLATE_PNG, retina: Some(TEMPLATE_PNG_2X), template: true };
    }
    TrayImage { file: "icon-32.png", png: ICON_32_PNG, retina: None, template: false }
}

/// A menu item's id, which is what comes back when it is picked.
pub const OPEN: &str = "open";
pub const AWAY: &str = "away";
pub const DUE: &str = "due";
pub const QUIT: &str = "quit";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub id: &'static str,
    pub label: String,
    pub enabled: bool,
}

/// The tray's menu (`Menu.buildFromTemplate` in `ensureTray`): Open Locust; the "since you were away" line when there
/// is one, which also opens the window; the next routine due, said but not pressable; Quit.
pub fn tray_menu(next_due: &str, away: Option<&str>) -> Vec<MenuItem> {
    let [open, due, quit] = tray_menu_labels(next_due);
    let mut items = vec![MenuItem { id: OPEN, label: open, enabled: true }];
    if let Some(away) = away {
        items.push(MenuItem { id: AWAY, label: away.to_owned(), enabled: true });
    }
    items.push(MenuItem { id: DUE, label: due, enabled: false });
    items.push(MenuItem { id: QUIT, label: quit, enabled: true });
    items
}

/// What the tray's items reach: the window, the tray itself, and the app.
pub trait TrayDoor {
    /// `leaveTray`: the icon goes; the next tray starts without a menu.
    fn leave_tray(&self);
    fn show_window(&self);
    fn focus_window(&self);
    /// The person's own quit (`confirmed`, so the close is not asked about again).
    fn quit(&self);
}

/// What a pick does (`trayOpen` is `toTray`'s `reopen`; Quit is `leaveTray(); trayQuit()`): Open Locust, the away
/// line and a click on the icon itself put the window back in front and take the icon away; Quit takes the icon
/// away and quits; the routine line does nothing.
pub fn act(id: &str, door: &dyn TrayDoor) {
    match id {
        OPEN | AWAY => {
            door.leave_tray();
            door.show_window();
            door.focus_window();
        }
        QUIT => {
            door.leave_tray();
            door.quit();
        }
        _ => {}
    }
}

/// A computer's icon in its menu bar or tray.
pub trait TrayHost {
    fn set_menu(&mut self, items: &[MenuItem]);
    fn set_tooltip(&mut self, line: &str);
}

impl<H: TrayHost + ?Sized> TrayHost for Box<H> {
    fn set_menu(&mut self, items: &[MenuItem]) {
        (**self).set_menu(items);
    }
    fn set_tooltip(&mut self, line: &str) {
        (**self).set_tooltip(line);
    }
}

/// One tray (`ensureTray`'s `say`, on its five-second beat): the line under the pointer says who is working every
/// time; the menu is rebuilt only when one of its labels changed (0.587), since a menu replaced under an open
/// pointer closes it.
pub struct MenuBar<H: TrayHost> {
    pub host: H,
    labels: Option<String>,
}

impl<H: TrayHost> MenuBar<H> {
    pub fn new(host: H) -> Self {
        Self { host, labels: None }
    }

    /// Says the tray's line and, when it changed, its menu. True when the menu was rebuilt.
    pub fn say(&mut self, working_names: &[&str], live_runs: usize, next_due: &str, away: Option<&str>) -> bool {
        self.host.set_tooltip(&tray_line(working_names, live_runs));
        let items = tray_menu(next_due, away);
        let labels = items.iter().map(|item| item.label.as_str()).collect::<Vec<_>>().join("\n");
        if self.labels.as_deref() == Some(labels.as_str()) {
            return false;
        }
        self.host.set_menu(&items);
        self.labels = Some(labels);
        true
    }
}

#[cfg(test)]
#[path = "menu_bar_tests.rs"]
mod menu_bar_tests;

#[cfg(test)]
#[path = "quit_guard_tray_parity_tests.rs"]
mod quit_guard_tray_parity_tests;
