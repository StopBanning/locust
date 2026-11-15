//! A live run the conversation list was read too early to know.
//!
//! The engine counts a run as running (`engine_api().running()`) a moment before it has written the run's owner and
//! the message it answers (`native.rs` `send`: the slot is claimed first, so two turns can never start on one
//! conversation). The window reads its list when the running turns change, so under load that one read could land in
//! the gap: the run had no teammate and no link to the message, and nothing read the list again while it ran. A relayed
//! run then never joined its exchange, and the Exchange strip had no Stop for it
//! (`relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run`, about 1 run in 10, 15 Nov).
//!
//! So while a live run has no owner in the list, the list is read again: once a second, and for at most ten seconds a
//! run (a run that never records an owner is not read for forever; its end reads the list anyway).

use std::collections::HashMap;

/// How often the list is read again while a live run is missing from it, in seconds.
pub(super) const AGAIN_EVERY_S: f64 = 1.0;
/// For how long after a run is first seen live, in seconds.
pub(super) const AT_MOST_S: f64 = 10.0;

#[derive(Debug)]
pub(super) struct UnlistedRuns {
    first_seen: HashMap<String, f64>,
    last_read: f64,
}

impl Default for UnlistedRuns {
    fn default() -> Self {
        UnlistedRuns { first_seen: HashMap::new(), last_read: f64::NEG_INFINITY }
    }
}

impl UnlistedRuns {
    /// Whether to read the list again now: some live run has no owner yet, inside its ten seconds, no read is under
    /// way, and the last such read was a second ago or more.
    pub fn read_again(&mut self, live: &[String], owners: &HashMap<String, String>, reading: bool, now_s: f64) -> bool {
        self.first_seen.retain(|id, _| live.contains(id));
        let mut missing = false;
        for id in live.iter().filter(|id| !owners.contains_key(*id)) {
            let first = *self.first_seen.entry(id.clone()).or_insert(now_s);
            missing |= now_s - first <= AT_MOST_S;
        }
        if !missing || reading || now_s - self.last_read < AGAIN_EVERY_S {
            return false;
        }
        self.last_read = now_s;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn a_live_run_with_no_owner_reads_the_list_again() {
        let mut u = UnlistedRuns::default();
        assert!(u.read_again(&live(&["m_pip"]), &HashMap::new(), false, 5.0));
    }

    #[test]
    fn a_live_run_whose_owner_is_listed_reads_nothing() {
        let mut u = UnlistedRuns::default();
        let owners = HashMap::from([("m_pip".to_owned(), "tm_pip".to_owned())]);
        assert!(!u.read_again(&live(&["m_pip"]), &owners, false, 5.0));
    }

    #[test]
    fn it_reads_again_once_a_second_not_on_every_poll() {
        let mut u = UnlistedRuns::default();
        let none = HashMap::new();
        assert!(u.read_again(&live(&["m_pip"]), &none, false, 5.0));
        assert!(!u.read_again(&live(&["m_pip"]), &none, false, 5.9));
        assert!(u.read_again(&live(&["m_pip"]), &none, false, 6.0));
    }

    #[test]
    fn it_waits_while_a_read_is_under_way() {
        let mut u = UnlistedRuns::default();
        assert!(!u.read_again(&live(&["m_pip"]), &HashMap::new(), true, 5.0));
        assert!(u.read_again(&live(&["m_pip"]), &HashMap::new(), false, 5.1));
    }

    #[test]
    fn it_gives_up_on_a_run_ten_seconds_after_it_was_first_seen() {
        let mut u = UnlistedRuns::default();
        let none = HashMap::new();
        assert!(u.read_again(&live(&["m_side"]), &none, false, 0.0));
        assert!(u.read_again(&live(&["m_side"]), &none, false, 10.0));
        assert!(!u.read_again(&live(&["m_side"]), &none, false, 10.5));
        assert!(!u.read_again(&live(&["m_side"]), &none, false, 11.5));
    }
}
