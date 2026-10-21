// JUDGES SEE THE CHECK (0.811): a real file-changing comparison of two free OpenCode models in a folder whose check
// command is set (Settings > Project folder), then Ask a judge on a free model. Each column's copy must run the check
// after its turn, the column and the judges' table must say what it said, and the judge's saved brief must carry each
// copy's result under its letter. A build without the change fails on the brief even when both copies pass.
// node _tools/drive-judges-see-the-check.mjs [--packaged <exe>] [--tag <name>]
import { readFile, writeFile } from 'node:fs/promises'
import { join, resolve, sep } from 'node:path'
import { FREE_ROUTE, git, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined
if (!FREE_ROUTE.model.endsWith('-free')) throw new Error('This drive uses free models only.')
const workspace = await scratchRepository('locust-judges-check-ws-')
await writeFile(join(workspace, 'sum.mjs'), 'export const sum = (a, b) => a - b\n')
await writeFile(join(workspace, 'sum.test.mjs'), "import { test } from 'node:test'\nimport assert from 'node:assert/strict'\nimport { sum } from './sum.mjs'\ntest('adds two numbers', () => assert.equal(sum(2, 3), 5))\n")
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'A small addition bug'], workspace)
const COMMAND = 'node --test sum.test.mjs'
const ask = 'Fix the integer addition bug in sum.mjs. Do not change other files or commit. Final message: one short sentence.'
const criteria = 'Fixes sum.mjs addition\nChanges nothing else'
const drive = await startDrive({
  name: 'judges-see-the-check', port: 9806, workspace, holdCursor: false,
  outPath: join(recordRoot('judges-see-the-check-2026-10-21'), arg('--tag') ?? 'live'),
  ...(arg('--packaged') === undefined ? {} : { packaged: arg('--packaged') }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', route: FREE_ROUTE, createdAt: '2026-10-21T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: false, relayHopCap: 2 } }
})
let checks = 0, failures = 0, compareId
const check = (what, ok, detail) => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ' -- ' + JSON.stringify(detail).slice(0, 1200)}`)
}
const call = async (expression) => JSON.parse(String(await drive.evaluate(`(async () => JSON.stringify(await ${expression}))()`)))
const comparison = async () => (await call('window.desktop.listCompares()')).data?.compares.find((item) => item.compareId === compareId)
const mission = async (id) => (await call(`window.desktop.readMission(${JSON.stringify(id)})`)).data?.mission
const view = `JSON.stringify({ text: document.querySelector('.lc-compare')?.innerText ?? '', said: document.querySelector('.lc-compare__judgesaid')?.innerText ?? '', sideways: document.documentElement.scrollWidth > innerWidth })`
try {
  await drive.ready()
  await drive.resize(1440, 900)
  // The person's own way in: Settings > Project folder writes the folder's check command.
  const settings = await call('window.desktop.readWorkspaceSettings()')
  const written = await call(`window.desktop.writeWorkspaceSettings(${JSON.stringify({ ...settings, checkCommand: COMMAND })})`)
  check('the folder\'s check command is set as Settings sets it', written.checkCommand === COMMAND, written.checkCommand)
  const catalog = await call('window.desktop.listModels()')
  const free = (catalog.ok ? catalog.data.models : []).filter((model) => model.runtime === 'opencode' && model.id.endsWith('-free'))
  const second = free.find((model) => model.id !== FREE_ROUTE.model)
  if (!free.some((model) => model.id === FREE_ROUTE.model) || !second) throw new Error('Two advertised free OpenCode models are required.')
  const routes = [FREE_ROUTE, { runtime: 'opencode', model: second.id }].map(({ runtime, model }) => ({ runtime, model }))
  check('the two columns and the judge use free OpenCode routes only', routes.every((route) => route.runtime === 'opencode' && route.model.endsWith('-free')), routes)
  const started = await call(`window.desktop.startCompare(${JSON.stringify({ teammateId: 'tm_wren', prompt: ask, changes: true, blind: true, routes })})`)
  if (!started.ok || started.data.refused.length > 0) throw new Error(JSON.stringify(started))
  compareId = started.data.compare.compareId
  check('the comparison changes copies from a recorded base', started.data.compare.changes === true && /^[a-f0-9]{40}$/i.test(started.data.compare.base ?? '') && started.data.compare.changesIn !== 'folder')
  // A bridge start bypasses App's local state; reload its saved comparisons (as drive-judge-reads-the-work does).
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
  const compare = await comparison()
  const columns = await Promise.all(compare.slots.map((column) => mission(column.missionIds.at(-1))))
  check('both free columns complete', columns.every((column) => column?.phase === 'completed'), columns.map((column) => ({ model: column?.model, phase: column?.phase })))
  if (!columns.every((column) => column?.phase === 'completed')) throw new Error('A column did not complete; no judge was asked.')
  // The check runs after each turn that changed files, in that column's copy, and lands on its mission.
  await drive.waitFor(`(async () => {
    const listed = await window.desktop.listCompares()
    const compare = listed.ok ? listed.data.compares.find((item) => item.compareId === ${JSON.stringify(compareId)}) : undefined
    const turns = await Promise.all((compare?.slots ?? []).map((column) => window.desktop.readMission(column.missionIds.at(-1))))
    return turns.length === 2 && turns.every((turn) => turn.ok && turn.data.mission.editCheck !== undefined)
  })()`, { timeoutMs: 240_000, what: 'each copy\'s check' })
  const checked = (await Promise.all(compare.slots.map((column) => mission(column.missionIds.at(-1))))).map((turn) => turn?.editCheck)
  check('each copy ran the folder\'s command after its turn', checked.every((one) => one?.command === COMMAND), checked)
  const sum = await Promise.all(compare.slots.map((column) => readFile(join(column.folder ?? '', 'sum.mjs'), 'utf8').catch(() => '')))
  // Its outcome is the copy's own: a column that fixed sum.mjs passes, one that did not fails.
  check('each outcome matches what its copy holds', checked.every((one, index) => (one?.outcome === 'passed') === /a\s*\+\s*b/.test(sum[index] ?? '')), { outcomes: checked.map((one) => one?.outcome), sum })
  await drive.waitFor(`(document.querySelector('.lc-compare')?.innerText ?? '').includes('after this turn')`, { timeoutMs: 30_000, what: 'the check shown in the columns' })
  const before = JSON.parse(String(await drive.evaluate(view)))
  check('each column shows what the check said after its turn', (before.text.match(/after this turn/g) ?? []).length >= 2, before.text.slice(0, 600))
  check('Ask a judge says its judges read the check', before.text.includes(`Each judge also reads what your check, ${COMMAND}, said in each copy.`))
  await drive.capture('both columns checked, Ask a judge says the judges read it', () => drive.evaluate(view))
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
  const judged = await comparison()
  const record = await mission(judged.judge.missionIds.at(-1))
  check('the judge completes read-only on the requested free model', record?.phase === 'completed' && record.model === FREE_ROUTE.model && record.sandbox === 'read-only', { phase: record?.phase, model: record?.model, sandbox: record?.sandbox })
  let material = record?.prompt ?? ''
  for (const match of material.matchAll(/is in the file (\.locust\/attachments\/judge-[^\s]+\.md)/g)) {
    const path = resolve(workspace, match[1])
    if (!path.startsWith(resolve(workspace) + sep)) throw new Error('A judge file escaped the scratch project.')
    material += '\n' + await readFile(path, 'utf8')
  }
  await writeFile(join(drive.out, 'judge-material.md'), material)
  const words = { passed: 'passed.', failed: 'failed', 'timed-out': 'ran past its time limit', 'could-not-run': 'could not run' }
  check('the judge read the folder\'s command and how to weigh it', material.includes(`The person's own check, \`${COMMAND}\`, ran in each answer's copy after its turn`) && material.includes('weigh it above anything the answer claims'))
  check('each blind answer carries its own copy\'s result', judged.slots.every((column, index) => material.includes(`The person's check of Answer ${column.slot.toUpperCase()}'s copy, after its turn: ${words[checked[index]?.outcome] ?? '?'}`)), checked.map((one) => one?.outcome))
  check('the work evidence does not identify the copies or comparison', !material.includes(compareId) && !material.includes('locust-judges-check-ws-'))
  await drive.waitFor(`(document.querySelector('.lc-compare__judgesaid')?.innerText.length ?? 0) > 0`, { what: 'the judge view rendered' })
  const after = JSON.parse(String(await drive.evaluate(view)))
  check('the judges\' table puts your check beside the score', /Your check/.test(after.text))
  check('the judge keeps nothing itself and the original project is unchanged', judged.kept === undefined && (await readFile(join(workspace, 'sum.mjs'), 'utf8')).includes('a - b'))
  for (const theme of ['dark', 'light']) {
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    for (const [width, height] of [[1440, 900], [1205, 803]]) {
      await drive.resize(width, height)
      await drive.evaluate(`document.querySelector('.lc-compare__judge')?.scrollIntoView({ block: 'center' })`)
      await sleep(200)
      const state = JSON.parse(String(await drive.evaluate(view)))
      check(`${theme} ${width}: the compare view has no page-wide overflow`, !state.sideways)
      await drive.capture(`${theme} judges see the check ${width} by ${height}`, () => JSON.stringify(state))
    }
  }
  check('the frames have no renderer errors', drive.record.every((frame) => frame.errors.length === 0))
} catch (error) {
  check('the drive completes', false, error instanceof Error ? error.message : String(error))
  await drive.capture('drive failure', () => drive.evaluate('document.body.innerText'))
} finally {
  say(`JUDGES SEE THE CHECK: ${checks - failures}/${checks} passed`)
  await drive.finish({ intro: 'Two real free file-changing columns in a folder with a check command, then the free Ask a judge button. Each copy must run the check, and the saved brief must carry each result under its letter.', extra: `Checks: ${checks - failures}/${checks}. Free routes only.` })
  if (failures > 0) process.exitCode = 1
}
