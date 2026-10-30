// Does the thread say what a finished turn really did (M24, M25)?
//
//   node _tools/drive-thread-reads-true.mjs [--packaged <exe>] [--tag <name>]
//
// Two finished Claude Code turns, seeded through Claude's own normalizer.
// One ran a read-only `sed -n '1,5p' README.md`: every `sed` counted as an
// edit, so the fold read "1 file" and drew the command as an edited file
// Claude "did not report". The other answered with nothing but a file handed
// over, and was told under it "This turn ended without a reply ... Nothing
// was changed." Sends nothing.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `thread-reads-true-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-threadtrue-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-threadtrue-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const NL = String.fromCharCode(10)

// One finished Claude turn per call, through Claude's own normalizer.
const seedTurn = async (missionId, prompt, records, minute) => {
  const when = `2026-09-24T12:0${String(minute)}:00.000Z`
  await ledger.createMission({
    missionId, runId: `run_${missionId}`, prompt,
    runtime: 'claude', model: 'haiku', requestedRouteId: 'claude', resolvedRouteId: 'claude-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', mode: 'ask', executionPolicyVersion: 1, createdAt: when
  })
  const claude = adapters.createClaudeEventNormalizer({ runId: `run_${missionId}`, missionId, cliVersion: '2.1.281', now: () => new Date(when) })
  let sequence = 0
  const feed = (value) => claude.accept({ sequence: ++sequence, raw: JSON.stringify(value) })
  const events = [
    ...feed({ type: 'system', subtype: 'init', session_id: `session_${missionId}`, model: 'haiku', tools: [] }),
    ...records.flatMap((record) => feed(record)),
    ...feed({ type: 'result', subtype: 'success', is_error: false, result: 'done' }),
    ...claude.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: sequence, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: when, finishedAt: when })
  ]
  await ledger.appendEvents(missionId, events)
  say(`seeded ${missionId}: ${events.map((e) => e.type).join(', ')}`)
}

// M24: a read-only sed, then a plain answer.
await seedTurn('mission_sed', 'SED TURN: show me the README head.', [
  { type: 'stream_event', event: { type: 'content_block_start', index: 0, content_block: { type: 'tool_use', id: 'toolu_sed', name: 'Bash' } } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'toolu_sed', name: 'Bash', input: { command: "sed -n '1,5p' README.md", description: 'Show the README head' } }] } },
  { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 'toolu_sed', content: '# scratch' }] } },
  { type: 'assistant', message: { id: 'msg_sed', content: [{ type: 'text', text: 'It starts with # scratch.' }] } }
], 1)
// M25: a reply that was nothing but a file handed over.
await seedTurn('mission_file', 'FILE TURN: hand me the README.', [
  { type: 'assistant', message: { id: 'msg_file', content: [{ type: 'text', text: ['<locust-file>', 'README.md :: the file you asked for', '</locust-file>'].join(NL) }] } }
], 2)

const drive = await startDrive({
  name: 'thread-reads-true',
  port: 9589,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
// Opens the conversation whose row matches `marker`, and returns the thread's text.
const open = (marker, label) => drive.capture(label, () => drive.evaluate(`(async () => {
  const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => (el.textContent ?? '').includes(${JSON.stringify(marker)}) && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
  row?.click()
  await new Promise((r) => setTimeout(r, 2000))
  return (document.querySelector('.lc-thread')?.innerText ?? 'no thread').split(String.fromCharCode(10)).map((line) => line.trim()).filter(Boolean).join(' | ')
})()`)).then(String)

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(1500)
  const sed = await open('SED TURN', 'open the turn that ran sed -n')
  say(`  sed turn: ${sed.slice(0, 300)}`)
  check('the read-only sed is not counted as a file', !sed.split(' | ').includes('1 file') && !sed.includes('did not report the change'), sed.slice(0, 200))
  const file = await open('FILE TURN', 'open the turn that handed over a file')
  say(`  file turn: ${file.slice(0, 300)}`)
  check('the file handed over is shown', file.includes('README.md'))
  check('the turn is not said to have ended without a reply', !file.includes('ended without a reply'))
  say(failures === 0 ? '\nTHREAD READS TRUE PASSED' : `\nTHREAD READS TRUE: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Two finished Claude Code turns, seeded through Claude's normalizer: a read-only sed, and a reply that was only a file handed over.` })
}
