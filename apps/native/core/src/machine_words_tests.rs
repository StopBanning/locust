//! a-mac-says-menu-bar-and-log-in.test.tsx (0.843): the cases of the words themselves. The window's own places are
//! checked in the window crate (`src/ui/machine_words_ui_tests.rs`).

use super::*;

const NOT_YET: Stay = Stay { open_at_login: false, login_available: true, keep_running: false };
const REACHED: Stay = Stay { open_at_login: true, login_available: true, keep_running: true };

#[test]
fn names_each_machine_as_its_own_settings_do() {
    assert_eq!(
        machine_words(Some("darwin")),
        MachineWords { tray: "menu bar", sign_in: "log in to this Mac", computer: "Mac", log_in: "log in", modifier: "Cmd" }
    );
    assert_eq!(
        machine_words(Some("win32")),
        MachineWords { tray: "tray", sign_in: "sign in to Windows", computer: "PC", log_in: "sign in", modifier: "Ctrl" }
    );
    assert_eq!(machine_words(None), machine_words(Some("win32")));
    assert_eq!(machine_words(Some("linux")), machine_words(Some("win32")), "anything not a Mac keeps the old words");
}

#[test]
fn offers_to_keep_locust_running_in_the_menu_bar_and_to_start_it_when_you_log_in_on_a_mac() {
    let mac = machine_words(Some("darwin"));
    let line = stay_line(mac, NOT_YET);
    assert!(line.contains("Keep it running in the menu bar when you close its window, and start it when you log in to this Mac."), "{line}");
    assert!(!line.contains("tray") && !line.contains("Windows"), "{line}");
    let reached = stay_line(mac, REACHED);
    assert!(
        reached.contains("it starts when you log in to this Mac, and it keeps running in the menu bar when you close its window. Quit from the menu bar"),
        "{reached}"
    );
}

#[test]
fn says_where_a_saved_voice_key_is_locked_on_each_machine() {
    assert_eq!(voice_key_locked_to(Some("darwin")), "this Mac's keychain");
    assert_eq!(voice_key_locked_to(Some("win32")), "your Windows account");
    assert_eq!(voice_key_locked_to(None), "your Windows account");
}

#[test]
fn keeps_windows_exactly_as_it_was() {
    let line = stay_line(machine_words(Some("win32")), NOT_YET);
    assert!(line.contains("Keep it running in the tray when you close its window, and start it when you sign in to Windows."), "{line}");
}
