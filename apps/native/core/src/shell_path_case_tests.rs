use super::login_shell_path_with_runner;

// case: reads only what is between the markers, whatever a shell profile prints
#[test]
fn noisy_login_shell_output_and_runner_timeout() {
    let said = login_shell_path_with_runner(Some("/bin/zsh"), "/bin/bash", |shell, args| {
        assert_eq!(shell, "/bin/zsh");
        assert_eq!(args, ["-ilc", "printf \"__LOCUST_PATH__%s__LOCUST_PATH__\" \"$PATH\""]);
        Some("Welcome back!\n__LOCUST_PATH__/opt/homebrew/bin:/usr/bin__LOCUST_PATH__".into())
    });
    assert_eq!(said.as_deref(), Some("/opt/homebrew/bin:/usr/bin"));

    // The failed run supplies no stdout, just like Electron's throwing runner.
    let timed_out = login_shell_path_with_runner(Some("/bin/zsh"), "/bin/bash", |_, _| None);
    assert_eq!(timed_out, None);
}
