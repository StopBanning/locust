// A judge reads a comparison's answers and gives its view (0.520).
//
//   LOCUST_SPEND=1 node _tools/drive-a-judge-reads-the-answers.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-01, on Artificial Analysis's Optima: "might help improve our
// compare mode". Optima grades answers with a judge model. A comparison of two
// seeded answers (one names a price, one does not), not yet decided: "Ask a
// judge" must offer a model that is not one of the two, run once on Codex
// (one short read-only run: Codex quota, which may be spent), show its view
// under the answers, keep nothing itself, and never list the judge's run as a
// conversation.

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
const workspace = await scratchRepository('locust-drive-judge-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-judge-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
// The chat box is on Codex, so Codex is the judge offered first: not one of the two compared.
const TEAMMATE = { teammateId: 'tm_atlas0000000000000000', name: 'Atlas', hue: 'blue', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }
const ASK = 'Which CRM should a five-person team buy? One sentence.'
const ANSWERS = {
  a: ['opencode/mimo-v2.6-flash-free', 'Mimo V2.6 Flash', 'HubSpot Starter: $20 per seat a month, so $100 for five, and it covers email and deals.'],
  b: ['opencode/ling-3.0-flash-fin-free', 'Ling 3.0 Flash', 'Some CRM would probably be good for a small team.']
}
const slots = []
const owners = {}
let n = 0
const at = new Date(Date.now() - 900_000).toISOString()
for (const [slot, [model, label, text]] of Object.entries(ANSWERS)) {
  n += 1
  const missionId = `mission_5e000000-0000-4000-8000-0007${String(n).padStart(8, '0')}`
  const runId = `run_5e0007${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt: ASK,
    runtime: 'opencode', model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 60 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
  owners[missionId] = TEAMMATE.teammateId
  slots.push({ slot, route: { runtime: 'opencode', model, label }, missionIds: [missionId] })
}
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares: [{ compareId: 'cmp_judge', teammateId: TEAMMATE.teammateId, prompt: ASK, createdAt: at, slots }] }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `judge-${tag}`,
  port: 9842,
  workspace,
  profilePath,
  spends: true,
  outPath: join(recordRoot('a-judge-reads-the-answers-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const state = `JSON.stringify({
  ask: !!document.querySelector('.lc-compare__judgeask'),
  choices: [...document.querySelectorAll('.lc-compare__judgepick option')].map((o) => o.innerText.trim()),
  groups: [...document.querySelectorAll('.lc-compare__judgepick optgroup')].map((g) => g.label),
  view: document.querySelector('.lc-compare__judge')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  kept: document.querySelector('.lc-compare__kept')?.innerText ?? '',
  label: document.querySelector('.lc-compare__judge .lc-compare__recordlabel')?.innerText ?? '',
  said: document.querySelector('.lc-compare__judgesaid')?.innerText.trim() ?? '',
  badges: [...document.querySelectorAll('.lc-compare__head')].map((h, i) => (h.querySelector('.lc-compare__judgebadge') ? 'abc'[i] : '')).join(''),
  sidebar: [...document.querySelectorAll('.lc-convrow button.lc-conv')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim()),
  // Its scorecard (0.759): the columns it scored, its points, each cell's verdict, and each column's score.
  scorecard: (() => {
    const card = document.querySelector('.lc-compare__scorecard')
    if (!card) return null
    return {
      heads: [...card.querySelectorAll('thead th')].map((th) => th.innerText.trim()),
      points: [...card.querySelectorAll('tbody th')].map((th) => th.innerText.trim()),
      verdicts: [...card.querySelectorAll('tbody td')].map((td) => td.querySelector('.lc-compare__verdict')?.innerText.trim() ?? ''),
      scores: [...card.querySelectorAll('tfoot tr:first-child td')].map((td) => td.innerText.trim())
    }
  })()
})`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  const before = JSON.parse(String(await drive.capture('the comparison, answers in', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM should/i.test(r.innerText))?.click()
    // Until the agents are found: before then only the two compared can judge.
    for (let i = 0; i < 80 && document.querySelectorAll('.lc-compare__judgepick option').length <= 2; i += 1) await new Promise((r) => setTimeout(r, 250))
    return ${state}
  })()`))))
  check('with the answers in, it offers a judge, and not one of the two compared first', before.ask && before.choices.length > 0 && !/Mimo V2.6|Ling 3.0/.test(before.choices[0] ?? ''), JSON.stringify(before.choices.slice(0, 4)))
  check('every model it can be is listed, under its agent (0.554)', before.groups.length >= 2 && before.choices.length > 16, `${before.choices.length} models; ${JSON.stringify(before.groups)}`)
  await drive.capture('the judge asked', () => drive.evaluate(`(async () => {
    const box = document.querySelector('.lc-compare__judgecriteria')
    // A box of one point a line since 0.759 (a textarea): its own prototype's setter, whichever element it is.
    const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(box), 'value').set
    setter.call(box, 'Names a price for five seats\\nSays which CRM to buy')
    box.dispatchEvent(new Event('input', { bubbles: true }))
    // The judge on Codex, as this drive was written for: the first offered is now a free model that answered here
    // first, and a free model rate-limited for the day leaves no judge to read (2026-10-12).
    const pick = document.querySelector('.lc-compare__judgepick')
    const codex = [...(pick?.querySelectorAll('optgroup') ?? [])].find((group) => group.label === 'Codex CLI')?.querySelector('option')
    if (pick && codex) {
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(pick, codex.value)
      pick.dispatchEvent(new Event('change', { bubbles: true }))
    }
    await new Promise((r) => setTimeout(r, 200))
    ;[...document.querySelectorAll('.lc-compare__judgeask button')].find((b) => b.innerText.trim() === 'Judge')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return ${state}
  })()`))
  // 0.554: while it reads, its steps and words show as they come, as any reply's do.
  let after = { view: '', label: '', said: '' }
  let shownWhileReading = ''
  for (let waited = 0; waited < 240_000; waited += 1500) {
    await sleep(1500)
    after = JSON.parse(String(await drive.evaluate(state)))
    if (/reading/i.test(after.label) && after.said.length > 0 && shownWhileReading === '') shownWhileReading = after.said.slice(0, 200)
    if (after.label.length > 0 && !/reading/i.test(after.label) && (after.said.length > 0 || /did not answer/.test(after.view))) break
  }
  await drive.capture('the judge\'s view', () => drive.evaluate(`document.querySelector('.lc-compare__judge')?.scrollIntoView(); 1`))
  check('the judge\'s view is shown under the answers, under the model that judged', /^THE JUDGE'S VIEW · .*(Codex|GPT|Account default)/i.test(after.view) && /Answer A/.test(after.view) && /Answer B/.test(after.view), after.view.slice(0, 400))
  check('it says which it would keep, and says it is a view, not a decision', /keep/i.test(after.view) && /you keep the answer/.test(after.view), after.view.slice(-200))
  check('while it read, what it was doing showed as it came, not a bare "Reading the answers"', shownWhileReading.length > 0, shownWhileReading)
  const named = [...after.said.matchAll(/(\bnot\s+|n't\s+)?\bkeep:?[\s*_]*answer\s+([a-c])\b/gi)].at(-1)
  const pick = named === undefined || named[1] !== undefined ? '' : named[2].toLowerCase()
  check('the column it would keep wears "Judge\'s pick", and only that one', pick !== '' && after.badges === pick, `said ${pick || 'none'}, marked ${after.badges || 'none'}`)
  // Its scorecard (0.759): A names a price and says which to buy; B does neither.
  const card = after.scorecard
  check('it scores both answers on both points, under their columns', card !== null && card.heads.length === 3 && card.heads[0] === 'What a good answer does' && JSON.stringify(card.points) === JSON.stringify(['Names a price for five seats', 'Says which CRM to buy']) && card.verdicts.length === 4 && card.verdicts.every((verdict) => verdict === 'met' || verdict === 'not met'), JSON.stringify(card))
  check('the answer that names a price meets that point, and the one that does not, does not', card !== null && card.verdicts[0] === 'met' && card.verdicts[1] === 'not met', JSON.stringify(card?.verdicts))
  check('each column has its score', card !== null && card.scores.length === 2 && card.scores.every((score) => /^\d of 2$/.test(score)), JSON.stringify(card?.scores))
  check('and the judge\'s words carry no scorecard line of their own', !/^\s*\**\s*(SCORE|POINT)\b/m.test(after.said), after.said.slice(-300))
  check('it kept nothing itself', after.kept === '', after.kept)
  check('and its run is not listed as a conversation', after.sidebar.length === 1, JSON.stringify(after.sidebar))
  const note = String(await drive.evaluate("document.querySelector('.lc-continuation')?.innerText ?? ''"))
  check('an ask in the undecided comparison is not said to go to one AI agent with a summary', note === '', note)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded comparison of two answers, judged once on Codex.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
