use super::*;

fn own_actions_poll(rt: &mut rune::Runtime<State>) {
    let until = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        rt.update(|s, tx| {
            crate::own_commands_ui::poll(s, tx);
        });
        frames(rt);
        if !rt.state.own_commands_job.is_pending() && !rt.state.own_commands_expand_job.is_pending()
        {
            rt.update(|s, tx| {
                crate::own_commands_ui::poll(s, tx);
            });
            frames(rt);
            return;
        }
        assert!(
            std::time::Instant::now() < until,
            "command job did not land"
        );
        std::thread::sleep(Duration::from_millis(3));
    }
}

#[test]
fn composer_clicking_a_project_bang_command_asks_first_and_not_now_runs_nothing() {
    let (root, mut s) = fresh();
    s.home.home_open = true;
    s.teammates[s.selected].folder = Some(root.0.join("project").to_string_lossy().into_owned());
    let folder = root.0.join("project/.locust/commands");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("status.md"),
        "Status:\n\n!`echo project-bang`\n",
    )
    .unwrap();
    let mut rt = window(s);
    own_actions_poll(&mut rt);
    type_words(&mut rt, "/status");
    click_word(&mut rt, "/status");
    own_actions_poll(&mut rt);
    assert!(
        rt.state.own_commands.ask.is_some(),
        "command did not wait: draft {:?}, project {:?}, commands {:?}",
        rt.state.draft.text,
        crate::own_commands_ui::context(&rt.state).and_then(|c| c.folder()),
        rt.state.own_commands.listed,
    );
    let drawn = words(&rt);
    for text in [
        "Allow this folder's commands",
        "echo project-bang",
        "Allow",
        "Not now",
    ] {
        assert!(drawn.contains(text), "{text} missing");
    }
    assert_eq!(rt.state.draft.text, "/status");
    assert!(rt.state.flight.is_none());
    click(&mut rt, "command-ask-not-now");
    assert!(rt.find("command-ask").is_none());
    assert_eq!(rt.state.draft.text, "/status");
    assert!(!locust_core::own_commands::allowed(
        &root.0.join("electron"),
        &root.0.join("project")
    ));
    assert!(rt.state.sent_history.is_empty());
}
