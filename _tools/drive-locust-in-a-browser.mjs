// Locust in a browser (0.830): a paired device let open the full window opens app.locust.lol/desk in a browser and
// gets the whole window -- the same window, that release's own code -- driving this computer: it lists the
// conversations, opens one, sends the next message and is answered, while this computer's window shows that Locust is
// open in a browser. What stays on this computer is refused. Use Locust here takes it back, and the browser says so;
// the browser can take it again. Full window off, and the browser is told no.
//
//   node _tools/drive-locust-in-a-browser.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// The relay is a local copy unless --relay names another, pointed at a local stand-in for Locust's downloads holding
// this build's window zip; start both first:
//   node _tools/window-zip.mjs <version> --out D:/work/scratch/desk-downloads/window/Locust-<version>-window.zip
//   node _tools/serve-window-zip.mjs D:/work/scratch/desk-downloads --port 8791
//   (apps/relay) npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local --var DESK_DOWNLOADS:http://127.0.0.1:8791/
// The browser is a headless Chrome at a laptop's size with a profile of its own. The conversation is SEEDED (Mira
// answered PELICAN); the message the browser sends runs on the free OpenCode model, so nothing is spent.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'http://127.0.0.1:8790').replace(/\/+$/, '')
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const BROWSER_PORT = 9798

const reachable = await fetch(`${RELAY}/desk/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} has no /desk/. Start it in apps/relay (see this file's header).`)
  process.exit(1)
}

const root = new URL('..', import.meta.url).pathname.slice(1)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-desk-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-desk-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'accept-edits' } }
const missionId = 'mission_5e000000-0000-4000-8000-000830000000'
const runId = 'run_5e0830'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'Write the word PELICAN in capitals and nothing else.',
  runtime: 'opencode', model: FREE_ROUTE.model, requestedRouteId: 'opencode', resolvedRouteId: `opencode:${FREE_ROUTE.model}`, cliVersion: null,
  workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
})
const process_ = { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at }
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'PELICAN', final: true }),
  event(2, 'run.completed', { runtimeTerminal: 'completed', process: process_ })
])
await ledger.flush?.()

const outPath = join(recordRoot('locust-in-a-browser-2026-10-23'), tag)
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `desk-${tag}`,
  port: 9797,
  workspace,
  profilePath,
  env: { LOCUST_RELAY: RELAY },
  outPath,
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* The browser: a headless Chrome at a laptop's size, over its own DevTools socket. */
let browser
async function openBrowser(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-desk-chrome-'))
  const chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${String(BROWSER_PORT)}`, `--user-data-dir=${userData}`, '--no-first-run', '--no-default-browser-check', '--window-size=1280,820', 'about:blank'], { stdio: 'ignore' })
  let target
  for (let tries = 0; tries < 40 && target === undefined; tries += 1) {
    await sleep(500)
    target = await fetch(`http://127.0.0.1:${String(BROWSER_PORT)}/json/list`).then((r) => r.json()).then((list) => list.find((one) => one.type === 'page')).catch(() => undefined)
  }
  if (target === undefined) throw new Error('the browser (headless Chrome) did not start')
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
  let id = 0
  const waiting = new Map()
  const logs = []
  socket.onmessage = (message) => {
    const data = JSON.parse(String(message.data))
    if (data.id !== undefined && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id) }
    if (data.method === 'Runtime.consoleAPICalled' || data.method === 'Runtime.exceptionThrown') logs.push(JSON.stringify(data.params).slice(0, 400))
  }
  const call = (method, params = {}) => new Promise((resolve) => { id += 1; waiting.set(id, resolve); socket.send(JSON.stringify({ id, method, params })) })
  await call('Emulation.setDeviceMetricsOverride', { width: 1280, height: 820, deviceScaleFactor: 1, mobile: false })
  await call('Page.enable')
  await call('Runtime.enable')
  await call('Page.navigate', { url })
  await sleep(2500)
  let shot = 0
  browser = {
    logs,
    evaluate: async (expression) => (await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value,
    /** A script that runs in every page this browser opens from now on, before the page's own. */
    onEveryPage: async (source) => { await call('Page.addScriptToEvaluateOnNewDocument', { source }) },
    /*
     * The relay out of reach in this tab, from its next load, as with the computer off: every socket the page opens
     * fails as an unreachable one does. Chrome's own URL blocking leaves WebSockets alone, so it is done in the page.
     */
    offline: async (on) => { await call('Runtime.evaluate', { expression: on ? `sessionStorage.setItem('drive-offline', '1')` : `sessionStorage.removeItem('drive-offline')` }) },
    navigate: async (to) => { await call('Page.navigate', { url: to }); await sleep(2500) },
    capture: async (label) => {
      shot += 1
      const data = (await call('Page.captureScreenshot', { format: 'png' })).result?.data
      if (data !== undefined) await writeFile(join(outPath, `browser-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
      say(`browser ${String(shot).padStart(2, '0')}. ${label}`)
    },
    close: () => { try { socket.close() } catch { /* gone */ } chrome.kill() }
  }
}
const PAGE = `(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`
const browserWait = async (pattern, seconds = 30) => {
  const deadline = Date.now() + seconds * 1000
  let seen = ''
  while (Date.now() < deadline) {
    seen = String(await browser.evaluate(`${PAGE} + ' ' + ([...document.querySelectorAll('[role="dialog"]')].filter((d) => d.getAttribute('aria-label') === 'Locust in a browser').map((d) => d.innerText).join(' '))`))
    if (pattern.test(seen)) return seen
    await sleep(1000)
  }
  return seen
}
const settingsScript = (body) => `(async () => {
  if (!document.querySelector('[data-setting="phone"]')) {
    if (!document.querySelector('.lc-settings__navitem')) {
      ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
      await new Promise((r) => setTimeout(r, 900))
    }
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 900))
  }
  const section = document.querySelector('[data-setting="phone"]')
  section?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const SECTION = `(section?.innerText ?? 'no section').replace(/\\s+/g, ' ')`
const PC_PAGE = `(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`
const pcWait = async (pattern, seconds = 30) => {
  const deadline = Date.now() + seconds * 1000
  let seen = ''
  while (Date.now() < deadline) {
    seen = String(await drive.evaluate(PC_PAGE).catch(() => ''))
    if (pattern.test(seen)) return seen
    await sleep(1000)
  }
  return seen
}

try {
  await drive.capture('launch: Mira answered PELICAN', () => drive.ready())
  await drive.resize(1215, 860)

  const on = JSON.parse(String(await drive.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ text: ${SECTION}, url: settings.code?.url ?? '' })`))))
  check('Your phone, from anywhere, is on with a code, and offers its link for a computer', on.url !== '' && /Copy the link/.test(on.text), on.text.slice(0, 300))

  await openBrowser(on.url)
  const paired = await browserWait(/Conversations/i)
  check('the browser pairs with the code’s link, as a phone does', /Conversations/i.test(paired), paired.slice(0, 200))

  // Not let in yet: the way in says so.
  await browser.navigate(`${RELAY}/desk/`)
  const refused = await browserWait(/Locust cannot open here/, 30)
  await browser.capture('the way in, before Full window is on')
  check('before Full window is on, the way in says this browser may not open the window, and where to turn it on', /may not open Locust’s window/.test(refused) && /Full window/.test(refused), refused.slice(0, 300))

  const allowed = String(await drive.capture('Settings: Full window on for the browser', () => drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250))
    section?.querySelector('.lc-phone__device [role="switch"]')?.click()
    for (let i = 0; i < 40 && section?.querySelector('.lc-phone__device [role="switch"]')?.getAttribute('aria-checked') !== 'true'; i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('Settings turns Full window on for that device, and says where to open it', /Full window/.test(allowed) && /app\.locust\.lol\/desk/.test(allowed), allowed.slice(0, 400))

  await browser.navigate(`${RELAY}/desk/`)
  const opened = await browserWait(/PELICAN/, 90)
  const where = String(await browser.evaluate('location.pathname'))
  await browser.capture('Locust in a browser: the whole window')
  check('the way in opens this computer’s version of the window, from the relay', /^\/desk\/v\/\d+\.\d+\.\d+\/desk\.html$/.test(where), where)
  check('the window in the browser lists the conversation, with its answer', /Write the word PELICAN/.test(opened), opened.slice(0, 300))
  check('and has no window controls of its own', String(await browser.evaluate(`document.querySelector('.lc-windowcontrols') === null`)) === 'true')
  const agents = await browserWait(/[1-9]\d* agents? ready/, 60)
  say(`browser line: ${String(await browser.evaluate('window.locustDesk?.state()'))}; waiting: ${JSON.stringify(await browser.evaluate('window.locustDesk?.pending()'))}`)
  check('the browser hears which agents this computer has', /[1-9]\d* agents? ready/.test(agents), (/\d+ agents? ready/.exec(agents) ?? ['none'])[0])

  /*
   * OPENS AT ONCE, ON WHAT IT LAST SAW (0.834, desk-snapshot.ts). Opened again, the window shows the conversation it
   * read last time at once, under the line that says so, and then opens live from this computer. What the first
   * moments showed is noted by a watcher in the page, since the step to live is a matter of seconds.
   */
  await sleep(2500)
  await browser.onEveryPage(`(() => {
    if (sessionStorage.getItem('drive-saw') !== null || !/\\/desk\\/v\\//.test(location.pathname)) return
    const started = Date.now()
    const look = () => {
      const line = document.querySelector('[aria-label="What Locust last showed here"]')
      if (line === null) { if (Date.now() - started < 15000) setTimeout(look, 50); return }
      sessionStorage.setItem('drive-saw', JSON.stringify({ ms: Date.now() - started, line: line.textContent, pelican: /PELICAN/.test(document.body.innerText) }))
      setTimeout(() => { const later = JSON.parse(sessionStorage.getItem('drive-saw')); later.pelican = later.pelican || /PELICAN/.test(document.body.innerText); later.refused = 'checked'; sessionStorage.setItem('drive-saw', JSON.stringify(later)) }, 400)
    }
    look()
  })()`)
  const deskAt = String(await browser.evaluate('location.href'))
  await browser.navigate(deskAt)
  const live = await browserWait(/PELICAN/, 90)
  await sleep(3000)
  const saw = JSON.parse(String(await browser.evaluate(`sessionStorage.getItem('drive-saw') ?? 'null'`)))
  await browser.capture('opened again: live, after what it last showed')
  check('opened again, the window shows at once what it last showed, and says so', saw !== null && /This is what Locust last showed here, at /.test(saw.line) && saw.pelican === true, JSON.stringify(saw))
  check('then it opens live from this computer, and the line goes', /PELICAN/.test(live) && String(await browser.evaluate(`document.querySelector('[aria-label="What Locust last showed here"]') === null && sessionStorage.getItem('locust-desk-live') === null`)) === 'true', live.slice(0, 200))
  check('what it keeps is sealed in the browser, never the conversation as text', String(await browser.evaluate(`new Promise((resolve) => { const r = indexedDB.open('locust-desk-snapshot', 1); r.onsuccess = () => { const g = r.result.transaction('kept', 'readonly').objectStore('kept').get('snapshot'); g.onsuccess = () => { const kept = g.result; resolve(kept && kept.box instanceof ArrayBuffer && !new TextDecoder().decode(kept.box).includes('PELICAN') ? 'sealed' : 'open: ' + JSON.stringify(kept).slice(0, 80)) } } })`)) === 'sealed')

  // With the computer out of reach -- the relay's room blocked -- it goes on showing what it last showed, and says so.
  await browser.onEveryPage(`(() => {
    if (sessionStorage.getItem('drive-offline') !== '1') return
    window.WebSocket = class extends EventTarget {
      constructor(url) {
        super(); this.url = String(url); this.readyState = 0
        setTimeout(() => { this.readyState = 3; for (const kind of ['error', 'close']) { const event = kind === 'close' ? new CloseEvent('close', { code: 1006 }) : new Event('error'); this['on' + kind]?.(event); this.dispatchEvent(event) } }, 50)
      }
      send() {} close() {}
    }
  })()`)
  await browser.offline(true)
  await browser.navigate(deskAt)
  await sleep(4000)
  const away = JSON.parse(String(await browser.evaluate(`JSON.stringify({ line: document.querySelector('[aria-label="What Locust last showed here"]')?.textContent ?? null, pelican: /PELICAN/.test(document.body.innerText) })`)))
  await browser.capture('computer out of reach: what Locust last showed, under its line')
  check('with the computer out of reach, it still shows what it last showed, under the line', away.pelican === true && /This is what Locust last showed here, at /.test(away.line ?? ''), JSON.stringify(away))
  check('and the line says the relay cannot be reached, and that it keeps trying', /relay cannot be reached from this browser.*keeps trying/.test(away.line ?? ''), away.line)
  await browser.offline(false)
  await browser.navigate(deskAt)
  // What it last showed comes first again, so live is when the line has gone and the conversation is there.
  let liveAgain = false
  for (let waited = 0; waited < 90 && !liveAgain; waited += 1) {
    await sleep(1000)
    liveAgain = String(await browser.evaluate(`document.querySelector('[aria-label="What Locust last showed here"]') === null && /PELICAN/.test(document.body.innerText)`).catch(() => 'false')) === 'true'
  }
  check('reached again, it opens live', liveAgain)

  const cover = await pcWait(/Locust is open in a browser/, 30)
  await drive.capture('this computer’s window: the cover', () => cover)
  check('this computer’s window shows that Locust is open in a browser, naming it, with the way back', /Locust is open in a browser/.test(cover) && /Use Locust here/.test(cover), cover.slice(0, 300))

  const refusedCall = String(await browser.evaluate(`window.desktop.terminal.shells().then(() => 'answered', (error) => error.message)`))
  check('a call that stays on this computer is refused in the browser, in words', refusedCall === 'This works in Locust on your computer.', refusedCall)

  /*
   * ROUTINES FROM A BROWSER (0.834, desk-calls.ts). A routine made and paused in the browser is on this computer,
   * paused, and shows on the browser's Routines page; exporting it, which opens a dialog here, stays here; removed in
   * the browser, it is gone from this computer.
   */
  const route = JSON.stringify(MIRA.route)
  const madeHere = JSON.parse(String(await browser.evaluate(`(async () => {
    const made = await window.desktop.createRoutine({ name: 'Weekly notes from a browser', teammateId: '${MIRA.teammateId}', route: ${route}, steps: ['Write the word ORCHID and nothing else.'], learnedFrom: [] })
    if (!made.ok) return JSON.stringify({ error: made.error })
    const paused = await window.desktop.pauseRoutine(made.data.routine.routineId, true)
    const exported = await window.desktop.exportRoutine({ routineId: made.data.routine.routineId }).then((answer) => JSON.stringify(answer), (error) => error.message)
    return JSON.stringify({ id: made.data.routine.routineId, paused: paused.ok && paused.data.routine.paused === true, exported })
  })()`)))
  const onThePc = JSON.parse(String(await drive.evaluate(`window.desktop.listRoutines().then((answer) => JSON.stringify(answer.ok ? answer.data.routines.map((one) => ({ id: one.routineId, name: one.name, paused: one.paused === true })) : answer))`)))
  check('a routine made and paused in the browser is on this computer, paused', madeHere.paused === true && Array.isArray(onThePc) && onThePc.some((one) => one.id === madeHere.id && one.paused), JSON.stringify({ madeHere, onThePc }))
  check('exporting it, which opens a dialog on this computer, stays there and says so', madeHere.exported === 'This works in Locust on your computer.', madeHere.exported)
  await browser.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Routines')?.click()`)
  const listed = await browserWait(/Weekly notes from a browser/, 20)
  await browser.capture('the browser: a routine made and paused there')
  // With no schedule, a pause changes nothing it shows; this computer's list above says it is paused.
  check('the browser\u2019s Routines page shows it', /Weekly notes from a browser/.test(listed), listed.slice(0, 300))
  const removed = String(await browser.evaluate(`window.desktop.removeRoutine('${madeHere.id}').then((answer) => String(answer.ok))`))
  const gone = String(await drive.evaluate(`window.desktop.listRoutines().then((answer) => String(answer.ok && !answer.data.routines.some((one) => one.routineId === '${madeHere.id}')))`))
  check('removed in the browser, it is gone from this computer', removed === 'true' && gone === 'true', JSON.stringify({ removed, gone }))

  /*
   * A TEAMMATE AND A ROOM FROM A BROWSER (0.834). Mira's instructions changed in the browser are hers on this
   * computer; a change to what she may spend is refused there; a room made in the browser is on this computer.
   */
  const mira = MIRA.teammateId
  const changed = JSON.parse(String(await browser.evaluate(`(async () => {
    const base = { teammateId: '${mira}', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', avatar: (await window.desktop.listTeammates()).data.teammates.find((one) => one.teammateId === '${mira}').avatar }
    const said = await window.desktop.updateTeammate({ ...base, instructions: 'Answer in one word when you can.' })
    const spend = await window.desktop.updateTeammate({ ...base, monthlyLimitUsd: 500 }).then((answer) => JSON.stringify(answer), (error) => error.message)
    const room = await window.desktop.createRoom({ name: 'Browser room', teammateIds: ['${mira}'] })
    return JSON.stringify({ said: said.ok, spend, room: room.ok ? room.data.room.roomId : JSON.stringify(room) })
  })()`)))
  const pcSees = JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify({ instructions: (await window.desktop.listTeammates()).data.teammates.find((one) => one.teammateId === '${mira}')?.instructions, limit: (await window.desktop.listTeammates()).data.teammates.find((one) => one.teammateId === '${mira}')?.monthlyLimitUsd ?? null, rooms: (await window.desktop.listRooms()).data.rooms.map((one) => one.name) }))()`)))
  check('a teammate\u2019s instructions changed in the browser are hers on this computer', changed.said === true && pcSees.instructions === 'Answer in one word when you can.', JSON.stringify({ changed, pcSees }))
  check('what a teammate may spend is not changed from a browser, and it says so', changed.spend === 'This works in Locust on your computer.' && pcSees.limit === null, JSON.stringify({ spend: changed.spend, limit: pcSees.limit }))
  check('a room made in the browser is on this computer', Array.isArray(pcSees.rooms) && pcSees.rooms.includes('Browser room'), JSON.stringify(pcSees.rooms))
  await browser.evaluate(`window.desktop.removeRoom('${changed.room}')`)

  // COMPARE FROM A BROWSER (0.835): a comparison started in the browser, on two free models, is on this computer.
  const compared = JSON.parse(String(await browser.evaluate(`window.desktop.startCompare({ prompt: 'Write the word TULIP and nothing else.', routes: [{ runtime: 'opencode', model: '${FREE_ROUTE.model}' }, { runtime: 'opencode', model: 'opencode/nemotron-3-ultra-free' }] }).then((answer) => JSON.stringify(answer.ok ? { id: answer.data.compare.compareId } : answer), (error) => JSON.stringify({ thrown: error.message }))`)))
  const pcCompares = JSON.parse(String(await drive.evaluate(`window.desktop.listCompares().then((answer) => JSON.stringify(answer.ok ? answer.data.compares.map((one) => ({ id: one.compareId, columns: one.slots.length })) : answer))`)))
  check('a comparison started in the browser is on this computer, with both models', typeof compared.id === 'string' && Array.isArray(pcCompares) && pcCompares.some((one) => one.id === compared.id && one.columns === 2), JSON.stringify({ compared, pcCompares }))
  await browser.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Conversations')?.click()`)

  // The conversation, opened and continued from the browser.
  await browser.evaluate(`[...document.querySelectorAll('.lc-convrow button.lc-conv, .lc-conv')].find((row) => /PELICAN/i.test(row.innerText || row.getAttribute('title') || ''))?.click()`)
  await sleep(2500)
  const sent = String(await browser.evaluate(sendAndWaitScript('Now write the word HARBOR in capitals and nothing else. Do not use any tool.', { waitSeconds: 300 })))
  await browser.capture('the browser: sent and answered')
  check('a message sent from the browser runs on this computer, and its answer streams into the browser', /HARBOR/.test(sent.slice(sent.indexOf('Do not use any tool.') + 20)), sent.slice(-300))

  // Taken back on this computer.
  await drive.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Use Locust here')?.click()`)
  const back = await pcWait(/Mira|PELICAN|Conversations/, 30)
  const told = await browserWait(/Locust is in use on your computer now/, 20)
  await browser.capture('the browser: told the computer took it back')
  check('Use Locust here on this computer brings the app back there', !/Locust is open in a browser/.test(back) && /PELICAN|Mira/.test(back), back.slice(0, 200))
  check('and the browser says so, with a way to use it there again', /Locust is in use on your computer now/.test(told) && /Use Locust here/.test(told), told.slice(0, 300))

  await browser.evaluate(`[...document.querySelectorAll('[role="dialog"]')].filter((d) => d.getAttribute('aria-label') === 'Locust in a browser').flatMap((d) => [...d.querySelectorAll('button')]).find((b) => b.innerText.trim() === 'Use Locust here')?.click()`)
  const again = await pcWait(/Locust is open in a browser/, 30)
  check('the browser takes it again, and this computer’s window shows the cover again', /Locust is open in a browser/.test(again), again.slice(0, 200))
  await drive.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Use Locust here')?.click()`)
  await pcWait(/PELICAN|Mira/, 30)

  // Full window off: the browser is told no.
  await drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250))
    section?.querySelector('.lc-phone__device [role="switch"]')?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return 1`))
  await browser.navigate(`${RELAY}/desk/`)
  const off = await browserWait(/Locust cannot open here/, 30)
  check('Full window off, the way in says no again', /may not open Locust’s window/.test(off), off.slice(0, 200))
  const errors = browser.logs.filter((line) => /exceptionThrown|"type":"error"/.test(line))
  say(`browser console errors: ${String(errors.length)}${errors.length === 0 ? '' : `\n${errors.slice(0, 8).join('\n')}`}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.stack ?? error.message : String(error)}`)
} finally {
  browser?.close()
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}. A seeded conversation; the browser is headless Chrome at 1280x820, paired with the code's link; its message runs on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` }).catch(() => undefined)
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
