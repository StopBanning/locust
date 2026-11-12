# Window tests with no screen (12 Nov 2026)

The live sessions drove the real window on Colin's screen, because background typing does not reach Rune's text box.
That stopped. `src/ui/window_drive_tests.rs` now drives the same window code with no window at all. This brings Phase 2A
of `docs/PLAN-fable-2026-11-12.md` forward.

## How it works

- `rune::Runtime::new(state, view, (1100, 780), env)` builds the real view, with no window and no screen.
- **Clicks:** `click(key)` lets motion settle (a card that slides in is clicked where it rests), finds the element by
  key, and presses there.
- **Typing:** `type_and_send` clicks the message box, pastes the text and presses Return.
- **Polling:** `pump` runs the window's own `poll` inside `rt.update`, so its changes redraw as they do in the app,
  until the test's condition holds.
- **Agents and data:** the fakes in `scripts/fixtures` through `native::hooks`, over a temporary data folder seeded with
  a team file. No Electron, no real agent, no network.

## The seven checks

Each was a live-session check or a live-session bug.

1. A message typed into the box is sent, and its answer is shown.
2. A model of another agent, picked in the picker after a search, moves the reply there. The ledger records the route
   switch and the brief.
3. A conversation opened from the face's card is the teammate's (name, face, mode), not "Codex · Conversation".
4. Choosing a teammate leaves the Board for their conversation.
5. A turn started outside the window's own send (as the door starts one) shows on the Board with its card, and Allow
   once from the Board finishes it.
6. Stop in the window ends a long turn with "Stopped" and no error line.
7. Quitting writes each running turn's cancelled record.

More since (Phase 1, Thu):
8. A mode chosen on the message box is the mode the turn runs in; picking a teammate goes back to theirs.
9. A pressed notification opens its conversation, from wherever the window was.
10. "All of Codec's conversations" opens the Board narrowed to Codec, and "Show everyone's" widens it.
11. The Team screen edits a teammate in a form and asks before removing one.
12. The message box's + attaches a file picked in the system's dialog (the dialog answered by the test).
13. How memory is kept is chosen on the Memory screen.

Phase 2A's last two (12 Nov, evening):
14. The folder chip names the folder the turn runs in: the teammate's own, else the one Locust has open. The turn's
    ledger records that same folder. Run once with the fallback taken out, it failed as the live bug did: "No folder".
15. Check 2 also reads the launched command: a model picked in the picker reaches the agent's command line
    (`--model sonnet` in the argv the ledger keeps).

With these, all eight of the plan's Phase 2A checks are window tests.

Checks 3, 5 and 7 were each run once with their fix taken out, and failed as the bug did.

## Things learned on the way

- **State changes must go through `rt.update`.** Setting `rt.state` directly does not tell Rune to rebuild the view.
- **A card can't be approved in its first half second.** That's deliberate (a stray click must not allow anything). The
  test waits 0.6 s of real time before Allow.
- **The picker's rows past its fold** are reached the way a person reaches them, by searching.

## Fixed on the way

A conversation opened from the list was a stand-in. It now takes its owner from the owner maps both apps record
(`missionOwners`), on the agent and model the conversation is on now:
- its header names the teammate;
- its picker offers every agent;
- replies run in the teammate's own mode. Before, it was always Ask.

## Not covered here

These need the system's own windows:
- the folder dialog itself (an `NSOpenPanel`);
- a notification banner;
- the Quick Ask panel's global shortcut.

Each one's decision logic has its own tests. Seeing them on screen waits for a session Colin agrees to, or stays out of
the demo.
