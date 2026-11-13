//! Settings › AI agents › "AI agents & accounts" (Electron `Screens.tsx`, the runtimes page): each AI agent this Mac
//! has, its tag, its version, and what to do when it is not ready. The words come from `locust_core::agents`, held to
//! Electron's own answers.
//!
//! Electron's rows also carry Install, Sign in and Update buttons that run on the computer. This window does not run
//! those yet, so it does what Electron does when its Install cannot run: it shows the line, with Copy.

use super::*;
use locust_core::agents::{self, Found, RowStatus};

pub(super) const HEADING: &str = "AI agents & accounts";
/// Electron's lede, minus "Claude Code and Codex can also use accounts you add here": named accounts are not built.
const LEDE: &str = "Each AI agent starts with this Mac's sign-in.";
/// Electron's, with "press its Install" made "run its line": this window has no Install button yet (see `shown_detail`).
const NONE_INSTALLED: &str = "None of these are on this machine yet. The quickest start is OpenCode: run its line below, no account, and its free model runs as soon as it is there. The others below are worth adding when you want a stronger model, and each needs its own sign-in.";
const CHECKING: &str = "Checking each AI agent on this Mac…";

/// Ask every agent again, in the background. Tests never run real agents: they set the results themselves.
pub(super) fn request(s: &mut State) {
    #[cfg(not(test))]
    if !s.settings_panel.agents_job.is_pending() {
        s.settings_panel.agents_job.request((), |()| agents::discover());
    }
    #[cfg(test)]
    let _ = s;
}

/// The background check's answer, when it lands.
pub(super) fn poll(s: &mut State) -> bool {
    match s.settings_panel.agents_job.take() {
        Some(found) => {
            s.settings_panel.agents = Some(found);
            true
        }
        None => false,
    }
}

/// RUNTIME_BAND, then the name: what can be used first, what is planned last (runtimeListOrder).
fn band(tag: &str) -> u8 {
    match tag {
        "READY" => 0,
        "EXPERIMENTAL" => 1,
        "SIGN IN" | "AT LIMIT" => 2,
        "NOT INSTALLED" => 3,
        "PLANNED" => 4,
        _ => 3,
    }
}

pub(super) fn ordered(found: &[Found]) -> Vec<(Found, RowStatus)> {
    let mut rows: Vec<(Found, RowStatus)> = found
        .iter()
        .map(|f| (f.clone(), agents::row_status(f, agents::integration_of(&f.id), false, None)))
        .collect();
    if found.iter().any(|f| f.installed) {
        rows.sort_by(|(a, ra), (b, rb)| {
            band(ra.tag).cmp(&band(rb.tag)).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
        });
    } else {
        // Nothing installed: OpenCode, the free start, first.
        rows.sort_by_key(|(f, _)| f.id != "opencode");
    }
    rows
}

/// The row's sentence. Electron's NOT INSTALLED sentence says "Press Install" because its row has an Install button
/// that runs npm; this window shows the line instead, so it says to run it. Everything else is Electron's word for word.
fn shown_detail(f: &Found, status: &RowStatus) -> String {
    if status.tag == "NOT INSTALLED" && agents::install_command(&f.id).is_some() {
        return status
            .detail
            .replace("Press Install, then Sign in.", "Run the line below, then sign in.")
            .replace("Press Install.", "Run the line below.");
    }
    status.detail.clone()
}

/// A line to run, with Copy (Electron's InstallCommand).
fn copy_line(l: &crate::look::controls::Look, key: String, line: String) -> El {
    let copied = line.clone();
    row()
        .gap(8.0)
        .center_y()
        .child(text(line).font(l.ty.mono).color(l.c("text-primary")))
        .child(
            text("Copy")
                .font(l.ty.label)
                .color(l.c("text-secondary"))
                .px(6.0)
                .py(2.0)
                .radius(4.0)
                .hover(l.c("hover-surface"))
                .key(key)
                .role("button")
                .label(format!("Copy: {copied}"))
                .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, copied.clone())),
        )
}

/// Sign in from a button (runtime-sign-in.ts): a Terminal window in the home folder running the agent's own sign-in
/// command, against the program discovery found. Only the command its facts name is ever run. Check again reads it
/// back once the person has signed in there.
fn sign_in(s: &mut State, id: &str) {
    let Some(line) = agents::sign_in_command(id, false) else { return };
    let words: Vec<String> = line.strip_prefix("run ").unwrap_or(&line).split(',').next().unwrap_or("").split_whitespace().map(str::to_owned).collect();
    let Some((program, args)) = words.split_first() else { return };
    if cfg!(test) {
        s.agents_signed_in.push(format!("{program} {}", args.join(" ")).trim().to_owned());
        return;
    }
    let said = (|| {
        let path = crate::runtime::find_executable(program).map_err(|_| "Locust could not find this agent on the machine. Install it first, then sign in.".to_owned())?;
        let store = s.data.clone().ok_or("Locust could not open its data folder.")?;
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).ok_or("Locust could not find your home folder.")?;
        let script = store.attachments_dir().map_err(|e| e.to_string())?.join(format!("sign-in-{}.command", locust_core::invariants::unique_nanos()));
        locust_core::runtime_terminal::open(&path, args, &home, &script, &store).map_err(|_| "Terminal could not be opened for the sign-in. Run the command shown in a terminal.".to_owned())
    })();
    s.agents_sign_in_said = said.err();
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut body = col()
        .key("agents-section")
        .gap(10.0)
        .child(text(HEADING.to_owned()).font(type_::TITLE).color(p.text))
        .child(text(LEDE.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    let Some(found) = &s.settings_panel.agents else {
        return body.child(text(CHECKING.to_owned()).font(l.ty.body).color(l.c("text-muted")));
    };
    if !found.is_empty() && !found.iter().any(|f| f.installed) {
        body = body.child(text(NONE_INSTALLED.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    let mut list = col()
        .gap(0.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-card"));
    for (f, status) in ordered(found) {
        let mut name = row()
            .gap(8.0)
            .center_y()
            .child(text(f.display_name.clone()).font(l.ty.control).color(l.c("text-primary")));
        if let Some(v) = &f.version {
            name = name.child(text(v.clone()).font(l.ty.mono).color(l.c("text-muted")));
        }
        let mut words = col().gap(4.0).grow(1.0).shrink().child(name);
        // "Signed in on this machine" is said once, in the lede: a row speaks only when its state is not the ordinary one.
        if status.tag != "READY" {
            words = words.child(text(shown_detail(&f, &status)).font(l.ty.body).color(l.c("text-secondary")).wrap());
        }
        if status.tag == "NOT INSTALLED"
            && let Some(command) = agents::install_command(&f.id)
        {
            words = words.child(copy_line(&l, format!("agents-copy-install-{}", f.id), command));
        }
        if status.tag == "SIGN IN"
            && let Some(line) = agents::sign_in_command(&f.id, false)
        {
            let command = line.strip_prefix("run ").unwrap_or(&line).to_owned();
            let id = f.id.clone();
            words = words
                .child(copy_line(&l, format!("agents-copy-sign-in-{}", f.id), command))
                .child(crate::look::controls::button(&l, "Sign in", crate::look::controls::Tone::Neutral, false).key(format!("agents-sign-in-{}", f.id)).self_start().on_press(move |s: &mut State| sign_in(s, &id)));
        }
        let tag = text(status.tag.to_owned())
            .font(l.ty.label)
            .color(if status.selectable { l.c("text-primary") } else { l.c("text-secondary") })
            .px(8.0)
            .py(2.0)
            .radius(999.0)
            .border(1.0, l.c("border-card"))
            // A row with a sentence under its name is taller than the tag: the tag keeps its own height at the top.
            .self_start();
        list = list.child(
            row()
                .key(format!("agents-row-{}", f.id))
                .gap(12.0)
                .padding(12.0)
                .w_full()
                .child(words)
                .child(tag),
        );
    }
    if let Some(said) = &s.agents_sign_in_said {
        body = body.child(text(said.clone()).font(l.ty.body).color(l.c("amber-text")).wrap());
    }
    body.child(list).child(
        text("Check again")
            .font(l.ty.label)
            .color(l.c("text-secondary"))
            .px(6.0)
            .py(2.0)
            .radius(4.0)
            .hover(l.c("hover-surface"))
            .key("agents-check-again")
            .role("button")
            .on_press(request),
    )
}

/// Settings › AI agents with made-up discovery results, one of each kind of row: for `--snap` frames.
pub(super) fn scene(s: &mut State) {
    use locust_core::agents::{Auth, Probe};
    crate::settings_panel::scene(s);
    s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    let agent = |id: &str, name: &str, version: Option<&str>, auth: Auth, status: Probe| Found {
        id: id.into(),
        display_name: name.into(),
        installed: status != Probe::Missing,
        version: version.map(str::to_owned),
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: id == "antigravity",
        checking: false,
    };
    s.settings_panel.agents = Some(vec![
        agent("codex", "Codex CLI", Some("0.167.2"), Auth::Authenticated, Probe::Ready),
        agent("claude", "Claude Code", Some("2.1.322"), Auth::Unauthenticated, Probe::AuthRequired),
        agent("cursor", "Cursor Agent", Some("2026.11.10-731fb78"), Auth::Authenticated, Probe::Ready),
        agent("antigravity", "Antigravity", Some("1.2.14"), Auth::Authenticated, Probe::Ready),
        agent("opencode", "OpenCode", None, Auth::NotApplicable, Probe::Missing),
        agent("kilo", "Kilo", Some("7.8.18"), Auth::NotApplicable, Probe::Ready),
        agent("muse", "Muse Code", None, Auth::Unknown, Probe::Missing),
        agent("gemini", "Gemini CLI", Some("0.66.0"), Auth::Authenticated, Probe::Ready),
    ]);
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}
