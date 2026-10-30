// Does the turn's steps line keep a long file name readable, and fit its row whole (0.604)?
//
//   node _tools/drive-the-steps-line-fits.mjs [--packaged <exe>] [--tag <name>]
//
// One seeded Codex turn, no model call, nothing spent: it edits
// a-full-rule-store-keeps-every-rule.test.ts (42 characters), reads a file,
// runs two commands, searches and lists. Colin's screenshot of 2026-10-04 had
// such a line cut by the row's own end-of-line ellipsis: "created
// a-full-rule-store-keeps-every-rule.test.ts, edited ap…" -- the second phrase
// cut mid-word, the first name shown whole. Since 0.604 a long name is cut in
// the middle with its extension kept, and the line is refitted to its row:
// names give way to counts, then trailing phrases to "and N more", so what
// shows is always whole phrases, and the full sentence is the line's title.
// The rows under the line (opened) still carry the full name.
//
// The window is driven at 1024 x 720, where the full sentence does not fit,
// and at 1440 x 900. The control (the 0.603 package) fails the fit check at
// 1024: the sentence overflows and the name is shown whole.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, openAllStepsScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('beta-fixes-2026-09-24'), `the-steps-line-fits-${tag}`)
await mkdir(OUT, { recursive: true })

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-steps-fit-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-steps-fit-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const LONG_NAME = 'a-full-rule-store-keeps-every-rule.test.ts'
const LONG = `${workspace.replace(/\\/g, '/')}/src/${LONG_NAME}`
const TURN = { missionId: 'mission_30000000-0000-4000-8000-000000000001', runId: 'run_300001', prompt: 'Keep every rule in the store and check the build' }

/** One finished Codex turn: a file_change naming the long file, a read, two commands, a search, a listing. */
async function seedTurn({ missionId, runId, prompt, startedAgoMs }) {
  const at = new Date(Date.now() - startedAgoMs).toISOString()
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const command = (id, text) => [
    { type: 'item.started', item: { id, type: 'command_execution', status: 'in_progress', command: text } },
    { type: 'item.completed', item: { id, type: 'command_execution', status: 'completed', command: text, exit_code: 0, aggregated_output: 'ok\n' } }
  ]
  const records = [
    { type: 'thread.started', thread_id: `thread_${runId}` },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'fc1', type: 'file_change', status: 'in_progress', changes: [{ path: LONG, kind: 'update' }] } },
    { type: 'item.completed', item: { id: 'fc1', type: 'file_change', status: 'completed', changes: [{ path: LONG, kind: 'update' }] } },
    ...command('c1', 'cat NOTES.md'),
    ...command('c2', 'pnpm test'),
    ...command('c3', 'pnpm tsc --noEmit'),
    ...command('c4', 'rg approval src'),
    ...command('c5', 'ls -la'),
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Every rule is kept; the build is clean.' } },
    { type: 'turn.completed', usage: { input_tokens: 2000, cached_input_tokens: 0, output_tokens: 80 } }
  ]
  const events = [
    ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
  ]
  await ledger.appendEvents(missionId, events)
}

await seedTurn({ ...TURN, startedAgoMs: 300_000 })
say(`seeded one finished Codex turn that edited ${LONG_NAME}`)

const drive = await startDrive({
  name: `the-steps-line-fits-${tag}`, port: 9798, workspace, profilePath, outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [TURN.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
/** The steps line as drawn: its text, whether it overflows its row, and its title. */
const lineState = async () => JSON.parse(String(await drive.evaluate(`(() => {
  const text = document.querySelector('.lc-thread .lc-steps__text')
  if (!text) return JSON.stringify({ found: false })
  return JSON.stringify({ found: true, text: text.innerText.replace(/\\s+/g, ' ').trim(), overflow: text.scrollWidth - text.clientWidth, width: text.clientWidth, title: text.getAttribute('title') ?? text.closest('button')?.getAttribute('title') ?? '' })
})()`)))
const MIDDLE_CUT = /^a-full-rule-stor.*…[^ ]*\.test\.ts/
try {
  await drive.ready()
  // Opened wide, where the sidebar's rows are on screen; then read narrow.
  await drive.resize(1440, 900)
  await sleep(2500)
  const opened = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('Keep every rule'))
    if (!row) return 'no conversation row'
    row.click()
    return 'opened'
  })()`))
  check('the seeded conversation opens', opened === 'opened', opened)
  await sleep(1200)
  await drive.resize(1024, 720)
  await sleep(1200)
  const narrow = await lineState()
  await drive.capture('the steps line at 1024 wide', async () => JSON.stringify(narrow))
  check('the turn has a steps line', narrow.found === true, JSON.stringify(narrow))
  say(`  at 1024: ${JSON.stringify(narrow)}`)
  check('at 1024 the line fits its row (no end-of-line cut)', narrow.found && narrow.overflow <= 0, `overflow ${String(narrow.overflow)} px: ${narrow.text}`)
  check('the long name is cut in the middle with its extension kept, or has given way to a count', narrow.found && (MIDDLE_CUT.test(narrow.text.replace(/^Edited /, '')) || /^Edited a file\b/.test(narrow.text)), narrow.text)
  // Whatever the line shows short of the whole name, the hover says it whole.
  if (narrow.found && !narrow.text.includes(LONG_NAME)) check('at 1024 the hover title carries the whole file name', typeof narrow.title === 'string' && narrow.title.includes(LONG_NAME), narrow.title)
  // Narrower still: a small window, where the column shrinks under the sentence.
  await drive.resize(800, 600)
  await sleep(1200)
  const tight = await lineState()
  await drive.capture('the steps line at 800 wide', async () => JSON.stringify(tight))
  say(`  at 800: ${JSON.stringify(tight)}`)
  check('at 800 the line fits its row (no end-of-line cut)', tight.found && tight.overflow <= 0, `overflow ${String(tight.overflow)} px: ${tight.text}`)
  if (tight.found && !tight.text.includes(LONG_NAME)) check('at 800 the hover title carries the whole file name', typeof tight.title === 'string' && tight.title.includes(LONG_NAME), tight.title)
  await drive.resize(1024, 720)
  await sleep(800)
  // Opened, the rows still say the whole name.
  await drive.evaluate(openAllStepsScript())
  await sleep(600)
  const rows = JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-thread .lc-filerow')].map((row) => row.innerText.replace(/\\s+/g, ' ').trim()))`)))
  check('opened, a row still carries the whole file name', rows.some((text) => text.includes(LONG_NAME)), JSON.stringify(rows).slice(0, 300))
  const edges = async () => JSON.parse(String(await drive.evaluate(`(() => {
    const rows = [...document.querySelectorAll('.lc-thread .lc-filerow')]
    const left = (row) => {
      const path = row.querySelector('.lc-filerow__path')
      return path === null ? null : Math.round(path.getBoundingClientRect().left)
    }
    const file = rows.filter((row) => !row.classList.contains('is-shell')).map(left).find((n) => n !== null) ?? null
    const shell = rows.find((row) => row.classList.contains('is-shell'))
    const path = shell?.querySelector('.lc-filerow__path')
    const badge = shell?.querySelector('.lc-shellbadge')
    return JSON.stringify({
      file,
      shell: shell === undefined ? null : left(shell),
      badgeAfter: path !== undefined && badge !== undefined && badge.getBoundingClientRect().left >= path.getBoundingClientRect().left + 8,
      badge: (badge?.innerText ?? '').replace(/\\s+/g, ' ').trim(),
      theme: document.documentElement.dataset.theme ?? ''
    })
  })()`)))
  const linedUp = async (where) => {
    await drive.evaluate(openAllStepsScript())
    const measured = await edges()
    await drive.capture(`opened steps, ${where}`, async () => JSON.stringify(measured))
    check(`at ${where} a command and a path start on one edge`, measured.file !== null && measured.shell !== null && Math.abs(measured.file - measured.shell) <= 2, JSON.stringify(measured))
    check(`at ${where} the result badge follows the command`, measured.badgeAfter === true && /exit 0|done|refused|running/.test(measured.badge), measured.badge)
  }
  await drive.resize(1440, 900)
  await drive.evaluate(`window.desktop.setTheme('dark')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'dark'`, { what: 'dark theme' })
  await linedUp('dark 1440')
  await drive.evaluate(`window.desktop.setTheme('light')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'light'`, { what: 'light theme' })
  await linedUp('light 1440')
  await drive.resize(1205, 803)
  await linedUp('light 1205')
  await drive.evaluate(`window.desktop.setTheme('dark')`)
  await drive.waitFor(`document.documentElement.dataset.theme === 'dark'`, { what: 'dark theme again' })
  await linedUp('dark 1205')
  await drive.evaluate(openAllStepsScript())
  await drive.resize(1440, 900)
  await sleep(1200)
  const wide = await lineState()
  await drive.capture('the steps line at 1440 wide', async () => JSON.stringify(wide))
  say(`  at 1440: ${JSON.stringify(wide)}`)
  check('at 1440 the line fits its row', wide.found && wide.overflow <= 0, `overflow ${String(wide.overflow)} px: ${wide.text}`)
  say(failures === 0 ? '\nTHE STEPS LINE FITS PASSED' : `\nTHE STEPS LINE FITS: ${String(failures)} FAILED`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever the last build wrote to out/'}. One seeded Codex turn editing a 42-character file name, read at 1024, 800 and 1440 wide.`, extra: `Checks failed: ${String(failures)}` })
  process.exitCode = failures === 0 ? 0 : 1
}
