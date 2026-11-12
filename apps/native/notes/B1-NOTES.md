# B1, first part: the door, served by Rust (11–12 Nov 2026)

Most of Colin's conversations start through Locust's door: another app (this chat, Codex, a chat app) hands work to a
teammate through the local MCP server Electron opens when "Let your other AI apps use Locust" is on. The Rust app had
only the client side of that door. It now serves its own.

## What was built

- **The server** (`core/src/door.rs`, from `locust-mcp-host.ts`). Off until turned on. On, it listens on 127.0.0.1 at
  a port the system picks and writes `locust-mcp-connection.json` (the URL and a new 64-hex bearer token, owner-only,
  written through a fresh temporary file) into the native data folder, so it and Electron can both run. Each request's
  headers are checked before any body is read: a missing or stale token is 401, an `Origin` header (a browser), another
  method or another path is 403, a body over 256 KB or with no length is 400. The token is checked again after the
  body arrives, so turning the door off also refuses a request still arriving. The connection file goes at load (a
  crashed process's token is never reused) and when the door closes. The switches (on, each teammate's own mode, let
  apps see your conversations) are saved in `locust-mcp.json` as Electron saves them; a switch that cannot be saved
  is not changed.
- **The tools** (`core/src/door_tools.rs`, from `locust-mcp-tools.ts`), with Electron's rules and words:
  `list_teammates`, `start_conversation`, `send_message`, `read_reply` (with `wait_seconds` up to 20), `stop_turn`,
  `list_conversations`. A turn runs in Ask unless the person turned on each teammate's own mode; then in the
  teammate's saved mode, or a mode and folder the app names. `read_only` always steps down to Ask. A follow-up keeps
  its conversation's mode, folder, runtime and model. A named folder must be one Locust knows, inside one, or a
  worktree of one of their repositories (`check_app_folder`, from `app-folder.ts`). The reply says the mode that ran
  and, when the host held a turn in another, the mode asked for. An answer carries Locust's warnings that came after
  it, and Locust's own tags come off it (`unwrap_protocol_tags`). Status for apps that do not parse text:
  working, waiting_for_person, done, failed or stopped. Only an app's own conversations are read or stopped unless the
  person lets apps see theirs. Two sends to one conversation cannot both continue the same turn.
- **The bridge** another app runs: `locust-native --door-bridge <connection file>`, a stdio MCP server (from
  `locust-mcp-bridge.mjs`). It reads the connection file on every call, refuses one that points anywhere but this
  machine's door, never follows a redirect, lists the same eleven tools with the read-only ones marked, and answers
  in the bridge's words when Locust is not running or does not answer.
- **In the app.** A real window opens the door at start when the person left it on, and closes it (and removes the
  connection file) when the window closes. A snapshot, video or script run never opens it.
  `locust-native --serve-door` opens it with no window until stdin closes, for scripted checks; it does not change the
  saved switch.
- **The engine records what really ran.** Every native ledger used to say `mode: ask`, `sandbox: read-only`, even
  for a turn in Auto. `mission.created` now carries the turn's real mode and sandbox, its folder (`command.cwd`) and,
  for a turn from the door, `startedBy: {kind: mcp, folder}`. Each turn's teammate is recorded in `missionOwners`, as
  Electron's host does.
- **Auto only where Settings allow it.** The native engine now refuses Auto unless Settings turn it on
  (`AUTO_OFF_REFUSAL`, Electron's words), for the window as well as the door. The Auto live tests turn it on in their
  own settings.

## Not offered yet, said in Electron's words for a server without them

- A model or effort for one turn, and `list_models`: the native engine has no model list yet ("This server does not
  take a model or effort").
- Routines from another app ("This Locust does not offer routines to other apps").
- `carry_on`, and `can_carry_on` is always false: carry-on is B2.
- `list_background_runs` answers an empty list: the native engine runs no background Claude turns.
- No Settings switch in the window yet: the door is turned on by its settings file or `--serve-door`.
- No voice-call copy of the door and no editor (`cli/`) sessions.
- `list_conversations` lists the newest 20 conversations from both ledger folders, not only the open folder's, and
  states are the turn's phase, without "its plan ran out".
- The connection file is owner-only on macOS and Linux; Windows ACLs are not set (the native Windows build does not
  run turns yet).

## How it was checked

- 21 new tests: the tools against a made-up Locust (each rule and refusal above), the listener over real HTTP (401,
  403, 400, 200, the file's 0600 mode, a new token after a restart, the door closed means the bridge says Locust is
  not running), the connection file's checks, the folder rule, and the tool list's read-only marks.
- Live, through the real bridge, on a throwaway native data folder and Scout (OpenCode, the free DeepSeek route):
  an Auto turn in a named folder wrote `hello.txt`; a follow-up continued the conversation and read it back; a
  `sleep 90` turn was stopped from the door and read back as stopped; `list_conversations` listed them as started by
  an app; a folder Locust does not know was refused with nothing started. All three ledgers passed Electron's reader,
  and their metadata has the same keys as an app-started OpenCode ledger Electron wrote.
- Not checked: the same calls sent to Electron's door side by side (it would start conversations in Colin's
  Electron); a teammate whose turn raises a card (no window was open, so the drive stayed in Auto); Claude Code,
  Codex or Codex's app-server through the door.
- Tests: 441 in the engine (19 ignored live tests), 190 in the window, all passing.
