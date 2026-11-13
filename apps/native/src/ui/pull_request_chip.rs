//! THE FOLDER'S PULL REQUEST, IN THE HEADER, ON LINKS, AND WATCHED FOR NEWS.
//!
//! Port of Electron's `PullRequestChip.tsx`, `PullRequestNewsBanner.tsx`, and `PullLinkChipView`.

use super::*;
use locust_core::{
    folder_commit::{FolderCommits, RealNetwork},
    pull_requests::{
        self, ConversationPiece, FolderPullRequest, IssueLinkState, LinkWatch, LinkedIssue,
        LinkedPullRequest, PULL_REQUEST_WATCH_MS, PullLinkAddress, PullLinkChecks, PullLinkReader,
        PullRequestChecks, PullRequestNews, PullRequestState, conversation_pieces, is_pull_link_moving,
        issue_link_hover, issue_link_word, now_ms, pull_link_hover, pull_link_word, pull_request_ask,
        pull_request_line, pull_request_news,
    },
};
use rune::Icon;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
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
    pub watch_job: Latest<(PathBuf, Option<FolderPullRequest>)>,
    pub folder: Option<PathBuf>,
    pub current: Option<(PathBuf, Option<FolderPullRequest>)>,
    pub was_running: bool,
    pub was_focused: bool,
    pub watched_urls: Vec<String>,
    pub watched_loaded: bool,
    pub last_watched: HashMap<String, FolderPullRequest>,
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
    pub last_link_tick_at: u64,
    pub record_opened_urls: bool,
    pub opened_urls: Vec<String>,
}

impl Default for PrState {
    fn default() -> Self {
        let (link_tx, link_rx) = channel();
        let (issue_tx, issue_rx) = channel();
        Self {
            commits: Arc::new(FolderCommits::default()),
            link_reader: Arc::new(PullLinkReader::new(Arc::new(RealNetwork))),
            read_job: Latest::default(),
            watch_job: Latest::default(),
            folder: None,
            current: None,
            was_running: false,
            was_focused: true,
            watched_urls: Vec::new(),
            watched_loaded: false,
            last_watched: HashMap::new(),
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
            last_link_tick_at: now_ms(),
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

fn start_watch_look(s: &mut State, folder: PathBuf) {
    let commits = s.pr.commits.clone();
    s.pr.watch_job.request(folder, move |folder| {
        let pr = commits.pull_request(&folder);
        (folder, pr)
    });
}

/// Triggers an immediate watch look for the active folder (test seam for headless drives).
#[cfg(test)]
pub(super) fn watch_look_now(s: &mut State) {
    if let Some(folder) = active_folder(s) {
        s.pr.commits.forget_sign_in();
        start_watch_look(s, folder);
    }
}

fn is_watched(s: &State, url: &str) -> bool {
    s.pr.watched_urls.iter().any(|u| u == url)
}

fn set_watched(s: &mut State, url: &str, on: bool) {
    s.pr.watched_urls.retain(|u| u != url);
    if on {
        s.pr.watched_urls.insert(0, url.to_owned());
        s.pr.watched_urls.truncate(MAX_WATCHED_URLS);
    } else {
        s.pr.last_watched.remove(url);
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
    if on {
        s.pr.last_watched.insert(pr.url.clone(), pr);
        let wait = Duration::from_millis(PULL_REQUEST_WATCH_MS);
        s.pr.next_watch_at = Some(Instant::now() + wait);
        tx.wake_after(wait);
    } else if s.pr.watched_urls.is_empty() {
        s.pr.next_watch_at = None;
    }
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

    if folder_changed {
        s.pr.folder = folder.clone();
        s.pr.read_job.cancel();
        s.pr.watch_job.cancel();
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
                if is_watched(s, &pr.url) {
                    s.pr.last_watched.entry(pr.url.clone()).or_insert_with(|| pr.clone());
                    if s.pr.next_watch_at.is_none() {
                        let wait = Duration::from_millis(PULL_REQUEST_WATCH_MS);
                        s.pr.next_watch_at = Some(Instant::now() + wait);
                        tx.wake_after(wait);
                    }
                }
            } else if is_watched(s, &pr.url) {
                set_watched(s, &pr.url, false);
            }
        }
        s.pr.current = Some((read_folder, found));
        changed = true;
    }

    if let Some(next_at) = s.pr.next_watch_at
        && let Some(folder_path) = &folder
        && let Some(pr) = current_pr(s)
        && is_watched(s, &pr.url)
        && matches!(pr.state, PullRequestState::Open | PullRequestState::Draft)
    {
        let now = Instant::now();
        if now >= next_at && !s.pr.watch_job.is_pending() {
            start_watch_look(s, folder_path.clone());
            let wait = Duration::from_millis(PULL_REQUEST_WATCH_MS);
            s.pr.next_watch_at = Some(now + wait);
            tx.wake_after(wait);
        } else {
            tx.wake_after(next_at.saturating_duration_since(now));
        }
    }

    if let Some((watch_folder, found)) = s.pr.watch_job.take()
        && folder.as_ref() == Some(&watch_folder)
    {
        if let Some(now_pr) = found {
            if is_watched(s, &now_pr.url) {
                if let Some(before) = s.pr.last_watched.get(&now_pr.url)
                    && let Some(news) = pull_request_news(before, &now_pr)
                {
                    s.pr.news = Some(news);
                }
                s.pr.last_watched.insert(now_pr.url.clone(), now_pr.clone());
                if matches!(now_pr.state, PullRequestState::Merged | PullRequestState::Closed) {
                    set_watched(s, &now_pr.url, false);
                    if s.pr.watched_urls.is_empty() {
                        s.pr.next_watch_at = None;
                    }
                }
            }
            s.pr.current = Some((watch_folder, Some(now_pr)));
            changed = true;
        }
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

    let (pulls, issues) = collect_conversation_links(s);
    for addr in pulls {
        let (_, should_read) = s.pr.link_watch.watch(&addr, now_epoch, is_pull_link_moving);
        if should_read {
            spawn_pull_link_read(s, addr);
        }
    }
    for addr in issues {
        let (_, should_read) = s.pr.issue_watch.watch(&addr, now_epoch, |_| false);
        if should_read {
            spawn_issue_link_read(s, addr);
        }
    }

    if focus_regained {
        for addr in s.pr.link_watch.on_focus(now_epoch) {
            spawn_pull_link_read(s, addr);
        }
        for addr in s.pr.issue_watch.on_focus(now_epoch) {
            spawn_issue_link_read(s, addr);
        }
    }

    if s.pr.link_watch.timer_armed() {
        let elapsed = now_epoch.saturating_sub(s.pr.last_link_tick_at);
        if elapsed >= pull_requests::PULL_LINK_AGAIN_MS {
            s.pr.last_link_tick_at = now_epoch;
            for addr in s.pr.link_watch.on_tick(is_pull_link_moving) {
                spawn_pull_link_read(s, addr);
            }
            tx.wake_after(Duration::from_millis(pull_requests::PULL_LINK_AGAIN_MS));
        } else {
            tx.wake_after(Duration::from_millis(
                pull_requests::PULL_LINK_AGAIN_MS - elapsed,
            ));
        }
    }

    changed
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
        .child(rune::icon(&GITHUB_MARK, 13.0).color(l.c("text-secondary")))
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
        chip = chip.child(el().size(6.0, 6.0).radius(3.0).fill(l.c(dot_tone)));
    }
    chip
}

/// Chip for a linked GitHub issue in a conversation reply.
pub(super) fn issue_link_chip(link: &LinkedIssue, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let hover = issue_link_hover(link);
    let word = issue_link_word(link.state);
    let tone = match link.state {
        IssueLinkState::Open => "green",
        IssueLinkState::ClosedCompleted => "blue",
        IssueLinkState::ClosedNotPlanned => "text-muted",
    };
    row()
        .py(2.0)
        .px(7.0)
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .fill(l.c("bg-control"))
        .border(1.0, l.c("border-card"))
        .role("status")
        .label(hover)
        .child(el().size(6.0, 6.0).radius(3.0).fill(l.c(tone)))
        .child(text(word).font(type_::LABEL).color(l.c("text-secondary")))
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

    #[test]
    fn pull_link_chip_wears_state_in_words_and_hover() {
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
}
