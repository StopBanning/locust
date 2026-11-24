//! Settings › AI agents, "Use for new conversations" (Electron `AgentAccountsSettings.tsx` 0.876): a ready account that
//! is not the one new conversations use offers the button, the one that is says so, and the press moves new
//! conversations through `Accounts::use_for_new`. Driven in the real view with no screen, as a person would.
use super::*;
use locust_core::agent_accounts::{Accounts, Cancel};
use rune::{Env, Runtime};
use std::path::Path;

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn a_ready_account_can_be_picked_for_new_conversations_and_back_to_this_computer() {
    let _reset = test_guard();
    let root = std::env::temp_dir().join(format!("locust-for-new-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    let dones: Arc<Mutex<Vec<Box<dyn FnOnce(bool) + Send>>>> = Arc::default();
    let held = dones.clone();
    let login: Arc<locust_core::agent_accounts::Login> = Arc::new(move |_: Agent, _: &Path, _: &[(&'static str, String)], done: Box<dyn FnOnce(bool) + Send>| {
        held.lock().unwrap().push(done);
        Ok(Box::new(|| {}) as Cancel)
    });
    let added = Accounts::new(&root, login, Box::new(|| None), Box::new(|_, _| false));
    let work = added.add("codex", "Work").unwrap().id;
    let home = added.add("codex", "Home").unwrap().id;
    // Work signs in first and takes new conversations; Home signs in after and does not.
    added.login(&work).unwrap();
    (dones.lock().unwrap().remove(0))(true);
    added.login(&home).unwrap();
    (dones.lock().unwrap().remove(0))(true);
    assert_eq!(added.for_new("codex").unwrap().as_deref(), Some(work.as_str()));
    use_accounts(added);

    let data = root.join("data");
    std::fs::create_dir_all(data.join("electron")).unwrap();
    let store = crate::store::Store::new(data.join("native"), data.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    let mut rt = Runtime::new(s, crate::view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.update(|s, _| {
        crate::settings_panel::open(s);
        s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    });
    settle(&mut rt);
    let (w, h) = (work[8..16].to_owned(), home[8..16].to_owned());

    let work_row = text_in(&rt, &format!("account-{w}"));
    assert!(work_row.contains("Signed in. New conversations use this account."), "{work_row}");
    assert!(rt.find(&format!("account-for-new-{w}")).is_none(), "the account new conversations use offers no button");
    let home_row = text_in(&rt, &format!("account-{h}"));
    assert!(home_row.contains("Signed in.") && !home_row.contains("New conversations use this account."), "{home_row}");
    assert!(home_row.contains("Use for new conversations"), "{home_row}");
    assert!(rt.find("account-for-new-this-computer-codex").is_some(), "this computer's sign-in can take new conversations back");
    // Lane check 23 Nov: a Codex account taking new conversations says nothing about Claude, whose new conversations
    // still use this computer's sign-in, so Claude's row offers no button.
    assert!(rt.find("account-this-computer-claude").is_some(), "Claude's own block is drawn");
    assert!(rt.find("account-for-new-this-computer-claude").is_none(), "Codex's pick does not move Claude off this computer");

    // Home is picked: new conversations move to it, and Work offers the button now.
    click(&mut rt, &format!("account-for-new-{h}"));
    let picked = rt.state.agent_accounts.accounts.len();
    assert_eq!(picked, 2, "{:?}", rt.state.agent_accounts.message);
    let now = accounts(&rt.state).and_then(|a| a.for_new("codex").ok().flatten());
    assert_eq!(now.as_deref(), Some(home.as_str()));
    assert!(text_in(&rt, &format!("account-{h}")).contains("Signed in. New conversations use this account."));
    assert!(rt.find(&format!("account-for-new-{h}")).is_none());
    assert!(rt.find(&format!("account-for-new-{w}")).is_some());

    // Back to this computer's sign-in: no added account has new conversations, and both offer the button.
    click(&mut rt, "account-for-new-this-computer-codex");
    let now = accounts(&rt.state).and_then(|a| a.for_new("codex").ok().flatten());
    assert_eq!(now, None);
    assert!(rt.find("account-for-new-this-computer-codex").is_none());
    assert!(rt.find(&format!("account-for-new-{w}")).is_some() && rt.find(&format!("account-for-new-{h}")).is_some());

    clear_accounts();
    let _ = std::fs::remove_dir_all(&root);
}
