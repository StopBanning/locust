// CALLED LINKS (2026-10-25). A green suite must catch a wrong password and an expired body being accepted.
import { execFileSync } from 'node:child_process'
import { readFileSync, rmSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const target = fileURLToPath(new URL('../src/room-core.ts', import.meta.url))
const report = fileURLToPath(new URL('../mutation-result.json', import.meta.url))
const original = readFileSync(target, 'utf8')
const mutations = [
  ['an unknown password is accepted', '([, hook]) => same(hook.hash, hash)', '([, hook]) => hook.hash.length > 0', 'answers the same 404 for a wrong password, an unknown room and a closed link; rotation retires the old one'],
  ['a week-old body is kept', 'letters.filter((letter) => now - letter.at < LETTER_KEPT_MS)', 'letters.filter(() => true)', 'collects offline letters once acknowledged, drops a week-old body and reports its loss']
]
function suite() {
  rmSync(report, { force: true })
  try { execFileSync('npx', ['vitest', 'run', 'test/a-called-link-leaves-a-letter.test.ts', '--maxWorkers=2', '--reporter=json', '--outputFile=mutation-result.json'], { cwd: root, shell: true, windowsHide: true, stdio: 'pipe' }) } catch { /* The JSON says whether assertions failed. */ }
  const result = JSON.parse(readFileSync(report, 'utf8'))
  const assertions = result.testResults.flatMap((file) => file.assertionResults ?? [])
  if (assertions.length !== 7) throw new Error('The expected assertions did not run')
  return assertions.filter((test) => test.status === 'failed').map((test) => test.title)
}
try {
  if (suite().length > 0) throw new Error('Baseline is not green')
  for (const [name, from, to, expected] of mutations) {
    if (!original.includes(from)) throw new Error(`Missing mutation anchor: ${name}`)
    writeFileSync(target, original.replace(from, to), 'utf8')
    const failed = suite()
    writeFileSync(target, original, 'utf8')
    if (!failed.includes(expected)) throw new Error(`Mutation survived: ${name}`)
    process.stdout.write(`CAUGHT: ${name}\n`)
  }
} finally {
  writeFileSync(target, original, 'utf8')
  rmSync(report, { force: true })
}
