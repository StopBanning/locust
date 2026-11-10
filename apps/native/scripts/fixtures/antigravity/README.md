# Antigravity captures

Real state left behind by Google's Antigravity IDE, measured 2026-09-03 on
Windows. They are the only source the `antigravity-events` and
`antigravity-projects` adapters were built from.

Antigravity has no streaming CLI. Its `language_server.exe` exposes an
`agentapi` verb pair — `new-conversation --model=<flash_lite|flash|pro>
<prompt>` prints JSON with a `conversationId`, and `send-message
<conversationId> <content>` continues it — and everything the agent then does
is observable only in the two files captured here.

| File | What it captures |
| --- | --- |
| `transcript-plain-reply.jsonl` | A whole conversation that only talked: `USER_INPUT`, `CHECKPOINT`, then a `PLANNER_RESPONSE` whose content is `OK`. |
| `transcript-write-then-followup.jsonl` | A conversation that wrote a file and was then continued with `send-message`: `USER_INPUT`, `CHECKPOINT`, a `PLANNER_RESPONSE` carrying `thinking` and a `write_to_file` call, a `GENERIC` line holding that call's result text, a `PLANNER_RESPONSE` of `DONE`, the follow-up (`SYSTEM_MESSAGE`), and a second `DONE`. |
| `summaries.pb` | `~/.gemini/antigravity/agyhub_summaries_proto.pb`, the IDE's own record of past conversations. It is the only place the `ANTIGRAVITY_PROJECT_ID` that `new-conversation` demands could be found. 17 conversation records, all naming project `daf0f8ec-bb8e-49e8-a445-954eb0a62d0f`. |

The transcripts live at
`~/.gemini/antigravity/brain/<conversationId>/.system_generated/logs/transcript.jsonl`
on the machine they came from — one JSON object per line, appended as each step
completes.

## What was changed

The two `.jsonl` files were scrubbed. Nothing else about them was touched:
every key, every value and the line order are as captured.

- The workspace the agent had open, `c:/Users/<home>/Documents/antigravtest`,
  reads `c:/work/pebble` — the same placeholder the Cursor and OpenCode
  fixtures use. Two occurrences, both in `transcript-write-then-followup.jsonl`
  (the `write_to_file` call's `TargetFile`, and the `file:///` URI in the tool
  result).
- The operator's home directory, `C:\Users\<home>`, reads `C:\Users\dev`. One
  occurrence per file, inside the `CHECKPOINT` summary, where Antigravity
  prints the path of the full transcript it is summarizing.

`summaries.pb` is **byte-for-byte as captured and was NOT scrubbed.** Its
workspace paths are length-prefixed strings nested three messages deep, so any
replacement that changed their length would have to have every enclosing
length prefix rewritten too — and the file has no published schema to rewrite
it against. The only thing it discloses is a local folder path,
`c:/Users/<home>/Documents/antigravtest`, in two encodings (plain, and a
percent-and-backslash form). It carries no prompt, no model output and no
credential.

The tests therefore assert the `summaries.pb` workspace under its real path,
not under `c:/work/pebble`.

## What these captures do NOT contain

Two cases the adapter handles are not in either file, and the tests that cover
them build the line by hand rather than pretending it was measured:

- A `write_to_file` call WITHOUT `Overwrite: true`. The one captured write set
  it, so the added-file diff branch rests on the flag's name, not on an
  observation.
- A line whose `status` is anything but `DONE`. Every line captured was
  finished by the time it was read.
