use super::*;

// case: browser: false turns pointing off too
#[test]
fn browser_policy_refusal_prevents_the_pick_command() {
    let mut s = State::default();
    {
        let mut host = crate::invariants::lock(&s.browser.host);
        host.add(crate::browser::Profile(None)).unwrap();
        host.tabs[0].url = "http://127.0.0.1:9786/".into();
        host.refusal = crate::browser::policy_refusal(
            &serde_json::json!({"browser": false}),
            std::path::Path::new("/synthetic/policy.json"),
        );
    }

    toggle(&mut s);
    assert!(s.status.contains("The browser panel is off."), "{}", s.status);
    assert!(s.page_points.waiting.is_none());
    {
        let mut host = crate::invariants::lock(&s.browser.host);
        assert!(!host.tabs[0].commands.iter().any(|c| matches!(c, crate::browser::Command::Point { .. })));
        host.refusal = None;
    }

    toggle(&mut s);
    assert!(s.page_points.waiting.is_some());
    let host = crate::invariants::lock(&s.browser.host);
    assert_eq!(host.tabs[0].commands.iter().filter(|c| matches!(c, crate::browser::Command::Point { .. })).count(), 1);
}
