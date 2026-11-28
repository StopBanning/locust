//! room-exchange.test.ts through the real room post view (`room_screen::view` -> `view_post` -> `room_exchange_for`
//! -> `sequence_of_post` & `foot_line`). Electron's own fixtures (Wren and Gem's four-turn tabs/spaces argument),
//! expected words from `room-exchange.test.ts` and `RoomScreen.tsx`.
use super::*;
use locust_core::benchmark::RunCost;
use locust_core::ledger::{PeerLink, Summary};
use locust_core::native::RelayStarting;
use locust_core::room_exchange::LiveTurn;
use rune::{ElementInfo, Env, Runtime};

fn tm(id: &str, name: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Engineer".into(),
        runtime: Some("cursor".into()),
        model: Some("composer-2.5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

struct TurnSpec {
    mission_id: &'static str,
    owner: &'static str,
    at: &'static str,
    text: &'static str,
    phase: &'static str,
    hop: Option<usize>,
    peer_msgs: &'static [&'static str],
    cost: Option<RunCost>,
    live: Option<LiveTurn>,
}

fn argument_turns() -> Vec<TurnSpec> {
    vec![
        TurnSpec {
            mission_id: "m_wren1",
            owner: "tm_wren",
            at: "2026-09-11T10:00:01.000Z",
            text: "Tabs.",
            phase: "completed",
            hop: None,
            peer_msgs: &["wm_1"],
            cost: None,
            live: None,
        },
        TurnSpec {
            mission_id: "m_gem1",
            owner: "tm_gem",
            at: "2026-09-11T10:00:02.000Z",
            text: "Until a line wraps.",
            phase: "completed",
            hop: None,
            peer_msgs: &["wm_1", "wm_2"],
            cost: None,
            live: None,
        },
        TurnSpec {
            mission_id: "m_wren2",
            owner: "tm_wren",
            at: "2026-09-11T10:00:03.000Z",
            text: "Do not align past the indent.",
            phase: "completed",
            hop: Some(3),
            peer_msgs: &["wm_2", "wm_3"],
            cost: None,
            live: None,
        },
        TurnSpec {
            mission_id: "m_gem2",
            owner: "tm_gem",
            at: "2026-09-11T10:00:04.000Z",
            text: "Formatters emit spaces.",
            phase: "completed",
            hop: Some(4),
            peer_msgs: &["wm_3"],
            cost: None,
            live: None,
        },
    ]
}

fn base_post() -> RoomPost {
    RoomPost {
        post_id: "post_1".into(),
        text: "Tabs or spaces?".into(),
        at: "2026-09-11T10:00:00.000Z".into(),
        missions: BTreeMap::from([
            ("tm_wren".into(), "m_wren1".into()),
            ("tm_gem".into(), "m_gem1".into()),
        ]),
        queued: Vec::new(),
        refused: BTreeMap::new(),
        to: Vec::new(),
    }
}

fn room_state(posts: Vec<RoomPost>, turns: Vec<TurnSpec>, cap: u32, starting: Option<RelayStarting>) -> State {
    let mut s = State::default();
    s.teammates = vec![tm("tm_wren", "Wren"), tm("tm_gem", "Gem")];
    s.relay_hop_cap = cap;
    s.relay_live.starting = starting;
    let room = PublicRoom {
        room_id: "room_style".into(),
        name: "Style".into(),
        teammate_ids: vec!["tm_wren".into(), "tm_gem".into()],
        created_at: "2026-09-11T09:59:00.000Z".into(),
        posts,
        tasks: Vec::new(),
    };
    s.rooms.active = RoomView::Room(room.room_id.clone());
    s.rooms.rooms = vec![room];
    for t in turns {
        let running = t.phase == "running";
        if running {
            s.seen_live.push(t.mission_id.into());
        }
        s.mission_owners.insert(t.mission_id.into(), t.owner.into());
        s.ledger_summaries.insert(
            t.mission_id.into(),
            Summary {
                mission_id: t.mission_id.into(),
                runtime: "cursor".into(),
                model: "composer-2.5".into(),
                created_at: t.at.into(),
                running,
                started_by_hop: t.hop,
                cost: t.cost,
                peer_links: t
                    .peer_msgs
                    .iter()
                    .map(|msg| PeerLink {
                        message_id: (*msg).into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        );
        s.rooms.transcripts.insert(
            t.mission_id.into(),
            RoomTranscriptView {
                phase: t.phase.into(),
                answer_text: t.text.into(),
                note_error: None,
                started_at: t.at.into(),
                runtime: "cursor".into(),
                model: "composer-2.5".into(),
                live: t.live,
            },
        );
    }
    s
}

fn render(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1100.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn rect_of(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    shown(rt)
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .map(|e| e.rect)
        .unwrap_or_else(|| panic!("no element with key `{key}`"))
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    shown(rt)
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .and_then(|e| e.text)
}

fn label_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    shown(rt)
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .and_then(|e| e.label)
}

fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let [x, y, w, h] = rect_of(rt, key);
    shown(rt)
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x - 0.5 && cx <= x + w + 0.5 && cy >= y - 0.5 && cy <= y + h + 0.5)
                .then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// --- whether a post is a conversation at all

// case: is not, when every member answered once and nobody replied
#[test]
fn is_not_when_every_member_answered_once_and_nobody_replied() {
    let mut turns = argument_turns();
    turns.truncate(2);
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert!(rt.presented("room-exchange-post_1").is_none());
    assert!(rt.presented("room-exchange-foot-post_1").is_none());
    assert!(rt.presented("room-answer-post_1-tm_wren").is_some());
    assert!(rt.presented("room-answer-post_1-tm_gem").is_some());
}

// case: is, the moment one more run exists than the post started
#[test]
fn is_the_moment_one_more_run_exists_than_the_post_started() {
    let mut turns = argument_turns();
    turns.truncate(3);
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert!(rt.presented("room-exchange-post_1").is_some());
    assert!(rt.presented("room-exchange-foot-post_1").is_some());
    assert!(rt.presented("room-answer-post_1-tm_wren").is_none());
    assert!(rt.presented("room-answer-post_1-tm_gem").is_none());
}

// --- the sequence

// case: is every turn in the order it was said, first answers included
#[test]
fn is_every_turn_in_the_order_it_was_said_first_answers_included() {
    let rt = render(room_state(vec![base_post()], argument_turns(), 6, None));
    let keys = [
        ("room-exchange-turn-post_1-said_m_wren1", "room-exchange-face-post_1-said_m_wren1", "Wren", "Tabs."),
        ("room-exchange-turn-post_1-said_m_gem1", "room-exchange-face-post_1-said_m_gem1", "Gem", "Until a line wraps."),
        ("room-exchange-turn-post_1-said_m_wren2", "room-exchange-face-post_1-said_m_wren2", "Wren", "Do not align past the indent."),
        ("room-exchange-turn-post_1-said_m_gem2", "room-exchange-face-post_1-said_m_gem2", "Gem", "Formatters emit spaces."),
    ];
    let mut prev_y = f32::NEG_INFINITY;
    for (turn_key, face_key, who, words) in keys {
        let r = rect_of(&rt, turn_key);
        assert!(r[1] > prev_y, "{turn_key} should sit below previous turn: {r:?} vs {prev_y}");
        prev_y = r[1];
        assert_eq!(label_of(&rt, face_key).as_deref(), Some(who));
        assert!(text_in(&rt, turn_key).contains(words), "{}", text_in(&rt, turn_key));
    }
    let foot_r = rect_of(&rt, "room-exchange-foot-post_1");
    assert!(foot_r[1] > prev_y, "foot sits below the last turn");
}

// case: groups consecutive turns by one speaker under a single face
#[test]
fn groups_consecutive_turns_by_one_speaker_under_a_single_face() {
    let mut turns = argument_turns();
    turns[2].owner = "tm_gem";
    turns[2].text = "And another thing.";
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert_eq!(label_of(&rt, "room-exchange-face-post_1-said_m_wren1").as_deref(), Some("Wren"));
    assert_eq!(label_of(&rt, "room-exchange-face-post_1-said_m_gem1").as_deref(), Some("Gem"));
    assert!(rt.presented("room-exchange-face-post_1-said_m_wren2").is_none());
    assert!(rt.presented("room-exchange-face-post_1-said_m_gem2").is_none());
    assert!(text_in(&rt, "room-exchange-turn-post_1-said_m_wren2").contains("And another thing."));
}

// case: draws a finished turn that said nothing, because absence is information
#[test]
fn draws_a_finished_turn_that_said_nothing_because_absence_is_information() {
    let mut turns = argument_turns();
    turns[2].text = "";
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    let y_gem1 = rect_of(&rt, "room-exchange-turn-post_1-said_m_gem1")[1];
    let y_silent = rect_of(&rt, "room-exchange-turn-post_1-silent_m_wren2")[1];
    let y_gem2 = rect_of(&rt, "room-exchange-turn-post_1-said_m_gem2")[1];
    assert!(y_gem1 < y_silent && y_silent < y_gem2, "silent turn stays in place: {y_gem1} < {y_silent} < {y_gem2}");
    assert_eq!(
        text_in(&rt, "room-exchange-turn-post_1-silent_m_wren2"),
        "Wren\u{2019}s turn ended without a reply \u{2014} the runtime finished and wrote nothing back. Nothing was changed."
    );
}

// case: draws a run that is still going as the turn it is, never as a silence
#[test]
fn draws_a_run_that_is_still_going_as_the_turn_it_is_never_as_a_silence() {
    let mut turns = argument_turns();
    turns[3].text = "";
    turns[3].phase = "running";
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert!(rt.presented("room-exchange-turn-post_1-silent_m_gem2").is_none());
    assert!(!shown(&rt).iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("ended without a reply"))));
    assert!(rt.presented("room-exchange-turn-post_1-replying_m_gem2").is_some());
    assert_eq!(label_of(&rt, "room-exchange-face-post_1-replying_m_gem2").as_deref(), Some("Gem"));
}

// case: carries what a live turn is doing, so a room can draw the thread’s line
#[test]
fn carries_what_a_live_turn_is_doing_so_a_room_can_draw_the_threads_line() {
    let mut turns = argument_turns();
    turns[3].text = "";
    turns[3].phase = "running";
    turns[3].live = Some(LiveTurn {
        register: "tool".into(),
        label: Some("README.md".into()),
        action: None,
        detail: None,
        started_at: "2026-09-11T10:00:05.000Z".into(),
        thinking: false,
    });
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert_eq!(
        text_of(&rt, "room-exchange-live-post_1-replying_m_gem2").as_deref(),
        Some("Using a tool \u{b7} README.md")
    );
}

// case: has nothing to say a hop is doing before its run exists
#[test]
fn has_nothing_to_say_a_hop_is_doing_before_its_run_exists() {
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem2".into(),
        hop: 5,
    });
    let rt = render(room_state(vec![base_post()], argument_turns(), 6, starting));
    assert_eq!(
        text_of(&rt, "room-exchange-live-post_1-replying_tm_wren_m_gem2").as_deref(),
        Some("Starting\u{2026}")
    );
}

// case: lists a member waiting for a slot, so the room does not look finished
#[test]
fn lists_a_member_waiting_for_a_slot_so_the_room_does_not_look_finished() {
    let mut post = base_post();
    post.queued = vec!["tm_gem".into()];
    let rt = render(room_state(vec![post], argument_turns(), 6, None));
    let y_gem2 = rect_of(&rt, "room-exchange-turn-post_1-said_m_gem2")[1];
    let y_wait = rect_of(&rt, "room-exchange-turn-post_1-waiting_tm_gem")[1];
    assert!(y_wait > y_gem2);
    assert_eq!(
        text_in(&rt, "room-exchange-turn-post_1-waiting_tm_gem"),
        "Gem is finishing another mission. Their reply is queued."
    );
}

// --- a reply the host is still starting

// case: is a turn in the sequence, with no mission of its own yet
#[test]
fn is_a_turn_in_the_sequence_with_no_mission_of_its_own_yet() {
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem2".into(),
        hop: 5,
    });
    let rt = render(room_state(vec![base_post()], argument_turns(), 6, starting));
    let y_gem2 = rect_of(&rt, "room-exchange-turn-post_1-said_m_gem2")[1];
    let y_replying = rect_of(&rt, "room-exchange-turn-post_1-replying_tm_wren_m_gem2")[1];
    assert!(y_replying > y_gem2);
    assert_eq!(label_of(&rt, "room-exchange-face-post_1-replying_tm_wren_m_gem2").as_deref(), Some("Wren"));
}

// case: makes a post a conversation on its own, before the second run exists
#[test]
fn makes_a_post_a_conversation_on_its_own_before_the_second_run_exists() {
    let mut turns = argument_turns();
    turns.truncate(2);
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem1".into(),
        hop: 1,
    });
    let rt = render(room_state(vec![base_post()], turns, 6, starting));
    assert!(rt.presented("room-exchange-post_1").is_some());
    assert!(rt.presented("room-answer-post_1-tm_wren").is_none());
    assert_eq!(label_of(&rt, "room-exchange-face-post_1-replying_tm_wren_m_gem1").as_deref(), Some("Wren"));
}

// case: belongs to the post whose conversation it answers, and no other
#[test]
fn belongs_to_the_post_whose_conversation_it_answers_and_no_other() {
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_elsewhere".into(),
        hop: 5,
    });
    let rt = render(room_state(vec![base_post()], argument_turns(), 6, starting));
    assert!(rt.presented("room-exchange-post_1").is_some());
    let replying_nodes = shown(&rt)
        .into_iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("room-exchange-turn-post_1-replying_"))
        })
        .count();
    assert_eq!(replying_nodes, 0);
}

// case: is not drawn twice when the run it becomes has already been announced
#[test]
fn is_not_drawn_twice_when_the_run_it_becomes_has_already_been_announced() {
    let mut turns = argument_turns();
    turns[2].text = "";
    turns[2].phase = "running";
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem1".into(),
        hop: 3,
    });
    let rt = render(room_state(vec![base_post()], turns, 6, starting));
    let replying_nodes: Vec<String> = shown(&rt)
        .into_iter()
        .filter_map(|e| e.key)
        .filter(|k| k.starts_with("room-exchange-turn-post_1-replying_"))
        .collect();
    assert_eq!(replying_nodes, vec!["room-exchange-turn-post_1-replying_m_wren2"]);
}

// case: is not an ending, even with the budget spent
#[test]
fn is_not_an_ending_even_with_the_budget_spent() {
    let mut turns_with = argument_turns();
    turns_with[3].hop = Some(6);
    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem2".into(),
        hop: 6,
    });
    let rt_live = render(room_state(vec![base_post()], turns_with, 6, starting));
    assert_eq!(
        text_of(&rt_live, "room-exchange-foot-post_1").as_deref(),
        Some("6 of 6 automatic replies")
    );

    let mut turns_done = argument_turns();
    turns_done[3].hop = Some(6);
    let rt_done = render(room_state(vec![base_post()], turns_done, 6, None));
    assert_eq!(
        text_of(&rt_done, "room-exchange-foot-post_1").as_deref(),
        Some("6 of 6 automatic replies \u{2014} post again to continue.")
    );
}

// --- a message that arrived after a newer post

// case: shows its time, because otherwise it reads as inserted into the past
#[test]
fn shows_its_time_because_otherwise_it_reads_as_inserted_into_the_past() {
    let later = RoomPost {
        post_id: "post_2".into(),
        text: "Another question.".into(),
        at: "2026-09-11T10:00:02.500Z".into(),
        missions: BTreeMap::new(),
        queued: Vec::new(),
        refused: BTreeMap::new(),
        to: Vec::new(),
    };
    let rt = render(room_state(vec![base_post(), later], argument_turns(), 6, None));
    assert!(rt.presented("room-exchange-time-post_1-m_wren1").is_none());
    assert!(rt.presented("room-exchange-time-post_1-m_gem1").is_none());
    assert!(rt.presented("room-exchange-time-post_1-m_wren2").is_some());
    assert!(rt.presented("room-exchange-time-post_1-m_gem2").is_some());
}

// case: shows no time at all when this is the newest post
#[test]
fn shows_no_time_at_all_when_this_is_the_newest_post() {
    let rt = render(room_state(vec![base_post()], argument_turns(), 6, None));
    assert!(rt.presented("room-exchange-post_1").is_some());
    for mid in ["m_wren1", "m_gem1", "m_wren2", "m_gem2"] {
        assert!(rt.presented(&format!("room-exchange-time-post_1-{mid}")).is_none());
    }
}

// --- the foot

// case: reads as a runway while the argument is still going
#[test]
fn reads_as_a_runway_while_the_argument_is_still_going() {
    let mut turns = argument_turns();
    turns[0].cost = Some(RunCost {
        input_tokens: Some(40_000),
        output_tokens: Some(1_000),
        ..Default::default()
    });
    turns[3].cost = Some(RunCost {
        input_tokens: Some(35_000),
        output_tokens: Some(800),
        ..Default::default()
    });
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert_eq!(
        text_of(&rt, "room-exchange-foot-post_1").as_deref(),
        Some("4 of 6 automatic replies \u{b7} 75k in \u{b7} 1.8k out")
    );
}

// case: names the cause and points at the composer once the budget is spent
#[test]
fn names_the_cause_and_points_at_the_composer_once_the_budget_is_spent() {
    let mut turns = argument_turns();
    turns[3].hop = Some(6);
    turns[0].cost = Some(RunCost {
        input_tokens: Some(50_000),
        output_tokens: Some(1_100),
        ..Default::default()
    });
    turns[3].cost = Some(RunCost {
        input_tokens: Some(31_000),
        output_tokens: Some(1_000),
        ..Default::default()
    });
    let rt = render(room_state(vec![base_post()], turns, 6, None));
    assert_eq!(
        text_of(&rt, "room-exchange-foot-post_1").as_deref(),
        Some("6 of 6 automatic replies \u{b7} 81k in \u{b7} 2.1k out \u{2014} post again to continue.")
    );
}

// case: says the budget even where nothing reported a cost
#[test]
fn says_the_budget_even_where_nothing_reported_a_cost() {
    let mut turns = argument_turns();
    turns.truncate(3);
    turns[2].hop = Some(1);
    let rt = render(room_state(vec![base_post()], turns, 1, None));
    assert_eq!(
        text_of(&rt, "room-exchange-foot-post_1").as_deref(),
        Some("1 of 1 automatic reply \u{2014} post again to continue.")
    );
}
