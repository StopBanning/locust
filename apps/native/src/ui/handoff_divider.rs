use super::*;

fn side_name(_runtime: &str, model: Option<&str>, runtime_label: &str) -> String {
    match model {
        Some(m) if !m.trim().is_empty() && m != "account-default" => m.to_owned(),
        _ => runtime_label.to_owned(),
    }
}

pub(super) fn view(
    from_meta: Option<&ledger::Metadata>,
    to_meta: &ledger::Metadata,
    p: &Palette,
) -> Option<El> {
    let carried_on = to_meta.started_by_kind.as_deref() == Some("carry-on") || to_meta.carried_on_for.is_some();
    if to_meta.continues_reason.as_deref() != Some("route-switch") && !carried_on {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let from_rt = from_meta.map(|m| m.runtime.as_str()).unwrap_or("codex");
    let to_rt = to_meta.runtime.as_str();
    let from_label = locust_core::carry_on::display_name(from_rt);
    let to_label = locust_core::carry_on::display_name(to_rt);
    let from_name = side_name(
        from_rt,
        from_meta.and_then(|m| (!m.model.is_empty()).then_some(m.model.as_str())),
        from_label,
    );
    let to_name = side_name(
        to_rt,
        (!to_meta.model.is_empty()).then_some(to_meta.model.as_str()),
        to_label,
    );

    let mut header = row()
        .w_full()
        .center_y()
        .gap(6.0)
        .child(text("Handed off").font(l.ty.label).color(l.c("text-muted")));
    if let Some(mark) = crate::look::controls::agent_mark(&l, from_rt, 11.0, l.c("text-primary")) {
        header = header.child(mark);
    }
    header = header
        .child(text(from_name).font(l.ty.label).color(l.c("text-primary")))
        .child(text("→").font(l.ty.label).color(l.c("text-muted")));
    if let Some(mark) = crate::look::controls::agent_mark(&l, to_rt, 11.0, l.c("text-primary")) {
        header = header.child(mark);
    }
    header = header.child(text(to_name).font(l.ty.label).color(l.c("text-primary")));
    if !to_meta.created_at.is_empty() {
        header = header.child(
            text(format!("· {}", when(&to_meta.created_at, clock_seconds())))
                .font(l.ty.label)
                .color(l.c("text-muted")),
        );
    }

    let mut body = col()
        .w_full()
        .gap(4.0)
        .py(8.0)
        .px(12.0)
        .radius(8.0)
        .border(1.0, p.hairline)
        .fill(l.c("bg-raised"))
        .child(header);

    if carried_on {
        let carried_note = if to_meta.carried_on_for.as_deref() == Some("provider") {
            format!(
                "{from_label}’s provider failed before it did anything, so Locust carried the work on with {to_label}."
            )
        } else {
            format!("{from_label} ran out of its plan, so Locust carried the work on with {to_label}.")
        };
        body = body.child(
            text(carried_note)
                .font(l.ty.body)
                .color(l.c("text-muted"))
                .w_full()
                .wrap(),
        );
    }

    let unsettled = to_meta.checkpoint_epoch.and_then(|epoch| from_meta?.checkpoint_unsettled.get(&epoch).copied()).unwrap_or(0);
    let note = if unsettled == 0 {
        format!("Nothing was in flight, so {to_label} picked up from a clean stop.")
    } else {
        format!("{unsettled} action{} had started and never reported back. {to_label} was told to check {} before building on {}.",
            if unsettled == 1 { "" } else { "s" }, if unsettled == 1 { "it" } else { "them" }, if unsettled == 1 { "it" } else { "them" })
    };
    body = body.child(
        text(note)
            .font(l.ty.body)
            .color(l.c("text-muted"))
            .w_full()
            .wrap(),
    );

    if !to_meta.omitted_briefing.is_empty() {
        body = body.child(
            text(format!(
                "Left out of the summary to fit: {}.",
                locust_core::carry_on::left_out_in_words(&to_meta.omitted_briefing)
            ))
            .font(l.ty.body)
            .color(p.amber)
            .w_full()
            .wrap(),
        );
    }

    if !to_meta.left_out_by_you.is_empty() {
        body = body.child(
            text(format!(
                "You left out {}.",
                locust_core::carry_on::left_out_in_words(&to_meta.left_out_by_you)
            ))
            .font(l.ty.body)
            .color(l.c("text-muted"))
            .w_full()
            .wrap(),
        );
    }

    Some(body)
}

pub(super) fn composer_omissions(s: &State, teammate: &Teammate, p: &Palette) -> Option<El> {
    let tip_meta = s.ledger.as_ref()?.snapshot.turn_metas.last()?;
    let pick = s.picks.get(&teammate.id);
    let next_runtime = pick
        .and_then(|pk| pk.runtime.as_deref())
        .or(teammate.runtime.as_deref())
        .unwrap_or("codex");
    if next_runtime == tip_meta.runtime {
        return None;
    }

    let l = crate::look::controls::Look::new(p.dark);
    let sections = [
        ("earlier", "the earlier messages"),
        ("settled", "the steps it finished"),
        ("summary", "its last reply"),
    ];
    let mut kept = vec!["the task"];
    for (name, words) in sections {
        if !s.left_out_by_you.iter().any(|x| x == name) {
            kept.push(words);
        }
    }
    let carried_line = if kept.len() <= 1 {
        kept.join("")
    } else {
        format!("{} and {}", kept[..kept.len() - 1].join(", "), kept.last().unwrap())
    };
    let mut parts_row = row().w_full().wrap().center_y().gap(6.0).child(
        text(format!("It carries {carried_line}."))
            .font(l.ty.label)
            .color(l.c("text-muted")),
    );
    for (name, words) in sections {
        let omitted = s.left_out_by_you.iter().any(|x| x == name);
        if omitted {
            continue;
        }
        let name_owned = name.to_owned();
        parts_row = parts_row.child(
            crate::look::controls::button(
                &l,
                &format!("Leave out {words}"),
                crate::look::controls::Tone::Neutral,
                false,
            )
            .key(format!("handoff-omit-{name}"))
            .on_press(move |s: &mut State| {
                if !s.left_out_by_you.iter().any(|x| x == &name_owned) {
                    s.left_out_by_you.push(name_owned.clone());
                }
            }),
        );
    }

    let mut box_el = col()
        .key("handoff-omissions")
        .w_full()
        .gap(4.0)
        .px(10.0)
        .py(6.0)
        .radius(6.0)
        .fill(l.c("bg-raised"))
        .child(parts_row);

    if !s.left_out_by_you.is_empty() {
        box_el = box_el.child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text(format!(
                        "You are leaving out {}.",
                        locust_core::carry_on::left_out_in_words(&s.left_out_by_you)
                    ))
                    .font(l.ty.label)
                    .color(l.c("text-muted")),
                )
                .child(
                    crate::look::controls::button(
                        &l,
                        "Put back",
                        crate::look::controls::Tone::Neutral,
                        false,
                    )
                    .key("handoff-put-back")
                    .on_press(|s: &mut State| {
                        s.left_out_by_you.clear();
                    }),
                ),
        );
    }

    Some(box_el)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(mission_id: &str, runtime: &str, model: &str) -> ledger::Metadata {
        ledger::Metadata {
            mission_id: mission_id.into(),
            prompt: "Ask".into(),
            runtime: runtime.into(),
            model: model.into(),
            created_at: "2026-10-16T14:05:00.000Z".into(),
            parent: None,
            checkpoint_epoch: None,
            checkpoint_unsettled: Default::default(),
            check_fix: None,
            side: false,
            cli_version: None,
            resolved_route_id: String::new(),
            sandbox: "read-only".into(),
            mode: Some("ask".into()),
            continues_reason: None,
            edited: false,
            started_by_kind: None,
            started_by_hop: None,
            relay: None,
            omitted_briefing: vec![],
            left_out_by_you: vec![],
            carried_on_for: None,
        }
    }

    #[test]
    fn a_reopened_handoff_names_the_recorded_uncertain_actions() {
        for count in [1,2] {
            let mut from=meta("m1","codex","account-default");
            from.checkpoint_unsettled.insert(1,count);
            // A different checkpoint must never replace the selected receipt.
            from.checkpoint_unsettled.insert(2,9);
            let mut to=meta("m2","claude","account-default");
            to.continues_reason=Some("route-switch".into());to.checkpoint_epoch=Some(1);
            let mut rt=rune::Runtime::new((from,to), |(from,to),_| view(Some(from),to,&Palette::new(true)).unwrap(), (600.0,400.0), rune::Env::new(true,1.0));
            rt.frame(0.0);
            let text=rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n");
            let expected=if count==1 {"1 action had started and never reported back. Claude Code was told to check it before building on it."}else{"2 actions had started and never reported back. Claude Code was told to check them before building on them."};
            assert!(text.contains(expected),"{text}");
            assert!(!text.contains("Nothing was in flight"),"{text}");
        }
    }

    #[test]
    fn handoff_divider_renders_models_carried_reason_and_omissions() {
        let p = Palette::new(true);
        let from = meta("m1", "codex", "gpt-6.1-sol");
        let mut to = meta("m2", "claude", "claude-opus-5-5");
        assert!(view(Some(&from), &to, &p).is_none());

        to.continues_reason = Some("route-switch".into());
        to.omitted_briefing = vec!["earlier".into()];
        to.left_out_by_you = vec!["summary".into(), "settled".into()];
        let el = view(Some(&from), &to, &p).expect("switch divider builds");
        let _ = el;

        let mut carried = meta("m3", "claude", "account-default");
        carried.started_by_kind = Some("carry-on".into());
        carried.carried_on_for = Some("provider".into());
        assert!(view(Some(&from), &carried, &p).is_some());
    }
}
