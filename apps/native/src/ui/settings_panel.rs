//! Categorized Settings screen (Area 11).

use super::*;
use crate::settings_pages::{
    self, HOW_IT_WORKS, RUNS_IN_LOCUST, SEARCH_EMPTY, SEARCH_PLACEHOLDER, SETTINGS_PAGES,
    SETTINGS_TITLE, SettingsGroup, SettingsPageId,
};
use crate::workspace_settings::{
    self, DATA_FOLDER_HEADING, MEMORY_ASK, MEMORY_KEEP, MEMORY_OFF, MEMORY_WHEN,
    PROJECT_FOLDER_LEDE, THEME_HEADING, ThemeChoice,
};

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub page: Option<SettingsPageId>,
    pub search: TextEdit,
    pub settings: workspace_settings::WorkspaceSettings,
    pub theme: ThemeChoice,
    pub data_folder: String,
    pub error: String,
    /// Locust's door, as its switches stand (None when this window has no door).
    pub door: Option<locust_core::door::State>,
    /// "When an agent cannot go on": the person's pick, and the agents that can carry work on here.
    pub plan_runs_out: Option<locust_core::carry_on::PlanRunsOut>,
    pub carry_choices: Vec<String>,
    /// "AI agents & accounts": what discovery found (None until the first check lands), and the check itself.
    pub agents: Option<Vec<locust_core::agents::Found>>,
    pub agents_job: Latest<Vec<locust_core::agents::Found>>,
    /// "OWN COPIES": the project folder's teammate copies, and what a Remove came back with.
    pub own_copies: Option<crate::own_copies::OwnCopies>,
    pub check_command: TextEdit,
    /// "Privacy & data": storage report, retention, and backup/restore state.
    pub privacy: crate::backup_section::PrivacyPanelState,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

fn active_button(label: impl Into<String>, p: &Palette, on: bool) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(if on { p.text } else { p.secondary })
        .padding(7.0)
        .fill(if on { p.selected } else { p.field })
        .radius(6.0)
}

pub(super) fn open(s: &mut State) {
    s.settings_panel.open = true;
    s.settings_panel.page = SettingsPageId::from_str("appearance");
    reload(s);
    if s.home.changelog.is_empty() {
        let text = crate::changelog::native_notes(None);
        s.home.changelog = crate::changelog::entries(&text);
        s.home.changelog_shown = crate::changelog::WHATS_NEW_PAGE;
    }
}

fn active_project_folder(s: &State) -> Option<String> {
    s.teammates
        .get(s.selected)
        .and_then(|t| t.folder.clone())
        .or_else(|| s.home.folder_path.clone())
        .filter(|p| !p.trim().is_empty())
}

fn reload(s: &mut State) {
    let active_ws = active_project_folder(s)
        .map(|p| workspace_settings::workspace_id_for(std::path::Path::new(&p)));
    match &s.data {
        None => {
            s.settings_panel.error = "Native data folder could not be determined.".into();
        }
        Some(store) => {
            s.settings_panel.data_folder = store.root().display().to_string();
            match workspace_settings::load(store) {
                Ok(settings) => {
                    let saved_cmd = active_ws
                        .as_deref()
                        .and_then(|ws| settings.check_commands.get(ws))
                        .cloned()
                        .unwrap_or_default();
                    s.settings_panel.check_command.replace_text(saved_cmd);
                    s.settings_panel.settings = settings;
                    s.settings_panel.error.clear();
                }
                Err(e) => s.settings_panel.error = e,
            }
            s.settings_panel.door = crate::door_now(locust_core::engine_api::DoorChange::Read);
            s.settings_panel.plan_runs_out = locust_core::people::team(store)
                .ok()
                .and_then(|team| locust_core::carry_on::parse_plan_runs_out(&team["settings"]["planRunsOut"]));
            s.settings_panel.carry_choices = locust_core::carry_on::installed_runtimes();
            s.settings_panel.theme = workspace_settings::load_theme(store);
            s.appearance = s.settings_panel.theme.to_appearance();
            // Round-trip keeps State.appearance and the stored theme aligned.
            s.settings_panel.theme = ThemeChoice::from_appearance(s.appearance);
        }
    }
    crate::agents_section::request(s);
    crate::backup_section::reload(s);
    let folder = active_project_folder(s);
    crate::own_copies::reload(s, folder);
}

fn persist(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.error = "Native data folder could not be determined.".into();
        return;
    };
    match workspace_settings::save(&store, &s.settings_panel.settings) {
        Ok(()) => {
            s.settings_panel.error.clear();
            // memoryMode is read by people::brief and memory acceptance.
        }
        Err(e) => s.settings_panel.error = e,
    }
}

fn set_theme(s: &mut State, theme: ThemeChoice) {
    s.settings_panel.theme = theme;
    s.appearance = theme.to_appearance();
    if let Some(store) = &s.data
        && let Err(e) = workspace_settings::save_theme(store, theme)
    {
        s.settings_panel.error = e;
    }
}

/// Screens.tsx's Sidebar line: Auto follows the window, Full and Rail hold whatever its width.
fn sidebar_line(s: &State, p: &Palette) -> El {
    let pref = s.layout_pref.as_str();
    let choice = |value: &'static str, label: &str| {
        active_button(label, p, pref == value || (value == "auto" && !matches!(pref, "wide" | "compact")))
            .on_press(move |s: &mut State| crate::rail::choose_layout(s, value))
    };
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .child(text("Sidebar").font(type_::TITLE).color(p.text))
        .child(text(crate::rail::layout_lede(pref, s.layout_compact.get())).font(type_::LABEL).color(p.secondary).wrap())
        .child(row().gap(6.0).child(choice("auto", "Auto")).child(choice("wide", "Full")).child(choice("compact", "Rail")))
}

/// The heading Electron's General page gives the door.
const DOOR_HEADING: &str = "Let your other AI apps use Locust";

/// One switch with its words (`.lc-settingline`), indented when it belongs to the line above (`--sub`).
fn setting_line(l: &crate::look::controls::Look, heading: &str, lede: &str, on: bool, sub: bool, press: fn(&mut State)) -> El {
    use crate::look::controls;
    let words = col()
        .grow(1.0)
        .gap(4.0)
        .child(text(heading.to_owned()).font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    row()
        .w_full()
        .gap(16.0)
        .padding(10.0)
        .pl(if sub { 34.0 } else { 10.0 })
        .center_y()
        .child(words)
        .child(controls::switch(l, on, heading, false).on_press(press))
}

/// A setup line to paste, with its Copy.
fn setup_block(l: &crate::look::controls::Look, said: &str, line: &str) -> El {
    let body = line.to_owned();
    col()
        .w_full()
        .gap(6.0)
        .child(text(said.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap())
        .child(
            row()
                .w_full()
                .gap(8.0)
                .padding(10.0)
                .radius(l.t.px("radius-control"))
                .fill(l.c("bg-control"))
                .child(col().grow(1.0).child(text(line.to_owned()).font(l.ty.mono).color(l.c("text-primary")).wrap()))
                .child(
                    text("Copy")
                        .font(l.ty.control)
                        .color(l.c("text-secondary"))
                        .py(4.0)
                        .px(8.0)
                        .radius(l.t.px("radius-control"))
                        .hover(l.c("hover-surface"))
                        .role("button")
                        .label(format!("Copy: {said}"))
                        .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, body.clone())),
                ),
        )
}

fn toggle_door(s: &mut State) {
    let on = s.settings_panel.door.as_ref().is_some_and(|d| d.settings.enabled);
    if let Some(state) = crate::door_now(locust_core::engine_api::DoorChange::Enabled(!on)) {
        s.settings_panel.door = Some(state);
    }
}

fn toggle_own_mode(s: &mut State) {
    let on = s.settings_panel.door.as_ref().is_some_and(|d| d.settings.own_mode);
    if let Some(state) = crate::door_now(locust_core::engine_api::DoorChange::OwnMode(!on)) {
        s.settings_panel.door = Some(state);
    }
}

fn toggle_conversations(s: &mut State) {
    let on = s.settings_panel.door.as_ref().is_some_and(|d| d.settings.conversations);
    if let Some(state) = crate::door_now(locust_core::engine_api::DoorChange::Conversations(!on)) {
        s.settings_panel.door = Some(state);
    }
}

/// LocustMcpSettings.tsx: the door's switch, and once it is on, each teammate's own mode, the person's
/// conversations, and the lines that add it to Claude Code and Codex.
fn door_section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let Some(door) = &s.settings_panel.door else {
        return deferred(DOOR_HEADING, p);
    };
    let set = &door.settings;
    let mut section = col().w_full().gap(2.0);
    section = section.child(setting_line(
        &l,
        DOOR_HEADING,
        &if set.enabled {
            format!(
                "On. Other apps on this computer can ask teammates and read replies. {}, with no automatic teammate handoffs. Locust must be running; your AI account usage and monthly limits still apply.",
                if set.own_mode { "New turns use each teammate's own mode" } else { "New turns are Ask mode (read only)" }
            )
        } else {
            "Off. No local server is listening.".to_owned()
        },
        set.enabled,
        false,
        toggle_door,
    ));
    if set.enabled {
        section = section
            .child(setting_line(
                &l,
                "Use each teammate's own mode",
                if set.own_mode {
                    "On. A turn another app starts runs in the mode the teammate is set to here, or the mode and folder the app asks for, so it can edit files. A folder must be one Locust knows or a worktree of one. Approval cards appear in this window and only you can answer them. Auto runs only if Auto is on in Settings."
                } else {
                    "Off. Every turn another app starts is Ask mode (read only), whatever mode the teammate is set to."
                },
                set.own_mode,
                true,
                toggle_own_mode,
            ))
            .child(setting_line(
                &l,
                "Let apps see your conversations",
                if set.conversations {
                    "On. An app can list, read, continue and stop your conversations, and carry one whose plan ran out on to another AI agent, by the same rules as its own: no higher mode, and cards answered only here."
                } else {
                    "Off. An app sees only the conversations it started. A turn it started that runs out can still be carried on to another AI agent."
                },
                set.conversations,
                true,
                toggle_conversations,
            ));
        let mut by_hand = col()
            .w_full()
            .gap(10.0)
            .padding(10.0)
            .child(text("Add Locust to Claude Code or Codex").font(l.ty.control).color(l.c("text-primary")));
        if let Some(line) = &door.claude_command {
            by_hand = by_hand.child(setup_block(&l, "Claude Code: run this in Terminal.", line));
        }
        if let Some(config) = &door.codex_config {
            by_hand = by_hand.child(setup_block(&l, "Codex: add this to ~/.codex/config.toml.", config));
        }
        by_hand = by_hand.child(
            text("Locust never changes another app's settings itself. Turning this off stops new requests, not turns already running.")
                .font(l.ty.body)
                .color(l.c("text-muted"))
                .wrap(),
        );
        section = section.child(by_hand);
    }
    if let Some(message) = &door.message {
        section = section.child(text(message.clone()).font(l.ty.body).color(l.c("amber-text")).wrap().padding(10.0));
    }
    section
}

fn set_plan(s: &mut State, plan: Option<locust_core::carry_on::PlanRunsOut>) {
    let Some(store) = s.data.clone() else { return };
    match locust_core::people::set_plan_runs_out(&store, plan.as_ref()) {
        Ok(()) => {
            s.settings_panel.plan_runs_out = plan;
            s.settings_panel.error.clear();
        }
        Err(e) => s.settings_panel.error = e,
    }
}

/// Screens.tsx "When an agent cannot go on": First one ready, or one agent; and Without asking once one is picked.
fn plan_runs_out_section(s: &State, p: &Palette) -> El {
    use locust_core::carry_on::{PlanRunsOut, display_name};
    let l = crate::look::controls::Look::new(p.dark);
    let plan = s.settings_panel.plan_runs_out.clone();
    let mut choices: Vec<String> = s.settings_panel.carry_choices.clone();
    if let Some(chosen) = &plan
        && !choices.contains(&chosen.runtime)
    {
        choices.push(chosen.runtime.clone());
    }
    let label = |runtime: &str| -> String {
        let named = match &plan {
            Some(c) if c.runtime == runtime && c.model.is_some() => format!("{} · {}", display_name(runtime), c.model.clone().unwrap_or_default()),
            _ => display_name(runtime).to_owned(),
        };
        if s.settings_panel.carry_choices.iter().any(|c| c == runtime) { named } else { format!("{named} (not ready)") }
    };
    let lede = match &plan {
        None => "A run whose plan runs out, or whose provider fails before it does anything, stops where it is. Its card offers to carry the work on with the first AI agent that is ready, told what was done. Pick one here to choose which.".to_owned(),
        Some(c) if c.automatic => format!("{} carries the work on by itself when another agent's plan runs out, or its provider fails before it does anything, told what was done.", label(&c.runtime)),
        Some(c) => format!("A run whose plan runs out, or whose provider fails before it does anything, stops where it is, and its card offers to carry the work on with {}.", label(&c.runtime)),
    };
    let mut picks = row().gap(6.0).flex_wrap();
    picks = picks.child(
        crate::look::controls::button(&l, "First one ready", if plan.is_none() { crate::look::controls::Tone::Active } else { crate::look::controls::Tone::Neutral }, false)
            .role("radio")
            .on_press(|s: &mut State| set_plan(s, None)),
    );
    for runtime in choices {
        let on = plan.as_ref().is_some_and(|c| c.runtime == runtime);
        let automatic = plan.as_ref().is_some_and(|c| c.automatic);
        let keep = plan.clone();
        let words = label(&runtime);
        picks = picks.child(
            crate::look::controls::button(&l, &words, if on { crate::look::controls::Tone::Active } else { crate::look::controls::Tone::Neutral }, false)
                .role("radio")
                .on_press(move |s: &mut State| {
                    let next = if on { keep.clone() } else { Some(PlanRunsOut { runtime: runtime.clone(), model: None, automatic }) };
                    set_plan(s, next);
                }),
        );
    }
    let without = match &plan {
        None => "Pick an agent above first: Locust never moves your work on its own to an agent you did not choose.",
        Some(c) if c.automatic => "On. Only for messages you send yourself, never a comparison, a routine or a reply between teammates.",
        Some(_) => "Off. The card waits for your press.",
    };
    let words = col()
        .grow(1.0)
        .gap(4.0)
        .child(text("Without asking").font(l.ty.control).color(l.c("text-primary")))
        .child(text(without).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let automatic = plan.as_ref().is_some_and(|c| c.automatic);
    let toggle = crate::look::controls::switch(&l, automatic, "Carry on without asking", plan.is_none()).on_press(|s: &mut State| {
        if let Some(mut c) = s.settings_panel.plan_runs_out.clone() {
            c.automatic = !c.automatic;
            set_plan(s, Some(c));
        }
    });
    col()
        .w_full()
        .gap(10.0)
        .padding(10.0)
        .child(text("When an agent cannot go on").font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede).font(l.ty.body).color(l.c("text-secondary")).wrap())
        .child(picks)
        .child(row().w_full().gap(16.0).pl(24.0).center_y().child(words).child(toggle))
}

fn deferred(heading: &str, p: &Palette) -> El {
    col()
        .w_full()
        .gap(4.0)
        .padding(10.0)
        .child(text(heading.to_owned()).font(type_::TITLE).color(p.text))
        .child(
            text(RUNS_IN_LOCUST.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
}

fn page_body(s: &State, p: &Palette, page: SettingsPageId) -> El {
    let query = s.settings_panel.search.text.trim().to_owned();
    let meta = settings_pages::page_by_id(page);
    let headings: Vec<&str> = if query.is_empty() {
        meta.headings.to_vec()
    } else {
        settings_pages::matched_headings(meta, &query)
    };
    let mut body = col().w_full().gap(8.0).padding(12.0);
    body = body.child(text(meta.label.to_owned()).font(type_::TITLE).color(p.text));

    match page {
        SettingsPageId::Appearance => {
            // Theme is drawn on Appearance (Electron ThemeSetting), even though
            // it is not in the search-index headings list.
            if query.is_empty()
                || settings_pages::matches("theme", &query)
                || settings_pages::matches("dark", &query)
                || settings_pages::matches("light", &query)
            {
                let theme = s.settings_panel.theme;
                body = body.child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .padding(10.0)
                        .child(
                            text(THEME_HEADING.to_owned())
                                .font(type_::TITLE)
                                .color(p.text),
                        )
                        .child(
                            text(workspace_settings::theme_lede(
                                theme,
                                Some(p.bg == Palette::new(true).bg),
                            ))
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                        )
                        .child(
                            row()
                                .gap(6.0)
                                .child(
                                    active_button(
                                        ThemeChoice::Dark.label(),
                                        p,
                                        theme == ThemeChoice::Dark,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Dark)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::Light.label(),
                                        p,
                                        theme == ThemeChoice::Light,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Light)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::System.label(),
                                        p,
                                        theme == ThemeChoice::System,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::System)),
                                ),
                        ),
                );
            }
            for heading in headings {
                if heading == "Sidebar" {
                    body = body.child(sidebar_line(s, p));
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::App => {
            for heading in headings {
                if heading == DOOR_HEADING {
                    body = body.child(door_section(s, p));
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Privacy => {
            for heading in headings {
                match heading {
                    "Privacy & local data" => {
                        body = body.child(crate::backup_section::storage_and_retention_section(s, p));
                    }
                    "Back up and restore" => {
                        body = body.child(crate::backup_section::backup_section(s, p));
                    }
                    _ => {
                        body = body.child(deferred(heading, p));
                    }
                }
            }
        }
        SettingsPageId::Runtimes => {
            for heading in headings {
                if heading == "When an agent cannot go on" {
                    body = body.child(plan_runs_out_section(s, p));
                } else if heading == crate::agents_section::HEADING {
                    body = body.child(crate::agents_section::section(s, p));
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Teammates => {
            for heading in headings {
                match heading {
                    "Saved approvals" => {
                        body = body
                            .child(
                                text(
                                    crate::approval_rule_store::SAVED_APPROVALS_HEADING.to_owned(),
                                )
                                .font(type_::TITLE)
                                .color(p.text)
                                .padding(10.0),
                            )
                            .child(
                                text(crate::approval_rule_store::SAVED_APPROVALS_LEDE.to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap()
                                    .padding(10.0),
                            )
                            .child(button("Open saved approvals", p).on_press(|s: &mut State| {
                                s.settings_panel.open = false;
                                crate::people_panel::open(s);
                                s.people_panel.tab = 2;
                                s.people_panel.memory_tab = false;
                            }));
                    }
                    _ => body = body.child(deferred(heading, p)),
                }
            }
        }
        SettingsPageId::Memory => {
            for heading in headings {
                if heading == "What your team remembers" {
                    let mode = s.settings_panel.settings.memory_mode.as_str();
                    body = body.child(
                        col()
                            .w_full()
                            .gap(6.0)
                            .padding(10.0)
                            .child(text(heading.to_owned()).font(type_::TITLE).color(p.text))
                            .child(
                                text(MEMORY_WHEN.to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary),
                            )
                            .child(
                                row()
                                    .gap(6.0)
                                    .child(active_button(MEMORY_KEEP, p, mode == "auto").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "auto".into();
                                            persist(s);
                                        },
                                    ))
                                    .child(active_button(MEMORY_ASK, p, mode == "ask").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "ask".into();
                                            persist(s);
                                        },
                                    ))
                                    .child(active_button(MEMORY_OFF, p, mode == "off").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "off".into();
                                            persist(s);
                                        },
                                    )),
                            ),
                    );
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Workspace => {
            let folder = active_project_folder(s);
            let ws_id = folder
                .as_deref()
                .map(|f| workspace_settings::workspace_id_for(std::path::Path::new(f)));
            let saved_cmd = ws_id
                .as_deref()
                .and_then(|ws| s.settings_panel.settings.check_commands.get(ws))
                .cloned()
                .unwrap_or_default();
            let draft_trimmed = s.settings_panel.check_command.text.trim().to_owned();
            let changed = draft_trimmed != saved_cmd;
            let has_check = !saved_cmd.is_empty();
            let fix_tries = ws_id
                .as_deref()
                .and_then(|ws| s.settings_panel.settings.check_fixes.get(ws).copied());

            body = body
                .child(
                    text(PROJECT_FOLDER_LEDE.to_owned())
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .wrap()
                        .padding(10.0),
                )
                .child(
                    col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .child(text(DATA_FOLDER_HEADING.to_owned()).font(type_::TITLE).color(p.text))
                        .child(
                            text(if s.settings_panel.data_folder.is_empty() {
                                "No folder chosen".into()
                            } else {
                                s.settings_panel.data_folder.clone()
                            })
                            .font(type_::LABEL)
                            .color(p.text)
                            .wrap(),
                        )
                        .child(
                            text(if s.settings_panel.data_folder.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "id {}",
                                    workspace_settings::workspace_id_for(std::path::Path::new(
                                        &s.settings_panel.data_folder
                                    ))
                                )
                            })
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                        )
                        .child(
                            text("Native keeps its own settings copy here, seeded once from Electron. Electron's folder is never written.")
                                .font(type_::LABEL)
                                .color(p.muted)
                                .wrap(),
                        ),
                );
            if let Some(ws) = ws_id {
                let save_label = if draft_trimmed.is_empty() && has_check {
                    "Turn off"
                } else {
                    "Save"
                };
                let ws_save = ws.clone();
                let mut check_col = col()
                    .w_full()
                    .gap(8.0)
                    .padding(10.0)
                    .child(text("CHECK AFTER EDITS").font(type_::TITLE).color(p.text))
                    .child(
                        text("A command Locust runs in a teammate's folder after a turn that changed files, such as npm test. Only failures that were not there at the last check count. Set here only: a project never chooses its own.")
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                    )
                    .child(
                        row()
                            .w_full()
                            .gap(8.0)
                            .center_y()
                            .child(
                                text_input(&s.settings_panel.check_command, type_::BODY)
                                    .key("settings-check-command")
                                    .grow(1.0)
                                    .padding(8.0)
                                    .radius(6.0)
                                    .fill(p.field)
                                    .color(p.text)
                                    .placeholder("npm test", p.muted)
                                    .on_edit(|s: &mut State| &mut s.settings_panel.check_command),
                            )
                            .child(
                                active_button(save_label, p, changed)
                                    .key("settings-check-save")
                                    .on_press(move |s: &mut State| {
                                        let trimmed = s.settings_panel.check_command.text.trim().to_owned();
                                        if trimmed.is_empty() {
                                            s.settings_panel.settings.check_commands.remove(&ws_save);
                                            s.settings_panel.check_command.replace_text(String::new());
                                            persist(s);
                                        } else if let Some(cmd) = locust_core::edit_check::parsed_check_command(&trimmed) {
                                            s.settings_panel.settings.check_commands.insert(ws_save.clone(), cmd.clone());
                                            s.settings_panel.check_command.replace_text(cmd);
                                            persist(s);
                                        }
                                    }),
                            ),
                    );
                let ws_off = ws.clone();
                let ws_on = ws.clone();
                let mut fail_row = row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(text("When it fails").font(type_::LABEL).color(p.secondary))
                    .child(
                        active_button("Show me", p, fix_tries.is_none())
                            .key("settings-check-fix-off")
                            .on_press(move |s: &mut State| {
                                s.settings_panel.settings.check_fixes.remove(&ws_off);
                                persist(s);
                            }),
                    )
                    .child(
                        active_button("Send it back to fix", p, fix_tries.is_some())
                            .key("settings-check-fix-on")
                            .on_press(move |s: &mut State| {
                                let has = s.settings_panel.settings.check_commands.contains_key(&ws_on);
                                let current = s.settings_panel.settings.check_fixes.get(&ws_on).copied();
                                if has || current.is_some() {
                                    let next = current.unwrap_or(locust_core::edit_check::DEFAULT_CHECK_FIXES);
                                    s.settings_panel.settings.check_fixes.insert(ws_on.clone(), next);
                                    persist(s);
                                }
                            }),
                    );
                if let Some(tries) = fix_tries {
                    fail_row = fail_row.child(text("at most").font(type_::LABEL).color(p.secondary));
                    for most in 1..=locust_core::edit_check::MAX_CHECK_FIXES {
                        let ws_m = ws.clone();
                        fail_row = fail_row.child(
                            active_button(most.to_string(), p, tries == most)
                                .key(format!("settings-check-fix-{most}"))
                                .on_press(move |s: &mut State| {
                                    s.settings_panel.settings.check_fixes.insert(ws_m.clone(), most);
                                    persist(s);
                                }),
                        );
                    }
                    fail_row = fail_row.child(
                        text(if tries == 1 { "fix" } else { "fixes" })
                            .font(type_::LABEL)
                            .color(p.secondary),
                    );
                }
                check_col = check_col.child(fail_row);
                body = body.child(check_col);
            }
            body = body.child(crate::own_copies::section(s, p));
        }
        SettingsPageId::WhatsNew => {
            if s.home.changelog.is_empty() {
                body = body.child(
                    text(crate::changelog::READING.to_owned())
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap()
                        .padding(10.0),
                );
            } else {
                body = body.child(crate::home_panel::whats_new_settings(s, p));
            }
        }

        _ => {
            for heading in headings {
                body = body.child(deferred(heading, p));
            }
        }
    }

    body = body.child(
        text(HOW_IT_WORKS.to_owned())
            .font(type_::LABEL)
            .color(p.muted)
            .padding(10.0),
    );
    body
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let query = s.settings_panel.search.text.trim().to_owned();
    let mut nav = col().w(200.0).gap(2.0).padding(8.0);
    let mut last_group: Option<SettingsGroup> = None;
    for page in SETTINGS_PAGES {
        if !query.is_empty() && !settings_pages::page_matches(page, &query) {
            continue;
        }
        if last_group != Some(page.group) {
            last_group = Some(page.group);
            nav = nav.child(
                text(page.group.label().to_owned())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .px(8.0)
                    .py(6.0),
            );
        }
        let id = page.id;
        let selected = s.settings_panel.page == Some(id);
        nav = nav.child(active_button(page.label, p, selected).w_full().on_press(
            move |s: &mut State| {
                s.settings_panel.page = Some(id);
            },
        ));
    }
    if last_group.is_none() {
        nav = nav.child(
            text(SEARCH_EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap()
                .padding(8.0),
        );
    }

    let page = s
        .settings_panel
        .page
        .filter(|id| {
            query.is_empty()
                || settings_pages::page_matches(settings_pages::page_by_id(*id), &query)
        })
        .or_else(|| {
            SETTINGS_PAGES
                .iter()
                .find(|page| query.is_empty() || settings_pages::page_matches(page, &query))
                .map(|p| p.id)
        });

    let mut content = col().grow(1.0).min_h(400.0).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if let Some(page) = page {
        // as_str keeps the page id vocabulary tied to SETTINGS_PAGES.
        let _page_id = page.as_str();
        content = content.child(page_body(s, p, page));
    } else {
        content = content.child(
            text(SEARCH_EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap()
                .padding(16.0),
        );
    }

    let mut shell = col()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(
            row()
                .w_full()
                .padding(12.0)
                .gap(8.0)
                .center_y()
                .child(
                    text(SETTINGS_TITLE.to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| s.settings_panel.open = false)),
        )
        .child(
            col().w_full().padding(12.0).child(
                text_input(&s.settings_panel.search, type_::BODY)
                    .key("settings-search")
                    .w_full()
                    .padding(10.0)
                    .radius(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .placeholder(SEARCH_PLACEHOLDER, p.muted)
                    .on_edit(|s: &mut State| &mut s.settings_panel.search),
            ),
        );
    if !s.settings_panel.error.is_empty() {
        shell = shell.child(
            text(s.settings_panel.error.clone())
                .font(type_::LABEL)
                .color(p.red)
                .wrap()
                .padding(12.0),
        );
    }
    shell.child(
        row()
            .w_full()
            .grow(1.0)
            .child(nav.fill(p.sidebar).h_full())
            .child(content.fill(p.bg).h_full()),
    )
}

pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.settings_panel = Panel {
        open: true,
        page: Some(SettingsPageId::Appearance),
        theme: ThemeChoice::Dark,
        data_folder: "/Users/locust/Library/Application Support/Locust-native".into(),
        settings: workspace_settings::WorkspaceSettings::default(),
        ..Panel::default()
    };
}

/// General with the door on: its switches and the lines to add it to another app.
pub(super) fn door_scene(s: &mut State) {
    scene(s);
    s.settings_panel.page = Some(SettingsPageId::App);
    let (claude, codex) = locust_core::door::setup(
        "/Applications/Locust Native.app/Contents/MacOS/locust-native",
        "/Users/locust/Library/Application Support/@teammate/desktop-native/locust-mcp-connection.json",
    );
    s.settings_panel.door = Some(locust_core::door::State {
        settings: locust_core::door::Settings { enabled: true, own_mode: true, conversations: false },
        claude_command: Some(claude),
        codex_config: Some(codex),
        message: None,
    });
}

/// AI agents with Claude Code picked to carry work on without asking.
pub(super) fn carry_settings_scene(s: &mut State) {
    scene(s);
    s.settings_panel.page = Some(SettingsPageId::Runtimes);
    s.settings_panel.carry_choices = vec!["claude".into(), "codex".into(), "cursor".into(), "opencode".into()];
    s.settings_panel.plan_runs_out = Some(locust_core::carry_on::PlanRunsOut { runtime: "claude".into(), model: None, automatic: true });
}

pub(super) fn carry_settings_light_scene(s: &mut State) {
    carry_settings_scene(s);
    s.appearance = Some(false);
    s.settings_panel.theme = ThemeChoice::Light;
}

pub(super) fn door_light_scene(s: &mut State) {
    door_scene(s);
    s.appearance = Some(false);
    s.settings_panel.theme = ThemeChoice::Light;
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
    s.settings_panel.theme = ThemeChoice::Light;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_settings_action_keeps_electron_fingerprint() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::SystemTime,
        };
        fn fingerprint(at: &Path) -> Vec<(PathBuf, u64, SystemTime, Vec<u8>)> {
            let mut entries: Vec<_> = fs::read_dir(at).unwrap().map(Result::unwrap).collect();
            entries.sort_by_key(|e| e.path());
            let mut out = Vec::new();
            for e in entries {
                let m = e.metadata().unwrap();
                out.push((
                    e.path(),
                    m.len(),
                    m.modified().unwrap(),
                    if m.is_file() {
                        fs::read(e.path()).unwrap()
                    } else {
                        Vec::new()
                    },
                ));
                if m.is_dir() {
                    out.extend(fingerprint(&e.path()));
                }
            }
            out
        }
        let root = std::env::temp_dir().join(format!("settings-review-{}", std::process::id()));
        fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
        let electron = root.join("electron");
        fs::write(
            electron.join("teammates.json"),
            r#"{"schemaVersion":1,"teammates":[],"settings":{"memoryMode":"auto"}}"#,
        )
        .unwrap();
        fs::write(electron.join("appearance.json"), r#"{"theme":"light"}"#).unwrap();
        fs::write(electron.join("mission-ledger/sentinel"), "untouched").unwrap();
        let before = fingerprint(&electron);
        let mut s = State {
            data: Some(store::Store::new(root.join("native"), electron.clone()).unwrap()),
            ..State::default()
        };
        open(&mut s);
        assert_eq!(s.appearance, Some(false));
        for theme in [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::System] {
            set_theme(&mut s, theme);
            assert_eq!(
                workspace_settings::load_theme(s.data.as_ref().unwrap()),
                theme
            );
        }
        for mode in ["ask", "off", "auto"] {
            s.settings_panel.settings.memory_mode = mode.into();
            persist(&mut s);
            assert_eq!(
                workspace_settings::load(s.data.as_ref().unwrap())
                    .unwrap()
                    .memory_mode,
                mode
            );
        }
        for page in SETTINGS_PAGES {
            s.settings_panel.page = Some(page.id);
            let _ = view(&s, &Palette::new(true));
        }
        crate::people_panel::open(&mut s);
        let _ = crate::usage_ledger::usage_turns(s.data.as_ref().unwrap()).unwrap();
        assert!(s.settings_panel.error.is_empty());
        assert_eq!(fingerprint(&electron), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scene_opens_appearance() {
        let mut s = State::default();
        scene(&mut s);
        assert!(s.settings_panel.open);
        assert_eq!(s.settings_panel.page, Some(SettingsPageId::Appearance));
    }
}
