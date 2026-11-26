use super::*;
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub tabs: Vec<terminal::Terminal>,
    pub active: usize,
    pub width: f32,
    pub error: Option<String>,
    pub commands: Vec<Entry>,
    pub preview: Option<vt100::Parser>,
    pub catchup: Latest<Result<usize, String>>,
    /// The turn the last catch-up was asked from, whether the Catch up button asked it, and whether the window had
    /// focus at the last poll (App.tsx 0.391: asked when a conversation is shown, and again on focus).
    pub asked: Option<String>,
    pub asked_by_hand: bool,
    pub was_focused: bool,
    /// How many catch-ups the window asked for on its own.
    pub asks: u32,
    pub selection: Option<(usize, usize)>,
    pub cols: usize,
    pub origin: [f32; 2],
}
pub(super) struct Entry {
    key: String,
    command: String,
    running: Option<own_command::Running>,
    result: Option<own_command::ResultRow>,
    carried: bool,
}
fn key(s: &State) -> String {
    conversation_draft_key(s).unwrap_or_default()
}
fn folder(s: &State) -> Option<std::path::PathBuf> {
    let (teammate, _) = selected_conversation(s)?;
    // Same host resolution the engine uses for turns (teammate → workspace.json → ~/Documents/Locust).
    let electron = s
        .data
        .as_ref()
        .map(|store| store.electron().to_path_buf())
        .or_else(|| native::store().map(|store| store.electron().to_path_buf()))
        .or_else(engine::default_data_folder)?;
    native::folder_for(teammate, &electron).ok()
}
fn add(s: &mut State) {
    if s.terminal.tabs.len() >= 8 {
        s.terminal.error = Some("Close a terminal tab before opening another.".into());
        return;
    }
    let result = folder(s)
        .ok_or_else(|| "Choose a project folder first. Your terminals are unchanged.".to_owned())
        .and_then(|p| terminal::Terminal::open(&p, &terminal::shell(), &["-l"]));
    match result {
        Ok(t) => {
            s.terminal.tabs.push(t);
            s.terminal.active = s.terminal.tabs.len() - 1;
            s.terminal.error = None;
        }
        Err(e) => s.terminal.error = Some(e),
    }
}
/// TerminalToggle.tsx: the terminal glyph in the toolbar.
pub(super) fn toggle(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let refusal = locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "terminal");
    let open = s.terminal.open && refusal.is_none();
    crate::look::controls::head_toggle(&crate::look::icons::TERMINAL, "Terminal", "Terminal (Ctrl+`)", open, refusal.as_deref())
        .on_press(toggle_open)
        .build(l.k)
        .key("terminal-toggle")
        .checked(open)
}
/// The toggle's press, and Ctrl+` from anywhere in the window (App.tsx `toggleTerminal`): nothing while the
/// organization has turned the terminal off.
pub(super) fn toggle_open(s: &mut State) {
    if locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "terminal").is_some() {
        return;
    }
    s.terminal.open = !s.terminal.open;
    if s.terminal.open && s.terminal.tabs.is_empty() {
        add(s);
    }
}
/// terminalKeys.ts `isTerminalToggle`: Ctrl and the backquote key (` or, shifted, ~) on every platform; never with Alt,
/// or with Command or the Windows key.
pub(super) fn is_toggle_key(event: &KeyEvent) -> bool {
    let backquote = match &event.key {
        Key::Command(c) => matches!(c, '`' | '~'),
        Key::Text(t) => t == "`" || t == "~",
        _ => false,
    };
    backquote && event.mods.ctrl && !event.mods.alt && !event.mods.logo
}
pub(super) fn run_command(s: &mut State) {
    let command = own_command::bang(&s.draft.text)
        .unwrap_or_default()
        .to_owned();
    let folder = folder(s);
    let result = own_command::validate(&command, folder.as_deref())
        .and_then(|_| {
            folder
                .as_deref()
                .ok_or_else(|| "Choose a folder first. Nothing was run.".to_owned())
        })
        .and_then(|folder| own_command::Running::start(&command, folder));
    match result {
        Ok(r) => {
            s.terminal.commands.push(Entry {
                key: key(s),
                command,
                running: Some(r),
                result: None,
                carried: false,
            });
            s.draft.clear();
        }
        Err(e) => s.composer_note = Some(e),
    }
}
/// End every tab and bang command (Electron `createTerminalHost(...).dispose`).
pub(super) fn dispose(s: &mut State) {
    for entry in &mut s.terminal.commands {
        if let Some(running) = &mut entry.running {
            running.stop();
        }
    }
    s.terminal.commands.clear();
    s.terminal.tabs.clear();
    s.terminal.open = false;
    s.terminal.error = None;
    s.terminal.selection = None;
    s.terminal.preview = None;
}

/// The settled conversation on screen whose terminal can be read back (App.tsx `catchUpId`): its newest turn, on
/// Claude Code or Codex, not running.
pub(super) fn catch_up_id(s: &State) -> Option<String> {
    let pane = s.ledger.as_ref()?;
    let latest = pane.snapshot.turn_metas.last()?;
    (!turn_running(s) && matches!(latest.runtime.as_str(), "claude" | "codex") && !pane.snapshot.tip.is_empty())
        .then(|| pane.snapshot.tip.clone())
}

/// WHAT HAPPENED IN THE TERMINAL COMES BACK (App.tsx 0.391): what the person did in the terminal is asked for when the
/// conversation is shown, and again whenever the window comes back into focus on it, so the promise the terminal
/// offer makes ("What you do there comes back into this conversation") is kept without a press.
fn ask_terminal_back(s: &mut State) {
    let focus_back = s.window_focused && !s.terminal.was_focused;
    s.terminal.was_focused = s.window_focused;
    let Some(id) = catch_up_id(s) else {
        return;
    };
    if (s.terminal.asked.as_deref() != Some(id.as_str()) || focus_back) && !s.terminal.catchup.is_pending() {
        s.terminal.asked = Some(id.clone());
        s.terminal.asked_by_hand = false;
        s.terminal.asks += 1;
        s.terminal.catchup.request(id, |id| runtime_terminal::catch_up(&id));
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = s.terminal.open;
    ask_terminal_back(s);
    if let Some(result) = s.terminal.catchup.take() {
        // Asked on its own, it speaks only when something came back.
        let by_hand = std::mem::take(&mut s.terminal.asked_by_hand);
        match result {
            Ok(0) | Err(_) if !by_hand => {}
            Ok(n) => s.composer_note = Some(format!("Brought back {n} terminal turns.")),
            Err(e) => s.composer_note = Some(e),
        }
        changed = true;
    }
    for t in &mut s.terminal.tabs {
        if let Err(e) = t.poll() {
            s.terminal.error = Some(e);
        }
    }
    for e in &mut s.terminal.commands {
        if let Some(r) = &mut e.running {
            match r.poll() {
                Ok(Some(result)) => {
                    e.result = Some(result);
                    e.running = None;
                    changed = true;
                }
                Err(message) => s.terminal.error = Some(message),
                _ => {}
            }
        }
    }
    changed
}
/// Electron keeps these entries locally and carries finished output with the next message once.
pub(super) fn carry(s: &mut State, prompt: String) -> String {
    let key = key(s);
    let mut blocks = vec![];
    for e in &mut s.terminal.commands {
        if e.key != key || e.carried {
            continue;
        }
        if let Some(r) = &e.result {
            let how = if r.stopped {
                "stopped before it finished".into()
            } else {
                r.exit
                    .map(|c| format!("exit {c}"))
                    .unwrap_or_else(|| "ended".into())
            };
            blocks.push(format!(
                "The person's command and its output ({how}):\n!{}\n{}{}",
                e.command,
                if r.output.is_empty() {
                    "(no output)"
                } else {
                    &r.output
                },
                r.omitted
                    .as_ref()
                    .map(|n| format!("\n{n}"))
                    .unwrap_or_default()
            ));
            e.carried = true;
        }
    }
    if blocks.is_empty() {
        prompt
    } else {
        format!("{}\n\n{prompt}", blocks.join("\n\n"))
    }
}
pub(super) fn commands(s: &State, p: &Palette) -> El {
    let mut out = col()
        .w_full()
        .max_h(230.)
        .scroll(ScrollSpec { stick_bottom: true })
        .gap(5.)
        .px(16.);
    if own_command::bang(&s.draft.text).is_some() {
        out = out.child(
            text(own_command::MODE_NOTE)
                .font(type_::LABEL)
                .color(p.amber)
                .w_full()
                .wrap()
                .role("status"),
        );
    }
    for (i, e) in s
        .terminal
        .commands
        .iter()
        .enumerate()
        .filter(|(_, e)| e.key == key(s))
    {
        let mut entry = col().w_full().gap(4.).child(
            text(format!("!{}", e.command))
                .font(type_::LABEL)
                .color(p.text)
                .w_full()
                .wrap()
                .selectable(true),
        );
        if let Some(r) = &e.result {
            entry = entry.child(
                text(r.output.clone())
                    .font(mono())
                    .color(p.secondary)
                    .w_full()
                    .wrap()
                    .selectable(true),
            );
            if let Some(n) = &r.omitted {
                entry = entry.child(text(n.clone()).color(p.muted).w_full().wrap());
            }
            let status = if r.stopped {
                "Stopped".into()
            } else {
                r.exit
                    .map(|c| format!("Exit {c}"))
                    .unwrap_or_else(|| "Ended".into())
            };
            entry = entry.child(
                text(format!("{status} · {}", ledger::duration_text(r.elapsed)))
                    .font(type_::LABEL)
                    .color(p.muted),
            );
        } else {
            entry = entry.child(
                row().gap(8.).child(text("Running").color(p.amber)).child(
                    text("Stop")
                        .color(p.secondary)
                        .key(format!("command-stop-{i}"))
                        .role("button")
                        .label("Stop this command")
                        .on_press(move |s: &mut State| {
                            if let Some(r) = s
                                .terminal
                                .commands
                                .get_mut(i)
                                .and_then(|e| e.running.as_mut())
                            {
                                r.stop();
                            }
                        }),
                ),
            );
        }
        out = out.child(entry);
    }
    out
}
fn mono() -> rune::TextStyle {
    rune::TextStyle {
        family: rune::Family::SystemMono,
        line_height: 16.,
        ..rune::TextStyle::new(12., 400.)
    }
}
fn ansi(c: vt100::Color, default: Color) -> Color {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => color(&format!("#{r:02x}{g:02x}{b:02x}")),
        vt100::Color::Idx(n) => {
            let table = [
                "#17191a", "#e4685d", "#7fc97f", "#e9b949", "#7799dd", "#b6a1e8", "#63c5c7",
                "#c7c3bc", "#8b9190", "#ff8877", "#a0e890", "#ffdd77", "#a0bfff", "#d9b5ff",
                "#8ce9eb", "#ffffff",
            ];
            if n < 16 {
                color(table[n as usize])
            } else if n >= 232 {
                let v = 8 + (n - 232) * 10;
                color(&format!("#{v:02x}{v:02x}{v:02x}"))
            } else {
                let n = n - 16;
                let f = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                let (r, g, b) = (f(n / 36), f(n / 6 % 6), f(n % 6));
                color(&format!("#{r:02x}{g:02x}{b:02x}"))
            }
        }
    }
}
fn grid(parser: &vt100::Parser, p: &Palette, selection: Option<(usize, usize)>) -> El {
    let screen = parser.screen();
    let (rows, cols) = screen.size();
    let mut out = col().gap(0.);
    for y in 0..rows.get() {
        let mut line = row().gap(0.).h(16.);
        for x in 0..cols.get() {
            if let Some(c) = screen.cell(y, x) {
                if c.is_wide_continuation() {
                    continue;
                }
                let mut fg = ansi(c.fgcolor(), p.text);
                let mut bg = ansi(c.bgcolor(), p.field);
                if c.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if selection.is_some_and(|(a, b)| {
                    (a.min(b)..=a.max(b)).contains(&(y as usize * cols.get() as usize + x as usize))
                }) {
                    bg = p.selected;
                }
                if !screen.hide_cursor() && screen.cursor_position() == (y, x) {
                    bg = p.secondary;
                    fg = p.field;
                }
                let style = rune::TextStyle {
                    weight: if c.bold() { 700. } else { 400. },
                    italic: c.italic(),
                    ..mono()
                };
                line = line.child(
                    text(if c.contents().is_empty() {
                        " ".into()
                    } else {
                        c.contents().to_owned()
                    })
                    .font(style)
                    .color(fg)
                    .fill(bg)
                    .w(if c.is_wide() { 14.4 } else { 7.2 })
                    .h(16.),
                );
            }
        }
        out = out.child(line);
    }
    out
}
fn input(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    // Ctrl+` is the window's, to close the terminal it is typing in; the shell never sees it.
    if is_toggle_key(event) {
        return false;
    }
    let Some(t) = s.terminal.tabs.get_mut(s.terminal.active) else {
        return false;
    };
    if event.is_command('c') {
        let parser = crate::invariants::lock(&t.screen);
        let screen = parser.screen();
        let (_, cols) = screen.size();
        let words = if let Some((a, b)) = s.terminal.selection {
            let mut out = String::new();
            for pos in a.min(b)..=a.max(b) {
                if pos > a.min(b) && pos % cols.get() as usize == 0 {
                    out.push('\n');
                }
                if let Some(c) = screen.cell(
                    (pos / cols.get() as usize) as u16,
                    (pos % cols.get() as usize) as u16,
                ) {
                    out.push_str(if c.contents().is_empty() {
                        " "
                    } else {
                        c.contents()
                    });
                }
            }
            out
        } else {
            screen.contents()
        };
        tx.copy(words);
        return true;
    }
    // Paste (Cmd+V, Ctrl+V off macOS) reads the clipboard and gives the shell all of it in one write. Rune hands an
    // element's own `on_paste` nothing unless it is the window's root, so the terminal asks for the clipboard itself.
    if event.is_command('v') {
        tx.read_clipboard(|s: &mut State, content: &rune::ClipboardContent, _: &mut Tx| paste_into(s, content.text()));
        return true;
    }
    if matches!(event.key, Key::PageUp | Key::PageDown) {
        let mut parser = crate::invariants::lock(&t.screen);
        let offset = parser.screen().scrollback();
        parser
            .screen_mut()
            .set_scrollback(if event.key == Key::PageUp {
                offset + 20
            } else {
                offset.saturating_sub(20)
            });
        return true;
    }
    let data = match &event.key {
        Key::Text(v) => {
            if event.mods.ctrl && v.len() == 1 {
                vec![v.as_bytes()[0] & 31]
            } else {
                v.as_bytes().to_vec()
            }
        }
        Key::Enter => b"\r".to_vec(),
        Key::Backspace => vec![127],
        Key::Tab => vec![9],
        Key::Escape => vec![27],
        Key::Up => b"\x1b[A".to_vec(),
        Key::Down => b"\x1b[B".to_vec(),
        Key::Right => b"\x1b[C".to_vec(),
        Key::Left => b"\x1b[D".to_vec(),
        _ => return false,
    };
    if let Err(e) = t.input(&data) {
        s.terminal.error = Some(e);
    }
    true
}
/// What was pasted, to the open tab's shell as typed, bracketed when the program there asked for it.
fn paste_into(s: &mut State, data: &str) {
    if let Some(t) = s.terminal.tabs.get_mut(s.terminal.active) {
        let bracketed = crate::invariants::lock(&t.screen).screen().bracketed_paste();
        let data = if bracketed { format!("\x1b[200~{data}\x1b[201~") } else { data.to_owned() };
        if let Err(e) = t.input(data.as_bytes()) {
            s.terminal.error = Some(e);
        }
    }
}
pub(super) fn view(s: &State, p: &Palette) -> El {
    let mut tabs = row().gap(5.);
    for (i, t) in s.terminal.tabs.iter().enumerate() {
        tabs = tabs
            .child(
                text(
                    t.folder
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                )
                .color(if i == s.terminal.active {
                    p.text
                } else {
                    p.muted
                })
                .on_press(move |s: &mut State| s.terminal.active = i),
            )
            .child(text("×").color(p.muted).on_press(move |s: &mut State| {
                s.terminal.tabs.remove(i);
                s.terminal.active = s
                    .terminal
                    .active
                    .min(s.terminal.tabs.len().saturating_sub(1));
            }));
    }
    let mut body = col()
        .key("terminal-screen")
        .grow(1.)
        .min_h(0.)
        .focusable(true)
        .on_key(input)
        .on_drag(|s: &mut State, d: &mut rune::Drag, _: &mut Tx| {
            let point = d.window_point();
            let x = ((point[0] - s.terminal.origin[0]) / 7.2).max(0.) as usize;
            let y = ((point[1] - s.terminal.origin[1]) / 16.).max(0.) as usize;
            let pos = y * s.terminal.cols.max(1) + x.min(s.terminal.cols.saturating_sub(1));
            if d.phase == rune::Phase::Down {
                s.terminal.selection = Some((pos, pos));
            } else if let Some((a, _)) = s.terminal.selection {
                s.terminal.selection = Some((a, pos));
            }
        })
        .on_layout(|s: &mut State, [x, y, w, h]| {
            s.terminal.origin = [x, y];
            s.terminal.cols = (w / 7.2) as usize;
            if let Some(t) = s.terminal.tabs.get_mut(s.terminal.active)
                && let Err(e) = t.resize((w / 7.2) as u16, (h / 16.) as u16)
            {
                s.terminal.error = Some(e);
            }
        });
    if let Some(t) = s.terminal.tabs.get(s.terminal.active) {
        body = body.child(grid(
            &crate::invariants::lock(&t.screen),
            p,
            s.terminal.selection,
        ));
        if let Some(code) = t.exit {
            body = body.child(text(format!("Process exited with code {code}.")).color(p.muted));
        }
    } else if let Some(parser) = &s.terminal.preview {
        body = body.child(grid(parser, p, None));
    } else {
        body = body.child(
            text("Open a terminal with +. New tabs use the conversation’s folder.")
                .color(p.muted)
                .w_full()
                .wrap(),
        );
    }
    let mut panel = col()
        .w(if s.terminal.width > 0. {
            s.terminal.width
        } else {
            380.
        })
        // In a narrow window the terminal gives way before the conversation does (it squeezed the message box
        // to "Messag Codec.." in Colin's 11/10 frame): it may shrink to 220 px, the drag handle's own minimum.
        .shrink()
        .min_w(220.)
        .h_full()
        .fill(p.field)
        .padding(8.)
        .gap(8.)
        .child(
            row()
                .gap(12.)
                .child(text("Terminal").color(p.text))
                .child(text("+").color(p.text).on_press(add))
                .child(
                    text("Hide terminal")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(|s: &mut State| s.terminal.open = false),
                ),
        )
        .child(tabs);
    if let Some(error) = &s.terminal.error {
        panel = panel.child(text(error.clone()).color(p.red).w_full().wrap());
    }
    row()
        .h_full()
        .child(
            el().w(5.)
                .h_full()
                .fill(p.hairline)
                .cursor(rune::Cursor::ResizeColumn)
                .on_drag(
                    |s: &mut State, d: &mut rune::Drag, _: &mut Tx| match d.phase {
                        rune::Phase::Down => {
                            d.data = if s.terminal.width > 0. {
                                s.terminal.width
                            } else {
                                380.
                            }
                        }
                        rune::Phase::Move => s.terminal.width = (d.data - d.dx).clamp(220., 900.),
                        _ => {}
                    },
                ),
        )
        .child(panel.child(body))
}
pub(super) fn scene(s: &mut State) {
    composer_base_scene(s, false);
    s.terminal.open = true;
    let mut p = terminal::parser(24, 43);
    p.process(b"$ printf 'hello'\r\n\x1b[32mhello\x1b[0m\r\n$ ");
    s.terminal.preview = Some(p);
    s.terminal.commands.push(Entry {
        key: key(s),
        command: "printf hello".into(),
        running: None,
        result: Some(own_command::ResultRow {
            output: "hello".into(),
            omitted: None,
            exit: Some(0),
            stopped: false,
            elapsed: 45,
        }),
        carried: false,
    });
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

pub(super) fn runtime_controls(s: &State, p: &Palette) -> El {
    let Some(pane) = &s.ledger else {
        return row();
    };
    let id = pane.snapshot.tip.clone();
    // TerminalDivider.tsx's clock, as an icon button. Open in terminal is the More actions menu's (0.797: the header is
    // a row of icons), `header_menu::items`.
    let l = crate::look::controls::Look::new(p.dark);
    row().gap(2.).child(crate::look::controls::head_icon(&crate::look::icons::CLOCK, "Catch up terminal", "Catch up on what happened in the terminal").on_press(move|s:&mut State|{if !turn_running(s)&&!s.terminal.catchup.is_pending(){s.terminal.asked_by_hand=true;s.terminal.catchup.request(id.clone(),|id|runtime_terminal::catch_up(&id));}}).build(l.k))
}

/// What opening the conversation in a terminal offers (OpenInTerminal.tsx `TerminalOffer`): whose interface opens, the
/// hover saying what it means, and why it cannot be pressed now, when it cannot.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TerminalOffer {
    pub runtime_name: String,
    pub label: String,
    pub title: String,
    pub disabled: Option<String>,
    comes_back: bool,
}

impl TerminalOffer {
    /// What is said once it opened in `place`: whether what happens there comes back.
    pub(super) fn opened(&self, place: &str) -> String {
        if self.comes_back {
            format!("Opened in {place}. What you do there comes back here when you return.")
        } else {
            format!("Opened in {place}. Locust won't see what happens there.")
        }
    }
}

/// The offer for a conversation (OpenInTerminal.tsx `terminalOffer`): none where there is no terminal to open -- no
/// turn yet, a runtime with no interface of its own to resume in (Antigravity only through its CLI), or a model of the
/// person's own, which runs on settings only Locust hands OpenCode.
pub(super) fn terminal_offer(
    runtime: Option<&str>,
    model: Option<&str>,
    mission_id: Option<&str>,
    running: bool,
    teammate_name: Option<&str>,
    antigravity_cli: bool,
) -> Option<TerminalOffer> {
    const RESUMABLE: &[&str] = &["codex", "claude", "copilot", "cursor", "opencode", "muse"];
    const COMES_BACK: &[&str] = &["codex", "claude"];
    mission_id?;
    let runtime = runtime.filter(|r| locust_core::route_at_start::is_mission_runtime(r))?;
    if !RESUMABLE.contains(&runtime) && !(runtime == "antigravity" && antigravity_cli) {
        return None;
    }
    if model.is_some_and(locust_core::own_models::is_own_route_model) {
        return None;
    }
    let runtime_name = locust_core::inspector::runtime_display_name(runtime).to_owned();
    let whose = teammate_name.map_or_else(|| "this folder".to_owned(), |name| format!("{name}'s folder"));
    let comes_back = COMES_BACK.contains(&runtime);
    let afterwards = if comes_back { "What you do there comes back into this conversation" } else { "Locust won't see what you do there" };
    Some(TerminalOffer {
        label: format!("Open in {runtime_name}, in a terminal"),
        title: format!(
            "Open in {runtime_name}, in a terminal\nThe same session, in {whose}. {afterwards}, and it runs with {runtime_name}'s own permissions, not this conversation's mode."
        ),
        disabled: running.then(|| {
            format!("Available when {} finishes: Locust's run and a terminal can't share one session at once.", teammate_name.unwrap_or("this run"))
        }),
        runtime_name,
        comes_back,
    })
}

/// The offer for the conversation on screen (App.tsx `shownTerminal`), from its latest turn.
pub(super) fn shown_terminal(s: &State) -> Option<TerminalOffer> {
    let pane = s.ledger.as_ref()?;
    let latest = pane.snapshot.turn_metas.last();
    let mission_id = latest.map(|m| m.mission_id.as_str()).filter(|id| !id.is_empty());
    let runtime = latest.map(|m| m.runtime.as_str()).filter(|r| !r.is_empty());
    let model = latest.map(|m| m.model.as_str()).filter(|m| !m.is_empty());
    let name = selected_conversation(s).map(|(t, _)| t.name.clone());
    terminal_offer(runtime, model, mission_id, turn_running(s), name.as_deref(), false)
}

/// Open in terminal, from the More actions menu: the conversation's session resumed in its runtime's own interface.
/// Whether it opens, and the reason when it does not, is Electron's `terminalRequestFor`
/// (`runtime_terminal::terminal_request_for`), over what this window knows of the conversation.
pub(super) fn open_in_terminal(s: &mut State) {
    let Some(open_id) = s.ledger.as_ref().map(|pane| pane.snapshot.tip.clone()) else { return };
    let offer = shown_terminal(s);
    let running = turn_running(s);
    let owner = selected_conversation(s).map(|(t, _)| (t.id.clone(), t.name.clone()));
    let place = folder(s).map(|f| f.to_string_lossy().into_owned());
    // The conversation's newest run, read from the ledger once, and only when it is asked for.
    let loaded = std::cell::OnceCell::new();
    let ledger = || loaded.get_or_init(|| native::real_tip(&open_id).and_then(|newest| runtime_terminal::mission(&newest)).ok()).as_ref();
    let result = (|| {
        let request = runtime_terminal::terminal_request_for(
            &open_id,
            &runtime_terminal::TerminalFacts {
                live: &|_| running,
                mission: &|_| {
                    let (m, rows) = ledger()?;
                    Some(runtime_terminal::TerminalMission {
                        runtime: m["runtime"].as_str()?.to_owned(),
                        model: m["model"].as_str().unwrap_or_default().to_owned(),
                        session: runtime_terminal::session(rows),
                    })
                },
                owner_of: &|_| owner.clone(),
                // A teammate's runs happen in its folder, as the engine resolves it for a turn.
                cwd_for: &|_| place.clone(),
                workspace_path: place.clone().unwrap_or_default(),
                launch_for: &|runtime| {
                    let command = match runtime {
                        "cursor" => "cursor-agent",
                        "antigravity" => "agy",
                        other => other,
                    };
                    let found = crate::runtime::find_executable(command).ok()?;
                    Some(runtime_terminal::TerminalLaunch {
                        command_name: command.to_owned(),
                        executable_path: found.to_string_lossy().into_owned(),
                        prefix_args: Vec::new(),
                        env: None,
                    })
                },
            },
        )?;
        if place.is_none() {
            return Err("Choose a folder first. Nothing was run.".to_owned());
        }
        let args = runtime_terminal::resume_args(&request.runtime, &request.session_id)
            .ok_or("This conversation has no session its runtime can resume in a terminal.")?;
        let store = native::store().ok_or("Native data folder is unavailable.")?;
        let account = ledger().and_then(|(m, _)| m["resolvedRouteId"].as_str()).and_then(locust_core::agent_accounts::account_from_route);
        let env = native::account_launch_env(&store, &request.runtime, account)?;
        let env_pairs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let dir = store.attachments_dir().map_err(|e| e.to_string())?;
        let script = dir.join(format!("terminal-{}.command", native::uuid_v4().map_err(|e| e.to_string())?));
        let file = std::path::Path::new(&request.launch.executable_path);
        runtime_terminal::open_with_env(file, &args, std::path::Path::new(&request.cwd), &script, &store, &env_pairs)
    })();
    s.composer_note = Some(match result {
        Ok(()) => offer.as_ref().map(|o| o.opened("Terminal")).unwrap_or_else(|| "Opened in Terminal.".into()),
        Err(e) => e,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_command_carries_once_and_stays_in_its_conversation() {
        let mut s = State::default();
        scene(&mut s);
        let prompt = carry(&mut s, "next".into());
        assert_eq!(
            prompt,
            "The person's command and its output (exit 0):\n!printf hello\nhello\n\nnext"
        );
        assert_eq!(carry(&mut s, "again".into()), "again");
        s.terminal.commands[0].carried = false;
        s.terminal.commands[0].key = "another".into();
        assert_eq!(carry(&mut s, "here".into()), "here");
    }
    #[test]
    fn command_mode_escapes_without_sending() {
        let mut s = State::default();
        s.draft.replace_text("!printf hi");
        let mut tx = Tx::default();
        let event = KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        };
        assert!(composer_input_key(&mut s, &event, &mut tx));
        assert_eq!(s.draft.text, "printf hi");
    }
    #[test]
    fn dispose_clears_tabs_and_commands() {
        let mut s = State::default();
        scene(&mut s);
        assert!(s.terminal.open);
        assert!(!s.terminal.commands.is_empty());
        dispose(&mut s);
        assert!(!s.terminal.open);
        assert!(s.terminal.tabs.is_empty());
        assert!(s.terminal.commands.is_empty());
        assert!(s.terminal.preview.is_none());
    }
}

#[cfg(test)]
#[path = "head_icons_tests.rs"]
mod head_icons_tests;

#[cfg(test)]
#[path = "terminal_first_tests.rs"]
mod terminal_first_tests;

#[cfg(test)]
#[path = "command_mode_tests.rs"]
mod command_mode_tests;

#[cfg(test)]
#[path = "terminal_offer_tests.rs"]
mod terminal_offer_tests;

#[cfg(test)]
#[path = "terminal_request_tests.rs"]
mod terminal_request_tests;

#[cfg(test)]
#[path = "command_carry_tests.rs"]
mod command_carry_tests;

#[cfg(test)]
#[path = "terminal_comes_back_tests.rs"]
mod terminal_comes_back_tests;
