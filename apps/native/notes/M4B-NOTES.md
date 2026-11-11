# M4b: Composer, one to one with Electron

Native window composer in this copy. No window for real turns; no live workspace file scan beyond scene fixtures. Area 2 rows closed in `PARITY.md` are the ones this piece owns; permission-mode / Compare / connector-off rows stay missing.

## Changes by file and TypeScript ports

| File | Change and TypeScript reference |
| --- | --- |
| `src/composer/file_mentions.rs` | `fileMentions.ts`: `at_query`, `without_at_query`, `file_matches` (rank + accent fold). |
| `src/composer/slash.rs` | `slashCommands.ts`: `SLASH_COMMANDS` words, `slash_query`, `available_commands`, `matching_commands`. |
| `src/composer/send_blocked.rs` | `status.ts` `sendBlockedReason` + live title from `Composer.tsx` (`a-disabled-send-says-why`). |
| `src/composer/drafts.rs` | `Composer.tsx` per-conversation `Map`; `pending…` keep-on-id; **not** disk (Electron does not persist drafts). |
| `src/composer/keys.rs` | `Composer.tsx` `keyDown` order: slash → `@` → Shift+Tab mode → Escape stop → history → Enter / Shift+Enter. |
| `src/composer/queue.rs` | `steering.ts`: verdict notes, fold/take/without/requeue, strip note wording. |
| `src/composer/queue_store.rs` | `queued-message-store.ts` + `queued-messages.ts`: schemaVersion 1, atomic replace under native data only. |
| `src/composer/attachments.rs` | `shared/attachments.ts`: preamble, cap 8, pasted-byte cap, chip label. |
| `src/composer/placeholder.rs` | Electron placeholder chain for the direct/chat path. |
| `src/store.rs` | `queued_messages_path`, `attachments_dir` (native root only). |
| `src/main.rs` | Multiline `text_input` + `rune::auto_grow` (180px / 11 lines); drafts on teammate switch; `@` / slash popups; queue strip (Edit / Discard / Send now / Stop and send now); paste (`on_input_paste` files+PNG) and drop (`on_drop_files`); disabled-send reason; scenes `composer-*`. Stays out of `src/runtime/`. |

## Rune API confirmed

- **Multiline:** default for `text_input`; Enter submits, Shift+Enter inserts a newline (`rune` `el.rs`). Cap via `auto_grow` + `max_h(180)`.
- **Paste files/images:** `El::on_input_paste` + `ClipboardContent::files` / `image().png()`.
- **Drop files:** `El::on_drop_files` (+ `accepts_files` available, unused here).

## What this piece cannot do (Rune / scope)

- **PDF page tiles / image thumbnails** as Electron `PdfTile` / `AttachedImage`: attachments are path chips only; no PDF rasteriser in this pin.
- **Native tooltip title** on the Send control (Electron `title={sendBlockedReason…}`): Rune `El` in this rev has no tooltip API; the blocked reason is shown as amber status text (scene `composer-blocked`).
- **Live `workspaceFiles()` listing:** scene fixtures supply paths; a full host walk of the project folder is not wired (Electron `main/workspace-files.ts`).
- **Runtime CLI slash list / own commands / `!` bang mode / issue picker / voice:** not in this piece’s ask.
- **Drafts across restarts:** Electron keeps them in a window-local `Map` only (`PARITY.md` Removed); native matches that. The queue file **does** persist under the native data folder.

## Verification

```sh
PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4b cargo test --release

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4b cargo build --release

# Snaps → /Users/locust/work/scratch/m4b-snaps/
locust-native --snap …/composer.png --scene composer --size 900x900 --scale 2
# (+ composer-light, composer-slash[-light], composer-mention[-light],
#    composer-queue[-light], composer-blocked[-light])
```

- Release tests: **184 passed, 0 failed, 6 ignored** (five consecutive `--release` runs).
- Release build: passed.
- Snaps rewritten to `/Users/locust/work/scratch/m4b-snaps/` and inspected: multiline composer dark/light; slash menu with Electron command words; `@` files-only for `@re` with README.md detail “the folder itself”; queue strip with Electron action labels and “Sends when … finishes”; blocked send showing `Nothing can run yet — install an AI agent from the list above.`

## Unconfirmed

- Real window with a live turn: queue drain after completion, Ctrl/Cmd+Enter stop-and-send-now, paste of a real screenshot into the native attachments dir.
- Live workspace file index for `@` matching against a large folder.
- PDF preview tiles and image thumbnails.
- Custom/project slash commands and runtime-advertised commands beyond the built-in Locust list.

## Review and fixes

Parity pass against Electron (`Composer.tsx`, `fileMentions.ts`, `slashCommands.ts`, `steering.ts`, `status.ts`, `shared/attachments.ts`, `main/queued-message-store.ts`) and crash leftovers. Fixes landed in this copy only.

### `@` popup (`composer-mention`)

- Teammate filter matches Electron: `name.toLowerCase().startsWith(typed)` (not substring / not role). Typing `@re` therefore lists **files only** — `@Argon` / `@Scout` do not appear (their names do not start with `re`). Covered by `mention_scene_uses_real_matching_and_excludes_attached_and_tagged`.
- Grouping/order: teammates first (cap 6), then `file_matches` files (max 8). Headings `Teammates` / `Files in this folder`. Root paths (no `/`) use detail **the folder itself** — README.md in the scene shows that line. Snaps `composer-mention` / `composer-mention-light` confirm.
- Accent fold + rank order ported in `src/composer/file_mentions.rs` (NFD strip, no ligature transliteration); attached paths and already-tagged mates are excluded like Electron.

### Words, placeholders, keys, buttons

- Placeholder chain in `src/composer/placeholder.rs` mirrors Electron’s direct/chat path (including queued `Edit the queued message to change it…`).
- Queue strip labels: `Edit`, `Discard`, `Send now`, `Stop and send now`; note `Sends when {name} finishes` (`src/composer/mod.rs`, `queue.rs`).
- Key order in `src/composer/keys.rs`: slash → `@` → Shift+Tab mode → Escape stop → history → Enter / Shift+Enter. Escape clears a typed slash command the way Electron does (`slash_escape_clears_the_command_like_electron`).
- Blocked send reason shown as amber status text (Rune has no tooltip); scene `composer-blocked` matches Electron’s install wording.

### Queue

- Persist shape matches `queued-message-store.ts`: `{ schemaVersion: 1, rows }`, atomic replace under the native data root only (`queue_store.rs`).
- Edit / Discard are conversation-scoped and roll back when the durable write fails (`queue_edit_and_discard_are_conversation_scoped_and_transactional`).
- Stopped / restored rows wait for an explicit send-now; the row is removed from disk **before** dispatch so an uncertain transport result cannot send twice (`stopped_queue_waits_for_explicit_send_and_is_removed_before_dispatch`, comment at the drain site in `main.rs`).

### Crash / flake cleanup

- Half-written slash-dismiss state and mention-popup conditions cleaned so scenes match live `at_choices_now` / `slash_choices_now`.
- Flaky `native::tests::a_continuation_follows_the_newest_turn_not_the_anchor`: test already ties mtimes and asserts graph tip `child_m`; remaining intermittent failures were temp-dir collisions — fixtures named only by clock could delete each other’s `mission-ledger` under parallel tests. `Temp::new` / window `setup()` now use `pid + nanos + AtomicU64` (`src/native/tests.rs`, `src/main.rs`).

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4b cargo test --release
# ×5 → each: 184 passed; 0 failed; 6 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4b cargo build --release

# Snaps → /Users/locust/work/scratch/m4b-snaps/
# composer, composer-light, composer-slash[-light], composer-mention[-light],
# composer-queue[-light], composer-blocked[-light] at 900×900 @2x
```
