// A long conversation has a rail of your own messages (prompt rail).
//
//   node _tools/drive-prompt-rail.mjs
//
// Six finished turns, seeded, nothing sent. The rail shows, a mark's hover is
// that message's first line, and a click puts the message at the top of the
// thread. Frames at 1440x900 and 1205x803, both themes, and one with the
// browser open. No model runs.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-rail-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-rail-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })

const PROMPTS = [
  'Where does the prompt rail sit?',
  'How far apart are the marks?',
  'Which mark is the current one?',
  'What does a hover say?',
  'Where does a click land?',
  'Does the rail stay off the words?'
]
const replyFor = (prompt) => Array.from({ length: 12 }, (_, index) => (
  `${prompt} Paragraph ${String(index + 1)}. The reply is long so the thread has somewhere to scroll, and the marks have room to spread.`
)).join('\n\n')

const base = Date.parse('2026-10-19T12:00:00.000Z')
for (let n = 0; n < PROMPTS.length; n += 1) {
  const missionId = `mission_rail_${String(n).padStart(2, '0')}`
  const runId = `run_rail_${String(n).padStart(2, '0')}`
  const at = new Date(base + n * 60_000).toISOString()
  const prompt = PROMPTS[n]
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at,
    ...(n === 0 ? {} : { continuesFrom: { missionId: `mission_rail_${String(n - 1).padStart(2, '0')}`, checkpointEpoch: 1, reason: 'follow-up' } })
  })
  const normalizer = adapters.createCodexEventNormalizer({
    runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at)
  })
  await ledger.appendEvents(missionId, [
    ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
    ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: replyFor(prompt) } }) }),
    ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
    ...normalizer.finish({
      exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false,
      forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false,
      oversizedRecordsDropped: 0, startedAt: at, finishedAt: at
    })
  ])
}
say(`seeded ${String(PROMPTS.length)} turns in ${workspaceId}`)

const drive = await startDrive({
  name: 'prompt-rail',
  port: 9637,
  workspace,
  profilePath,
  sendsNothing: true,
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}

const READ = `(() => {
  const thread = document.querySelector('.lc-thread')
  const column = document.querySelector('.lc-thread__column')
  const rail = document.querySelector('.lc-promptrail')
  const marks = [...document.querySelectorAll('.lc-promptrail__mark')]
  const tip = document.querySelector('.lc-promptrail__tip')
  if (thread === null || column === null) return JSON.stringify({ found: false })
  const bar = thread.offsetWidth - thread.clientWidth
  const pad = Number.parseFloat(getComputedStyle(thread).paddingRight) || 0
  const threadBox = thread.getBoundingClientRect()
  const columnBox = column.getBoundingClientRect()
  const room = threadBox.right - bar - pad - columnBox.right
  const mark = marks[0]?.getBoundingClientRect()
  const current = marks.findIndex((one) => one.getAttribute('aria-current') === 'true')
  return JSON.stringify({
    found: true,
    rail: rail !== null,
    marks: marks.length,
    lines: marks.map((one) => one.getAttribute('aria-label')),
    current,
    room: Math.round(room),
    scrollable: thread.scrollHeight > thread.clientHeight + 1,
    overlap: mark === undefined ? null : Math.round(columnBox.right - mark.left),
    center: Math.round(Math.abs((columnBox.left + columnBox.right) / 2 - (threadBox.left + threadBox.right) / 2)),
    tip: tip === null ? '' : (tip.innerText ?? '').replace(/\\s+/g, ' ').trim(),
    theme: document.documentElement.dataset.theme ?? '',
    browser: document.querySelector('.lc-browser:not([hidden])') !== null
  })
})()`

const OPEN = `(async () => {
  for (let i = 0; i < 50; i += 1) {
    const row = [...document.querySelectorAll('.lc-conv')].find((el) => /prompt rail|far apart|hover say|click land|off the words|current one/i.test(el.innerText ?? ''))
    if (row) {
      row.click()
      return 'clicked ' + (row.innerText ?? '').replace(/\\s+/g, ' ').trim().slice(0, 80)
    }
    await new Promise((r) => setTimeout(r, 200))
  }
  const rows = [...document.querySelectorAll('.lc-conv')].map((el) => (el.innerText ?? '').replace(/\\s+/g, ' ').trim().slice(0, 60))
  return 'no row: ' + rows.join(' | ')
})()`

const HOVER = `(async () => {
  const marks = [...document.querySelectorAll('.lc-promptrail__mark')]
  const mark = marks[1]
  if (mark === undefined) return 'no mark'
  mark.focus({ preventScroll: true })
  for (let i = 0; i < 20; i += 1) {
    const tip = document.querySelector('.lc-promptrail__tip')
    if (tip !== null && /How far apart are the marks\\?/.test(tip.innerText ?? '') && /Ctrl\\+Alt\\+Up/.test(tip.innerText ?? '')) {
      return (tip.innerText ?? '').replace(/\\s+/g, ' ').trim()
    }
    await new Promise((r) => setTimeout(r, 50))
  }
  return document.querySelector('.lc-promptrail__tip')?.innerText ?? 'no tip'
})()`

const JUMP = `(async () => {
  const marks = [...document.querySelectorAll('.lc-promptrail__mark')]
  const mark = marks[1]
  const thread = document.querySelector('.lc-thread')
  const bubble = document.querySelector('[data-prompt-mark="How far apart are the marks?"]')
  if (mark === undefined || thread === null || bubble === null) return JSON.stringify({ ok: false, why: 'missing' })
  mark.click()
  let gap = 999
  for (let i = 0; i < 40; i += 1) {
    gap = Math.round(bubble.getBoundingClientRect().top - thread.getBoundingClientRect().top)
    if (Math.abs(gap) <= 8) break
    await new Promise((r) => setTimeout(r, 50))
  }
  return JSON.stringify({
    ok: Math.abs(gap) <= 8,
    gap,
    hit: bubble.classList.contains('is-railhit'),
    current: [...document.querySelectorAll('.lc-promptrail__mark')].findIndex((one) => one.getAttribute('aria-current') === 'true')
  })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const opened = String(await drive.capture('dark 1440, open the conversation', () => drive.evaluate(OPEN)))
  check('the conversation opened', opened.startsWith('clicked'), opened)
  await drive.waitFor(`document.querySelectorAll('.lc-promptrail__mark').length >= 6`, { what: 'the prompt rail', timeoutMs: 15_000 }).catch(() => undefined)
  await sleep(400)
  const wide = JSON.parse(String(await drive.capture('dark 1440, the rail', () => drive.evaluate(READ))))
  check('six marks, and the thread can scroll', wide.rail === true && wide.marks === 6 && wide.scrollable === true, JSON.stringify(wide))
  check('the marks are your messages, in order', JSON.stringify(wide.lines) === JSON.stringify(PROMPTS), JSON.stringify(wide.lines))
  check('one mark is current', wide.current >= 0, String(wide.current))
  check('the strip does not cover the words', wide.overlap !== null && wide.overlap <= 2, `overlap ${String(wide.overlap)} room ${String(wide.room)}`)
  check('the column stays centred', wide.center <= 12, String(wide.center))

  const tip = String(await drive.capture('dark 1440, hover a mark', () => drive.evaluate(HOVER)))
  check('the hover is that message, and names the shortcut', /How far apart are the marks\?/.test(tip) && /Ctrl\+Alt\+Up and Ctrl\+Alt\+Down/.test(tip), tip)

  const jumped = JSON.parse(String(await drive.capture('dark 1440, the message at the top', () => drive.evaluate(JUMP))))
  check('the clicked message lands at the top', jumped.ok === true, JSON.stringify(jumped))
  check('the line is marked, the way Find marks a match', jumped.hit === true, JSON.stringify(jumped))

  await drive.evaluate(`window.desktop.setTheme('light')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'light'`, { what: 'light theme' })
  const lightWide = JSON.parse(String(await drive.capture('light 1440, the rail', () => drive.evaluate(READ))))
  check('light, still six marks, still off the words', lightWide.theme === 'light' && lightWide.marks === 6 && lightWide.overlap <= 2, JSON.stringify(lightWide))

  await drive.evaluate(`window.desktop.setTheme('dark')`)
  await drive.resize(1205, 803)
  await drive.waitFor(`document.documentElement.dataset.theme === 'dark'`, { what: 'dark theme' })
  await sleep(300)
  const narrow = JSON.parse(String(await drive.capture('dark 1205, the rail', () => drive.evaluate(READ))))
  check('1205 still has the rail, off the words', narrow.rail === true && narrow.marks === 6 && narrow.overlap <= 2, JSON.stringify(narrow))
  check('1205 still scrolls', narrow.scrollable === true, String(narrow.scrollable))

  await drive.evaluate(`window.desktop.setTheme('light')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'light'`, { what: 'light theme again' })
  const lightNarrow = JSON.parse(String(await drive.capture('light 1205, the rail', () => drive.evaluate(READ))))
  check('light 1205 still has the rail, off the words', lightNarrow.theme === 'light' && lightNarrow.marks === 6 && lightNarrow.overlap <= 2, JSON.stringify(lightNarrow))

  await drive.evaluate(`window.desktop.setTheme('dark')`)
  await drive.resize(1440, 900)
  await drive.waitFor(`document.documentElement.dataset.theme === 'dark'`, { what: 'dark before the browser' })
  const browser = JSON.parse(String(await drive.capture('dark 1440, browser open', () => drive.evaluate(`(async () => {
    document.querySelector('button[aria-label="Browser"]')?.click()
    for (let i = 0; i < 30; i += 1) {
      if (document.querySelector('.lc-browser:not([hidden])') !== null) break
      await new Promise((r) => setTimeout(r, 100))
    }
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
    await new Promise((r) => setTimeout(r, 100))
    return ${READ}
  })()`))))
  check('the browser panel is open beside the thread', browser.browser === true, JSON.stringify(browser))
  if (browser.rail === true) {
    check('with the browser open the strip still stays off the words', browser.overlap <= 2, JSON.stringify(browser))
  } else {
    check('with the browser open the thread is too narrow for the strip, so it is hidden', browser.room < 18, JSON.stringify(browser))
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: 'Build: out/. Six seeded turns, nothing sent. A rail of your messages: hover names the line, a click puts it at the top. 1440x900 and 1205x803, both themes, and the browser open.'
  })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
