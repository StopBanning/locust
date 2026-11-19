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
    } else if name == "manage_task" {
        "Checked on a command"
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
    let title = if d.helper { locust_core::activity_entries::helper_description(name, title) } else { title };
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
        } else if matches!(name.to_lowercase().as_str(), "bash" | "shell" | "exec_command" | "commandexecution" | "run_command" | "manage_task") {
            // A shell row already says what ran; a word beside it would only repeat it.
            ""
        } else {
            locust_core::activity_entries::file_tool_word(name)
        };
        if tool_word.is_empty() || locust_core::activity_entries::tool_name(name, title).is_none() {
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

/// A plan with no run behind it is the teammate's answer, not a card (ThreadItems.tsx `PlanSteps` with no outcomes): an
/// ordered list beside the face, numbered rather than given states, with no PLAN label and no counter. Past nine steps
/// the ordinal gutter widens so the text column stays flush (`is-wide`). No steps draws nothing.
pub(super) fn plan_answer(steps: &[ledger::PlanStep], scope: &str, p: &Palette) -> Option<El> {
    if steps.is_empty() {
        return None;
    }
    let wide = steps.len() > 9;
    let gutter = if wide { 22.0 } else { 14.0 };
    let mut list = col().w_full().gap(6.0).key(format!("{scope}/plan-answer{}", if wide { "/is-wide" } else { "" }));
    for (index, step) in steps.iter().enumerate() {
        list = list.child(
            row()
                .w_full()
                .gap(8.0)
                .child(text(format!("{}", index + 1)).font(type_::BODY.mono()).color(p.muted).w(gutter).key(format!("{scope}/plan-ordinal/{index}")))
                .child(text(step.text.clone()).font(type_::BODY).color(p.text).grow(1.0).shrink().wrap()),
        );
    }
    // The thread keys each item it draws, so the list's own key sits one level in.
    Some(col().w_full().child(list))
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
    activity_item_with_files(item, p, s, scope, locust_core::activity_entries::files(item))
}
fn activity_item_with_files(item: &ledger::Item, p: &Palette, s: &State, scope: &str, files: Vec<locust_core::activity_entries::FileEntry>) -> El {
    let finished = s.ledger.as_ref().is_some_and(|pane| !pane.snapshot.running);
    if let ledger::Item::Plan { steps, finished: ended, .. } = item
        && !steps.is_empty()
    {
        return plan_card(steps, finished || *ended, p);
    }
    if files.is_empty() && !locust_core::activity_entries::files(item).is_empty() {
        let key = format!("{scope}/step/{}", activity_id(item));
        let open = s.activity_open.get(&key).copied().unwrap_or(false);
        let mut step = col().w_full().child(disclosure(key, "Step change".into(), open, p));
        if open { step = step.child(activity_item_with_files(item, p, s, scope, locust_core::activity_entries::files(item))); }
        return step;
    }
    if !files.is_empty() {
        let default = locust_core::activity_entries::default_open(&files);
        let mut rows = col().w_full().gap(4.0);
        for entry in &files {
            let key = format!("{scope}/{}", entry.key);
            let open = s.activity_open.get(&key).copied().unwrap_or(default == Some(entry.key.as_str()));
            // ActivityCard.tsx: "large" when it is, and counts only when they are the change's own (fileRowCounts).
            let mut label = format!("{} {}", entry.file.path, entry.file.status);
            if entry.large { label.push_str(" · large"); }
            if let Some((a, d)) = entry.row_counts().filter(|(a, d)| a + d > 0) {
                label.push(' ');
                label.push_str(&locust_core::changes::line_counts(a, d));
            }
            let mut card = col().w_full().child(disclosure(key, label, open, p));
            if open { card = card.child(super::review_changes::file_diff(s, None, false, &entry.file, p)); }
            rows = rows.child(card);
        }
        return rows;
    }
    if let Some((name, _, too_large)) = locust_core::activity_entries::unreported(item) {
        return text(format!("{name} · {}", if too_large { "result too large to keep" } else { "change not reported" })).font(type_::LABEL).color(p.muted).w_full().wrap_anywhere();
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
        // A thought that is only its headline already said everything on its row.
        ledger::Item::Thought { text, .. } => !without_headline(text).trim().is_empty(),
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
    let finished = items.iter().filter_map(|i| if let ledger::Item::Tool { detail, .. } = i { Some(detail.finished) } else { None }).all(|f| f);
    let workspace = selected_conversation(s).and_then(|(t,_)| t.folder.as_deref());
    let entries = locust_core::activity_entries::net_entries(items, workspace);
    let counts = locust_core::activity_entries::header_counts(items, workspace);
    // The line is `stepsLine`'s (Thread.tsx: the closed line, or the open one, which drops a lone thought's preview):
    // its sentence, then the file counts beside it (ActivityCard's LineCounts), and its amber outcomes under it.
    let line = locust_core::steps_line::steps_line(items, finished, workspace, open);
    let mut label = line.segments[0].text.clone();
    if let Some((added, removed)) = counts.filter(|(a, d)| a + d > 0) {
        label.push(' ');
        label.push_str(&locust_core::changes::line_counts(added, removed));
    }
    let rest: Vec<&locust_core::steps_line::Segment> = line.segments[1..].iter().collect();
    let outcome = (!rest.is_empty()).then(|| (rest.iter().map(|seg| seg.text.as_str()).collect::<Vec<_>>().join(" \u{b7} "), rest.iter().any(|seg| seg.amber) || line.segments[0].amber));
    // A thought that is only its headline has nothing to open onto (ActivityCard 0.875, Colin's frame of 13 Nov:
    // "Waiting for script output" opened onto an empty quote bar). The headline is already the line, so such a
    // thought draws nothing inside the step, and a step with nothing else inside is a line with no chevron.
    let headline_only = |item: &ledger::Item| matches!(item, ledger::Item::Thought { text, .. } if without_headline(text).trim().is_empty());
    let listed: Vec<&ledger::Item> = items.iter().filter(|item| !headline_only(item)).collect();
    let heading = if listed.is_empty() {
        el().w_full().py(4.0).px(2.0).child(text(label).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere())
    } else {
        disclosure(key, label, open, p)
    };
    let mut group = col().w_full().gap(4.0).child(heading);
    if let Some((said, amber)) = &outcome {
        group = group.child(text(said).font(type_::LABEL).color(if *amber { p.amber } else { p.muted }).w_full());
    }
    // The pictures the step read or made, under its line whether or not it is open (ThreadImage, laid out as Claude's
    // app shows a turn's screenshots).
    if let Some(strip) = crate::step_pictures::step_picture_strip(s, items, scope, p) {
        group = group.child(strip);
    }
    if listed.is_empty() {
        return group;
    }
    if open {
        if let [ledger::Item::Thought { text: words, .. }] = listed.as_slice() {
            // A single thought already named its headline on the line (ActivityCard.tsx:535).
            group = group.child(
                Markdown::new(without_headline(words).trim().to_owned())
                    .selectable(true)
                    .render(&markdown_style(p))
                    .w_full(),
            );
        } else {
            // A run of plain tool calls folds into one quiet row that names what it covers (foldPlainToolRuns).
            for folded_row in locust_core::tool_fold::fold_plain_tool_runs(items) {
                group = match folded_row {
                    locust_core::tool_fold::Row::One(i) if headline_only(&items[i]) => group,
                    locust_core::tool_fold::Row::One(i) => {
                        let paths = locust_core::activity_summary::unreported_paths(&items[i]);
                        if paths.is_empty() { group.child(activity_item_with_files(&items[i], p, s, scope, entries.iter().filter(|(at,_)| *at==i).map(|(_,f)|f.clone()).collect())) }
                        else {
                            let mut rows = col().w_full().gap(4.0);
                            for path in paths { rows = rows.child(text(format!("{} · change not reported", locust_core::path_words::display_path(&path, selected_conversation(s).and_then(|(t, _)| t.folder.as_deref())))).font(type_::LABEL).color(p.muted).w_full().wrap_anywhere()); }
                            group.child(rows)
                        }
                    },
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
                observed_edit: false,
                patch: None,
                reported_by: String::new(),
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

    #[test]
    fn the_files_card_uses_the_net_patch_and_keeps_step_evidence_behind_disclosure() {
        let mut items = vec![];
        for (id, observed, added) in [("step", false, 1), ("host", true, 2)] {
            let mut item = sample_tool(id, "edit", "README.md", false);
            if let ledger::Item::Tool {detail,..} = &mut item {
                detail.observed_edit = observed;
                detail.patch = Some(locust_core::activity_entries::Patch {
                    text: if added == 1 { "--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n" } else { "--- a/README.md\n+++ b/README.md\n@@ -1,2 +1,2 @@\n-old\n-draft\n+new\n+ready\n" }.into(),
                    added, removed: added, truncated: false,
                });
            }
            items.push(item);
        }
        let mut s = State::default();
        s.activity_open.insert("conv/group/step".into(), true);
        let said = group_text(&items, &mut s);
        assert!(said.contains("+2 \u{2212}2"), "{said}");
        assert!(!said.contains("+3"), "{said}");
        assert!(said.contains("Step change"), "{said}");
        assert!(!said.contains("+1 \u{2212}1"), "{said}");
        s.activity_open.insert("conv/step/step".into(), true);
        assert!(group_text(&items, &mut s).contains("+1 \u{2212}1"));
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
        // stepsLine's counts: mixed kinds name nothing (0.610).
        assert!(folded_text.contains("Read a file, ran a command"), "{folded_text}");
        assert!(!folded_text.contains("Read Cargo.toml"));
        assert!(!folded_text.contains("cargo test"));

        s.activity_open.insert("conv/group/t1".into(), true);
        let open_text = group_text(&items, &mut s);
        assert!(open_text.contains("Read Cargo.toml"));
        assert!(open_text.contains("cargo test"));
    }

    #[test]
    fn named_files_without_a_diff_reach_separate_headless_rows() {
        let items = vec![sample_tool("t1", "file_change", "C:/w/README.md\nC:/w/src/prices.ts", false)];
        let mut s = State::default();
        s.activity_open.insert("conv/group/t1".into(), true);
        let said = group_text(&items, &mut s);
        assert!(said.contains("Edited 2 files"), "{said}");
        assert!(said.contains("C:/w/README.md · change not reported"), "{said}");
        assert!(said.contains("…/src/prices.ts · change not reported"), "{said}");
        assert_eq!(said.matches("change not reported").count(), 2);
        assert!(!said.contains("file_change"));
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

    fn thought(id: &str, words: &str) -> ledger::Item {
        ledger::Item::Thought { id: id.into(), text: words.into(), duration_ms: Some(2_000) }
    }

    fn group_buttons(items: &[ledger::Item], s: &mut State) -> usize {
        let taken = std::mem::take(s);
        let mut rt = rune::Runtime::new(
            (taken, items.to_vec()),
            |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
            (600.0, 400.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let n = rt.elements().into_iter().filter(|e| e.role == "button").count();
        *s = rt.state.0;
        n
    }

    /// a-step-with-nothing-inside-is-a-line.test.tsx: a step whose only thought is its headline is a line with no
    /// chevron, since there is nothing to press for; it says the headline and nothing else.
    #[test]
    fn a_step_whose_thought_is_only_its_headline_is_a_line_with_nothing_to_press() {
        let items = vec![thought("th1", "**Waiting for script output**")];
        let mut s = State::default();
        assert_eq!(group_buttons(&items, &mut s), 0, "no chevron to press");
        // Even if it had been opened before, nothing opens under it.
        s.activity_open.insert("conv/group/th1".into(), true);
        assert_eq!(group_text(&items, &mut s), "Waiting for script output");
        assert_eq!(group_buttons(&items, &mut s), 0);
    }

    /// a-step-with-nothing-inside-is-a-line.test.tsx: a thought with words under its headline still opens onto
    /// them, without the headline said twice.
    #[test]
    fn a_step_whose_thought_has_words_under_its_headline_still_opens_onto_them() {
        let items = vec![thought("th1", "**Clarifying control 18 source**\n\nThe grid reads row 18 from the fixture.")];
        let mut s = State::default();
        assert_eq!(group_buttons(&items, &mut s), 1);
        s.activity_open.insert("conv/group/th1".into(), true);
        let said = group_text(&items, &mut s);
        assert!(said.contains("The grid reads row 18 from the fixture."), "{said}");
        assert_eq!(said.matches("Clarifying control 18 source").count(), 1, "the headline once: {said}");
    }

    /// a-step-with-nothing-inside-is-a-line.test.tsx: in a step with tool calls, a thought that is only its headline
    /// names the step and draws no row of its own inside it.
    #[test]
    fn a_headline_only_thought_names_the_step_and_draws_no_row_inside_it() {
        let items = vec![thought("th1", "**Checking the build**"), sample_tool("t1", "exec_command", "cargo build", false)];
        let mut s = State::default();
        assert!(group_text(&items, &mut s).contains("Checking the build"));
        s.activity_open.insert("conv/group/th1".into(), true);
        let said = group_text(&items, &mut s);
        assert!(said.contains("cargo build"), "{said}");
        assert!(!said.contains("Thought for"), "no row for the headline-only thought: {said}");
        assert_eq!(said.matches("Checking the build").count(), 1, "the headline is said once, on the step line, not again as a row: {said}");
    }

    /// A thought row elsewhere (a helper's calls) whose text is only its headline is not a button either.
    #[test]
    fn a_thought_row_that_is_only_its_headline_does_not_open() {
        let item = thought("th9", "**Reading the fixture**");
        let s = State::default();
        let mut rt = rune::Runtime::new((s, item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (600.0, 200.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let els = rt.elements();
        assert!(els.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("Reading the fixture"))));
        assert!(!els.iter().any(|e| e.role == "button"), "nothing to open");
    }

    /// mcp-tool-rows.test.ts, on the row: a connector call reads as its tool and its server, not one machine name.
    #[test]
    fn a_connector_call_reads_as_its_tool_and_its_server() {
        let ledger::Item::Tool { status, detail, .. } = sample_tool("m1", "x", "", false) else { unreachable!() };
        let label = tool_row_label("mcp__claude_ai_Robinhood__get_watchlists", status, "", &detail, true);
        assert!(label.starts_with("get_watchlists · Robinhood · "), "{label}");
        assert!(!label.contains("mcp__"), "{label}");
    }

    #[test]
    fn antigravity_rows_read_like_claude() {
        let ledger::Item::Tool { status, detail, .. } = sample_tool("m1", "x", "", false) else { unreachable!() };
        let check = tool_row_label("manage_task", status, "", &detail, true);
        assert!(check.starts_with("Checked on a command · "), "{check}");
        assert!(!check.contains("manage_task"), "{check}");
        let mut refused = detail.clone();
        refused.runtime_status = "refused".into();
        let shell = tool_row_label("run_command", ledger::Status::Failed, "git status", &refused, true);
        assert_eq!(shell, "git status · refused");
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
    #[test]
    fn an_unnamed_helper_reaches_the_window_as_unnamed() {
        let mut item=sample_tool("h", "Task", "", false);
        if let ledger::Item::Tool { detail, .. }=&mut item { detail.helper=true; }
        // Assert the actual row builder, used by both the open group and helper drawers.
        if let ledger::Item::Tool {name,summary,status,detail}=&item {
            assert_eq!(tool_row_label(name,*status,summary,detail,true), "a subagent, unnamed · subagent · reported back");
        }
    }

    #[test]
    fn completion_patches_reach_the_headless_change_rows() {
        let mut item = sample_tool("t1", "apply_patch", "apply_patch", false);
        if let ledger::Item::Tool { detail, .. } = &mut item {
            detail.patch = Some(locust_core::activity_entries::Patch {
                text: "--- a/src/billing.ts\n+++ b/src/billing.ts\n@@ -1 +1 @@\n-old\n+new\n".into(),
                added: 900, removed: 900, truncated: false,
            });
        }
        let mut s = State::default();
        s.activity_open.insert("conv/group/t1".into(), true);
        let said = group_text(&[item], &mut s);
        assert!(said.contains("src/billing.ts MODIFIED +1 \u{2212}1"), "{said}");
        assert!(said.contains("old") && said.contains("new"), "{said}");
        assert!(!said.contains("900") && !said.contains("change not reported"), "{said}");
    }

    #[test]
    fn repeated_patch_reports_draw_once_and_keep_one_total() {
        let mut first = sample_tool("t1", "apply_patch", "notes.md", false);
        if let ledger::Item::Tool { detail, .. } = &mut first {
            detail.patch = Some(locust_core::activity_entries::Patch { text: "--- a/notes.md\n+++ b/notes.md\n@@ -1 +1,2 @@\n a\n+ALPHA-TOUCHED\n".into(), added:1, removed:0, truncated:false });
        }
        let mut second = first.clone();
        if let ledger::Item::Tool { detail, .. } = &mut second { detail.id="t2".into(); }
        let mut s=State::default(); s.activity_open.insert("conv/group/t1".into(),true);
        let said=group_text(&[first,second],&mut s);
        assert_eq!(said.matches("notes.md MODIFIED +1").count(),1,"{said}");
        assert!(!said.contains("+2"),"{said}");
    }

    // a-large-diff-says-the-right-counts.test.tsx: a cut excerpt's lines are never shown as the change's counts.
    fn cut_lines() -> String {
        let mut out = vec!["--- a/src/big.ts".to_owned(), "+++ b/src/big.ts".to_owned(), "@@ -1,1700 +1,1700 @@".to_owned()];
        out.extend((1..=747).map(|i| format!("-line {i}")));
        out.join("\n")
    }

    fn edit(text: String, added: usize, removed: usize, truncated: bool) -> ledger::Item {
        let mut item = sample_tool("t1", "edit", "src/big.ts", false);
        if let ledger::Item::Tool { detail, .. } = &mut item {
            detail.patch = Some(locust_core::activity_entries::Patch { text, added, removed, truncated });
        }
        item
    }

    fn opened(item: ledger::Item) -> String {
        let mut s = State::default();
        s.activity_open.insert("conv/group/t1".into(), true);
        group_text(&[item], &mut s)
    }

    #[test]
    fn never_says_0_with_a_deletion_count_when_lines_were_added_in_a_recorded_large_rewrite() {
        let said = opened(edit(cut_lines(), 1700, 1700, true));
        assert!(said.contains("src/big.ts MODIFIED · large +1700 \u{2212}1700"), "{said}");
        assert!(!said.contains("+0"), "{said}");
        assert!(!said.contains("\u{2212}747"), "{said}");
    }

    #[test]
    fn shows_no_counts_and_lets_large_say_it_when_true_counts_cannot_be_known() {
        let text = format!("{}\n--- a/src/other.ts\n+++ b/src/other.ts\n@@ -1,1 +1,1 @@\n-a\n+b\n", cut_lines());
        let said = opened(edit(text, 1701, 1701, true));
        // Electron reads this cut patch as one file (the hunk's counts decide) and, one file, trusts the tool's total.
        assert!(said.contains("src/big.ts MODIFIED · large"), "{said}");
        assert!(!said.contains("+0"), "{said}");
        assert!(!said.contains("\u{2212}747"), "{said}");
    }

    #[test]
    fn leaves_a_small_diffs_counts_unchanged() {
        let small = "--- a/src/small.ts\n+++ b/src/small.ts\n@@ -1,2 +1,3 @@\n unchanged\n-old\n+new 1\n+new 2".to_owned();
        let mut item = edit(small, 2, 1, false);
        if let ledger::Item::Tool { summary, .. } = &mut item {
            *summary = "src/small.ts".into();
        }
        let said = opened(item);
        assert!(said.contains("src/small.ts MODIFIED +2 \u{2212}1"), "{said}");
        assert!(!said.contains("large"), "{said}");
    }

}
