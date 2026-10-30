// Your phone (0.775): Settings shows a code; the phone's page lists the conversations, opens one, sends the next
// message (the window draws it as the person's own), answers a card, stays paired after a reload, and is unpaired
// by New code and by turning it off.
//
//   node _tools/drive-your-phone.mjs [--packaged <exe>] [--tag <name>]
//
// The phone is a headless Chrome at a phone's size with a profile of its own, on the address the code holds. The
// conversation is SEEDED (Mira answered PELICAN); the message the phone sends runs on the free OpenCode model, so
// nothing is spent. Turning the page on makes Locust listen on the network: Windows may ask whether it can, once.

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
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const PHONE_PORT = 9776

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-phone-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-phone-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// Approve each action: a file she makes raises a card, which the phone answers.
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-15T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }
const missionId = 'mission_5e000000-0000-4000-8000-000775000000'
const runId = 'run_5e0775'
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
  name: `phone-${tag}`,
  port: 9775,
  workspace,
  profilePath,
  outPath: join(recordRoot('your-phone-2026-10-15'), tag),
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
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-phone-chrome-'))
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
const THREAD = `(document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ')`

try {
  await drive.capture('launch: Mira answered PELICAN', () => drive.ready())
  await drive.resize(1215, 860)

  const off = String(await drive.capture('Settings > General: Your phone, off', () => drive.evaluate(settingsScript(`
    await new Promise((r) => setTimeout(r, 400))
    return (section?.innerText ?? 'no section').replace(/\\s+/g, ' ')`))))
  check('Settings has Your phone, off until it is turned on', /Your phone/.test(off) && /^Your phone Off\./.test(off.trim()), off)

  say('Turning the page on: Windows may ask whether Locust can use the network (that choice is Colin’s).')
  const on = JSON.parse(String(await drive.capture('Your phone, on: the code', () => drive.evaluate(settingsScript(`
    // Since 0.826 From anywhere is the first choice; this drive is the home-network way.
    ;[...(section?.querySelectorAll('[role="radio"]') ?? [])].find((b) => b.innerText.trim() === 'Only on this Wi-Fi')?.click()
    await new Promise((r) => setTimeout(r, 300))
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__qr'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.settings()
    return JSON.stringify({ text: (section?.innerText ?? '').replace(/\\s+/g, ' '), qr: section?.querySelector('.lc-phone__qr path')?.getAttribute('d')?.length ?? 0, url: settings.url ?? '' })`)))))
  check('on, Settings shows a code to scan and says what it is for', on.qr > 500 && /Scan the code with your phone’s camera/.test(on.text), `${on.text.slice(0, 300)} (qr path ${String(on.qr)})`)
  check('the code holds this computer’s home-network address and a key', /^http:\/\/\d+\.\d+\.\d+\.\d+:\d+\/#[A-Za-z0-9_-]{32}$/.test(on.url), on.url.replace(/#.*/, '#…'))

  await openPhone(on.url)
  const list = await phoneWait(/PELICAN/)
  await phone.capture('the phone: conversations')
  // innerText reads the heading as styled, in capitals.
  check('the phone lists the conversation by what was asked, with its answer', /Conversations/i.test(list) && /Write the word PELICAN/.test(list) && /Mira · done/.test(list), list.slice(0, 300))
  check('the key left the address bar', !String(await phone.evaluate('location.href')).includes('#'), await phone.evaluate('location.href'))

  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  const thread = await phoneWait(/PELICAN.*PELICAN/)
  await phone.capture('the phone: the conversation')
  check('opened, it shows what was asked and the answer, by the teammate’s name', /Write the word PELICAN in capitals and nothing else\./.test(thread) && /Mira PELICAN/.test(thread), thread.slice(0, 300))

  const NEXT = 'Now write the word HARBOR in capitals and nothing else. Do not use any tool.'
  await phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(NEXT)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
  // Answered: past the message, HARBOR again, and nothing still working.
  const answered = await phoneWait(/Now write the word HARBOR[^]*HARBOR(?![^]*Working…)/, 300)
  await phone.capture('the phone: sent, and answered')
  check('a message sent from the phone is answered there', /Now write the word HARBOR[^]*HARBOR/.test(answered) && !/Working…/.test(answered), answered.slice(-300))

  await sleep(2000)
  const desk = String(await drive.capture('the window: the phone’s message is the person’s own', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()
    await new Promise((r) => setTimeout(r, 600))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /PELICAN/i.test(r.innerText))?.click()
    // Until the answer is in: HARBOR after the message, and nothing starting or working.
    for (let i = 0; i < 90; i += 1) {
      const said = ${THREAD}
      const after = said.slice(said.indexOf('Do not use any tool.') + 20)
      if (said.includes('Do not use any tool.') && /HARBOR/.test(after) && !/Starting…|Working/.test(after)) break
      await new Promise((r) => setTimeout(r, 1000))
    }
    return ${THREAD}
  })()`)))
  check('the window shows the phone’s message as the person’s, then the answer, in the same conversation', /Now write the word HARBOR/.test(desk) && /HARBOR/.test(desk.slice(desk.indexOf('Do not use any tool.') + 20)), desk.slice(-400))
  const rows = String(await drive.evaluate(`[...document.querySelectorAll('.lc-convrow')].map((r) => r.innerText.replace(/\\s+/g, ' ')).join(' | ')`))
  check('it is still one conversation in the sidebar', (rows.match(/PELICAN/gi) ?? []).length === 1, rows)

  // A card: Mira, approving each action, is asked from the phone to make a file.
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  await phoneWait(/HARBOR/, 15)
  const MAKE = 'Create a file named phone-check.txt in this folder containing the word OK. Use your file-writing tool.'
  await phone.evaluate(`(() => { const box = document.querySelector('textarea'); box.value = ${JSON.stringify(MAKE)}; box.dispatchEvent(new Event('input')); document.querySelector('form.compose button').click() })()`)
  await sleep(1500)
  await phone.evaluate(`document.querySelector('.back')?.click()`)
  const asking = await phoneWait(/Waiting on you/i, 240)
  await phone.capture('the phone: a card waiting on you')
  const raised = /Waiting on you/i.test(asking)
  check('a card the run raises shows on the phone, with who asks', raised && /Mira asks/.test(asking), asking.slice(0, 300))
  if (raised) {
    // The window's card (ApprovalCard.tsx): a group named for what it is.
    const deskCard = Number(await drive.evaluate(`document.querySelectorAll('[role="group"][aria-label="Approval required"]').length`))
    check('the same card is in the window', deskCard > 0, `${String(deskCard)} card(s)`)
    await phone.evaluate(`[...document.querySelectorAll('button.act')].find((b) => b.innerText.trim() === 'Allow once')?.click()`)
    await sleep(2500)
    await phone.capture('the phone: allowed once')
    const left = Number(await drive.capture('the window: the card answered from the phone is gone', () => drive.evaluate(`document.querySelectorAll('[role="group"][aria-label="Approval required"]').length`)))
    check('answered on the phone, the window’s card comes down', deskCard > 0 && left === 0, `${String(deskCard)} before, ${String(left)} after`)
    let made = false
    for (let i = 0; i < 90 && !made; i += 1) {
      made = await access(join(workspace, 'phone-check.txt')).then(() => true, () => false)
      if (!made) {
        // A second card (a model that asks twice) is answered the same way.
        await phone.evaluate(`[...document.querySelectorAll('button.act')].find((b) => b.innerText.trim() === 'Allow once')?.click()`)
        await sleep(2000)
      }
    }
    const content = made ? (await readFile(join(workspace, 'phone-check.txt'), 'utf8')).trim() : '(no file)'
    check('allowed from the phone, the action ran: the file is there', made && /OK/.test(content), content)
  }

  await phone.reload()
  const after = await phoneWait(/Conversations/i)
  check('reloaded, the phone is still paired', /Conversations/i.test(after) && !/Pair this phone/.test(after), after.slice(0, 200))

  await drive.evaluate(settingsScript(`
    await new Promise((r) => setTimeout(r, 400))
    ;[...(section?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'New code')?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return 1`))
  const unpaired = await phoneWait(/Pair this phone/, 15)
  await phone.capture('the phone: unpaired by New code')
  check('New code unpairs the phone: it asks to scan again', /Pair this phone with Locust/.test(unpaired), unpaired.slice(0, 200))

  const fresh = String(await drive.evaluate('window.desktop.phone.settings().then((s) => s.url ?? "")'))
  await phone.navigate(fresh)
  const repaired = await phoneWait(/Conversations/i, 15)
  check('the new code, opened in the same tab, pairs it again', /Conversations/i.test(repaired), repaired.slice(0, 200))

  const offText = String(await drive.capture('Your phone, off again', () => drive.evaluate(settingsScript(`
    await new Promise((r) => setTimeout(r, 400))
    section?.querySelector('[role="switch"]')?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return (section?.innerText ?? '').replace(/\\s+/g, ' ')`))))
  const reach = await fetch(new URL('/', fresh), { signal: AbortSignal.timeout(3000) }).then((r) => String(r.status)).catch(() => 'no answer')
  check('off, the page is gone: nothing answers at its address', /^Your phone Off\./.test(offText.trim()) && reach === 'no answer', `${offText.slice(0, 120)} / ${reach}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  phone?.close()
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded conversation; the phone is headless Chrome at 390x844 on the code's address; its message runs on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
