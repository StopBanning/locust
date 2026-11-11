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
    pub boot_open: bool,
    pub splash_open: bool,
    pub folder_path: Option<String>,
    pub folder_invented: bool,
    pub boot: BootState,
    pub boot_view: Option<BootView>,
    pub away: Option<AwaySummary>,
    pub to_look_at: HashSet<String>,
    pub asks: HashMap<String, BoardAsk>,
    pub waiting_for: HashMap<String, String>,
    pub changelog: Vec<ChangelogEntry>,
    pub changelog_shown: usize,
    pub splash_entries: Vec<ChangelogEntry>,
    pub seen_version: Option<String>,
    pub current_version: String,
    pub first_launch_done: bool,
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
    s.p12.home_open = true;
    s.p12.board_open = false;
    s.p12.boot_open = false;
    sync_board_asks(s);
    refresh_away(s);
    refresh_changelog(s);
}

pub(super) fn open_board(s: &mut State) {
    s.p12.board_open = true;
    s.p12.home_open = false;
    s.p12.boot_open = false;
    sync_board_asks(s);
}

pub(super) fn open_boot(s: &mut State) {
    s.p12.boot_open = true;
    s.p12.home_open = false;
    s.p12.board_open = false;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    s.p12.boot = boot_view::path_discovery_sweep(now);
    s.p12.boot_view = Some(boot_view::boot_view(
        &s.p12.boot,
        boot_view::BootPhase::Probing,
        now,
    ));
    s.nothing_installed = s
        .p12
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
    s.p12.changelog = changelog::entries(&text);
    if s.p12.changelog_shown == 0 {
        s.p12.changelog_shown = changelog::WHATS_NEW_PAGE;
    }
    if s.p12.current_version.is_empty() {
        s.p12.current_version = s
            .p12
            .changelog
            .first()
            .map(|e| e.version.clone())
            .unwrap_or_else(|| "0.1.0".into());
    }
    let _ = changelog::entry_for(&text, &s.p12.current_version);
    let splash = changelog::splash_entries(
        &s.p12.changelog,
        s.p12.seen_version.as_deref(),
        &s.p12.current_version,
    );
    s.p12.splash_entries = splash.into_iter().cloned().collect();
    s.p12.splash_open = !s.p12.splash_entries.is_empty();
}

fn refresh_away(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.p12.away = None;
        return;
    };
    let Some(mark) = away::read_attention_mark(&store) else {
        s.p12.away = None;
        return;
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    if away::was_away(Some(&mark), now_ms, AWAY_MINIMUM_MS).is_none() {
        s.p12.away = None;
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
    let needs = s.p12.asks.keys().cloned().collect();
    s.p12.away = away::since_you_were_away(&mark, &rows, &[], &needs);
}

pub(super) fn sync_board_asks(s: &mut State) -> bool {
    let waiting = native::waiting_approvals();
    update_board_asks(s, &waiting)
}

fn update_board_asks(s: &mut State, waiting: &[approval::Request]) -> bool {
    let old = std::mem::take(&mut s.p12.asks);
    s.p12.waiting_for.clear();
    for req in waiting {
        s.p12.waiting_for.insert(
            req.mission_id.clone(),
            if req.summary.is_empty() {
                WAITING_ON_YOU.into()
            } else {
                req.summary.clone()
            },
        );
        // The oldest card for each conversation is answered first, as in Electron.
        s.p12
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
    old != s.p12.asks
}

fn answer_board(s: &mut State, approval_id: &str, approve: bool) {
    let Some(ask) = s
        .p12
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
    match native::decide_approval(approval_id, decision, None) {
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

pub(super) fn record_finishes(s: &mut State, next: &[ConversationRow]) {
    let looking = if s.p12.home_open || s.p12.board_open || s.p12.boot_open {
        None
    } else {
        selected_conversation(s).and_then(|(_, c)| c.conversation_id.clone())
    };
    for row in next {
        if matches!(row.state.as_str(), "completed" | "failed")
            && s.recent
                .iter()
                .any(|old| old.id == row.id && old.state == "running")
            && looking.as_deref() != Some(row.id.as_str())
        {
            s.p12.to_look_at.insert(row.id.clone());
        }
    }
}

pub(super) fn opened(s: &mut State, id: &str) {
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
        s.p12.to_look_at.remove(&key);
    }
    s.p12.home_open = false;
    s.p12.board_open = false;
    s.p12.boot_open = false;
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
    s.p12.away = None;
}

fn apply_starter(s: &mut State, starter_id: &str, prompt: &str, mode: Option<&str>) {
    if let Some(mode) = mode {
        s.mode = mode.into();
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
    let prompt = if s.p12.folder_invented {
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
    s.p12.home_open = false;
}

/// When the composer sends from Home, switch to a project the message names.
pub(super) fn maybe_switch_named_folder(s: &mut State, text: &str) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let current = s
        .p12
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
            s.p12.folder_path = Some(entry.path);
            s.p12.folder_invented = false;
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
            "route": { "runtime": "codex", "model": "", "mode": "ask" },
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
    s.p12.first_launch_done = true;
    s.status = format!("Added the {} team.", template.title);
    s.status_tone = StatusTone::Green;
    s.team = Latest::default();
}

pub(super) fn home_view(s: &State, p: &Palette) -> El {
    let mut body = col().w_full().gap(12.0).padding(16.0);
    body = body
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text("Home".to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| s.p12.home_open = false)),
        )
        .child(
            text(COVER_CLAIM.to_owned())
                .font(type_::BODY)
                .color(p.secondary)
                .wrap(),
        );

    // Folder strip
    let folder_line = s
        .p12
        .folder_path
        .clone()
        .unwrap_or_else(|| NO_FOLDER_CHOSEN.into());
    body = body.child(
        col()
            .w_full()
            .gap(6.0)
            .padding(10.0)
            .fill(p.raised)
            .radius(8.0)
            .child(text(folder_line).font(type_::LABEL).color(p.text).wrap())
            .child(
                text(FOLDER_LEDE.to_owned())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap(),
            )
            .child(button(CHOOSE_FOLDER, p).on_press(|s: &mut State| {
                // Native has no OS folder picker in this round; invent a working folder once.
                if let Some(store) = s.data.clone() {
                    let path = store.root().join("Projects").join("Locust");
                    let _ = std::fs::create_dir_all(&path);
                    let now =
                        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                    match folders::use_folder(&store, &path, &now) {
                        Ok(entry) => {
                            s.p12.folder_path = Some(entry.path);
                            s.p12.folder_invented = true;
                        }
                        Err(e) => {
                            s.status = e;
                            s.status_tone = StatusTone::Amber;
                        }
                    }
                }
            })),
    );
    if s.p12.folder_invented {
        body = body.child(
            text(MADE_FOLDER.to_owned())
                .font(type_::LABEL)
                .color(p.amber),
        );
        if let Some(path) = &s.p12.folder_path {
            body = body.child(
                text(folders::made_folder_note(path))
                    .font(type_::LABEL)
                    .color(p.amber)
                    .wrap(),
            );
        }
    }

    // Away
    if let Some(summary) = &s.p12.away {
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
                        list = list.child(
                            text(format!("{title} · {kind} {when}"))
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .wrap()
                                .on_press(move |s: &mut State| {
                                    s.opening_conversation = Some(mission_id.clone());
                                    s.p12.home_open = false;
                                }),
                        );
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
                    .child(button(TEAM_FROM_IMAGE, p).on_press(|s: &mut State| {
                        s.status = format!("{} — runs in Locust for now", TEAM_FROM_IMAGE);
                        s.status_tone = StatusTone::Muted;
                    }))
                    .child(button(NEW_TEAMMATE, p).on_press(|s: &mut State| {
                        crate::people_panel::open(s);
                        s.p12.home_open = false;
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
                            .on_press(move |s: &mut State| apply_template(s, id)),
                    ),
            );
        }
    } else {
        body = body.child(text(YOUR_TEAM.to_owned()).font(type_::TITLE).color(p.text));
        for mate in &s.teammates {
            body = body.child(
                text(format!("{} · {}", mate.name, mate.role))
                    .font(type_::LABEL)
                    .color(p.secondary),
            );
        }
        body = body.child(button(NEW_TEAMMATE, p).on_press(|s: &mut State| {
            crate::people_panel::open(s);
            s.p12.home_open = false;
        }));
    }

    // Starters
    let roles: Vec<&str> = s.teammates.iter().map(|t| t.role.as_str()).collect();
    let starters = starters::starters_for_roles(&roles);
    body = body
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text(START_A_CONVERSATION.to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button(COMPARE_MODELS, p).on_press(|s: &mut State| {
                    s.status = format!("{COMPARE_MODELS} — runs in Locust for now");
                    s.status_tone = StatusTone::Muted;
                }))
                .child(button(START_FROM_ISSUE, p).on_press(|s: &mut State| {
                    s.status = format!("{START_FROM_ISSUE} — runs in Locust for now");
                    s.status_tone = StatusTone::Muted;
                })),
        )
        .child(text(PICK_ONE.to_owned()).font(type_::LABEL).color(p.muted))
        .child(
            text(CONNECT_RUNTIME.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    for starter in starters {
        let starter_id = starter.id.clone();
        let prompt = starter.prompt.to_owned();
        let mode = starter.mode.map(str::to_owned);
        let compare_label = if starter.compare_changes { COMPARE } else { "" };
        body = body.child(
            col()
                .w_full()
                .gap(2.0)
                .padding(8.0)
                .fill(p.raised)
                .radius(6.0)
                .child(
                    text(starter.label.to_owned())
                        .font(type_::BODY)
                        .color(p.text),
                )
                .child(
                    text(starter.line.to_owned())
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap(),
                )
                .child(
                    text(compare_label.to_owned())
                        .font(type_::LABEL)
                        .color(p.secondary),
                )
                .on_press(move |s: &mut State| {
                    apply_starter(s, &starter_id, &prompt, mode.as_deref());
                }),
        );
    }

    body.scroll(ScrollSpec {
        stick_bottom: false,
    })
}

pub(super) fn board_view(s: &State, p: &Palette) -> El {
    let missions = recent_as_missions(s);
    let facts = ConversationBoardFacts {
        needs_you: s.p12.asks.keys().cloned().collect(),
        to_look_at: s.p12.to_look_at.clone(),
    };
    let board = conversation_board::conversation_board(&missions, &[], &HashSet::new(), &facts);
    let meta = conversation_board::board_meta(board.as_deref());
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
            .child(button("Close", p).on_press(|s: &mut State| s.p12.board_open = false)),
    );

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
            for section in sections {
                body = body.child(
                    text(section.title.to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .padding(4.0),
                );
                for card in section.members {
                    let mission_id = card.row.mission_id.clone();
                    let title = card.row.title.clone();
                    let under = card.parent_id.as_ref().and_then(|parent| {
                        missions
                            .iter()
                            .find(|m| m.mission_id == *parent)
                            .map(|m| conversation_board::under_title(&m.title))
                    });
                    let column_key = section.key.as_str();
                    let ask = s.p12.asks.get(&mission_id).cloned().or_else(|| {
                        card.row
                            .member_ids
                            .iter()
                            .find_map(|id| s.p12.asks.get(id).cloned())
                    });
                    let state_line = match section.key {
                        ConversationBoardKey::NeedsYou => s
                            .p12
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
                    let mut card_el = col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .fill(p.raised)
                        .radius(8.0)
                        .child(
                            text(format!("{title} · {column_key}"))
                                .font(type_::BODY)
                                .color(p.text),
                        );
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
                                    .child(primary(ALLOW_ONCE, p).on_press(move |s: &mut State| {
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
                                    s.p12.board_open = false;
                                }));
                        }
                    } else {
                        card_el = card_el.child(text(state_line).font(type_::LABEL).color(p.muted));
                    }
                    let open_id = mission_id.clone();
                    body = body.child(card_el.on_press(move |s: &mut State| {
                        s.opening_conversation = Some(open_id.clone());
                        s.p12.to_look_at.remove(&open_id);
                        s.p12.board_open = false;
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
    let now = s.p12.boot.finished.map(|(at, _, _)| at + 800).unwrap_or(0);
    let phase = s
        .p12
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
        .p12
        .boot_view
        .clone()
        .unwrap_or_else(|| boot_view::boot_view(&s.p12.boot, phase, now));
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
            text(boot_view::deferred_others_sentence(2, true, true))
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
                s.p12.boot_open = false;
                s.p12.first_launch_done = true;
                open_home(s);
            }))
            .child(button("Skip", p).on_press(|s: &mut State| {
                s.p12.boot_open = false;
                open_home(s);
            })),
    );
    body
}

pub(super) fn whats_new_settings(s: &State, p: &Palette) -> El {
    let mut body = col().w_full().gap(8.0).padding(12.0);
    if s.p12.changelog.is_empty() {
        return body.child(
            text(changelog::EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    }
    for entry in s.p12.changelog.iter().take(s.p12.changelog_shown) {
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
                text(group.text.clone())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap()
                    .padding(2.0),
            );
        }
    }
    if s.p12.changelog_shown < s.p12.changelog.len() {
        body = body.child(button(changelog::SHOW_OLDER, p).on_press(|s: &mut State| {
            s.p12.changelog_shown =
                (s.p12.changelog_shown + changelog::WHATS_NEW_MORE).min(s.p12.changelog.len());
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
                    s.p12.splash_open = false;
                    s.p12.seen_version = Some(s.p12.current_version.clone());
                })),
        );
    for entry in &s.p12.splash_entries {
        body = body.child(
            text(changelog::version_heading(entry))
                .font(type_::BODY)
                .color(p.text),
        );
        for group in &entry.groups {
            body = body.child(
                text(group.text.clone())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap(),
            );
        }
    }
    body.child(
        row()
            .gap(8.0)
            .child(button(changelog::SEE_EVERY, p).on_press(|s: &mut State| {
                s.p12.splash_open = false;
                s.p12.seen_version = Some(s.p12.current_version.clone());
                crate::settings_panel::open(s);
                s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
                refresh_changelog(s);
            }))
            .child(primary(changelog::GOT_IT, p).on_press(|s: &mut State| {
                s.p12.splash_open = false;
                s.p12.seen_version = Some(s.p12.current_version.clone());
            })),
    )
}

pub(super) fn home_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.p12 = Panel {
        home_open: true,
        current_version: "0.1.0".into(),
        seen_version: Some("0.0.1".into()),
        changelog: changelog::entries(changelog::NATIVE_CHANGELOG),
        changelog_shown: changelog::WHATS_NEW_PAGE,
        folder_path: None,
        ..Panel::default()
    };
    // Force splash for the scene from the embedded big mark.
    s.p12.splash_entries = changelog::splash_entries(
        &s.p12.changelog,
        s.p12.seen_version.as_deref(),
        &s.p12.current_version,
    )
    .into_iter()
    .cloned()
    .collect();
    s.p12.splash_open = !s.p12.splash_entries.is_empty();
}

pub(super) fn home_light_scene(s: &mut State) {
    home_scene(s);
    s.appearance = Some(false);
}

pub(super) fn board_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.p12.board_open = true;
    s.p12.home_open = false;
    // Demo board rows.
    s.recent = vec![
        ConversationRow {
            id: "m_live".into(),
            teammate: "tm_wren".into(),
            title: "Wire the board".into(),
            started_by: "the person".into(),
            state: "running".into(),
            updated: "2026-11-10T12:00:00.000Z".into(),
        },
        ConversationRow {
            id: "m_ask".into(),
            teammate: "tm_wren".into(),
            title: "Needs a say-so".into(),
            started_by: "the person".into(),
            state: "running".into(),
            updated: "2026-11-10T12:05:00.000Z".into(),
        },
        ConversationRow {
            id: "m_done".into(),
            teammate: "tm_wren".into(),
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
        claude_input: None,
    });
    s.p12.to_look_at.insert("m_done".into());
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
    s.p12.boot = BootState {
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
    s.p12.boot_view = Some(boot_view::boot_view(
        &s.p12.boot,
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
    s.p12.splash_open = false;
    s.p12.away = Some(AwaySummary {
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
    s.p12.changelog = changelog::entries(&notes);
    s.p12.changelog_shown = changelog::WHATS_NEW_PAGE;
    // Settings shell only: the scene is a synthetic value and needs no data reads.
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    s.p12.seen_version = Some("0.0.1".into());
    s.p12.current_version = "0.1.0".into();
    s.p12.splash_entries = changelog::splash_entries(
        &s.p12.changelog,
        s.p12.seen_version.as_deref(),
        &s.p12.current_version,
    )
    .into_iter()
    .cloned()
    .collect();
    s.p12.splash_open = !s.p12.splash_entries.is_empty();
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
        assert!(s.p12.home_open);
        board_scene(&mut s);
        assert!(s.p12.board_open);
        assert!(s.p12.asks.contains_key("m_ask"));
        boot_scene(&mut s);
        assert!(s.p12.boot_open);
        assert!(s.p12.boot_view.is_some());
        whatsnew_scene(&mut s);
        assert!(s.p12.splash_open);
        assert_eq!(s.p12.splash_entries.len(), 1);
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
        let root = std::env::temp_dir().join("p12-board-channel");
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let _hook = native::hooks::install(store, None);
        let channel = native::approvals_channel();
        let handles: Vec<_> = ["p12_board_one", "p12_board_two"]
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
                .filter(|r| r.mission_id.starts_with("p12_board_"))
                .collect();
            if waiting.len() == 2 {
                break waiting;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "two cards were not raised"
            );
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
                .filter(|r| r.mission_id.starts_with("p12_board_"))
                .count(),
            2
        );
        s.p12.asks.get_mut(&waiting[0].mission_id).unwrap().shown -=
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
        for mission in ["p12_board_one", "p12_board_two"] {
            assert_eq!(channel.take_recorded_for(mission).len(), 1);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn away_counts_conversations_once_across_continued_turns() {
        let root = std::env::temp_dir().join("p12-away-grouping");
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
            },
        );
        refresh_away(&mut s);
        let summary = s.p12.away.as_ref().unwrap();
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
        s.p12.to_look_at.clear();
        let mut next = s.recent.clone();
        next[0].state = "completed".into();
        next[1].state = "cancelled".into();
        record_finishes(&mut s, &next);
        assert_eq!(s.p12.to_look_at, HashSet::from(["m_live".into()]));
        s.recent = next;
        opened(&mut s, "m_live");
        assert!(s.p12.to_look_at.is_empty());
        assert!(!s.p12.board_open);
    }
}
