// Records what Electron's pure picker functions return, for the Rust port to replay (parity/recorded/picker.jsonl).
// Run through record.sh. Inputs: every string and string list written in the functions' own Electron test files, plus
// traps (empty, case, emoji and other text JavaScript counts in UTF-16 units, unknown words, odd suffixes).
import { readFileSync } from 'node:fs'
import { effortDescription, effortFooter, effortName } from '../../../desktop/src/renderer/src/effortLevels.ts'
import { effortRank, effortScale, joinEffort, levelsLine, splitEffort } from '../../../desktop/src/renderer/src/effortScale.ts'
import { freeTagOf } from '../../../desktop/src/renderer/src/routeName.ts'
import { defaultEffort } from '../../../desktop/src/renderer/src/status.ts'

// The renderer's source folder, from record.sh (import.meta.url is the bundle's temporary file).
const here = new URL(`file://${process.env.ELECTRON_RENDERER_SRC}/`)
const TEST_FILES = [
  'effortLevels.test.ts',
  'the-chip-keeps-free.test.ts',
  'the-effort-control-is-claude-codes.test.tsx',
  'the-picker-opens-on-what-works.test.ts',
  'a-model-is-one-line.test.tsx',
]

const strings = new Set<string>()
const lists: string[][] = []
for (const name of TEST_FILES) {
  let text = ''
  try {
    text = readFileSync(new URL(name, here), 'utf8')
  } catch {
    continue
  }
  for (const m of text.matchAll(/'((?:\\.|[^'\\\n])*)'/g)) strings.add(m[1].replace(/\\(.)/g, '$1'))
  for (const m of text.matchAll(/\[\s*('(?:\\.|[^'\\\n])*'\s*(?:,\s*'(?:\\.|[^'\\\n])*'\s*)*),?\s*\]/g)) {
    lists.push([...m[1].matchAll(/'((?:\\.|[^'\\\n])*)'/g)].map((x) => x[1].replace(/\\(.)/g, '$1')))
  }
}

const TRAP_WORDS = ['', ' ', 'none', 'minimal', 'low', 'medium', 'balanced', 'high', 'xhigh', 'max', 'ultra', 'fast',
  'HIGH', 'High', 'Low', 'turbo', 'high-fast', 'low-fast', 'medium-fast', 'xhigh-fast', 'max-fast', '-fast',
  'fast-fast', 'high-fast-fast', 'éclair', 'ñ', '🔥', '🔥hot', 'ǆ', 'ß', 'İ', '́a']
const TRAP_LABELS = ['', 'Free', ' Free', '  Free', 'X Free', 'x free', 'FREE', ' free', 'Model Free ', 'Model\tFree',
  'Muse Spark 1.3 Contributor Free', 'Kimi K2 Free', 'Gemini 🚀 Free', '🚀 Free', '🚀Free', 'é Free', 'ab Free',
  'a Free', 'Freedom', 'Grok free', '👨‍👩‍👧 Free', 'İ Free', 'ﬀ Free', 'Model FREE', 'Model Fr ee']

const words = [...new Set([...TRAP_WORDS, ...strings].filter((s) => s.length <= 40))]
const labels = [...new Set([...TRAP_LABELS, ...strings].filter((s) => s.length <= 80))]
const pool = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra', 'balanced', 'turbo', 'high-fast',
  'low-fast', 'fast', 'HIGH', '🔥']
const levelLists: string[][] = [[], ...lists, ...pool.map((a) => [a])]
for (const a of pool) for (const b of pool) levelLists.push([a, b])
for (let i = 0; i + 3 <= pool.length; i += 1) levelLists.push(pool.slice(i, i + 3), pool.slice(i).reverse())
levelLists.push(pool, [...pool].reverse(), ['low', 'low', 'high'], ['high-fast', 'high'], ['turbo', 'zoom', 'low'])

const out: string[] = []
const record = (fn: string, args: unknown[], value: unknown) =>
  out.push(JSON.stringify({ fn, args, out: value === undefined ? null : value }))

for (const w of words) {
  record('effortName', [w], effortName(w))
  record('effortDescription', [w], effortDescription(w))
  record('effortFooter', [w], effortFooter(w))
  record('splitEffort', [w], splitEffort(w))
  record('effortRank', [w], effortRank(w))
}
for (const l of labels) record('freeTagOf', [l], freeTagOf(l))
for (const list of levelLists) {
  record('effortScale', [list], effortScale(list))
  record('levelsLine', [list], levelsLine(list, effortName))
  record('defaultEffort', [list, null], defaultEffort(list))
  for (const d of ['high', 'medium', 'turbo', '']) record('defaultEffort', [list, d], defaultEffort(list, d))
  for (const base of ['low', 'high', 'max', 'turbo', '']) {
    for (const fast of [false, true]) record('joinEffort', [base, fast, list], joinEffort(base, fast, list))
  }
}
process.stdout.write(out.join('\n') + '\n')
