// CALLED LINKS (2026-10-25). A green suite must catch a wrong password and an expired body being accepted.
// AN ACCOUNT'S COPIES (2026-10-30): and one account reading another's, a file past the quota, or a stranger at the room.
import { execFileSync } from 'node:child_process'
import { readFileSync, rmSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const report = fileURLToPath(new URL('../mutation-result.json', import.meta.url))
const HOOKS = { file: 'test/a-called-link-leaves-a-letter.test.ts', assertions: 7 }
const SYNC = { file: 'test/a-computer-keeps-a-sealed-copy-under-its-account.test.ts', assertions: 8 }
const mutations = [
  ['src/room-core.ts', HOOKS, 'an unknown password is accepted', '([, hook]) => same(hook.hash, hash)', '([, hook]) => hook.hash.length > 0', 'answers the same 404 for a wrong password, an unknown room and a closed link; rotation retires the old one'],
  ['src/room-core.ts', HOOKS, 'a week-old body is kept', 'letters.filter((letter) => now - letter.at < LETTER_KEPT_MS)', 'letters.filter(() => true)', 'collects offline letters once acknowledged, drops a week-old body and reports its loss'],
  ['src/sync.ts', SYNC, 'a copy is read under any account', 'export const syncPrefix = (user: number, room?: string): string => `acct/${String(user)}/', 'export const syncPrefix = (user: number, room?: string): string => `acct/${String(7)}/', 'are read from any room of the same account, and never from another account'],
  ['src/sync.ts', SYNC, 'the quota is not checked', 'if (total - before + request.size > SYNC_QUOTA)', 'if (false)', 'keep a file no larger than MAX_SYNC_OBJECT, and an account no larger than its quota'],
  ['src/room-core.ts', SYNC, 'any token keeps a copy', "!same(held.owner, await host.sha256(token))) return { status: 401, body: { error: 'auth' } }\n      if (held.github === undefined", "false) return { status: 401, body: { error: 'auth' } }\n      if (held.github === undefined", 'is nobody with another token, no token, or at a room not tied to an account']
]
function suite({ file, assertions: expected }) {
  rmSync(report, { force: true })
  try { execFileSync('npx', ['vitest', 'run', file, '--maxWorkers=2', '--reporter=json', '--outputFile=mutation-result.json'], { cwd: root, shell: true, windowsHide: true, stdio: 'pipe' }) } catch { /* The JSON says whether assertions failed. */ }
  const result = JSON.parse(readFileSync(report, 'utf8'))
  const assertions = result.testResults.flatMap((one) => one.assertionResults ?? [])
  if (assertions.length !== expected) throw new Error(`The expected assertions did not run in ${file}`)
  return assertions.filter((test) => test.status === 'failed').map((test) => test.title)
}
const originals = new Map()
try {
  for (const tests of [HOOKS, SYNC]) if (suite(tests).length > 0) throw new Error(`Baseline is not green: ${tests.file}`)
  for (const [path, tests, name, from, to, expected] of mutations) {
    const target = fileURLToPath(new URL(`../${path}`, import.meta.url))
    if (!originals.has(target)) originals.set(target, readFileSync(target, 'utf8'))
    const original = originals.get(target)
    if (!original.includes(from)) throw new Error(`Missing mutation anchor: ${name}`)
    writeFileSync(target, original.replace(from, to), 'utf8')
    const failed = suite(tests)
    writeFileSync(target, original, 'utf8')
    if (!failed.includes(expected)) throw new Error(`Mutation survived: ${name}`)
    process.stdout.write(`CAUGHT: ${name}\n`)
  }
} finally {
  for (const [target, original] of originals) writeFileSync(target, original, 'utf8')
  rmSync(report, { force: true })
}
