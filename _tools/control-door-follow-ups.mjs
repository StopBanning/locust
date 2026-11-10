// Each regression must fail as an assertion when its fix is removed.
import { readFileSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '..')
const door = 'apps/desktop/src/main/locust-mcp-tools.ts'
const test = 'src/main/a-carried-conversation-keeps-its-route-and-reports-its-status.test.ts'
const controls = [
  [door, 'const kept = previous?.runtime === undefined', 'const kept = true', 'apps/desktop', test, 'continues on the carried runtime'],
  [door, "? 'waiting_for_person' : 'working'", "? 'working' : 'working'", 'apps/desktop', test, 'keeps readable text beside machine status'],
  [door, 'await options.turnFolder?.(result.data.missionId) ?? folder', 'undefined ?? folder', 'apps/desktop', test, 'continues on the carried runtime'],
  ['apps/desktop/src/shared/carry-on.ts', "? 'process' : undefined", '? undefined : undefined', 'apps/desktop', test, 'offers a process crash after tools ran'],
  ['packages/runtime-adapters/src/process-runner.ts', 'text = projectCursorResult(text)', 'text = text', 'packages/runtime-adapters', 'test/a-large-cursor-result-keeps-its-terminal-status.test.ts', 'keeps success and failure after a repeated answer']
]
for (const [file, from, to, cwd, tests, name] of controls) {
  const path = resolve(root, file)
  const original = readFileSync(path, 'utf8')
  if (!original.includes(from)) throw new Error(`Missing control: ${from}`)
  try {
    writeFileSync(path, original.replace(from, to))
    const run = spawnSync('npx', ['-y', 'pnpm@11.19.0', 'exec', 'vitest', 'run', tests, '-t', name, '--reporter=json'], { cwd: resolve(root, cwd), encoding: 'utf8' })
    const output = run.stdout.slice(run.stdout.indexOf('{'))
    const report = JSON.parse(output)
    const caught = report.testResults.flatMap(file => file.assertionResults).some(test => test.status === 'failed' && test.title.includes(name))
    if (run.status === 0 || !caught) throw new Error(`Control did not fail its named assertion: ${name}\n${run.stderr}`)
    console.log(`Caught: ${name} (${from})`)
  } finally { writeFileSync(path, original) }
}
