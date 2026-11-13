use super::*;
use locust_core::relay::{
    ExchangeMission, ExchangeOverview, ExchangePeerMessage, exchange_across, exchange_of,
};

pub(super) fn compute(s: &State) -> Option<(ExchangeOverview, u32)> {
    let pane = s.ledger.as_ref()?;
    let tip_id = if pane.snapshot.tip.is_empty() {
        pane.conversation.conversation_id.as_deref()?
    } else {
        pane.snapshot.tip.as_str()
    };
    let live_set: std::collections::HashSet<&str> =
        s.seen_live.iter().map(String::as_str).collect();
    let owner_name = |tid: Option<&str>| -> Option<String> {
        let tid = tid?;
        s.teammates.iter().find(|t| t.id == tid).map(|t| t.name.clone())
    };
    let mut missions: Vec<ExchangeMission> = s
        .ledger_summaries
        .values()
        .map(|sum| {
            let links = sum
                .peer_links
                .iter()
                .map(|l| ExchangePeerMessage {
                    message_id: l.message_id.clone(),
                })
                .collect();
            let live = live_set.contains(sum.mission_id.as_str()) || sum.running;
            let owner_id = s.mission_owners.get(&sum.mission_id).cloned();
            ExchangeMission {
                mission_id: sum.mission_id.clone(),
                runtime: sum.runtime.clone(),
                model: sum.model.clone(),
                cost: sum.cost.clone(),
                peer_messages: links,
                started_by_hop: sum.started_by_hop,
                teammate_name: owner_name(owner_id.as_deref()),
                teammate_id: owner_id,
                live,
                run_id: live.then(|| sum.mission_id.clone()),
            }
        })
        .collect();
    if !missions.iter().any(|m| m.mission_id == tip_id)
        && let Some(tip_meta) = pane.snapshot.turn_metas.last()
    {
        let links = pane
            .snapshot
            .peer_links
            .iter()
            .filter(|(mid, _)| mid == &tip_meta.mission_id)
            .map(|(_, l)| ExchangePeerMessage {
                message_id: l.message_id.clone(),
            })
            .collect();
        let live = live_set.contains(tip_meta.mission_id.as_str()) || pane.snapshot.running;
        let owner_id = s.mission_owners.get(&tip_meta.mission_id).cloned();
        missions.push(ExchangeMission {
            mission_id: tip_meta.mission_id.clone(),
            runtime: tip_meta.runtime.clone(),
            model: tip_meta.model.clone(),
            cost: pane.snapshot.cost.clone(),
            peer_messages: links,
            started_by_hop: tip_meta.started_by_hop,
            teammate_name: owner_name(owner_id.as_deref()),
            teammate_id: owner_id,
            live,
            run_id: live.then(|| tip_meta.mission_id.clone()),
        });
    }
    let chain_ids: Vec<String> = pane
        .snapshot
        .turn_metas
        .iter()
        .map(|m| m.mission_id.clone())
        .collect();
    let chain_refs: Vec<&str> = chain_ids.iter().map(String::as_str).collect();
    let overview = if chain_refs.len() > 1 {
        exchange_across(&chain_refs, &missions)
    } else {
        exchange_of(tip_id, &missions)
    }?;
    Some((overview, s.relay_hop_cap))
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let (exchange, cap) = compute(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let live = !exchange.live_run_ids.is_empty();
    let tip_key = s
        .ledger
        .as_ref()
        .map(|pane| pane.snapshot.tip.clone())
        .unwrap_or_default();
    let open = live || s.exchange_open.as_deref() == Some(tip_key.as_str());
    let cost = locust_core::benchmark::cost_line(exchange.cost.as_ref());
    let cap_noun = if cap == 1 { "reply" } else { "replies" };
    let hops_line = format!("{} of {cap} automatic {cap_noun}", exchange.hops);

    if !live && !open {
        let names = exchange
            .participants
            .iter()
            .map(|part| part.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
        let mut rest = format!("{names} · {hops_line}");
        if let Some(c) = &cost {
            rest.push_str(&format!(" · {c}"));
        }
        let open_key = tip_key.clone();
        return Some(
            row()
                .key("exchange-strip")
                .w_full()
                .center_y()
                .gap(8.0)
                .px(24.0)
                .py(8.0)
                .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                .fill(l.c("bg-raised"))
                .role("button")
                .hover(l.c("hover-surface"))
                .on_press(move |s: &mut State| {
                    s.exchange_open = Some(open_key.clone());
                })
                .child(text("Exchange").font(l.ty.label).color(l.c("text-muted")))
                .child(
                    text(rest)
                        .font(l.ty.label)
                        .color(l.c("text-primary"))
                        .grow(1.0)
                        .truncate(),
                )
                .into(),
        );
    }

    let mut who = row().wrap().center_y().gap(10.0);
    for (idx, part) in exchange.participants.iter().enumerate() {
        let mut cell = row().center_y().gap(5.0);
        if idx > 0 {
            cell = cell.child(text("·").font(l.ty.label).color(l.c("text-muted")));
        }
        cell = cell
            .child(
                el().size(6.0, 6.0)
                    .radius(3.0)
                    .fill(if part.live { p.green } else { l.c("text-muted") }),
            )
            .child(text(part.name.clone()).font(l.ty.label).color(l.c("text-primary")))
            .child(
                text(format!(
                    "{} / {}",
                    locust_core::carry_on::display_name(&part.runtime),
                    if part.model == "account-default" {
                        "default"
                    } else {
                        part.model.as_str()
                    }
                ))
                .font(l.ty.label)
                .color(l.c("text-muted")),
            );
        who = who.child(cell);
    }

    let mut strip = row()
        .key("exchange-strip")
        .w_full()
        .center_y()
        .gap(12.0)
        .px(24.0)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .fill(l.c("bg-raised"));
    if !live {
        strip = strip.child(
            crate::look::controls::button(
                &l,
                "Collapse",
                crate::look::controls::Tone::Neutral,
                false,
            )
            .key("exchange-collapse")
            .on_press(|s: &mut State| {
                s.exchange_open = None;
            }),
        );
    }
    strip = strip
        .child(text("Exchange").font(l.ty.label).color(l.c("text-muted")))
        .child(who.grow(1.0))
        .child(text(hops_line).font(l.ty.label).color(l.c("text-muted")));
    if let Some(c) = cost {
        strip = strip.child(text(c).font(l.ty.label).color(l.c("text-muted")));
    }
    if live {
        let run_ids = exchange.live_run_ids.clone();
        let label = if s.exchange_stopping {
            "Stopping…".to_owned()
        } else if run_ids.len() == 1 {
            "Stop the run".to_owned()
        } else {
            format!("Stop {} runs", run_ids.len())
        };
        let mut stop_btn = crate::look::controls::button(
            &l,
            &label,
            crate::look::controls::Tone::Neutral,
            s.exchange_stopping,
        )
        .key("exchange-stop");
        if !s.exchange_stopping {
            stop_btn = stop_btn.on_press(move |s: &mut State| {
                s.exchange_stopping = true;
                for run_id in &run_ids {
                    let _ = locust_core::engine_api::api().stop(run_id);
                }
                s.exchange_stopping = false;
            });
        }
        strip = strip.child(stop_btn);
    }
    Some(strip)
}
