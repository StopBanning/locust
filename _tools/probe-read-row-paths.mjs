// Does a turn's fold name the files it read, not their whole paths?
//
//   node _tools/probe-read-row-paths.mjs [--packaged <exe>] [--tag <name>]
//
// Yurt's beta report (2026-09-23, #16): "Read rows print full absolute paths,
// cut off" -- "read 3 — C:\Users\<home>\Documents\locust-scratch\locust-walk-
// ws-EfOL3P, C:\Users\<home>\Documents\loc..." on an OpenCode turn. Seeds an
// OpenCode run that read the folder and two files by their absolute paths,
// opens it, opens the fold, and reads the row. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `read-row-paths-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = resolve(await scratchRepository('locust-readrow-ws-'))
const WORKSPACE_ID = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-readrow-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const T0 = '2026-09-23T05:00:00.000Z'
const T1 = '2026-09-23T05:00:03.000Z'
const T2 = '2026-09-23T05:00:08.000Z'
const MODEL = 'opencode/muse-spark-1.3-contributor-free'
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 9, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: T0, finishedAt: T2
}
const metadata = {
  missionId: 'mission_reads', runId: 'run_reads', prompt: 'What is in this folder?',
  runtime: 'opencode', model: MODEL, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default',
  cliVersion: '1.18.27', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: T0
}
const read = (itemId, path) => [
  { type: 'tool.started', occurredAt: T1, payload: { itemId, toolKind: 'tool_use', name: 'read', command: path, phase: 'started', evidence: { redacted: true } } },
  { type: 'tool.completed', occurredAt: T1, payload: { itemId, toolKind: 'tool_use', name: 'read', command: path, phase: 'completed', evidence: { redacted: true } } }
]
const events = [
  { type: 'run.started', occurredAt: T0, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  ...read('r1', workspace),
  ...read('r2', join(workspace, 'README.md')),
  ...read('r3', join(workspace, 'src', 'index.ts')),
  { type: 'message.delta', occurredAt: T2, payload: { itemId: 'answer', operation: 'append', text: 'A README and one source file.', final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: T2, payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: T0, metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_reads:${String(index + 1)}`, runId: 'run_reads', missionId: 'mission_reads', sequence: index + 1, sourceAdapter: 'opencode' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_reads.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2 } }), 'utf8')

const drive = await startDrive({
  name: `read-row-paths-${tag}`,
  port: 9430,
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
  const seen = JSON.parse(await drive.capture('the fold of a turn that read three things', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return JSON.stringify({ error: 'no conversation row' })
    row.click()
    await new Promise((r) => setTimeout(r, 1200))
    const fold = document.querySelector('.lc-thread .lc-activity')
    if (!fold) return JSON.stringify({ error: 'no fold' })
    if (fold.getAttribute('aria-expanded') !== 'true') fold.click()
    await new Promise((r) => setTimeout(r, 500))
    const lines = [...document.querySelectorAll('.lc-thread .lc-activity__list > *')].map((node) => node.innerText.replace(/\\s+/g, ' ').trim()).filter(Boolean)
    return JSON.stringify({ lines })
  })()`)))
  say(`fold: ${JSON.stringify(seen)}`)
  const reads = (seen.lines ?? []).find((line) => /^read 3/.test(line)) ?? ''
  check('the reads fold into one row', reads.length > 0, (seen.lines ?? []).join(' | ') || seen.error)
  check('naming the folder and the files, not the disk', /read 3 — locust-readrow-ws-[^,]+, README\.md, src\/index\.ts/.test(reads) && !/[A-Z]:\\\\/.test(reads), reads)
  say(failures === 0 ? '\nREAD ROW PATHS PASSED' : `\nREAD ROW PATHS: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'An OpenCode turn that read the folder and two files by absolute path, seeded and reopened. Nothing was sent.' })
}
