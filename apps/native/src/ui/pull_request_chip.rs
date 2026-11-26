//! THE FOLDER'S PULL REQUEST, IN THE HEADER, ON LINKS, AND WATCHED FOR NEWS.
//!
//! Port of Electron's `PullRequestChip.tsx`, `PullRequestNewsBanner.tsx`, and `PullLinkChipView`.

use super::*;
use locust_core::{
    folder_commit::{FolderCommits, RealNetwork},
    pull_requests::{
        self, ConversationPiece, FolderPullRequest, IssueLinkState, LinkWatch, LinkedIssue,
        LinkedPullRequest, PullLinkAddress, PullLinkChecks, PullLinkReader, PullRequestChecks,
        PullRequestNews, PullRequestState, PullRequestWatch, conversation_pieces, is_pull_link_moving,
        issue_link_hover, issue_link_word, now_ms, pull_link_hover, pull_link_word, pull_request_ask,
        pull_request_line,
    },
};
use rune::Icon;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};

const WATCHED_FILE: &str = "watched_pull_requests.json";
const MAX_WATCHED_URLS: usize = 50;

pub static GITHUB_MARK: Icon = Icon {
    name: "github",
    view: 16.0,
    body: r#"<path fill="currentColor" d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/>"#,
};

pub(super) struct PrState {
    pub commits: Arc<FolderCommits>,
    pub link_reader: Arc<PullLinkReader>,
    pub read_job: Latest<(PathBuf, Option<FolderPullRequest>)>,
    /// Core's watch (Electron main's `createPullRequestWatch`, index.ts 3566): it keeps what is watched, compares each
    /// look with the one before and stops on a merge. The window is its `schedule`: while `timer_armed`, it asks for a
    /// look every `every_ms`. Its looks run `gh` off this thread (`watch_off_thread`).
    pub watch: Arc<PullRequestWatch>,
    /// What the watch reads: the folder whose pull request is watched, and the commits reader (kept current in `poll`).
    pub watch_target: Arc<Mutex<(Option<PathBuf>, Arc<FolderCommits>)>>,
    pub watch_said: Receiver<WatchSaid>,
    /// Watch calls still running off the UI thread, so `poll` keeps coming back for what they say.
    pub watch_out: Arc<AtomicUsize>,
    pub folder: Option<PathBuf>,
    pub current: Option<(PathBuf, Option<FolderPullRequest>)>,
    pub was_running: bool,
    pub was_focused: bool,
    pub watched_urls: Vec<String>,
    pub watched_loaded: bool,
    pub next_watch_at: Option<Instant>,
    pub news: Option<PullRequestNews>,
    pub link_watch: LinkWatch<LinkedPullRequest>,
    pub issue_watch: LinkWatch<LinkedIssue>,
    pub link_pending: HashSet<PullLinkAddress>,
    pub issue_pending: HashSet<PullLinkAddress>,
    pub link_tx: Sender<(PullLinkAddress, Option<LinkedPullRequest>)>,
    pub link_rx: Receiver<(PullLinkAddress, Option<LinkedPullRequest>)>,
    pub issue_tx: Sender<(PullLinkAddress, Option<LinkedIssue>)>,
    pub issue_rx: Receiver<(PullLinkAddress, Option<LinkedIssue>)>,
    pub next_link_tick_at: Option<u64>,
    pub record_opened_urls: bool,
    pub opened_urls: Vec<String>,
}

/// What core's watch tells the window: a look's pull request (for the chip), or news (Electron's `onNews`).
pub(super) enum WatchSaid {
    Read(PathBuf, Option<FolderPullRequest>),
    News(PullRequestNews),
}

/// Core's watch over `target`: each look reads the folder's pull request as the window last named it, and both what it
/// read and any news come back on one channel.
fn core_watch(target: &Arc<Mutex<(Option<PathBuf>, Arc<FolderCommits>)>>) -> (Arc<PullRequestWatch>, Receiver<WatchSaid>) {
    let (said_tx, said_rx) = channel();
    let (read_tx, target) = (said_tx.clone(), target.clone());
    let watch = PullRequestWatch::new(
        move || {
            let (folder, commits) = target.lock().map(|t| t.clone()).ok()?;
            let folder = folder?;
            let pr = commits.pull_request(&folder);
            let _ = read_tx.send(WatchSaid::Read(folder, pr.clone()));
            pr
        },
        move |news| {
            let _ = said_tx.send(WatchSaid::News(news));
        },
    );
    (Arc::new(watch), said_rx)
}

impl Default for PrState {
    fn default() -> Self {
        let (link_tx, link_rx) = channel();
        let (issue_tx, issue_rx) = channel();
        let commits = Arc::new(FolderCommits::default());
        let watch_target = Arc::new(Mutex::new((None, commits.clone())));
        let (watch, watch_said) = core_watch(&watch_target);
        Self {
            commits,
            link_reader: Arc::new(PullLinkReader::new(Arc::new(RealNetwork))),
            read_job: Latest::default(),
            watch,
            watch_target,
            watch_said,
            watch_out: Arc::new(AtomicUsize::new(0)),
            folder: None,
            current: None,
            was_running: false,
            was_focused: true,
            watched_urls: Vec::new(),
            watched_loaded: false,
            next_watch_at: None,
            news: None,
            link_watch: LinkWatch::default(),
            issue_watch: LinkWatch::default(),
            link_pending: HashSet::new(),
            issue_pending: HashSet::new(),
            link_tx,
            link_rx,
            issue_tx,
            issue_rx,
            next_link_tick_at: None,
            record_opened_urls: false,
            opened_urls: Vec::new(),
        }
    }
}

fn active_folder(s: &State) -> Option<PathBuf> {
    selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(PathBuf::from)
}

fn load_watched_if_needed(s: &mut State) {
    if s.pr.watched_loaded {
        return;
    }
    let Some(store) = &s.data else { return };
    s.pr.watched_loaded = true;
    if let Ok(raw) = std::fs::read_to_string(store.root().join(WATCHED_FILE))
        && let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        s.pr.watched_urls = arr
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .take(MAX_WATCHED_URLS)
            .collect();
    }
}

fn save_watched(s: &State) {
    let Some(store) = &s.data else { return };
    let list: Vec<serde_json::Value> = s
        .pr
        .watched_urls
        .iter()
        .take(MAX_WATCHED_URLS)
        .cloned()
        .map(serde_json::Value::String)
        .collect();
    let _ = store.save_json(WATCHED_FILE, &serde_json::Value::Array(list));
}

pub(super) fn open_github_url(s: &mut State, url: &str) {
    if pull_requests::github_link_of(url).is_none() {
        return;
    }
    if s.pr.record_opened_urls {
        s.pr.opened_urls.push(url.to_owned());
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("/usr/bin/open");
        cmd.arg(url);
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        let _ = cmd.spawn();
    }
}

fn start_folder_read(s: &mut State, folder: PathBuf) {
    let commits = s.pr.commits.clone();
    s.pr.read_job.request(folder, move |folder| {
        let pr = commits.pull_request(&folder);
        (folder, pr)
    });
}

/// A call into core's watch, off the UI thread: `watch(url, true)` and `look` both run `gh`.
fn watch_off_thread(s: &State, call: impl FnOnce(&PullRequestWatch) + Send + 'static) {
    let (watch, out) = (s.pr.watch.clone(), s.pr.watch_out.clone());
    out.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        call(&watch);
        out.fetch_sub(1, Ordering::SeqCst);
    });
}

/// One look now, as a tick of the watch's timer would ask (test seam for headless drives, Electron's `look`).
#[cfg(test)]
pub(super) fn watch_look_now(s: &mut State) {
    s.pr.commits.forget_sign_in();
    watch_off_thread(s, PullRequestWatch::look);
}

fn is_watched(s: &State, url: &str) -> bool {
    s.pr.watched_urls.iter().any(|u| u == url)
}

/// The window's own list (PullRequestChip.tsx `keepWatched`), and core's watch told the same.
fn set_watched(s: &mut State, url: &str, on: bool) {
    s.pr.watched_urls.retain(|u| u != url);
    if on {
        s.pr.watched_urls.insert(0, url.to_owned());
        s.pr.watched_urls.truncate(MAX_WATCHED_URLS);
        // The first look is the one later ones are compared with (pull-request-watch.ts `watch`).
        let url = url.to_owned();
        watch_off_thread(s, move |watch| watch.watch(&url, true));
    } else {
        s.pr.watch.watch(url, false);
    }
    save_watched(s);
}

fn current_pr(s: &State) -> Option<&FolderPullRequest> {
    let folder = active_folder(s)?;
    s.pr.current
        .as_ref()
        .filter(|(f, _)| *f == folder)
        .and_then(|(_, pr)| pr.as_ref())
}

fn toggle_watch(s: &mut State, tx: &mut Tx) {
    let Some(pr) = current_pr(s).cloned() else { return };
    if !matches!(pr.state, PullRequestState::Open | PullRequestState::Draft) {
        return;
    }
    let on = !is_watched(s, &pr.url);
    set_watched(s, &pr.url, on);
    tx.wake_after(Duration::from_millis(50));
}

fn collect_conversation_links(s: &State) -> (Vec<PullLinkAddress>, Vec<PullLinkAddress>) {
    let mut pulls = Vec::new();
    let mut issues = Vec::new();
    let mut seen_pulls = HashSet::new();
    let mut seen_issues = HashSet::new();
    let Some(pane) = &s.ledger else {
        return (pulls, issues);
    };
    for item in &pane.items {
        let text_body = match item {
            ledger::Item::User(body) => body.as_str(),
            ledger::Item::Assistant { text, .. } => text.as_str(),
            _ => continue,
        };
        for piece in conversation_pieces(text_body) {
            if let ConversationPiece::Link { pull, issue, .. } = piece {
                if let Some(addr) = pull
                    && seen_pulls.insert(addr.clone())
                {
                    pulls.push(addr);
                }
                if let Some(addr) = issue
                    && seen_issues.insert(addr.clone())
                {
                    issues.push(addr);
                }
            }
        }
    }
    (pulls, issues)
}

fn spawn_pull_link_read(s: &mut State, addr: PullLinkAddress) {
    if !s.pr.link_pending.insert(addr.clone()) {
        return;
    }
    let reader = s.pr.link_reader.clone();
    let tx = s.pr.link_tx.clone();
    std::thread::spawn(move || {
        let value = reader.read_address(&addr);
        let _ = tx.send((addr, value));
    });
}

fn spawn_issue_link_read(s: &mut State, addr: PullLinkAddress) {
    if !s.pr.issue_pending.insert(addr.clone()) {
        return;
    }
    let reader = s.pr.link_reader.clone();
    let tx = s.pr.issue_tx.clone();
    std::thread::spawn(move || {
        let value = reader.read_issue_address(&addr);
        let _ = tx.send((addr, value));
    });
}

pub(super) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    load_watched_if_needed(s);

    let running = turn_running(s);
    let folder = active_folder(s);
    let focus_regained = !s.pr.was_focused && s.window_focused;
    let run_ended = s.pr.was_running && !running;
    let folder_changed = s.pr.folder != folder;
    s.pr.was_running = running;
    s.pr.was_focused = s.window_focused;

    if let Ok(mut target) = s.pr.watch_target.lock() {
        *target = (folder.clone(), s.pr.commits.clone());
    }
    if folder_changed {
        s.pr.folder = folder.clone();
        s.pr.read_job.cancel();
        if folder.is_none() {
            s.pr.current = None;
            changed = true;
        }
    }

    if let Some(folder_path) = &folder
        && !running
        && (folder_changed || run_ended || focus_regained)
    {
        start_folder_read(s, folder_path.clone());
    }

    if let Some((read_folder, found)) = s.pr.read_job.take()
        && folder.as_ref() == Some(&read_folder)
    {
        if let Some(pr) = &found {
            if matches!(pr.state, PullRequestState::Open | PullRequestState::Draft) {
                // A pull request watched before is watched again when it shows (PullRequestChip.tsx, 0.731).
                if is_watched(s, &pr.url) && !s.pr.watch.watching(&pr.url) {
                    let url = pr.url.clone();
                    watch_off_thread(s, move |watch| watch.watch(&url, true));
                }
            } else if is_watched(s, &pr.url) {
                set_watched(s, &pr.url, false);
            }
        }
        s.pr.current = Some((read_folder, found));
        changed = true;
    }

    // The watch's timer: one while anything is watched, none otherwise (`timer_armed`); each tick is one look.
    if s.pr.watch.timer_armed() {
        let every = Duration::from_millis(s.pr.watch.every_ms());
        let now = Instant::now();
        let next_at = *s.pr.next_watch_at.get_or_insert(now + every);
        if now >= next_at {
            watch_off_thread(s, PullRequestWatch::look);
            s.pr.next_watch_at = Some(now + every);
            tx.wake_after(every);
        } else {
            tx.wake_after(next_at.saturating_duration_since(now));
        }
    } else {
        s.pr.next_watch_at = None;
    }

    // What the watch read and said: the chip takes the newest read; news is the banner; a merged or closed one the
    // watch has let go is no longer on the window's list either.
    while let Ok(said) = s.pr.watch_said.try_recv() {
        match said {
            WatchSaid::News(news) => s.pr.news = Some(news),
            WatchSaid::Read(read_folder, found) => {
                if folder.as_ref() != Some(&read_folder) {
                    continue;
                }
                if let Some(pr) = &found
                    && matches!(pr.state, PullRequestState::Merged | PullRequestState::Closed)
                    && is_watched(s, &pr.url)
                {
                    set_watched(s, &pr.url.clone(), false);
                }
                if found.is_some() {
                    s.pr.current = Some((read_folder, found));
                }
            }
        }
        changed = true;
    }
    if s.pr.watch_out.load(Ordering::SeqCst) > 0 {
        tx.wake_after(Duration::from_millis(50));
    }

    // Conversation links: read once, refresh on focus after 1 min, poll every 3 min while running.
    let now_epoch = now_ms();
    while let Ok((addr, val)) = s.pr.link_rx.try_recv() {
        s.pr.link_pending.remove(&addr);
        s.pr.link_watch.finish_read(&addr, val, now_epoch, is_pull_link_moving);
        changed = true;
    }
    while let Ok((addr, val)) = s.pr.issue_rx.try_recv() {
        s.pr.issue_pending.remove(&addr);
        s.pr.issue_watch.finish_read(&addr, val, now_epoch, |_| false);
        changed = true;
    }

    // Each link drawn now holds one listener; a link that left the screen lets its listener go.
    let (pulls, issues) = collect_conversation_links(s);
    for addr in s.pr.link_watch.show(&pulls, now_epoch, is_pull_link_moving) {
        spawn_pull_link_read(s, addr);
    }
    for addr in s.pr.issue_watch.show(&issues, now_epoch, |_| false) {
        spawn_issue_link_read(s, addr);
    }

    if focus_regained {
        for addr in s.pr.link_watch.on_focus(now_epoch) {
            spawn_pull_link_read(s, addr);
        }
        for addr in s.pr.issue_watch.on_focus(now_epoch) {
            spawn_issue_link_read(s, addr);
        }
    }

    let period = s.pr.link_watch.again_ms();
    let armed = s.pr.link_watch.timer_armed();
    if let Some((read, wait)) = link_tick(&mut s.pr.next_link_tick_at, armed, now_epoch, period) {
        if read {
            for addr in s.pr.link_watch.on_tick(is_pull_link_moving) {
                spawn_pull_link_read(s, addr);
            }
        }
        tx.wake_after(Duration::from_millis(wait));
    }

    changed
}

/// Wait a full period from arming, even if the window has been open for hours.
fn link_tick(next: &mut Option<u64>, armed: bool, now: u64, period: u64) -> Option<(bool, u64)> {
    if !armed {
        *next = None;
        return None;
    }
    let due = *next.get_or_insert_with(|| now.saturating_add(period));
    if now >= due {
        *next = Some(now.saturating_add(period));
        Some((true, period))
    } else {
        Some((false, due - now))
    }
}

#[cfg(test)]
mod link_tick_tests {
    use super::link_tick;

    #[test]
    fn a_new_timer_waits_a_full_interval_each_time_it_is_armed() {
        let mut next = None;
        assert_eq!(link_tick(&mut next, true, 600_000, 180_000), Some((false, 180_000)));
        assert_eq!(link_tick(&mut next, true, 779_999, 180_000), Some((false, 1)));
        assert_eq!(link_tick(&mut next, true, 780_000, 180_000), Some((true, 180_000)));
        assert_eq!(link_tick(&mut next, false, 800_000, 180_000), None);
        assert_eq!(link_tick(&mut next, true, 810_000, 180_000), Some((false, 180_000)));
    }
}

fn pr_tone_name(pr: &FolderPullRequest) -> &'static str {
    match pr.state {
        PullRequestState::Merged => "blue",
        PullRequestState::Closed | PullRequestState::Draft => "text-muted",
        PullRequestState::Open => match pr.checks {
            PullRequestChecks::Failing => "red",
            PullRequestChecks::Pending => "amber",
            PullRequestChecks::Passing | PullRequestChecks::None => "green",
        },
    }
}

/// The pull request chip itself, from `FolderPullRequest`: what a test draws.
pub(super) fn chip_view(pr: &FolderPullRequest, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let line = pull_request_line(pr);
    let tone = pr_tone_name(pr);
    let url = pr.url.clone();
    row()
        .key("pr-chip")
        .h(26.0)
        .px(8.0)
        .gap(5.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .fill(l.c("bg-control"))
        .border(1.0, l.c("border-control"))
        .hover(l.c("hover-surface"))
        .focusable(true)
        .focus_ring(2.0, 2.0, l.c("focus-ring"))
        .role("button")
        .label(format!("Pull request {}, {}. Opens on GitHub.", pr.number, line))
        .help(format!("Pull request #{}: {}\n{}\nOpens on GitHub", pr.number, pr.title, line))
        .child(rune::icon(&GITHUB_MARK, 13.0).key("pr-chip-mark").color(l.c("text-secondary")))
        .child(
            text(format!("#{}", pr.number))
                .font(type_::LABEL)
                .color(l.c("text-secondary")),
        )
        .child(el().size(6.0, 6.0).radius(3.0).fill(l.c(tone)))
        .child(
            text(pr.state.word())
                .font(type_::LABEL)
                .color(l.c("text-primary")),
        )
        .on_press(move |s: &mut State| open_github_url(s, &url))
}

/// Watch / Watching toggle beside the chip for open or draft pull requests.
pub(super) fn watch_toggle(watching: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let label = if watching { "Watching" } else { "Watch" };
    let (border, fill, ink) = if watching {
        ("lime-border", "lime-tint", "lime-text")
    } else {
        ("border-control", "bg-control", "text-secondary")
    };
    row()
        .key("pr-watch-toggle")
        .h(26.0)
        .px(8.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .fill(l.c(fill))
        .border(1.0, l.c(border))
        .hover(l.c("hover-surface"))
        .focusable(true)
        .focus_ring(2.0, 2.0, l.c("focus-ring"))
        .role("button")
        .label(label)
        .child(text(label).font(type_::LABEL).color(l.c(ink)))
        .on_press_tx(|s: &mut State, tx: &mut Tx| toggle_watch(s, tx))
}

/// Header control for the active conversation's folder pull request.
pub(super) fn control(s: &State, p: &Palette) -> El {
    let Some(pr) = current_pr(s) else {
        return row();
    };
    let mut out = row().gap(4.0).center_y().child(chip_view(pr, p));
    if matches!(pr.state, PullRequestState::Open | PullRequestState::Draft) {
        out = out.child(watch_toggle(is_watched(s, &pr.url), p));
    }
    out
}

/// News banner above the composer when a watched pull request changes.
pub(super) fn news_banner(s: &State, p: &Palette) -> Option<El> {
    let news = s.pr.news.as_ref()?;
    let teammate_name = selected_conversation(s).map(|(t, _)| t.name.as_str());
    Some(news_banner_view(news, teammate_name, p))
}

pub(super) fn news_banner_view(
    news: &PullRequestNews,
    teammate_name: Option<&str>,
    p: &Palette,
) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let (border, fill) = if news.needs_work {
        ("amber-border", "amber-tint")
    } else {
        ("border-card", "bg-raised")
    };
    let mut actions = row().gap(6.0).center_y();
    if news.needs_work {
        let ask_label = match teammate_name {
            Some(name) => format!("Ask {name} to fix it"),
            None => "Ask to fix it".to_owned(),
        };
        let ask_text = pull_request_ask(news);
        actions = actions.child(
            crate::look::controls::button(&l, &ask_label, crate::look::controls::Tone::Neutral, false)
                .key("pr-news-ask")
                .on_press(move |s: &mut State| {
                    s.draft.replace_text(&ask_text);
                    s.draft.focused = true;
                }),
        );
    }
    let open_url = news.url.clone();
    actions = actions
        .child(
            crate::look::controls::button(&l, "Open on GitHub", crate::look::controls::Tone::Neutral, false)
                .key("pr-news-open")
                .on_press(move |s: &mut State| open_github_url(s, &open_url)),
        )
        .child(
            crate::look::controls::button(&l, "Dismiss", crate::look::controls::Tone::Neutral, false)
                .key("pr-news-dismiss")
                .on_press(|s: &mut State| {
                    s.pr.news = None;
                }),
        );

    row()
        .key("pr-news-banner")
        .w_full()
        .px(12.0)
        .py(8.0)
        .gap(10.0)
        .center_y()
        .radius(l.t.px("radius-card"))
        .fill(l.c(fill))
        .border(1.0, l.c(border))
        .child(
            text(format!("Pull request #{}: {}.", news.number, news.said))
                .font(type_::LABEL)
                .color(l.c("text-primary"))
                .grow(1.0)
                .shrink()
                .wrap(),
        )
        .child(actions)
}

/// Chip beside/below a pull request link in a conversation reply.
pub(super) fn pull_link_chip(link: &LinkedPullRequest, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let hover = pull_link_hover(link);
    let word = pull_link_word(link.state);
    let mut chip = row()
        .key("pr-link-chip")
        .py(2.0)
        .px(7.0)
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .fill(l.c("bg-control"))
        .border(1.0, l.c("border-card"))
        .role("status")
        .label(hover)
        .child(text(word).font(type_::LABEL).color(l.c("text-secondary")));
    if link.checks != PullLinkChecks::None {
        let dot_tone = match link.checks {
            PullLinkChecks::Failing => "red",
            PullLinkChecks::Running => "amber",
            PullLinkChecks::Passing => "green",
            PullLinkChecks::None => "text-muted",
        };
        chip = chip.child(el().key("pr-link-mark").size(6.0, 6.0).radius(3.0).fill(l.c(dot_tone)));
    }
    chip
}

/// Renders a person's message words, turning bare GitHub pull-request/issue URLs and markdown links into clickable links.
pub(super) fn linked_words(body: &str, style: rune::TextStyle, p: &Palette) -> El {
    let pieces = conversation_pieces(body);
    let has_link = pieces.iter().any(|piece| matches!(piece, ConversationPiece::Link { .. }));
    if !has_link {
        return text(body.to_owned())
            .font(style)
            .color(p.text)
            .shrink()
            .wrap_anywhere()
            .selectable(true);
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut line = row().flex_wrap().shrink();
    for piece in pieces {
        match piece {
            ConversationPiece::Text(words) => {
                line = line.child(
                    text(words)
                        .font(style)
                        .color(p.text)
                        .shrink()
                        .wrap_anywhere()
                        .selectable(true),
                );
            }
            ConversationPiece::Link { text: label, href, .. } => {
                let url = href.clone();
                line = line.child(
                    text(label)
                        .key("user-link")
                        .font(style)
                        .color(l.c("blue"))
                        .shrink()
                        .wrap_anywhere()
                        .role("link")
                        .label(href)
                        .on_press(move |s: &mut State| open_github_url(s, &url)),
                );
            }
        }
    }
    line
}

/// Chip for a linked GitHub issue in a conversation reply.
pub(super) fn issue_link_chip(link: &LinkedIssue, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let hover = issue_link_hover(link);
    let word = issue_link_word(link.state);
    // shell.css `.lc-prlink[data-issue-state=...]`: open green, completed violet, not planned grey; never a pull
    // request's red, which would say a finished issue failed.
    let (tone, ink, tint) = match link.state {
        IssueLinkState::Open => ("green", "green-text", "green-tint"),
        IssueLinkState::ClosedCompleted => ("violet", "violet-text", "violet-tint"),
        IssueLinkState::ClosedNotPlanned => ("text-muted", "text-muted", "chip-bg"),
    };
    row()
        .key("issue-link-chip")
        .py(2.0)
        .px(7.0)
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .fill(l.c(tint))
        .border(1.0, l.c("border-card"))
        .role("status")
        .label(hover.clone())
        .help(hover)
        .child(el().key("issue-link-dot").size(6.0, 6.0).radius(3.0).fill(l.c(tone)))
        .child(text(word).key("issue-link-state").font(type_::LABEL).color(l.c(ink)))
}

/// Renders link state chips for any GitHub pull request or issue links in a message that have loaded.
pub(super) fn link_chips_for_text(s: &State, body: &str, p: &Palette) -> Option<El> {
    let mut chips = Vec::new();
    let mut seen_pulls = HashSet::new();
    let mut seen_issues = HashSet::new();
    for piece in conversation_pieces(body) {
        if let ConversationPiece::Link { href, pull, issue, .. } = piece {
            if let Some(addr) = pull
                && seen_pulls.insert(addr.clone())
                && let Some(link) = s.pr.link_watch.current(&addr)
            {
                let url = href.clone();
                chips.push(
                    row()
                        .gap(4.0)
                        .center_y()
                        .child(text(format!("{}#{}:", addr.repo, addr.number)).font(type_::LABEL).color(p.muted))
                        .child(pull_link_chip(link, p))
                        .on_press(move |s: &mut State| open_github_url(s, &url)),
                );
            }
            if let Some(addr) = issue
                && seen_issues.insert(addr.clone())
                && let Some(link) = s.pr.issue_watch.current(&addr)
            {
                let url = href.clone();
                chips.push(
                    row()
                        .gap(4.0)
                        .center_y()
                        .child(text(format!("{}#{}:", addr.repo, addr.number)).font(type_::LABEL).color(p.muted))
                        .child(issue_link_chip(link, p))
                        .on_press(move |s: &mut State| open_github_url(s, &url)),
                );
            }
        }
    }
    if chips.is_empty() {
        return None;
    }
    let mut strip = row().gap(8.0).wrap();
    for chip in chips {
        strip = strip.child(chip);
    }
    Some(strip)
}

#[cfg(test)]
#[path = "pull_request_news_tests.rs"]
mod pull_request_news_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    struct TestHarness {
        state: State,
        build: Box<dyn Fn(&State) -> El>,
    }

    fn harness_view(h: &TestHarness, _: &mut Cx<TestHarness>) -> El {
        col().w_full().h_full().child((h.build)(&h.state))
    }

    fn render_tree(build: impl Fn(&State) -> El + 'static) -> Runtime<TestHarness> {
        let mut rt = Runtime::new(
            TestHarness {
                state: State::default(),
                build: Box::new(build),
            },
            harness_view,
            (800.0, 200.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt
    }

    fn collect_text(rt: &Runtime<TestHarness>) -> String {
        let mut out = String::new();
        for node in rt.elements() {
            if let Some(t) = node.text {
                out.push_str(&t);
                out.push('\n');
            }
            if let Some(lbl) = node.label {
                out.push_str(&lbl);
                out.push('\n');
            }
        }
        out
    }

    // --- a-pull-request-chip-says-its-state.test.tsx ---

    // case: names the pull request by number and state, with GitHub’s mark, and the whole line on hover
    #[test]
    fn the_chip_names_the_number_and_state_with_githubs_mark_and_the_whole_line_on_hover() {
        let pr = FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Open,
            checks: PullRequestChecks::Passing,
            check_count: 2,
            not_passed: 0,
            review: None,
            conflicts: false,
            updated_at: None,
        };
        let rt = render_tree(move |_| chip_view(&pr, &Palette::new(true)));
        let all = rt.elements();
        let chip = all.iter().find(|e| e.key.as_deref() == Some("pr-chip")).expect("the chip");
        // Electron's title attribute, word for word.
        assert_eq!(
            chip.help.as_deref(),
            Some("Pull request #17: Fix the cart total\nOpen · all 2 checks passed\nOpens on GitHub")
        );
        let mark = all.iter().find(|e| e.key.as_deref() == Some("pr-chip-mark")).expect("GitHub's mark");
        assert_eq!(mark.role, "icon");
        assert_eq!(GITHUB_MARK.view, 16.0, "GitHub's mark is drawn in a 16 x 16 box, like Electron's viewBox");
        assert!(GITHUB_MARK.body.contains("M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59"));
        let texts: Vec<String> = all.iter().filter_map(|e| e.text.clone()).collect();
        assert_eq!(texts, vec!["#17".to_string(), "Open".to_string()]);
    }

    // case: wears its checks’ tone while open, and merged’s own once merged
    #[test]
    fn pull_request_chip_names_number_state_and_line() {
        let pr = FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Open,
            checks: PullRequestChecks::Passing,
            check_count: 2,
            not_passed: 0,
            review: None,
            conflicts: false,
            updated_at: None,
        };
        let rt = render_tree(move |_| chip_view(&pr, &Palette::new(true)));
        let text = collect_text(&rt);
        assert!(text.contains("#17"));
        assert!(text.contains("Open"));
        assert!(text.contains("Pull request 17, Open · all 2 checks passed. Opens on GitHub."));
        assert_eq!(pr_tone_name(&FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Open,
            checks: PullRequestChecks::Failing,
            check_count: 2,
            not_passed: 1,
            review: None,
            conflicts: false,
            updated_at: None,
        }), "red");
        assert_eq!(pr_tone_name(&FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Open,
            checks: PullRequestChecks::Pending,
            check_count: 2,
            not_passed: 1,
            review: None,
            conflicts: false,
            updated_at: None,
        }), "amber");
        assert_eq!(pr_tone_name(&FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Merged,
            checks: PullRequestChecks::Failing,
            check_count: 2,
            not_passed: 1,
            review: None,
            conflicts: false,
            updated_at: None,
        }), "blue");
        assert_eq!(pr_tone_name(&FolderPullRequest {
            number: 17,
            title: "Fix the cart total".into(),
            url: "https://github.com/someone/pebble/pull/17".into(),
            state: PullRequestState::Draft,
            checks: PullRequestChecks::None,
            check_count: 0,
            not_passed: 0,
            review: None,
            conflicts: false,
            updated_at: None,
        }), "text-muted");
    }

    // --- a-pull-request-link-wears-its-state.test.tsx ---

    // case: wears the state in words, and the checks as one mark
    #[test]
    fn pull_link_chip_wears_state_in_words_and_hover() {
        let l = crate::look::controls::Look::new(true);
        let running = LinkedPullRequest {
            state: PullRequestState::Open,
            checks: PullLinkChecks::Running,
            passed: 3,
            failed: 0,
            running: 1,
            total: 4,
        };
        let rt1 = render_tree(move |_| pull_link_chip(&running, &Palette::new(true)));
        let t1 = collect_text(&rt1);
        assert!(t1.contains("Open"));
        assert!(t1.contains("Open · 3 of 4 checks passed · 1 running"));
        let mark1 = rt1.elements().into_iter().find(|e| e.key.as_deref() == Some("pr-link-mark")).expect("running mark");
        assert_eq!(mark1.fill, l.c("amber"));

        let failing = LinkedPullRequest {
            state: PullRequestState::Open,
            checks: PullLinkChecks::Failing,
            passed: 1,
            failed: 1,
            running: 0,
            total: 2,
        };
        let rt2 = render_tree(move |_| pull_link_chip(&failing, &Palette::new(true)));
        assert!(collect_text(&rt2).contains("Open · 1 of 2 checks passed · 1 failing"));
        let mark2 = rt2.elements().into_iter().find(|e| e.key.as_deref() == Some("pr-link-mark")).expect("failing mark");
        assert_eq!(mark2.fill, l.c("red"));

        let merged = LinkedPullRequest {
            state: PullRequestState::Merged,
            checks: PullLinkChecks::None,
            passed: 2,
            failed: 0,
            running: 0,
            total: 2,
        };
        let rt3 = render_tree(move |_| pull_link_chip(&merged, &Palette::new(true)));
        let t3 = collect_text(&rt3);
        assert!(t3.contains("Merged"));
        assert!(!t3.contains("checks"));
        assert!(rt3.find("pr-link-mark").is_none(), "no checks mark once merged");
    }

    // --- pull-request-news-is-said.test.tsx ---

    #[test]
    fn pull_request_news_banner_says_what_changed_and_offers_fix_only_when_needed() {
        let news = PullRequestNews {
            number: 12,
            url: "https://github.com/acme/shop/pull/12".into(),
            said: "1 of 3 checks did not pass".into(),
            needs_work: true,
        };
        let n1 = news.clone();
        let rt1 = render_tree(move |_| news_banner_view(&n1, Some("Wren"), &Palette::new(true)));
        let t1 = collect_text(&rt1);
        assert!(t1.contains("Pull request #12: 1 of 3 checks did not pass."));
        assert!(t1.contains("Ask Wren to fix it"));
        assert!(t1.contains("Open on GitHub"));
        assert_eq!(
            pull_request_ask(&news),
            "Pull request #12 (https://github.com/acme/shop/pull/12): 1 of 3 checks did not pass. Look at what is wrong, fix it on this branch, and push the fix."
        );

        let approved = PullRequestNews {
            said: "It was approved".into(),
            needs_work: false,
            ..news
        };
        let rt2 = render_tree(move |_| news_banner_view(&approved, Some("Wren"), &Palette::new(true)));
        let t2 = collect_text(&rt2);
        assert!(!t2.contains("to fix it"));
        assert!(t2.contains("Open on GitHub"));

        let rt3 = render_tree(|_| watch_toggle(false, &Palette::new(true)));
        assert!(collect_text(&rt3).contains("Watch"));
        let rt4 = render_tree(|_| watch_toggle(true, &Palette::new(true)));
        assert!(collect_text(&rt4).contains("Watching"));
    }
    // --- an-issue-link-wears-its-state.test.tsx ---

    const ISSUE_URL: &str = "https://github.com/acme/app/issues/3";

    fn issue(state: IssueLinkState) -> LinkedIssue {
        LinkedIssue { state, title: "The cart forgets".into() }
    }

    fn address() -> PullLinkAddress {
        PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 3 }
    }

    // case: wears open, closed and not planned, its standing and title on hover
    #[test]
    fn the_chip_beside_an_issue_link_wears_open_closed_and_not_planned_its_standing_and_title_on_hover() {
        let l = crate::look::controls::Look::new(true);
        // Electron's words and colors: open green, completed violet, not planned grey; never the red of an abandoned
        // pull request.
        for (state, word, hover, tone) in [
            (IssueLinkState::Open, "Open", "Open · The cart forgets", "green"),
            (IssueLinkState::ClosedCompleted, "Closed", "Closed as completed · The cart forgets", "violet"),
            (IssueLinkState::ClosedNotPlanned, "Not planned", "Closed as not planned · The cart forgets", "text-muted"),
        ] {
            let link = issue(state);
            let rt = render_tree(move |_| issue_link_chip(&link, &Palette::new(true)));
            let all = rt.elements();
            let chip = all.iter().find(|e| e.key.as_deref() == Some("issue-link-chip")).expect("the chip");
            assert_eq!(chip.help.as_deref(), Some(hover), "the hover says its standing, then the title");
            assert_eq!(chip.label.as_deref(), Some(hover), "and a screen reader hears the same");
            let said = all.iter().find(|e| e.key.as_deref() == Some("issue-link-state")).expect("the state word");
            assert_eq!(said.text.as_deref(), Some(word));
            let dot = all.iter().find(|e| e.key.as_deref() == Some("issue-link-dot")).expect("the state's dot");
            assert_eq!(dot.fill, l.c(tone), "{word}");
            assert_ne!(dot.fill, l.c("red"), "{word}: a finished issue never wears a failure's red");
            // No mark for checks it has not got: the pull request chip's mark is not drawn.
            assert!(all.iter().all(|e| e.key.as_deref() != Some("pr-link-mark")), "{word}");
        }
    }

    // case: makes a bare issue in the person's message a link, as a pull request is
    #[test]
    fn a_bare_issue_in_the_persons_message_is_a_link_as_a_pull_request_is() {
        let words = |body: &'static str| {
            let rt = render_tree(move |_| linked_words(body, type_::BODY, &Palette::new(true)));
            let all = rt.elements();
            let links: Vec<(Option<String>, Option<String>)> =
                all.iter().filter(|e| e.role == "link").map(|e| (e.text.clone(), e.label.clone())).collect();
            let plain: Vec<String> = all.iter().filter(|e| e.role != "link").filter_map(|e| e.text.clone()).collect();
            (links, plain)
        };
        let (links, plain) = words("See https://github.com/acme/app/issues/3 today");
        assert_eq!(links, [(Some(ISSUE_URL.to_owned()), Some(ISSUE_URL.to_owned()))]);
        assert_eq!(plain, ["See ", " today"]);
        let (links, _) = words("See [the issue](https://github.com/acme/app/issues/3).");
        assert_eq!(links, [(Some("the issue".to_owned()), Some(ISSUE_URL.to_owned()))]);
        // No answer has arrived yet, so no chip is worn.
        assert!(link_chips_for_text(&State::default(), "See https://github.com/acme/app/issues/3 today", &Palette::new(true)).is_none());
    }

    /// A `gh` that answers `issue view` with `answer` (None: no gh on the machine), counting the asks; no other call
    /// is answered.
    struct IssueGh {
        asked: std::sync::atomic::AtomicUsize,
        answer: Option<&'static str>,
    }
    impl locust_core::folder_commit::Network for IssueGh {
        fn run(&self, program: &str, args: &[String], _cwd: &std::path::Path) -> Result<String, String> {
            if program == "gh" && args.first().map(String::as_str) == Some("issue") && args.get(1).map(String::as_str) == Some("view") {
                self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return self.answer.map(str::to_owned).ok_or_else(|| "spawn gh ENOENT".to_owned());
            }
            Err("unexpected call".into())
        }
    }
    struct NoGh;
    impl locust_core::folder_commit::Network for NoGh {
        fn run(&self, _: &str, _: &[String], _: &std::path::Path) -> Result<String, String> {
            Err("no gh".into())
        }
    }

    /// A conversation whose person wrote the issue link, with `gh` standing in.
    fn conversation_with_the_link(gh: &Arc<IssueGh>) -> State {
        let mut s = State::default();
        crate::scenes::reply_code_scene(&mut s);
        s.pr.commits = Arc::new(locust_core::folder_commit::FolderCommits::new(Box::new(NoGh)));
        // The reader keeps no answer of its own, so every ask the window makes reaches `gh` and is counted.
        s.pr.link_reader = Arc::new(PullLinkReader::with_options(gh.clone(), Arc::new(|| 0), 0, 2));
        if let Some(pane) = s.ledger.as_mut() {
            pane.items.push(ledger::Item::User(format!("See {ISSUE_URL} today")));
        }
        s
    }

    fn pump_until(s: &mut State, done: impl Fn(&State) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            poll(s, &mut Tx::default());
            if done(s) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    // case: reads on show, again on focus after a minute, and never on a timer
    #[test]
    fn an_issue_link_is_read_on_show_again_on_focus_after_a_minute_and_never_on_a_timer() {
        use std::sync::atomic::Ordering;
        let gh = Arc::new(IssueGh { asked: Default::default(), answer: Some(r#"{"state":"OPEN","stateReason":"","title":"The cart forgets"}"#) });
        let mut s = conversation_with_the_link(&gh);
        // Not on screen yet: nothing is read, and nothing is worn.
        assert_eq!(s.pr.issue_watch.current(&address()), None);
        assert_eq!(gh.asked.load(Ordering::SeqCst), 0);
        // Shown: read, once, and the answer is worn.
        assert!(pump_until(&mut s, |s| s.pr.issue_watch.current(&address()).is_some()), "the issue was read");
        assert_eq!(s.pr.issue_watch.current(&address()), Some(&issue(IssueLinkState::Open)));
        assert_eq!(gh.asked.load(Ordering::SeqCst), 1);
        // The window leaves and comes back inside the minute: no new read.
        s.window_focused = false;
        poll(&mut s, &mut Tx::default());
        s.window_focused = true;
        for _ in 0..30 {
            poll(&mut s, &mut Tx::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(gh.asked.load(Ordering::SeqCst), 1, "inside the minute it is not read again");
        // Never on a timer, however long it sits: nothing is armed for an issue, and a tick reads nothing.
        assert!(!s.pr.issue_watch.timer_armed());
        assert!(s.pr.issue_watch.on_tick(|_| true).is_empty());
        // In the conversation: the person's link is a link, and the answer is worn beside it.
        let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.frame(0.05);
        let all = rt.elements();
        let link = all.iter().find(|e| e.key.as_deref() == Some("user-link")).expect("the link");
        assert_eq!(link.label.as_deref(), Some(ISSUE_URL));
        let chip = all.iter().find(|e| e.key.as_deref() == Some("issue-link-chip")).expect("the chip is worn once read");
        assert_eq!(chip.label.as_deref(), Some("Open · The cart forgets"));
        // The minute itself, on the watch's own clock (Electron's test moves its clock the same way).
        let mut watch = LinkWatch::<LinkedIssue>::default();
        assert_eq!(watch.show(&[address()], 10_000, |_| false), [address()]);
        watch.finish_read(&address(), Some(issue(IssueLinkState::Open)), 10_000, |_| false);
        assert!(watch.on_focus(40_000).is_empty(), "back after 30 s: no new read");
        assert_eq!(watch.on_focus(71_000), [address()], "back after a minute: read again");
        assert!(!watch.timer_armed());
    }

    // case: wears open, closed and not planned, its standing and title on hover (the colors, shell.css `.lc-prlink[data-issue-state]`)
    #[test]
    fn the_state_word_and_the_chip_take_each_states_ink_and_tint() {
        let l = crate::look::controls::Look::new(true);
        // shell.css: open green-text on green-tint; completed violet-text on violet-tint; not planned text-muted on chip-bg.
        for (state, ink, tint) in [
            (IssueLinkState::Open, "green-text", "green-tint"),
            (IssueLinkState::ClosedCompleted, "violet-text", "violet-tint"),
            (IssueLinkState::ClosedNotPlanned, "text-muted", "chip-bg"),
        ] {
            let link = issue(state);
            let rt = render_tree(move |_| issue_link_chip(&link, &Palette::new(true)));
            let all = rt.elements();
            let chip = all.iter().find(|e| e.key.as_deref() == Some("issue-link-chip")).expect("the chip");
            assert_eq!(chip.fill, l.c(tint), "{ink}: the chip's tint");
            let said = all.iter().find(|e| e.key.as_deref() == Some("issue-link-state")).expect("the state word");
            assert_eq!(said.text_color, Some(l.c(ink)), "{tint}: the word's ink");
        }
    }

    // case: says nothing and wears nothing when gh cannot answer
    #[test]
    fn an_issue_link_says_nothing_and_wears_nothing_when_gh_cannot_answer() {
        use std::sync::atomic::Ordering;
        let gh = Arc::new(IssueGh { asked: Default::default(), answer: None });
        let mut s = conversation_with_the_link(&gh);
        assert!(pump_until(&mut s, |s| gh.asked.load(Ordering::SeqCst) == 1 && s.pr.issue_pending.is_empty()), "gh was asked");
        assert_eq!(s.pr.issue_watch.current(&address()), None);
        assert!(link_chips_for_text(&s, &format!("See {ISSUE_URL} today"), &Palette::new(true)).is_none(), "no chip is worn");
    }
}
