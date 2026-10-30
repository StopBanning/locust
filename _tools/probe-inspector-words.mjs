// Does the inspector's Activity rail say what happened in words?
//
//   node _tools/probe-inspector-words.mjs [--packaged <exe>] [--tag <name>]
//
// The design review (#10): "Inspector labels in words, not step.started".
// Every rail row was the ledger's event name -- `runtime.started · codex`,
// `tool.completed · shell`, `run.completed`. Seeds one run that ran a
// command, opens it, opens Activity, and reads the rail. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `inspector-words-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-inspector-ws-')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-inspector-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const T0 = '2026-09-23T05:00:00.000Z'
const T1 = '2026-09-23T05:00:04.000Z'
const T2 = '2026-09-23T05:00:09.000Z'
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 5, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: T0, finishedAt: T2
}
const metadata = {
  missionId: 'mission_words', runId: 'run_words', prompt: 'Count the lines in the README',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
  cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: T0
}
const events = [
  { type: 'run.started', occurredAt: T0, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  { type: 'tool.started', occurredAt: T1, payload: { itemId: 'cmd_1', toolKind: 'command_execution', name: 'shell', phase: 'started', command: 'wc -l README.md', evidence: { redacted: true } } },
  { type: 'tool.completed', occurredAt: T1, payload: { itemId: 'cmd_1', toolKind: 'command_execution', name: 'shell', phase: 'completed', command: 'wc -l README.md', exitCode: 0, evidence: { redacted: true } } },
  { type: 'message.delta', occurredAt: T2, payload: { itemId: 'answer', operation: 'append', text: 'README.md has 1 line.', final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: T2, payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: T0, metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_words:${String(index + 1)}`, runId: 'run_words', missionId: 'mission_words', sequence: index + 1, sourceAdapter: 'codex' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_words.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2 } }), 'utf8')

const drive = await startDrive({
  name: `inspector-words-${tag}`,
  port: 9427,
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
  const rail = JSON.parse(await drive.capture('the Activity rail of a finished run', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return JSON.stringify({ error: 'no conversation row' })
    row.click()
    await new Promise((r) => setTimeout(r, 1200))
    const activity = [...document.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Activity')
    if (!activity) return JSON.stringify({ error: 'no Activity button' })
    activity.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-rail__name'); i += 1) await new Promise((r) => setTimeout(r, 200))
    return JSON.stringify({ rows: [...document.querySelectorAll('.lc-rail__event')].map((node) => ({
      name: node.querySelector('.lc-rail__name')?.innerText.trim() ?? '',
      meta: node.querySelector('.lc-rail__meta')?.innerText.trim() ?? ''
    })) })
  })()`)))
  say(`rail: ${JSON.stringify(rail)}`)
  const names = (rail.rows ?? []).map((row) => row.name)
  check('the rail has the run in it', names.length >= 3, rail.error ?? String(names.length))
  check('no row is an event name', !names.some((name) => /\b(run|tool|step|route|plan|runtime|adapter)\.[a-z_]+/.test(name)), names.join(' | '))
  check('it says where the run started, and that it finished', names.some((name) => /^Started on /.test(name)) && names.includes('Finished'), names.join(' | '))
  check('the command reads as the command', names.includes('shell · wc -l README.md') && names.includes('shell finished'), names.join(' | '))

  const details = JSON.parse(await drive.capture('the Details tab', () => drive.evaluate(`(async () => {
    const tab = [...document.querySelectorAll('.lc-tab')].find((b) => b.innerText.trim() === 'Details')
    if (!tab) return JSON.stringify({ error: 'no Details tab' })
    tab.click()
    await new Promise((r) => setTimeout(r, 400))
    const pairs = []
    for (const dt of document.querySelectorAll('.lc-inspector__scroll dt')) pairs.push([dt.textContent.trim(), dt.nextElementSibling?.textContent.trim() ?? ''])
    return JSON.stringify({ pairs })
  })()`)))
  say(`details: ${JSON.stringify(details)}`)
  const value = (label) => (details.pairs ?? []).find(([name]) => name === label)?.[1] ?? ''
  check('Details names the runtime and model the way the composer does', value('Runtime') === 'Codex CLI 0.153.0' && value('Model') === 'Account Default', `${value('Runtime')} / ${value('Model')}`)
  check('and says which sign-in the run used, not its route id', value('Account') === 'your Codex CLI sign-in' && !(details.pairs ?? []).some(([, said]) => /-account:/.test(said)), value('Account'))
  say(failures === 0 ? '\nINSPECTOR WORDS PASSED' : `\nINSPECTOR WORDS: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: "A finished run's Activity rail, seeded and reopened. Nothing was sent." })
}
