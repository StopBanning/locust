# Muse Code captures

`exec-help.txt` is `muse exec --help` verbatim. It is what the capability
probe reads, and the reason it is kept here rather than only in the probe
folder: the detection was pointed at the TOP-LEVEL `muse --help` at first,
which names the subcommands and not one of the flags a mission passes, so
discovery reported a working runtime `unsupported`.

`echo-provider-run.jsonl` is one real `muse exec --provider echo --json`
stream from Muse Code 1.3.0-R3401.1, captured 2026-09-21 on Windows. It is
the only source the `muse-events` adapter was built from, and the full probe
it came out of — `--help`, `exec --help`, `--version`, the stderr, and the
240 KB MSP schema the binary generates for itself — is in
`docs/muse-probe-2026-09-21/`.

Nothing was scrubbed, because there was nothing to scrub: the prompt is
`Reply with exactly PING`, the workspace was a throwaway temp directory whose
path appears only on stderr, and the echo provider contacts no model, so the
stream carries no provider blobs and no system prompt.

**The echo provider is free and needs no account.** Regenerate this file, or
capture a longer one, with:

```
muse exec --provider echo --json --workspace <a temp folder> "<a prompt>"
```

The capture arrived with two artefacts of PowerShell's redirect rather than
of Muse: a UTF-8 byte-order mark on the front, and CRLF line endings. **The
mark is deliberately left in** — it is why the adapter strips one, and it
makes the first record this suite hands over the awkward one. The carriage
returns are not preserved: git normalised them to LF on commit, which is
fine and is why the loader strips a trailing `\r` anyway — a file
re-captured on this machine will have them again.

**What this capture does not contain: a tool call under a paying provider.**
The three tasks in it are Muse's own machinery — a skill reminder, the model
turn, a verify reminder. The adapter's tool rows are therefore exercised by a
synthetic lifecycle built on this envelope, and that test says so where it
stands.

`wonder-tool-run-1.4.6.jsonl` is the first paid run, 2026-10-12: Muse Code
1.4.6-R6000.1, `--model muse-wonder-1.0 --reasoning-effort low
--max-model-steps 4 --disable-write --disable-shell`, asked to list the files
in a folder of four. It called `search` three times and answered (missing two
of the four files: the model's miss, kept as it was). Its 95 records showed
what the echo capture never could: `run.model.configured`, `tool.result`,
and the task states `status`, `output` and `rejected`.
