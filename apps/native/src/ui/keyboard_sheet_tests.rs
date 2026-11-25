//! a-mac-says-cmd.test.tsx and keyboardShortcuts.test.ts through the window: the shortcuts sheet drawn for a Mac and for
//! a PC (`view_on`), a key to a cap, and the rows held to the handlers that take them. Expected keys and words are
//! Electron's own.
use super::*;
use crate::keyboard_shortcuts::{SHORTCUTS, ShortcutRow};
use rune::{Env, Runtime};

fn sheet(platform: &'static str) -> Runtime<State> {
    let s = State::default();
    let mut rt = match platform {
        "darwin" => Runtime::new(s, |s, _| view_on(s, &Palette::new(true), "darwin"), (760.0, 4000.0), Env::new(true, 1.0)),
        _ => Runtime::new(s, |s, _| view_on(s, &Palette::new(true), "win32"), (760.0, 4000.0), Env::new(true, 1.0)),
    };
    rt.frame(0.0);
    rt
}

/// The caps drawn for the row that does `does`: each chord's keys, and "or" between chords, in order.
fn caps_of(rt: &Runtime<State>, does: &str) -> Vec<String> {
    let els = rt.elements();
    let row_at = SHORTCUTS.iter().position(|r| r.does == does).expect("a row that does it");
    let row = &SHORTCUTS[row_at];
    let nth = SHORTCUTS.iter().filter(|r| r.group == row.group).position(|r| r.does == does).unwrap();
    let key = format!("keys-{}-{nth}", row.group.label());
    let at = els.iter().position(|e| e.key.as_deref() == Some(key.as_str())).unwrap_or_else(|| panic!("no {key}"));
    els.iter().filter(|e| e.parent == Some(at)).filter_map(|e| e.text.clone()).collect()
}

fn all_words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().flat_map(|e| [e.text, e.label, e.help]).flatten().collect()
}

#[test]
fn says_cmd_for_the_chords_that_take_it_and_option_for_alt() {
    // case: says Cmd for the chords that take it, and Option for Alt
    assert_eq!(keys_on("Ctrl+K", "darwin", false), "Cmd+K");
    assert_eq!(keys_on("Ctrl 1", "darwin", false), "Cmd 1");
    assert_eq!(keys_on("Ctrl+Shift+M", "darwin", false), "Cmd+Shift+M");
    assert_eq!(keys_on("Alt+Shift+Up / Alt+Shift+Down", "darwin", false), "Option+Shift+Up / Option+Shift+Down");
    assert_eq!(keys_on("Ctrl+K", "win32", false), "Ctrl+K");
    assert_eq!(keys_on("Ctrl+K", "", false), "Ctrl+K");
    // Through the sheet a Mac draws: Cmd and Option, in its marks.
    let mac = sheet("darwin");
    assert_eq!(caps_of(&mac, "Permission mode"), ["⌘", "⇧", "M"]);
    assert_eq!(caps_of(&mac, "A conversation that is waiting for you"), ["⌥", "⇧", "↑", "or", "⌥", "⇧", "↓"]);
}

#[test]
fn keeps_control_where_the_handler_listens_for_control_alone() {
    // case: keeps Control where the handler listens for Control alone
    let kept: Vec<&str> = SHORTCUTS.iter().filter(|r| r.control_on_mac).map(|r| r.keys).collect();
    assert_eq!(kept, ["Ctrl+`", "Ctrl+Alt+Up / Ctrl+Alt+Down", "Ctrl+Alt+Space"]);
    assert_eq!(keys_on("Ctrl+Alt+Space", "darwin", true), "Ctrl+Option+Space");
    // What makes the labels true: the window's chords take Cmd on a Mac (Ctrl elsewhere), the rail's Ctrl alone.
    let cmd_k = KeyEvent { key: Key::Command('k'), mods: rune::Mods { cmd: true, ..Default::default() } };
    assert!(cmd_k.is_command('k'));
    let rail = |mods: rune::Mods| crate::prompt_rail::prompt_rail_step(&crate::prompt_rail::RailKey::from(&KeyEvent { key: Key::Up, mods }));
    assert_eq!(rail(rune::Mods { ctrl: true, alt: true, ..Default::default() }), Some(-1));
    assert_eq!(rail(rune::Mods { cmd: true, alt: true, ..Default::default() }), None, "Cmd is not Control");
    assert_eq!(rail(rune::Mods { ctrl: true, cmd: true, alt: true, ..Default::default() }), None);
}

#[test]
fn draws_the_shortcuts_sheet_in_a_macs_marks_and_a_pcs_in_its_own_words_a_key_to_a_cap() {
    // case: draws the shortcuts sheet in a Mac's marks, and a PC's in its own words, a key to a cap
    let mac = sheet("darwin");
    assert_eq!(caps_of(&mac, "Open the command palette"), ["⌘", "K"]);
    assert_eq!(caps_of(&mac, "Terminal"), ["⌃", "`"], "Control alone: the Mac's own mark for it");
    assert!(!caps_of(&mac, "Open the command palette").iter().any(|k| k == "Ctrl"));
    let pc = sheet("win32");
    assert_eq!(caps_of(&pc, "Open the command palette"), ["Ctrl", "K"]);
    assert_eq!(caps_of(&pc, "Zoom an image, fit it, or its actual size"), ["+", "or", "−", "or", "0", "or", "1"]);
    let cap = mac.elements().into_iter().find(|e| e.text.as_deref() == Some("⌘")).unwrap();
    assert_eq!(cap.role, "kbd", "each key its own cap");
}

#[test]
fn leaves_no_ctrl_written_into_a_palette_hint_or_a_button_title() {
    // case: leaves no Ctrl written into a palette hint or a button title
    let mut s = State::default();
    s.palette_actions = default_palette_actions(&[], true, true);
    s.palette_open = true;
    let mut rt = Runtime::new(s, |s, _| command_palette(s, &Palette::new(true)), (600.0, 500.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let said = all_words(&rt);
    assert!(said.iter().any(|w| w == "Command palette"), "{said:?}");
    assert!(!said.iter().any(|w| w.contains("Ctrl")), "{said:?}");
    // The button titles of the sidebar and the screens, and the message box's hint, as Electron reads its sources.
    for (name, source) in [("sidebar.rs", include_str!("sidebar.rs")), ("missions_screen.rs", include_str!("missions_screen.rs"))] {
        for line in source.lines().filter(|l| l.contains(".help(")) {
            assert!(!line.contains("Ctrl"), "{name}: {line}");
        }
    }
    assert!(!include_str!("composer_wiring.rs").contains("(Ctrl+Enter while typing)"));
}

/// Where each row the window binds is handled (keyboardShortcuts.ts `mark` / `file`): the words in that file that take
/// the key. A row the window does not bind says so on the sheet instead.
const MARKS: &[(&str, &str, &str, &str)] = &[
    ("Ctrl+F", "Find in this conversation", "main.rs", "open_find(s);"),
    ("Ctrl+K", "Open the command palette", "main.rs", "s.palette_open = true;"),
    ("Alt+Up / Alt+Down", "Previous or next conversation", "main.rs", "if sidebar::step_from_key(s, event, tx)"),
    ("Alt+Shift+Up / Alt+Shift+Down", "A conversation that is waiting for you", "sidebar.rs", "s.home.waiting_for.contains_key(key)"),
    ("Ctrl+1", "Home", "main.rs", "missions_screen::open(s);"),
    ("Ctrl+I", "Open the inspector", "main.rs", "if event.is_command('i') {"),
    ("Ctrl+2", "Teammates", "main.rs", "if event.is_command('2') {"),
    ("Ctrl+3", "Settings", "main.rs", "if event.is_command('3') {"),
    ("Ctrl+4", "Rooms", "main.rs", "if event.is_command('4') {"),
    ("Ctrl+5", "Memory", "main.rs", "if event.is_command('5') {"),
    ("Ctrl+`", "Terminal", "main.rs", "if terminal_panel::is_toggle_key(event) {"),
    ("Ctrl+/", "Keyboard shortcuts", "main.rs", "if event.is_command('/') {"),
    ("Ctrl+Alt+Up / Ctrl+Alt+Down", "Move among your own messages", "main.rs", "prompt_rail::handle_key(s, event, tx)"),
    ("Ctrl+O", "Normal or Verbose steps", "main.rs", "settings_panel::toggle_transcript_view(s);"),
    ("Enter", "Send", "keys.rs", "if key == ComposerKey::Enter && !mods.shift {"),
    ("Shift+Enter", "A new line", "keys.rs", "ComposerKey::Enter && !mods.shift"),
    ("Ctrl+Enter", "Send now, while a reply is running", "keys.rs", "return KeyAction::Submit { send_now };"),
    ("Shift+Tab", "Next permission mode", "keys.rs", "if key == ComposerKey::Tab && mods.shift"),
    ("Escape", "Stop the reply, when the box is focused and no menu is open", "keys.rs", "if key == ComposerKey::Escape && ctx.running && !ctx.menu_open"),
    ("Up / Down", "Recall a sent message from an empty box", "keys.rs", "return KeyAction::HistoryUp;"),
    ("Escape", "Close a dialog", "main.rs", "s.keyboard_sheet.open = false;"),
    ("1–9", "Answer a question card", "main.rs", "question_card::handle_key(s, event)"),
    ("1–9", "Pick that row in an open menu", "main.rs", "if thread_view::mode_menu_digit(s, event) {"),
    ("+ / − / 0 / 1", "Zoom an image, fit it, or its actual size", "step_pictures.rs", r#"Key::Text(t) if t == "+" || t == "=" => { zoom_to(s, 1.25); true }"#),
    ("Ctrl+Shift+M", "Permission mode", "main.rs", "if event.mods.shift && !event.mods.alt && event.is_command('m') {"),
    ("Ctrl+Shift+I", "Model", "main.rs", "if event.mods.shift && !event.mods.alt && event.is_command('i') {"),
    ("Ctrl+Shift+E", "Effort", "main.rs", "if event.mods.shift && !event.mods.alt && event.is_command('e') {"),
];

fn source(file: &str) -> &'static str {
    match file {
        "main.rs" => include_str!("../main.rs"),
        "sidebar.rs" => include_str!("sidebar.rs"),
        "keys.rs" => include_str!("../composer/keys.rs"),
        "step_pictures.rs" => include_str!("step_pictures.rs"),
        other => panic!("no source {other}"),
    }
}

#[test]
fn groups_them_for_a_person_and_every_row_is_a_key_the_code_actually_handles() {
    // case: groups them for a person, and every row is a key the code actually handles
    let names: Vec<&str> = shortcut_groups().into_iter().map(|(g, _)| g.label()).collect();
    assert_eq!(names, ["Conversations", "The message box", "Panes", "Menus"]);
    for row in SHORTCUTS {
        let mark = MARKS.iter().find(|(keys, does, _, _)| *keys == row.keys && *does == row.does);
        match (row.native, mark) {
            (true, Some((_, _, file, words))) => assert!(source(file).contains(words), "{}: {file} has no {words}", row.keys),
            (true, None) => panic!("{} is marked bound but nothing says where", row.keys),
            (false, Some(_)) => panic!("{} is handled but the sheet says it runs in Locust", row.keys),
            (false, None) => {}
        }
    }
    // The sheet tells a person which rows this window does not take yet, and only those.
    let pc = sheet("win32");
    let notes = pc.elements().into_iter().filter(|e| e.text.as_deref() == Some("runs in Locust for now")).count();
    assert_eq!(notes, SHORTCUTS.iter().filter(|r: &&ShortcutRow| !r.native).count());
    assert!(caps_of(&pc, "Home") == ["Ctrl", "1"]);
}
