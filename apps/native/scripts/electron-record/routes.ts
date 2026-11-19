// Records Electron's routeModelName (routeName.ts) for the Rust port's route_model_name to replay
// (parity/recorded/routes.jsonl). Module state is left empty (no catalog spellings, no own models, no alias names
// learned at run time), which is what the Rust function, stateless, stands for.
// Inputs: every id-like string in the route-name test files, each with every runtime, plus traps.
import { readFileSync } from 'node:fs'
import { routeModelName } from '../../../desktop/src/renderer/src/routeName.ts'

// The renderer's source folder, from record.sh (import.meta.url is the bundle's temporary file).
const here = new URL(`file://${process.env.ELECTRON_RENDERER_SRC}/`)
const TEST_FILES = [
  'routeName.test.ts',
  'a-model-has-one-name.test.ts',
  'a-kilo-model-has-one-name.test.ts',
  'a-claude-route-names-its-version.test.tsx',
  'a-default-route-names-its-model.test.tsx',
  'your-own-model-is-named-as-yours.test.tsx',
  'the-chip-keeps-free.test.ts',
  'the-picker-opens-on-what-works.test.ts',
]
const RUNTIMES = ['claude', 'codex', 'cursor', 'copilot', 'opencode', 'antigravity', 'kilo', 'muse', 'gemini', 'apple']

const ids = new Set<string>()
for (const name of TEST_FILES) {
  let text = ''
  try {
    text = readFileSync(new URL(name, here), 'utf8')
  } catch {
    continue
  }
  for (const m of text.matchAll(/(['"`])([A-Za-z0-9._:/-]{2,80})\1/g)) {
    if (/[-/]/.test(m[2]) || /^[a-z]+$/.test(m[2])) ids.add(m[2])
  }
}
for (const trap of ['', 'auto', 'AUTO', ' Auto ', 'gpt-6', 'gpt-6-codex', 'gpt-6.1-mini', 'claude-opus-5-5',
  'claude-sonnet-5-5-20261001', 'opus', 'sonnet', 'haiku', 'fable', 'opencode/deepseek-v4.1-flash-free',
  'opencode/nemotron-3.5-lightning-free', 'deepseek-v4-flash', 'glm-5', 'qwen3-coder', 'kimi-k2-free', 'mimo-v2',
  'minimax-m3', 'grok-4.7-high', 'gemini-4-neon', 'gemini-4-hydrogen-medium', 'flash', 'pro', 'flash_lite',
  'cursor-auto', 'copilot-gpt-6', 'kilo/kilo-auto/free', 'nvidia/nemotron-3.5-lightning:free', 'a--b', '-lead',
  'trail-', 'émile-2', 'model-🚀-1', 'x-1-2-3', 'v1-2', 'gpt-oss-120b', 'llama-4-70b-vl', 'ai-21', 'cli-2',
  'own-0123abcd/my-model', 'acp_goose', 'acp_goose::deepseek-v4-flash']) ids.add(trap)

const out: string[] = []
for (const runtime of RUNTIMES) {
  for (const id of ids) {
    let value: unknown
    try {
      value = routeModelName(runtime, id)
    } catch (e) {
      value = { threw: String(e) }
    }
    out.push(JSON.stringify({ fn: 'routeModelName', args: [runtime, id], out: value }))
  }
}
process.stdout.write(out.join('\n') + '\n')
