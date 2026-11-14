use super::*;

/// `.lc-steps__line`: a quiet line that opens what it names, a chevron before the words in the reading face.
pub(super) fn disclosure(key: String, label: String, open: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
    row()
        .w_full()
        .gap(6.0)
        .py(4.0)
        .px(2.0)
        .radius(5.0)
        .child(crate::look::controls::glyph(&l, chevron, 12.0, Some(l.c("text-muted"))).mt(3.0))
        .child(text(label.clone()).font(l.ty.control).color(l.c("text-muted")).grow(1.0).shrink().wrap_anywhere())
        .role("button")
        .label(label)
        .on_press(move |s: &mut State| {
            s.activity_open.insert(key.clone(), !open);
        })
}

pub(super) fn tool_result(
    status: ledger::Status,
    d: &ledger::ToolDetail,
    finished: bool,
) -> String {
    let finished = finished || d.finished;
    if status == ledger::Status::Running {
        return (if finished {
            "did not report"
        } else if d.helper {
            "working on it"
        } else {
            "running"
        })
        .into();
    }
    if matches!(d.runtime_status.as_str(), "declined" | "refused") {
        return d.runtime_status.clone();
    }
    if status == ledger::Status::Failed {
        return if !d.helper
            && let Some(code) = d.exit_code
        {
            format!("failed · exit {code}")
        } else {
            "failed".into()
        };
    }
    if d.background {
        return match d.background_ended.as_str() {
            "completed" => {
                if d.helper {
                    "reported back"
                } else {
                    "done"
                }
            }
            "failed" => "failed",
            "stopped" => "stopped",
            "stopped-with-run" => {
                if d.helper {
                    "stopped when the run ended"
                } else {
                    "stopped"
                }
            }
            "ended" => "ended",
            _ => {
                if finished {
                    "did not report"
                } else if d.helper {
                    "working in the background"
                } else {
                    "running"
                }
            }
        }
        .into();
    }
    if d.helper {
        "reported back".into()
    } else if let Some(code) = d.exit_code {
        format!("exit {code}")
    } else {
        "done".into()
    }
}

pub(super) fn is_shell_tool(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "bash" | "shell" | "exec_command" | "commandexecution"
    )
}

pub(super) fn thought_headline(text: &str) -> Option<&str> {
    text.trim_start()
        .strip_prefix("**")
        .and_then(|rest| rest.split_once("**"))
        .and_then(|(headline, _)| {
            ((2..=80).contains(&headline.len()) && !headline.contains('\n')).then_some(headline)
        })
}

pub(super) fn without_headline(text: &str) -> &str {
    match thought_headline(text) {
        Some(headline) => {
            let marker = format!("**{headline}**");
            text.find(&marker)
                .map(|at| text[at + marker.len()..].trim_start())
                .unwrap_or(text)
        }
        None => text,
    }
}

pub(super) fn tool_row_label(
    name: &str,
    status: ledger::Status,
    summary: &str,
    d: &ledger::ToolDetail,
    finished: bool,
) -> String {
    let result = tool_result(status, d, finished);
    // A connector call names the server as its tool and the tool as its name (mcpToolParts).
    let mcp = locust_core::mcp_names::mcp_tool_parts(name);
    let title = if !summary.is_empty() {
        summary
    } else if let Some(parts) = &mcp {
        &parts.tool
    } else if !d.arguments.is_empty() {
        &d.arguments
    } else {
        name
    };
    // A shell row is one line: what ran, without the Windows host that ran it (shellCommandText).
    let shell_title;
    let title = if locust_core::mission_export::is_shell_tool(name, None)
        || name.eq_ignore_ascii_case("exec_command")
        || name.eq_ignore_ascii_case("commandexecution")
    {
        shell_title = locust_core::shell_words::shell_command_text(title).lines().next().unwrap_or("").trim().to_owned();
        shell_title.as_str()
    } else {
        title
    };
    let mut label = if d.helper {
        let mut row = format!(
            "{title} · {}subagent",
            if d.helper_type.is_empty() {
                String::new()
            } else {
                format!("{} ", d.helper_type)
            }
        );
        if !d.children.is_empty() {
            row.push_str(&format!(
                " · {} {}",
                d.children.len(),
                if d.children.len() == 1 {
                    "call"
                } else {
                    "calls"
                }
            ));
        }
        row.push_str(&format!(" · {result}"));
        row
    } else {
        let tool_word = if let Some(parts) = &mcp {
            parts.server.as_str()
        } else if locust_core::mcp_names::is_bare_connector_tool(name) {
            "a connector"
        } else { match name.to_lowercase().as_str() {
            "read" | "read_file" | "readfile" | "view_file" | "cat" => "Read",
            "write" | "write_file" | "writefile" => "Write",
            "edit" | "edit_file" | "editfile" | "apply_patch" => "Edit",
            "grep" | "grep_search" | "ripgrep" => "Grep",
            "glob" | "list_dir" | "list_directory" | "ls" => "Glob",
            "bash" | "shell" | "exec_command" | "commandexecution" => "",
            _ => name,
        } };
        if tool_word.is_empty() {
            format!("{title} · {result}")
        } else {
            format!("{title} · {tool_word} · {result}")
        }
    };
    if d.helper
        && status == ledger::Status::Done
        && !d.output.trim().is_empty()
        && result == "reported back"
    {
        label.push_str(&format!(
            " · {}",
            d.output
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(160)
                .collect::<String>()
        ));
    }
    if result == "working in the background" && !d.progress.is_empty() {
        label.push_str(&format!(" · {}", d.progress));
    }
    // BackgroundBadge is shell-only (ActivityCard.tsx:1298); helpers already say it in tool_result.
    if d.background && !d.helper {
        label.push_str(if d.background_ended == "stopped-with-run" {
            " · when the run ended"
        } else {
            " · in the background"
        });
    }
    if is_shell_tool(name)
        && d.output_reported
        && d.output.trim().is_empty()
        && status != ledger::Status::Running
    {
        label.push_str(" · no output");
    }
    let refused = matches!(d.runtime_status.as_str(), "declined" | "refused");
    if is_shell_tool(name)
        && !d.background
        && !refused
        && status != ledger::Status::Running
        && let Some(ms) = d.duration_ms.filter(|ms| *ms >= 1000)
    {
        label.push_str(&format!(" · {}", ledger::duration_text(ms)));
    }
    label
}

pub(super) fn activity_drawer_key(scope: &str, item: &ledger::Item) -> String {
    match item {
        ledger::Item::Plan { id, .. } => format!("{scope}/plan/{id}"),
        ledger::Item::Thought { id, .. } => format!("{scope}/{id}"),
        ledger::Item::Tool { detail, .. } => format!("{scope}/{}", detail.id),
        _ => format!("{scope}/item"),
    }
}

/// ThreadItems.tsx PlanCard (`.lc-plancard`): a card with a head ("PLAN", how many are done) over the steps, each with
/// its marker: green once done, a pulsing dot on the step underway, faded when the run ended without reaching it.
fn plan_card(steps: &[ledger::PlanStep], finished: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let done = steps.iter().filter(|r| r.state == "done").count();
    let unreached = steps.len() - done;
    let mono = |size: f32| rune::TextStyle::new(size, 400.0).mono();
    let mut counts = row().gap(6.0).center_y().child(text(format!("{done} of {} done", steps.len())).font(mono(11.5)).color(l.c("text-muted")));
    if finished && unreached > 0 {
        counts = counts
            .child(text("·").font(mono(11.5)).color(l.c("text-muted")))
            .child(text(format!("{unreached} not checked off")).font(mono(11.5)).color(l.c("amber-text")));
    }
    let head = row()
        .w_full()
        .center_y()
        .py(8.0)
        .px(12.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
        .child(text("PLAN").font(mono(11.5).tracking(0.9)).color(l.c("text-muted")).grow(1.0))
        .child(counts);
    let mut list = col().w_full().gap(8.0).pt(12.0).px(12.0).pb(12.0);
    for step in steps {
        let running = step.state == "running" && !finished;
        let is_done = step.state == "done";
        let (edge, fill, ink) = if is_done {
            (l.c("green-border"), Some(l.c("green-tint")), l.c("green-text"))
        } else if running {
            (l.c("lime-border-strong"), None, l.c("lime-text"))
        } else {
            (l.c("border-card"), None, l.c("text-muted"))
        };
        let mut dot = el().size(6.0, 6.0).radius(3.0).fill(ink);
        if running {
            dot = dot.cycle(Cycle { period: 1.6, phase: 0.0, stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)], still: 1.0, fps: 30.0 });
        }
        let mut marker = row().size(16.0, 16.0).mt(2.0).center().radius(8.0).border(1.0, edge).child(dot);
        if let Some(fill) = fill {
            marker = marker.fill(fill);
        }
        let words = if running {
            rune::TextStyle::new(13.0, 500.0).mono()
        } else {
            mono(13.0)
        };
        let ink_words = if running { l.c("text-primary") } else if is_done { l.c("text-secondary") } else { l.c("text-muted") };
        let mut line = row().w_full().gap(8.0).child(marker).child(text(step.text.clone()).font(words).color(ink_words).grow(1.0).shrink().wrap());
        if finished && !is_done {
            line = line.opacity(0.55);
        }
        list = list.child(line);
    }
    col()
        .w_full()
        .radius(10.0)
        .border(1.0, l.c("border-card"))
        .fill(l.c("bg-raised"))
        .clip_rounded(10.0)
        .child(head)
        .child(list)
}

pub(super) fn activity_item(item: &ledger::Item, p: &Palette, s: &State, scope: &str) -> El {
    let finished = s.ledger.as_ref().is_some_and(|pane| !pane.snapshot.running);
    if let ledger::Item::Plan { steps, finished: ended, .. } = item
        && !steps.is_empty()
    {
        return plan_card(steps, finished || *ended, p);
    }
    let label = match item {
        ledger::Item::Tool {
            name,
            status,
            summary,
            detail: d,
        } => {
            // A file the call acted on reads relative to the conversation's folder, and Locust's own copies as the
            // file they copy (displayPath); a command or a sentence is left as it is.
            let folder = selected_conversation(s).and_then(|(t, _)| t.folder.clone());
            let path_like = !summary.is_empty() && !summary.contains(char::is_whitespace) && (summary.contains('/') || summary.contains('\\'));
            let shown = if path_like && !is_shell_tool(name) { locust_core::path_words::display_path(summary, folder.as_deref()) } else { summary.clone() };
            tool_row_label(name, *status, &shown, d, finished)
        }
        ledger::Item::Plan {
            steps,
            finished: ended,
            ..
        } => {
            let finished = finished || *ended;
            let done = steps.iter().filter(|r| r.state == "done").count();
            format!(
                "PLAN · {done} of {} done{}",
                steps.len(),
                if finished && done < steps.len() {
                    format!(" · {} not checked off", steps.len() - done)
                } else {
                    String::new()
                }
            )
        }
        ledger::Item::Thought {
            text, duration_ms, ..
        } => {
            let mut label = duration_ms
                .filter(|ms| *ms >= 500)
                .map(|ms| format!("Thought for {}", ledger::duration_text(ms)))
                .unwrap_or_else(|| "Thought".into());
            if let Some(headline) = thought_headline(text) {
                label.push_str(&format!(" · {headline}"));
            }
            label
        }
        _ => return col(),
    };
    let key = activity_drawer_key(scope, item);
    let open = s
        .activity_open
        .get(&key)
        .copied()
        .unwrap_or(matches!(item, ledger::Item::Plan { .. }));
    let expandable = match item {
        ledger::Item::Thought { text, .. } => !text.is_empty(),
        ledger::Item::Tool { detail, .. } => {
            !detail.arguments.is_empty() || !detail.output.is_empty() || !detail.children.is_empty()
        }
        ledger::Item::Plan { steps, .. } => !steps.is_empty(),
        _ => false,
    };
    let heading = if expandable {
        disclosure(key.clone(), label, open, p)
    } else {
        el().w_full().padding(8.0).child(
            text(label)
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .shrink()
                .wrap_anywhere(),
        )
    };
    let mut card = col().w_full().gap(4.0);
    if matches!(item, ledger::Item::Plan { .. }) {
        card = card.radius(7.0).border(1.0, p.hairline);
    }
    card = card.child(heading);
    if open {
        // Under the chevron, not flush with it; a command and its output in the code face on a quiet well.
        let l = crate::look::controls::Look::new(p.dark);
        let code = rune::TextStyle::new(11.5, 400.0).mono().lh(17.0);
        let mut body = col().w_full().pl(20.0).pr(4.0).pb(6.0).gap(8.0);
        match item {
            ledger::Item::Tool { detail: d, .. } => {
                if !d.arguments.is_empty() {
                    body = body.child(
                        el().w_full().padding(10.0).radius(8.0).fill(l.c("hover-surface")).child(text(d.arguments.clone()).font(code).color(l.c("text-secondary")).w_full().wrap_anywhere().selectable(true)),
                    );
                }
                if !d.output.is_empty() {
                    let all_key = format!("{key}/output");
                    let all = s.activity_open.get(&all_key).copied().unwrap_or(false);
                    let output=ledger::bounded_output(&d.output,all);
                    let lines: Vec<_>=d.output.trim_end().split('\n').collect();
                    if !all && lines.len()>16 {
                        body=body.child(text(lines[..8].join("\n")).font(code).color(l.c("text-muted")).w_full().shrink().wrap_anywhere().selectable(true));
                        body=body.child(disclosure(all_key,format!("{} more lines",lines.len()-16),false,p));
                        body=body.child(text(lines[lines.len()-8..].join("\n")).font(code).color(l.c("text-muted")).w_full().shrink().wrap_anywhere().selectable(true));
                    } else { body=body.child(text(output).font(code).color(l.c("text-muted")).w_full().shrink().wrap_anywhere().selectable(true)); }
                }
                // A helper's own calls, one step in, each saying how it ended (HelperCalls).
                if !d.children.is_empty() {
                    body = body.child(text("What the helper did").font(type_::LABEL).color(p.muted).key(format!("{key}/helper-calls")));
                }
                for child in &d.children { body=body.child(activity_item(child,p,s,&key)); }
            }
            ledger::Item::Plan { steps, finished: ended, .. } => {
                let finished=finished || *ended;
                for (n, step) in steps.iter().enumerate() {
                    let mark = if step.state=="done" {"✓"} else if step.state=="running" && !finished {"●"} else {"○"};
                    body=body.child(text(format!("{mark} {}. {}", n+1, step.text)).font(type_::LABEL).color(if step.state=="running" && !finished {p.text} else {p.muted}).wrap_anywhere());
                }
            }
            ledger::Item::Thought { text: words, .. }
                // Multi-item thought rows keep the full text under the headline button (ActivityCard.tsx:866).
                if !words.is_empty() => {
                    body=body.child(Markdown::new(words.clone()).selectable(true).render(&markdown_style(p)).w_full());
                }
            _=>{}
        }
        card = card.child(body);
    }
    card
}

pub(super) fn is_activity(item: &ledger::Item) -> bool {
    matches!(
        item,
        ledger::Item::Tool { .. } | ledger::Item::Thought { .. }
    )
}
pub(super) fn activity_id(item: &ledger::Item) -> &str {
    match item {
        ledger::Item::Tool { detail, .. } => &detail.id,
        ledger::Item::Thought { id, .. } | ledger::Item::Plan { id, .. } => id,
        _ => "",
    }
}
pub(super) fn activity_group(items: &[ledger::Item], p: &Palette, s: &State, scope: &str) -> El {
    let key = format!("{scope}/group/{}", activity_id(&items[0]));
    let open = s.activity_open.get(&key).copied().unwrap_or(false);
    let mut commands = 0;
    let mut helpers = 0;
    let mut tools = 0;
    let mut thought_ms = 0;
    let mut headline = None;
    for item in items {
        match item {
            ledger::Item::Tool { name, detail, .. } => {
                if detail.helper {
                    helpers += 1;
                } else if matches!(
                    name.to_lowercase().as_str(),
                    "bash" | "shell" | "exec_command" | "commandexecution"
                ) {
                    commands += 1;
                } else {
                    tools += 1;
                }
            }
            ledger::Item::Thought {
                text, duration_ms, ..
            } => {
                thought_ms += duration_ms.unwrap_or(0);
                if let Some(h) = thought_headline(text) {
                    headline = Some(h.to_owned());
                }
            }
            _ => {}
        }
    }
    let mut words = Vec::new();
    if commands > 0 {
        words.push(format!(
            "ran {commands} {}",
            if commands == 1 { "command" } else { "commands" }
        ));
    }
    if helpers > 0 {
        words.push(format!(
            "asked {helpers} {}",
            if helpers == 1 {
                "subagent"
            } else {
                "subagents"
            }
        ));
    }
    if tools > 0 {
        words.push(format!(
            "{tools} other tool {}",
            if tools == 1 { "call" } else { "calls" }
        ));
    }
    let mut label = headline.unwrap_or_else(|| {
        if words.is_empty() {
            if thought_ms >= 1000 {
                format!("Thought for {}", ledger::duration_text(thought_ms))
            } else {
                "Thought".into()
            }
        } else {
            words.join(", ")
        }
    });
    if let Some(first) = label.chars().next() {
        label = format!("{}{}", first.to_uppercase(), &label[first.len_utf8()..]);
    }
    let heading = if matches!(items,[ledger::Item::Thought {text,..}] if text.is_empty()) {
        el().w_full()
            .padding(8.0)
            .child(text(label).font(type_::LABEL).color(p.muted))
    } else {
        disclosure(key, label, open, p)
    };
    let mut group = col().w_full().gap(4.0).child(heading);
    if open {
        if let [ledger::Item::Thought { text: words, .. }] = items {
            // Single-thought groups already named the headline on the button (ActivityCard.tsx:535).
            let body = without_headline(words);
            if !body.is_empty() {
                group = group.child(
                    Markdown::new(body.to_owned())
                        .selectable(true)
                        .render(&markdown_style(p))
                        .w_full(),
                );
            }
        } else {
            // A run of plain tool calls folds into one quiet row that names what it covers (foldPlainToolRuns).
            for folded_row in locust_core::tool_fold::fold_plain_tool_runs(items) {
                group = match folded_row {
                    locust_core::tool_fold::Row::One(i) => group.child(activity_item(&items[i], p, s, scope)),
                    locust_core::tool_fold::Row::Folded { items: folded, names, verb } => group.child(
                        row()
                            .key(format!("{scope}/folded/{}", activity_id(&items[folded[0]])))
                            .w_full()
                            .gap(6.0)
                            .padding(6.0)
                            .child(text(locust_core::tool_fold::folded_tools_lead(names.len(), verb.as_deref())).font(type_::LABEL).color(p.secondary))
                            .child(text(locust_core::tool_fold::folded_tools_names(&names)).font(type_::LABEL.mono()).color(p.muted).wrap_anywhere()),
                    ),
                };
            }
        }
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool(id: &str, name: &str, summary: &str, running: bool) -> ledger::Item {
        ledger::Item::Tool {
            name: name.into(),
            summary: summary.into(),
            detail: Box::new(ledger::ToolDetail {
                id: id.into(),
                parent: String::new(),
                arguments: String::new(),
                output: String::new(),
                output_reported: false,
                runtime_status: String::new(),
                duration_ms: None,
                exit_code: if running { None } else { Some(0) },
                helper: false,
                helper_type: String::new(),
                background: false,
                finished: !running,
                progress: String::new(),
                background_ended: String::new(),
                children: Vec::new(),
            }),
            status: if running {
                ledger::Status::Running
            } else {
                ledger::Status::Done
            },
        }
    }

    fn group_text(items: &[ledger::Item], s: &mut State) -> String {
        let taken = std::mem::take(s);
        let mut rt = rune::Runtime::new(
            (taken, items.to_vec()),
            |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
            (600.0, 400.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let out = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        *s = rt.state.0;
        out
    }

    // Ported from renderer/src/the-newest-group-stays-folded.test.tsx & a-growing-group-shows-its-rows.test.tsx
    #[test]
    fn activity_group_stays_folded_until_opened_and_then_shows_every_row() {
        let items = vec![
            sample_tool("t1", "read_file", "Read Cargo.toml", false),
            sample_tool("t2", "exec_command", "cargo test", true),
        ];
        let mut s = State::default();
        let folded_text = group_text(&items, &mut s);
        assert!(folded_text.contains("Ran 1 command, 1 other tool call"));
        assert!(!folded_text.contains("Read Cargo.toml"));
        assert!(!folded_text.contains("cargo test"));

        s.activity_open.insert("conv/group/t1".into(), true);
        let open_text = group_text(&items, &mut s);
        assert!(open_text.contains("Read Cargo.toml"));
        assert!(open_text.contains("cargo test"));
    }

    /// fold-tool-runs.test.ts, drawn: an opened group folds three reads into one row naming them, and keeps the
    /// command between them on its own row.
    #[test]
    fn an_opened_group_folds_a_run_of_reads_into_one_row_and_keeps_the_command() {
        let items = vec![
            sample_tool("t1", "read_file", "a.ts", false),
            sample_tool("t2", "read_file", "b.ts", false),
            sample_tool("t3", "read_file", "c.ts", false),
            sample_tool("t4", "exec_command", "cargo test", false),
        ];
        let mut s = State::default();
        s.activity_open.insert("conv/group/t1".into(), true);
        let said = group_text(&items, &mut s);
        assert!(said.contains("Read 3 files") && said.contains("a.ts, b.ts, c.ts"), "{said}");
        assert!(said.contains("cargo test"), "the command keeps its row: {said}");
        assert!(!said.contains("a.ts · Read"), "the reads are not drawn one by one: {said}");
    }

    /// mcp-tool-rows.test.ts, on the row: a connector call reads as its tool and its server, not one machine name.
    #[test]
    fn a_connector_call_reads_as_its_tool_and_its_server() {
        let ledger::Item::Tool { status, detail, .. } = sample_tool("m1", "x", "", false) else { unreachable!() };
        let label = tool_row_label("mcp__claude_ai_Robinhood__get_watchlists", status, "", &detail, true);
        assert!(label.starts_with("get_watchlists · Robinhood · "), "{label}");
        assert!(!label.contains("mcp__"), "{label}");
    }

    /// a-helpers-calls-fold-under-its-row.test.tsx, drawn: the helper row counts its calls and stays folded until
    /// pressed; opened, it shows what the helper did, each call saying how it ended.
    #[test]
    fn a_helpers_calls_count_on_its_row_and_open_onto_each_call() {
        let mut helper = sample_tool("h1", "Agent", "Count the files", false);
        if let ledger::Item::Tool { detail, .. } = &mut helper {
            detail.helper = true;
            detail.helper_type = "Explore".into();
            // A Glob has no exit code: it says how it ended in words.
            let mut glob = sample_tool("g1", "Glob", "**/*", false);
            if let ledger::Item::Tool { detail, .. } = &mut glob {
                detail.exit_code = None;
            }
            detail.children = vec![glob];
        }
        let items = vec![helper];
        let mut s = State::default();
        s.activity_open.insert("conv/group/h1".into(), true);
        let shut = group_text(&items, &mut s);
        assert!(shut.contains("1 call"), "{shut}");
        assert!(!shut.contains("What the helper did"), "folded until pressed: {shut}");
        let key = activity_drawer_key("conv", &items[0]);
        s.activity_open.insert(key, true);
        let open = group_text(&items, &mut s);
        assert!(open.contains("What the helper did") && open.contains("**/*") && open.contains("done"), "{open}");
    }
}
