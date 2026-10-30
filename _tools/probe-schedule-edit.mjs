// Can a person change their mind about a schedule?
//
//   node _tools/probe-schedule-edit.mjs <profile> <workspace>
//
// Drives a profile that already holds one routine (a kept schedule-smoke
// profile does): Edit it, see the choice it was saved with, switch it to
// daily at 07:30, save, read the card and the file; Edit again, switch it
// off, save, and see the schedule gone from both. Read-only on the runtimes:
// nothing is run.

import { spawn } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { homedir } from 'node:os'
import { localPath } from './local-path.mjs'

const [profile, workspace] = process.argv.slice(2)
if (!profile || !workspace) {
  console.error('usage: node _tools/probe-schedule-edit.mjs <profile> <workspace>')
  process.exit(2)
}
const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9297
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

// Start from a known schedule whatever the profile's last run left behind.
{
  const path = join(profile, 'routines.json')
  const file = JSON.parse(await readFile(path, 'utf8'))
  file.routines[0] = { ...file.routines[0], schedule: { kind: 'every', hours: 4 } }
  await writeFile(path, `${JSON.stringify(file, null, 2)}
`, 'utf8')
}

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
  const consoleErrors = []
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (message.method === 'Runtime.exceptionThrown') consoleErrors.push(message.params?.exceptionDetails?.text ?? 'exception')
    if (message.method === 'Runtime.consoleAPICalled' && message.params?.type === 'error') consoleErrors.push(String(message.params.args?.[0]?.value ?? 'console.error'))
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const send = (method, params = {}) => new Promise((resolve_) => {
    const next = ++id
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 60_000)
    pending.set(next, (message) => { clearTimeout(gaveUp); resolve_(message) })
    socket.send(JSON.stringify({ id: next, method, params }))
  })
  const evaluate = async (expression) => {
    const message = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    const thrown = message?.result?.exceptionDetails
    if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 300))
    return message?.result?.result?.value
  }
  await send('Runtime.enable')

  say('1. the app starts on the kept profile')
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  check('discovery finished', ready === true)

  const openEdit = () => evaluate(`(async () => {
    if (!document.querySelector('.lc-routinerow')) {
      const team = [...document.querySelectorAll('button')].find(b => b.getAttribute('title') === 'Team (Ctrl 2)')
      team.click()
      await new Promise(r => setTimeout(r, 800))
    }
    const edit = [...document.querySelectorAll('.lc-routinerow button')].find(b => b.innerText.trim() === 'Edit')
    if (!edit) return 'no Edit button'
    edit.click()
    await new Promise(r => setTimeout(r, 500))
    const dialog = document.querySelector('[role=dialog][aria-label="Edit routine"]')
    if (!dialog) return 'dialog did not open'
    const checked = [...dialog.querySelectorAll('[role=radio]')].filter(b => b.getAttribute('aria-checked') === 'true').map(b => b.innerText.trim())
    const hours = dialog.querySelector('select[aria-label="Hours between runs"]')
    const time = dialog.querySelector('input[aria-label="Time of day"]')
    return JSON.stringify({ checked, hours: hours ? hours.value : null, time: time ? time.value : null })
  })()`)
  const cardText = () => evaluate(`(async () => {
    await new Promise(r => setTimeout(r, 600))
    return [...document.querySelectorAll('.lc-routinerow')].map(r => r.innerText.replace(/\\s+/g, ' ')).join(' | ')
  })()`)
  const routineOnDisk = async () => JSON.parse(await readFile(join(profile, 'routines.json'), 'utf8')).routines?.[0]

  say('2. Edit shows the schedule it was saved with')
  const first = await openEdit()
  let parsed = {}
  try { parsed = JSON.parse(String(first)) } catch { /* below */ }
  check('"Every few hours" is the checked choice, hours reads 4', parsed.checked?.[0] === 'Every few hours' && parsed.hours === '4', String(first))

  say('3. switch to daily at 07:30 and save')
  const daily = await evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog][aria-label="Edit routine"]')
    const choice = [...dialog.querySelectorAll('[role=radio]')].find(b => /^Daily( at a time)?$/.test(b.innerText.trim()))
    choice.click()
    await new Promise(r => setTimeout(r, 300))
    const time = dialog.querySelector('input[aria-label="Time of day"]')
    if (!time) return 'no time input'
    const before = time.value
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
    setter.call(time, '07:30')
    time.dispatchEvent(new Event('input', { bubbles: true }))
    time.dispatchEvent(new Event('change', { bubbles: true }))
    await new Promise(r => setTimeout(r, 200))
    const after = time.value
    const save = [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Save changes')
    if (!save || save.disabled) return 'save disabled'
    save.click()
    await new Promise(r => setTimeout(r, 800))
    return JSON.stringify({ before, after, closed: !document.querySelector('[role=dialog][aria-label="Edit routine"]') })
  })()`)
  let d = {}
  try { d = JSON.parse(String(daily)) } catch { /* below */ }
  check('the time defaulted to 09:00, took 07:30, and the dialog closed', d.before === '09:00' && d.after === '07:30' && d.closed === true, String(daily))
  const cardDaily = await cardText()
  check('the card reads "daily at 07:30 · next …07:30"', /daily at 07:30 · next (tomorrow )?07:30/.test(String(cardDaily)), String(cardDaily).slice(0, 200))
  const diskDaily = await routineOnDisk()
  check('the file says daily at 07:30', diskDaily?.schedule?.kind === 'daily' && diskDaily?.schedule?.at === '07:30', JSON.stringify(diskDaily?.schedule))
  check('the run count and last run survived the edit', diskDaily?.runs === 1 && typeof diskDaily?.lastRunAt === 'string', JSON.stringify({ runs: diskDaily?.runs }))

  say('3b. pressing the choice already made keeps the time')
  const again = await openEdit()
  const pressed = await evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog][aria-label="Edit routine"]')
    const choice = [...dialog.querySelectorAll('[role=radio]')].find(b => /^Daily( at a time)?$/.test(b.innerText.trim()))
    choice.click()
    await new Promise(r => setTimeout(r, 200))
    const time = dialog.querySelector('input[aria-label="Time of day"]')
    const kept = time ? time.value : null
    const cancel = [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Cancel')
    cancel.click()
    await new Promise(r => setTimeout(r, 400))
    return kept
  })()`)
  check('Daily pressed twice still reads 07:30', /07:30/.test(String(again)) && pressed === '07:30', String(pressed))

  say('4. Edit again: it shows daily, switch it off, save')
  const second = await openEdit()
  let p2 = {}
  try { p2 = JSON.parse(String(second)) } catch { /* below */ }
  check('"Daily at a time" is checked and the time reads 07:30', /^Daily( at a time)?$/.test(p2.checked?.[0] ?? '') && p2.time === '07:30', String(second))
  const off = await evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog][aria-label="Edit routine"]')
    const choice = [...dialog.querySelectorAll('[role=radio]')].find(b => /^(Only when|When) I press Run$/.test(b.innerText.trim()))
    choice.click()
    await new Promise(r => setTimeout(r, 300))
    const note = [...dialog.querySelectorAll('p')].some(p => /Only while Locust is open/.test(p.innerText))
    const save = [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Save changes')
    save.click()
    await new Promise(r => setTimeout(r, 800))
    return JSON.stringify({ noteGone: !note, closed: !document.querySelector('[role=dialog][aria-label="Edit routine"]') })
  })()`)
  let o = {}
  try { o = JSON.parse(String(off)) } catch { /* below */ }
  check('the limits note left with the schedule and the dialog closed', o.noteGone === true && o.closed === true, String(off))
  const cardOff = await cardText()
  check('the card no longer names a schedule', !/every|daily|next|due now/.test(String(cardOff)), String(cardOff).slice(0, 200))
  const diskOff = await routineOnDisk()
  check('the file has no schedule, and still the run count', diskOff?.schedule === undefined && diskOff?.runs === 1, JSON.stringify({ schedule: diskOff?.schedule, runs: diskOff?.runs }))

  say('5. Cancel changes nothing')
  const cancelled = await evaluate(`(async () => {
    if (!document.querySelector('.lc-routinerow')) {
      const team = [...document.querySelectorAll('button')].find(b => b.getAttribute('title') === 'Team (Ctrl 2)')
      team.click()
      await new Promise(r => setTimeout(r, 600))
    }
    const edit = [...document.querySelectorAll('.lc-routinerow button')].find(b => b.innerText.trim() === 'Edit')
    edit.click()
    await new Promise(r => setTimeout(r, 400))
    const dialog = document.querySelector('[role=dialog][aria-label="Edit routine"]')
    const choice = [...dialog.querySelectorAll('[role=radio]')].find(b => /Every few hours/.test(b.innerText))
    choice.click()
    await new Promise(r => setTimeout(r, 200))
    const cancel = [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Cancel')
    cancel.click()
    await new Promise(r => setTimeout(r, 500))
    return [...document.querySelectorAll('.lc-routinerow')].map(r => r.innerText.replace(/\\s+/g, ' ')).join(' | ')
  })()`)
  check('after Cancel the card still has no schedule', !/every|daily/.test(String(cancelled)), String(cancelled).slice(0, 200))
  check('the file still has no schedule', (await routineOnDisk())?.schedule === undefined)

  say('6. console')
  check('no renderer errors during the probe', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '))
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
}
if (failures > 0) { say(`\n${String(failures)} SCHEDULE EDIT PROBE FAILURE(S)`); process.exit(1) }
say('\nSCHEDULE EDIT PROBE PASSED')
