// Does the approval card show the change?
//
//   node _tools/probe-approval-diff.mjs
//
// One Codex run through the BUILT app: a seeded teammate on Codex CLI in
// "Approve each action" mode is asked to create a file. The approval card
// that stops the run must carry the change itself -- the file, the +1, the
// line -- read off the screen. Then Deny, and the file must not exist.
// The refusal is what makes this safe to run on a real account.

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath, teammateFace } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9293
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail = '') => {
  if (ok) say(`  [PASS] ${label}${detail ? ` -- ${detail}` : ''}`)
  else { failures += 1; say(`  [FAIL] ${label}${detail ? ` -- ${detail}` : ''}`) }
}
try {
  const already = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(1500) })
  if (already.ok) { say(`  [FAIL] something is already debugging on port ${String(PORT)}`); process.exit(1) }
} catch { /* free */ }

const workspace = await mkdtemp(join(tmpdir(), 'locust-approval-ws-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
const profile = await mkdtemp(join(tmpdir(), 'locust-approval-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
const T0 = '2026-09-05T05:00:00.000Z'
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: { runtime: 'codex', model: 'account-default', mode: 'approve-each' } }],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 6, memoryMode: 'off' }
}))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
child.stdout.on('data', () => undefined)
child.stderr.on('data', () => undefined)
try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) => new Promise((resolve_) => {
    const next = ++id
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 400_000)
    pending.set(next, (message) => {
      clearTimeout(gaveUp)
      const thrown = message.result?.exceptionDetails
      if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 200))
      resolve_(message.result?.result?.value)
    })
    socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })

  say('1. the app starts')
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  check('discovery finished', ready === true)
  if (ready !== true) say(`       body: ${String(await evaluate(`document.body.innerText.replace(/\\s+/g, ' ').slice(0, 400)`))}`)

  say('2. Wren on Codex, approve-each, is asked to create a file')
  const mode = await evaluate(`(async () => {
    const who = ${teammateFace('Wren')}
    who.click()
    await new Promise(r => setTimeout(r, 500))
    const control = document.querySelector('button[title="Permission mode"]')
    return control ? control.innerText.replace(/\\s+/g, ' ').trim() : 'no mode control'
  })()`)
  check('the composer shows Approve each action', /Approve each action/.test(String(mode)), String(mode))
  const card = await evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Using your file-editing tool (apply_patch), create a new file named SMOKE.txt in this directory containing exactly the line: smoke ok. Do not run shell commands and do not ask me anything first.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); break }
    }
    for (let i = 0; i < 480; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      const approval = document.querySelector('[role=group][aria-label="Approval required"]')
      if (approval) {
        await new Promise(r => setTimeout(r, 300))
        const patch = approval.querySelector('.lc-approval__patch')
        return JSON.stringify({
          summary: [...approval.querySelectorAll('dd')].map(d => d.innerText)[0],
          patch: patch ? patch.innerText.replace(/\\s+/g, ' ').trim().slice(0, 300) : null,
          files: [...approval.querySelectorAll('.lc-filerow__path, .lc-diff__path, [class*=diff][class*=path]')].map(n => n.innerText),
          adds: [...approval.querySelectorAll('.lc-diff__addmark')].map(n => n.innerText)
        })
      }
      if (!document.querySelector('button[aria-label^="Stop the running"]')) return JSON.stringify({ ended: true })
    }
    return JSON.stringify({ timeout: true })
  })()`)
  let c = {}
  try { c = JSON.parse(String(card)) } catch { /* below */ }
  say(`       ${String(card).slice(0, 400)}`)
  check('an approval card stopped the run', c.summary !== undefined, String(card).slice(0, 120))
  check('the card names the change: 1 file', /Change 1 file/.test(String(c.summary)), String(c.summary))
  check('the card carries the diff with the new line', c.patch !== null && /SMOKE\.txt/.test(String(c.patch)) && /smoke ok/.test(String(c.patch)), String(c.patch))
  check('and counts one added line', Array.isArray(c.adds) && c.adds.some((a) => /\+1\\b/.test(a)), JSON.stringify(c.adds))

  say('3. Deny, and nothing is written')
  const denied = await evaluate(`(async () => {
    // Codex asks again after a refusal (a retry, or a command to check the
    // folder). Every ask is refused, up to a bound, until the run ends.
    let denials = 0
    for (let i = 0; i < 600; i += 1) {
      const approval = document.querySelector('[role=group][aria-label="Approval required"]')
      const deny = approval && [...approval.querySelectorAll('button')].find(b => b.innerText.trim() === 'Deny' && !b.disabled)
      if (deny && denials < 6) { deny.click(); denials += 1; await new Promise(r => setTimeout(r, 800)); continue }
      if (!document.querySelector('button[aria-label^="Stop the running"]')) return 'ended after ' + denials + ' denial(s)'
      await new Promise(r => setTimeout(r, 500))
    }
    return 'still running after ' + denials + ' denial(s)'
  })()`)
  check('the run ended after the refusals', /^ended/.test(String(denied)), String(denied))
  check('SMOKE.txt was not created', !existsSync(join(workspace, 'SMOKE.txt')))
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
if (failures > 0) { say(`\n${String(failures)} APPROVAL DIFF PROBE FAILURE(S)`); process.exit(1) }
say('\nAPPROVAL DIFF PROBE PASSED')
