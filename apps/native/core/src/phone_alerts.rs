//! When your phone buzzes (Electron `main/phone-alerts.ts`, 0.827). A phone away from the desk hears a run waiting on
//! you while Locust's window is behind, by these rules:
//!
//! - only while Locust's window is behind another app or minimized;
//! - at most one buzz every [`MERGE_MS`]: what lands in between joins the next one, the most pressing first;
//! - approvals and questions always count as pressing; anything else, past [`DAILY_BUDGET`] a day, waits for the brief;
//! - quiet hours hold every buzz for the brief, approvals too unless the person lets them through at night;
//! - a Scout card that can wait a while, left untouched for [`ESCALATE_MS`], buzzes once.
//!
//! What a lock screen shows is who and what kind, never a command, a file's name or a message's words. Sending, the
//! clock and the timers are handed in, so the rules are tested where they are written.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use chrono::{Local, NaiveDate, TimeZone};

use crate::scout::log::ScoutHeldAlert;
use crate::scout::scheduler::ScoutScheduler;
use crate::scout::types::HeldWhy;

pub const MERGE_MS: i64 = 120_000;
pub const DAILY_BUDGET: u32 = 5;
pub const ESCALATE_MS: i64 = 4 * 60 * 60_000;
const HELD_KEPT: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Approval,
    Question,
    Limit,
    Scout,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Question => "question",
            Self::Limit => "limit",
            Self::Scout => "scout",
        }
    }

    /// The first rank is said first when buzzes are merged.
    fn rank(self) -> u8 {
        match self {
            Self::Approval => 0,
            Self::Question => 1,
            Self::Limit => 2,
            Self::Scout => 3,
        }
    }

    fn pressing(self) -> bool {
        matches!(self, Self::Approval | Self::Question)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneAlert {
    pub kind: AlertKind,
    pub title: String,
    pub body: String,
    /// Notifications with one tag replace each other on the phone.
    pub tag: String,
    /// An approval's or question's card (0.837): a chat app shows it with its buttons. Never sent to a phone.
    pub approval_id: Option<String>,
    /// Who asks, for that card. Never sent to a phone either.
    pub who: Option<String>,
}

impl PhoneAlert {
    fn plain(kind: AlertKind, title: String, body: &str, tag: &str) -> Self {
        Self { kind, title, body: body.to_owned(), tag: tag.to_owned(), approval_id: None, who: None }
    }
}

/// How a held alert is kept for the morning brief: Scout's own shape, so its `held_alerts` hook reads it as it is.
/// Electron's `HeldAlert` also keeps the alert's kind; Scout's shape has no field for it and nothing reads it.
pub type HeldAlert = ScoutHeldAlert;

/// A Scout card as the phone may name it: by the watch it came from, never by what the card says.
pub fn scout_alert_body(key: &str) -> &'static str {
    let kind = key.find(':').map_or("", |at| &key[..at]);
    match kind {
        "pr" => "One of your pull requests needs you.",
        "review" => "A review is asked of you.",
        "issue" => "There is a new issue.",
        "branch" => "A check failed on a branch.",
        "check" => "The folder’s check fails.",
        "comment" | "comment-answer" => "Someone asked Locust on GitHub.",
        "waiting" => "A teammate is waiting on you.",
        "failed" => "A teammate’s turn failed.",
        _ => "Scout found something for you.",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskKind {
    Approval,
    Question,
}

pub fn approval_alert(name: Option<&str>, kind: AskKind, approval_id: Option<&str>) -> PhoneAlert {
    let who = name.unwrap_or("A teammate");
    let mut alert = match kind {
        AskKind::Question => PhoneAlert::plain(
            AlertKind::Question,
            format!("{who} is asking you something"),
            "The turn waits for your answer.",
            "waiting",
        ),
        AskKind::Approval => PhoneAlert::plain(
            AlertKind::Approval,
            format!("{who} needs your approval"),
            "The turn is paused until you answer.",
            "waiting",
        ),
    };
    if let Some(id) = approval_id {
        alert.approval_id = Some(id.to_owned());
        alert.who = name.map(str::to_owned);
    }
    alert
}

/// A routine holds for an OK before its next step (0.840). Who, never the routine's name or its step's words.
pub fn routine_ok_alert(name: Option<&str>) -> PhoneAlert {
    PhoneAlert::plain(
        AlertKind::Approval,
        format!("{} waits for your OK", name.unwrap_or("A teammate")),
        "A routine’s next step starts when you go ahead.",
        "waiting",
    )
}

pub fn limit_alert(name: Option<&str>, runtime_name: &str) -> PhoneAlert {
    PhoneAlert::plain(
        AlertKind::Limit,
        format!("{} hit the {runtime_name} limit", name.unwrap_or("A teammate")),
        "Pick where the work carries on.",
        "limit",
    )
}

pub fn scout_alert(key: &str) -> PhoneAlert {
    let short: String = key.chars().take(40).collect();
    PhoneAlert::plain(AlertKind::Scout, "Scout".to_owned(), scout_alert_body(key), &format!("scout:{short}"))
}

/// Several alerts as one buzz: the most pressing says itself, and the rest are counted.
pub fn merged_alert(alerts: &[PhoneAlert]) -> PhoneAlert {
    let mut sorted = alerts.to_vec();
    sorted.sort_by_key(|alert| alert.kind.rank());
    let first = sorted.remove(0);
    if sorted.is_empty() {
        return first;
    }
    let more = sorted.len();
    PhoneAlert {
        body: format!("{} And {more} more thing{} for you in Locust.", first.body, if more == 1 { "" } else { "s" }),
        tag: "locust".to_owned(),
        ..first
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardUrgency {
    Now,
    Soon,
    Later,
}

#[derive(Clone, Debug)]
pub struct NewScoutCard {
    pub card_id: String,
    pub key: String,
    pub urgency: CardUrgency,
}

type Send_ = dyn Fn(&PhoneAlert, &[PhoneAlert]) + Send + Sync;
type Ask = dyn Fn() -> bool + Send + Sync;

pub struct PhoneAlertsOptions {
    /// Hands one buzz to the phones, and its parts to the chat apps that are on (0.837).
    pub send: Arc<Send_>,
    /// Whether the person is away from Locust's window: it is behind another app, or minimized.
    pub away: Arc<Ask>,
    /// Whether the moment `at_ms` is in the person's quiet hours.
    pub quiet: Arc<dyn Fn(i64) -> bool + Send + Sync>,
    /// Whether approvals and questions may buzz in quiet hours.
    pub approvals_at_night: Option<Arc<Ask>>,
    /// Whether a Scout card is still waiting, untouched: neither started nor dismissed.
    pub card_open: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    pub now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub scheduler: Arc<dyn ScoutScheduler>,
}

#[derive(Default)]
struct AlertsState {
    last_sent: Option<i64>,
    waiting: Vec<PhoneAlert>,
    flush_at: Option<u64>,
    spent_day: Option<NaiveDate>,
    spent: u32,
    held: Vec<HeldAlert>,
    escalations: HashMap<String, u64>,
}

pub struct PhoneAlerts {
    options: PhoneAlertsOptions,
    state: Mutex<AlertsState>,
    me: Weak<PhoneAlerts>,
}

fn local_day(at_ms: i64) -> Option<NaiveDate> {
    Local.timestamp_millis_opt(at_ms).single().map(|at| at.date_naive())
}

impl PhoneAlerts {
    pub fn new(options: PhoneAlertsOptions) -> Arc<Self> {
        Arc::new_cyclic(|me| Self { options, state: Mutex::new(AlertsState::default()), me: me.clone() })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AlertsState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes what waits and marks the buzz as sent; the caller sends it with no lock held.
    fn take_flush(&self, state: &mut AlertsState) -> Option<Vec<PhoneAlert>> {
        state.flush_at = None;
        if state.waiting.is_empty() {
            return None;
        }
        state.last_sent = Some((self.options.now_ms)());
        Some(std::mem::take(&mut state.waiting))
    }

    fn send(&self, said: Option<Vec<PhoneAlert>>) {
        if let Some(said) = said {
            (self.options.send)(&merged_alert(&said), &said);
        }
    }

    fn flush(&self) {
        let said = {
            let mut state = self.lock();
            self.take_flush(&mut state)
        };
        self.send(said);
    }

    /// The one door every alert goes through.
    fn raise(&self, alert: PhoneAlert, about: Option<&str>) {
        if !(self.options.away)() {
            return;
        }
        let at = (self.options.now_ms)();
        let night = self.options.approvals_at_night.as_ref().is_some_and(|ask| ask());
        let quiet = (self.options.quiet)(at) && !(alert.kind.pressing() && night);
        let said = {
            let mut state = self.lock();
            let hold = |state: &mut AlertsState, why: HeldWhy| {
                state.held.push(HeldAlert { at, title: alert.title.clone(), why, about: about.map(str::to_owned) });
                if state.held.len() > HELD_KEPT {
                    let over = state.held.len() - HELD_KEPT;
                    state.held.drain(..over);
                }
            };
            if quiet {
                hold(&mut state, HeldWhy::QuietHours);
                return;
            }
            if !alert.kind.pressing() {
                let day = local_day(at);
                if state.spent_day != day {
                    state.spent_day = day;
                    state.spent = 0;
                }
                if state.spent >= DAILY_BUDGET {
                    hold(&mut state, HeldWhy::DailyLimit);
                    return;
                }
                state.spent += 1;
            }
            state.waiting.push(alert);
            if state.flush_at.is_some() {
                return;
            }
            let wait = state.last_sent.map_or(0, |last| last + MERGE_MS - at);
            if wait <= 0 {
                self.take_flush(&mut state)
            } else {
                let me = self.me.clone();
                let handle = self.options.scheduler.later(
                    wait as u64,
                    Box::new(move || {
                        if let Some(me) = me.upgrade() {
                            me.flush();
                        }
                    }),
                );
                state.flush_at = Some(handle);
                None
            }
        };
        self.send(said);
    }

    pub fn approval(&self, name: Option<&str>, kind: AskKind, approval_id: Option<&str>) {
        self.raise(approval_alert(name, kind, approval_id), None);
    }

    pub fn limit(&self, name: Option<&str>, runtime_name: &str) {
        self.raise(limit_alert(name, runtime_name), None);
    }

    pub fn routine_ok(&self, name: Option<&str>) {
        self.raise(routine_ok_alert(name), None);
    }

    /// New Scout cards: one that can't wait buzzes now; one that can wait a while buzzes once if it is left untouched.
    pub fn scout_cards(&self, cards: &[NewScoutCard]) {
        for card in cards {
            match card.urgency {
                CardUrgency::Now => self.raise(scout_alert(&card.key), Some(&card.key)),
                CardUrgency::Soon => {
                    if self.options.card_open.is_none() || self.lock().escalations.contains_key(&card.card_id) {
                        continue;
                    }
                    let me = self.me.clone();
                    let (card_id, key) = (card.card_id.clone(), card.key.clone());
                    let handle = self.options.scheduler.later(
                        ESCALATE_MS as u64,
                        Box::new(move || {
                            let Some(me) = me.upgrade() else { return };
                            me.lock().escalations.remove(&card_id);
                            let open = me.options.card_open.as_ref().is_some_and(|open| open(&card_id));
                            if open {
                                me.raise(scout_alert(&key), Some(&key));
                            }
                        }),
                    );
                    self.lock().escalations.insert(card.card_id.clone(), handle);
                }
                CardUrgency::Later => {}
            }
        }
    }

    /// What was held since `since_ms`, for the morning brief.
    pub fn held_since(&self, since_ms: i64) -> Vec<HeldAlert> {
        self.lock().held.iter().filter(|held| held.at >= since_ms).cloned().collect()
    }

    pub fn dispose(&self) {
        let (flush, escalations) = {
            let mut state = self.lock();
            (state.flush_at.take(), std::mem::take(&mut state.escalations))
        };
        if let Some(handle) = flush {
            self.options.scheduler.cancel(handle);
        }
        for handle in escalations.into_values() {
            self.options.scheduler.cancel(handle);
        }
    }
}

#[cfg(test)]
#[path = "phone_alerts_tests.rs"]
mod tests;
