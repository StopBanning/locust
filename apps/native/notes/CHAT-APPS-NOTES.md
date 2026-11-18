# Chat apps in the Rust app (17 Nov 2026)

## What was built
1. **Shared rules (`core/src/chat_apps.rs`)**: `ChatAppId`, `ChatAppSettings`, `teammate_commands`, `addressed_teammate` (using `NSString::decomposedStringWithCompatibilityMapping` on macOS for NFKD accent folding), `pairing_code_in`, `spaced_code`. All 4 tests from `a-teammate-is-named-the-way-a-person-writes.test.ts` carried.
2. **Shared link state machine (`core/src/chat_link.rs`, `core/src/chat_link_tests.rs`)**: `create_chat_link`, `chat_answer`, `exact_line`, `migrated` (0.831 numeric ids), owner-only `0o600` persistence, encrypted tokens via `SecretBox`, pairing/wrong-code rotation, routing (`/cmd`, `Name:`, `@cmd`, `/new`, `/who`, `/waiting`, `/help`, replies, held messages), approval/question cards (`Allow once` / `Deny` / option buttons / free-text reply), `alerts`, and `refusal`.
3. **Answer formatters (`core/src/chat_{discord,slack,telegram}_fmt.rs`)**: All 18 tests from `an-answer-reads-right-in-{discord,slack,telegram}.test.ts` carried.
4. **macOS HTTP & WebSocket transport (`core/src/chat_http.rs`)**: `HttpClient` and `WsClient` traits, plus `MacUrlSessionClient` using Apple's `NSURLSession` and `NSURLSessionWebSocketTask` via `objc2-foundation` (enabling `NSError`, `NSURLResponse`, `NSURLSession`, `block2` features; `block2 v0.6.2` was already in `Cargo.lock`, no new crates downloaded).
5. **App transports (`core/src/chat_{telegram,discord,slack}.rs`)**:
   - Telegram: `getUpdates` long poll, `sendMessage` (HTML + plain fallback), `editMessageText`, `sendChatAction`, `setMyCommands`, `getMe`/`getWebhookInfo`.
   - Discord: Gateway WebSocket (heartbeats, identify, resume, `READY`, `RESUMED`, `MESSAGE_CREATE`, `INTERACTION_CREATE`), REST `/users/@me`, `/applications/@me`, `/gateway/bot`, `/channels/*/messages`, `/applications/*/commands`, 5x5 button layout.
   - Slack: Socket Mode (`apps.connections.open`, envelope acks, `hello`, `disconnect`, `events_api`, `interactive`), Web API (`auth.test`, `chat.postMessage`, `chat.update`, `reactions.add`/`remove`, `users.info`).
6. **Host & Settings UI (`core/src/chat_host.rs`, `src/ui/chat_apps_section.rs`, `src/ui/settings_panel.rs`)**: Wired into `native::on_run_ended`, `Settings > General > Chat apps`, and headless `--scene chat-apps`.

## What is left / needs Colin
- **Live check with Colin's own bots**: `MacUrlSessionClient` (`NSURLSession` / `NSURLSessionWebSocketTask`) has been compiled and type-checked on macOS, but has not been run against live Discord, Slack, or Telegram servers (per the no-network-in-tests rule).
- **Card raised/settled push hooks**: `chat_host::on_run_ended` is wired in `native.rs`, and `chat_host` calls `native::decide_approval` / `native::answer_approval`, while live `card_raised` / `card_settled` broadcast hooks from `approval.rs` and away-alert forwarding remain to be wired.
- **Telegram QR code widget**: Settings shows the pairing link and Copy/Open buttons; rendering a QR code matrix in Rune remains.
