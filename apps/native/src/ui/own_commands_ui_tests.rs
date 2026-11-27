//! The person's own commands as the window reads them (App.tsx's `ownCommandMenu` effect): both folders read in a job,
//! and read again when the folder changes. Words and order are Electron's (own-command-host.ts `readTree`).
use super::*;
use crate::store::Store;
use locust_core::own_commands::Source;
use rune::{Env, Runtime};
use std::path::Path;

pub(super) struct Dir(pub PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A data folder, an open folder in it, and a window state over them with no teammate on screen.
pub(super) fn state() -> (Dir, State) {
    let root = std::env::temp_dir().join(format!(
        "locust-own-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let mut s = State::default();
    s.data = Some(Store::new(root.join("native"), root.join("electron")).unwrap());
    s.home.folder_path = Some(root.join("work").to_string_lossy().into_owned());
    (Dir(root), s)
}

pub(super) fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The window's own poll (main.rs `poll`, which reads the commands and takes an expanded one) until nothing of theirs is
/// in flight and what landed has been taken: a landed answer stops being pending before a poll takes it.
pub(super) fn read_now(s: &mut State) {
    let busy = |s: &State| s.own_commands_job.is_pending() || s.own_commands_expand_job.is_pending();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        crate::poll(s, 0.0, &mut Tx::default());
        if !busy(s) {
            crate::poll(s, 0.0, &mut Tx::default());
            if !busy(s) {
                return;
            }
        }
        assert!(std::time::Instant::now() < deadline, "the commands were never read");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn listed(s: &State) -> Vec<(String, Source)> {
    s.own_commands.listed.iter().map(|c| (c.name.clone(), c.source)).collect()
}

#[test]
fn the_window_reads_your_commands_and_the_folders_and_again_when_the_folder_changes() {
    let (dir, mut s) = state();
    write(&dir.0.join("electron/commands/team/review.md"), "---\ndescription: Looks at the diff\n---\nReview\n");
    write(&dir.0.join("work/.locust/commands/status.md"), "Status\n");
    read_now(&mut s);
    assert_eq!(listed(&s), vec![("team/review".into(), Source::Global), ("status".into(), Source::Project)]);
    assert_eq!(s.own_commands.listed[0].description.as_deref(), Some("Looks at the diff"));
    // Another folder open: its commands, not the last one's.
    write(&dir.0.join("other/.locust/commands/ship.md"), "Ship\n");
    s.home.folder_path = Some(dir.0.join("other").to_string_lossy().into_owned());
    read_now(&mut s);
    assert_eq!(listed(&s), vec![("team/review".into(), Source::Global), ("ship".into(), Source::Project)]);
}

// ---- the `/` menu (Composer.tsx's own rows, `runSlash`, CommandAsk) through the composer's own state functions

fn menu(s: &State) -> Vec<(String, Option<String>, String)> {
    crate::composer_wiring::slash_choices_now(s).into_iter().map(|c| (c.name, c.group, c.detail)).collect()
}

fn key(key: Key) -> KeyEvent {
    KeyEvent { key, mods: Default::default() }
}

fn nothing_sent(s: &State) -> bool {
    s.flight.is_none() && s.queue_rows.is_empty()
}

/// The ask card as the composer draws it (terminal_panel::commands, under the box).
fn drawn_words(s: State) -> (Vec<String>, Vec<rune::ElementInfo>, State) {
    let mut rt = Runtime::new(s, |s, _| crate::terminal_panel::commands(s, &Palette::new(true)), (700.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    (elements.iter().filter_map(|e| e.text.clone()).collect(), elements, rt.state)
}

/// Composer.tsx: yours sit after Locust's own and before a runtime's list, under "Your commands" or "This folder", with
/// the file's description or Electron's words for where it is; a name Locust uses stays Locust's. `/name the rest`
/// offers that command alone, and choosing it puts the prompt, filled in, into the box: nothing is sent.
#[test]
fn chooses_a_command_by_filling_the_box() {
    // case: chooses a command by filling the box
    let (dir, mut s) = state();
    write(&dir.0.join("electron/commands/team/review.md"), "---\ndescription: Looks at the diff\nmode: plan\n---\nReview $ARGUMENTS\n");
    write(&dir.0.join("electron/commands/ship.md"), "Ship it.\n");
    write(&dir.0.join("electron/commands/plan.md"), "Not Locust's plan.\n");
    write(&dir.0.join("work/.locust/commands/status.md"), "Status\n");
    read_now(&mut s);
    s.draft.replace_text("/");
    let rows = menu(&s);
    let yours: Vec<_> = rows.iter().filter(|(_, group, _)| group.is_some()).cloned().collect();
    assert_eq!(
        yours,
        vec![
            ("ship".into(), Some("Your commands".into()), "A command of yours.".into()),
            ("team/review".into(), Some("Your commands".into()), "Looks at the diff".into()),
            ("status".into(), Some("This folder".into()), "A command in this folder.".into()),
        ]
    );
    let first_yours = rows.iter().position(|(_, group, _)| group.is_some()).unwrap();
    assert!(rows[..first_yours].iter().any(|(name, ..)| name == "plan"), "Locust's own come first: {rows:?}");
    assert_eq!(rows.iter().filter(|(name, ..)| name == "plan").count(), 1, "a file named plan does not take Locust's /plan");
    s.draft.replace_text("/te");
    assert_eq!(menu(&s).into_iter().map(|(name, ..)| name).collect::<Vec<_>>(), vec!["team/review"]);
    // `/name the rest`: that command alone, and Enter chooses it with the rest as its arguments.
    s.draft.replace_text("/team/review the notes");
    assert_eq!(menu(&s).into_iter().map(|(name, ..)| name).collect::<Vec<_>>(), vec!["team/review"]);
    assert!(crate::composer_wiring::composer_input_key(&mut s, &key(Key::Enter), &mut Tx::default()));
    read_now(&mut s);
    assert_eq!(s.draft.text, "Review the notes\n");
    assert_eq!(s.composer_mode.as_deref(), Some("plan"), "the file's mode is taken, as Electron's setMode");
    assert!(nothing_sent(&s), "choosing fills the box; nothing is sent");
    // Chosen from the menu by press, a command with no arguments: its prompt alone.
    s.draft.replace_text("/sh");
    let ship = crate::composer_wiring::slash_choices_now(&s).into_iter().find(|c| c.name == "ship").unwrap();
    crate::composer_wiring::run_slash_command(&mut s, &ship);
    read_now(&mut s);
    assert_eq!(s.draft.text, "Ship it.\n");
    assert!(nothing_sent(&s));
}

/// a-bang-in-the-box-is-command-mode.test.tsx: a folder's command with a `!` line asks first with CommandAsk's words,
/// Enter in the box neither sends nor chooses while it waits, Escape is Not now, and Allow runs the line and fills the box.
#[test]
fn asks_before_a_project_command_runs_and_choosing_one_fills_the_box_instead_of_sending() {
    // case: asks before a project command runs, and choosing one fills the box instead of sending
    let (dir, mut s) = state();
    write(&dir.0.join("work/.locust/commands/status.md"), "Status:\n\n!`echo project-bang`\n");
    read_now(&mut s);
    s.draft.replace_text("/status");
    let status = crate::composer_wiring::slash_choices_now(&s).into_iter().find(|c| c.name == "status").unwrap();
    crate::composer_wiring::run_slash_command(&mut s, &status);
    read_now(&mut s);
    let (words, elements, mut s) = drawn_words(s);
    assert!(words.iter().any(|w| w == "This folder's commands have not been allowed to run yet. Allow them once, and these will run:"), "{words:?}");
    assert!(words.iter().any(|w| w == "echo project-bang"), "{words:?}");
    assert!(words.iter().any(|w| w == "Allow") && words.iter().any(|w| w == "Not now"), "{words:?}");
    assert!(elements.iter().any(|e| e.role == "dialog" && e.label.as_deref() == Some("Allow this folder's commands")));
    assert_eq!(s.draft.text, "/status", "the box is as it was");
    // The card is the decision: Enter neither sends nor chooses, the menu is shut, and Send does nothing.
    assert!(crate::composer_wiring::slash_choices_now(&s).is_empty());
    assert!(crate::composer_wiring::composer_input_key(&mut s, &key(Key::Enter), &mut Tx::default()));
    crate::composer_wiring::submit_composer(&mut s, &mut Tx::default(), false);
    assert_eq!(s.draft.text, "/status");
    // Whatever is typed meanwhile waits too: a `!` command neither runs nor is refused.
    s.draft.replace_text("!echo typed-meanwhile");
    crate::composer_wiring::submit_composer(&mut s, &mut Tx::default(), false);
    assert_eq!((s.draft.text.as_str(), s.composer_note.as_deref()), ("!echo typed-meanwhile", None));
    assert!(nothing_sent(&s) && s.terminal.commands.is_empty());
    s.draft.replace_text("/status");
    // Escape is Not now: the card goes and the box stays.
    assert!(crate::composer_wiring::composer_input_key(&mut s, &key(Key::Escape), &mut Tx::default()));
    assert!(s.own_commands.ask.is_none());
    assert_eq!(s.draft.text, "/status");
    // Asked again, Allow: the folder is allowed, the line runs, and its output is in the box. Nothing is sent.
    crate::composer_wiring::run_slash_command(&mut s, &status);
    read_now(&mut s);
    assert!(s.own_commands.ask.is_some());
    crate::own_commands_ui::allow_ask(&mut s);
    read_now(&mut s);
    assert!(s.own_commands.ask.is_none());
    assert_eq!(s.draft.text, "Status:\n\noutput of `echo project-bang` (exit 0):\nproject-bang\n\n");
    assert!(nothing_sent(&s));
    assert!(locust_core::own_commands::allowed(&dir.0.join("electron"), &dir.0.join("work")));
}

/// Composer.tsx's order: Locust's own, then yours, then a runtime's long list; while a turn runs yours stay offered and
/// the runtime's go, as Electron builds `slashMatches`.
#[test]
fn yours_sit_between_locusts_own_and_the_runtimes_and_stay_while_a_turn_runs() {
    use locust_core::own_commands::StoredCommand;
    let yours = composer::own_slash_commands(&[StoredCommand {
        name: "team/review".into(),
        description: None,
        teammate: None,
        mode: None,
        body: "Review".into(),
        source: Source::Global,
    }]);
    let theirs = composer::runtime_slash_commands("Claude Code", &[("compact".into(), String::new(), String::new())]);
    let offered = |running: bool| {
        composer::available_commands(composer::AvailableInput {
            modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
            running,
            issues: false,
            tune: false,
            setup: true,
            own_commands: &yours,
            runtime_commands: &theirs,
        })
        .into_iter()
        .map(|c| c.name)
        .collect::<Vec<_>>()
    };
    assert_eq!(offered(false), vec!["ask", "plan", "edit", "approve", "model", "setup", "auto", "team/review", "compact"]);
    assert_eq!(offered(true), vec!["ask", "plan", "edit", "approve", "model", "stop", "setup", "auto", "team/review"]);
}

// App.tsx applyOwnCommand: the prompt goes into the box with `attachments: []` (setHandBack), so what was attached to the
// message being written is let go.
#[test]
fn choosing_a_command_lets_go_of_what_was_attached_to_the_message_being_written() {
    let (dir, mut s) = state();
    write(&dir.0.join("electron/commands/ship.md"), "Ship it.\n");
    read_now(&mut s);
    s.attached.push("/tmp/a-picture.png".into());
    super::choose(&mut s, "ship", Source::Global, "");
    read_now(&mut s);
    assert_eq!(s.draft.text, "Ship it.\n");
    assert!(s.attached.is_empty(), "{:?}", s.attached);
}

// App.tsx applyOwnCommand: `setSelectedTeammateId(mate.teammateId)` for the teammate the file names. For the one already
// on screen that is no change at all: the mode picked in the box stays, and no menu is closed.
#[test]
fn a_command_naming_the_teammate_already_on_screen_changes_nothing_else() {
    let (dir, mut s) = state();
    write(&dir.0.join("electron/commands/ship.md"), "---\nteammate: Wren\n---\nShip it.\n");
    s.teammates.push(locust_core::engine::Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("claude".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.composer_mode = Some("plan".into());
    s.mode_menu = true;
    read_now(&mut s);
    super::choose(&mut s, "ship", Source::Global, "");
    read_now(&mut s);
    assert_eq!(s.draft.text, "Ship it.\n");
    assert_eq!(s.composer_mode.as_deref(), Some("plan"), "the mode the person picked stays: the teammate is the same one");
    assert!(s.mode_menu, "no menu is closed by choosing the teammate who is already there");
}
