use super::*;
use crate::conversation_list::{
    SidebarMission, conversation_keys, held_for, narrowing_line, needs_you_pinned_and_recent, rows_with_children,
};
use crate::conversation_search::missions_matching;
use crate::settled::is_put_away;

/// A run that is never a conversation: Scout's own looks and a comparison's judge (App.tsx `isHiddenRun`). Scout names
/// its looks' turns in its state (`lookIds`); a turn that says it was started by Scout or a judge is one too.
pub(super) fn is_background_run(s: &State, id: &str) -> bool {
    s.scout.state.as_ref().is_some_and(|scout| scout.look_ids.iter().any(|look| look == id))
        || matches!(
            s.ledger_summaries.get(id).and_then(|m| m.started_by_kind.as_deref()),
            Some("scout" | "judge")
        )
}

pub(super) fn recent_as_missions(s: &State) -> Vec<SidebarMission> {
    let judges = locust_core::compare::judge_mission_ids(&s.compare.compares);
    let reruns = locust_core::compare::rerun_mission_ids(&s.compare.compares);
    let mut rows = Vec::new();
    let by_id: HashMap<_, _> = s.recent.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut groups: Vec<(String, Vec<&ConversationRow>)> = Vec::new();
    for item in &s.recent {
        if judges.contains(&item.id) || reruns.contains(&item.id) || is_background_run(s, &item.id) {
            continue;
        }
        let mut root = item.id.as_str();
        let mut seen = std::collections::HashSet::from([root]);
        while let Some(parent) = s
            .ledger_summaries
            .get(root)
            .and_then(|m| m.parent.as_deref())
        {
            if !by_id.contains_key(parent) || !seen.insert(parent) {
                break;
            }
            root = parent;
        }
        if let Some((_, members)) = groups.iter_mut().find(|(key, _)| key == root) {
            members.push(item);
        } else {
            groups.push((root.to_owned(), vec![item]));
        }
    }
    for (root, mut members) in groups {
        members.sort_by_key(|m| {
            s.ledger_summaries
                .get(&m.id)
                .map(|m| m.created_at.as_str())
                .unwrap_or("")
        });
        let continued: std::collections::HashSet<_> = members
            .iter()
            .filter_map(|m| s.ledger_summaries.get(&m.id)?.parent.as_deref())
            .collect();
        let Some(leaf) = members
            .iter()
            .rev()
            .find(|m| !continued.contains(m.id.as_str()))
            .or_else(|| members.last())
        else {
            continue;
        };
        let mut mission = SidebarMission {
            mission_id: leaf.id.clone(),
            title: by_id
                .get(root.as_str())
                .map(|m| m.title.clone())
                .unwrap_or_else(|| leaf.title.clone()),
            phase: leaf.state.clone(),
            last_at: Some(leaf.updated.clone()),
            root_id: Some(root),
            member_ids: members.iter().map(|m| m.id.clone()).collect(),
            owner_id: None,
            said: members
                .iter()
                .filter_map(|m| s.ledger_summaries.get(&m.id).map(|m| m.words.as_str()))
                .collect::<Vec<_>>()
                .join(" · "),
            nested_under: None,
            integrity_issue_count: 0,
            needs_you: false,
            app_started: false,
        };
        if let Some(title) = held_for(&mission, &s.org.titles) {
            mission.title = title.clone();
        }
        // Who started its first turn: the root's record, else the earliest turn listed, else this one.
        let kind = |id: &str| s.ledger_summaries.get(id).map(|m| m.started_by_kind.as_deref());
        mission.app_started = crate::conversation_list::app_started_conversation(
            kind(&leaf.id).flatten(),
            members.first().and_then(|m| kind(&m.id)),
            mission.root_id.as_deref().and_then(kind),
        );
        rows.push(mission);
    }
    let mut list = crate::conversation_list::conversation_rows(&rows);
    let nested_rows: Vec<locust_core::relay::NestedRow> = list
        .iter()
        .map(|m| locust_core::relay::NestedRow {
            mission_id: m.mission_id.clone(),
            member_ids: m.member_ids.clone(),
        })
        .collect();
    let parents = locust_core::relay::nested_under(&nested_rows, |nr| {
        let row = list.iter().find(|r| r.mission_id == nr.mission_id)?;
        let root_id = row.root_id.as_deref().unwrap_or(&row.mission_id);
        let summary = s.ledger_summaries.get(root_id)?;
        if summary.started_by_kind.as_deref() != Some("relay") {
            return None;
        }
        summary
            .peer_links
            .iter()
            .find(|l| l.direction == "received")
            .and_then(|l| {
                s.workroom_messages
                    .get(&l.message_id)
                    .map(|msg| msg.from_mission.clone())
            })
            .or_else(|| summary.relay.as_ref().and_then(|r| r.root_mission_id.clone()))
    });
    for m in &mut list {
        if let Some(parent) = parents.get(&m.mission_id) {
            m.nested_under = Some(parent.clone());
        }
        // Any turn of it with a question, an approval or a decision open (`home.waiting_for`, the Board's asks).
        m.needs_you = conversation_keys(m).iter().any(|key| s.home.waiting_for.contains_key(key));
    }
    list
}

/// Find the comparison through any turn, including an answer that was replaced by a retry.
fn comparison_slot<'a>(
    compares: &'a [locust_core::compare::Compare],
    ids: &[String],
) -> Option<(&'a locust_core::compare::Compare, &'a locust_core::compare::CompareSlot)> {
    compares.iter().find_map(|compare| {
        let slot = compare.slots.iter().find(|slot| {
            slot.mission_ids.iter().chain(&slot.retried).any(|id| ids.contains(id))
        })?;
        Some((compare, slot))
    })
}

pub(super) fn comparison_of<'a>(s: &'a State, ids: &[String]) -> Option<&'a locust_core::compare::Compare> {
    comparison_slot(&s.compare.compares, ids).map(|(compare, _)| compare)
}

fn comparison_member_ids(row: &SidebarMission) -> &[String] {
    if row.member_ids.is_empty() { std::slice::from_ref(&row.mission_id) } else { &row.member_ids }
}

pub(super) fn comparison_row_visible(s: &State, row: &SidebarMission) -> bool {
    let ids = comparison_member_ids(row);
    let Some((compare, slot)) = comparison_slot(&s.compare.compares, ids) else { return true };
    let visible = compare.kept.as_ref().map(|kept| kept.slot)
        .or_else(|| compare.slots.iter().find(|slot| !slot.mission_ids.is_empty()).map(|slot| slot.slot));
    Some(slot.slot) == visible && slot.mission_ids.iter().any(|id| ids.contains(id))
}

pub(super) fn open_comparison(s: &mut State, id: &str) {
    crate::room_screen::close(s);
    crate::missions_screen::close(s);
    s.compare.active_id = Some(id.to_owned());
    crate::compare_view::read_catalog_if_needed(s);
    s.benchmarks_open = false;
    s.home.home_open = false;
    s.home.board_open = false;
    s.home.boot_open = false;
    s.people_panel.open = false;
    s.settings_panel.open = false;
    s.routines.open = false;
}

fn undecided_comparison<'a>(s: &'a State, row: &SidebarMission) -> Option<&'a locust_core::compare::Compare> {
    comparison_of(s, comparison_member_ids(row)).filter(|compare| compare.kept.is_none())
}

pub(super) fn open_sidebar_mission(s: &mut State, row: &SidebarMission, tx: &mut Tx) {
    let compare_id = undecided_comparison(s, row).map(|compare| compare.compare_id.clone());
    if let Some(item) = s.recent.iter().find(|item| item.id == row.mission_id).cloned() {
        s.compare.active_id = None;
        open_recent(s, item, tx);
    }
    if let Some(id) = compare_id {
        open_comparison(s, &id);
    }
}

pub(super) fn sidebar_missions_now(s: &State) -> Vec<SidebarMission> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let selected = selected_conversation(s)
        .and_then(|(_, c)| c.conversation_id.clone())
        .or_else(|| s.ledger.as_ref().map(|p| p.snapshot.tip.clone()));
    let query = s.sidebar_query.text.clone();
    let searching = !query.trim().is_empty();
    let missions = recent_as_missions(s);
    let visible: Vec<SidebarMission> = missions
        .into_iter()
        .filter(|m| comparison_row_visible(s, m))
        .filter(|m| {
            let entry = held_for(m, &s.org.settled);
            let put_away = is_put_away(m.last_at.as_deref(), entry, now_ms);
            // A settled conversation that waits on the person is still listed (Sidebar.tsx `mission.needsYou ||`).
            if !put_away || m.needs_you {
                return true;
            }
            // Settled rows hide unless selected or searching (Electron Sidebar).
            searching
                || selected
                    .as_deref()
                    .is_some_and(|id| conversation_keys(m).iter().any(|k| k == id))
        })
        .collect();
    let visible: Vec<SidebarMission> =
        visible.into_iter().filter(|m| crate::conversation_list::conversation_source_matches(m, s.sidebar_source)).collect();
    // A face's filter narrows the list to that teammate's conversations (Sidebar.tsx `faceFilter`).
    let visible: Vec<SidebarMission> = match s.sidebar_face_filter.as_deref() {
        Some(face) => visible.into_iter().filter(|m| row_owner(s, m).as_deref() == Some(face)).collect(),
        None => visible,
    };
    let matched: Vec<SidebarMission> = missions_matching(
        &visible,
        &query,
        Some(|row: &SidebarMission| {
            s.recent
                .iter()
                .find(|r| r.id == row.mission_id)
                .map(|r| r.teammate.clone())
        }),
    )
    .into_iter()
    .cloned()
    .collect();
    matched
}

/// The teammate a conversation belongs to (conversationList.ts `ownerOf`): the row's own owner, the team file's, else
/// the teammate its turns were listed under.
pub(super) fn row_owner(s: &State, item: &SidebarMission) -> Option<String> {
    item.owner_id.clone()
        .or_else(|| s.data.as_ref().and_then(|store| people::owner_of(store, &item.mission_id)))
        .or_else(|| {
            let keys = conversation_keys(item);
            let name = s.recent.iter().find(|r| keys.contains(&r.id) && !r.teammate.is_empty())?.teammate.as_str();
            s.teammates.iter().find(|mate| mate.name == name).map(|mate| mate.id.clone())
        })
}

/// Faces drawn in the strip when the whole team fits, and when a `+N` count shares the row (Sidebar.tsx).
pub(super) const FACES_WITHOUT_CHIP: usize = 5;
pub(super) const FACES_WITH_CHIP: usize = 4;

/// How many faces the strip draws for a team of `team`: all of five or fewer, else four and a count.
pub(super) fn shown_face_count(team: usize) -> usize {
    if team <= FACES_WITHOUT_CHIP { team } else { FACES_WITH_CHIP }
}

/// The team, the ones being worked with first (Sidebar.tsx `facesByRecency`): each by its newest conversation's time,
/// one with none (or none that is a time) last, ties kept in roster order.
pub(super) fn faces_by_recency<'a>(teammates: &'a [Teammate], owned: &[(Option<String>, Option<String>)]) -> Vec<&'a Teammate> {
    let newest = |id: &str| {
        owned.iter()
            .filter(|(owner, _)| owner.as_deref() == Some(id))
            .filter_map(|(_, at)| chrono::DateTime::parse_from_rfc3339(at.as_deref()?).ok())
            .map(|at| at.timestamp_millis())
            .fold(0, i64::max)
    };
    let mut order: Vec<&Teammate> = teammates.iter().collect();
    order.sort_by_key(|mate| std::cmp::Reverse(newest(&mate.id)));
    order
}

/// The routine whose replay began this conversation (App.tsx `routineOf(run.startedBy) ?? routineOf(root.startedBy)`).
pub(super) fn row_routine(s: &State, item: &SidebarMission) -> Option<String> {
    let of = |id: &str| s.ledger_summaries.get(id)
        .and_then(|m| crate::conversation_list::routine_of(m.started_by_kind.as_deref(), m.started_by_routine.as_deref()));
    of(&item.mission_id).or_else(|| item.root_id.as_deref().and_then(of))
}

/// Who a run belongs to: the owners the list was read with, the team file's, else the teammate it was listed under.
fn run_owner(s: &State, id: &str) -> Option<String> {
    s.mission_owners.get(id).cloned()
        .or_else(|| s.data.as_ref().and_then(|store| people::owner_of(store, id)))
        .or_else(|| {
            let name = s.recent.iter().find(|r| r.id == id && !r.teammate.is_empty())?.teammate.as_str();
            s.teammates.iter().find(|mate| mate.name == name).map(|mate| mate.id.clone())
        })
}

/// App.tsx `routineStepByTeammate`: the routine step a teammate is replaying right now -- its name, the step, and of
/// how many -- from a running run a saved routine started.
pub(super) fn routine_step_of(s: &State, teammate: &str) -> Option<(String, usize, usize)> {
    s.ledger_summaries.values().filter(|m| m.running).find_map(|m| {
        let id = crate::conversation_list::routine_of(m.started_by_kind.as_deref(), m.started_by_routine.as_deref())?;
        let routine = s.org.routines.iter().find(|r| r.id == id)?;
        (run_owner(s, &m.mission_id).as_deref() == Some(teammate)).then_some((routine.name.clone(), m.started_by_step?, routine.steps))
    })
}

/// The teammate's live activity is handing work to a subagent (App.tsx `liveActivity[teammateId] === 'delegating'`),
/// read as their face reads it (teammate_faces.rs `view_of`).
fn teammate_delegating(s: &State, teammate: &str) -> bool {
    s.seen_live.iter()
        .filter(|id| s.mission_owners.get(*id).is_some_and(|owner| owner == teammate))
        .find_map(|id| s.live_faces.by_mission.get(id).copied())
        == Some(locust_core::bot_rig::LiveActivity::Delegating)
}

// routeName.ts's own-model identity, rather than any model beginning with "own-".
fn is_own_mark_model(model: &str) -> bool {
    model.strip_prefix("own-").and_then(|rest| rest.split_once('/'))
        .is_some_and(|(id, name)| id.len() == 8 && !name.is_empty()
            && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

fn mission_row(s: &State, p: &Palette, item: &SidebarMission, picked: bool) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let compare = undecided_comparison(s, item);
    let blind = compare.is_some_and(|compare| compare.blind);
    let picked = compare.map_or(picked, |compare| s.compare.active_id.as_deref() == Some(&compare.compare_id));
    // `.lc-conv`: one line, 32 high, whatever the title (the-flat-sidebar-is-written-down).
    let mut title = row().w_full().h(CONVERSATION_ROW_HEIGHT).px(10.0).center_y().gap(8.0);
    let owner = row_owner(s, item);
    let by = owner.as_deref().and_then(|id| s.teammates.iter().find(|mate| mate.id == id));
    // Every row's mark takes the owner's face footprint (`BOT_SIZE.conversationOwner`), so titles start on one x.
    if blind {
    } else if let Some(mate) = by {
        title = title.child(row().size(OWNER_FACE, OWNER_FACE).center()
            .child(roster_face(s, "conversation-owner", mate, None, None, OWNER_FACE, &l))
            // The face names its teammate rather than hiding from a screen reader (Sidebar.tsx `TeammateBot name`).
            .role("img").label(mate.name.clone())
            .key(format!("conversation-owner-{}", item.mission_id)));
    } else {
        let summary = s.ledger_summaries.get(&item.mission_id);
        let mark = summary.filter(|m| !m.runtime.is_empty() && !m.model.is_empty() && !is_own_mark_model(&m.model))
            .and_then(|m| crate::look::controls::agent_mark(&l, &m.runtime, 13.0, l.c("text-muted"))
                .map(|mark| row().size(OWNER_FACE, OWNER_FACE).center().child(mark)
                    .key(format!("conversation-runtime-{}", item.mission_id))
                    .role("img").label(locust_core::inspector::runtime_display_name(&m.runtime))));
        // `.lc-conv__nobody`: a 16 box, 2 either side, centred in the face's 20.
        title = title.child(mark.unwrap_or_else(|| el().size(16.0, 16.0).mx((OWNER_FACE - 16.0) / 2.0).radius(4.0)
            .border(1.0, l.c("border-hairline")).a11y_hidden()
            .key(format!("conversation-nobody-{}", item.mission_id))));
    }
    // Another app started it: a plug before the title, as a routine wears its clock, said to a screen reader and on
    // hover, and no words after the title (Sidebar.tsx `lc-conv__routine lc-conv__app`, 0.872).
    if item.app_started {
        let key = format!("conversation-app-{}", item.mission_id);
        let plug = row().size(14.0, 18.0).center()
            .child(crate::look::controls::glyph(&l, &crate::look::icons::PLUG, 11.0, Some(l.c("text-muted"))))
            .role("img").label(APP_STARTED).key(key.clone());
        title = title.child(crate::hover::tip(s, &l, &key, APP_STARTED, plug));
    }
    // A routine's run wears the Routines clock before its title, said to a screen reader; the conversation it was
    // saved from does not (Sidebar.tsx `lc-conv__routine`, the 0.271 recheck). A deleted routine is still "a routine".
    let from_routine = row_routine(s, item).map(|id| match s.org.routines.iter().find(|r| r.id == id) {
        Some(r) => format!("from the routine {}", r.name),
        None => "from a routine".to_owned(),
    });
    if let Some(from) = &from_routine {
        let said = format!("{}{}", from[..1].to_uppercase(), &from[1..]);
        title = title.child(row().size(14.0, 18.0).center()
            .child(crate::look::controls::glyph(&l, &crate::look::icons::CLOCK, 11.0, Some(l.c("text-muted"))))
            .role("img").label(said).key(format!("conversation-routine-{}", item.mission_id)));
    }
    // A working row's title shimmers instead of wearing a dot (Sidebar.tsx `lc-conv__title is-working`).
    let working = item.phase == "running";
    let name = if working {
        crate::working_shimmer::title(&item.title, l.c("text-primary"), s.reduced_motion.get())
    } else {
        text(item.title.clone()).color(l.c("text-name"))
    };
    title = title.child(
        name.font(rune::TextStyle::new(14.0, 400.0))
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .truncate()
            .key(format!("conversation-title-{}", item.mission_id)),
    );
    if compare.is_some() {
        title = title.child(text("vs").font(l.ty.mono).color(p.muted).label("A comparison"));
    }
    if item.needs_you {
        // Waiting on the person: amber words where the age goes and the amber dot, in place of the shimmer or the
        // failed mark (Sidebar.tsx `lc-conv__age lc-tone-amber` "needs you", `lc-dot lc-tone-amber`).
        title = title
            .child(text("needs you").font(rune::TextStyle::new(11.5, 400.0)).color(p.amber).key(format!("conversation-needs-you-{}", item.mission_id)))
            .child(el().size(6.0, 6.0).radius(3.0).fill(p.amber).a11y_hidden().key(format!("conversation-needs-you-dot-{}", item.mission_id)));
    } else if conversation_keys(item).iter().any(|id| crate::voice_panel::calling(s, id)) {
        title = title.child(text("on a call").font(type_::LABEL).color(p.secondary)
            .key(format!("conversation-on-call-{}", item.mission_id)));
    } else if working && by.is_some_and(|mate| teammate_delegating(s, &mate.id)) {
        // A subagent at work is said on the conversation that sent it (Sidebar.tsx `delegating`, Yurt's #17).
        title = title.child(text("subagent working").font(l.ty.mono).color(p.muted).key(format!("conversation-age-{}", item.mission_id)));
    } else if let Some((name, step, of)) =
        by.filter(|_| working && from_routine.is_some()).and_then(|mate| routine_step_of(s, &mate.id))
    {
        // A routine's run says which step it is on while it runs, where its age would be (Sidebar.tsx `runningStep`).
        title = title.child(text(format!("step {step} of {of}")).font(l.ty.mono).color(p.muted)
            .help(format!("{name}: step {step} of {of}")).key(format!("conversation-age-{}", item.mission_id)));
    } else if working {
        // The dot's footprint, kept empty, so a run starting never moves the row (`.lc-conv__working`, 6 by 6).
        title = title.child(el().size(6.0, 6.0).a11y_hidden().key(format!("conversation-working-{}", item.mission_id)));
    } else if item.phase == "failed" {
        title = title.child(el().size(5.0, 5.0).radius(2.5).fill(p.red));
    }
    let id = item.mission_id.clone();
    // Whose it is goes in the hover with the title, as a second line no longer says it (Sidebar.tsx `hover`).
    let hover = std::iter::once(item.title.clone())
        .chain(by.filter(|_| !blind).map(|mate| mate.name.clone()))
        .chain(from_routine.clone())
        .collect::<Vec<_>>()
        .join(" · ");
    if picked || s.recent_hover.as_deref() == Some(&item.mission_id) {
        let menu_id = item.mission_id.clone();
        let menu_key = format!("org-menu-open-{menu_id}");
        title = title.child(
            crate::look::controls::head_icon(&crate::look::icons::DOTS, &format!("Actions for {}", item.title), "Actions")
                .control_size(rune_kit::ControlSize::Small)
                .on_press(move |s: &mut State| s.org_menu_id = Some(menu_id.clone()))
                .build(l.k)
                .key(menu_key),
        );
    } else {
        title = title.child(el().size(22.0, 22.0));
    }
    let hover_id = id.clone();
    let mission = item.clone();
    let row_key = compare.map_or_else(|| format!("recent-{id}"), |compare| format!("sidebar-compare-{}", compare.compare_id));
    let mut row_el = col()
        .on_hover_change(move |s: &mut State, over, _| {
            if over {
                s.recent_hover = Some(hover_id.clone());
            } else if s.recent_hover.as_deref() == Some(&hover_id) {
                s.recent_hover = None;
            }
        })
        .key(row_key)
        .w_full()
        .radius(8.0)
        .fill(if picked { l.c("bg-selected") } else { crate::window_glass::sidebar_row_ground(s, p) })
        .hover(l.c("hover-surface"))
        .help(hover)
        .child(title)
        .on_press_tx(move |s: &mut State, tx| open_sidebar_mission(s, &mission, tx))
        // Right-click: the conversation's menu at the pointer (Sidebar.tsx onContextMenu -> App.tsx openMissionMenu).
        .on_secondary_press({
            let (menu_id, menu_title) = (item.mission_id.clone(), item.title.clone());
            move |s: &mut State, at: rune::PointerPress, _| {
                s.org_menu_id = None;
                s.row_menu = Some(crate::context_menu::conversation_menu(s, &menu_id, &menu_title, (at.x, at.y)));
            }
        });
    if s.org_menu_id.as_deref() == Some(item.mission_id.as_str()) {
        row_el = row_el.child(org_menu(s, p, &item.mission_id));
    }
    row_el
}

/// `.lc-conv`'s height: one line, never two.
pub(super) const CONVERSATION_ROW_HEIGHT: f32 = 32.0;
/// `BOT_SIZE.conversationOwner`: the owner's face on a row, and the footprint every stand-in for it takes.
pub(super) const OWNER_FACE: f32 = 20.0;

/// What the plug on an app-started row says (Sidebar.tsx's aria-label and title).
const APP_STARTED: &str = "Started by another app through Locust";

/// Sidebar.tsx's Conversation source menu: offered once any conversation came from another app, or while it narrows.
fn source_menu_shown(s: &State) -> bool {
    s.sidebar_source != crate::conversation_list::ConversationSource::All || recent_as_missions(s).iter().any(|m| m.app_started)
}

fn source_select(s: &State) -> rune_kit::Select {
    use crate::conversation_list::ConversationSource;
    let words: Vec<&str> = ConversationSource::ALL.iter().map(|(_, w)| *w).collect();
    let highlighted = usize::try_from(s.sidebar_source_active).ok().and_then(|i| words.get(i).copied());
    rune_kit::Select::new("sidebar-source", &words)
        .label("Conversation source")
        .current(Some(s.sidebar_source.words()))
        .highlighted(if s.sidebar_source_open { highlighted } else { None })
        .open(s.sidebar_source_open)
        .on_toggle(|s: &mut State| {
            s.sidebar_source_open = !s.sidebar_source_open;
            s.sidebar_source_active = source_index(s.sidebar_source);
        })
        .on_close(|s: &mut State| s.sidebar_source_open = false)
        .on_choose(|s: &mut State, words: &str, _tx: &mut Tx| {
            if let Some(source) = ConversationSource::from_words(words) {
                s.sidebar_source = source;
            }
            s.sidebar_source_open = false;
        })
}

fn source_index(source: crate::conversation_list::ConversationSource) -> i64 {
    crate::conversation_list::ConversationSource::ALL.iter().position(|(s, _)| *s == source).map_or(-1, |i| i as i64)
}

/// The source select's keyboard, open or closed (Select.tsx `onKeyDown` over `applySelectKey`): arrows, Home, End and a
/// typed letter move the highlight, Enter and Space choose it, 1-9 choose a row, Escape and Tab close and keep the value.
pub(super) fn source_key(s: &mut State, event: &KeyEvent, _tx: &mut Tx) -> bool {
    use crate::conversation_list::ConversationSource;
    use crate::look::select_keys::{SelectKeyInput, TYPEAHEAD_SECONDS, apply_select_key, dom_key};
    let Some(key) = dom_key(&event.key) else { return false };
    let labels: Vec<&str> = ConversationSource::ALL.iter().map(|(_, w)| *w).collect();
    let enabled: Vec<i64> = (0..labels.len() as i64).collect();
    let now = std::time::Instant::now();
    let live = s.sidebar_source_typed.1.is_some_and(|at| now.duration_since(at).as_secs_f64() < TYPEAHEAD_SECONDS);
    let result = apply_select_key(&SelectKeyInput {
        open: s.sidebar_source_open,
        active: s.sidebar_source_active,
        selected: source_index(s.sidebar_source),
        enabled: &enabled,
        labels: &labels,
        key: &key,
        alt: event.mods.alt,
        ctrl: event.mods.ctrl,
        meta: event.mods.cmd,
        query: &s.sidebar_source_typed.0,
        query_live: live,
    });
    if !result.prevent && result.open == s.sidebar_source_open {
        return false;
    }
    s.sidebar_source_typed = (result.query, Some(now));
    s.sidebar_source_active = result.active;
    // A choice closes it: `apply_select_key` answers closed whenever it chooses.
    s.sidebar_source_open = result.open;
    if result.choose && let Some((source, _)) = usize::try_from(result.active).ok().and_then(|i| ConversationSource::ALL.get(i)) {
        s.sidebar_source = *source;
    }
    result.prevent
}

/// The source menu's field, in the sidebar under the search.
fn source_menu(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    source_menu_shown(s).then(|| col().w_full().pt(6.0).child(source_select(s).build(&l.k, &mut Vec::new()).on_key(source_key)))
}

/// Its open listbox, over the whole window as the right-click menu is (main.rs `view`): drawn inside the sidebar it
/// would be clipped by the list's scroll and could not be pressed.
pub(super) fn source_menu_layer(s: &State, p: &Palette) -> Vec<El> {
    let mut layer = Vec::new();
    if s.sidebar_source_open && source_menu_shown(s) {
        let l = crate::look::controls::Look::new(p.dark);
        source_select(s).build(&l.k, &mut layer);
    }
    layer.into_iter().map(|e| e.on_key(source_key)).collect()
}

fn org_menu(s: &State, p: &Palette, mission_id: &str) -> El {
    let pinned = s.org.pins.iter().any(|k| {
        let missions = recent_as_missions(s);
        missions
            .iter()
            .any(|m| m.mission_id == mission_id && conversation_keys(m).iter().any(|id| id == k))
            || k == mission_id
    });
    let settled = recent_as_missions(s)
        .iter()
        .find(|m| m.mission_id == mission_id)
        .is_some_and(|m| held_for(m, &s.org.settled).is_some());
    let id = mission_id.to_owned();
    let mut menu = col().w_full().gap(4.0).pt(6.0);
    let actions: [(&str, &str); 6] = [
        ("Open beside", "beside"),
        ("Rename", "rename"),
        (if pinned { "Unpin" } else { "Pin to top" }, "pin"),
        (if settled { "Bring back" } else { "Settle" }, "settle"),
        ("Save the record…", "save-record"),
        ("Delete", "delete"),
    ];
    for (label, action) in actions {
        let id = id.clone();
        let action = action.to_owned();
        menu = menu.child(
            text(label.to_owned())
                .key(format!("org-{action}-{id}"))
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    apply_org_action(s, &id, &action);
                }),
        );
    }
    menu = menu.child(text("Snooze").font(type_::LABEL).color(p.muted));
    for choice in settled::snooze_choices(chrono::Local::now()) {
        let id = id.clone();
        menu = menu.child(
            text(choice.label)
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    apply_org_action(s, &id, &format!("snooze:{}", choice.until));
                }),
        );
    }
    menu = menu.child(text("Move to project").font(type_::LABEL).color(p.muted));
    for (gid, label) in std::iter::once((String::new(), "Ungrouped".to_owned())).chain(
        s.org
            .groups
            .iter()
            .map(|g| (g.group_id.clone(), g.name.clone())),
    ) {
        let id = id.clone();
        menu = menu.child(text(label).font(type_::LABEL).color(p.secondary).on_press(
            move |s: &mut State| {
                apply_org_action(s, &id, &format!("group:{gid}"));
            },
        ));
    }
    let id = id.clone();
    menu.child(
        text("New project…")
            .font(type_::LABEL)
            .color(p.secondary)
            .on_press(move |s: &mut State| {
                apply_org_action(s, &id, "new-project");
            }),
    )
}

pub(super) fn apply_org_action(s: &mut State, mission_id: &str, action: &str) {
    if action == "save-record" {
        crate::save_record::open(s, mission_id);
        s.org_menu_id = None;
        return;
    }
    let Some(store) = s.data.clone() else {
        s.status = "No native data folder for conversation organization.".into();
        s.status_tone = StatusTone::Amber;
        s.org_menu_id = None;
        return;
    };
    let row = recent_as_missions(s)
        .into_iter()
        .find(|m| conversation_keys(m).iter().any(|k| k == mission_id));
    let stable_id = row
        .as_ref()
        .and_then(|m| m.root_id.clone())
        .unwrap_or_else(|| mission_id.to_owned());
    let mission_id = if action == "beside" {
        mission_id
    } else {
        stable_id.as_str()
    };
    let result = match action {
        "beside" => {
            let title = row
                .as_ref()
                .map(|r| r.title.clone())
                .unwrap_or_else(|| "Conversation".into());
            let owner_name = s
                .recent
                .iter()
                .find(|r| r.id == mission_id)
                .map(|r| r.teammate.clone());
            let middle = selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.clone())
                .or_else(|| s.ledger.as_ref().map(|p| p.snapshot.tip.clone()));
            if beside::beside_allowed(middle.as_deref(), mission_id) {
                let snapshot = ledger_dir()
                    .map(|dir| native::session(dir, mission_id.into()).poll())
                    .transpose()
                    .map_err(|e| e.to_string());
                let snapshot = match snapshot {
                    Ok(value) => value,
                    Err(error) => {
                        s.status = error;
                        s.status_tone = StatusTone::Amber;
                        return;
                    }
                };
                let lines = snapshot.as_ref().map(|snap| beside::beside_lines(&snap.items)).unwrap_or_default();
                s.beside = Some(beside::BesideState {
                    mission_id: mission_id.into(),
                    title,
                    owner_name,
                    prompt: String::new(),
                    reply_lines: lines,
                });
            }
            Ok(())
        }
        "new-project" => {
            s.rename_id = Some(format!("@new-project:{mission_id}"));
            s.rename_draft.clear();
            s.rename_draft.focused = true;
            Ok(())
        }
        "rename" => {
            s.rename_id = Some(mission_id.into());
            let current = s.org.titles.get(mission_id).cloned().unwrap_or_default();
            s.rename_draft.replace_text(&current);
            Ok(())
        }
        "pin" => {
            let keys = row
                .as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]);
            org::toggle_conversation_pin(&store, &mut s.org, mission_id, &keys)
        }
        "settle" => {
            let keys = row
                .as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]);
            org::toggle_conversation_settled(&store, &mut s.org, mission_id, &keys)
        }
        "delete" => {
            // Every turn of the conversation goes, as Electron's App deletes each of the row's `memberIds`: an earlier
            // turn left behind would bring the conversation back as that turn.
            let keys = row.as_ref().map(conversation_keys).unwrap_or_else(|| vec![mission_id.into()]);
            let deleted = delete_conversation(s, &store, &keys);
            if deleted.is_ok() {
                after_deleting(s);
            }
            deleted
        }
        action if action.starts_with("snooze:") => org::settle_mission(
            &store,
            &mut s.org,
            mission_id,
            Some(Some(action[7..].into())),
        ),
        action if action.starts_with("group:") => {
            let gid = (action.len() > 6).then_some(&action[6..]);
            let keys = row
                .as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]);
            let res = org::assign_conversation(&store, &mut s.org, mission_id, &keys, gid);
            if res.is_ok()
                && let Some(gid) = gid
                && let Some(route) = s
                    .org
                    .groups
                    .iter()
                    .find(|g| g.group_id == gid)
                    .and_then(|g| g.route.as_ref())
                    .cloned()
            {
                let shown = selected_conversation(s)
                    .and_then(|(_, c)| c.conversation_id.as_deref())
                    .or_else(|| {
                        s.ledger
                            .as_ref()
                            .filter(|pane| !pane.snapshot.tip.is_empty())
                            .map(|pane| pane.snapshot.tip.as_str())
                    });
                if shown.is_none_or(|id| keys.iter().any(|k| k == id)) {
                    crate::group_settings_dialog::seed_route_from_group(s, &route);
                }
            }
            res
        }
        _ => Ok(()),
    };
    if let Err(words) = result {
        s.status = words;
        s.status_tone = StatusTone::Amber;
    }
    s.org_menu_id = None;
}

/// Delete one conversation, every turn of it (`keys`): the one path the row menu's Delete and the Conversations
/// screen's bar both take. Refused whole while any turn runs; a conversation only in Electron's record is said so.
pub(super) fn delete_conversation(s: &mut State, store: &crate::store::Store, keys: &[String]) -> Result<(), String> {
    let running = locust_core::engine_api::api().running();
    let dir = store.ledger_dir().map_err(|e| e.to_string())?;
    if keys.iter().any(|k| running.contains(k)) {
        return Err(locust_core::trash::STILL_RUNNING.to_owned());
    }
    let mut deleted = 0;
    for key in keys {
        if locust_core::trash::delete_mission(&dir, key, &running)? {
            locust_core::diagnostic_log::this_session().deleted(store.root(), key);
            deleted += 1;
        }
    }
    if deleted == 0 {
        // Not in this app's record: Electron's conversations are read here, never changed.
        return Err(locust_core::trash::ELECTRON_ONLY.to_owned());
    }
    let shown = s.ledger.as_ref().and_then(|pane| pane.conversation.conversation_id.clone());
    if shown.is_some_and(|id| keys.contains(&id)) {
        s.ledger = None;
    }
    Ok(())
}

/// What follows a delete, once however many conversations went: the trash and storage lines, and the list read again.
pub(super) fn after_deleting(s: &mut State) {
    crate::backup_section::reload(s);
    load_team(s);
}

/// The runtime a roster face wears the mark of: its teammate's, unless the teammate runs on a model of the person's own
/// (an OpenCode route through their own key, which no maker's mark is true of) or has no route.
pub(crate) fn roster_runtime(teammate: &Teammate) -> Option<&str> {
    let runtime = teammate.runtime.as_deref()?;
    let own = teammate.model.as_deref().is_some_and(|model| locust_core::spend::is_own_model(runtime, model));
    (!own).then_some(runtime)
}

/// A roster face (Sidebar.tsx 1511-1519, TeammateBot): the live rig showing the teammate's one state
/// (`teammate_faces`), turned toward a teammate it is handing a message to or getting one from.
pub(super) fn roster_face(
    s: &State,
    surface: &str,
    teammate: &Teammate,
    view: Option<&locust_core::face_status::StatusView>,
    glance: Option<locust_core::bot_rig::GlanceSide>,
    size: f32,
    l: &crate::look::controls::Look,
) -> El {
    use locust_core::bot_rig::FaceActivity;
    crate::look::faces::bot_face(
        s,
        &format!("{surface}-{}", teammate.id),
        &teammate.id,
        &teammate.name,
        size,
        crate::look::faces::BotFaceOpts {
            activity: view.map_or(FaceActivity::Idle, |v| v.activity),
            presence: view.and_then(crate::teammate_faces::presence_dot),
            glance,
            motion: locust_core::bot_rig::BotMotionLevel::Subtle,
            // Which model is who: the runtime's mark, and none on a model of the person's own (Sidebar.tsx).
            runtime: roster_runtime(teammate),
            ..Default::default()
        },
        l,
    )
}

/// `.lc-sectionlabel`: mono capitals, tracked wide, muted.
/// A room is working while a conversation one of its posts started is running.
pub(super) fn room_working(room: &locust_core::room_store::PublicRoom, running: &[String]) -> bool {
    room.posts.iter().any(|post| post.missions.values().any(|mission| running.contains(mission)))
}

/// Whether a drawn row shimmers, so the window keeps moving the band (and stops asking for frames once none does).
pub(super) fn shimmering(s: &State) -> bool {
    // The narrow rail draws no titles.
    if s.layout_compact.get() {
        return false;
    }
    if drawn_missions(s).iter().any(|item| item.phase == "running") {
        return true;
    }
    let running = locust_core::engine_api::api().running();
    !running.is_empty() && s.rooms.rooms.iter().any(|room| room_working(room, &running))
}

fn section_label(p: &Palette, words: &str) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    crate::look::controls::label(&l, words).w_full().px(10.0).pt(16.0).pb(6.0)
}

/// A list heading as Sidebar.tsx draws it (`.lc-project__name`: Pinned, a group's name, Ungrouped, Recents): the
/// label's size and colour, in the words' own case, since shell.css gives it no text-transform.
fn section_name(p: &Palette, words: &str) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    text(words.to_owned()).font(l.ty.label).color(l.c("text-muted")).w_full().px(10.0).pt(16.0).pb(6.0)
}

/// Same nesting order for drawing and keyboard navigation; folded groups never enter it.
fn nested_order<'a>(list: &'a [SidebarMission], all: &'a [SidebarMission]) -> Vec<&'a SidebarMission> {
    rows_with_children(list, all).into_iter().map(|(m, _)| m).collect()
}

fn drawn_missions(s: &State) -> Vec<SidebarMission> {
    // An error replaces the rows on screen too.
    if s.recent_error.is_some() { return Vec::new(); }
    let missions = sidebar_missions_now(s);
    let (waiting, pinned, recent) = needs_you_pinned_and_recent(&missions, &s.org.pins);
    let mut out: Vec<_> = nested_order(&waiting, &missions).into_iter().cloned().collect();
    out.extend(nested_order(&pinned, &missions).into_iter().cloned());
    let groups_shown = !s.org.groups.is_empty() && !s.org.groups_unreadable;
    if groups_shown {
        for group in &s.org.groups {
            if s.folded_groups.contains(&group.group_id) { continue; }
            let members: Vec<_> = recent.iter().filter(|m| held_for(m, &s.org.members).is_some_and(|mem| mem.group_id == group.group_id)).cloned().collect();
            out.extend(nested_order(&members, &missions).into_iter().cloned());
        }
    }
    let rest: Vec<_> = recent.into_iter().filter(|m| !groups_shown || held_for(m, &s.org.members).is_none()).collect();
    out.extend(nested_order(&rest, &missions).into_iter().cloned());
    out
}

pub(super) fn step_from_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    let accel = if cfg!(target_os = "macos") { event.mods.cmd } else { event.mods.ctrl };
    if !event.mods.alt || accel || !matches!(event.key, Key::Up | Key::Down) { return false; }
    let rows = drawn_missions(s);
    let shown = selected_conversation(s).and_then(|(_, c)| c.conversation_id.clone())
        .or_else(|| s.ledger.as_ref().map(|p| p.snapshot.tip.clone()));
    let shown = rows.iter().find(|r| {
        if let Some(active) = &s.compare.active_id {
            undecided_comparison(s, r).is_some_and(|compare| &compare.compare_id == active)
        } else {
            shown.as_deref().is_some_and(|id| conversation_keys(r).iter().any(|key| key == id))
        }
    }).map(|r| r.mission_id.as_str());
    let ids: Vec<_> = rows.iter().map(|r|r.mission_id.clone()).collect();
    let next = crate::conversation_list::step_conversation(&ids, shown, event.key == Key::Down, |id| {
        !event.mods.shift || rows.iter().find(|r| r.mission_id == id).is_some_and(|r| conversation_keys(r).iter().any(|key| s.home.waiting_for.contains_key(key)))
    });
    if let Some(row) = next.and_then(|id| rows.iter().find(|r| r.mission_id == id)) {
        open_sidebar_mission(s, row, tx);
    }
    true
}

/// How many rows a page of the sidebar's long conversation list draws (ListWindow.tsx `LIST_PAGE`).
pub(super) const LIST_PAGE: usize = 80;

/// How many ungrouped rows to draw: at least `budget` (one page at a time), and always far enough to hold the one open
/// (`Sidebar.tsx`: `Math.max(listBudget, openAt + 1)`), capped at `total`.
pub(super) fn list_window_count(total: usize, budget: usize, open_at: Option<usize>) -> usize {
    let want = budget.max(open_at.map_or(0, |i| i + 1));
    total.min(want)
}

/// The footer button's words for the rows not drawn yet (ListWindow.tsx `ListMore`).
pub(super) fn list_more_label(remaining: usize) -> String {
    if remaining == 1 {
        "1 more conversation".to_owned()
    } else {
        format!("{remaining} more conversations")
    }
}

/// Sidebar.tsx: a search that matches nothing says so, rather than emptying into what reads as "you have none". The
/// noun is the list's own: it holds conversations, which the Missions screen does not filter.
const NO_MATCH: &str = "No conversations match that.";

pub(super) fn sidebar(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    // The sidebar's places, at the top, one to a line (`.lc-sidebar__places`): Conversations, Rooms and Routines are
    // reached here whatever else the sidebar draws; the roster is reached from the faces row's Team.
    let nav = |icon: &'static rune::Icon, words: &str| {
        row()
            .w_full()
            .gap(10.0)
            .py(7.0)
            .px(10.0)
            .center_y()
            .radius(8.0)
            .hover(l.c("hover-surface"))
            .child(crate::look::controls::glyph(&l, icon, 14.0, Some(l.c("text-muted"))))
            .child(text(words.to_owned()).font(rune::TextStyle::new(13.5, 400.0)).color(l.c("text-secondary")))
            .role("button")
            .label(words.to_owned())
    };
    let mut places = col().key("sidebar-places").w_full().px(10.0).pb(6.0).gap(1.0)
        .child(nav(&crate::look::icons::INBOX, "Conversations").key("sidebar-place-conversations").on_press(crate::missions_screen::open))
        .child(nav(&crate::look::icons::USERS, "Rooms").key("sidebar-place-rooms").on_press(crate::room_screen::open_list))
        .child(nav(&crate::look::icons::CLOCK, "Routines").key("sidebar-place-routines").on_press(crate::routines_panel::open));
    let cloud_count = crate::cloud_tasks::sidebar_count(s);
    if cloud_count > 0 {
        places = places.child(
            nav(&crate::look::icons::CLOUD, "Cloud tasks")
                .key("sidebar-place-cloud-tasks")
                .child(rune::spacer())
                .child(
                    text(cloud_count.to_string())
                        .key("sidebar-cloud-tasks-count")
                        .font(l.ty.mono)
                        .color(l.c("text-muted")),
                )
                .on_press(crate::cloud_tasks::open_from_sidebar),
        );
    }
    if s.compare.compares.iter().any(|c| !c.reruns.is_empty()) {
        places = places.child(nav(&crate::look::icons::TARGET, "Benchmarks").on_press(crate::benchmarks_panel::open));
    }
    let missions = sidebar_missions_now(s);
    let all_rows: Vec<SidebarMission> = recent_as_missions(s).into_iter().filter(|m| comparison_row_visible(s, m)).collect();
    let all_count = all_rows.len();
    let owned: Vec<(Option<String>, Option<String>)> = all_rows.iter().map(|m| (row_owner(s, m), m.last_at.clone())).collect();
    let (waiting, pinned, recent_rows) = needs_you_pinned_and_recent(&missions, &s.org.pins);
    let show_sections = !s.org.pins.is_empty();
    let groups_shown = !s.org.groups.is_empty() && !s.org.groups_unreadable;

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(2.0).child(
        row()
            .w_full()
            .center_y()
            .pl(10.0)
            .pt(14.0)
            .pb(4.0)
            .child(crate::look::controls::label(&l, "Recent").grow(1.0))
            .child(
                crate::look::controls::head_icon(&crate::look::icons::REFRESH, "Refresh", "Refresh the list")
                    .control_size(rune_kit::ControlSize::Small)
                    .on_press(refresh_recent)
                    .build(l.k)
                    .key("refresh-recent"),
            ),
    );

    if let Some(menu) = source_menu(s, &l) {
        recent = recent.child(menu);
    }

    // A source narrowing says which, before the rest of the line or the count (Sidebar.tsx `narrowing`): one sentence
    // for the face and the search together, with the count, and one Clear for both.
    let filtered_to = s.sidebar_face_filter.as_deref().and_then(|id| s.teammates.iter().find(|mate| mate.id == id));
    let in_face = owned.iter().filter(|(owner, _)| owner.is_some() && owner.as_deref() == s.sidebar_face_filter.as_deref()).count();
    let query_line = narrowing_line(filtered_to.map(|mate| mate.name.as_str()), &s.sidebar_query.text, missions.len(), in_face, all_count);
    let narrowing = match s.sidebar_source {
        crate::conversation_list::ConversationSource::All => query_line,
        source => Some(format!(
            "{} · {}",
            if source == crate::conversation_list::ConversationSource::App { "Via apps" } else { "Started here" },
            query_line.unwrap_or_else(|| format!("{} of {} conversations", missions.len(), all_count)),
        )),
    };
    let clear = narrowing.map(|line| {
        // `.lc-faces__clear`: the sentence on the left, the way out on the right.
        row().key("sidebar-faces-clear").mx(10.0).mb(8.0).px(10.0).py(4.0).gap(10.0).center_y().radius(8.0)
            .fill(l.c("hover-surface"))
            .child(text(line).font(type_::LABEL).color(l.c("text-secondary")).grow(1.0).shrink().min_w(0.0).truncate()
                .key("sidebar-faces-clearsaid"))
            .child(text("Clear").font(type_::LABEL).color(l.c("text-muted")).role("button").label("Clear")
                .key("sidebar-faces-clearlink")
                .on_press(|s: &mut State| {
                    s.sidebar_face_filter = None;
                    s.sidebar_query.clear();
                }))
    });

    // Records that are not whole say so, in Screens.tsx's words: a conversation kept up to a bad record, a file that
    // could not be read at all, a folder that could not be listed.
    if let Some(words) = s.recent_damage.words() {
        recent = recent.child(
            text(words)
                .font(type_::LABEL)
                .color(p.amber)
                .w_full()
                .px(12.0)
                .wrap()
                .key("sidebar-records-damage"),
        );
    }

    // A history longer than the list says how much of it is listed (Screens.tsx).
    if let Some(words) = locust_core::native::listed_words(s.recent.len(), s.recent_total) {
        recent = recent.child(
            text(words)
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap()
                .key("sidebar-listed-of"),
        );
    }

    let unreadable: Vec<_> = [
        (s.org.groups_unreadable, "groups"),
        (s.org.teammates_org_unreadable, "teammates"),
        (s.org.routines_unreadable, "routines"),
    ]
    .into_iter()
    .filter_map(|(bad, name)| bad.then_some(name))
    .collect();
    if let Some((happened, safe)) = crate::conversation_list::unreadable_sentence(&unreadable) {
        recent = recent
            .child(
                text(happened)
                    .font(type_::LABEL)
                    .color(p.amber)
                    .w_full()
                    .px(12.0)
                    .wrap(),
            )
            .child(
                text(safe)
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .px(12.0)
                    .wrap(),
            );
    }

    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone())
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap_anywhere(),
        );
    } else if missions.is_empty() && s.recent.is_empty() && !groups_shown {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else if !s.sidebar_query.text.trim().is_empty() {
                NO_MATCH
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .px(12.0)
            .wrap(),
        );
    } else if missions.is_empty() {
        recent = recent.child(
            text(if s.sidebar_query.text.trim().is_empty() { "Nothing in here yet." } else { NO_MATCH })
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap(),
        );
    } else {
        let is_picked = |item: &SidebarMission| {
            selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.mission_id.as_str())
                || s.ledger.as_ref().is_some_and(|p| {
                    !p.snapshot.tip.is_empty() && p.snapshot.tip == item.mission_id
                })
        };
        let append_rows = |mut container: El, list: &[SidebarMission]| -> El {
            for (item, nested) in rows_with_children(list, &missions) {
                if nested {
                    container = container.child(col().key(format!("nested-{}", item.mission_id)).w_full().pl(16.0).child(mission_row(s, p, item, is_picked(item))));
                    continue;
                }
                container = container.child(mission_row(s, p, item, is_picked(item)));
            }
            container
        };
        // NEEDS YOU, above the pins (Sidebar.tsx `lc-project__name lc-tone-amber`), whenever anything waits.
        if !waiting.is_empty() {
            recent = recent.child(section_label(p, "Needs you").color(p.amber).key("sidebar-needs-you"));
            recent = append_rows(recent, &waiting);
        }
        if show_sections && !pinned.is_empty() {
            recent = recent.child(section_name(p, "Pinned"));
            recent = append_rows(recent, &pinned);
        }
        // Project groups
        if groups_shown {
            for group in &s.org.groups {
                let gid = group.group_id.clone();
                let gname = group.name.clone();
                let rename_key = format!("@rename-group:{gid}");
                let renaming_here = s.rename_id.as_deref() == Some(rename_key.as_str());
                let folded = s.folded_groups.contains(&gid);
                if renaming_here {
                    let save_gid = gid.clone();
                    recent = recent.child(
                        col()
                            .w_full()
                            .px(10.0)
                            .pt(12.0)
                            .pb(4.0)
                            .gap(6.0)
                            .child(
                                text_input(&s.rename_draft, type_::BODY)
                                    .key(format!("group-rename-input-{gid}"))
                                    .w_full()
                                    .single_line()
                                    .color(p.text)
                                    .placeholder("Project name", p.muted)
                                    .padding(6.0)
                                    .fill(p.field)
                                    .on_edit(|s: &mut State| &mut s.rename_draft),
                            )
                            .child(
                                row()
                                    .gap(8.0)
                                    .child(
                                        text("Save")
                                            .key(format!("group-rename-save-{gid}"))
                                            .font(type_::LABEL)
                                            .color(p.secondary)
                                            .on_press(move |s: &mut State| {
                                                if let Some(store) = s.data.clone() {
                                                    let title = s.rename_draft.text.clone();
                                                    if !title.trim().is_empty()
                                                        && let Err(words) = org::rename_group(
                                                            &store,
                                                            &mut s.org,
                                                            &save_gid,
                                                            &title,
                                                        )
                                                    {
                                                        s.status = words;
                                                        s.status_tone = StatusTone::Amber;
                                                    }
                                                }
                                                s.rename_id = None;
                                                s.rename_draft.clear();
                                            }),
                                    )
                                    .child(
                                        text("Cancel")
                                            .font(type_::LABEL)
                                            .color(p.muted)
                                            .on_press(|s: &mut State| {
                                                s.rename_id = None;
                                                s.rename_draft.clear();
                                            }),
                                    ),
                            ),
                    );
                } else {
                    let fold_gid = gid.clone();
                    let menu_gid = gid.clone();
                    let chevron = if folded {
                        &crate::look::icons::CHEVRON_RIGHT
                    } else {
                        &crate::look::icons::CHEVRON_DOWN
                    };
                    recent = recent.child(
                        row()
                            .w_full()
                            .center_y()
                            .px(10.0)
                            .pt(14.0)
                            .pb(4.0)
                            .gap(6.0)
                            .child(
                                row()
                                    .key(format!("group-head-{gid}"))
                                    .grow(1.0)
                                    .center_y()
                                    .gap(6.0)
                                    .role("button")
                                    .on_press(move |s: &mut State| {
                                        if !s.folded_groups.remove(&fold_gid) {
                                            s.folded_groups.insert(fold_gid.clone());
                                        }
                                    })
                                    .child(crate::look::controls::glyph(
                                        &l,
                                        chevron,
                                        11.0,
                                        Some(l.c("text-muted")),
                                    ))
                                    .child(text(gname.clone()).font(l.ty.label).color(l.c("text-muted")).grow(1.0)),
                            )
                            .child(
                                text("…")
                                    .key(format!("group-menu-{gid}"))
                                    .font(type_::LABEL)
                                    .color(p.muted)
                                    .px(6.0)
                                    .py(2.0)
                                    .radius(4.0)
                                    .hover(l.c("hover-surface"))
                                    .role("button")
                                    .on_press(move |s: &mut State| {
                                        s.group_remove_confirm = false;
                                        s.group_menu_id =
                                            if s.group_menu_id.as_deref() == Some(menu_gid.as_str())
                                            {
                                                None
                                            } else {
                                                Some(menu_gid.clone())
                                            };
                                    }),
                            ),
                    );
                }
                if s.group_menu_id.as_deref() == Some(gid.as_str()) {
                    let settings_gid = gid.clone();
                    let ren_gid = gid.clone();
                    let ren_name = gname.clone();
                    let rem_gid = gid.clone();
                    let remove_label = if s.group_remove_confirm {
                        "Remove for good?"
                    } else {
                        "Remove project"
                    };
                    recent = recent.child(
                        col()
                            .key(format!("group-menu-panel-{gid}"))
                            .w_full()
                            .mx(10.0)
                            .padding(8.0)
                            .gap(6.0)
                            .radius(8.0)
                            .fill(p.raised)
                            .border(1.0, p.hairline)
                            .child(
                                text("Project settings…")
                                    .key(format!("group-settings-{gid}"))
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .on_press(move |s: &mut State| {
                                        s.group_menu_id = None;
                                        s.group_remove_confirm = false;
                                        crate::group_settings_dialog::open(s, &settings_gid);
                                    }),
                            )
                            .child(
                                text("Rename")
                                    .key(format!("group-rename-{gid}"))
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .on_press(move |s: &mut State| {
                                        s.group_menu_id = None;
                                        s.group_remove_confirm = false;
                                        s.rename_id = Some(format!("@rename-group:{ren_gid}"));
                                        s.rename_draft.replace_text(&ren_name);
                                        s.rename_draft.focused = true;
                                    }),
                            )
                            .child(
                                text(remove_label)
                                    .key(format!("group-remove-{gid}"))
                                    .font(type_::LABEL)
                                    .color(p.red)
                                    .on_press(move |s: &mut State| {
                                        if !s.group_remove_confirm {
                                            s.group_remove_confirm = true;
                                            return;
                                        }
                                        if let Some(store) = s.data.clone()
                                            && let Err(words) =
                                                org::remove_group(&store, &mut s.org, &rem_gid)
                                        {
                                            s.status = words;
                                            s.status_tone = StatusTone::Amber;
                                        }
                                        s.group_menu_id = None;
                                        s.group_remove_confirm = false;
                                    }),
                            ),
                    );
                }
                let in_group: Vec<SidebarMission> = recent_rows
                    .iter()
                    .filter(|m| {
                        held_for(m, &s.org.members).is_some_and(|mem| mem.group_id == group.group_id)
                    })
                    .cloned()
                    .collect();
                if !folded {
                    if in_group.is_empty() {
                        recent = recent.child(
                            text("Nothing in here yet.")
                                .font(type_::LABEL)
                                .color(p.muted)
                                .w_full()
                                .px(14.0)
                                .py(4.0),
                        );
                    } else {
                        recent = append_rows(recent, &in_group);
                    }
                }
            }
        }
        let rest = if groups_shown {
            recent_rows
                .iter()
                .filter(|m| held_for(m, &s.org.members).is_none())
                .cloned()
                .collect::<Vec<_>>()
        } else {
            recent_rows.clone()
        };
        // Each room is one row among them, where its newest answer was (conversationList.ts `withRoomsFolded`).
        let entries = with_rooms_folded(&rest, &s.rooms.rooms);
        if !entries.is_empty() && (show_sections || groups_shown) {
            recent = recent.child(section_name(
                p,
                if groups_shown { "Ungrouped" } else { "Recents" },
            ));
        }
        let open_at = entries.iter().position(|entry| match entry {
            ListEntry::Conversation(item) => is_picked(item),
            ListEntry::Room { answers, .. } => answers.iter().any(|item| is_picked(item)),
        });
        let budget = s.sidebar_list_budget.max(LIST_PAGE);
        let drawn = list_window_count(entries.len(), budget, open_at);
        let not_drawn = entries.len().saturating_sub(drawn);
        for entry in &entries[..drawn] {
            recent = match entry {
                ListEntry::Conversation(item) => append_rows(recent, std::slice::from_ref(*item)),
                ListEntry::Room { room, answers } => recent.child(room_row(s, p, room, answers)),
            };
        }
        if not_drawn > 0 {
            recent = recent.child(
                row()
                    .key("sidebar-list-more")
                    .role("button")
                    .w_full()
                    .py(8.0)
                    .px(10.0)
                    .radius(8.0)
                    .hover(l.c("hover-surface"))
                    .child(
                        text(list_more_label(not_drawn))
                            .font(rune::TextStyle::new(11.5, 400.0).lh(15.0))
                            .color(l.c("text-muted")),
                    )
                    .on_press(|s: &mut State| {
                        s.sidebar_list_budget = s.sidebar_list_budget.max(LIST_PAGE) + LIST_PAGE;
                    }),
            );
        }
    }

    if let Some(id) = &s.rename_id {
        let id = id.clone();
        recent = recent.child(
            col()
                .w_full()
                .px(12.0)
                .gap(6.0)
                .child(
                    text(if id.starts_with("@new-project:") {
                        "Name this project"
                    } else {
                        "Name this conversation"
                    })
                    .font(type_::LABEL)
                    .color(p.secondary),
                )
                .child(
                    text_input(&s.rename_draft, type_::BODY)
                        .key("rename-draft")
                        .w_full()
                        .color(p.text)
                        .placeholder(
                            if id.starts_with("@new-project:") {
                                "Name this project"
                            } else {
                                "Name this conversation"
                            },
                            p.muted,
                        )
                        .padding(8.0)
                        .fill(p.field)
                        .on_edit(|s: &mut State| &mut s.rename_draft),
                )
                .child(
                    text("Save")
                        .key("rename-save")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(move |s: &mut State| {
                            if let Some(store) = s.data.clone() {
                                let title = s.rename_draft.text.clone();
                                if let Err(words) = if let Some(mission) =
                                    id.strip_prefix("@new-project:")
                                {
                                    org::create_group(&store, &mut s.org, &title).and_then(|gid| {
                                        if mission.is_empty() {
                                            Ok(())
                                        } else {
                                            org::assign_to_group(
                                                &store,
                                                &mut s.org,
                                                mission,
                                                Some(&gid),
                                            )
                                        }
                                    })
                                } else {
                                    {
                                        let keys = recent_as_missions(s)
                                            .iter()
                                            .find(|m| conversation_keys(m).contains(&id))
                                            .map(conversation_keys)
                                            .unwrap_or_else(|| vec![id.clone()]);
                                        org::rename_conversation(
                                            &store, &mut s.org, &id, &keys, &title,
                                        )
                                    }
                                } {
                                    s.status = words;
                                    s.status_tone = StatusTone::Amber;
                                }
                            }
                            s.rename_id = None;
                            s.rename_draft.clear();
                        }),
                ),
        );
    }

    // `.lc-search`: 32 high, radius 8, the raised fill and a hairline, the magnifier before the words.
    let search = row().w_full().px(10.0).pb(8.0).child(
        row()
            .w_full()
            .h(32.0)
            .gap(9.0)
            .px(10.0)
            .center_y()
            .radius(8.0)
            .fill(l.c("bg-raised-2"))
            .border(1.0, l.c("border-hairline"))
            .child(crate::look::controls::glyph(&l, &crate::look::icons::SEARCH, 14.0, Some(l.c("text-muted"))))
            .child(
                text_input(&s.sidebar_query, rune::TextStyle::new(13.0, 400.0))
                    .key("sidebar-search")
                    .single_line()
                    .grow(1.0)
                    .color(l.c("text-primary"))
                    .placeholder("Search conversations", l.c("text-muted"))
                    .on_edit(|s: &mut State| &mut s.sidebar_query),
            ),
    );

    col()
        .key("sidebar")
        .w(crate::window_glass::sidebar_width(s))
        .h_full()
        .pl(crate::window_glass::sidebar_inset(s))
        .fill(crate::window_glass::sidebar_ground(s, p))
        .border_sides([0.0, 1.0, 0.0, 0.0], crate::window_glass::edge(s, p.hairline))
        .child(
            // `.lc-sidebar__brand`: under the title bar now, which holds the window's lights.
            col()
                .w_full()
                .pt(18.0)
                .px(16.0)
                .pb(12.0)
                // The lockup is the way home and the `+` starts a conversation, side by side (`.lc-sidebar__brand`).
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(12.0)
                        .child(
                            row()
                                .key("sidebar-home-button")
                                .grow(1.0)
                                .child(crate::look::controls::brand_row(&l))
                                .role("button")
                                .label("Home")
                                .help("Home")
                                .on_press(crate::rail::go_home),
                        )
                        .child(
                            el().key("sidebar-add-button")
                                .on_layout(|s: &mut State, r| {
                                    if s.add_menu_at != r {
                                        s.add_menu_at = r;
                                    }
                                })
                                .child(
                                    crate::look::controls::head_icon(&crate::look::icons::PLUS, "Add", "Add")
                                        .on_press(|s: &mut State| {
                                            crate::context_menu::toggle_add_menu(s, "sidebar-add-button", s.add_menu_at);
                                        })
                                        .build(l.k)
                                        .expanded(crate::context_menu::is_anchored_to(s, "sidebar-add-button")),
                                ),
                        ),
                ),
        )
        .child(search)
        .child(places)
        .children(faces_strip(s, p, &l, &owned))
        .children(clear)
        .child(
            col()
                .key("team-list")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(col().w_full().child(recent)),
        )
        .child(footer(&l))
}

/// The roster, one row tall (`.lc-faces`), in the wide sidebar only: the faces being worked with, at most five, else
/// four and a count of the rest, then Team at the row's end. Never scrolled sideways.
fn faces_strip(s: &State, p: &Palette, l: &crate::look::controls::Look, owned: &[(Option<String>, Option<String>)]) -> Option<El> {
    if s.teammates.is_empty() {
        return None;
    }
    let order = faces_by_recency(&s.teammates, owned);
    let shown = shown_face_count(order.len());
    let rest = order.len() - shown;
    let views = crate::teammate_faces::views(s);
    // The strip is a row: a handoff between two shown faces turns them toward each other along it (Sidebar.tsx
    // `stripGlances = glancesAmong(shownFaces, handoffs, 'row')`).
    let line: Vec<String> = order.iter().take(shown).map(|mate| mate.id.clone()).collect();
    let handoffs: Vec<locust_core::bot_rig::Handoff<'_>> = s.handoff_glances.held.iter()
        .map(|h| locust_core::bot_rig::Handoff { key: &h.key, from: &h.from, to: &h.to })
        .collect();
    let glances = locust_core::bot_rig::glances_among(&line, &handoffs, locust_core::bot_rig::GlanceAxis::Row);
    let on_id = s.ledger.as_ref().map(|pane| pane.header.id.clone()).or_else(|| s.teammates.get(s.selected).map(|mate| mate.id.clone()));
    let mut strip = row().key("sidebar-faces").w_full().px(10.0).pt(2.0).pb(10.0).gap(FACES_GAP).center_y();
    for teammate in order.into_iter().take(shown) {
        let on = on_id.as_deref() == Some(teammate.id.as_str());
        let filtering = s.sidebar_face_filter.as_deref() == Some(teammate.id.as_str());
        let view = views.get(&teammate.id);
        let (id, hover_id) = (teammate.id.clone(), teammate.id.clone());
        let mut face = el()
            .key(format!("teammate-{}", teammate.id))
            .padding(1.0)
            .radius(8.0)
            .border(if filtering { 1.5 } else { 1.0 }, if on || filtering { l.c("focus-edge") } else { Color::TRANSPARENT })
            .fill(if on { l.c("bg-selected") } else { crate::window_glass::sidebar_row_ground(s, p) })
            .hover(l.c("hover-surface"))
            .child(roster_face(s, "roster", teammate, view, glances.get(&teammate.id).copied(), STRIP_FACE, l));
        if let Some(view) = view {
            face = face.child(el().absolute(STRIP_FACE - 4.0, STRIP_FACE - 4.0).size(6.0, 6.0).radius(3.0)
                .fill(crate::teammate_faces::tone_color(l, view.tone)).a11y_hidden());
        }
        let mut face = face.role("button").label(format!("{} — open their conversation", teammate.name));
        if let Some(view) = view {
            face = face.description(locust_core::face_status::state_in_words(view));
        }
        // Pointing at a face opens its card, where the list filter is (RailFlyout.tsx `onFilter`); pressing it opens
        // the teammate's conversation.
        strip = strip.child(face
            .on_hover_change(move |s: &mut State, over, _| {
                if over {
                    s.rail_over_face = Some(hover_id.clone());
                    if !s.rail_pinned {
                        s.rail_open = Some(hover_id.clone());
                    }
                } else if s.rail_over_face.as_deref() == Some(hover_id.as_str()) {
                    s.rail_over_face = None;
                    s.rail_left_at = Some(std::time::Instant::now());
                }
            })
            .on_press_tx(move |s: &mut State, tx| select_teammate(s, &id, tx)));
    }
    if rest > 0 {
        strip = strip.child(
            row().key("sidebar-faces-more").h(26.0).px(8.0).center().radius(8.0).border(1.0, l.c("border-hairline"))
                .hover(l.c("hover-surface"))
                .child(text(format!("+{rest}")).font(rune::TextStyle::new(11.5, 400.0).mono()).color(l.c("text-muted")))
                .role("button")
                .label(format!("{rest} more on the team — open the roster"))
                .help(format!("{rest} more on the team — open the roster"))
                .on_press(crate::people_panel::open),
        );
    }
    // `.lc-faces__team`: pushed to the end, so however many faces there are the way to the roster does not move.
    strip = strip.child(el().grow(1.0)).child(
        row().key("sidebar-faces-team").py(4.0).px(6.0).gap(5.0).center_y().radius(8.0).hover(l.c("hover-surface"))
            .child(crate::look::controls::glyph(l, &crate::look::icons::USERS, 13.0, Some(l.c("text-muted"))))
            .child(text("Team").font(rune::TextStyle::new(11.5, 400.0)).color(l.c("text-muted")))
            .role("button")
            .label("Team")
            .on_press(crate::people_panel::open),
    );
    Some(strip)
}

/// A face in the strip: Electron's 32 in its 268 column; 26 in the Rust window's 248, so five faces, the gaps and Team
/// end inside the column as Electron's measured five did.
pub(super) const STRIP_FACE: f32 = 26.0;
const FACES_GAP: f32 = 4.0;

/// `.lc-sidebar__footer`: Settings and Board in equal cells under a hairline, as Electron's wide sidebar ends.
fn footer(l: &crate::look::controls::Look) -> El {
    let cell = |icon: &'static rune::Icon, words: &'static str, press: fn(&mut State)| {
        row()
            .w(0.0)
            .grow(1.0)
            .py(7.0)
            .gap(7.0)
            .center()
            .radius(7.0)
            .hover(l.c("hover-surface"))
            .child(crate::look::controls::glyph(l, icon, 14.0, Some(l.c("text-secondary"))))
            .child(text(words).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")))
            .role("button")
            .label(words)
            .key(format!("sidebar-footer-{}", words.to_lowercase()))
            .on_press(press)
    };
    row()
        .key("sidebar-footer")
        .w_full()
        .gap(2.0)
        .pt(8.0)
        .px(10.0)
        .pb(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
        .child(cell(&crate::look::icons::SETTINGS, "Settings", settings_panel::open))
        .child(cell(&crate::look::icons::COLUMNS, "Board", home_panel::open_board))
}

#[cfg(test)]
#[path = "long_history_line_tests.rs"]
mod long_history_line_tests;

#[cfg(test)]
#[path = "sidebar_search_words_tests.rs"]
mod sidebar_search_words_tests;

#[cfg(test)]
#[path = "torn_ledger_line_tests.rs"]
mod torn_ledger_line_tests;

#[cfg(test)]
#[path = "teammate_bot_in_window_tests.rs"]
mod teammate_bot_in_window_tests;

#[cfg(test)]
mod review_tests {
    use super::*;
    // a-long-conversation-is-still-one-conversation.test.ts, exact chain values.
    fn long_chain(length: usize) -> State {
        let mut s = State::default();
        for index in 0..length {
            let id = format!("mission_{index:03}");
            let prompt = if index == 0 { "look into locust for me".into() } else { format!("turn {index}") };
            let at = format!("2026-09-21T00:{index:02}:00.000Z");
            s.recent.push(ConversationRow { id: id.clone(), title: prompt.clone(), teammate: String::new(), started_by: "native".into(), state: "completed".into(), updated: at.clone() });
            s.ledger_summaries.insert(id.clone(), ledger::Summary {
                mission_id: id, title: prompt.clone(), words: prompt, runtime: "antigravity".into(),
                parent: (index > 0).then(|| format!("mission_{:03}", index - 1)),
                created_at: at.clone(), updated: at, ..Default::default()
            });
        }
        s
    }
    #[test]
    fn finds_the_real_root_36_turns_back() {
        // Each prefix puts that turn at the tip, just as rootMission does for every turn.
        for length in 1..=36 {
            let rows = recent_as_missions(&long_chain(length));
            assert!(!rows.is_empty(), "length {length}");
            assert!(rows.iter().all(|r| r.root_id.as_deref() == Some("mission_000")), "length {length}: {rows:?}");
        }
    }
    #[test]
    fn collapses_every_turn_of_it_into_one_sidebar_row() {
        let rows = recent_as_missions(&long_chain(36));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].member_ids.len(), 36);
        assert_eq!(rows[0].title, "look into locust for me");
    }
    #[test]
    fn does_not_care_how_long_the_conversation_gets() {
        for length in [33, 65, 200] {
            let rows = recent_as_missions(&long_chain(length));
            assert_eq!(rows.len(), 1, "length {length}");
            assert_eq!(rows[0].root_id.as_deref(), Some("mission_000"));
        }
    }
    #[test]
    fn still_stops_on_a_cycle_rather_than_spinning() {
        let mut s = long_chain(2);
        s.ledger_summaries.get_mut("mission_000").unwrap().parent = Some("mission_001".into());
        let rows = recent_as_missions(&s);
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|r| matches!(r.root_id.as_deref(), Some("mission_000" | "mission_001"))));
        let by = s.ledger_summaries.iter().map(|(id, m)| (id.clone(), ledger::Metadata { mission_id: id.clone(), prompt: m.words.clone(), parent: m.parent.clone(), continues_reason: Some("follow-up".into()), ..Default::default() })).collect::<HashMap<_,_>>();
        assert_eq!(locust_core::conversation_turns::typed_prompt(&by["mission_001"], &by), "turn 1");
    }
    #[test]
    fn stops_at_a_turn_that_is_missing_rather_than_guessing_past_it() {
        let mut s = long_chain(10);
        s.recent.drain(..4);
        s.ledger_summaries.retain(|id, _| id.as_str() >= "mission_004");
        let rows = recent_as_missions(&s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].root_id.as_deref(), Some("mission_004"));
    }

    fn chain() -> State {
        let mut s = State::default();
        s.demo = true;
        for (id, parent, words, time) in [
            ("ship", Some("signup"), "Good ship it", "03"),
            ("signup", Some("root"), "Check signup", "02"),
            ("root", None, "Login redirect", "01"),
            ("other", None, "Other", "00"),
        ] {
            let at = format!("2026-10-01T10:00:{time}Z");
            s.recent.push(ConversationRow {
                id: id.into(),
                title: words.into(),
                teammate: "Atlas".into(),
                started_by: "native".into(),
                state: "completed".into(),
                updated: at.clone(),
            });
            s.ledger_summaries.insert(
                id.into(),
                ledger::Summary {
                    mission_id: id.into(),
                    title: words.into(),
                    parent: parent.map(str::to_owned),
                    words: words.into(),
                    runtime: "codex".into(),
                    model: String::new(),
                    created_at: at.clone(),
                    updated: at,
                    running: false,
                    failed: false,
                    ..Default::default()
                },
            );
        }
        s
    }
    #[test]
    fn real_sidebar_rows_collapse_turns_keep_names_groups_and_search_every_prompt() {
        let mut s = chain();
        s.org.titles.insert("signup".into(), "Chosen name".into());
        s.org.members.insert(
            "root".into(),
            org::GroupMembership {
                group_id: "project".into(),
                at: None,
            },
        );
        let rows = recent_as_missions(&s);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].mission_id, "ship");
        assert_eq!(rows[0].root_id.as_deref(), Some("root"));
        assert_eq!(rows[0].member_ids, ["root", "signup", "ship"]);
        assert_eq!(rows[0].title, "Chosen name");
        assert_eq!(
            held_for(&rows[0], &s.org.members).map(|m| m.group_id.as_str()),
            Some("project")
        );
        s.sidebar_query.replace_text("signup");
        assert_eq!(sidebar_missions_now(&s).len(), 1);
        s.sidebar_query.replace_text("atlas");
        assert_eq!(sidebar_missions_now(&s).len(), 2);
        s.ledger_summaries.get_mut("root").unwrap().parent = Some("ship".into());
        assert!(!recent_as_missions(&s).is_empty(), "cycles terminate");
    }

    #[test]
    fn palette_and_find_keys_step_wrap_close_and_execute() {
        let mut s = chain();
        let mut tx = Tx::default();
        let event = |key, shift| KeyEvent {
            key,
            mods: rune::Mods {
                shift,
                ..Default::default()
            },
        };
        assert!(on_key(&mut s, &event(Key::Command('k'), false), &mut tx));
        assert!(s.palette_open);
        assert!(on_key(&mut s, &event(Key::Up, false), &mut tx));
        assert_eq!(s.palette_index, s.palette_actions.len() - 1);
        assert!(on_key(&mut s, &event(Key::Down, false), &mut tx));
        assert_eq!(s.palette_index, 0);
        assert!(on_key(&mut s, &event(Key::Enter, false), &mut tx));
        assert!(!s.palette_open);
        s.find_open = true;
        s.find_pieces = vec!["word word".into()];
        s.find_show_earlier = true;
        s.find_query.replace_text("word");
        on_key(&mut s, &event(Key::Enter, false), &mut tx);
        assert_eq!(s.find_index, 1);
        on_key(&mut s, &event(Key::Enter, true), &mut tx);
        assert_eq!(s.find_index, 0);
        on_key(&mut s, &event(Key::Escape, false), &mut tx);
        assert!(!s.find_open);
        assert!(s.find_query.text.is_empty());
    }

    #[test]
    fn relayed_conversations_nest_one_level_deep_under_their_origin() {
        let mut s = State::default();
        s.demo = true;
        s.workroom_messages.insert(
            "wm_relayed".into(),
            locust_core::workroom::Message {
                message_id: "wm_relayed".into(),
                from: locust_core::workroom::Party {
                    teammate_id: "tm_codec".into(),
                    name: "Codec".into(),
                },
                from_mission: "origin".into(),
                to: locust_core::workroom::Party {
                    teammate_id: "tm_pip".into(),
                    name: "Pip".into(),
                },
                text: "question".into(),
                posted_at: "2026-10-01T10:00:01Z".into(),
                sequence: 1,
                observed: None,
            },
        );
        for (id, words, time, relay, started_by_kind, peer_links) in [
            ("origin", "Start exchange", "01", None, Some("native".to_owned()), vec![]),
            (
                "relayed",
                "Pip's reply",
                "02",
                Some(locust_core::relay::RelayOrigin {
                    hop: 1,
                    root_mission_id: Some("origin".into()),
                    last_mission_of: vec![("tm_codec".into(), "origin".into())],
                    relayed_to: vec!["tm_pip".into()],
                    answering: vec!["wm_relayed".into()],
                }),
                Some("relay".to_owned()),
                vec![ledger::PeerLink {
                    direction: "received".into(),
                    message_id: "wm_relayed".into(),
                    peer_teammate_id: "tm_codec".into(),
                    occurred_at: "2026-10-01T10:00:01Z".into(),
                }],
            ),
            ("standalone", "Standalone", "03", None, Some("native".to_owned()), vec![]),
        ] {
            let at = format!("2026-10-01T10:00:{time}Z");
            s.recent.push(ConversationRow {
                id: id.into(),
                title: words.into(),
                teammate: "Codec".into(),
                started_by: started_by_kind.clone().unwrap_or_else(|| "native".into()),
                state: "completed".into(),
                updated: at.clone(),
            });
            s.ledger_summaries.insert(
                id.into(),
                ledger::Summary {
                    mission_id: id.into(),
                    title: words.into(),
                    parent: None,
                    words: words.into(),
                    runtime: "codex".into(),
                    model: String::new(),
                    created_at: at.clone(),
                    updated: at,
                    started_by_kind,
                    relay,
                    peer_links,
                    ..Default::default()
                },
            );
        }
        let rows = recent_as_missions(&s);
        let orig = rows.iter().find(|r| r.mission_id == "origin").unwrap();
        let rel = rows.iter().find(|r| r.mission_id == "relayed").unwrap();
        let lone = rows.iter().find(|r| r.mission_id == "standalone").unwrap();
        assert_eq!(orig.nested_under, None);
        assert_eq!(rel.nested_under.as_deref(), Some("origin"));
        assert_eq!(lone.nested_under, None);
    }

    // Ported from renderer/src/groups-unreadable-is-a-line.test.tsx (4 it cases)
    fn sidebar_text(s: &mut State) -> String {
        let taken = std::mem::take(s);
        let mut rt = rune::Runtime::new(
            taken,
            |s, _| sidebar(s, &Palette::new(true)),
            (280.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let out = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        *s = rt.state;
        out
    }

    #[test]
    fn groups_unreadable_shows_warning_hides_groups_and_recovers_on_reload() {
        let temp = std::env::temp_dir().join(format!("locust-sb-unreadable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let electron = temp.join("electron");
        let native = temp.join("native");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        let store = crate::store::Store::new(native, electron).unwrap();
        let gpath = store.groups_json_path().unwrap();
        std::fs::write(&gpath, "{broken").unwrap();

        let mut s = State::default();
        s.org = org::load_org(&store);
        s.data = Some(store.clone());
        s.recent.push(ConversationRow {
            id: "m_1".into(),
            title: "First chat".into(),
            teammate: "Codec".into(),
            started_by: "native".into(),
            state: "completed".into(),
            updated: "2026-10-01T10:00:00Z".into(),
        });
        assert!(s.org.groups_unreadable);

        let text = sidebar_text(&mut s);
        assert!(text.contains("Groups could not be read — everything is listed ungrouped."));
        assert!(text.contains("Nothing is saved over the file until it reads again."));
        assert!(text.contains("First chat"));

        // Writes are refused and broken file stays untouched
        assert!(org::create_group(&store, &mut s.org, "Recovered").is_err());
        assert_eq!(std::fs::read_to_string(&gpath).unwrap(), "{broken");

        // Recovering once file is valid clears the line
        std::fs::write(
            &gpath,
            serde_json::json!({
                "schemaVersion": 1,
                "groups": [{"groupId": "grp_ok", "name": "Recovered", "createdAt": "2026-10-01T10:00:00Z"}],
                "members": {"m_1": {"groupId": "grp_ok", "at": "2026-10-01T10:00:00Z"}}
            })
            .to_string(),
        )
        .unwrap();
        s.org = org::load_org(&store);
        assert!(!s.org.groups_unreadable);
        let recovered_text = sidebar_text(&mut s);
        assert!(!recovered_text.contains("Groups could not be read"));
        assert!(recovered_text.to_lowercase().contains("recovered"));
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn sidebar_folds_project_groups_and_seeds_composer_route_on_assign() {
        let temp = std::env::temp_dir().join(format!("locust-sb-fold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let electron = temp.join("electron");
        let native = temp.join("native");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        let store = crate::store::Store::new(native, electron).unwrap();

        let mut s = State::default();
        s.data = Some(store.clone());
        s.teammates.push(Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Custom".into(),
            model: Some("gpt-5".into()),
            runtime: Some("codex".into()),
            folder: None,
            runs_in: "ask".into(),
            account: None,
            busy: false,
        });
        s.conversations.insert(
            "tm_codec".into(),
            Conversation {
                conversation_id: Some("m_active".into()),
                turns: Vec::new(),
            },
        );
        s.recent.push(ConversationRow {
            id: "m_active".into(),
            title: "Build groups".into(),
            teammate: "Codec".into(),
            started_by: "native".into(),
            state: "completed".into(),
            updated: "2026-10-01T10:00:00Z".into(),
        });

        let gid = org::create_group(&store, &mut s.org, "Locust").unwrap();
        org::set_group_route(
            &store,
            &mut s.org,
            &gid,
            Some(org::GroupRoute {
                runtime: "claude".into(),
                model: "claude-sonnet".into(),
                mode: "plan".into(),
                account: None,
                effort: None,
            }),
        )
        .unwrap();

        apply_org_action(&mut s, "m_active", &format!("group:{gid}"));
        assert_eq!(s.picks.get("tm_codec").unwrap().model, "claude-sonnet");
        assert_eq!(s.composer_mode.as_deref(), Some("plan"));

        let open_text = sidebar_text(&mut s);
        assert!(open_text.contains("Build groups"));
        assert_eq!(drawn_missions(&s).iter().map(|m| m.mission_id.as_str()).collect::<Vec<_>>(), ["m_active"]);

        s.folded_groups.insert(gid.clone());
        let folded_text = sidebar_text(&mut s);
        assert!(!folded_text.contains("Build groups"));
        assert!(drawn_missions(&s).is_empty(), "keyboard navigation skips folded rows too");
        let _ = std::fs::remove_dir_all(&temp);
    }
}


#[cfg(test)]
mod renderer_model_mark_tests {
    use super::*;

    fn draw(id: &str, route: Option<(&str, &str)>) -> rune::Runtime<State> {
        let mut s = State::default();
        let at = "2026-10-02T05:00:00.000Z";
        s.recent.push(ConversationRow { id: id.into(), title: "yo".into(),
            teammate: String::new(), started_by: String::new(), state: "completed".into(), updated: at.into() });
        if let Some((runtime, model)) = route {
            s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id: id.into(), title: "yo".into(),
                runtime: runtime.into(), model: model.into(), updated: at.into(), created_at: at.into(), ..Default::default() });
        }
        let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)),
            (280.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    #[test]
    fn wears_its_models_mark_where_the_face_goes_named() {
        let rt = draw("mission_claude", Some(("claude", "claude-opus-5-5")));
        assert!(rt.find("conversation-runtime-mission_claude").is_some());
        assert!(rt.elements().iter().any(|e| e.label.as_deref() == Some("Claude Code")));
        assert!(rt.find("conversation-nobody-mission_claude").is_none());
    }

    #[test]
    fn keeps_the_empty_place_for_a_model_of_your_own_or_a_runtime_not_yet_known() {
        let rt = draw("mission_own", Some(("opencode", "own-0a1b2c3d/llama-local")));
        assert!(rt.find("conversation-nobody-mission_own").is_some());
        assert!(rt.find("conversation-runtime-mission_own").is_none());
        let rt = draw("mission_starting", None);
        assert!(rt.find("conversation-nobody-mission_starting").is_some());
        assert!(rt.find("conversation-runtime-mission_starting").is_none());
    }
}

#[cfg(test)]
mod renderer_step_wiring_tests {
    use super::*;
    #[test]
    fn are_wired_each_sidebar_row_carries_its_id_and_the_window_opens_the_step_it_finds() {
        let mut s = State::default();
        s.demo = true;
        s.teammates.push(Teammate { id:"tm_wren".into(),name:"Wren".into(),role:"Custom".into(),runtime:None,model:None,account:None,busy:false,runs_in:"ask".into(),folder:None });
        for (id, second) in [("a",4),("b",3),("c",2),("d",1)] {
            s.recent.push(ConversationRow { id:id.into(),title:id.into(),teammate:"Wren".into(),started_by:"native".into(),state:"completed".into(),updated:format!("2026-10-19T10:00:0{second}Z") });
        }
        let mut rt = rune::Runtime::new(s, |s,_| sidebar(s,&Palette::new(true)), (280.0,800.0),rune::Env::new(true,1.0));
        rt.frame(0.0);
        for id in ["a","b","c","d"] { assert!(rt.find(&format!("recent-{id}")).is_some(), "{id}"); }
        let s = &mut rt.state;
        let mut tx = Tx::default();
        let key = |shift| KeyEvent { key:Key::Down,mods:rune::Mods {alt:true,shift,..Default::default()} };
        assert!(on_key(s,&key(false),&mut tx));
        assert_eq!(s.conversations["tm_wren"].conversation_id.as_deref(),Some("a"));
        assert!(on_key(s,&key(false),&mut tx));
        assert_eq!(s.conversations["tm_wren"].conversation_id.as_deref(),Some("b"));
        s.home.waiting_for.insert("d".into(),"A decision".into());
        assert!(on_key(s,&key(true),&mut tx));
        assert_eq!(s.conversations["tm_wren"].conversation_id.as_deref(),Some("d"));
        assert!(!on_key(s,&KeyEvent { key:Key::Down,mods:rune::Mods {alt:true,cmd:cfg!(target_os="macos"),ctrl:!cfg!(target_os="macos"),..Default::default()} },&mut tx));
    }
}

#[cfg(test)]
#[path = "sidebar_needs_you_tests.rs"]
mod sidebar_needs_you_tests;

#[cfg(test)]
#[path = "sidebar_app_started_tests.rs"]
mod sidebar_app_started_tests;

#[cfg(test)]
#[path = "sidebar_list_window_tests.rs"]
mod sidebar_list_window_tests;

#[cfg(test)]
#[path = "sidebar_flat_tests.rs"]
mod sidebar_flat_tests;

#[path = "sidebar_rooms.rs"]
mod sidebar_rooms;
use sidebar_rooms::{ListEntry, room_row, with_rooms_folded};

#[cfg(test)]
#[path = "groups_unreadable_line_tests.rs"]
mod groups_unreadable_line_tests;

#[cfg(test)]
#[path = "sidebar_order_tests.rs"]
mod sidebar_order_tests;

#[cfg(test)]
#[path = "sidebar_pinned_tests.rs"]
mod sidebar_pinned_tests;

#[cfg(test)]
#[path = "sidebar_put_away_tests.rs"]
mod sidebar_put_away_tests;

#[cfg(test)]
#[path = "sidebar_routine_run_tests.rs"]
mod sidebar_routine_run_tests;

#[cfg(test)]
#[path = "name_and_group_kept_tests.rs"]
mod name_and_group_kept_tests;

#[cfg(test)]
#[path = "groups_hold_tests.rs"]
mod groups_hold_tests;

#[cfg(test)]
#[path = "scout_runs_kept_out_tests.rs"]
mod scout_runs_kept_out_tests;
