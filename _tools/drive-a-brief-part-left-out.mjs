// A part of a handoff brief can be left out, and is (0.527).
//
//   LOCUST_SPEND=1 node _tools/drive-a-brief-part-left-out.mjs [--packaged <exe>] [--tag <name>]
//
// Product ideas, round four: "with a way to drop a section". A seeded
// conversation finished on OpenCode, its last reply recommending HubSpot; its
// teammate is now on Codex (GPT-6-Luna, low effort: Codex quota, which may be
// spent), so the next message is a handoff. Under the box, "its last reply"
// carries a ×; pressed, the line says it is being left out, Put back undoes
// it. Left out and sent, the new run must not know the recommendation, the
// divider says "You left out its last reply.", and it still does after a
// reload, from the record.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-brief-left-out-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-brief-left-out-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// Now on Codex: the conversation below was on OpenCode, so the next message switches.
const TEAMMATE = { teammateId: 'tm_wren00000000000000000', name: 'Wren', hue: 'lime', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'gpt-6-luna', mode: 'ask', effort: 'low' } }
const missionId = 'mission_5e000000-0000-4000-8000-000700000000'
const runId = 'run_5e0007'
const at = new Date(Date.now() - 600_000).toISOString()
const reply = 'I compared the three vendor quotes against your five requirements. HubSpot Starter is the cheapest that meets all five, so I recommend HubSpot.'
await ledger.createMission({
  missionId, runId, prompt: 'Compare the three CRM vendor quotes and recommend one',
  runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: reply, final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 120 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `brief-left-out-${tag}`,
  port: 9849,
  workspace,
  profilePath,
  spends: true,
  outPath: join(recordRoot('a-brief-part-left-out-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: { [missionId]: TEAMMATE.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const NOTE = `document.querySelector('.lc-continuation')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no note'`
const openConversation = `(async () => {
  ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM vendor/i.test(r.innerText))?.click()
  await new Promise((r) => setTimeout(r, 1500))
  return 1
})()`
const QUESTION = 'Do not do any work and do not read any file. Reply with one line only: the name of the vendor the previous agent recommended, or NOT TOLD if what you were given does not say.'

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1209, 770)
  await drive.evaluate(openConversation)
  const before = String(await drive.capture('a reply typed: the brief, each part that may go with a ×', () => drive.evaluate(`(async () => {
    const box = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(box, 'Which one?')
    box.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 1500))
    const parts = [...document.querySelectorAll('.lc-continuation__part')].map((b) => b.getAttribute('aria-label'))
    return JSON.stringify({ note: ${NOTE}, parts })
  })()`)))
  const shown = JSON.parse(before)
  check('it carries the task and its last reply, and only the reply may be left out', /It carries the task and its last reply\./.test(shown.note) && JSON.stringify(shown.parts) === '["Leave out its last reply"]', before)
  const dropped = String(await drive.capture('its last reply, left out', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-continuation__part[aria-label="Leave out its last reply"]')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return ${NOTE}
  })()`)))
  check('pressed, the line says it is being left out, with Put back', /It carries the task\. You are leaving out its last reply\. Put back/.test(dropped), dropped)
  const back = String(await drive.capture('Put back', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-continuation__putback')].find((b) => b.innerText.trim() === 'Put back')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return ${NOTE}
  })()`)))
  check('Put back carries it again', /It carries the task and its last reply\./.test(back) && !/leaving out/.test(back), back)
  await drive.evaluate(`(async () => {
    document.querySelector('.lc-continuation__part[aria-label="Leave out its last reply"]')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return 1
  })()`)
  const sent = String(await drive.capture('left out and sent: the answer', () => drive.evaluate(sendAndWaitScript(QUESTION, { waitSeconds: 300 }))))
  // What came back after the question (the seeded reply naming HubSpot is above it).
  const asked = sent.lastIndexOf('does not say.')
  const answer = asked < 0 ? '' : sent.slice(asked + 'does not say.'.length)
  check('the new run was not told the recommendation', asked >= 0 && answer.trim().length > 0 && !/hubspot/i.test(answer), sent)
  const divider = String(await drive.capture('the divider', () => drive.evaluate(`[...document.querySelectorAll('.lc-handoff')].map((d) => d.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')`)))
  check('the divider says the person left it out', /You left out its last reply\./.test(divider), divider)
  await drive.evaluate('location.reload()').catch(() => undefined)
  await sleep(4000)
  await drive.ready().catch(() => undefined)
  await drive.evaluate(openConversation)
  const reopened = String(await drive.capture('reloaded: the divider, from the record', () => drive.evaluate(`[...document.querySelectorAll('.lc-handoff')].map((d) => d.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')`)))
  check('reloaded, the divider still says it, from the record', /You left out its last reply\./.test(reopened), reopened)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A finished OpenCode conversation, its teammate now on Codex (GPT-6-Luna, low); one reply sent with its last reply left out.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
