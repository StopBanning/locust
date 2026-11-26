//! Suggested tasks under a reply (`NextSteps.tsx`).
//! Drawn only once the message is finished, with Put away (×), Do it here and Start. Electron's Start all is left out
//! until this window can start several turns at once.

use super::*;
use locust_core::protocol_tags::{self, NextStep};

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
            text(count_label)
                .font(l.ty.label)
                .color(l.c("text-muted"))
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
                text(step.brief.clone())
                    .font(l.ty.label)
                    .color(l.c("text-secondary"))
                    .wrap(),
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
}
