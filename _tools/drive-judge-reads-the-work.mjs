// A real file-changing comparison, two free OpenCode models, then Ask a judge on a free model (0.810).
// The saved judge brief must carry actual hunks: an answers-only build fails even if a model mentions a filename.
// node _tools/drive-judge-reads-the-work.mjs [--packaged <exe>] [--tag <name>]
import { readFile, writeFile } from 'node:fs/promises'
import { join, resolve, sep } from 'node:path'
import { FREE_ROUTE, git, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined
if (!FREE_ROUTE.model.endsWith('-free')) throw new Error('This drive uses free models only.')
const workspace = await scratchRepository('locust-judge-work-ws-')
await writeFile(join(workspace, 'sum.mjs'), 'export const sum = (a, b) => a - b\n')
await writeFile(join(workspace, 'sum.test.mjs'), "import { test } from 'node:test'\nimport assert from 'node:assert/strict'\nimport { sum } from './sum.mjs'\ntest('adds two numbers', () => assert.equal(sum(2, 3), 5))\n")
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'A small addition bug'], workspace)
const ask = 'Fix the integer addition bug in sum.mjs. Add a negative-number regression case in sum.test.mjs. Run node --test sum.test.mjs. Do not change other files or commit. Final message: one short sentence.'
const criteria = 'Fixes sum.mjs addition\nAdds the negative-number test\nClaims agree with the actual diff'
const drive = await startDrive({
  name: 'judge-reads-the-work', port: 9828, workspace, holdCursor: false,
  outPath: join(recordRoot('judge-reads-the-work-2026-10-21'), arg('--tag') ?? 'live'),
  ...(arg('--packaged') === undefined ? {} : { packaged: arg('--packaged') }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', route: FREE_ROUTE, createdAt: '2026-10-21T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: false, relayHopCap: 2 } }
})
let checks = 0, failures = 0, compareId
const check = (what, ok, detail) => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ' -- ' + JSON.stringify(detail).slice(0, 1200)}`)
}
const comparisons = async () => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listCompares()))()`)))
const mission = async (id) => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.readMission(${JSON.stringify(id)})))()`)))
const view = `JSON.stringify({ text: document.querySelector('.lc-compare')?.innerText ?? '', said: document.querySelector('.lc-compare__judgesaid')?.innerText ?? '', sideways: document.documentElement.scrollWidth > innerWidth })`
try {
  await drive.ready()
  await drive.resize(1440, 900)
  const catalog = JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.listModels()))()`)))
  const free = (catalog.ok ? catalog.data.models : []).filter((model) => model.runtime === 'opencode' && model.id.endsWith('-free'))
  const second = free.find((model) => model.id !== FREE_ROUTE.model)
  if (!free.some((model) => model.id === FREE_ROUTE.model) || !second) throw new Error('Two advertised free OpenCode models are required.')
  const routes = [FREE_ROUTE, { runtime: 'opencode', model: second.id }].map(({ runtime, model }) => ({ runtime, model }))
  check('the two columns and judge use free OpenCode routes only', routes.every((route) => route.runtime === 'opencode' && route.model.endsWith('-free')), routes)
  const started = JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await window.desktop.startCompare(${JSON.stringify({ teammateId: 'tm_wren', prompt: ask, changes: true, blind: true, routes })})))()`)))
  if (!started.ok || started.data.refused.length > 0) throw new Error(JSON.stringify(started))
  compareId = started.data.compare.compareId
  check('the comparison changes copies from a recorded base', started.data.compare.changes === true && /^[a-f0-9]{40}$/i.test(started.data.compare.base ?? '') && started.data.compare.changesIn !== 'folder')
  // As in drive-compare-in-any-folder: a bridge start bypasses App's local state; reload its saved comparisons.
  await drive.evaluate('location.reload(); 1')
  await drive.waitFor(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].some((row) => /^vs/.test(row.innerText.trim()))`, { timeoutMs: 30_000, what: 'the comparison in history' })
  await drive.evaluate(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((row) => /^vs/.test(row.innerText.trim())).click()`)
  await drive.waitForSelector('.lc-compare')
  await drive.capture('two free models start changing separate copies', () => drive.evaluate(view))
  await drive.waitFor(`(async () => {
    const listed = await window.desktop.listCompares()
    const compare = listed.ok ? listed.data.compares.find((item) => item.compareId === ${JSON.stringify(compareId)}) : undefined
    if (!compare || compare.slots.some((column) => column.missionIds.length === 0)) return false
    const turns = await Promise.all(compare.slots.map((column) => window.desktop.readMission(column.missionIds.at(-1))))
    return turns.every((turn) => turn.ok && turn.data.mission.events.some((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type)))
  })()`, { timeoutMs: 300_000, what: 'both real comparison turns end' })
  const compare = (await comparisons()).data?.compares.find((item) => item.compareId === compareId)
  const columns = await Promise.all(compare.slots.map(async (column) => (await mission(column.missionIds.at(-1))).data?.mission))
  check('both free columns complete', columns.every((column) => column?.phase === 'completed'), columns.map((column) => ({ model: column?.model, phase: column?.phase })))
  if (!columns.every((column) => column?.phase === 'completed')) throw new Error('A column did not complete; no judge was asked.')
  await drive.capture('both columns finished, Ask a judge available', () => drive.evaluate(view))
  await drive.waitForSelector('.lc-compare__judgeask .lc-compare__judgepick')
  await drive.evaluate(`document.querySelector('.lc-compare__judgeask .lc-compare__judgepick').click()`)
  await drive.waitForSelector('.lc-select__menu [role="option"]')
  const picked = await drive.evaluate(`(() => {
    const option = [...document.querySelectorAll('.lc-select__menu [role="option"]')].find((item) => item.dataset.value === ${JSON.stringify(`opencode:${FREE_ROUTE.model}`)})
    if (!option || option.disabled) return false
    option.click(); return true
  })()`)
  if (!picked) throw new Error('The free judge is not offered; no other route will be used.')
  await drive.evaluate(`(() => {
    const box = document.querySelector('.lc-compare__judgecriteria')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(box, ${JSON.stringify(criteria)})
    box.dispatchEvent(new Event('input', { bubbles: true }))
  })()`)
  await sleep(150)
  await drive.capture('Ask a judge on the free model', () => drive.evaluate(`(() => {
    const button = [...document.querySelector('.lc-compare__judgeask').querySelectorAll('button')].find((item) => item.textContent.trim() === 'Judge')
    if (!button || button.disabled) throw new Error('Judge is not ready')
    button.click(); return 'The person asks once; no answer is kept.'
  })()`))
  await drive.waitFor(`(async () => {
    const listed = await window.desktop.listCompares()
    const id = listed.ok ? listed.data.compares.find((item) => item.compareId === ${JSON.stringify(compareId)})?.judge?.missionIds.at(-1) : undefined
    if (!id) return false
    const read = await window.desktop.readMission(id)
    return read.ok && read.data.mission.events.some((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type))
  })()`, { timeoutMs: 240_000, what: 'the free judge ends' })
  const judged = (await comparisons()).data?.compares.find((item) => item.compareId === compareId)
  const record = (await mission(judged.judge.missionIds.at(-1))).data?.mission
  check('the judge completes read-only on the requested free model', record?.phase === 'completed' && record.model === FREE_ROUTE.model && record.sandbox === 'read-only', { phase: record?.phase, model: record?.model, sandbox: record?.sandbox })
  let material = record?.prompt ?? ''
  // The long-brief path is part of the same assertion, not an excuse to skip checking the diff.
  for (const match of material.matchAll(/is in the file (\.locust\/attachments\/judge-[^\s]+\.md)/g)) {
    const path = resolve(workspace, match[1])
    if (!path.startsWith(resolve(workspace) + sep)) throw new Error('A judge file escaped the scratch project.')
    material += '\n' + await readFile(path, 'utf8')
  }
  await writeFile(join(drive.out, 'judge-material.md'), material)
  check('each blind answer carries actual sum.mjs diff hunks, not only a final message', ['A', 'B'].every((letter) => material.includes(`Changes made by Answer ${letter}:`)) && (material.match(/File "sum\.mjs"/g) ?? []).length === 2 && /@@/.test(material), material.slice(0, 300))
  check('the work evidence does not identify the copies or comparison', !material.includes(compareId) && !material.includes('locust-judge-work-ws-'))
  await drive.waitFor(`(document.querySelector('.lc-compare__judgesaid')?.innerText.length ?? 0) > 0`, { what: 'the judge view rendered' })
  const said = JSON.parse(String(await drive.evaluate(view)))
  check('the judge names an actual changed file or the addition fix', /sum\.mjs|sum\.test\.mjs|addition|negative.number/i.test(said.said), said.said)
  check('the judge keeps nothing itself and the original project is unchanged', judged.kept === undefined && (await readFile(join(workspace, 'sum.mjs'), 'utf8')).includes('a - b'))
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height)
      await drive.evaluate(`document.querySelector('.lc-compare__judge').scrollIntoView({ block: 'center' })`)
      await sleep(200)
      const state = JSON.parse(String(await drive.evaluate(view)))
      check(`${theme} ${width}: the judge view has no page-wide overflow`, !state.sideways)
      await drive.capture(`${theme} judge reads the work ${width} by ${height}`, () => JSON.stringify(state))
    }
  }
  check('the frames have no renderer errors', drive.record.every((frame) => frame.errors.length === 0))
} catch (error) {
  check('the drive completes', false, error instanceof Error ? error.message : String(error))
  await drive.capture('drive failure', () => drive.evaluate('document.body.innerText'))
} finally {
  say(`JUDGE READS THE WORK: ${checks - failures}/${checks} passed`)
  await drive.finish({ intro: 'Two real free file-changing columns, then the free Ask a judge button. The saved brief must include both blind diffs; an answers-only build cannot pass.', extra: `Checks: ${checks - failures}/${checks}. No paid routes or global configuration changes.` })
  if (failures > 0) process.exitCode = 1
}
