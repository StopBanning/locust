// THE LAUNCH CLIP, TAKE BY TAKE (2026-10-31): the real app, filmed while this drive does what a person does.
// Take 3: the window at 1600x1000 painted at twice the pixels (the whole app forced to a scale of 2: an emulated
// scale paints sharp but the screencast still sends it at 1600x1000), so the cut can go in close and stay sharp; the
// boxes the cut frames its shots on, sampled as it films; and a phone beside it. The beats: Home with a team on four
// agents; this week's trending skills kept; a message to Kit, answered on a free model; the same conversation carried
// on from the phone, its answer arriving on both. (Take 2 asked Kit from the phone to build it in Approve each, for
// a card to answer there: Kilo has no per-action approvals, and the phone said so. The runtimes that do ask are paid
// or, tonight, out of free quota, and a drive stays on free models.)
//
//   node _tools/film-launch-clip.mjs [--take <name>] [--from <beat>] [--folder <path>]
//
// --folder puts the project somewhere that reads like a project in a tool row ("Searched for ~/work/weekend-app",
// not the film's scratch folder); it must not exist yet, and it is removed afterwards.
// --from 3 films from the message to Kit on: a retake of the last beats, for a cut that takes the first ones from an
// earlier take (the launch film reads up to two takes).
//
// Writes the footage (frames/, phone/), the boxes and the moments (take.json) to ~/locust-scratch/film/; the cut is
// the launch film in the site's teaser toolkit (teaser/launch.js), which reads take.json. The phone is a headless
// Chrome at a phone's size on the home-network address Your phone gives (nothing leaves this computer); Kit runs on
// Kilo's free model, so nothing is spent.
import { execFileSync, spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { basename, join } from 'node:path'
import { say, startDrive } from './drive-lib.mjs'
import { startFilming, startSampling } from './film-lib.mjs'

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const arg = (name, otherwise) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : otherwise)
const FROM = Number(arg('--from', '1'))
const TAKE = arg('--take', 'next')
const WINDOW = [1600, 1000]
const SCALE = 2
const PHONE = [390, 844]
const PHONE_PORT = 9777
const CHROME = process.env.LOCUST_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

const teammate = (id, name, hue, roleTitle, runtime, model, mode = 'ask') => ({ teammateId: id, name, hue, role: 'Custom', roleTitle, createdAt: '2026-10-31T12:00:00.000Z', route: { runtime, model, mode } })
const seed = {
  schemaVersion: 1,
  teammates: [
    teammate('tm_wrenclip0000000000000', 'Wren', 'lime', 'Builder', 'claude', 'opus'),
    teammate('tm_atlasclip000000000000', 'Atlas', 'blue', 'Reviewer', 'codex', 'gpt-6.1-sol'),
    teammate('tm_irisclip0000000000000', 'Iris', 'violet', 'Researcher', 'antigravity', 'gemini-4-argon'),
    teammate('tm_kitclip00000000000000', 'Kit', 'clay', 'Writer', 'kilo', 'kilo/kilo-auto/free')
  ],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
}

// A project folder with a name worth reading in the title bar; the footage lives beside it, out of the repository.
const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
const filmRoot = join(homedir(), 'locust-scratch', 'film', `launch-take-${TAKE}-${stamp}`)
const FOLDER = arg('--folder', undefined)
if (FOLDER !== undefined && existsSync(FOLDER)) throw new Error(`${FOLDER} is there already: --folder must be a new folder`)
const workspace = FOLDER ?? join(filmRoot, 'weekend-app')
await mkdir(workspace, { recursive: true })
await writeFile(join(workspace, 'index.html'), '<!doctype html>\n<title>Weekend App</title>\n<h1>Weekend App</h1>\n<p>Plan a weekend with friends: pick a place, split the costs, vote on the plan.</p>\n<button>Start a plan</button>\n', 'utf8')
await writeFile(join(workspace, 'README.md'), '# Weekend App\n\nA small web app for planning a weekend with friends.\n', 'utf8')
execFileSync('git', ['init', '-q'], { cwd: workspace })
execFileSync('git', ['add', '.'], { cwd: workspace })
execFileSync('git', ['-c', 'user.name=Film', '-c', 'user.email=film@locust.lol', 'commit', '-qm', 'Weekend App'], { cwd: workspace })

const drive = await startDrive({ name: `launch-clip-take-${TAKE}`, port: 9902, workspace, seed, holdCursor: false, keep: true, outPath: join(filmRoot, 'record'), extraArgs: [`--force-device-scale-factor=${String(SCALE)}`] })
const section = `document.querySelector('[data-setting="skills-from-github"]')`
const until = async (expression, seconds = 60) => {
  for (let n = 0; n < seconds * 4; n += 1) { if (await drive.evaluate(expression)) return true; await pause(250) }
  return false
}

/* The phone: a headless Chrome over its own DevTools socket, with what film-lib needs (send, on) to film it too. */
let chrome
async function openPhone(url) {
  chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${String(PHONE_PORT)}`, `--user-data-dir=${join(filmRoot, 'phone-chrome')}`, '--no-first-run', '--no-default-browser-check', '--hide-scrollbars', `--force-device-scale-factor=${String(SCALE)}`, `--window-size=${String(PHONE[0])},${String(PHONE[1])}`, 'about:blank'], { stdio: 'ignore' })
  let target
  for (let tries = 0; tries < 40 && target === undefined; tries += 1) {
    await pause(500)
    target = await fetch(`http://127.0.0.1:${String(PHONE_PORT)}/json/list`).then((r) => r.json()).then((list) => list.find((one) => one.type === 'page')).catch(() => undefined)
  }
  if (target === undefined) throw new Error('the phone (headless Chrome) did not start')
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
  let id = 0
  const waiting = new Map()
  const listeners = new Set()
  socket.onmessage = (message) => {
    const data = JSON.parse(String(message.data))
    if (data.id !== undefined && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id); return }
    for (const listener of listeners) listener(data)
  }
  const send = (method, params = {}) => new Promise((resolve) => { id += 1; waiting.set(id, resolve); socket.send(JSON.stringify({ id, method, params })) })
  await send('Emulation.setDeviceMetricsOverride', { width: PHONE[0], height: PHONE[1], deviceScaleFactor: SCALE, mobile: true })
  await send('Page.enable')
  await send('Page.navigate', { url })
  await pause(2500)
  return {
    send,
    on: (listener) => { listeners.add(listener); return () => { listeners.delete(listener) } },
    evaluate: async (expression) => (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value,
    close: () => { try { socket.close() } catch { /* gone */ } chrome?.kill() }
  }
}
const PAGE = `(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`
let phone
const phoneWait = async (pattern, seconds = 30) => {
  const deadline = Date.now() + seconds * 1000
  let seen = ''
  while (Date.now() < deadline) {
    seen = String(await phone.evaluate(PAGE))
    if (pattern.test(seen)) return seen
    await pause(400)
  }
  return seen
}

// The boxes the cut frames its shots on, in the window's own pixels: [x, y, w, h], or null when not on screen.
const BOXES = `JSON.stringify((() => {
  const box = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 ? [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)] : null }
  const q = (s) => document.querySelector(s)
  const shelf = q('[data-setting="skills-from-github"]')
  const tile = (text) => [...(shelf?.querySelectorAll('.lc-skilllib__tile') ?? [])].find((one) => one.innerText.includes(text))
  const heading = [...(shelf?.querySelectorAll('.lc-skilllib__name') ?? [])].find((one) => one.innerText === 'New this week')
  const column = q('.lc-thread__column')
  const parts = column ? [...column.children].filter((one) => one.getBoundingClientRect().height > 4) : []
  return {
    team: box(q('.lc-hometeam')), starts: box(q('.lc-homestarts')), dock: box(q('form.command-dock')), faces: box(q('.lc-faces')),
    kit: box([...document.querySelectorAll('.lc-faces__one')].find((one) => /Kit/.test(one.getAttribute('aria-label') ?? ''))),
    week: box(heading), tstack: box(tile('t3dotgg/tstack')), unslop: box(tile('unslop')), rust: box(tile('BeautifulRust')), mac: box(tile('bama')),
    preview: box(shelf?.querySelector('.lc-skilllib__preview')), actions: box(shelf?.querySelector('.lc-skilllib__actions')),
    keep: box(shelf?.querySelector('.lc-skilllib__actions .lc-primarybutton')), kept: box(shelf?.querySelector('.lc-skilllib__kept')),
    last: box(parts.at(-1)), prev: box(parts.at(-2)), prev2: box(parts.at(-3)),
    card: box(q('[role="group"][aria-label="Approval required"]')), send: box(q('button[aria-label="Send"]'))
  }
})())`
const PHONE_BOXES = `JSON.stringify((() => {
  const box = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 ? [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)] : null }
  return {
    allow: box([...document.querySelectorAll('button.act')].find((one) => one.innerText.trim() === 'Allow once')),
    compose: box(document.querySelector('textarea')), send: box(document.querySelector('form.compose button')), convo: box(document.querySelector('button.convo'))
  }
})())`

const marks = {}
const mark = (name) => { marks[name] = Date.now() / 1000; say(`mark ${name}`) }
let deskFilm, phoneFilm, deskBoxes, phoneBoxes
try {
  await drive.ready()
  await drive.resize(...WINDOW, SCALE)
  const painted = String(await drive.evaluate('devicePixelRatio + " " + innerWidth + "x" + innerHeight'))
  say(`window: ${painted}`)
  if (!painted.startsWith(`${String(SCALE)} `)) throw new Error(`the window paints at ${painted}, not at a scale of ${String(SCALE)}`)
  await pause(1500)

  // Off camera: Your phone, on this Wi-Fi (Settings > General), then back to Home.
  const url = String(await drive.evaluate(`(async () => {
    if (!document.querySelector('[data-setting="phone"]')) {
      if (!document.querySelector('.lc-settings__navitem')) {
        ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
        await new Promise((r) => setTimeout(r, 900))
      }
      ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
      await new Promise((r) => setTimeout(r, 900))
    }
    const phone = document.querySelector('[data-setting="phone"]')
    ;[...(phone?.querySelectorAll('[role="radio"]') ?? [])].find((one) => one.innerText.trim() === 'Only on this Wi-Fi')?.click()
    await new Promise((r) => setTimeout(r, 300))
    phone?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 40 && !phone?.querySelector('.lc-phone__qr'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.settings()
    return settings.url ?? ''
  })()`))
  if (!/^http:\/\/\d+\.\d+\.\d+\.\d+:\d+\/#/.test(url)) throw new Error(`Your phone gave no home-network address: ${url.replace(/#.*/, '#…')}`)
  await drive.evaluate(`[...document.querySelectorAll('button')].find((one) => /^(Back|Close settings)$/.test(one.innerText.trim()))?.click()`)
  if (!(await until(`!!document.querySelector('.lc-hometeam')`, 5))) {
    await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find((one) => /Settings/.test(one.innerText))?.click()`)
    await until(`!!document.querySelector('.lc-hometeam')`, 5)
  }
  phone = await openPhone(url)
  say(`phone: ${(await phoneWait(/Conversations/i, 20)).slice(0, 120)}`)
  await pause(2000)

  deskFilm = await startFilming(drive, join(drive.out, 'frames'))
  phoneFilm = await startFilming(phone, join(drive.out, 'phone'))
  deskBoxes = startSampling(drive.evaluate, BOXES)
  phoneBoxes = startSampling(phone.evaluate, PHONE_BOXES)

  // 1. Home: the team, each on its own agent.
  if (FROM <= 1) {
    mark('home')
    await pause(5500)
  }

  // 2. The trending skills, kept for every agent.
  if (FROM <= 2) {
    mark('settings')
    await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find(b => /Settings/.test(b.innerText))?.click()`)
    await until(`!!document.querySelector('.lc-settings__navitem')`)
    await drive.evaluate(`[...document.querySelectorAll('.lc-settings__navitem')].find(b => b.innerText.trim() === 'Teammates')?.click()`)
    await until(`!!${section}`)
    mark('shelf')
    await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__name')].find(el => el.innerText === 'New this week')?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
    await pause(2200)
    mark('week')
    await pause(2200)
    mark('tstack')
    await drive.evaluate(`[...${section}.querySelectorAll('.lc-skilllib__tile')].find(tile => tile.innerText.includes('t3dotgg/tstack')).querySelector('.lc-skilllib__pack').click()`)
    await until(`!!${section}.querySelector('.lc-skilllib__actions .lc-primarybutton')`, 90)
    mark('preview')
    await drive.evaluate(`${section}.querySelector('.lc-skilllib__preview')?.scrollIntoView({ behavior: 'smooth', block: 'start' })`)
    await pause(2200)
    await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`)
    await pause(1200)
    mark('keep')
    await drive.evaluate(`${section}.querySelector('.lc-skilllib__actions .lc-primarybutton').click()`)
    await until(`${section}.querySelectorAll('.lc-skilllib__kept').length === 1`, 240)
    await drive.evaluate(`${section}.querySelector('.lc-skilllib__kept')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`)
    mark('kept')
    await pause(3000)
  }

  // 3. A message to Kit; the answer streams in.
  mark('kit')
  await drive.evaluate(`[...document.querySelectorAll('.lc-faces__one')].find((one) => /Kit/.test(one.getAttribute('aria-label') ?? ''))?.click()`)
  await until(`(document.querySelector('form.command-dock textarea')?.getAttribute('placeholder') ?? '').startsWith('Message Kit')`, 20)
  await pause(800)
  const ask = 'Our app plans weekends with friends. Suggest one feature to build next, in two sentences.'
  mark('type')
  await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    field.focus()
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    const text = ${JSON.stringify(ask)}
    for (let i = 1; i <= text.length; i += 1) {
      setter.call(field, text.slice(0, i))
      field.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise((r) => setTimeout(r, 26))
    }
    return true
  })()`)
  mark('typed')
  await pause(500)
  mark('send')
  await drive.evaluate(`document.querySelector('button[aria-label="Send"]')?.click()`)
  let started = Date.now()
  await pause(3000)
  while (Date.now() - started < 90_000 && await drive.evaluate(`!!document.querySelector('button[aria-label^="Stop the running"]')`)) await pause(400)
  mark('answered')
  const said = String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(-400) ?? ''`))
  say(`answer: ${said}`)
  if (/Internal error|stopped with an error/i.test(said)) say('DESK TURN FAILED: retake')
  await pause(2500)

  // 4. The same conversation, carried on from the phone: the answer arrives on both.
  mark('phone')
  await phone.evaluate(`document.querySelector('button.convo')?.click()`)
  await phoneWait(/Suggest one feature/, 15)
  await pause(1800)
  const next = 'Nice. What would its first screen show? Three short bullets.'
  mark('phoneType')
  // The box is asked for afresh at every key: the page redraws the conversation as it polls, and the box with it
  // (take 3 typed into a box already gone, and sent half the message).
  const typedOnPhone = String(await phone.evaluate(`(async () => {
    const text = ${JSON.stringify(next)}
    for (let i = 1; i <= text.length; i += 1) {
      const box = document.querySelector('textarea')
      box.value = text.slice(0, i)
      box.dispatchEvent(new Event('input'))
      await new Promise((r) => setTimeout(r, 34))
    }
    return document.querySelector('textarea').value
  })()`))
  if (typedOnPhone !== next) say(`the phone's box holds "${typedOnPhone}", not the whole message`)
  await pause(500)
  mark('phoneSend')
  await phone.evaluate(`document.querySelector('form.compose button')?.click()`)
  await pause(2500)
  // Answered on the phone: past the message, an answer, and nothing still working.
  let answered = ''
  for (const deadline = Date.now() + 120_000; Date.now() < deadline; await pause(500)) {
    answered = String(await phone.evaluate(PAGE))
    const after = answered.slice(answered.lastIndexOf('Three short bullets.') + 20)
    if (/stopped with an error/i.test(after)) {
      say('PHONE TURN FAILED: retake')
      break
    }
    if (answered.includes('Three short bullets.') && !/Working…|Starting…/.test(after) && after.replace(/\bSend\b/g, '').trim().length > 40) break
  }
  mark('phoneAnswered')
  say(`phone: ${answered.slice(-400)}`)
  started = Date.now()
  while (Date.now() - started < 20_000 && await drive.evaluate(`!!document.querySelector('button[aria-label^="Stop the running"]')`)) await pause(400)
  mark('deskAnswered')
  await pause(2500)
  mark('end')
} catch (error) { say(error instanceof Error ? error.stack ?? error.message : String(error)) }
finally {
  try {
    const rects = await deskBoxes?.stop()
    const phoneRects = await phoneBoxes?.stop()
    const desk = await deskFilm?.stop()
    const shots = await phoneFilm?.stop()
    if (desk?.length) {
      // Every time in take.json is in the film's own seconds: since the window's first frame.
      const t0 = desk[0].at
      const rel = (at) => Math.round((at - t0) * 1000) / 1000
      const version = JSON.parse(await readFile(new URL('../apps/desktop/package.json', import.meta.url), 'utf8')).version
      const take = {
        made: new Date().toISOString(), version, window: WINDOW, phoneWindow: PHONE, scale: SCALE,
        desktop: desk.map((frame) => [`frames/${basename(frame.file)}`, rel(frame.at)]),
        phone: (shots ?? []).map((frame) => [`phone/${basename(frame.file)}`, rel(frame.at)]),
        rects: (rects ?? []).map((sample) => [rel(sample.at), sample.value]),
        phoneRects: (phoneRects ?? []).map((sample) => [rel(sample.at), sample.value]),
        marks: Object.fromEntries(Object.entries(marks).map(([name, at]) => [name, rel(at)]))
      }
      await writeFile(join(drive.out, 'take.json'), `${JSON.stringify(take)}\n`, 'utf8')
      say(`take: ${String(desk.length)} window frames, ${String(take.phone.length)} phone frames, ${String(take.rects.length)} box samples over ${(desk.at(-1).at - t0).toFixed(1)} s`)
      say(`marks ${JSON.stringify(take.marks)}`)
      say(`take.json: ${join(drive.out, 'take.json')}`)
    }
  } catch (error) { say(`saving the take: ${error instanceof Error ? error.message : String(error)}`) }
  phone?.close()
  await drive.finish({ intro: `Launch clip, take ${TAKE}` })
  if (FOLDER !== undefined) await rm(FOLDER, { recursive: true, force: true })
}
