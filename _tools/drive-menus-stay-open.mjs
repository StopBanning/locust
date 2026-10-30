// Do menus stay open while the thread scrolls itself (H11)?
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

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `menus-stay-open-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const adapters = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-menus-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-menus-profile-'))
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
  name: 'menus-stay-open',
  port: 9561,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
// The thread scrolls itself while it follows a reply; a scroll event on it is
// what that does. The sidebar list scrolling is the case that must still close.
const scrollOf = (selector) => `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return 'no ' + ${JSON.stringify(selector)}; el.dispatchEvent(new Event('scroll')); return 'scrolled' })()`
try {
  await drive.capture('launch: 22 missions in the ledger', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(2000)
  await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /OLDEST QUESTION/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
    row?.click()
    await new Promise((r) => setTimeout(r, 2000))
  })()`)
  const threadSel = String(await drive.evaluate(`document.querySelector('.lc-thread') ? '.lc-thread' : 'none'`))
  check('a thread is on screen', threadSel === '.lc-thread')

  // A right-click menu on another sidebar row.
  const opened = String(await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /Question number 21/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
    if (!row) return 'no row'
    const box = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 10, clientY: box.top + 5 }))
    await new Promise((r) => setTimeout(r, 400))
    return document.querySelector('.lc-context') ? 'open' : 'not open'
  })()`))
  check('the right-click menu opens', opened === 'open', opened)
  await drive.evaluate(scrollOf('.lc-thread'))
  await sleep(300)
  const afterThread = String(await drive.evaluate(`document.querySelector('.lc-context') ? 'open' : 'closed'`))
  await drive.capture('the thread scrolled itself: the menu', () => drive.evaluate(`document.querySelector('.lc-context')?.innerText.replace(/\\s+/g, ' ') ?? 'closed'`))
  check('the thread scrolling itself leaves the menu open', afterThread === 'open', afterThread)
  const listSel = String(await drive.evaluate(`(() => { const row = [...document.querySelectorAll('li, button')].find((el) => /Question number 21/.test(el.textContent ?? '')); let el = row?.parentElement; while (el && getComputedStyle(el).overflowY !== 'auto' && getComputedStyle(el).overflowY !== 'scroll') el = el.parentElement; if (!el) return 'none'; el.setAttribute('data-drive-list', '1'); return '[data-drive-list]' })()`))
  await drive.evaluate(scrollOf(listSel))
  await sleep(300)
  const afterList = String(await drive.evaluate(`document.querySelector('.lc-context') ? 'open' : 'closed'`))
  check('scrolling the list the menu was opened from still closes it', afterList === 'closed', `${listSel}: ${afterList}`)

  // The composer's mode menu, open while the thread scrolls itself.
  const mode = String(await drive.evaluate(`(async () => {
    const button = [...document.querySelectorAll('form.command-dock button')].find((b) => b.getAttribute('aria-haspopup') === 'menu' && /Edit|Ask|Auto|Plan|Approve/.test(b.textContent ?? ''))
    if (!button) return 'no mode button'
    button.click()
    await new Promise((r) => setTimeout(r, 400))
    return button.getAttribute('aria-expanded') === 'true' ? 'open' : 'not open'
  })()`))
  check('the composer mode menu opens', mode === 'open', mode)
  await drive.evaluate(scrollOf('.lc-thread'))
  await sleep(300)
  const modeAfter = String(await drive.evaluate(`[...document.querySelectorAll('form.command-dock button')].find((b) => b.getAttribute('aria-haspopup') === 'menu' && /Edit|Ask|Auto|Plan|Approve/.test(b.textContent ?? ''))?.getAttribute('aria-expanded') ?? 'none'`))
  check('and stays open while the thread scrolls itself', modeAfter === 'true', modeAfter)
  say(failures === 0 ? '\nMENUS STAY OPEN PASSED' : `\nMENUS STAY OPEN: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. A right-click menu and the composer's mode menu, open while the thread scrolls.` })
}
