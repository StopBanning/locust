use super::*;

const QUEUED_SEND_NOT_ACKNOWLEDGED: &str = "not sent — press Send to try again";

fn keep_unacknowledged_queue_row(s: &mut State, row: composer::QueuedRow) {
    // Keep the original queue words in memory even if saving the recovery fails. The row is held, not retried.
    s.queue_rows.insert(0, row);
    persist_queue(s);
}

pub(super) fn restore_failed_draft(s: &mut State, message: String) {
    // Detaching can drop the acknowledgement. Offer the original queued words back, but never dispatch them again
    // without another press. Keep text typed after dispatch in the composer.
    if s.queue_in_flight.as_ref().is_some_and(|row| row.held.as_deref() == Some(QUEUED_SEND_NOT_ACKNOWLEDGED))
        && let Some(row) = s.queue_in_flight.take()
    {
        keep_unacknowledged_queue_row(s, row);
        return;
    }
    // Fix: restore only failed sends; never overwrite text typed after dispatch.
    let text = if s.draft.text.is_empty() {
        message
    } else {
        format!("{message}\n\n{}", s.draft.text)
    };
    s.draft.replace_text(text);
}

/// A message the queue sent that the host refused as busy goes back in front of the line, pointed at the conversation
/// it was typed in and not tried again before its retry time (App.tsx `startMission`'s `requeue`, `retriedAfterBusy`).
/// An explicit Send instead goes back held on any refusal. True means its words should not also enter the draft.
pub(super) fn requeued_after_busy(s: &mut State, error: &str) -> bool {
    let Some(row) = s.queue_in_flight.take() else {
        return false;
    };
    if row.held.as_deref() == Some(QUEUED_SEND_NOT_ACKNOWLEDGED) {
        keep_unacknowledged_queue_row(s, row);
        return true;
    }
    if !error.contains("already running") {
        return false;
    }
    let back = composer::queue::retried_after_busy(&row, &row.key, epoch_ms());
    s.queue_rows.insert(0, back);
    persist_queue(s);
    true
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub(super) fn conversation_draft_key(s: &State) -> Option<String> {
    let (teammate, conversation) = selected_conversation(s)?;
    Some(composer::draft_key(
        conversation.conversation_id.as_deref(),
        &teammate.id,
    ))
}

pub(super) fn sync_draft_key(s: &mut State) {
    let next = conversation_draft_key(s);
    let fill = s.drafts.switch(next, &s.draft.text);
    if fill != s.draft.text {
        s.draft.replace_text(fill);
    }
}

pub(super) fn queue_key_now(s: &State) -> Option<String> {
    let (_, conversation) = selected_conversation(s)?;
    conversation
        .conversation_id
        .clone()
        .or_else(|| conversation_draft_key(s))
}

pub(super) fn turn_running(s: &State) -> bool {
    s.flight.is_some()
        || s.opening_teammate.is_some()
        || s.ledger
            .as_ref()
            .is_some_and(|p| p.snapshot.running || p.after_tip.is_some())
}

pub(super) fn working_name(s: &State) -> String {
    selected_conversation(s)
        .map(|(t, _)| t.name.clone())
        .unwrap_or_else(|| "this mission".into())
}

pub(super) fn load_composer_store(s: &mut State) {
    if s.data.is_some() || s.demo {
        return;
    }
    if let Ok(store) = store::Store::local() {
        match composer::read_queue(&store) {
            Ok(rows) => s.queue_rows = rows,
            Err(message) => {
                s.queue_error = Some(format!(
                    "Saved messages could not be read. Reopen Locust to try again; the saved file has been kept. ({message})"
                ));
            }
        }
        s.org = org::load_org(&store);
        let prefs = crate::workspace_settings::load_appearance(&store);
        s.appearance = prefs.theme.to_appearance();
        s.settings_panel.theme = prefs.theme;
        s.settings_panel.look = prefs.look;
        s.settings_panel.glass_ground = prefs.glass_ground;
        s.settings_panel.keep_running = locust_core::keep_awake::read_keep_running(
            &store.root().join(locust_core::keep_awake::BACKGROUND_FILE),
        );
        s.settings_panel.stay_awake = locust_core::keep_awake::read_stay_awake(
            &store.root().join(locust_core::keep_awake::AWAKE_FILE),
        );
        if let Ok(settings) = crate::workspace_settings::load(&store) {
            s.layout_pref = settings.layout;
        }
        s.limits_job.request(store.clone(), |store| locust_core::limits::limited_runtimes(&store));
        s.data = Some(store);
    }
}

pub(super) fn persist_queue(s: &mut State) -> bool {
    let Some(store) = s.data.as_ref() else {
        return s.demo;
    };
    match composer::write_queue(store, &s.queue_rows) {
        Ok(_) => {
            s.queue_error = None;
            return true;
        }
        Err(message) => {
            s.queue_error = Some(format!(
                "This queue change was not saved. Your queued messages are still here; try again. ({message})"
            ));
        }
    }
    false
}

pub(super) fn enqueue_follow_up(s: &mut State, text: String, attachments: Vec<String>) -> bool {
    let Some(key) = queue_key_now(s) else {
        return false;
    };
    s.queue_rows.push(composer::QueuedRow {
        id: composer::new_queue_id(),
        key,
        text,
        origin: composer::Origin::Person,
        attachments,
        issue: None,
        restored: false,
        held: None,
        retry_at: None,
        tries: None,
    });
    if !persist_queue(s) {
        s.queue_rows.pop();
        return false;
    }
    s.off_the_queue = false;
    true
}

/// Electron `runtimeIsUsable`: it answered, and answered ready.
fn usable(f: &locust_core::agents::Found) -> bool {
    f.ready && f.status == locust_core::agents::Probe::Ready
}

/// What discovery says of the box's own agent (Composer.tsx `selectedReady`), or nothing when it cannot say: no
/// answer yet, an account route or an added agent (the host finds those when the message goes), or an agent whose
/// check is still out. Nothing keeps the window's own answer, so a launch is never blocked on a sweep in flight.
pub(super) fn discovered_ready(s: &State) -> Option<bool> {
    let agents = s.settings_panel.agents.as_ref()?;
    let route = crate::group_settings_dialog::current_composer_route(s)?;
    if route.account.is_some() || route.runtime == "acp" {
        return None;
    }
    let own = agents.iter().find(|f| f.id == route.runtime)?;
    if own.checking {
        return None;
    }
    Some(usable(own))
}

/// Whether the box's agent can take this message: discovery's word when it has one, else the window's.
pub(super) fn selected_ready_now(s: &State) -> bool {
    discovered_ready(s).unwrap_or(s.selected_ready)
}

pub(super) fn composer_placeholder(s: &State) -> String {
    if s.cloud.codex.on {
        return "Describe a task for Codex Cloud…".into();
    }
    if native_blocked(s)
        && let Some((teammate, _)) = selected_conversation(s)
    {
        return format!("{} runs in Locust for now", teammate.name);
    }
    let teammate_name = selected_conversation(s).map(|(t, _)| t.name.as_str());
    let agents = s.settings_panel.agents.as_deref().unwrap_or(&[]);
    // Something else here can run, though the box does not point at it (`readyElsewhere`).
    let ready_elsewhere = agents
        .iter()
        .find(|f| usable(f) && locust_core::route_at_start::is_mission_runtime(&f.id))
        .map(|f| f.display_name.as_str());
    // "and sign in" only when an agent has SAID it wants a sign-in.
    let auth_required = agents.iter().any(|f| {
        f.status == locust_core::agents::Probe::AuthRequired || f.auth == locust_core::agents::Auth::Unauthenticated
    });
    let installed = agents.iter().any(|f| f.installed);
    // Counted only once the window has stopped asking (`runtimesGaveUp`); until then, installed is "Checking".
    let gave_up = installed && s.settings_panel.agents_gave_up;
    let installed_silent = if gave_up { agents.iter().filter(|f| f.installed && !usable(f)).count() } else { 0 };
    composer::placeholder(composer::PlaceholderInput {
        working_now: turn_running(s),
        working_name: &working_name(s),
        queued: queue_front(s).is_some(),
        route_can_run: s.route_can_run && !native_blocked(s),
        selected_ready: selected_ready_now(s) && !s.nothing_installed,
        selected_display: None,
        teammate_name,
        discovery_loading: s.settings_panel.agents.is_none() && s.settings_panel.agents_job.is_pending(),
        discovery_error: false,
        ready_elsewhere,
        auth_required,
        installed_silent,
        checking: installed && !gave_up,
        nothing_installed: s.nothing_installed,
    })
}

pub(super) fn queue_front(s: &State) -> Option<composer::QueuedRow> {
    let key = queue_key_now(s);
    let own: Vec<_> = composer::queued_in(&s.queue_rows, key.as_deref())
        .into_iter()
        .cloned()
        .collect();
    let mut front = own.first()?.clone();
    front.text = own
        .iter()
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    front.attachments = own.iter().flat_map(|r| r.attachments.clone()).collect();
    Some(front)
}

pub(super) fn unqueue(s: &mut State, edit: bool) -> bool {
    let Some(front) = queue_front(s) else {
        return false;
    };
    let before = s.queue_rows.clone();
    s.queue_rows = composer::without_queue_of(&s.queue_rows, Some(&front.key));
    if !persist_queue(s) {
        s.queue_rows = before;
        return false;
    }
    if edit {
        s.draft.replace_text(front.text);
        s.attached = front.attachments;
    }
    s.off_the_queue = edit;
    true
}

pub(super) fn send_blocked_now(s: &State) -> Option<&'static str> {
    composer::send_blocked_reason(composer::SendBlocked {
        nothing_installed: s.nothing_installed,
        runtime_ready: selected_ready_now(s) && !native_blocked(s),
        route_can_run: s.route_can_run,
        busy: s.team.is_pending() || s.opening_teammate.is_some(),
        empty: s.draft.text.trim().is_empty() && s.attached.is_empty(),
    })
}

pub(super) fn slash_choices_now(s: &State) -> Vec<composer::SlashCommand> {
    // The allow card is the decision: no menu while it waits (Composer.tsx's `commandAsk`).
    if s.own_commands.ask.is_some() {
        return Vec::new();
    }
    let yours = crate::own_commands_ui::menu_rows(s);
    if s.scene_popup == ComposerScenePopup::Slash {
        return composer::matching_commands(
            "",
            &composer::available_commands(composer::AvailableInput {
                modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
                running: turn_running(s),
                issues: false,
                tune: !s.teammates.is_empty(),
                setup: true,
                own_commands: &yours,
                runtime_commands: &[],
            }),
        );
    }
    // `/name the rest` is still that command (Composer.tsx `typedOwn`): Enter fills the box with the rest as its
    // arguments.
    let names: Vec<&str> = yours.iter().map(|c| c.name.as_str()).collect();
    if let Some((name, _)) = locust_core::own_commands::arguments_of_own_command(&s.draft.text, &names) {
        let with_args: Vec<_> = yours.iter().filter(|c| c.name.to_lowercase() == name.to_lowercase()).cloned().collect();
        if !with_args.is_empty() {
            return with_args;
        }
    }
    let Some(query) = composer::slash_query(&s.draft.text) else {
        return Vec::new();
    };
    // Writing a runtime command's arguments: the menu stays shut, so Enter sends it.
    if crate::runtime_commands_ui::writing_arguments(s) {
        return Vec::new();
    }
    let theirs = crate::runtime_commands_ui::menu_rows(s);
    let available = composer::available_commands(composer::AvailableInput {
        modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
        running: turn_running(s),
        issues: crate::issue_list::offered(s),
        tune: !s.teammates.is_empty(),
        setup: true,
        own_commands: &yours,
        runtime_commands: &theirs,
    });
    composer::matching_commands(&query, &available)
}

#[derive(Clone)]
pub(super) enum AtChoice {
    Teammate {
        id: String,
        name: String,
        role: String,
    },
    File {
        path: String,
    },
}

pub(super) fn at_choices_now(s: &State) -> Vec<AtChoice> {
    let Some(typed) = composer::at_query(&s.draft.text) else {
        return Vec::new();
    };
    if composer::slash_query(&s.draft.text).is_some() {
        return Vec::new();
    }
    if s.at_dismissed.as_deref() == Some(s.draft.text.as_str()) {
        return Vec::new();
    }
    let current_id = selected_conversation(s).map(|(t, _)| t.id.clone());
    let mut out = Vec::new();
    for mate in s.teammates.iter().filter(|m| {
        Some(m.id.as_str()) != current_id.as_deref()
            && !s.tagged.contains(&m.id)
            && m.name.to_lowercase().starts_with(&typed.to_lowercase())
    }) {
        if out.len() >= 6 {
            break;
        }
        out.push(AtChoice::Teammate {
            id: mate.id.clone(),
            name: mate.name.clone(),
            role: mate.role.clone(),
        });
    }
    let files = composer::file_matches(
        &s.project_files
            .iter()
            .filter(|p| !s.attached.contains(p))
            .cloned()
            .collect::<Vec<_>>(),
        &typed,
        8,
    );
    for path in files {
        out.push(AtChoice::File { path });
    }
    out
}

pub(super) fn pick_at_choice(s: &mut State, choice: AtChoice) {
    s.at_at = 0;
    s.draft
        .replace_text(composer::without_at_query(&s.draft.text));
    match choice {
        AtChoice::Teammate { id, .. } => {
            if !s.tagged.contains(&id) && s.tagged.len() < locust_core::tagging::MAX_TAGGED {
                s.tagged.push(id);
            }
        }
        AtChoice::File { path } => attach_files(s, &[path]),
    }
}

pub(super) fn run_slash_command(s: &mut State, command: &composer::SlashCommand) {
    s.slash_at = 0;
    match &command.action {
        composer::SlashAction::Mode(mode) => {
            s.composer_mode = Some((*mode).to_owned());
            s.draft.clear();
        }
        composer::SlashAction::Stop => {
            s.draft.clear();
            // stop() needs Tx; callers that can stop invoke it separately.
        }
        composer::SlashAction::Setup => {
            s.draft.clear();
            crate::setup_card::open_from_composer(s);
        }
        // The issue list. Choosing one fills the box. Nothing is sent.
        composer::SlashAction::Issues => {
            s.draft.clear();
            crate::issue_list::open_list(s);
        }
        composer::SlashAction::Tune => {
            s.draft.clear();
            crate::tune_teammate::open_from_composer(s);
        }
        // The person's own command fills the box with its prompt (Composer.tsx `ownCommandChoice`). Nothing is sent.
        composer::SlashAction::Own(source) => {
            let args = locust_core::own_commands::arguments_of_own_command(&s.draft.text, &[command.name.as_str()])
                .map(|(_, args)| args)
                .unwrap_or_default();
            crate::own_commands_ui::choose(s, &command.name, *source, &args);
        }
        // The runtime's own command: `/name ` in the box, where its arguments go; sending it sends the command.
        composer::SlashAction::Runtime => {
            s.draft.replace_text(format!("/{} ", command.name));
        }
        composer::SlashAction::Route => {
            s.draft.clear();
            s.composer_note = Some(format!(
                "/{name} — {detail}",
                name = command.name,
                detail = command.detail
            ));
        }
    }
}

pub(super) fn attach_dropped_paths(s: &mut State, paths: &[std::path::PathBuf]) {
    let as_str: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    attach_files(s, &as_str);
}

/// Files going with the next message, kept to a message's limit. A PDF among the new ones starts being opened for the
/// agent at once (Electron's `readAttachedPdfs`), in the folder the turn will run in (`native::folder_for`: the
/// teammate's own, else the one Locust has open), so Send usually finds it read, and otherwise waits for this reading.
pub(super) fn attach_files(s: &mut State, added: &[String]) {
    let (next, note) = composer::attach_paths(&s.attached, added);
    let new: Vec<String> = next.iter().filter(|path| !s.attached.contains(path)).cloned().collect();
    s.attached = next;
    if let Some(note) = note {
        s.composer_note = Some(note);
    }
    let teammate = s.ledger.as_ref().map(|pane| &pane.header).or_else(|| s.teammates.get(s.selected));
    if let Some(folder) = teammate.and_then(|t| t.folder.clone()).or_else(|| s.home.folder_path.clone()) {
        let folder = std::path::PathBuf::from(folder);
        let sources: std::collections::HashMap<_, _> = new.iter()
            .filter(|path| locust_core::pdf::is_pdf_path(path))
            .filter_map(|path| attachment_source(&folder, path).map(|stamp| (path.clone(), stamp)))
            .collect();
        let jobs = locust_core::pdf::read_attached_pdfs(&folder, &new);
        let images = s.image_cache.clone();
        for job in jobs {
            let folder = folder.clone();
            let sources = sources.clone();
            let images = images.clone();
            std::thread::spawn(move || {
                let Some(reading) = job.join().ok().flatten() else { return };
                let path = reading.pdf.clone();
                if sources.get(&path).is_none_or(|before| attachment_source(&folder, &path).as_ref() != Some(before)) {
                    return;
                }
                let first = reading.pictures.first().cloned();
                crate::pdf_pages_ui::keep(&folder, &path, reading);
                // The image cache wakes the window once the first page decodes.
                if let (Some(images), Some(first)) = (images, first)
                    && let Ok(picture) = crate::workspace_image::workspace_image(&first, &folder, std::slice::from_ref(&folder))
                {
                    let _ = images.file(picture, crate::step_pictures::PICTURE_MAX_HEIGHT, 2.0);
                }
            });
        }
    }
}

fn attachment_source(folder: &std::path::Path, path: &str) -> Option<(std::path::PathBuf, u64, std::time::SystemTime)> {
    let source = std::fs::canonicalize(folder.join(path)).ok()?;
    let meta = std::fs::metadata(&source).ok()?;
    meta.is_file().then_some((source, meta.len(), meta.modified().ok()?))
}

pub(super) fn save_pasted_image(s: &mut State, png: &[u8]) -> Option<String> {
    if png.len() > composer::MAX_PASTED_BYTES {
        s.composer_note = Some("That image is too large to attach.".into());
        return None;
    }
    let store = s.data.as_ref()?;
    let dir = store.attachments_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("paste-{}.png", composer::new_queue_id());
    let path = dir.join(&name);
    std::fs::write(&path, png).ok()?;
    Some(path.to_string_lossy().replace('\\', "/"))
}

pub(super) fn composer_input_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    // The allow card is the decision (Composer.tsx): Enter must not send the command's name, and Escape is Not now.
    if s.own_commands.ask.is_some() {
        match event.key {
            Key::Escape => {
                crate::own_commands_ui::not_now(s);
                return true;
            }
            Key::Enter if !event.mods.shift => return true,
            _ => return false,
        }
    }
    if s.draft.text.starts_with('!') && event.key == Key::Escape {
        s.draft.replace_text(own_command::leave_command_mode(&s.draft.text));
        return true;
    }
    let slash = slash_choices_now(s);
    let at = at_choices_now(s);
    let key = match event.key {
        Key::Up => composer::ComposerKey::ArrowUp,
        Key::Down => composer::ComposerKey::ArrowDown,
        Key::Enter => composer::ComposerKey::Enter,
        Key::Escape => composer::ComposerKey::Escape,
        Key::Tab => composer::ComposerKey::Tab,
        _ => composer::ComposerKey::Other,
    };
    let mods = composer::KeyMods {
        shift: event.mods.shift,
        ctrl: event.mods.ctrl,
        meta: event.mods.cmd,
        alt: event.mods.alt,
    };
    let action = composer::handle_key(
        key,
        mods,
        &composer::keys::KeyContext {
            composing: false,
            slash_open: !slash.is_empty(),
            slash_len: slash.len(),
            at_open: !at.is_empty(),
            at_len: at.len(),
            running: turn_running(s),
            menu_open: s.model_menu.is_some(),
            value_empty: s.draft.text.is_empty(),
            recalling: s.recall_at >= 0,
            history_len: s.sent_history.len(),
            can_queue: turn_running(s),
            modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
        },
    );
    match action {
        composer::KeyAction::Pass => false,
        composer::KeyAction::SlashMove { delta } => {
            let len = slash.len().max(1);
            if delta > 0 {
                s.slash_at = (s.slash_at + 1) % len;
            } else {
                s.slash_at = (s.slash_at + len - 1) % len;
            }
            true
        }
        composer::KeyAction::SlashChoose => {
            if let Some(cmd) = slash.get(s.slash_at.min(slash.len().saturating_sub(1))) {
                let should_stop = matches!(cmd.action, composer::SlashAction::Stop);
                run_slash_command(s, cmd);
                if should_stop {
                    stop(s);
                }
            }
            true
        }
        composer::KeyAction::SlashDismiss => {
            s.draft.clear();
            true
        }
        composer::KeyAction::AtMove { delta } => {
            let len = at.len().max(1);
            if delta > 0 {
                s.at_at = (s.at_at + 1) % len;
            } else {
                s.at_at = (s.at_at + len - 1) % len;
            }
            true
        }
        composer::KeyAction::AtChoose => {
            if let Some(choice) = at.get(s.at_at.min(at.len().saturating_sub(1))) {
                pick_at_choice(s, choice.clone());
            }
            true
        }
        composer::KeyAction::AtDismiss => {
            s.at_dismissed = Some(s.draft.text.clone());
            true
        }
        composer::KeyAction::HistoryUp => {
            let next = if s.recall_at < 0 {
                s.sent_history.len() as isize - 1
            } else {
                (s.recall_at - 1).max(0)
            };
            s.recall_at = next;
            if let Some(text) = s.sent_history.get(next as usize) {
                s.draft.replace_text(text.clone());
            }
            true
        }
        composer::KeyAction::HistoryDown => {
            let next = s.recall_at + 1;
            if next >= s.sent_history.len() as isize {
                s.recall_at = -1;
                s.draft.clear();
            } else {
                s.recall_at = next;
                if let Some(text) = s.sent_history.get(next as usize) {
                    s.draft.replace_text(text.clone());
                }
            }
            true
        }
        composer::KeyAction::Submit { send_now } => {
            submit_composer(s, tx, send_now);
            true
        }
        composer::KeyAction::StopRun => {
            stop(s);
            true
        }
        composer::KeyAction::CycleMode => {
            cycle_mode(s);
            true
        }
    }
}

/// Shift+Tab on the message box: the mode chip moves to the next mode this teammate's route can run (in the menu's
/// order), Auto only while Settings' Auto switch is on. The mode a turn runs in is the chip's (`composer_mode`).
pub(super) fn cycle_mode(s: &mut State) {
    let Some(teammate) = s.teammates.get(s.selected) else { return };
    let runtime = s
        .picks
        .get(&teammate.id)
        .and_then(|p| p.runtime.clone())
        .or_else(|| teammate.runtime.clone())
        .unwrap_or_else(|| "codex".into());
    let current = s.composer_mode.clone().unwrap_or_else(|| teammate.runs_in.clone());
    let usable: Vec<&str> = locust_core::mode_facts::MODE_FACTS
        .iter()
        .map(|f| f.mode)
        .filter(|mode| locust_core::mode_facts::runs_on(mode, &runtime))
        .collect();
    let auto_allowed = s.settings_panel.settings.auto_mode;
    if let Some(next) = composer::shift_tab_mode(&current, &usable, auto_allowed) {
        s.composer_mode = Some(next);
    }
}

pub(super) fn submit_composer(s: &mut State, tx: &mut Tx, send_now: bool) {
    // Nothing goes while the allow card waits (Composer.tsx `submit`'s `commandAsk !== undefined`).
    if s.demo || s.own_commands.ask.is_some() {
        return;
    }
    if s.terminal.catchup.is_pending() {
        s.composer_note = Some("Reading terminal turns…".into());
        return;
    }
    let typed = s.draft.text.trim().to_owned();
    if typed.eq_ignore_ascii_case("/setup") {
        s.draft.clear();
        crate::setup_card::open_from_composer(s);
        return;
    }
    let lower = typed.to_ascii_lowercase();
    if lower == "/tune" || lower.starts_with("/tune ") {
        let seed = typed.get(5..).map(str::trim).unwrap_or("").to_owned();
        s.draft.clear();
        crate::tune_teammate::open_from_composer(s);
        if !seed.is_empty()
            && let Some(card) = s.tune_card.as_mut()
        {
            card.ask.replace_text(seed);
        }
        return;
    }
    if typed.is_empty() && s.attached.is_empty() && s.page_points.chips.is_empty() {
        if let Some(why) = send_blocked_now(s) {
            s.composer_note = Some(why.to_owned());
        }
        return;
    }
    if own_command::bang(&s.draft.text).is_some() {
        terminal_panel::run_command(s);
        return;
    }
    if crate::claude_cloud_panel::send(s, &typed) {
        return;
    }
    if crate::cloud_tasks::send(s, &typed) {
        return;
    }
    if turn_running(s) {
        // A queued message carries the parts pointed at too; they leave the box only once it is queued.
        let (queued, attached) = crate::page_points_ui::with_points(s, &typed, &s.attached);
        if queue_front(s).is_some() || !enqueue_follow_up(s, queued, attached) {
            return;
        }
        crate::tag_send::send_tags(s, &composer::with_attachments(&typed, &s.attached));
        crate::page_points_ui::sent(s);
        s.draft.clear();
        s.attached.clear();
        s.tagged.clear();
        if send_now {
            s.queue_send_now = queue_key_now(s);
            stop(s);
            // Queued send after stop is drained in poll when the turn ends.
        }
        return;
    }
    if send_blocked_now(s).is_some() || !can_send(s) {
        if let Some(why) = send_blocked_now(s) {
            s.composer_note = Some(why.to_owned());
        } else if !s.route_can_run {
            s.composer_note = Some(
                "Locust cannot run that AI agent fully yet, so it cannot start here. Pick another model."
                    .into(),
            );
        } else if s.nothing_installed || !selected_ready_now(s) {
            s.composer_note = Some(
                "No AI agent on this machine can start yet. Settings lists what to install.".into(),
            );
        }
        return;
    }
    if crate::folder_confirmation::hold_send(s) {
        return;
    }
    if s.sent_history.last().map(String::as_str) != Some(typed.as_str()) && !typed.is_empty() {
        s.sent_history.push(typed.clone());
        if s.sent_history.len() > 50 {
            s.sent_history.remove(0);
        }
    }
    s.recall_at = -1;
    // The parts pointed at in the browser go with this message, after the person's words (PagePointChips).
    let typed = crate::page_points_ui::take_into(s, &typed);
    s.draft
        .replace_text(composer::with_attachments(&typed, &s.attached));
    crate::tag_send::send_tags(s, &s.draft.text.clone());
    s.attached.clear();
    s.tagged.clear();
    s.off_the_queue = false;
    // The person's own send: no queued row is in flight, whatever an earlier one left.
    s.queue_in_flight = None;
    send(s, tx);
}

pub(super) fn queue_phase(s: &State) -> &'static str {
    if let Some(pane) = &s.ledger {
        // The worker is moved out of the pane during a poll. The selected thread's
        // last accepted snapshot, not the worker's presence, says how its tip ended.
        if pane.snapshot.running || pane.after_tip.is_some() || pane.loading {
            return "running";
        }
        if pane.snapshot.interrupted {
            return "interrupted";
        }
        if pane.snapshot.failed {
            return "failed";
        }
        match &pane.snapshot.run_end {
            Some(ledger::RunEnd::Completed) => return "completed",
            Some(ledger::RunEnd::Failed { .. }) => return "failed",
            Some(ledger::RunEnd::Cancelled) => return "cancelled",
            None => {}
        }
        return pane.worker.as_ref().map_or("unknown", |w| w.tip_phase());
    }
    if matches!(s.status_tone, StatusTone::Red) {
        "failed"
    } else {
        "completed"
    }
}

pub(super) fn drain_queue_after_turn(s: &mut State, tx: &mut Tx) {
    let Some(key) = queue_key_now(s) else {
        return;
    };
    let front = composer::queued_in(&s.queue_rows, Some(&key))
        .first()
        .map(|r| (*r).clone());
    let Some(front) = front else {
        return;
    };
    let forced = s.queue_send_now.as_deref() == Some(key.as_str());
    let verdict = composer::queued_verdict(composer::VerdictInput {
        running: turn_running(s),
        phase: Some(queue_phase(s)),
        on_screen: true,
        restored: front.restored,
        held: front.held.as_deref(),
    });
    if turn_running(s) || (!forced && !matches!(verdict, composer::QueuedVerdict::Send)) {
        return;
    }
    // Refused as busy a moment ago: not before its retry time, and the window looks again then.
    if let Some(wait) = front.retry_at.and_then(|at| at.checked_sub(epoch_ms())).filter(|wait| *wait > 0) {
        tx.wake_after(std::time::Duration::from_millis(wait));
        return;
    }
    let (going, rest) = composer::take_next(&s.queue_rows, &key);
    let Some(mut going) = going else {
        return;
    };
    // A folded row can exceed the size of one saved row. Do not remove the originals when the flight slot could
    // not put that fold back safely. Recovering every original row across the acknowledgement needs a wider change.
    if composer::queue_store::serialize_queued(std::slice::from_ref(&going)).is_err() {
        s.queue_send_now = None;
        if let Some(front) = s.queue_rows.iter_mut().find(|row| row.key == key) {
            front.held = Some(QUEUED_SEND_NOT_ACKNOWLEDGED.into());
        }
        s.queue_error = Some("These queued messages cannot be sent together. Edit them to send smaller messages. Nothing was sent.".into());
        return;
    }
    let draft = s.draft.text.clone();
    s.draft
        .replace_text(composer::with_attachments(&going.text, &going.attachments));
    if !can_send(s) || s.compare.composer_mode != compare_view::CompareComposerMode::Direct
        || crate::folder_confirmation::hold_send(s)
    {
        s.draft.replace_text(draft);
        return;
    }
    let before = std::mem::replace(&mut s.queue_rows, rest);
    if !persist_queue(s) {
        s.queue_rows = before;
        // A failed removal consumed the button press, not the row. Fixing the disk must not send it.
        s.queue_send_now = None;
        if let Some(front) = s.queue_rows.iter_mut().find(|row| row.key == key) {
            front.held = Some(QUEUED_SEND_NOT_ACKNOWLEDGED.into());
        }
        s.draft.replace_text(draft);
        return;
    }
    // Remove durably BEFORE dispatch. An explicit Send that wasn't acknowledged is offered back held.
    // A fresh automatic send explicitly refused as busy still uses the existing retry backoff.
    s.queue_send_now = None;
    if forced {
        going.held = Some(QUEUED_SEND_NOT_ACKNOWLEDGED.into());
    }
    s.queue_in_flight = Some(going);
    send(s, tx);
    if s.flight.is_none() {
        s.queue_in_flight = None;
    }
    s.draft.replace_text(draft);
}

#[cfg(test)]
#[path = "queued_busy_waits_tests.rs"]
mod queued_busy_waits_tests;

#[cfg(test)]
#[path = "mode_cycle_tests.rs"]
mod mode_cycle_tests;

#[cfg(test)]
#[path = "composer_counts_tests.rs"]
mod composer_counts_tests;

#[cfg(test)]
#[path = "queued_conversation_owner_tests.rs"]
mod queued_conversation_owner_tests;

#[cfg(test)]
#[path = "queue_durability_tests.rs"]
mod queue_durability_tests;

#[cfg(test)]
#[path = "blockers1_pdf_attach_tests.rs"]
mod blockers1_pdf_attach_tests;
