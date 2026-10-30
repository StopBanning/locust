// Does the Missions screen show and delete what it says (L23, L25)?
//
//   node _tools/drive-missions-says-true.mjs [--packaged <exe>] [--tag <name>]
//
// Two missions are seeded: a finished one of Wren's and an interrupted one
// of nobody's. L23: in the rail, Wren's card offers "All of Wren's
// conversations" -- it opened everyone's missions (or, with Missions already
// open, closed it). L25: with both picked, a filter that hides one left the
// bar saying "2 selected ... Delete 2", and Delete took only the one shown.
// Sends nothing.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `missions-says-true-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-missionstrue-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-missionstrue-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const seed = async (missionId, prompt, finished, minute) => {
  const at = `2026-09-24T12:0${String(minute)}:00.000Z`
  await ledger.createMission({
    missionId, runId: `run_${missionId}`, prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const codex = adapters.createCodexEventNormalizer({ runId: `run_${missionId}`, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
  await ledger.appendEvents(missionId, [
    ...codex.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
    ...codex.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'Done.' } }) }),
    ...(finished
      ? [
          ...codex.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
          ...codex.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })
        ]
      : [])
  ])
}
await seed('mission_wren', 'WREN MISSION: tidy the README.', true, 1)
await seed('mission_nobody', 'NOBODY MISSION: list the files.', false, 2)

const drive = await startDrive({
  name: 'missions-says-true',
  port: 9599,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }],
    missionOwners: { mission_wren: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const rows = () => drive.evaluate(`[...document.querySelectorAll('.lc-screen .lc-missionrow')].map((row) => row.innerText.replace(/\\s+/g, ' ').slice(0, 40)).join(' | ')`).then(String)
try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1150, 780)
  await sleep(1500)
  // L23, from the rail's card.
  const opened = String(await drive.capture("Wren's card: All of Wren's conversations", () => drive.evaluate(`(async () => {
    document.querySelector('.lc-railslot button.lc-row--button')?.click()
    await new Promise((r) => setTimeout(r, 700))
    const action = [...document.querySelectorAll('.lc-railflyout__action')].find((b) => /All of Wren/.test(b.textContent ?? ''))
    if (!action) return 'no action on the card'
    action.click()
    await new Promise((r) => setTimeout(r, 1000))
    return document.querySelector('.lc-screen__title')?.textContent?.trim() ?? 'no screen'
  })()`)))
  const shown = await rows()
  say(`  Missions after the card: ${shown}`)
  check('the card opens Missions', /Conversations|Missions/.test(opened), opened)
  check("it shows Wren's missions only", /WREN MISSION/.test(shown) && !/NOBODY MISSION/.test(shown), shown)
  // L25, on everyone's missions.
  await drive.evaluate(`(async () => {
    const back = [...document.querySelectorAll('.lc-filter')].find((b) => /Show everyone/.test(b.textContent ?? ''))
    back?.click()
    await new Promise((r) => setTimeout(r, 500))
  })()`)
  const bar = String(await drive.capture('pick both, then filter to Completed', () => drive.evaluate(`(async () => {
    for (const box of document.querySelectorAll('.lc-screen input.lc-missionrow__pick:not([disabled])')) box.click()
    await new Promise((r) => setTimeout(r, 400))
    const before = document.querySelector('.lc-pickbar')?.innerText.replace(/\\s+/g, ' ') ?? 'no bar'
    ;[...document.querySelectorAll('.lc-filter')].find((b) => b.textContent.trim() === 'Completed')?.click()
    await new Promise((r) => setTimeout(r, 500))
    const after = document.querySelector('.lc-pickbar')?.innerText.replace(/\\s+/g, ' ') ?? 'no bar'
    return JSON.stringify({ before, after })
  })()`)))
  say(`  pick bar: ${bar}`)
  const { before, after } = JSON.parse(bar)
  check('both were picked', /2 selected/.test(before), before)
  check('filtered to one, the bar counts the one it will delete', /1 selected/.test(after) && /Delete 1/.test(after), after)
  say(failures === 0 ? '\nMISSIONS SAYS TRUE PASSED' : `\nMISSIONS SAYS TRUE: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. A finished mission of Wren's and an interrupted one of nobody's; the rail card's All of Wren's conversations, then picks across a filter.` })
}
