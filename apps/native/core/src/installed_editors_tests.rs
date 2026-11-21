//! Electron's `main/installed-editors.test.ts`, with its exact values. The cmd lines are the ones Electron's
//! `cmdLauncherLine` builds for the same input (read from Node, 21 Nov).

use super::*;

// case: editors are taken from PATH > lists one that is on PATH and skips one that is not, without guessing an install folder
#[test]
fn lists_one_that_is_on_path_and_skips_one_that_is_not_without_guessing_an_install_folder() {
    // A Windows PATH, so it is built and read by Windows' rules whatever machine runs the test.
    let there = ["C:\\tools\\code.cmd", "C:\\tools\\cursor.exe"];
    let found = editors_on_path("win32", Some("C:/empty;C:/tools"), &|file| there.contains(&file));
    assert_eq!(
        found,
        vec![
            InstalledEditor { id: EditorId::Vscode, command: "C:\\tools\\code.cmd".into() },
            InstalledEditor { id: EditorId::Cursor, command: "C:\\tools\\cursor.exe".into() },
        ]
    );
    assert!(editors_on_path("win32", Some("C:/empty"), &|_| false).is_empty());
    assert_eq!(
        editors_on_path("linux", Some("/usr/bin"), &|file| file == "/usr/bin/code"),
        vec![InstalledEditor { id: EditorId::Vscode, command: "/usr/bin/code".into() }]
    );
}

#[test]
fn takes_nothing_from_a_relative_folder_or_no_path_at_all() {
    let anything = |_: &str| true;
    assert!(editors_on_path("linux", Some("bin:./tools:"), &anything).is_empty());
    assert!(editors_on_path("win32", Some("tools;C:relative"), &anything).is_empty());
    assert!(editors_on_path("darwin", None, &anything).is_empty());
    // A UNC folder is absolute.
    assert_eq!(
        editors_on_path("win32", Some("\\\\server\\share"), &|f| f == "\\\\server\\share\\code.cmd")[0].command,
        "\\\\server\\share\\code.cmd"
    );
}

// case: editors are taken from PATH > keeps a file as an argument, and runs a Windows command shim through System32 cmd.exe, verbatim
#[test]
fn keeps_a_file_as_an_argument_and_runs_a_windows_command_shim_through_system32_cmd_exe_verbatim() {
    let shim = editor_spawn("C:/tools/code.cmd", "C:/work/app.ts", "win32", Some("C:\\Windows")).unwrap();
    assert_eq!(shim.file, "C:\\Windows\\System32\\cmd.exe");
    assert!(shim.verbatim);
    assert_eq!(shim.args, vec!["/d /s /c \"C:/tools/code.cmd ^\"C:/work/app.ts^\"\"".to_owned()]);
    assert_eq!(
        editor_spawn("/usr/bin/code", "/work/app.ts", "linux", None),
        Some(EditorSpawn { file: "/usr/bin/code".into(), args: vec!["/work/app.ts".into()], verbatim: false })
    );
    assert_eq!(
        editor_spawn("C:/tools/cursor.exe", "C:/work/a&b.ts", "win32", None),
        Some(EditorSpawn { file: "C:/tools/cursor.exe".into(), args: vec!["C:/work/a&b.ts".into()], verbatim: false })
    );
    // With no SystemRoot, still System32's cmd.exe, never a bare name a working folder could plant.
    assert_eq!(editor_spawn("C:/tools/code.CMD", "x", "win32", None).unwrap().file, "C:\\Windows\\System32\\cmd.exe");
    assert!(editor_spawn("C:/tools/old.bat", "x", "win32", None).unwrap().verbatim);
    // A .cmd off Windows is just a file to run.
    assert!(!editor_spawn("/opt/code.cmd", "x", "darwin", None).unwrap().verbatim);
}

// case: editors are taken from PATH > passes a file named with & to the shim whole, and runs nothing else
#[test]
fn passes_a_file_named_with_an_ampersand_to_the_shim_whole_and_runs_nothing_else() {
    // Electron runs this one against real cmd.exe on Windows. Here, the line it hands cmd.exe: every & and space
    // caret-escaped inside the quoted argument, so `mkdir pwned` stays text.
    let launch = editor_spawn("C:\\Program Files\\code.cmd", "C:\\w\\a&mkdir pwned&b.txt", "win32", Some("C:\\Windows")).unwrap();
    assert_eq!(launch.args, vec!["/d /s /c \"C:\\Program^ Files\\code.cmd ^\"C:\\w\\a^&mkdir^ pwned^&b.txt^\"\"".to_owned()]);
}

#[test]
fn escapes_quotes_and_backslashes_as_electron_does_and_refuses_a_line_break() {
    assert_eq!(
        cmd_launcher_line("x.cmd", &["a\\\"b\\\\\"c\"d\\"]).unwrap(),
        "/d /s /c \"x.cmd ^\"a\\\\\\^\"b\\\\\\\\^\"c\\^\"d\\\\^\"\""
    );
    assert_eq!(
        cmd_launcher_line("x.cmd", &["a\nb"]).unwrap_err(),
        "This runtime is started through a .cmd launcher, and cmd.exe cannot pass a line break in an argument. Nothing was started."
    );
    assert!(cmd_launcher_line("x\r.cmd", &[]).is_err());
    assert_eq!(editor_spawn("C:/code.cmd", "a\r\nb", "win32", None), None);
}
