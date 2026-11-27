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
    let title = els.iter().find(|e| e.text.as_deref() == Some("Approval required — exact action")).expect("the title");
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
    let facts = [("Action", "approval-action"), ("Exact", "approval-exact"), ("In short", "approval-plain"), ("Where", "approval-where"), ("Effects", "approval-effects")];
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
    let widest = rune::text::measure("In short", &[], rune::TextStyle::new(12.0, 600.0), None).0;
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
