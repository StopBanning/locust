// A reply on another runtime says what it carries before it is sent (0.517).
//
//   node _tools/drive-a-handoff-says-what-it-carries.mjs [--packaged <exe>] [--tag <name>]
//
// Product ideas, round four: the brief a new runtime starts from drops whole
// sections to fit, and nothing showed that before the switch. A seeded
// conversation finished on OpenCode; the box is moved to Codex, so the next
// message is a handoff. Typing a short reply must say what goes (the task and
// its last reply); a long one must say what is left out to fit. Nothing is
// sent.
//
// Since 0.552 each chat keeps its own model: reopening the conversation puts
// the box back on OpenCode whatever the teammate is on now, so the drive moves
// the box itself, as a person switching models mid-conversation does.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, pickRouteScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-handoff-carries-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-handoff-carries-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// Now on Codex: the conversation below was on OpenCode, so the next message switches.
const TEAMMATE = { teammateId: 'tm_wren00000000000000000', name: 'Wren', hue: 'lime', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }
const missionId = 'mission_5e000000-0000-4000-8000-000400000000'
const runId = 'run_5e0004'
const at = new Date(Date.now() - 600_000).toISOString()
// A reply long enough that a long next message squeezes it out.
const reply = `Here is the comparison of the three vendors. ${'Each one was checked against the five requirements you listed, with prices for five seats. '.repeat(12)}HubSpot Starter is the cheapest that meets all five.`
await ledger.createMission({
  missionId, runId, prompt: 'Compare the three CRM vendor quotes and recommend one',
  runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: reply, final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 400 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `handoff-carries-${tag}`,
  port: 9838,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-handoff-says-what-it-carries-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: { [missionId]: TEAMMATE.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
// Types into the box as a person would, then reads the line above it once the preview has had time.
const typeAndRead = (text) => `(async () => {
  const box = document.querySelector('form.command-dock textarea')
  if (!box) return 'no box'
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(box, ${JSON.stringify(text)})
  box.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 1500))
  return document.querySelector('.lc-continuation')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no note'
})()`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM vendor/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
  })()`)
  await sleep(500)
  const moved = String(await drive.evaluate(pickRouteScript({ group: '/codex/i' })))
  say(`   the box moved: ${moved}`)
  check('the box can be moved to Codex', /Codex/.test(moved), moved)
  const short = String(await drive.capture('a short reply, typed', () => drive.evaluate(typeAndRead('Now make it a table'))))
  check('the note says where it goes, in plain words', /Your next message goes to Codex CLI with a summary of this conversation, not OpenCode's memory of it\./.test(short), short)
  check('and what it carries: the task and its last reply', /It carries the task and its last reply\./.test(short) && !/Left out/.test(short), short)
  const long = String(await drive.capture('a long reply, typed', () => drive.evaluate(typeAndRead(`Now redo it with these requirements. ${'Every seat needs SSO, an audit log and an EU data region. '.repeat(120)}`))))
  check('a long reply says what is left out to fit', /Left out to fit: its last reply\./.test(long), long)
  const cleared = String(await drive.capture('the box emptied', () => drive.evaluate(typeAndRead(''))))
  check('emptied, it carries everything again', /It carries the task and its last reply\./.test(cleared), cleared)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A finished OpenCode conversation, its teammate now on Codex; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
