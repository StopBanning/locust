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

## Seen while gating (18 Nov)
`permission_host_tests` fail only under heavy load (load 8, release builds running): today
`carries_the_persons_reason_for_a_denial_when_they_gave_one` (release) and
`refuses_a_questions_answer_aimed_at_a_permission_and_says_so` (60 s timeout). Each passes alone in under 0.1 s. Worth a
look: something in those tests waits on wall time.

## Routes (18 Nov)

`routes.ts` records `routeModelName(runtime, id)` for every id in the eight route-name test files, under ten runtimes, plus
traps: 1,160 cases. The first replay found 83 differences, four causes: a bare Claude alias (`opus`) is named by the
version it means today ("Opus 5.5"); an id with a provider splits at its first slash only ("Kilo/Kilo Auto/free");
`own-xxxxxxxx/name` is the name alone; `auto` is trimmed before the check. All four fixed in `route_model_name`; 0 of
1,160 differ. Five breaks of the fix were all caught.
The first gates then caught `route_name_tests::invents_nothing_every_word_comes_from_the_id`: that case is Electron's
`modelDisplayName('claude', 'sonnet')` ("Sonnet"), with no alias read, which Rust had folded into `route_model_name`.
`model_display_name` is now its own function; the chip's `route_model_name` and the hover both end in it, so the
hover gets the first-slash, own-model and trimmed-auto fixes too. (`mut6.py` does not run a clean baseline first: a
test that already fails shows as "catching" every break. Look at the recorded test's line, not the others.)
