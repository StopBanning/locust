# Electron's answers, recorded and replayed (18 Nov 2026)

Idea taken from rust-anything (github.com/erkamyaman/rust-anything): record what the original returns on many inputs,
and make the port replay every case. No tool installed. A small recorder of our own does it.

- `scripts/electron-record/<area>.ts` calls Electron's own functions from this checkout's `apps/desktop` source.
- `scripts/electron-record/record.sh <area>` bundles it with esbuild from an Electron checkout's `node_modules`, runs it
  in Node, and writes `parity/recorded/<area>.jsonl` (one `{fn, args, out}` per line).
- A Rust test per area replays every line against the Rust function and lists every difference.

Inputs: every string and string list written in the functions' own Electron test files, plus traps:
- empty strings and case;
- emoji and other text JavaScript counts in UTF-16 units;
- unknown words and odd suffixes.

## Pilot: the picker (`picker.ts`, `core/src/recorded_picker_tests.rs`)
- Functions:
  - effortLevels.ts: `effortName`, `effortDescription`, `effortFooter`;
  - effortScale.ts: `splitEffort`, `effortRank`, `effortScale`, `levelsLine`, `joinEffort`;
  - routeName.ts: `freeTagOf`;
  - status.ts: `defaultEffort`.
- Result: 4,809 recorded cases, 0 differences, 0 unreadable (an unreadable line would be JavaScript cutting a
  character in half).
- Breaks, each caught by the replay:
  - `effort_name` made case-sensitive;
  - `free_tag_of` made case-sensitive;
  - `split_effort` taking `-fast` as a fast variant of nothing.

## Next
Areas with pure functions that hand-ported tests may have missed special cases in:
- route names (`routeName.ts`);
- the live line's words (`missionView.ts` `liveActionLine`);
- markdown prep (`agentText.ts`);
- shell words;
- failure words.

Record again whenever Electron changes one of them.
