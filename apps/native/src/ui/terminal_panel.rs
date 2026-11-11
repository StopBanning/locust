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
pub(super) fn toggle(p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    crate::look::controls::head_icon(&crate::look::icons::TERMINAL, "Terminal", "Terminal (Ctrl+`)")
        .on_press(|s: &mut State| {
            s.terminal.open = !s.terminal.open;
            if s.terminal.open && s.terminal.tabs.is_empty() {
                add(s);
            }
        })
        .build(l.k)
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

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = s.terminal.open;
    if let Some(result) = s.terminal.catchup.take() {
        s.composer_note = Some(match result {
            Ok(n) => format!("Brought back {n} terminal turns."),
            Err(e) => e,
        });
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
                .wrap(),
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
        .on_paste(|s: &mut State, data: &str, _: &mut Tx| {
            if let Some(t) = s.terminal.tabs.get_mut(s.terminal.active) {
                let bracketed = crate::invariants::lock(&t.screen)
                    .screen()
                    .bracketed_paste();
                let data = if bracketed {
                    format!("\x1b[200~{data}\x1b[201~")
                } else {
                    data.to_owned()
                };
                if let Err(e) = t.input(data.as_bytes()) {
                    s.terminal.error = Some(e);
                }
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
    let open_id = id.clone();
    // OpenInTerminal.tsx's code glyph, and the catch-up beside it (TerminalDivider.tsx's clock), as icon buttons.
    let l = crate::look::controls::Look::new(p.dark);
    let _ = p;
    row().gap(2.).child(crate::look::controls::head_icon(&crate::look::icons::CODE, "Open in terminal", "Open this conversation in a terminal").on_press(move|s:&mut State|{
 if turn_running(s){s.composer_note=Some("This conversation is still running. Open it in a terminal once the run finishes: two programs on one session at once would talk over each other.".into());return;}
 let result=(||{let newest=native::real_tip(&open_id)?;let(m,rows)=runtime_terminal::mission(&newest)?;let runtime=m["runtime"].as_str().ok_or("That conversation is not in the ledger.")?;if m["model"].as_str().is_some_and(|m|m.starts_with("own-")){return Err("A model of your own runs on settings only Locust gives OpenCode, so a terminal could not run it. Keep this conversation here.".into());}let session=runtime_terminal::session(&rows).ok_or("This conversation has no session its runtime can resume in a terminal.")?;let args=runtime_terminal::resume_args(runtime,&session).ok_or("This conversation has no session its runtime can resume in a terminal.")?;let executable=crate::runtime::find_executable(match runtime {"cursor"=>"cursor-agent","antigravity"=>"agy",other=>other}).map_err(|e|e.to_string())?;let folder=folder(s).ok_or("Choose a folder first. Nothing was run.")?;let store=native::store().ok_or("Native data folder is unavailable.")?;let dir=store.attachments_dir().map_err(|e|e.to_string())?;let script=dir.join(format!("terminal-{}.command",native::uuid_v4().map_err(|e|e.to_string())?));runtime_terminal::open(&executable,&args,&folder,&script,&store)?;Ok::<_,String>(matches!(runtime,"claude"|"codex"))})();
 s.composer_note=Some(match result{Ok(true)=>"Opened in Terminal. Use Catch up terminal when you return.".into(),Ok(false)=>"Opened in Terminal. Locust won't see what happens there.".into(),Err(e)=>e});
 }).build(l.k)).child(crate::look::controls::head_icon(&crate::look::icons::CLOCK, "Catch up terminal", "Catch up on what happened in the terminal").on_press(move|s:&mut State|{if !turn_running(s)&&!s.terminal.catchup.is_pending(){s.terminal.catchup.request(id.clone(),|id|runtime_terminal::catch_up(&id));}}).build(l.k))
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
