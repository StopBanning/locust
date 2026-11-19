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

## Command and path words (18 Nov)

`words.ts` records missionView.ts's `shellCommandText`, `withoutShellWrapper`, `relativePath` and `mcpToolParts`: 6,942
cases from their seven test files plus traps. 0 differ on the first replay: these were ported exactly. Breaks: the
case-insensitive folder match and the first-dot MCP split are caught by the replay; the doubled-quote undo by a unit
test; `without_shell_wrapper`'s "nothing after the wrapper" guard cannot be reached (the wrapper pattern needs a space
after it, and the command is trimmed first), so its break survives as an equivalent change. `record.sh` now points
`@teammate/runtime-adapters` at its source, so files that import it can be recorded.

## Cost words (18 Nov)

`cost.ts` records cost.ts's `costLine` and `moneyLine` over a grid of 513 costs (each field present or not; cents,
thousands and millions at their edges): 1,026 cases. The first replay found 216 differences, three causes, all in
Rust's `cost_line`: a zero cost said "$0" (Electron "$0.00"); under a cent it said "$0.0010" (Electron "< $0.01");
and an exact half cent rounded to even ("$0.12" for 0.125) where JavaScript's `toFixed` rounds up ("$0.13").
`benchmark::to_fixed` now rounds as `toFixed` does, from the number's exact digits, so 1.005 (really 1.00499...)
still reads "1.00" in both. 0 of 1,026 differ. The connector words (`connectorMentioned`) wait: `connectorLabel`
lives in a React component, which the recorder cannot load yet.

## Failure sentences (18 Nov)

`failure.ts` records what Electron says when a run fails: missionView.ts `failureMessage` and `sameSentence`, and
runtime-adapters `providerErrorSentence`, for the strings in their seven test files, as messages and as stderr, under
nine runtimes: 11,197 cases. 0 differ on the first replay: these were ported exactly. Three breaks (the two-way
same-sentence match, the last-word wrapper, the out-of-capacity wrapper) were all caught. `record.sh` now records from
`packages/runtime-adapters` sources too. The grid is trimmed to keep the file near 2 MB.
