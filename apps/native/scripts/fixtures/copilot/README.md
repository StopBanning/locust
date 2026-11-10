# Copilot CLI captures

Real `copilot -p ... --output-format json` streams from GitHub Copilot CLI
1.0.82, captured 2026-09-03 on Windows. They are the only source the
`copilot-events` adapter was built from.

They are **not** byte-for-byte copies. Four things were replaced, and nothing
else was touched:

- **Machine paths.** The captured workspace was a temporary directory under the
  operator's profile; it reads `C:\work\pebble` here, the same placeholder the
  Cursor fixtures use. `C:\Users\<operator>` became `C:\Users\dev`.
- **The system prompt.** `model.messages_snapshot` carried the CLI's entire
  system prompt and the full conversation. The system message is trimmed to its
  first sentence plus a `[system prompt trimmed in fixture]` marker. That first
  sentence is deliberately kept: the tests assert it never reaches the ledger,
  and a marker nobody could recognise would prove nothing.
- **Opaque provider blobs.** `apiCallId`, `api_id`, `encryptedContent`,
  `encrypted_content`, `reasoningOpaque`, `reasoning_opaque`, `reasoningId`,
  `model_call_id` and `previousResponseId` held multi-kilobyte base64 payloads
  tied to the account's traffic. Each is now `[opaque <key> elided in fixture]`.
  The keys themselves remain, so the adapter is still exercised against records
  that carry them.
- **Local skills.** `session.skills_loaded` listed the operator's own skills by
  name, description and path. Renamed to `skill-1`, `skill-2`, ... with the
  descriptions elided.

| File | What it captures |
| --- | --- |
| `plain-reply.jsonl` | A one-word answer with no tools. Exit 0. |
| `write-apply-patch.jsonl` | `apply_patch` creates `notes.txt`; `result.detailedContent` carries the `diff --git`. Exit 0. |
| `read-only-tools-denied.jsonl` | Run with `--deny-tool=write,shell`: `apply_patch` refused by rule `write`, `powershell` refused by rule `shell`, no file created. Exit 0. |
| `session-first-turn.jsonl` | A first run with a host-supplied `--session-id`. |
| `session-resumed-turn.jsonl` | The same session continued with `--resume=<uuid>`; `result.sessionId` matches the first turn's. |
| `policy-denied.jsonl` | An account whose plan does not include the CLI: four session records, then exit 1. |
| `stderr-policy-denied.txt` | The stderr of that run. |
| `stderr-unknown-model.txt` | The stderr of a run given a `--model` the account cannot use. Exit 1, no JSON records. |
