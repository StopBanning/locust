use super::*;

pub(super) fn sidebar(s: &State, p: &Palette) -> El {
    let mut rows = col().w_full().gap(6.0).padding(10.0);

    for (index, teammate) in s.teammates.iter().enumerate() {
        let initial = teammate
            .name
            .trim_start()
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| "?".into());
        let selected = s
            .ledger
            .as_ref()
            .map(|pane| pane.header.id == teammate.id)
            .unwrap_or(s.selected == index);
        let busy = teammate.busy
            || s.flight
                .as_ref()
                .is_some_and(|f| f.teammate_id == teammate.id)
            || s.ledger
                .as_ref()
                .is_some_and(|pane| pane.header.id == teammate.id && pane.snapshot.running);

        let mut title = row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                el().size(22.0, 22.0)
                    .radius(11.0)
                    .center()
                    .fill(p.raised)
                    .child(text(initial).font(type_::LABEL).color(p.secondary)),
            )
            .child(
                text(teammate.name.clone())
                    .font(type_::TITLE)
                    .color(p.text)
                    .grow(1.0)
                    .shrink(),
            );
        if busy {
            title = title.child(el().size(6.0, 6.0).radius(3.0).fill(p.green));
        }
        let id = teammate.id.clone();
        rows = rows.child(
            col()
                .key(format!("teammate-{id}"))
                .w_full()
                .padding(12.0)
                .gap(5.0)
                .radius(8.0)
                .fill(if selected { p.selected } else { p.sidebar })
                .child(title)
                .child(
                    text(teammate.role.clone())
                        .font(type_::BODY)
                        .color(p.secondary)
                        .w_full()
                        .pl(30.0)
                        .wrap_anywhere(),
                )
                .child(
                    text(agent_line(s, teammate))
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full()
                        .pl(30.0)
                        .wrap_anywhere(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    select_teammate(s, &id, tx);
                }),
        );
    }

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(4.0).child(
        row()
            .w_full()
            .center_y()
            .px(12.0)
            .py(10.0)
            .child(
                text("Recent")
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .grow(1.0),
            )
            .child(
                text("Refresh")
                    .key("refresh-recent")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .on_press(refresh_recent),
            ),
    );
    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone())
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap_anywhere(),
        );
    } else if s.recent.is_empty() {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .px(12.0)
            .wrap(),
        );
    } else {
        for item in s.recent.iter().take(12) {
            // Fix 5 (Finding 9): highlight Recent row when either the root conversation ID or active continuation tip matches.
            let picked = selected_conversation(s).and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.id.as_str())
                || s.ledger
                    .as_ref()
                    .is_some_and(|p| !p.snapshot.tip.is_empty() && p.snapshot.tip == item.id);
            let mut title = row().w_full().center_y().gap(6.0).child(
                text(item.title.clone())
                    .font(type_::BODY)
                    .color(p.text)
                    .grow(1.0)
                    .shrink()
                    .truncate(),
            );
            let dot = match item.state.as_str() {
                "running" => Some(p.green),
                "failed" => Some(p.red),
                _ => None,
            };
            if let Some(ink) = dot {
                title = title.child(el().size(5.0, 5.0).radius(2.5).fill(ink));
            }
            let item = item.clone();
            recent = recent.child(
                col()
                    .key(format!("recent-{}", item.id))
                    .w_full()
                    .padding(12.0)
                    .gap(5.0)
                    .radius(8.0)
                    .fill(if picked { p.selected } else { p.sidebar })
                    .child(title)
                    .child(
                        text(format!(
                            "{}{} · {}",
                            item.teammate,
                            if item.started_by == "native" {
                                " · native"
                            } else {
                                ""
                            },
                            when(&item.updated, clock_seconds()),
                        ))
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full()
                        .truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        open_recent(s, item.clone(), tx);
                    }),
            );
        }
    }

    col()
        .w(248.0)
        .h_full()
        .fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.hairline)
        .child(
            col()
                .w_full()
                .h(88.0)
                .pt(48.0)
                .pl(52.0)
                .pr(20.0)
                .window_drag()
                .child(text("LOCUST").font(type_::LABEL).color(p.secondary)),
        )
        .child(
            col()
                .key("team-list")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(col().w_full().child(rows).child(recent)),
        )
}
