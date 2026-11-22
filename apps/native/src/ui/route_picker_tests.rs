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

fn chip_runtime(runtime: &str, model: &str, model_display: &str, resolved: Option<(&str, &str)>) -> Runtime<State> {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        account: None,
        busy: false,
        runs_in: "local".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.models.insert(
        "tm_1".into(),
        vec![engine::Models {
            runtime: runtime.into(),
            model: None,
            effort: None,
            teammate: None,
            models: vec![engine::Model {
                id: model.into(),
                name: model_display.into(),
                efforts: vec![],
            }],
        }],
    );
    s.settings_panel.agents = Some(vec![locust_core::agents::Found {
        id: runtime.into(),
        display_name: runtime.into(),
        installed: true,
        version: None,
        auth: locust_core::agents::Auth::Authenticated,
        ready: true,
        status: locust_core::agents::Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }]);
    if let Some((k, v)) = resolved {
        s.resolved_model_names.insert(k.into(), v.into());
    }
    fn chip_view(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        crate::thread_view(s, &p)
    }
    let mut rt = Runtime::new(s, chip_view, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

// a-default-route-names-its-model.test.tsx
#[test]
fn names_the_model_the_default_turned_out_to_be_once_a_run_has_said() {
    let mut rt = chip_runtime(
        "claude",
        "account-default",
        "Account default",
        Some(("claude:account-default", "claude-opus-5-5")),
    );
    let chip_words = text_in(&rt, "model-chip").join(" ");
    assert!(chip_words.contains("Opus 5.5"), "got: {chip_words}");
    assert!(!chip_words.contains("account-default"));
    let (x, y) = rt.find("model-chip").expect("model-chip");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("model-chip-tip-tooltip"));
    let tip_text = tip.and_then(|e| e.label).unwrap_or_default();
    assert!(tip_text.contains("its own default model, Opus 5.5 on its last run"), "got: {tip_text}");
    assert!(!tip_text.contains("account-default"));
}

// a-default-route-names-its-model.test.tsx
#[test]
fn reads_as_the_runtime_alone_until_then_and_never_as_its_id() {
    let mut rt = chip_runtime("claude", "account-default", "Account default", None);
    let chip_words = text_in(&rt, "model-chip").join(" ");
    assert!(chip_words.contains("Claude"), "got: {chip_words}");
    assert!(!chip_words.contains("No AI agent"));
    assert!(!chip_words.contains("account-default"));
    assert!(!chip_words.contains("Account default"));
    let (x, y) = rt.find("model-chip").expect("model-chip");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("model-chip-tip-tooltip"));
    let tip_text = tip.and_then(|e| e.label).unwrap_or_default();
    assert!(tip_text.contains("its own default model"), "got: {tip_text}");
    assert!(!tip_text.contains("account-default"));
}

// the-chip-keeps-free.test.ts
#[test]
fn the_chip_draws_free_as_its_own_tag_outside_the_truncated_name() {
    let rt = chip_runtime(
        "opencode",
        "opencode/muse-spark-1.3-contributor-free",
        "Muse Spark 1.3 Contributor Free",
        None,
    );
    assert!(rt.find("model-chip-free").is_some(), "Free tag should be drawn as its own element");
    assert_eq!(text_in(&rt, "model-chip-free"), vec!["Free"]);
    let chip_words = text_in(&rt, "model-chip");
    assert!(chip_words.iter().any(|w| w == "Muse Spark 1.3 Contributor"), "got: {chip_words:?}");
}

// routeChipReading.test.ts
#[test]
fn the_model_and_its_effort_read_as_one_control() {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("claude".into()),
        model: Some("haiku".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.models.insert(
        "tm_1".into(),
        vec![engine::Models {
            runtime: "claude".into(),
            model: None,
            effort: Some("medium".into()),
            teammate: None,
            models: vec![engine::Model { id: "haiku".into(), name: "Haiku 5.5".into(), efforts: vec!["medium".into()] }],
        }],
    );
    fn view_chip(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_chip, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(text_in(&rt, "model-chip").iter().any(|w| w == "Haiku 5.5 · Medium"));

    rt.update(|s, _| {
        s.picks.insert("tm_1".into(), TurnPick { model: "opus".into(), effort: Some("high-fast".into()), runtime: None, account: None });
        s.models.get_mut("tm_1").unwrap()[0].models.push(engine::Model { id: "opus".into(), name: "Opus 5.5".into(), efforts: vec!["high-fast".into()] });
    });
    rt.frame(0.016);
    assert!(text_in(&rt, "model-chip").iter().any(|w| w.contains("Opus 5.5 · High · Fast")));
}

// the-header-says-money-or-nothing.test.ts
#[test]
fn what_the_conversations_header_says_a_run_cost_is_money_or_nothing() {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("claude".into()),
        model: Some("opus".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    let mut snap = ledger::Snapshot::default();
    snap.cost = Some(locust_core::benchmark::RunCost { input_tokens: Some(9000), output_tokens: Some(240), usd: Some(0.12), ..Default::default() });
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: s.teammates[0].clone(),
        conversation: Conversation::default(),
        items: vec![],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: snap,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    fn view_hdr(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_hdr, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(text_in(&rt, "thread-header-route").join(" ").contains(" · $0.12"));

    rt.update(|s, _| {
        s.ledger.as_mut().unwrap().snapshot.cost = Some(locust_core::benchmark::RunCost {
            input_tokens: Some(9000),
            output_tokens: Some(240),
            plan: true,
            ..Default::default()
        });
    });
    rt.frame(0.016);
    let words = text_in(&rt, "thread-header-route").join(" ");
    assert!(!words.contains("9.0k") && !words.contains("in your plan"), "got: {words}");
}

// connectors-are-not-the-mode.test.tsx
#[test]
fn reaches_the_screen_without_opening_the_menu() {
    let mut rt = chip_runtime("claude", "sonnet", "Sonnet 5.5", None);
    let (x, y) = rt.find("mode-chip").expect("mode-chip");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("mode-chip-tip-tooltip"));
    let tip_text = tip.and_then(|e| e.label).unwrap_or_default();
    assert!(tip_text.contains("Your connectors work"), "got: {tip_text}");
}

// a-restored-queue-offers-its-actions.test.tsx
#[test]
fn the_restored_queue_shows_the_original_words_its_hold_reason_and_edit_discard_and_send() {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Ash".into(),
        role: "Code".into(),
        runtime: Some("opencode".into()),
        model: Some("opencode/free".into()),
        account: None,
        busy: false,
        runs_in: "accept-edits".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation { conversation_id: Some("conv_1".into()), ..Default::default() });
    s.queue_rows.push(crate::composer::QueuedRow {
        id: "q1".into(),
        key: "conv_1".into(),
        text: "Please review paragraph two.".into(),
        origin: crate::composer::Origin::Person,
        attachments: vec![],
        issue: None,
        restored: true,
        held: None,
        retry_at: None,
        tries: None,
    });
    fn view_q(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_q, (1000.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let q = text_in(&rt, "queued").join(" | ");
    assert!(q.contains("Please review paragraph two."), "got: {q}");
    assert!(q.contains("Saved before Locust closed"), "got: {q}");
    assert!(q.contains("press Send to continue"), "got: {q}");
    assert!(q.contains("Edit") && q.contains("Discard") && q.contains("Send now"), "got: {q}");
    assert!(!q.contains("Sends when Ash finishes"));
}

#[test]
fn a_queue_read_failure_is_visible_even_when_no_rows_can_be_displayed() {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Ash".into(),
        role: "Code".into(),
        runtime: Some("opencode".into()),
        model: Some("opencode/free".into()),
        account: None,
        busy: false,
        runs_in: "accept-edits".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.queue_error = Some("Saved messages are unreadable; the file has been kept.".into());
    fn view_qe(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_qe, (1000.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(texts(&rt).iter().any(|w| w.contains("Saved messages are unreadable; the file has been kept.")));
}

// a-row-repeats-nothing-its-heading-says.test.tsx
#[test]
fn leaves_out_a_line_that_only_names_the_group_and_the_ready_tag() {
    let rt = picker(
        vec![
            agent("antigravity", vec![m("flash", "Flash", &[])]),
            agent("opencode", vec![m("opencode/glm-5.3-flash-free", "GLM 5.3 Flash Free", &[])]),
        ],
        None,
    );
    let flash = text_in(&rt, "model-antigravity-flash");
    assert!(!flash.iter().any(|w| w == "READY"));
    assert!(!flash.iter().any(|w| w.contains("Through Antigravity")));
}

// three-placements.test.ts, part 1, in the window: the header's route line is the runtime and then the mission strip.
#[test]
fn the_conversations_header_names_the_model_and_says_the_phase_only_when_it_is_news() {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("claude".into()),
        model: Some("sonnet".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    let mut snap = ledger::Snapshot::default();
    snap.running = true;
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: s.teammates[0].clone(),
        conversation: Conversation::default(),
        items: vec![],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: snap,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    fn view_hdr(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_hdr, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(text_in(&rt, "thread-header-route").join(" "), "Claude Code · Sonnet 5.5 · running");

    rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.running = false);
    rt.frame(0.016);
    assert_eq!(text_in(&rt, "thread-header-route").join(" "), "Claude Code · Sonnet 5.5", "a finished run says nothing about it");

    rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.failed = true);
    rt.frame(0.016);
    assert_eq!(text_in(&rt, "thread-header-route").join(" "), "Claude Code · Sonnet 5.5 · failed");
}

// Electron's `a-picker-row-is-named-not-an-id.test.tsx`: OpenCode's catalogue names a model by its id without the
// provider, and the picker drew that raw while the chip spelled it.

// case: named by its id without the provider is spelled out
#[test]
fn an_opencode_model_named_by_its_bare_id_is_spelled_out() {
    let rt = picker(vec![agent("opencode", vec![m("opencode/ling-3.0-flash-fin-free", "ling-3.0-flash-fin-free", &[])])], None);
    let row = text_in(&rt, "model-opencode-opencode/ling-3.0-flash-fin-free");
    assert!(!row.iter().any(|t| t == "ling-3.0-flash-fin-free"), "{row:?}");
    assert!(row.iter().any(|t| t.starts_with("Ling 3.0 Flash Fin")), "{row:?}");
}

// case: with a name of its own keeps that name exactly
#[test]
fn an_opencode_model_with_a_name_of_its_own_keeps_it_exactly() {
    let rt = picker(vec![agent("opencode", vec![m("opencode/big-pickle", "Big Pickle", &[])])], None);
    assert!(text_in(&rt, "model-opencode-opencode/big-pickle").iter().any(|t| t == "Big Pickle"));
    // A name the runtime gave, however it is cased, is never restyled.
    let rt = picker(vec![agent("opencode", vec![m("opencode/qwen-x", "qwen x (beta)", &[])])], None);
    assert!(text_in(&rt, "model-opencode-opencode/qwen-x").iter().any(|t| t == "qwen x (beta)"));
}

#[test]
fn reads_as_a_name_rather_than_an_identifier() {
    // case: reads as a name rather than an identifier
    let mut rt = picker(vec![agent("opencode", vec![m("muse-spark-1.3-contributor-free", "muse-spark-1.3-contributor-free", &[])])], None);
    rt.frame(0.016);
    let words = texts(&rt);
    assert!(words.iter().any(|t| t == "Muse Spark 1.3 Contributor Free"), "{words:?}");
    assert!(!words.iter().any(|t| t == "muse-spark-1.3-contributor-free"));
}
#[test]
fn leaves_a_name_the_runtime_really_gave_exactly_as_it_wrote_it() {
    // case: leaves a name the runtime really gave exactly as it wrote it
    let mut rt = picker(vec![agent("opencode", vec![m("cursor-grok-4.6-high", "Cursor Grok 4.6", &[])])], None);
    rt.frame(0.016);
    assert!(texts(&rt).iter().any(|t| t == "Cursor Grok 4.6"));
}
#[test]
fn reads_account_default_in_every_group_as_claude_codes_row_does() {
    // case: reads "Account Default" in every group, as Claude Code's row does
    let mut rt = picker(vec![agent("opencode", vec![m("account-default", "Account default", &[])])], None);
    rt.frame(0.016);
    assert!(texts(&rt).iter().any(|t| t == "Account Default"));
    assert!(!texts(&rt).iter().any(|t| t == "Account default"));
}
fn qa_models() -> Runtime<State> {
    picker(vec![agent("opencode",vec![
        m("opencode/muse-spark-1.2-contributor-free","Muse Spark 1.2 (contributor, free)",&[]),
        m("opencode/big-pickle","Big Pickle",&[]),
    ])],None)
}
// case: says which OpenCode model is free
#[test]
fn says_which_opencode_model_is_free() {
    let mut rt = qa_models(); rt.frame(0.016);
    let line = text_in(&rt,"model-opencode-opencode/muse-spark-1.2-contributor-free").join(" ");
    assert!(line.contains("Free") || line.contains("no sign-in"),"{line}");
}
// case: draws the rows it was given
#[test]
fn draws_the_rows_it_was_given() {
    let mut rt = qa_models(); rt.frame(0.016);
    let words = texts(&rt);
    assert!(words.iter().any(|s| s=="Muse Spark 1.2 (contributor, free)"),"{words:?}");
    assert!(words.iter().any(|s| s=="Big Pickle"),"{words:?}");
}
