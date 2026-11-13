use super::*;

pub(super) fn restore_failed_draft(s: &mut State, message: String) {
    // Fix: restore only failed sends; never overwrite text typed after dispatch.
    let text = if s.draft.text.is_empty() {
        message
    } else {
        format!("{message}\n\n{}", s.draft.text)
    };
    s.draft.replace_text(text);
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
        s.appearance = crate::workspace_settings::load_theme(&store).to_appearance();
        if let Ok(settings) = crate::workspace_settings::load(&store) {
            s.layout_pref = settings.layout;
        }
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
    });
    if !persist_queue(s) {
        s.queue_rows.pop();
        return false;
    }
    s.off_the_queue = false;
    true
}

pub(super) fn composer_placeholder(s: &State) -> String {
    if native_blocked(s)
        && let Some((teammate, _)) = selected_conversation(s)
    {
        return format!("{} runs in Locust for now", teammate.name);
    }
    let teammate_name = selected_conversation(s).map(|(t, _)| t.name.as_str());
    composer::placeholder(composer::PlaceholderInput {
        working_now: turn_running(s),
        working_name: &working_name(s),
        queued: queue_front(s).is_some(),
        route_can_run: s.route_can_run && !native_blocked(s),
        selected_ready: s.selected_ready && !s.nothing_installed,
        selected_display: None,
        teammate_name,
        discovery_loading: false,
        discovery_error: false,
        ready_elsewhere: None,
        auth_required: false,
        installed_silent: 0,
        checking: false,
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
        runtime_ready: s.selected_ready && !native_blocked(s),
        route_can_run: s.route_can_run,
        busy: s.team.is_pending() || s.opening_teammate.is_some(),
        empty: s.draft.text.trim().is_empty() && s.attached.is_empty(),
    })
}

pub(super) fn slash_choices_now(s: &State) -> Vec<composer::SlashCommand> {
    if s.scene_popup == ComposerScenePopup::Slash {
        return composer::matching_commands(
            "",
            &composer::available_commands(composer::AvailableInput {
                modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
                running: turn_running(s),
                can_swarm: false,
                issues: false,
                tune: false,
                setup: true,
                runtime_commands: &[],
            }),
        );
    }
    let Some(query) = composer::slash_query(&s.draft.text) else {
        return Vec::new();
    };
    let available = composer::available_commands(composer::AvailableInput {
        modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
        running: turn_running(s),
        can_swarm: false,
        issues: false,
        tune: false,
        setup: true,
        runtime_commands: &[],
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
            if !s.tagged.contains(&id) && s.tagged.len() < 5 {
                s.tagged.push(id);
            }
        }
        AtChoice::File { path } => {
            let (next, note) = composer::attach_paths(&s.attached, &[path]);
            s.attached = next;
            if let Some(note) = note {
                s.composer_note = Some(note);
            }
        }
    }
}

pub(super) fn run_slash_command(s: &mut State, command: &composer::SlashCommand) {
    s.slash_at = 0;
    match &command.action {
        composer::SlashAction::Mode(mode) => {
            s.mode = (*mode).to_owned();
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
        composer::SlashAction::Route
        | composer::SlashAction::Swarm
        | composer::SlashAction::Issues
        | composer::SlashAction::Tune => {
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
    let (next, note) = composer::attach_paths(&s.attached, &as_str);
    s.attached = next;
    if let Some(note) = note {
        s.composer_note = Some(note);
    }
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
    if s.draft.text.starts_with('!') && event.key == Key::Escape {
        s.draft.replace_text(s.draft.text[1..].to_owned());
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
            let usable = ["ask", "plan", "accept-edits", "approve-each", "auto"];
            if let Some(next) = composer::shift_tab_mode(&s.mode, &usable) {
                s.mode = next;
            }
            true
        }
    }
}

pub(super) fn submit_composer(s: &mut State, tx: &mut Tx, send_now: bool) {
    if s.demo {
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
    if typed.is_empty() && s.attached.is_empty() {
        if let Some(why) = send_blocked_now(s) {
            s.composer_note = Some(why.to_owned());
        }
        return;
    }
    if own_command::bang(&s.draft.text).is_some() {
        terminal_panel::run_command(s);
        return;
    }
    if turn_running(s) {
        if queue_front(s).is_some() || !enqueue_follow_up(s, typed.clone(), s.attached.clone()) {
            return;
        }
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
        } else if s.nothing_installed || !s.selected_ready {
            s.composer_note = Some(
                "No AI agent on this machine can start yet. Settings lists what to install.".into(),
            );
        }
        return;
    }
    if s.sent_history.last().map(String::as_str) != Some(typed.as_str()) && !typed.is_empty() {
        s.sent_history.push(typed.clone());
        if s.sent_history.len() > 50 {
            s.sent_history.remove(0);
        }
    }
    s.recall_at = -1;
    s.draft
        .replace_text(composer::with_attachments(&typed, &s.attached));
    s.attached.clear();
    s.tagged.clear();
    s.off_the_queue = false;
    send(s, tx);
}

pub(super) fn queue_phase(s: &State) -> &'static str {
    if let Some(pane) = &s.ledger {
        return pane.worker.as_ref().map_or("running", |w| w.tip_phase());
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
    let (going, rest) = composer::take_next(&s.queue_rows, &key);
    let Some(going) = going else {
        return;
    };
    let draft = s.draft.text.clone();
    s.draft
        .replace_text(composer::with_attachments(&going.text, &going.attachments));
    if !can_send(s) {
        s.draft.replace_text(draft);
        return;
    }
    let before = std::mem::replace(&mut s.queue_rows, rest);
    if !persist_queue(s) {
        s.queue_rows = before;
        s.draft.replace_text(draft);
        return;
    }
    // Remove durably BEFORE dispatch. An uncertain transport result must never retry itself.
    s.queue_send_now = None;
    send(s, tx);
    s.draft.replace_text(draft);
}
