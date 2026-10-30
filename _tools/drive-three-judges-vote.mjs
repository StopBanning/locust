// THREE JUDGES VOTING (0.765): a comparison's answers scored by three judges, a point counting when two agree.
//
//   LOCUST_SPEND=1 node _tools/drive-three-judges-vote.mjs [--packaged <exe>] [--tag <name>]
//
// The canvas "Your Own Benchmark" (56riCzZ3ZzR5AQ2sg6ssJm): "Three judge models check every run against every line of
// the rubric. A line counts when two of three agree." A seeded comparison of two answers (one names a price, one does
// not): Ask three judges to vote, the points written, Judge waits for them; three judges read at once -- Codex (one
// short read-only run: Codex quota, which may be spent) and two free OpenCode models, two OpenCode processes at once
// as a benchmark's judges are -- and the table shows each point as their vote, a dot a judge. No judge's run is a
// conversation; nothing is kept.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-panel-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-panel-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_atlas0000000000000000', name: 'Atlas', hue: 'blue', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }
const ASK = 'Which CRM should a five-person team buy? One sentence.'
const POINTS = ['Names a price for five seats', 'Says which CRM to buy']
// The first counts twice (0.767, weighted points): written with its weight, scored without it.
const WRITTEN = [`${POINTS[0]} ×2`, POINTS[1]]
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
  const missionId = `mission_5e000000-0000-4000-8000-0765${String(n).padStart(8, '0')}`
  const runId = `run_5e0765${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt: ASK,
    runtime: 'opencode', model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 60, usd: 0 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
  owners[missionId] = TEAMMATE.teammateId
  slots.push({ slot, route: { runtime: 'opencode', model, label }, missionIds: [missionId] })
}
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares: [{ compareId: 'cmp_panel', teammateId: TEAMMATE.teammateId, prompt: ASK, createdAt: at, slots }] }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `panel-${tag}`,
  port: 9846,
  workspace,
  profilePath,
  spends: true,
  outPath: join(recordRoot('three-judges-vote-2026-10-12'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const state = `JSON.stringify({
  // The judge's ask is the FIRST .lc-compare__judgeask: Ask another model below it wears the same class.
  askLabel: document.querySelector('.lc-compare__judgeask .lc-compare__recordlabel')?.innerText.trim() ?? '',
  pickers: [...(document.querySelector('.lc-compare__judgeask')?.querySelectorAll('select') ?? [])].map((s) => ({ label: s.getAttribute('aria-label'), value: s.value, text: s.selectedOptions[0]?.innerText.trim() ?? '' })),
  judgeDisabled: [...(document.querySelector('.lc-compare__judgeask')?.querySelectorAll('button') ?? [])].find((b) => /^(Judge|Asking…)$/.test(b.innerText.trim()))?.disabled ?? null,
  note: document.querySelector('.lc-compare__judgeask .lc-compare__judgenote')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  label: document.querySelector('.lc-compare__judge .lc-compare__recordlabel')?.innerText.trim() ?? '',
  view: document.querySelector('.lc-compare__judge')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  said: document.querySelector('.lc-compare__judgesaid')?.innerText.trim() ?? '',
  badges: [...document.querySelectorAll('.lc-compare__judgebadge')].map((b) => b.innerText.trim()),
  kept: document.querySelector('.lc-compare__kept')?.innerText ?? '',
  sidebar: [...document.querySelectorAll('.lc-convrow button.lc-conv')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim()),
  scorecard: (() => {
    const card = document.querySelector('.lc-compare__scorecard')
    if (!card) return null
    return {
      points: [...card.querySelectorAll('tbody th')].map((th) => th.firstChild?.textContent?.trim() ?? ''),
      weights: [...card.querySelectorAll('tbody th')].map((th) => th.querySelector('.lc-compare__weight')?.innerText.trim() ?? ''),
      verdicts: [...card.querySelectorAll('tbody td')].map((td) => td.querySelector('.lc-compare__verdict')?.firstChild?.textContent?.trim() ?? ''),
      votes: [...card.querySelectorAll('tbody td')].map((td) => td.querySelector('.lc-compare__votes')?.getAttribute('aria-label') ?? ''),
      dots: [...card.querySelectorAll('tbody td')].map((td) => [...td.querySelectorAll('.lc-compare__vote')].map((dot) => (dot.classList.contains('is-met') ? '●' : '○')).join('')),
      scores: [...card.querySelectorAll('tfoot tr:first-child td')].map((td) => td.innerText.trim())
    }
  })(),
  pageScrollsSideways: document.documentElement.scrollWidth > document.documentElement.clientWidth
})`
const read = async () => JSON.parse(String(await drive.evaluate(state)))
const pressLink = (words) => drive.evaluate(`(() => {
  const found = [...(document.querySelector('.lc-compare__judgeask')?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === ${JSON.stringify(words)})
  if (!found) return 'no ' + ${JSON.stringify(words)}
  found.click()
  return 'pressed'
})()`)

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM should/i.test(r.innerText))?.click()
    for (let i = 0; i < 160 && document.querySelectorAll('.lc-compare__judgeask')[0]?.querySelectorAll('option').length <= 8; i += 1) await new Promise((r) => setTimeout(r, 250))
    document.querySelector('.lc-compare__judgeask')?.scrollIntoView({ block: 'center' })
    return 1
  })()`)
  const one = await read()
  check('one judge is asked as before, with three a press away', one.askLabel === 'ASK A JUDGE' && one.pickers.length === 1 && /Ask three judges to vote/.test(one.note), `${one.askLabel} · ${String(one.pickers.length)} picker · ${one.note.slice(-60)}`)
  say(`  three judges: ${String(await pressLink('Ask three judges to vote'))}`)
  await sleep(400)
  const three = await drive.capture('three judges, before the points', () => read())
  const keys = three.pickers.map((picker) => picker.value)
  check('three judges: three pickers, each a different model', three.askLabel === 'ASK THREE JUDGES' && three.pickers.length === 3 && new Set(keys).size === 3, JSON.stringify(three.pickers))
  const offered = JSON.parse(String(await drive.evaluate(`JSON.stringify([...(document.querySelector('.lc-compare__judgeask')?.querySelectorAll('select')[1]?.querySelectorAll('option') ?? [])].map((o) => o.value))`)))
  say(`  the second judge's picker offers: ${offered.join(', ')}`)
  const freeOthers = offered.filter((key) => /^opencode:.+-free$/.test(key) && !/mimo-v2\.6-flash|ling-3\.0-flash-fin/.test(key))
  check('the two beside the first are never the two compared, and are free ones when there are', keys.slice(1).every((key) => !/mimo-v2\.6-flash|ling-3\.0-flash-fin/.test(key)) && (freeOthers.length === 0 || keys.slice(1).some((key) => /-free$/.test(key))), `${JSON.stringify(keys)} · free others offered: ${String(freeOthers.length)}`)
  check('and Judge waits for the points they vote on', three.judgeDisabled === true && /They vote on the points you write/.test(three.note), `disabled ${String(three.judgeDisabled)} · ${three.note}`)

  // DRAFT THE POINTS (0.766): on the first judge's model -- Codex here, one short read-only run -- into the box.
  await drive.evaluate(`(async () => {
    const pick = document.querySelector('.lc-compare__judgeask').querySelectorAll('select')[0]
    const codex = [...pick.querySelectorAll('option')].find((o) => o.value === 'codex:account-default')
    if (codex) {
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(pick, codex.value)
      pick.dispatchEvent(new Event('change', { bubbles: true }))
    }
    await new Promise((r) => setTimeout(r, 300))
    return 1
  })()`)
  const offeredDraft = String(await drive.evaluate(`document.querySelector('.lc-compare__draft')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`))
  check('with the box empty, it offers to draft the points on the first judge, by its agent\'s name', offeredDraft === 'Draft the points from your question with Codex CLI, then edit them.', offeredDraft)
  say(`  draft: ${String(await pressLink('Draft the points'))}`)
  let drafting = ''
  let drafted = ''
  for (let waited = 0; waited < 240_000; waited += 1000) {
    await sleep(1000)
    const now = JSON.parse(String(await drive.evaluate(`JSON.stringify({ line: document.querySelector('.lc-compare__draft')?.innerText.trim() ?? '', box: document.querySelector('.lc-compare__judgecriteria')?.value ?? '', problem: document.querySelector('.lc-compare__judgeask .lc-compare__problem')?.innerText ?? '' })`)))
    if (/is drafting the points/.test(now.line) && drafting === '') drafting = now.line
    if (now.box.trim().length > 0 || now.problem.length > 0) {
      drafted = now.box.trim().length > 0 ? now.box : `PROBLEM: ${now.problem}`
      break
    }
  }
  await drive.capture('the points, drafted', () => read())
  const draftLines = drafted.split('\n').filter((line) => line.trim().length > 0)
  check('while it drafts, it says so', /is drafting the points from your question/.test(drafting), drafting)
  check('the box fills with three to five points to edit, none of them a scorecard line', draftLines.length >= 3 && draftLines.length <= 5 && draftLines.every((line) => !/^(POINT|SCORE)\b/i.test(line)), drafted)
  const sidebarNow = JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-convrow button.lc-conv')].map((b) => b.innerText.trim()))`)))
  check('and the drafting run is not listed as a conversation', sidebarNow.length === 1, JSON.stringify(sidebarNow))

  // The first on Codex; the other two on free OpenCode models that are not the two compared.
  const picked = JSON.parse(String(await drive.evaluate(`(async () => {
    const set = (select, value) => {
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(select, value)
      select.dispatchEvent(new Event('change', { bubbles: true }))
    }
    const pickers = () => [...document.querySelector('.lc-compare__judgeask').querySelectorAll('select')]
    const codex = [...pickers()[0].querySelectorAll('option')].find((o) => o.value === 'codex:account-default') ?? [...pickers()[0].querySelectorAll('optgroup')].find((g) => g.label === 'Codex CLI')?.querySelector('option')
    if (codex) set(pickers()[0], codex.value)
    await new Promise((r) => setTimeout(r, 200))
    for (const index of [1, 2]) {
      const free = [...pickers()[index].querySelectorAll('option')].find((o) => o.value.startsWith('opencode:') && o.value.endsWith('-free') && !/mimo-v2.6-flash|ling-3.0-flash-fin/.test(o.value) && !pickers().some((other, at) => at !== index && other.value === o.value))
      if (free) set(pickers()[index], free.value)
      await new Promise((r) => setTimeout(r, 200))
    }
    const box = document.querySelector('.lc-compare__judgecriteria')
    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(box), 'value').set.call(box, ${JSON.stringify(WRITTEN.join('\n'))})
    box.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    return JSON.stringify(pickers().map((s) => s.value))
  })()`)))
  const ready = await drive.capture('three judges, ready', () => read())
  // What React holds, not only what the DOM shows: the judges named while they read are the ones asked.
  check('with the points written, Judge can be pressed, the first on Codex', ready.judgeDisabled === false && picked[0]?.startsWith('codex:') === true && new Set(picked).size === 3 && picked.slice(1).every((value) => !/mimo-v2\.6-flash|ling-3\.0-flash-fin/.test(value)), JSON.stringify(picked))
  say(`  judge: ${String(await pressLink('Judge'))}`)

  let after = await read()
  let readingLabel = ''
  for (let waited = 0; waited < 420_000; waited += 2000) {
    await sleep(2000)
    after = await read()
    if (/reading/i.test(after.label) && readingLabel === '') readingLabel = after.label
    if (after.label.length > 0 && !/reading/i.test(after.label)) break
  }
  await drive.capture("the judges' view", () => drive.evaluate(`document.querySelector('.lc-compare__judge')?.scrollIntoView(); 1`))
  check('while they read, the view names all three', /^THE JUDGES' VIEW · .+, .+ and .+ · reading$/.test(readingLabel), readingLabel)
  // The judges asked are the three the pickers showed when Judge was pressed.
  // An account's default is named by its agent in the heading: "Codex CLI / Account Default" reads "Codex CLI".
  // By model, from the picker's own text; an account's default is named by its agent, so it is matched by key.
  const shownNames = ready.pickers.map((picker) => (picker.value.endsWith(':account-default') ? '' : picker.text.split(' / ').at(-1) ?? picker.text))
  check('the judges asked are the three the pickers showed', shownNames.length === 3 && shownNames.every((name) => readingLabel.includes(name)) && /^THE JUDGES' VIEW · Codex CLI, /.test(readingLabel), `${JSON.stringify(ready.pickers.map((picker) => picker.value))} vs ${readingLabel}`)
  check("once they are done, it is the judges' view", /^THE JUDGES' VIEW · .+, .+ and .+$/.test(after.label) && !/reading/.test(after.label), after.label)
  const card = after.scorecard
  check('every point of both answers is their vote, a dot a judge that answered', card !== null && JSON.stringify(card.points) === JSON.stringify(POINTS) && card.verdicts.length === 4 && card.dots.every((dots) => dots.length >= 2 && dots.length <= 3), JSON.stringify(card))
  check('the point written with ×2 says so, and counts twice in the score', card !== null && JSON.stringify(card.weights) === JSON.stringify(['×2', '']) && JSON.stringify(card.scores) === JSON.stringify(['3 of 3', '0 of 3']), JSON.stringify({ weights: card?.weights, scores: card?.scores }))
  check('each verdict is what most of them said', card !== null && card.verdicts.every((verdict, index) => {
    const dots = card.dots[index] ?? ''
    const met = [...dots].filter((dot) => dot === '●').length
    return verdict === (met * 2 > dots.length ? 'met' : 'not met')
  }), JSON.stringify({ verdicts: card?.verdicts, dots: card?.dots }))
  check('the answer that names a price meets that point, and the one that does not, does not', card !== null && card.verdicts[0] === 'met' && card.verdicts[1] === 'not met', JSON.stringify(card?.verdicts))
  check('the table says what it is, and whose words follow it', /a point counts when more of them say met than not/.test(after.view) && /The words below are .+'s\./.test(after.view), after.view.slice(0, 600))
  check('their words carry no scorecard line', !/^\s*\**\s*(SCORE|POINT)\b/m.test(after.said), after.said.slice(-300))
  check('it kept nothing itself', after.kept === '', after.kept)
  check('and no judge\'s run is listed as a conversation', after.sidebar.length === 1, JSON.stringify(after.sidebar))
  check('nothing runs off the page', !after.pageScrollsSideways)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded comparison of two answers, judged by three: Codex and two free OpenCode models.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
