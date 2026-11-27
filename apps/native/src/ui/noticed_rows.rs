//! Home's "Scout noticed" notices as one row each (Big Brother's review of 26 Nov, change 4,
//! `docs/REVIEW-ui-changes-2026-11-26.md`, which Colin approved as an improvement on Electron's `ScoutCards.tsx:116-117`
//! and `:157`): what happened as the title, its age, then Open and Dismiss together on the right. A notice that repeats
//! folds into one row, and a limit notice goes once that limit has reset.
//!
//! Only a notice that is just words and an Open is a row. One that needs the person now, or carries a draft, a post, a
//! fix, a take or a tune-up, keeps its card: a row has nowhere to put those buttons.

use super::*;
use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};

/// A notice that is only words and an Open: it can be a row. Its detail and chips (a failed turn's "It was asked: ..." and
/// its "failed" tag) have no place in a row; the Open it carries leads to the conversation that has them.
pub(super) fn is_row(card: &ScoutCard) -> bool {
    let fact = &card.fact;
    fact.urgency != ScoutUrgency::Now
        && fact.draft.is_none()
        && fact.post.is_none()
        && fact.tune.is_none()
        && fact.comment_ask.is_none()
        && fact.lines.as_ref().is_none_or(Vec::is_empty)
        && card.fix.is_none()
        && card.take.is_none()
        && !card.reading
}

/// A turn that failed on a usage limit: whose, which window, and when it resets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LimitNotice {
    /// "Wren", from "Wren's turn failed: ..."; none when the card names nobody.
    pub who: Option<String>,
    /// "5-hour", "weekly": the window that ran out, as the run said it; none when it did not say.
    pub window: Option<String>,
    pub resets: Option<DateTime<Utc>>,
}

/// The notice's limit, when the turn failed on one ("You've hit your 5-hour limit · resets 8:30pm", "5-hour limit
/// reached", "Usage limit reached").
pub(super) fn limit_of(card: &ScoutCard) -> Option<LimitNotice> {
    let said = card.fact.said.as_str();
    locust_core::invariants::regex(r"(?i)\b(?:usage|rate) limit\b|\b(?:\d+-(?:hour|day)|weekly|daily|monthly) (?:usage )?limit\b|\b(?:hit|reached) (?:your|its|the) limit\b").find(said)?;
    let window = locust_core::invariants::regex(r"(?i)\b(\d+-(?:hour|day)|weekly|daily|monthly) (?:usage )?limit").captures(said).and_then(|c| c.get(1)).map(|m| m.as_str().to_lowercase());
    let who = locust_core::invariants::regex(r"^(.+?)['\u{2019}]s turn failed").captures(said).and_then(|c| c.get(1)).map(|m| m.as_str().to_owned());
    let at = DateTime::parse_from_rfc3339(&card.at).ok().map(|t| t.with_timezone(&Utc))?;
    Some(LimitNotice { who, window, resets: reset_of(said, at) })
}

/// When the limit the sentence names resets: an instant, or the next time the clock reads what it says ("resets 8:30pm",
/// "resets Wed 8:30 PM") after the notice was written. None when the sentence gives no time.
pub(super) fn reset_of(said: &str, written: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if let Some(iso) = locust_core::invariants::regex(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z").find(said) {
        return DateTime::parse_from_rfc3339(iso.as_str()).ok().map(|t| t.with_timezone(&Utc));
    }
    let c = locust_core::invariants::regex(r"(?i)resets (?:(mon|tue|wed|thu|fri|sat|sun)[a-z]* )?(\d{1,2})(?::(\d{2}))? ?([ap])\.?m").captures(said)?;
    let hour12: u32 = c[2].parse().ok().filter(|h| (1..=12).contains(h))?;
    let minute: u32 = c.get(3).map_or(Some(0), |m| m.as_str().parse().ok().filter(|m| *m < 60))?;
    let pm = c[4].eq_ignore_ascii_case("p");
    let hour = hour12 % 12 + if pm { 12 } else { 0 };
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    let from = written.with_timezone(&Local);
    let weekday = c.get(1).map(|d| d.as_str().to_lowercase());
    for days in 0..=7 {
        let date = from.date_naive() + Duration::days(days);
        let local = Local.from_local_datetime(&date.and_time(time)).earliest()?;
        if local.with_timezone(&Utc) <= written {
            continue;
        }
        if let Some(day) = &weekday
            && !local.format("%a").to_string().to_lowercase().starts_with(day.as_str())
        {
            continue;
        }
        return Some(local.with_timezone(&Utc));
    }
    None
}

/// A limit that has reset is no longer news (review change 4).
pub(super) fn is_over(card: &ScoutCard, now: DateTime<Utc>) -> bool {
    limit_of(card).and_then(|l| l.resets).is_some_and(|resets| resets <= now)
}

/// One row: the notices that say the same thing, newest first.
pub(super) struct Notice<'a> {
    pub cards: Vec<&'a ScoutCard>,
}

impl Notice<'_> {
    pub(super) fn newest(&self) -> &ScoutCard {
        self.cards[0]
    }

    /// What happened, as the title: "Wren's turn hit the 5-hour limit", or the notice's own words.
    pub(super) fn title(&self) -> String {
        let Some(limit) = limit_of(self.newest()) else { return self.newest().fact.said.clone() };
        let one_teammate = self.cards.iter().all(|c| limit_of(c).is_some_and(|l| l.who == limit.who));
        let subject = match (&limit.who, one_teammate) {
            (Some(who), true) => format!("{who}\u{2019}s turn"),
            (None, true) => "A turn".to_owned(),
            (_, false) => "Turns".to_owned(),
        };
        match &limit.window {
            Some(window) => format!("{subject} hit the {window} limit"),
            None => format!("{subject} hit the usage limit"),
        }
    }
}

/// What makes two notices the same: the same limit (a limit is the account's, so two teammates hitting one window that
/// resets together are one notice; with no reset time, the same teammate and window), or the same words.
fn fold_key(card: &ScoutCard) -> String {
    match limit_of(card) {
        Some(limit) => match limit.resets {
            Some(resets) => format!("limit:{}:{}", limit.window.unwrap_or_default(), resets.timestamp() / 60),
            None => format!("limit:{}:{}", limit.window.unwrap_or_default(), limit.who.unwrap_or_default()),
        },
        None => card.fact.said.trim().to_owned(),
    }
}

/// What Home lists, in order: a card in full, or a row for one or several notices.
pub(super) enum Entry<'a> {
    Card(&'a ScoutCard),
    Row(Notice<'a>),
}

impl Entry<'_> {
    pub(super) fn holds(&self, card_id: &str) -> bool {
        match self {
            Entry::Card(card) => card.card_id == card_id,
            Entry::Row(notice) => notice.cards.iter().any(|c| c.card_id == card_id),
        }
    }
}

/// The cards Home lists, what needs the person first (`view::ordered_cards`), without the limit notices whose limit has
/// reset.
pub(super) fn ordered(cards: &[ScoutCard], now: DateTime<Utc>) -> Vec<&ScoutCard> {
    let live = |c: &&ScoutCard| !is_row(c) || !is_over(c, now);
    let mut out: Vec<&ScoutCard> = cards.iter().filter(|c| c.fact.urgency == ScoutUrgency::Now).filter(live).collect();
    out.extend(cards.iter().filter(|c| c.fact.urgency != ScoutUrgency::Now).filter(live));
    out
}

/// The first three entries, or all of them; one the person was brought to (`focus`) goes first when it is past the three.
pub(super) fn visible<'a>(all: Vec<Entry<'a>>, show_all: bool, focus: Option<&str>) -> (Vec<Entry<'a>>, usize) {
    let total = all.len();
    if show_all {
        return (all, total);
    }
    let past = focus.and_then(|id| all.iter().position(|e| e.holds(id))).filter(|at| *at >= SCOUT_SHOWN_CARDS);
    let mut shown: Vec<Entry<'a>> = Vec::new();
    let mut brought = None;
    for (at, entry) in all.into_iter().enumerate() {
        if Some(at) == past {
            brought = Some(entry);
        } else if at < SCOUT_SHOWN_CARDS {
            shown.push(entry);
        }
    }
    if let Some(entry) = brought {
        shown.insert(0, entry);
    }
    (shown, total)
}

/// `ordered` is the cards as Home orders them (what needs the person first). Over limits are dropped, repeats fold into
/// the row of the first of them.
pub(super) fn entries<'a>(ordered: &[&'a ScoutCard], now: DateTime<Utc>) -> Vec<Entry<'a>> {
    let mut out: Vec<Entry<'a>> = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    for &card in ordered {
        if !is_row(card) {
            out.push(Entry::Card(card));
            keys.push(None);
            continue;
        }
        if is_over(card, now) {
            continue;
        }
        let key = fold_key(card);
        match keys.iter().position(|k| k.as_deref() == Some(key.as_str())) {
            Some(at) => {
                if let Entry::Row(notice) = &mut out[at] {
                    notice.cards.push(card);
                    notice.cards.sort_by(|a, b| b.at.cmp(&a.at));
                }
            }
            None => {
                out.push(Entry::Row(Notice { cards: vec![card] }));
                keys.push(Some(key));
            }
        }
    }
    out
}

/// The row: title, age, how many times, then Open and Dismiss together on the right.
pub(super) fn row_view(notice: &Notice<'_>, focused: bool, now_ms: i64, l: &Look) -> El {
    let card = notice.newest();
    let age = locust_core::scout::view::ago(&card.at, now_ms);
    let mut line = row()
        .key(format!("scout-card-{}", card.card_id))
        .w_full()
        .gap(10.0)
        .center_y()
        .py(8.0)
        .px(12.0)
        .radius(8.0)
        .fill(l.c("bg-raised"))
        .border(if focused { 1.5 } else { 1.0 }, if focused { l.c("amber-border") } else { l.c("border-card") })
        .child(text(notice.title()).key(format!("scout-row-title-{}", card.card_id)).font(l.ty.control).color(l.c("text-primary")).grow(1.0).shrink().min_w(0.0).wrap());
    if !age.is_empty() {
        line = line.child(text(age).key(format!("scout-row-age-{}", card.card_id)).font(type_::LABEL).color(l.c("text-muted")));
    }
    if notice.cards.len() > 1 {
        line = line.child(text(format!("{}\u{d7}", notice.cards.len())).key(format!("scout-row-count-{}", card.card_id)).font(type_::LABEL).color(l.c("text-muted")));
    }
    if let Some(open) = card.fact.open.clone() {
        let label = match &open {
            ScoutOpen::Url { label, .. } | ScoutOpen::Conversation { label, .. } => label.clone(),
        };
        line = line.child(
            controls::button(l, &label, Tone::Neutral, false)
                .role("button")
                .label(label)
                .key(format!("scout-open-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| super::open_target(s, open.clone(), tx)),
        );
    }
    let ids: Vec<String> = notice.cards.iter().map(|c| c.card_id.clone()).collect();
    line.child(
        controls::ghost_button(l, "Dismiss")
            .role("button")
            .label("Dismiss")
            .key(format!("scout-dismiss-{}", card.card_id))
            .on_press(move |s: &mut State| super::dismiss_cards(s, ids.clone(), None)),
    )
}

#[cfg(test)]
#[path = "noticed_rows_tests.rs"]
mod noticed_rows_tests;
