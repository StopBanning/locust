//! Area 13 surfaces: Home, Board, first-launch/boot, away, What's new splash.

use super::*;
use crate::away::{self, AWAY_MINIMUM_MS, AwaySummary};
use crate::boot_view::{self, BootState, BootView};
use crate::changelog::{self, ChangelogEntry};
use crate::conversation_board::{
    self, ALLOW_ONCE, ANSWER, BOARD_CLEAR, BOARD_CLEAR_LINE, BOARD_TITLE, ConversationBoardFacts,
    ConversationBoardKey, DENY, REVIEW, WAITING_ON_YOU, WANTS_TO_RUN, WORKING,
};
use crate::folders::{self, CHOOSE_FOLDER, FOLDER_LEDE, MADE_FOLDER, NO_FOLDER_CHOSEN};
use crate::named_folder::{self, NamedFolderContext, RealFiles};
use crate::starters::{
    self, COMPARE, COMPARE_MODELS, CONNECT_RUNTIME, COVER_CLAIM, NEW_TEAMMATE, PICK_ONE,
    START_A_CONVERSATION, START_FROM_ISSUE, START_WITH_A_TEAM, TEAM_FROM_IMAGE, TEAM_TEMPLATES,
    YOUR_TEAM,
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Panel {
    pub home_open: bool,
    pub board_open: bool,
    /// The Board narrowed to one teammate's conversations (the rail card's "All of Scout's conversations").
    pub board_teammate: Option<String>,
    pub boot_open: bool,
    pub splash_open: bool,
    pub folder_path: Option<String>,
    pub folder_invented: bool,
    pub boot: BootState,
    pub boot_view: Option<BootView>,
    pub away: Option<AwaySummary>,
    /// What each away row's run came to, read once when the summary is (AwayList's saidOf).
    pub away_said: HashMap<String, String>,
    pub to_look_at: HashSet<String>,
    /// Team roster finishes are cleared by looking at any conversation of that teammate (App.tsx finishedUnseen).
    pub teammates_unseen: HashSet<String>,
    pub asks: HashMap<String, BoardAsk>,
    pub waiting_for: HashMap<String, String>,
    pub changelog: Vec<ChangelogEntry>,
    pub changelog_shown: usize,
    pub splash_entries: Vec<ChangelogEntry>,
    pub seen_version: Option<String>,
    pub current_version: String,
    pub first_launch_done: bool,
    /// The cover's parts' clock (coverScript.ts): runs while the cover plays, banked while it is held.
    pub cover_clock: locust_core::cover_script::CoverClock,
    /// What Home last measured of itself, and what that decided (FirstLaunch.tsx's measure step).
    pub fit: HomeMeasure,
    /// Show all, pressed on the folded accounts line: the whole list stays open (FirstLaunch.tsx `agentsOpen`).
    pub agents_open: bool,
}

/// Home's own measure: the pane it scrolls in, the cover's card, the column as drawn, and what they decided.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct HomeMeasure {
    pub pane_height: Option<f64>,
    pub card_width: Option<f64>,
    /// The column's height as drawn, and what in it does not move with the cover, at the spacing it was drawn at.
    pub column: Option<(f64, f64, usize, bool)>,
    pub seen: locust_core::home_cover::HomeSpacingSeen,
    pub tight: bool,
    /// The cover's growth into spare height; `None` before a measure (drawn as 1).
    pub grow: Option<f64>,
}

impl HomeMeasure {
    /// Decides again from what was measured. Changes nothing until the pane, the card and the column are all known.
    fn decide(&mut self) {
        let (Some(pane), Some(card), Some((_, sections, count, tight_now))) = (self.pane_height, self.card_width, self.column) else {
            return;
        };
        let (seen, fit) = locust_core::home_cover::home_fit(self.seen, count, sections, tight_now, card, pane);
        self.seen = seen;
        self.tight = fit.tight;
        self.grow = Some(fit.grow);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BoardAsk {
    pub approval_id: String,
    pub kind: String,
    pub command: Option<String>,
    pub where_: Option<String>,
    pub shown: std::time::Instant,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

fn primary(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.text)
        .padding(7.0)
        .fill(p.selected)
        .radius(6.0)
}

pub(super) fn open_home(s: &mut State) {
    crate::room_screen::close(s);
    s.home.home_open = true;
    s.home.board_open = false;
    s.home.boot_open = false;
    sync_board_asks(s);
    refresh_away(s);
    refresh_changelog(s);
    crate::usage_line::request(s);
    crate::scout_cards::request_refresh_state(s);
    // Home draws the AI agents (first, when none is installed): ask for them if nothing has answered yet.
    if s.settings_panel.agents.is_none() {
        crate::agents_section::request(s);
    }
}

pub(super) fn open_board(s: &mut State) {
    crate::room_screen::close(s);
    s.home.board_teammate = None;
    s.home.board_open = true;
    s.home.home_open = false;
    s.home.boot_open = false;
    sync_board_asks(s);
}

pub(super) fn open_boot(s: &mut State) {
    s.home.boot_open = true;
    s.home.home_open = false;
    s.home.board_open = false;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    s.home.boot = boot_view::path_discovery_sweep(now);
    s.home.boot_view = Some(boot_view::boot_view(
        &s.home.boot,
        boot_view::BootPhase::Probing,
        now,
    ));
    s.nothing_installed = s
        .home
        .boot
        .finished
        .map(|(_, ready, _)| ready == 0)
        .unwrap_or(true);
}

fn refresh_changelog(s: &mut State) {
    let resources = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let paths = changelog::changelog_paths(&resources, &resources);
    let text = changelog::read_changelog(&paths).unwrap_or_else(|| changelog::native_notes(None));
    s.home.changelog = changelog::entries(&text);
    if s.home.changelog_shown == 0 {
        s.home.changelog_shown = changelog::WHATS_NEW_PAGE;
    }
    if s.home.current_version.is_empty() {
        s.home.current_version = s
            .home
            .changelog
            .first()
            .map(|e| e.version.clone())
            .unwrap_or_else(|| "0.1.0".into());
    }
    let _ = changelog::entry_for(&text, &s.home.current_version);
    let splash = changelog::splash_entries(
        &s.home.changelog,
        s.home.seen_version.as_deref(),
        &s.home.current_version,
    );
    s.home.splash_entries = splash.into_iter().cloned().collect();
    s.home.splash_open = !s.home.splash_entries.is_empty();
}

fn refresh_away(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.home.away = None;
        return;
    };
    let Some(mark) = away::read_attention_mark(&store) else {
        s.home.away = None;
        return;
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    if away::was_away(Some(&mark), now_ms, AWAY_MINIMUM_MS).is_none() {
        s.home.away = None;
        return;
    }
    let rows = recent_as_missions(s)
        .into_iter()
        .map(|r| away::AwayRow {
            mission_id: r.mission_id,
            title: r.title,
            phase: r.phase,
            last_at: r.last_at,
            owner_id: r.owner_id,
            routine_id: None,
            member_ids: r.member_ids,
            root_id: r.root_id,
        })
        .collect::<Vec<_>>();
    let needs = s.home.asks.keys().cloned().collect();
    s.home.away = away::since_you_were_away(&mark, &rows, &[], &needs);
    s.home.away_said = s
        .home
        .away
        .as_ref()
        .map(|summary| away_said_for(summary, |id| locust_core::native::turn_records(id).map(|turn| turn.events)))
        .unwrap_or_default();
}

/// What each finished or failed row's run came to, from its record's events.
pub(super) fn away_said_for(summary: &AwaySummary, events_of: impl Fn(&str) -> Option<Vec<serde_json::Value>>) -> HashMap<String, String> {
    summary
        .ran
        .iter()
        .chain(summary.failed.iter())
        .filter_map(|item| Some((item.mission_id.clone(), away::said_of(&events_of(&item.mission_id)?)?)))
        .collect()
}

pub(super) fn sync_board_asks(s: &mut State) -> bool {
    let waiting = locust_core::engine_api::api().waiting();
    update_board_asks(s, &waiting)
}

fn update_board_asks(s: &mut State, waiting: &[approval::Request]) -> bool {
    let old = std::mem::take(&mut s.home.asks);
    s.home.waiting_for.clear();
    for req in waiting {
        s.home.waiting_for.insert(
            req.mission_id.clone(),
            if req.summary.is_empty() {
                WAITING_ON_YOU.into()
            } else {
                req.summary.clone()
            },
        );
        // The oldest card for each conversation is answered first, as in Electron.
        s.home
            .asks
            .entry(req.mission_id.clone())
            .or_insert_with(|| BoardAsk {
                approval_id: req.approval_id.clone(),
                kind: req.kind.as_str().into(),
                command: (req.kind == approval::Kind::Command).then(|| {
                    if req.detail.is_empty() {
                        req.summary.clone()
                    } else {
                        req.detail.clone()
                    }
                }),
                where_: req.cwd.clone(),
                shown: old
                    .get(&req.mission_id)
                    .filter(|ask| ask.approval_id == req.approval_id)
                    .map(|ask| ask.shown)
                    .unwrap_or_else(std::time::Instant::now),
            });
    }
    old != s.home.asks
}

fn answer_board(s: &mut State, approval_id: &str, approve: bool) {
    let Some(ask) = s
        .home
        .asks
        .values()
        .find(|ask| ask.approval_id == approval_id)
    else {
        return;
    };
    if approve && ask.shown.elapsed().as_millis() < 500 {
        return;
    }
    let decision = if approve {
        approval::Decision::ApproveOnce
    } else {
        approval::Decision::Deny
    };
    match locust_core::engine_api::api().decide(approval_id, decision, None) {
        Ok(()) => {
            if s.pending_approval
                .as_ref()
                .is_some_and(|req| req.approval_id == approval_id)
            {
                s.pending_approval = None;
            }
            s.approval_notice = None;
            sync_board_asks(s);
        }
        Err(e) => {
            sync_board_asks(s);
            s.approval_notice = Some(e);
        }
    }
}

fn workroom_visible(s: &State) -> bool {
    !(s.home.home_open || s.home.board_open || s.home.boot_open
        || s.people_panel.open || s.settings_panel.open || s.routines.open || s.changes.open
        || s.compare.active_id.is_some() || s.benchmarks_open || s.rooms.active != crate::room_screen::RoomView::None)
}

/// Like App.tsx's screen effect: returning to the workroom clears that teammate's roster mark.
pub(super) fn mark_seen_if_looking(s: &mut State) -> bool {
    if !workroom_visible(s) { return false; }
    let owner = selected_conversation(s).map(|(mate, _)| mate.id.clone());
    owner.is_some_and(|id| s.home.teammates_unseen.remove(&id))
}

pub(super) fn record_finishes(s: &mut State, next: &[ConversationRow]) {
    let in_workroom = workroom_visible(s);
    let looking = if in_workroom { selected_conversation(s).and_then(|(_, c)| c.conversation_id.clone()) } else { None };
    let looking_owner = if in_workroom { selected_conversation(s).map(|(mate, _)| mate.id.clone()) } else { None };
    for row in next {
        if matches!(row.state.as_str(), "completed" | "failed")
            && s.recent.iter().any(|old| old.id == row.id && old.state == "running")
        {
            if looking.as_deref() != Some(row.id.as_str()) {
                s.home.to_look_at.insert(row.id.clone());
            }
            if let Some(owner) = s.people_panel.team["missionOwners"][&row.id].as_str()
                && looking_owner.as_deref() != Some(owner)
            {
                s.home.teammates_unseen.insert(owner.to_owned());
            }
        }
    }
}

pub(super) fn opened(s: &mut State, id: &str) {
    if let Some(owner) = s.people_panel.team["missionOwners"][id].as_str() { s.home.teammates_unseen.remove(owner); }
    let keys = recent_as_missions(s)
        .iter()
        .find(|row| {
            conversation_list::conversation_keys(row)
                .iter()
                .any(|key| key == id)
        })
        .map(conversation_list::conversation_keys)
        .unwrap_or_else(|| vec![id.to_owned()]);
    for key in keys {
        s.home.to_look_at.remove(&key);
    }
    s.home.home_open = false;
    s.home.board_open = false;
    s.home.boot_open = false;
}

pub(super) fn save_attention(s: &mut State) {
    if let Some(store) = &s.data {
        let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if let Err(error) = away::write_attention_mark(store, &at) {
            s.status = error;
            s.status_tone = StatusTone::Amber;
        }
    }
}

fn mark_away_seen(s: &mut State) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let path = store.root().join(away::AWAY_FILE);
    if !away::attention_path_is_native(&store, &path) {
        s.status = "Away mark refused outside the native data folder.".into();
        s.status_tone = StatusTone::Amber;
        return;
    }
    if let Err(error) = away::write_attention_mark(&store, &at) {
        s.status = error;
        s.status_tone = StatusTone::Amber;
        return;
    }
    let _ = away::away_counts_from(&serde_json::json!({"ran":0,"failed":0,"missed":0,"waiting":0}));
    s.home.away = None;
}

fn apply_starter(s: &mut State, starter_id: &str, prompt: &str, mode: Option<&str>) {
    if let Some(mode) = mode {
        s.composer_mode = Some(mode.into());
    }
    let team: Vec<(String, String)> = s
        .teammates
        .iter()
        .map(|t| (t.id.clone(), t.role.clone()))
        .collect();
    if let Some(teammate_id) = starters::teammate_for_starter(starter_id, &team)
        && let Some(idx) = s.teammates.iter().position(|t| t.id == teammate_id)
    {
        s.selected = idx;
    }
    let prompt = if s.home.folder_invented {
        let held: Vec<&str> = s
            .teammates
            .get(s.selected)
            .map(|_| Vec::new())
            .unwrap_or_default();
        if let Some(fresh) = starters::fresh_starters_for(&held) {
            fresh.first().copied().unwrap_or(prompt)
        } else {
            prompt
        }
    } else {
        prompt
    };
    maybe_switch_named_folder(s, prompt);
    s.draft = TextEdit::new(prompt);
    s.home.home_open = false;
}

/// When the composer sends from Home, switch to a project the message names.
pub(super) fn maybe_switch_named_folder(s: &mut State, text: &str) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let current = s
        .home
        .folder_path
        .clone()
        .unwrap_or_else(|| store.root().display().to_string());
    let current_path = std::path::PathBuf::from(&current);
    let root = store.root().to_path_buf();
    let is_no = move |p: &std::path::Path| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        name == ".claude" || name == "Locust" || p == root.as_path()
    };
    let same = |a: &std::path::Path, b: &std::path::Path| {
        a == b || folders::same_as(&store, a).ok().as_deref() == Some(b.to_string_lossy().as_ref())
    };
    let files = RealFiles;
    let ctx = NamedFolderContext {
        current: &current_path,
        is_no_project: &is_no,
        same: &same,
        windows: cfg!(windows),
        files: &files,
    };
    if let Some(project) = named_folder::project_named_in(text, &ctx) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if let Ok(entry) = folders::use_folder(&store, &project, &now) {
            let _ = native::set_open_folder(&store, std::path::Path::new(&entry.path));
            s.home.folder_path = Some(entry.path);
            s.home.folder_invented = false;
        }
    }
    let _ = folders::list(&store);
    let _ = folders::path_of(&store, "ws_missing");
    let _ = folders::recover_folder_path("ws_missing", text);
    let _ = folders::same_folder_at_start(&store.root().join(folders::FOLDERS_FILE), &current_path);
    let _ = folders::learn(&store, &[]);
    let _ = folders::move_folder(&store, "ws_missing", &current_path);
    let _ = folders::UNKNOWN_FOLDER;
    let _ = folders::CHOOSE_EXISTING;
    let _ = named_folder::paths_named_in(text, cfg!(windows));
    let _ = named_folder::checkout_of(&current_path, &files);
    let _ = named_folder::repository_of(&current_path, &files);
}

fn apply_template(s: &mut State, template_id: &str) {
    let Some(template) = starters::template_by_id(template_id) else {
        return;
    };
    let Some(store) = s.data.clone() else {
        s.status = "No native data folder for the starter team.".into();
        s.status_tone = StatusTone::Amber;
        return;
    };
    for mate in template.teammates {
        let mut entry = serde_json::json!({
            "name": mate.name,
            "role": mate.role,
            "instructions": "",
            "hue": mate.hue,
            "starters": mate.starters,
        });
        if let Some(title) = mate.role_title {
            entry["roleTitle"] = serde_json::json!(title);
        }
        if let Err(e) = people::save_teammate(&store, entry) {
            s.status = e;
            s.status_tone = StatusTone::Amber;
            return;
        }
    }
    s.home.first_launch_done = true;
    s.status = format!("Added the {} team.", template.title);
    s.status_tone = StatusTone::Green;
    s.team = Latest::default();
}

/// The icon StarterList.tsx gives each starter.
fn starter_icon(label: &str) -> &'static rune::Icon {
    use crate::look::icons;
    match label {
        "Plan before building" => &icons::ROUTE,
        "Debug an issue" => &icons::TARGET,
        "Review my changes" | "Check a document" => &icons::EYE,
        "Build a landing page" => &icons::FILE,
        "Research a question" => &icons::SEARCH,
        "Work out the numbers" => &icons::GRID,
        "Ask the whole team" => &icons::USERS,
        "Put it on a schedule" => &icons::CLOCK,
        _ => &icons::SPARK,
    }
}

/// Home's banner: the Locust lockup and its line on the boot screen's dark glass, dark in both themes as Electron's.
/// HomeCover.tsx's glass line while work runs (0.610): who waits on you, then who is working, by the same facts the
/// cards below say (`teammate_status`). None: the glass keeps its claim.
fn cover_status(s: &State) -> Option<String> {
    let (mut working, mut waiting) = (Vec::new(), Vec::new());
    for mate in &s.teammates {
        match teammate_status(s, &mate.id, mate.busy, mate.runtime.as_deref(), &mate.role).status {
            "working" => working.push(mate.name.as_str()),
            "approval-needed" => waiting.push(mate.name.as_str()),
            _ => {}
        }
    }
    locust_core::home_cover::glass_status(&working, &waiting)
}

/// Seconds on the cover's own clock: from the first time Home asked, so every reading shares one origin.
fn cover_now() -> f64 {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    EPOCH.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// The cover is on once the agents have answered: the moment the loading screen's work is done (HomeCover `ready`).
fn cover_ready(s: &State) -> bool {
    s.settings_panel.agents.is_some()
}

/// The parts' clock runs only while the cover is on, in front, and not asked to hold still.
fn cover_playing(s: &State) -> bool {
    cover_ready(s) && s.window_focused && !s.reduced_motion.get()
}

/// What one bot on the cover was drawn as, for the window's own tests (HomeCover's `data-*` attributes).
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CoverDrawn {
    pub key: &'static str,
    pub bot: &'static str,
    pub hue: Option<&'static str>,
    pub state: &'static str,
    pub beat: &'static str,
    pub eyes: Option<[String; 2]>,
    pub tone: locust_core::bot_spec::Phosphor,
    pub floating: bool,
    pub zzz: bool,
}

#[cfg(test)]
thread_local! {
    pub(super) static DRAWN_COVER: std::cell::RefCell<Vec<CoverDrawn>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// lcBotFloat: up 6% of the bot's height and back, on a curve, over 3.8 s. Electron steps it in 11; this is smooth.
const FLOAT_CURVE: &[[f32; 2]] = &[
    [0.0, 0.0], [0.1, 0.095], [0.2, 0.345], [0.3, 0.655], [0.4, 0.905], [0.5, 1.0],
    [0.6, 0.905], [0.7, 0.655], [0.8, 0.345], [0.9, 0.095], [1.0, 0.0],
];

/// HomeCover.tsx, drawn to the column: the boot screen's bezel and glass, the lockup and the claim on the glass under
/// its scanlines, and the three standing on its top edge, each playing its part. Everything is placed on the cover's
/// own grid and drawn at the card's width's scale, grown into the room Home measured (`home_cover::cover_place`).
fn cover(s: &State, l: &crate::look::controls::Look, status: Option<String>) -> El {
    use locust_core::cover_script::{self as script, CoverBeat};
    use locust_core::home_cover::{cover_place, claim_size, CoverPart, COVER_CAST};
    let card_width = s.home.fit.card_width.unwrap_or(0.0);
    let at = cover_place(card_width, s.home.fit.grow.unwrap_or(1.0));
    let k = at.drawn as f32;
    let ready = cover_ready(s);
    let playing = cover_playing(s);
    let still = !ready || s.reduced_motion.get();
    let elapsed = s.home.cover_clock.elapsed(cover_now());
    let (worker, thinker, _) = script::cover_beats(elapsed);
    let beat_of = |part: CoverPart| -> CoverBeat {
        match part {
            CoverPart::Worker if still => script::STILL_WORKER,
            CoverPart::Worker => worker,
            CoverPart::Thinker if still => script::STILL_THINKER,
            CoverPart::Thinker => thinker,
            // Not carried: the sleeper waking for a pointer that comes near (`hopperWakes`); it sleeps.
            CoverPart::Sleeper => script::SLEEPING,
        }
    };

    let ink = l.c("boot-ink");
    let line = status.clone().unwrap_or_else(|| COVER_CLAIM.to_owned());
    let mark_w = 100.0 * k;
    let mark_h = mark_w * 176.39 / 306.89;
    let word_h = 48.0 * k * 0.62;
    let word_w = word_h * 283.65 / 39.36;
    let m = at.machine;
    // The claim steps aside when the glass is narrower than it (0.405): out of the layout, still measured, so it comes
    // back when it fits again.
    let glass_w = m.width as f32 - 2.0 * 9.0 * k - 2.0;
    let claim_fits = rune::memo::layout("home-cover-line").is_none_or(|r| r[2] + 24.0 <= glass_w);
    let mut claim = text(line.to_uppercase())
        .font(rune::TextStyle::new(claim_size(at.drawn) as f32, 400.0).mono().tracking(0.16 * claim_size(at.drawn) as f32))
        .color(if claim_fits { l.c("boot-ink-muted") } else { l.c("boot-ink-muted").alpha(0.0) })
        .key("home-cover-line");
    if !claim_fits {
        claim = claim.absolute(0.0, 0.0);
    }
    let screen = col()
        .center()
        .gap(16.0 * k)
        .child(
            row()
                .gap(18.0 * k)
                .center_y()
                .key("home-cover-lockup")
                .child(rune::icon(&crate::look::icons::LOCUST_MARK, mark_w).color(ink).my(-(mark_w - mark_h) / 2.0).a11y_hidden())
                .child(rune::icon(&crate::look::icons::LOCUST_WORDMARK, word_w).color(ink).my(-(word_w - word_h) / 2.0)),
        )
        .child(claim);
    // The tube's scanlines lie over the lockup and the claim, as the boot screen's do: drawn after them, so on top.
    let scan = rune::canvas({
        let (w, h) = (glass_w, m.height as f32 - 2.0 * 9.0 * k - 2.0);
        let mut shape = rune::Shape::new().view_box(w, h);
        let mut y = 1.0;
        while y < h {
            shape = shape.fill(vec![[0.0, y], [w, y], [w, y + 1.0], [0.0, y + 1.0]], rune::Color::BLACK.alpha(0.16));
            y += 3.0;
        }
        shape
    })
    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
    .key("home-cover-scan")
    .a11y_hidden();
    let glass = col()
        .w_full()
        .h_full()
        .center()
        .radius(10.0 * k)
        .fill(l.c("boot-screen-mid"))
        .border(1.0, l.c("boot-screen-cold"))
        .clip_rounded(10.0 * k)
        .key("home-cover-glass")
        .child(screen)
        .child(scan);
    let machine = col()
        .absolute(m.x as f32, m.y as f32)
        .w(m.width as f32)
        .h(m.height as f32)
        .padding(9.0 * k)
        .radius(18.0 * k)
        .fill(l.c("boot-bezel-hi"))
        .border(1.0, l.c("border-hairline"))
        .key("home-cover-machine")
        .child(glass)
        .role("img")
        .label(format!("Locust. {line}"));

    let mut card = col()
        .w_full()
        .h(at.height as f32)
        .mt(at.reach as f32)
        .key("home-cover")
        .on_layout_tx(|s: &mut State, [_, _, w, _], tx| {
            let w = f64::from(w);
            if s.home.fit.card_width == Some(w) {
                tx.live_only();
                return;
            }
            s.home.fit.card_width = Some(w);
            s.home.fit.decide();
        })
        .child(machine);
    #[cfg(test)]
    DRAWN_COVER.with(|d| d.borrow_mut().clear());
    for (mate, place) in COVER_CAST.iter().zip(at.faces) {
        let beat = beat_of(mate.part);
        let size = place.width as f32;
        let pose_state = locust_core::bot_rig::BotAvatarState::from_str(if ready { mate.state } else { "default" });
        let eyes = beat.eyes.map(|e| [e[0].to_owned(), e[1].to_owned()]).unwrap_or_else(|| locust_core::bot_rig::SCREEN_RESTING_EYES.map(str::to_owned));
        let snap = crate::look::bot_stage::BotSnapshot {
            pose: locust_core::bot_rig::rest_pose(pose_state),
            eyes: Some(eyes.clone()),
            tone: beat.phosphor,
            flash: None,
            flash_progress: 1.0,
            power_on: None,
            shut: 0.0,
            lag_x: 0.0,
            lag_y: 0.0,
            // Not drawn: the hop. The rig's lift would hold for the whole beat, which reads as a float, not a hop.
            hop: false,
            blinking: false,
            screen: true,
            plush: s.settings_panel.settings.plush,
            s_seconds: 0.0,
            hovered: false,
        };
        let face = locust_core::bot_spec::preset_of(mate.bot).map_or(locust_core::bot_spec::BotFace::Eyes, |p| p.face);
        // No presence dot and no waiting ring on a character (0.610): who is really working is said on the glass.
        let shape = crate::look::bots::draw_bot_shape(&crate::look::bots::BotDrawSpec {
            shape: mate.bot,
            face,
            hue: mate.hue.unwrap_or(""),
            size,
            light_theme: !l.k.dark,
            presence: None,
            ring: false,
            snap: &snap,
        });
        let beat_name = if ready { beat.name } else { "still" };
        let floating = ready && mate.floats;
        let mut bot = rune::canvas(shape).size(size, size).key(format!("home-cover-bot-{}-{beat_name}", mate.key)).a11y_hidden();
        if floating {
            bot = bot.looping(rune::LoopSpec::slide(3.8, [0.0, 0.0], [0.0, -0.06 * size]).easing(rune::Easing::Piecewise(FLOAT_CURVE)));
        }
        let asleep = ready && mate.part == CoverPart::Sleeper;
        let mut slot = col().absolute(place.x as f32, place.y as f32).w(size).h(size).key(format!("home-cover-face-{}", mate.key)).child(bot);
        if asleep {
            slot = slot.child(
                text("z z z")
                    .font(rune::TextStyle::new((size * 0.18).max(8.0), 700.0).mono())
                    .color(crate::look::bots::color_from_css("#99eeff", 1.0))
                    .absolute(size * 0.7, size * 0.06)
                    .key("home-cover-zzz")
                    .a11y_hidden()
                    .looping(rune::LoopSpec::opacity(2.4, 0.0, 0.35, 1.0)),
            );
        }
        #[cfg(test)]
        DRAWN_COVER.with(|d| d.borrow_mut().push(CoverDrawn {
            key: mate.key, bot: mate.bot, hue: mate.hue, state: if ready { mate.state } else { "still" }, beat: beat_name, eyes: Some(eyes), tone: beat.phosphor, floating, zzz: asleep,
        }));
        card = card.child(slot);
    }
    // The parts' clock: an empty mark keyed by the beats and whether the cover plays. Each new key is laid out once,
    // and that is where the clock is started or held and the one wake is asked for, at the next change of beat. The
    // cover is drawn again only then, never on a tick.
    let clock_key = format!("home-cover-clock-{playing}-{}-{}", worker.name, thinker.name);
    card.child(
        row()
            .absolute(0.0, 0.0)
            .w(0.0)
            .h(0.0)
            .key(clock_key)
            .a11y_hidden()
            .on_layout_tx(move |s: &mut State, _, tx| {
                let now = cover_now();
                let playing = cover_playing(s);
                s.home.cover_clock.set_playing(playing, now);
                if playing {
                    let (_, _, left) = locust_core::cover_script::cover_beats(s.home.cover_clock.elapsed(now));
                    tx.wake_after(std::time::Duration::from_secs_f64(left + 0.03));
                }
            }),
    )
}

/// A Home section's head: its title, a count when it has one, and what sits at its right.
fn section_head(l: &crate::look::controls::Look, title: &str, count: Option<usize>, right: Option<El>) -> El {
    let mut head = row().w_full().center_y().gap(8.0).pt(10.0).child(text(title.to_owned()).font(rune::TextStyle::new(14.0, 500.0)).color(l.c("text-primary")));
    if let Some(n) = count {
        head = head.child(text(n.to_string()).font(l.ty.mono).color(l.c("text-muted")));
    }
    head = head.child(rune::spacer());
    if let Some(right) = right {
        head = head.child(right);
    }
    head
}

/// Use the latest discovery answer without turning a pending probe into a sign-in wall.
pub(super) fn runtime_blocked_label(s: &State, runtime: Option<&str>) -> Option<&'static str> {
    if let Some(agents) = &s.settings_panel.agents {
        return locust_core::teammate_reach::blocked_label(
            runtime.and_then(|id| agents.iter().find(|a| a.id == id)),
            locust_core::teammate_reach::runtime_reach(agents, true),
        );
    }
    // Before richer discovery answers, keep the boot sweep's proven outcomes only.
    s.home.boot.probes.iter().find(|p| Some(p.id.as_str()) == runtime)
        .and_then(|p| match p.outcome { Some("missing") => Some("AI agent not installed"), Some("needs-signin") => Some("Sign-in needed"), _ => None })
}

/// The live mission's runtime, then the newest recorded runtime, then the saved route.
/// Owners come from the cached roster record, so drawing never reads the store.
pub(super) fn runtime_of_teammate<'a>(s: &'a State, id: &str, route: Option<&'a str>) -> Option<&'a str> {
    let mut owned: Vec<_> = s.ledger_summaries.values().filter(|m|
        s.people_panel.team["missionOwners"][&m.mission_id].as_str() == Some(id)
    ).collect();
    owned.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.mission_id.cmp(&b.mission_id)));
    owned.iter().find(|m| m.running).map(|m| m.runtime.as_str()).filter(|r| !r.is_empty())
        .or_else(|| owned.iter().find(|m| !m.runtime.is_empty()).map(|m| m.runtime.as_str()))
        .or(route)
}

/// The same projected state supplies both roster surfaces and the board group.
pub(super) fn teammate_status(s: &State, id: &str, busy: bool, runtime: Option<&str>, role: &str) -> locust_core::teammate_reach::StatusView {
    use locust_core::teammate_reach::{Reach, runtime_reach, status_view};
    let owns = |mission: &str| s.people_panel.team["missionOwners"][mission].as_str() == Some(id);
    let approvals = s.home.waiting_for.keys().filter(|key| owns(key)).count();
    let running = busy || s.flight.as_ref().is_some_and(|f| f.teammate_id == id)
        || s.ledger.as_ref().is_some_and(|p| p.header.id == id && p.snapshot.running);
    let runtime = runtime_of_teammate(s, id, runtime);
    if let Some(agents) = &s.settings_panel.agents {
        return status_view(runtime.and_then(|id| agents.iter().find(|a| a.id == id)), runtime_reach(agents, true), running, approvals, role);
    }
    let mut view = status_view(None, Reach::default(), running, approvals, role);
    if let Some(label) = runtime_blocked_label(s, runtime) {
        view = locust_core::teammate_reach::StatusView { status: "blocked", label: label.into(), tone: "red", presence: "blocked" };
    }
    view
}
pub(super) fn teammate_status_color(view: &locust_core::teammate_reach::StatusView, p: &Palette) -> Color {
    match view.tone { "red" => p.red, "amber" => p.amber, "live" => p.green, _ => p.muted }
}
pub(super) fn teammate_presence(view: &locust_core::teammate_reach::StatusView, p: &Palette) -> El {
    if view.presence == "none" { return el().w(6.0).h(6.0); }
    el().w(6.0).h(6.0).radius(3.0).fill(teammate_status_color(view, p)).role("status").label(view.presence)
}
/// Use recorded owner ids, never matching a person's display name.
pub(super) fn teammate_section(s: &State, id: &str, busy: bool, runtime: Option<&str>) -> crate::team_board::Section {
    use crate::team_board::{Status, board_section_of};
    let view = teammate_status(s, id, busy, runtime, "");
    board_section_of(Some(match view.status { "approval-needed" => Status::ApprovalNeeded, "blocked" => Status::Blocked,
        "working" => Status::Working, _ => Status::Idle }), s.home.teammates_unseen.contains(id))
}

/// FirstLaunch.tsx's `lc-intro`: one line under the machine, centred on it (Colin, 2026-09-24: "text is way off
/// center"), for a first run only, which lasts until the first conversation. Said once discovery has answered. Its
/// second sentence is a promise about OpenCode's price list, withdrawn only on evidence (`free_start_still_free`).
fn intro_line(s: &State) -> Option<String> {
    let found = s.settings_panel.agents.as_ref()?;
    if !(s.teammates.is_empty() || s.recent.is_empty()) {
        return None;
    }
    let opencode_ready = found.iter().any(|f| f.id == locust_core::agents::FREE_START_RUNTIME && f.status == locust_core::agents::Probe::Ready);
    let listed = s.models.values().flatten().filter(|a| a.runtime == locust_core::agents::FREE_START_RUNTIME).flat_map(|a| a.models.iter().map(|m| m.id.as_str()));
    Some(match locust_core::home_order::free_start_still_free(opencode_ready, listed) {
        locust_core::home_order::FreeStart::No => boot_view::INTRO,
        _ => boot_view::INTRO_WITH_FREE,
    }.to_owned())
}

/// `waitingLine` for the card a teammate's conversation waits on (`home_team_state::waiting_line`): the question as
/// asked, the command it wants to run, or the approval it asks for. Owners are recorded ids, never names.
fn waiting_line_for(s: &State, teammate: &str) -> Option<String> {
    use locust_core::home_team_state::{Waiting, waiting_line};
    let owners = &s.people_panel.team["missionOwners"];
    let (mission, ask) = s.home.asks.iter().filter(|(m, _)| owners[m.as_str()].as_str() == Some(teammate)).min_by_key(|(_, a)| a.shown)?;
    let summary = s.home.waiting_for.get(mission).map(String::as_str).unwrap_or_default();
    waiting_line(match ask.kind.as_str() {
        "question" => Waiting::Approval { what: summary, asking: true, command: false },
        "command" => Waiting::Approval { what: ask.command.as_deref().unwrap_or(summary), asking: false, command: true },
        _ => Waiting::Approval { what: summary, asking: false, command: false },
    })
}

/// When a teammate last worked: the newest conversation recorded as theirs (by owner id).
fn last_worked_at(s: &State, teammate: &str) -> Option<String> {
    let owners = &s.people_panel.team["missionOwners"];
    s.recent.iter().filter(|r| owners[r.id.as_str()].as_str() == Some(teammate)).map(|r| r.updated.clone()).max()
}

/// Home's AI agents: the whole list, or, once they are all ready (`agents_fold`), one line -- "Connected accounts",
/// how many are ready, their marks and Show all -- so a returning Home fits its team on one screen (FirstLaunch.tsx).
fn home_agents(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let found = s.settings_panel.agents.as_deref().unwrap_or_default();
    let rows: Vec<locust_core::home_order::AgentRow> = found.iter().map(|f| locust_core::home_order::AgentRow { installed: f.installed, ready: f.ready, checking: f.checking }).collect();
    if s.settings_panel.agents.is_none() || !locust_core::home_order::agents_fold(&rows, s.teammates.len(), s.home.agents_open) {
        return crate::agents_section::section(s, p).key("home-agents");
    }
    let ready: Vec<&locust_core::agents::Found> = found.iter().filter(|f| f.ready).collect();
    let checking: Vec<&locust_core::agents::Found> = found.iter().filter(|f| f.installed && f.checking && !f.ready).collect();
    let mut line = row()
        .key("home-agents")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text("Connected accounts").font(l.ty.control).color(l.c("text-primary")))
        .child(text(format!("{} ready", ready.len())).font(l.ty.label).color(l.c("green-text")).key("home-agents-ready"));
    match checking.as_slice() {
        [] => {}
        [one] => line = line.child(text(format!("· checking {}", one.display_name)).font(l.ty.label).color(l.c("text-muted"))),
        many => line = line.child(text(format!("· checking {} more", many.len())).font(l.ty.label).color(l.c("text-muted"))),
    }
    let mut marks = row().gap(6.0).center_y();
    for f in &ready {
        if let Some(mark) = crate::look::controls::agent_mark(&l, &f.id, 13.0, l.c("text-secondary")) {
            marks = marks.child(el().child(mark).role("img").label(f.display_name.clone()).help(f.display_name.clone()));
        }
    }
    line.child(marks).child(rune::spacer()).child(
        text("Show all")
            .font(l.ty.label)
            .color(l.c("text-secondary"))
            .px(6.0)
            .py(2.0)
            .radius(4.0)
            .hover(l.c("hover-surface"))
            .role("button")
            .key("home-agents-show-all")
            .on_press(|s: &mut State| s.home.agents_open = true),
    )
}

pub(super) fn home_view(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    // Home's spacing: normal, or tight when even the smallest cover does not fit (FirstLaunch.tsx `homeIsShort`).
    let (pad, gap) = locust_core::home_cover::home_spacing(s.home.fit.tight);
    // The sections in an inner column that is only as tall as they are: in the window, the scroll pane stretches its
    // content to its own height, so the outer column read as the pane's height and Home was always short.
    let mut body = HomeColumn { el: col().w_full().gap(gap as f32), count: 0 };
    body = body.child(
        row()
            .w_full()
            .child(rune::spacer())
            .child(
                crate::look::controls::head_icon(&crate::look::icons::CLOSE, "Close Home", "Close")
                    .on_press(|s: &mut State| s.home.home_open = false)
                    .build(l.k),
            )
            .key("home-close-row"),
    );
    body = body.child(cover(s, &l, cover_status(s)));
    if let Some(line) = intro_line(s) {
        body = body.child(row().w_full().justify_center().child(text(line).font(l.ty.body).color(l.c("text-secondary")).wrap().key("home-intro")));
    }
    // FirstLaunch.tsx `accountsLead`: with nothing installed, the press that makes a team runnable comes before the
    // teams and the folder; once anything is installed, after them, so a returning Home never reorders.
    let installed: Option<Vec<bool>> = s.settings_panel.agents.as_ref().map(|found| found.iter().map(|f| f.installed).collect());
    let accounts_lead = locust_core::home_order::accounts_lead(installed.as_deref());
    // home-fits-one-screen (0.799): with a team, the accounts sit between the cover and the team, and the starters
    // after it, so a returning Home reads cover, accounts, team, what to start.
    let accounts_above = accounts_lead || !s.teammates.is_empty();
    if accounts_above {
        body = body.child(home_agents(s, p));
    }

    // Folder strip
    let folder_line = s
        .home
        .folder_path
        .clone()
        .unwrap_or_else(|| NO_FOLDER_CHOSEN.into());
    body = body.child(
        col()
            .w_full()
            .gap(6.0)
            .padding(12.0)
            .fill(l.c("bg-raised"))
            .border(1.0, l.c("border-card"))
            .radius(10.0)
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(crate::look::controls::glyph(&l, &crate::look::icons::FOLDER, 14.0, Some(l.c("text-muted"))))
                    .child(text(folder_line).font(l.ty.control).color(l.c("text-primary")).wrap()),
            )
            .child(text(FOLDER_LEDE.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap())
            .child(row().pt(4.0).child(crate::look::controls::ghost_button(&l, CHOOSE_FOLDER).on_press_tx(|s: &mut State, tx: &mut Tx| {
                crate::restored_folders_card::choose_workspace(s, tx);
            }))),
    );
    if s.home.folder_invented {
        body = body.child(
            text(MADE_FOLDER.to_owned())
                .font(type_::LABEL)
                .color(p.amber),
        );
        if let Some(path) = &s.home.folder_path {
            body = body.child(
                text(folders::made_folder_note(path))
                    .font(type_::LABEL)
                    .color(p.amber)
                    .wrap(),
            );
        }
    }

    // Away
    if let Some(summary) = &s.home.away {
        let counts = away::away_counts(summary);
        let line = away::away_line(Some(counts)).unwrap_or_default().replacen(
            "Since you were away: ",
            "",
            1,
        );
        body = body.child(
            col()
                .w_full()
                .gap(6.0)
                .padding(10.0)
                .fill(p.raised)
                .radius(8.0)
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .child(
                            text(away::HEADING.to_owned())
                                .font(type_::TITLE)
                                .color(p.text)
                                .grow(1.0),
                        )
                        .child(text(line).font(type_::LABEL).color(p.muted).grow(1.0))
                        .child(button(away::GOT_IT, p).on_press(|s: &mut State| mark_away_seen(s))),
                )
                .child({
                    let mut list = col().w_full().gap(4.0);
                    if let Some(blurb) = away::waiting_blurb(counts.waiting) {
                        list = list.child(text(blurb).font(type_::LABEL).color(p.amber).wrap());
                    }
                    for item in summary
                        .failed
                        .iter()
                        .take(6)
                        .chain(summary.ran.iter().take(6))
                    {
                        let mission_id = item.mission_id.clone();
                        let title = item.title.clone();
                        let kind = if summary
                            .failed
                            .iter()
                            .any(|f| f.mission_id == item.mission_id)
                        {
                            "failed"
                        } else {
                            "finished"
                        };
                        let when =
                            away::when_label(&item.at, chrono::Utc::now().timestamp_millis());
                        // Under the title, what the run came to: what it did, or why it failed (AwayList).
                        let mut entry = col()
                            .w_full()
                            .gap(2.0)
                            .key(format!("away-row-{mission_id}"))
                            .child(text(format!("{title} · {kind} {when}")).font(type_::LABEL).color(p.secondary).wrap());
                        if let Some(said) = s.home.away_said.get(&item.mission_id) {
                            entry = entry.child(text(said.clone()).key(format!("away-said-{mission_id}")).font(type_::LABEL).color(p.muted).wrap());
                        }
                        list = list.child(entry.on_press(move |s: &mut State| {
                            s.opening_conversation = Some(mission_id.clone());
                            s.home.home_open = false;
                        }));
                    }
                    list
                }),
        );
    }

    // Team / templates
    if s.teammates.is_empty() {
        body = body
            .child(
                text(START_WITH_A_TEAM.to_owned())
                    .font(type_::TITLE)
                    .color(p.text),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(button(TEAM_FROM_IMAGE, p).key("home-team-from-image").on_press_tx(|_s: &mut State, tx: &mut Tx| {
                        crate::people_panel::import_team_from_image(tx);
                    }))
                    .child(button(NEW_TEAMMATE, p).on_press(|s: &mut State| {
                        crate::people_panel::open(s);
                        s.home.home_open = false;
                    })),
            );
        for template in &TEAM_TEMPLATES {
            let id = template.template_id;
            let title = template.title;
            let line = template.line;
            body = body.child(
                col()
                    .w_full()
                    .gap(4.0)
                    .padding(10.0)
                    .fill(p.field)
                    .radius(8.0)
                    .child(text(title.to_owned()).font(type_::BODY).color(p.text))
                    .child(
                        text(line.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    )
                    .child(
                        primary("Add this team", p)
                            .key(format!("home-template-{id}"))
                            .on_press(move |s: &mut State| apply_template(s, id)),
                    ),
            );
        }
    } else {
        let add = crate::look::controls::ghost_button(&l, &format!("+  {NEW_TEAMMATE}")).on_press(|s: &mut State| {
            crate::people_panel::open(s);
            s.home.home_open = false;
        });
        if let Some(usage) = crate::usage_line::line(s, &l) {
            body = body.child(usage);
        }
        if let Some(scout) = crate::scout_cards::home_section(s, &l) {
            body = body.child(scout);
        }
        body = body.child(section_head(&l, YOUR_TEAM, Some(s.teammates.len()), Some(add)));
        // HomeTeam.tsx's cards: a square for the face (faces come in C4), the name, the role, the agent and model.
        let faces = crate::teammate_faces::views(s);
        let groups = crate::team_board::team_board(&s.teammates, |mate| teammate_section(s, &mate.id, mate.busy, mate.runtime.as_deref()));
        let grouped = groups.is_some();
        let has_ready = groups.as_ref().is_none_or(|groups| groups.iter().any(|(section, _)| *section == crate::team_board::Section::Ready));
        for (section, members) in groups.unwrap_or_else(|| vec![(crate::team_board::Section::Ready, s.teammates.iter().collect())]) {
        if grouped { body = body.child(section_head(&l, section.title(), Some(members.len()), None)); }
        let mut grid = row().w_full().gap(10.0).flex_wrap();
        for mate in members {
            let runtime = mate.runtime.as_deref().unwrap_or("codex");
            let oc_models = s
                .models
                .get(&mate.id)
                .or_else(|| s.models.get(CARRY_TARGET))
                .and_then(|agents| agents.iter().find(|a| a.runtime == "opencode"))
                .map(|a| a.models.as_slice())
                .unwrap_or(&[]);
            let dropped = locust_core::connectors_off::dropped_free_model(
                runtime,
                mate.model.as_deref(),
                oc_models,
            );
            let mut agent = row().gap(5.0).center_y();
            if let Some(mark) = crate::look::controls::agent_mark(&l, runtime, 11.0, l.c("text-muted")) {
                agent = agent.child(mark);
            }
            agent = agent.child(
                text(mate.model.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| "account default".into()))
                    .font(rune::TextStyle::new(11.5, 400.0))
                    .color(if dropped { p.amber } else { l.c("text-muted") })
                    .truncate()
                    .shrink(),
            );
            let looked = crate::scout_cards::looked_line_for_teammate(s, &mate.id);
            let status = teammate_status(s, &mate.id, mate.busy, mate.runtime.as_deref(), &mate.role);
            // HomeTeam.tsx (0.606/0.609/0.610): the state beside the name, the same fact the sidebar face draws; while
            // they wait, what on, under the name in the role's place; idle, what they last did on hover.
            let activity = faces.get(&mate.id).map_or(locust_core::bot_rig::FaceActivity::Idle, |v| v.activity);
            let state = locust_core::home_team_state::team_card_state(Some(&status), activity);
            let doing = state.as_ref().filter(|st| st.tone != "red").and_then(|_| waiting_line_for(s, &mate.id));
            let looked_line = looked.as_ref().map(|(line, _)| line.clone());
            let busy_line = doing.is_some();
            let watch_line = doing.is_none() && looked_line.is_some();
            let sub_text = doing.or(looked_line).unwrap_or_else(|| mate.role.clone());
            let mut name_row = row().gap(6.0).center_y().child(text(mate.name.clone()).font(rune::TextStyle::new(14.0, 500.0)).color(l.c("text-name")).truncate().shrink());
            if let Some(st) = &state {
                let tone = match st.tone { "red" => p.red, "amber" => p.amber, _ => p.green };
                name_row = name_row.child(text(st.word.clone()).font(rune::TextStyle::new(11.5, 500.0)).color(tone).label(st.spoken.clone()));
            }
            let last_worked = locust_core::home_team_state::last_worked_phrase(last_worked_at(s, &mate.id).as_deref(), chrono::Utc::now());
            let mut card = row()
                .gap(12.0)
                .padding(12.0)
                .center_y()
                .radius(10.0)
                .fill(l.c("bg-raised"))
                .border(1.0, l.c("border-card"))
                .child(crate::look::faces::tile(s.image_cache.as_deref(), &mate.id, &mate.name, s.looks.get(&mate.id), s.settings_panel.settings.terminal_faces, &l))
                .child(
                    col()
                        .gap(2.0)
                        .grow(1.0)
                        .shrink()
                        .child(name_row)
                        .child(row().gap(5.0).center_y().child(teammate_presence(&status, p)).child({
                            let line = text(sub_text).font(rune::TextStyle::new(12.5, 400.0)).color(if busy_line { l.c("text-secondary") } else { l.c("text-muted") }).truncate();
                            // Scout's watch takes its role's place on its own card; the role is still the line's hover.
                            if watch_line { line.help(mate.role.clone()).key("home-card-watch") } else { line }
                        }))
                        .child(agent),
                )
                .key(format!("home-card-{}", mate.id));
            if state.is_none() { card = card.help(last_worked); }
            if let Some((_, looking)) = looked {
                let mut eye = row()
                    .padding(6.0)
                    .radius(6.0)
                    .hover(l.c("hover-surface"))
                    .role("button")
                    .label(format!("{}: look now", mate.name))
                    .help(if looking { "Looking…" } else { "Look now" })
                    .key("scout-look-now")
                    .child(crate::look::controls::glyph(
                        &l,
                        &crate::look::icons::EYE,
                        14.0,
                        Some(if looking { l.c("text-muted") } else { l.c("text-secondary") }),
                    ));
                if !looking {
                    eye = eye.on_press(|s: &mut State| crate::scout_cards::look_now(s));
                }
                grid = grid.child(
                    row()
                        .w(246.0)
                        .gap(4.0)
                        .center_y()
                        .child(card.grow(1.0).shrink())
                        .child(eye),
                );
            } else {
                grid = grid.child(card.w(246.0));
            }
        }
        if section == crate::team_board::Section::Ready && crate::scout_cards::invite_offer_active(s) {
            grid = grid.child(crate::scout_cards::invite_team_tile(&l));
        }
        body = body.child(grid);
        }
        // The invitation is an action, not a Ready teammate: keep it reachable
        // even when every teammate belongs to an urgent section.
        if !has_ready && crate::scout_cards::invite_offer_active(s) {
            body = body.child(row().w_full().gap(10.0).child(crate::scout_cards::invite_team_tile(&l)));
        }
    }

    if !accounts_above {
        body = body.child(home_agents(s, p));
    }

    // Starters
    let roles: Vec<&str> = s.teammates.iter().map(|t| t.role.as_str()).collect();
    let starters = starters::starters_for_roles(&roles);
    let links = row()
        .gap(4.0)
        .child(crate::look::controls::composer_chip(&l, None, START_FROM_ISSUE, false).on_press(|s: &mut State| {
            s.status = format!("{START_FROM_ISSUE} — runs in Locust for now");
            s.status_tone = StatusTone::Muted;
        }))
        .child(
            crate::look::controls::composer_chip(
                &l,
                None,
                COMPARE_MODELS,
                false,
            )
            .child(crate::look::controls::glyph(&l, &crate::look::icons::CHEVRON_RIGHT, 12.0, Some(l.c("text-muted"))))
            .on_press(|s: &mut State| {
                s.status = format!("{COMPARE_MODELS} — runs in Locust for now");
                s.status_tone = StatusTone::Muted;
            }),
        );
    body = body
        .child(section_head(&l, START_A_CONVERSATION, None, Some(links)))
        .child(text(format!("{PICK_ONE} {CONNECT_RUNTIME}")).font(l.ty.body).color(l.c("text-muted")).wrap());
    // StarterList.tsx: two columns of rows, each its glyph and its words over a hairline.
    let mut list = row().w_full().gap(0.0).flex_wrap();
    for starter in starters {
        let starter_id = starter.id.clone();
        let prompt = starter.prompt.to_owned();
        let mode = starter.mode.map(str::to_owned);
        let compare_label = if starter.compare_changes { COMPARE } else { "" };
        let mut item = row()
            .w_frac(0.5)
            .pr(24.0)
            .child(
                row()
                    .w_full()
                    .gap(12.0)
                    .py(9.0)
                    .px(4.0)
                    .center_y()
                    .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
                    .hover(l.c("hover-surface"))
                    .child(crate::look::controls::glyph(&l, starter_icon(starter.label), 15.0, Some(l.c("text-muted"))))
                    .child(text(starter.label.to_owned()).font(rune::TextStyle::new(13.5, 400.0)).color(l.c("text-primary")).grow(1.0))
                    .child(text(compare_label.to_owned()).font(l.ty.mono).color(l.c("text-muted")))
                    .role("button")
                    .label(starter.label.to_owned()),
            )
            .on_press(move |s: &mut State| {
                apply_starter(s, &starter_id, &prompt, mode.as_deref());
            });
        item = item.key(format!("starter-{}", starter.id));
        list = list.child(item);
    }
    body = body.child(list);

    // What in the column does not move with the cover: the sections' own height as drawn, less the gaps between them
    // and the cover's footprint as drawn. Measured from itself, so a cover that grows never shrinks the room it was
    // given. The column's height kept beside it is the sections and the padding: the page as tall as it is drawn.
    let count = body.count;
    let tight_now = s.home.fit.tight;
    let place = locust_core::home_cover::cover_place(s.home.fit.card_width.unwrap_or(0.0), s.home.fit.grow.unwrap_or(1.0));
    let footprint = place.height + place.reach;
    let sections_el = body.el.key("home-sections").on_layout_tx(move |s: &mut State, [_, _, _, h], tx| {
        let h = f64::from(h);
        let sections = (h - gap * count.saturating_sub(1) as f64 - footprint).max(0.0);
        let measured = Some((h + 2.0 * pad, sections, count, tight_now));
        if s.home.fit.column == measured {
            tx.live_only();
            return;
        }
        s.home.fit.column = measured;
        s.home.fit.decide();
    });
    let column = col().w_full().max_w(760.0).pt(pad as f32).pb(pad as f32).child(sections_el);
    // A centred column, as Home's `max-width` holds it in Electron. It fills the window's column and scrolls inside
    // it: without `grow` and `min_h(0)` the pane grew to its content, nothing scrolled, and what fell below the
    // window could not be pressed (Scout's Send, when the accounts came above the team).
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(28.0)
        .items_center()
        .child(column)
        .scroll(ScrollSpec { stick_bottom: false })
        .key("home-pane")
        .on_layout_tx(|s: &mut State, [_, _, _, h], tx| {
            let h = f64::from(h);
            if s.home.fit.pane_height == Some(h) {
                tx.live_only();
                return;
            }
            s.home.fit.pane_height = Some(h);
            s.home.fit.decide();
        })
}

/// Home's column, counting what it holds: the measure needs the number of its gaps (FirstLaunch.tsx `children.length`).
struct HomeColumn {
    el: El,
    count: usize,
}

impl HomeColumn {
    fn child(mut self, child: El) -> Self {
        self.count += 1;
        self.el = self.el.child(child);
        self
    }
}

/// The Board with one teammate's conversations only (Electron opens their list from the rail card).
pub(super) fn open_board_for(s: &mut State, teammate: &str) {
    open_board(s);
    s.home.board_teammate = Some(teammate.to_owned());
}

pub(super) fn board_view(s: &State, p: &Palette) -> El {
    let mut missions = recent_as_missions(s);
    if let Some(name) = &s.home.board_teammate {
        missions.retain(|m| {
            std::iter::once(&m.mission_id).chain(m.member_ids.iter()).any(|id| s.recent.iter().any(|r| &r.id == id && &r.teammate == name))
        });
    }
    let facts = ConversationBoardFacts {
        needs_you: s.home.asks.keys().cloned().collect(),
        to_look_at: s.home.to_look_at.clone(),
    };
    let scout_waits = crate::scout_cards::board_waiting_cards(s);
    let board = conversation_board::conversation_board_with_extra(
        &missions,
        &[],
        &HashSet::new(),
        &facts,
        scout_waits.len(),
    );
    let meta = conversation_board::board_meta_with_extra(board.as_deref(), scout_waits.len());
    let mut body = col().w_full().gap(10.0).padding(16.0);
    body = body.child(
        row()
            .w_full()
            .center_y()
            .child(
                text(BOARD_TITLE.to_owned())
                    .font(type_::TITLE)
                    .color(p.text)
                    .grow(1.0),
            )
            .child(text(meta).font(type_::LABEL).color(p.muted).grow(1.0))
            .child(button("Close", p).on_press(|s: &mut State| s.home.board_open = false)),
    );
    if let Some(name) = &s.home.board_teammate {
        body = body.child(
            row()
                .key("board-narrowed")
                .w_full()
                .gap(10.0)
                .center_y()
                .child(text(format!("Only {name}'s conversations")).font(type_::LABEL).color(p.secondary))
                .child(button("Show everyone's", p).key("board-everyone").on_press(|s: &mut State| s.home.board_teammate = None)),
        );
    }

    match board {
        None => {
            body = body
                .child(
                    text(BOARD_CLEAR.to_owned())
                        .font(type_::TITLE)
                        .color(p.text),
                )
                .child(
                    text(BOARD_CLEAR_LINE.to_owned())
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap(),
                );
        }
        Some(sections) => {
            let l = crate::look::controls::Look::new(p.dark);
            for section in sections {
                let section_count = if section.key == ConversationBoardKey::NeedsYou {
                    section.members.len() + scout_waits.len()
                } else {
                    section.members.len()
                };
                let mut sec_header = row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .padding(4.0)
                    .child(text(section.title.to_owned()).font(type_::TITLE).color(p.text));
                if section.key == ConversationBoardKey::NeedsYou && section_count > 0 {
                    sec_header = sec_header.child(
                        text(section_count.to_string())
                            .font(type_::LABEL)
                            .color(l.c("amber-text"))
                            .key("board-needs-you-count"),
                    );
                }
                body = body.child(sec_header);
                if section.key == ConversationBoardKey::NeedsYou {
                    for card in &scout_waits {
                        body = body.child(crate::scout_cards::board_wait_card(s, card, &l));
                    }
                }
                for card in section.members {
                    let mission_id = card.row.mission_id.clone();
                    let title = card.row.title.clone();
                    let under = card.parent_id.as_ref().and_then(|parent| {
                        missions
                            .iter()
                            .find(|m| m.mission_id == *parent)
                            .map(|m| conversation_board::under_title(&m.title))
                    });
                    let ask = s.home.asks.get(&mission_id).cloned().or_else(|| {
                        card.row
                            .member_ids
                            .iter()
                            .find_map(|id| s.home.asks.get(id).cloned())
                    });
                    let state_line = match section.key {
                        ConversationBoardKey::NeedsYou => s
                            .home
                            .waiting_for
                            .get(&mission_id)
                            .cloned()
                            .unwrap_or_else(|| WAITING_ON_YOU.into()),
                        ConversationBoardKey::Working => WORKING.into(),
                        ConversationBoardKey::ToLookAt => "finished".into(),
                        ConversationBoardKey::Done => {
                            if card.row.phase == "failed" {
                                "failed".into()
                            } else if card.row.phase == "completed" {
                                "done".into()
                            } else {
                                "stopped".into()
                            }
                        }
                    };
                    // The card's words, beside its teammate's face (built below).
                    let mut card_el = col().grow(1.0).shrink().gap(4.0);
                    // Whose conversation it is: BoardView's face beside the title, the name under it.
                    let who = s.recent.iter().find(|r| r.id == mission_id).map(|r| r.teammate.clone());
                    let mate = who.as_ref().and_then(|name| s.teammates.iter().find(|t| &t.name == name));
                    let l = crate::look::controls::Look::new(p.dark);
                    let face = mate.map(|mate| crate::look::faces::face(s.image_cache.as_deref(), &mate.id, &mate.name, s.looks.get(&mate.id), 26.0, s.settings_panel.settings.terminal_faces, &l));
                    card_el = card_el.child(text(title).font(type_::BODY).color(p.text).truncate());
                    if let Some(who) = who {
                        card_el = card_el.child(text(who).font(type_::LABEL).color(p.secondary));
                    }
                    if let Some(under) = under {
                        card_el = card_el.child(text(under).font(type_::LABEL).color(p.muted));
                    }
                    if let Some(ask) = ask {
                        let state = if ask.command.is_some() {
                            WANTS_TO_RUN.into()
                        } else {
                            state_line.clone()
                        };
                        card_el = card_el.child(text(state).font(type_::LABEL).color(p.amber));
                        if let Some(command) = &ask.command {
                            card_el = card_el.child(
                                text(command.clone())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap(),
                            );
                        }
                        if let Some(where_) = &ask.where_ {
                            card_el = card_el.child(
                                text(conversation_board::in_where(where_))
                                    .font(type_::LABEL)
                                    .color(p.muted),
                            );
                        }
                        let approval_id = ask.approval_id.clone();
                        if ask.command.is_some() {
                            let deny_id = approval_id.clone();
                            card_el = card_el.child(
                                row()
                                    .gap(8.0)
                                    .child(primary(ALLOW_ONCE, p).key(format!("board-allow-{mission_id}")).on_press(move |s: &mut State| {
                                        answer_board(s, &approval_id, true)
                                    }))
                                    .child(button(DENY, p).on_press(move |s: &mut State| {
                                        answer_board(s, &deny_id, false)
                                    })),
                            );
                        } else {
                            let label = if ask.kind == "question" {
                                ANSWER
                            } else {
                                REVIEW
                            };
                            let open_id = mission_id.clone();
                            card_el =
                                card_el.child(button(label, p).on_press(move |s: &mut State| {
                                    s.opening_conversation = Some(open_id.clone());
                                    s.home.board_open = false;
                                }));
                        }
                    } else {
                        card_el = card_el.child(text(state_line).font(type_::LABEL).color(p.muted));
                    }
                    let open_id = mission_id.clone();
                    let mut card_el_ = row().w_full().gap(12.0).padding(10.0).fill(p.raised).radius(8.0);
                    if let Some(face) = face {
                        card_el_ = card_el_.child(face);
                    }
                    let card_el = card_el_.child(card_el);
                    body = body.child(card_el.on_press(move |s: &mut State| {
                        s.opening_conversation = Some(open_id.clone());
                        s.home.to_look_at.remove(&open_id);
                        s.home.board_open = false;
                    }));
                }
            }
        }
    }
    body.scroll(ScrollSpec {
        stick_bottom: false,
    })
}

pub(super) fn boot_view_ui(s: &State, p: &Palette) -> El {
    let now = s.home.boot.finished.map(|(at, _, _)| at + 800).unwrap_or(0);
    let phase = s
        .home
        .boot
        .finished
        .map(|(at, _, _)| boot_view::phase_at(at, now))
        .unwrap_or(boot_view::BootPhase::Idle);
    let _ = matches!(
        phase,
        boot_view::BootPhase::Settling
            | boot_view::BootPhase::Dissolving
            | boot_view::BootPhase::Gone
            | boot_view::BootPhase::Settled
            | boot_view::BootPhase::Probing
            | boot_view::BootPhase::Idle
    );
    let view = s
        .home
        .boot_view
        .clone()
        .unwrap_or_else(|| boot_view::boot_view(&s.home.boot, phase, now));
    let mut body = col().w_full().gap(8.0).padding(16.0);
    body = body
        .child(
            text(boot_view::COVER_CLAIM.to_owned())
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(
            text(boot_view::LOOKING_AGENTS.to_owned())
                .font(type_::LABEL)
                .color(p.muted),
        )
        .child(
            text(boot_view::INTRO_WITH_FREE.to_owned())
                .font(type_::BODY)
                .color(p.secondary)
                .wrap(),
        )
        .child(
            text(boot_view::INTRO.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
        .child(
            row()
                .gap(8.0)
                .child(button(boot_view::USE_FREE, p).on_press(|s: &mut State| {
                    s.status = boot_view::USE_FREE.into();
                    s.status_tone = StatusTone::Muted;
                }))
                .child(
                    text(boot_view::CONNECTED_ACCOUNTS.to_owned())
                        .font(type_::LABEL)
                        .color(p.text),
                )
                .child(button(boot_view::SHOW_ALL, p).on_press(|s: &mut State| {
                    s.status = boot_view::SHOW_ALL.into();
                })),
        )
        .child(
            // The first-launch section here is still a stand-in with no agent rows to hide; FirstLaunch.tsx derives this
            // list from the rows it does not draw.
            text(boot_view::deferred_others_sentence(&[true, false]))
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
        .child(
            text(view.progress.clone())
                .font(type_::LABEL)
                .color(p.secondary),
        );
    if view.rows.is_empty() {
        body = body.child(
            text(boot_view::DISCOVERY_ERROR.to_owned())
                .font(type_::LABEL)
                .color(p.red)
                .wrap(),
        );
    }
    for pre in &view.preamble {
        let line = match &pre.tag {
            Some(tag) => format!("{}  {}  [{tag}]", pre.key, pre.value),
            None => format!("{}  {}", pre.key, pre.value),
        };
        body = body.child(text(line).font(type_::LABEL).color(p.muted));
    }
    for row in &view.rows {
        let result = row.result.clone().unwrap_or_else(|| "…".into());
        body = body.child(
            text(format!("{}  {result}  {}", row.product, row.elapsed))
                .font(type_::LABEL)
                .color(match row.tone {
                    "ready" => p.green,
                    "needs-signin" | "missing" => p.amber,
                    "error" => p.red,
                    _ => p.secondary,
                }),
        );
    }
    if !view.summary.is_empty() {
        body = body.child(
            text(view.summary.clone())
                .font(type_::LABEL)
                .color(p.text)
                .wrap(),
        );
    }
    body = body.child(
        row()
            .gap(8.0)
            .child(primary("Continue", p).on_press(|s: &mut State| {
                s.home.boot_open = false;
                s.home.first_launch_done = true;
                open_home(s);
            }))
            .child(button("Skip", p).on_press(|s: &mut State| {
                s.home.boot_open = false;
                open_home(s);
            })),
    );
    body
}

pub(super) fn whats_new_settings(s: &State, p: &Palette) -> El {
    let mut body = col().w_full().gap(8.0).padding(12.0);
    if s.home.changelog.is_empty() {
        return body.child(
            text(changelog::EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    }
    for entry in s.home.changelog.iter().take(s.home.changelog_shown) {
        body = body.child(
            text(changelog::version_heading(entry))
                .font(type_::TITLE)
                .color(p.text)
                .padding(4.0),
        );
        for group in &entry.groups {
            if let Some(label) = &group.label {
                body = body.child(
                    text(label.clone())
                        .font(type_::BODY)
                        .color(p.secondary)
                        .padding(2.0),
                );
            }
            body = body.child(
                // WhatsNew.tsx draws each group through AgentText: its Markdown, never the raw `**`.
                col().w_full().padding(2.0).child(rune_kit::Markdown::new(group.text.clone()).selectable(true).render(&markdown_style(p)).w_full()),
            );
        }
    }
    if s.home.changelog_shown < s.home.changelog.len() {
        body = body.child(button(changelog::SHOW_OLDER, p).on_press(|s: &mut State| {
            s.home.changelog_shown =
                (s.home.changelog_shown + changelog::WHATS_NEW_MORE).min(s.home.changelog.len());
        }));
    }
    body
}

pub(super) fn splash_view(s: &State, p: &Palette) -> El {
    let mut body = col()
        .w(420.0)
        .gap(10.0)
        .padding(16.0)
        .fill(p.raised)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(
                    text(changelog::SPLASH_TITLE.to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| {
                    s.home.splash_open = false;
                    s.home.seen_version = Some(s.home.current_version.clone());
                })),
        );
    for entry in &s.home.splash_entries {
        body = body.child(
            text(changelog::version_heading(entry))
                .font(type_::BODY)
                .color(p.text),
        );
        for group in &entry.groups {
            body = body.child(
                // WhatsNew.tsx draws each group through AgentText: its Markdown, never the raw `**`.
                rune_kit::Markdown::new(group.text.clone()).selectable(true).render(&markdown_style(p)).w_full(),
            );
        }
    }
    body.child(
        row()
            .gap(8.0)
            .child(button(changelog::SEE_EVERY, p).on_press(|s: &mut State| {
                s.home.splash_open = false;
                s.home.seen_version = Some(s.home.current_version.clone());
                crate::settings_panel::open(s);
                s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
                refresh_changelog(s);
            }))
            .child(primary(changelog::GOT_IT, p).on_press(|s: &mut State| {
                s.home.splash_open = false;
                s.home.seen_version = Some(s.home.current_version.clone());
            })),
    )
}

pub(super) fn home_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.home = Panel {
        home_open: true,
        current_version: "0.1.0".into(),
        seen_version: Some("0.0.1".into()),
        changelog: changelog::entries(changelog::NATIVE_CHANGELOG),
        changelog_shown: changelog::WHATS_NEW_PAGE,
        folder_path: None,
        ..Panel::default()
    };
    // Force splash for the scene from the embedded big mark.
    s.home.splash_entries = changelog::splash_entries(
        &s.home.changelog,
        s.home.seen_version.as_deref(),
        &s.home.current_version,
    )
    .into_iter()
    .cloned()
    .collect();
    s.home.splash_open = !s.home.splash_entries.is_empty();
}

pub(super) fn home_light_scene(s: &mut State) {
    home_scene(s);
    s.appearance = Some(false);
}

pub(super) fn board_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.home.board_open = true;
    s.home.home_open = false;
    // Demo board rows.
    s.recent = vec![
        ConversationRow {
            id: "m_live".into(),
            teammate: "Codec".into(),
            title: "Wire the board".into(),
            started_by: "the person".into(),
            state: "running".into(),
            updated: "2026-11-10T12:00:00.000Z".into(),
        },
        ConversationRow {
            id: "m_ask".into(),
            teammate: "Codec".into(),
            title: "Needs a say-so".into(),
            started_by: "the person".into(),
            state: "running".into(),
            updated: "2026-11-10T12:05:00.000Z".into(),
        },
        ConversationRow {
            id: "m_done".into(),
            teammate: "Codec".into(),
            title: "Finished earlier".into(),
            started_by: "the person".into(),
            state: "completed".into(),
            updated: "2026-11-10T11:00:00.000Z".into(),
        },
    ];
    s.pending_approval = Some(approval::Request {
        approval_id: "ask_demo".into(),
        mission_id: "m_ask".into(),
        runtime: "codex".into(),
        kind: approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "ls -la".into(),
        cwd: Some("/tmp/demo".into()),
        requested_at: "2026-11-10T12:05:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false, answer_in: None, claude_input: None,
        questions: Vec::new(),
    });
    s.home.to_look_at.insert("m_done".into());
    update_board_asks(
        s,
        &s.pending_approval.clone().into_iter().collect::<Vec<_>>(),
    );
}

pub(super) fn board_light_scene(s: &mut State) {
    board_scene(s);
    s.appearance = Some(false);
}

pub(super) fn boot_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    open_boot(s);
    // Deterministic demo rows for snaps (no live PATH dependence in the picture).
    s.home.boot = BootState {
        phase: boot_view::BootPhase::Probing,
        started_at: Some(0),
        context: Some(boot_view::BootContext {
            version: "0.1.0".into(),
            platform: "darwin".into(),
            workspace: "native".into(),
            branch: Some("exec/native".into()),
            clean: Some(true),
            ledger_path: "/Users/colin/Library/Application Support/Locust-native/mission-ledger"
                .into(),
            ledger_ok: true,
        }),
        probes: vec![
            boot_view::BootProbe {
                id: "codex".into(),
                bin: "codex".into(),
                product: "Codex".into(),
                at: 0,
                outcome: Some("ready"),
                version: Some("0.50.0".into()),
            },
            boot_view::BootProbe {
                id: "claude".into(),
                bin: "claude".into(),
                product: "Claude Code".into(),
                at: 0,
                outcome: Some("missing"),
                version: None,
            },
            boot_view::BootProbe {
                id: "opencode".into(),
                bin: "opencode".into(),
                product: "OpenCode".into(),
                at: 0,
                outcome: Some("ready"),
                version: None,
            },
        ],
        finished: Some((100, 2, 1)),
    };
    s.home.boot_view = Some(boot_view::boot_view(
        &s.home.boot,
        boot_view::BootPhase::Settled,
        800,
    ));
}

pub(super) fn boot_light_scene(s: &mut State) {
    boot_scene(s);
    s.appearance = Some(false);
}

pub(super) fn away_scene(s: &mut State) {
    home_scene(s);
    s.home.splash_open = false;
    s.home.away = Some(AwaySummary {
        since: "2026-11-10T01:00:00.000Z".into(),
        ran: vec![away::AwayItem {
            mission_id: "m1".into(),
            title: "Overnight polish".into(),
            at: "2026-11-10T06:12:00.000Z".into(),
            owner_id: None,
            routine_id: None,
        }],
        failed: vec![away::AwayItem {
            mission_id: "m2".into(),
            title: "Deploy check".into(),
            at: "2026-11-10T05:00:00.000Z".into(),
            owner_id: None,
            routine_id: None,
        }],
        missed: vec![],
        waiting: 1,
    });
    s.home.away_said = HashMap::from([
        ("m1".to_owned(), "Fixed 6 checks that had gone stale and found one real bug; the fixes wait in a copy for you to keep.".to_owned()),
        ("m2".to_owned(), "OpenCode's service refused the free model, so OpenCode stopped.".to_owned()),
    ]);
}

pub(super) fn away_light_scene(s: &mut State) {
    away_scene(s);
    s.appearance = Some(false);
}

pub(super) fn whatsnew_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    // Both ends of the splash window must exist. This older entry is demo data only.
    let notes = format!(
        "{}\n## 0.0.1 - 2026-11-09\n\nEarlier preview.\n",
        changelog::NATIVE_CHANGELOG
    );
    s.home.changelog = changelog::entries(&notes);
    s.home.changelog_shown = changelog::WHATS_NEW_PAGE;
    // Settings shell only: the scene is a synthetic value and needs no data reads.
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    s.home.seen_version = Some("0.0.1".into());
    s.home.current_version = "0.1.0".into();
    s.home.splash_entries = changelog::splash_entries(
        &s.home.changelog,
        s.home.seen_version.as_deref(),
        &s.home.current_version,
    )
    .into_iter()
    .cloned()
    .collect();
    s.home.splash_open = !s.home.splash_entries.is_empty();
}

pub(super) fn whatsnew_light_scene(s: &mut State) {
    whatsnew_scene(s);
    s.appearance = Some(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_open_expected_surfaces() {
        let mut s = State::default();
        home_scene(&mut s);
        assert!(s.home.home_open);
        board_scene(&mut s);
        assert!(s.home.board_open);
        assert!(s.home.asks.contains_key("m_ask"));
        boot_scene(&mut s);
        assert!(s.home.boot_open);
        assert!(s.home.boot_view.is_some());
        whatsnew_scene(&mut s);
        assert!(s.home.splash_open);
        assert_eq!(s.home.splash_entries.len(), 1);
        assert!(s.settings_panel.error.is_empty());
    }

    #[test]
    fn board_allow_once_words() {
        assert_eq!(ALLOW_ONCE, "Allow once");
        assert_eq!(DENY, "Deny");
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn board_answers_all_conversations_through_channel_once_and_never_automatically() {
        let root = { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let root = std::env::temp_dir().join(format!("home-board-channel-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))); let _ = std::fs::remove_dir_all(&root); root };
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let _hook = native::hooks::install(store, None);
        let channel = native::approvals_channel();
        let handles: Vec<_> = ["board_one", "board_two"]
            .into_iter()
            .map(|mission| {
                let channel = channel.clone();
                std::thread::spawn(move || {
                    channel.ask_codex(
                        "review",
                        mission,
                        "item/commandExecution/requestApproval",
                        &serde_json::json!({"command":"echo synthetic"}),
                        serde_json::json!(1),
                        &|| false,
                    )
                })
            })
            .collect();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let waiting = loop {
            let waiting: Vec<_> = channel
                .waiting()
                .into_iter()
                .filter(|r| r.mission_id.starts_with("board_"))
                .collect();
            if waiting.len() == 2 {
                break waiting;
            }
            if std::time::Instant::now() >= deadline {
                let all: Vec<_> = channel.waiting().into_iter().map(|r| r.mission_id).collect();
                let got: Vec<String> = handles.into_iter().filter(|h| h.is_finished()).map(|h| format!("{:?}", h.join())).collect();
                panic!("two cards were not raised; waiting={all:?} finished={got:?}");
            }
            std::thread::yield_now();
        };
        let mut s = State::default();
        assert!(update_board_asks(&mut s, &waiting));
        assert!(!update_board_asks(&mut s, &waiting));
        assert!(s.pending_approval.is_none());
        let id = waiting[0].approval_id.clone();
        answer_board(&mut s, &id, true);
        assert_eq!(
            channel
                .waiting()
                .iter()
                .filter(|r| r.mission_id.starts_with("board_"))
                .count(),
            2
        );
        s.home.asks.get_mut(&waiting[0].mission_id).unwrap().shown -=
            std::time::Duration::from_secs(1);
        answer_board(&mut s, &id, true);
        answer_board(&mut s, &id, false);
        answer_board(&mut s, &waiting[1].approval_id, false);
        let decisions: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap())
            .collect();
        assert!(decisions.iter().any(|v| v["decision"] == "accept"));
        assert!(decisions.iter().any(|v| v["decision"] == "reject"));
        for mission in ["board_one", "board_two"] {
            assert_eq!(channel.take_recorded_for(mission).len(), 1);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn away_counts_conversations_once_across_continued_turns() {
        let root = { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let root = std::env::temp_dir().join(format!("home-away-grouping-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))); let _ = std::fs::remove_dir_all(&root); root };
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        away::write_attention_mark(&store, "2026-01-01T00:00:00Z").unwrap();
        let mut s = State::default();
        board_scene(&mut s);
        s.data = Some(store);
        s.recent[0].state = "completed".into();
        s.recent[1].state = "completed".into();
        s.ledger_summaries.insert(
            "m_ask".into(),
            ledger::Summary {
                parent: Some("m_live".into()),
                words: String::new(),
                mission_id: "m_ask".into(),
                title: String::new(),
                runtime: "codex".into(),
                model: String::new(),
                created_at: "2026-11-10T12:05:00Z".into(),
                running: false,
                failed: false,
                updated: "2026-11-10T12:05:00Z".into(),
                ..Default::default()
            },
        );
        refresh_away(&mut s);
        let summary = s.home.away.as_ref().unwrap();
        assert_eq!(summary.ran.len(), 2);
        assert_eq!(summary.waiting, 1);
        s.sidebar_query.text = "no matches".into();
        assert_eq!(recent_as_missions(&s).len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod finish_review_tests {
    use super::*;
    #[test]
    fn finishes_while_elsewhere_wait_and_opening_clears_the_mark() {
        let mut s = State::default();
        board_scene(&mut s);
        s.home.to_look_at.clear();
        let mut next = s.recent.clone();
        next[0].state = "completed".into();
        next[1].state = "cancelled".into();
        record_finishes(&mut s, &next);
        assert_eq!(s.home.to_look_at, HashSet::from(["m_live".into()]));
        s.recent = next;
        opened(&mut s, "m_live");
        assert!(s.home.to_look_at.is_empty());
        assert!(!s.home.board_open);
    }
}

/// The system's own folder dialog; the folder chosen becomes the one Locust has open (Home's Choose folder, and the
/// message box's + menu).
pub(super) fn choose_folder(tx: &mut Tx) {
    let options = rune::FileDialogOptions { title: "Choose a project folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        let Some(store) = s.data.clone() else { return };
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        match folders::use_folder(&store, &path, &now).and_then(|entry| native::set_open_folder(&store, std::path::Path::new(&entry.path)).map(|_| entry)) {
            Ok(entry) => {
                s.home.folder_path = Some(entry.path);
                s.home.folder_invented = false;
            }
            Err(e) => {
                s.status = e;
                s.status_tone = StatusTone::Amber;
            }
        }
    });
}

#[cfg(test)]
mod renderer_runtime_tests {
    use super::*;
    #[test]
    fn is_judged_by_the_route_it_is_set_to_use() {
        assert_eq!(runtime_of_teammate(&State::default(), "tm_wisp", Some("claude")), Some("claude"));
    }
    fn with_missions(missions: &[(&str, &str)]) -> State {
        let mut s = State::default();
        for (i, (phase, runtime)) in missions.iter().enumerate() {
            let id = format!("mission_{i}");
            s.people_panel.team["missionOwners"][&id] = serde_json::json!("tm_wisp");
            s.ledger_summaries.insert(id.clone(), ledger::Summary { mission_id:id,
                runtime:(*runtime).into(), running:*phase == "running", updated:format!("2026-09-14T00:00:0{}Z", 9-i), ..Default::default() });
        }
        s
    }
    #[test]
    fn still_prefers_a_running_mission_then_a_past_one_over_the_route() {
        for (missions, expected) in [
            (vec![("running", "codex")], "codex"),
            (vec![("completed", "cursor")], "cursor"),
            (vec![("completed", "cursor"), ("running", "opencode")], "opencode"),
        ] {
            assert_eq!(runtime_of_teammate(&with_missions(&missions), "tm_wisp", Some("claude")), Some(expected));
        }
    }
    #[test]
    fn is_judged_by_nothing_when_it_has_neither_missions_nor_a_route() {
        assert_eq!(runtime_of_teammate(&State::default(), "tm_wisp", None), None);
    }
    #[test]
    fn home_and_team_judge_the_recorded_runtime_before_the_saved_route() {
        use locust_core::agents::{Found, Auth, Probe};
        for home in [true, false] {
            let mut s = with_missions(&[("completed", "codex")]);
            crate::team_scene(&mut s);
            s.teammates.truncate(1);
            s.teammates[0].id = "tm_wisp".into();
            s.teammates[0].name = "Wisp".into();
            s.teammates[0].runtime = Some("claude".into());
            s.teammates[0].busy = false;
            s.people_panel.team["teammates"] = serde_json::json!([{
                "teammateId":"tm_wisp", "name":"Wisp", "role":"Docs & QA", "route":{"runtime":"claude"}
            }]);
            s.settings_panel.agents = Some([("claude", false), ("codex", true)].into_iter().map(|(id, ready)| Found {
                id:id.into(), display_name:id.into(), installed:true, version:Some("1.0.0".into()),
                auth:if ready {Auth::Authenticated} else {Auth::Unauthenticated}, ready,
                status:if ready {Probe::Ready} else {Probe::AuthRequired}, unsupported_note:None,
                through_cli:false, checking:false,
            }).collect());
            let mut rt = rune::Runtime::new((s,home), |(s,home),_| if *home { home_view(s,&Palette::new(true)) }
                else { crate::people_panel::view(s,&Palette::new(true)) }, (1200.0,1800.0),rune::Env::new(true,1.0));
            rt.frame(0.0);
            let shown = rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n");
            assert!(shown.contains("Wisp"), "home={home}: {shown}");
            assert!(!shown.contains("Sign-in needed"), "home={home}: {shown}");
            assert!(!shown.contains("Needs you"), "home={home}: {shown}");
        }
    }
    #[test]
    fn runtime_ownership_uses_ids_even_when_another_teammate_has_the_same_name() {
        let s = with_missions(&[("running", "codex")]);
        assert_eq!(runtime_of_teammate(&s, "tm_other", Some("claude")), Some("claude"));
    }
    #[test]
    fn home_and_team_draw_the_signed_out_warning_and_presence_from_the_same_status() {
        use locust_core::agents::{Found, Auth, Probe};
        for home in [true, false] {
            let mut s = State::default();
            crate::team_scene(&mut s);
            s.teammates.truncate(1);
            let mate = &mut s.teammates[0];
            mate.id = "tm_wisp".into(); mate.name = "Wisp".into(); mate.role = "Docs & QA".into();
            mate.runtime = Some("claude".into()); mate.busy = false;
            s.people_panel.team = serde_json::json!({"teammates":[{"teammateId":"tm_wisp","name":"Wisp","role":"Docs & QA","route":{"runtime":"claude"}}]});
            s.settings_panel.agents = Some([("claude", false), ("codex", true)].into_iter().map(|(id, ready)| Found {
                id:id.into(), display_name:id.into(), installed:true, version:None,
                auth:if ready {Auth::Authenticated} else {Auth::Unauthenticated}, ready,
                status:if ready {Probe::Ready} else {Probe::AuthRequired}, unsupported_note:None, through_cli:false, checking:false,
            }).collect());
            let v = teammate_status(&s, "tm_wisp", false, Some("claude"), "Docs & QA");
            assert_eq!(v.status, "blocked");
            assert_eq!(teammate_status_color(&v, &Palette::new(true)), Palette::new(true).red);
            let mut rt = rune::Runtime::new((s,home), |(s,home),_| if *home { home_view(s,&Palette::new(true)) }
                else { crate::people_panel::view(s,&Palette::new(true)) }, (1200.0,1800.0),rune::Env::new(true,1.0));
            rt.frame(0.0);
            let elements = rt.elements();
            // Home's card says it beside the name, mid-line (HomeTeam.tsx, homeTeamState `lowerFirst`); Team says it as a line.
            let said = if home { "sign-in needed" } else { "Sign-in needed" };
            assert!(elements.iter().any(|e| e.text.as_deref() == Some(said)), "home={home}");
            assert!(elements.iter().any(|e| e.label.as_deref() == Some("blocked")), "home={home}");
            assert!(!elements.iter().any(|e| e.text.as_ref().is_some_and(|t| t.contains("Docs & QA · idle"))), "home={home}");
        }
    }

    #[test]
    fn approval_presence_uses_cached_owner_ids_and_stays_distinct_from_a_sign_in_wall() {
        let mut s = State::default();
        s.people_panel.team = serde_json::json!({"missionOwners":{"m1":"tm_wisp","m2":"tm_other"}});
        s.home.waiting_for.insert("m1".into(), "A decision".into());
        let v = teammate_status(&s, "tm_wisp", true, Some("codex"), "Builder");
        assert_eq!(v.status, "approval-needed");
        assert_eq!(v.label, "Builder · waiting on you");
        assert_eq!(v.presence, "approval");
        assert_eq!(teammate_status_color(&v, &Palette::new(true)), Palette::new(true).amber);
        assert_eq!(teammate_section(&s, "tm_wisp", true, Some("codex")), crate::team_board::Section::NeedsYou);
        let other = teammate_status(&s, "tm_other", false, Some("codex"), "Docs & QA");
        assert_eq!(other.status, "idle");
        assert_eq!(other.presence, "none");
        assert_eq!(teammate_section(&s, "tm_other", false, Some("codex")), crate::team_board::Section::Ready);
    }

}

#[cfg(test)]
#[path = "whats_new_tests.rs"]
mod whats_new_tests;

#[cfg(test)]
#[path = "home_install_first_tests.rs"]
mod home_install_first_tests;

#[cfg(test)]
#[path = "home_cover_status_tests.rs"]
mod home_cover_status_tests;

#[cfg(test)]
#[path = "home_team_cards_tests.rs"]
mod home_team_cards_tests;

#[cfg(test)]
#[path = "cover_ink_tests.rs"]
mod cover_ink_tests;
#[cfg(test)]
#[path = "home_cover_tests.rs"]
mod home_cover_tests;

#[cfg(test)]
#[path = "home_agents_fold_tests.rs"]
mod home_agents_fold_tests;
