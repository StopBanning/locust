//! Two teammates look at each other when one hands the other a message (`glances.ts`, App.tsx 3322-3328).
//!
//! Electron hears each posted peer message as it happens and holds it as a handoff for `RECEIVED_GLANCE_MS`. This
//! window learns of messages when its recent list is read again, so a message is a handoff the first time a read
//! holds it, and never on the first read after launch (those were posted before the window was there to see them).
//! The faces in one line (the sidebar's roster, the rail: both columns, as Electron's roster is) then turn toward
//! each other through `glances_among`, and each face holds its glance for `GLANCE_HOLD_MS` (`bot_stage`).

use super::*;
use locust_core::bot_rig::{glances_among, GlanceAxis, GlanceSide, Handoff, RECEIVED_GLANCE_MS};

/// One teammate handing another a message, until `until_s` on the window's clock.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HeldHandoff {
    pub key: String,
    pub from: String,
    pub to: String,
    pub until_s: f64,
}

#[derive(Debug, Default)]
pub(super) struct HandoffGlances {
    /// The message ids the last read held; None before the first read.
    seen: Option<std::collections::HashSet<String>>,
    pub held: Vec<HeldHandoff>,
}

impl HandoffGlances {
    /// A read of the workroom came back at `now_s`: each message it holds for the first time is a handoff.
    pub fn read(&mut self, messages: &HashMap<String, locust_core::workroom::Message>, now_s: f64) {
        let first = self.seen.is_none();
        let seen = self.seen.get_or_insert_with(Default::default);
        let mut fresh: Vec<&locust_core::workroom::Message> =
            messages.values().filter(|m| !seen.contains(&m.message_id)).collect();
        // In the order they were posted, so a face in two handoffs looks toward the latest (glances.ts).
        fresh.sort_by_key(|m| m.sequence);
        *seen = messages.keys().cloned().collect();
        if first {
            return;
        }
        let until_s = now_s + RECEIVED_GLANCE_MS as f64 / 1000.0;
        for m in fresh {
            self.held.retain(|h| h.key != m.message_id);
            self.held.push(HeldHandoff {
                key: m.message_id.clone(),
                from: m.from.teammate_id.clone(),
                to: m.to.teammate_id.clone(),
                until_s,
            });
        }
    }

    /// Lets each handoff go once its moment is over; true when one went.
    pub fn expire(&mut self, now_s: f64) -> bool {
        let before = self.held.len();
        self.held.retain(|h| now_s < h.until_s);
        self.held.len() != before
    }

    /// The side each face in `line` looks to, a column of faces as the roster is.
    pub fn in_column(&self, line: &[String]) -> HashMap<String, GlanceSide> {
        let handoffs: Vec<Handoff<'_>> =
            self.held.iter().map(|h| Handoff { key: &h.key, from: &h.from, to: &h.to }).collect();
        glances_among(line, &handoffs, GlanceAxis::Column)
    }
}

/// The roster's glances: the team in the order the sidebar and the rail draw it.
pub(super) fn roster_glances(s: &State) -> HashMap<String, GlanceSide> {
    let line: Vec<String> = s.teammates.iter().map(|t| t.id.clone()).collect();
    s.handoff_glances.in_column(&line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::workroom::{Message, Party};

    fn message(id: &str, sequence: u64, from: &str, to: &str) -> (String, Message) {
        let party = |id: &str| Party { teammate_id: id.into(), name: id.into() };
        (
            id.into(),
            Message {
                message_id: id.into(),
                sequence,
                from: party(from),
                from_mission: "m".into(),
                to: party(to),
                text: "hi".into(),
                posted_at: "2026-11-15T12:00:00.000Z".into(),
                observed: None,
            },
        )
    }

    fn line() -> Vec<String> {
        ["wren", "atlas", "sable"].map(String::from).to_vec()
    }

    // App.tsx 3326-3328: a posted message is a handoff, and the two faces turn toward each other down the column.
    #[test]
    fn a_message_handed_over_turns_the_two_faces_toward_each_other() {
        let mut g = HandoffGlances::default();
        g.read(&HashMap::new(), 0.0);
        g.read(&HashMap::from([message("msg_1", 1, "sable", "wren")]), 10.0);
        let sides = g.in_column(&line());
        assert_eq!(sides.get("sable"), Some(&GlanceSide::Up));
        assert_eq!(sides.get("wren"), Some(&GlanceSide::Down));
        assert!(!sides.contains_key("atlas"));
    }

    #[test]
    fn what_was_already_there_when_the_window_opened_is_no_handoff() {
        let mut g = HandoffGlances::default();
        g.read(&HashMap::from([message("msg_1", 1, "sable", "wren")]), 0.0);
        assert!(g.held.is_empty());
        g.read(&HashMap::from([message("msg_1", 1, "sable", "wren")]), 5.0);
        assert!(g.held.is_empty(), "the same message read again is not a new one");
    }

    // RECEIVED_GLANCE_MS (faceState.ts:214): the handoff lasts 2.8 s from when it was heard.
    #[test]
    fn a_handoff_lasts_two_point_eight_seconds_from_when_it_was_heard() {
        let mut g = HandoffGlances::default();
        g.read(&HashMap::new(), 0.0);
        g.read(&HashMap::from([message("msg_1", 1, "wren", "atlas")]), 10.0);
        assert!(!g.expire(12.79));
        assert_eq!(g.held.len(), 1);
        assert!(g.expire(12.8));
        assert!(g.in_column(&line()).is_empty());
    }

    // Sidebar.tsx 621 and 1518: the roster's two faces, drawn by the real sidebar, turn toward each other on their rigs.
    #[test]
    fn the_rosters_two_faces_turn_toward_each_other_when_one_hands_the_other_a_message() {
        let mate = |id: &str| Teammate {
            id: id.into(),
            name: id.into(),
            role: String::new(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        };
        let mut s = State::default();
        s.teammates = vec![mate("wren"), mate("atlas"), mate("sable")];
        s.handoff_glances.read(&HashMap::new(), 0.0);
        s.handoff_glances.read(&HashMap::from([message("msg_1", 1, "wren", "sable")]), 0.0);
        let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        // The wide sidebar's roster is the faces strip, a row (Sidebar.tsx `stripGlances =
        // glancesAmong(shownFaces, handoffs, 'row')`), so the two look along it.
        let aim = |id: &str| rt.state.bot_stage.pointer_target(&format!("roster-{id}"));
        assert_eq!(aim("wren"), Some((1.0, 0.0, 1.0)), "wren looks along the row at sable");
        assert_eq!(aim("sable"), Some((-1.0, 0.0, 1.0)), "sable looks back along the row at wren");
        assert_eq!(aim("atlas"), Some((0.0, 0.0, 0.0)), "atlas, between them, looks at nobody");
    }
}
