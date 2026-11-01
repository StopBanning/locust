// Settings in a browser (0.849): every Settings page, drawn by the window on this computer and by Locust in a browser
// (app.locust.lol/desk, paired as a phone is), photographed side by side, with a table of what each page holds: the
// notes that say a setting works on the computer, the controls a person can press, and any "could not be read, saved
// or changed". Then Plans is turned off from the browser, and this computer's settings are read to see it landed.
// Colin found four dead controls in Safari by hand (2026-11-01); this is the sweep that finds them instead.
// The chat box too (0.850): a file attached in the browser must arrive in the folder on the computer, Choose a folder
// and Talk are held there, and the microphone says where its setup is done. And Send feedback opens the issue in the
// browser itself (0.853).
//
//   node _tools/drive-settings-in-a-browser.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// The same local relay and downloads stand-in as drive-locust-in-a-browser.mjs; start both first:
//   node _tools/window-zip.mjs <version> --out ~/work/scratch/desk-downloads/window/Locust-<version>-window.zip
//   node _tools/serve-window-zip.mjs ~/work/scratch/desk-downloads --port 8791
//   (apps/relay) npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local --var DESK_DOWNLOADS:http://127.0.0.1:8791/
// The browser is a headless Chrome (LOCUST_CHROME names it) at a laptop's size with a profile of its own. Nothing is
// sent to a model.

import { spawn } from 'node:child_process'
import { mkdtemp, readdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'http://127.0.0.1:8790').replace(/\/+$/, '')
const CHROME = process.env.LOCUST_CHROME ?? (process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : 'C:/Program Files/Google/Chrome/Application/chrome.exe')
const BROWSER_PORT = 9799

/** Settings' pages, as its own list names them, read from the window, so a page added later is swept too (0.850). */
let PAGES = []
const pagesScript = `(async () => {
  if (!document.querySelector('.lc-settings__navitem')) {
    ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-settings__navitem'); i += 1) await new Promise((r) => setTimeout(r, 250))
  }
  return JSON.stringify([...document.querySelectorAll('.lc-settings__navitem')].map((one) => one.innerText.trim()).filter(Boolean))
})()`
/** A changelog quotes the failures it says were fixed: its page is checked for holding nothing back only. */
const CHANGELOG = /^What/
/** Pressed in the browser: not ones that only open a link or another page. */
const NOT_PRESSED = /^(Repository:|What’s new|What's new|Show older versions)/
/** What a control held for the computer says (shared/desk-calls.ts WINDOW_ONLY_SAYS). */
const SAYS = 'This works in Locust on your computer.'
/** The chat box, from wherever the window is: Home, by the logo (Ctrl+1 opens the conversations list instead). */
const chatBoxScript = `(async () => {
  for (let i = 0; i < 3 && !document.querySelector('.lc-plusmenu__main'); i += 1) {
    document.querySelector('button[aria-label="Home"]')?.click()
    for (let j = 0; j < 12 && !document.querySelector('.lc-plusmenu__main'); j += 1) await new Promise((r) => setTimeout(r, 250))
  }
  const folder = document.querySelector('[data-satellite="folder"]')
  const talk = document.querySelector('.lc-call__button')
  return JSON.stringify({
    box: document.querySelector('.lc-plusmenu__main') !== null,
    picker: document.querySelector('input.lc-filepick[type="file"]') !== null,
    folderHeld: folder?.disabled === true,
    folderTitle: folder?.getAttribute('title') ?? '',
    talk: talk === null ? 'none' : { held: talk.disabled, label: talk.getAttribute('aria-label') }
  })
})()`

const reachable = await fetch(`${RELAY}/desk/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} has no /desk/. Start it in apps/relay (see this file's header).`)
  process.exit(1)
}

const workspace = await scratchRepository('locust-drive-settings-desk-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-settings-desk-profile-'))
const ASH = { teammateId: 'tm_ash0000000000000000000', name: 'Ash', hue: 'clay', role: 'Custom', roleTitle: 'Helper', createdAt: '2026-11-01T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'accept-edits' } }
const outPath = join(recordRoot('settings-in-a-browser-2026-11-01'), tag)
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `settings-desk-${tag}`,
  port: 9796,
  workspace,
  profilePath,
  env: { LOCUST_RELAY: RELAY },
  outPath,
  sendsNothing: true,
  seed: { schemaVersion: 1, teammates: [ASH], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', keepATodoList: true } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* The browser: a headless Chrome over its own DevTools socket (as drive-locust-in-a-browser.mjs). */
let browser
async function openBrowser(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-settings-chrome-'))
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
  socket.onmessage = (message) => {
    const data = JSON.parse(String(message.data))
    if (data.id !== undefined && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id) }
  }
  const call = (method, params = {}) => new Promise((resolve) => { id += 1; waiting.set(id, resolve); socket.send(JSON.stringify({ id, method, params })) })
  await call('Emulation.setDeviceMetricsOverride', { width: 1280, height: 820, deviceScaleFactor: 1, mobile: false })
  await call('Page.enable')
  await call('Runtime.enable')
  await call('Page.navigate', { url })
  await sleep(2500)
  let shot = 0
  browser = {
    call,
    evaluate: async (expression) => (await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value,
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
    seen = String(await browser.evaluate(PAGE))
    if (pattern.test(seen)) return seen
    await sleep(1000)
  }
  return seen
}
const phoneScript = (body) => `(async () => {
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

/** One Settings page opened, and what it holds: words that say a setting works on the computer, controls, failures. */
const pageScript = (label) => `(async () => {
  if (!document.querySelector('.lc-settings__navitem')) {
    // Its shortcut, whatever the sidebar's layout: a press on Appearance may have chosen the rail, where it is an icon.
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '3', ctrlKey: true, bubbles: true }))
    for (let i = 0; i < 20 && !document.querySelector('.lc-settings__navitem'); i += 1) await new Promise((r) => setTimeout(r, 250))
  }
  const item = [...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith(${JSON.stringify(label)}))
  if (!item) return JSON.stringify({ missing: true })
  item.click()
  await new Promise((r) => setTimeout(r, 2500))
  const pane = document.querySelector('.lc-settings__pane') ?? document.body
  pane.scrollTop = 0
  const controls = [...pane.querySelectorAll('button, input, select, textarea, [role="switch"]')].filter((one) => one.offsetParent !== null || one.getClientRects().length > 0)
  const off = (one) => one.disabled === true || one.closest('fieldset[disabled]') !== null
  const text = (pane.innerText ?? '').replace(/\\s+/g, ' ')
  return JSON.stringify({
    notes: pane.querySelectorAll('.lc-oncomputer__said').length,
    pressable: controls.filter((one) => !off(one)).length,
    pressed: controls.filter((one) => !off(one)).map((one) => (one.getAttribute('aria-label') ?? one.innerText ?? one.value ?? '').replace(/\\s+/g, ' ').trim().slice(0, 40)),
    held: controls.filter(off).length,
    broken: (text.match(/[^.]*could not be (read|saved|changed)[^.]*\\./gi) ?? []).slice(0, 4)
  })
})()`

/** Every window.desktop function, nested groups too, wrapped to note a refusal that says it works on the computer. */
const listenScript = `(async () => {
  const SAYS = 'This works in Locust on your computer.'
  if (window.__refused === undefined) {
    window.__refused = []
    const wrap = (holder, path, depth) => {
      for (const key of Object.keys(holder)) {
        const value = holder[key]
        if (typeof value === 'function') {
          try {
            holder[key] = function (...args) {
              const answer = value.apply(this, args)
              if (answer && typeof answer.then === 'function') answer.then(undefined, (error) => { if (error?.message === SAYS) window.__refused.push(path + key) })
              return answer
            }
          } catch { /* fixed */ }
        } else if (value && typeof value === 'object' && depth < 3) wrap(value, path + key + '.', depth + 1)
      }
    }
    wrap(window.desktop, '', 0)
  }
  // Proven once (0.850, the PC chat's review): a call a browser is always refused, made here, must be noted. A
  // window.desktop that could not be wrapped would otherwise let every press pass by noting nothing.
  const before = window.__refused.length
  await window.desktop.writeWorkspaceSettings({}).catch(() => undefined)
  await new Promise((r) => setTimeout(r, 50))
  return window.__refused.slice(before).includes('writeWorkspaceSettings') ? 'listening' : 'not listening'
})()`
/** Each pressable control on the page, pressed in turn: which ones made a call a browser is refused. */
const pressScript = `(async () => {
  const pane = document.querySelector('.lc-settings__pane')
  if (pane === null) return JSON.stringify({ pressed: 0, refused: [], left: 'not on Settings' })
  const off = (one) => one.disabled === true || one.closest('fieldset[disabled]') !== null
  const label = (one) => (one.getAttribute('aria-label') ?? one.innerText ?? '').replace(/\\s+/g, ' ').trim().slice(0, 50)
  const controls = [...pane.querySelectorAll('button, [role="switch"], [role="radio"]')].filter((one) => !off(one) && !${NOT_PRESSED}.test(label(one)))
  const refused = []
  let left
  for (const one of controls) {
    if (document.querySelector('.lc-settings__pane') === null) { left = 'left Settings'; break }
    if (!one.isConnected || off(one)) continue
    const before = window.__refused.length
    one.click()
    await new Promise((r) => setTimeout(r, 700))
    if (window.__refused.length > before) refused.push(label(one) + ' -> ' + window.__refused.slice(before).join(', '))
    // A menu or dialog it opened is closed again, so the next control is the page's own.
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  }
  return JSON.stringify({ pressed: controls.length, refused, ...(left === undefined ? {} : { left }) })
})()`

const rows = []
try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1280, 820)

  // The window first: once the browser has Locust's window, this computer's window says so in its place.
  PAGES = JSON.parse(String(await drive.evaluate(pagesScript)))
  check('Settings lists its pages', PAGES.length > 5, PAGES.join(', '))
  const window = new Map()
  for (const label of PAGES) {
    const there = JSON.parse(String(await drive.capture(`window: ${label}`, () => drive.evaluate(pageScript(label)))))
    window.set(label, there)
    // What's new quotes the old failures it says were fixed, so only its holding nothing back is checked.
    check(`${label}, in the window: nothing says a setting could not be read, saved or changed, and nothing is held for a browser`, (CHANGELOG.test(label) || (there.broken?.length ?? 0) === 0) && there.notes === 0, JSON.stringify(there))
  }
  const box = JSON.parse(String(await drive.capture('window: the chat box', () => drive.evaluate(chatBoxScript))))
  check('the chat box, in the window: the computer’s own picker, and nothing held for a browser', box.box && !box.picker && !box.folderHeld && box.talk?.held !== true, JSON.stringify(box))

  const on = JSON.parse(String(await drive.evaluate(phoneScript(`
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ url: settings.code?.url ?? '' })`))))
  check('Your phone, from anywhere, is on with a code', on.url !== '', on.url)
  await openBrowser(on.url)
  check('the browser pairs with the code’s link', /Conversations/i.test(await browserWait(/Conversations/i)))
  const allowed = String(await drive.evaluate(phoneScript(`
    for (let i = 0; i < 40 && !section?.querySelector('.lc-phone__device'); i += 1) await new Promise((r) => setTimeout(r, 250))
    section?.querySelector('.lc-phone__device [role="switch"]')?.click()
    for (let i = 0; i < 40 && section?.querySelector('.lc-phone__device [role="switch"]')?.getAttribute('aria-checked') !== 'true'; i += 1) await new Promise((r) => setTimeout(r, 250))
    return (section?.innerText ?? '').replace(/\\s+/g, ' ')`)))
  check('Full window is on for that browser', /Full window/.test(allowed), allowed.slice(0, 200))
  await browser.navigate(`${RELAY}/desk/`)
  check('the browser opens the whole window', /Settings/.test(await browserWait(/[1-9]\d* agents? ready|Settings/, 90)))

  // Plans, turned off from the browser, is off on the computer.
  await browser.evaluate(pageScript('Teammates'))
  const pressed = String(await browser.evaluate(`(async () => {
    const plans = document.querySelector('[aria-label="Stop asking teammates to keep a todo list"]')
    if (!plans) return 'no Plans switch'
    if (plans.disabled || plans.closest('fieldset[disabled]')) return 'Plans is held on the computer'
    plans.click()
    await new Promise((r) => setTimeout(r, 2500))
    return plans.getAttribute('aria-checked') ?? 'no state'
  })()`))
  await browser.capture('browser: Plans turned off')
  const landed = await drive.evaluate(`window.desktop.readWorkspaceSettings().then((settings) => settings.keepATodoList)`)
  check('Plans turns off from the browser, and the computer keeps it', pressed === 'false' && landed === false, `browser says ${pressed}; the computer keeps ${String(landed)}`)

  // Then every page in the browser: what it holds, and each pressable control pressed, failing on a refusal.
  check('the browser notes every call refused because it stays on the computer', String(await browser.evaluate(listenScript)) === 'listening')
  for (const label of PAGES) {
    const here = JSON.parse(String(await browser.evaluate(pageScript(label))))
    await browser.capture(`browser: ${label}`)
    const pressed = CHANGELOG.test(label) ? { pressed: 0, refused: [] } : JSON.parse(String(await browser.evaluate(pressScript)))
    rows.push({ label, there: window.get(label) ?? {}, here, pressed })
    check(`${label}, in the browser: nothing says a setting could not be read, saved or changed`, here.missing !== true && (CHANGELOG.test(label) || (here.broken?.length ?? 0) === 0), JSON.stringify(here))
    check(`${label}, in the browser: every control it lets a person press works from here (${String(pressed.pressed)} pressed)`, pressed.refused.length === 0, pressed.refused.join('; '))
  }

  const table = ['| Page | Window: pressable | Browser: pressable | Browser: pressed | Browser: refused | Browser: held | Browser: notes |', '|---|---|---|---|---|---|---|',
    ...rows.map(({ label, there, here, pressed }) => `| ${label} | ${String(there.pressable ?? '-')} | ${String(here.pressable ?? '-')} | ${String(pressed.pressed)} | ${pressed.refused.length === 0 ? 'none' : pressed.refused.join('; ')} | ${String(here.held ?? '-')} | ${String(here.notes ?? '-')} |`)].join('\n')
  await writeFile(join(outPath, 'TABLE.md'), `${table}\n`, 'utf8')
  say(table)

  // The chat box in the browser (0.850).
  const here = JSON.parse(String(await browser.evaluate(chatBoxScript)))
  await browser.capture('browser: the chat box')
  check('the chat box, in the browser: the browser’s own picker, and Choose a folder held with the words', here.box && here.picker && here.folderHeld && here.folderTitle === SAYS, JSON.stringify(here))
  check('Talk, in the browser: held, or not drawn where this computer has no Talk', here.talk === 'none' || (here.talk.held === true && here.talk.label === 'Talk, on your computer'), JSON.stringify(here.talk))
  const picked = join(await mkdtemp(join(tmpdir(), 'locust-drive-attach-')), 'drive-attach.txt')
  await writeFile(picked, 'Sent from Locust in a browser.\n', 'utf8')
  const root = (await browser.call('DOM.getDocument', { depth: 1 })).result?.root?.nodeId
  const input = root === undefined ? undefined : (await browser.call('DOM.querySelector', { nodeId: root, selector: 'input.lc-filepick' })).result?.nodeId
  if (input !== undefined && input !== 0) await browser.call('DOM.setFileInputFiles', { nodeId: input, files: [picked] })
  const tile = await browserWait(/drive-attach/, 30)
  const attached = await readdir(join(workspace, '.locust', 'attachments')).catch(() => [])
  await browser.capture('browser: a file attached from the browser')
  check('Attach files, in the browser: the file picked there is on its tile, and in the folder on the computer', /drive-attach/.test(tile) && attached.some((name) => name.startsWith('drive-attach')), `on the computer: ${attached.join(', ') || 'nothing'}`)
  // And back (0.853): a copy of a file downloads into the browser, its bytes sent by the computer through the relay.
  const back = attached.find((name) => name.startsWith('drive-attach'))
  const copy = back === undefined ? { ok: false, message: 'nothing attached' } : JSON.parse(String(await browser.evaluate(`window.desktop.downloadFile(${JSON.stringify(join(workspace, '.locust', 'attachments', back))}).then((answer) => JSON.stringify(answer.ok ? { ok: true, name: answer.name, text: new TextDecoder().decode(answer.bytes) } : answer))`)))
  check('a copy, in the browser: the computer sends the file\'s bytes for the browser\'s downloads', copy.ok === true && copy.text === 'Sent from Locust in a browser.\n', JSON.stringify(copy).slice(0, 300))
  const mic = String(await browser.evaluate(`(async () => {
    const button = document.querySelector('.lc-voice__button')
    if (!button) return 'no microphone'
    button.click()
    for (let i = 0; i < 40 && !document.querySelector('.lc-voice__message'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return document.querySelector('.lc-voice__message')?.textContent ?? 'nothing said'
  })()`))
  await browser.capture('browser: the microphone says where its setup is done')
  check('the microphone, in the browser: says where its setup is done, not that it could not finish', /in Locust on your computer/.test(mic), mic)

  // Send feedback from the browser (0.853): the report is built there and opened in a new tab. window.open is
  // recorded rather than called, so nothing reaches GitHub.
  const sent = JSON.parse(String(await browser.evaluate(`(async () => {
    window.__opened = []
    window.open = (url) => { window.__opened.push(String(url)); return null }
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '3', ctrlKey: true, bubbles: true }))
    for (let i = 0; i < 20 && !document.querySelector('.lc-settings__navitem'); i += 1) await new Promise((r) => setTimeout(r, 250))
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 900))
    const open = [...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Send feedback')
    if (!open) return JSON.stringify({ step: 'no Send feedback button' })
    if (open.disabled || open.closest('fieldset[disabled]')) return JSON.stringify({ step: 'Send feedback is held' })
    open.click()
    await new Promise((r) => setTimeout(r, 1500))
    const area = document.querySelector('.lc-feedback__text')
    if (!area) return JSON.stringify({ step: 'no feedback box' })
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(area, 'Sent from the browser drive.')
    area.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    ;[...document.querySelectorAll('.lc-feedback button, .lc-dialog__foot button')].find((one) => one.innerText.trim() === 'Send')?.click()
    await new Promise((r) => setTimeout(r, 700))
    return JSON.stringify({ step: 'sent', opened: window.__opened, said: document.querySelector('.lc-feedback')?.innerText ?? '' })
  })()`)))
  await browser.capture('browser: Send feedback from the browser')
  const issue = new URL(sent.opened?.[0] ?? 'about:blank')
  check('Send feedback, in the browser: opens the GitHub issue there, with the words and the computer\'s version', sent.step === 'sent' && issue.href.startsWith('https://github.com/StopBanning/locust/issues/new') && (issue.searchParams.get('body') ?? '').includes('Sent from the browser drive.'), JSON.stringify(sent).slice(0, 400))
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  browser?.close()
  await drive.finish({ intro: 'Every Settings page in the window and in Locust in a browser, with what each holds back, Plans changed from the browser, and the chat box: a file attached from the browser, and what it holds back.' })
}
say(failures === 0 ? 'SETTINGS IN A BROWSER: PASSED' : `SETTINGS IN A BROWSER: ${String(failures)} check(s) failed`)
process.exit(failures === 0 ? 0 : 1)
