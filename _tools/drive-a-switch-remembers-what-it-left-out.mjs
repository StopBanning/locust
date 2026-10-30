// A switch to another AI agent still says what it left out, after a reopen (0.519).
//
//   node _tools/drive-a-switch-remembers-what-it-left-out.mjs [--packaged <exe>] [--tag <name>]
//
// The divider after a switch said, in amber, which parts of the summary did
// not fit -- but only in the window that made the switch: the record kept
// none of it, so a reopened conversation drew the divider without it. A
// seeded conversation switched from OpenCode to Codex, its record saying the
// last reply and the earlier messages were left out. Reopened, the divider
// must say so, in words. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-switch-left-out-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-switch-left-out-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_juno0000000000000000', name: 'Juno', hue: 'violet', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z' }
const PROCESS = (at) => ({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at })
const first = 'mission_5e000000-0000-4000-8000-000600000001'
const second = 'mission_5e000000-0000-4000-8000-000600000002'
const at1 = new Date(Date.now() - 1_200_000).toISOString()
const at2 = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId: first, runId: 'run_5e00061', prompt: 'Compare the three CRM vendor quotes',
  runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at1
})
const event = (missionId, runId, sequence, type, occurredAt, payload) => ({ id: `event_${missionId.slice(-2)}_${String(sequence)}`, runId, missionId, sequence, occurredAt, sourceAdapter: missionId === first ? 'opencode' : 'codex', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(first, [
  event(first, 'run_5e00061', 1, 'message.delta', at1, { itemId: 'answer', operation: 'append', text: 'HubSpot Starter is the cheapest that meets all five.', final: true }),
  event(first, 'run_5e00061', 2, 'run.completed', at1, { usage: { inputTokens: 900, outputTokens: 40 }, process: PROCESS(at1) })
])
const checkpoint = await ledger.createCheckpoint(first, 'route-switch')
// The reply on Codex, as the host writes it: the brief, ending with the person's words.
await ledger.createMission({
  missionId: second, runId: 'run_5e00062',
  prompt: 'You are continuing work that another agent (OpenCode) started.\n\nThe person now asks:\n\nNow make it a table',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at2,
  continuesFrom: { missionId: first, checkpointEpoch: checkpoint.epoch, reason: 'route-switch', leftOut: ['summary', 'earlier'] }
})
await ledger.appendEvents(second, [
  event(second, 'run_5e00062', 1, 'message.delta', at2, { itemId: 'answer', operation: 'append', text: '| Vendor | Price |\n| --- | --- |\n| HubSpot | $20 |', final: true }),
  event(second, 'run_5e00062', 2, 'run.completed', at2, { usage: { inputTokens: 900, outputTokens: 40 }, process: PROCESS(at2) })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `switch-left-out-${tag}`,
  port: 9841,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-switch-remembers-what-it-left-out-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: { [first]: TEAMMATE.teammateId, [second]: TEAMMATE.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  const divider = String(await drive.capture('the conversation, reopened', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM vendor/i.test(r.innerText))?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-handoff'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 500))
    return document.querySelector('.lc-handoff')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no divider'
  })()`)))
  // Since 0.779 each side is its model by name, beside its runtime's mark; Codex on its account default is "Codex".
  check('the divider is drawn between the two AI agents', /Handed off\s*Mimo V2\.6 Flash( Free)?\s*→\s*Codex/i.test(divider), divider)
  check('and says, after a reopen, what the summary left out -- in words', /Left out of the summary to fit: its last reply and the earlier messages\./.test(divider), divider)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A recorded switch from OpenCode to Codex that left two parts out; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
