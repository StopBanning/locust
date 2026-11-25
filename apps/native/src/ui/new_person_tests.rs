//! Electron's `a-new-person-is-met-where-they-are.test.tsx`, the cases Rust builds, with Electron's values and words:
//! a teammate's starters in an empty folder (`idle_teammate.rs`'s `starters`, Electron's `startersFor`), which
//! teammate a starter row goes to (`starters::teammate_for_starter`), and Home's usage line
//! (`usage::usage_summary`, `usage_line_facts`). Two cases read Electron's `App.tsx` and `FirstLaunch.tsx` source.
use super::*;
use crate::starters::{TEAM_TEMPLATES, TemplateTeammate, teammate_for_starter};
use locust_core::usage::{UsageRange, UsageTurn, usage_line_facts, usage_summary};
use serde_json::json;

fn mate(team: &str, name: &str) -> &'static TemplateTeammate {
    TEAM_TEMPLATES.iter().find(|t| t.template_id == team).unwrap().teammates.iter().find(|m| m.name == name).unwrap()
}
fn starters_for(m: &TemplateTeammate, fresh: bool) -> Vec<String> {
    starters(&json!({ "starters": m.starters }), m.role, fresh)
}
fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| s.to_string()).collect()
}

#[test]
fn in_an_empty_folder_offers_to_make_something_not_to_read_a_project_that_is_not_there() {
    let wren = mate("software", "Wren");
    let fresh = starters_for(wren, true);
    assert_eq!(fresh, owned(wren.fresh_starters.expect("Wren has fresh starters")));
    let joined = fresh.join(" ").to_lowercase();
    assert!(!["this project", "this repo", "riskiest"].iter().any(|w| joined.contains(w)), "{joined}");
    assert!(starters(&json!({}), "Code & Migrations", true).join(" ").contains("Make a small web page here"));
}

#[test]
fn offers_its_own_starters_once_the_folder_holds_something_and_one_without_fresh_ones_keeps_its_own() {
    let wren = mate("software", "Wren");
    assert_eq!(starters_for(wren, false), owned(wren.starters));
    let sable = mate("money", "Sable");
    assert_eq!(starters_for(sable, true), owned(sable.starters));
}

/// The function's half; that the window passes the starter's id and the team's roles is Electron's `App.tsx` source.
#[test]
fn sends_a_starter_row_to_the_teammate_whose_work_it_is() {
    let team: Vec<(String, String)> = [("wren", "Code & Migrations"), ("juno", "Docs & QA"), ("atlas", "Research & Briefs")]
        .iter().map(|(id, role)| (id.to_string(), role.to_string())).collect();
    assert_eq!(teammate_for_starter("page", &team).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("debug", &team).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("review", &team).as_deref(), Some("juno"));
    assert_eq!(teammate_for_starter("review", &team[..1]).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("page", &[("sable".into(), "Research & Briefs".into())]), None);
}

fn at(hour: u32) -> chrono::DateTime<chrono::Local> {
    use chrono::TimeZone;
    chrono::Local.with_ymd_and_hms(2026, 10, 30, hour, 0, 0).single().unwrap()
}
fn turn(runtime: &str, model: &str, conversation: &str, plan: bool) -> UsageTurn {
    UsageTurn { at: at(20).to_rfc3339(), runtime: runtime.into(), model: model.into(), conversation: conversation.into(),
        input_tokens: Some(58_000), output_tokens: Some(1_000), cache_read_tokens: None, cache_write_tokens: None, usd: None,
        premium_requests: None, plan }
}

#[test]
fn the_usage_line_says_all_free_when_every_turn_was_on_a_free_model() {
    let turns = [turn("opencode", "deepseek-v4.1-flash-free", "c1", false)];
    assert_eq!(usage_line_facts(&usage_summary(&turns, UsageRange::Days30, at(21))), "1 turn · 59k tokens · all free");
}

#[test]
fn the_usage_line_says_nothing_of_free_when_any_turn_was_not() {
    let turns = [turn("opencode", "deepseek-v4.1-flash-free", "c1", false), turn("claude", "claude-opus-4-6", "c2", true)];
    assert!(!usage_line_facts(&usage_summary(&turns, UsageRange::Days30, at(21))).contains("free"));
}

/// One teammate, Wren, idle in `folder`, as the conversation view draws her.
fn wren_in(folder: &std::path::Path) -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates = vec![Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: None,
        model: None, account: None, busy: false, runs_in: "accept-edits".into(), folder: None }];
    s.selected = 0;
    s.conversations.insert("tm_wren".into(), Conversation::default());
    s.people_panel.team = json!({"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"}]});
    s.home.folder_path = Some(folder.to_string_lossy().into_owned());
    s
}
fn words_of(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

// case: is told the folder is empty by the window, which asks again after each turn
/// Electron's App.tsx hands IdleTeammate `folderEmpty={workspaceEmpty}` and sets it again from each turn's answer. Here
/// the window asks as it draws the conversation (`folder_holds_nothing_yet`), at most every two seconds: the SAME window,
/// still open after a turn wrote the folder's first file, stops offering to make something and offers Wren's own.
#[test]
fn is_told_the_folder_is_empty_by_the_window_which_asks_again_after_each_turn() {
    let folder = std::env::temp_dir().join(format!("locust-new-person-asks-again-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    struct Gone(std::path::PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _gone = Gone(folder.clone());
    let wren = mate("software", "Wren");
    let fresh = wren.fresh_starters.expect("Wren has fresh starters")[0];
    let own = wren.starters[0];
    let mut rt = rune::Runtime::new(wren_in(&folder), |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
        (1200.0, 1800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let empty = words_of(&rt);
    assert!(empty.contains(fresh) && !empty.contains(own), "an empty folder: {empty}");
    // A turn writes the folder's first file.
    std::fs::write(folder.join("index.html"), "<h1>Crumb</h1>").unwrap();
    // Within two seconds the window keeps the answer it had: the view is drawn far more often than a folder changes.
    rt.update(|s, _| s.status = "a turn ended".into());
    rt.frame(0.016);
    assert!(words_of(&rt).contains(fresh), "asked at most every two seconds");
    // Past them, the next drawing asks again, and the folder now holds something.
    rt.update(|s, _| {
        let mut seen = s.folder_empty_seen.borrow_mut();
        let (_, at, _) = seen.as_mut().expect("the window asked the folder when it drew Wren");
        *at = at.checked_sub(std::time::Duration::from_secs(3)).unwrap();
    });
    rt.frame(0.032);
    let held = words_of(&rt);
    assert!(held.contains(own) && !held.contains(fresh), "asked again after the turn: {held}");
}

// case: keeps the first-run line and its free model until the first conversation
/// FirstLaunch.tsx draws `lc-intro` while `teammateCount === 0 || noConversationsYet`, and App.tsx passes
/// `noConversationsYet={history.length === 0}`: a team just made does not end the first run, its first conversation
/// does. Rust's Home (`home_panel::intro_line`, drawn as `home-intro`) keeps the line, with its promise that OpenCode
/// works without an account, until `recent` lists a conversation. (Rust's Home has no "Use a free model" press beside
/// it; the promise is the free model the line keeps.)
#[test]
fn keeps_the_first_run_line_and_its_free_model_until_the_first_conversation() {
    use locust_core::agents::{Auth, Found, Probe};
    let opencode = Found { id: "opencode".into(), display_name: "OpenCode".into(), installed: true, version: Some("1.18.27".into()),
        auth: Auth::Authenticated, ready: true, status: Probe::Ready, unsupported_note: None, through_cli: false, checking: false };
    let intro = |s: State| -> Option<String> {
        let mut rt = rune::Runtime::new(s, |s, _| crate::home_panel::home_view(s, &Palette::new(true)), (1215.0, 4000.0),
            rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some("home-intro")).and_then(|e| e.text)
    };
    let home = |teammates: usize, conversations: usize| {
        let mut s = State::default();
        s.home.folder_path = Some("/Users/someone/Documents/Locust".into());
        s.settings_panel.agents = Some(vec![opencode.clone()]);
        for n in 0..teammates {
            s.teammates.push(Teammate { id: format!("tm_{n}"), name: format!("Mate {n}"), role: "Code & Migrations".into(),
                runtime: Some("opencode".into()), model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
        }
        for n in 0..conversations {
            s.recent.push(locust_core::engine::ConversationRow { id: format!("mission_{n}"), teammate: "Mate 0".into(),
                title: "Make a small web page here.".into(), started_by: "the person".into(), state: "completed".into(),
                updated: "2026-10-30T20:00:00.000Z".into() });
        }
        s
    };
    let with_free = Some(crate::boot_view::INTRO_WITH_FREE.to_owned());
    assert_eq!(intro(home(0, 0)), with_free, "a first run");
    assert_eq!(intro(home(3, 0)), with_free, "a team just made, nothing said to it yet: still a first run");
    assert_eq!(intro(home(3, 1)), None, "the first conversation ends it");
    assert_eq!(intro(home(0, 1)), with_free, "with no teammate at all, Home is a first run again");
}
