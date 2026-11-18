//! Your phone buzzes only when it must (Electron `main/your-phone-buzzes-only-when-it-must.test.ts`, 0.827), one test
//! per Electron case with Electron's values.

use super::*;
use crate::scout::scheduler::FakeScheduler;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

type Heard = Arc<Mutex<Vec<(PhoneAlert, Vec<PhoneAlert>)>>>;

struct Harness {
    alerts: Arc<PhoneAlerts>,
    heard: Heard,
    clock: Arc<FakeScheduler>,
    away: Arc<AtomicBool>,
    quiet: Arc<AtomicBool>,
    night: Arc<AtomicBool>,
    open: Arc<Mutex<HashSet<String>>>,
}

impl Harness {
    fn sent(&self) -> Vec<PhoneAlert> {
        self.heard.lock().unwrap().iter().map(|(alert, _)| alert.clone()).collect()
    }

    fn tick(&self, ms: i64) {
        self.clock.advance_ms(ms as u64);
    }
}

fn start() -> i64 {
    Local.with_ymd_and_hms(2026, 10, 23, 14, 0, 0).single().unwrap().timestamp_millis()
}

fn harness_with(card_open: bool, night: bool) -> Harness {
    let clock = FakeScheduler::new(start());
    let heard: Heard = Arc::new(Mutex::new(Vec::new()));
    let away = Arc::new(AtomicBool::new(true));
    let quiet = Arc::new(AtomicBool::new(false));
    let at_night = Arc::new(AtomicBool::new(false));
    let open = Arc::new(Mutex::new(HashSet::new()));
    let alerts = PhoneAlerts::new(PhoneAlertsOptions {
        send: {
            let heard = heard.clone();
            Arc::new(move |alert: &PhoneAlert, parts: &[PhoneAlert]| {
                heard.lock().unwrap().push((alert.clone(), parts.to_vec()))
            })
        },
        away: {
            let away = away.clone();
            Arc::new(move || away.load(Ordering::SeqCst))
        },
        quiet: {
            let quiet = quiet.clone();
            Arc::new(move |_at| quiet.load(Ordering::SeqCst))
        },
        approvals_at_night: night.then(|| {
            let at_night = at_night.clone();
            Arc::new(move || at_night.load(Ordering::SeqCst)) as Arc<Ask>
        }),
        card_open: card_open.then(|| {
            let open = open.clone();
            Arc::new(move |id: &str| open.lock().unwrap().contains(id)) as Arc<dyn Fn(&str) -> bool + Send + Sync>
        }),
        now_ms: {
            let clock = clock.clone();
            Arc::new(move || clock.now_ms())
        },
        scheduler: clock.clone(),
    });
    Harness { alerts, heard, clock, away, quiet, night: at_night, open }
}

fn harness() -> Harness {
    harness_with(true, true)
}

fn card(id: &str, key: &str, urgency: CardUrgency) -> NewScoutCard {
    NewScoutCard { card_id: id.to_owned(), key: key.to_owned(), urgency }
}

fn alert(kind: AlertKind, title: &str, body: &str, tag: &str) -> PhoneAlert {
    PhoneAlert {
        kind,
        title: title.to_owned(),
        body: body.to_owned(),
        tag: tag.to_owned(),
        approval_id: None,
        who: None,
    }
}

#[test]
fn buzzes_for_an_approval_at_once_naming_who_and_what_kind_never_the_command() {
    let h = harness();
    h.alerts.approval(Some("Mira"), AskKind::Approval, None);
    assert_eq!(
        h.sent(),
        vec![alert(AlertKind::Approval, "Mira needs your approval", "The turn is paused until you answer.", "waiting")]
    );
}

#[test]
fn stays_quiet_while_locusts_window_is_in_front_the_window_shows_it() {
    let h = harness();
    h.away.store(false, Ordering::SeqCst);
    h.alerts.approval(Some("Mira"), AskKind::Approval, None);
    h.alerts.scout_cards(&[card("c1", "check:abc", CardUrgency::Now)]);
    assert_eq!(h.sent(), vec![]);
}

#[test]
fn buzzes_at_most_once_every_two_minutes_the_rest_joining_the_next_buzz_the_most_pressing_first() {
    let h = harness();
    h.alerts.scout_cards(&[card("c1", "check:abc", CardUrgency::Now)]);
    assert_eq!(h.sent().len(), 1);
    h.tick(10_000);
    h.alerts.limit(Some("Ivy"), "Codex");
    h.alerts.approval(Some("Mira"), AskKind::Approval, None);
    assert_eq!(h.sent().len(), 1);
    h.tick(MERGE_MS - 10_000);
    assert_eq!(h.sent().len(), 2);
    assert_eq!(
        h.sent()[1],
        alert(
            AlertKind::Approval,
            "Mira needs your approval",
            "The turn is paused until you answer. And 1 more thing for you in Locust.",
            "locust"
        )
    );
}

#[test]
fn spends_a_daily_budget_on_anything_but_approvals_and_questions_and_holds_the_rest_for_the_brief() {
    let h = harness();
    for i in 0..DAILY_BUDGET + 2 {
        h.alerts.scout_cards(&[card(&format!("c{i}"), "pr:x", CardUrgency::Now)]);
        h.tick(MERGE_MS);
    }
    assert_eq!(h.sent().len(), DAILY_BUDGET as usize);
    h.alerts.approval(Some("Mira"), AskKind::Question, None);
    h.tick(MERGE_MS);
    assert_eq!(h.sent().last().unwrap().title, "Mira is asking you something");
    let whys: Vec<HeldWhy> = h.alerts.held_since(0).iter().map(|held| held.why).collect();
    assert_eq!(whys, vec![HeldWhy::DailyLimit, HeldWhy::DailyLimit]);
}

#[test]
fn holds_every_buzz_in_quiet_hours_for_the_brief_approvals_too_unless_they_may_come_through_at_night() {
    let h = harness();
    h.quiet.store(true, Ordering::SeqCst);
    h.alerts.approval(Some("Mira"), AskKind::Approval, None);
    h.alerts.scout_cards(&[card("c1", "check:abc", CardUrgency::Now)]);
    assert_eq!(h.sent(), vec![]);
    let held: Vec<(String, HeldWhy)> =
        h.alerts.held_since(0).iter().map(|held| (held.title.clone(), held.why)).collect();
    assert_eq!(
        held,
        vec![
            ("Mira needs your approval".to_owned(), HeldWhy::QuietHours),
            ("Scout".to_owned(), HeldWhy::QuietHours)
        ]
    );
    h.night.store(true, Ordering::SeqCst);
    h.alerts.approval(Some("Mira"), AskKind::Approval, None);
    h.alerts.scout_cards(&[card("c2", "check:def", CardUrgency::Now)]);
    let kinds: Vec<AlertKind> = h.sent().iter().map(|alert| alert.kind).collect();
    assert_eq!(kinds, vec![AlertKind::Approval]);
}

#[test]
fn buzzes_once_for_a_card_that_could_wait_when_it_is_still_untouched_four_hours_later() {
    let h = harness();
    h.open.lock().unwrap().insert("c1".to_owned());
    h.alerts.scout_cards(&[card("c1", "review:x", CardUrgency::Soon), card("c2", "issue:y", CardUrgency::Soon)]);
    assert_eq!(h.sent(), vec![]);
    h.tick(ESCALATE_MS);
    // c2 was started or dismissed in the meantime: only c1 buzzes.
    assert_eq!(h.sent(), vec![alert(AlertKind::Scout, "Scout", "A review is asked of you.", "scout:review:x")]);
    h.tick(ESCALATE_MS);
    assert_eq!(h.sent().len(), 1);
}

#[test]
fn never_buzzes_for_a_card_that_can_wait_as_long_as_it_likes() {
    let h = harness();
    h.alerts.scout_cards(&[card("c1", "pr:x", CardUrgency::Later)]);
    h.tick(ESCALATE_MS * 2);
    assert_eq!(h.sent(), vec![]);
}

#[test]
fn hands_each_buzzs_parts_on_too_a_card_by_its_id_for_the_chat_apps_that_are_on() {
    // Electron's case hands in no approvalsAtNight and no cardOpen.
    let h = harness_with(false, false);
    h.alerts.limit(Some("Ivy"), "Codex");
    h.tick(10_000);
    h.alerts.approval(Some("Mira"), AskKind::Approval, Some("ap_1"));
    h.alerts.scout_cards(&[card("c1", "branch:main", CardUrgency::Now)]);
    h.tick(MERGE_MS);
    let heard = h.heard.lock().unwrap().clone();
    let parts: Vec<Vec<(AlertKind, Option<String>, Option<String>)>> = heard
        .iter()
        .map(|(_, parts)| parts.iter().map(|p| (p.kind, p.approval_id.clone(), p.who.clone())).collect())
        .collect();
    assert_eq!(
        parts,
        vec![
            vec![(AlertKind::Limit, None, None)],
            vec![
                (AlertKind::Approval, Some("ap_1".to_owned()), Some("Mira".to_owned())),
                (AlertKind::Scout, None, None)
            ],
        ]
    );
    // The phone's buzz is still who and what kind: the merged one leads with the card.
    assert_eq!(heard[1].0.title, "Mira needs your approval");
}

#[test]
fn names_a_scout_card_by_the_watch_it_came_from_never_by_its_words() {
    assert_eq!(scout_alert_body("pr:owner/repo#42:checks"), "One of your pull requests needs you.");
    assert_eq!(scout_alert_body("check:npm test"), "The folder’s check fails.");
    assert_eq!(scout_alert_body("comment:owner/repo:991"), "Someone asked Locust on GitHub.");
    assert_eq!(scout_alert_body("brief:2026-10-23"), "Scout found something for you.");
}
