//! Electron's `a-run-end-remembers-first-and-keeps-going.test.ts` (4 cases): a run's end runs in order, memory first and
//! once, and one step failing never skips the steps after it.
use super::*;
use std::cell::RefCell;

struct Chain {
    order: RefCell<Vec<&'static str>>,
    notes: RefCell<Vec<String>>,
    failing: Option<&'static str>,
}
impl Chain {
    fn new(failing: Option<&'static str>) -> Self {
        Self {
            order: RefCell::new(Vec::new()),
            notes: RefCell::new(Vec::new()),
            failing,
        }
    }
    fn step(&self, label: &'static str) -> impl FnOnce() -> Result<(), String> + '_ {
        move || {
            self.order.borrow_mut().push(label);
            if self.failing == Some(label) {
                Err(format!("{label} broke"))
            } else {
                Ok(())
            }
        }
    }
    fn end(&self) -> RunEnd<'_> {
        RunEnd::new("mission_1", |line| self.notes.borrow_mut().push(line))
    }
    fn shared(&self, end: &mut RunEnd<'_>) {
        end.shared(self.step("memory"), self.step("relay share"));
    }
    fn ended(&self, end: &mut RunEnd<'_>) {
        end.ended(
            self.step("memory"),
            self.step("relay"),
            vec![
                ("routine", Box::new(self.step("routine"))),
                ("room tasks", Box::new(self.step("room tasks"))),
                ("attention", Box::new(self.step("attention"))),
            ],
        );
    }
    fn order(&self) -> Vec<&'static str> {
        self.order.borrow().clone()
    }
}

// case: remembers before the relay starts anyone, from a share
#[test]
fn remembers_before_the_relay_starts_anyone_from_a_share() {
    let c = Chain::new(None);
    let mut end = c.end();
    c.shared(&mut end);
    assert_eq!(c.order(), ["memory", "relay share"]);
}

// case: remembers once, whether the share or the end came first
#[test]
fn remembers_once_whether_the_share_or_the_end_came_first() {
    let c = Chain::new(None);
    let mut end = c.end();
    c.shared(&mut end);
    c.ended(&mut end);
    assert_eq!(c.order().iter().filter(|l| **l == "memory").count(), 1);
    assert_eq!(
        c.order(),
        [
            "memory",
            "relay share",
            "relay",
            "routine",
            "room tasks",
            "attention"
        ]
    );
    let end_first = Chain::new(None);
    let mut end = end_first.end();
    end_first.ended(&mut end);
    end_first.shared(&mut end);
    assert_eq!(end_first.order(), ["memory", "relay", "routine", "room tasks", "attention", "relay share"]);
    // The end alone remembers too, first; and remembering where the reply is read leaves the end nothing to do again.
    let alone = Chain::new(None);
    let mut end = alone.end();
    alone.ended(&mut end);
    assert_eq!(
        alone.order(),
        ["memory", "relay", "routine", "room tasks", "attention"]
    );
    let early = Chain::new(None);
    let mut end = early.end();
    end.remember(early.step("memory"));
    early.shared(&mut end);
    early.ended(&mut end);
    assert_eq!(
        early.order(),
        [
            "memory",
            "relay share",
            "relay",
            "routine",
            "room tasks",
            "attention"
        ]
    );
}

// case: runs every step when one throws, and says which failed
#[test]
fn runs_every_step_when_one_throws_and_says_which_failed() {
    let c = Chain::new(Some("relay"));
    let mut end = c.end();
    c.ended(&mut end);
    drop(end);
    assert_eq!(
        c.order(),
        ["memory", "relay", "routine", "room tasks", "attention"]
    );
    assert_eq!(
        *c.notes.borrow(),
        ["relay failed for mission_1: relay broke"]
    );
    // A step after the relay failing is named too, and the rest still run.
    let later = Chain::new(Some("routine"));
    let mut end = later.end();
    later.ended(&mut end);
    drop(end);
    assert_eq!(
        later.order(),
        ["memory", "relay", "routine", "room tasks", "attention"]
    );
    assert_eq!(
        *later.notes.borrow(),
        ["routine failed for mission_1: routine broke"]
    );
}

// case: still starts the recipient when remembering fails
#[test]
fn still_starts_the_recipient_when_remembering_fails() {
    let c = Chain::new(Some("memory"));
    let mut end = c.end();
    c.shared(&mut end);
    drop(end);
    assert_eq!(c.order(), ["memory", "relay share"]);
    assert_eq!(
        *c.notes.borrow(),
        ["memory failed for mission_1: memory broke"]
    );
}
