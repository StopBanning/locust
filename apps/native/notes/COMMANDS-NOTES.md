# Runtime commands, sent as typed, and Compact now (13 Nov)

Started by a Sonnet subagent (core: b7d724d0, 7df7c597), stopped at Colin's word (no subagents; delegate through
Locust). I reviewed the core and wrote the window side, parity and these notes.

## What was built

- `core/src/runtime_commands.rs`: each runtime's own slash commands, kept once in `runtime-commands.json` outside every
  mission's record (Electron's file read until native writes its own): Claude Code's learned from its run, OpenCode's
  from its server, Codex's three (review, compact, init) fixed. Claude Code's that would rewrite the person's own setup
  are hidden.
- `native::send_from`: a message the person typed that starts with one of the conversation's runtime's commands goes to
  the runtime as the command (bare to Claude Code; by name with only what follows to OpenCode; Codex review and compact
  as requests of their own, init as a turn with Locust's words). Never for words an app, routine, relay, room or
  comparison started, never on a route switch or a side question.
- `core/src/compact_now.rs`: the offer (`compactOffer`) and its words.
- `src/ui/runtime_commands_ui.rs`: the lists read in a job at start and after each turn; the `/` menu rows under the
  runtime's name; choosing one writes `/name ` into the box, and the menu stays shut while its arguments are written;
  the context ring becomes a button where Compact now can be offered and opens its menu; Compact now sends `/compact`
  and keeps the box, or queues it behind a running turn.

## Special cases, broken against the whole suite

| Case | Line | Test that fails |
|---|---|---|
| Only the person's own message is sent bare | `origin.started_by.is_none()` in `send_from` | `a_command_is_briefed_like_any_message_when_the_words_came_from_a_routine_whatever_the_words_say` (and two more) |
| Claude Code's setup commands hidden | `visible()` | `the_commands_a_runtime_listed_are_offered_without_the_ones_that_would_change_the_persons_own_setup` (and three more) |
| Another model picked: refused with why | `next_runtime != runtime` | `compact_now_says_why_it_cannot_with_a_message_already_waiting_or_another_model_picked` |
| The box is kept | restore in `compact` | `compact_now_from_the_ring_sends_compact_and_keeps_the_box` |

## Remaining

- The ring while a turn is still starting (no route of its own yet), and the reading reset after a compaction.
- Compact now sends through `send`, which also carries any pending diff notes and terminal output with the message.
