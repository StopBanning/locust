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
) -> Vec<PaletteAction> {
    // Electron supplies only commands the current build can execute.
    let mut actions = vec![PaletteAction {
        id: "back".into(),
        group: "This conversation".into(),
        label: "Back to the conversation".into(),
        keywords: None,
        when_typed: false,
    }];
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
        id: "home".into(),
        group: "Go to".into(),
        label: "Home".into(),
        keywords: Some("cover team starters away".into()),
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

pub(super) fn command_palette(s: &State, p: &Palette) -> El {
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
        "stop" => stop(s),
        "home" => crate::home_panel::open_home(s),
        "board" => crate::home_panel::open_board(s),
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
}
