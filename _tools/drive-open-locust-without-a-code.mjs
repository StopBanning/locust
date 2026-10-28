// A browser opens Locust without a code to carry (0.838). Colin, 2026-10-28, of app.locust.lol/desk's "Pair this
// browser first": "w/o this slight friction". Three ways round it, driven end to end against a local relay:
//
//  1. The way in, not paired yet, says what Locust is and every way in: Sign in with GitHub, a code, Download Locust.
//  2. Settings' Open in your browser, with phones off: one click turns on From anywhere, pairs this computer's browser
//     and opens the whole window in it, already let have it.
//  3. Sign in with GitHub: Settings ties this computer to the account once; then a fresh browser that signs in to
//     GitHub opens the whole window with no code. The PC checks the account with GitHub itself: a relay that vouches
//     for one account while GitHub says another is refused. Signed out, no browser gets in that way.
//
//   node _tools/drive-open-locust-without-a-code.mjs [--packaged <exe>] [--tag <name>]
//
// GitHub is stood in for here, on 127.0.0.1:8792: nobody signs in to the real GitHub. Start first, as for
// drive-locust-in-a-browser.mjs, with the relay told of the stand-in:
//   node _tools/window-zip.mjs <version> --out D:/work/scratch/desk-downloads/window/Locust-<version>-window.zip
//   node _tools/serve-window-zip.mjs D:/work/scratch/desk-downloads --port 8791
//   (apps/relay) npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local --infer-origin-from-routes=false --var DESK_DOWNLOADS:http://127.0.0.1:8791/
//     --var GITHUB_CLIENT_ID:drive-client --var GITHUB_CLIENT_SECRET:drive-stand-in
//     --var GITHUB_WEB:http://127.0.0.1:8792 --var GITHUB_API:http://127.0.0.1:8792
// Locust's links to the person's browser are written down (LOCUST_SIGN_IN_PAGES), never opened in theirs.
// --infer-origin-from-routes=false keeps the relay at its own local address: without it, it thinks it is app.locust.lol
// and sends GitHub's stand-in back there.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = 'http://127.0.0.1:8790'
const GITHUB = 'http://127.0.0.1:8792'
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const BROWSER_PORT = 9799

/*
 * GITHUB, STOOD IN FOR. /login/oauth/authorize signs the browser straight in as whoever `as` is now, and sends it
 * back; the code is exchanged for a token; /user says whose a token is. `lie` makes the relay's question and the PC's
 * get different answers, as a relay that vouched for the wrong account would see.
 */
let as = { id: 42, login: 'octo' }
let lie = false
const tokens = new Map()
const asked = []
let codes = 0
const github = createServer((request, response) => {
  const url = new URL(request.url, GITHUB)
  asked.push(`${request.method} ${url.pathname}`)
  if (url.pathname === '/login/oauth/authorize') {
    codes += 1
    const code = `code${String(codes)}`
    tokens.set(code, { ...as })
    response.writeHead(302, { location: `${url.searchParams.get('redirect_uri')}?code=${code}&state=${url.searchParams.get('state')}` })
    response.end()
    return
  }
  if (url.pathname === '/login/oauth/access_token' && request.method === 'POST') {
    let body = ''
    request.on('data', (chunk) => { body += chunk })
    request.on('end', () => {
      const said = JSON.parse(body || '{}')
      const who = said.client_secret === 'drive-stand-in' ? tokens.get(said.code) : undefined
      if (who === undefined) { response.writeHead(200, { 'content-type': 'application/json' }); response.end('{"error":"bad_verification_code"}'); return }
      const token = `gho_${createHash('sha256').update(said.code).digest('hex').slice(0, 36)}`
      tokens.set(token, { ...who, calls: 0 })
      response.writeHead(200, { 'content-type': 'application/json' })
      response.end(JSON.stringify({ access_token: token, token_type: 'bearer', scope: '' }))
    })
    return
  }
  if (url.pathname === '/user') {
    const token = /^Bearer (.+)$/.exec(request.headers.authorization ?? '')?.[1]
    const who = token === undefined ? undefined : tokens.get(token)
    if (who === undefined) { response.writeHead(401); response.end('{}'); return }
    who.calls += 1
    const answer = lie && who.calls > 1 ? { id: 7, login: 'someone-else' } : who
    response.writeHead(200, { 'content-type': 'application/json' })
    response.end(JSON.stringify({ id: answer.id, login: answer.login }))
    return
  }
  response.writeHead(404)
  response.end()
})
await new Promise((resolve) => github.listen(8792, '127.0.0.1', resolve))

const reachable = await fetch(`${RELAY}/desk/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.text(), () => '')
if (!reachable.includes('var GITHUB = true;')) {
  say(`The relay at ${RELAY} has no /desk/ that offers GitHub. Start it in apps/relay with the stand-in's vars (see this file's header).`)
  github.close()
  process.exit(1)
}

const root = new URL('..', import.meta.url).pathname.slice(1)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-nocode-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-nocode-profile-'))
const pagesFile = join(profilePath, 'opened-for-the-person.txt')
await writeFile(pagesFile, '', 'utf8')
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Writer', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'accept-edits' } }
const missionId = 'mission_5e000000-0000-4000-8000-000838000000'
const runId = 'run_5e0838'
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

const outPath = join(recordRoot('open-locust-without-a-code-2026-10-28'), tag)
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `nocode-${tag}`,
  port: 9796,
  workspace,
  profilePath,
  sendsNothing: true,
  env: { LOCUST_RELAY: RELAY, LOCUST_GITHUB_API: GITHUB, LOCUST_SIGN_IN_PAGES: pagesFile },
  outPath,
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

/* A browser: a headless Chrome with a profile of its own each time, so each is a browser never paired before. */
let shot = 0
async function aBrowser(width = 1280, height = 820) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-nocode-chrome-'))
  const chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${String(BROWSER_PORT)}`, `--user-data-dir=${userData}`, '--no-first-run', '--no-default-browser-check', `--window-size=${String(width)},${String(height)}`, 'about:blank'], { stdio: 'ignore' })
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
  await call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 })
  await call('Page.enable')
  await call('Runtime.enable')
  const evaluate = async (expression) => (await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value
  return {
    evaluate,
    navigate: async (to) => { await call('Page.navigate', { url: to }); await sleep(2000) },
    text: async () => String(await evaluate(`(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`)),
    wait: async (pattern, seconds = 30) => {
      const deadline = Date.now() + seconds * 1000
      let seen = ''
      while (Date.now() < deadline) {
        seen = String(await evaluate(`(document.body?.innerText ?? '').replace(/\\s+/g, ' ')`).catch(() => ''))
        if (pattern.test(seen)) return seen
        await sleep(1000)
      }
      return seen
    },
    capture: async (label) => {
      shot += 1
      const data = (await call('Page.captureScreenshot', { format: 'png' })).result?.data
      if (data !== undefined) await writeFile(join(outPath, `browser-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
      say(`browser ${String(shot).padStart(2, '0')}. ${label}`)
    },
    close: async () => { try { socket.close() } catch { /* gone */ } chrome.kill(); await sleep(800) }
  }
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
  const press = (label) => [...(section?.querySelectorAll('button') ?? [])].find((one) => one.innerText.trim() === label)
  ${body}
})()`
const SECTION = `(section?.innerText ?? 'no section').replace(/\\s+/g, ' ')`
/** What Locust has written down for the person's browser, newest last. */
const opened = async () => (await readFile(pagesFile, 'utf8')).split('\n').filter(Boolean)
const nextOpened = async (count, seconds = 20) => {
  for (let i = 0; i < seconds * 4; i += 1) { const lines = await opened(); if (lines.length > count) return lines.at(-1); await sleep(250) }
  return undefined
}
const devices = async () => (await drive.evaluate('window.desktop.phone.anywhere().then((s) => JSON.stringify(s.devices))').then((text) => JSON.parse(String(text)), () => []))

try {
  await drive.capture('launch: Mira answered PELICAN', () => drive.ready())
  await drive.resize(1215, 860)

  // 1. The way in, not paired yet.
  let browser = await aBrowser()
  await browser.navigate(`${RELAY}/desk/`)
  const wayIn = await browser.wait(/Open Locust in this browser/)
  await browser.capture('the way in, never paired: every way in')
  check('the way in says what Locust is, with Sign in with GitHub, a code and Download Locust', /Locust runs on your computer/.test(wayIn) && /Sign in with GitHub/.test(wayIn) && /Open in your browser opens it there/.test(wayIn) && /Download Locust/.test(wayIn) && !/Pair this browser first/.test(wayIn), wayIn.slice(0, 500))
  await browser.close()
  browser = await aBrowser(390, 844)
  await browser.navigate(`${RELAY}/desk/`)
  await browser.wait(/Open Locust in this browser/)
  const narrow = String(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'))
  await browser.capture('the way in at a phone’s width')
  check('at a phone’s width it fits, with no sideways scroll', narrow === 'true', narrow)
  await browser.close()

  // 2. Open in your browser, with phones off.
  const before = (await opened()).length
  const offer = String(await drive.capture('Settings: phones off, Open in your browser offered', () => drive.evaluate(settingsScript(`return ${SECTION}`))))
  check('with phones off, Settings offers Open in your browser, saying it turns From anywhere on', /Or open Locust’s whole window in this computer’s browser\. It turns this on, from anywhere\./.test(offer), offer.slice(0, 400))
  await drive.evaluate(settingsScript(`press('Open in your browser')?.click(); return ''`))
  const deskCode = await nextOpened(before, 30)
  check('one click turns From anywhere on and hands this computer’s browser a code for the full window', /^http:\/\/127\.0\.0\.1:8790\/#p=[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\.[A-Za-z0-9_-]{22}\.d$/.test(deskCode ?? ''), deskCode)
  const opening = String(await drive.capture('Settings: opening in this computer’s browser', () => drive.evaluate(settingsScript(`return ${SECTION} + ' | qr ' + String(section?.querySelector('.lc-phone__qr') !== null)`))))
  check('while that code waits, Settings shows no code for a phone to scan', /Opening Locust in this computer’s browser/.test(opening) && /qr false$/.test(opening), opening.slice(0, 400))
  browser = await aBrowser()
  await browser.navigate(deskCode)
  const full = await browser.wait(/PELICAN/, 90)
  const where = String(await browser.evaluate('location.pathname'))
  await browser.capture('Open in your browser: the whole window, no other step')
  check('the browser pairs and goes straight on to the whole window', /^\/desk\/v\/\d+\.\d+\.\d+\/desk\.html$/.test(where) && /Write the word PELICAN/.test(full), `${where} ${full.slice(0, 200)}`)
  const first = await devices()
  check('Settings lists that browser with Full window already on', first.length === 1 && first[0].desk === true && first[0].github === undefined, JSON.stringify(first))
  await browser.close()
  // Take the window back here, for what follows.
  await drive.evaluate(`[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Use Locust here')?.click()`).catch(() => undefined)
  await sleep(2500)

  // 3. Sign in with GitHub: tie this computer once.
  const offered = String(await drive.capture('Settings: Sign in with GitHub, before', () => drive.evaluate(settingsScript(`return ${SECTION}`))))
  check('Settings offers Sign in with GitHub and says what is checked', /Sign in with GitHub, and any browser where you sign in to GitHub opens Locust/.test(offered) && /asks GitHub for no permissions/.test(offered), offered.slice(0, 500))
  const count = (await opened()).length
  await drive.evaluate(settingsScript(`press('Sign in with GitHub')?.click(); return ''`))
  const start = await nextOpened(count)
  check('Sign in with GitHub sends the person to the relay’s GitHub sign-in, with this computer’s link', /^http:\/\/127\.0\.0\.1:8790\/v1\/github\/start\?room=[A-Za-z0-9_-]{22}&link=[A-Za-z0-9_-]{43}$/.test(start ?? ''), start)
  browser = await aBrowser()
  await browser.navigate(start)
  const tiedPage = await browser.wait(/Signed in as octo/)
  await browser.capture('GitHub’s sign-in, from the computer: signed in as octo')
  check('GitHub signs the person in, and the page says what that now does', /Now any browser where you sign in to GitHub as octo opens Locust on /.test(tiedPage), tiedPage.slice(0, 300))
  await browser.close()
  const tied = String(await drive.capture('Settings: signed in with GitHub as octo', () => drive.evaluate(settingsScript(`
    for (let i = 0; i < 40 && !/as octo/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('Settings says which account lets a browser in, and offers Sign out of GitHub', /Any browser where you sign in to GitHub as octo opens Locust/.test(tied) && /Sign out of GitHub/.test(tied), tied.slice(0, 500))

  // A fresh browser, never paired: sign in, and the whole window opens.
  browser = await aBrowser()
  await browser.navigate(`${RELAY}/desk/`)
  await browser.wait(/Sign in with GitHub/)
  await browser.evaluate(`document.querySelector('a.go')?.click()`)
  const signedIn = await browser.wait(/PELICAN/, 90)
  const there = String(await browser.evaluate('location.pathname'))
  await browser.capture('a fresh browser, signed in with GitHub: the whole window')
  check('a browser never paired signs in with GitHub and opens the whole window, no code', /^\/desk\/v\/\d+\.\d+\.\d+\/desk\.html$/.test(there) && /Write the word PELICAN/.test(signedIn), `${there} ${signedIn.slice(0, 200)}`)
  const second = await devices()
  check('Settings lists it, signed in with GitHub, with Full window on', second.length === 2 && second[1].github === true && second[1].desk === true, JSON.stringify(second))
  await browser.close()
  await drive.evaluate(`[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Use Locust here')?.click()`).catch(() => undefined)
  await sleep(2500)
  const listed = String(await drive.capture('Settings: the browser that signed in with GitHub', () => drive.evaluate(settingsScript(`return ${SECTION}`))))
  check('and says so beside it', /signed in with GitHub/.test(listed), listed.slice(0, 500))

  // The relay vouches for octo; GitHub tells the PC otherwise: the PC does not let it in.
  lie = true
  browser = await aBrowser()
  await browser.navigate(`${RELAY}/desk/`)
  await browser.wait(/Sign in with GitHub/)
  await browser.evaluate(`document.querySelector('a.go')?.click()`)
  const lied = await browser.wait(/different GitHub account/, 45)
  await browser.capture('GitHub tells the computer another account: refused')
  check('when GitHub tells the computer a different account than the relay saw, the computer refuses it', /signed in with a different GitHub account \(octo\)/.test(lied), lied.slice(0, 300))
  check('and nothing more is paired', (await devices()).length === 2)
  await browser.close()
  lie = false

  // Another account, with no computer: the relay says so.
  as = { id: 7, login: 'someone-else' }
  browser = await aBrowser()
  await browser.navigate(`${RELAY}/desk/`)
  await browser.wait(/Sign in with GitHub/)
  await browser.evaluate(`document.querySelector('a.go')?.click()`)
  const stranger = await browser.wait(/No computer is signed in yet/)
  await browser.capture('another GitHub account: no computer for it')
  check('another GitHub account is told no Locust is signed in as it, and how to sign one in', /No Locust is signed in to GitHub as someone-else/.test(stranger) && /Download Locust/.test(stranger), stranger.slice(0, 300))
  await browser.close()
  as = { id: 42, login: 'octo' }

  // Signed out, octo's sign-in opens nothing.
  const out = String(await drive.capture('Settings: signed out of GitHub', () => drive.evaluate(settingsScript(`
    press('Sign out of GitHub')?.click()
    for (let i = 0; i < 40 && !/Sign in with GitHub, and any browser/.test(section?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${SECTION}`))))
  check('Sign out of GitHub goes back to offering Sign in with GitHub', /Sign in with GitHub, and any browser/.test(out), out.slice(0, 300))
  browser = await aBrowser()
  await browser.navigate(`${RELAY}/desk/`)
  await browser.wait(/Sign in with GitHub/)
  await browser.evaluate(`document.querySelector('a.go')?.click()`)
  const after = await browser.wait(/No computer is signed in yet/)
  await browser.capture('signed out: octo’s sign-in opens nothing')
  check('signed out, signing in as octo opens nothing', /No Locust is signed in to GitHub as octo/.test(after), after.slice(0, 300))
  await browser.close()
  check('the stand-in GitHub was asked only by the relay and the PC, never a real one', asked.length > 0 && asked.every((line) => /^(GET|POST) \/(login\/oauth\/(authorize|access_token)|user)$/.test(line)), asked.slice(0, 12).join(', '))

  // Off again, as it was.
  await drive.evaluate(settingsScript(`section?.querySelector('[role="switch"]')?.click(); return ''`))
  await sleep(2000)
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${String(error?.stack ?? error).slice(0, 800)}`)
} finally {
  github.close()
  await drive.finish({ intro: 'A browser opens Locust without a code to carry (0.838)', extra: `failures ${String(failures)}` })
}
process.exit(failures === 0 ? 0 : 1)
