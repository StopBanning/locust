// Does a long finished fold keep the answer near?
//
//   node _tools/probe-long-fold.mjs [--packaged <exe>] [--tag <name>]
//
// Yurt's #2 / B3: a twelve-file turn opened a twenty-five-row fold 969px
// tall in an 800px window, and the answer was off the screen. Folds stay
// open (Colin 9/8); a finished one now shows its first ten rows and "Show N
// more". Seeds a run with fifteen commands, opens it, reads the fold, and
// presses "Show". SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `long-fold-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = resolve(await scratchRepository('locust-long-fold-ws-'))
const WORKSPACE_ID = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-long-fold-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const COMMANDS = 15
const at = (second) => new Date(Date.UTC(2026, 8, 23, 5, 0, second)).toISOString()
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at(0), finishedAt: at(59)
}
const metadata = {
  missionId: 'mission_long', runId: 'run_long', prompt: 'Check each package builds',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
  cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at(0)
}
const events = [{ type: 'run.started', occurredAt: at(0), payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } }]
for (let index = 1; index <= COMMANDS; index += 1) {
  const command = `pnpm --filter package-${String(index)} build`
  events.push({ type: 'tool.started', occurredAt: at(index * 3), payload: { itemId: `cmd_${String(index)}`, toolKind: 'command_execution', name: 'shell', phase: 'started', command, evidence: { redacted: true } } })
  events.push({ type: 'tool.completed', occurredAt: at(index * 3 + 1), payload: { itemId: `cmd_${String(index)}`, toolKind: 'command_execution', name: 'shell', phase: 'completed', command, exitCode: 0, evidence: { redacted: true } } })
}
events.push({ type: 'message.delta', occurredAt: at(58), payload: { itemId: 'answer', operation: 'append', text: 'All fifteen packages build.', final: true, evidence: { redacted: true } } })
events.push({ type: 'run.completed', occurredAt: at(59), payload: { process: PROCESS, evidence: { redacted: true } } })
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: at(0), metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_long:${String(index + 1)}`, runId: 'run_long', missionId: 'mission_long', sequence: index + 1, sourceAdapter: 'codex' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_long.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2 } }), 'utf8')

const drive = await startDrive({
  name: `long-fold-${tag}`,
  port: 9433,
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
const READ = `(() => {
  const list = document.querySelector('.lc-thread .lc-activity__list')
  const answer = [...document.querySelectorAll('.lc-thread .lc-agentline__body')].find((node) => /fifteen/.test(node.textContent))
  const view = document.querySelector('.lc-thread')?.getBoundingClientRect()
  return JSON.stringify({
    rows: list ? list.querySelectorAll('.lc-filerow.is-shell').length : -1,
    more: document.querySelector('.lc-thread .lc-activity__more')?.textContent.trim() ?? '',
    answerTop: answer ? Math.round(answer.getBoundingClientRect().top) : null,
    viewBottom: view ? Math.round(view.bottom) : null
  })
})()`

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const opened = await drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return 'no conversation row'
    row.click()
    await new Promise((r) => setTimeout(r, 1200))
    const fold = document.querySelector('.lc-thread .lc-activity')
    if (fold && fold.getAttribute('aria-expanded') !== 'true') fold.click()
    await new Promise((r) => setTimeout(r, 500))
    return 'opened'
  })()`)
  check('the seeded run opens', opened === 'opened', opened)
  const capped = JSON.parse(await drive.capture('the finished fold, capped', () => drive.evaluate(READ)))
  say(`capped: ${JSON.stringify(capped)}`)
  check('the finished fold shows its first ten rows', capped.rows === 10, String(capped.rows))
  check(`and says how many more`, capped.more === `Show ${String(COMMANDS - 10)} more`, capped.more)
  const all = JSON.parse(await drive.capture('after Show more', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-thread .lc-activity__more')?.click()
    await new Promise((r) => setTimeout(r, 400))
    return ${READ}
  })()`)))
  say(`all: ${JSON.stringify(all)}`)
  check('one press shows every row', all.rows === COMMANDS && all.more === '', `${String(all.rows)} rows, button "${all.more}"`)
  say(failures === 0 ? '\nLONG FOLD PASSED' : `\nLONG FOLD: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A finished run with fifteen commands, seeded and reopened. Nothing was sent.' })
}
