// Written while your computer is off (0.837): a phone paired through Locust's relay keeps what it last saw, and with
// Locust closed it still lists the conversations and opens one. A message written then waits at the relay, sealed;
// Take back removes it; Locust, opened again, sends it through the phone's own path, and the phone says so. One written
// more than twelve hours before is held for the phone to decide. Twenty may wait at once.
//
//   node _tools/drive-write-while-your-computer-is-off.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// The relay is a copy run locally in Cloudflare's own runtime, as for drive-your-phone-anywhere.mjs. Start it first,
// in apps/relay:
//   npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local
// The conversation is SEEDED (Mira answered PELICAN); the message Locust sends when it is back runs on the free
// OpenCode model, so nothing is spent.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { FREE_ROUTE, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'http://127.0.0.1:8790').replace(/\/+$/, '')
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const PHONE_PORT = 9788
const OUT = arg('--out') ?? join('D:/work/scratch/phone-while-off', tag)

const reachable = await fetch(`${RELAY}/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} does not answer. Start it in apps/relay: npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local`)
  process.exit(1)
}

const root = new URL('..', import.meta.url).pathname.slice(1)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-notes-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-notes-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE } }
const missionId = 'mission_5e000000-0000-4000-8000-000837000000'
const runId = 'run_5e0837'
const at = new Date(Date.now() - 600_000).toISOString()
const ASK = 'Write the word PELICAN in capitals and nothing else.'
await ledger.createMission({
  missionId, runId, prompt: ASK,
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
const seed = { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* The phone: a headless Chrome over its own DevTools socket. It outlives Locust's window. */
let phone
let chrome
async function openPhone(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-notes-chrome-'))
  chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${String(PHONE_PORT)}`, `--user-data-dir=${userData}`, '--no-first-run', '--no-default-browser-check', '--window-size=390,844', 'about:blank'], { stdio: 'ignore' })
  let target
  for (let tries = 0; tries < 40 && target === undefined; tries += 1) {
    await sleep(500)
    target = await fetch(`http://127.0.0.1:${String(PHONE_PORT)}/json/list`).then((r) => r.json()).then((list) => list.find((one) => one.type === 'page')).catch(() => undefined)
  }
  if (target === undefined) throw new Error('the phone (headless Chrome) did not start')
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
  let id = 0
  const waiting = new Map()
  socket.onmessage = (message) => {
    const data = JSON.parse(String(message.data))
    if (data.id !== undefined && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id) }
  }
  const call = (method, params = {}) => new Promise((resolve) => { id += 1; waiting.set(id, resolve); socket.send(JSON.stringify({ id, method, params })) })
  await call('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true })
  await call('Page.enable')
  await call('Page.navigate', { url })
  await sleep(2500)
  let shot = 0
  phone = {
    evaluate: async (expression) => (await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value,
    capture: async (label) => {
      shot += 1
      const data = (await call('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })).result?.data
      if (data !== undefined) await writeFile(join(OUT, `phone-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
      say(`phone ${String(shot).padStart(2, '0')}. ${label}`)
    },
    close: () => { try { socket.close() } catch { /* gone */ } chrome?.kill() }
  }
}
const PAGE = `(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`
const phoneWait = async (pattern, seconds = 30) => {
  const deadline = Date.now() + seconds * 1000
  let seen = ''
  while (Date.now() < deadline) {
    seen = String(await phone.evaluate(PAGE))
    if (pattern.test(seen)) return seen
    await sleep(500)
  }
  return seen
}
const write = (text) => phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(text)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
const toastSaid = () => phone.evaluate(`document.querySelector('.toast')?.innerText ?? ''`)
const press = (words, scope = 'document') => phone.evaluate(`(() => { const b = [...${scope}.querySelectorAll('button')].find((one) => one.innerText.trim() === ${JSON.stringify(words)}); b?.click(); return !!b })()`)
const waitCards = () => phone.evaluate(`document.querySelectorAll('[data-note]').length`)
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
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`
const NEXT = 'Now write the word HARBOR in capitals and nothing else. Do not use any tool.'

const first = await startDrive({ ...(packaged === undefined ? {} : { packaged }), name: `notes-${tag}`, port: 9787, workspace, profilePath, keep: true, env: { LOCUST_RELAY: RELAY }, outPath: join(OUT, 'window-before'), seed })
try {
  await first.capture('launch: Mira answered PELICAN', () => first.ready())
  await first.resize(1215, 860)
  const on = JSON.parse(String(await first.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ url: settings.code?.url ?? '', connected: settings.connected })`))))
  check('on from anywhere, Settings has a code', on.url !== '' && on.connected === true, on.url.replace(/#p=.*/, '#p=…'))
  await openPhone(on.url)
  const list = await phoneWait(/PELICAN/)
  check('paired, the phone lists the conversation', /Write the word PELICAN/.test(list), list.slice(0, 200))
  // Opened once, so the phone keeps its turns too.
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  await phoneWait(/PELICAN.*PELICAN/)
  await sleep(1500)
  await phone.evaluate(`document.querySelector('.back')?.click()`)
  await phoneWait(/Conversations/)
} finally {
  await first.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}. Locust on, a phone paired, then Locust closed.`, extra: `Checks failed so far: ${String(failures)}` })
}

try {
  const off = await phoneWait(/Your computer is off/, 30)
  await phone.capture('Locust closed: the phone shows what it last saw')
  check('with Locust closed, the phone says the computer is off and when it last saw it', /Your computer is off\. This is what your phone last saw, at /.test(off), off.slice(0, 260))
  check('and still lists the conversation', /Write the word PELICAN/.test(off), off.slice(0, 300))

  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  const thread = await phoneWait(/as your phone last saw it/, 15)
  await phone.capture('a conversation, as the phone last saw it, with Leave')
  check('a conversation opens on its kept turns, and its box offers Leave', /Write the word PELICAN in capitals/.test(thread) && /Mira PELICAN/.test(thread)
    && String(await phone.evaluate(`document.querySelector('form.compose button')?.innerText`)) === 'Leave', thread.slice(0, 300))

  await write(NEXT)
  const left = await phoneWait(/Waiting for your computer\. Locust sends it/, 15)
  await phone.capture('the message waits for the computer')
  check('a message left waits, said under the conversation', /Now write the word HARBOR/.test(left) && /Waiting to send/i.test(left), left.slice(-300))

  await write('Never mind this one.')
  await phoneWait(/Never mind this one/, 15)
  await phone.capture('a second message waiting')
  await phone.evaluate(`(() => { const card = [...document.querySelectorAll('[data-note]')].find((one) => /Never mind/.test(one.innerText)); [...card.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Take back').click() })()`)
  await sleep(1500)
  const toastBack = await toastSaid()
  await phone.capture('the second one taken back')
  check('Take back removes it from the relay, and says nothing was sent', /Taken back\. Nothing was sent\./.test(toastBack) && Number(await waitCards()) === 1, toastBack)

  // Written "yesterday": the phone's clock moved back thirteen hours for this one message.
  await phone.evaluate(`(() => { const real = Date.now; Date.now = () => real() - 13 * 60 * 60 * 1000; window.__realNow = real })()`)
  await write('A message from yesterday.')
  await phoneWait(/A message from yesterday/, 15)
  await phone.evaluate(`Date.now = window.__realNow`)
  check('three left, one taken back: two wait', Number(await waitCards()) === 2, await waitCards())

  await write('x'.repeat(8001))
  await sleep(800)
  const tooLong = await toastSaid()
  await phone.capture('too long to wait')
  check('an over-long message is refused in words, before it leaves the phone', /too long to wait/.test(tooLong), tooLong)

  // Twenty may wait: eighteen more, then the twenty-first is refused, and the extras taken back.
  for (let i = 1; i <= 18; i += 1) { await write(`Extra ${String(i)}`); await phoneWait(new RegExp(`Extra ${String(i)}(?!\\d)`), 10) }
  await write('One too many.')
  await sleep(1500)
  const full = await toastSaid()
  await phone.capture('the twenty-first is refused')
  check('the twenty-first waiting message is refused, and says what to do', /Twenty messages are already waiting/.test(full), full)
  for (let i = 0; i < 25 && Number(await phone.evaluate(`[...document.querySelectorAll('[data-note]')].filter((one) => /Extra/.test(one.innerText)).length`)) > 0; i += 1) {
    await phone.evaluate(`(() => { const card = [...document.querySelectorAll('[data-note]')].find((one) => /Extra/.test(one.innerText)); [...card.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Take back')?.click() })()`)
    await sleep(700)
  }
  check('the extras taken back, two wait again', Number(await waitCards()) === 2, await waitCards())

  // The relay's own storage: sealed boxes, never the words.
  const state = join(root, 'apps', 'relay', '.wrangler', 'state')
  const files = []
  const walk = async (folder) => { for (const entry of await readdir(folder, { withFileTypes: true }).catch(() => [])) { const path = join(folder, entry.name); if (entry.isDirectory()) await walk(path); else files.push(path) } }
  await walk(state)
  let found = []
  for (const file of files) { const bytes = await readFile(file).catch(() => Buffer.alloc(0)); if (bytes.includes('HARBOR') || bytes.includes('from yesterday')) found.push(file) }
  check('the relay\'s storage holds none of the words left', files.length > 0 && found.length === 0, `${String(files.length)} files read; with the words: ${found.join(', ') || 'none'}`)
} catch (error) {
  failures += 1
  say(`drive failed while off: ${error instanceof Error ? error.message : String(error)}`)
}

const second = await startDrive({ ...(packaged === undefined ? {} : { packaged }), name: `notes-${tag}-back`, port: 9787, workspace, profilePath, keep: true, env: { LOCUST_RELAY: RELAY }, outPath: join(OUT, 'window-after') })
try {
  await second.capture('Locust opened again', () => second.ready())
  await second.resize(1215, 860)
  const sent = await phoneWait(/Locust sent the message you left/, 60)
  await phone.capture('the computer is back: the message was sent')
  check('the phone says the message it left was sent', /Your computer is back, and Locust sent the message you left\./.test(sent), sent.slice(0, 200))
  const held = await phoneWait(/did not send this by itself/, 20)
  await phone.capture('the one from yesterday is held for the person')
  check('the one written thirteen hours ago is held, with Send now and Discard', /A message from yesterday/.test(held) && /did not send this by itself/.test(held)
    && String(await phone.evaluate(`[...document.querySelectorAll('[data-note] button')].map((b) => b.innerText.trim()).join(',')`)) === 'Send now,Discard', held.slice(0, 400))

  const window = String(await second.capture('the window: the phone\'s message, answered', () => second.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /PELICAN/i.test(r.innerText))?.click()
    for (let i = 0; i < 180; i += 1) {
      const said = ${THREAD}
      if (said.includes('Do not use any tool.') && /HARBOR/.test(said.slice(said.indexOf('Do not use any tool.') + 20))) break
      await new Promise((r) => setTimeout(r, 1000))
    }
    return ${THREAD}
  })()`)))
  check('Locust sent it as the person\'s next message, and Mira answered', /Now write the word HARBOR/.test(window) && /HARBOR/.test(window.slice(window.indexOf('Do not use any tool.') + 20)), window.slice(-300))

  await press('Discard')
  await sleep(1500)
  const gone = await toastSaid()
  await phone.capture('the held one discarded')
  check('Discard lets the held one go', /Discarded\./.test(gone) && Number(await waitCards()) === 0, gone)
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  const live = await phoneWait(/HARBOR.*HARBOR/, 30)
  await phone.capture('the phone, live again: the conversation with the answer')
  check('live again, the phone shows the message and its answer', /Now write the word HARBOR/.test(live), live.slice(-200))
} catch (error) {
  failures += 1
  say(`drive failed after: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  phone?.close()
  await second.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}. Locust opened again on the same profile.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
