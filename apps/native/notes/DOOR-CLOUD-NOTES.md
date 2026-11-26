# Door cloud 1 (25 Nov): the Rust door starts, lists, follows up and reads Claude cloud chats

Lane `door-cloud-1`, branch `exec/native-door-cloud-1`, from f788674a. Colin wants the Locust chat to start Claude cloud
lanes itself through the Rust app. The window already had them (`src/ui/claude_cloud_panel.rs`, engine
`core/src/claude_cloud.rs`); the door had no way to ask. Electron's door has none either, so this is new and has no
Electron test: it is recorded here, not in `parity/tests.tsv`.

## READ THIS FIRST: on a Mac the door starts a cloud chat but cannot read or follow it up

`ClaudeCloud::start` starts a session *unseen* (in a hidden terminal, which draws the session's id and link) **only on
Windows** (`start_unseen` returns `None` unless the platform is Win32). On macOS it opens Claude Code's own window in
the person's terminal app and records the session with **no session id and no link**. `ClaudeCloud::send` and `check`
both refuse a session with no id. So a chat the door starts on this Mac is real and listed (with the generic
`https://claude.ai/code` link), but `send_message` and `read_reply` on it can only answer "Locust does not know this
session's id... See it on claude.ai", and the person has to answer Claude Code's trust question in the window that opened
if it asks one. Reading works on a Mac (Cloud 4 row 1) once a session's id is known; it is the *start* that lacks it.

Not fixed here: `start_unseen` is in `claude_cloud.rs`, which this lane may only add a small public function to, and a
hidden Mac start is not small (it changes the window's own start too, and `claude --cloud`'s output on a Mac was never
measured). **Follow-up (its own lane, owns claude_cloud.rs):** let `start_unseen` run on Darwin through
`run_in_pseudo_terminal` with argv (as `check_once` already does), env scrubbed of `CLAUDE*` markers, read with
`read_cloud_start`; measure it once with a real session first. Until then the lane gives Colin start + list + link on a
Mac, and the whole loop on Windows.

## What changed

- `start_conversation` takes `cloud: true` (`door_tools.rs` `start_cloud`). Claude teammate only (any other runtime, or no
  teammate: refused in words, "Nothing was started"). No `mode` or `read_only` with it (a cloud chat has no Locust mode).
  Model and effort go through the same `choice_of` check as a turn. Folder: a named one needs "Use each teammate's own
  mode" and `check_folder`; else the teammate's folder; else the folder Locust has open. A folder with no GitHub remote
  is refused (the window only warns: a task would start with no project). The task is at most 4,000 characters
  (`cloud_task_text` would silently cut it, so the door refuses instead). The reply gives `conversation_id` (the `cc_…`
  session id), `folder`, `repo`, `branch`, `url`, `session_known`, and `warnings` for unpushed commits, uncommitted
  changes, or an unknown push state (Claude's cloud starts from the newest pushed commit of the branch).
- The folder's saved organization environment goes with the start (`door_cloud::start_with`): `ClaudeCloud::start`
  with no environment would erase the one saved for that folder.
- `door-cloud.json` beside `claude-cloud.json` (`door_cloud.rs`): the sessions an app started, by id **and start time**
  (the engine counts ids from 1 in every run, so an id alone could name another session after a restart), with how many
  exchanges the last read saw and how many follow-ups were sent since. An app reads, follows up and is told how to stop
  only chats it started, unless "Let apps see your conversations" is on (the rule turns already follow). A voice call
  never reaches a cloud chat.
- `list_conversations` lists cloud chats too (the window's own as "the person"), newest first, with `cloud: true`,
  `url`, `started_by`, `teammate`, `folder`.
- `send_message` on a `cc_` id: `ClaudeCloud::send` (the session itself; at most 4,000 characters; no
  mode/folder/model/effort/read_only). Claude Code's own words pass through, with the chat's page; an unconfirmed send
  says to look before sending again and is never retried.
- `read_reply` on a `cc_` id: one `ClaudeCloud::check` per call (a fresh teleport into a worktree of its own beside the
  folder; the person's folder is untouched; it can take up to two minutes; `wait_seconds` is ignored). "Still working"
  until the session has a reply: the newest exchange has an answer *and* the exchanges are as many as were started and
  sent, so a read straight after a follow-up never returns the answer before it. An answer lists the files the session
  changed (path, +added -removed) and says the person brings them in with Apply in the cloud panel.
- `stop_turn` on a `cc_` id: refused plainly, with the chat's link. Checked: `ClaudeCloud` has no stop (only start,
  send, check, apply, forget, home), so this is the honest answer. `carry_on` and `answer_question` on a `cc_` id are
  refused in words.
- The tool schema and descriptions of `start_conversation`, `send_message`, `read_reply` and `stop_turn` say what
  `cloud` does (newest pushed commit, folder must be a pushed GitHub checkout, Claude teammate, no mode, 4,000
  characters, up to two minutes per read).

Files: `core/src/door_cloud.rs` (new), `door_tools.rs`, `door.rs`, `lib.rs` (the mod line), and one small public function
in `claude_cloud.rs`: `ClaudeCloud::all()` (every session from every folder; `list` is one folder's).

## Seams (no test runs the real `claude`)

`door_cloud::Seams { claude, github }`: what the live door runs Claude's cloud and git on. `door_cloud::seams()` is
`Seams::live()` unless a test installed its own (`#[cfg(test)]`); the engine asks it on every call. (An `Engine` field
would have broken `Engine { .. }` literals in test files this lane does not own.) Tests build `ClaudeCloud::new` with a
stub `discover`, `with_terminal`, `with_exec`, `with_run_window`; the read tests play the teleport with real git in a
temp repository (as `claude_cloud_read_tests.rs` does) and never run Claude Code.

## Call sites (outside test files)

| New public function | Called from |
| --- | --- |
| `door_cloud::start_with` | `door.rs:990` (`Engine::cloud_start`), reached from `door_tools.rs:1184` (`start_cloud`), from `door_tools.rs:864` (`call`, `start_conversation` with `cloud: true`) |
| `door_cloud::where_with` | `door.rs:986` (`Engine::cloud_where`), from `door_tools.rs:1176` |
| `door_cloud::chats_with` | `door.rs:994` (`Engine::cloud_chats`), from `door_tools.rs:960` (`list_conversations`) via `cloud_rows` |
| `door_cloud::chat_with` | `door.rs:998` (`Engine::cloud_chat`), from `door_tools.rs:1033` (`cloud_chat_of`: send, read, stop) |
| `door_cloud::send_with` | `door.rs:1002` (`Engine::cloud_send`), from `door_tools.rs:1085` (`cloud_send_or_read`), from `door_tools.rs:1667` (`send_or_read_claimed`, `send_message`) |
| `door_cloud::read_with` | `door.rs:1006` (`Engine::cloud_read`), from `door_tools.rs:1093`, same route (`read_reply`) |
| `door_cloud::seams`, `marks_path`, `link_of` | `door.rs` Engine methods; `door_tools.rs` rows and replies |
| `ClaudeCloud::all` (claude_cloud.rs) | `door_cloud::chats_with` |

`Engine` is the live `Locust` of `Door::for_store` / `serve_door_in` (the door the Locust chat reaches).

## Tests

`core/src/door_cloud_tests.rs` (13, the real engine with Claude Code and git stubbed):
a task goes to Claude's cloud and is recorded as an app's; a window-started session is listed as the person's; an id the
engine counted again after a restart is not taken for the door's session; the folder's saved environment goes with the
task and is not erased; on a Mac Claude Code's own window opens and the session has no id; a refusal from Claude Code is
passed on and nothing recorded; a start that cannot be recorded says so; the live door starts and lists through its own
Claude Code; a follow-up goes to the session and is counted; a read gives the answer and the changed files in a worktree
of its own (real git; the folder is untouched); a read straight after a follow-up is still working until the reply is
there; a session with no id cannot be read and says so; the live door follows up, reads and stops.

`core/src/door_cloud_tools_tests.rs` (22, the tools over a made-up Locust): start in the teammate's folder; model and
effort; Claude teammate only; no mode/read_only and the 4,000 limit; `cloud: false` is an ordinary conversation; named
folder rules; no GitHub remote refused; unpushed/dirty/unknown warnings; Claude Code's refusal word for word;
`list_conversations` rows; a voice call cannot start one; schema and description; send goes through the engine; an
unconfirmed send is passed on and not retried; a follow-up that does not fit is refused; an app reaches only its own
unless apps may see all (and a voice call never); read is still working (one look, `wait_seconds` ignored); read gives
the answer, files and page; a failed read says why and where; stop says it cannot and gives the link; carry_on and
answer_question refuse a cloud id; descriptions say it all.

## Break runs

Each entry is `path|||old ==> new`, run with `python3 -u ~/work/scratch/mut6.py "$PWD" <entries> locust-core` from
`apps/native`; every break compiles; every break was KILLED (a test named in the `.out` failed), none survived.

| Row | Break list | Output | Caught |
| --- | --- | --- | --- |
| 1 start + list | `~/work/scratch/evidence/door-cloud-1/row1-start.entries` | `row1-start.out` | 19 of 19 |
| 2 follow-up, read, stop | `~/work/scratch/evidence/door-cloud-1/row2-follow-read-stop.entries` | `row2-follow-read-stop.out` | 19 of 19 |

The call-site breaks: `door_tools.rs` `Value::Bool(true) => return self.start_cloud(args)` bypassed, the `cloud_start`
call replaced by an error value, `return self.cloud_send_or_read(..)` and `return self.stop_cloud(..)` bypassed, the
`cloud_send` and `cloud_read` calls replaced; and in `door.rs` each `Engine` cloud method's call into `door_cloud`
replaced (caught by the live-door tests). Also on the engine: `claude.start`, `claude.send`, `claude.check` and
`claude.all` replaced inside `door_cloud.rs`.

## Not done / not measured

- The Mac start gap above. Nothing here was run against the real `claude`: the start text it draws (`Created cloud
  session...`, the claude.ai link) is the engine's existing reading, measured on Windows.
- A read is a teleport and can take up to two minutes; an MCP client with a shorter timeout will give up first (the read
  keeps going; the next read joins or restarts it). A cheaper "is it done" signal would need Claude Code to offer one.
- The engine's session ids (`cc_…0001`) restart at 1 each run. The door guards its own records with the start time;
  the window's list can still show two sessions with one id after a restart. A fix belongs in `claude_cloud.rs`.
- The 40-exchange bound of a reading: a session past 40 exchanges can never count past it, so the door treats 40 as
  "arrived".
