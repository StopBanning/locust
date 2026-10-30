// Does a Codex run's row read as the shell got it, its image tool in words, its plan step without a stop (0.610)?
//
//   node _tools/drive-a-codex-run-reads-as-words.mjs [--packaged <exe>] [--tag <name>]
//
// One seeded Codex turn, no model call, nothing spent, shaped like Colin's
// screenshot of 2026-10-04 (Casper on GPT-6.1-Sol): a plan whose first step
// ends in a full stop, a command Codex reported with every backslash escaped
// -- its own powershell.exe path included -- and an imageGeneration item. The
// opened steps must show `Get-Content 'C:\Users\...'` with single
// backslashes and the steps line must say "made an image". The live line is a
// unit test (the-live-line-says-which-plan-step, missionView.test): a seeded
// turn is finished, so it draws none.
// The control (the 0.609 package) shows the doubled backslashes and "used
// imageGeneration".

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, openAllStepsScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('beta-fixes-2026-09-24'), `a-codex-run-reads-as-words-${tag}`)
await mkdir(OUT, { recursive: true })

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-codex-words-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-codex-words-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const CASPER = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa2', name: 'Casper', hue: 'blue', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const TURN = { missionId: 'mission_40000000-0000-4000-8000-000000000001', runId: 'run_400001', prompt: 'Mock up how you would fix the dashboard' }
// As Codex reported it on Windows: the whole command escaped, the host's own path too.
const ESCAPED = '"C:\\\\Windows\\\\System32\\\\WindowsPowerShell\\\\v1.0\\\\powershell.exe" -Command "Get-Content \'C:\\\\Users\\\\me\\\\.codex\\\\skills\\\\.system\\\\imagegen\\\\SKILL.md\'; Get-Content notes.md"'
const SHOWN = "Get-Content 'C:\\Users\\me\\.codex\\skills\\.system\\imagegen\\SKILL.md'"

async function seedTurn({ missionId, runId, prompt, startedAgoMs }) {
  const at = new Date(Date.now() - startedAgoMs).toISOString()
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.160.0', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: `thread_${runId}` },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'plan1', type: 'todo_list', items: [
      { text: 'Inspect the screenshot and relevant visual-design notes.', completed: true },
      { text: 'Generate a dashboard redesign mockup from the screenshot.', completed: true },
      { text: 'Deliver the mockup and describe the main improvements.', completed: true }
    ] } },
    // A thought with no bold headline (0.610): it must not lead the steps line, as Claude Code's lines never do.
    { type: 'item.completed', item: { id: 'r1', type: 'reasoning', text: 'Looking at the skill file before drawing anything.' } },
    { type: 'item.started', item: { id: 'c1', type: 'command_execution', status: 'in_progress', command: ESCAPED } },
    { type: 'item.completed', item: { id: 'c1', type: 'command_execution', status: 'completed', command: ESCAPED, exit_code: 0, aggregated_output: 'ok\n' } }
  ]
  const closing = [
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Here is the mockup.' } },
    { type: 'turn.completed', usage: { input_tokens: 2000, cached_input_tokens: 0, output_tokens: 80 } }
  ]
  const accepted = [...records, ...closing].map((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) }))
  const opening = accepted.slice(0, records.length).flat()
  const after = accepted.slice(records.length).flat()
  /*
   * The image tool as Codex's app-server reports it (Colin's run went through
   * app-server; the exec-JSON reader has no image item): a tool.started and a
   * tool.completed named "imageGeneration", the shape read off his ledger --
   * keys only, nothing of his. Spliced in after the command, then the whole
   * turn renumbered, because the ledger takes one unbroken sequence.
   */
  const imageAt = new Date(Date.parse(at) + 8_000).toISOString()
  const image = [
    { runId, missionId, occurredAt: imageAt, sourceAdapter: 'codex', type: 'tool.started', payload: { itemId: 'img1', toolKind: 'imageGeneration', name: 'imageGeneration', phase: 'started', evidence: { redacted: true } } },
    { runId, missionId, occurredAt: imageAt, sourceAdapter: 'codex', type: 'tool.completed', payload: { itemId: 'img1', toolKind: 'imageGeneration', name: 'imageGeneration', status: 'completed', phase: 'completed', evidence: { redacted: true } } }
  ]
  const finished = normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length + closing.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
  const events = [...opening, ...image, ...after, ...finished].map((event, i) => ({ ...event, sequence: i + 1, id: `${runId}:codex:${String(i + 1)}` }))
  await ledger.appendEvents(missionId, events)
  return events.map((event) => `${event.type}${event.payload?.toolKind ? `:${String(event.payload.toolKind)}` : ''}${event.payload?.name ? `(${String(event.payload.name)})` : ''}`)
}

const seededTypes = await seedTurn({ ...TURN, startedAgoMs: 300_000 })
say(`seeded one finished Codex turn: ${seededTypes.join(' ')}`)

const drive = await startDrive({
  name: `a-codex-run-reads-as-words-${tag}`, port: 9805, workspace, profilePath, outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [CASPER], missionOwners: { [TURN.missionId]: CASPER.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2000)
  const opened = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('Mock up how'))
    if (!row) return 'no conversation row'
    row.click()
    return 'opened'
  })()`))
  check('the seeded conversation opens', opened === 'opened', opened)
  await sleep(1500)
  const line = String(await drive.evaluate(`[...document.querySelectorAll('.lc-thread .lc-steps__text')].map((el) => el.innerText.trim()).join(' || ')`))
  await drive.evaluate(openAllStepsScript())
  await sleep(700)
  const rows = JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-thread .lc-filerow')].map((row) => row.innerText.replace(/\\s+/g, ' ').trim()))`)))
  await drive.capture('the turn, its steps opened', async () => JSON.stringify({ line, rows }))
  say(`  steps line: ${line}`)
  say(`  rows: ${JSON.stringify(rows)}`)
  check('the command row reads as the shell got it, single backslashes', rows.some((text) => text.includes(SHOWN)), JSON.stringify(rows))
  check('no row carries a doubled backslash', !rows.some((text) => text.includes('\\\\')), JSON.stringify(rows))
  check('the steps line says the image tool in words', /made an image/i.test(line) && !/imageGeneration/.test(line), line)
  check('no steps line leads with a thought (0.610)', !/Thought for|Thought:/.test(line), line)
  check('the image row says what it did, not the program name', rows.some((text) => /^Made an image\b/.test(text)) && !rows.some((text) => /imageGeneration/.test(text)), JSON.stringify(rows))
  say(failures === 0 ? '\nA CODEX RUN READS AS WORDS PASSED' : `\nA CODEX RUN READS AS WORDS: ${String(failures)} FAILED`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever the last build wrote to out/'}. One seeded Codex turn with an escaped PowerShell command, an imageGeneration item and a plan.`, extra: `Checks failed: ${String(failures)}` })
  process.exitCode = failures === 0 ? 0 : 1
}
