# Electron main-process tests carried into Rust

Lane: `~/work/locust-wt/maxlane`, branch `exec/claude-tests`, cut from native-engine at 6cd55444. Each batch takes about
five `apps/desktop/src/main/*.test.ts` files. For each case whose behaviour Rust has, there is one Rust test with
Electron's values and words, and a break check where the named test must fail. Rows are in `parity/tests.tsv`.

Break checks ran through `~/work/scratch/maxlane/mutall.py`. It is a copy of `~/work/scratch/mut6.py` that prints
every failing test name, not just the first three. That way "caught by the named test" can be read off the output.

## Batch 1: handoff, mode, handover note, route at start, remembered effort

| File | Row | Rust tests |
| --- | --- | --- |
| `handoff.test.ts` | carried (8/8) | `core/src/handoff_brief_tests.rs` |
| `a-handoff-keeps-its-mode.test.ts` | partial (1/3) | `core/src/mission_mode_tests.rs` |
| `a-handoff-without-a-note-says-so.test.ts` | carried (4/4) | `core/src/handover_note_tests.rs` |
| `route-at-start.test.ts` | partial (16/17) | `core/src/route_at_start_tests.rs` |
| `route-remembers-effort.test.ts` | waiting | none |

Bugs found and fixed:
- **The brief had no `kept`.** Electron's `composeHandoffPrompt` returns the sections it carried, in reading order and
  without the person's next words. The preview under the box reads that list. `carry_on::Briefing` now has `kept`.
- **The saved route dropped the request's account.** When a start matched the teammate's saved route, Rust took the
  saved route's account even when that was none. Electron sets the saved account only when there is one, so the
  request's own account stays. Fixed in `route_at_start`.

Found and not fixed (outside these files):
- `compose_handoff_prompt_with` leaves out the sections the person chose to drop, but it does not add Electron's
  `choiceNotice` line, "(The person chose to leave out ...)". Electron also takes that line's length out of the
  budget. This belongs to `a-person-can-leave-out-part-of-a-brief.test.ts`.

Why some rows are not carried:
- Mode: Electron's `parsedMissionMode` reads a missing or malformed mode as Ask. Rust has no lenient reader. Its
  request paths take a typed mode, and the door refuses one it does not know. The third case checks Electron's
  `index.ts` source text, so it has no Rust counterpart.
- Route at start: "returns no permission mode" holds by type, because `StartRoute` has no mode field. No test was
  written, so the row is partial.
- Remembered effort: Rust never records the route a start used. There is no `rememberRoute`, and a teammate's route
  changes only through the people panel. The teammate-store reader (`native::route_of`) also leaves effort out of the
  `Route` it builds. The window reads effort straight from the JSON instead (`native.rs` job.effort).

Break checks (all caught by the named test): `kept` including next; a blank next counted as an instruction; next
not charged against the budget (refuses); newest-that-fit taking the oldest; the clip notice's words; stopped partway
with work in flight; the clean-finish wording; the request's account; the override for another teammate; the saved
effort; `modelChoice` ignored; the conversation's own route; the pick key; Kilo's Auto Free; an explicit pick
ignored; a share to anyone counted as a note; any share counted as a note to the owner; the no-note row's words; the
board brief's words; `plan` read as Ask.
