//! Electron `main/the-menu-bar-uses-a-template.test.ts`, and the menu `ensureTray` builds beside it. Expected pictures
//! are Electron's own files in `apps/desktop/resources`, read here; expected words are Electron's literals.

use super::*;
use std::cell::RefCell;
use std::path::PathBuf;

fn electron_resource(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../desktop/resources").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A PNG's width and height, from its header (`png.readUInt32BE(16)` and `(20)`).
fn png_size(png: &[u8]) -> (u32, u32) {
    (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()))
}

// case: on darwin the tray uses the template image marked as a template and elsewhere the old icons
#[test]
fn on_darwin_the_tray_uses_the_template_image_marked_as_a_template_and_elsewhere_the_old_icons() {
    // Windows takes icon-32.png where Electron took icon.ico (packaged) or icon-512.png: Rune's tray takes pixels.
    for (platform, file, template) in [("darwin", "trayTemplate.png", true), ("linux", "icon-32.png", false), ("win32", "icon-32.png", false)] {
        let image = tray_image(platform);
        assert_eq!(image.file, file, "{platform}");
        assert_eq!(image.template, template, "{platform}: marked as a template only on darwin");
        assert_eq!(image.png, electron_resource(file).as_slice(), "{platform}: Electron's own picture");
        // The 16-point picture keeps its @2x representation with it.
        assert_eq!(image.retina.map(<[u8]>::to_vec), (platform == "darwin").then(|| electron_resource("trayTemplate@2x.png")), "{platform}");
        // Through the host that shows it off a Mac: Electron's picture, at its own size, never resized.
        let pixels = image::load_from_memory_with_format(&electron_resource(file), image::ImageFormat::Png).unwrap().to_rgba8();
        let (w, h) = png_size(&electron_resource(file));
        assert_eq!(crate::os_hosts::menu_bar_icon(platform), Ok(rune::TrayIcon::rgba(w, h, pixels.into_raw())), "{platform}");
    }
    // The template pictures ship with the program, at 16 and 32 pixels.
    assert_eq!(png_size(TEMPLATE_PNG), (16, 16));
    assert_eq!(png_size(TEMPLATE_PNG_2X), (32, 32));
}

#[derive(Default)]
struct Seen(RefCell<Vec<String>>);

impl TrayHost for &Seen {
    fn set_menu(&mut self, items: &[MenuItem]) {
        let labels = items.iter().map(|i| format!("{}{}", i.label, if i.enabled { "" } else { " (off)" })).collect::<Vec<_>>();
        self.0.borrow_mut().push(format!("menu: {}", labels.join(" | ")));
    }
    fn set_tooltip(&mut self, line: &str) {
        self.0.borrow_mut().push(format!("tip: {line}"));
    }
}

impl TrayDoor for Seen {
    fn leave_tray(&self) {
        self.0.borrow_mut().push("leave".into());
    }
    fn show_window(&self) {
        self.0.borrow_mut().push("show".into());
    }
    fn focus_window(&self) {
        self.0.borrow_mut().push("focus".into());
    }
    fn quit(&self) {
        self.0.borrow_mut().push("quit".into());
    }
}

/// `ensureTray`'s menu and `toTray`'s picks: the line says who is working on every beat, the menu is rebuilt only when
/// a label changed, Open Locust and the away line bring the window back, the routine line is off, Quit leaves and quits.
#[test]
fn the_menu_says_open_the_next_routine_and_quit_and_each_does_what_it_says() {
    let seen = Seen::default();
    let mut bar = MenuBar::new(&seen);
    assert!(bar.say(&["Mira"], 1, "Next routine: Morning brief at 9:00 AM", None));
    assert!(!bar.say(&["Mira"], 1, "Next routine: Morning brief at 9:00 AM", None), "the same labels: not rebuilt");
    assert!(bar.say(&[], 0, "Next routine: Morning brief at 9:00 AM", Some("Since you were away: 2 finished")));
    assert_eq!(
        seen.0.take(),
        vec![
            "tip: Locust: Mira is working.",
            "menu: Open Locust | Next routine: Morning brief at 9:00 AM (off) | Quit",
            "tip: Locust: Mira is working.",
            "tip: Locust: nobody is working. Click to open.",
            "menu: Open Locust | Since you were away: 2 finished | Next routine: Morning brief at 9:00 AM (off) | Quit",
        ]
    );

    for (id, did) in [(OPEN, vec!["leave", "show", "focus"]), (AWAY, vec!["leave", "show", "focus"]), (DUE, vec![]), (QUIT, vec!["leave", "quit"])] {
        act(id, &seen);
        assert_eq!(seen.0.take(), did, "{id}");
    }

    // Rune's tray (Linux, Windows) draws the same items by id, the routine line greyed out.
    assert_eq!(
        crate::os_hosts::tray_items(&tray_menu("No routine is due", None)),
        vec![
            rune::TrayItem::action("open", "Open Locust"),
            rune::TrayItem::action("due", "No routine is due").enabled(false),
            rune::TrayItem::action("quit", "Quit"),
        ]
    );
}
