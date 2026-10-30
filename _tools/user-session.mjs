// Drive the BUILT app the way a person would, and write down what was seen.
//
//   node _tools/user-session.mjs [--keep]
//
// Not a smoke: nothing here asserts. Each step does what a person does --
// open the app on a real repository, make a teammate from the dialog with
// Own branch on, send a message on the free model, read the reply, open
// Memory, Rooms, Settings, go home from the logo -- and records the screen
// as text plus a screenshot under docs/user-session/<stamp>/. The judging
// is done afterwards by reading the record, so a hitch is a thing that was
// seen, not a thing the script was told to look for.

import { spawn, execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath, FREE_ROW, FREE_SEARCH, teammateFace } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9291
const FREE_MODEL = FREE_SEARCH
const KEEP = process.argv.includes('--keep')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
const git = (args, cwd) => new Promise((resolve, reject) => {
  execFile('git', args, { cwd, windowsHide: true }, (error, stdout) => (error ? reject(error) : resolve(stdout)))
})
try {
  const already = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(1500) })
  if (already.ok) { say(`something is already debugging on port ${String(PORT)}`); process.exit(1) }
} catch { /* free */ }

const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
const OUT = join(localPath(new URL('../docs/user-session/', import.meta.url)), stamp)
await mkdir(OUT, { recursive: true })
const workspace = await mkdtemp(join(tmpdir(), 'locust-session-ws-'))
await git(['init', '-q', '-b', 'main'], workspace)
await git(['config', 'user.email', 'session@locust.test'], workspace)
await git(['config', 'user.name', 'Locust session'], workspace)
await writeFile(join(workspace, 'README.md'), '# scratch\n\nA scratch project for a user session.\n', 'utf8')
await writeFile(join(workspace, 'LOCUST.md'), 'Keep answers to one paragraph.\n', 'utf8')
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'first'], workspace)
const profile = await mkdtemp(join(tmpdir(), 'locust-session-'))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))
const record = []
let step = 0

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
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 400_000)
    pending.set(next, (message) => { clearTimeout(gaveUp); resolve_(message) })
    socket.send(JSON.stringify({ id: next, method, params }))
  })
  const evaluate = async (expression) => {
    const message = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    const thrown = message?.result?.exceptionDetails
    if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 200))
    return message?.result?.result?.value
  }
  await send('Runtime.enable')

  /** One step: do it, then keep the screen's text and picture. */
  const capture = async (title, action) => {
    step += 1
    let note
    try {
      note = await action()
    } catch (error) {
      note = `threw: ${error instanceof Error ? error.message : String(error)}`
    }
    await sleep(600)
    const text = await evaluate(`document.body.innerText`)
    const shot = await send('Page.captureScreenshot', { format: 'png' })
    const name = `${String(step).padStart(2, '0')}-${title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}`
    if (shot?.result?.data) await writeFile(join(OUT, `${name}.png`), Buffer.from(shot.result.data, 'base64'))
    await writeFile(join(OUT, `${name}.txt`), String(text ?? ''), 'utf8')
    record.push({ step, title, note: note ?? '', errors: consoleErrors.splice(0) })
    say(`${String(step).padStart(2, '0')}. ${title}${note ? ` -- ${String(note).slice(0, 160)}` : ''}`)
  }

  await capture('launch', () => evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return 'discovery finished; ' + (document.querySelector('.lc-connected')?.innerText ?? '')
      await new Promise(r => setTimeout(r, 250))
    }
    return 'discovery never finished'
  })()`))

  await capture('open new teammate dialog', () => evaluate(`(async () => {
    document.querySelector('button[aria-label="New teammate"]').click()
    await new Promise(r => setTimeout(r, 400))
    return document.querySelector('[role=dialog]') ? 'dialog open' : 'no dialog'
  })()`))

  await capture('name it Nova, Custom role with a title, Own branch on', () => evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog]')
    const input = dialog.querySelector('input')
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
    set.call(input, 'Nova'); input.dispatchEvent(new Event('input', { bubbles: true }))
    const custom = [...dialog.querySelectorAll('.lc-rolecard')].find(b => b.querySelector('.lc-rolecard__name')?.textContent.trim() === 'Custom')
    if (custom) custom.click()
    await new Promise(r => setTimeout(r, 200))
    const title = [...dialog.querySelectorAll('input')].find(i => /Release manager/.test(i.placeholder))
    if (title) { set.call(title, 'Release manager'); title.dispatchEvent(new Event('input', { bubbles: true })) }
    const own = dialog.querySelector('[role=switch][aria-label="Own copy"]')
    if (own) own.click()
    await new Promise(r => setTimeout(r, 200))
    return 'custom=' + !!custom + ' title=' + !!title + ' own=' + (own ? own.getAttribute('aria-checked') : 'no switch') + ' · ' + (dialog.querySelector('.lc-field--switch')?.innerText.replace(/\\s+/g, ' ').slice(0, 120) ?? '')
  })()`))

  await capture('create the teammate', () => evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog]')
    const create = [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Create teammate')
    if (!create || create.disabled) return 'create disabled'
    create.click()
    await new Promise(r => setTimeout(r, 800))
    return document.querySelector('.lc-sidebar').innerText.replace(/\\s+/g, ' ').slice(0, 200)
  })()`))

  await capture('pick Nova and choose the free OpenCode route', () => evaluate(`(async () => {
    const who = ${teammateFace('Nova')}
    if (!who) return 'no Nova row'
    who.click()
    await new Promise(r => setTimeout(r, 400))
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    control.click()
    let target
    for (let attempt = 0; attempt < 60 && !target; attempt += 1) {
      const picker = document.querySelector('.lc-picker')
      if (!picker) { control.click(); await new Promise(r => setTimeout(r, 500)); continue }
      const box = picker.querySelector('.lc-picker__input')
      const setInput = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setInput.call(box, ${JSON.stringify(FREE_MODEL)})
      box.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise(r => setTimeout(r, 500))
      let group = ''
      for (const node of picker.querySelector('.lc-picker__list').children) {
        const header = node.querySelector('.lc-picker__group')
        if (header) group = header.innerText
        const row = node.querySelector('.lc-picker__row')
        if (row && !row.disabled && /opencode/i.test(group) && ${FREE_ROW}.test(row.innerText)) { target = row; break }
      }
      if (!target) await new Promise(r => setTimeout(r, 500))
    }
    if (!target) return 'free model not offered; rows: ' + [...document.querySelectorAll('.lc-picker__row')].map(r => r.innerText).slice(0, 5).join(' | ')
    target.click()
    await new Promise(r => setTimeout(r, 400))
    return [...document.querySelectorAll('.lc-control')].map(c => c.innerText.replace(/\\s+/g, ' ').trim()).join(' · ')
  })()`))

  await capture('send a first message and wait', () => evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Read README.md and tell me in one sentence what this project is. Then, using your file tool, add a line to README.md saying "Nova was here".')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); break }
    }
    for (let i = 0; i < 720; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      if (!document.querySelector('button[aria-label^="Stop the running"]')) break
    }
    await new Promise(r => setTimeout(r, 800))
    return document.querySelector('.lc-thread, main')?.innerText.replace(/\\s+/g, ' ').slice(-400) ?? ''
  })()`))

  await capture('open the activity fold', () => evaluate(`(async () => {
    const fold = document.querySelector('.lc-activity')
    if (!fold) return 'no activity fold'
    // Only when it is not already open. Since 0.49.0 a finished turn's fold
    // opens itself, so an unconditional click CLOSES it and every row below
    // then reads as absent -- a harness reporting a bare screen at an app
    // that is fine.
    if (fold.getAttribute('aria-expanded') !== 'true') fold.click()
    await new Promise(r => setTimeout(r, 300))
    return [...document.querySelectorAll('.lc-filerow')].map(r => r.innerText.replace(/\\s+/g, ' ').trim()).join(' | ').slice(0, 300)
  })()`))

  await capture('go home from the logo', () => evaluate(`(async () => {
    document.querySelector('.lc-brand__lockup').click()
    await new Promise(r => setTimeout(r, 400))
    return document.querySelector('.lc-runtimepanel') ? 'home screen' : 'not home: ' + document.body.innerText.replace(/\\s+/g, ' ').slice(0, 120)
  })()`))

  await capture('open Memory with Ctrl 5', () => evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '5', ctrlKey: true, bubbles: true }))
    await new Promise(r => setTimeout(r, 600))
    return document.querySelector('.lc-screen__title')?.innerText + ' · ' + (document.querySelector('.lc-screen__meta')?.innerText ?? '')
  })()`))

  await capture('open Rooms with Ctrl 4', () => evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '4', ctrlKey: true, bubbles: true }))
    await new Promise(r => setTimeout(r, 600))
    return document.querySelector('.lc-screen__title')?.innerText + ' · ' + (document.querySelector('.lc-screen__meta')?.innerText ?? '')
  })()`))

  await capture('open Settings from the rail', () => evaluate(`(async () => {
    document.querySelector('button[title="Settings (Ctrl 3)"]').click()
    await new Promise(r => setTimeout(r, 800))
    const headings = [...document.querySelectorAll('.lc-settings__heading')].map(h => h.innerText)
    const tags = [...document.querySelectorAll('.lc-policyrow .lc-tag')].map(t => t.textContent.trim() + ': ' + (t.parentElement?.querySelector('.lc-settings__note')?.innerText.replace(/\\s+/g, ' ').slice(0, 90) ?? ''))
    return headings.join(' / ') + ' || ' + tags.join(' || ')
  })()`))

  await capture('Team screen, then edit Nova', () => evaluate(`(async () => {
    document.querySelector('button[title="Team (Ctrl 2)"]').click()
    await new Promise(r => setTimeout(r, 600))
    const edit = [...document.querySelectorAll('button')].find(b => b.innerText.trim() === 'Edit')
    if (!edit) return 'no Edit on the Team screen: ' + document.body.innerText.replace(/\\s+/g, ' ').slice(0, 160)
    edit.click()
    await new Promise(r => setTimeout(r, 500))
    const dialog = document.querySelector('[role=dialog]')
    return dialog ? 'edit dialog: own=' + dialog.querySelector('[role=switch][aria-label="Own copy"]')?.getAttribute('aria-checked') + ' title=' + ([...dialog.querySelectorAll('input')].find(i => /Release manager/.test(i.placeholder))?.value ?? '?') : 'no dialog'
  })()`))

  await capture('cancel the edit and read the sidebar', () => evaluate(`(async () => {
    const dialog = document.querySelector('[role=dialog]')
    const cancel = dialog && [...dialog.querySelectorAll('button')].find(b => b.innerText.trim() === 'Cancel')
    if (cancel) cancel.click()
    await new Promise(r => setTimeout(r, 400))
    return document.querySelector('.lc-sidebar').innerText.replace(/\\s+/g, ' ').slice(0, 240)
  })()`))

  const branch = await git(['branch', '--list', 'locust/*'], workspace).catch(() => '')
  const mainReadme = await (await import('node:fs/promises')).readFile(join(workspace, 'README.md'), 'utf8')
  record.push({ step: step + 1, title: 'disk after the session', note: `branches: ${branch.trim().replace(/\s+/g, ' ')} · main README unchanged: ${String(!/Nova was here/.test(mainReadme))}`, errors: [] })
} catch (error) {
  record.push({ step: step + 1, title: 'session aborted', note: error instanceof Error ? error.message : String(error), errors: [] })
} finally {
  const lines = [
    `# User session ${stamp}`,
    '',
    'Build: whatever `pnpm build` last wrote to out/. Route: OpenCode free model. Folder: a scratch git repository with README.md and LOCUST.md.',
    '',
    '| # | Step | What was seen | Renderer errors |',
    '|---|---|---|---|',
    ...record.map((entry) => `| ${String(entry.step)} | ${entry.title} | ${String(entry.note).replace(/\|/g, '/').slice(0, 220)} | ${entry.errors.length === 0 ? '0' : entry.errors.join('; ').slice(0, 120)} |`),
    '',
    '## Judgement',
    '',
    '(filled in after reading the captures)',
    ''
  ]
  await writeFile(join(OUT, 'SESSION.md'), lines.join('\n'), 'utf8')
  say(`\nrecord: ${OUT}`)
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
  if (KEEP) say(`profile kept at ${profile}; workspace at ${workspace}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
