// Is a tool call a model wrote as TEXT drawn as code, not as its reply?
//
//   node _tools/probe-tool-call-as-text.mjs [--packaged <exe>] [--tag <name>]
//
// The 0.271 design recheck: a free model via OpenCode wrote the edit it meant
// to make -- `<tool_call><function=edit>...` -- as words, and the thread drew
// it as the reply's first paragraph. That reply is seeded into a scratch
// profile's ledger and opened from Missions. SPENDS NOTHING: no run starts.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `tool-call-as-text-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-toolcall-ws-')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-toolcall-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const T0 = '2026-09-23T05:00:00.000Z'
const T1 = '2026-09-23T05:00:12.000Z'
const MODEL = 'opencode/ling-3.0-flash-fin-free'
const REPLY = '<tool_call><function=edit><parameter=path>notes.md</parameter><parameter=text>Ship it</parameter></function></tool_call>\nI updated the notes.'
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: T0, finishedAt: T1
}
const metadata = {
  missionId: 'mission_toolcall', runId: 'run_toolcall', prompt: 'Update the notes to say ship it',
  runtime: 'opencode', model: MODEL, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default',
  cliVersion: '1.18.27', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: T0
}
const events = [
  { type: 'run.started', occurredAt: T0, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  { type: 'message.delta', occurredAt: T1, payload: { itemId: 'answer', operation: 'append', text: REPLY, final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: T1, payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: T0, metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_toolcall:${String(index + 1)}`, runId: 'run_toolcall', missionId: 'mission_toolcall', sequence: index + 1, sourceAdapter: 'opencode' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_toolcall.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2 } }), 'utf8')

const drive = await startDrive({
  name: `tool-call-as-text-${tag}`,
  port: 9423,
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
  const opened = await drive.evaluate(`(async () => {
    const tab = [...document.querySelectorAll('button')].find((b) => /^(Conversations|Missions)$/.test(b.innerText.trim()))
    if (!tab) return 'no Missions button'
    tab.click()
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = [...document.querySelectorAll('.lc-missionrow')].find((n) => /ship it/i.test(n.innerText))
    }
    if (!row) return 'no row for the seeded mission'
    // While the row is in view: its cost cell (Yurt's #14 -- a bare dash read
    // as an empty column). The free model reported no number.
    window.__costCell = row.querySelector('.lc-missionrow__cost')?.innerText.trim() ?? ''
    row.click()
    await new Promise((r) => setTimeout(r, 1500))
    return 'opened'
  })()`)
  check('the seeded mission opens', opened === 'opened', opened)
  const cell = await drive.evaluate('window.__costCell')
  check("its Missions row says the free model's run was free, not a bare dash", cell === 'free', cell)
  const seen = JSON.parse(await drive.capture('the reply, as the thread draws it', () => drive.evaluate(`(() => {
    const body = document.querySelector('.lc-thread .lc-agentline__body')
    if (!body) return JSON.stringify({ error: 'no reply in the thread' })
    const code = body.querySelector('pre.lc-code')
    const prose = [...body.children].filter((node) => node.tagName !== 'PRE').map((node) => node.innerText.trim()).filter(Boolean)
    return JSON.stringify({
      label: code?.querySelector('.lc-code__lang')?.innerText.trim() ?? '',
      code: code?.querySelector('code')?.innerText ?? '',
      prose,
      first: body.innerText.trim().slice(0, 80)
    })
  })()`)))
  say(`seen: ${JSON.stringify(seen)}`)
  check('the call is a code block labelled "tool call"', /tool call/i.test(seen.label) && /<function=edit>/.test(seen.code), `${seen.label}: ${seen.code.slice(0, 60)}`)
  check('what the model said to the person is prose, outside it', seen.prose.some((line) => line === 'I updated the notes.'), JSON.stringify(seen.prose))
  check('no paragraph of the reply is the raw call', !seen.prose.some((line) => /<tool_call>|<function=/.test(line)), JSON.stringify(seen.prose))
  say(failures === 0 ? '\nTOOL CALL AS TEXT PASSED' : `\nTOOL CALL AS TEXT: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A reply whose first line is a tool call the model wrote as text, seeded and reopened. Nothing was sent.' })
}
