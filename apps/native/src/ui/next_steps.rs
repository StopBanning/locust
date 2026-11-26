//! Suggested tasks under a reply (`NextSteps.tsx`).
//! Drawn only once the message is finished, with Put away (×), Do it here and Start. Electron's Start all is left out
//! until this window can start several turns at once.

use super::*;
use locust_core::protocol_tags::{self, NextStep};

/// The brief under a suggested task's title: 13 px sans on Electron's 1.45 lines (`.lc-nextstep__brief`).
const BRIEF: rune::TextStyle = type_::BODY.lh(13.0 * 1.45);
/// Lines of it shown before "…" (`-webkit-line-clamp: 2`).
const BRIEF_LINES: usize = 2;

fn gone_key(scope: &str, step: &NextStep) -> String {
    format!("{scope}::{}", step.title)
}

pub(super) fn view(
    s: &State,
    steps: &[NextStep],
    scope: &str,
    can_here: bool,
    p: &Palette,
) -> Option<El> {
    let shown: Vec<&NextStep> = steps
        .iter()
        .filter(|step| !s.next_steps_gone.contains(&gone_key(scope, step)))
        .collect();
    if shown.is_empty() {
        return None;
    }

    let l = crate::look::controls::Look::new(p.dark);
    let count_label = if shown.len() == 1 {
        "1 suggested task".to_owned()
    } else {
        format!("{} suggested tasks", shown.len())
    };

    let head = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            // Electron: `.lc-nextsteps__count { color: var(--lc-text-secondary) }`, plain sans at the body's size.
            text(count_label)
                .font(type_::BODY)
                .color(l.c("text-secondary"))
                .grow(1.0),
        );
    // No "Start all" yet: Electron starts each task in a conversation of its own at once, and this window runs one
    // turn of its own at a time. Starting only the first while putting the rest away would lose them unstarted, so
    // each task keeps its own Start until the window can start several.
    let mut box_col = col()
        .key(format!("next-steps-{scope}"))
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-card"))
        .child(head);

    for (idx, step) in shown.iter().enumerate() {
        let gkey = gone_key(scope, step);
        let away_key = gkey.clone();
        let here_key = gkey.clone();
        let start_key = gkey;

        let words = col()
            .grow(1.0)
            .gap(2.0)
            .child(
                text(step.title.clone())
                    .font(l.ty.body)
                    .color(l.c("text-primary"))
                    .wrap(),
            )
            .child(
                // Electron's `.lc-nextstep__brief`: plain sans, secondary colour, 1.45 lines, cut at two (shell.css:7006).
                text(step.brief.clone())
                    .font(BRIEF)
                    .color(l.c("text-secondary"))
                    .max_lines(BRIEF_LINES),
            );

        let away_btn = row()
            .key(format!("next-step-away-{scope}-{idx}"))
            .px(6.0)
            .py(2.0)
            .radius(4.0)
            .hover(l.c("hover-surface"))
            .role("button")
            .label(format!("Put away: {}", step.title))
            .child(text("×").font(l.ty.body).color(l.c("text-muted")))
            .on_press(move |s: &mut State| {
                s.next_steps_gone.insert(away_key.clone());
            });

        let top_row = row().w_full().gap(8.0).child(words).child(away_btn);

        let mut actions = row().w_full().gap(8.0);
        if can_here {
            let st = (*step).clone();
            actions = actions.child(
                crate::look::controls::button(&l, "Do it here", crate::look::controls::Tone::Neutral, false)
                    .key(format!("next-step-here-{scope}-{idx}"))
                    .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                        s.next_steps_gone.insert(here_key.clone());
                        s.draft.replace_text(protocol_tags::next_step_prompt(&st));
                        submit_composer(s, tx, false);
                    }),
            );
        }
        let st = (*step).clone();
        actions = actions.child(
            crate::look::controls::button(&l, "Start", crate::look::controls::Tone::Neutral, false)
                .key(format!("next-step-start-{scope}-{idx}"))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                    s.next_steps_gone.insert(start_key.clone());
                    fresh_conversation(s);
                    s.draft.replace_text(protocol_tags::next_step_prompt(&st));
                    submit_composer(s, tx, false);
                }),
        );

        box_col = box_col.child(
            col()
                .key(format!("next-step-{scope}-{idx}"))
                .w_full()
                .gap(8.0)
                .padding(10.0)
                .radius(6.0)
                .fill(l.c("bg-window"))
                .border(1.0, l.c("border-control"))
                .child(top_row)
                .child(actions),
        );
    }

    Some(box_col)
}

#[cfg(test)]
mod tests {
    //! Electron's `a-reply-suggests-tasks-as-cards.test.tsx`, the cards and the window's side of "the teammates", drawn
    //! in the window from a reply that ends in a `<locust-next>` block.
    use super::*;
    use rune::{Env, Runtime};

    const REPLY: &str = "Done: the update images are cleaned up after an install.\n\n<locust-next>\n\
        Clean the Linux cache too :: Do the same for the AppImage cache, keeping the one waiting to install.\n\
        - Tell people in What's new :: Add a line to the next changelog saying old updates no longer pile up.\n\
        </locust-next>";

    /// The thread with that reply as its last message: a turn `running` or not, the reply `whole` (final) or not.
    fn thread(running: bool, whole: bool) -> String {
        thread_with(running, whole, |_| {})
    }

    /// `thread`, with the window's state changed by `tweak` after the conversation is set up.
    fn thread_with(running: bool, whole: bool, tweak: impl FnOnce(&mut State)) -> String {
        let mut s = State::default();
        crate::scenes::ledger_scene(&mut s, false, false);
        let pane = s.ledger.as_mut().expect("a conversation");
        pane.items = vec![
            ledger::Item::User("Clean up the update images.".into()),
            ledger::Item::Assistant { item_id: "reply-next".into(), r#final: whole, text: REPLY.into() },
        ];
        pane.snapshot.running = running;
        pane.after_tip = None;
        s.flight = None;
        tweak(&mut s);
        let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.frame(0.05);
        rt.elements().into_iter().filter(|e| !e.exiting).flat_map(|e| [e.text, e.label]).flatten().collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn offer_no_do_it_here_where_the_conversation_cannot_take_a_turn_now() {
        // case: offer no Do it here where the conversation cannot take a turn now
        let idle = thread(false, true);
        assert!(idle.contains("2 suggested tasks") && idle.contains("Do it here"), "{idle}");
        // Another turn is running: the cards stay, each with its Start, and none offers Do it here.
        let busy = thread(true, true);
        assert!(busy.contains("2 suggested tasks") && busy.contains("Clean the Linux cache too"), "{busy}");
        assert!(busy.contains("Start") && !busy.contains("Do it here"), "{busy}");
    }

    #[test]
    fn the_window_draws_the_cards_only_once_the_reply_is_whole() {
        // Electron: `const suggested = message.final || !options.running ? parseNextSteps(message.text) : []`. A reply still
        // arriving in a running turn shows no cards and never its half-written tags; a whole one, or a turn that is over,
        // shows them.
        let arriving = thread(true, false);
        assert!(!arriving.contains("suggested task") && !arriving.contains("locust-next"), "{arriving}");
        assert!(thread(true, true).contains("2 suggested tasks"));
        assert!(thread(false, false).contains("2 suggested tasks"), "a turn that is over shows them whatever final says");
    }

    #[test]
    fn offer_no_do_it_here_while_a_send_is_in_flight_or_a_card_waits_for_the_person() {
        // NextSteps.tsx: Do it here is this conversation's next turn, so it is not offered where the conversation cannot
        // take one now: a message already on its way, or an approval waiting for the person's answer.
        let sending = thread_with(false, true, |s| {
            s.flight = Some(Flight { teammate_id: "tm_wren".into(), conversation_id: None, header: None, after_tip: None, message: Some("Go on.".into()) });
        });
        assert!(sending.contains("2 suggested tasks") && sending.contains("Start") && !sending.contains("Do it here"), "{sending}");
        let waiting = thread_with(false, true, |s| {
            s.pending_approval = Some(locust_core::approval::Request {
                approval_id: "ap_1".into(),
                mission_id: "m1".into(),
                runtime: "codex".into(),
                kind: locust_core::approval::Kind::Command,
                summary: "Run a command".into(),
                detail: String::new(),
                cwd: None,
                requested_at: String::new(),
                always_covers: None,
                always_key: None,
                data_sent_says: None,
                reversible_says: None,
                patch: None,
                claude_input: None,
                skippable: false,
                answer_in: None,
                questions: Vec::new(),
            });
        });
        assert!(waiting.contains("2 suggested tasks") && waiting.contains("Start") && !waiting.contains("Do it here"), "{waiting}");
    }

    // ---- Row 5 of notes/TYPE-NOTES.md: the card's words, as Electron's stylesheet sets them -----------------------

    const SHELL_CSS: &str = include_str!("../../../desktop/src/renderer/src/shell.css");

    /// The declarations of one rule of shell.css, from its selector to the closing brace.
    fn rule(selector: &str) -> &'static str {
        let at = SHELL_CSS.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("shell.css has no {selector}"));
        let rest = &SHELL_CSS[at..];
        &rest[..rest.find('}').expect("a closed rule")]
    }

    const LONG: &str = "Go through every cache the updater keeps, the AppImage one, the one for the Mac and the one the Windows build \
        writes, and remove each that is older than the version now installed, keeping the one waiting to install, then \
        say in the changelog which folders were touched and how much space each one gave back.";

    /// The elements of the thread whose last reply carries two suggested tasks, the first with a one-line brief and the
    /// second with `LONG`, in a window `width` wide (dark).
    fn cards(width: f32) -> Vec<rune::ElementInfo> {
        let mut s = State::default();
        crate::scenes::ledger_scene(&mut s, false, false);
        let pane = s.ledger.as_mut().expect("a conversation");
        let reply = format!("Done.\n\n<locust-next>\nShort one :: A single line.\nLong one :: {LONG}\n</locust-next>");
        pane.items = vec![
            ledger::Item::User("Clean up.".into()),
            ledger::Item::Assistant { item_id: "reply-next".into(), r#final: true, text: reply },
        ];
        pane.snapshot.running = false;
        pane.after_tip = None;
        s.flight = None;
        let mut rt = Runtime::new(s, crate::view, (width, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.frame(0.05);
        rt.elements().into_iter().filter(|e| !e.exiting).collect()
    }

    fn with_text<'a>(els: &'a [rune::ElementInfo], text: &str) -> &'a rune::ElementInfo {
        els.iter().find(|e| e.text.as_deref() == Some(text)).unwrap_or_else(|| panic!("no text {text:?} on screen"))
    }

    #[test]
    fn a_task_description_is_sans_on_1_45_lines_in_the_secondary_colour_cut_at_two_lines() {
        // shell.css: color text-secondary, line-height 1.45, -webkit-line-clamp 2.
        let brief = rule(".lc-nextstep__brief");
        assert!(brief.contains("color: var(--lc-text-secondary);") && brief.contains("line-height: 1.45;") && brief.contains("-webkit-line-clamp: 2;"), "{brief}");
        assert!(!brief.contains("font-family") && !brief.contains("letter-spacing") && !brief.contains("uppercase"), "plain sans: {brief}");
        let secondary = crate::look::tokens(true).color("text-secondary");
        let els = cards(560.0);
        let short = with_text(&els, "A single line.");
        assert_eq!(short.text_color, Some(secondary), "the secondary colour, not the mono label's muted one");
        // 13 px text on 1.45 lines is 18.85 px a line (the mono label was 10.5 px on 13.1): one line here, two at most there.
        let line = 13.0 * 1.45;
        assert!((short.rect[3] - line).abs() < 0.6, "one line is {line} px tall, got {}", short.rect[3]);
        let long = with_text(&els, LONG);
        assert!((long.rect[3] - 2.0 * line).abs() < 1.2, "cut at two lines of {line} px, got {} px for a {}-character brief", long.rect[3], LONG.len());
    }

    #[test]
    fn the_count_is_plain_secondary_text_not_the_spaced_mono_label() {
        // shell.css: `.lc-nextsteps__count { color: var(--lc-text-secondary) }`, no size, family or spacing of its own.
        let count = rule(".lc-nextsteps__count");
        assert!(count.contains("color: var(--lc-text-secondary);") && !count.contains("font-") && !count.contains("letter-spacing") && !count.contains("uppercase"), "{count}");
        let els = cards(560.0);
        let head = with_text(&els, "2 suggested tasks");
        assert_eq!(head.text_color, Some(crate::look::tokens(true).color("text-secondary")));
        // Body text sits on the body's 1.55 lines, 13 px: 20.15 px, where the mono label's 10.5 px line was 13.1.
        assert!((head.rect[3] - 13.0 * 1.55).abs() < 0.6, "got {} px", head.rect[3]);
    }
}
