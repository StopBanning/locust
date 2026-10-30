// Your phone, from anywhere (0.826): Settings pairs a phone through Locust's relay, and the phone's page, served by
// that relay, lists the conversations, opens one, sends the next message, answers a card and stays paired after a
// reload. Settings lists the phone; Remove there stops it at once; a new code pairs it again; off unpairs it.
//
//   node _tools/drive-your-phone-anywhere.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// The relay is a copy run locally in Cloudflare's own runtime unless --relay names another (the live one is
// https://app.locust.lol). Start the local one first, in apps/relay:
//   npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local
// Locust is pointed at it with LOCUST_RELAY. The phone is a headless Chrome at a phone's size with a profile of its
// own. The conversation is SEEDED (Mira answered PELICAN); the messages the phone sends run on the free OpenCode
// model, so nothing is spent.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { access, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'http://127.0.0.1:8790').replace(/\/+$/, '')
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const PHONE_PORT = 9778

const reachable = await fetch(`${RELAY}/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} does not answer. Start it in apps/relay: npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local`)
  process.exit(1)
}

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-anywhere-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-anywhere-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// Approve each action: a file she makes raises a card, which the phone answers.
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }
const missionId = 'mission_5e000000-0000-4000-8000-000826000000'
const runId = 'run_5e0826'
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

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `phone-anywhere-${tag}`,
  port: 9777,
  workspace,
  profilePath,
  env: { LOCUST_RELAY: RELAY },
  outPath: join(recordRoot('your-phone-anywhere-2026-10-23'), tag),
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* The phone: a headless Chrome over its own DevTools socket. */
let phone
let chrome
async function openPhone(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-anywhere-chrome-'))
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
    navigate: async (to) => { await call('Page.navigate', { url: to }); await sleep(2500) },
    reload: async () => { await call('Page.reload', {}); await sleep(2500) },
    capture: async (label) => {
      shot += 1
      const data = (await call('Page.captureScreenshot', { format: 'png' })).result?.data
      if (data !== undefined) await writeFile(join(drive.out, `phone-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
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
    await sleep(1000)
  }
  return seen
}
// Opens Settings only when it is not open already: pressing Settings again closes it.
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
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`

try {
  await drive.capture('launch: Mira answered PELICAN', () => drive.ready())
  await drive.resize(1215, 860)

  const off = String(await drive.capture('Settings > General: Your phone, off, From anywhere chosen', () => drive.evaluate(settingsScript(`
    await new Promise((r) => setTimeout(r, 400))
    const way = [...(section?.querySelectorAll('[role="radio"]') ?? [])].find((b) => b.getAttribute('aria-checked') === 'true')?.innerText ?? 'none'
    return ${SECTION} + ' || chosen: ' + way`))))
  check('Your phone is off until turned on, with From anywhere chosen for when it is', /^Your phone Off\./.test(off.trim()) && /chosen: From anywhere$/.test(off), off)

  const on = JSON.parse(String(await drive.capture('Your phone, on from anywhere: the code', () => drive.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ text: ${SECTION}, qr: section?.querySelector('.lc-phone__qr path')?.getAttribute('d')?.length ?? 0, url: settings.code?.url ?? '', connected: settings.connected })`)))))
  check('on, Settings shows a code and says this computer is connected to the relay', on.qr > 500 && on.connected === true && /Connected to Locust’s relay\./.test(on.text) && /On, from anywhere\./.test(on.text), `${on.text.slice(0, 300)} (qr path ${String(on.qr)})`)
  check('the code holds the relay, this computer’s room and key, and a secret', new RegExp(`^${RELAY.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/#p=[A-Za-z0-9_-]{22}\\.[A-Za-z0-9_-]{43}\\.[A-Za-z0-9_-]{22}$`).test(on.url), on.url.replace(/#p=.*/, '#p=…'))
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`Your phone, on from anywhere, at ${String(width)} by ${String(height)}`, () => drive.evaluate(settingsScript('return ' + SECTION)))
  }
  await drive.resize(1215, 860)

  await openPhone(on.url)
  const list = await phoneWait(/PELICAN/)
  await phone.capture('the phone: paired through the relay, conversations')
  check('paired through the relay, the phone lists the conversation with its answer', /Conversations/i.test(list) && /Write the word PELICAN/.test(list) && /Mira · done/.test(list), list.slice(0, 300))
  check('the code left the address bar', !String(await phone.evaluate('location.href')).includes('#'), await phone.evaluate('location.href'))

  const listed = String(await drive.capture('Settings: the paired phone', () => drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return [...(section?.querySelectorAll('.lc-phone__device') ?? [])].map((row) => row.innerText.replace(/\\s+/g, ' ')).join(' | ')`))))
  // Named by its browser and system since 0.830 (a headless Chrome on Windows here), with its Full window switch.
  check('Settings lists the phone, open now, with a Remove', /Chrome on Windows Open now Full window Remove/.test(listed), listed)

  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  const thread = await phoneWait(/PELICAN.*PELICAN/)
  await phone.capture('the phone: the conversation')
  check('opened, it shows what was asked and the answer', /Write the word PELICAN in capitals and nothing else\./.test(thread) && /Mira PELICAN/.test(thread), thread.slice(0, 300))

  const NEXT = 'Now write the word HARBOR in capitals and nothing else. Do not use any tool.'
  await phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(NEXT)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
  const answered = await phoneWait(/Now write the word HARBOR[^]*HARBOR(?![^]*Working…)/, 300)
  await phone.capture('the phone: sent through the relay, and answered')
  check('a message sent from the phone through the relay is answered there', /Now write the word HARBOR[^]*HARBOR/.test(answered) && !/Working…/.test(answered), answered.slice(-300))

  const desk = String(await drive.capture('the window: the phone’s message is the person’s own', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /PELICAN/i.test(r.innerText))?.click()
    for (let i = 0; i < 60; i += 1) {
      const said = ${THREAD}
      if (said.includes('Do not use any tool.') && /HARBOR/.test(said.slice(said.indexOf('Do not use any tool.') + 20))) break
      await new Promise((r) => setTimeout(r, 1000))
    }
    return ${THREAD}
  })()`)))
  check('the window shows the phone’s message, then the answer, in the same conversation', /Now write the word HARBOR/.test(desk) && /HARBOR/.test(desk.slice(desk.indexOf('Do not use any tool.') + 20)), desk.slice(-300))

  // A card: Mira, approving each action, is asked from the phone to make a file.
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  await phoneWait(/HARBOR/, 15)
  const MAKE = 'Create a file named anywhere-check.txt in this folder containing the word OK. Use your file-writing tool.'
  await phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(MAKE)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
  await sleep(1500)
  await phone.evaluate(`document.querySelector('.back')?.click()`)
  const asking = await phoneWait(/Waiting on you/i, 240)
  await phone.capture('the phone: a card waiting on you')
  const raised = /Waiting on you/i.test(asking)
  check('a card the run raises shows on the phone, with who asks', raised && /Mira asks/.test(asking), asking.slice(0, 300))
  if (raised) {
    await phone.evaluate(`[...document.querySelectorAll('button.act')].find((b) => b.innerText.trim() === 'Allow once')?.click()`)
    await sleep(2500)
    await phone.capture('the phone: allowed once, through the relay')
    let made = false
    for (let i = 0; i < 90 && !made; i += 1) {
      made = await access(join(workspace, 'anywhere-check.txt')).then(() => true, () => false)
      if (!made) {
        await phone.evaluate(`[...document.querySelectorAll('button.act')].find((b) => b.innerText.trim() === 'Allow once')?.click()`)
        await sleep(2000)
      }
    }
    const content = made ? (await readFile(join(workspace, 'anywhere-check.txt'), 'utf8')).trim() : '(no file)'
    check('allowed from the phone through the relay, the action ran: the file is there', made && /OK/.test(content), content)
  }

  await phone.reload()
  const after = await phoneWait(/Conversations/i)
  check('reloaded, the phone is still paired: its keys stayed in the browser', /Conversations/i.test(after) && !/Pair this phone/.test(after), after.slice(0, 200))

  await drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250))
    // Remove by its name: the row has a Full window switch before it since 0.830.
    ;[...(section?.querySelectorAll('.lc-phone__device button') ?? [])].find((b) => b.innerText.trim() === 'Remove')?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return 1`))
  const removed = await phoneWait(/Pair this phone/, 15)
  await phone.capture('the phone: removed in Settings')
  check('Remove in Settings stops the phone at once, and it says why', /Locust removed this phone/.test(removed), removed.slice(0, 200))

  const again = JSON.parse(String(await drive.capture('Settings: Pair a phone, a new code', () => drive.evaluate(settingsScript(`
    ;[...(section?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Pair a phone')?.click()
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__qr'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ text: ${SECTION}, url: settings.code?.url ?? '' })`)))))
  check('Pair a phone makes a new code', again.url !== '' && again.url !== on.url, again.text.slice(0, 200))
  await phone.navigate(again.url)
  const repaired = await phoneWait(/Conversations/i, 20)
  check('the new code, opened in the same tab, pairs the phone again', /Conversations/i.test(repaired), repaired.slice(0, 200))

  const offText = String(await drive.capture('Your phone, off again', () => drive.evaluate(settingsScript(`
    section?.querySelector('[role="switch"]')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return ${SECTION}`))))
  const unpaired = await phoneWait(/no longer paired/, 30)
  await phone.capture('the phone: unpaired when it was turned off')
  check('off, every phone is unpaired: the relay forgot the room', /^Your phone Off\./.test(offText.trim()) && /This phone is no longer paired/.test(unpaired), `${offText.slice(0, 120)} / ${unpaired.slice(0, 160)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  phone?.close()
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}. A seeded conversation; the phone is headless Chrome at 390x844 on the code's address; its messages run on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
