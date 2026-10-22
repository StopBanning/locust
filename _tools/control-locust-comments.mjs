// Scoped mutation controls. Runs only the comment test file; never launches Locust or reaches GitHub.
import { readFileSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { join, dirname } from 'node:path'
import { createRequire } from 'node:module'
const root = fileURLToPath(new URL('../apps/desktop/', import.meta.url))
const vitest = join(dirname(createRequire(new URL('../apps/desktop/package.json', import.meta.url)).resolve('vitest/package.json')), 'vitest.mjs')
const test = 'src/main/scout-turns-your-comments-into-asks.test.ts'
const controls = [
  ['src/main/scout-comments.ts', "author.login.toLowerCase() !== own.toLowerCase()", 'false', 'ignores another author, an old comment edited today, and an inline trigger'],
  ['src/main/scout-comments.ts', 'seen.has(String(row.id))', 'false', 'reads the signed-in login once and a single since-bounded comments page per look'],
  ['src/main/scout-comments.ts', 'mate.name.toLowerCase() === first?.toLowerCase()', 'false', 'routes a named teammate without changing the task and marks all issue context as other authors words'],
  ['src/main/scout.ts', '|| card.post.attempted || deps.postComment', '|| false || deps.postComment', 'does not post until pressed and sends edited text through fake gh stdin to the right endpoint']
]
for (const [file, before, after, name] of controls) {
  const path = join(root, file), original = readFileSync(path, 'utf8')
  if (!original.includes(before)) throw new Error(`Missing mutation anchor: ${before}`)
  try {
    writeFileSync(path, original.replace(before, after))
    const result = spawnSync(process.execPath, [vitest, 'run', test, '-t', name, '--reporter=json'], { cwd: root, encoding: 'utf8', windowsHide: true })
    let report
    try { report = JSON.parse(result.stdout) } catch { throw new Error(`Mutation did not produce a test report: ${result.stderr}`) }
    const assertion = report.testResults?.flatMap((suite) => suite.assertionResults ?? []).find((one) => one.title === name)
    if (result.status === 0 || assertion?.status !== 'failed' || !assertion.failureMessages?.some((message) => /AssertionError/.test(message))) throw new Error(`Mutation was not caught by its named assertion: ${name}`)
    console.log(`CAUGHT: ${name}`)
  } finally { writeFileSync(path, original) }
}
