//! Project settings dialog (`apps/desktop/src/renderer/src/components/GroupSettingsDialog.tsx`).
//!
//! Edits a project group's standing instructions (briefed on every future turn after the
//! folder's `LOCUST.md`) and its default route (seeded in the composer when a conversation
//! joins the project).

use crate::look::controls::{Look, button, ghost_button, glyph, primary_button, Tone};
use crate::look::icons;
use crate::{org, selected_conversation, Palette, State, TurnPick, LEDGER_TARGET, type_};
use locust_core::groups::{is_valid_route, utf16_prefix, GroupRoute, MAX_GROUP_INSTRUCTIONS_LENGTH};
use rune::{col, el, row, text, text_input, El, TextEdit};

const CLAIM: &str = "Briefs every turn started from now on. Turns already run were not briefed.";
const FOOT: &str = "Given after the folder's own LOCUST.md.";
const PLACEHOLDER: &str =
    "Run the tests before calling anything done. Keep changes to the files the task names.";
const ROUTE_CLAIM: &str =
    "Selected in the composer when a conversation joins this project. Each conversation owns its route after that.";
const ROUTE_NONE: &str = "None — each conversation keeps what it has";
const WARN_AT: usize = 3600;

#[derive(Default)]
pub(super) struct Panel {
    pub(super) open: bool,
    pub(super) group_id: String,
    pub(super) group_name: String,
    pub(super) saved_instructions: String,
    pub(super) draft: TextEdit,
    pub(super) saved_route: Option<GroupRoute>,
    pub(super) route: Option<GroupRoute>,
    pub(super) error: Option<String>,
}

pub(super) fn open(s: &mut State, group_id: &str) {
    let Some(group) = s.org.groups.iter().find(|g| g.group_id == group_id).cloned() else {
        return;
    };
    let mut draft = TextEdit::new(&group.instructions);
    draft.focused = true;
    s.group_settings = Panel {
        open: true,
        group_id: group.group_id,
        group_name: group.name,
        saved_instructions: group.instructions,
        draft,
        saved_route: group.route.clone(),
        route: group.route,
        error: None,
    };
}

pub(super) fn close(s: &mut State) {
    s.group_settings.open = false;
    s.group_settings.error = None;
}

/// The composer's current route (runtime, model, mode, account, effort) for `Use current`.
pub(super) fn current_composer_route(s: &State) -> Option<GroupRoute> {
    let teammate = selected_conversation(s)
        .map(|(t, _)| t)
        .or_else(|| s.teammates.get(s.selected))?;
    let pick = s.picks.get(&teammate.id);
    let runtime = pick
        .and_then(|p| p.runtime.clone())
        .or_else(|| teammate.runtime.clone())?;
    let model = pick
        .map(|p| p.model.clone())
        .filter(|m| !m.trim().is_empty())
        .or_else(|| teammate.model.clone().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| "account-default".into());
    let mode = s
        .composer_mode
        .clone()
        .unwrap_or_else(|| teammate.runs_in.clone());
    let account = pick
        .and_then(|p| p.account.as_ref().and_then(|a| a.clone()))
        .or_else(|| teammate.account.clone());
    let effort = pick.and_then(|p| p.effort.clone());
    let route = GroupRoute {
        runtime,
        model,
        mode,
        account,
        effort,
    };
    is_valid_route(&route).then_some(route)
}

pub(super) fn seed_route_from_group(s: &mut State, route: &GroupRoute) {
    let target = selected_conversation(s)
        .map(|(t, _)| t.id.clone())
        .or_else(|| s.teammates.get(s.selected).map(|t| t.id.clone()))
        .unwrap_or_else(|| LEDGER_TARGET.to_owned());
    s.picks.insert(
        target,
        TurnPick {
            model: route.model.clone(),
            effort: route.effort.clone(),
            runtime: Some(route.runtime.clone()),
            account: route.account.clone().map(Some),
        },
    );
    s.composer_mode = Some(route.mode.clone());
}

pub(super) fn route_label(s: &State, route: &GroupRoute) -> String {
    let rt_name = locust_core::carry_on::display_name(&route.runtime);
    let model_name = if route.model == "account-default" {
        "Account default".to_owned()
    } else {
        s.models
            .values()
            .flatten()
            .filter(|a| a.runtime == route.runtime)
            .flat_map(|a| a.models.iter())
            .find(|m| m.id == route.model)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| route.model.clone())
    };
    let mode_chip = locust_core::mode_facts::fact(&route.mode).chip;
    let mut parts = vec![format!("{rt_name} / {model_name}"), mode_chip.to_owned()];
    if let Some(effort) = &route.effort
        && !effort.is_empty()
    {
        parts.push(effort.clone());
    }
    parts.join(" · ")
}

pub(super) fn save(s: &mut State) -> bool {
    let Some(store) = s.data.clone() else {
        s.group_settings.error = Some("No native data folder for project settings.".into());
        return false;
    };
    let gid = s.group_settings.group_id.clone();
    let trimmed = utf16_prefix(
        s.group_settings.draft.text.trim(),
        MAX_GROUP_INSTRUCTIONS_LENGTH,
    );
    let inst_changed = trimmed != s.group_settings.saved_instructions.trim();
    let route_changed = s.group_settings.route != s.group_settings.saved_route;
    if !inst_changed && !route_changed {
        return false;
    }
    if inst_changed {
        if let Err(err) = org::set_group_instructions(&store, &mut s.org, &gid, &trimmed) {
            s.group_settings.error = Some(err);
            return false;
        }
        s.group_settings.saved_instructions = trimmed;
    }
    if route_changed {
        let next_route = s.group_settings.route.clone();
        if let Err(err) = org::set_group_route(&store, &mut s.org, &gid, next_route.clone()) {
            s.group_settings.error = Some(err);
            return false;
        }
        s.group_settings.saved_route = next_route;
    }
    close(s);
    true
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let trimmed_draft = utf16_prefix(
        s.group_settings.draft.text.trim(),
        MAX_GROUP_INSTRUCTIONS_LENGTH,
    );
    let utf16_len = s.group_settings.draft.text.encode_utf16().count();
    let inst_changed = trimmed_draft != s.group_settings.saved_instructions.trim();
    let route_changed = s.group_settings.route != s.group_settings.saved_route;
    let changed = inst_changed || route_changed;
    let clearing_only =
        trimmed_draft.is_empty() && !s.group_settings.saved_instructions.trim().is_empty();
    let save_label = if clearing_only && !route_changed {
        "Clear instructions"
    } else {
        "Save"
    };

    let mut inst_head = row()
        .w_full()
        .center_y()
        .child(
            text("Standing instructions")
                .font(type_::LABEL)
                .color(p.secondary)
                .grow(1.0),
        );
    if utf16_len > WARN_AT {
        inst_head = inst_head.child(
            text(format!("{utf16_len} / {MAX_GROUP_INSTRUCTIONS_LENGTH}"))
                .key("group-settings-count")
                .font(type_::LABEL)
                .color(if utf16_len > MAX_GROUP_INSTRUCTIONS_LENGTH {
                    p.red
                } else {
                    p.muted
                }),
        );
    }

    let route_now_label = s
        .group_settings
        .route
        .as_ref()
        .map(|r| route_label(s, r))
        .unwrap_or_else(|| ROUTE_NONE.to_owned());
    let composer_route = current_composer_route(s);
    let route_foot = if let Some(picked) = &s.group_settings.route {
        format!("Composer now shows {}.", route_label(s, picked))
    } else if let Some(cur) = &composer_route {
        format!("Use current takes {} from the composer.", route_label(s, cur))
    } else {
        "Pick a teammate in the composer to set a default route here.".to_owned()
    };

    let mut card = col()
        .w(500.0)
        .padding(20.0)
        .gap(14.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .radius(12.0)
        .key("group-settings-dialog")
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Project settings")
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(
                    text(s.group_settings.group_name.clone())
                        .key("group-settings-name")
                        .font(type_::LABEL)
                        .color(p.muted),
                )
                .child(
                    glyph(&l, &icons::CLOSE, 18.0, Some(p.muted))
                        .padding(4.0)
                        .radius(6.0)
                        .key("group-settings-close")
                        .on_press(|s: &mut State| close(s)),
                ),
        )
        .child(
            col()
                .w_full()
                .gap(6.0)
                .child(inst_head)
                .child(text(CLAIM).font(type_::BODY).color(p.secondary).wrap())
                .child(
                    text_input(&s.group_settings.draft, type_::BODY)
                        .key("group-settings-instructions")
                        .w_full()
                        .min_h(96.0)
                        .max_h(180.0)
                        .padding(10.0)
                        .fill(p.field)
                        .border(1.0, p.hairline)
                        .radius(8.0)
                        .color(p.text)
                        .placeholder(PLACEHOLDER, p.muted)
                        .on_edit(|s: &mut State| &mut s.group_settings.draft),
                )
                .child(text(FOOT).font(type_::LABEL).color(p.muted).wrap()),
        )
        .child(
            col()
                .w_full()
                .gap(6.0)
                .child(
                    text("Default route")
                        .font(type_::LABEL)
                        .color(p.secondary),
                )
                .child(text(ROUTE_CLAIM).font(type_::BODY).color(p.secondary).wrap())
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(8.0)
                        .child(
                            el().grow(1.0)
                                .padding(8.0)
                                .radius(6.0)
                                .fill(p.field)
                                .border(1.0, p.hairline)
                                .child(
                                    text(route_now_label)
                                        .key("group-settings-route-value")
                                        .font(type_::BODY)
                                        .color(if s.group_settings.route.is_some() {
                                            p.text
                                        } else {
                                            p.muted
                                        })
                                        .wrap(),
                                ),
                        )
                        .child(
                            button(&l, "Use current", Tone::Neutral, composer_route.is_none())
                                .key("group-settings-use-current")
                                .on_press(|s: &mut State| {
                                    if let Some(cur) = current_composer_route(s) {
                                        s.group_settings.route = Some(cur);
                                        s.group_settings.error = None;
                                    }
                                }),
                        )
                        .child(
                            button(
                                &l,
                                "Clear",
                                Tone::Neutral,
                                s.group_settings.route.is_none(),
                            )
                            .key("group-settings-clear-route")
                            .on_press(|s: &mut State| {
                                s.group_settings.route = None;
                                s.group_settings.error = None;
                            }),
                        ),
                )
                .child(text(route_foot).font(type_::LABEL).color(p.muted).wrap()),
        );

    if let Some(error) = &s.group_settings.error {
        card = card.child(
            text(error.clone())
                .key("group-settings-error")
                .font(type_::LABEL)
                .color(p.red)
                .wrap(),
        );
    }

    card.child(
        row()
            .w_full()
            .gap(8.0)
            .pt(4.0)
            .child(el().grow(1.0))
            .child(
                ghost_button(&l, "Cancel")
                    .key("group-settings-cancel")
                    .on_press(|s: &mut State| close(s)),
            )
            .child(
                primary_button(&l, save_label, !changed)
                    .key("group-settings-save")
                    .on_press(|s: &mut State| {
                        save(s);
                    }),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Teammate;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TICK: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let n = TICK.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "locust-group-dlg-{}-{}",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_state() -> (TempDir, State) {
        let dir = TempDir::new();
        let electron = dir.0.join("electron");
        let native = dir.0.join("native");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        let store = crate::store::Store::new(native, electron).unwrap();
        let mut s = State::default();
        s.data = Some(store);
        s.teammates.push(Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Custom".into(),
            model: Some("gpt-5".into()),
            runtime: Some("codex".into()),
            folder: None,
            runs_in: "ask".into(),
            account: None,
            busy: false,
        });
        (dir, s)
    }

    fn all_text(s: &mut State) -> String {
        let taken = std::mem::take(s);
        let mut rt = rune::Runtime::new(
            taken,
            |s, _| view(s, &Palette::new(true)),
            (900.0, 700.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let out = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        *s = rt.state;
        out
    }

    // Ported from renderer/src/group-settings-dialog.test.tsx (5 it cases)

    #[test]
    fn renders_project_settings_instructions_and_default_route_empty_state() {
        let (_dir, mut s) = test_state();
        let gid = org::create_group(s.data.as_ref().unwrap(), &mut s.org, "Locust").unwrap();
        open(&mut s, &gid);
        let text = all_text(&mut s);
        assert!(text.contains("Project settings"));
        assert!(text.contains("Locust"));
        assert!(text.contains("Standing instructions"));
        assert!(text.contains(CLAIM));
        assert!(text.contains("Default route"));
        assert!(text.contains(ROUTE_NONE));
        assert!(text.contains("Use current"));
        assert!(text.contains("Clear"));
        assert!(text.contains("Cancel"));
        assert!(text.contains("Save"));
    }

    #[test]
    fn shows_character_counter_above_3600_and_clear_instructions_label() {
        let (_dir, mut s) = test_state();
        let gid = org::create_group(s.data.as_ref().unwrap(), &mut s.org, "Locust").unwrap();
        org::set_group_instructions(s.data.as_ref().unwrap(), &mut s.org, &gid, "Saved rule.")
            .unwrap();
        open(&mut s, &gid);

        s.group_settings.draft.replace_text(&"a".repeat(3700));
        let text = all_text(&mut s);
        assert!(text.contains("3700 / 4000"));
        assert!(text.contains("Save"));

        s.group_settings.draft.replace_text("");
        let cleared_text = all_text(&mut s);
        assert!(cleared_text.contains("Clear instructions"));
    }

    #[test]
    fn use_current_and_clear_update_route_and_save_persists_both() {
        let (_dir, mut s) = test_state();
        let gid = org::create_group(s.data.as_ref().unwrap(), &mut s.org, "Locust").unwrap();
        s.picks.insert(
            "tm_codec".into(),
            TurnPick {
                model: "claude-sonnet".into(),
                effort: Some("high".into()),
                runtime: Some("claude".into()),
                account: None,
            },
        );
        s.composer_mode = Some("accept-edits".into());
        open(&mut s, &gid);

        let cur = current_composer_route(&s).unwrap();
        assert_eq!(cur.runtime, "claude");
        assert_eq!(cur.model, "claude-sonnet");
        assert_eq!(cur.mode, "accept-edits");
        assert_eq!(cur.effort.as_deref(), Some("high"));

        s.group_settings.route = Some(cur.clone());
        s.group_settings.draft.replace_text("  Keep tests passing.  ");
        let with_route = all_text(&mut s);
        assert!(with_route.contains("Composer now shows"));

        assert!(save(&mut s));
        assert!(!s.group_settings.open);
        let reloaded = org::load_org(s.data.as_ref().unwrap());
        assert_eq!(reloaded.groups[0].instructions, "Keep tests passing.");
        assert_eq!(reloaded.groups[0].route.as_ref(), Some(&cur));

        // Reopen and clear route
        open(&mut s, &gid);
        s.group_settings.route = None;
        assert!(save(&mut s));
        let reloaded = org::load_org(s.data.as_ref().unwrap());
        assert_eq!(reloaded.groups[0].route, None);
    }

    #[test]
    fn save_does_nothing_when_unchanged_and_refuses_secrets_while_keeping_open() {
        let (_dir, mut s) = test_state();
        let gid = org::create_group(s.data.as_ref().unwrap(), &mut s.org, "Trading").unwrap();
        open(&mut s, &gid);
        assert!(!save(&mut s));
        assert!(s.group_settings.open);

        let secret = format!("Use {}-ant-api03-1234567890abcdefghijklmnop here", "sk");
        s.group_settings.draft.replace_text(&secret);
        assert!(!save(&mut s));
        assert!(s.group_settings.open);
        let err = s.group_settings.error.as_deref().unwrap();
        assert!(err.contains("Trading"), "err: {err}");
        let err_text = all_text(&mut s);
        assert!(err_text.contains("Trading"));
    }

    #[test]
    fn seed_route_from_group_populates_composer_pick_and_mode() {
        let (_dir, mut s) = test_state();
        let route = GroupRoute {
            runtime: "claude".into(),
            model: "claude-opus".into(),
            mode: "plan".into(),
            account: Some("acct_work".into()),
            effort: Some("high".into()),
        };
        seed_route_from_group(&mut s, &route);
        let pick = s.picks.get("tm_codec").unwrap();
        assert_eq!(pick.runtime.as_deref(), Some("claude"));
        assert_eq!(pick.model, "claude-opus");
        assert_eq!(pick.effort.as_deref(), Some("high"));
        assert_eq!(pick.account, Some(Some("acct_work".into())));
        assert_eq!(s.composer_mode.as_deref(), Some("plan"));
    }
}
