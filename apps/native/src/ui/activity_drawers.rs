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

/// Verbose (⌘O, transcriptView.ts): every step group and every row in it starts open, unless the person opened or
/// closed it themselves (ActivityCard `isOpen`: a toggle wins, then verbose).
pub(super) fn verbose(s: &State) -> bool {
    s.settings_panel.settings.verbose()
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

/// `shellResultClass`: the colour of a command's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShellBadge {
    Ok,
    Failed,
    Running,
    Stalled,
}

pub(super) fn shell_badge(status: ledger::Status, d: &ledger::ToolDetail, finished: bool) -> ShellBadge {
    let finished = finished || d.finished;
    if status == ledger::Status::Running {
        return if finished { ShellBadge::Stalled } else { ShellBadge::Running };
    }
    if matches!(d.runtime_status.as_str(), "declined" | "refused") {
        return ShellBadge::Stalled;
    }
    if status == ledger::Status::Failed {
        return ShellBadge::Failed;
    }
    if d.background {
        return match d.background_ended.as_str() {
            "completed" => ShellBadge::Ok,
            "failed" => ShellBadge::Failed,
            "" => if finished { ShellBadge::Stalled } else { ShellBadge::Running },
            _ => ShellBadge::Stalled,
        };
    }
    ShellBadge::Ok
}

/// The command is behind the fold, not on the row: the row led with a description, or the command runs past its
/// first line (the row draws one line).
pub(super) fn shell_command_folded(summary: &str, arguments: &str) -> bool {
    !arguments.trim().is_empty() && (!summary.is_empty() || arguments.trim().lines().count() > 1)
}

/// `.lc-filerow.is-shell`: the command first, then its verdict in a green, red, amber or quiet badge, then the
/// quiet words after it ("no output", the time it took). A chevron only when something is behind it (`is-static`).
fn shell_heading(key: String, label: String, result: &str, badge: ShellBadge, badges: Vec<El>, expandable: bool, open: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let marker = format!(" · {result}");
    let (title, rest) = match label.rfind(&marker) {
        Some(at) => (label[..at].to_owned(), label[at + marker.len()..].trim_start_matches(" · ").to_owned()),
        None => (label.clone(), String::new()),
    };
    let (fg, bg) = match badge {
        ShellBadge::Ok => (l.c("green-text"), l.c("green-tint")),
        ShellBadge::Failed => (l.c("red"), l.c("red-tint")),
        ShellBadge::Stalled => (l.c("amber-text"), l.c("amber-tint")),
        ShellBadge::Running => (l.c("text-muted"), l.c("bg-raised-2")),
    };
    let mut line = row().w_full().gap(6.0).py(4.0).px(2.0).radius(5.0).label(label);
    if expandable {
        let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
        line = line.child(crate::look::controls::glyph(&l, chevron, 12.0, Some(l.c("text-muted"))).mt(3.0));
    } else {
        // Where the chevron would be, so a command that cannot open starts where the paths and other commands do.
        line = line.child(el().key(format!("{key}/edge")).w(12.0));
    }
    line = line
        .child(text(title).font(type_::LABEL.mono()).color(l.c("text-secondary")).shrink().wrap_anywhere())
        .child(
            el().key(format!("{key}/badge")).px(6.0).radius(5.0).fill(bg)
                .child(text(result.to_owned()).font(type_::LABEL.mono()).color(fg)),
        );
    for said in badges {
        line = line.child(said);
    }
    if !rest.is_empty() {
        line = line.child(text(rest).font(type_::LABEL).color(l.c("text-muted")).shrink().wrap_anywhere());
    }
    if expandable {
        line.key(key.clone()).role("button").on_press(move |s: &mut State| {
            s.activity_open.insert(key.clone(), !open);
        })
    } else {
        line.key(key)
    }
}

/// ActivityCard.tsx ReachBadge (0.578): a command that acted on programs this run did not start says so on its row, in
/// a few words, the whole sentence on hover. In Auto no card is asked, so the row is the only place left to say that
/// `taskkill //F //IM python.exe` stopped every Python on the computer. Not on a refused or declined command: it never
/// ran.
fn reach_badge(s: &State, key: &str, d: &ledger::ToolDetail, p: &Palette) -> Option<El> {
    if matches!(d.runtime_status.as_str(), "declined" | "refused") {
        return None;
    }
    let reach = locust_core::command_reach::command_reach(Some(&d.arguments))?;
    let l = crate::look::controls::Look::new(p.dark);
    let badge_key = format!("{key}/reach/{}", reach.kind.as_str());
    let badge = el()
        .key(badge_key.clone())
        .label(reach.said.clone())
        .px(6.0)
        .radius(5.0)
        .fill(l.c("amber-tint"))
        .child(text(reach.short.clone()).font(type_::LABEL.mono()).color(l.c("amber-text")));
    Some(super::hover::tip(s, &l, &badge_key, &reach.said, badge))
}

/// Codex's Windows sandbox never started this command (0.736, core `codex_failure`): the row says so in a few words,
/// the whole sentence on hover, where it said only "failed" (ActivityCard.tsx `SandboxBadge`).
fn sandbox_never_started(status: ledger::Status, d: &ledger::ToolDetail) -> bool {
    status == ledger::Status::Failed && locust_core::codex_failure::sandbox_did_not_start(Some(&d.output))
}

fn sandbox_badge(s: &State, key: &str, status: ledger::Status, d: &ledger::ToolDetail, p: &Palette) -> Option<El> {
    if !sandbox_never_started(status, d) {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let said = locust_core::codex_failure::SANDBOX_DID_NOT_START;
    let badge_key = format!("{key}/sandbox");
    let badge = el()
        .key(badge_key.clone())
        .label(said)
        .px(6.0)
        .radius(5.0)
        .fill(l.c("amber-tint"))
        .child(text("sandbox did not start").font(type_::LABEL.mono()).color(l.c("amber-text")));
    Some(super::hover::tip(s, &l, &badge_key, said, badge))
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
    // ActivityCard's tool and file rows say a live call is "still running"; a command's own badge says "running".
    let shell_like = matches!(name.to_lowercase().as_str(), "bash" | "shell" | "exec_command" | "commandexecution" | "run_command" | "manage_task");
    let result = if result == "running" && !d.helper && !shell_like { "still running".to_owned() } else { result };
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
/// The unticked count's words on hover (ThreadItems.tsx PlanSteps): the checklist is the runtime's bookkeeping, not a
/// check of the work.
pub(super) const UNTICKED_STOPPED: &str = "The run stopped before checking these off its list: see what it ran below.";
pub(super) const UNTICKED_ENDED: &str =
    "The run ended without checking these off its list. That is the runtime's checklist, not a check of the work: see what it ran below.";

/// The plan's own marker is independent of what the live line is doing.
const PLAN_ORB: &str = "listening";

fn plan_card(s: &State, id: &str, steps: &[ledger::PlanStep], finished: bool, stopped: bool, underway: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let done = steps.iter().filter(|r| r.state == "done").count();
    let unreached = steps.len() - done;
    let mono = |size: f32| rune::TextStyle::new(size, 400.0).mono();
    let mut counts = row().gap(6.0).center_y().child(text(format!("{done} of {} done", steps.len())).font(mono(11.5)).color(l.c("text-muted")));
    // AMBER ONLY WHEN THE RUN STOPPED SHORT (0.368): after a run that completed, the count is said in the header's own
    // voice; amber says interrupted.
    if finished && unreached > 0 {
        let (ink, why, key) = if stopped {
            (l.c("amber-text"), UNTICKED_STOPPED, format!("{id}/plan-unreached/is-stopped"))
        } else {
            (l.c("text-muted"), UNTICKED_ENDED, format!("{id}/plan-unreached"))
        };
        let count = el().key(key.clone()).label(why).child(text(format!("{unreached} not checked off")).font(mono(11.5)).color(ink));
        counts = counts
            .child(text("·").font(mono(11.5)).color(l.c("text-muted")))
            .child(super::hover::tip(s, &l, &key, why, count));
    }
    use crate::look::orb::{Surface, view as orb};
    let underway = underway && !finished;
    let head_orb = underway.then(|| orb(format!("{id}/plan-head-orb"), "solving", Surface::PlanHead, s, p.dark)).flatten();
    let head = row()
        .w_full()
        .center_y()
        .py(8.0)
        .px(12.0)
        .gap(8.0)
        .children(head_orb)
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
        .child(text("PLAN").font(mono(11.5).tracking(0.9)).color(l.c("text-muted")).grow(1.0))
        .child(counts);
    let mut list = col().w_full().gap(8.0).pt(12.0).px(12.0).pb(12.0);
    for (index, step) in steps.iter().enumerate() {
        let running = step.state == "running" && !finished;
        let is_done = step.state == "done";
        let (edge, fill, ink) = if is_done {
            (l.c("green-border"), Some(l.c("green-tint")), l.c("green-text"))
        } else if running {
            (l.c("lime-border-strong"), None, l.c("lime-text"))
        } else {
            (l.c("border-card"), None, l.c("text-muted"))
        };
        // The orb REPLACES the pulsing pip rather than joining it: two things pulsing on one row is the row saying
        // "now" twice. Only while a turn is underway; nothing pulses once the run has stopped.
        let orb = (running && underway).then(|| {
            orb(format!("{id}/plan-orb/{PLAN_ORB}"), PLAN_ORB, Surface::PlanStep, s, p.dark)
        }).flatten();
        let marker = row().size(20.0, 20.0).center().child(orb.unwrap_or_else(|| {
            let mut dot = el().size(6.0, 6.0).radius(3.0).fill(ink);
            if running {
                dot = dot.cycle(Cycle { period: 1.6, phase: 0.0, stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)], still: 1.0, fps: 30.0 }).key(format!("{id}/plan-dot/{index}/is-pulsing"));
            }
            let mut marker = row().size(16.0, 16.0).center().radius(8.0).border(1.0, edge).child(dot);
            if let Some(fill) = fill {
                marker = marker.fill(fill);
            }
            marker
        }));
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

/// A row per file, each folded but the one `default` names (or every one, in verbose mode). A new Markdown file opens
/// as the document it is, and a new web page as the page, run beside the conversation.
fn file_rows(files: &[locust_core::activity_entries::FileEntry], default: Option<&str>, p: &Palette, s: &State, scope: &str) -> El {
    let mut rows = col().w_full().gap(4.0);
    for entry in files {
        let key = format!("{scope}/{}", entry.key);
        let open = s.activity_open.get(&key).copied().unwrap_or(verbose(s) || default == Some(entry.key.as_str()));
        // ActivityCard.tsx: "large" when it is, and counts only when they are the change's own (fileRowCounts).
        let mut label = format!("{} {}", entry.file.path, entry.file.status);
        if entry.large { label.push_str(" · large"); }
        if let Some((a, d)) = entry.row_counts().filter(|(a, d)| a + d > 0) {
            label.push(' ');
            label.push_str(&locust_core::changes::line_counts(a, d));
        }
        let mut card = col().w_full().child(disclosure(key.clone(), label, open, p));
        if open {
            card = card.child(match locust_core::changes::new_document(&entry.file) {
                Some(locust_core::changes::NewDocument::Markdown) => doc_preview(s, &key, entry, p),
                Some(locust_core::changes::NewDocument::Page) => page_preview(s, &key, entry, p),
                _ => super::review_changes::file_diff(s, None, false, &entry.file, entry.truncated, p),
            });
        }
        rows = rows.child(card);
    }
    rows
}

pub(super) fn activity_item(item: &ledger::Item, p: &Palette, s: &State, scope: &str) -> El {
    activity_item_with_files(item, p, s, scope, locust_core::activity_entries::files(item))
}
fn activity_item_with_files(item: &ledger::Item, p: &Palette, s: &State, scope: &str, files: Vec<locust_core::activity_entries::FileEntry>) -> El {
    let finished = s.ledger.as_ref().is_some_and(|pane| !pane.snapshot.running);
    if let ledger::Item::Plan { id, steps, finished: ended, stopped } = item
        && !steps.is_empty()
    {
        // Thread.tsx `underway={runningOrb !== undefined}`: whether the live line is up, and nothing about it.
        let underway = super::messages::live_sweeping(s);
        return plan_card(s, id, steps, finished || *ended, *stopped, underway, p);
    }
    if files.is_empty() && !locust_core::activity_entries::files(item).is_empty() {
        let key = format!("{scope}/step/{}", activity_id(item));
        let open = s.activity_open.get(&key).copied().unwrap_or(verbose(s));
        let mut step = col().w_full().child(disclosure(key, "Step change".into(), open, p));
        if open { step = step.child(activity_item_with_files(item, p, s, scope, locust_core::activity_entries::files(item))); }
        return step;
    }
    if !files.is_empty() {
        let default = locust_core::activity_entries::default_open(&files).map(str::to_owned);
        return file_rows(&files, default.as_deref(), p, s, scope);
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
        .unwrap_or(verbose(s) || matches!(item, ledger::Item::Plan { .. }));
    let expandable = match item {
        // A thought that is only its headline already said everything on its row.
        ledger::Item::Thought { text, .. } => !without_headline(text).trim().is_empty(),
        ledger::Item::Tool { name, summary, detail, .. } if is_shell_tool(name) && !detail.helper => {
            !detail.output.trim().is_empty() || shell_command_folded(summary, &detail.arguments) || !detail.children.is_empty()
        }
        ledger::Item::Tool { detail, .. } => {
            !detail.arguments.is_empty() || !detail.output.is_empty() || !detail.children.is_empty()
        }
        ledger::Item::Plan { steps, .. } => !steps.is_empty(),
        _ => false,
    };
    let shell = match item {
        ledger::Item::Tool { name, status, detail, .. } if is_shell_tool(name) && !detail.helper => {
            let badges = [reach_badge(s, &key, detail, p), sandbox_badge(s, &key, *status, detail, p)].into_iter().flatten().collect();
            Some((tool_result(*status, detail, finished), shell_badge(*status, detail, finished), badges))
        }
        _ => None,
    };
    let heading = if let Some((result, badge, badges)) = shell {
        shell_heading(key.clone(), label, &result, badge, badges, expandable, open, p)
    } else if expandable {
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
            ledger::Item::Tool { name, summary, detail: d, status, .. } => {
                // A shell row already shows its command; the fold repeats it only when the row led with a
                // description, or showed just the first of its lines (ActivityCard.tsx: `entry.title !== undefined`).
                let command_shown = is_shell_tool(name) && !d.helper && !shell_command_folded(summary, &d.arguments);
                if !d.arguments.is_empty() && !command_shown {
                    body = body.child(
                        el().w_full().padding(10.0).radius(8.0).fill(l.c("hover-surface")).child(text(d.arguments.clone()).font(code).color(l.c("text-secondary")).w_full().wrap_anywhere().selectable(true)),
                    );
                }
                // Opened, the sandbox's sentence is said above what Codex printed (`lc-shellwhy`).
                if is_shell_tool(name) && !d.helper && sandbox_never_started(*status, d) {
                    body = body.child(text(locust_core::codex_failure::SANDBOX_DID_NOT_START).font(type_::LABEL).color(l.c("amber-text")).w_full().wrap().key(format!("{key}/sandbox-why")));
                }
                if !d.output.trim().is_empty() {
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
/// DocPreview.tsx: a new Markdown file drawn as the page it is (the renderer a reply uses), with Open beside it when
/// there is a folder to open it from, and "Show the change" for its diff (and back).
fn doc_preview(s: &State, key: &str, entry: &locust_core::activity_entries::FileEntry, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let as_change_key = format!("{key}/as-change");
    let name = entry.file.path.replace('\\', "/").rsplit('/').next().unwrap_or(&entry.file.path).to_owned();
    let switch = |label: &str| {
        let flip = as_change_key.clone();
        crate::look::controls::button(&l, label, crate::look::controls::Tone::Neutral, false).on_press(move |s: &mut State| {
            let now = s.activity_open.get(&flip).copied().unwrap_or(false);
            s.activity_open.insert(flip.clone(), !now);
        })
    };
    if s.activity_open.get(&as_change_key).copied().unwrap_or(false) {
        return col()
            .w_full()
            .gap(6.0)
            .child(super::review_changes::file_diff(s, None, false, &entry.file, entry.truncated, p))
            .child(row().child(switch("Show it as a document")));
    }
    let mut foot = row().w_full().gap(8.0).center_y();
    if selected_conversation(s).is_some_and(|(t, _)| t.folder.is_some()) {
        let path = entry.file.path.clone();
        foot = foot.child(
            crate::look::controls::button(&l, &format!("Open {name}"), crate::look::controls::Tone::Neutral, false).on_press(move |s: &mut State| {
                super::reply_actions::open_file(s, super::reply_actions::FileTarget { path: path.clone(), start: 0, end: 0 });
            }),
        );
    }
    if entry.truncated {
        foot = foot.child(text("Only its start was recorded here.").font(type_::LABEL).color(p.muted));
    }
    foot = foot.child(switch("Show the change"));
    col()
        .w_full()
        .gap(6.0)
        .role("region")
        .label(format!("{name}, as written"))
        .child(super::reply_actions::reply_with_copy(s, &locust_core::changes::document_text_of(&entry.file), false, false, p))
        .child(foot)
}

/// DocPreview.tsx's PagePreview (0.425): a new web page, run. Electron frames it inside the card; this window has no
/// frame inside the thread, so its Page runs it at its own address in the browser beside the conversation, and
/// "Show the change" is its source.
fn page_preview(s: &State, key: &str, entry: &locust_core::activity_entries::FileEntry, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let as_change_key = format!("{key}/as-change");
    let name = entry.file.path.replace('\\', "/").rsplit('/').next().unwrap_or(&entry.file.path).to_owned();
    let flip = as_change_key.clone();
    let as_change = s.activity_open.get(&as_change_key).copied().unwrap_or(false);
    let switch = crate::look::controls::button(&l, if as_change { "Show it as a page" } else { "Show the change" }, crate::look::controls::Tone::Neutral, false).on_press(move |s: &mut State| {
        let now = s.activity_open.get(&flip).copied().unwrap_or(false);
        s.activity_open.insert(flip.clone(), !now);
    });
    if as_change {
        return col().w_full().gap(6.0).child(super::review_changes::file_diff(s, None, false, &entry.file, entry.truncated, p)).child(row().child(switch));
    }
    col()
        .w_full()
        .gap(6.0)
        .key(format!("{key}/page"))
        .role("region")
        .label(format!("{name}, running"))
        .child(text(super::reply_page_ui::RUNS_APART).font(type_::LABEL).color(p.muted).w_full().wrap_anywhere())
        .child(row().w_full().gap(8.0).center_y().child(super::reply_page_ui::page_control(format!("{key}/run"), entry.file.path.clone(), p)).child(switch))
}

/// How many of a finished turn's changed files show before "Show N more" (ActivityCard.tsx `FILE_ROWS_SHOWN`).
pub(super) const FILE_ROWS_SHOWN: usize = 5;

/// Thread.tsx `TurnFoot`'s files card: the turn's files one row each, headed by `files_heading`, open until closed.
/// `turn` is the finished turn's items; `scope` keys the card's open state; `mission` is the turn's own id, whose
/// Undo (TurnUndo) sits under the card and, once it has put files back, adds "undone" to the heading (0.840).
pub(super) fn turn_foot_card(turn: &[ledger::Item], p: &Palette, s: &State, scope: &str, mission: Option<&str>) -> Option<El> {
    let workspace = selected_conversation(s).and_then(|(t, _)| t.folder.as_deref());
    let foot = locust_core::turn_foot::turn_foot(turn, workspace)?;
    let key = format!("{scope}/files");
    let open = s.activity_open.get(&key).copied().unwrap_or(true);
    let undone = mission.is_some_and(|m| crate::turn_undo::put_back(s, m));
    let heading = locust_core::turn_foot::files_heading(foot.edited, foot.seen, undone);
    let mut card = col().w_full().gap(4.0).key(format!("{scope}/turn-foot")).child(disclosure(key, heading, open, p));
    if open {
        // ActivityCard.tsx's files variant (0.580): only a new web page opens, wherever it is in the list.
        let entries: Vec<_> = foot.rows.iter().filter_map(|r| match r { locust_core::turn_foot::Row::File { entry, .. } => Some(entry.clone()), _ => None }).collect();
        let page = locust_core::activity_entries::new_page_entry(&entries).map(str::to_owned);
        // A finished turn's files show their first five, as Claude Code's list does (ActivityCard 0.835): the rest are
        // a press away, unless only a row or two would hide behind it. Verbose shows them all.
        let all_key = format!("{scope}/files/all");
        let capped = !verbose(s) && !s.activity_open.get(&all_key).copied().unwrap_or(false) && foot.rows.len() > FILE_ROWS_SHOWN + 2;
        let shown = if capped { &foot.rows[..FILE_ROWS_SHOWN] } else { &foot.rows[..] };
        for row in shown {
            card = card.child(match row {
                locust_core::turn_foot::Row::File { entry, .. } => file_rows(std::slice::from_ref(entry), page.as_deref(), p, s, scope),
                locust_core::turn_foot::Row::Unreported { at, path, .. } => {
                    let settled = !matches!(&turn[*at], ledger::Item::Tool { status: ledger::Status::Running, .. });
                    text(format!(
                        "{} \u{b7} {}",
                        locust_core::path_words::display_path(path, workspace),
                        if settled { "change not reported" } else { "not confirmed" }
                    ))
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .wrap_anywhere()
                }
            });
        }
        if capped {
            let l = crate::look::controls::Look::new(p.dark);
            card = card.child(
                crate::look::controls::ghost_button(&l, &format!("Show {} more", foot.rows.len() - FILE_ROWS_SHOWN))
                    .key(format!("{scope}/files/more"))
                    .on_press(move |s: &mut State| {
                        s.activity_open.insert(all_key.clone(), true);
                    }),
            );
        }
    }
    if let Some(undo) = mission.and_then(|m| crate::turn_undo::view(s, m, scope, p)) {
        card = card.child(undo);
    }
    Some(card)
}

pub(super) fn activity_group(items: &[ledger::Item], p: &Palette, s: &State, scope: &str) -> El {
    let key = format!("{scope}/group/{}", activity_id(&items[0]));
    let open = s.activity_open.get(&key).copied().unwrap_or(verbose(s));
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
    // The line's place in the thread, kept as it grows (missionView's steps key).
    let group_key = key.clone();
    let heading = if listed.is_empty() {
        el().w_full().py(4.0).px(2.0).child(text(label).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere())
    } else {
        disclosure(key, label, open, p)
    };
    let mut group = col().w_full().gap(4.0).key(group_key).child(heading);
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
                    // ONE WAY A THOUGHT READS (ActivityCard 0.515, Colin's two frames of 1 Oct: "i think the first is
                    // cleaner"): inside a step line a thought beside other steps is its words, as a lone thought's are,
                    // never a "Thought for 3s · <headline>" row that opens onto the headline again.
                    locust_core::tool_fold::Row::One(i) if matches!(&items[i], ledger::Item::Thought { .. }) => {
                        let ledger::Item::Thought { text: words, .. } = &items[i] else { unreachable!() };
                        group.child(
                            col().key(format!("{scope}/steps-thought/{}", activity_id(&items[i]))).w_full().child(
                                Markdown::new(without_headline(words).trim().to_owned()).selectable(true).render(&markdown_style(p)).w_full(),
                            ),
                        )
                    }
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
#[path = "note_code_tests.rs"]
mod note_code_tests;

#[cfg(test)]
#[path = "thought_line_tests.rs"]
mod thought_line_tests;
#[cfg(test)]
#[path = "claude_thinking_row_tests.rs"]
mod claude_thinking_row_tests;

#[cfg(test)]
#[path = "background_helper_tests.rs"]
mod background_helper_tests;

#[cfg(test)]
#[path = "plan_unticked_tests.rs"]
mod plan_unticked_tests;
#[cfg(test)]
#[path = "plan_underway_tests.rs"]
mod plan_underway_tests;

#[cfg(test)]
#[path = "nothing_still_running_tests.rs"]
mod nothing_still_running_tests;

#[cfg(test)]
#[path = "reach_says_so_tests.rs"]
mod reach_says_so_tests;

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
                after_end: false,
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

    // a-new-document-reads-as-one.test.tsx: a new Markdown file in the fold reads as its page.
    const NEW_GUIDE: &str = "diff --git a/brand-guide.md b/brand-guide.md\nnew file mode 100644\n--- /dev/null\n+++ b/brand-guide.md\n@@ -0,0 +1,5 @@\n+# Small Bakery Brand Guide\n+\n+## Colour Palette\n+\n+- **Crust** `#6B4226` for headlines";

    fn written(path: &str, patch: &str) -> ledger::Item {
        let mut item = sample_tool("t1", "write", path, false);
        if let ledger::Item::Tool { detail, .. } = &mut item {
            let (a, d) = locust_core::changes::parse(patch).iter().fold((0, 0), |(a, d), f| { let c = f.counts(); (a + c.0, d + c.1) });
            detail.patch = Some(locust_core::activity_entries::Patch { text: patch.into(), added: a, removed: d, truncated: false });
        }
        item
    }

    /// The fold open with its one file open, as Electron's `openByDefault`; and the roles drawn.
    fn doc_fold(item: ledger::Item, folder: bool) -> (String, Vec<(&'static str, String)>) {
        doc_fold_as(item, folder, false)
    }

    fn doc_fold_as(item: ledger::Item, folder: bool, as_change: bool) -> (String, Vec<(&'static str, String)>) {
        let mut s = State::default();
        if as_change {
            s.activity_open.insert("conv/t1/file/0/as-change".into(), true);
        }
        s.activity_open.insert("conv/group/t1".into(), true);
        s.activity_open.insert("conv/t1/file/0".into(), true);
        if folder {
            s.teammates.push(crate::Teammate { id: "tm".into(), name: "Iris".into(), role: "Custom".into(), model: None, runtime: Some("opencode".into()), folder: Some("C:/work".into()), runs_in: "ask".into(), account: None, busy: false });
            s.conversations.insert("tm".into(), Default::default());
        }
        let mut rt = rune::Runtime::new((s, vec![item]), |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"), (700.0, 900.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let all = rt.elements();
        (all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n"), all.iter().map(|e| (e.role, e.label.clone().or(e.text.clone()).unwrap_or_default())).collect())
    }

    #[test]
    fn is_drawn_as_the_page_it_is_with_the_whole_file_one_press_away() {
        let (said, roles) = doc_fold(written("C:/work/brand-guide.md", NEW_GUIDE), true);
        assert!(roles.iter().any(|(r, l)| *r == "region" && l == "brand-guide.md, as written"), "{said}");
        assert!(roles.iter().any(|(r, l)| *r == "heading" && l.contains("Small Bakery Brand Guide")), "{roles:?}");
        assert!(roles.iter().any(|(r, l)| *r == "button" && l == "Open brand-guide.md"), "{roles:?}");
        assert!(roles.iter().any(|(r, l)| *r == "button" && l == "Show the change"), "{roles:?}");
        assert!(!said.contains("## Colour Palette"), "no raw heading marks: {said}");
        assert!(!said.contains("# Small Bakery"), "{said}");
    }

    #[test]
    fn still_opens_a_changed_document_as_its_change_and_a_new_script_as_code() {
        let edited = "diff --git a/brand-guide.md b/brand-guide.md\n--- a/brand-guide.md\n+++ b/brand-guide.md\n@@ -1,3 +1,3 @@\n # Small Bakery Brand Guide\n-Warm and plain.\n+Warm, plain and proud.\n ## Colour Palette";
        let (said, roles) = doc_fold(written("C:/work/brand-guide.md", edited), true);
        assert!(!roles.iter().any(|(r, _)| *r == "region"), "{said}");
        // Both sides drawn as the change, what changed within the line marked apart (diff.ts `pairedSpans`).
        assert!(said.contains("Warm\n and plain\n.") && said.contains("Warm\n, plain and proud\n."), "{said}");
        let (code, roles) = doc_fold(written("C:/work/build.ts", "--- /dev/null\n+++ b/build.ts\n@@ -0,0 +1,1 @@\n+export const x = 1"), true);
        assert!(!roles.iter().any(|(r, _)| *r == "region"), "{code}");
        // Drawn as code: highlighted, so its words are tokens of their own.
        assert!(code.contains("export") && code.contains("const") && code.contains("@@ -0,0 +1 @@"), "{code}");
    }

    #[test]
    fn offers_no_open_where_the_fold_has_no_viewer_to_open() {
        let (_, roles) = doc_fold(written("C:/work/brand-guide.md", NEW_GUIDE), false);
        assert!(roles.iter().any(|(r, l)| *r == "button" && l == "Show the change"));
        assert!(!roles.iter().any(|(_, l)| l == "Open brand-guide.md"), "{roles:?}");
    }

    #[test]
    fn a_new_document_shows_its_change_on_a_press_and_goes_back_to_the_page() {
        let (said, roles) = doc_fold_as(written("C:/work/brand-guide.md", NEW_GUIDE), true, true);
        assert!(!roles.iter().any(|(r, _)| *r == "region"), "{said}");
        assert!(said.contains("@@ -0,0 +1,5 @@"), "the diff: {said}");
        assert!(roles.iter().any(|(r, l)| *r == "button" && l == "Show it as a document"), "{roles:?}");
    }


    /// One shell row drawn alone (ActivityCard's command row), with its elements.
    fn shell_row(command: &str, output: Option<&str>, exit: i32, failed: bool) -> Vec<rune::ElementInfo> {
        let mut item = sample_tool("c1", "shell", "", false);
        if let ledger::Item::Tool { detail, status, .. } = &mut item {
            detail.arguments = command.into();
            detail.output = output.unwrap_or("").into();
            detail.output_reported = output.is_some();
            detail.exit_code = Some(exit.into());
            if failed { *status = ledger::Status::Failed; }
        }
        let mut rt = rune::Runtime::new(
            (State::default(), item),
            |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"),
            (600.0, 200.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
    }

    fn texts(els: &[rune::ElementInfo]) -> Vec<String> { els.iter().filter_map(|e| e.text.clone()).collect() }
    fn opens(els: &[rune::ElementInfo]) -> usize { els.iter().filter(|e| e.role == "button").count() }
    fn badge_fill(els: &[rune::ElementInfo]) -> Option<rune::Color> { els.iter().find(|e| e.key.as_deref() == Some("conv/c1/badge")).map(|e| e.fill) }

    #[test]
    fn a_clean_run_keeps_a_green_exit_badge_after_the_command() {
        let els = shell_row("seq 1 300", Some("1\n2\n3"), 0, false);
        let t = texts(&els);
        let command = t.iter().position(|x| x == "seq 1 300").expect("the command is drawn");
        let badge = t.iter().position(|x| x == "exit 0").expect("the badge is drawn");
        assert!(command < badge, "{t:?}");
        let l = crate::look::controls::Look::new(true);
        assert_eq!(badge_fill(&els), Some(l.c("green-tint")));
    }

    #[test]
    fn a_failing_run_is_red_and_still_says_so() {
        let els = shell_row("pnpm test billing", Some("FAIL src/billing.test.ts"), 1, true);
        assert!(texts(&els).iter().any(|x| x == "failed · exit 1"), "{:?}", texts(&els));
        let l = crate::look::controls::Look::new(true);
        assert_eq!(badge_fill(&els), Some(l.c("red-tint")));
    }

    #[test]
    fn a_command_that_printed_nothing_says_so_and_does_not_offer_to_open() {
        let els = shell_row("chmod +x run.sh", Some(""), 0, false);
        assert!(texts(&els).iter().any(|x| x == "no output"), "{:?}", texts(&els));
        assert_eq!(opens(&els), 0);
    }

    #[test]
    fn a_command_whose_output_the_runtime_never_reported_is_not_called_empty() {
        let els = shell_row("git status", None, 0, false);
        assert!(!texts(&els).iter().any(|x| x.contains("no output")), "{:?}", texts(&els));
        assert_eq!(opens(&els), 0);
    }

    #[test]
    fn a_command_with_output_offers_to_open() {
        let els = shell_row("ls -la", Some("a\nb"), 0, false);
        assert_eq!(opens(&els), 1);
    }

    #[test]
    fn a_described_or_multi_line_command_opens_onto_the_command() {
        assert!(shell_command_folded("Checked the free route", "curl x"));
        assert!(shell_command_folded("", "cd a\nmake"));
        assert!(!shell_command_folded("", "git status"));
        assert!(!shell_command_folded("Checked", ""));
    }

    fn left_edge_items(failed: bool) -> Vec<ledger::Item> {
        let mut edit = sample_tool("e1", "edit", "src/webhook.ts", false);
        if let ledger::Item::Tool { detail, .. } = &mut edit {
            detail.patch = Some(locust_core::activity_entries::Patch {
                text: "--- a/src/webhook.ts\n+++ b/src/webhook.ts\n@@ -1 +1 @@\n-old\n+new\n".into(),
                added: 1, removed: 1, truncated: false,
            });
        }
        let mut run = sample_tool("c1", "shell", "", false);
        if let ledger::Item::Tool { detail, status, .. } = &mut run {
            detail.arguments = if failed { "pnpm test" } else { "rg handleLegacyWebhook src/" }.into();
            detail.output = if failed { "FAIL" } else { "src/webhook.ts:1" }.into();
            detail.output_reported = true;
            detail.exit_code = Some(if failed { 1 } else { 0 });
            if failed { *status = ledger::Status::Failed; }
        }
        if failed { vec![run] } else { vec![edit, run] }
    }

    fn group_elements(items: &[ledger::Item], open: &str) -> Vec<rune::ElementInfo> {
        let mut s = State::default();
        s.activity_open.insert(open.into(), true);
        let mut rt = rune::Runtime::new(
            (s, items.to_vec()),
            |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
            (600.0, 400.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
    }

    #[test]
    fn puts_the_command_before_its_badge_and_the_path_before_done() {
        let els = group_elements(&left_edge_items(false), "conv/group/e1");
        let x = |words: &str| els.iter().find(|e| e.text.as_deref() == Some(words)).map(|e| (e.rect[0], e.rect[1])).unwrap_or_else(|| panic!("{words} not drawn"));
        let path = els.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("src/webhook.ts "))).expect("the file row");
        let (command_x, command_y) = x("rg handleLegacyWebhook src/");
        let (badge_x, badge_y) = x("exit 0");
        assert_eq!(command_x, path.rect[0], "the command starts where the path does");
        assert!(command_x < badge_x && command_y == badge_y, "the badge ends the row");
        assert!(path.text.as_deref().unwrap().ends_with("+1 \u{2212}1"), "the path leads its row");
        // A command with nothing behind it keeps the same edge.
        let mut quiet = left_edge_items(false);
        if let ledger::Item::Tool { detail, .. } = &mut quiet[1] { detail.output = String::new(); }
        let els = group_elements(&quiet, "conv/group/e1");
        let command = els.iter().find(|e| e.text.as_deref() == Some("rg handleLegacyWebhook src/")).expect("the quiet command");
        assert_eq!(command.rect[0], path.rect[0]);
    }

    #[test]
    fn keeps_a_failures_wording_where_the_row_ends() {
        let els = group_elements(&left_edge_items(true), "conv/group/c1");
        let at = |words: &str| els.iter().find(|e| e.text.as_deref() == Some(words)).map(|e| e.rect[0]).unwrap_or_else(|| panic!("{words} not drawn"));
        assert!(at("pnpm test") < at("failed · exit 1"));
        let l = crate::look::controls::Look::new(true);
        assert_eq!(els.iter().find(|e| e.key.as_deref() == Some("conv/c1/badge")).map(|e| e.fill), Some(l.c("red-tint")));
    }

    // case: times the folded line and the open steps
    #[test]
    fn verbose_opens_every_step_of_a_long_transcript() {
        let items: Vec<ledger::Item> = (0..220)
            .map(|n| {
                let mut item = sample_tool(&format!("s{n}"), "bash", &format!("Checked step {n}"), false);
                if let ledger::Item::Tool { detail, .. } = &mut item {
                    detail.arguments = format!("echo step-{n}");
                    detail.output = format!("done {n}");
                    detail.output_reported = true;
                }
                item
            })
            .collect();
        let draw = |verbose: bool| {
            let mut s = State::default();
            if verbose { s.settings_panel.settings.transcript_view = "verbose".into(); }
            let started = std::time::Instant::now();
            let said = group_text(&items, &mut s);
            (said, started.elapsed())
        };
        let (folded, folded_took) = draw(false);
        let (open, open_took) = draw(true);
        assert!(!folded.contains("done 219"), "Normal draws one line");
        assert!(!folded.contains("Checked step 0"));
        assert!(open.contains("Checked step 0") && open.contains("Checked step 219"));
        assert!(open.contains("done 0") && open.contains("done 219"), "Verbose opens each command's output");
        assert!(folded_took.as_millis() < 2000 && open_took.as_millis() < 2000, "{folded_took:?} {open_took:?}");
        // A row the person closed stays closed in Verbose.
        let mut s = State::default();
        s.settings_panel.settings.transcript_view = "verbose".into();
        s.activity_open.insert("conv/s7".into(), false);
        let said = group_text(&items, &mut s);
        assert!(said.contains("Checked step 7") && !said.contains("done 7\n") && said.contains("done 8"));
    }

    #[test]
    fn verbose_switches_for_this_moment_when_there_is_no_store() {
        let mut s = State::default();
        settings_panel::toggle_transcript_view(&mut s);
        assert!(verbose(&s));
        settings_panel::toggle_transcript_view(&mut s);
        assert!(!verbose(&s));
    }
}

#[cfg(test)]
#[path = "thought_folds_to_one_line_tests.rs"]
mod thought_folds_to_one_line_tests;

#[cfg(test)]
#[path = "codex_sandbox_row_tests.rs"]
mod codex_sandbox_row_tests;

#[cfg(test)]
#[path = "activity_drawers_orb_tests.rs"]
mod activity_drawers_orb_tests;
