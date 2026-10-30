// Does what a teammate said between its steps stay where it said it?
//
//   node _tools/probe-narration-in-the-fold.mjs [--packaged <exe>] [--tag <name>]
//
// Yurt's beta report (#15), walkthrough 06: a free model's finished turn
// read "Creating your HELLO file and lining up verification. / File write is
// underway -- then I'll print it back to confirm. / DONE" under a fold that
// already showed the file written and printed -- the first two were said
// BEFORE the tool calls, as messages of their own. Seeds that turn as the
// runtime sent it, opens it, and reads where each line is drawn: in the fold
// among the steps, or below it. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `narration-in-the-fold-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = resolve(await scratchRepository('locust-narration-ws-'))
const WORKSPACE_ID = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-narration-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const FIRST = 'Creating your HELLO file and lining up verification.'
const SECOND = "File write is underway — then I'll print it back to confirm."
const ANSWER = 'DONE'
const at = (second) => new Date(Date.UTC(2026, 8, 23, 5, 0, second)).toISOString()
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at(0), finishedAt: at(40)
}
const metadata = {
  missionId: 'mission_hello', runId: 'run_hello',
  prompt: 'Create a file called hello.txt containing the single word HELLO, then run a command that prints it back, then say DONE.',
  runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode',
  cliVersion: '1.18.27', workspaceId: WORKSPACE_ID, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at(0)
}
const message = (itemId, text, second) => ({ type: 'message.delta', occurredAt: at(second), payload: { itemId, operation: 'append', text, final: true, evidence: { redacted: true } } })
const events = [
  { type: 'run.started', occurredAt: at(0), payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  message('m1', FIRST, 4),
  { type: 'tool.started', occurredAt: at(6), payload: { itemId: 't1', toolKind: 'file_change', name: 'write', phase: 'started', command: 'hello.txt', evidence: { redacted: true } } },
  { type: 'tool.completed', occurredAt: at(8), payload: { itemId: 't1', toolKind: 'file_change', name: 'write', phase: 'completed', command: 'hello.txt', evidence: { redacted: true } } },
  message('m2', SECOND, 12),
  { type: 'tool.started', occurredAt: at(14), payload: { itemId: 't2', toolKind: 'command_execution', name: 'shell', phase: 'started', command: 'cat hello.txt', evidence: { redacted: true } } },
  { type: 'tool.completed', occurredAt: at(16), payload: { itemId: 't2', toolKind: 'command_execution', name: 'shell', phase: 'completed', command: 'cat hello.txt', exitCode: 0, evidence: { redacted: true } } },
  message('m3', ANSWER, 30),
  { type: 'run.completed', occurredAt: at(40), payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: at(0), metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_hello:${String(index + 1)}`, runId: 'run_hello', missionId: 'mission_hello', sequence: index + 1, sourceAdapter: 'opencode' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_hello.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
  missionOwners: { mission_hello: 'tm_wren' },
  settings: { swarm: false, relay: false, relayHopCap: 2 }
}), 'utf8')

const drive = await startDrive({
  name: `narration-in-the-fold-${tag}`,
  port: 9438,
  workspace,
  profilePath: profile,
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged })
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const read = JSON.parse(await drive.capture('the seeded turn, opened', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return JSON.stringify({ error: 'no conversation row' })
    row.click()
    await new Promise((r) => setTimeout(r, 1500))
    // Whether the fold arrives open: the narration is only in view if it does.
    const toggle = document.querySelector('.lc-thread .lc-activity')
    const openOnArrival = toggle?.getAttribute('aria-expanded') ?? 'no fold'
    if (toggle && toggle.getAttribute('aria-expanded') !== 'true') {
      toggle.click()
      await new Promise((r) => setTimeout(r, 500))
    }
    const list = document.querySelector('.lc-thread .lc-activity__list')
    const rows = list ? [...list.querySelectorAll('.lc-filerow')].map((node) => node.classList.contains('lc-filerow--said') ? 'said: ' + node.textContent.trim() : 'step') : []
    const below = [...document.querySelectorAll('.lc-thread .lc-agentline__body')].map((node) => node.textContent.trim())
    return JSON.stringify({ openOnArrival, rows, below })
  })()`)))
  say(`fold open on arrival: ${String(read.openOnArrival)}`)
  say(`fold rows: ${JSON.stringify(read.rows)}`)
  say(`below the fold: ${JSON.stringify(read.below)}`)
  check('what was said before each step is in the fold, before that step', JSON.stringify(read.rows) === JSON.stringify([`said: ${FIRST}`, 'step', `said: ${SECOND}`, 'step']), JSON.stringify(read.rows))
  check('and only the answer is below the fold', JSON.stringify(read.below) === JSON.stringify([ANSWER]), JSON.stringify(read.below))
  say(failures === 0 ? '\nNARRATION IN THE FOLD PASSED' : `\nNARRATION IN THE FOLD: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: "Yurt's walkthrough turn, seeded as the runtime sent it, opened and read. Nothing was sent." })
}
