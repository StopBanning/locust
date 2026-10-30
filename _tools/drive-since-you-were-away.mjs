// Home says what happened since you were away (0.590, PRD R17).
//
//   node _tools/drive-since-you-were-away.mjs [--packaged <exe>] [--tag <name>] [--recent]
//
// A profile whose attention mark (away.json) is two hours old, a ledger with
// one turn that finished and one that failed an hour ago, and a routine whose
// 07:00 slot was recorded missed while Locust was closed. Home must open with
// the list -- "1 ran · 1 failed · 1 missed", failed row first -- the ran row
// must open its record, Home must still show the list afterwards, and Got it
// must take it away and move the mark to now. `--recent` is the control: a
// mark five minutes old is a break, not an absence, and Home shows nothing.
// Sends nothing; no model runs.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const recent = process.argv.includes('--recent')

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-away-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-away-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const minutesAgo = (minutes) => new Date(Date.now() - minutes * 60_000).toISOString()

// Two finished turns, an hour ago: one that completed, one whose process failed.
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const turn = async (missionId, runId, prompt, said, endedMinutesAgo, exitCode) => {
  const startedAt = minutesAgo(endedMinutesAgo + 3)
  await ledger.createMission({
    missionId, runId, prompt, runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: startedAt
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({
    runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1',
    now: () => new Date(Date.parse(startedAt) + (tick += 1) * 20_000)
  })
  const records = [
    { type: 'thread.started', thread_id: `thread_${runId}` },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: said } },
    ...(exitCode === 0 ? [{ type: 'turn.completed', usage: { input_tokens: 1200, cached_input_tokens: 0, output_tokens: 80 } }] : [])
  ]
  await ledger.appendEvents(missionId, [
    ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({
      exitCode, signal: null, stderr: exitCode === 0 ? '' : 'codex: the model returned an error (503)', stderrTruncated: false, recordCount: records.length, cancelled: false,
      forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0,
      startedAt, finishedAt: minutesAgo(endedMinutesAgo)
    })
  ])
}
const RAN = 'mission_10000000-0000-4000-8000-000000000011'
const FAILED = 'mission_10000000-0000-4000-8000-000000000012'
await turn(RAN, 'run_100011', 'Write the morning digest for the team.', 'The digest is written to DIGEST.md.', 60, 0)
await turn(FAILED, 'run_100012', 'Run the nightly tests and fix what fails.', 'Starting the tests.', 90, 1)
say('seeded one completed and one failed turn, an hour and a half ago')

const routine = {
  routineId: 'rt_aaaaaaaaaaaaaaaaaaaaaaa1', teammateId: WREN.teammateId, name: 'Morning digest', steps: ['Summarize what the team did yesterday into DIGEST.md.'],
  route: { runtime: 'codex', model: 'account-default', mode: 'ask' }, learnedFrom: [], createdAt: minutesAgo(3 * 24 * 60), runs: 2, lastRunAt: minutesAgo(2 * 24 * 60),
  schedule: { kind: 'daily', at: '07:00' }, missedAt: minutesAgo(70), history: [{ kind: 'missed', dueAt: minutesAgo(70), recordedAt: minutesAgo(50) }]
}
const drive = await startDrive({
  name: `since-you-were-away-${tag}`, port: 9855, workspace, profilePath, outPath: join(recordRoot('since-you-were-away-2026-10-04'), tag), sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [RAN]: WREN.teammateId, [FAILED]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } },
  files: {
    'away.json': { at: minutesAgo(recent ? 5 : 120) },
    'routines.json': { schemaVersion: 1, routines: [routine] }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const AWAY = `document.querySelector('.lc-away')`
const readAway = `(() => {
  const away = ${AWAY}
  if (!away) return JSON.stringify({ present: false })
  return JSON.stringify({
    present: true,
    line: away.querySelector('.lc-away__line')?.innerText.trim() ?? '',
    kinds: [...away.querySelectorAll('.lc-away__row')].map((row) => row.getAttribute('data-kind')),
    titles: [...away.querySelectorAll('.lc-away__title')].map((el) => el.innerText.trim()),
    waiting: away.querySelector('.lc-away__waiting')?.innerText.trim() ?? ''
  })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2500)
  const seen = JSON.parse(String(await drive.capture(recent ? 'Home, five minutes later: no list' : 'Home, back after two hours', () => drive.evaluate(readAway))))
  if (recent) {
    check('a five-minute break shows no "Since you were away" (control)', seen.present === false, JSON.stringify(seen))
  } else {
    check('Home opens with the list', seen.present === true, JSON.stringify(seen))
    check('the head line counts one ran, one failed, one missed, nothing waiting', seen.line === '1 ran · 1 failed · 1 missed', seen.line)
    check('failed first, then the missed slot, then what ran', JSON.stringify(seen.kinds) === JSON.stringify(['failed', 'missed', 'ran']), JSON.stringify(seen.kinds))
    check('the rows name the turns and the routine', JSON.stringify(seen.titles) === JSON.stringify(['Run the nightly tests and fix what fails.', 'Morning digest', 'Write the morning digest for the team.']), JSON.stringify(seen.titles))
    check('nobody is waiting, so no waiting line', seen.waiting === '', seen.waiting)
    const opened = String(await drive.capture('the ran row opens its record', () => drive.evaluate(`(async () => {
      const row = [...document.querySelectorAll('.lc-away__row')].find((one) => one.getAttribute('data-kind') === 'ran')
      if (!row) return 'no ran row'
      row.click()
      for (let i = 0; i < 40; i += 1) {
        await new Promise((r) => setTimeout(r, 250))
        const thread = document.querySelector('.lc-thread')?.innerText ?? ''
        if (/The digest is written/.test(thread)) return 'record open: ' + thread.replace(/\\s+/g, ' ').slice(0, 120)
      }
      return 'thread: ' + (document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? 'none')
    })()`)))
    check('the record opens with its words', /^record open:/.test(opened), opened)
    const back = JSON.parse(String(await drive.capture('Home again: the list stays until Got it', () => drive.evaluate(`(async () => {
      document.querySelector('[aria-label="Home"]')?.click()
      await new Promise((r) => setTimeout(r, 1200))
      return ${readAway}
    })()`))))
    check('back on Home, the list is still there', back.present === true && back.kinds.length === 3, JSON.stringify(back))
    const gone = JSON.parse(String(await drive.capture('Got it', () => drive.evaluate(`(async () => {
      ;[...document.querySelectorAll('.lc-away__seen')].find((b) => b.innerText.trim() === 'Got it')?.click()
      await new Promise((r) => setTimeout(r, 1200))
      return ${readAway}
    })()`))))
    check('Got it takes the list away', gone.present === false, JSON.stringify(gone))
    await sleep(800)
    const mark = JSON.parse(await readFile(join(profilePath, 'away.json'), 'utf8'))
    const age = Date.now() - Date.parse(mark.at)
    check('and the attention mark moved to now', Number.isFinite(age) && age >= 0 && age < 120_000, `mark ${mark.at}, ${String(Math.round(age / 1000))} s old`)
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. ${recent ? 'A five-minute break: Home must show nothing.' : 'Back after two hours: one ran, one failed, one missed; Got it clears it.'} No model ran.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
