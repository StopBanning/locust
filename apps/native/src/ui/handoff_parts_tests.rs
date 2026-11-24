//! Two Electron files about a conversation that moves to another runtime, through the window: the divider drawn in the
//! thread (`view`), the parts under the message box (`composer_omissions`, pressed), what a send takes with it
//! (`crosses_runtimes`, which the send asks), and Save conversation as routine from the header
//! (`routines_panel::save_conversation`). a-runtime-switch-shows-where-it-happened.test.tsx and
//! a-brief-part-can-be-left-out.test.tsx; expected words are Electron's, written out.
use super::*;
use rune::{Env, Runtime};

fn meta(mission_id: &str, runtime: &str) -> ledger::Metadata {
    ledger::Metadata { mission_id: mission_id.into(), prompt: "Ask".into(), runtime: runtime.into(), model: "account-default".into(), ..Default::default() }
}

fn switched(from: &str, to: &str, tweak: impl FnOnce(&mut ledger::Metadata)) -> (ledger::Metadata, ledger::Metadata) {
    let mut after = meta("m2", to);
    after.continues_reason = Some("route-switch".into());
    tweak(&mut after);
    (meta("m1", from), after)
}

/// The divider drawn: its element (role and label) and every word on it.
fn divider(from: ledger::Metadata, to: ledger::Metadata) -> (rune::ElementInfo, Vec<String>) {
    let mut rt = Runtime::new((from, to), |(from, to), _| view(Some(from), to, &Palette::new(true)).unwrap(), (700.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let seam = all.iter().find(|e| e.role == "separator").cloned().expect("the divider is a separator");
    (seam, all.into_iter().filter_map(|e| e.text).collect())
}

/// A conversation whose last turn ran on Codex, its teammate now on `next`, with `left_out` already chosen.
fn composer(next: &str, left_out: &[&str]) -> State {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(next.into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m1".into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot { tip: "m1".into(), turn_metas: vec![meta("m1", "codex")], ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s.left_out_by_you = left_out.iter().map(|x| (*x).to_owned()).collect();
    s
}

fn drawn(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let teammate = s.ledger.as_ref().unwrap().header.clone();
            col().w(900.0).child(composer_omissions(s, &teammate, &Palette::new(true)).unwrap_or_else(|| col().key("no-omissions")))
        },
        (900.0, 300.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn controls(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.role == "button").filter_map(|e| e.label).collect()
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is drawn"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

// ---- a-runtime-switch-shows-where-it-happened.test.tsx

// case: names every runtime by its name
#[test]
fn names_every_runtime_by_its_name() {
    let (from, to) = switched("codex", "opencode", |_| {});
    let (seam, words) = divider(from, to);
    assert_eq!(seam.label.as_deref(), Some("Handed off from Codex to OpenCode"));
    assert!(!words.iter().any(|w| w.contains("opencode")), "{words:?}");
    let (from, to) = switched("cursor", "claude", |_| {});
    assert_eq!(divider(from, to).0.label.as_deref(), Some("Handed off from Cursor Agent to Claude Code"));
}

fn handed_turn(id: &str) -> Option<(ledger::Metadata, String)> {
    let mut first = meta("r1", "codex");
    first.prompt = "Remember the word marigold.".into();
    let mut second = meta("r2", "opencode");
    second.prompt = "You are continuing a conversation another agent (Codex) started.\n\nWhat was done: ...\n\nThe person now asks:\n\nWhat was the word?".into();
    second.parent = Some("r1".into());
    second.continues_reason = Some("route-switch".into());
    second.checkpoint_epoch = Some(2);
    match id {
        "r1" => Some((first, "completed".into())),
        "r2" => Some((second, "completed".into())),
        _ => None,
    }
}

// case: takes the words typed on each side of the switch, never the host’s briefing
#[test]
fn takes_the_words_typed_on_each_side_of_the_switch_never_the_hosts_briefing() {
    let mut s = composer("opencode", &[]);
    s.routines.read_saved = |tip| locust_core::routine_draft::saved_from(tip, handed_turn);
    crate::routines_panel::save_conversation(&mut s, "r2");
    assert!(s.routines.open, "Save conversation as routine opened the editor");
    let steps: Vec<String> = s.routines.steps.iter().map(|f| f.text.clone()).collect();
    assert_eq!(steps, ["Remember the word marigold.", "What was the word?"]);
}

// ---- a-brief-part-can-be-left-out.test.tsx

// case: marks the earlier messages, the finished steps and the last reply as droppable, and nothing else
#[test]
fn marks_the_earlier_messages_the_finished_steps_and_the_last_reply_as_droppable_and_nothing_else() {
    let rt = drawn(composer("claude", &[]));
    assert_eq!(controls(&rt), ["Leave out the earlier messages", "Leave out the steps it finished", "Leave out its last reply"]);
    assert!(!controls(&rt).iter().any(|c| c.contains("task") || c.contains("never reported back")), "{:?}", controls(&rt));
}

// case: says what the person left out, in their words
#[test]
fn says_what_the_person_left_out_in_their_words() {
    // Electron's composer says it as the parts and their × (Composer.tsx): "It carries the task and the earlier
    // messages." and then "You are leaving out its last reply."
    let mut rt = drawn(composer("claude", &[]));
    press(&mut rt, "handoff-omit-settled");
    press(&mut rt, "handoff-omit-summary");
    let said = texts(&rt);
    assert!(said.iter().any(|t| t == "It carries the task and the earlier messages."), "{said:?}");
    assert!(said.iter().any(|t| t == "You are leaving out the steps it finished and its last reply."), "{said:?}");
}

// case: reads the same from an older host that sends no choice
#[test]
fn reads_the_same_from_an_older_host_that_sends_no_choice() {
    // The engine's own composition, cached for what is on screen (refresh_preview), with no choice in it: the line under
    // the box is the task alone, and nothing says the person left anything out.
    let mut s = composer("claude", &[]);
    let (key, _, _) = preview_key(&s).expect("a reply on another runtime has a preview");
    let preview = locust_core::carry_on::HandoffPreview::Switch { kept: vec!["task".into()], omitted: vec![], unsettled_count: 0, task_by_file: false, task_clipped: false };
    s.handoff_preview = Some((key, locust_core::carry_on::handoff_preview_line(Some(&preview))));
    let said = texts(&drawn(s));
    assert!(said.iter().any(|t| t == "It carries the task."), "{said:?}");
    assert!(!said.iter().any(|t| t.contains("leaving out") || t.contains("left out")), "{said:?}");
}

// case: joins the parts as a sentence does
#[test]
fn joins_the_parts_as_a_sentence_does() {
    let line = |left: &[&str]| texts(&drawn(composer("claude", left))).into_iter().find(|t| t.starts_with("It carries")).unwrap();
    assert_eq!(line(&[]), "It carries the task, the earlier messages, the steps it finished and its last reply.");
    assert_eq!(line(&["earlier"]), "It carries the task, the steps it finished and its last reply.");
    assert_eq!(line(&["earlier", "settled"]), "It carries the task and its last reply.");
    assert_eq!(line(&["earlier", "settled", "summary"]), "It carries the task.");
}

// case: puts a control on each part that may go, and a way to put them back
#[test]
fn puts_a_control_on_each_part_that_may_go_and_a_way_to_put_them_back() {
    let mut rt = drawn(composer("claude", &[]));
    assert!(!controls(&rt).iter().any(|c| c == "Put back"), "nothing to put back yet");
    press(&mut rt, "handoff-omit-earlier");
    assert_eq!(rt.state.left_out_by_you, ["earlier"]);
    assert!(controls(&rt).iter().any(|c| c == "Put back"), "{:?}", controls(&rt));
    assert!(!controls(&rt).iter().any(|c| c == "Leave out the earlier messages"), "{:?}", controls(&rt));
    press(&mut rt, "handoff-put-back");
    assert!(rt.state.left_out_by_you.is_empty());
    assert_eq!(controls(&rt), ["Leave out the earlier messages", "Leave out the steps it finished", "Leave out its last reply"]);
}

// case: sends the choice with the message only when it crosses runtimes
#[test]
fn sends_the_choice_with_the_message_only_when_it_crosses_runtimes() {
    // The send takes the choice only when this answers yes (main.rs, beside take_turn_pick).
    let across = composer("claude", &["summary"]);
    assert!(crosses_runtimes(&across));
    // The same runtime again: the box offers no parts, and the choice made earlier is not sent.
    let same = composer("codex", &["summary"]);
    assert!(!crosses_runtimes(&same));
    assert!(drawn(same).find("no-omissions").is_some(), "no parts under the box on the same runtime");
    // A pick of another runtime for this reply crosses even when the teammate's own runtime is the same.
    let mut picked = composer("codex", &["summary"]);
    picked.picks.insert("tm_wren".into(), TurnPick { model: "account-default".into(), effort: None, runtime: Some("claude".into()), account: None });
    assert!(crosses_runtimes(&picked));
}

// case: says what the person left out, plainly, beside what did not fit
#[test]
fn says_what_the_person_left_out_plainly_beside_what_did_not_fit() {
    let (from, to) = switched("codex", "claude", |m| {
        m.omitted_briefing = vec!["earlier".into()];
        m.left_out_by_you = vec!["summary".into(), "settled".into()];
    });
    let words = divider(from, to).1;
    assert!(words.iter().any(|w| w == "Left out of the summary to fit: the earlier messages."), "{words:?}");
    assert!(words.iter().any(|w| w == "You left out its last reply and the steps it finished."), "{words:?}");
}

// case: says nothing more when nothing was left out
#[test]
fn says_nothing_more_when_nothing_was_left_out() {
    let (from, to) = switched("codex", "claude", |_| {});
    let words = divider(from, to).1;
    assert!(!words.iter().any(|w| w.contains("You left out")), "{words:?}");
}
