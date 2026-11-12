//! THE NARROW WINDOW'S RAIL AND THE TITLE BAR (renderer/layout.ts, Sidebar.tsx `compact`, RailFlyout.tsx,
//! railFlyout.ts, TitleBar.tsx; shell.css `.lc-shell.is-compact`, `.lc-railflyout`, `.lc-titlebar`).
//!
//! At 1199 points wide or less, unless the person chose otherwise in Settings › Appearance › Sidebar, the sidebar folds
//! to a 64-point rail: the mark (the way home), the `+`, the places as icons, a face for each teammate with how many
//! conversations sit behind it, and Settings at the foot. The rail draws no conversation rows, so hovering a face opens
//! that teammate's conversations in a card beside it, and pressing the face pins the card.
//!
//! Above everything, in both layouts, the 38-point title bar: the folder's name centred, as a Mac titles a window, and
//! at the right the browser and terminal toggles while no conversation is open, and how many turns are running.

use super::*;
use crate::conversation_list::{SidebarMission, by_live_then_recent};

/// COMPACT_MAX_WIDTH: at or below this width `auto` chooses the rail.
pub(super) const COMPACT_MAX_WIDTH: f32 = 1199.0;
/// RAIL_ROWS_SHOWN: the card lists this many conversations.
pub(super) const RAIL_ROWS_SHOWN: usize = 6;
/// `--lc-sidebar-rail-width`.
pub(super) const RAIL_WIDTH: f32 = 64.0;
/// `--lc-titlebar-height`.
pub(super) const TITLEBAR_HEIGHT: f32 = 38.0;
/// How long the card waits after the pointer leaves before it closes (Sidebar.tsx railLeave).
const CLOSE_AFTER_MS: u128 = 160;

/// resolveLayout: a chosen preference wins at every width; `auto` asks the width.
pub(super) fn compact_for(preference: &str, width: f32) -> bool {
    match preference {
        "compact" => true,
        "wide" => false,
        _ => width <= COMPACT_MAX_WIDTH,
    }
}

/// railCountBadge: drawn only past one.
pub(super) fn count_badge(count: usize) -> Option<String> {
    (count > 1).then(|| count.to_string())
}

/// shortAgo: `now`, `5m`, `2h`, `3d`, `2w`.
pub(super) fn short_ago(iso: Option<&str>, now_ms: i64) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(iso?).ok()?.timestamp_millis();
    let seconds = ((now_ms - at) / 1000).max(0);
    Some(match seconds {
        s if s < 60 => "now".into(),
        s if s < 3_600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3_600),
        s if s < 604_800 => format!("{}d", s / 86_400),
        s => format!("{}w", s / 604_800),
    })
}

/// railRows: the live conversation first, then newest first; `6 of 9` when more than fits.
pub(super) fn rail_rows(missions: &[SidebarMission]) -> (Vec<SidebarMission>, String) {
    let mut ordered = missions.to_vec();
    ordered.sort_by(by_live_then_recent);
    let label = if ordered.len() > RAIL_ROWS_SHOWN {
        format!("{RAIL_ROWS_SHOWN} of {}", ordered.len())
    } else {
        ordered.len().to_string()
    };
    ordered.truncate(RAIL_ROWS_SHOWN);
    (ordered, label)
}

/// railEmptyLine.
pub(super) fn empty_line(name: &str) -> String {
    format!("Nothing yet — pick {name} and write below.")
}

/// The conversations behind one teammate's face: the rows whose turns that teammate took.
fn owned_by(s: &State, teammate: &Teammate) -> Vec<SidebarMission> {
    sidebar::recent_as_missions(s)
        .into_iter()
        .filter(|m| {
            std::iter::once(&m.mission_id)
                .chain(m.member_ids.iter())
                .any(|id| s.recent.iter().any(|r| &r.id == id && r.teammate == teammate.name))
        })
        .collect()
}

/// Settings › Appearance › Sidebar: kept in the workspace settings Electron reads (`layout`).
pub(super) fn choose_layout(s: &mut State, preference: &str) {
    s.layout_pref = preference.to_owned();
    s.settings_panel.settings.layout = preference.to_owned();
    if let Some(store) = s.data.clone() {
        match workspace_settings::load(&store) {
            Ok(mut settings) => {
                settings.layout = preference.to_owned();
                if let Err(e) = workspace_settings::save(&store, &settings) {
                    s.settings_panel.error = e;
                }
            }
            Err(e) => s.settings_panel.error = e,
        }
    }
}

/// The Sidebar line's words (Screens.tsx), saying which layout `auto` picked for this window.
pub(super) fn layout_lede(preference: &str, compact_now: bool) -> String {
    match preference {
        "wide" => "Full. The names beside the faces, whatever the window.".into(),
        "compact" => "Rail. Faces only, whatever the window.".into(),
        _ => format!("Auto. Following this window, currently {}.", if compact_now { "the rail" } else { "full" }),
    }
}

fn close(s: &mut State) {
    s.rail_open = None;
    s.rail_pinned = false;
    s.rail_left_at = None;
}

/// Closes a card the pointer left, once it has been gone long enough to not be on its way into the card.
pub(super) fn poll(s: &mut State) -> bool {
    if s.rail_pinned || s.rail_open.is_none() || s.rail_over_face || s.rail_over_card {
        return false;
    }
    if s.rail_left_at.is_some_and(|t| t.elapsed().as_millis() >= CLOSE_AFTER_MS) {
        close(s);
        return true;
    }
    false
}

fn place(l: &crate::look::controls::Look, icon: &'static rune::Icon, words: &str, press: fn(&mut State)) -> El {
    row()
        .w_full()
        .py(8.0)
        .center()
        .radius(7.0)
        .hover(l.c("hover-surface"))
        .child(crate::look::controls::glyph(l, icon, 14.0, Some(l.c("text-secondary"))))
        .role("button")
        .label(words.to_owned())
        .help(words.to_owned())
        .on_press(move |s: &mut State| {
            close(s);
            press(s);
        })
}

fn new_conversation(s: &mut State) {
    s.home.home_open = false;
    s.home.board_open = false;
    s.settings_panel.open = false;
    s.people_panel.open = false;
    s.routines.open = false;
    fresh_conversation(s);
}

fn go_home(s: &mut State) {
    s.settings_panel.open = false;
    s.people_panel.open = false;
    s.routines.open = false;
    home_panel::open_home(s);
}

/// `.lc-shell.is-compact .lc-sidebar`: the 64-point rail.
pub(super) fn rail(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    // The mark, the way home, and the `+` under it (`.lc-sidebar__brand`, stacked).
    let mark_w = 26.0_f32;
    let brand = col()
        .w_full()
        .items_center()
        .gap(8.0)
        .pt(12.0)
        .pb(10.0)
        .child(
            row()
                .w(40.0)
                .h(24.0)
                .center()
                .radius(7.0)
                .hover(l.c("hover-surface"))
                .child(
                    rune::icon(&crate::look::icons::LOCUST_MARK, mark_w)
                        .color(l.c("text-primary").alpha(0.95))
                        .a11y_hidden(),
                )
                .role("button")
                .label("Home")
                .help("Home")
                .on_press(|s: &mut State| {
                    close(s);
                    go_home(s);
                }),
        )
        .child(
            crate::look::controls::head_icon(&crate::look::icons::PLUS, "New conversation", "New conversation (⌘ N)")
                .on_press(|s: &mut State| {
                    close(s);
                    new_conversation(s);
                })
                .build(l.k),
        );
    // The places, one to a row (`.lc-shell.is-compact .lc-sidebar__places`). The wide sidebar's list of conversations
    // is the Board here: the rail has no list of its own.
    let places = col()
        .w_full()
        .px(8.0)
        .py(4.0)
        .child(place(&l, &crate::look::icons::INBOX, "All conversations", home_panel::open_board))
        .child(place(&l, &crate::look::icons::USERS, "Teammates & Memory", crate::people_panel::open))
        .child(place(&l, &crate::look::icons::CLOCK, "Automations", crate::routines_panel::open));

    let selected_id = s
        .ledger
        .as_ref()
        .map(|pane| pane.header.id.clone())
        .or_else(|| s.teammates.get(s.selected).map(|t| t.id.clone()));
    let mut faces = col().w_full().pt(6.0);
    for teammate in &s.teammates {
        let owned = owned_by(s, teammate).len();
        let selected = selected_id.as_deref() == Some(teammate.id.as_str());
        let open = s.rail_open.as_deref() == Some(teammate.id.as_str());
        let id = teammate.id.clone();
        let hover_id = teammate.id.clone();
        let mut slot = el()
            .key(format!("rail-{}", teammate.id))
            .w_full()
            .h(60.0)
            .child(
                row()
                    .absolute(8.0, 6.0)
                    .w(RAIL_WIDTH - 16.0)
                    .h(48.0)
                    .center()
                    .radius(9.0)
                    .fill(if open { l.c("bg-selected") } else { p.sidebar })
                    .hover(l.c("hover-surface"))
                    .child(crate::look::faces::face(s.image_cache.as_deref(), &teammate.id, &teammate.name, s.looks.get(&teammate.id), 32.0, &l)),
            );
        // The selected teammate: a 2-point lime bar on the rail's left edge.
        if selected {
            slot = slot.child(el().absolute(0.0, 12.0).size(2.0, 36.0).radius(1.0).fill(l.c("lime")));
        }
        if let Some(badge) = count_badge(owned) {
            slot = slot.child(
                row()
                    .absolute(10.0, 8.0)
                    .min_w(14.0)
                    .h(14.0)
                    .px(3.0)
                    .center()
                    .radius(7.0)
                    .fill(l.c("bg-raised-2"))
                    .border(1.0, l.c("border-strong"))
                    .child(text(badge).font(rune::TextStyle::new(9.5, 400.0).mono().lh(12.0)).color(l.c("text-muted")))
                    .a11y_hidden(),
            );
        }
        let state_words = if teammate.busy { "working" } else { "idle" };
        faces = faces.child(
            slot.role("button")
                .label(format!("{}, {} · {}", teammate.name, teammate.role, state_words))
                .on_hover_change(move |s: &mut State, over, _| {
                    s.rail_over_face = over;
                    if over {
                        if !s.rail_pinned {
                            s.rail_open = Some(hover_id.clone());
                        }
                    } else {
                        s.rail_left_at = Some(std::time::Instant::now());
                    }
                })
                .on_press_tx(move |s: &mut State, tx| {
                    let pinned_here = s.rail_pinned && s.rail_open.as_deref() == Some(id.as_str());
                    select_teammate(s, &id, tx);
                    if pinned_here {
                        close(s);
                    } else {
                        s.rail_open = Some(id.clone());
                        s.rail_pinned = true;
                    }
                }),
        );
    }

    let foot = col()
        .w_full()
        .px(8.0)
        .pt(8.0)
        .pb(12.0)
        .child(place(&l, &crate::look::icons::SETTINGS, "Settings", settings_panel::open))
        .child(place(&l, &crate::look::icons::COLUMNS, "Board", home_panel::open_board));

    col()
        .key("rail")
        .w(RAIL_WIDTH)
        .h_full()
        .fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.hairline)
        .child(brand)
        .child(places)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(faces),
        )
        .child(foot)
}

/// RailFlyout.tsx: the open face's conversations, beside the rail at the face's height.
pub(super) fn flyout(s: &State, p: &Palette, top: f32) -> Option<El> {
    let id = s.rail_open.as_deref()?;
    let teammate = s.teammates.iter().find(|t| t.id == id)?;
    let l = crate::look::controls::Look::new(p.dark);
    let busy = teammate.busy || s.flight.as_ref().is_some_and(|f| f.teammate_id == teammate.id);
    let status = if busy { format!("{} · working", teammate.role) } else { teammate.role.clone() };
    let route = agent_line(s, teammate);
    let mut route_row = row().w_full().gap(4.0).center_y();
    if let Some(mark) = crate::look::controls::agent_mark(&l, teammate.runtime.as_deref().unwrap_or("codex"), 11.0, l.c("text-muted")) {
        route_row = route_row.child(mark);
    }
    route_row = route_row.child(text(route).font(rune::TextStyle::new(10.5, 400.0).mono()).color(l.c("text-muted")).shrink().truncate());
    let head = row()
        .w_full()
        .gap(10.0)
        .pt(12.0)
        .px(12.0)
        .pb(10.0)
        .child(crate::look::faces::face(s.image_cache.as_deref(), &teammate.id, &teammate.name, s.looks.get(&teammate.id), 30.0, &l))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(teammate.name.clone()).font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name")))
                .child(text(status).font(rune::TextStyle::new(13.0, 400.0)).color(if busy { p.green } else { l.c("text-secondary") }).w_full().truncate())
                .child(route_row),
        );
    let rule = || el().w_full().h(1.0).fill(l.c("border-hairline"));
    let owned = owned_by(s, teammate);
    let mut card = col()
        .key("rail-flyout")
        .w(268.0)
        .fill(l.c("bg-raised-2"))
        .border(1.0, l.c("border-strong"))
        .radius(12.0)
        .shadow(if p.dark { 24.0 } else { 18.0 }, if p.dark { 60.0 } else { 48.0 }, Color::BLACK.alpha(if p.dark { 0.6 } else { 0.16 }))
        .child(head)
        .child(rule());
    if owned.is_empty() {
        card = card.child(text(empty_line(&teammate.name)).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-muted")).w_full().px(12.0).py(10.0).wrap());
    } else {
        let (shown, count) = rail_rows(&owned);
        card = card.child(
            row()
                .w_full()
                .gap(8.0)
                .pt(9.0)
                .px(12.0)
                .pb(5.0)
                .center_y()
                .child(crate::look::controls::label(&l, "Conversations").grow(1.0))
                .child(text(count).font(rune::TextStyle::new(10.5, 400.0).mono()).color(l.c("text-muted"))),
        );
        let now = chrono::Utc::now().timestamp_millis();
        let open_id = s.ledger.as_ref().and_then(|pane| pane.conversation.conversation_id.clone());
        let mut list = col().w_full().px(6.0).pb(6.0).gap(1.0);
        for mission in shown {
            let live = mission.phase == "running";
            let active = open_id.as_ref().is_some_and(|o| &mission.mission_id == o || mission.member_ids.contains(o));
            let dot = match mission.phase.as_str() {
                "running" => p.green,
                "failed" => p.red,
                "waiting" | "blocked" => p.amber,
                _ => l.c("text-faint"),
            };
            let mut line = row()
                .key(format!("rail-row-{}", mission.mission_id))
                .w_full()
                .h(30.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .radius(7.0)
                .fill(if live || active { l.c("bg-selected") } else { l.c("bg-raised-2") })
                .hover(l.c("hover-surface"))
                .child(el().size(6.0, 6.0).radius(3.0).fill(dot))
                .child(
                    text(mission.title.clone())
                        .font(rune::TextStyle::new(13.0, 400.0))
                        .color(if live || active { l.c("text-name") } else { l.c("text-secondary") })
                        .grow(1.0)
                        .shrink()
                        .truncate(),
                );
            if mission.member_ids.len() > 1 {
                line = line.child(
                    text(mission.member_ids.len().to_string())
                        .font(rune::TextStyle::new(10.5, 400.0).mono())
                        .color(l.c("text-muted"))
                        .px(4.0)
                        .radius(4.0)
                        .fill(l.c("hover-surface")),
                );
            }
            if let Some(age) = short_ago(mission.last_at.as_deref(), now) {
                line = line.child(text(age).font(rune::TextStyle::new(10.5, 400.0).mono()).color(l.c("text-muted")));
            }
            let row_id = mission.mission_id.clone();
            list = list.child(line.role("button").label(mission.title.clone()).on_press_tx(move |s: &mut State, tx| {
                close(s);
                if let Some(item) = s.recent.iter().find(|r| r.id == row_id).cloned() {
                    open_recent(s, item, tx);
                }
            }));
        }
        card = card.child(list);
    }
    let action = |icon: &'static rune::Icon, words: String| {
        row()
            .w_full()
            .h(30.0)
            .px(8.0)
            .gap(9.0)
            .center_y()
            .radius(7.0)
            .hover(l.c("hover-surface"))
            .child(crate::look::controls::glyph(&l, icon, 14.0, Some(l.c("text-faint"))))
            .child(text(words.clone()).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")).grow(1.0).shrink().truncate())
            .role("button")
            .label(words)
    };
    let new_id = teammate.id.clone();
    card = card.child(rule()).child(
        col()
            .w_full()
            .padding(6.0)
            .gap(1.0)
            .child(action(&crate::look::icons::INBOX, format!("All of {}'s conversations", teammate.name)).on_press(|s: &mut State| {
                close(s);
                home_panel::open_board(s);
            }))
            .child(action(&crate::look::icons::PLUS, format!("New conversation with {}", teammate.name)).on_press_tx(move |s: &mut State, tx| {
                close(s);
                select_teammate(s, &new_id, tx);
                new_conversation(s);
            })),
    );
    let card = card.on_hover_change(|s: &mut State, over, _| {
        s.rail_over_card = over;
        if !over {
            s.rail_left_at = Some(std::time::Instant::now());
        }
    });
    let card = if s.rail_pinned { card.modal(true, close) } else { card };
    Some(el().absolute(RAIL_WIDTH + 8.0, (top - 6.0).max(8.0)).child(card))
}

/// TitleBar.tsx: the folder's name (or `Locust` and the build with none), the toggles while no conversation is open,
/// and how many turns are running.
pub(super) fn title_bar(s: &State, p: &Palette, toggles: bool, running: usize) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let name = s
        .home
        .folder_path
        .as_deref()
        .and_then(|path| std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if s.home.current_version.is_empty() { "Locust".into() } else { format!("Locust {}", s.home.current_version) }
        });
    let mut right = row().gap(12.0).center_y();
    if toggles {
        right = right.child(browser_panel::toggle(p)).child(terminal_panel::toggle(p));
    }
    if running > 0 {
        right = right.child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    el().size(6.0, 6.0).radius(3.0).fill(p.green).cycle(Cycle {
                        period: 1.6,
                        phase: 0.0,
                        stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)],
                        still: 1.0,
                        fps: 30.0,
                    }),
                )
                .child(text(format!("{running} running")).font(rune::TextStyle::new(11.5, 400.0).mono()).color(l.c("text-muted"))),
        );
    }
    row()
        .key("titlebar")
        .w_full()
        .h(TITLEBAR_HEIGHT)
        .px(12.0)
        .center_y()
        .fill(l.c("bg-chrome"))
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
        .window_drag()
        .child(el().w(0.0).grow(1.0))
        .child(text(name).font(rune::TextStyle::new(13.0, 600.0)).color(l.c("text-muted")).truncate())
        .child(row().w(0.0).grow(1.0).justify_end().child(right.no_window_drag()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mission(id: &str, phase: &str, at: &str) -> SidebarMission {
        SidebarMission {
            mission_id: id.into(),
            title: id.into(),
            phase: phase.into(),
            last_at: Some(at.into()),
            root_id: None,
            member_ids: vec![id.into()],
            owner_id: None,
            said: String::new(),
            nested_under: None,
            integrity_issue_count: 0,
        }
    }

    #[test]
    fn the_rail_follows_the_width_unless_someone_chose() {
        assert!(compact_for("auto", 1100.0) && compact_for("auto", 1199.0) && !compact_for("auto", 1200.0));
        assert!(compact_for("compact", 1600.0) && !compact_for("wide", 900.0));
        assert_eq!(layout_lede("auto", true), "Auto. Following this window, currently the rail.");
        assert_eq!(layout_lede("wide", true), "Full. The names beside the faces, whatever the window.");
    }

    #[test]
    fn the_card_says_what_electron_says() {
        assert_eq!(count_badge(1), None);
        assert_eq!(count_badge(2).as_deref(), Some("2"));
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-11T12:00:00Z").unwrap().timestamp_millis();
        let ago = |iso: &str| short_ago(Some(iso), now).unwrap();
        assert_eq!(ago("2026-11-11T11:59:30Z"), "now");
        assert_eq!(ago("2026-11-11T11:55:00Z"), "5m");
        assert_eq!(ago("2026-11-11T09:00:00Z"), "3h");
        assert_eq!(ago("2026-11-08T12:00:00Z"), "3d");
        assert_eq!(ago("2026-10-21T12:00:00Z"), "3w");
        assert_eq!(short_ago(Some("not a time"), now), None);
        let many: Vec<_> = (0..9).map(|i| mission(&format!("m{i}"), if i == 4 { "running" } else { "completed" }, &format!("2026-11-0{}T10:00:00Z", i + 1))).collect();
        let (shown, label) = rail_rows(&many);
        assert_eq!(label, "6 of 9");
        assert_eq!(shown.len(), 6);
        assert_eq!(shown[0].mission_id, "m4", "the live one first");
        assert_eq!(shown[1].mission_id, "m8", "then newest first");
        assert_eq!(empty_line("Wren"), "Nothing yet — pick Wren and write below.");
    }
}

/// The rail with Scout's card pinned open beside it, over Home.
pub(super) fn flyout_scene(s: &mut State) {
    home_panel::home_scene(s);
    s.rail_open = s.teammates.last().map(|t| t.id.clone());
    s.rail_pinned = true;
}

pub(super) fn flyout_light_scene(s: &mut State) {
    flyout_scene(s);
    s.appearance = Some(false);
}

/// Settings › Appearance with the Sidebar line, the rail chosen.
pub(super) fn sidebar_setting_scene(s: &mut State) {
    settings_panel::scene(s);
    s.layout_pref = "compact".into();
}
