// Alerts on your phone (0.827): a phone paired from anywhere turns alerts on from its page; Settings says so and offers
// approvals in quiet hours. With the phone's page closed and Locust's window behind, a run waiting on an approval
// buzzes the phone: the PC locks the alert for that phone's browser, the relay signs it and posts it to the browser's
// own push service, and the phone's service worker shows it. The lock screen says who and what kind, never the file.
//
//   node _tools/drive-your-phone-alerts.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// Real pushes need a relay that holds the signing key, so the relay is the live one unless --relay names another.
// The phone is a Chrome at a phone's size with a profile of its own, notifications allowed for the relay's address (a
// person taps Allow), its window off the screen. Not headless: a headless Chrome's push subscription is refused by
// Google's push service as gone (measured 10/23), so a real one is the only phone that shows a real push. Its messages
// run on the free OpenCode model, so nothing is spent.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { access, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'https://app.locust.lol').replace(/\/+$/, '')
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const PHONE_PORT = 9779

const reachable = await fetch(`${RELAY}/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} does not answer.`)
  process.exit(1)
}

const root = new URL('..', import.meta.url).pathname.slice(1)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-alerts-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-alerts-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// Approve each action: a file she makes raises a card, which buzzes the phone.
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }
const missionId = 'mission_5e000000-0000-4000-8000-000827000000'
const runId = 'run_5e0827'
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

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `phone-alerts-${tag}`,
  port: 9776,
  workspace,
  profilePath,
  env: { LOCUST_RELAY: RELAY },
  outPath: join(recordRoot('your-phone-alerts-2026-10-23'), tag),
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* The phone: a Chrome over its own DevTools sockets, one for the browser and one for the tab. */
let phone
let chrome
const connect = async (url, onEvent) => {
  const socket = new WebSocket(url)
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
  let id = 0
  const waiting = new Map()
  socket.onmessage = (message) => {
    const data = JSON.parse(String(message.data))
    if (data.id !== undefined && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id) } else if (data.method !== undefined) onEvent?.(data)
  }
  return { socket, call: (method, params = {}) => new Promise((resolve) => { id += 1; waiting.set(id, resolve); socket.send(JSON.stringify({ id, method, params })) }) }
}
async function openPhone(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-alerts-chrome-'))
  chrome = spawn(CHROME, [`--remote-debugging-port=${String(PHONE_PORT)}`, `--user-data-dir=${userData}`, '--no-first-run', '--no-default-browser-check', '--window-size=390,844', '--window-position=-3000,0', 'about:blank'], { stdio: 'ignore' })
  let target
  for (let tries = 0; tries < 40 && target === undefined; tries += 1) {
    await sleep(500)
    target = await fetch(`http://127.0.0.1:${String(PHONE_PORT)}/json/list`).then((r) => r.json()).then((list) => list.find((one) => one.type === 'page')).catch(() => undefined)
  }
  if (target === undefined) throw new Error('the phone (Chrome) did not start')
  const version = await fetch(`http://127.0.0.1:${String(PHONE_PORT)}/json/version`).then((r) => r.json())
  const browser = await connect(version.webSocketDebuggerUrl)
  // The person taps Allow when the page asks to send notifications.
  const granted = await browser.call('Browser.grantPermissions', { origin: RELAY, permissions: ['notifications'] })
  if (granted.error !== undefined) throw new Error(`notifications could not be allowed: ${JSON.stringify(granted.error)}`)
  const registrations = []
  const tab = await connect(target.webSocketDebuggerUrl, (data) => {
    if (data.method === 'ServiceWorker.workerRegistrationUpdated') for (const one of data.params.registrations) if (!one.isDeleted) registrations.push(one)
  })
  await tab.call('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true })
  await tab.call('Page.enable')
  await tab.call('ServiceWorker.enable')
  await tab.call('Page.navigate', { url })
  await sleep(2500)
  let shot = 0
  phone = {
    registrations,
    call: tab.call,
    evaluate: async (expression) => (await tab.call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value,
    navigate: async (to) => { await tab.call('Page.navigate', { url: to }); await sleep(2500) },
    // In the pocket, and out again: the window minimized hides the page as a locked screen does.
    pocket: async (hidden) => {
      const { result } = await browser.call('Browser.getWindowForTarget', { targetId: target.id })
      await browser.call('Browser.setWindowBounds', { windowId: result.windowId, bounds: { windowState: hidden ? 'minimized' : 'normal' } })
      await sleep(1500)
    },
    capture: async (label) => {
      shot += 1
      const data = (await tab.call('Page.captureScreenshot', { format: 'png' })).result?.data
      if (data !== undefined) await writeFile(join(drive.out, `phone-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
      say(`phone ${String(shot).padStart(2, '0')}. ${label}`)
    },
    close: () => { for (const one of [tab, browser]) { try { one.socket.close() } catch { /* gone */ } } chrome?.kill() }
  }
}
const PAGE = `(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`
const phoneWait = async (pattern, seconds = 30) => {
  const deadline = Date.now() + seconds * 1000
  let seen = ''
  while (Date.now() < deadline) {
    seen = String(await phone.evaluate(PAGE))
    if (pattern.test(seen)) return seen
    await sleep(1000)
  }
  return seen
}
// What the phone's lock screen holds: the notifications its service worker showed, read from the page in the pocket.
const NOTIFICATIONS = `(async () => {
  const reg = await navigator.serviceWorker.getRegistration('/')
  if (!reg) return JSON.stringify({ none: 'no service worker' })
  const shown = await reg.getNotifications()
  return JSON.stringify(shown.map((one) => ({ title: one.title, body: one.body, tag: one.tag })))
})()`
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
const DEVICES = `[...(section?.querySelectorAll('.lc-phone__device') ?? [])].map((row) => row.innerText.replace(/\\s+/g, ' ')).join(' | ')`

try {
  await drive.capture('launch: Mira answered PELICAN', () => drive.ready())
  await drive.resize(1215, 860)

  const on = JSON.parse(String(await drive.capture('Your phone, on from anywhere: the code', () => drive.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ text: ${SECTION}, url: settings.code?.url ?? '', connected: settings.connected })`)))))
  check('on, this computer is connected to the relay and shows a code', on.connected === true && on.url.startsWith(`${RELAY}/#p=`), on.text.slice(0, 200))

  await openPhone(on.url)
  const offer = await phoneWait(/Turn on alerts/)
  await phone.capture('the phone: paired, alerts offered')
  check('paired, the phone offers alerts', /Conversations/i.test(offer) && /Turn on alerts/.test(offer), offer.slice(0, 400))
  check('the page put its service worker in', phone.registrations.some((one) => one.scopeURL === `${RELAY}/`), JSON.stringify(phone.registrations.map((one) => one.scopeURL)))
  const quietBefore = String(await drive.evaluate(settingsScript(`for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250)); return ${SECTION}`)))
  check('before alerts are on, Settings does not offer approvals in quiet hours', !/Let approvals and questions buzz/.test(quietBefore) && !/alerts on/.test(quietBefore), quietBefore.slice(0, 300))

  await phone.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Turn on alerts')?.click()`)
  const turnedOn = await phoneWait(/Turn off alerts|could not|Something went wrong/i, 45)
  await phone.capture('the phone: alerts on')
  check('Turn on alerts signs this browser up with its push service', /Turn off alerts/.test(turnedOn), turnedOn.slice(0, 400))
  const subscription = JSON.parse(String(await phone.evaluate(`navigator.serviceWorker.ready.then((reg) => reg.pushManager.getSubscription()).then((sub) => JSON.stringify({ endpoint: sub ? new URL(sub.endpoint).host : '' }))`)))
  check('the browser holds a subscription at a push service', /^(fcm\.googleapis\.com|.*\.push\.services\.mozilla\.com|web\.push\.apple\.com|.*\.notify\.windows\.com)$/.test(subscription.endpoint), subscription.endpoint)

  const listed = String(await drive.capture('Settings: the phone, alerts on', () => drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !/alerts on/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${DEVICES} + ' || ' + ${SECTION}`))))
  check('Settings lists the phone with its alerts on', /Open now · alerts on/.test(listed), listed.slice(0, 300))
  check('Settings offers approvals in quiet hours, off until chosen', /Let approvals and questions buzz in quiet hours/.test(listed), listed.slice(-300))
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Settings, alerts on, at ${String(width)} by ${String(height)}`, () => drive.evaluate(settingsScript(`section?.querySelector('.lc-phone__night')?.scrollIntoView({ block: 'center' }); return 1`)))
  }
  await drive.resize(1215, 860)
  const night = String(await drive.evaluate(settingsScript(`
    section?.querySelector('[aria-label="Let approvals buzz in quiet hours"]')?.click()
    await new Promise((r) => setTimeout(r, 900))
    const first = section?.querySelector('[aria-label="Let approvals buzz in quiet hours"]')?.getAttribute('aria-checked')
    const saved = (await window.desktop.phone.anywhere()).nightApprovals
    section?.querySelector('[aria-label="Let approvals buzz in quiet hours"]')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return first + ' ' + String(saved) + ' ' + section?.querySelector('[aria-label="Let approvals buzz in quiet hours"]')?.getAttribute('aria-checked')`)))
  check('the quiet-hours switch turns on, is kept, and turns off again', night === 'true true false', night)

  // Away: the phone asks Mira for a file, then goes in the pocket: its page hidden, as a locked screen hides it, so it
  // lets go of its line. Locust's window is behind (a drive's window never has the focus), so the approval her action
  // waits on goes to the phone.
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  await phoneWait(/PELICAN.*PELICAN/, 15)
  const MAKE = 'Create a file named alerts-check.txt in this folder containing the word OK. Use your file-writing tool.'
  await phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(MAKE)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
  await sleep(2000)
  await phone.capture('the phone: asked Mira for a file')
  await phone.pocket(true)
  check('in the pocket, the page is hidden', await phone.evaluate('document.visibilityState') === 'hidden', await phone.evaluate('document.visibilityState'))
  say('the phone: in the pocket; waiting for the approval to buzz it')
  let shown = []
  const deadline = Date.now() + 300_000
  let said = ''
  while (Date.now() < deadline) {
    const read = JSON.parse(String(await phone.evaluate(NOTIFICATIONS) ?? '[]'))
    if (Array.isArray(read) && read.length > 0) { shown = read; break }
    // How the computer sees the phone meanwhile: open or not, alerts on or not.
    const now = String(await drive.evaluate(`window.desktop.phone.anywhere().then((s) => JSON.stringify({ connected: s.connected, devices: s.devices.map((d) => ({ online: d.online, alerts: d.alerts === true })) }))`))
    if (now !== said) { say(`  the computer sees: ${now}`); said = now }
    await sleep(3000)
  }
  check('with the page hidden, the waiting approval buzzed the phone through its push service', shown.some((one) => one.title === 'Mira needs your approval'), JSON.stringify(shown))
  check('the lock screen says who and what kind, never the file or the message', shown.length > 0 && shown.every((one) => !/alerts-check|OK|Create a file/.test(`${one.title} ${one.body}`)) && shown.some((one) => one.body === 'The turn is paused until you answer.'), JSON.stringify(shown))
  await drive.capture('the window: Mira waiting on an approval', () => drive.evaluate('1'))

  // The person takes the phone out and answers.
  await phone.pocket(false)
  const asking = await phoneWait(/Waiting on you/i, 60)
  await phone.capture('the phone: opened from the alert, the card')
  check('opened, the phone shows the card the alert was about', /Waiting on you/i.test(asking) && /Mira asks/.test(asking), asking.slice(0, 300))
  let made = false
  for (let i = 0; i < 90 && !made; i += 1) {
    await phone.evaluate(`[...document.querySelectorAll('button.act')].find((b) => b.innerText.trim() === 'Allow once')?.click()`)
    await sleep(2000)
    made = await access(join(workspace, 'alerts-check.txt')).then(() => true, () => false)
  }
  check('allowed from the phone, the action ran', made, made ? 'alerts-check.txt is there' : '(no file)')

  await phone.evaluate(`document.querySelector('.back')?.click()`)
  await phoneWait(/Turn off alerts/, 15)
  await phone.evaluate(`[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Turn off alerts')?.click()`)
  const turnedOff = await phoneWait(/Turn on alerts/, 30)
  await phone.capture('the phone: alerts off')
  const after = String(await drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && /alerts on/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${DEVICES} + ' || ' + ${SECTION}`)))
  check('Turn off alerts on the phone: Settings no longer says alerts on, nor offers the quiet-hours switch', /Turn on alerts/.test(turnedOff) && !/alerts on/.test(after) && !/Let approvals and questions buzz/.test(after), after.slice(0, 300))

  await drive.capture('Your phone, off again', () => drive.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return ${SECTION}`)))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  phone?.close()
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}. A seeded conversation; the phone is a Chrome at 390x844, off the screen, with notifications allowed; its messages run on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
