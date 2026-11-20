//! Categorized Settings screen (Area 11).

use super::*;
use crate::settings_pages::{
    self, RUNS_IN_LOCUST, SEARCH_EMPTY, SEARCH_PLACEHOLDER, SETTINGS_PAGES,
    SETTINGS_TITLE, SettingsGroup, SettingsPageId,
};
use crate::workspace_settings::{
    self, AppearancePrefs, DATA_FOLDER_HEADING, GlassGround, LookChoice, MEMORY_ASK, MEMORY_KEEP,
    MEMORY_OFF, MEMORY_WHEN, PROJECT_FOLDER_LEDE, THEME_HEADING, ThemeChoice,
};

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    /// Which "How it works" folds are open, by key.
    pub folds: crate::how_it_works::Folds,
    /// "What a face says": which face the legend shows, and since when.
    pub face_legend: locust_core::face_legend::LegendClock,
    pub page: Option<SettingsPageId>,
    pub search: TextEdit,
    pub settings: workspace_settings::WorkspaceSettings,
    pub theme: ThemeChoice,
    pub look: LookChoice,
    pub glass_ground: GlassGround,
    pub liquid_offered: bool,
    pub keep_running: bool,
    pub stay_awake: bool,
    pub login_item: locust_core::login_item::LoginItemState,
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
    /// "Your phone": the phone host's switch and link (None when this window has no phone host).
    pub phone: Option<locust_core::phone_host::PhoneSettings>,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

/// A radio's name to a screen reader, with ", chosen" on the one picked (Electron's aria-checked).
fn chosen_label(label: &str, chosen: bool) -> String {
    format!("{label}{}", if chosen { ", chosen" } else { "" })
}

/// The line under "When it fails" (CheckAfterEditsRow): what happens to a failure, in the folder's own choice.
pub(super) fn check_fix_note(has_check: bool, fix_tries: Option<u32>) -> String {
    match fix_tries {
        None if has_check => "What is new in the failure is shown under the turn, with a button to send it to the teammate.".into(),
        None => "Save a check first. Then a failure can be shown to you, or sent back to the teammate to fix.".into(),
        Some(tries) => format!(
            "After a turn you sent, what is new in the failure goes back to the same teammate, on the same model, until the check passes: {tries} time{} at most, and it stops sooner when a fix changes nothing. Each fix is one more turn.",
            if tries == 1 { "" } else { "s" }
        ),
    }
}

fn active_button(label: impl Into<String>, p: &Palette, on: bool) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(if on { p.text } else { p.secondary })
        .padding(7.0)
        .fill(if on { p.selected } else { p.field })
        .radius(6.0)
}

/// Reads whether Locust starts when the person logs in (the Startup switch), for Settings and the routines screen.
pub(super) fn read_login_item(s: &mut State) {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let item = locust_core::login_item::LoginItem::new(
        crate::os_hosts::login_item(),
        crate::os_hosts::packaged(),
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
        platform,
    );
    s.settings_panel.login_item = item.read();
}

pub(super) fn open(s: &mut State) {
    crate::room_screen::close(s);
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
            let prefs = workspace_settings::load_appearance(store);
            s.settings_panel.theme = prefs.theme;
            s.settings_panel.look = prefs.look;
            s.settings_panel.glass_ground = prefs.glass_ground;
            s.appearance = prefs.theme.to_appearance();
            // Round-trip keeps State.appearance and the stored theme aligned.
            s.settings_panel.theme = ThemeChoice::from_appearance(s.appearance);
            s.settings_panel.keep_running = locust_core::keep_awake::read_keep_running(
                &store.root().join(locust_core::keep_awake::BACKGROUND_FILE),
            );
            s.settings_panel.stay_awake = locust_core::keep_awake::read_stay_awake(
                &store.root().join(locust_core::keep_awake::AWAKE_FILE),
            );
            read_login_item(s);
            crate::phone_section::read(s);
        }
    }
    crate::agents_section::request(s);
    crate::github_section::request(s);
    crate::chat_apps_section::reload(s);
    crate::claude_app_card::request(s);
    crate::agent_accounts_card::reload(s);
    crate::own_models_card::reload(s);
    crate::connectors_section::request(s, false);
    crate::backup_section::reload(s);
    let folder = active_project_folder(s);
    crate::own_copies::reload(s, folder);
    crate::scout_cards::request_refresh_state(s);
}

/// ⌘O: Normal or Verbose steps, remembered (App.tsx `toggleTranscriptView`). A window whose store refuses still
/// switches for this moment.
pub(super) fn toggle_transcript_view(s: &mut State) {
    let settings = &mut s.settings_panel.settings;
    settings.transcript_view = if settings.verbose() { "normal" } else { "verbose" }.into();
    persist(s);
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
    save_appearance_panel(s);
}

fn set_look(s: &mut State, look: LookChoice) {
    s.settings_panel.look = look;
    save_appearance_panel(s);
}

fn set_glass_ground(s: &mut State, ground: GlassGround) {
    s.settings_panel.glass_ground = ground;
    save_appearance_panel(s);
}

fn save_appearance_panel(s: &mut State) {
    let prefs = AppearancePrefs {
        theme: s.settings_panel.theme,
        look: s.settings_panel.look,
        glass_ground: s.settings_panel.glass_ground,
    };
    if let Some(store) = &s.data
        && let Err(e) = workspace_settings::save_appearance(store, prefs)
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
    let l = crate::look::controls::Look::new(p.dark);
    let words = col()
        .gap(6.0)
        .child(text("Sidebar").role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(text(crate::rail::layout_lede(pref, s.layout_compact.get())).font(type_::LABEL).color(p.secondary).wrap());
    let choices = row().gap(6.0).child(choice("auto", "Auto")).child(choice("wide", "Full")).child(choice("compact", "Rail"));
    line(&l, "settings-line-sidebar", 10.0, words, choices)
}

/// The heading Electron's General page gives the door.
const DOOR_HEADING: &str = "Let your other AI apps use Locust";

/// A setting that is one control, drawn as one line (`.lc-settings__section--line` > `.lc-settingline`): a hairline
/// above it, no card and no fill, the name and description on the left (`words`) and the control on the right. The
/// line is `key`; its hairline is `<key>-hairline` and the row under it `<key>-row`.
fn line(l: &crate::look::controls::Look, key: &str, indent: f32, words: El, control: El) -> El {
    col()
        .key(key.to_owned())
        .w_full()
        .child(el().key(format!("{key}-hairline")).w_full().h(1.0).fill(l.c("border-hairline")))
        .child(
            row()
                .key(format!("{key}-row"))
                .w_full()
                .gap(24.0)
                .py(14.0)
                .px(10.0)
                .pl(indent)
                .center_y()
                .child(words.grow(1.0).shrink().min_w(0.0))
                .child(control),
        )
}

/// One switch with its words (`.lc-settingline`), indented when it belongs to the line above (`--sub`).
fn setting_line(l: &crate::look::controls::Look, heading: &str, lede: &str, on: bool, sub: bool, press: fn(&mut State)) -> El {
    use crate::look::controls;
    let words = col()
        .gap(4.0)
        .child(text(heading.to_owned()).font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let key = format!("settings-line-{}", heading.to_lowercase().replace(' ', "-"));
    line(l, &key, if sub { 34.0 } else { 10.0 }, words, controls::switch(l, on, heading, false).on_press(press))
}

/// A setup line to paste, with its Copy.
/// One line to paste, with a Copy named for what it copies ("Copy the OpenClaw command", Electron's CopyButton).
fn setup_block(l: &crate::look::controls::Look, said: &str, copies: &str, line: &str) -> El {
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
                        .label(format!("Copy {copies}"))
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

/// Screens.tsx "Auto mode": one setting line, the switch named for what it controls. Off unless picked; the
/// organization can hold it off (`mode_allowed(.., "auto")`), and then the switch is drawn but does nothing.
pub(super) const AUTO_ON: &str = "On. A run started in Auto goes ahead without asking and is not kept to the project folder.";
pub(super) const AUTO_OFF: &str = "Off. Teammates may change files inside the project folder and nowhere else.";

fn auto_mode_section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let managed = !locust_core::org_policy::mode_allowed(Some(&locust_core::org_policy::current()), "auto");
    let on = s.settings_panel.settings.auto_mode;
    let mut words = col()
        .gap(4.0)
        .child(text("Auto mode").role("heading").level(2).font(l.ty.control).color(l.c("text-primary")));
    if managed {
        words = words.child(text("Set by your organization.").font(l.ty.body).color(l.c("text-muted")));
    }
    words = words.child(text(if on { AUTO_ON } else { AUTO_OFF }).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let mut knob = crate::look::controls::switch(&l, on, "Auto mode", managed).key("settings-auto-mode");
    if !managed {
        knob = knob.on_press(|s: &mut State| {
            s.settings_panel.settings.auto_mode = !s.settings_panel.settings.auto_mode;
            persist(s);
        });
    }
    col()
        .w_full()
        .child(line(&l, "settings-line-auto-mode", 10.0, words, knob))
        .child(more(s, p, "auto-mode"))
}

/// The section's "How it works" (`how_it_works::FOLDS` under `key`), kept open or closed in `settings_panel.folds`.
fn more(s: &State, p: &Palette, key: &'static str) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    crate::how_it_works::fold(&l, &s.settings_panel.folds, key, crate::how_it_works::words(key), move |s: &mut State| {
        s.settings_panel.folds.toggle(key)
    })
}


/// LocustMcpSettings.tsx: the door's switch, and once it is on, each teammate's own mode, the person's
/// conversations, and the lines that add it to Claude Code and Codex.
fn door_section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let managed = locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "otherApps").is_some();
    let Some(door) = &s.settings_panel.door else {
        if !managed {
            return deferred(DOOR_HEADING, p);
        }
        let words = col()
            .grow(1.0)
            .gap(4.0)
            .child(text(DOOR_HEADING.to_owned()).role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
            .child(text("Set by your organization.").font(l.ty.body).color(l.c("text-muted")));
        return row()
            .w_full()
            .gap(16.0)
            .padding(10.0)
            .center_y()
            .child(words)
            .child(crate::look::controls::switch(&l, false, DOOR_HEADING, true).key("door-switch"));
    };
    let set = &door.settings;
    let mut section = col().w_full().gap(2.0);
    if managed {
        let words = col()
            .grow(1.0)
            .gap(4.0)
            .child(text(DOOR_HEADING.to_owned()).role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
            .child(text("Off.").font(l.ty.body).color(l.c("text-secondary")))
            .child(text("Set by your organization.").font(l.ty.body).color(l.c("text-muted")));
        return section.child(
            row()
                .w_full()
                .gap(16.0)
                .padding(10.0)
                .center_y()
                .child(words)
                .child(crate::look::controls::switch(&l, false, DOOR_HEADING, true).key("door-switch")),
        );
    }
    section = section.child(setting_line(
        &l,
        DOOR_HEADING,
        &if set.enabled {
            format!(
                "On. Other apps on this computer can ask teammates, run your routines and read replies. {}, with no automatic teammate handoffs. Locust must be running; your AI account usage and monthly limits still apply.",
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
    }
    if let Some(card) = crate::claude_app_card::section(s, p) {
        section = section.child(card);
    }
    if set.enabled {
        let mut by_hand = col()
            .w_full()
            .gap(10.0)
            .padding(10.0)
            .child(text("Add Locust to Claude Code, Codex or OpenClaw by hand").font(l.ty.control).color(l.c("text-primary")));
        if let Some(line) = &door.claude_command {
            by_hand = by_hand.child(setup_block(&l, "Claude Code \u{2014} run this in Terminal:", "the Claude Code command", line));
        }
        if let Some(config) = &door.codex_config {
            by_hand = by_hand.child(setup_block(&l, "Codex \u{2014} add this to ~/.codex/config.toml:", "the Codex settings", config));
        }
        if let Some(line) = &door.openclaw_command {
            by_hand = by_hand.child(setup_block(
                &l,
                "OpenClaw \u{2014} run this in Terminal. It checks the connection first, and its tool policy decides which of its chats can use it:",
                "the OpenClaw command",
                line,
            ));
        }
        by_hand = by_hand.child(
            text("Locust changes another app\u{2019}s settings only when you press Add to the Claude app; it keeps a copy of the file first, and Remove takes its entry out again. Turning this off stops new requests, not turns already running.")
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
        .child(text("When an agent cannot go on").role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede).font(l.ty.body).color(l.c("text-secondary")).wrap())
        .child(picks)
        .child(row().w_full().gap(16.0).pl(24.0).center_y().child(words).child(toggle))
}

fn org_policy_section(p: &Palette) -> Option<El> {
    let policy = locust_core::org_policy::current();
    if !policy.present && policy.error.is_none() {
        return None;
    }
    if let Some(err) = &policy.error {
        return Some(
            col()
                .key("org-policy-alert")
                .role("alert")
                .w_full()
                .gap(4.0)
                .padding(10.0)
                .radius(6.0)
                .fill(p.raised)
                .border(1.0, p.red)
                .child(text("Managed by your organization").role("heading").level(2).font(type_::LABEL).color(p.red))
                .child(text(err.clone()).font(type_::BODY).color(p.text).wrap()),
        );
    }
    let labels = [
        ("allowedAgents", "Allowed AI agents"),
        ("allowedModels", "Allowed models"),
        ("highestMode", "Highest permission mode"),
        ("monthlyLimitUsd", "Maximum monthly limit"),
        ("phone", "Phone pairing"),
        ("cloud", "Cloud computers and tasks"),
        ("otherApps", "Access from other AI apps"),
        ("chatApps", "Chat apps"),
        ("terminal", "Terminal"),
        ("browser", "Browser"),
        ("accounts", "Accounts"),
    ];
    let mut box_el = col()
        .key("org-policy-section")
        .w_full()
        .gap(4.0)
        .padding(10.0)
        .child(text("Managed by your organization").role("heading").level(2).font(type_::LABEL).color(p.text))
        .child(text(policy.path.clone()).font(type_::LABEL).color(p.muted).wrap());
    for (key, title) in labels {
        if let Some(val) = policy.policy.get(key) {
            let words = locust_core::org_policy::setting_words(key, val);
            box_el = box_el.child(text(format!("{title}: {words}")).font(type_::BODY).color(p.secondary).wrap());
        }
    }
    Some(box_el)
}

fn deferred(heading: &str, p: &Palette) -> El {
    col()
        .w_full()
        .gap(4.0)
        .padding(10.0)
        .child(text(heading.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(
            text(RUNS_IN_LOCUST.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
}

/// A section's heading on a Settings page (Screens.tsx `<h2 className="lc-settings__heading">`): the words the search
/// index lists for the page, marked as a heading so the index can be held to what is drawn.
fn section_heading(words: &str, p: &Palette) -> El {
    text(words.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text).px(10.0).pt(10.0)
}

/// The page Settings shows: the one picked, while the search still finds it, else the first the search finds.
fn shown_page(s: &State) -> Option<SettingsPageId> {
    let query = s.settings_panel.search.text.trim().to_owned();
    s.settings_panel
        .page
        .filter(|id| query.is_empty() || settings_pages::page_matches(settings_pages::page_by_id(*id), &query))
        .or_else(|| {
            SETTINGS_PAGES
                .iter()
                .find(|page| query.is_empty() || settings_pages::page_matches(page, &query))
                .map(|p| p.id)
        })
}

/// In `poll`: the face legend's clock, while Appearance is on screen (FaceLegend.tsx 71-76). It rests as Home's cover
/// does (`useCoverActivity`): while the window is not focused, with reduced motion, or after 45 s untouched
/// (`cover_watch`). Off screen it starts again from Idle, as the component does when it mounts.
pub(super) fn tick_face_legend(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    use locust_core::face_legend::LegendClock;
    if !(s.settings_panel.open && shown_page(s) == Some(SettingsPageId::Appearance)) {
        s.settings_panel.face_legend = LegendClock::default();
        return false;
    }
    let paused = crate::cover_watch::away(s, now);
    let changed = s.settings_panel.face_legend.tick(now, paused);
    if let Some(due) = s.settings_panel.face_legend.due_in(now) {
        tx.wake_after(Duration::from_secs_f64(due.max(0.016)));
    }
    changed
}

/// "What a face says" (FaceLegend.tsx): one teammate, a teal droid with eyes at 40, going through every face in turn,
/// each with its dot and its name under it. A press shows the next.
fn face_legend(s: &State, p: &Palette, l: &crate::look::controls::Look) -> El {
    use crate::look::bot_stage::PresenceDot;
    use locust_core::face_legend::{legend_lede, legend_label, legend_name, legend_presence, LEGEND_HUE, LEGEND_SHAPE, LEGEND_SIZE};
    use locust_core::face_status::Presence;
    let step = s.settings_panel.face_legend.shown();
    let name = legend_name(step.activity);
    let presence = match legend_presence(step.activity) {
        Presence::Working => Some(PresenceDot::Working),
        Presence::Approval => Some(PresenceDot::Waiting),
        Presence::Blocked => Some(PresenceDot::Blocked),
        Presence::None => None,
    };
    let look = locust_core::native::FaceLook {
        hue: LEGEND_HUE.into(),
        parts: Some([0, 0, 0]),
        bot: Some((LEGEND_SHAPE.into(), "eyes".into())),
        bot_screen: None,
        pet: None,
    };
    let face = crate::look::faces::bot_face_with_look(
        s,
        "settings-face-legend",
        "settings-face-legend",
        &name,
        Some(&look),
        LEGEND_SIZE,
        crate::look::faces::BotFaceOpts {
            activity: step.activity,
            presence,
            motion: locust_core::bot_rig::BotMotionLevel::Subtle,
            ..Default::default()
        },
        l,
    );
    let legend = col()
        .key("settings-face-legend")
        .role("button")
        .label(legend_label(step.activity))
        .focusable(true)
        .focus_ring(2.0, 2.0, l.c("focus-ring"))
        .center()
        .gap(4.0)
        .padding(6.0)
        .radius(8.0)
        .hover(l.c("hover-surface"))
        .on_press(|s: &mut State| s.settings_panel.face_legend.press())
        .child(face)
        .child(text(name).key("settings-face-legend-name").font(type_::LABEL).color(p.secondary).a11y_hidden());
    let words = col()
        .gap(6.0)
        .child(text("What a face says".to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(text(legend_lede(s.settings_panel.settings.terminal_faces).to_owned()).font(type_::LABEL).color(p.secondary).wrap());
    line(l, "settings-face-legend-row", 10.0, words, legend)
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
    // A page whose name is also one of its section headings draws that heading itself (Electron's h2), so the name
    // is not said twice.
    if !meta.headings.contains(&meta.label) {
        body = body.child(text(meta.label.to_owned()).font(type_::TITLE).color(p.text));
    }

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
                                    .key("settings-theme-dark")
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Dark)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::Light.label(),
                                        p,
                                        theme == ThemeChoice::Light,
                                    )
                                    .key("settings-theme-light")
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Light)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::System.label(),
                                        p,
                                        theme == ThemeChoice::System,
                                    )
                                    .key("settings-theme-system")
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::System)),
                                ),
                        ),
                );
            }
            if s.settings_panel.liquid_offered
                && (query.is_empty()
                    || settings_pages::matches("look", &query)
                    || settings_pages::matches("glass", &query))
            {
                let look = s.settings_panel.look;
                let ground = s.settings_panel.glass_ground;
                let platform = match std::env::consts::OS {
                    "macos" => "darwin",
                    "windows" => "win32",
                    other => other,
                };
                let mut look_col = col()
                    .w_full()
                    .gap(6.0)
                    .padding(10.0)
                    .child(text("Look".to_owned()).font(type_::TITLE).color(p.text))
                    .child(
                        text(workspace_settings::look_lede(look, platform).to_owned())
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                    )
                    .child(
                        row()
                            .gap(6.0)
                            .child(
                                active_button(
                                    LookChoice::Standard.label(),
                                    p,
                                    look == LookChoice::Standard,
                                )
                                .key("settings-look-standard")
                                .on_press(|s: &mut State| set_look(s, LookChoice::Standard)),
                            )
                            .child(
                                active_button(
                                    LookChoice::Liquid.label(),
                                    p,
                                    look == LookChoice::Liquid,
                                )
                                .key("settings-look-liquid")
                                .on_press(|s: &mut State| set_look(s, LookChoice::Liquid)),
                            ),
                    );
                if look == LookChoice::Liquid {
                    look_col = look_col
                        .child(
                            text("Behind the glass".to_owned())
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .child(
                            text(workspace_settings::ground_lede(ground).to_owned())
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .wrap(),
                        )
                        .child(
                            row()
                                .gap(6.0)
                                .child(
                                    active_button(
                                        GlassGround::Desktop.label(),
                                        p,
                                        ground == GlassGround::Desktop,
                                    )
                                    .key("settings-ground-desktop")
                                    .on_press(|s: &mut State| {
                                        set_glass_ground(s, GlassGround::Desktop)
                                    }),
                                )
                                .child(
                                    active_button(
                                        GlassGround::Rainbow.label(),
                                        p,
                                        ground == GlassGround::Rainbow,
                                    )
                                    .key("settings-ground-rainbow")
                                    .on_press(|s: &mut State| {
                                        set_glass_ground(s, GlassGround::Rainbow)
                                    }),
                                ),
                        );
                }
                body = body.child(look_col);
            }
            let terminal_faces = s.settings_panel.settings.terminal_faces;
            let plush = s.settings_panel.settings.plush;
            let l = crate::look::controls::Look::new(p.bg == Palette::new(true).bg);
            for heading in headings {
                if heading == "Sidebar" {
                    body = body.child(sidebar_line(s, p));
                } else if heading == "Code font" {
                    let paper = s.settings_panel.settings.code_font == "paper";
                    body = body.child(line(
                        &l,
                        "settings-line-code-font",
                        10.0,
                        col()
                            .gap(6.0)
                            .child(text("Code font".to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
                            .child(
                                text("The face for code, paths and commands. Takes effect the next time Locust starts.".to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap(),
                            ),
                                row()
                                    .gap(6.0)
                                    .child(active_button("Geist Mono", p, !paper).key("settings-code-font-geist").on_press(|s: &mut State| {
                                        s.settings_panel.settings.code_font = "geist".into();
                                        persist(s);
                                    }))
                                    .child(active_button("Paper Mono", p, paper).key("settings-code-font-paper").on_press(|s: &mut State| {
                                        s.settings_panel.settings.code_font = "paper".into();
                                        persist(s);
                                    })),
                    ));
                } else if heading == "Terminal faces" {
                    body = body.child(line(
                        &l,
                        "settings-line-terminal-faces",
                        10.0,
                        col()
                            .gap(6.0)
                            .child(text("Terminal faces".to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
                            .child(
                                text("Draw CRT screen expressions on screen-ready bots and Codex Buddy. Turn off for classic eyes everywhere.".to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap(),
                            ),
                        crate::look::controls::switch(&l, terminal_faces, "Terminal faces", false)
                            .key("settings-terminal-faces")
                            .on_press(|s: &mut State| {
                                s.settings_panel.settings.terminal_faces = !s.settings_panel.settings.terminal_faces;
                                persist(s);
                            }),
                    ));
                } else if heading == "What a face says" {
                    body = body.child(face_legend(s, p, &l));
                } else if heading == "Plush" {
                    body = body.child(
                        col()
                            .w_full()
                            .gap(6.0)
                            .padding(10.0)
                            .child(text("Plush".to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
                            .child(
                                text("Soft felt and brushed-fur contours instead of glossy plastic.".to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap(),
                            )
                            .child(
                                row().w_full().center_y().gap(10.0).child(
                                    crate::look::controls::switch(&l, plush, "Plush", false)
                                        .key("settings-plush")
                                        .on_press(|s: &mut State| {
                                            s.settings_panel.settings.plush =
                                                !s.settings_panel.settings.plush;
                                            persist(s);
                                        }),
                                ),
                            ),
                    );
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::App => {
            let check_command = active_project_folder(s).and_then(|folder| {
                let ws = workspace_settings::workspace_id_for(std::path::Path::new(&folder));
                s.settings_panel.settings.check_commands.get(&ws).cloned()
            });
            for heading in headings {
                if heading == "Managed by your organization" {
                    if let Some(section) = org_policy_section(p) {
                        body = body.child(section);
                    }
                } else if heading == DOOR_HEADING {
                    body = body.child(door_section(s, p));
                } else if heading == "Chat apps" {
                    body = body.child(crate::chat_apps_section::section(s, p)).child(more(s, p, "chat-apps"));
                } else if heading == crate::phone_section::HEADING {
                    body = body.child(crate::phone_section::section(s, p));
                    if let Some(remote) = crate::remote_mcp_card::section(s, p) {
                        body = body.child(remote);
                    }
                } else if heading == "Startup" {
                    body = body.child(crate::quit_guard_dialog::startup_section(s, p));
                } else if heading == "Report a problem" {
                    body = body.child(crate::feedback_dialog::settings_section(s, p)).child(more(s, p, "report-a-problem"));
                } else if heading == locust_core::scout::view::SCOUT_SETTING_HEADING {
                    body = body.child(crate::scout_setting::section(
                        s,
                        p,
                        check_command.as_deref(),
                    ));
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Privacy => {
            for heading in headings {
                match heading {
                    "Privacy & local data" => {
                        body = body.child(crate::backup_section::storage_and_retention_section(s, p)).child(more(s, p, "privacy"));
                    }
                    "Trash" => {
                        body = body.child(crate::trash_section::section(s, p));
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
        SettingsPageId::Connectors => {
            for heading in headings {
                if heading == "GitHub" {
                    body = body.child(section_heading(heading, p)).child(crate::github_section::section(s, p)).child(more(s, p, "github"));
                } else if heading == "Connectors" {
                    body = body.child(section_heading(heading, p)).child(crate::connectors_section::section(s, p));
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Runtimes => {
            for heading in headings {
                if heading == "When an agent cannot go on" {
                    body = body.child(plan_runs_out_section(s, p));
                } else if heading == crate::what_locust_can_stop_ui::HEADING {
                    body = body.child(crate::what_locust_can_stop_ui::section(s, p));
                } else if heading == crate::agents_section::HEADING {
                    body = body.child(crate::agents_section::section(s, p));
                    if let Some(accounts) = crate::agent_accounts_card::section(s, p) {
                        body = body.child(accounts);
                    }
                    if let Some(remote) = crate::remote_control_card::section(s, p) {
                        body = body.child(remote);
                    }
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
                                .role("heading")
                                .level(2)
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
                    "Auto mode" => body = body.child(auto_mode_section(s, p)),
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
                            .child(text(heading.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
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
                    body = body.child(more(s, p, "memory"));
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
                .child(section_heading("Project folder", p))
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
                        .child(text(DATA_FOLDER_HEADING.to_owned()).role("heading").level(3).font(type_::LABEL).color(p.text))
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
                            .role("radio")
                            .label(chosen_label("Show me", fix_tries.is_none()))
                            .key("settings-check-fix-off")
                            .on_press(move |s: &mut State| {
                                s.settings_panel.settings.check_fixes.remove(&ws_off);
                                persist(s);
                            }),
                    )
                    .child({
                        // Off until there is a check to run (CheckAfterEditsRow): it says why on hover.
                        let unusable = !has_check && fix_tries.is_none();
                        let mut on = active_button("Send it back to fix", p, fix_tries.is_some())
                            .role("radio")
                            .label(chosen_label("Send it back to fix", fix_tries.is_some()))
                            .key("settings-check-fix-on");
                        if unusable {
                            on = on.opacity(0.45).help("Save a check first.");
                        }
                        on
                            .on_press(move |s: &mut State| {
                                let has = s.settings_panel.settings.check_commands.contains_key(&ws_on);
                                let current = s.settings_panel.settings.check_fixes.get(&ws_on).copied();
                                if has || current.is_some() {
                                    let next = current.unwrap_or(locust_core::edit_check::DEFAULT_CHECK_FIXES);
                                    s.settings_panel.settings.check_fixes.insert(ws_on.clone(), next);
                                    persist(s);
                                }
                            })
                    });
                if let Some(tries) = fix_tries {
                    fail_row = fail_row.child(text("at most").font(type_::LABEL).color(p.secondary));
                    for most in 1..=locust_core::edit_check::MAX_CHECK_FIXES {
                        let ws_m = ws.clone();
                        fail_row = fail_row.child(
                            active_button(most.to_string(), p, tries == most)
                                .role("radio")
                                .label(chosen_label(&format!("{most} {}", if most == 1 { "fix" } else { "fixes" }), tries == most))
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
                check_col = check_col.child(fail_row).child(
                    text(check_fix_note(has_check, fix_tries)).key("settings-check-fix-note").font(type_::LABEL).color(p.secondary).w_full().wrap(),
                );
                body = body.child(check_col);
            }
            body = body.child(crate::own_copies::section(s, p));
        }
        SettingsPageId::Relay => {
            let l = crate::look::controls::Look::new(p.dark);
            let relay = s.settings_panel.settings.relay;
            let cap = s.settings_panel.settings.relay_hop_cap;
            let interrupt = s.settings_panel.settings.interrupt;
            let lede = if relay {
                format!(
                    "Teammates answer each other on their own, each on its own route, and stop after {cap} automatic {}.",
                    if cap == 1 { "reply" } else { "replies" }
                )
            } else {
                "Teammates do not answer each other on their own. A message waits until its recipient next runs.".to_owned()
            };
            body = body.child(section_heading("Between teammates", p));
            body = body
                .child(
                    text(lede)
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .wrap()
                        .padding(10.0),
                )
                .child(more(s, p, "between-teammates"))
                .child(
                    col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .child(
                            text(if relay {
                                "Automatic replies: on. Teammates reply to each other until the work is done."
                            } else {
                                "Automatic replies: off. Messages wait for the recipient's next run."
                            })
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                        )
                        .child(
                            row().w_full().center_y().gap(10.0).child(
                                crate::look::controls::switch(&l, relay, "Automatic replies", false)
                                    .key("settings-relay-auto")
                                    .on_press(|s: &mut State| {
                                        s.settings_panel.settings.relay = !s.settings_panel.settings.relay;
                                        persist(s);
                                    }),
                            ),
                        ),
                )
                .child(
                    col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .child(
                            text(if interrupt {
                                "Urgent messages interrupt: on. An urgent message stops the recipient part-way. Their unfinished work stays in their own conversation."
                            } else {
                                "Urgent messages interrupt: off. An urgent message still waits for the recipient to finish."
                            })
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                        )
                        .child(
                            row().w_full().center_y().gap(10.0).child({
                                let mut sw = crate::look::controls::switch(
                                    &l,
                                    interrupt,
                                    "Urgent messages interrupt",
                                    !relay,
                                )
                                .key("settings-relay-interrupt");
                                if relay {
                                    sw = sw.on_press(|s: &mut State| {
                                        s.settings_panel.settings.interrupt =
                                            !s.settings_panel.settings.interrupt;
                                        persist(s);
                                    });
                                }
                                sw
                            }),
                        ),
                );
            let mut cap_row = row().w_full().center_y().gap(6.0);
            for opt in [2u32, 4, 8, 12, 16, 24] {
                let mut btn = active_button(opt.to_string(), p, cap == opt)
                    .key(format!("settings-relay-cap-{opt}"))
                    .disabled(!relay);
                if relay {
                    btn = btn.on_press(move |s: &mut State| {
                        s.settings_panel.settings.relay_hop_cap = opt;
                        persist(s);
                    });
                }
                cap_row = cap_row.child(btn);
            }
            body = body.child(
                col()
                    .w_full()
                    .gap(6.0)
                    .padding(10.0)
                    .child(
                        text("Automatic replies per exchange before they wait for you")
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                    )
                    .child(cap_row),
            );
        }
        SettingsPageId::WhatsNew => {
            // Screens.tsx: "Changelog", and which version is running.
            let version = if s.home.current_version.is_empty() { "the one running".to_owned() } else { s.home.current_version.clone() };
            body = body.child(section_heading("Changelog", p)).child(
                text(format!("Every version of Locust, newest first. This is {version}."))
                    .key("settings-changelog-lede")
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .wrap()
                    .px(10.0),
            );
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

        SettingsPageId::Models => {
            body = body.child(section_heading("Your own models", p)).child(crate::own_models_card::section(s, p)).child(more(s, p, "own-models"));
        }
        _ => {
            for heading in headings {
                body = body.child(deferred(heading, p));
            }
        }
    }

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
        // Screens.tsx: `<span className="lc-settings__navicon"><Icon name={entry.icon} size={16} /></span>` before the name.
        let l = crate::look::controls::Look::new(p.dark);
        let icon = crate::look::controls::glyph(&l, page.icon, 16.0, Some(if selected { p.text } else { p.secondary }))
            .key(format!("settings-navicon-{}", id.as_str()));
        nav = nav.child(
            row()
                .key(format!("settings-nav-{}", id.as_str()))
                .w_full()
                .gap(8.0)
                .center_y()
                .padding(7.0)
                .fill(if selected { p.selected } else { p.field })
                .radius(6.0)
                .role("button")
                .label(page.label)
                .child(icon)
                .child(
                    text(page.label)
                        .font(type_::LABEL)
                        .color(if selected { p.text } else { p.secondary }),
                )
                .on_press(move |s: &mut State| {
                    s.settings_panel.page = Some(id);
                }),
        );
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

    let page = shown_page(s);

    let mut content = col().grow(1.0).min_h(0.0).min_w(0.0).scroll(ScrollSpec {
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
            .shrink()
            .min_h(0.0)
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
    let (claude, codex, openclaw) = locust_core::door::setup(
        "/Applications/Locust Native.app/Contents/MacOS/locust-native",
        "/Users/locust/Library/Application Support/@teammate/desktop-native/locust-mcp-connection.json",
    );
    s.settings_panel.door = Some(locust_core::door::State {
        settings: locust_core::door::Settings { enabled: true, own_mode: true, conversations: false },
        claude_command: Some(claude),
        codex_config: Some(codex),
        openclaw_command: Some(openclaw),
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
#[path = "settings_door_tests.rs"]
mod settings_door_tests;

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

    // the-face-legend-shows-every-face.test.tsx, its fifth case; the other four are core's `face_legend::tests`.
    mod face_legend_tests {
        use super::*;
        use crate::look::bot_stage::PresenceDot;
        use locust_core::bot_rig::FaceActivity;

        fn legend_window() -> rune::Runtime<State> {
            let mut s = State::default();
            scene(&mut s);
            s.window_focused = true;
            let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1100.0, 2400.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            rt
        }

        fn legend_node(rt: &rune::Runtime<State>) -> serde_json::Value {
            fn find(node: &serde_json::Value) -> Option<serde_json::Value> {
                if node["key"] == "settings-face-legend" {
                    return Some(node.clone());
                }
                node["children"].as_array()?.iter().find_map(find)
            }
            find(&rt.accessibility_json()).expect("the face legend in the accessibility tree")
        }

        fn shown_name(rt: &rune::Runtime<State>) -> Option<String> {
            rt.elements().into_iter().find(|e| e.key.as_deref() == Some("settings-face-legend-name")).and_then(|e| e.text)
        }

        fn tick(rt: &mut rune::Runtime<State>, now: f64) {
            rt.update(move |s, tx| {
                tick_face_legend(s, now, tx);
            });
            rt.frame(0.016);
        }

        #[test]
        fn is_a_button_that_says_which_face_it_shows_the_face_under_it_a_real_teammate_face() {
            let rt = legend_window();
            let legend = legend_node(&rt);
            assert_eq!(legend["role"], "button");
            assert_eq!(legend["name"], "A teammate's face: Idle. Show the next one.");
            assert_eq!(rt.state.bot_stage.desired_activity("settings-face-legend"), Some(FaceActivity::Idle));
            assert_eq!(crate::look::faces::drawn_look("settings-face-legend"), Some(("droid".into(), "teal".into())));
            assert_eq!(shown_name(&rt).as_deref(), Some("Idle"));
            let face = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("bot-settings-face-legend")).expect("the live face");
            assert_eq!((face.rect[2], face.rect[3]), (40.0, 40.0));
        }

        #[test]
        fn a_press_shows_the_next_face_with_its_dot_and_its_name() {
            let mut rt = legend_window();
            let (x, y) = rt.find("settings-face-legend").unwrap();
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(0.016);
            assert_eq!(rt.state.bot_stage.desired_activity("settings-face-legend"), Some(FaceActivity::Thinking));
            assert_eq!(crate::look::faces::drawn_presence("settings-face-legend"), Some(Some(PresenceDot::Working)));
            assert_eq!(shown_name(&rt).as_deref(), Some("Thinking"));
            assert_eq!(legend_node(&rt)["name"], "A teammate's face: Thinking. Show the next one.");
        }

        #[test]
        fn each_face_has_its_time_then_the_next_comes_and_waiting_wears_the_amber_dot() {
            let mut rt = legend_window();
            let mut now = 100.0;
            tick(&mut rt, now);
            for (seconds, next) in [(2.6, "Thinking"), (3.4, "Working"), (3.0, "Replying"), (2.8, "Done"), (2.4, "Waiting on you")] {
                tick(&mut rt, now + seconds - 0.1);
                assert_ne!(shown_name(&rt).as_deref(), Some(next), "too soon for {next}");
                now += seconds;
                tick(&mut rt, now);
                assert_eq!(shown_name(&rt).as_deref(), Some(next));
            }
            assert_eq!(crate::look::faces::drawn_presence("settings-face-legend"), Some(Some(PresenceDot::Waiting)));
        }

        /// A poll as main.rs does it: the cover told what the window did, then the legend's clock.
        fn poll_at(rt: &mut rune::Runtime<State>, now: f64, input: Option<rune::time::Instant>) {
            rt.update(move |s, tx| {
                crate::cover_watch::observe(s, now, input);
                tick_face_legend(s, now, tx);
            });
            rt.frame(0.016);
        }

        // FaceLegend.tsx 66-74 with useCoverActivity: untouched for 45 s it stops where it is; input starts it again,
        // giving the face it stopped on its full time.
        #[test]
        fn it_rests_after_45_seconds_untouched_and_input_starts_it_again() {
            let mut rt = legend_window();
            let mut now = 0.0;
            poll_at(&mut rt, now, None);
            let mut changes = 0;
            loop {
                let due = rt.state.settings_panel.face_legend.due_in(now).expect("running");
                if now + due >= 45.0 {
                    break;
                }
                now += due;
                poll_at(&mut rt, now, None);
                changes += 1;
            }
            assert!(changes >= 8, "it went through every face first: {changes}");
            let rested = shown_name(&rt);
            poll_at(&mut rt, 45.0, None);
            assert!(rt.state.settings_panel.face_legend.due_in(45.0).is_none(), "no clock left while it rests");
            poll_at(&mut rt, 90.0, None);
            assert_eq!(shown_name(&rt), rested, "it stops where it is");
            poll_at(&mut rt, 91.0, Some(rune::time::Instant::now()));
            let seconds = rt.state.settings_panel.face_legend.shown().seconds;
            poll_at(&mut rt, 91.0 + seconds - 0.1, None);
            assert_eq!(shown_name(&rt), rested, "its full time again");
            poll_at(&mut rt, 91.0 + seconds, None);
            assert_ne!(shown_name(&rt), rested, "then on");
        }

        #[test]
        fn it_rests_behind_other_windows_and_starts_from_idle_when_appearance_is_shown_again() {
            let mut rt = legend_window();
            rt.update(|s, _| s.window_focused = false);
            tick(&mut rt, 0.0);
            tick(&mut rt, 60.0);
            assert_eq!(shown_name(&rt).as_deref(), Some("Idle"), "behind other windows it stops where it is");
            rt.update(|s, _| s.window_focused = true);
            tick(&mut rt, 61.0);
            tick(&mut rt, 63.6);
            assert_eq!(shown_name(&rt).as_deref(), Some("Thinking"));
            rt.update(|s, _| s.settings_panel.page = Some(SettingsPageId::App));
            tick(&mut rt, 64.0);
            rt.update(|s, _| s.settings_panel.page = Some(SettingsPageId::Appearance));
            rt.frame(0.016);
            assert_eq!(shown_name(&rt).as_deref(), Some("Idle"));
        }
    }

    #[test]
    fn scene_opens_appearance() {
        let mut s = State::default();
        scene(&mut s);
        assert!(s.settings_panel.open);
        assert_eq!(s.settings_panel.page, Some(SettingsPageId::Appearance));
    }

    #[test]
    fn relay_settings_page_renders_and_persists_switches_and_hop_cap() {
        let root = std::env::temp_dir().join(format!("settings-relay-{}", std::process::id()));
        std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let mut s = State {
            data: Some(store.clone()),
            ..State::default()
        };
        open(&mut s);
        s.settings_panel.page = Some(SettingsPageId::Relay);
        let _ = view(&s, &Palette::new(true));

        s.settings_panel.settings.relay = false;
        s.settings_panel.settings.interrupt = true;
        s.settings_panel.settings.relay_hop_cap = 24;
        persist(&mut s);

        let saved = workspace_settings::load(&store).unwrap();
        assert!(!saved.relay);
        assert!(saved.interrupt);
        assert_eq!(saved.relay_hop_cap, 24);
        let _ = view(&s, &Palette::new(false));
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
#[path = "settings_pages_real_tests.rs"]
mod pages_real_tests;

#[cfg(test)]
#[path = "settings_switches_tests.rs"]
mod switches_tests;

#[cfg(test)]
#[path = "settings_look_tests.rs"]
mod settings_look_tests;

#[cfg(test)]
#[path = "settings_read_like_claudes_tests.rs"]
mod settings_read_like_claudes_tests;

#[cfg(test)]
#[path = "check_fix_settings_tests.rs"]
mod check_fix_settings_tests;
