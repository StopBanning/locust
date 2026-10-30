// An earlier version of a document reads as the document it was (0.517).
//
//   node _tools/drive-an-earlier-version-reads.mjs [--packaged <exe>] [--tag <name>]
//
// Sol, Workflow 2 (work that is not code): an older version of a document
// opened as a code diff. A seeded three-turn conversation makes plan.md,
// changes "Monday" to "Tuesday", then adds a line; the last reply hands the
// file over. In the viewer, turn 1 must read as the document it was (Monday,
// no third line), with what that turn changed one press away; "Now" is the
// file as it is. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-earlier-version-ws-')
await writeFile(join(workspace, 'plan.md'), '# Plan\n\nShip on Tuesday\n\nTell the team\n', 'utf8')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-earlier-version-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_quill0000000000000000', name: 'Quill', hue: 'violet', role: 'Custom', roleTitle: 'Docs', createdAt: '2026-09-05T05:00:00.000Z' }
const PROCESS = (at) => ({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at })
const TURNS = [
  ['Write a short plan for the launch in plan.md', 'diff --git a/plan.md b/plan.md\nnew file mode 100644\n--- /dev/null\n+++ b/plan.md\n@@ -0,0 +1,3 @@\n+# Plan\n+\n+Ship on Monday\n', 'Wrote plan.md.'],
  ['Move it to Tuesday', 'diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,3 +1,3 @@\n # Plan\n \n-Ship on Monday\n+Ship on Tuesday\n', 'Moved to Tuesday.'],
  ['Add that we tell the team', 'diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,3 +1,5 @@\n # Plan\n \n Ship on Tuesday\n+\n+Tell the team\n', 'Added it.\n\n<locust-file>\nplan.md :: the plan\n</locust-file>']
]
const owners = {}
let previous
for (const [index, [prompt, diff, reply]] of TURNS.entries()) {
  const missionId = `mission_5e000000-0000-4000-8000-0003000000${String(index).padStart(2, '0')}`
  const runId = `run_5e0003${String(index)}`
  const at = new Date(Date.now() - (TURNS.length - index) * 300_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at,
    ...(previous === undefined ? {} : { continuesFrom: { missionId: previous, checkpointEpoch: 1, reason: 'follow-up' } })
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(index)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  const added = diff.split('\n').filter((line) => line.startsWith('+') && !line.startsWith('+++')).length
  const removed = diff.split('\n').filter((line) => line.startsWith('-') && !line.startsWith('---')).length
  await ledger.appendEvents(missionId, [
    event(1, 'tool.started', { itemId: `t${String(index)}`, toolKind: 'file_change', name: 'write', phase: 'started', command: 'plan.md' }),
    event(2, 'tool.completed', { itemId: `t${String(index)}`, toolKind: 'file_change', name: 'write', phase: 'completed', command: 'plan.md', patch: { text: diff, added, removed, truncated: false } }),
    event(3, 'message.delta', { itemId: `a${String(index)}`, operation: 'append', text: reply, final: true }),
    event(4, 'run.completed', { usage: { inputTokens: 900, outputTokens: 40 }, process: PROCESS(at) })
  ])
  owners[missionId] = TEAMMATE.teammateId
  previous = missionId
}
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `earlier-version-reads-${tag}`,
  port: 9837,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('an-earlier-version-reads-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const viewer = `JSON.stringify({
  note: document.querySelector('.lc-viewer .lc-viewer__versionnote')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  prose: document.querySelector('.lc-viewer .lc-viewer__scroll .lc-viewer__prose, .lc-viewer .lc-viewer__prose')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  diff: !!document.querySelector('.lc-viewer .lc-diff, .lc-viewer [class*="lc-diff"]:not([class*="lc-docchange"])'),
  // 0.530: a document's change reads as its words, each passage as a reader sees it.
  words: [...document.querySelectorAll('.lc-viewer .lc-docchange__passage')].map((p) => p.innerText.replace(/\\s+/g, ' ').trim()),
  struck: [...document.querySelectorAll('.lc-viewer .lc-docchange del')].map((d) => d.innerText.trim()),
  marked: [...document.querySelectorAll('.lc-viewer .lc-docchange ins')].map((d) => d.innerText.trim()),
  chips: [...document.querySelectorAll('.lc-viewer .lc-viewer__version')].map((b) => b.innerText.trim())
})`
const press = (label) => `(async () => {
  ;[...document.querySelectorAll('.lc-viewer button')].find((b) => b.innerText.trim() === ${JSON.stringify(label)})?.click()
  await new Promise((r) => setTimeout(r, 600))
  return ${viewer}
})()`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /launch|plan|team/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    ;[...document.querySelectorAll('button')].find((b) => /plan\\.md/.test(b.innerText) && /the plan/.test(b.innerText))?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-viewer'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 600))
  })()`)
  await sleep(300)
  const now = JSON.parse(String(await drive.capture('plan.md, now', () => drive.evaluate(viewer))))
  check('the file opens, with a chip for each turn that changed it, and Now', JSON.stringify(now.chips) === JSON.stringify(['1', '2', '3', 'Now']), JSON.stringify(now))
  const first = JSON.parse(String(await drive.capture('turn 1, as it was', () => drive.evaluate(press('1')))))
  check('turn 1 reads as the document it was: Monday, and no third line', /The file as it was after turn 1\./.test(first.note) && /Ship on Monday/.test(first.prose) && !/Tuesday|Tell the team/.test(first.prose) && !first.diff, JSON.stringify(first))
  const change = JSON.parse(String(await drive.capture('turn 1, what it changed', () => drive.evaluate(press('What turn 1 changed')))))
  check('what that turn changed is one press away, in the document\'s words: it added the heading and the line', /What turn 1 changed/.test(change.note) && !change.diff && JSON.stringify(change.marked) === JSON.stringify(['Plan', 'Ship on Monday']), JSON.stringify(change))
  const second = JSON.parse(String(await drive.capture('turn 2, as it was', () => drive.evaluate(press('2')))))
  check('turn 2 reads as it was: Tuesday, still no third line', /Ship on Tuesday/.test(second.prose) && !/Tell the team/.test(second.prose), JSON.stringify(second))
  const moved = JSON.parse(String(await drive.capture('turn 2, what it changed, in words', () => drive.evaluate(press('What turn 2 changed')))))
  check('turn 2\'s change reads "Ship on Monday Tuesday", Monday struck and Tuesday marked, no @@ in sight', JSON.stringify(moved.struck) === '["Monday"]' && JSON.stringify(moved.marked) === '["Tuesday"]' && !moved.diff, JSON.stringify(moved))
  const lines = JSON.parse(String(await drive.capture('the lines, one press away', () => drive.evaluate(press('Show the lines that changed')))))
  check('the lines that changed are one press away', lines.diff && lines.words.length === 0, JSON.stringify(lines))
  await drive.evaluate(press('Show it as words'))
  const back = JSON.parse(String(await drive.capture('Now again', () => drive.evaluate(press('Now')))))
  check('Now is the file as it is', /Ship on Tuesday/.test(back.prose) && /Tell the team/.test(back.prose), JSON.stringify(back))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Three seeded turns change plan.md; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
