# P13 — built-in browser (step 2)

P13 closes four Area 8 rows as **done** (tabs/URL/navigation/reload/viewport; teammate browser tools with permission gates; console/network logs; separate teammate sign-in profiles), one as **partial** (local page preview without CompareView/FileViewer comparison surfaces), and leaves point-at/screenshot attach **missing**. Area 5 connector/browser permissions moves to **partial** (browser gates live; connector cards remain). Compatibility choice: keep macOS 13 — macOS 14+ named persistent profile stores; macOS 13 isolated non-persistent stores with the panel words “On this macOS, teammate sign-ins last until Locust quits.” Recount: **32 done, 34 partial, 87 missing, 153 total**. M5 remains unreached. No interactive window and no real turns were run.

## What changed

- `src/browser.rs` — tab host, Electron-matching URL resolve (https/http/localhost/workspace file + symlink escape refusal), profile ownership, bounded console/network logs, org-policy refusal, temporary-sign-in constant.
- `src/browser_actions.rs` — Electron gate categories (credential/submit/purchase/send/download/leave-origin); unknown actions fail closed upstream.
- `src/browser_mac.rs` — objc2 WKWebView child of Rune’s AppKit view; per-profile `WKWebsiteDataStore` (persistent identifier on 14+, `nonPersistentDataStore` on 13); navigation/UI/scheme delegates; media/motion denied; forget removes only that store.
- `src/page_preview.rs` — `locust-page` capability roots; O_NOFOLLOW walk; hidden/key/secret refusal; 64 MiB bound; GET/HEAD-shaped responses for the scheme handler.
- `src/browser_tools.rs` — per-run private MCP Unix socket (`--browser-bridge`); tools for tabs/open/console/network/nav/read/click/type/select/press; asks via `approval::ask_browser` (no saved-rule bypass); profile-scoped tab access.
- `src/browser_capture.js` / `src/browser_dom.js` — page diagnostics injection and tool DOM probe/act scripts.
- `src/ui/browser_panel.rs` — Browser toggle/panel chrome, temporary-sign-in banner, profile menu / Forget sign-ins, address bar, viewport layout rect, headless scenes `p13-browser` / `p13-browser-light`.
- Wiring: `main.rs` modules/poll/attach/dispose/scenes; `native.rs` registers browser MCP on supported runtimes; `approval.rs` `ask_browser`; runtime ACP/Codex config hooks as edited in-tree.

Electron under `/Users/locust/work/locust/apps/desktop/src` was read only. Electron’s data folder was not written. Tests use temp folders only.

## TypeScript behavior carried over

| Electron test source (under desktop/src) | Rust coverage |
| --- | --- |
| `main/a-teammate-browses-and-asks-before-it-acts.test.ts` (gates/ask) | `browser_actions` + `ask_browser` + tool recheck after deny/close |
| `main/a-teammate-signs-in-apart-in-the-browser.test.ts` (profile isolation) | profile-scoped tabs; forget closes only that profile’s tabs |
| `main/a-page-runs-in-its-own-frame.test.ts` / `main/page-preview.ts` (confinement) | `page_preview` roots, never_served, size/mime/index |
| `main/browser-url.ts` rules | `browser::resolve` web/loopback/workspace/symlink cases |
| `main/browser-host.ts` log bound / tool scope | 100-entry logs; scoped readers; tool origin control |
| `main/a-part-of-the-browser-can-be-pointed-at.test.ts`, `main/a-part-of-a-page-can-be-pointed-at.test.ts` | **not ported** — point-at left missing this round |
| CompareView/FileViewer comparison preview UI | **not ported** — local preview only |
| Remote-browser / Desk / phone browser suites | out of Area 8 step 2; not claimed |

## Remaining — not silently skipped

1. Point-at element and screenshot/context attach (`browser-point` / `page-pick` / PagePointChips).
2. CompareView/FileViewer comparison page preview surfaces (local `locust-page` exists).
3. Connector permission-host cards (browser side of the shared permissions row is in).
4. Cursor/Muse/Antigravity/Kilo browser MCP registration (claude/codex/copilot/opencode/acp only).
5. Live window: WKWebView paint inside the Rune region, resize/clip/scale/z-order, focus/IME/keyboard handoff, real cookie isolation and restart persistence, device-permission refusal, console/network capture completeness, preview request confinement under a real page load.
6. `LOCUST_REAL_*` turns and interactive window exit were not run.

## Verification, 11 November 2026

From `/Users/locust/Documents/Locust/native-review/p13/native` with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p13
```

- `cargo test --release` ×3 after last source edit: **462 passed, 0 failed, 13 ignored** each. Ignored stay the live MCP/runtime one-liners. Logs: `/Users/locust/work/scratch/p13-checks/test-final-{1,2,3}.log`.
- `cargo build --release`: exit 0, **zero compiler warnings**. Log: `/Users/locust/work/scratch/p13-checks/build.log`.
- `cargo clippy --release`: exit 0; four existing `too_many_arguments` in prior modules only. No new findings in P13 browser modules. Log: `/Users/locust/work/scratch/p13-checks/clippy.log`.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` against the release binary: exit 0 (synthetic). Logs: `/Users/locust/work/scratch/p13-checks/m1-cli.log`, `m3a-cli.log`.
- Scenes looked at under `/Users/locust/work/scratch/p13-snaps/` at `--size 900x900 --scale 1`: `p13-browser`, `p13-browser-light`. Inspected: Browser panel with Codec’s sign-ins, temporary-sign-ins amber disclosure, Example Domain tab, Back/Forward/Reload, address `https://example.com/`, headless viewport chrome placeholder (no live WKWebView in scenes).

Not verified: live window/exit, real turns, real WKWebView paint/persistence/focus, point-at.

## Feasibility carry-forward

Step 1 source findings (Rune `WindowHandle::with_raw_handle`, layout geometry, macOS 14 `dataStoreForIdentifier`) still hold; step 2 implements the macOS 13-compatible branch rather than raising the deployment floor above `LSMinimumSystemVersion` 13.0.
