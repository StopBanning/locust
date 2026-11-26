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
