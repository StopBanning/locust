//! Electron's `renderer/src/a-pull-link-is-read-once.test.ts` (4 of 4), against `LinkWatch`, the state Rust's window
//! keeps for the pull request links a conversation shows (`ui/pull_request_chip.rs::poll` starts the reads it asks for).
//! Shown links share one read. A minute later the window coming forward reads again. A timer runs only while checks
//! are still running, and a link that has left the screen is not read.
use super::*;

fn address() -> PullLinkAddress {
    PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 42 }
}

fn passing() -> LinkedPullRequest {
    LinkedPullRequest { state: PullRequestState::Open, checks: PullLinkChecks::Passing, passed: 2, failed: 0, running: 0, total: 2 }
}

fn running() -> LinkedPullRequest {
    LinkedPullRequest { state: PullRequestState::Open, checks: PullLinkChecks::Running, passed: 3, failed: 0, running: 1, total: 4 }
}

/// A watch and a gh that answers `answer`, counting each read the watch asks for, as Electron's harness does.
struct Harness {
    watch: LinkWatch<LinkedPullRequest>,
    answer: Option<LinkedPullRequest>,
    reads: usize,
    time: u64,
}

impl Harness {
    fn new(answer: Option<LinkedPullRequest>) -> Self {
        Self { watch: LinkWatch::default(), answer, reads: 0, time: 1_000 }
    }

    /// Starts each read asked for and lets it finish, as Electron's `flush` does.
    fn read(&mut self, asked: Vec<PullLinkAddress>) {
        for a in asked {
            self.reads += 1;
            let answer = self.answer.clone();
            self.watch.finish_read(&a, answer, self.time, is_pull_link_moving);
        }
    }

    fn watch(&mut self) -> u64 {
        let (id, read) = self.watch.watch(&address(), self.time, is_pull_link_moving);
        if read {
            self.read(vec![address()]);
        }
        id
    }

    fn focus(&mut self) {
        let asked = self.watch.on_focus(self.time);
        self.read(asked);
    }

    fn tick(&mut self) {
        let asked = self.watch.on_tick(is_pull_link_moving);
        self.read(asked);
    }

    fn show(&mut self, drawn: &[PullLinkAddress]) {
        let asked = self.watch.show(drawn, self.time, is_pull_link_moving);
        self.read(asked);
    }
}

#[test]
fn reads_once_for_many_links_and_again_when_the_window_comes_back_after_a_minute() {
    // case: reads once for many links, and again when the window comes back after a minute
    let mut h = Harness::new(Some(passing()));
    // Three watchers asking while the first read is still out: one read, shared.
    let (a, first) = h.watch.watch(&address(), h.time, is_pull_link_moving);
    let (b, second) = h.watch.watch(&address(), h.time, is_pull_link_moving);
    let (c, third) = h.watch.watch(&address(), h.time, is_pull_link_moving);
    assert_eq!((first, second, third), (true, false, false), "only the first watcher starts a read");
    h.read(vec![address()]);
    assert_eq!(h.reads, 1);
    assert_eq!(h.watch.current(&address()), Some(&passing()));
    h.focus();
    assert_eq!(h.reads, 1, "the window coming back within the minute reads nothing");
    h.watch();
    assert_eq!(h.reads, 1, "a fourth link within the minute reads nothing");
    h.time += 60_000;
    h.focus();
    assert_eq!(h.reads, 2, "a minute later, the window coming back reads again");
    // The window's own way in: two chips of one link drawn together hold one listener and ask for one read.
    let mut w = Harness::new(Some(passing()));
    w.show(&[address(), address()]);
    assert_eq!(w.reads, 1);
    w.show(&[address()]);
    assert_eq!(w.reads, 1, "a link still drawn is not read again on the next frame");
    for id in [a, b, c] {
        h.watch.unwatch(&address(), id, is_pull_link_moving);
    }
}

#[test]
fn does_not_keep_a_timer_once_the_checks_have_settled() {
    // case: does not keep a timer once the checks have settled
    let mut h = Harness::new(Some(passing()));
    let id = h.watch();
    assert!(!h.watch.timer_armed());
    assert!(h.watch.focus_armed());
    h.watch.unwatch(&address(), id, is_pull_link_moving);
    assert!(!h.watch.focus_armed());
    // Through the window: a settled link drawn arms no timer, and leaving the screen drops the focus read.
    let mut w = Harness::new(Some(passing()));
    w.show(&[address()]);
    assert!(!w.watch.timer_armed() && w.watch.focus_armed());
    w.show(&[]);
    assert!(!w.watch.focus_armed());
}

#[test]
fn reads_every_few_minutes_only_while_checks_are_running_and_stops_when_the_link_is_gone() {
    // case: reads every few minutes only while checks are running, and stops when the link is gone
    let mut h = Harness::new(Some(running()));
    h.show(&[address()]);
    assert_eq!(h.reads, 1);
    assert!(h.watch.timer_armed());
    assert_eq!(h.watch.again_ms(), 180_000);
    h.tick();
    assert_eq!(h.reads, 2, "the timer reads a link whose checks are running, whatever its age");
    // The link leaves the screen: no timer, no focus read, and a tick that fires anyway reads nothing.
    h.show(&[]);
    assert!(!h.watch.timer_armed());
    assert!(!h.watch.focus_armed());
    h.tick();
    h.time += 60_000;
    h.focus();
    assert_eq!(h.reads, 2);
    // A read the timer asks for while one is already out is not asked twice.
    let mut w = Harness::new(Some(running()));
    w.show(&[address()]);
    assert_eq!(w.watch.on_tick(is_pull_link_moving), vec![address()]);
    assert!(w.watch.on_tick(is_pull_link_moving).is_empty(), "a tick while the read is out asks for nothing");
}

#[test]
fn reads_nothing_when_gh_cannot() {
    // case: reads nothing when gh cannot
    let mut h = Harness::new(None);
    h.show(&[address()]);
    assert_eq!(h.reads, 1);
    assert!(h.watch.current(&address()).is_none());
    // The failed read still counts as the read: nothing is asked again within the minute, and no timer runs.
    h.show(&[address()]);
    h.focus();
    assert_eq!(h.reads, 1);
    assert!(!h.watch.timer_armed());
}
