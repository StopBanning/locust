use super::*;

#[derive(Clone)]
pub(super) struct PeerMsgView {
    pub message_id: String,
    pub direction: String,
    pub from_id: String,
    pub from_name: String,
    pub from_mission: Option<String>,
    pub to_id: String,
    pub to_name: String,
    pub text: Option<String>,
    pub posted_at: String,
}

pub(super) fn open_mission_by_id(s: &mut State, mission_id: String, tx: &mut Tx) {
    if let Some(item) = s.recent.iter().find(|r| r.id == mission_id).cloned() {
        open_recent(s, item, tx);
        return;
    }
    let row = s
        .ledger_summaries
        .get(&mission_id)
        .map(|m| ConversationRow {
            id: m.mission_id.clone(),
            teammate: s
                .mission_owners
                .get(&m.mission_id)
                .and_then(|tid| s.teammates.iter().find(|t| t.id == *tid))
                .map(|t| t.name.clone())
                .unwrap_or_else(|| engine::runtime_name(&m.runtime).into()),
            title: m.title.clone(),
            started_by: "native".into(),
            state: if m.running {
                "running"
            } else if m.failed {
                "failed"
            } else {
                "completed"
            }
            .into(),
            updated: m.updated.clone(),
        })
        .unwrap_or_else(|| ConversationRow {
            id: mission_id,
            teammate: "Teammate".into(),
            title: "Conversation".into(),
            started_by: "native".into(),
            state: "completed".into(),
            updated: String::new(),
        });
    open_recent(s, row, tx);
}

fn received_in_map(s: &State) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for sum in s.ledger_summaries.values() {
        for link in &sum.peer_links {
            if link.direction == "received" {
                map.entry(link.message_id.clone())
                    .or_insert_with(|| sum.mission_id.clone());
            }
        }
    }
    map
}

pub(super) fn messages_for_mission(s: &State, mission_id: &str) -> Vec<PeerMsgView> {
    let Some(pane) = &s.ledger else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (link_mission, link) in &pane.snapshot.peer_links {
        if link_mission != mission_id {
            continue;
        }
        let wm = s.workroom_messages.get(&link.message_id);
        let peer_name = s
            .teammates
            .iter()
            .find(|t| t.id == link.peer_teammate_id)
            .map(|t| t.name.clone())
            .or_else(|| {
                wm.map(|m| {
                    if link.direction == "received" {
                        m.from.name.clone()
                    } else {
                        m.to.name.clone()
                    }
                })
            })
            .unwrap_or_else(|| "Teammate".to_owned());
        let (from_id, from_name, to_id, to_name) = if link.direction == "received" {
            let self_id = s
                .mission_owners
                .get(mission_id)
                .cloned()
                .unwrap_or_default();
            (
                link.peer_teammate_id.clone(),
                peer_name,
                self_id,
                "You".to_owned(),
            )
        } else {
            let self_id = s
                .mission_owners
                .get(mission_id)
                .cloned()
                .unwrap_or_default();
            (
                self_id.clone(),
                s.teammates
                    .iter()
                    .find(|t| t.id == self_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "Teammate".to_owned()),
                link.peer_teammate_id.clone(),
                peer_name,
            )
        };
        out.push(PeerMsgView {
            message_id: link.message_id.clone(),
            direction: link.direction.clone(),
            from_id,
            from_name,
            from_mission: wm.map(|m| m.from_mission.clone()),
            to_id,
            to_name,
            text: wm.map(|m| m.text.clone()),
            posted_at: wm
                .map(|m| m.posted_at.clone())
                .unwrap_or_else(|| link.occurred_at.clone()),
        });
    }
    out
}

pub(super) fn handed_out(s: &State, msgs: &[PeerMsgView], p: &Palette) -> Option<El> {
    let posted: Vec<&PeerMsgView> = msgs.iter().filter(|m| m.direction == "posted").collect();
    if posted.is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let received_in = received_in_map(s);
    let mut card = col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(8.0)
        .border(1.0, p.hairline)
        .fill(l.c("bg-raised"))
        .child(
            text(format!("Handed out · {}", posted.len()))
                .font(l.ty.label)
                .color(l.c("text-muted")),
        );
    for msg in posted {
        let body = msg
            .text
            .clone()
            .unwrap_or_else(|| "This message is no longer in the workroom.".to_owned());
        let line_text = format!("To {}: {body}", msg.to_name);
        if let Some(target_mission) = received_in.get(&msg.message_id).cloned() {
            card = card.child(
                text(line_text)
                    .font(l.ty.body)
                    .color(l.c("text-primary"))
                    .w_full()
                    .wrap()
                    .role("button")
                    .hover(l.c("hover-surface"))
                    .on_press_tx(move |s: &mut State, tx| {
                        open_mission_by_id(s, target_mission.clone(), tx);
                    }),
            );
        } else {
            card = card.child(
                text(line_text)
                    .font(l.ty.body)
                    .color(l.c("text-primary"))
                    .w_full()
                    .wrap(),
            );
        }
    }
    Some(card)
}

pub(super) fn cards_for_mission(
    s: &State,
    self_teammate_id: &str,
    mission_id: &str,
    msgs: &[PeerMsgView],
    p: &Palette,
) -> Vec<El> {
    if msgs.is_empty() {
        return Vec::new();
    }
    let l = crate::look::controls::Look::new(p.dark);
    let received_in = received_in_map(s);
    let mut groups: Vec<(String, String, Vec<&PeerMsgView>)> = Vec::new();
    for msg in msgs {
        let (peer_id, peer_name) = if msg.direction == "received" {
            (&msg.from_id, &msg.from_name)
        } else {
            (&msg.to_id, &msg.to_name)
        };
        if let Some((_, _, list)) = groups.iter_mut().find(|(id, _, _)| id == peer_id) {
            list.push(msg);
        } else {
            groups.push((peer_id.clone(), peer_name.clone(), vec![msg]));
        }
    }

    let mut out = Vec::new();
    for (peer_id, peer_name, group) in groups {
        let toggle_key = format!("{mission_id}:{peer_id}");
        let open = s
            .peer_thread_open
            .get(&toggle_key)
            .copied()
            .unwrap_or_else(|| group.len() <= 4);
        let first_from = group.first().map(|m| m.from_id.as_str()).unwrap_or("");
        let same_sender = group.iter().all(|m| m.from_id == first_from);
        let count = group.len();
        let noun = if count == 1 { "message" } else { "messages" };
        let header_label = if !same_sender {
            format!("{count} {noun} with {peer_name}")
        } else if first_from == self_teammate_id {
            format!("{count} {noun} to {peer_name}")
        } else {
            format!("{count} {noun} from {peer_name}")
        };
        let snippet = (!open).then(|| {
            let raw = group
                .first()
                .and_then(|m| m.text.as_deref())
                .unwrap_or("");
            let trimmed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            trimmed.chars().take(72).collect::<String>()
        });

        let tk = toggle_key.clone();
        let mut toggle_row = row()
            .key(format!("peer-toggle-{toggle_key}"))
            .w_full()
            .center_y()
            .gap(8.0)
            .role("button")
            .hover(l.c("hover-surface"))
            .on_press(move |s: &mut State| {
                let current = s
                    .peer_thread_open
                    .get(&tk)
                    .copied()
                    .unwrap_or_else(|| count <= 4);
                s.peer_thread_open.insert(tk.clone(), !current);
            })
            .child(
                text(header_label)
                    .font(l.ty.label)
                    .color(l.c("text-primary")),
            );
        if let Some(snip) = snippet
            && !snip.is_empty()
        {
            toggle_row = toggle_row.child(
                text(snip)
                    .font(l.ty.label)
                    .color(l.c("text-muted"))
                    .shrink()
                    .truncate(),
            );
        }

        let mut header_bar = row().w_full().center_y().gap(8.0).child(toggle_row.grow(1.0));
        let sender_mission = group
            .iter()
            .rev()
            .find(|m| m.direction == "received")
            .and_then(|m| m.from_mission.clone());
        if let Some(origin_mission) = sender_mission {
            header_bar = header_bar.child(
                crate::look::controls::button(
                    &l,
                    &format!("Open {peer_name}'s conversation ↗"),
                    crate::look::controls::Tone::Neutral,
                    false,
                )
                .key(format!("peer-open-sender-{mission_id}-{peer_id}"))
                .on_press_tx(move |s: &mut State, tx| {
                    open_mission_by_id(s, origin_mission.clone(), tx);
                }),
            );
        }

        let mut card = col()
            .key(format!("peer-thread-{mission_id}-{peer_id}"))
            .w_full()
            .gap(6.0)
            .padding(10.0)
            .radius(8.0)
            .border(1.0, p.hairline)
            .fill(l.c("bg-raised"))
            .child(header_bar);

        if open {
            for msg in group {
                let mut msg_col = col().w_full().gap(2.0);
                if !same_sender {
                    let who_line = if msg.posted_at.is_empty() {
                        format!("{} sent", msg.from_name)
                    } else {
                        format!("{} sent · {}", msg.from_name, when(&msg.posted_at, clock_seconds()))
                    };
                    msg_col = msg_col.child(
                        text(who_line)
                            .font(l.ty.label)
                            .color(l.c("text-muted")),
                    );
                }
                let body = msg
                    .text
                    .clone()
                    .unwrap_or_else(|| "This message is no longer in the workroom.".to_owned());
                if let Some(target_mission) = received_in.get(&msg.message_id).cloned() {
                    msg_col = msg_col.child(
                        text(body)
                            .font(l.ty.body)
                            .color(l.c("text-primary"))
                            .w_full()
                            .wrap()
                            .role("button")
                            .hover(l.c("hover-surface"))
                            .on_press_tx(move |s: &mut State, tx| {
                                open_mission_by_id(s, target_mission.clone(), tx);
                            }),
                    );
                } else {
                    msg_col = msg_col.child(
                        text(body)
                            .font(l.ty.body)
                            .color(l.c("text-primary"))
                            .w_full()
                            .wrap()
                            .selectable(true),
                    );
                }
                card = card.child(msg_col);
            }
        }
        out.push(card);
    }
    out
}

pub(super) fn live_relay_rows(s: &State, p: &Palette) -> Vec<El> {
    let l = crate::look::controls::Look::new(p.dark);
    let mut rows = Vec::new();
    let chain_ids: Vec<&str> = s
        .ledger
        .as_ref()
        .map(|pane| {
            pane.snapshot
                .turn_metas
                .iter()
                .map(|m| m.mission_id.as_str())
                .collect()
        })
        .unwrap_or_default();
    if let Some(starting) = &s.relay_live.starting
        && chain_ids.contains(&starting.answering.as_str())
    {
        rows.push(
            text(format!("Starting {}…", starting.name))
                .font(l.ty.label)
                .color(l.c("text-muted")),
        );
    }
    for notice in &s.relay_live.notices {
        if chain_ids.is_empty() || chain_ids.contains(&notice.mission_id.as_str()) {
            let ink = if notice.level == "warning" {
                p.amber
            } else {
                l.c("text-muted")
            };
            rows.push(text(notice.message.clone()).font(l.ty.body).color(ink).wrap());
        }
    }
    rows
}
