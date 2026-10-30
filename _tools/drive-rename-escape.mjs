// Does Escape abandon a rename, or save it? (code review B4, renderer-screens)
//
//   node _tools/drive-rename-escape.mjs [--packaged <exe>] [--tag <name>]
//
// Every rename field in the app says "Enter commits, Escape abandons, losing
// focus commits". Escape ends the rename by removing the field -- and if the
// browser reports that removal as the field losing focus, the blur handler
// commits exactly what Escape was meant to throw away. Whether Chromium does
// is the whole question, so it is measured: a seeded conversation of Wren's
// in the wide sidebar, Rename from its menu, a new name typed, a REAL Escape
// key (CDP Input.dispatchKeyEvent, not a synthetic event), and the name read
// back. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `rename-escape-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-renameesc-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-renameesc-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const at = '2026-09-25T12:00:00.000Z'
await ledger.createMission({
  missionId: 'mission_esc', runId: 'run_esc', prompt: 'KEEP THIS NAME: tidy the README.',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const codex = adapters.createCodexEventNormalizer({ runId: 'run_esc', missionId: 'mission_esc', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
await ledger.appendEvents('mission_esc', [
  ...codex.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_esc' }) }),
  ...codex.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'Tidied.' } }) }),
  ...codex.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
  ...codex.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })
])

const drive = await startDrive({
  name: 'rename-escape',
  port: 9594,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }],
    missionOwners: { mission_esc: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

const key = async (name, code, keyCode) => {
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode })
}
const renameAndType = (text) => drive.evaluate(`(async () => {
  const row = document.querySelector('.lc-conv')
  if (!row) return 'no conversation row'
  const box = row.getBoundingClientRect()
  row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 20, clientY: box.top + 5 }))
  await new Promise((r) => setTimeout(r, 500))
  const rename = [...document.querySelectorAll('.lc-context__item')].find((b) => /^Rename/.test(b.textContent.trim()))
  if (!rename) return 'no Rename in the menu'
  rename.click()
  await new Promise((r) => setTimeout(r, 700))
  const input = document.querySelector('input[aria-label="Name this conversation"]')
  if (!input) return 'no field'
  input.focus()
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(text)})
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 300))
  return 'typed, focused: ' + String(document.activeElement === input)
})()`)
const title = () => drive.evaluate(`(() => {
  const row = document.querySelector('.lc-conv')
  return (row?.getAttribute('title') ?? row?.textContent ?? 'no row').replace(/\\s+/g, ' ').slice(0, 120)
})()`)

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1400, 860)
  await sleep(1200)
  await drive.capture('the conversation, before', title)
  await drive.capture('Rename, type THROWN AWAY, press a real Escape', async () => {
    const typed = await renameAndType('THROWN AWAY')
    await key('Escape', 'Escape', 27)
    await sleep(1500)
    const field = await drive.evaluate(`String(document.querySelector('input[aria-label="Name this conversation"]') !== null)`)
    return `${String(typed)} || field still open: ${String(field)} || name now: ${String(await title())}`
  })
  await drive.capture('Rename, type KEPT BY ENTER, press a real Enter', async () => {
    const typed = await renameAndType('KEPT BY ENTER')
    await key('Enter', 'Enter', 13)
    await sleep(1500)
    return `${String(typed)} || name now: ${String(await title())}`
  })
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. One seeded conversation of Wren's in the wide sidebar; renamed twice, once abandoned with Escape and once kept with Enter.` })
}
