//! The approval card in Electron's shape, found by name in the drawn card: ApprovalCard.tsx's markup and shell.css's
//! `.lc-card.is-pending.is-amber` (pending fill, amber edge and ring, radius 10), `.lc-card__head` (shield and amber
//! title, then `lc-rail__meta`: the runtime's mark and name), `.lc-receipt` (a label column, `max-content`, then the
//! values from one edge; labels in sentence case since the 0.862 pass).

use super::*;
use rune::{Env, Runtime};

fn asked() -> approval::Request {
    approval::Request {
        approval_id: "ap_shape".into(),
        mission_id: "mission_1".into(),
        runtime: "codex".into(),
        kind: approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "touch hello.txt".into(),
        cwd: Some("/tmp/project".into()),
        requested_at: "2026-11-26T00:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false, answer_in: None, claude_input: None,
        questions: Vec::new(),
    }
}

fn drawn(request: approval::Request) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.pending_approval = Some(request);
    let mut rt = Runtime::new(s, |s, _| col().w_full().padding(20.0).child(approval_card(s, &Palette::new(true))), (900.0, 700.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn find<'a>(els: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    els.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}: {:?}", els.iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>()))
}

#[test]
fn the_card_is_the_pending_amber_card() {
    let els = drawn(asked());
    let l = crate::look::controls::Look::new(true);
    let card = find(&els, "approval-ap_shape");
    assert_eq!(card.fill, l.c("bg-pending"), "`.lc-card.is-pending`: the pending fill, not the amber tint or a raised grey");
    assert_eq!(card.radius, 10.0, "`--lc-radius-card`");
    assert_eq!(card.label.as_deref(), Some("Approval required"), "role group, aria-label Approval required");
}

#[test]
fn the_head_has_the_shield_and_amber_title_left_and_who_asks_at_the_right() {
    let els = drawn(asked());
    let l = crate::look::controls::Look::new(true);
    let card = find(&els, "approval-ap_shape").rect;
    let shield = find(&els, "approval-shield").rect;
    let title = els.iter().find(|e| e.text.as_deref() == Some("Codex CLI wants to run a command")).expect("the title");
    let mark = find(&els, "approval-mark-codex").rect;
    let name = find(&els, "approval-runtime");
    assert_eq!(&shield[2..], &[13.0, 13.0], "`<Icon name=shield size={{13}} />`");
    assert!(shield[0] < title.rect[0], "the shield leads the title");
    assert_eq!(title.text_color, Some(l.c("amber-text")), "`.lc-approval__title`: amber text");
    assert!(mark[0] > title.rect[0] + title.rect[2], "the mark is right of the title");
    assert!(mark[0] < name.rect[0], "the mark, then the name");
    assert_eq!(name.text.as_deref(), Some("Codex CLI"));
    assert_eq!(name.text_color, Some(l.c("text-muted")));
    let right = card[0] + card[2];
    assert!((right - 12.0 - (name.rect[0] + name.rect[2])).abs() < 1.5, "who asks sits at the card's right, 12 in: {} vs {}", name.rect[0] + name.rect[2], right);
}

#[test]
fn the_receipt_is_a_label_column_then_the_values_from_one_edge() {
    let els = drawn(asked());
    let l = crate::look::controls::Look::new(true);
    let card = find(&els, "approval-ap_shape").rect;
    let facts = [("Exact", "approval-exact"), ("In short", "approval-plain"), ("Where", "approval-where")];
    let mut label_x = Vec::new();
    let mut value_x = Vec::new();
    let mut tops = Vec::new();
    for (label, key) in facts {
        let words = find(&els, &format!("approval-label-{}", key.trim_start_matches("approval-")));
        assert_eq!(words.text.as_deref(), Some(label), "the label in sentence case, no colon");
        assert_eq!(words.text_color, Some(l.c("text-secondary")));
        let value = find(&els, key);
        assert!(words.rect[0] + words.rect[2] <= value.rect[0], "{label}: the label, then its value");
        label_x.push(words.rect[0]);
        value_x.push(value.rect[0]);
        tops.push(value.rect[1]);
    }
    assert!(label_x.iter().all(|x| (x - card[0] - 12.0).abs() < 0.5), "labels start 12 in: {label_x:?}");
    assert!(value_x.windows(2).all(|w| w[0] == w[1]), "every value starts at one edge: {value_x:?}");
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "one fact per row, in Electron's order: {tops:?}");
    // The column is as wide as its widest label (`max-content`) plus the 16 gap (`--lc-space-6`).
    let widest = rune::text::measure("In short", &[], crate::look::type_::FACT_LABEL, None).0;
    assert!((value_x[0] - label_x[0] - widest - 16.0).abs() < 1.0, "label column {} for widest {widest}", value_x[0] - label_x[0]);
    assert_eq!(find(&els, "approval-exact").text.as_deref(), Some("touch hello.txt"));
}

#[test]
fn an_empty_exact_has_no_row() {
    let mut request = asked();
    request.detail.clear();
    let els = drawn(request);
    assert!(els.iter().all(|e| e.key.as_deref() != Some("approval-exact")));
    assert!(els.iter().all(|e| e.text.as_deref() != Some("—")));
}

/// REVIEW-ui-changes-2026-11-26.md change 5 (Colin's yes, 26 Nov), over ApprovalCard.tsx:366 and :387-434: the title is
/// "<Teammate> wants to run a command", the command directly under it, and no "Action" row saying it again.
#[test]
fn the_title_says_who_wants_what_and_the_command_is_right_under_it() {
    let mut s = State::default();
    crate::scenes::approval_scene_dark(&mut s);
    let mut rt = Runtime::new(s, |s, cx| crate::view(s, cx), (1215.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let els = rt.elements();
    let title = els.iter().find(|e| e.text.as_deref() == Some("Codec wants to run a command")).expect("the teammate and what it wants");
    let rows: Vec<&rune::ElementInfo> = els.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("approval-row-"))).collect();
    let first = rows.iter().min_by(|a, b| a.rect[1].total_cmp(&b.rect[1])).expect("receipt rows");
    assert_eq!(first.key.as_deref(), Some("approval-row-exact"), "the command is the first thing under the title");
    assert!(first.rect[1] > title.rect[1]);
    assert!(els.iter().all(|e| e.key.as_deref() != Some("approval-action")), "no Action row repeating the title");
}

/// Change 5: one verb on the buttons, "Allow once", "Always allow this session", "Deny…" (ApprovalCard.tsx:534, :546,
/// :555 use Approve, Allow and Yes).
#[test]
fn the_buttons_use_one_verb() {
    let els = drawn(asked());
    let said: Vec<&str> = els.iter().filter_map(|e| e.text.as_deref()).collect();
    for button in ["Allow once", "Always allow this session", "Deny…"] {
        assert!(said.contains(&button), "{button}: {said:?}");
    }
    assert!(!said.iter().any(|t| t.starts_with("Approve") || t.starts_with("Yes")), "{said:?}");
    // The note under the buttons says "Always" lasts until this run ends (approval::card_note): only the session's
    // button may say Always; the saved rule, which outlasts the run, says so in its own words.
    assert_eq!(said.iter().filter(|t| t.starts_with("Always")).count(), 1, "{said:?}");
    assert!(said.contains(&"Allow this command from now on"), "{said:?}");
}
