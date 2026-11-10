use super::*;

pub(super) fn message_column(items: &[ledger::Item], name: &str, p: &Palette, s: &State) -> El {
    let mut messages = col().w_full().gap(24.0).padding(24.0);
    let style = markdown_style(p);
    for (index, item) in items.iter().enumerate() {
        let conversation = s
            .ledger
            .as_ref()
            .and_then(|p| p.conversation.conversation_id.as_deref())
            .unwrap_or("preview");
        let key = format!("{conversation}/ledger-item-{index}");
        if matches!(item,ledger::Item::Plan {steps,..} if steps.is_empty()) {
            continue;
        }
        if is_activity(item) {
            if index > 0 && is_activity(&items[index - 1]) {
                continue;
            }
            let end = items[index..]
                .iter()
                .position(|i| !is_activity(i))
                .map(|n| index + n)
                .unwrap_or(items.len());
            messages = messages.child(activity_group(&items[index..end], p, s, conversation));
            continue;
        }
        let content = match item {
            ledger::Item::CarryOn { conversation_id } => {
                if s.flight.is_some()
                    || s.sending.is_pending()
                    || s.ledger
                        .as_ref()
                        .is_some_and(|pane| pane.snapshot.running || pane.after_tip.is_some())
                {
                    continue;
                }
                let id = conversation_id.clone();
                let mut card = col()
                    .w_full()
                    .gap(8.0)
                    .padding(12.0)
                    .radius(9.0)
                    .fill(p.raised)
                    .child(text("Carry on with").font(type_::LABEL).color(p.text))
                    .child(
                        el().padding(8.0)
                            .child(text("Choose AI agent and model").color(p.text))
                            .on_press(|s: &mut State| open_models(s, CARRY_TARGET.into())),
                    );
                if s.model_menu.as_deref() == Some(CARRY_TARGET) {
                    if let Some(agents) = s.models.get(CARRY_TARGET).filter(|a| !a.is_empty()) {
                        let mut list = col()
                            .key(format!("{key}-carry-menu"))
                            .w_full()
                            .max_h(180.0)
                            .scroll(ScrollSpec {
                                stick_bottom: false,
                            })
                            .gap(4.0);
                        for agent in agents {
                            for model in &agent.models {
                                let (id, runtime, model_id) =
                                    (id.clone(), agent.runtime.clone(), model.id.clone());
                                list = list.child(
                                    el().padding(8.0)
                                        .disabled(s.flight.is_some())
                                        .child(
                                            text(format!(
                                                "{} · {}",
                                                engine::runtime_name(&runtime),
                                                model.name
                                            ))
                                            .color(p.text),
                                        )
                                        .on_press(move |s: &mut State| {
                                            carry(s, id.clone(), runtime.clone(), model_id.clone())
                                        }),
                                );
                            }
                        }
                        card = card.child(list);
                    } else {
                        card = card.child(text("Loading models…").color(p.muted));
                    }
                }
                card
            }
            ledger::Item::User(body) => row().w_full().justify_end().child(
                row().w_frac(0.7).justify_end().child(
                    text(body.clone())
                        .font(type_::BODY)
                        .color(p.text)
                        .shrink()
                        .wrap_anywhere()
                        .selectable(true)
                        .padding(12.0)
                        .radius(10.0)
                        .fill(p.your_bubble)
                        .border(1.0, p.hairline),
                ),
            ),
            ledger::Item::Assistant {
                text: body,
                r#final,
                ..
            } => col()
                .w_full()
                .gap(8.0)
                .child(text(name).font(type_::LABEL).color(p.muted))
                .child(
                    Markdown::new(body.clone())
                        .streaming(!*r#final)
                        .selectable(true)
                        .render(&style)
                        .key(format!("{key}-markdown"))
                        .w_full(),
                ),
            ledger::Item::Tool { .. }
            | ledger::Item::Plan { .. }
            | ledger::Item::Thought { .. } => activity_item(item, p, s, conversation),
            ledger::Item::Note { level, text: body } => text(if level.is_empty() {
                body.clone()
            } else {
                format!("{level}: {body}")
            })
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap_anywhere()
            .selectable(true),
        };
        messages = messages.child(content.key(key));
    }
    messages.child(approval_card(s, p))
}
