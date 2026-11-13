//! Settings › Connectors › "GitHub" (Electron `GitHubAccount.tsx`, 0.720 to 0.739): the GitHub CLI's own account,
//! signed in from the card with a one-time code, installed or updated with the computer's own package manager. The
//! words and every program call are `locust_core::github_account`; this file draws the card and runs its calls off
//! the window's thread.

use super::*;
use locust_core::github_account::{
    Account, CliVersions, GITHUB_CLI_PAGE, GithubAccount, InstallResult, RealGh, SignInCode, SignInResult, account_line, cli_is_behind,
};
use std::sync::{Arc, Mutex};

/// GitHub's mark, drawn as Electron's `GitHubMark` draws it.
pub(super) static GITHUB_MARK: rune::Icon = rune::Icon {
    name: "github-mark",
    view: 16.0,
    body: r#"<path fill="currentColor" d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/>"#,
};

#[derive(Default)]
pub(super) struct Card {
    pub account: Option<GithubAccount>,
    pub versions: Option<CliVersions>,
    pub signing_in: bool,
    pub code: Option<SignInCode>,
    pub copied: bool,
    pub said: Option<String>,
    pub installing: bool,
    pub updating: bool,
    /// Pages the card opened, kept instead of opened while `record_opened` is on (tests).
    pub record_opened: bool,
    pub opened: Vec<String>,
}

pub(super) enum Answer {
    Read(GithubAccount, CliVersions),
    SignedIn(SignInResult),
    Installed(InstallResult),
}

static ACCOUNT: Mutex<Option<Arc<Account>>> = Mutex::new(None);
/// The code a sign-in printed, from its job's thread to the card.
static CODE: Mutex<Option<SignInCode>> = Mutex::new(None);

fn account() -> Arc<Account> {
    ACCOUNT.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(|| Arc::new(Account::new(Arc::new(RealGh)))).clone()
}

/// Tests answer for gh, brew and winget themselves.
#[cfg(test)]
pub(super) fn use_gh(gh: Arc<dyn locust_core::github_account::Gh>) {
    *ACCOUNT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Account::new(gh)));
}

/// Ask the GitHub CLI who is signed in, and which version it is, off the window's thread.
pub(super) fn request(s: &mut State) {
    if s.github_job.is_pending() {
        return;
    }
    s.github_job.request((), |()| {
        let account = account();
        Answer::Read(account.read(), account.versions())
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if s.github.signing_in && s.github.code.is_none() {
        let code = CODE.lock().unwrap_or_else(|e| e.into_inner()).take();
        if code.is_some() {
            s.github.code = code;
            changed = true;
        }
    }
    if let Some(answer) = s.github_job.take() {
        changed = true;
        let card = &mut s.github;
        match answer {
            Answer::Read(account, versions) => {
                card.account = Some(account);
                card.versions = Some(versions);
            }
            Answer::SignedIn(result) => {
                card.signing_in = false;
                card.code = None;
                card.copied = false;
                match result {
                    SignInResult::Ok(account) => {
                        card.said = None;
                        card.account = Some(account);
                    }
                    SignInResult::Refused(message) => card.said = Some(message),
                }
            }
            Answer::Installed(result) => {
                card.installing = false;
                card.updating = false;
                match result {
                    InstallResult::Ok(account) => {
                        card.said = None;
                        card.account = Some(account);
                        card.versions = None;
                        request(s);
                    }
                    InstallResult::Refused { message, .. } => card.said = Some(message),
                }
            }
        }
    }
    changed
}

fn sign_in(s: &mut State) {
    *CODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    s.github.signing_in = true;
    s.github.code = None;
    s.github.copied = false;
    s.github.said = None;
    s.github_job.request((), |()| {
        Answer::SignedIn(account().sign_in(|code| *CODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(code)))
    });
}

fn install(s: &mut State, update: bool) {
    if update {
        s.github.updating = true;
    } else {
        s.github.installing = true;
    }
    s.github.said = None;
    s.github_job.request(update, |update| Answer::Installed(if update { account().update() } else { account().install() }));
}

/// Open one of GitHub's own pages in the person's browser: only github.com and cli.github.com.
fn open(s: &mut State, url: &str) {
    if !(url.starts_with("https://github.com/") || url == GITHUB_CLI_PAGE) {
        return;
    }
    if s.github.record_opened || cfg!(test) {
        s.github.opened.push(url.to_owned());
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn button(label: &str, key: &str, primary: bool, p: &Palette) -> El {
    let b = text(label.to_owned()).font(type_::LABEL).px(12.0).py(6.0).radius(6.0).key(key.to_owned()).role("button");
    if primary { b.color(p.bg).fill(p.text) } else { b.color(p.secondary).border(1.0, p.hairline) }
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let card = &s.github;
    let signed_in = matches!(card.account, Some(GithubAccount::SignedIn { .. }));
    let busy = card.signing_in || card.installing || card.updating;
    let name = match &card.account {
        Some(GithubAccount::SignedIn { login, .. }) => login.clone(),
        _ => "GitHub".to_owned(),
    };
    let line = if card.signing_in {
        "Waiting for GitHub to say yes.".to_owned()
    } else if card.installing {
        "Installing the GitHub CLI. Your computer may ask you to allow it.".to_owned()
    } else if card.updating {
        "Updating the GitHub CLI. Your computer may ask you to allow it.".to_owned()
    } else {
        card.account.as_ref().map(account_line).unwrap_or_else(|| "Asking the GitHub CLI.".to_owned())
    };
    let mut actions = row().gap(8.0).center_y();
    if !busy && card.account == Some(GithubAccount::NoCli) {
        actions = actions
            .child(button("Install the GitHub CLI", "github-install", true, p).on_press(|s: &mut State| install(s, false)))
            .child(button("Get it from GitHub", "github-get-it", false, p).on_press(|s: &mut State| open(s, GITHUB_CLI_PAGE)));
    }
    if !busy && matches!(card.account, Some(GithubAccount::SignedOut | GithubAccount::Expired { .. } | GithubAccount::Unknown { .. })) {
        actions = actions.child(
            row()
                .gap(6.0)
                .center_y()
                .px(12.0)
                .py(6.0)
                .radius(6.0)
                .fill(p.text)
                .child(crate::look::controls::glyph(&l, &GITHUB_MARK, 14.0, Some(p.bg)))
                .child(text("Sign in with GitHub").font(type_::LABEL).color(p.bg))
                .key("github-sign-in")
                .role("button")
                .label("Sign in with GitHub")
                .on_press(sign_in),
        );
    }
    if !busy && let Some(GithubAccount::SignedIn { login, .. }) = &card.account {
        let page = format!("https://github.com/{login}");
        actions = actions.child(button("Open on GitHub", "github-open", false, p).on_press(move |s: &mut State| open(s, &page)));
    }
    if !busy && card.account.is_some() {
        actions = actions.child(button("Check again", "github-check-again", false, p).on_press(|s: &mut State| {
            s.github.versions = None;
            request(s)
        }));
    }
    if card.signing_in {
        actions = actions.child(button("Stop", "github-stop", false, p).on_press(|_s: &mut State| account().cancel()));
    }
    let head = row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(crate::look::controls::glyph(&l, &GITHUB_MARK, 20.0, Some(if signed_in { p.text } else { p.muted })))
        .child(
            col()
                .gap(2.0)
                .grow(1.0)
                .shrink()
                .child(text(name).font(type_::LABEL).color(p.text))
                .child(text(line).font(type_::BODY).color(p.secondary).wrap()),
        )
        .child(actions);
    let mut body = col().w_full().gap(10.0).padding(12.0).radius(8.0).border(1.0, p.hairline).key("github-card").child(head);
    let behind = card.account.as_ref().is_some_and(|a| *a != GithubAccount::NoCli) && cli_is_behind(card.versions.as_ref());
    if behind && !busy && let Some(CliVersions { installed: Some(installed), latest: Some(latest) }) = &card.versions {
        body = body.child(
            row()
                .gap(10.0)
                .center_y()
                .child(text(format!("GitHub CLI {latest} is out. This computer has {installed}.")).font(type_::BODY).color(p.secondary).wrap())
                .child(button("Update the GitHub CLI", "github-update", false, p).on_press(|s: &mut State| install(s, true))),
        );
    }
    if card.signing_in {
        body = body.child(match &card.code {
            None => row().child(text("Asking GitHub for a code.").font(type_::BODY).color(p.secondary)),
            Some(code) => {
                let (shown, url) = (code.code.clone(), code.url.clone());
                row()
                    .gap(10.0)
                    .center_y()
                    .child(text("Enter this code on GitHub\u{2019}s page, then come back. It lasts fifteen minutes.").font(type_::BODY).color(p.secondary).wrap().shrink())
                    .child(text(code.code.clone()).font(rune::TextStyle::new(16.0, 600.0).mono()).color(p.text).key("github-code"))
                    .child(
                        button(if card.copied { "Copied. Open GitHub again" } else { "Copy code and open GitHub" }, "github-copy-code", true, p).on_press_tx(move |s: &mut State, tx| {
                            crate::reply_actions::request_copy(tx, shown.clone());
                            s.github.copied = true;
                            open(s, &url);
                        }),
                    )
            }
        });
    }
    if let Some(said) = &card.said {
        body = body.child(text(said.clone()).font(type_::BODY).color(p.amber).wrap().key("github-said"));
    }
    body
}

#[cfg(test)]
mod tests {
    //! your-github-shows-on-connectors.test.tsx: the card in each state, drawn headless.
    use super::*;

    fn shown(card: Card) -> String {
        let mut s = State::default();
        s.github = card;
        let mut rt = rune::Runtime::new(s, move |s: &State, _cx: &mut rune::Cx<State>| section(s, &Palette::new(true)), (900.0, 400.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
    }

    fn signed_in() -> GithubAccount {
        GithubAccount::SignedIn { login: "octo-cat".into(), scopes: vec![] }
    }

    #[test]
    fn wears_githubs_mark_and_names_who_it_is_signed_in_as() {
        let text = shown(Card { account: Some(signed_in()), ..Card::default() });
        assert!(text.contains("octo-cat") && text.contains("Signed in to GitHub.") && text.contains("Open on GitHub"), "{text}");
        assert!(!text.contains("Sign in with GitHub"), "{text}");
        assert_eq!(GITHUB_MARK.view, 16.0);
    }

    #[test]
    fn offers_a_sign_in_when_nobody_is_signed_in_and_the_cli_when_it_is_missing() {
        assert!(shown(Card { account: Some(GithubAccount::SignedOut), ..Card::default() }).contains("Sign in with GitHub"));
        assert!(shown(Card { account: Some(GithubAccount::Expired { login: Some("octo-cat".into()) }), ..Card::default() }).contains("Sign in with GitHub"));
        let missing = shown(Card { account: Some(GithubAccount::NoCli), ..Card::default() });
        assert!(missing.contains("Install the GitHub CLI") && missing.contains("Get it from GitHub") && !missing.contains("Sign in with GitHub"), "{missing}");
        let installing = shown(Card { account: Some(GithubAccount::NoCli), installing: true, ..Card::default() });
        assert!(installing.contains("Installing the GitHub CLI.") && !installing.contains("Check again"), "{installing}");
        assert!(!installing.split(" | ").any(|t| t == "Install the GitHub CLI"), "{installing}");
        assert!(shown(Card::default()).contains("Asking the GitHub CLI."));
    }

    #[test]
    fn says_when_github_has_a_newer_github_cli_with_the_update_a_press_away() {
        let v = |i: &str, l: Option<&str>| Some(CliVersions { installed: Some(i.into()), latest: l.map(str::to_owned) });
        let behind = shown(Card { account: Some(signed_in()), versions: v("2.96.0", Some("2.102.0")), ..Card::default() });
        assert!(behind.contains("GitHub CLI 2.102.0 is out. This computer has 2.96.0.") && behind.contains("Update the GitHub CLI"), "{behind}");
        assert!(!shown(Card { account: Some(signed_in()), versions: v("2.102.0", Some("2.102.0")), ..Card::default() }).contains("Update the GitHub CLI"));
        assert!(!shown(Card { account: Some(signed_in()), versions: v("2.96.0", None), ..Card::default() }).contains("is out"));
        let updating = shown(Card { account: Some(signed_in()), versions: v("2.96.0", Some("2.102.0")), updating: true, ..Card::default() });
        assert!(updating.contains("Updating the GitHub CLI.") && !updating.split(" | ").any(|t| t == "Update the GitHub CLI"), "{updating}");
    }

    #[test]
    fn shows_the_one_time_code_while_the_sign_in_waits_with_a_way_to_stop() {
        let asking = shown(Card { account: Some(GithubAccount::SignedOut), signing_in: true, ..Card::default() });
        assert!(asking.contains("Asking GitHub for a code.") && asking.contains("Stop") && !asking.contains("Sign in with GitHub"), "{asking}");
        let code = Some(SignInCode { code: "AB12-CD34".into(), url: "https://github.com/login/device".into() });
        let waiting = shown(Card { account: Some(GithubAccount::SignedOut), signing_in: true, code: code.clone(), ..Card::default() });
        assert!(waiting.contains("AB12-CD34") && waiting.contains("Copy code and open GitHub"), "{waiting}");
        assert!(shown(Card { account: Some(GithubAccount::SignedOut), signing_in: true, copied: true, code, ..Card::default() }).contains("Copied. Open GitHub again"));
    }

    #[test]
    fn is_found_by_the_words_a_person_types_for_it() {
        let connectors = crate::settings_pages::SETTINGS_PAGES.iter().find(|p| p.id == crate::settings_pages::SettingsPageId::Connectors).unwrap();
        assert_eq!(connectors.headings[0], "GitHub");
        for word in ["github", "gh", "pull request", "source control"] {
            assert!(crate::settings_pages::matched_headings(connectors, word).contains(&"GitHub"), "{word}");
        }
    }

    #[test]
    fn opens_only_githubs_own_pages() {
        let mut s = State::default();
        open(&mut s, "https://github.com/octo-cat");
        open(&mut s, GITHUB_CLI_PAGE);
        open(&mut s, "https://evil.example/login/device");
        assert_eq!(s.github.opened, vec!["https://github.com/octo-cat".to_owned(), GITHUB_CLI_PAGE.to_owned()]);
    }
}
