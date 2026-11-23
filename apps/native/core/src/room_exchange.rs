//! A room post that became a conversation, in the order it was said (`renderer/src/roomExchange.ts`).
//!
//! A grid claims the answers arrived in parallel and none is a reply to another. The instant one more run
//! exists than the post started (or the host is starting a reply), the post becomes a sequence — first answers
//! included, with finished silences, live replies, and waiting members drawn in place.

use std::collections::{BTreeMap, HashMap, HashSet};

/// What a mission is doing right now, in the room's words (`roomExchange.ts` `LiveTurn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveTurn {
    pub register: String,
    pub label: Option<String>,
    pub action: Option<String>,
    pub detail: Option<String>,
    pub started_at: String,
    pub thinking: bool,
}

/// One thing said, one silence, one waiting member, or one live reply in order (`RoomExchangeItem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomExchangeItem {
    Said {
        key: String,
        teammate_id: String,
        name: String,
        text: String,
        at: Option<String>,
        mission_id: String,
        show_time: bool,
        starts_speaker: bool,
    },
    Silent {
        key: String,
        teammate_id: String,
        name: String,
        mission_id: String,
    },
    Waiting {
        key: String,
        teammate_id: String,
        name: String,
    },
    Replying {
        key: String,
        teammate_id: String,
        name: String,
        mission_id: Option<String>,
        live: Option<LiveTurn>,
    },
}

/// Why an exchange stopped (`RoomExchangeFoot.ending`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomExchangeEnding {
    OutOfReplies,
}

/// The runway / ending metadata at the foot of a room exchange (`RoomExchangeFoot`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomExchangeFoot {
    pub hops: usize,
    pub cap: usize,
    pub ending: Option<RoomExchangeEnding>,
}

/// The full ordered sequence and foot for a post that became an exchange (`RoomExchange`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomExchange {
    pub items: Vec<RoomExchangeItem>,
    pub foot: RoomExchangeFoot,
}

/// The post's own missions and waiting members (`PostForExchange`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostForExchange {
    pub post_id: String,
    pub at: String,
    /// `teammate_id -> mission_id`
    pub missions: BTreeMap<String, String>,
    pub waiting: Vec<String>,
}

/// A reply the host has decided on and is starting: no mission of its own yet (`StartingReply`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartingReply {
    pub teammate_id: String,
    pub answering: String,
}

/// Minimal mission record needed for ownership lookup in `sequence_of_post`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangeMissionInfo {
    pub mission_id: String,
    pub teammate_id: String,
}

/// Inputs to `sequence_of_post` (`SequenceInput`).
pub struct SequenceInput<'a> {
    pub post: &'a PostForExchange,
    pub missions: &'a HashMap<String, ExchangeMissionInfo>,
    pub reached: &'a [String],
    pub text_of: Box<dyn Fn(&str) -> Option<String> + 'a>,
    pub started_at_of: Box<dyn Fn(&str) -> Option<String> + 'a>,
    pub finished_of: Box<dyn Fn(&str) -> bool + 'a>,
    pub name_of: Box<dyn Fn(&str) -> String + 'a>,
    pub later_post_at: Option<&'a str>,
    pub hops: usize,
    pub cap: usize,
    pub starting: &'a [StartingReply],
    pub live_of: Box<dyn Fn(&str) -> Option<LiveTurn> + 'a>,
}

/// Whether this post is a conversation at all (`postBecameAnExchange`).
pub fn post_became_an_exchange(
    post: &PostForExchange,
    reached: &[String],
    starting: &[StartingReply],
) -> bool {
    reached.len() > post.missions.len() || !starting.is_empty()
}

/// Build the ordered sequence for a post that became an exchange, or `None` to keep its grid (`sequenceOfPost`).
pub fn sequence_of_post(input: &SequenceInput<'_>) -> Option<RoomExchange> {
    let starting: Vec<&StartingReply> = input
        .starting
        .iter()
        .filter(|entry| input.reached.iter().any(|id| id == &entry.answering))
        .collect();
    let starting_owned: Vec<StartingReply> = starting.iter().map(|s| (*s).clone()).collect();
    if !post_became_an_exchange(input.post, input.reached, &starting_owned) {
        return None;
    }

    let mut owner_of: HashMap<&str, &str> = HashMap::new();
    for (teammate_id, mission_id) in &input.post.missions {
        owner_of.insert(mission_id.as_str(), teammate_id.as_str());
    }

    struct OrderedEntry {
        mission_id: String,
        started_at: Option<String>,
        teammate_id: Option<String>,
    }

    let mut ordered: Vec<OrderedEntry> = input
        .reached
        .iter()
        .map(|mission_id| OrderedEntry {
            mission_id: mission_id.clone(),
            started_at: (input.started_at_of)(mission_id),
            teammate_id: owner_of
                .get(mission_id.as_str())
                .map(|s| (*s).to_owned())
                .or_else(|| input.missions.get(mission_id).map(|m| m.teammate_id.clone())),
        })
        .collect();
    ordered.sort_by(|a, b| {
        let sa = a.started_at.as_deref().unwrap_or("\u{fffd}");
        let sb = b.started_at.as_deref().unwrap_or("\u{fffd}");
        sa.cmp(sb)
    });

    let mut items: Vec<RoomExchangeItem> = Vec::new();
    let mut last_speaker: Option<String> = None;
    for entry in ordered {
        let Some(teammate_id) = entry.teammate_id else {
            continue;
        };
        let name = (input.name_of)(&teammate_id);
        let text = (input.text_of)(&entry.mission_id);
        if text.as_deref().map(str::trim).unwrap_or("").is_empty() {
            if !(input.finished_of)(&entry.mission_id) {
                items.push(RoomExchangeItem::Replying {
                    key: format!("replying_{}", entry.mission_id),
                    teammate_id,
                    name,
                    mission_id: Some(entry.mission_id.clone()),
                    live: (input.live_of)(&entry.mission_id),
                });
                last_speaker = None;
                continue;
            }
            items.push(RoomExchangeItem::Silent {
                key: format!("silent_{}", entry.mission_id),
                teammate_id,
                name,
                mission_id: entry.mission_id,
            });
            last_speaker = None;
            continue;
        }
        let show_time = input.later_post_at.is_some()
            && entry.started_at.is_some()
            && entry.started_at.as_deref() > input.later_post_at;
        let starts_speaker = last_speaker.as_deref() != Some(teammate_id.as_str());
        items.push(RoomExchangeItem::Said {
            key: format!("said_{}", entry.mission_id),
            teammate_id: teammate_id.clone(),
            name,
            text: text.unwrap_or_default(),
            at: entry.started_at,
            mission_id: entry.mission_id,
            show_time,
            starts_speaker,
        });
        last_speaker = Some(teammate_id);
    }

    let mut already_replying: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            RoomExchangeItem::Replying { teammate_id, .. } => Some(teammate_id.clone()),
            _ => None,
        })
        .collect();
    for entry in starting {
        if !already_replying.insert(entry.teammate_id.clone()) {
            continue;
        }
        items.push(RoomExchangeItem::Replying {
            key: format!("replying_{}_{}", entry.teammate_id, entry.answering),
            teammate_id: entry.teammate_id.clone(),
            name: (input.name_of)(&entry.teammate_id),
            mission_id: None,
            live: None,
        });
    }

    for teammate_id in &input.post.waiting {
        items.push(RoomExchangeItem::Waiting {
            key: format!("waiting_{teammate_id}"),
            teammate_id: teammate_id.clone(),
            name: (input.name_of)(teammate_id),
        });
    }

    let any_replying = items
        .iter()
        .any(|i| matches!(i, RoomExchangeItem::Replying { .. }));
    let ending = if input.hops >= input.cap && !any_replying {
        Some(RoomExchangeEnding::OutOfReplies)
    } else {
        None
    };

    Some(RoomExchange {
        items,
        foot: RoomExchangeFoot {
            hops: input.hops,
            cap: input.cap,
            ending,
        },
    })
}

/// The foot's one line: the budget, the cost, and the ending if there is one (`footLine`).
pub fn foot_line(foot: &RoomExchangeFoot, cost_text: Option<&str>) -> String {
    let unit = if foot.cap == 1 { "reply" } else { "replies" };
    let budget = format!("{} of {} automatic {unit}", foot.hops, foot.cap);
    let line = match cost_text {
        Some(cost) => format!("{budget} \u{b7} {cost}"),
        None => budget,
    };
    if foot.ending == Some(RoomExchangeEnding::OutOfReplies) {
        format!("{line} \u{2014} post again to continue.")
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post_with_waiting(waiting: &[&str]) -> PostForExchange {
        let mut missions = BTreeMap::new();
        missions.insert("tm_wren".into(), "m_wren1".into());
        missions.insert("tm_gem".into(), "m_gem1".into());
        PostForExchange {
            post_id: "post_1".into(),
            at: "2026-09-11T10:00:00.000Z".into(),
            missions,
            waiting: waiting.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn base_post() -> PostForExchange {
        post_with_waiting(&[])
    }

    fn arg_entry(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
        match id {
            "m_wren1" => Some(("tm_wren", "2026-09-11T10:00:01.000Z", "Tabs.")),
            "m_gem1" => Some(("tm_gem", "2026-09-11T10:00:02.000Z", "Until a line wraps.")),
            "m_wren2" => Some(("tm_wren", "2026-09-11T10:00:03.000Z", "Do not align past the indent.")),
            "m_gem2" => Some(("tm_gem", "2026-09-11T10:00:04.000Z", "Formatters emit spaces.")),
            _ => None,
        }
    }

    fn base_reached() -> Vec<String> {
        ["m_wren1", "m_gem1", "m_wren2", "m_gem2"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn base_missions(reached: &[String]) -> HashMap<String, ExchangeMissionInfo> {
        reached
            .iter()
            .filter_map(|id| {
                let (owner, _, _) = arg_entry(id)?;
                Some((
                    id.clone(),
                    ExchangeMissionInfo {
                        mission_id: id.clone(),
                        teammate_id: owner.to_owned(),
                    },
                ))
            })
            .collect()
    }

    fn name_of(id: &str) -> String {
        match id {
            "tm_wren" => "Wren".into(),
            "tm_gem" => "Gem".into(),
            _ => "someone".into(),
        }
    }

    fn base_input<'a>(
        post: &'a PostForExchange,
        missions: &'a HashMap<String, ExchangeMissionInfo>,
        reached: &'a [String],
        starting: &'a [StartingReply],
    ) -> SequenceInput<'a> {
        SequenceInput {
            post,
            missions,
            reached,
            text_of: Box::new(|id| arg_entry(id).map(|(_, _, t)| t.to_owned())),
            started_at_of: Box::new(|id| arg_entry(id).map(|(_, at, _)| at.to_owned())),
            finished_of: Box::new(|_| true),
            name_of: Box::new(name_of),
            later_post_at: None,
            hops: 4,
            cap: 6,
            starting,
            live_of: Box::new(|_| None),
        }
    }

    // --- whether a post is a conversation at all

    // case: is not, when every member answered once and nobody replied
    #[test]
    fn is_not_when_every_member_answered_once_and_nobody_replied() {
        let post = base_post();
        let reached = vec!["m_wren1".to_owned(), "m_gem1".to_owned()];
        let missions = base_missions(&reached);
        assert!(!post_became_an_exchange(&post, &reached, &[]));
        assert!(sequence_of_post(&base_input(&post, &missions, &reached, &[])).is_none());
    }

    // case: is, the moment one more run exists than the post started
    #[test]
    fn is_the_moment_one_more_run_exists_than_the_post_started() {
        let post = base_post();
        let reached = vec!["m_wren1".to_owned(), "m_gem1".to_owned(), "m_wren2".to_owned()];
        assert!(post_became_an_exchange(&post, &reached, &[]));
    }

    // --- the sequence

    // case: is every turn in the order it was said, first answers included
    #[test]
    fn is_every_turn_in_the_order_it_was_said_first_answers_included() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &[])).unwrap();
        let summary: Vec<(&str, &str)> = found
            .items
            .iter()
            .map(|item| match item {
                RoomExchangeItem::Said { name, .. } => ("said", name.as_str()),
                _ => ("other", ""),
            })
            .collect();
        assert_eq!(summary, vec![("said", "Wren"), ("said", "Gem"), ("said", "Wren"), ("said", "Gem")]);
    }

    // case: groups consecutive turns by one speaker under a single face
    #[test]
    fn groups_consecutive_turns_by_one_speaker_under_a_single_face() {
        let post = base_post();
        let reached = base_reached();
        let mut missions = base_missions(&reached);
        missions.get_mut("m_wren2").unwrap().teammate_id = "tm_gem".into();
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.text_of = Box::new(|id| {
            if id == "m_wren2" {
                Some("And another thing.".into())
            } else {
                arg_entry(id).map(|(_, _, t)| t.to_owned())
            }
        });
        let found = sequence_of_post(&input).unwrap();
        let starts: Vec<Option<bool>> = found
            .items
            .iter()
            .map(|item| match item {
                RoomExchangeItem::Said { starts_speaker, .. } => Some(*starts_speaker),
                _ => None,
            })
            .collect();
        assert_eq!(starts, vec![Some(true), Some(true), Some(false), Some(false)]);
    }

    // case: draws a finished turn that said nothing, because absence is information
    #[test]
    fn draws_a_finished_turn_that_said_nothing_because_absence_is_information() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.text_of = Box::new(|id| {
            if id == "m_wren2" {
                None
            } else {
                arg_entry(id).map(|(_, _, t)| t.to_owned())
            }
        });
        let found = sequence_of_post(&input).unwrap();
        let pos = found
            .items
            .iter()
            .position(|item| matches!(item, RoomExchangeItem::Silent { name, mission_id, .. } if name == "Wren" && mission_id == "m_wren2"));
        assert_eq!(pos, Some(2));
    }

    // case: draws a run that is still going as the turn it is, never as a silence
    #[test]
    fn draws_a_run_that_is_still_going_as_the_turn_it_is_never_as_a_silence() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.text_of = Box::new(|id| {
            if id == "m_gem2" {
                None
            } else {
                arg_entry(id).map(|(_, _, t)| t.to_owned())
            }
        });
        input.finished_of = Box::new(|id| id != "m_gem2");
        let found = sequence_of_post(&input).unwrap();
        assert!(!found.items.iter().any(|i| matches!(i, RoomExchangeItem::Silent { .. })));
        assert!(matches!(
            found.items.last(),
            Some(RoomExchangeItem::Replying { name, mission_id, .. })
                if name == "Gem" && mission_id.as_deref() == Some("m_gem2")
        ));
    }

    // case: carries what a live turn is doing, so a room can draw the thread’s line
    #[test]
    fn carries_what_a_live_turn_is_doing_so_a_room_can_draw_the_threads_line() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.text_of = Box::new(|id| {
            if id == "m_gem2" {
                None
            } else {
                arg_entry(id).map(|(_, _, t)| t.to_owned())
            }
        });
        input.finished_of = Box::new(|id| id != "m_gem2");
        input.live_of = Box::new(|id| {
            (id == "m_gem2").then(|| LiveTurn {
                register: "tool".into(),
                label: Some("README.md".into()),
                action: None,
                detail: None,
                started_at: "2026-09-11T10:00:05.000Z".into(),
                thinking: false,
            })
        });
        let found = sequence_of_post(&input).unwrap();
        assert!(matches!(
            found.items.last(),
            Some(RoomExchangeItem::Replying { live: Some(live), .. })
                if live.register == "tool" && live.label.as_deref() == Some("README.md")
        ));
    }

    // case: has nothing to say a hop is doing before its run exists
    #[test]
    fn has_nothing_to_say_a_hop_is_doing_before_its_run_exists() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_gem2".into(),
        }];
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &starting)).unwrap();
        assert!(matches!(
            found.items.last(),
            Some(RoomExchangeItem::Replying { live: None, .. })
        ));
    }

    // case: lists a member waiting for a slot, so the room does not look finished
    #[test]
    fn lists_a_member_waiting_for_a_slot_so_the_room_does_not_look_finished() {
        let post = post_with_waiting(&["tm_gem"]);
        let reached = base_reached();
        let missions = base_missions(&reached);
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &[])).unwrap();
        assert!(matches!(
            found.items.last(),
            Some(RoomExchangeItem::Waiting { name, .. }) if name == "Gem"
        ));
    }

    // --- a reply the host is still starting

    // case: is a turn in the sequence, with no mission of its own yet
    #[test]
    fn is_a_turn_in_the_sequence_with_no_mission_of_its_own_yet() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_gem2".into(),
        }];
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &starting)).unwrap();
        assert!(matches!(
            found.items.last(),
            Some(RoomExchangeItem::Replying { name, mission_id: None, .. }) if name == "Wren"
        ));
    }

    // case: makes a post a conversation on its own, before the second run exists
    #[test]
    fn makes_a_post_a_conversation_on_its_own_before_the_second_run_exists() {
        let post = base_post();
        let reached = vec!["m_wren1".to_owned(), "m_gem1".to_owned()];
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_gem1".into(),
        }];
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &starting));
        assert!(matches!(
            found.and_then(|ex| ex.items.into_iter().last()),
            Some(RoomExchangeItem::Replying { name, .. }) if name == "Wren"
        ));
    }

    // case: belongs to the post whose conversation it answers, and no other
    #[test]
    fn belongs_to_the_post_whose_conversation_it_answers_and_no_other() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_elsewhere".into(),
        }];
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &starting)).unwrap();
        assert!(!found.items.iter().any(|i| matches!(i, RoomExchangeItem::Replying { .. })));
    }

    // case: is not drawn twice when the run it becomes has already been announced
    #[test]
    fn is_not_drawn_twice_when_the_run_it_becomes_has_already_been_announced() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_gem1".into(),
        }];
        let mut input = base_input(&post, &missions, &reached, &starting);
        input.text_of = Box::new(|id| {
            if id == "m_wren2" {
                None
            } else {
                arg_entry(id).map(|(_, _, t)| t.to_owned())
            }
        });
        input.finished_of = Box::new(|id| id != "m_wren2");
        let found = sequence_of_post(&input).unwrap();
        let replying_count = found
            .items
            .iter()
            .filter(|i| matches!(i, RoomExchangeItem::Replying { .. }))
            .count();
        assert_eq!(replying_count, 1);
    }

    // case: is not an ending, even with the budget spent
    #[test]
    fn is_not_an_ending_even_with_the_budget_spent() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let starting = vec![StartingReply {
            teammate_id: "tm_wren".into(),
            answering: "m_gem2".into(),
        }];
        let mut with_starting = base_input(&post, &missions, &reached, &starting);
        with_starting.hops = 6;
        with_starting.cap = 6;
        assert_eq!(sequence_of_post(&with_starting).unwrap().foot.ending, None);

        let mut without_starting = base_input(&post, &missions, &reached, &[]);
        without_starting.hops = 6;
        without_starting.cap = 6;
        assert_eq!(
            sequence_of_post(&without_starting).unwrap().foot.ending,
            Some(RoomExchangeEnding::OutOfReplies)
        );
    }

    // --- a message that arrived after a newer post

    // case: shows its time, because otherwise it reads as inserted into the past
    #[test]
    fn shows_its_time_because_otherwise_it_reads_as_inserted_into_the_past() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.later_post_at = Some("2026-09-11T10:00:02.500Z");
        let found = sequence_of_post(&input).unwrap();
        let times: Vec<Option<bool>> = found
            .items
            .iter()
            .map(|item| match item {
                RoomExchangeItem::Said { show_time, .. } => Some(*show_time),
                _ => None,
            })
            .collect();
        assert_eq!(times, vec![Some(false), Some(false), Some(true), Some(true)]);
    }

    // case: shows no time at all when this is the newest post
    #[test]
    fn shows_no_time_at_all_when_this_is_the_newest_post() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &[])).unwrap();
        assert!(found
            .items
            .iter()
            .all(|item| !matches!(item, RoomExchangeItem::Said { show_time: true, .. })));
    }

    // --- the foot

    // case: reads as a runway while the argument is still going
    #[test]
    fn reads_as_a_runway_while_the_argument_is_still_going() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let found = sequence_of_post(&base_input(&post, &missions, &reached, &[])).unwrap();
        assert_eq!(found.foot.ending, None);
        assert_eq!(
            foot_line(&found.foot, Some("75k in · 1.8k out")),
            "4 of 6 automatic replies · 75k in · 1.8k out"
        );
    }

    // case: names the cause and points at the composer once the budget is spent
    #[test]
    fn names_the_cause_and_points_at_the_composer_once_the_budget_is_spent() {
        let post = base_post();
        let reached = base_reached();
        let missions = base_missions(&reached);
        let mut input = base_input(&post, &missions, &reached, &[]);
        input.hops = 6;
        input.cap = 6;
        let found = sequence_of_post(&input).unwrap();
        assert_eq!(found.foot.ending, Some(RoomExchangeEnding::OutOfReplies));
        assert_eq!(
            foot_line(&found.foot, Some("81k in · 2.1k out")),
            "6 of 6 automatic replies · 81k in · 2.1k out — post again to continue."
        );
    }

    // case: says the budget even where nothing reported a cost
    #[test]
    fn says_the_budget_even_where_nothing_reported_a_cost() {
        let foot = RoomExchangeFoot {
            hops: 1,
            cap: 1,
            ending: Some(RoomExchangeEnding::OutOfReplies),
        };
        assert_eq!(foot_line(&foot, None), "1 of 1 automatic reply — post again to continue.");
    }
}
