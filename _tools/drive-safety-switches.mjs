// Do the safety switches hold (M33, M34)? Shift+Tab never turns Auto on;
// a double-click never confirms a delete.
//
//   node _tools/drive-old-conversation.mjs [--packaged <exe>] [--tag <name>]
//
// History sends the newest twenty missions whole and every other one as a
// row, and nothing fetched the rest: an older conversation opened on the
// person's words alone. The ledger is seeded, with the mission store's own
// writer, with 22 finished missions; the oldest is a two-turn conversation.
// It is opened from the sidebar, and both of its replies must be in the
// thread. Sends nothing.

import { mkdir, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { readdir, readFile } from 'node:fs/promises'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `safety-switches-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const adapters = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-safety-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-safety-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })

// 22 missions, oldest first; the first two are one conversation.
const base = Date.parse('2026-09-20T09:00:00.000Z')
for (let n = 0; n < 22; n += 1) {
  const missionId = `mission_seed_${String(n).padStart(2, '0')}`
  const runId = `run_seed_${String(n).padStart(2, '0')}`
  const at = new Date(base + n * 60_000).toISOString()
  await ledger.createMission({
    missionId, runId,
    prompt: n === 0 ? 'OLDEST QUESTION: what does the build do?' : n === 1 ? 'OLDEST FOLLOW-UP: and the tests?' : `Question number ${String(n)}.`,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at,
    ...(n === 1 ? { continuesFrom: { missionId: 'mission_seed_00', checkpointEpoch: 1, reason: 'follow-up' } } : {})
  })
  const reply = n === 0 ? 'OLDEST REPLY: it compiles the app.' : n === 1 ? 'OLDEST FOLLOW-UP REPLY: they run with pnpm test.' : `Reply number ${String(n)}.`
  // Real events, made the way a run makes them: Codex's own records through its normalizer.
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
  await ledger.appendEvents(missionId, [
    ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
    ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: reply } }) }),
    ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })
  ])
}
say(`seeded 22 missions in ${workspaceId}`)

const drive = await startDrive({
  name: 'safety-switches',
  port: 9569,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const mouse = (type, x, y, clickCount) => drive.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mousePressed' ? 1 : 0, clickCount })
const click = async (x, y, clickCount = 1) => { await mouse('mousePressed', x, y, clickCount); await mouse('mouseReleased', x, y, clickCount) }
const savedAutoMode = async () => {
  for (const name of await readdir(profilePath)) {
    if (!name.endsWith('.json')) continue
    try {
      const parsed = JSON.parse(await readFile(join(profilePath, name), 'utf8'))
      if (parsed?.settings !== undefined) return parsed.settings.autoMode === true
    } catch {
      // Not the file.
    }
  }
  return undefined
}
try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(2000)

  // M33: Shift+Tab round the modes, from the composer.
  const modes = String(await drive.capture('Shift+Tab through every mode', () => drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    field?.focus()
    const seen = []
    const mode = () => [...document.querySelectorAll('form.command-dock button')].find((b) => b.getAttribute('aria-haspopup') === 'menu' && /Edit|Ask|Auto|Plan|Approve/.test(b.textContent ?? ''))?.textContent?.trim()
    for (let i = 0; i < 7; i += 1) {
      field?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }))
      await new Promise((r) => setTimeout(r, 300))
      seen.push(mode())
    }
    return seen.join(' > ')
  })()`)))
  say(`  modes: ${modes}`)
  await sleep(800)
  const auto = await savedAutoMode()
  check('Shift+Tab never turned the Auto switch on', auto === false, `autoMode saved: ${String(auto)}`)
  check('and never landed on Auto', !/Auto/.test(modes), modes)

  // M34: a double-click on Delete only arms it.
  const target = async () => JSON.parse(String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /Question number 21/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
    const box = row?.getBoundingClientRect()
    return JSON.stringify(box === undefined ? null : { x: box.left + 20, y: box.top + box.height / 2 })
  })()`)))
  const row = await target()
  check('the conversation is listed', row !== null)
  await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /Question number 21/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
    const box = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 20, clientY: box.top + 5 }))
  })()`)
  await sleep(500)
  const item = JSON.parse(String(await drive.evaluate(`(() => {
    const el = [...document.querySelectorAll('.lc-context__item')].find((b) => /^Delete/.test(b.textContent.trim()))
    const box = el?.getBoundingClientRect()
    return JSON.stringify(box === undefined ? null : { x: box.left + box.width / 2, y: box.top + box.height / 2 })
  })()`)))
  check('the menu offers Delete', item !== null)
  await click(item.x, item.y, 1)
  await sleep(40)
  await click(item.x, item.y, 2)
  await sleep(1200)
  const afterDouble = await target()
  await drive.capture('a double-click on Delete', () => drive.evaluate(`document.querySelector('.lc-context')?.innerText.replace(/\\s+/g, ' ') ?? 'menu closed'`))
  check('a double-click did not delete it', afterDouble !== null)
  const stillArmed = String(await drive.evaluate(`[...document.querySelectorAll('.lc-context__item')].map((b) => b.textContent.trim()).find((t) => /for good\\?/.test(t)) ?? 'not armed'`))
  check('it only armed it', /for good\?/.test(stillArmed), stillArmed)
  const armedItem = JSON.parse(String(await drive.evaluate(`(() => {
    const el = [...document.querySelectorAll('.lc-context__item')].find((b) => /for good\\?/.test(b.textContent))
    const box = el?.getBoundingClientRect()
    return JSON.stringify(box === undefined ? null : { x: box.left + box.width / 2, y: box.top + box.height / 2 })
  })()`)))
  if (armedItem !== null) {
    await sleep(600)
    await click(armedItem.x, armedItem.y, 1)
    await sleep(1500)
  }
  check('a separate click then deletes it', (await target()) === null)
  say(failures === 0 ? '\nSAFETY SWITCHES PASSED' : `\nSAFETY SWITCHES: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Shift+Tab round the modes with Auto off; a real double-click on a conversation's Delete.` })
}
