//! The model picker's list, drawn headless through `thread_view::model_picker`: Electron's
//! `a-model-is-one-line.test.tsx`, `older-claude-versions-are-folded.test.tsx`'s drawn cases and
//! `the-picker-opens-on-what-works.test.ts`'s order as the window applies it. The rules' own cases are in
//! `core/src/route_rows_tests.rs` and `core/src/models_tests.rs`.
use super::*;
use locust_core::engine::{Model, Models, Teammate};
use rune::{Env, Runtime};
use serde_json::Value;

const T: &str = "tm_pick";

fn mate() -> Teammate {
    Teammate {
        id: T.into(),
        name: "Wren".into(),
        role: "Coder".into(),
        runtime: Some("claude".into()),
        model: Some("opus".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let model = s.picks.get(T).map(|p| p.model.clone()).unwrap_or_else(|| "opus".into());
    col().w_full().child(crate::thread_view::model_picker(s, &crate::look::controls::Look::new(true), T, &mate(), None, &model, None, false))
}

fn m(id: &str, name: &str, efforts: &[&str]) -> Model {
    Model { id: id.into(), name: name.into(), efforts: efforts.iter().map(|e| (*e).to_owned()).collect() }
}

fn agent(runtime: &str, models: Vec<Model>) -> Models {
    Models { runtime: runtime.into(), model: None, effort: None, teammate: None, models }
}

fn picker(agents: Vec<Models>, pick: Option<&str>) -> Runtime<State> {
    let mut s = State::default();
    s.models.insert(T.into(), agents);
    if let Some(p) = pick {
        s.picks.insert(T.into(), TurnPick { model: p.into(), effort: None, runtime: None, account: None });
    }
    let mut rt = Runtime::new(s, view, (600.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|n| n.text).collect()
}

/// The texts drawn inside the element keyed `key`.
fn text_in(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect()
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

const FIVE: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

// --- a-model-is-one-line.test.tsx

#[test]
fn keeps_what_the_catalogue_says_and_leaves_the_effort_levels_to_the_hover() {
    let mut rt = picker(vec![agent("claude", vec![m("fable", "Fable 5.1", &FIVE)])], Some("fable"));
    let line = text_in(&rt, "model-claude-fable");
    assert!(line.iter().any(|t| t == "Always the newest Fable"), "{line:?}");
    assert!(!line.iter().any(|t| t.contains("xhigh")), "the levels are the hover's: {line:?}");
    let (x, y) = rt.find("model-claude-fable").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("model-claude-fable-tip-tooltip")).expect("the row's hover");
    assert_eq!(tip.label.as_deref(), Some("Fable 5.1 · Always the newest Fable · 5 effort levels · low, medium, high, xhigh, max"));
    assert!(tip.visible[2] > 0.0 && tip.visible[3] > 0.0, "the hover is not clipped away: {:?}", tip.visible);
}

#[test]
fn a_model_the_catalogue_says_nothing_about_keeps_its_levels_as_its_line_in_words() {
    let rt = picker(vec![agent("claude", vec![m("claude-sonnet-5-5", "Sonnet 5.5", &["low", "high"])])], None);
    assert!(text_in(&rt, "model-claude-claude-sonnet-5-5").iter().any(|t| t == "Low, High"));
    let cursorish = ["low", "low-fast", "medium", "high", "xhigh", "max", "max-fast"];
    let rt = picker(vec![agent("cursor", vec![m("claude-opus-5-5", "Opus 5.5", &cursorish)])], None);
    assert!(text_in(&rt, "model-cursor-claude-opus-5-5").iter().any(|t| t == "Low to Max · Fast"), "{:?}", texts(&rt));
}

// --- older-claude-versions-are-folded.test.tsx: the fold

fn claude_now() -> Vec<Models> {
    vec![agent("claude", vec![m("opus", "Opus 5.5", &[])])]
}

#[test]
fn keeps_older_versions_out_of_the_list_until_it_is_opened_and_says_how_many_it_holds() {
    let mut rt = picker(claude_now(), Some("opus"));
    let said = texts(&rt);
    assert!(said.iter().any(|t| t == "Opus 5.5"));
    assert!(!said.iter().any(|t| t == "Opus 4.8" || t == "Sonnet 4.6"), "{said:?}");
    let fold = node(&rt.accessibility_json(), "picker-fold-claude").cloned().expect("the fold");
    assert!(fold["states"].as_array().is_some_and(|s| s.iter().any(|v| v == "collapsed")), "{fold}");
    // Claude Code's registry holds ten older versions; none is what an alias means now.
    assert_eq!(text_in(&rt, "picker-fold-count-claude"), ["10"]);
    let (x, y) = rt.find("picker-fold-claude").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(texts(&rt).iter().any(|t| t == "Opus 4.8"), "opened, it shows them");
}

#[test]
fn never_folds_away_the_route_in_use() {
    let rt = picker(claude_now(), Some("claude-opus-4-8"));
    assert!(texts(&rt).iter().any(|t| t == "Opus 4.8"));
    assert_eq!(text_in(&rt, "picker-fold-count-claude"), ["9"], "what is left under the fold is only the others");
}

#[test]
fn lets_a_search_reach_into_the_fold_without_opening_it() {
    let mut s = State::default();
    s.models.insert(T.into(), claude_now());
    s.model_query.text = "opus 4.8".into();
    let mut rt = Runtime::new(s, view, (600.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(texts(&rt).iter().any(|t| t == "Opus 4.8"), "{:?}", texts(&rt));
    assert!(rt.find("picker-fold-claude").is_none(), "searched, nothing is folded");
}

// --- the-picker-opens-on-what-works.test.ts: the window's group order

fn group_order(rt: &Runtime<State>) -> Vec<String> {
    let names = ["Claude Code", "Codex CLI", "OpenCode"];
    texts(rt).into_iter().filter(|t| names.contains(&t.as_str())).collect()
}

#[test]
fn the_window_lifts_what_can_run_and_sorts_alphabetically_inside_a_band() {
    // Listed codex, opencode, claude; codex is at its limit.
    let agents = vec![
        agent("codex", vec![m("gpt-5.6", "GPT-5.6", &[])]),
        agent("opencode", vec![m("opencode/beam-free", "Beam Free", &[])]),
        agent("claude", vec![m("sonnet", "Sonnet 5.5", &[])]),
    ];
    let mut s = State::default();
    s.models.insert(T.into(), agents);
    s.limited_runtimes.insert(locust_core::agent_accounts::usage_key("codex", None), "limited".into());
    let mut rt = Runtime::new(s, view, (600.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(group_order(&rt), ["Claude Code", "OpenCode", "Codex CLI"]);
}

#[test]
fn a_claude_accounts_group_has_its_own_fold_keyed_as_its_rows_are() {
    let mut s = State::default();
    s.models.insert(T.into(), claude_now());
    s.agent_accounts.accounts.push(locust_core::agent_accounts::Account {
        id: "account_work".into(),
        agent: locust_core::agent_accounts::Agent::Claude,
        name: "Work".into(),
        state: locust_core::agent_accounts::AccountState::Ready,
    });
    let mut rt = Runtime::new(s, view, (600.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.find("picker-fold-claude").is_some() && rt.find("picker-fold-claude:account_work").is_some());
    let (x, y) = rt.find("picker-fold-claude:account_work").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.find("model-claude-claude-opus-4-8:account_work").is_some(), "the account's older rows, under its own keys");
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("model-claude-claude-opus-4-8")).collect();
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(keys.len(), unique.len(), "no key drawn twice: {keys:?}");
}
