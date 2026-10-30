// A conversation handed to another teammate takes the message box with it (0.567).
//
//   node _tools/drive-an-assigned-conversation-takes-the-box.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-03: a W7 conversation on Antigravity hit its quota; he used
// Assign to -> Codex on it and replied, and "it started a whole new chat". His
// ledger had it: a new conversation for the OLD teammate, 15 seconds old when
// he deleted it, then a second send that did continue. Opening a conversation
// picks its owner in the box; assigning the one on screen did not, so the
// reply went to someone who no longer owned it.
//
// Seeded: a finished OpenCode conversation of Clay's. Assign it to Wren and
// type a reply: the box must now send to Wren (the pressed face), and must not
// announce a new conversation. The route stays the conversation's own (0.552).
// Nothing is sent.

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
const workspace = await scratchRepository('locust-drive-assigned-box-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-assigned-box-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const CLAY = { teammateId: 'tm_clay0000000000000000', name: 'Clay', hue: 'clay', role: 'Custom', roleTitle: 'Chief of Staff', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', mode: 'ask' } }
const WREN = { teammateId: 'tm_wren0000000000000000', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:01:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }
const missionId = 'mission_5e000000-0000-4000-8000-000500000000'
const runId = 'run_5e0005'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'Draft the routine inputs plan for W7',
  runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'The plan has three parts: the inputs a routine asks for, the file it travels as, and the dialog that fills it.', final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 200 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `assigned-box-${tag}`,
  port: 9887,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('an-assigned-conversation-takes-the-box-2026-10-03'), tag),
  seed: { schemaVersion: 1, teammates: [CLAY, WREN], missionOwners: { [missionId]: CLAY.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const typeAndRead = (text) => `(async () => {
  const box = document.querySelector('form.command-dock textarea')
  if (!box) return 'no box'
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(box, ${JSON.stringify(text)})
  box.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 1500))
  // Who the box sends to is the face the sidebar has pressed (selectedTeammateId).
  const to = (document.querySelector('.lc-faces__one[aria-pressed="true"]')?.getAttribute('aria-label') ?? 'nobody').split(' — ')[0]
  return 'to ' + to + ' | ' + (document.querySelector('.lc-continuation')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no note')
})()`
const row = `[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /routine inputs plan/i.test(r.innerText))`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await drive.evaluate(`(async () => { ${row}?.click(); await new Promise((r) => setTimeout(r, 1500)) })()`)
  await sleep(500)
  const before = String(await drive.capture('Clay\'s conversation open, a reply typed', () => drive.evaluate(typeAndRead('keep working'))))
  check('before: the box sends to Clay, whose conversation it is, and says nothing more', before === 'to Clay | no note', before)

  const menu = String(await drive.capture('right-click it, Assign to, Wren', () => drive.evaluate(`(async () => {
    const r = ${row}
    if (!r) return 'no row'
    const box = r.getBoundingClientRect()
    r.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: Math.round(box.left + 20), clientY: Math.round(box.top + 10) }))
    await new Promise((res) => setTimeout(res, 600))
    const assign = [...document.querySelectorAll('.lc-context__item')].find((b) => /^Assign to/.test(b.innerText.trim()))
    if (!assign) return 'NO ASSIGN ROW: ' + [...document.querySelectorAll('.lc-context__item')].map((b) => b.innerText.trim()).join(' | ')
    assign.click()
    assign.closest('.lc-context__row')?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }))
    await new Promise((res) => setTimeout(res, 500))
    const wren = [...document.querySelectorAll('.lc-context__sub .lc-context__item')].find((b) => /^Wren/.test(b.innerText.trim()))
    if (!wren) return 'NO WREN IN THE SUBMENU'
    wren.click()
    await new Promise((res) => setTimeout(res, 1500))
    return 'assigned'
  })()`)))
  check('the conversation was handed to Wren', menu === 'assigned', menu)

  const after = String(await drive.capture('the same reply, after the hand-over', () => drive.evaluate(typeAndRead('keep working'))))
  check('after: the box sends to Wren, who has the conversation now', /^to Wren \|/.test(after), after)
  check('and the reply continues it: no new conversation is announced', !/starts a new conversation/.test(after), after)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A finished OpenCode conversation of Clay's, assigned to Wren (Codex); nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
