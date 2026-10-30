// Each chat keeps its own model, mode, effort and chat mode (0.552).
//
//   node _tools/drive-models-stay-put.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-02: "everytime i switch to a model and go to a new chat it
// never saves to that model or that teammate or if i have a teammate set to a
// certain model i should be able to run a sepate chat with a new model
// without assigning that as their new model" -- and: "each time i switch to a
// new chat it just keeps the mode from the previous chat, there should be
// persistence in each chat, for permissions, model, chat type, and effort".
//
// Seeded: Ash (free OpenCode, Ask); conversation A is Ash's, ran on Claude
// Sonnet in Ask; conversation B is nobody's, ran on OpenCode in Edit.
// Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, FREE_ROW, pickRouteScript, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

// B ran on FREE_ROUTE.model (the RUNS table below), which is whichever free model LOCUST_FREE_MODEL names (drive-lib.mjs:
// "LOCUST_FREE_MODEL picks another of OpenCode's free models when this one is down"). The check named Muse Spark, so the
// 2026-10-06 sweep -- on Fledge Alpha -- read a correct chat box ("OpenCode / Fledge Alpha Free") as another chat's model.
// Its name as the picker draws it, from the same words the drive's route picker uses (FREE_ROW).
const FREE_NAME = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i')

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-stay-put-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-stay-put-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const RUNS = [
  { n: 1, prompt: 'Ash reads the plan', runtime: 'claude', model: 'claude-sonnet-5', mode: 'ask', sandbox: 'read-only' },
  { n: 2, prompt: 'List the TODOs', runtime: 'opencode', model: FREE_ROUTE.model, mode: 'accept-edits', sandbox: 'workspace-write' }
]
for (const run of RUNS) {
  const missionId = `mission_5f000000-0000-4000-8000-00010000000${String(run.n)}`
  const runId = `run_5f000${String(run.n)}`
  const at = new Date(Date.now() - 3_600_000 + run.n * 60_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: run.prompt, runtime: run.runtime, model: run.model, mode: run.mode, requestedRouteId: run.runtime,
    resolvedRouteId: `${run.runtime}-account:default`, cliVersion: null, workspaceId, sandbox: run.sandbox, executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(run.n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: run.runtime, type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'Done.', final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 90, outputTokens: 4 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
}
await ledger.flush()

const ASH_ROUTE = { ...FREE_ROUTE, mode: 'ask' }
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `models-stay-put-${tag}`,
  port: 9871,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('models-stay-put-2026-10-02'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_ash', name: 'Ash', hue: 'clay', role: 'Custom', roleTitle: 'Builder', createdAt: '2026-09-05T05:00:00.000Z', route: ASH_ROUTE }],
    missionOwners: { 'mission_5f000000-0000-4000-8000-000100000001': 'tm_ash' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
// What the chat box says right now.
const BOX = `(() => {
  const route = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  return JSON.stringify({
    route: route?.innerText.replace(/\\s+/g, ' ').trim() ?? null,
    mode: document.querySelector('button[aria-label="Permission mode"]')?.innerText.replace(/\\s+/g, ' ').trim() ?? null,
    chat: document.querySelector('.lc-control--chatmode')?.getAttribute('aria-label') ?? null
  })
})()`
const box = async (what) => JSON.parse(String(await drive.capture(what, () => drive.evaluate(BOX))))
const open = (word) => drive.evaluate(`(async () => {
  const row = [...document.querySelectorAll('.lc-conv')].find((one) => one.title.startsWith(${JSON.stringify(word)}))
  row?.click()
  await new Promise((r) => setTimeout(r, 1500))
  return row ? 'opened' : 'no row'
})()`)
const home = () => drive.evaluate(`(async () => { document.querySelector('button[aria-label="Home"]')?.click(); await new Promise((r) => setTimeout(r, 1200)); return 'home' })()`)
const newWithAsh = () => drive.evaluate(`(async () => {
  const face = [...document.querySelectorAll('.lc-faces__one')].find((one) => (one.getAttribute('aria-label') ?? '').includes('Ash'))
  face?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }))
  let item
  for (let i = 0; i < 20 && !item; i += 1) {
    await new Promise((r) => setTimeout(r, 200))
    item = [...document.querySelectorAll('button')].find((el) => el.innerText.trim() === 'New conversation with Ash')
  }
  item?.click()
  await new Promise((r) => setTimeout(r, 1200))
  return item ? 'new with Ash' : 'no flyout item'
})()`)
const pickMode = (name) => drive.evaluate(`(async () => {
  document.querySelector('button[aria-label="Permission mode"]')?.click()
  await new Promise((r) => setTimeout(r, 400))
  const item = [...document.querySelectorAll('.lc-menu[aria-label="Permission mode"] .lc-menu__item')].find((el) => el.innerText.trim().startsWith(${JSON.stringify(name)}))
  item?.click()
  await new Promise((r) => setTimeout(r, 600))
  return item ? 'picked' : 'no item: ' + [...document.querySelectorAll('.lc-menu[aria-label="Permission mode"] .lc-menu__item')].map((el) => el.innerText.split('\\n')[0]).join(' | ')
})()`)
const pickChatMode = (name) => drive.evaluate(`(async () => {
  document.querySelector('.lc-control--chatmode')?.click()
  await new Promise((r) => setTimeout(r, 400))
  const item = [...document.querySelectorAll('.lc-menu__item')].find((el) => el.innerText.trim().startsWith(${JSON.stringify(name)}))
  item?.click()
  await new Promise((r) => setTimeout(r, 600))
  return item ? 'picked' : 'no item'
})()`)
const roster = async () => JSON.parse(await readFile(join(profilePath, 'teammates.json'), 'utf8').catch(() => '{}'))

try {
  await drive.ready()
  await drive.resize(1209, 770)
  await drive.evaluate(`(async () => { for (let i = 0; i < 40 && document.querySelectorAll('.lc-conv').length < 2; i += 1) await new Promise((r) => setTimeout(r, 250)) })()`)
  // Until discovery has found the runtimes, the route chip says "No AI agent".
  await drive.evaluate(`(async () => {
    const chip = () => [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')?.innerText ?? ''
    for (let i = 0; i < 120 && (chip() === '' || /no ai agent/i.test(chip())); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1500))
  })()`)

  // 1. A conversation shows what it ran on, not its teammate's model.
  say(String(await open('Ash reads')))
  const a1 = await box('A opened')
  check('A shows the model it ran on (Sonnet), not Ash\'s', /sonnet/i.test(a1.route ?? ''), JSON.stringify(a1))
  check('A shows the mode it ran in (Ask)', /ask/i.test(a1.mode ?? ''), JSON.stringify(a1))

  // 2. A mode picked in A stays with A.
  say(String(await pickMode('Plan')))
  const a2 = await box('A, Plan picked')
  check('Plan picked in A', /plan/i.test(a2.mode ?? ''), JSON.stringify(a2))
  say(String(await open('List the')))
  const b1 = await box('B opened')
  check('B does not take A\'s Plan', !/plan/i.test(b1.mode ?? ''), JSON.stringify(b1))
  check(`B shows its own model (OpenCode ${FREE_ROUTE.model.replace(/^opencode\//, '')}), not A's Sonnet`, FREE_NAME.test(b1.route ?? '') && /opencode/i.test(b1.route ?? '') && !/sonnet/i.test(b1.route ?? ''), JSON.stringify(b1))
  say(String(await open('Ash reads')))
  const a3 = await box('A again')
  check('back in A, Plan is still picked', /plan/i.test(a3.mode ?? ''), JSON.stringify(a3))
  check('back in A, still Sonnet', /sonnet/i.test(a3.route ?? ''), JSON.stringify(a3))

  // 3. A new chat with Ash starts on Ash's model and mode, not A's.
  say(String(await newWithAsh()))
  const n1 = await box('new chat with Ash')
  check('a new chat with Ash starts on Ash\'s model', !/sonnet/i.test(n1.route ?? '') && n1.route !== null, JSON.stringify(n1))
  check('a new chat with Ash starts in Ash\'s mode (Ask)', /ask/i.test(n1.mode ?? ''), JSON.stringify(n1))

  // 4. The chat mode stays with its chat.
  say(String(await home()))
  say(String(await pickChatMode('Compare')))
  const h1 = await box('home, Compare on')
  check('Compare on for the new chat', /compare/i.test(h1.chat ?? ''), JSON.stringify(h1))
  say(String(await open('Ash reads')))
  const a4 = await box('A, after Compare at home')
  check('A stays Direct', /direct/i.test(a4.chat ?? ''), JSON.stringify(a4))
  say(String(await home()))
  const h2 = await box('home again')
  check('the new chat is still on Compare', /compare/i.test(h2.chat ?? ''), JSON.stringify(h2))
  say(String(await pickChatMode('Direct')))

  // 5. A model picked for a new chat with nobody is kept, and Ash is untouched.
  const picked = String(await drive.evaluate(pickRouteScript({ group: '/opencode/i', search: 'nemotron', row: '/nemotron/i' })))
  say(picked)
  // The roster is written behind the pick: read it until the pick is in, not once after a fixed 1.5 s (on the sweep's
  // busy machine the write landed later than that and a correct pick read as unsaved).
  let saved
  for (let waited = 0; waited < 15_000; waited += 250) {
    await new Promise((r) => setTimeout(r, 250))
    saved = (await roster()).settings?.newChatRoute
    if (saved !== undefined && /nemotron/i.test(saved.model ?? '')) break
  }
  check('the new chat\'s model is saved for the next launch', saved !== undefined && /nemotron/i.test(saved.model ?? ''), JSON.stringify(saved))
  say(String(await open('Ash reads')))
  say(String(await home()))
  const h3 = await box('home after a conversation')
  check('a new chat with nobody is still on that model', /nemotron/i.test(h3.route ?? ''), JSON.stringify(h3))
  const ash = (await roster()).teammates?.find((one) => one.teammateId === 'tm_ash')?.route
  check('Ash\'s saved model is unchanged', ash?.model === ASH_ROUTE.model && ash?.mode === 'ask', JSON.stringify(ash))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Ash plus two seeded conversations; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
