//! a-command-that-reaches-other-programs-says-so.test.tsx: a command that stops programs its run did not start says so
//! on its approval card before it runs, and on its row after. Neither Always nor a saved rule is offered for one, and
//! an Always given earlier in the run covers no command like it, with the card saying why it asks again.

use super::*;
use locust_core::approval::{self, Answer, Decision, Kind, Request};

const ARENA: &str = "taskkill //F //IM python.exe";
const SAID: &str = "Stops every python.exe on this computer, not only the ones this run started.";

fn asked(detail: &str) -> Request {
    Request {
        approval_id: format!("ap_reach_{}", detail.len()),
        mission_id: "mission_1".into(),
        runtime: "claude".into(),
        kind: Kind::Command,
        summary: "Run a command".into(),
        detail: detail.into(),
        cwd: Some("C:\\work".into()),
        requested_at: "2026-10-03T22:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: vec![],
    }
}

fn card(req: Request) -> Vec<String> {
    let mut s = State::default();
    s.pending_approval = Some(req);
    let mut rt = rune::Runtime::new(s, |s, _| col().w_full().child(crate::approval_card::approval_card(s, &Palette::new(true))), (800.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().flat_map(|e| [e.text, e.label]).flatten().collect()
}

fn says(words: &[String], part: &str) -> bool {
    words.iter().any(|w| w.contains(part))
}

// case: says what the command reaches before it runs
#[test]
fn the_card_says_what_the_command_reaches_before_it_runs() {
    let words = card(asked(ARENA));
    assert!(words.iter().any(|w| w == &format!("Reaches: {SAID}")), "{words:?}");
}

// case: has no such row for a command that acts on its own work
#[test]
fn the_card_has_no_reach_row_for_a_command_on_its_own_work() {
    for command in ["taskkill /F /PID 1234", "python -m pytest -q"] {
        assert!(!says(&card(asked(command)), "Reaches"), "{command}");
    }
}

// case: offers neither Always nor a saved rule, and says why
#[test]
fn a_reaching_command_is_offered_neither_always_nor_a_saved_rule() {
    let words = card(Request { always_covers: Some("anything matching “taskkill *”".into()), ..asked(ARENA) });
    assert!(says(&words, "Allow once") && says(&words, "Deny"), "{words:?}");
    assert!(!says(&words, "Always allow this session"), "{words:?}");
    assert!(!says(&words, "don't ask again") && !says(&words, "don’t ask again"), "{words:?}");
    assert!(!says(&words, "taskkill *"), "{words:?}");
    assert!(says(&words, "asked about every time"), "{words:?}");
}

// case: still offers both on an ordinary command (control)
#[test]
fn an_ordinary_command_still_offers_always() {
    let words = card(Request { always_covers: Some("anything matching “python *”".into()), ..asked("python -m pytest -q") });
    assert!(says(&words, "Always allow this session"), "{words:?}");
    assert!(says(&words, "python *"), "{words:?}");
}

// case: says a saved rule still comes first, where Locust keeps the Always
#[test]
fn a_saved_rule_still_comes_first_where_locust_keeps_the_always() {
    let kept = card(Request { always_covers: Some("every command it runs".into()), always_key: Some("claude:Bash".into()), ..asked("npm test") });
    assert!(kept.iter().any(|w| w == "Nothing has happened yet. “Always” allows every command it runs, until this run ends. Your saved rules still come first."), "{kept:?}");
    let codex = card(Request { runtime: "codex".into(), ..asked("npm test") });
    assert!(codex.iter().any(|w| w == "Nothing has happened yet. “Always” lasts until this run ends."), "{codex:?}");
    assert!(!says(&codex, "saved rules still come first"), "{codex:?}");
}

// case: says why it is asked again when Always was given earlier in the run
#[test]
fn says_why_it_is_asked_again_when_always_was_given_earlier_in_the_run() {
    let why = "Asked again, though you chose Always earlier in this run: this command stops every python.exe, and such a command is asked about every time.";
    let req = Request { always_key: Some("claude:Bash".into()), ..asked(ARENA) };
    assert_eq!(approval::asked_again_why(&req, true).as_deref(), Some(why));
    assert_eq!(approval::asked_again_why(&req, false), None, "no Always given, nothing to explain");
    assert_eq!(approval::asked_again_why(&Request { always_key: Some("claude:Bash".into()), ..asked("npm test") }, true), None);

    // Through the channel: an Always on one Bash card, then a reaching Bash command in the same run.
    let ch = approval::Channel::new();
    let ask = |ch: &std::sync::Arc<approval::Channel>, id: &'static str, command: &'static str| {
        let worker = ch.clone();
        std::thread::spawn(move || worker.ask_claude("r", "m_reach", id, "Bash", &serde_json::json!({"command": command}), "/tmp", &|| false))
    };
    let wait = |ch: &std::sync::Arc<approval::Channel>| {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while ch.waiting().is_empty() {
            assert!(std::time::Instant::now() < end, "no card was raised");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        ch.waiting()[0].clone()
    };
    let first = ask(&ch, "a", "npm test");
    let card_a = wait(&ch);
    assert_eq!(approval::asked_again_of(&card_a.approval_id), None);
    ch.decide(Answer::Decision { approval_id: card_a.approval_id.clone(), decision: Decision::ApproveAlways, reason: None }).unwrap();
    first.join().unwrap().unwrap();
    let second = ask(&ch, "b", ARENA);
    let card_b = wait(&ch);
    assert_eq!(approval::asked_again_of(&card_b.approval_id).as_deref(), Some(why));
    let words = card(card_b.clone());
    assert!(words.iter().any(|w| w == &format!("Nothing has happened yet. {why}")), "{words:?}");
    assert!(!says(&words, "Always allow this session"), "{words:?}");
    ch.release("m_reach");
    let _ = second.join();
}

fn row(command: &str, status: &str) -> Vec<rune::ElementInfo> {
    let item = ledger::Item::Tool {
        name: "Bash".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail { id: "c1".into(), arguments: command.into(), exit_code: Some(0), finished: true, runtime_status: status.into(), ..Default::default() }),
        status: ledger::Status::Done,
    };
    let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (700.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn reach_badge(els: &[rune::ElementInfo]) -> Option<&rune::ElementInfo> {
    els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.contains("/reach/")))
}

// case: says what the command reached after it ran
#[test]
fn the_row_says_what_the_command_reached_after_it_ran() {
    let els = row(ARENA, "");
    assert!(els.iter().any(|e| e.text.as_deref() == Some("stops every python.exe")), "{:?}", els.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>());
    let badge = reach_badge(&els).expect("the reach badge");
    assert!(els.iter().any(|e| e.label.as_deref() == Some(SAID)), "{:?}", els.iter().map(|e| (e.key.clone(), e.label.clone(), e.role)).collect::<Vec<_>>());
    assert!(badge.key.as_deref().unwrap().contains("/reach/every-process-named"), "{:?}", badge.key);
}

// case: says nothing more about a command that acts on its own work
#[test]
fn the_row_says_nothing_more_about_a_command_on_its_own_work() {
    assert!(reach_badge(&row("kill 1234", "")).is_none());
    assert!(reach_badge(&row("echo \"taskkill /IM python.exe\"", "")).is_none());
}

// case: says nothing about a command that was refused, since it never ran
#[test]
fn the_row_says_nothing_about_a_command_that_never_ran() {
    assert!(reach_badge(&row(ARENA, "refused")).is_none());
    assert!(reach_badge(&row(ARENA, "declined")).is_none());
}
