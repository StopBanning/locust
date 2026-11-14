//! Home Scout surface (`ScoutCards.tsx`, `ScoutWatching`, `ScoutLooked`, `ScoutInvite`).
//!
//! Draws Scout's noticed cards, watching status, team-card look line + Look now eye button,
//! and the Meet Scout invite card/dialog. All disk reads and writes run through `ScoutHost`
//! off the UI thread via `Latest` jobs; `view` functions read only `&State`.

use super::*;
use crate::look::controls::{self, Look, Tone};
use crate::look::icons;
use locust_core::attention;
use locust_core::connectors_off::free_models_best_first;
use locust_core::scout::host::ScoutHost;
use locust_core::scout::types::{
    ScoutCard, ScoutChange, ScoutDraft, ScoutOpen, ScoutOutcome, ScoutState, ScoutTone,
    ScoutUrgency,
};
#[cfg(test)]
use locust_core::scout::types::ScoutSettings;
use locust_core::scout::view::{
    self, SCOUT_INVITE_LEDE, SCOUT_INVITE_NAME, SCOUT_INVITE_ROLE, SCOUT_INVITE_ROUTE,
    SCOUT_INVITE_TITLE, SCOUT_NOT_NOW_LABEL, SCOUT_SAW_PAGE, SCOUT_SHOWN_CARDS,
};
use locust_core::scout::{ScoutLogSnapshot, ScoutRoute, TurnOnOutcome};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) enum ActionOutcome {
    State(ScoutState),
    Looked(ScoutState),
    TurnedOn {
        outcome: TurnOnOutcome,
        roster: Option<Vec<Teammate>>,
    },
}

pub(super) struct Panel {
    pub host: Option<Arc<ScoutHost>>,
    pub host_root: Option<PathBuf>,
    pub started: bool,
    pub state: Option<ScoutState>,
    pub state_seq: u64,
    /// Optional fixed timestamp for deterministic rendering in unit tests.
    pub now_ms: Option<i64>,
    pub looking: bool,
    pub show_all: bool,
    pub focus_card_id: Option<String>,
    /// Per-window dismissal of the Home "Meet Scout" invite (`App.tsx:301`).
    pub not_now: bool,
    pub invite_open: bool,
    pub invite_busy: bool,
    pub invite_message: Option<String>,
    /// Settings > "What Scout saw": stays `None` until the person presses Show.
    pub log: Option<Result<ScoutLogSnapshot, String>>,
    pub log_loading: bool,
    pub log_open: bool,
    pub log_aside_only: bool,
    pub log_shown: usize,
    pub goal_draft: Option<TextEdit>,
    pub goal_pending: Option<String>,
    pub notes_draft: Option<TextEdit>,
    pub notes_pending: Option<String>,
    pub setting_busy: bool,
    pub setting_message: Option<String>,
    pub state_job: Latest<ScoutState>,
    pub action_job: Latest<ActionOutcome>,
    pub log_job: Latest<Result<ScoutLogSnapshot, String>>,
    pub opencode_job: Latest<Vec<engine::Models>>,
    pub opencode_requested: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            host: None,
            host_root: None,
            started: false,
            state: None,
            state_seq: 0,
            now_ms: None,
            looking: false,
            show_all: false,
            focus_card_id: None,
            not_now: false,
            invite_open: false,
            invite_busy: false,
            invite_message: None,
            log: None,
            log_loading: false,
            log_open: false,
            log_aside_only: false,
            log_shown: SCOUT_SAW_PAGE,
            goal_draft: None,
            goal_pending: None,
            notes_draft: None,
            notes_pending: None,
            setting_busy: false,
            setting_message: None,
            state_job: Latest::default(),
            action_job: Latest::default(),
            log_job: Latest::default(),
            opencode_job: Latest::default(),
            opencode_requested: false,
        }
    }
}

pub(super) fn setup_wakers(s: &mut State, waker: &rune::Waker) {
    s.scout.state_job.set_waker(waker.clone());
    s.scout.action_job.set_waker(waker.clone());
    s.scout.log_job.set_waker(waker.clone());
    s.scout.opencode_job.set_waker(waker.clone());
}

pub(super) fn effective_now_ms(s: &State) -> i64 {
    s.scout
        .now_ms
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
}

fn workspace_folder(s: &State) -> Option<PathBuf> {
    s.home
        .folder_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

pub(super) fn ensure_host(s: &mut State) -> Option<Arc<ScoutHost>> {
    let store = s.data.clone()?;
    if s.scout.host.is_some() && s.scout.host_root.as_deref() == Some(store.root()) {
        if let Some(host) = &s.scout.host {
            let folder = workspace_folder(s);
            if host.deps.workspace_folder() != folder {
                host.deps.set_folder(folder);
            }
        }
        return s.scout.host.clone();
    }
    let host = Arc::new(ScoutHost::start(store.clone()));
    host.deps.set_folder(workspace_folder(s));
    s.scout.host = Some(host.clone());
    s.scout.host_root = Some(store.root().to_path_buf());
    s.scout.state_seq = 0;
    s.scout.started = false;
    let load_host = host.clone();
    s.scout.state_job.request((), move |()| load_host.scout.state());
    Some(host)
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if s.data.is_some()
        && s.scout.host_root.as_deref() != s.data.as_ref().map(|d| d.root())
    {
        let _ = ensure_host(s);
    }
    if let Some(host) = &s.scout.host {
        let folder = workspace_folder(s);
        if host.deps.workspace_folder() != folder {
            host.deps.set_folder(folder);
        }
    }

    #[cfg(not(test))]
    if s.engine == Some(Kind::Native)
        && !s.scout.started
        && let Some(host) = s.scout.host.clone()
    {
        s.scout.started = true;
        std::thread::spawn(move || host.scout.start(15_000));
    }

    if !s.demo
        && s.engine == Some(Kind::Native)
        && (s.home.home_open || s.settings_panel.open)
        && !s.scout.opencode_requested
        && default_scout_route(s).is_none()
    {
        s.scout.opencode_requested = true;
        s.scout.opencode_job.request((), |()| {
            locust_core::door::list_models_here(None)
                .and_then(|text| engine::parse_models(&text))
                .unwrap_or_default()
        });
    }

    if let Some(models) = s.scout.opencode_job.take() {
        let opencode: Vec<engine::Models> = models
            .into_iter()
            .filter(|m| m.runtime == "opencode")
            .collect();
        if !opencode.is_empty() {
            s.models.insert("opencode".into(), opencode);
            changed = true;
        }
    }

    if let Some(st) = s.scout.state_job.take() {
        s.scout.state = Some(st);
        changed = true;
    }

    if let Some(outcome) = s.scout.action_job.take() {
        changed = true;
        match outcome {
            ActionOutcome::State(st) => {
                s.scout.setting_busy = false;
                if let Some(pending) = s.scout.goal_pending.take() {
                    if s.scout
                        .goal_draft
                        .as_ref()
                        .is_some_and(|e| e.text == pending)
                    {
                        s.scout.goal_draft = None;
                    }
                }
                if let Some(pending) = s.scout.notes_pending.take() {
                    if s.scout
                        .notes_draft
                        .as_ref()
                        .is_some_and(|e| e.text == pending)
                    {
                        s.scout.notes_draft = None;
                    }
                }
                s.scout.state = Some(st);
            }
            ActionOutcome::Looked(st) => {
                s.scout.looking = false;
                s.scout.setting_busy = false;
                s.scout.state = Some(st);
            }
            ActionOutcome::TurnedOn { outcome, roster } => {
                s.scout.setting_busy = false;
                s.scout.invite_busy = false;
                match outcome {
                    TurnOnOutcome::Ok(st) => {
                        s.scout.invite_open = false;
                        s.scout.invite_message = None;
                        s.scout.setting_message = None;
                        s.scout.state = Some(st);
                        if let Some(team) = roster {
                            super::apply_team(s, team);
                        }
                    }
                    TurnOnOutcome::Refused(why) => {
                        s.scout.invite_message = Some(why.clone());
                        s.scout.setting_message = Some(why);
                    }
                }
            }
        }
    }

    if let Some(res) = s.scout.log_job.take() {
        s.scout.log_loading = false;
        s.scout.log = Some(res);
        changed = true;
    }

    if let Some(host) = &s.scout.host {
        let seq = host.deps.state_seq();
        if seq != s.scout.state_seq {
            s.scout.state_seq = seq;
            if let Some(latest) = host.deps.latest_state() {
                s.scout.state = Some(latest);
                changed = true;
            }
        }
        let notices = host.deps.drain_notifications();
        if !notices.is_empty()
            && attention::should_notify(s.window_focused, crate::notify::supported())
        {
            for card in notices {
                let mission = match &card.fact.open {
                    Some(ScoutOpen::Conversation { mission_id, .. }) => Some(mission_id.as_str()),
                    _ => None,
                };
                crate::notify::show(
                    &card.fact.said,
                    card.fact.detail.as_deref().unwrap_or(""),
                    mission,
                );
            }
        }
    }

    changed
}

/// Best OpenCode free model available in `s.models` (`connectors_off::free_models_best_first`).
pub(super) fn default_scout_route(s: &State) -> Option<(ScoutRoute, String)> {
    let mut candidates = Vec::new();
    for list in s.models.values() {
        for group in list {
            if group.runtime == "opencode" {
                candidates.extend(group.models.iter().cloned());
            }
        }
    }
    let best = free_models_best_first(&candidates).into_iter().next()?;
    let label = if best.name.trim().is_empty() {
        best.id.clone()
    } else {
        best.name.clone()
    };
    Some((
        ScoutRoute {
            runtime: "opencode".into(),
            model: best.id.clone(),
            account: None,
        },
        label,
    ))
}

pub(super) fn request_refresh_state(s: &mut State) {
    if let Some(host) = ensure_host(s) {
        s.scout.state_job.request((), move |()| host.scout.state());
    }
}

pub(super) fn look_now(s: &mut State) {
    if s.scout.looking {
        return;
    }
    let Some(host) = ensure_host(s) else { return };
    s.scout.looking = true;
    s.scout
        .action_job
        .request((), move |()| ActionOutcome::Looked(host.scout.look()));
}

pub(super) fn turn_on_scout(s: &mut State, from_invite: bool) {
    if s.scout.invite_busy || s.scout.setting_busy {
        return;
    }
    let mut route = default_scout_route(s).map(|(r, _)| r);
    let can_lookup_models = !s.demo && s.engine == Some(Kind::Native);
    let Some(host) = ensure_host(s) else {
        let msg = "Native data folder could not be read.".to_owned();
        if from_invite {
            s.scout.invite_message = Some(msg);
        } else {
            s.scout.setting_message = Some(msg);
        }
        return;
    };
    if from_invite {
        s.scout.invite_busy = true;
        s.scout.invite_message = None;
    } else {
        s.scout.setting_busy = true;
        s.scout.setting_message = None;
    }
    let store = s.data.clone();
    s.scout.action_job.request((), move |()| {
        if route.is_none() && can_lookup_models {
            if let Ok(parsed) =
                locust_core::door::list_models_here(None).and_then(|t| engine::parse_models(&t))
            {
                let opencode: Vec<engine::Model> = parsed
                    .into_iter()
                    .filter(|m| m.runtime == "opencode")
                    .flat_map(|m| m.models)
                    .collect();
                if let Some(best) = free_models_best_first(&opencode).into_iter().next() {
                    route = Some(ScoutRoute {
                        runtime: "opencode".into(),
                        model: best.id.clone(),
                        account: None,
                    });
                }
            }
        }
        let outcome = host.scout.turn_on(route.as_ref());
        let roster = if matches!(outcome, TurnOnOutcome::Ok(_)) {
            store
                .as_ref()
                .and_then(|st| native::roster_in(st).ok())
        } else {
            None
        };
        ActionOutcome::TurnedOn { outcome, roster }
    });
}

pub(super) fn apply_change(s: &mut State, change: ScoutChange) {
    let Some(host) = ensure_host(s) else { return };
    s.scout.setting_busy = true;
    s.scout.setting_message = None;
    s.scout
        .action_job
        .request((), move |()| ActionOutcome::State(host.scout.set(&change)));
}

pub(super) fn save_notes(s: &mut State, text: String) {
    let Some(host) = ensure_host(s) else { return };
    s.scout.setting_busy = true;
    s.scout.setting_message = None;
    s.scout.action_job.request((), move |()| {
        ActionOutcome::State(host.scout.save_notes(&text))
    });
}

pub(super) fn load_log(s: &mut State) {
    let Some(host) = ensure_host(s) else {
        s.scout.log = Some(Err("Native data folder could not be read.".into()));
        return;
    };
    s.scout.log_loading = true;
    s.scout
        .log_job
        .request((), move |()| Ok(host.scout.log()));
}

pub(super) fn dismiss_card(s: &mut State, card_id: String, how: Option<ScoutOutcome>) {
    if let Some(host) = ensure_host(s) {
        s.scout.action_job.request((), move |()| {
            ActionOutcome::State(host.scout.dismiss(&card_id, how))
        });
    } else if let Some(st) = &mut s.scout.state {
        st.cards.retain(|c| c.card_id != card_id);
    }
}

pub(super) fn tune_card(s: &mut State, card_id: String) {
    if let Some(host) = ensure_host(s) {
        s.scout.action_job.request((), move |()| {
            ActionOutcome::State(host.scout.tune(&card_id))
        });
    }
}

pub(super) fn open_target(s: &mut State, open: ScoutOpen, tx: &mut Tx) {
    match open {
        ScoutOpen::Url { url, .. } => {
            crate::published_cards::open_web_link(s, &url);
        }
        ScoutOpen::Conversation { mission_id, .. } => {
            s.home.home_open = false;
            s.home.board_open = false;
            crate::peer_thread::open_mission_by_id(s, mission_id, tx);
        }
    }
}

pub(super) fn send_draft(s: &mut State, card_id: String, draft: ScoutDraft, tx: &mut Tx) {
    super::select_teammate(s, &draft.teammate_id, tx);
    super::fresh_conversation(s);
    s.draft.replace_text(&draft.message);
    super::send(s, tx);
    s.home.home_open = false;
    s.home.board_open = false;
    dismiss_card(s, card_id, Some(ScoutOutcome::Sent));
}

pub(super) fn draft_into_box(s: &mut State, card_id: String, draft: ScoutDraft, tx: &mut Tx) {
    super::select_teammate(s, &draft.teammate_id, tx);
    s.draft.replace_text(&draft.message);
    s.draft.focused = true;
    s.home.home_open = false;
    s.home.board_open = false;
    dismiss_card(s, card_id, Some(ScoutOutcome::Changed));
}

pub(super) fn on_team(s: &State, st: &ScoutState) -> bool {
    st.settings
        .teammate_id
        .as_deref()
        .is_some_and(|id| s.teammates.iter().any(|t| t.id == id))
}

/// Home's "Meet Scout" slot condition (`App.tsx:10018-10025`).
pub(super) fn invite_offer_active(s: &State) -> bool {
    let Some(st) = s.scout.state.as_ref() else {
        return false;
    };
    let has_team = !s.teammates.is_empty();
    let has_folder = workspace_folder(s).is_some();
    let has_default_model = default_scout_route(s).is_some();
    view::should_offer_scout_invite(
        &st.settings,
        s.scout.not_now,
        has_team,
        has_folder,
        has_default_model,
    )
}

/// Line replacing `mate.role` on Scout's Home team card when Scout is on, clear, and on the team.
pub(super) fn looked_line_for_teammate(s: &State, mate_id: &str) -> Option<(String, bool)> {
    let st = s.scout.state.as_ref()?;
    if !st.settings.on || !st.cards.is_empty() {
        return None;
    }
    if st.settings.teammate_id.as_deref() != Some(mate_id) {
        return None;
    }
    let line = view::looked_team_line(
        st.last_look_at.as_deref(),
        s.scout.looking,
        effective_now_ms(s),
    );
    Some((line, s.scout.looking))
}

fn chip_colors(tone: ScoutTone, l: &Look) -> (Color, Color, Color) {
    match tone {
        ScoutTone::Amber => (l.c("amber-text"), l.c("amber-tint"), l.c("amber-border")),
        ScoutTone::Red => (l.c("red-text"), l.c("red-tint"), l.c("red-border")),
        ScoutTone::Green => (l.c("green-text"), l.c("green-tint"), l.c("green-border")),
        ScoutTone::Blue => (l.c("blue-text"), l.c("blue-tint"), l.c("blue-border")),
        ScoutTone::Quiet => (l.c("chip-text"), l.c("chip-bg"), l.c("border-card")),
    }
}

fn render_card(card: &ScoutCard, focused: bool, now_ms: i64, l: &Look) -> El {
    let urgent = card.fact.urgency == ScoutUrgency::Now;
    let border = if focused || urgent {
        l.c("amber-border")
    } else {
        l.c("border-card")
    };
    let fill = if urgent {
        l.c("amber-tint")
    } else {
        l.c("bg-raised")
    };
    let kind_color = if urgent {
        l.c("amber-text")
    } else {
        l.c("text-muted")
    };
    let time = view::ago(&card.at, now_ms);
    let stamp = if time.is_empty() {
        "Scout".to_owned()
    } else {
        format!("Scout · {time}")
    };

    let mut box_el = col()
        .w_full()
        .gap(8.0)
        .padding(14.0)
        .radius(10.0)
        .fill(fill)
        .border(if focused { 1.5 } else { 1.0 }, border)
        .key(format!("scout-card-{}", card.card_id))
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text(view::card_kind(card).to_owned())
                        .font(rune::TextStyle::new(11.5, 600.0))
                        .color(kind_color)
                        .grow(1.0),
                )
                .child(
                    text(stamp)
                        .font(rune::TextStyle::new(11.5, 400.0))
                        .color(l.c("text-muted")),
                ),
        )
        .child(
            text(card.fact.said.clone())
                .font(rune::TextStyle::new(14.0, 600.0))
                .color(l.c("text-primary"))
                .wrap(),
        );

    if let Some(detail) = card
        .fact
        .detail
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty() && *d != card.fact.said.trim())
    {
        box_el = box_el.child(
            text(detail.to_owned())
                .font(rune::TextStyle::new(12.5, 400.0))
                .color(l.c("text-secondary"))
                .wrap(),
        );
    }

    if let Some(lines) = &card.fact.lines
        && !lines.is_empty()
    {
        let mut lines_col = col().w_full().gap(3.0);
        for line in lines {
            lines_col = lines_col.child(
                text(line.clone())
                    .font(rune::TextStyle::new(12.5, 400.0))
                    .color(l.c("text-secondary"))
                    .wrap(),
            );
        }
        box_el = box_el.child(lines_col);
    }

    if let Some(quote) = card
        .fact
        .quote
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        box_el = box_el.child(
            col()
                .w_full()
                .padding(8.0)
                .radius(6.0)
                .fill(l.c("bg-window"))
                .border(1.0, l.c("border-hairline"))
                .child(
                    text(quote.to_owned())
                        .font(rune::TextStyle::new(12.0, 400.0))
                        .color(l.c("text-secondary"))
                        .wrap(),
                ),
        );
    }

    if let Some(chips) = &card.fact.chips
        && !chips.is_empty()
    {
        let mut chips_row = row().gap(6.0).wrap();
        for chip in chips {
            let (fg, bg, brd) = chip_colors(chip.tone, l);
            chips_row = chips_row.child(
                row()
                    .padding(6.0)
                    .radius(6.0)
                    .fill(bg)
                    .border(1.0, brd)
                    .child(
                        text(chip.text.clone())
                            .font(rune::TextStyle::new(11.5, 500.0))
                            .color(fg),
                    ),
            );
        }
        box_el = box_el.child(chips_row);
    }

    if let Some(take) = card
        .take
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        box_el = box_el.child(
            text(take.to_owned())
                .font(rune::TextStyle::new(12.5, 400.0))
                .color(l.c("text-primary"))
                .wrap(),
        );
    } else if card.reading {
        box_el = box_el.child(
            text("Reading against your goal…".to_owned())
                .font(rune::TextStyle::new(12.0, 400.0))
                .color(l.c("text-muted"))
                .wrap(),
        );
    }

    if let Some(draft) = &card.fact.draft {
        box_el = box_el.child(
            col()
                .w_full()
                .gap(4.0)
                .padding(10.0)
                .radius(8.0)
                .fill(l.c("bg-window"))
                .border(1.0, l.c("border-hairline"))
                .child(
                    text(format!("Drafted for {}", draft.name))
                        .font(rune::TextStyle::new(11.5, 600.0))
                        .color(l.c("text-muted")),
                )
                .child(
                    text(draft.message.clone())
                        .font(rune::TextStyle::new(12.5, 400.0))
                        .color(l.c("text-primary"))
                        .wrap(),
                ),
        );
    }

    // 2a card actions only (2b `/locust` comment posting and `FixLine` belong to Part 2b).
    let mut actions = row().w_full().gap(8.0).center_y();
    if let Some(draft) = card.fact.draft.clone() {
        let send_label = format!("Send to {}", draft.name);
        let id_send = card.card_id.clone();
        let d_send = draft.clone();
        actions = actions.child(
            controls::primary_button(l, &send_label, false)
                .role("button")
                .label(send_label)
                .key(format!("scout-send-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| {
                    send_draft(s, id_send.clone(), d_send.clone(), tx);
                }),
        );
        let id_box = card.card_id.clone();
        actions = actions.child(
            controls::button(l, "Change it first", Tone::Neutral, false)
                .role("button")
                .label("Change it first")
                .key(format!("scout-draft-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| {
                    draft_into_box(s, id_box.clone(), draft.clone(), tx);
                }),
        );
    }
    if let Some(open) = card.fact.open.clone() {
        let open_label = match &open {
            ScoutOpen::Url { label, .. } | ScoutOpen::Conversation { label, .. } => label.clone(),
        };
        actions = actions.child(
            controls::button(l, &open_label, Tone::Neutral, false)
                .role("button")
                .label(open_label)
                .key(format!("scout-open-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| {
                    open_target(s, open.clone(), tx);
                }),
        );
    }
    if let Some(tune) = &card.fact.tune {
        let tune_label = tune.label.clone();
        let id_tune = card.card_id.clone();
        actions = actions.child(
            controls::button(l, &tune_label, Tone::Neutral, false)
                .role("button")
                .label(tune_label)
                .key(format!("scout-tune-{}", card.card_id))
                .on_press(move |s: &mut State| {
                    tune_card(s, id_tune.clone());
                }),
        );
    }
    let id_dismiss = card.card_id.clone();
    actions = actions.child(el().grow(1.0)).child(
        controls::ghost_button(l, "Dismiss")
            .role("button")
            .label("Dismiss")
            .key(format!("scout-dismiss-{}", card.card_id))
            .on_press(move |s: &mut State| {
                dismiss_card(s, id_dismiss.clone(), None);
            }),
    );

    box_el.child(actions)
}

/// Home Scout cards section, or watching fallback when Scout is off the team and clear.
pub(super) fn home_section(s: &State, l: &Look) -> Option<El> {
    let st = s.scout.state.as_ref()?;
    if !st.settings.on {
        return None;
    }
    let now_ms = effective_now_ms(s);
    if st.cards.is_empty() {
        if on_team(s, st) {
            return None;
        }
        let line = view::watching_line(st.last_look_at.as_deref(), now_ms);
        let look_label = if s.scout.looking {
            "Looking…"
        } else {
            "Look now"
        };
        let mut look_btn = controls::button(l, look_label, Tone::Neutral, s.scout.looking)
            .role("button")
            .label(look_label)
            .key("scout-watching-look-now");
        if !s.scout.looking {
            look_btn = look_btn.on_press(|s: &mut State| look_now(s));
        }
        return Some(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .padding(10.0)
                .radius(8.0)
                .fill(l.c("bg-raised"))
                .border(1.0, l.c("border-card"))
                .key("scout-watching")
                .child(
                    text(format!("Scout is watching · {line}"))
                        .font(rune::TextStyle::new(12.5, 400.0))
                        .color(l.c("text-secondary"))
                        .grow(1.0)
                        .wrap(),
                )
                .child(look_btn),
        );
    }

    let (shown, total) = view::visible_cards(
        &st.cards,
        s.scout.show_all,
        s.scout.focus_card_id.as_deref(),
    );
    let mut header = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(view::cards_heading(total))
                .font(rune::TextStyle::new(12.0, 600.0))
                .color(l.c("text-muted"))
                .grow(1.0),
        );
    if total > SCOUT_SHOWN_CARDS {
        let toggle_label = if s.scout.show_all {
            "Show fewer".to_owned()
        } else {
            format!("Show all {total}")
        };
        header = header.child(
            controls::ghost_button(l, &toggle_label)
                .role("button")
                .label(toggle_label)
                .key("scout-show-all")
                .on_press(|s: &mut State| {
                    s.scout.show_all = !s.scout.show_all;
                    s.scout.focus_card_id = None;
                }),
        );
    }

    let mut sec = col().w_full().gap(10.0).key("scout-cards").child(header);
    if let Some(why) = st
        .model_said
        .as_deref()
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        sec = sec.child(
            row()
                .w_full()
                .padding(10.0)
                .radius(8.0)
                .fill(l.c("amber-tint"))
                .border(1.0, l.c("amber-border"))
                .key("scout-model-said")
                .child(
                    text(view::model_said_banner(why))
                        .font(rune::TextStyle::new(12.5, 400.0))
                        .color(l.c("amber-text"))
                        .wrap(),
                ),
        );
    }

    let mut list = col().w_full().gap(10.0);
    for card in shown {
        let focused = s.scout.focus_card_id.as_deref() == Some(card.card_id.as_str());
        list = list.child(render_card(card, focused, now_ms, l));
    }
    Some(sec.child(list))
}

/// Urgent (`Urgency::Now`) Scout cards shown at the top of Board > "Needs you" (`BoardScreen.tsx`).
pub(super) fn board_waiting_cards(s: &State) -> Vec<&ScoutCard> {
    let Some(st) = s.scout.state.as_ref() else {
        return Vec::new();
    };
    if !st.settings.on {
        return Vec::new();
    }
    let filter_name = s.home.board_teammate.as_deref();
    st.cards
        .iter()
        .filter(|c| c.fact.urgency == ScoutUrgency::Now)
        .filter(|c| match filter_name {
            None => true,
            Some("Scout") => true,
            Some(name) => c.fact.draft.as_ref().is_some_and(|d| d.name == name),
        })
        .collect()
}

/// Render a Scout urgent card on the Board ("Needs you" column above conversations).
pub(super) fn board_wait_card(s: &State, card: &ScoutCard, l: &Look) -> El {
    let tid = s
        .scout
        .state
        .as_ref()
        .and_then(|st| st.settings.teammate_id.as_deref());
    let mate = tid.and_then(|id| s.teammates.iter().find(|t| t.id == id));
    let (face_id, face_name, face_look) = match mate {
        Some(m) => (m.id.as_str(), m.name.as_str(), s.looks.get(&m.id)),
        None => ("scout", "Scout", None),
    };
    let face = crate::look::faces::face(
        s.image_cache.as_deref(),
        face_id,
        face_name,
        face_look,
        26.0,
        l,
    );

    let mut content = col()
        .grow(1.0)
        .shrink()
        .gap(6.0)
        .child(
            text(card.fact.said.clone())
                .font(type_::BODY)
                .color(l.c("text-primary"))
                .wrap(),
        )
        .child(
            text(face_name.to_owned())
                .font(type_::LABEL)
                .color(l.c("amber-text")),
        );

    let mut actions = row().w_full().gap(8.0).flex_wrap();
    if let Some(draft) = &card.fact.draft {
        let send_label = format!("Send to {}", draft.name);
        let d_send = draft.clone();
        let id_send = card.card_id.clone();
        actions = actions.child(
            controls::button(l, &send_label, Tone::Active, false)
                .role("button")
                .label(send_label)
                .key(format!("board-scout-send-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| {
                    s.home.board_open = false;
                    send_draft(s, id_send.clone(), d_send.clone(), tx);
                }),
        );

        let d_edit = draft.clone();
        let id_edit = card.card_id.clone();
        actions = actions.child(
            controls::button(l, "Change it first", Tone::Neutral, false)
                .role("button")
                .label("Change it first")
                .key(format!("board-scout-change-{}", card.card_id))
                .on_press_tx(move |s: &mut State, tx| {
                    s.home.board_open = false;
                    draft_into_box(s, id_edit.clone(), d_edit.clone(), tx);
                }),
        );
    }

    let id_dismiss = card.card_id.clone();
    actions = actions.child(
        controls::ghost_button(l, "Dismiss")
            .role("button")
            .label("Dismiss")
            .key(format!("board-scout-dismiss-{}", card.card_id))
            .on_press(move |s: &mut State| {
                dismiss_card(s, id_dismiss.clone(), None);
            }),
    );

    content = content.child(actions);

    let focus_id = card.card_id.clone();
    row()
        .w_full()
        .gap(12.0)
        .padding(10.0)
        .radius(8.0)
        .fill(l.c("amber-tint"))
        .border(1.0, l.c("amber-border"))
        .key(format!("board-scout-{}", card.card_id))
        .child(face)
        .child(content)
        .on_press(move |s: &mut State| {
            s.home.board_open = false;
            s.scout.focus_card_id = Some(focus_id.clone());
            crate::home_panel::open_home(s);
        })
}

/// Home team grid invite tile ("Meet Scout") with separate Not now button (`HomeTeam.tsx`).
pub(super) fn invite_team_tile(l: &Look) -> El {
    let card_btn = row()
        .grow(1.0)
        .shrink()
        .gap(12.0)
        .padding(12.0)
        .center_y()
        .radius(10.0)
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-card"))
        .hover(l.c("hover-surface"))
        .role("button")
        .label(SCOUT_INVITE_NAME)
        .key("scout-invite-card")
        .on_press(|s: &mut State| {
            s.scout.invite_open = true;
            s.scout.invite_message = None;
        })
        .child(controls::glyph(l, &icons::EYE, 18.0, Some(l.c("text-secondary"))))
        .child(
            col()
                .gap(2.0)
                .grow(1.0)
                .shrink()
                .child(
                    text(SCOUT_INVITE_NAME)
                        .font(rune::TextStyle::new(14.0, 500.0))
                        .color(l.c("text-name"))
                        .truncate(),
                )
                .child(
                    text(SCOUT_INVITE_ROLE)
                        .font(rune::TextStyle::new(12.5, 400.0))
                        .color(l.c("text-secondary"))
                        .truncate(),
                )
                .child(
                    text(SCOUT_INVITE_ROUTE)
                        .font(rune::TextStyle::new(11.5, 400.0))
                        .color(l.c("text-muted"))
                        .truncate(),
                ),
        );

    let dismiss_btn = row()
        .padding(6.0)
        .radius(6.0)
        .hover(l.c("hover-surface"))
        .role("button")
        .label(SCOUT_NOT_NOW_LABEL)
        .key("scout-not-now")
        .on_press(|s: &mut State| {
            s.scout.not_now = true;
            s.scout.invite_open = false;
        })
        .child(controls::glyph(l, &icons::CLOSE, 13.0, Some(l.c("text-muted"))));

    row()
        .w(246.0)
        .gap(4.0)
        .center_y()
        .child(card_btn)
        .child(dismiss_btn)
}

/// Modal dialog opened from Home's `Meet Scout` tile.
pub(super) fn invite_dialog(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let mut body = col()
        .w_full()
        .gap(10.0)
        .child(
            text(SCOUT_INVITE_LEDE)
                .font(rune::TextStyle::new(13.0, 400.0))
                .color(l.c("text-secondary"))
                .wrap(),
        );
    if let Some((_, model_name)) = default_scout_route(s) {
        body = body.child(
            text(view::invite_sends_line(&model_name))
                .font(rune::TextStyle::new(12.5, 400.0))
                .color(l.c("text-muted"))
                .wrap(),
        );
    }
    if let Some(msg) = &s.scout.invite_message {
        body = body.child(
            text(msg.clone())
                .font(rune::TextStyle::new(12.5, 400.0))
                .color(l.c("red-text"))
                .wrap(),
        );
    }

    let turn_on_label = if s.scout.invite_busy {
        "Turning on…"
    } else {
        "Turn on Scout"
    };
    let mut turn_on_btn = controls::primary_button(&l, turn_on_label, s.scout.invite_busy)
        .role("button")
        .label(turn_on_label)
        .key("scout-invite-turn-on");
    if !s.scout.invite_busy {
        turn_on_btn = turn_on_btn.on_press(|s: &mut State| turn_on_scout(s, true));
    }

    let not_now_btn = controls::button(&l, "Not now", Tone::Neutral, false)
        .role("button")
        .label("Not now")
        .key("scout-invite-not-now")
        .on_press(|s: &mut State| {
            s.scout.not_now = true;
            s.scout.invite_open = false;
            s.scout.invite_message = None;
        });

    let close_btn = controls::ghost_button(&l, "Close")
        .role("button")
        .label("Close")
        .key("scout-invite-close")
        .on_press(|s: &mut State| {
            s.scout.invite_open = false;
            s.scout.invite_message = None;
        });

    let actions = vec![turn_on_btn, not_now_btn, close_btn];
    col()
        .w(440.0)
        .padding(20.0)
        .gap(14.0)
        .radius(12.0)
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("dialog-edge"))
        .key("scout-invite-dialog")
        .child(
            text(SCOUT_INVITE_TITLE)
                .font(rune::TextStyle::new(16.0, 600.0))
                .color(l.c("text-primary"))
                .wrap(),
        )
        .child(body)
        .child(row().gap(8.0).children(actions))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use locust_core::engine::Model;
    use locust_core::scout::types::{ScoutChip, ScoutFact, ScoutTune, ScoutWatchKey};
    use rune::{Env, Runtime};

    fn home_sec_el(s: &State, _: &mut Cx<State>) -> El {
        col()
            .w_full()
            .h_full()
            .child(home_section(s, &Look::new(true)).unwrap_or_else(el))
    }

    fn invite_tile_el(_s: &State, _: &mut Cx<State>) -> El {
        col().w_full().h_full().child(invite_team_tile(&Look::new(true)))
    }

    fn invite_dlg_el(s: &State, _: &mut Cx<State>) -> El {
        col()
            .w_full()
            .h_full()
            .child(invite_dialog(s, &Palette::new(true)))
    }

    pub(crate) fn render_state(
        state: State,
        build: fn(&State, &mut Cx<State>) -> El,
    ) -> Runtime<State> {
        let mut rt = Runtime::new(state, build, (960.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    pub(crate) fn click_key(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("no element {key} on screen"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
    }

    pub(crate) fn collect_rt_text(rt: &Runtime<State>) -> String {
        let mut out = Vec::new();
        for node in rt.elements() {
            if let Some(t) = node.text {
                out.push(t);
            }
            if let Some(l) = node.label
                && !out.contains(&l)
            {
                out.push(l);
            }
        }
        out.join("\n")
    }

    fn base_state(on: bool) -> State {
        let mut s = State::default();
        s.scout.now_ms = Some(1_776_000_000_000);
        s.scout.state = Some(ScoutState {
            settings: ScoutSettings {
                on,
                ..ScoutSettings::default()
            },
            cards: Vec::new(),
            look_ids: Vec::new(),
            last_look_at: None,
            model_said: None,
            today: None,
            notes: Vec::new(),
        });
        s
    }

    fn make_card(id: &str, urgency: ScoutUrgency, said: &str) -> ScoutCard {
        ScoutCard {
            card_id: id.into(),
            at: "2026-04-12T10:00:00.000Z".into(),
            reading: false,
            take: None,
            raw_fix: None,
            fact: ScoutFact::simple(id, urgency, said),
        }
    }

    // --- 1:1 ports of `scout-says-what-needs-you.test.tsx` (13 Part-2a cases) ---

    #[test]
    fn renders_cards_puts_now_first_shows_the_drafted_message_and_dims_the_notice_below_it() {
        let mut s = base_state(true);
        let mut soon = make_card("soon-1", ScoutUrgency::Soon, "PR #7 failing");
        soon.fact.detail = Some("Tests failed on main".into());
        let mut now = make_card("now-1", ScoutUrgency::Now, "Lin waiting on design question");
        now.fact.draft = Some(ScoutDraft {
            teammate_id: "lin".into(),
            name: "Lin".into(),
            message: "Use the small token table.".into(),
        });
        s.scout.state.as_mut().unwrap().cards = vec![soon, now];

        let rt = render_state(s, home_sec_el);
        let all = collect_rt_text(&rt);
        let now_pos = all.find("Lin waiting on design question").unwrap();
        let soon_pos = all.find("PR #7 failing").unwrap();
        assert!(now_pos < soon_pos, "Now urgency must render before Soon");
        assert!(all.contains("Needs you"));
        assert!(all.contains("Noticed"));
        assert!(all.contains("Drafted for Lin"));
        assert!(all.contains("Use the small token table."));
    }

    #[test]
    fn calls_on_open_on_send_on_draft_into_box_and_on_dismiss_when_their_buttons_are_pressed() {
        let mut s = base_state(true);
        let mut card = make_card("c1", ScoutUrgency::Now, "Review asked");
        card.fact.open = Some(ScoutOpen::Url {
            url: "https://github.com/org/repo/pull/4".into(),
            label: "Open PR".into(),
        });
        card.fact.draft = Some(ScoutDraft {
            teammate_id: "lin".into(),
            name: "Lin".into(),
            message: "Please review PR #4.".into(),
        });
        s.scout.state.as_mut().unwrap().cards = vec![card];

        let mut rt = render_state(s, home_sec_el);
        assert!(rt.find("scout-send-c1").is_some());
        assert!(rt.find("scout-draft-c1").is_some());
        assert!(rt.find("scout-open-c1").is_some());
        assert!(rt.find("scout-dismiss-c1").is_some());

        click_key(&mut rt, "scout-open-c1");
        assert_eq!(rt.state.opened_links, vec!["https://github.com/org/repo/pull/4"]);

        click_key(&mut rt, "scout-dismiss-c1");
        assert!(rt.state.scout.state.as_ref().unwrap().cards.is_empty());
    }

    #[test]
    fn limits_to_three_cards_until_show_all_is_clicked_and_keeps_a_focused_card_in_sight() {
        let mut s = base_state(true);
        s.scout.state.as_mut().unwrap().cards = (1..=5)
            .map(|i| make_card(&format!("c{i}"), ScoutUrgency::Soon, &format!("Notice {i}")))
            .collect();
        s.scout.focus_card_id = Some("c5".into());

        let mut rt = render_state(s, home_sec_el);
        assert!(rt.find("scout-card-c1").is_some());
        assert!(rt.find("scout-card-c3").is_some());
        assert!(rt.find("scout-card-c4").is_none());
        assert!(rt.find("scout-card-c5").is_some());
        assert!(rt.find("scout-show-all").is_some());

        click_key(&mut rt, "scout-show-all");
        assert!(rt.find("scout-card-c4").is_some());
        assert!(rt.find("scout-card-c5").is_some());
    }

    #[test]
    fn says_first_look_is_on_its_way_before_the_first_look() {
        let s = base_state(true);
        let rt = render_state(s, home_sec_el);
        let text = collect_rt_text(&rt);
        assert!(text.contains("its first look is on its way"));
    }

    #[test]
    fn says_when_it_last_looked_once_a_look_has_happened() {
        let mut s = base_state(true);
        s.scout.now_ms = Some(
            chrono::DateTime::parse_from_rfc3339("2026-04-12T10:12:00.000Z")
                .unwrap()
                .timestamp_millis(),
        );
        s.scout.state.as_mut().unwrap().last_look_at = Some("2026-04-12T10:00:00.000Z".into());
        let rt = render_state(s, home_sec_el);
        let text = collect_rt_text(&rt);
        assert!(text.contains("looked 12 min ago · nothing needs you"));
    }

    #[test]
    fn shows_the_model_reason_banner_when_the_model_failed() {
        let mut s = base_state(true);
        let st = s.scout.state.as_mut().unwrap();
        st.cards = vec![make_card("c1", ScoutUrgency::Soon, "Fallback card")];
        st.model_said = Some("OpenCode free limit reached.".into());
        let rt = render_state(s, home_sec_el);
        assert!(rt.find("scout-model-said").is_some());
        let text = collect_rt_text(&rt);
        assert!(text.contains("OpenCode free limit reached. The cards below are in Locust’s words."));
    }

    #[test]
    fn renders_the_meet_scout_invite_with_the_free_model_name_and_turn_on_button() {
        let mut s = base_state(false);
        s.teammates.push(Teammate {
            id: "lin".into(),
            name: "Lin".into(),
            role: "Engineer".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        });
        s.home.folder_path = Some("/tmp/proj".into());
        s.models.insert(
            "opencode".into(),
            vec![engine::Models {
                teammate: None,
                runtime: "opencode".into(),
                model: None,
                effort: None,
                models: vec![Model {
                    id: "opencode/qwen3-free".into(),
                    name: "Qwen 3 Free".into(),
                    efforts: vec![],
                }],
            }],
        );

        assert!(invite_offer_active(&s));
        let mut tile = render_state(State::default(), invite_tile_el);
        assert!(tile.find("scout-invite-card").is_some());
        assert!(tile.find("scout-not-now").is_some());
        click_key(&mut tile, "scout-not-now");
        assert!(tile.state.scout.not_now);

        let dlg = render_state(s, invite_dlg_el);
        let joined = collect_rt_text(&dlg);
        assert!(joined.contains(SCOUT_INVITE_TITLE));
        assert!(joined.contains("Qwen 3 Free"));
        assert!(dlg.find("scout-invite-turn-on").is_some());
    }

    #[test]
    fn hides_meet_scout_invite_when_no_folder_or_no_free_model_or_already_on() {
        let mut s = base_state(false);
        s.teammates.push(Teammate {
            id: "lin".into(),
            name: "Lin".into(),
            role: "Engineer".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        });
        assert!(!invite_offer_active(&s));

        s.home.folder_path = Some("/tmp/proj".into());
        assert!(!invite_offer_active(&s));

        s.models.insert(
            "opencode".into(),
            vec![engine::Models {
                teammate: None,
                runtime: "opencode".into(),
                model: None,
                effort: None,
                models: vec![Model {
                    id: "opencode/qwen3-free".into(),
                    name: "Qwen 3 Free".into(),
                    efforts: vec![],
                }],
            }],
        );
        assert!(invite_offer_active(&s));

        s.scout.state.as_mut().unwrap().settings.on = true;
        assert!(!invite_offer_active(&s));
    }

    #[test]
    fn shows_the_invite_error_when_turn_on_refuses() {
        let mut s = base_state(false);
        s.scout.invite_open = true;
        s.scout.invite_message = Some("OpenCode could not add Scout.".into());
        let dlg = render_state(s, invite_dlg_el);
        assert!(collect_rt_text(&dlg).contains("OpenCode could not add Scout."));
    }

    #[test]
    fn dims_scout_cards_into_watching_when_cards_clear() {
        let mut s = base_state(true);
        s.scout.state.as_mut().unwrap().cards = vec![make_card("c1", ScoutUrgency::Soon, "PR ready")];
        let mut rt = render_state(s, home_sec_el);
        assert!(rt.find("scout-cards").is_some());

        rt.update(|st, _| st.scout.state.as_mut().unwrap().cards.clear());
        rt.frame(0.0);
        assert!(rt.find("scout-watching").is_some());
    }

    #[test]
    fn shows_look_now_button_in_scout_watching_and_triggers_on_look_now() {
        let s = base_state(true);
        let rt = render_state(s, home_sec_el);
        assert!(rt.find("scout-watching-look-now").is_some());
    }

    #[test]
    fn renders_chip_tones_quote_lines_and_detail_on_a_card() {
        let mut s = base_state(true);
        let mut card = make_card("c1", ScoutUrgency::Soon, "Build status");
        card.fact.detail = Some("Two checks ran".into());
        card.fact.quote = Some("error: flaky test".into());
        card.fact.lines = Some(vec!["unit tests passed".into(), "lint failed".into()]);
        card.fact.chips = Some(vec![
            ScoutChip::new("2 passed", ScoutTone::Green),
            ScoutChip::new("1 failed", ScoutTone::Red),
        ]);
        s.scout.state.as_mut().unwrap().cards = vec![card];
        let rt = render_state(s, home_sec_el);
        let joined = collect_rt_text(&rt);
        for expected in [
            "Two checks ran",
            "error: flaky test",
            "unit tests passed",
            "lint failed",
            "2 passed",
            "1 failed",
        ] {
            assert!(joined.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn shows_looked_line_on_scout_team_card_when_on_team_and_clear_with_look_now_eye_button() {
        let mut s = base_state(true);
        s.scout.now_ms = Some(
            chrono::DateTime::parse_from_rfc3339("2026-04-12T10:05:00.000Z")
                .unwrap()
                .timestamp_millis(),
        );
        let st = s.scout.state.as_mut().unwrap();
        st.settings.teammate_id = Some("scout-tm".into());
        st.last_look_at = Some("2026-04-12T10:00:00.000Z".into());
        s.teammates.push(Teammate {
            id: "scout-tm".into(),
            name: "Scout".into(),
            role: "Scout".into(),
            runtime: Some("opencode".into()),
            model: Some("opencode/qwen3-free".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        });
        assert!(home_section(&s, &Look::new(true)).is_none());
        let (line, looking) = looked_line_for_teammate(&s, "scout-tm").unwrap();
        assert!(!looking);
        assert_eq!(line, "Looked 5 min ago · all clear");
    }

    // --- 1:1 ports of `a-week-with-scout-is-one-card.test.tsx` (2 cases) ---

    #[test]
    fn renders_your_week_card_with_weekly_lines_and_tune_action_button() {
        let mut s = base_state(true);
        let mut card = make_card("week:1", ScoutUrgency::Soon, "Quiet week: 14 looks, 1 card.");
        card.fact.lines = Some(vec!["Checks stayed green.".into()]);
        card.fact.tune = Some(ScoutTune {
            label: "Stop watching checks".into(),
            watch_off: Some(ScoutWatchKey::Check),
            every_minutes: None,
        });
        s.scout.state.as_mut().unwrap().cards = vec![card];
        let rt = render_state(s, home_sec_el);
        let joined = collect_rt_text(&rt);
        assert!(joined.contains("Your week"));
        assert!(joined.contains("Quiet week: 14 looks, 1 card."));
        assert!(joined.contains("Checks stayed green."));
        assert!(joined.contains("Stop watching checks"));
        assert!(rt.find("scout-tune-week:1").is_some());
    }

    #[test]
    fn pressing_tune_button_on_your_week_card_calls_tune() {
        let root = std::env::temp_dir().join(format!("scout-tune-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let st = store::Store::new(root.join("n"), root.join("e")).unwrap();
        let mut week = make_card("week:2", ScoutUrgency::Soon, "Quiet week");
        week.fact.tune = Some(ScoutTune {
            label: "Stop watching checks".into(),
            watch_off: Some(ScoutWatchKey::Check),
            every_minutes: None,
        });
        let seed = serde_json::json!({
            "settings": { "on": true },
            "cards": [week.to_json(true)],
        });
        std::fs::create_dir_all(st.root()).unwrap();
        std::fs::write(
            st.root().join("scout.json"),
            serde_json::to_string_pretty(&seed).unwrap(),
        )
        .unwrap();
        let host = ScoutHost::start(st.clone());
        let updated = host.scout.tune("week:2");
        assert!(!updated.settings.watches.check);
        assert!(updated.cards.is_empty());
        drop(host);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn free_model_selection_prefers_ranked_opencode_free_models() {
        let mut s = State::default();
        s.models.insert(
            "opencode".into(),
            vec![engine::Models {
                teammate: None,
                runtime: "opencode".into(),
                model: None,
                effort: None,
                models: vec![
                    Model {
                        id: "opencode/other-free".into(),
                        name: "Other Free".into(),
                        efforts: vec![],
                    },
                    Model {
                        id: "opencode/qwen3-coder-free".into(),
                        name: "Qwen 3 Coder".into(),
                        efforts: vec![],
                    },
                ],
            }],
        );
        let (route, name) = default_scout_route(&s).unwrap();
        assert_eq!(route.runtime, "opencode");
        assert!(route.model.ends_with("-free"));
        assert!(!name.is_empty());
    }

    // --- 1:1 ports of `a-scout-card-that-needs-you-is-on-the-board.test.tsx` (6 cases) ---

    fn board_test_state(cards: Vec<ScoutCard>, with_asking_mission: bool) -> State {
        let mut s = base_state(true);
        s.scout.state.as_mut().unwrap().settings.teammate_id = Some("tm_scout".into());
        s.scout.state.as_mut().unwrap().cards = cards;
        s.teammates.push(Teammate {
            id: "tm_scout".into(),
            name: "Scout".into(),
            role: "Custom".into(),
            runtime: Some("opencode".into()),
            model: Some("opencode/qwen3-coder-free".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        });
        s.teammates.push(Teammate {
            id: "tm_wren".into(),
            name: "Wren".into(),
            role: "Code & Migrations".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "edit".into(),
            folder: None,
        });
        if with_asking_mission {
            s.recent.push(engine::ConversationRow {
                id: "asks".into(),
                teammate: "Wren".into(),
                title: "Rename the invoices".into(),
                started_by: "the person".into(),
                state: "running".into(),
                updated: "2026-10-04T05:58:00.000Z".into(),
            });
            s.home.asks.insert(
                "asks".into(),
                crate::home_panel::BoardAsk {
                    approval_id: "ap_asks".into(),
                    kind: "command".into(),
                    command: Some("git mv invoices old-invoices".into()),
                    where_: None,
                    shown: std::time::Instant::now(),
                },
            );
        }
        s.recent.push(engine::ConversationRow {
            id: "old".into(),
            teammate: "Wren".into(),
            title: "Yesterday".into(),
            started_by: "the person".into(),
            state: "completed".into(),
            updated: "2026-10-03T05:00:00.000Z".into(),
        });
        s
    }

    fn draw_board_dark(s: &State, _cx: &mut Cx<State>) -> El {
        crate::home_panel::board_view(s, &Palette::new(true))
    }

    fn draw_board_light(s: &State, _cx: &mut Cx<State>) -> El {
        crate::home_panel::board_view(s, &Palette::new(false))
    }

    fn board_wait_card_now(with_draft: bool) -> ScoutCard {
        let mut card = make_card(
            "card_now",
            ScoutUrgency::Now,
            "Atlas has waited 25 minutes for your OK.",
        );
        if with_draft {
            card.fact.draft = Some(ScoutDraft {
                teammate_id: "tm_atlas".into(),
                name: "Atlas".into(),
                message: "Go ahead with the migration.".into(),
            });
        }
        card
    }

    #[test]
    fn board_scout_card_sits_above_waiting_conversation_with_actions_and_count() {
        let s = board_test_state(vec![board_wait_card_now(true)], true);
        let rt = render_state(s, draw_board_dark);
        let all = collect_rt_text(&rt);
        let scout_pos = all
            .find("Atlas has waited 25 minutes for your OK.")
            .expect("Scout wait card");
        let conv_pos = all
            .find("Rename the invoices")
            .expect("Waiting conversation card");
        assert!(scout_pos < conv_pos, "Scout wait card must render above waiting conversation");
        assert!(all.contains("Scout"));
        assert!(all.contains("Send to Atlas"));
        assert!(all.contains("Change it first"));
        assert!(all.contains("Dismiss"));
        assert!(all.contains("2 waiting on you"));
        assert!(rt.find("board-needs-you-count").is_some());
    }

    #[test]
    fn board_scout_card_offers_dismiss_alone_without_draft() {
        let s = board_test_state(vec![board_wait_card_now(false)], true);
        let rt = render_state(s, draw_board_dark);
        let all = collect_rt_text(&rt);
        assert!(all.contains("Dismiss"));
        assert!(!all.contains("Send to "));
        assert!(!all.contains("Change it first"));
        assert!(rt.find("board-scout-dismiss-card_now").is_some());
        assert!(rt.find("board-scout-send-card_now").is_none());
        assert!(rt.find("board-scout-change-card_now").is_none());
    }

    #[test]
    fn board_scout_card_forms_needs_you_column_when_no_conversation_waits() {
        let s = board_test_state(vec![board_wait_card_now(false)], false);
        let rt = render_state(s, draw_board_dark);
        let all = collect_rt_text(&rt);
        assert!(all.contains("Needs you"));
        assert!(all.contains("Atlas has waited 25 minutes for your OK."));
        assert!(all.contains("1 waiting on you"));
        assert!(!all.contains("all quiet"));
        assert!(all.contains("Yesterday"));
    }

    #[test]
    fn board_leaves_column_as_before_when_no_urgent_scout_card_waits() {
        let soon = make_card("card_soon", ScoutUrgency::Soon, "FYI notice");
        let s = board_test_state(vec![soon], true);
        let rt = render_state(s, draw_board_dark);
        let all = collect_rt_text(&rt);
        assert!(rt.find("board-scout-card_soon").is_none());
        assert!(all.contains("1 waiting on you"));
        assert!(all.find("Needs you").unwrap() < all.find("Rename the invoices").unwrap());
    }

    #[test]
    fn board_scout_card_uses_theme_tokens_in_both_dark_and_light_modes() {
        let dark_rt = render_state(
            board_test_state(vec![board_wait_card_now(true)], true),
            draw_board_dark,
        );
        let light_rt = render_state(
            board_test_state(vec![board_wait_card_now(true)], true),
            draw_board_light,
        );
        assert_ne!(
            Look::new(true).c("amber-tint"),
            Look::new(false).c("amber-tint")
        );
        assert!(dark_rt.find("board-scout-card_now").is_some());
        assert!(light_rt.find("board-scout-card_now").is_some());
    }

    #[test]
    fn clicking_board_scout_card_opens_home_with_card_focused() {
        let s = board_test_state(vec![board_wait_card_now(false)], true);
        let mut s = s;
        s.home.board_open = true;
        let mut rt = render_state(s, draw_board_dark);
        click_key(&mut rt, "board-scout-card_now");
        assert!(!rt.state.home.board_open);
        assert!(rt.state.home.home_open);
        assert_eq!(rt.state.scout.focus_card_id.as_deref(), Some("card_now"));
    }
}
