//! shared/a-command-file-becomes-a-prompt.test.ts and the reading half of main/a-bang-runs-in-the-conversations-folder
//! .test.ts (own-command-host.ts `readTree`). Expected words and values are Electron's.
use super::*;

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp(label: &str) -> Dir {
    let dir = std::env::temp_dir().join(format!(
        "locust-own-cmd-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Dir(dir)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn names(list: &[StoredCommand]) -> Vec<(&str, Source)> {
    list.iter().map(|c| (c.name.as_str(), c.source)).collect()
}

#[test]
fn reads_a_command_file_including_a_nested_name_and_ignores_a_mode_it_does_not_have() {
    // case: reads a command file, including a nested name, and ignores a mode it does not have
    let parsed = parse_own_command(
        "---\ndescription: Looks at the diff\nteammate: Wren\nmode: plan\n---\nReview $ARGUMENTS\n",
        "team/review",
        Source::Global,
    );
    assert_eq!(
        parsed,
        StoredCommand {
            name: "team/review".into(),
            description: Some("Looks at the diff".into()),
            teammate: Some("Wren".into()),
            mode: Some("plan"),
            body: "Review $ARGUMENTS\n".into(),
            source: Source::Global,
        }
    );
    assert_eq!(command_name_from_path("team/review.md"), "team/review");
    assert_eq!(command_name_from_path("notes\\ship.md"), "notes/ship");
    assert_eq!(parse_own_command("---\nmode: sideways\n---\nGo\n", "go", Source::Global).mode, None);
    let plain = parse_own_command("Just the prompt.\n", "plain", Source::Global);
    assert_eq!(plain.body, "Just the prompt.\n");
    assert_eq!(plain.description, None);
    assert_eq!(plain.mode, None);
}

/// MODE_WORDS and fieldOf: each word a file may give, quotes taken off, an empty value no value, unknown keys ignored.
#[test]
fn every_mode_word_and_a_quoted_field_read_as_electron_reads_them() {
    for (word, mode) in [
        ("ask", "ask"),
        ("plan", "plan"),
        ("edit", "accept-edits"),
        ("accept-edits", "accept-edits"),
        ("approve", "approve-each"),
        ("approve-each", "approve-each"),
        ("auto", "auto"),
        ("PLAN", "plan"),
    ] {
        let parsed = parse_own_command(&format!("---\nmode: {word}\n---\nGo\n"), "go", Source::Project);
        assert_eq!(parsed.mode, Some(mode), "{word}");
    }
    let quoted = parse_own_command(
        "---\r\ncolour: blue\r\ndescription: \"  Looks at it  \"\r\nteammate: ''\r\n---\r\nBody\n",
        "q",
        Source::Project,
    );
    assert_eq!(quoted.description.as_deref(), Some("Looks at it"));
    assert_eq!(quoted.teammate, None);
    assert_eq!(quoted.body, "Body\n");
    // One blank line after the block is taken off; the block is optional.
    assert_eq!(parse_own_command("---\nmode: ask\n---\n\nBody", "b", Source::Global).body, "Body");
}

/// A top-of-file block that is never closed is no block: the whole file is the prompt, and the file is still listed.
#[test]
fn a_broken_top_of_file_block_is_read_as_the_prompt() {
    let parsed = parse_own_command("---\ndescription: never closed\nReview\n", "broken", Source::Global);
    assert_eq!(parsed.body, "---\ndescription: never closed\nReview\n");
    assert_eq!(parsed.description, None);
    let empty = parse_own_command("---\n---\nBody\n", "empty", Source::Global);
    assert_eq!(empty.body, "---\n---\nBody\n", "an empty block is not one either: Electron's pattern needs a line in it");
}

/// readTree over both folders: the person's own first, then the conversation folder's, each saying where it came from,
/// nested folders giving nested names, and only Markdown files.
#[test]
fn both_folders_are_read_yours_first_with_nested_names_and_where_each_came_from() {
    let profile = temp("profile");
    let folder = temp("folder");
    write(&profile.0.join("commands/team/review.md"), "---\ndescription: Looks at the diff\n---\nReview\n");
    write(&profile.0.join("commands/ship.MD"), "Ship it.\n");
    write(&profile.0.join("commands/notes.txt"), "Not a command.\n");
    write(&folder.0.join(".locust/commands/status.md"), "---\nmode: plan\nteammate: Wren\n---\nStatus $1\n");
    let all = commands_in(&profile.0, Some(&folder.0));
    assert_eq!(
        names(&all),
        vec![("ship", Source::Global), ("team/review", Source::Global), ("status", Source::Project)]
    );
    assert_eq!(all[1].description.as_deref(), Some("Looks at the diff"));
    assert_eq!((all[2].mode, all[2].teammate.as_deref()), (Some("plan"), Some("Wren")));
    // With no folder, only yours.
    assert_eq!(names(&commands_in(&profile.0, None)), vec![("ship", Source::Global), ("team/review", Source::Global)]);
    assert_eq!(global_dir(&profile.0), profile.0.join("commands"));
    assert_eq!(project_dir(&folder.0), folder.0.join(".locust").join("commands"));
}

/// What readTree passes over: a missing folder gives none; a dot name, a link, a ninth folder deep and a file over
/// MAX_FILE are left out while the rest are read; a file that is not UTF-8 is read as Node reads it, with marks.
#[test]
fn a_missing_folder_gives_none_and_what_cannot_be_a_command_is_left_out() {
    let profile = temp("skips");
    assert!(read_tree(&profile.0.join("commands"), Source::Global).is_empty(), "no folder yet: no commands, no error");
    let root = profile.0.join("commands");
    write(&root.join(".hidden.md"), "Hidden.\n");
    write(&root.join(".git/inside.md"), "Inside a dot folder.\n");
    write(&root.join("long.md"), &"x".repeat(MAX_FILE + 1));
    write(&root.join("just-right.md"), &"x".repeat(MAX_FILE));
    std::fs::write(root.join("latin.md"), b"caf\xe9\n").unwrap();
    // A folder whose path has eight parts is read; one of nine is not.
    let nine: PathBuf = (1..=MAX_DEPTH + 1).map(|n| format!("d{n}")).collect();
    write(&root.join(&nine).join("too-deep.md"), "In a ninth folder.\n");
    let eight: PathBuf = (1..=MAX_DEPTH).map(|n| format!("d{n}")).collect();
    write(&root.join(&eight).join("deep-enough.md"), "In an eighth folder.\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("just-right.md"), root.join("linked.md")).unwrap();
    let read = read_tree(&root, Source::Global);
    assert_eq!(
        read.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        vec!["d1/d2/d3/d4/d5/d6/d7/d8/deep-enough", "just-right", "latin"]
    );
    assert_eq!(read[2].body, "caf\u{fffd}\n");
}

/// MAX_COMMANDS: a folder of more files gives the first 200, and nothing past them.
#[test]
fn a_folder_gives_at_most_two_hundred_commands() {
    let profile = temp("many");
    for n in 0..(MAX_COMMANDS + 5) {
        write(&profile.0.join(format!("commands/c{n:03}.md")), "Go.\n");
    }
    let read = read_tree(&profile.0.join("commands"), Source::Global);
    assert_eq!(read.len(), MAX_COMMANDS);
    assert_eq!(read.last().map(|c| c.name.as_str()), Some("c199"));
}

// ---- a file becomes a prompt (shared/own-commands.ts, own-command-host.ts `expand` and `allow`)

#[test]
fn gives_1_and_2_the_words_quotes_grouping_and_the_last_the_rest() {
    // case: gives $1 and $2 the words, quotes grouping, and the last the rest
    assert_eq!(split_command_words("\"hello world\" Sam"), vec!["hello world", "Sam"]);
    assert_eq!(expand_template("See $1 then $2", "one two three"), "See one then two three");
    assert_eq!(expand_template("Say $1 to $2", "\"hello world\" Sam"), "Say hello world to Sam");
    assert_eq!(expand_template("Task: $ARGUMENTS", "fix the bug"), "Task: fix the bug");
    assert_eq!(expand_template("Review this.", "the api"), "Review this.\n\nthe api");
    assert_eq!(expand_template("Review this.", ""), "Review this.");
    assert_eq!(expand_template("Keep $1", ""), "Keep ");
}

#[test]
fn finds_a_bang_block_and_the_arguments_typed_after_a_command_name() {
    // case: finds a bang block and the arguments typed after a command name
    assert_eq!(
        bang_blocks("Status:\n\n!`echo hello`\n"),
        vec![BangBlock { command: "echo hello".into(), token: "!`echo hello`".into() }]
    );
    assert_eq!(bang_blocks("No bang here"), vec![]);
    assert_eq!(
        arguments_of_own_command("/team/review the notes", &["team/review"]),
        Some(("team/review".into(), "the notes".into()))
    );
    assert_eq!(arguments_of_own_command("/team/review", &["team/review"]), None);
    assert_eq!(arguments_of_own_command("/plan the work", &["team/review"]), None);
}

#[test]
fn treats_a_leading_bang_as_command_mode_and_escape_takes_the_mark_off() {
    // case: treats a leading bang as command mode, and Escape takes the mark off
    use crate::own_command::{bang, leave_command_mode};
    assert_eq!(bang("!echo hello"), Some("echo hello"));
    assert_eq!(bang("! echo hello"), Some("echo hello"));
    assert_eq!(bang("echo hello"), None);
    assert_eq!(bang(" !echo"), None);
    assert_eq!(leave_command_mode("!echo hello"), "echo hello");
    assert_eq!(leave_command_mode("!"), "");
}

/// oneShot on a Mac or Linux: the line runs as `<shell> -lc <line>` in the conversation's folder (the shell is the one
/// the terminal panel opens). PowerShell's stdin, cmd's /d /s /c and WSL's `-- bash -lc` are Windows', not this port's.
#[test]
fn runs_one_command_in_the_terminal_panel_shell() {
    // case: runs one command in the terminal panel shell
    let folder = temp("shell");
    let ran = run_line("pwd -P; echo hello", &folder.0);
    let canonical = folder.0.canonicalize().unwrap();
    assert_eq!(ran.output, format!("{}\nhello\n", canonical.display()));
    assert_eq!((ran.exit, ran.stopped), (Some(0), false));
    let failed = run_line("exit 3", &folder.0);
    assert_eq!(failed.exit, Some(3));
}

/// A fake `!` line runner, as Electron's test spawns a scripted process: what it was asked to run, and its output.
fn scripted(output: &'static str, asked: std::rc::Rc<std::cell::RefCell<Vec<(String, PathBuf)>>>) -> impl FnMut(&str, &Path) -> crate::own_command::ResultRow {
    move |command, folder| {
        asked.borrow_mut().push((command.to_owned(), folder.to_path_buf()));
        crate::own_command::ResultRow { output: output.into(), omitted: None, exit: Some(0), stopped: false, elapsed: 1 }
    }
}

#[test]
fn lists_a_nested_command_and_runs_a_global_bang_at_once() {
    // case: lists a nested command and runs a global bang at once
    let profile = temp("global-bang");
    let folder = temp("shop");
    write(&profile.0.join("commands/team/review.md"), "---\ndescription: Looks at the diff\n---\nReview\n\n!`echo hello`\n");
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let listed = commands_in(&profile.0, Some(&folder.0));
    assert_eq!(listed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["team/review"]);
    assert_eq!(listed[0].source, Source::Global);
    let expanded = expand(&profile.0, Some(&folder.0), "team/review", Source::Global, "", None, &mut scripted("hello\n", asked.clone()));
    let Expanded::Text { text, .. } = expanded else { panic!("a global command runs its bang at once: {expanded:?}") };
    assert_eq!(asked.borrow().iter().map(|(c, f)| (c.as_str(), f.clone())).collect::<Vec<_>>(), vec![("echo hello", folder.0.clone())]);
    assert!(text.contains("hello"), "{text}");
    assert!(!text.contains("!`echo hello`"), "{text}");
    // Where the output went, in Electron's words.
    assert_eq!(text, "Review\n\noutput of `echo hello` (exit 0):\nhello\n\n");
}

#[test]
fn waits_for_a_yes_before_a_project_command_runs_a_bang_then_remembers_the_folder() {
    // case: waits for a yes before a project command runs a bang, then remembers the folder
    let profile = temp("project-bang");
    let folder = temp("project");
    write(
        &folder.0.join(".locust/commands/status.md"),
        "---\ndescription: This folder\nmode: plan\nteammate: Wren\n---\nStatus $1\n\n!`echo $1`\n",
    );
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let first = expand(&profile.0, Some(&folder.0), "status", Source::Project, "notes", None, &mut scripted("notes\n", asked.clone()));
    assert_eq!(first, Expanded::Ask(vec!["echo notes".into()]));
    assert!(asked.borrow().is_empty(), "nothing runs before the yes");
    assert_eq!(allow(&profile.0, Some(&folder.0)), Ok(()));
    let second = expand(&profile.0, Some(&folder.0), "status", Source::Project, "notes", None, &mut scripted("notes\n", asked.clone()));
    assert_eq!(asked.borrow().iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), vec!["echo notes"]);
    let Expanded::Text { text, mode, teammate } = second else { panic!("allowed, it runs: {second:?}") };
    assert!(text.contains("notes"), "{text}");
    assert_eq!(mode, Some("plan"));
    assert_eq!(teammate.as_deref(), Some("Wren"));
    // Remembered in the profile, once, as Electron writes command-allows.json.
    let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(allows_path(&profile.0)).unwrap()).unwrap();
    assert_eq!(kept, serde_json::json!([folder.0.to_string_lossy()]));
    assert_eq!(allow(&profile.0, Some(&folder.0.join("."))), Ok(()));
    let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(allows_path(&profile.0)).unwrap()).unwrap();
    assert_eq!(kept, serde_json::json!([folder.0.to_string_lossy()]), "the same folder is kept once");
}

/// expand's refusals, in Electron's words: a command gone from its folder, a `!` line with the terminal turned off or
/// with no folder to run in, and Allow with no folder. A prompt with no `!` line needs neither the terminal nor a folder.
#[test]
fn a_prompt_without_a_bang_needs_no_terminal_and_each_refusal_is_electrons() {
    let profile = temp("refusals");
    write(&profile.0.join("commands/plain.md"), "---\nmode: edit\n---\nJust $ARGUMENTS\n");
    write(&profile.0.join("commands/bang.md"), "Run !`echo hi`\n");
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let off = Some("Your organization has turned the terminal off.".to_owned());
    assert_eq!(
        expand(&profile.0, None, "PLAIN", Source::Global, "this", off.clone(), &mut scripted("", asked.clone())),
        Expanded::Text { text: "Just this\n".into(), mode: Some("accept-edits"), teammate: None }
    );
    assert_eq!(
        expand(&profile.0, None, "plain", Source::Project, "", None, &mut scripted("", asked.clone())),
        Expanded::Refused("That command is not in this folder. Nothing was changed.".into())
    );
    assert_eq!(
        expand(&profile.0, Some(&profile.0), "bang", Source::Global, "", off.clone(), &mut scripted("", asked.clone())),
        Expanded::Refused(off.unwrap())
    );
    assert_eq!(
        expand(&profile.0, None, "bang", Source::Global, "", None, &mut scripted("", asked.clone())),
        Expanded::Refused("Choose a folder first. Nothing was run.".into())
    );
    assert!(asked.borrow().is_empty());
    assert_eq!(allow(&profile.0, None), Err("Choose a folder first. Nothing was allowed.".into()));
}

/// Each line's ending in Electron's words: no output, stopped, ended without a code, and what was left out.
#[test]
fn each_bang_lines_output_says_how_it_ended() {
    let profile = temp("endings");
    let folder = temp("endings-folder");
    write(&profile.0.join("commands/three.md"), "A !`one` B !`two` C !`three`");
    let mut n = 0;
    let mut run = |_: &str, _: &Path| {
        n += 1;
        match n {
            1 => crate::own_command::ResultRow { output: String::new(), omitted: None, exit: Some(0), stopped: false, elapsed: 0 },
            2 => crate::own_command::ResultRow { output: "half".into(), omitted: Some("Left out 3 lines from the start.".into()), exit: None, stopped: true, elapsed: 0 },
            _ => crate::own_command::ResultRow { output: "x".into(), omitted: None, exit: None, stopped: false, elapsed: 0 },
        }
    };
    let Expanded::Text { text, .. } = expand(&profile.0, Some(&folder.0), "three", Source::Global, "", None, &mut run) else { panic!() };
    assert_eq!(
        text,
        "A output of `one` (exit 0):\n(no output) B output of `two` (stopped before it finished):\nhalf\nLeft out 3 lines from the start. C output of `three` (ended):\nx"
    );
}

// ---- Open folder (own-command-ipc.ts `OWN_COMMAND_OPEN`, own-command-host.ts `directory`)

/// The host names the folder: yours is `<profile>/commands`, made when missing; a folder's is `<folder>/.locust/commands`.
/// With no folder open a folder's own is refused in Electron's words and nothing opens. (Electron's window-sent path
/// and other-window refusals have no channel here to refuse.)
#[test]
fn opens_a_commands_folder_the_host_names_never_one_the_window_names() {
    // case: opens a commands folder the host names, never one the window names
    let profile = temp("open");
    let folder = temp("open-folder");
    let mut opened = Vec::new();
    let mut open = |dir: &Path| {
        opened.push(dir.to_path_buf());
        Ok(())
    };
    assert_eq!(open_folder(&profile.0, Source::Global, None, &mut open), Ok(()));
    assert_eq!(
        open_folder(&profile.0, Source::Project, None, &mut open),
        Err("Choose a folder first. There is nowhere to open.".into())
    );
    assert_eq!(open_folder(&profile.0, Source::Project, Some(&folder.0), &mut open), Ok(()));
    assert_eq!(opened, vec![profile.0.join("commands"), folder.0.join(".locust").join("commands")]);
    assert!(profile.0.join("commands").is_dir() && folder.0.join(".locust/commands").is_dir(), "made when missing");
    assert_eq!(directory(&profile.0, Source::Project, None), None);
    // What the file manager says when it fails is the answer.
    assert_eq!(
        open_folder(&profile.0, Source::Global, None, &mut |_: &Path| Err("No application knows how to open it.".to_owned())),
        Err("No application knows how to open it.".into())
    );
    // A commands folder that cannot be made: ChatCommands' words, and nothing opens.
    std::fs::write(profile.0.join("blocked"), "a file, not a folder").unwrap();
    let mut never = |_: &Path| -> Result<(), String> { panic!("nothing opens") };
    assert_eq!(
        open_folder(&profile.0.join("blocked"), Source::Global, None, &mut never),
        Err("That folder could not be opened. Your commands are where they were.".into())
    );
}
