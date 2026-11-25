//! Command palette — port of `renderer/src/components/CommandPalette.tsx`.

use super::*;
use crate::conversation_list::SidebarMission;

#[derive(Clone, Debug)]
pub struct PaletteAction {
    pub id: String,
    pub group: String,
    pub label: String,
    pub keywords: Option<String>,
    /// Listed only once something is typed.
    pub when_typed: bool,
}

/// Filter and group the way Electron does: group order is first appearance; within group, stable.
pub fn palette_matches(actions: &[PaletteAction], query: &str) -> Vec<PaletteAction> {
    let needle = query.trim().to_lowercase();
    let matches: Vec<&PaletteAction> = if needle.is_empty() {
        actions.iter().filter(|a| !a.when_typed).collect()
    } else {
        actions
            .iter()
            .filter(|a| {
                format!(
                    "{} {} {}",
                    a.group,
                    a.label,
                    a.keywords.as_deref().unwrap_or("")
                )
                .to_lowercase()
                .contains(&needle)
            })
            .collect()
    };
    let mut order = std::collections::HashMap::new();
    let mut next = 0usize;
    for action in &matches {
        if let std::collections::hash_map::Entry::Vacant(slot) = order.entry(action.group.clone()) {
            slot.insert(next);
            next += 1;
        }
    }
    let mut grouped: Vec<PaletteAction> = matches.into_iter().cloned().collect();
    grouped.sort_by(|left, right| {
        order
            .get(&left.group)
            .unwrap_or(&0)
            .cmp(order.get(&right.group).unwrap_or(&0))
    });
    grouped
}

pub fn default_palette_actions(
    conversations: &[SidebarMission],
    has_flight: bool,
    has_record_conversation: bool,
) -> Vec<PaletteAction> {
    // Electron supplies only commands the current build can execute.
    let mut actions = vec![PaletteAction {
        id: "back".into(),
        group: "This conversation".into(),
        label: "Back to the conversation".into(),
        keywords: None,
        when_typed: false,
    }];
    if has_record_conversation {
        actions.push(PaletteAction {
            id: "save-record".into(),
            group: "This conversation".into(),
            label: "Save the record…".into(),
            keywords: Some("export markdown record json".into()),
            when_typed: false,
        });
    }
    if has_flight {
        actions.push(PaletteAction {
            id: "stop".into(),
            group: "This conversation".into(),
            label: "Stop the running reply".into(),
            keywords: None,
            when_typed: false,
        });
    }
    actions.push(PaletteAction {
        id: "import-conversation".into(),
        group: "Go to".into(),
        label: "Import a conversation…".into(),
        keywords: Some("claude codex opencode cursor session".into()),
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "home".into(),
        group: "Go to".into(),
        label: "Home".into(),
        keywords: Some("cover team starters away".into()),
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "go-missions".into(),
        group: "Go to".into(),
        label: "Conversations".into(),
        keywords: None,
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "board".into(),
        group: "Go to".into(),
        label: "Board".into(),
        keywords: Some("needs you working done approvals".into()),
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "settings".into(),
        group: "Go to".into(),
        label: "Settings".into(),
        keywords: Some("preferences appearance theme".into()),
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "usage".into(),
        group: "Go to".into(),
        label: "Usage".into(),
        keywords: Some("cost spend tokens meters".into()),
        when_typed: false,
    });
    actions.push(PaletteAction {
        id: "keyboard".into(),
        group: "Go to".into(),
        label: "Keyboard shortcuts".into(),
        keywords: Some("hotkeys chords".into()),
        when_typed: false,
    });
    for (i, row) in conversations.iter().enumerate() {
        actions.push(PaletteAction {
            id: format!("conv:{}", row.mission_id),
            group: "Conversations".into(),
            label: row.title.clone(),
            keywords: Some(format!("{} {}", row.mission_id, row.said)),
            when_typed: i >= 5,
        });
    }
    actions
}

/// App.tsx: a conversation in the palette is found by every turn's words and by its teammate's name too
/// (`keywords: ${mission.said} ${owner}`), as the sidebar's search finds it.
pub(super) fn with_owner_names(s: &mut State) {
    let owners: std::collections::HashMap<&str, &str> = s.recent.iter().map(|r| (r.id.as_str(), r.teammate.as_str())).collect();
    for action in &mut s.palette_actions {
        if let Some(id) = action.id.strip_prefix("conv:")
            && let Some(owner) = owners.get(id).filter(|o| !o.trim().is_empty())
        {
            let words = action.keywords.get_or_insert_with(String::new);
            words.push(' ');
            words.push_str(owner);
        }
    }
}

/// The palette head's bottom edge: lit (`border-hover`) while the query holds focus, hairline otherwise.
pub(super) fn palette_head_edge(focused: bool, l: &crate::look::controls::Look) -> Color {
    if focused { l.c("border-hover") } else { l.c("border-hairline") }
}

pub(super) fn command_palette(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let head_edge = palette_head_edge(s.palette_query.focused, &l);
    let actions = palette_matches(&s.palette_actions, &s.palette_query.text);
    let mut list = col().w_full().gap(2.0).padding(8.0);
    if actions.is_empty() {
        list = list.child(
            text("Nothing matches that.")
                .font(type_::BODY)
                .color(p.muted)
                .w_full()
                .padding(12.0),
        );
    } else {
        let mut last_group = String::new();
        for (i, action) in actions.iter().enumerate() {
            if action.group != last_group {
                last_group = action.group.clone();
                list = list.child(
                    text(action.group.clone())
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full()
                        .px(12.0)
                        .py(6.0),
                );
            }
            let selected = i == s.palette_index;
            let label = action.label.clone();
            let id = action.id.clone();
            list = list.child(
                row()
                    .key(format!("palette-{id}"))
                    .w_full()
                    .center_y()
                    .px(12.0)
                    .py(10.0)
                    .radius(6.0)
                    .fill(if selected { p.selected } else { p.raised })
                    .child(
                        text(label)
                            .font(type_::BODY)
                            .color(p.text)
                            .grow(1.0)
                            .truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        run_palette_action(s, &id, tx);
                    }),
            );
        }
    }

    col()
        .key("command-palette")
        .w(520.0)
        .h(420.0)
        .radius(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            col()
                .key("palette-head")
                .w_full()
                .child(
                    col()
                        .w_full()
                        .padding(12.0)
                        .gap(8.0)
                        .child(
                            text("Command palette")
                                .font(type_::LABEL)
                                .color(p.secondary),
                        )
                        .child(
                            text_input(&s.palette_query, type_::BODY)
                                .key("palette-query")
                                .no_focus_ring()
                                .on_input_key(on_key)
                                .w_full()
                                .color(p.text)
                                .placeholder("Search conversations, or type a command…", p.muted)
                                .padding(10.0)
                                .radius(6.0)
                                .fill(p.field)
                                .on_edit(|s: &mut State| {
                                    s.palette_index = 0;
                                    &mut s.palette_query
                                }),
                        ),
                )
                .child(el().key("palette-head-edge").w_full().h(1.0).fill(head_edge)),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(200.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        )
}

pub(super) fn run_palette_action(s: &mut State, id: &str, tx: &mut Tx) {
    s.palette_open = false;
    match id {
        "back" => {}
        "save-record" => {
            if let Some(mission_id) = crate::active_record_mission_id(s) {
                crate::save_record::open(s, mission_id);
            }
        }
        "import-conversation" => crate::import_dialog::open(s),
        "stop" => stop(s),
        "home" => crate::home_panel::open_home(s),
        "board" => crate::home_panel::open_board(s),
        "go-missions" => crate::missions_screen::open(s),
        "settings" => crate::settings_panel::open(s),
        "usage" => crate::usage_panel::open(s),
        "keyboard" => crate::keyboard_sheet::open(s),
        id if id.starts_with("conv:") => {
            let mission_id = &id[5..];
            if let Some(item) = s.recent.iter().find(|r| r.id == mission_id).cloned() {
                // Use the same opener as a sidebar row.
                open_recent(s, item, tx);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "search_every_turn_tests.rs"]
mod search_every_turn_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_filters_and_groups_like_electron() {
        let actions = vec![
            PaletteAction {
                id: "a".into(),
                group: "Go to".into(),
                label: "Team".into(),
                keywords: None,
                when_typed: false,
            },
            PaletteAction {
                id: "b".into(),
                group: "Conversations".into(),
                label: "Hidden until typed".into(),
                keywords: Some("invoice".into()),
                when_typed: true,
            },
            PaletteAction {
                id: "c".into(),
                group: "Go to".into(),
                label: "Settings".into(),
                keywords: None,
                when_typed: false,
            },
        ];
        let empty = palette_matches(&actions, "");
        assert_eq!(empty.len(), 2);
        assert!(empty.iter().all(|a| a.id != "b"));
        let found = palette_matches(&actions, "invoice");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "b");
        let go = palette_matches(&actions, "go to");
        assert_eq!(
            go.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        assert_eq!(
            empty.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(),
            vec!["Team", "Settings"]
        );
    }

    #[test]
    fn palette_empty_words() {
        assert!(palette_matches(&[], "x").is_empty());
    }

    #[test]
    fn palette_includes_save_record_only_when_conversation_is_open() {
        let without = default_palette_actions(&[], false, false);
        assert!(without.iter().all(|a| a.id != "save-record"));

        let with = default_palette_actions(&[], false, true);
        let entry = with
            .iter()
            .find(|a| a.id == "save-record")
            .expect("save-record palette action");
        assert_eq!(entry.group, "This conversation");
        assert_eq!(entry.label, "Save the record…");
    }

    // case: turns the global focus ring off at a specificity that beats it
    #[test]
    fn turns_the_global_focus_ring_off_at_a_specificity_that_beats_it() {
        let mut s = State::default();
        s.palette_open = true;
        s.palette_query.focused = true;
        let mut rt = rune::Runtime::new(
            s,
            |s, _| command_palette(s, &Palette::new(true)),
            (600.0, 500.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        assert!(rt.find("palette-query").is_some());
        let src = include_str!("command_palette.rs");
        let at = src.find(r#".key("palette-query")"#).expect("palette-query");
        let block = &src[at..at + 180];
        assert!(block.contains(".no_focus_ring()"), "palette-query must suppress the global focus ring: {block}");
        assert!(!block.contains(".focus_ring("), "palette-query must not wear an inner focus ring: {block}");
    }

    // case: keeps the head’s lit edge as the focus it shows
    #[test]
    fn keeps_the_heads_lit_edge_as_the_focus_it_shows() {
        let l = crate::look::controls::Look::new(true);
        let draw = |focused: bool| {
            let mut s = State::default();
            s.palette_open = true;
            s.palette_query.focused = focused;
            let mut rt = rune::Runtime::new(
                s,
                |s, _| command_palette(s, &Palette::new(true)),
                (600.0, 500.0),
                rune::Env::new(true, 1.0),
            );
            rt.frame(0.0);
            rt.elements()
                .into_iter()
                .find(|e| e.key.as_deref() == Some("palette-head-edge"))
                .expect("palette-head-edge")
                .fill
        };
        let lit = draw(true);
        let dim = draw(false);
        assert_eq!(lit, l.c("border-hover"));
        assert_eq!(dim, l.c("border-hairline"));
        assert_ne!(lit, dim);
    }
}
