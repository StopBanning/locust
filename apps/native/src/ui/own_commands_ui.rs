//! The person's own commands in the window (Electron App.tsx's `ownCommandMenu` effect over
//! `own-command-host.ts`): the Markdown files in `<profile>/commands` and in the conversation folder's
//! `.locust/commands`, read in a job whenever the conversation, its teammate or the open folder changes, never while
//! drawing.
use super::*;
use locust_core::own_commands::{self, Expanded, Source, StoredCommand};
use std::path::PathBuf;

/// Who a command is read for: the profile its own folder is in, the conversation's teammate, and the folder Locust has
/// open (App.tsx's context: the run's mission, the picked teammate, else the workspace).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Context {
    pub profile: PathBuf,
    pub teammate: Option<Teammate>,
    pub open: Option<String>,
}

impl Context {
    /// The conversation's folder, found as its terminal and its `!` commands find it (`terminal_panel::folder`): the
    /// teammate's own, else the folder Locust has open. Reads the disk, so only in a job or on a press.
    pub fn folder(&self) -> Option<PathBuf> {
        self.teammate
            .as_ref()
            .and_then(|teammate| native::folder_for(teammate, &self.profile).ok())
            .or_else(|| self.open.as_ref().map(PathBuf::from))
    }
}

#[derive(Default)]
pub(super) struct Own {
    /// What the `/` menu offers of the person's own, in readTree's order: theirs, then the folder's.
    pub listed: Vec<StoredCommand>,
    /// The context the list was last asked for.
    pub read_for: Option<Context>,
    /// A folder's command waiting for Allow or Not now.
    pub ask: Option<Ask>,
}

/// The context now, or nothing before Locust has a data folder.
pub(super) fn context(s: &State) -> Option<Context> {
    let profile = s
        .data
        .as_ref()
        .map(|store| store.electron().to_path_buf())
        .or_else(|| native::store().map(|store| store.electron().to_path_buf()))?;
    Some(Context {
        profile,
        teammate: selected_conversation(s).map(|(teammate, _)| teammate.clone()),
        open: s.home.folder_path.clone(),
    })
}

/// Take a finished read, and ask again when the context has changed; and take a chosen command once it is expanded.
pub(super) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    if let Some((chosen, answer)) = s.own_commands_expand_job.take() {
        landed(s, tx, chosen, answer);
        changed = true;
    }
    if let Some(listed) = s.own_commands_job.take() {
        s.own_commands.listed = listed;
        changed = true;
    }
    let Some(wanted) = context(s) else {
        return changed;
    };
    if s.own_commands.read_for.as_ref() != Some(&wanted) {
        s.own_commands.read_for = Some(wanted.clone());
        s.own_commands_job.request(wanted, |context| own_commands::commands_in(&context.profile, context.folder().as_deref()));
    }
    changed
}

/// A folder's command waiting for the person's yes before its `!` lines run (App.tsx `commandAsk`): what to expand
/// again once allowed, and the lines the card names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ask {
    pub name: String,
    pub source: Source,
    pub args: String,
    pub lines: Vec<String>,
}

/// One command chosen from the menu, as App.tsx's `chooseOwnCommand` and `allowCommandAsk` send it to be expanded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Chosen {
    pub name: String,
    pub source: Source,
    pub args: String,
}

/// The person's own commands as `/` menu rows (Composer.tsx's `ownRows`), in readTree's order.
pub(super) fn menu_rows(s: &State) -> Vec<composer::SlashCommand> {
    composer::own_slash_commands(&s.own_commands.listed)
}

/// Choosing one of the person's commands (App.tsx `chooseOwnCommand`): its prompt is expanded off the window's thread
/// and lands in the box; nothing is sent.
pub(super) fn choose(s: &mut State, name: &str, source: Source, args: &str) {
    let Some(context) = context(s) else { return };
    let chosen = Chosen { name: name.to_owned(), source, args: args.to_owned() };
    s.own_commands_expand_job.request((context, chosen), |(context, chosen)| {
        let answer = own_commands::expand_here(&context.profile, context.folder().as_deref(), &chosen.name, chosen.source, &chosen.args);
        (chosen, answer)
    });
}

/// Allow, on the card (App.tsx `allowCommandAsk`): the folder is allowed for good, then the command is expanded again.
/// A folder that could not be allowed leaves the card as it was.
pub(super) fn allow_ask(s: &mut State) {
    let (Some(ask), Some(context)) = (s.own_commands.ask.clone(), context(s)) else { return };
    let chosen = Chosen { name: ask.name, source: ask.source, args: ask.args };
    s.own_commands_expand_job.request((context, chosen), |(context, chosen)| {
        let folder = context.folder();
        let answer = match own_commands::allow(&context.profile, folder.as_deref()) {
            Ok(()) => own_commands::expand_here(&context.profile, folder.as_deref(), &chosen.name, chosen.source, &chosen.args),
            Err(refused) => Expanded::Refused(refused),
        };
        (chosen, answer)
    });
}

/// Not now, on the card or Escape in the box: the card goes and the box stays as it was.
pub(super) fn not_now(s: &mut State) {
    s.own_commands.ask = None;
}

/// An expanded command lands (App.tsx's `.then`): a refusal changes nothing, as Electron's `if (!answer.ok) return`; the
/// folder's lines wait on the card; a prompt goes into the box.
fn landed(s: &mut State, tx: &mut Tx, chosen: Chosen, answer: Expanded) {
    match answer {
        Expanded::Refused(_) => {}
        Expanded::Ask(lines) => s.own_commands.ask = Some(Ask { name: chosen.name, source: chosen.source, args: chosen.args, lines }),
        Expanded::Text { text, mode, teammate } => apply(s, tx, text, mode, teammate),
    }
}

/// App.tsx `applyOwnCommand`: the mode the file names when the organization allows it (Auto turned on in Settings
/// first, as choosing Auto does), the teammate it names when there is one by that name, and the prompt into the box
/// with nothing attached. Nothing is sent.
fn apply(s: &mut State, tx: &mut Tx, text: String, mode: Option<&'static str>, teammate: Option<String>) {
    if let Some(name) = teammate {
        let mate = s.teammates.iter().find(|one| one.name.to_lowercase() == name.to_lowercase()).map(|one| one.id.clone());
        let current = selected_conversation(s).map(|(t, _)| t.id.clone());
        if let Some(id) = mate
            && current.as_deref() != Some(id.as_str())
        {
            select_teammate(s, &id, tx);
        }
    }
    if let Some(mode) = mode
        && locust_core::org_policy::mode_allowed(Some(&locust_core::org_policy::current()), mode)
    {
        if mode == "auto" {
            enable_auto_mode(s);
        }
        s.composer_mode = Some(mode.to_owned());
    }
    s.own_commands.ask = None;
    s.draft.replace_text(text);
    s.attached.clear();
}

/// Settings' Auto switch turned on, as App.tsx does before a command's Auto mode (`writeWorkspaceSettings`).
fn enable_auto_mode(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    if let Ok(mut settings) = crate::workspace_settings::load(&store)
        && !settings.auto_mode
    {
        settings.auto_mode = true;
        if crate::workspace_settings::save(&store, &settings).is_ok() {
            s.settings_panel.settings.auto_mode = true;
        }
    }
}

/// CommandAsk (OwnCommand.tsx): a folder's `!` lines, named, before they run. Allow is for the folder, once; Not now
/// leaves the box as it was.
pub(super) fn ask_card(s: &State, p: &Palette) -> Option<El> {
    let ask = s.own_commands.ask.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let mut lines = col().w_full().gap(2.);
    for line in &ask.lines {
        lines = lines.child(text(line.clone()).font(crate::terminal_panel::mono()).color(p.text).w_full().wrap().selectable(true));
    }
    Some(
        col()
            .key("command-ask")
            .w_full()
            .gap(8.)
            .padding(10.)
            .radius(9.)
            .fill(p.raised)
            .border(1., p.hairline)
            .role("dialog")
            .label("Allow this folder's commands")
            .child(
                text("This folder's commands have not been allowed to run yet. Allow them once, and these will run:")
                    .color(p.text)
                    .w_full()
                    .wrap(),
            )
            .child(lines)
            .child(
                row()
                    .gap(8.)
                    .child(crate::look::controls::button(&l, "Allow", crate::look::controls::Tone::Neutral, false).key("command-ask-allow").on_press(allow_ask))
                    .child(crate::look::controls::button(&l, "Not now", crate::look::controls::Tone::Neutral, false).key("command-ask-not-now").on_press(not_now)),
            ),
    )
}

#[cfg(test)]
#[path = "own_commands_ui_tests.rs"]
mod own_commands_ui_tests;
