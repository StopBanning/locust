// THREE CLAUDE JUDGES ON THE API CREDITS (0.810): a real file-changing comparison of two free OpenCode models, then
// Ask three judges to vote -- Claude Haiku, Sonnet and Opus 5.5, added as Your own models on Anthropic's
// OpenAI-compatible API with the key from Colin's masked pop-up. The live test the judge-reads-the-work handoff could
// not run (a paid judge, the panel), on the promotional API credits Colin set aside for tests (expire 2026-11-06).
//
//   LOCUST_SPEND=1 LOCUST_OWN_MODEL_KEY=<from the pop-up, never typed here> node _tools/drive-judges-on-api-credits.mjs
//        [--packaged <exe>] [--tag <name>]
//
// The key is read once, removed from this process's environment, typed only into Locust's own masked Key field in
// this drive's throwaway profile (kept there encrypted by Windows), and never printed. finish() deletes the profile.
import { readFile, writeFile } from 'node:fs/promises'
import { join, resolve, sep } from 'node:path'
import { FREE_ROUTE, git, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const KEY = process.env.LOCUST_OWN_MODEL_KEY || undefined
delete process.env.LOCUST_OWN_MODEL_KEY
if (KEY === undefined) { say('needs LOCUST_OWN_MODEL_KEY, from the masked pop-up'); process.exit(1) }
const BASE = 'https://api.anthropic.com/v1'
const JUDGES = [
  { name: 'Claude Haiku 5.5 (API)', model: 'claude-haiku-5-5' },
  { name: 'Claude Sonnet 5.5 (API)', model: 'claude-sonnet-5-5' },
  { name: 'Claude Opus 5.5 (API)', model: 'claude-opus-5-5' }
]
const arg = (name) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined

// The server first: a drive against nothing proves nothing.
const listed = await fetch(`${BASE}/models`, { headers: { 'x-api-key': KEY, 'anthropic-version': '2023-06-01' } }).then((response) => response.json()).catch(() => undefined)
const ids = Array.isArray(listed?.data) ? listed.data.map((entry) => entry.id) : []
const missing = JUDGES.filter((judge) => !ids.includes(judge.model)).map((judge) => judge.model)
if (missing.length > 0) { say(`the API does not list ${missing.join(', ')}; nothing was started`); process.exit(1) }

const workspace = await scratchRepository('locust-api-judges-ws-')
await writeFile(join(workspace, 'sum.mjs'), 'export const sum = (a, b) => a - b\n')
await writeFile(join(workspace, 'sum.test.mjs'), "import { test } from 'node:test'\nimport assert from 'node:assert/strict'\nimport { sum } from './sum.mjs'\ntest('adds two numbers', () => assert.equal(sum(2, 3), 5))\n")
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'A small addition bug'], workspace)
const ask = 'Fix the integer addition bug in sum.mjs. Add a negative-number regression case in sum.test.mjs. Run node --test sum.test.mjs. Do not change other files or commit. Final message: one short sentence.'
const criteria = 'Fixes sum.mjs addition\nAdds the negative-number test\nClaims agree with the actual diff'
const drive = await startDrive({
  name: 'judges-on-api-credits', port: 9832, workspace, holdCursor: true, spends: true,
  outPath: join(recordRoot('judges-on-api-credits-2026-10-21'), arg('--tag') ?? 'live'),
  ...(arg('--packaged') === undefined ? {} : { packaged: arg('--packaged') }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', route: FREE_ROUTE, createdAt: '2026-10-21T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: false, relayHopCap: 2 } }
})
let checks = 0, failures = 0, compareId
const check = (what, ok, detail) => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ' -- ' + JSON.stringify(detail).slice(0, 1200)}`)
}
const typeInto = (selector, value) => `(() => {
  const input = document.querySelector(${JSON.stringify(selector)})
  if (!input) return false
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(value)})
  input.dispatchEvent(new Event('input', { bubbles: true }))
  return true
})()`
const field = (index) => `.lc-ownmodel__form label:nth-of-type(${String(index)}) input`
const press = (label, waitSeconds) => `(async () => {
  const button = [...document.querySelectorAll('.lc-ownmodel__actions button')].find((b) => b.textContent.trim() === ${JSON.stringify(label)})
  if (!button || button.disabled) return 'no usable ' + ${JSON.stringify(label)}
  button.click()
  for (let i = 0; i < ${String(waitSeconds * 4)}; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const said = document.querySelector('.lc-ownmodel__form .lc-ownmodel__said')?.textContent
    if (said && !said.startsWith('Testing')) return said
  }
  return 'nothing said'
})()`
const comparisons = async () => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listCompares()))()`)))
const mission = async (id) => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.readMission(${JSON.stringify(id)})))()`)))
const view = `JSON.stringify({ text: document.querySelector('.lc-compare')?.innerText ?? '', said: document.querySelector('.lc-compare__judge')?.innerText ?? '', sideways: document.documentElement.scrollWidth > innerWidth })`
const ended = (id) => `(async () => {
  const read = await window.desktop.readMission(${JSON.stringify(id)})
  return read.ok && read.data.mission.events.some((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type))
})()`
try {
  await drive.ready()
  await drive.resize(1440, 900)

  // Your own models: the three judges, each tested against the real API before it is added.
  await drive.evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '3', ctrlKey: true, bubbles: true }))
    await new Promise((r) => setTimeout(r, 900))
    ;[...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Your own models')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-ownmodel__form'); i += 1) await new Promise((r) => setTimeout(r, 150))
  })()`)
  for (const judge of JUDGES) {
    await drive.evaluate(typeInto(field(1), judge.name))
    await drive.evaluate(typeInto(field(2), judge.model))
    await drive.evaluate(typeInto(field(3), BASE))
    await drive.evaluate(typeInto(field(4), KEY))
    const tested = String(await drive.evaluate(press('Test', 90)))
    check(`${judge.name} answers its Test on the real API`, /answered/i.test(tested), tested)
    const added = String(await drive.evaluate(press('Add model', 30)))
    check(`${judge.name} is added`, /Your models|added/i.test(added) || added === 'nothing said', added)
  }
  await drive.capture('Settings, Your own models: the three Claude judges on the API', () => drive.evaluate(`[...document.querySelectorAll('.lc-ownmodel')].map((row) => row.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')`))
  await drive.evaluate(`window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); 1`)

  // The comparison: two free OpenCode models changing separate copies.
  const catalog = JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listModels()))()`)))
  const free = (catalog.ok ? catalog.data.models : []).filter((model) => model.runtime === 'opencode' && model.id.endsWith('-free'))
  const second = free.find((model) => model.id !== FREE_ROUTE.model)
  if (!free.some((model) => model.id === FREE_ROUTE.model) || !second) throw new Error('Two advertised free OpenCode models are required.')
  const routes = [FREE_ROUTE, { runtime: 'opencode', model: second.id }].map(({ runtime, model }) => ({ runtime, model }))
  const started = JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.startCompare(${JSON.stringify({ teammateId: 'tm_wren', prompt: ask, changes: true, blind: true, routes })})))()`)))
  if (!started.ok || started.data.refused.length > 0) throw new Error(JSON.stringify(started))
  compareId = started.data.compare.compareId
  await drive.evaluate('location.reload(); 1')
  await drive.waitFor(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].some((row) => /^vs/.test(row.innerText.trim()))`, { timeoutMs: 30_000, what: 'the comparison in history' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((row) => /^vs/.test(row.innerText.trim())).click()`)
  await drive.waitForSelector('.lc-compare')
  await drive.waitFor(`(async () => {
    const listed = await window.desktop.listCompares()
    const compare = listed.ok ? listed.data.compares.find((item) => item.compareId === ${JSON.stringify(compareId)}) : undefined
    if (!compare || compare.slots.some((column) => column.missionIds.length === 0)) return false
    const turns = await Promise.all(compare.slots.map((column) => window.desktop.readMission(column.missionIds.at(-1))))
    return turns.every((turn) => turn.ok && turn.data.mission.events.some((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type)))
  })()`, { timeoutMs: 300_000, what: 'both free columns end' })
  const compare = (await comparisons()).data?.compares.find((item) => item.compareId === compareId)
  const columns = await Promise.all(compare.slots.map(async (column) => (await mission(column.missionIds.at(-1))).data?.mission))
  check('both free columns complete', columns.every((column) => column?.phase === 'completed'), columns.map((column) => ({ model: column?.model, phase: column?.phase })))
  if (!columns.every((column) => column?.phase === 'completed')) throw new Error('A column did not complete; no judge was asked.')

  // Ask three judges to vote: Haiku, Sonnet and Opus on the API, on the person's points.
  await drive.waitForSelector('.lc-compare__judgeask')
  await drive.evaluate(`[...document.querySelectorAll('.lc-compare__judgeask button, .lc-compare__judgenote button')].find((b) => b.textContent.trim() === 'Ask three judges to vote')?.click(); 1`)
  await drive.waitFor(`document.querySelectorAll('.lc-compare__judgepick').length === 3`, { what: 'three judge pickers' })
  // By each picker's own name, never its place on the page: picked by place, the first run's panel came out Opus,
  // Haiku and a free model (2026-10-21).
  const PICKERS = ['The first judge', 'The second judge', 'The third judge']
  for (const [index, judge] of JUDGES.entries()) {
    await drive.evaluate(`document.querySelector('.lc-compare__judgepick[aria-label=${JSON.stringify(PICKERS[index])}]').click(); 1`)
    await drive.waitForSelector('.lc-select__menu [role="option"]')
    const picked = await drive.evaluate(`(() => {
      const option = [...document.querySelectorAll('.lc-select__menu [role="option"]')].find((item) => item.innerText.includes(${JSON.stringify(judge.name)}))
      if (!option || option.disabled) return false
      option.click(); return true
    })()`)
    if (!picked) throw new Error(`${judge.name} is not offered as a judge`)
    await sleep(300)
  }
  const shown = JSON.parse(String(await drive.evaluate(`JSON.stringify(${JSON.stringify(PICKERS)}.map((label) => document.querySelector('.lc-compare__judgepick[aria-label="' + label + '"]')?.innerText.trim() ?? ''))`)))
  check('the three pickers show Haiku, Sonnet and Opus on the API before Judge is pressed', JUDGES.every((judge, index) => shown[index]?.includes(judge.name)), shown)
  if (!JUDGES.every((judge, index) => shown[index]?.includes(judge.name))) throw new Error('The panel is not the three Claude judges; nothing was asked.')
  await drive.evaluate(`(() => {
    const box = document.querySelector('.lc-compare__judgecriteria')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(box, ${JSON.stringify(criteria)})
    box.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await sleep(200)
  await drive.capture('Ask three judges: Haiku, Sonnet and Opus on the API', () => drive.evaluate(`(() => {
    const button = [...document.querySelector('.lc-compare__judgeask').querySelectorAll('button')].find((item) => item.textContent.trim() === 'Judge')
    if (!button || button.disabled) throw new Error('Judge is not ready')
    button.click(); return 'Three judges asked; nothing is kept.'
  })()`))
  await drive.waitFor(`(async () => {
    const listed = await window.desktop.listCompares()
    const judge = listed.ok ? listed.data.compares.find((item) => item.compareId === ${JSON.stringify(compareId)})?.judge : undefined
    const ids = judge === undefined ? [] : [judge.missionIds.at(-1), ...(judge.panel ?? []).map((one) => one.missionId)].filter(Boolean)
    if (ids.length < 3) return false
    const reads = await Promise.all(ids.map((id) => window.desktop.readMission(id)))
    return reads.every((read) => read.ok && read.data.mission.events.some((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type)))
  })()`, { timeoutMs: 600_000, what: 'all three API judges end' })
  const judged = (await comparisons()).data?.compares.find((item) => item.compareId === compareId)
  const judgeIds = [judged.judge.missionIds.at(-1), ...(judged.judge.panel ?? []).map((one) => one.missionId)]
  const records = await Promise.all(judgeIds.map(async (id) => (await mission(id)).data?.mission))
  // Each model is one of the three added on the API -- not merely three different models (the first run's check
  // passed a free model in Sonnet's place).
  const claudeModels = records.map((record) => /^own-[a-z0-9]+\/(claude-[a-z0-9-]+)$/.exec(record?.model ?? '')?.[1])
  check('all three judges complete, read-only, on Haiku, Sonnet and Opus via the API', records.every((record) => record?.phase === 'completed' && record.sandbox === 'read-only') && JUDGES.every((judge) => claudeModels.includes(judge.model)), records.map((record) => ({ model: record?.model, phase: record?.phase, sandbox: record?.sandbox })))
  let material = records[0]?.prompt ?? ''
  for (const match of material.matchAll(/is in the file (\.locust\/attachments\/judge-[^\s]+\.md)/g)) {
    const path = resolve(workspace, match[1])
    if (!path.startsWith(resolve(workspace) + sep)) throw new Error('A judge file escaped the scratch project.')
    material += '\n' + await readFile(path, 'utf8')
  }
  await writeFile(join(drive.out, 'judge-material.md'), material)
  check('the judges read both blind diffs, not only the final messages', ['A', 'B'].every((letter) => material.includes(`Changes made by Answer ${letter}:`)) && /@@/.test(material))
  // What each judge cost, from its own run's usage, for the credits log.
  const usage = records.map((record) => {
    const done = [...(record?.events ?? [])].reverse().find((event) => event.type === 'run.completed')
    return { model: record?.model, usage: done?.payload?.usage ?? done?.payload?.tokens ?? null }
  })
  say(`judge usage: ${JSON.stringify(usage)}`)
  await writeFile(join(drive.out, 'judge-usage.json'), JSON.stringify(usage, null, 2))
  await drive.waitFor(`(document.querySelector('.lc-compare__judge')?.innerText.length ?? 0) > 40`, { what: 'the panel view rendered' })
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    await drive.evaluate(`document.querySelector('.lc-compare__judge').scrollIntoView({ block: 'center' })`)
    await sleep(250)
    await drive.capture(`${theme}: the three judges' votes`, () => drive.evaluate(view))
  }
  check('the judges keep nothing, and the original project is unchanged', judged.kept === undefined && (await readFile(join(workspace, 'sum.mjs'), 'utf8')).includes('a - b'))
  check('the frames have no renderer errors', drive.record.every((frame) => frame.errors.length === 0))
} catch (error) {
  check('the drive completes', false, error instanceof Error ? error.message : String(error))
  await drive.capture('drive failure', () => drive.evaluate('document.body.innerText'))
} finally {
  say(`JUDGES ON API CREDITS: ${checks - failures}/${checks} passed`)
  await drive.finish({ intro: 'Two free file-changing columns, then three Claude judges on the API credits (Haiku, Sonnet, Opus 5.5) voting on the person\'s points. The key came from a masked pop-up and was never printed.', extra: `Checks: ${checks - failures}/${checks}.` })
  if (failures > 0) process.exitCode = 1
}
