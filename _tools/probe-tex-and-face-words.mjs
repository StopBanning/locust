// Does a reply draw a lone TeX macro as its symbol, and does a face say its
// state in words?
//
//   node _tools/probe-tex-and-face-words.mjs [--packaged <exe>] [--tag <name>]
//
// Yurt's beta report (#1): an Antigravity reply read `README.md $\rightarrow$
// docs/README.md`. Seeds a finished run whose answer has a lone macro, two
// sums of money and a formula, opens it, and reads the reply as drawn: the
// macro becomes its arrow and nothing else changes. Then B4's remainder: the
// faces strip's face for the run's teammate carries its state as a
// description, and keeps the name the drives find it by. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `tex-and-face-words-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = resolve(await scratchRepository('locust-tex-ws-'))
const WORKSPACE_ID = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-tex-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const ANSWER = 'Moved README.md $\\rightarrow$ docs/README.md. The plan costs $5 and $10 later, and $x \\to y$ stays a formula.'
const DRAWN = 'Moved README.md → docs/README.md. The plan costs $5 and $10 later, and $x \\to y$ stays a formula.'

const at = (second) => new Date(Date.UTC(2026, 8, 23, 5, 0, second)).toISOString()
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at(0), finishedAt: at(9)
}
const metadata = {
  missionId: 'mission_tex', runId: 'run_tex', prompt: 'Move the README into docs',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
  cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at(0)
}
const events = [
  { type: 'run.started', occurredAt: at(0), payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  { type: 'message.delta', occurredAt: at(8), payload: { itemId: 'answer', operation: 'append', text: ANSWER, final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: at(9), payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: at(0), metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_tex:${String(index + 1)}`, runId: 'run_tex', missionId: 'mission_tex', sequence: index + 1, sourceAdapter: 'codex' }
  }))
})
await writeFile(join(profile, 'mission-ledger', 'mission_tex.jsonl'), `${lines.join('\n')}\n`, 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
  missionOwners: { mission_tex: 'tm_wren' },
  settings: { swarm: false, relay: false, relayHopCap: 2 }
}), 'utf8')

const drive = await startDrive({
  name: `tex-and-face-words-${tag}`,
  port: 9435,
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
  const reply = JSON.parse(await drive.capture('the seeded reply, opened', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return JSON.stringify({ error: 'no conversation row' })
    row.click()
    let body
    for (let i = 0; i < 40 && !body; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      body = [...document.querySelectorAll('.lc-thread .lc-agentline__body')].find((node) => /README/.test(node.textContent))
    }
    return JSON.stringify({ text: body ? body.textContent.replace(/\\s+/g, ' ').trim() : null })
  })()`)))
  say(`reply: ${JSON.stringify(reply)}`)
  check('the lone macro is drawn as its arrow, and the money and the formula are as written', reply.text === DRAWN, reply.text ?? reply.error)

  const face = JSON.parse(await drive.evaluate(`(() => {
    const face = ${teammateFace('Wren')}
    return JSON.stringify(face ? { label: face.getAttribute('aria-label'), description: face.getAttribute('aria-description') } : null)
  })()`))
  say(`face: ${JSON.stringify(face)}`)
  check("Wren's face is still found by its name", face !== null && face.label === 'Wren — open their conversation', JSON.stringify(face))
  check('and says what Wren is doing, in words', face !== null && typeof face.description === 'string' && face.description.length > 0, face?.description ?? 'none')
  say(failures === 0 ? '\nTEX AND FACE WORDS PASSED' : `\nTEX AND FACE WORDS: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A finished run seeded with a lone TeX macro, money and a formula in its answer, and the faces strip read. Nothing was sent.' })
}
