# Pull requests in the header, on links, and watched for news (12 Nov 2026)

When a conversation's branch has a pull request on GitHub, the thread header shows its number, state, and check status, offers a Watch toggle for open/draft PRs, shows a news banner above the composer when a watched PR changes, and displays state chips for GitHub pull request and issue links in messages.

## What was built

- **Core pull request, link, and watch engine** (`core/src/pull_requests.rs`, `core/src/folder_commit.rs`):
  - `pull_request_of`, `pull_requests_of`, `pull_request_line`, `pull_request_news`, and `pull_request_ask` parse `gh pr view --json` / `gh pr list --json` output and format header lines, news sentences, and fix prompts.
  - `FolderCommits::pull_request` and `FolderCommits::forget_sign_in` reuse `FolderCommits`'s `Network` trait, `gh_signed_in`, `remote_of`, and `safe_branch` to read the branch's PR (skipping the default branch and caching per-folder/per-branch for `PULL_REQUEST_CACHE_MS = 60_000`, invalidated on `gh pr create` or `forget_sign_in`).
  - `github_link_of`, `pull_link_request`, `issue_link_request`, `linked_pull_request_of`, `linked_issue_of`, `pull_link_hover`, `issue_link_hover`, and `conversation_pieces` parse and validate GitHub PR/issue URLs and split message text into pieces while leaving `` `code` `` spans untouched.
  - `PullLinkReader` deduplicates concurrent in-flight `gh` reads for the same `(kind, owner, repo, number)`, bounds concurrent `gh` processes to `PULL_LINK_AT_ONCE = 2`, and caches both hits and errors for `PULL_LINK_CACHE_MS = 60_000`.
  - `PullRequestWatch` (`PULL_REQUEST_WATCH_MS = 3 * 60_000 = 180_000`) and `LinkWatch` (`PULL_LINK_AGAIN_MS = 180_000`) manage periodic watch polling, focus-based refreshes after 1 minute, and automatic unwatching on `Merged` or `Closed`.
- **UI chip, watch toggle, news banner, and link chips** (`src/ui/pull_request_chip.rs`, `src/ui/app_state.rs`, `src/ui/thread_view.rs`, `src/ui/messages.rs`, `src/main.rs`):
  - `pull_request_chip::control` draws the GitHub mark, `#<number>`, status dot (`green`/`red`/`amber`/`blue`/`text-muted`), and state word in the thread header, plus the `Watch`/`Watching` toggle for open or draft PRs (persisted up to 50 URLs in `watched_pull_requests.json`).
  - `pull_request_chip::news_banner` renders above the composer when a watched PR changes, with `Ask <teammate> to fix it` (populating `s.draft` without auto-sending), `Open on GitHub`, and `Dismiss`.
  - `pull_request_chip::link_chips_for_text` renders state chips for GitHub PR and issue links in user and assistant messages once loaded by `PullLinkReader`.
- **Headless window test** (`src/ui/window_drive_tests.rs`):
  - `a_branch_pull_request_shows_its_chip_watches_and_shows_the_news_banner` verifies the header chip, watch toggle, news banner, `Ask Codec to fix it` draft population, `Open on GitHub`, and `Dismiss` using an injected fake `Network` (no real `gh` or network calls).

## Judgement calls

- **Reusing `FolderCommits::Network` instead of a second `gh` runner**: `PullLinkReader` takes `Arc<dyn Network>` and `FolderCommits::pull_request` lives directly on `FolderCommits`, so all `gh` calls share the same `RealNetwork` implementation and test seam.
- **Link chips rendered as a strip below the message**: `rune_kit::Markdown` does not expose an inline chip hook for arbitrary URLs, so `pull_request_chip::link_chips_for_text` renders loaded PR/issue link chips in a compact wrapping row directly beneath the message bubble/markdown body while `poll` drives background `PullLinkReader` fetches.
- **No regex lookahead in `conversation_pieces` / `valid_repo`**: Rust's `regex` crate does not support `(?!...)` lookaheads, so `valid_repo` checks `repo != "." && repo != ".."` explicitly and `conversation_pieces` checks the trailing byte after a bare GitHub URL match.
- **Persisting watched URLs in `Store`**: Watched PR URLs are saved in `watched_pull_requests.json` (capped at 50) via `Store` instead of browser `localStorage`.

## Which TypeScript tests came over

- `apps/desktop/src/main/a-pull-request-shows-in-the-header.test.ts` -> `core/src/pull_requests_tests.rs`
- `apps/desktop/src/main/a-pull-request-can-be-watched.test.ts` -> `core/src/pull_requests_tests.rs`
- `apps/desktop/src/main/a-pull-request-link-wears-its-state.test.ts` -> `core/src/pull_requests_tests.rs`
- `apps/desktop/src/renderer/src/a-pull-link-is-read-once.test.ts` -> `core/src/pull_requests_tests.rs`
- `apps/desktop/src/renderer/src/a-pull-request-chip-says-its-state.test.tsx` -> `src/ui/pull_request_chip.rs`
- `apps/desktop/src/renderer/src/a-pull-request-link-wears-its-state.test.tsx` -> `src/ui/pull_request_chip.rs`
- `apps/desktop/src/renderer/src/pull-request-news-is-said.test.tsx` -> `src/ui/pull_request_chip.rs`

## What was left out

- Inline link chips *inside* `rune_kit::Markdown` paragraphs (rendered as a strip beneath the message instead).
- OS desktop notifications when a watched pull request changes while the window is unfocused (only the in-window news banner is shown).
