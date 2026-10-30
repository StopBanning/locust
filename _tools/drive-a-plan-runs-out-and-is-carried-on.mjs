// A turn whose plan ran out is carried on with one press (0.774).
//
//   node _tools/drive-a-plan-runs-out-and-is-carried-on.mjs [--packaged <exe>] [--tag <name>]
//
// A plan cannot be run out on purpose for free, so the stopped turn is SEEDED: Mira's Codex turn wrote "PELICAN,"
// and stopped on Codex's own "You've hit your usage limit." -- the record a real one leaves. Settings has a free
// OpenCode model picked to carry on (Without asking off), so the card offers it and nothing is spent. Pressed, the
// host starts the next turn on it as a reply on another agent: it must be briefed with the task and what was done
// (the answer finishes with HARBOR), the divider must say the plan ran out, no bubble may put Locust's words in the
// person's mouth, and the divider must come back from the record after a reload.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-carry-on-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-carry-on-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-15T01:00:00.000Z', route: FREE_ROUTE }
const missionId = 'mission_5e000000-0000-4000-8000-000774000000'
const runId = 'run_5e0774'
const at = new Date(Date.now() - 600_000).toISOString()
const ASK = 'Write the word PELICAN, then a comma, then the word HARBOR, all in capitals, on one line. Do not write anything else and do not use any tool.'
await ledger.createMission({
  missionId, runId, prompt: ASK,
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
})
const process_ = { exitCode: 1, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at }
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'codex', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'PELICAN,', final: false }),
  event(2, 'run.failed', { kind: 'quota-exhausted', message: "You've hit your usage limit. Try again later.", runtimeTerminal: 'failed', process: process_ })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `carry-on-${tag}`,
  port: 9774,
  workspace,
  profilePath,
  outPath: join(recordRoot('a-plan-runs-out-and-is-carried-on-2026-10-15'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [MIRA],
    missionOwners: { [missionId]: MIRA.teammateId },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', planRunsOut: { runtime: 'opencode', model: FREE_ROUTE.model, automatic: false } }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const text = (selector) => drive.evaluate(`[...document.querySelectorAll(${JSON.stringify(selector)})].map((one) => one.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')`)
const openConversation = `(async () => {
  ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /PELICAN/i.test(r.innerText))?.click()
  await new Promise((r) => setTimeout(r, 1500))
  return 1
})()`
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`

try {
  await drive.capture('launch: a Codex turn that ran out of its plan', () => drive.ready())
  await drive.resize(1215, 860)
  await drive.evaluate(openConversation)
  const card = String(await drive.capture('the stopped turn: its card offers to carry on', () => text('[data-carry-on]')))
  check('the card says the plan ran out and offers the picked agent by name', /Codex CLI ran out of its plan\./.test(card) && /Carry on with .*Free/i.test(card), card)

  const pressed = String(await drive.evaluate(`(() => {
    const button = document.querySelector('[data-carry-on] button')
    if (!button) return 'no button'
    button.click()
    return button.innerText.trim()
  })()`))
  say(`  pressed: ${pressed}`)
  // The new turn runs on the free model: wait for an answer after the divider, or for the turn to end without one.
  let thread = ''
  const deadline = Date.now() + 300_000
  while (Date.now() < deadline) {
    thread = String(await drive.evaluate(THREAD))
    const afterSeam = thread.slice(thread.indexOf('carried the work on'))
    if (/carried the work on/.test(thread) && /HARBOR/.test(afterSeam)) break
    if (/could not be started|could not carry/i.test(thread)) break
    await sleep(3000)
  }
  // Let the turn settle before it is read.
  await sleep(4000)
  thread = String(await drive.evaluate(THREAD))
  await drive.capture('carried on: the divider, then the answer', () => drive.evaluate(THREAD))
  const divider = String(await text('.lc-handoff'))
  check('the divider says Codex ran out of its plan and the work went to OpenCode', /Codex ran out of its plan, so Locust carried the work on with OpenCode\./.test(divider), divider)
  const afterSeam = thread.slice(thread.indexOf('carried the work on'))
  check('the next agent was briefed with the task: its answer finishes it with HARBOR', /HARBOR/.test(afterSeam), afterSeam.slice(0, 400))
  check('no bubble puts Locust\'s instruction in the person\'s voice', !/Carry on with the task from where it stopped/.test(thread), thread.slice(-600))
  const rows = String(await text('.lc-convrow'))
  check('it is still one conversation in the sidebar', (rows.match(/PELICAN/gi) ?? []).length === 1, rows)

  await drive.evaluate('location.reload()').catch(() => undefined)
  await sleep(4000)
  await drive.ready().catch(() => undefined)
  await drive.evaluate(openConversation)
  const reopened = String(await drive.capture('reloaded: the divider, from the record', () => text('.lc-handoff')))
  check('reloaded, the divider still says the plan ran out, from the record', /ran out of its plan, so Locust carried the work on/.test(reopened), reopened)
  const card2 = String(await text('[data-carry-on]'))
  check('and the stopped turn no longer offers to carry on', card2 === '', card2)
  // The first run of this drive passed every check live and then, reloaded, drew Locust's instruction as the
  // person's own message: the reopened run had lost who started it.
  const bubbles = String(await text('.lc-thread .lc-msg--user, .lc-thread [data-role="user"], .lc-thread .lc-user'))
  const reloadedThread = String(await drive.evaluate(THREAD))
  check('reloaded, still no bubble in the person\'s voice for the carried-on turn', !/Carry on with the task from where it stopped/.test(reloadedThread), `${bubbles.slice(0, 300)} || ${reloadedThread.slice(-500)}`)

  const settings = String(await drive.capture('Settings > AI agents: When a plan runs out', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
    await new Promise((r) => setTimeout(r, 900))
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('AI agents'))?.click()
    await new Promise((r) => setTimeout(r, 900))
    const section = document.querySelector('[data-setting="plan-runs-out"]')
    section?.scrollIntoView({ block: 'center' })
    await new Promise((r) => setTimeout(r, 400))
    const chosen = section?.querySelector('[role="radio"][aria-checked="true"]')?.innerText.trim() ?? 'none'
    return JSON.stringify({ text: (section?.innerText ?? 'no section').replace(/\\s+/g, ' ').slice(0, 500), chosen })
  })()`)))
  const shown = JSON.parse(settings)
  check('Settings names the picked agent, and Without asking is off', /^OpenCode/.test(shown.chosen) && /card offers to carry the work on with OpenCode/.test(shown.text) && /Off\. The card waits for your press\./.test(shown.text), settings)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded Codex turn that ran out of its plan; carried on with one press to ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
