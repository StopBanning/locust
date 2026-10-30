// A whole web page in a reply runs on a stage (0.553).
//
//   node _tools/drive-a-reply-page-runs.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-02, over three arcade games that arrived as code blocks in
// an Ask comparison: "we got to find a way to have these display and work
// properly". One seeded conversation whose answer is a page in a code block;
// its script paints a canvas, so a capture shows it ran. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-reply-page-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-reply-page-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const PAGE = [
  '<!DOCTYPE html>',
  '<html><head><meta charset="utf-8"><title>Locust Dodge</title>',
  '<style>body{margin:0;background:#10140e;display:flex;align-items:center;justify-content:center;height:100vh}</style></head>',
  '<body><canvas id="c" width="480" height="300"></canvas>',
  '<script>const x=document.getElementById("c").getContext("2d");x.fillStyle="#9be15d";x.fillRect(0,0,480,300);x.fillStyle="#10140e";x.font="bold 40px sans-serif";x.fillText("SCRIPT RAN",110,165);document.title="ran"</script>',
  '</body></html>'
].join('\n')
const ANSWER = 'Here is the game, in one file:\n\n```html\n' + PAGE + '\n```\n\nSave it as dodge.html and open it.'
const missionId = 'mission_5a000000-0000-4000-8000-000100000001'
const runId = 'run_5a0001'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'make a small browser arcade game', runtime: 'opencode', model: FREE_ROUTE.model, mode: 'ask', requestedRouteId: 'opencode',
  resolvedRouteId: 'opencode-account:default', cliVersion: null, workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const event = (sequence, type, payload) => ({ id: `event_1_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: ANSWER, final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 90, outputTokens: 400 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `a-reply-page-runs-${tag}`,
  port: 9872,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-reply-page-runs-2026-10-02'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
try {
  await drive.ready()
  await drive.resize(1209, 900)
  const seen = JSON.parse(String(await drive.capture('the page runs on a stage', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && document.querySelectorAll('.lc-conv').length < 1; i += 1) await new Promise((r) => setTimeout(r, 250))
    document.querySelector('.lc-conv')?.click()
    let frame
    for (let i = 0; i < 40 && !frame; i += 1) { await new Promise((r) => setTimeout(r, 250)); frame = document.querySelector('.lc-replypage__frame') }
    await new Promise((r) => setTimeout(r, 2000))
    frame?.scrollIntoView({ block: 'center' })
    await new Promise((r) => setTimeout(r, 600))
    const box = frame?.getBoundingClientRect()
    return JSON.stringify({
      stage: document.querySelector('.lc-replypage') !== null,
      src: frame?.getAttribute('src') ?? null,
      ratio: box ? Math.round((box.width / box.height) * 100) / 100 : null,
      rawCode: [...document.querySelectorAll('pre.lc-code code')].some((code) => code.textContent.includes('SCRIPT RAN')),
      after: document.body.innerText.includes('Save it as dodge.html')
    })
  })()`))))
  say(JSON.stringify(seen))
  check('the reply\'s page is on a stage', seen.stage === true, JSON.stringify(seen))
  check('served at its own locust-page address', /^locust-page:\/\//.test(seen.src ?? ''), seen.src)
  check('the stage is about 16:10', seen.ratio !== null && Math.abs(seen.ratio - 1.6) < 0.05, String(seen.ratio))
  check('its code is not also shown raw', seen.rawCode === false, JSON.stringify(seen))
  check('the words after the page are still there', seen.after === true)
  const code = JSON.parse(String(await drive.capture('the Code tab', () => drive.evaluate(`(async () => {
    [...document.querySelectorAll('.lc-replypage__tab')].find((tab) => tab.innerText.trim() === 'Code')?.click()
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({ shown: document.querySelector('.lc-replypage__code code')?.textContent.includes('SCRIPT RAN') ?? false })
  })()`))))
  check('the Code tab shows the code', code.shown === true, JSON.stringify(code))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One seeded reply with a page in it.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
