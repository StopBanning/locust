# Desk 3: Rust Window Events vs Electron `DESK_EVENTS`

In Electron (`apps/desktop/src/main/index.ts:627-634`), `mirrorToDesk` wraps `window.webContents.send` so every message the main process pushes to its window also calls `deskHost?.mirror(channel, args)`. `DeskHost::mirror` forwards only when a browser holds the window (`DeskHost::active`) and `is_desk_event(channel)` (`core/src/desk.rs:436-450`, matching `shared/desk-calls.ts:354-359`) allows the channel.

In the native Rust window (`apps/native/src/main.rs`), the window does not receive push IPC messages from a separate main process; instead `main.rs::poll(s, now, tx)` polls the shared engine (`core/src/engine_api.rs`) and background jobs once per tick. Mirroring therefore observes state transitions at one place per poll tick (`phone_section::mirror_desk`) and calls `DeskHost::mirror` + `DeskHost::flush` for each matched channel, only while a browser holds the window.

## Channel-by-Channel Map (`DESK_EVENTS`)

| Electron channel | Electron payload (`args`) | Rust window counterpart | Status / notes |
| --- | --- | --- | --- |
| `appearance:changed` | `[AppearanceState]` (`{ theme, resolved, look, liquidOffered, glassGround }`, `index.ts:1355`) | `s.appearance` (`Option<bool>`), formatted by `phone_section::appearance_now(s)` | Matched. |
| `app:update-state` | `[AppUpdateState]` (`{ phase, currentVersion, everyBuild, availableVersion?, downloadUrl?, percent?, message?, heldBack? }`, `index.ts:9535`) | `s.settings_panel.update` (`locust_core::updates::AppUpdateState`), polled in `update_banner::poll(s)` (`main.rs:2333`) | Matched. |
| `away:since-changed` | `[{ since: number \| null }]` (`index.ts:11078`) | `s.home.cover_sleeper.away_since` (`Option<i64>`), updated in `home_panel::poll_cover(s, tx)` (`main.rs:2279`) | Matched. |
| `codex-mission:update` | `[CodexMissionUpdate]` (`index.ts:2683, 2699, 3147, 3203, 10223, 10518`, `shared/ipc.ts:3311`) | Polled from `engine_api::api().running()` (`s.seen_live`, `main.rs:2348-2357`), `engine_api::api().running_checks()`, `engine_api::api().relay_live_state(&watched_ids)` (`s.relay_live`, `main.rs:2379-2384`), and `ledger_tick` (`s.ledger`, `main.rs:2731`) | Matched in part: run start/completion events (`kind: "event"` with `run.started` / `run.completed`), `edit-check-started`, `relay-notice`, and `relay-starting` / `relay-start-settled` have Rust counterparts; streaming `message-deltas`, cloud-computer steps, and compare-reruns have no live push stream in `EngineApi` yet. |
| `mission-approval:request` | `[MissionApprovalRequest]` (`{ approvalId, runId, missionId, runtime, kind, summary, detail, cwd, requestedAt, ... }`, `index.ts:2577`) | Newly appearing cards in `engine_api::api().waiting()` (`Vec<approval::Request>`), polled in `sync_approvals(s)` (`main.rs:2345`) and formatted by `phone_section::waiting_card` | Matched. |
| `mission-approval:withdrawn` | `[approvalId]` (`string`, `index.ts:2473, 10408, 10554, 10685`) | Cards previously in `engine_api::api().waiting()` that are no longer waiting | Matched. |
| `folder:pull-request-news` | `[PullRequestNews]` (`{ number, url, said, needsWork }`, `index.ts:3570`) | `s.pr.news` (`Option<pull_requests::PullRequestNews>`), updated in `pull_request_chip::poll(s, tx)` (`main.rs:2324`) | Matched. |
| `runtime-discovery:event` | `[RuntimeDiscoveryEvent]` (`index.ts:3228`) | `s.settings_panel.agents` updated in `agents_section::poll(s)` (`main.rs:2291`) | No per-event Rust counterpart yet: `agents_section` replaces a snapshot vector rather than emitting `RuntimeDiscoveryEvent` frames. |
| `runtime:install-progress` | `[{ runtime, line }]` (`index.ts:3667`) | `s.agents_install.shown` (`Option<String>`) in `agents_section::poll(s)` (`main.rs:2291`) | Partial: `shown` holds only the latest line (no runtime id stored alongside `shown` when polled without tracking the active install). |
| `runtime-updates:changed` | `[RuntimeUpdatesState]` (`index.ts:3405`) | `s.agent_updates.state` (`agent_updates::State`), polled in `agents_section::poll_updates(s)` (`main.rs:2291`) | Matched. |
| `scout:away-changed` | `[ScoutAwayStatus]` (`index.ts:8548`) | None | No Rust counterpart yet: `core/src/scout_away.rs` is not instantiated by the native window (`phone_section.rs:497-500`). |
| `scout:changed` | `[ScoutState]` (`index.ts:8708`) | `s.scout.state` (`Option<scout::types::ScoutState>`), polled in `scout_cards::poll(s)` (`main.rs:2311`), serialized with `ScoutState::to_json()` | Matched. |
| `voice:progress` | `[percent]` (`number`, `index.ts:4748`) | None | No Rust counterpart yet: `voice_panel::tick` has no local Whisper model download progress stream. |

## Non-`DESK_EVENTS` (Window-Only)

Channels in `WINDOW_ONLY_EVENTS` (`core/src/desk.rs:453-470`) — such as `locust:github-sign-in-code`, `phone:anywhere-changed`, `terminal:event`, `voice-call:event`, `telegram:changed`, `discord:changed`, `slack:changed`, `account:changed`, `remote-mcp:changed` — are rejected by `is_desk_event` inside `DeskHost::mirror` and are never sent to a browser.
