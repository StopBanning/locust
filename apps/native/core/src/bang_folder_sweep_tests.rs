
#[cfg(unix)]
// case: runs in the conversation folder, in the terminal panel's default shell, not a path the window sent
#[test]
fn sweep_bang_uses_the_selected_folder_and_the_same_shell_as_the_terminal() {
    let dir = std::env::temp_dir().join(format!("locust-bang-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir(&dir).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let _remove = Remove(dir.clone());
    let result = crate::own_commands::run_line("printf '%s\n' \"$0\"; pwd -P; echo hello", &dir);
    assert_eq!(result.exit, Some(0), "{}", result.output);
    assert!(!result.stopped);
    let shell = crate::terminal::shell();
    let folder = std::fs::canonicalize(&dir).unwrap();
    assert!(result.output.lines().any(|line| line == shell.to_string_lossy() || shell.file_name().is_some_and(|file| line.trim_start_matches('-') == file.to_string_lossy())));
    assert!(result.output.lines().any(|line| line == folder.to_string_lossy()));
    assert!(result.output.ends_with("hello\n"));
}
