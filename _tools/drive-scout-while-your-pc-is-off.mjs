// Scout while your computer is off (0.829): Settings adds Scout's away look to the folder's GitHub repository by a
// pull request; merged, it is on. With Locust closed, the away look runs, finds a new issue and leaves it at the relay;
// the phone, paired from anywhere, shows it under "While your computer is off". Locust opened again puts it on Home,
// told already, and What Scout saw has the look. Off undoes it: the secret goes and the workflow is turned off.
//
//   node _tools/drive-scout-while-your-pc-is-off.mjs [--packaged <exe>] [--tag <name>] [--relay <address>]
//
// The repository is a throwaway private one, StopBanning/locust-away-test, read and written with the signed-in gh.
// With the local relay (apps/relay: npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local) the away look runs here
// with Locust closed, as the workflow would; with --relay https://app.locust.lol it runs as the real workflow on
// GitHub (`gh workflow run`). Scout reads with the free OpenCode model, so nothing is spent.

import { execFile, spawn } from 'node:child_process'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { APP_DIR, FREE_ROUTE, recordRoot, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const RELAY = (arg('--relay') ?? 'http://127.0.0.1:8790').replace(/\/+$/, '')
const LIVE = !/^http:\/\/127\.0\.0\.1/.test(RELAY)
const REPO = 'StopBanning/locust-away-test'
const CHROME = process.env.LOCUST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
const PHONE_PORT = 9788
const PORT = 9787

const gh = (args, options = {}) => new Promise((resolve) => {
  const child = execFile('gh', args, { windowsHide: true, timeout: 120_000, maxBuffer: 8 * 1024 * 1024, ...options }, (error, stdout, stderr) => resolve({ code: error ? (error.code ?? 1) : 0, stdout: String(stdout), stderr: String(stderr) }))
  if (options.input !== undefined) child.stdin?.end(options.input)
})

const reachable = await fetch(`${RELAY}/`, { signal: AbortSignal.timeout(5000) }).then((answer) => answer.ok, () => false)
if (!reachable) {
  say(`The relay at ${RELAY} does not answer. Start it in apps/relay: npx wrangler@4 dev --port 8790 --ip 127.0.0.1 --local`)
  process.exit(1)
}

/** The repository as a person left it: nothing of Scout's, no open pull request or issue of an earlier run. */
async function tidyRepo() {
  for (const pr of JSON.parse((await gh(['pr', 'list', '--repo', REPO, '--state', 'open', '--json', 'number'])).stdout || '[]')) await gh(['pr', 'close', String(pr.number), '--repo', REPO, '--delete-branch'])
  for (const issue of JSON.parse((await gh(['issue', 'list', '--repo', REPO, '--state', 'open', '--json', 'number'])).stdout || '[]')) await gh(['issue', 'close', String(issue.number), '--repo', REPO])
  await gh(['secret', 'delete', 'LOCUST_SCOUT', '--repo', REPO])
  // A merged look from an earlier run is taken out, so this one goes through the pull request again.
  for (const path of ['.github/workflows/locust-scout.yml', '.github/locust/scout-away.mjs']) {
    const sha = (await gh(['api', `repos/${REPO}/contents/${path}`, '--jq', '.sha'])).stdout.trim()
    if (/^[0-9a-f]{40}$/.test(sha)) await gh(['api', '-X', 'DELETE', `repos/${REPO}/contents/${path}`, '-f', `message=Tidy after the away drive: ${path}`, '-f', `sha=${sha}`])
  }
}
await tidyRepo()

const root = process.env.LOCUST_SCRATCH ?? tmpdir()
const workspace = join(await mkdtemp(join(root, 'locust-drive-away-ws-')), 'locust-away-test')
const cloned = await gh(['repo', 'clone', REPO, workspace])
if (cloned.code !== 0) { say(`could not clone ${REPO}: ${cloned.stderr}`); process.exit(1) }
const profilePath = await mkdtemp(join(root, 'locust-drive-away-profile-'))
const SCOUT = { teammateId: 'tm_scout', name: 'Scout', hue: 'teal', role: 'Custom', roleTitle: 'Keeps watch', createdAt: '2026-10-23T01:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }
await writeFile(join(profilePath, 'scout.json'), JSON.stringify({ settings: { on: true, teammateId: 'tm_scout', everyMinutes: 60, watches: { github: true, teammates: false, comments: false, check: false }, notify: false, waitWhileWorking: false }, cards: [], seen: [], lookIds: [], prs: [], notes: [] }))
const seed = { schemaVersion: 1, teammates: [SCOUT], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
const outPath = join(recordRoot('scout-while-your-pc-is-off-2026-10-23'), tag)

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}

// Kept: the same profile opens again once the computer is back.
let drive = await startDrive({ ...(packaged === undefined ? {} : { packaged }), name: `scout-away-${tag}`, port: PORT, workspace, profilePath, env: { LOCUST_RELAY: RELAY }, outPath, seed, keep: true })

/* The phone: a headless Chrome at a phone's size, over its own DevTools socket. */
let phone
async function openPhone(url) {
  const userData = await mkdtemp(join(tmpdir(), 'locust-drive-away-chrome-'))
  const chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${String(PHONE_PORT)}`, `--user-data-dir=${userData}`, '--no-first-run', '--no-default-browser-check', '--window-size=390,844', 'about:blank'], { stdio: 'ignore' })
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
    reload: async () => { await call('Page.reload', {}); await sleep(2500) },
    capture: async (label) => {
      shot += 1
      const data = (await call('Page.captureScreenshot', { format: 'png' })).result?.data
      if (data !== undefined) await writeFile(join(outPath, `phone-${String(shot).padStart(2, '0')}-${label.replace(/[^a-z0-9]+/gi, '-').slice(0, 50)}.png`), Buffer.from(data, 'base64'))
      say(`phone ${String(shot).padStart(2, '0')}. ${label}`)
    },
    close: () => { try { socket.close() } catch { /* gone */ } chrome.kill() }
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

// Opens Settings > General only when it is not open already, then the named section.
const settingsScript = (setting, body) => `(async () => {
  if (!document.querySelector('[data-setting="${setting}"]')) {
    if (!document.querySelector('.lc-settings__navitem')) {
      ;[...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Settings')?.click()
      await new Promise((r) => setTimeout(r, 900))
    }
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((one) => one.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 900))
  }
  const section = document.querySelector('[data-setting="${setting}"]')
  section?.scrollIntoView({ block: 'center' })
  ${body}
})()`
const AWAY_ROW = `[...(section?.querySelectorAll('.lc-settingline') ?? [])].find((line) => line.querySelector('h3')?.innerText.trim() === 'Look while my computer is off')`
const awayRow = (body = '') => settingsScript('scout', `
  let row = ${AWAY_ROW}
  for (let i = 0; i < 20 && !row; i += 1) { await new Promise((r) => setTimeout(r, 250)); row = ${AWAY_ROW} }
  row?.scrollIntoView({ block: 'center' })
  ${body}
  return (row?.innerText ?? 'no row').replace(/\\s+/g, ' ')`)

let issueNumber
try {
  await drive.capture('launch: Scout on, watching this folder’s GitHub', () => drive.ready())
  await drive.resize(1215, 860)

  const blocked = String(await drive.capture('Settings: Look while my computer is off, before the phone is on', () => drive.evaluate(awayRow())))
  check('it says the phone’s way from anywhere comes first, and cannot be turned on yet', /Turn on “Your phone, from anywhere” first/.test(blocked), blocked)

  // The phone, from anywhere: on, and paired.
  const code = JSON.parse(String(await drive.evaluate(settingsScript('phone', `
    section?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !(section?.querySelector('.lc-phone__qr') && /Connected to Locust’s relay/.test(section?.innerText ?? '')); i += 1) await new Promise((r) => setTimeout(r, 250))
    const settings = await window.desktop.phone.anywhere()
    return JSON.stringify({ url: settings.code?.url ?? '', connected: settings.connected })`))))
  check('the phone’s way from anywhere is on and connected', code.connected === true && code.url !== '', code.url.replace(/#p=.*/, '#p=…'))
  await openPhone(code.url)
  await phoneWait(/Conversations/i)
  await phone.capture('the phone: paired')

  const offRow = String(await drive.capture('Settings: Look while my computer is off, ready to turn on', () => drive.evaluate(awayRow())))
  check('then it says what it does, and can be turned on', /a workflow in this folder’s GitHub repository looks for Scout/.test(offRow) && !/first/.test(offRow), offRow)

  const waiting = String(await drive.capture('turned on: merge the pull request', () => drive.evaluate(awayRow(`
    row?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 120 && !/Merge the pull request|could not/.test(row?.innerText ?? ''); i += 1) await new Promise((r) => setTimeout(r, 500))`))))
  check('turned on, it asks for the pull request to be merged, with a button to open it', /Merge the pull request in StopBanning\/locust-away-test to start/.test(waiting) && /Open the pull request/.test(waiting), waiting)
  for (const [width, height] of [[960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    await drive.capture(`the row, waiting, at ${String(width)} by ${String(height)}`, () => drive.evaluate(awayRow()))
  }
  await drive.resize(1215, 860)

  const prs = JSON.parse((await gh(['pr', 'list', '--repo', REPO, '--state', 'open', '--json', 'number,title,headRefName,files'])).stdout || '[]')
  const pr = prs.find((one) => one.headRefName === 'locust/scout-away')
  check('GitHub has the pull request, with the workflow and the one file it runs', pr !== undefined && pr.files.map((file) => file.path).sort().join(',') === '.github/locust/scout-away.mjs,.github/workflows/locust-scout.yml', JSON.stringify(pr ?? prs))
  const secrets = (await gh(['secret', 'list', '--repo', REPO])).stdout
  check('and the repository has the LOCUST_SCOUT secret', /LOCUST_SCOUT/.test(secrets), secrets.trim())

  // The person merges it.
  const merged = await gh(['pr', 'merge', String(pr?.number ?? 0), '--repo', REPO, '--squash', '--delete-branch'])
  check('the pull request merges', merged.code === 0, merged.stderr.trim())
  const onRow = String(await drive.capture('merged: on', () => drive.evaluate(awayRow(`
    for (let i = 0; i < 90 && !/^Look while my computer is off On for/.test((row?.innerText ?? '').replace(/\\s+/g, ' ')); i += 1) await new Promise((r) => setTimeout(r, 1000))`))))
  check('Locust notices the merge: on, every two hours for a private repository, with its cost', /On for StopBanning\/locust-away-test/.test(onRow) && /every two hours/.test(onRow) && /360 of the 2,000 free Actions minutes/.test(onRow) && /No away look yet/.test(onRow), onRow)

  // The phone seen again asks for the away key; Locust closes: the computer is off.
  await phone.reload()
  await phoneWait(/Conversations/i)
  await sleep(1500)
  const kept = JSON.parse(await readFile(join(profilePath, 'scout-away.json'), 'utf8'))
  const relayFile = JSON.parse(await readFile(join(profilePath, 'phone-relay.json'), 'utf8'))
  await drive.finish({ intro: 'First launch: Scout on; the phone paired from anywhere; the away look turned on and its pull request merged.' })

  const off = await phoneWait(/While your computer is off/i, 30)
  await phone.capture('the phone: the computer is off')
  check('with the computer off, the phone says so, and that Scout looks from GitHub meanwhile', /Your computer is not connected/.test(off) && /While your computer is off/i.test(off) && /Nothing has come from it yet/.test(off), off.slice(0, 300))

  // Something happens on GitHub while the computer is off.
  const made = await gh(['issue', 'create', '--repo', REPO, '--title', 'Sign-in page shows a blank screen', '--body', 'Seen after the last deploy.'])
  issueNumber = /\/issues\/(\d+)/.exec(made.stdout)?.[1]
  check('an issue is opened on GitHub', issueNumber !== undefined, made.stdout.trim() || made.stderr.trim())

  let looked
  if (LIVE) {
    const ran = await gh(['workflow', 'run', 'Locust Scout', '--repo', REPO])
    check('the workflow is started on GitHub', ran.code === 0, ran.stderr.trim())
    await sleep(8000)
    const run = JSON.parse((await gh(['run', 'list', '--repo', REPO, '--workflow', 'Locust Scout', '--limit', '1', '--json', 'databaseId'])).stdout || '[]')[0]
    await gh(['run', 'watch', String(run?.databaseId ?? 0), '--repo', REPO, '--exit-status'], { timeout: 600_000 })
    looked = (await gh(['run', 'view', String(run?.databaseId ?? 0), '--repo', REPO, '--log'])).stdout
  } else {
    // As the workflow runs it: the one file, its settings, the repository, with Locust closed.
    const config = { v: 1, relay: relayFile.relay, room: kept.room, token: kept.token, key: kept.key, login: kept.login, tz: Intl.DateTimeFormat().resolvedOptions().timeZone }
    looked = await new Promise((resolve) => {
      execFile(process.execPath, [join(APP_DIR, 'resources', 'scout-away.mjs')], { env: { ...process.env, LOCUST_SCOUT: JSON.stringify(config), GH_REPO: REPO }, timeout: 120_000 }, (error, stdout, stderr) => resolve(`${String(stdout)}${String(stderr)}${error ? ` (exit ${String(error.code)})` : ''}`))
    })
  }
  check('the away look found the new issue, and its log names nothing from it', /Scout looked: 1 new thing\./.test(looked) && !/blank screen/.test(looked), looked.trim().split(/\r?\n/).filter((line) => /Scout|Locust/.test(line)).slice(-3).join(' / '))

  const shown = await phoneWait(/new issue/i, 60)
  await phone.capture('the phone: what Scout found while the computer is off')
  check('the phone shows what it found, with a link to the issue on GitHub', /Scout looked at StopBanning\/locust-away-test on GitHub/.test(shown) && /Sign-in page shows a blank screen/.test(shown) && /Open on GitHub/.test(shown), shown.slice(0, 400))
  const link = String(await phone.evaluate(`[...document.querySelectorAll('a.act')].map((a) => a.href).join(' ')`))
  check('and the link goes to that repository on GitHub only', new RegExp(`^https://github\\.com/${REPO}/issues/${String(issueNumber)}$`).test(link), link)

  // The computer is back.
  drive = await startDrive({ ...(packaged === undefined ? {} : { packaged }), name: `scout-away-${tag}-back`, port: PORT, workspace, profilePath, env: { LOCUST_RELAY: RELAY }, outPath, stepFrom: 20 })
  await drive.ready()
  await drive.resize(1215, 860)
  const card = String(await drive.capture('back: the card on Home, from while the computer was off', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 60; i += 1) {
      const card = [...document.querySelectorAll('[data-scout-card]')].find((one) => /blank screen/.test(one.innerText))
      if (card) return card.innerText.replace(/\\s+/g, ' ')
      await new Promise((r) => setTimeout(r, 1000))
    }
    return 'no card'
  })()`)))
  check('Home has the card, marked as found while the PC was off', /blank screen/.test(card) && /while your PC was off/.test(card), card)
  const state = JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then((s) => JSON.stringify(s))')))
  check('one card for it, not two', state.cards.filter((one) => /blank screen/.test(one.said)).length === 1, state.cards.map((one) => one.said).join(' | '))
  const log = JSON.parse(String(await drive.evaluate('window.desktop.scout.log().then((l) => JSON.stringify(l))')))
  const away = log.entries.filter((entry) => entry.away === true)
  check('What Scout saw has the away look, and what it raised', away.length >= 1 && away.some((entry) => entry.raised.some((item) => /blank screen/.test(item.said))), JSON.stringify(away.at(-1) ?? {}).slice(0, 300))
  const backRow = String(await drive.capture('Settings: the last away look', () => drive.evaluate(awayRow(`
    for (let i = 0; i < 20 && !/Last away look/.test(row?.innerText ?? ''); i += 1) { await new Promise((r) => setTimeout(r, 500)); row = ${AWAY_ROW} }`))))
  check('Settings says when the last away look was', /Last away look: (just now|\d+ min ago)\./.test(backRow), backRow)
  const back = await phoneWait(/Conversations/i, 30)
  check('the phone, with the computer back, shows Locust again', /Conversations/i.test(back) && !/While your computer is off/i.test(back), back.slice(0, 200))

  // Off: undone.
  const offAgain = String(await drive.capture('turned off', () => drive.evaluate(awayRow(`
    row?.querySelector('[role="switch"]')?.click()
    for (let i = 0; i < 60 && !/^Look while my computer is off When your computer is off/.test((row?.innerText ?? '').replace(/\\s+/g, ' ')); i += 1) { await new Promise((r) => setTimeout(r, 500)); row = ${AWAY_ROW} }`))))
  check('off, the row is back to how it began', /When your computer is off, a workflow in this folder’s GitHub repository/.test(offAgain), offAgain)
  const gone = (await gh(['secret', 'list', '--repo', REPO])).stdout
  const workflows = JSON.parse((await gh(['workflow', 'list', '--repo', REPO, '--all', '--json', 'name,state'])).stdout || '[]')
  check('the secret is gone, and the workflow is turned off', !/LOCUST_SCOUT/.test(gone) && workflows.find((one) => one.name === 'Locust Scout')?.state === 'disabled_manually', `${gone.trim() || '(no secrets)'} / ${JSON.stringify(workflows)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.stack ?? error.message : String(error)}`)
} finally {
  phone?.close()
  if (issueNumber !== undefined) await gh(['issue', 'close', issueNumber, '--repo', REPO])
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Relay: ${RELAY}${LIVE ? ' (the away look ran as the real workflow on GitHub)' : ' (the away look ran here, as the workflow would)'}. Repository: ${REPO}. Scout reads with ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` }).catch(() => undefined)
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
