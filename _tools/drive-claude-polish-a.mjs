// The quieter message box, the shortcut sheet, Verbose, and a file's right-click.
//
//   node _tools/drive-claude-polish-a.mjs
//
// Sends nothing. A scratch profile, one finished Codex turn and one recovered
// turn that never finished. No model is asked. The stop square needs a live
// process, so the "while a run works" frames are that recovered turn: the
// closest screen this drive can show without spending.
//
// Frames: D:/work/scratch/grok-polish-a/

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, appendFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const OUT = 'D:/work/scratch/grok-polish-a'
// The window does not read the clipboard (it only writes it), so the drive reads it from outside, as a person pasting would.
const systemClipboard = () => process.platform === 'win32'
  ? execFileSync(join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'), ['-NoProfile', '-Command', 'Get-Clipboard'], { encoding: 'utf8' }).trim()
  : 'not read off Windows'
await mkdir(OUT, { recursive: true })

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-polish-a-ws-')
await appendFile(join(workspace, 'README.md'), '\nA line added after the first commit, so Changes has a row.\n', 'utf8')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-polish-a-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z', route: FREE_ROUTE }
const FILE = `${workspace.replace(/\\/g, '/')}/src/signup.ts`

async function seedFinished() {
  const missionId = 'mission_30000000-0000-4000-8000-0000000000a1'
  const runId = 'run_3000a1'
  const at = new Date(Date.now() - 120_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: 'Update the signup form',
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: 'thread_polish_a' },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'c1', type: 'command_execution', status: 'in_progress', command: 'cat src/signup.ts' } },
    { type: 'item.completed', item: { id: 'c1', type: 'command_execution', status: 'completed', command: 'cat src/signup.ts', exit_code: 0, aggregated_output: 'export const form = true\n' } },
    { type: 'item.started', item: { id: 'fc1', type: 'file_change', status: 'in_progress', changes: [{ path: FILE, kind: 'update' }] } },
    { type: 'item.completed', item: { id: 'fc1', type: 'file_change', status: 'completed', changes: [{ path: FILE, kind: 'update' }] } },
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'The form now lives in `src/signup.ts`.' } },
    { type: 'turn.completed', usage: { input_tokens: 400, cached_input_tokens: 0, output_tokens: 30 } }
  ]
  const events = [
    ...records.flatMap((record, index) => normalizer.accept({ sequence: index + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 12_000).toISOString() })
  ]
  await ledger.appendEvents(missionId, events)
  return missionId
}

async function seedUnfinished() {
  const missionId = 'mission_30000000-0000-4000-8000-0000000000a2'
  const runId = 'run_3000a2'
  const at = new Date(Date.now() - 30_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: 'Still checking the signup form',
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: 'thread_polish_b' },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'c1', type: 'command_execution', status: 'in_progress', command: 'cat src/signup.ts' } }
  ]
  await ledger.appendEvents(missionId, records.flatMap((record, index) => normalizer.accept({ sequence: index + 1, raw: JSON.stringify(record) })))
  return missionId
}

const finishedId = await seedFinished()
const unfinishedId = await seedUnfinished()
say('seeded a finished turn and a recovered turn that never finished')

const drive = await startDrive({
  name: 'claude-polish-a',
  port: 9747,
  workspace,
  profilePath,
  outPath: OUT,
  sendsNothing: true,
  focused: true,
  seed: {
    schemaVersion: 1,
    teammates: [WREN],
    missionOwners: { [finishedId]: WREN.teammateId, [unfinishedId]: WREN.teammateId },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const fit = `(() => {
  const under = document.querySelector('.lc-composer__under')
  const box = document.querySelector('.lc-composer__box, .lc-composer__controls')
  if (!under) return JSON.stringify({ under: false, size: [innerWidth, innerHeight] })
  const ur = under.getBoundingClientRect()
  const br = box ? box.getBoundingClientRect() : undefined
  const kids = [...under.querySelectorAll('button')].map((el) => {
    const r = el.getBoundingClientRect()
    const name = (el.getAttribute('aria-label') || el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 80)
    return { name, left: Math.round(r.left), right: Math.round(r.right), clipped: r.right > innerWidth - 1 || r.left < 0 || r.width < 8 }
  })
  return JSON.stringify({
    size: [innerWidth, innerHeight],
    below: br ? ur.top >= br.bottom - 4 : null,
    overflow: under.scrollWidth > under.clientWidth + 2,
    stop: document.querySelector('button[aria-label^="Stop the running"]') !== null,
    kids
  })
})()`

const openConversation = async (word) => {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const opened = String(await drive.evaluate(`(() => {
      const row = [...document.querySelectorAll('button.lc-conv')].find((button) => (button.innerText || button.textContent || '').includes(${JSON.stringify(word)}))
      if (!row) return 'no row'
      row.click()
      return 'opened'
    })()`))
    if (opened === 'opened') {
      const shown = String(await drive.evaluate(`(document.querySelector('.lc-thread')?.innerText ?? '').includes(${JSON.stringify(word)}) ? 'shown' : 'not yet'`))
      if (shown === 'shown') return 'opened'
    }
    await sleep(300)
  }
  return 'not shown'
}

const typeBox = (text) => drive.evaluate(`(() => {
  const field = document.querySelector('form.command-dock textarea')
  if (!field) return 'no box'
  field.focus()
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  return 'typed'
})()`)

const key = async (name, code, vk, { ctrl = false, shift = false } = {}) => {
  const modifiers = (ctrl ? 2 : 0) | (shift ? 8 : 0)
  const base = { key: name, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, modifiers }
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', ...base })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', ...base })
  await sleep(350)
}

const menuText = () => drive.evaluate(`(() => {
  const open = document.querySelector('.lc-dialog, .lc-picker, .lc-menu[role="menu"], .lc-context')
  const steps = document.querySelector('.lc-thread .lc-steps__line, .lc-thread .lc-activity')
  return JSON.stringify({
    dialog: document.querySelector('.lc-shortcuts')?.innerText?.replace(/\\s+/g, ' ').slice(0, 500) ?? '',
    menu: document.querySelector('.lc-menu[role="menu"]')?.innerText?.replace(/\\s+/g, ' ').slice(0, 300) ?? '',
    picker: document.querySelector('.lc-picker')?.innerText?.replace(/\\s+/g, ' ').slice(0, 400) ?? '',
    effort: document.querySelector('[aria-label="Reasoning effort"], [data-effort-focus]') !== null,
    context: document.querySelector('.lc-context')?.innerText?.replace(/\\s+/g, ' ').slice(0, 400) ?? '',
    stepsOpen: steps?.getAttribute('aria-expanded') ?? 'none',
    chip: document.querySelector('.lc-composer__under button[aria-haspopup="listbox"]')?.getAttribute('aria-label') ?? '',
    mode: document.querySelector('button[aria-label="Permission mode"]')?.innerText?.replace(/\\s+/g, ' ').trim() ?? ''
  })
})()`)

try {
  await drive.ready()
  await sleep(800)
  const opened = String(await openConversation('Update the signup form'))
  say(`opened finished: ${opened}`)
  await sleep(800)
  for (const [width, height] of [[1280, 800], [960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(500)
    await drive.capture(`${width}x${height} empty box`, () => drive.evaluate(fit))
  }
  await typeBox('Look at the signup form')
  for (const [width, height] of [[1280, 800], [960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(400)
    await drive.capture(`${width}x${height} text typed`, () => drive.evaluate(fit))
  }
  await typeBox('')
  await drive.resize(1280, 800)
  const going = String(await openConversation('Still checking'))
  say(`opened unfinished: ${going}`)
  await sleep(800)
  for (const [width, height] of [[1280, 800], [960, 720], [760, 640]]) {
    await drive.resize(width, height)
    await sleep(400)
    await drive.capture(`${width}x${height} recovered unfinished turn`, () => drive.evaluate(fit))
  }
  await drive.resize(1280, 800)
  await sleep(700)
  say(`back to finished: ${String(await openConversation('Update the signup form'))}`)
  await sleep(400)

  await key('/', 'Slash', 191, { ctrl: true })
  await sleep(400)
  await drive.capture('shortcuts sheet', menuText)

  await key('Escape', 'Escape', 27)
  await sleep(300)
  await key('M', 'KeyM', 77, { ctrl: true, shift: true })
  await sleep(400)
  await drive.capture('mode menu', menuText)
  await drive.evaluate(`document.activeElement?.blur()`)
  await key('1', 'Digit1', 49)
  await sleep(300)
  await drive.capture('mode menu digit 1', menuText)

  await key('Escape', 'Escape', 27)
  await sleep(200)
  await key('I', 'KeyI', 73, { ctrl: true, shift: true })
  await sleep(500)
  await drive.capture('model menu', menuText)
  await drive.evaluate(`document.querySelector('.lc-picker input, .lc-picker textarea')?.blur()`)
  await key('2', 'Digit2', 50)
  await sleep(400)
  await drive.capture('model menu digit 2', menuText)

  await key('Escape', 'Escape', 27)
  await sleep(200)
  await key('E', 'KeyE', 69, { ctrl: true, shift: true })
  await sleep(500)
  await drive.capture('effort menu', menuText)
  await key('Escape', 'Escape', 27)
  await sleep(200)

  await key('o', 'KeyO', 79, { ctrl: true })
  await sleep(400)
  await drive.capture('verbose', menuText)
  await key('o', 'KeyO', 79, { ctrl: true })
  await sleep(400)
  await drive.capture('normal again', menuText)

  const answerTarget = String(await drive.evaluate(`(() => {
    const code = document.querySelector('code.lc-code--inline[data-path]')
    if (!code) return 'no path'
    const r = code.getBoundingClientRect()
    code.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 8, clientY: r.top + 6, button: 2 }))
    return code.getAttribute('data-path')
  })()`))
  say(`answer path target: ${answerTarget}`)
  await sleep(400)
  await drive.capture('path in the answer menu', menuText)
  const copiedAnswer = await drive.evaluate(`(async () => {
    const button = [...document.querySelectorAll('.lc-context__item')].find((el) => (el.innerText || '').trim() === 'Copy path')
    if (!button) return 'no Copy path'
    button.click()
    await new Promise((r) => setTimeout(r, 250))
    return 'clicked'
  })()`).then((clicked) => (clicked === 'clicked' ? `clipboard:${systemClipboard()}` : String(clicked)))
  await drive.capture('copied answer path', async () => String(copiedAnswer))
  await drive.evaluate(`document.body.click()`)
  await sleep(200)

  await drive.evaluate(`document.querySelector('button[aria-label="Changes"]')?.click()`)
  await sleep(1200)
  await drive.evaluate(`(() => {
    const head = document.querySelector('.lc-changes__filehead')
    if (!head) return 'no file row'
    const r = head.getBoundingClientRect()
    head.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 12, clientY: r.top + 8, button: 2 }))
    return 'menu'
  })()`)
  await sleep(400)
  await drive.capture('changed file menu', menuText)
  const copiedFile = await drive.evaluate(`(async () => {
    const button = [...document.querySelectorAll('.lc-context__item')].find((el) => (el.innerText || '').trim() === 'Copy path')
    if (!button) return 'no Copy path'
    button.click()
    await new Promise((r) => setTimeout(r, 250))
    return 'clicked'
  })()`).then((clicked) => (clicked === 'clicked' ? `clipboard:${systemClipboard()}` : String(clicked)))
  await drive.capture('copied changed file path', async () => String(copiedFile))
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.stack ?? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Quiet message box, shortcuts, Verbose, and a file right-click. No model turn.' })
}
