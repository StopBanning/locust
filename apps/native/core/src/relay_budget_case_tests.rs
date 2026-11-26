//! `apps/desktop/src/main/the-reply-budget-is-counted-right.test.ts`, one test per case, on that file's fixtures: Wren,
//! Booty and Nova all on Claude Code / haiku, read-only; a start takes 5 ms; a returned answer is "The build command is
//! pnpm check.". Every expected word and number below is the one Electron's test expects.

use super::case_tests::{Host, HostState, harness, route, said, st, tm};
use super::*;
use std::collections::{BTreeMap, HashMap};

fn wren() -> PeerRosterEntry {
    tm("tm_wren", "Wren", "Code & Migrations", Some(route("claude", "haiku", "ask")))
}

fn booty() -> PeerRosterEntry {
    tm("tm_booty", "Booty", "Custom", Some(route("claude", "haiku", "ask")))
}

fn nova() -> PeerRosterEntry {
    tm("tm_nova", "Nova", "Docs & QA", Some(route("claude", "haiku", "ask")))
}

fn budget(cap: usize, set: impl FnOnce(&mut HostState)) -> Relay<Host> {
    harness(|s| {
        s.cap = cap;
        s.peers = [wren(), booty(), nova()]
            .into_iter()
            .map(|p| (p.teammate_id.clone(), p))
            .collect::<HashMap<_, _>>();
        s.final_reply = Some("The build command is pnpm check.".into());
        s.start_delay_ms = 5;
        set(s);
    })
}

fn room_post(s: &mut HostState) {
    s.exchange_of.insert("mission_wren".into(), "room:r1:p1".into());
    s.exchange_of.insert("mission_nova".into(), "room:r1:p1".into());
}

fn message(from: &PeerRosterEntry, to: &PeerRosterEntry, text: &str) -> RelayedMessage {
    RelayedMessage {
        message_id: format!("wm_{}_{}", from.teammate_id, to.teammate_id),
        from_teammate_id: from.teammate_id.clone(),
        from_name: from.name.clone(),
        from_mission_id: "m".into(),
        to_teammate_id: to.teammate_id.clone(),
        to_name: to.name.clone(),
        text: text.into(),
        urgent: false,
        defer: false,
        wants_answer: false,
    }
}

fn wanting(m: RelayedMessage) -> RelayedMessage {
    RelayedMessage {
        wants_answer: true,
        ..m
    }
}

fn sharing(mission_id: &str, sender: PeerRosterEntry, relay: Option<RelayOrigin>) -> SharingMission {
    SharingMission {
        run_id: format!("run_{mission_id}"),
        mission_id: mission_id.into(),
        runtime: "claude".into(),
        sandbox: "read-only".into(),
        model: Some("haiku".into()),
        peer: sender,
        relay,
        started_by: None,
    }
}

fn branch(hop: usize, last: &[(&str, &str)]) -> Option<RelayOrigin> {
    Some(RelayOrigin {
        hop,
        root_mission_id: Some("mission_root".into()),
        last_mission_of: last.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect(),
        relayed_to: Vec::new(),
        answering: Vec::new(),
    })
}

fn from_root() -> Option<RelayOrigin> {
    branch(1, &[("tm_wren", "mission_root")])
}

fn starts(relay: &Relay<Host>) -> Vec<StartRequest> {
    st(relay).starts.clone()
}

fn who(relay: &Relay<Host>) -> Vec<String> {
    starts(relay).iter().map(|s| s.peer.teammate_id.clone()).collect()
}

fn told_stopped(relay: &Relay<Host>, words: &str) -> bool {
    said(relay).iter().any(|l| l.starts_with(words))
}

fn busy() -> Option<Result<StartResult, String>> {
    Some(Ok(StartResult::Busy { pool: false }))
}

// ---------------------------------------------------------------------------------------------------------------------
// the budget is spent at the decision
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn lets_only_one_of_two_replies_decided_at_the_same_moment_through_the_last_slot() {
    let relay = budget(2, |_| {});
    // The exchange has spent one already.
    relay.on_shared(&sharing("mission_root", wren(), None), &[message(&wren(), &booty(), "What is the build command?")]);
    assert_eq!(starts(&relay).len(), 1);
    // Two replies in one exchange, decided together, one slot left: two threads, each start taking 5 ms.
    std::thread::scope(|scope| {
        scope.spawn(|| {
            relay.on_shared(
                &sharing("mission_b", booty(), from_root()),
                &[message(&booty(), &nova(), "Nova, what does the lint step run?")],
            )
        });
        scope.spawn(|| {
            relay.on_shared(
                &sharing("mission_c", nova(), from_root()),
                &[message(&nova(), &booty(), "Booty, which test is failing?")],
            )
        });
    });
    assert_eq!(starts(&relay).len(), 2);
    assert!(told_stopped(&relay, "Stopped after 2 automatic replies"));
}

#[test]
fn gives_the_count_back_when_no_run_comes_of_the_decision() {
    let relay = budget(1, |s| {
        s.start_results = vec![Some(Ok(StartResult::Refused {
            message: "No runtime.".into(),
        }))]
    });
    relay.on_shared(&sharing("mission_root", wren(), None), &[message(&wren(), &booty(), "What is the build command?")]);
    // The refused start spent nothing, so the one slot is still there.
    relay.on_shared(&sharing("mission_root", wren(), None), &[message(&wren(), &nova(), "What does lint run?")]);
    assert_eq!(starts(&relay).len(), 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// the budget is one per room post
// ---------------------------------------------------------------------------------------------------------------------

fn post(post_id: &str, missions: &[(&str, &str)]) -> crate::room_store::RoomPost {
    crate::room_store::RoomPost {
        post_id: post_id.into(),
        text: String::new(),
        at: String::new(),
        missions: missions.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect::<BTreeMap<_, _>>(),
        queued: Vec::new(),
        refused: BTreeMap::new(),
        to: Vec::new(),
    }
}

/// `native.rs` `NativeRelayHost::exchange_of` reads the room store through this.
#[test]
fn is_found_from_the_room_store_every_mission_of_one_post_one_key() {
    let rooms = vec![crate::room_store::PublicRoom {
        room_id: "r1".into(),
        name: String::new(),
        teammate_ids: Vec::new(),
        created_at: String::new(),
        posts: vec![
            post("p1", &[("tm_wren", "mission_wren"), ("tm_nova", "mission_nova")]),
            post("p2", &[("tm_wren", "mission_later")]),
        ],
        tasks: Vec::new(),
    }];
    use crate::room_store::exchange_of_room_post as of;
    assert_eq!(of(&rooms, "mission_wren").as_deref(), Some("room:r1:p1"));
    assert_eq!(of(&rooms, "mission_nova").as_deref(), Some("room:r1:p1"));
    assert_eq!(of(&rooms, "mission_later").as_deref(), Some("room:r1:p2"));
    assert_eq!(of(&rooms, "mission_elsewhere"), None);
}

#[test]
fn counts_every_member_of_the_post_against_one_budget() {
    let relay = budget(1, room_post);
    // Two members answering the same post, each writing to a colleague.
    relay.on_shared(&sharing("mission_wren", wren(), None), &[message(&wren(), &booty(), "Booty, what is the build command?")]);
    relay.on_shared(&sharing("mission_nova", nova(), None), &[message(&nova(), &booty(), "Booty, which test is failing?")]);
    let all = starts(&relay);
    assert_eq!(all.len(), 1);
    assert!(told_stopped(&relay, "Stopped after 1 automatic reply"));
    assert_eq!(all[0].relay.root_mission_id.as_deref(), Some("room:r1:p1"));
}

#[test]
fn tells_a_held_reply_the_number_it_is_when_it_starts_not_when_it_was_held() {
    let relay = budget(6, |s| {
        room_post(s);
        s.start_results = vec![busy()];
    });
    // Held: Booty is mid-run when Wren writes.
    relay.on_shared(&sharing("mission_wren", wren(), None), &[message(&wren(), &booty(), "Booty, what is the build command?")]);
    // Meanwhile the post's other member spends a reply.
    relay.on_shared(&sharing("mission_nova", nova(), None), &[message(&nova(), &wren(), "Wren, which test is failing?")]);
    // Booty's run ends; the held reply starts now, second in the exchange.
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_booty_before".into(),
        peer: Some(booty()),
        relay: None,
    });
    let held = starts(&relay).into_iter().filter(|s| s.peer.teammate_id == "tm_booty").last().unwrap();
    assert!(held.prompt.contains("This is automatic reply 2 of 6."));
}

#[test]
fn counts_a_held_reply_when_it_starts_so_the_budget_still_binds_after_it() {
    let relay = budget(2, |s| {
        room_post(s);
        s.start_results = vec![busy()];
    });
    relay.on_shared(&sharing("mission_wren", wren(), None), &[message(&wren(), &booty(), "Booty, what is the build command?")]);
    relay.on_shared(&sharing("mission_nova", nova(), None), &[message(&nova(), &wren(), "Wren, which test is failing?")]);
    // The held reply starts: the exchange has now spent both.
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_booty_before".into(),
        peer: Some(booty()),
        relay: None,
    });
    let before = starts(&relay).len();
    relay.on_shared(&sharing("mission_nova", nova(), None), &[message(&nova(), &booty(), "Booty, and the lint step?")]);
    assert_eq!(starts(&relay).len(), before);
    assert!(told_stopped(&relay, "Stopped after 2 automatic replies"));
}

#[test]
fn tells_each_reply_its_number_in_the_exchange_not_the_depth_of_its_chain() {
    let relay = budget(6, room_post);
    relay.on_shared(&sharing("mission_wren", wren(), None), &[message(&wren(), &booty(), "Booty, what is the build command?")]);
    relay.on_shared(&sharing("mission_nova", nova(), None), &[message(&nova(), &booty(), "Booty, which test is failing?")]);
    let all = starts(&relay);
    assert!(all[0].prompt.contains("This is automatic reply 1 of 6."));
    // Hop one of its own chain, and the second reply of the exchange.
    assert!(all[1].prompt.contains("This is automatic reply 2 of 6."));
}

// ---------------------------------------------------------------------------------------------------------------------
// a message that says its recipient need do nothing
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn is_read_the_way_it_was_meant() {
    assert!(says_nothing_is_needed("The request is closed. No further action is needed from you."));
    assert!(says_nothing_is_needed("You don't need to do anything with this."));
    // The recipient's own "from me" is an answer being given, not a closing.
    assert!(!says_nothing_is_needed("TANGERINE. Nothing further is needed from me on this turn."));
    // Asking for something makes it a request, whatever else it says.
    assert!(!says_nothing_is_needed(
        "Please fix the flaky test. No further action is needed from you after that."
    ));
    assert!(!says_nothing_is_needed("No further action is needed from you -- or is there?"));
    // A reply is not work: "no reply needed" can come with a job.
    assert!(!says_nothing_is_needed("Rename the helper to fetchAll. No reply needed."));
}

#[test]
fn is_delivered_without_starting_a_run_and_says_why() {
    let relay = budget(6, |_| {});
    relay.on_shared(
        &sharing("mission_root", wren(), None),
        &[message(&wren(), &booty(), "The build is green. No further action is needed from you.")],
    );
    assert!(starts(&relay).is_empty());
    assert!(said(&relay).contains(
        &"Sent to Booty to read on their next run: the message says nothing more is needed from them.".to_owned()
    ));
}

#[test]
fn still_starts_the_run_of_the_teammate_whose_conversation_the_person_is_reading() {
    let relay = budget(6, |_| {});
    // Booty answers Wren, the person's teammate: that run is how the person hears.
    relay.on_shared(
        &sharing("mission_b", booty(), from_root()),
        &[message(&booty(), &wren(), "TANGERINE. No further action is needed from you.")],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].peer.teammate_id, "tm_wren");
}

#[test]
fn still_starts_one_that_wants_an_answer() {
    let relay = budget(6, |_| {});
    relay.on_shared(
        &sharing("mission_root", wren(), None),
        &[wanting(message(&wren(), &booty(), "No further action is needed from you."))],
    );
    assert_eq!(starts(&relay).len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------------
// every run the relay starts is counted
// ---------------------------------------------------------------------------------------------------------------------

fn meeting_of_two(relay: &Relay<Host>) {
    // A question to two teammates is a meeting: both reply, then Wren's turn.
    relay.on_shared(
        &sharing("mission_root", wren(), None),
        &[message(&wren(), &booty(), "Booty, the build command?"), message(&wren(), &nova(), "Nova, the lint step?")],
    );
    relay.on_shared(&sharing("mission_1", booty(), from_root()), &[message(&booty(), &wren(), "pnpm check.")]);
    relay.on_shared(&sharing("mission_2", nova(), from_root()), &[message(&nova(), &wren(), "eslint.")]);
}

#[test]
fn counts_the_turn_of_the_asker_that_closes_a_meeting() {
    let relay = budget(3, |_| {});
    meeting_of_two(&relay);
    assert_eq!(who(&relay), vec!["tm_booty", "tm_nova", "tm_wren"]);
    // Three spent of three: the next reply in the exchange stops.
    relay.on_shared(
        &sharing("mission_3", wren(), branch(2, &[("tm_wren", "mission_root")])),
        &[message(&wren(), &booty(), "Booty, one more thing: the test command?")],
    );
    assert_eq!(starts(&relay).len(), 3);
    assert!(told_stopped(&relay, "Stopped after 3 automatic replies"));
}

#[test]
fn counts_the_turn_an_answer_brought_back_starts() {
    let relay = budget(2, |_| {});
    // Wren asks; Booty answers in its own conversation and ends without writing back.
    relay.on_shared(
        &sharing("mission_root", wren(), None),
        &[wanting(message(&wren(), &booty(), "Booty, what is the build command?"))],
    );
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_1".into(),
        peer: Some(booty()),
        relay: from_root(),
    });
    assert_eq!(who(&relay), vec!["tm_booty", "tm_wren"]);
    // Two spent of two -- asked from a shallow branch, so the depth of its own chain (1) cannot be what stops it.
    relay.on_shared(
        &sharing("mission_2", nova(), from_root()),
        &[message(&nova(), &booty(), "Booty, and the test command?")],
    );
    assert_eq!(starts(&relay).len(), 2);
    assert!(told_stopped(&relay, "Stopped after 2 automatic replies"));
}

// ---------------------------------------------------------------------------------------------------------------------
// the reply rules, said once to each teammate
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn is_the_full_brief_on_a_first_reply_and_a_line_on_the_next() {
    let relay = budget(12, |_| {});
    relay.on_shared(&sharing("mission_root", wren(), None), &[message(&wren(), &booty(), "Booty, what is the build command?")]);
    // Hop 2, into the person's conversation; hop 3, Booty's first reply turn.
    let r = |i: usize| Some(starts(&relay)[i].relay.clone());
    relay.on_shared(&sharing("mission_1", booty(), r(0)), &[message(&booty(), &wren(), "Which package?")]);
    relay.on_shared(&sharing("mission_2", wren(), r(1)), &[message(&wren(), &booty(), "The desktop app.")]);
    // Hop 4, Wren again; hop 5, Booty again: both have been told.
    relay.on_shared(&sharing("mission_3", booty(), r(2)), &[message(&booty(), &wren(), "Then which script?")]);
    relay.on_shared(&sharing("mission_4", wren(), r(3)), &[message(&wren(), &booty(), "The build script.")]);
    assert_eq!(who(&relay), vec!["tm_booty", "tm_wren", "tm_booty", "tm_wren", "tm_booty"]);
    let all = starts(&relay);
    let full = "reach nobody and cost a run each";
    let line = "The rules for replies from your earlier turn in this exchange still hold";
    // First reply turns: the whole of it.
    assert!(all[2].prompt.contains(full));
    assert!(!all[2].prompt.contains(line));
    // Later ones: the line, the count, and for the person's teammate its job.
    assert!(all[3].prompt.contains(line));
    assert!(!all[3].prompt.contains("is a MODEL, not a person"));
    assert!(all[3].prompt.contains("The person who started this conversation reads it"));
    assert!(all[3].prompt.contains("This is automatic reply 4 of 12."));
    assert!(all[4].prompt.contains(line));
    assert!(!all[4].prompt.contains("reads it"));
    assert!(all[4].prompt.contains("<locust-share to=\"Wren\">"));
    // Electron counts a string's length in UTF-16 units; these briefs are ASCII.
    assert!(all[4].prompt.encode_utf16().count() < all[2].prompt.encode_utf16().count() / 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// the turn that brings the result to the person
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn closes_a_meeting_by_writing_to_the_person_not_by_saying_nobody_is_there() {
    let relay = budget(12, |_| {});
    meeting_of_two(&relay);
    let close = starts(&relay).into_iter().find(|s| s.peer.teammate_id == "tm_wren").unwrap();
    assert!(close.prompt.contains("The person who started this conversation reads it and is waiting on this"));
    assert!(!close.prompt.contains("There is no person in this exchange"));
}

#[test]
fn still_says_nobody_is_there_to_a_teammate_the_person_is_not_reading() {
    let relay = budget(12, |_| {});
    // Booty, answering Wren's question, opens its own meeting with two others.
    relay.on_shared(
        &sharing("mission_b", booty(), from_root()),
        &[message(&booty(), &wren(), "Wren, which package?"), message(&booty(), &nova(), "Nova, which script?")],
    );
    let inner = branch(2, &[("tm_wren", "mission_root"), ("tm_booty", "mission_b")]);
    relay.on_shared(&sharing("mission_1", wren(), inner.clone()), &[message(&wren(), &booty(), "The desktop app.")]);
    relay.on_shared(&sharing("mission_2", nova(), inner), &[message(&nova(), &booty(), "The build script.")]);
    let close = starts(&relay).into_iter().filter(|s| s.peer.teammate_id == "tm_booty").last().unwrap();
    assert!(close.prompt.contains("There is no person in this exchange"));
}
