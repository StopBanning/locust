// Records what Electron says when a run fails (missionView.ts: failureMessage, sameSentence; runtime-adapters
// provider-error.ts: providerErrorSentence), for the Rust port to replay (parity/recorded/failure.jsonl). Run through
// record.sh. Inputs: every string in the functions' own test files, as messages and as stderr, under each runtime.
import { readFileSync } from 'node:fs'
import { failureMessage, sameSentence } from '../../../desktop/src/renderer/src/missionView.ts'
import { providerErrorSentence } from '../../../../packages/runtime-adapters/src/provider-error.ts'

const here = new URL(`file://${process.env.ELECTRON_RENDERER_SRC}/`)
const adapters = new URL('../../../../packages/runtime-adapters/test/', here)
const FILES: URL[] = [
  ...['a-limit-note-leaves-when-it-stops-being-true.test.ts', 'a-provider-error-is-readable-and-said-once.test.tsx', 'its-own-settings-file.test.ts', 'missionView.test.ts'].map((n) => new URL(n, here)),
  ...['provider-error.test.ts', 'opencode-error.test.ts', 'your-own-model-is-a-provider.test.ts'].map((n) => new URL(n, adapters))
]
const strings = new Set<string>()
for (const url of FILES) {
  let text = ''
  try {
    text = readFileSync(url, 'utf8')
  } catch {
    continue
  }
  for (const m of text.matchAll(/'((?:\\.|[^'\\\n])*)'/g)) strings.add(m[1].replace(/\\n/g, '\n').replace(/\\(.)/g, '$1'))
  for (const m of text.matchAll(/"((?:\\.|[^"\\\n])*)"/g)) strings.add(m[1].replace(/\\n/g, '\n').replace(/\\(.)/g, '$1'))
}
// Messages: sentences, not identifiers. Stderr: anything multi-word, plus log-shaped traps.
const messages = [...strings].filter((s) => s.length >= 8 && s.length <= 160 && /\s/.test(s))
const STDERR = [undefined, '', '\n\n', 'plain last line', 'first\nsecond\nthe last line', 'INFO all good\nERROR something broke',
  '{"level":"error","message":"rate limit exceeded"}', '2026-11-18T00:00:00Z ERROR quota exceeded for model', 'error: EBUSY: resource busy or locked, open \'/Users/x/.codex/config.toml\'',
  'Error: 429 Too Many Requests', 'out of capacity', 'The run failed.', '   \n  trailing spaces  \n']
const stderrs = [...new Set([...STDERR, ...messages.filter((_, i) => i % 10 === 0).slice(0, 6)])]
const RUNTIMES = [undefined, 'claude', 'codex', 'cursor', 'opencode', 'antigravity', 'copilot', 'kilo', 'muse']

const out: string[] = []
const record = (fn: string, args: unknown[], value: unknown) => out.push(JSON.stringify({ fn, args, out: value === undefined ? null : value }))
const attempt = (f: () => unknown) => {
  try {
    return f()
  } catch (e) {
    return { threw: String(e) }
  }
}
for (const runtime of RUNTIMES) for (const m of messages) record('providerErrorSentence', [m, runtime ?? null], attempt(() => providerErrorSentence(m, runtime as never)))
for (const runtime of RUNTIMES) {
  for (const m of messages.filter((_, i) => i % 4 === 0).slice(0, 20)) {
    for (const stderr of stderrs) {
      record('failureMessage', [m, stderr ?? null, runtime ?? null], attempt(() => failureMessage({ message: m, ...(stderr === undefined ? {} : { process: { stderr } }) }, runtime as never)))
    }
  }
}
for (const a of messages.slice(0, 40)) for (const b of [...messages.slice(0, 40), 'The run failed', '  the run failed.  ', '']) record('sameSentence', [a, b], sameSentence(a, b))
process.stdout.write(out.join('\n') + '\n')
