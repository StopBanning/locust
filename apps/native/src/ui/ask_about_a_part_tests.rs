//! a-part-of-a-reply-can-be-asked-about.test.tsx through the window: words chosen in a reply, Ask about this pressed
//! (`selection_ask_chip`), and the quote in the conversation's own message box; and the Import a conversation dialog
//! drawn (`import_dialog::view`), before its list arrives and with sessions in it. Expected words are Electron's.
use super::*;
use rune::{Env, Runtime};

/// `chosen` selected in a reply, the chip pressed; what the message box then holds.
fn asked_about(chosen: &str) -> String {
    let mut s = State::default();
    s.selection_ask = Some(SelectionAskState { scope: "m1/ledger-item-1".into(), text: chosen.into() });
    let mut rt = Runtime::new(s, |s, _| col().w(800.0).h(400.0).child(selection_ask_chip(s, &Palette::new(true)).unwrap_or_else(el)), (800.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (x, y) = rt.find("selection-ask").expect("the Ask about this chip");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.selection_ask.is_none(), "the chip goes once pressed");
    rt.state.draft.text.clone()
}

// case: is quoted line by line, blank lines kept as quote lines
#[test]
fn is_quoted_line_by_line_blank_lines_kept_as_quote_lines() {
    assert_eq!(asked_about("The rank is at most 3.\n\n\n\nSo AB is singular."), "> The rank is at most 3.\n>\n> So AB is singular.\n\n");
}

// case: is cut past its limit, and says so with an ellipsis
#[test]
fn is_cut_past_its_limit_and_says_so_with_an_ellipsis() {
    // Electron's MAX_QUOTE is 1,500; the quote is the box's text before the blank line under it.
    let boxed = asked_about(&"x".repeat(1_500 + 50));
    let quote = boxed.strip_suffix("\n\n").expect("a blank line under the quote");
    assert!(quote.encode_utf16().count() <= 1_500 + 2, "{}", quote.len());
    assert!(quote.ends_with('\u{2026}'), "{quote}");
}

fn dialog(sessions: Option<Vec<locust_core::session_import::ImportableSession>>) -> Vec<String> {
    let mut s = State::default();
    s.import_dialog.open = true;
    s.import_dialog.sessions = sessions;
    let mut rt = Runtime::new(s, |s, _| crate::import_dialog::view(s, &Palette::new(true)), (700.0, 700.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

// case: says it is looking before the list arrives, and names both runtimes
#[test]
fn says_it_is_looking_before_the_list_arrives_and_names_both_runtimes() {
    let said = dialog(None);
    assert!(said.iter().any(|t| t == "Import a conversation"), "{said:?}");
    assert!(said.iter().any(|t| t.contains("the last 30 days")), "{said:?}");
    assert!(said.iter().any(|t| t == "Looking for your Claude Code, Codex, OpenCode and Cursor Agent sessions…"), "{said:?}");
}

// case: says ages the way the sidebar does
#[test]
fn says_ages_the_way_the_sidebar_does() {
    let now = chrono::Utc::now();
    let ago = |minutes: i64, id: &str| locust_core::session_import::ImportableSession {
        runtime: locust_core::session_import::ImportRuntime::Claude,
        session_id: id.into(),
        title: format!("Session {id}"),
        cwd: "/work/app".into(),
        folder_name: "app".into(),
        updated_at: (now - chrono::Duration::minutes(minutes)).to_rfc3339(),
        bytes: 10,
        open_now: false,
        folder_missing: false,
    };
    // Electron's three: 3 minutes, 5 hours and 3 days before now.
    let said = dialog(Some(vec![ago(3, "a"), ago(5 * 60, "b"), ago(3 * 24 * 60, "c")]));
    for age in ["Claude Code · app · 3m", "Claude Code · app · 5h", "Claude Code · app · 3d"] {
        assert!(said.iter().any(|t| t == age), "{age}: {said:?}");
    }
}
