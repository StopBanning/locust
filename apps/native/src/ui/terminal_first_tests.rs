//! the-terminal-opens-before-the-first-message.test.tsx (4 cases), through the window: the title bar drawn by the
//! window's own view with no conversation yet, read as a screen reader hears it; Ctrl+` pressed through the window's
//! key handler (`on_key`); and a paste, Ctrl+V into a real terminal, read back from what the shell received. Expected
//! words are Electron's.
use super::*;
use serde_json::Value;

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|c| node(c, key))
}

fn states(n: &Value) -> Vec<String> {
    n["states"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn terminal_off() {
    let policy = [("terminal".to_string(), serde_json::json!(false))].into_iter().collect();
    locust_core::org_policy::use_policy_on_this_thread(Some(locust_core::org_policy::PolicyState { path: "/test/policy.json".into(), present: true, policy, error: None }));
}

struct Back;
impl Drop for Back {
    fn drop(&mut self) {
        locust_core::org_policy::use_policy_on_this_thread(None);
    }
}

/// The whole window on its home, before any conversation exists.
fn window() -> rune::Runtime<State> {
    let mut s = State::default();
    crate::home_panel::open_home(&mut s);
    let mut rt = rune::Runtime::new(s, crate::view, (1200.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn toggle_drawn(rt: &rune::Runtime<State>) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-toggle")).expect("the terminal toggle is in the title bar")
}

fn press(key: Key, ctrl: bool, alt: bool, logo: bool) -> KeyEvent {
    // Off macOS, Ctrl is also the primary modifier (`cmd`).
    KeyEvent { key, mods: rune::Mods { ctrl, cmd: ctrl, alt, logo, ..Default::default() } }
}

// case: offers an accessible toggle in the title bar before a conversation exists
#[test]
fn offers_an_accessible_toggle_in_the_title_bar_before_a_conversation_exists() {
    let _back = Back;
    locust_core::org_policy::use_policy_on_this_thread(None);
    let rt = window();
    assert!(rt.state.ledger.is_none() && rt.state.teammates.is_empty());
    let toggle = toggle_drawn(&rt);
    assert_eq!(toggle.role, "button");
    assert_eq!(toggle.label.as_deref(), Some("Terminal"));
    // Electron says the shortcut as aria-keyshortcuts="Control+`"; Rune's tree has no such field, so the window says it
    // in the hover, and the shortcut itself is the next case.
    assert!(toggle.help.as_deref().is_some_and(|h| h.contains("Ctrl+`")), "{:?}", toggle.help);
    let tree = rt.accessibility_json();
    assert!(states(node(&tree, "terminal-toggle").expect("in the tree")).iter().any(|s| s == "unchecked"), "closed, it reads as not pressed");
}

// case: shows the organization restriction on the disabled terminal control
#[test]
fn shows_the_organization_restriction_on_the_disabled_terminal_control() {
    let _back = Back;
    terminal_off();
    let mut rt = window();
    let toggle = toggle_drawn(&rt);
    assert_eq!(toggle.label.as_deref(), Some("Terminal, set by your organization"));
    assert!(toggle.help.as_deref().is_some_and(|h| h.starts_with("The terminal panel is off.")), "{:?}", toggle.help);
    let tree = rt.accessibility_json();
    assert!(states(node(&tree, "terminal-toggle").unwrap()).iter().any(|s| s == "disabled"));
    // Nor does the shortcut open it.
    assert!(on_key(&mut rt.state, &press(Key::Command('`'), true, false, false), &mut Tx::default()));
    assert!(!rt.state.terminal.open);
}

// case: recognizes Ctrl backtick on every platform and leaves unrelated keys alone
#[test]
fn recognizes_ctrl_backtick_on_every_platform_and_leaves_unrelated_keys_alone() {
    let _back = Back;
    locust_core::org_policy::use_policy_on_this_thread(None);
    let opens = |event: KeyEvent| {
        let mut s = State::default();
        on_key(&mut s, &event, &mut Tx::default());
        s.terminal.open
    };
    assert!(opens(press(Key::Command('`'), true, false, false)), "Ctrl+` off macOS");
    assert!(opens(press(Key::Command('~'), true, false, false)), "Ctrl+Shift+`");
    assert!(opens(KeyEvent { key: Key::Text("`".into()), mods: rune::Mods { ctrl: true, ..Default::default() } }), "Ctrl+` on macOS, where Ctrl is not Command");
    assert!(!opens(press(Key::Text("`".into()), false, false, false)), "a backquote typed");
    assert!(!opens(press(Key::Command('`'), true, true, false)), "with Alt");
    assert!(!opens(press(Key::Command('`'), true, false, true)), "with Command or the Windows key");
    assert!(!opens(press(Key::Command('a'), true, false, false)), "another key");
    // Pressed again, it closes.
    let mut s = State::default();
    on_key(&mut s, &press(Key::Command('`'), true, false, false), &mut Tx::default());
    on_key(&mut s, &press(Key::Command('`'), true, false, false), &mut Tx::default());
    assert!(!s.terminal.open);
}

// case: keeps a pasted Unicode character whole at every IPC boundary
#[cfg(unix)]
#[test]
fn keeps_a_pasted_unicode_character_whole_at_every_ipc_boundary() {
    let _back = Back;
    locust_core::org_policy::use_policy_on_this_thread(None);
    let dir = std::env::temp_dir().join(format!("locust-terminal-paste-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    struct Gone(std::path::PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _gone = Gone(dir.clone());
    // Electron's text: 65,535 UTF-16 units, then one character of two, then more. Here it is one UTF-8 string, and the
    // paste goes to the terminal in one write; the shell keeps exactly the bytes it was given.
    let pasted = format!("{}{}tail", "x".repeat(65535), char::from_u32(0x1f600).unwrap());
    let script = format!("stty raw -echo; printf ready; head -c {} > pasted", pasted.len());
    let shell = locust_core::terminal::Terminal::open(&dir, std::path::Path::new("/bin/sh"), &["-c", &script]).expect("a terminal");
    let screen = shell.screen.clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !crate::invariants::lock(&screen).screen().contents().contains("ready") {
        assert!(std::time::Instant::now() < deadline, "the shell never got ready");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut s = State::default();
    s.terminal.open = true;
    s.terminal.tabs.push(shell);
    let mut rt = rune::Runtime::new(s, |s, _| crate::terminal_panel::view(s, &Palette::new(true)), (900.0, 500.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let (x, y) = rt.find("terminal-screen").expect("the terminal is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("terminal-screen") && e.focused), "pressed, the terminal takes the keys");
    // Pasted as a person pastes: the clipboard holds it, and Ctrl+V goes to the focused terminal.
    let board = rune::clipboard::memory();
    board.set_text(&pasted);
    rt.use_clipboard(&board);
    rt.key(KeyEvent { key: Key::Command('v'), mods: rune::Mods { ctrl: true, cmd: true, ..Default::default() } });
    rt.frame(0.032);
    rt.frame(0.048);
    assert_eq!(rt.state.terminal.error, None);
    let out = dir.join("pasted");
    while std::fs::metadata(&out).map(|m| m.len() as usize).unwrap_or(0) < pasted.len() {
        assert!(std::time::Instant::now() < deadline, "the shell never received the whole paste: {:?} bytes, screen {:?}", std::fs::metadata(&out).map(|m| m.len()), crate::invariants::lock(&rt.state.terminal.tabs[0].screen).screen().contents());
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(&out).unwrap(), pasted);
}
