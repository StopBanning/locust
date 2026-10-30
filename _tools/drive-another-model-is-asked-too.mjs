// A comparison asks one more model the same question (0.523).
//
//   LOCUST_SPEND=1 node _tools/drive-another-model-is-asked-too.mjs [--packaged <exe>] [--tag <name>]
//
// After Artificial Analysis's Optima, which runs a benchmark again on each new
// model. A decided comparison of two seeded answers: "Ask another model"
// must offer only models not in it, and "Ask it too" on Codex (GPT-6-Luna,
// low: Codex quota, which may be spent) adds a third column that answers the
// same question. What was kept stays kept, and the record counts the new one.

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
const workspace = await scratchRepository('locust-drive-add-model-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-add-model-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_atlas0000000000000000', name: 'Atlas', hue: 'blue', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'gpt-6-luna', mode: 'ask', effort: 'low' } }
const ASK = 'Name one CRM a five-person team could buy. Reply with the name only.'
const ANSWERS = {
  a: ['opencode/mimo-v2.6-flash-free', 'Mimo V2.6 Flash', 'HubSpot'],
  b: ['opencode/ling-3.0-flash-fin-free', 'Ling 3.0 Flash', 'Pipedrive']
}
const slots = []
const owners = {}
let n = 0
const at = new Date(Date.now() - 900_000).toISOString()
for (const [slot, [model, label, text]] of Object.entries(ANSWERS)) {
  n += 1
  const missionId = `mission_5e000000-0000-4000-8000-0008${String(n).padStart(8, '0')}`
  const runId = `run_5e0008${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt: ASK,
    runtime: 'opencode', model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 10 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
  owners[missionId] = TEAMMATE.teammateId
  slots.push({ slot, route: { runtime: 'opencode', model, label }, missionIds: [missionId] })
}
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares: [{ compareId: 'cmp_add', teammateId: TEAMMATE.teammateId, prompt: ASK, createdAt: at, slots, kept: { slot: 'a', at } }] }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `another-model-${tag}`,
  port: 9845,
  workspace,
  profilePath,
  spends: true,
  outPath: join(recordRoot('another-model-is-asked-too-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const state = `JSON.stringify({
  heads: [...document.querySelectorAll('.lc-compare__head')].map((h) => h.innerText.replace(/\\s+/g, ' ').trim()),
  cells: [...document.querySelectorAll('.lc-compare__cell')].map((c) => c.innerText.replace(/\\s+/g, ' ').trim()),
  choices: [...document.querySelectorAll('select[aria-label="The model to ask too"] option')].map((o) => o.innerText.trim()),
  kept: [...document.querySelectorAll('.lc-compare__kept')].length,
  record: [...document.querySelectorAll('.lc-compare__record tbody tr')].map((tr) => tr.innerText.replace(/\\s+/g, ' ').trim())
})`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  const before = JSON.parse(String(await drive.capture('the kept comparison', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Open the comparison')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 800))
    return ${state}
  })()`))))
  check('it offers another model, never one already in it', before.choices.length > 0 && !before.choices.some((label) => /Mimo V2.6 Flash|Ling 3.0 Flash/.test(label)) && before.choices.some((label) => /Codex/.test(label)), JSON.stringify(before.choices.slice(0, 5)))
  await drive.capture('Ask it too, on Codex', () => drive.evaluate(`(async () => {
    const select = document.querySelector('select[aria-label="The model to ask too"]')
    const codex = [...select.options].find((o) => /Codex/.test(o.innerText))
    const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set
    setter.call(select, codex.value)
    select.dispatchEvent(new Event('change', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Ask it too')?.click()
    await new Promise((r) => setTimeout(r, 2000))
    return 1
  })()`))
  let after = before
  for (let waited = 0; waited < 240_000; waited += 4000) {
    await sleep(4000)
    after = JSON.parse(String(await drive.evaluate(state)))
    if (after.heads.length === 3 && /done/.test(after.heads[2] ?? '')) break
  }
  await sleep(1500)
  after = JSON.parse(String(await drive.capture('the third column, answered', () => drive.evaluate(state))))
  check('a third column, on Codex, answered the same question', after.heads.length === 3 && /Luna/.test(after.heads[2] ?? '') && !/^Codex CLI \//.test(after.heads[2] ?? '') && /done/.test(after.heads[2] ?? '') && (after.cells[2] ?? '').length > 0, JSON.stringify({ heads: after.heads, cells: after.cells }))
  check('what was kept stays kept', after.kept === 1, String(after.kept))
  check('and the record counts the new model, not kept', after.record.length === 3 && after.record.some((row) => /Luna|Codex/.test(row) && /0 of 1/.test(row)), JSON.stringify(after.record))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A decided comparison of two seeded answers; Codex asked too.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
