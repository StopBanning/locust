# B2, first part: hand-over and carry-on on the native engine (12 Nov 2026)

A turn that stops when its plan runs out, its provider fails before it does anything, or its process dies can now be
carried on by another agent, and a reply on another agent is no longer refused. Both start the new agent cold, with
a brief of the work so far, as Electron does.

## What was built

- **Why a turn stopped** (`core/src/carry_on.rs`, from shared/carry-on.ts): `carry_on_reason` (plan, provider before
  any tool, or a crashed process or protocol; never a finished, stopped or refused turn), the instruction the carried
  turn gets, and the refusal for Approve each on an agent that cannot ask first.
- **The checkpoint** (from mission-store's checkpoint.ts): the stopped turn reconciled from its ledger alone. Actions
  that finished, named by what they ran; actions that started and never reported back; safety (unsafe when the
  ledger was cut short, approval-required with an action of unknown outcome); the transcript digest; the last
  answer. It is appended to the stopped turn's ledger when that ledger is the native engine's. Electron's ledgers are
  only read, so a conversation from Electron is reconciled without the record being written.
- **The brief** (from main/handoff.ts `composeHandoffPrompt`): the task, the turns before it (up to four, each clipped),
  the actions to check before redoing, the finished actions, the previous agent's last words, then the latest
  instruction. Within 8,000 characters: optional sections shrink to their newest items or drop whole, and the brief
  says what was left out; a long task gives up its end beside a reply, and a rescue with no reply is refused rather
  than clipped. Not ported: a long task by file, and the person's own leave-outs (the window has neither yet).
- **A reply on another agent** (`native::send_from`, the route switch in codex-mission.ts): reconciled, briefed,
  recorded with `continuesFrom: {reason: route-switch, checkpointEpoch, leftOut?}`, the brief as the turn's words.
  The native engine used to refuse this ("This conversation ran on another agent").
- **Carry-on** (`native::carry`, from main/carry-on.ts `carry`): the stopped turn's mode, an app's folder checked again,
  the teammate the conversation belongs to, `startedBy: {kind: carry-on, from}`, and the turn remembered so a
  second press while the first starts is refused. The carried turn takes the new agent's own effort, not the
  teammate's saved one.
- **The door's `carry_on`**, with Electron's rules: only a stopped turn, only an Ask turn unless the person turned on
  each teammate's own mode, and only to an agent this Locust can run. `list_models` with no teammate lists those
  agents (each whose program is found), with no models: there is no catalog yet, so a model named for a carry-on is
  passed to the agent unchecked. `read_reply` and `list_conversations` now say when a turn can be carried on and why
  it stopped.

## How it was checked

- **Against Electron's own code.** `scripts/handoff-goldens.mjs` bundles carry-on.ts, checkpoint.ts and handoff.ts and
  runs eight stopped turns through them (a plan that ran out mid-work, a provider failure, a crashed process, a
  refusal, a reply after a finished turn, after a stop, a long task clipped beside a reply, and 45 finished actions
  shrunk to fit), plus twelve refusal cases, into `parity/handoff.json`. Rust matches every reason, instruction,
  checkpoint and brief byte for byte (`carry_on_tests.rs`).
- **Live**, `real_carry_on_from_a_plan_that_ran_out`: a Codex turn that chose a code word, said it, and ran out of its
  plan before writing it down, carried on to OpenCode's free DeepSeek route in Auto. Twice in a row the carried turn
  wrote exactly `AMBER-42` into `answer.txt`, a word that existed only in the brief, and Electron's reader accepted
  both ledgers, the stopped one with its new checkpoint record. An earlier version of the test asked for a file that
  was not there; the model rightly refused, and the test passed anyway because the refusal quoted the word, so the
  check was made strict.
- The engine test that asserted the old refusal now asserts the switch: the brief, the link and the checkpoint.
- Tests: 445 in the engine (20 ignored live tests), 190 in the window, all passing.

## Second part, the same night: the window

- **The door's switches in Settings** (General, from LocustMcpSettings.tsx): "Let your other AI apps use Locust", and
  once it is on, "Use each teammate's own mode" and "Let apps see your conversations" as indented sub-switches, in
  Electron's words, and the lines that add it to Claude Code and Codex with a Copy each. Scenes `settings-door` and
  `settings-door-light`.
- **The stopped turn's card** (Thread.tsx's carry-on block): the reason in a sentence on a green rule, one press for
  the agent Locust suggests (the person's pick in Settings when it can run here, else the first ready agent in
  Electron's order, `carry_on_offer`), and "Choose another agent" for the rest. On the native engine it now carries
  on here; it used to say carrying on "runs in Locust for now". The transcript's card knows which agent stopped and
  why. Scenes `carry-on` and `carry-on-light`.
- **Without asking** (main/carry-on.ts `onRunEnded`): when the person picked an agent and turned on Without asking, a
  turn they sent from the chat box that stopped on its plan, provider or process is carried on at once. Never a turn
  an app or a carry-on started, so one empty plan cannot start a chain. When it cannot (the pick is the agent that
  stopped, or the carry-on is refused), the stopped turn's thread says why (`host.carry_on`, Electron's words),
  appended to its native ledger; Electron's reader accepts it.
- Tests: 446 in the engine, 190 in the window.

## Not done

- A Settings control to pick the carry-on agent and Without asking: the pick is read from `planRunsOut` (seeded from
  Electron's settings), but the Rust window cannot change it yet.
- Checking a model against a catalog before a carry-on.
- The window does not draw the switch's seam from the native engine's receipt yet.
