// Does Rename work in the rail layout, and is New group offered only where it works (M35)?
//
//   node _tools/drive-rail-rename.mjs [--packaged <exe>] [--tag <name>]
//
// At 1150 wide the sidebar is the rail. A conversation of Wren's is seeded;
// the person opens Wren's card, right-clicks the conversation and picks
// Rename. The field lived only in the wide list, so nothing appeared -- and
// the + menu offered New group, which named nothing there either. Sends
// nothing.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `rail-rename-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-railrename-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-railrename-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const at = '2026-09-24T12:00:00.000Z'
await ledger.createMission({
  missionId: 'mission_rail', runId: 'run_rail', prompt: 'OLD NAME: tidy the README.',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const codex = adapters.createCodexEventNormalizer({ runId: 'run_rail', missionId: 'mission_rail', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
await ledger.appendEvents('mission_rail', [
  ...codex.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_rail' }) }),
  ...codex.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'Tidied.' } }) }),
  ...codex.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
  ...codex.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })
])

const drive = await startDrive({
  name: 'rail-rename',
  port: 9593,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }],
    missionOwners: { mission_rail: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1150, 780)
  await sleep(1500)
  const rail = String(await drive.evaluate(`document.querySelector('.lc-shell')?.className ?? 'no shell'`))
  check('the sidebar is the rail at 1150 wide', /is-compact/.test(rail), rail)
  const opened = String(await drive.capture("open Wren's card and right-click the conversation", () => drive.evaluate(`(async () => {
    const slot = document.querySelector('.lc-railslot button.lc-row--button')
    if (!slot) return 'no rail slot'
    slot.click()
    await new Promise((r) => setTimeout(r, 700))
    const row = [...document.querySelectorAll('.lc-railflyout__row')].find((b) => /OLD NAME/.test(b.textContent ?? ''))
    if (!row) return 'no row in the card: ' + (document.querySelector('.lc-railflyout')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? 'no card')
    const box = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 20, clientY: box.top + 5 }))
    await new Promise((r) => setTimeout(r, 500))
    const rename = [...document.querySelectorAll('.lc-context__item')].find((b) => /^Rename/.test(b.textContent.trim()))
    if (!rename) return 'no Rename in the menu'
    rename.click()
    await new Promise((r) => setTimeout(r, 800))
    return 'chose Rename'
  })()`)))
  check('Rename was chosen from the card', opened === 'chose Rename', opened)
  const field = String(await drive.evaluate(`(() => {
    const input = document.querySelector('input[aria-label="Name this conversation"]')
    return input === null ? 'no field' : 'field, focused: ' + String(document.activeElement === input)
  })()`))
  await drive.capture('what Rename shows in the rail', () => field)
  check('a field to name it appears in the rail', /^field/.test(field), field)
  if (/^field/.test(field)) {
    await drive.evaluate(`(async () => {
      const input = document.querySelector('input[aria-label="Name this conversation"]')
      input.focus()
      const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      set.call(input, 'NEW NAME from the rail')
      input.dispatchEvent(new Event('input', { bubbles: true }))
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
      await new Promise((r) => setTimeout(r, 1200))
    })()`)
    const named = String(await drive.capture('after Enter', () => drive.evaluate(`document.querySelector('.lc-railflyout')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? (document.querySelector('.lc-workroom__header')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? '')`)))
    check('the conversation takes the new name', /NEW NAME from the rail/.test(named), named)
  }
  const add = String(await drive.capture('the + menu in the rail', () => drive.evaluate(`(async () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    const plus = [...document.querySelectorAll('.lc-sidebar button')].find((b) => /^Add|New/.test(b.getAttribute('aria-label') ?? ''))
    if (!plus) return 'no + button'
    plus.click()
    await new Promise((r) => setTimeout(r, 500))
    return [...document.querySelectorAll('.lc-context__item')].map((b) => b.textContent.trim()).join(' | ')
  })()`)))
  check('the + menu offers no New group where no group can be named', !/New group/.test(add) && /New teammate/.test(add), add)
  say(failures === 0 ? '\nRAIL RENAME PASSED' : `\nRAIL RENAME: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. The rail layout at 1150 wide; Rename from Wren's card, and the + menu.` })
}
