#!/usr/bin/env node
// What Electron's own carry-on code says for a set of stopped turns (B2): why each can be carried on, the
// instruction, the refusal, the checkpoint (mission-store reconcileMission) and the hand-over brief
// (main/handoff.ts composeHandoffPrompt), written to parity/handoff.json for core/src/carry_on_tests.rs.
//
//   node scripts/handoff-goldens.mjs [--electron <checkout>]
//
// The checkout must have apps/desktop's dependencies installed (it uses that esbuild). Nothing is sent anywhere.

import { mkdtemp, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const args = process.argv.slice(2)
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback)
const ROOT = resolve(option('--electron', process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, '..', '..', '..')))
const DESKTOP = join(ROOT, 'apps', 'desktop')
const require = createRequire(join(DESKTOP, 'package.json'))
const esbuild = createRequire(require.resolve('vite/package.json'))('esbuild')
const path = (p) => JSON.stringify(join(ROOT, p).split('\\').join('/'))

const work = await mkdtemp(join(tmpdir(), 'locust-handoff-goldens-'))
await writeFile(join(work, 'entry.ts'), `
export { composeHandoffPrompt, howTurnEnded } from ${path('apps/desktop/src/main/handoff.ts')}
export { carryOnReason, carryOnPrompt, carryOnRefusal } from ${path('apps/desktop/src/shared/carry-on.ts')}
export { reconcileMission } from ${path('packages/mission-store/src/checkpoint.ts')}
`, 'utf8')
await esbuild.build({
  entryPoints: [join(work, 'entry.ts')],
  bundle: true,
  format: 'esm',
  platform: 'node',
  target: 'node20',
  outfile: join(work, 'entry.mjs'),
  nodePaths: [join(DESKTOP, 'node_modules'), join(ROOT, 'node_modules')],
  absWorkingDir: ROOT,
  logLevel: 'warning'
})
const { composeHandoffPrompt, howTurnEnded, carryOnReason, carryOnPrompt, carryOnRefusal, reconcileMission } = await import(pathToFileURL(join(work, 'entry.mjs')).href)

let seq = 0
const at = (n) => `2026-11-12T03:00:${String(n).padStart(2, '0')}.000Z`
const ev = (type, payload) => { seq += 1; return { id: `run_x:native:${seq}`, sequence: seq, occurredAt: at(seq % 60), type, payload } }
const tool = (id, toolKind, name, command) => [ev('tool.started', { itemId: id, toolKind, name, ...(command === undefined ? {} : { command }), phase: 'started' })]
const done = (id, toolKind, name) => [ev('tool.completed', { itemId: id, toolKind, name, phase: 'completed' })]
const say = (item, text, final = true) => [ev('message.delta', { itemId: item, operation: 'append', text, final })]
const meta = (prompt, runtime = 'codex') => ({ missionId: 'mission_prior', runId: 'run_prior', prompt, runtime, model: 'account-default' })

const long = 'Refactor the payment module. '.repeat(170)
const cases = []
const add = (name, prompt, events, next, earlier = [], extra = {}) => { cases.push({ name, prompt, events, next, earlier, ...extra }); seq = 0 }

add('plan ran out mid-work', 'Fix the failing test in app.ts', [
  ...tool('c1', 'command', 'shell', 'npm test'), ...done('c1', 'command', 'shell'),
  ...tool('c2', 'file_change', 'apply_patch', 'app.ts'),
  ...say('m1', 'I found the bug in '), ...say('m1', 'parse().'),
  ev('route.limit_detected', { kind: 'quota-exhausted', message: 'You hit your usage limit.' }),
  ev('run.failed', { kind: 'quota-exhausted', message: 'You hit your usage limit.' })
], undefined)
seq = 0
add('provider failed first', 'Write a haiku', [
  ev('run.failed', { kind: 'process-failed', message: 'stream disconnected before completion', process: { stderr: 'error\nHTTP 503 Service Unavailable\n' } })
], undefined)
add('process crashed after a tool', 'Rename the file', [
  ...tool('c1', 'command', 'shell', 'git mv a b'), ...done('c1', 'command', 'shell'),
  ev('run.failed', { kind: 'process-failed', message: 'Codex exited with code 1.' })
], undefined)
add('refused is never carried', 'Read outside', [
  ev('run.failed', { kind: 'process-failed', message: 'The read was refused: outside the folder.' })
], undefined)
add('finished turn, reply on another agent', 'What does main.rs do?', [
  ...say('m1', 'It starts the window.'), ev('run.completed', {})
], 'Now add a test for it.', [{ asked: 'Open the project', answered: 'Opened.' }, { asked: 'List the files', answered: undefined }])
add('stopped by the person, reply', 'Summarize the repo', [
  ...tool('r1', 'read', 'read', 'README.md'), ...done('r1', 'read', 'read'), ev('run.cancelled', {})
], 'Carry on but shorter.')
add('long task clipped beside a reply', long, [
  ...say('m1', 'Started.'), ev('run.failed', { kind: 'quota-exhausted', message: 'limit' })
], 'Finish the refactor. '.repeat(160))
add('many settled actions shrink', 'Tidy every file', [
  ...Array.from({ length: 45 }, (_, i) => [...tool(`t${i}`, 'command', 'shell', `prettier --write src/file-number-${i}-${'x'.repeat(120)}.ts`), ...done(`t${i}`, 'command', 'shell')]).flat(),
  ev('run.failed', { kind: 'quota-exhausted', message: 'limit' })
], 'Keep going with the last ones. '.repeat(120))

const out = cases.map((c) => {
  const checkpoint = reconcileMission({ metadata: meta(c.prompt), events: c.events, issues: [] }, { reason: 'route-switch', epoch: 1, createdAt: at(0) })
  const briefing = composeHandoffPrompt(c.prompt, checkpoint, 'Codex CLI', c.next, c.earlier, undefined, [], howTurnEnded(c.events))
  const reason = carryOnReason(c.events)
  return {
    name: c.name, prompt: c.prompt, events: c.events, next: c.next ?? null, earlier: c.earlier.map((t) => ({ asked: t.asked, answered: t.answered ?? null })),
    reason: reason ?? null,
    instruction: reason === undefined ? null : carryOnPrompt('codex', reason),
    ended: howTurnEnded(c.events) ?? null,
    checkpoint,
    briefing: briefing === undefined ? null : { prompt: briefing.prompt, omitted: briefing.omitted, taskClipped: briefing.taskClipped === true }
  }
})
const refusals = ['claude', 'codex', 'cursor', 'opencode', 'copilot', 'antigravity'].flatMap((to) => ['approve-each', 'auto'].map((mode) => ({ mode, to, refusal: carryOnRefusal(mode, to) ?? null })))
const commit = execFileSync('git', ['-C', ROOT, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim()
await writeFile(join(here, '..', 'parity', 'handoff.json'), JSON.stringify({ electronCommit: commit, generatedBy: 'scripts/handoff-goldens.mjs', cases: out, refusals }, null, 1) + '\n', 'utf8')
console.log(`wrote ${out.length} cases and ${refusals.length} refusals from Electron ${commit}`)
