# Apple Intelligence as a runtime (17 Nov 2026)

Colin passed on a friend's question: "could you add Siri AI to Locust?" Siri takes no messages from another app, but the
model behind Apple Intelligence does: since macOS 26 any app may ask it through Foundation Models, on the Mac, free,
with no account, and nothing sent anywhere. Colin: "Yes, build it."

## What was measured first (this Mac, macOS 27.2.1)
- `SystemLanguageModel.default.availability`: available. `contextSize`: **8192** tokens.
- `streamResponse` yields the whole answer so far each time, not the words added (the helper diffs).
- A one-sentence answer takes about 2 s; a teammate turn with its brief, about 4 s.
- A prompt over the limit throws, in Apple's words: "Content contains 12059 tokens, which exceeds the maximum allowed
  context size of 8192." macOS 27 throws `LanguageModelError`; macOS 26 named it `GenerationError` (both are read).
- **Why not "Your own models" through OpenCode**: OpenCode sends a chat-only model 30,449 characters of system prompt
  and 10 tool definitions (51,869 characters, about 13,000 tokens) on every turn, measured with a throwaway listener.
  That is more than the model can read before the person has said a word.

## How it works
- `core/helpers/apple-model/main.swift`, built by `core/build.rs` with `xcrun swiftc` (macOS only; no Swift, or
  `LOCUST_NO_APPLE_HELPER` set, and the runtime is simply left out: cfg `apple_helper`), embedded with
  `include_bytes!`, and written to `$TMPDIR/locust-apple-model-<sha>` the first time it is needed (rewritten if the bytes
  differ). `--probe` says whether the model is there; otherwise one JSON request on stdin, JSON lines on stdout. A fresh
  session per request, no files, no network; it exits when the answer is done, or when Locust goes away.
- `core/src/runtime/apple.rs`: the driver (Muse's shape), the reader, the words for every way it cannot answer, the
  probe (once per launch), and `brief`.
- **Its own brief.** Locust's usual brief teaches the tags an agent writes (`<locust-ask>`, `<locust-memory>`,
  `<locust-next>`, files, shares). Measured: this model answered "what is a git branch?" correctly and then wrote out a
  `<locust-ask>` with the template's own placeholders, which would become junk cards. So it is told who it is, that it
  can only talk, the teammate's own instructions, then what was said. Answers came back clean after.
- **No session.** It emits no thread id, so every follow-up starts cold and is told what was said
  (`compose_cold_follow_up`, bounded at 8,000 characters). A real two-turn conversation fit.
- **Ask and Plan only** (Plan runs wherever Ask does). It has no tools: the inspector says it may not touch files,
  commands or the web.
- Listed in the model catalog as "On-device model" under "Apple Intelligence" only where the probe says available; no
  account-default row. Among the AI agents: READY, or UNAVAILABLE with the Mac's reason (turned off: where to turn it
  on; still downloading; a Mac that cannot run it); no row where there is no helper.
- Mark: a chip with a spark (`look/controls.rs`), not Apple's logo.

## Tests
`core/src/runtime/apple_tests.rs`: 17 with a synthetic helper (`scripts/fixtures/apple-model.py`), and two that ask the
real model, ignored unless run with `LOCUST_REAL_APPLE=1 ... --run-ignored only -E 'test(/real_apple/)'`.
Scene `models-apple`: the picker with Apple's model under Codex's.

## Not done
- **Electron** has none of this. Its ledger reader (`packages/mission-store`) refuses a runtime it does not list, so a
  Rust ledger of an Apple turn is not readable by Electron until Electron learns "apple" (a schema bump, v28). The
  friend runs Electron: this reaches them only with an Electron release, which is Colin's call.
- **Siri the other way round** ("Hey Siri, ask Locust ..."): App Intents in a signed app bundle. Later.
- Tools: Foundation Models can call tools, but with 8,192 tokens a coding agent's file tools do not fit. Talk only.
- Apple's Private Cloud Compute model (`PrivateCloudComputeLanguageModel`, macOS 27) is in the SDK; not tried.
