// W7: make a routine with two inputs, run on FREE_ROUTE, read its ledger prompt,
// export the exact allowed keys, and import into a fresh profile through the preview.
// Native file dialogs use dev-only LOCUST_ROUTINE_FILE_PATH, so `--packaged <exe>`
// (0.567) drives everything up to the finished run and stops before Export.
import { mkdtemp, readFile, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { FREE_ROUTE, startDrive, scratchRepository, say, sleep } from './drive-lib.mjs'

if (!FREE_ROUTE.model.startsWith('opencode/') || !FREE_ROUTE.model.endsWith('-free')) throw new Error('This drive requires a free OpenCode route.')
const route = { ...FREE_ROUTE, mode: 'ask' }
const packaged = process.argv.includes('--packaged') ? process.argv[process.argv.indexOf('--packaged') + 1] : undefined
const workspace = await scratchRepository('locust-routine-inputs-ws-')
const transfer = await mkdtemp(join(tmpdir(), 'locust-routine-inputs-file-'))
const filePath = join(transfer, 'Portable notes.routine.md')
const seed = (id, name) => ({ schemaVersion: 1,
  teammates: [{ teammateId: id, name, hue: 'lime', role: 'Custom', roleTitle: 'Office', createdAt: '2026-10-03T00:00:00Z', route }],
  missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } })
let failures = 0
let checks = 0
const check = (what, ok, detail = '') => {
  checks += 1
  if (!ok) failures += 1
  say(`[${ok ? 'PASS' : 'FAIL'}] ${what}${detail ? ` -- ${String(detail).slice(0, 400)}` : ''}`)
}
const SET = `(selector, value) => {
  const node = document.querySelector(selector)
  if (!node) throw new Error('No input: ' + selector)
  const proto = node.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : node.tagName === 'SELECT' ? HTMLSelectElement.prototype : HTMLInputElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value').set.call(node, value)
  node.dispatchEvent(new Event(node.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }))
}`
const click = `(label) => { const node = [...document.querySelectorAll('button')].find((b) => b.innerText.trim() === label); if (!node) throw new Error('No button: ' + label); node.click() }`
/*
 * Locust's own dropdown (components/Select.tsx): a combobox button and its options, not a native select, so SET's
 * setter threw "Illegal invocation" on it, the kind stayed Text, and the choice input went untried while every check
 * still passed (the 0.817 sweep).
 */
const CHOOSE = `async (label, option) => {
  const box = [...document.querySelectorAll('[role=combobox]')].find((one) => one.getAttribute('aria-label') === label)
  if (!box) throw new Error('No dropdown: ' + label)
  box.click()
  await new Promise((r) => setTimeout(r, 250))
  // Its own list: every dropdown keeps its closed list in the page, hidden, so the first "Choice" anywhere was the
  // first input's, and pressing it changed that one (the 0.818 drive).
  const list = document.getElementById(box.getAttribute('aria-controls') ?? '')
  const item = [...(list?.querySelectorAll('[role=option]') ?? [])].find((one) => one.textContent.trim() === option)
  if (!item) throw new Error('No option ' + option + ' in ' + label)
  item.click()
  await new Promise((r) => setTimeout(r, 250))
}`
/** What a field shows: a text box's value, or the option a dropdown shows. */
const SHOWN = `(selector) => { const node = document.querySelector(selector); return node === null ? undefined : node.getAttribute('role') === 'combobox' ? node.innerText.trim() : node.value }`
const navigation = async (drive) => {
  await drive.evaluate(`[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()`)
  await sleep(600)
}
let drive
let sourceProfile
let exported
try {
  drive = await startDrive({ ...(packaged === undefined ? {} : { packaged }), name: 'routine-inputs-source', port: 9897, workspace, seed: seed('tm_source', 'Cedar'), keep: true, env: { LOCUST_ROUTINE_FILE_PATH: filePath } })
  sourceProfile = drive.profile
  await drive.ready()
  await drive.resize(1200, 720)
  await drive.send('Page.bringToFront')
  await navigation(drive)
  await drive.evaluate(`(${click})('New routine')`)
  await sleep(400)
  const step = 'Do not use tools. Reply with the topic and style only. Topic: {{topic}}. Style: {{style}}.'
  await drive.evaluate(`(${SET})('#routine-name', 'Portable notes'); (${SET})('textarea[aria-label="Step 1"]', ${JSON.stringify(step)})`)
  await drive.evaluate(`(${click})('Add input')`)
  await sleep(150)
  await drive.evaluate(`(${SET})('[aria-label="Input 1 key"]', 'topic'); (${SET})('[aria-label="Input 1 label"]', 'Topic'); (${SET})('[aria-label="Input 1 default"]', 'default topic')`)
  await drive.evaluate(`(${click})('Add input')`)
  await sleep(150)
  await drive.evaluate(`(${SET})('[aria-label="Input 2 key"]', 'style'); (${SET})('[aria-label="Input 2 label"]', 'Style')`)
  await drive.evaluate(`(${CHOOSE})('Input 2 kind', 'Choice')`)
  await sleep(200)
  await drive.evaluate(`(${SET})('[aria-label="Input 2 choices"]', ${JSON.stringify('Brief\nDetailed')})`)
  await sleep(200)
  await drive.evaluate(`(${CHOOSE})('Input 2 default', 'Brief')`)
  const editor = await drive.capture('two declared inputs in the routine editor', () => drive.evaluate(`({ count: document.querySelectorAll('.lc-routineinputs__entry').length, kind: (${SHOWN})('[aria-label="Input 2 kind"]'), saveDisabled: [...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Save routine')?.disabled })`))
  check('editor has two valid declarations, the second a choice, and allows saving', editor.count === 2 && editor.kind === 'Choice' && editor.saveDisabled === false, JSON.stringify(editor))
  await drive.evaluate(`(${click})('Save routine')`)
  await sleep(700)
  const saved = await drive.evaluate(`(async () => { const answer = await window.desktop.listRoutines(); return answer.ok ? answer.data.routines[0] : undefined })()`)
  check('routine is saved with two input declarations, the second a choice of two', saved?.inputs?.length === 2 && saved.inputs[1]?.kind === 'choice' && JSON.stringify(saved.inputs[1]?.choices) === '["Brief","Detailed"]' && saved.steps[0] === step, JSON.stringify(saved?.inputs))
  if (!saved) throw new Error('No saved routine')
  const automatic = await drive.evaluate(`window.desktop.runRoutine(${JSON.stringify(saved.routineId)}, { topic: 'a'.repeat(4001), style: 'Brief' })`)
  check('an overlong value is refused before any run', !automatic.ok && /4,000/.test(automatic.error.message), JSON.stringify(automatic))
  await drive.evaluate(`(${click})('Run')`)
  await sleep(400)
  const defaults = await drive.capture('Run asks for inputs with defaults filled', () => drive.evaluate(`({ open: !!document.querySelector('[role=dialog][aria-label="Run routine"]'), topic: (${SHOWN})('[aria-label="Value for Topic"]'), style: (${SHOWN})('[aria-label="Value for Style"]') })`))
  check('manual Run opens the dialog and fills both defaults', defaults.open && defaults.topic === 'default topic' && defaults.style === 'Brief', JSON.stringify(defaults))
  await drive.evaluate(`(${SET})('[aria-label="Value for Topic"]', 'portable-alpha')`)
  await drive.evaluate(`(${CHOOSE})('Value for Style', 'Detailed')`)
  await drive.capture('values entered before starting', () => drive.evaluate(`document.querySelector('[role=dialog]')?.innerText`))
  await drive.evaluate(`(${click})('Run routine')`)
  let mission
  for (let waited = 0; waited < 120_000; waited += 1000) {
    await sleep(1000)
    const history = await drive.evaluate('window.desktop.getMissionHistory()')
    mission = history.ok ? history.data.missions.find((entry) => entry.startedBy?.routineId === saved.routineId) : undefined
    if (mission) break
  }
  check('run creates a durable mission on the free OpenCode route', mission?.runtime === 'opencode' && mission.model === route.model, JSON.stringify({ runtime: mission?.runtime, model: mission?.model }))
  if (!mission) throw new Error('No routine mission arrived')
  const ledgerFiles = await readdir(join(sourceProfile, 'mission-ledger'))
  let ledger = ''
  for (const name of ledgerFiles.filter((name) => name.endsWith('.jsonl'))) {
    const text = await readFile(join(sourceProfile, 'mission-ledger', name), 'utf8')
    if (text.includes(mission.missionId)) ledger += text
  }
  const expected = 'Topic: portable-alpha. Style: Detailed.'
  check('the ledger prompt holds the substituted text and no unresolved markers', ledger.includes(expected) && !ledger.includes('{{topic}}') && !ledger.includes('{{style}}'), mission.prompt?.slice(-300))
  for (let waited = 0; waited < 240_000; waited += 2000) {
    const answer = await drive.evaluate(`window.desktop.readMission(${JSON.stringify(mission.missionId)})`)
    if (answer.ok) mission = answer.data.mission
    // A new ledger without a terminal event is projected as interrupted by a
    // standalone read. It is still starting; only a terminal receipt settles it.
    if (['completed', 'failed', 'cancelled'].includes(mission.phase)) break
    await sleep(2000)
  }
  check('the free run completes', mission.phase === 'completed', mission.phase)
  await drive.capture('the routine run finished', () => drive.evaluate(`document.querySelector('.lc-thread')?.innerText.slice(-1200)`))
  if (packaged !== undefined) {
    check('source captures have no renderer errors', drive.record.every((entry) => entry.errors.length === 0))
    throw new Error('PACKAGED-STOP')
  }
  await navigation(drive)
  await drive.evaluate(`document.querySelector('button[aria-label="Export Portable notes"]')?.click()`)
  await sleep(700)
  // A routine is exported as Markdown (0.834): the settings between --- lines, then the title and each step.
  const written = await readFile(filePath, 'utf8')
  const top = written.split('\n---\n')[0]
  check('export is Markdown a person reads: settings, then the title and each step', /^---\nlocust-routine: 1\n/.test(written) && written.includes(`\n# Portable notes\n\n## Step 1\n\n${step}\n`), written.slice(0, 200))
  check('export settings name only what a routine is', [...top.matchAll(/^([a-z-]+):/gm)].every(([, key]) => ['locust-routine', 'runs-on', 'connectors', 'inputs'].includes(key)), top)
  check('export contains no machine or execution data', !written.includes(saved.routineId) && !written.includes('tm_source') && !written.includes(sourceProfile), written.slice(0, 200))
  check('export preserves templates and declarations without entered values', (written.match(/^ {2}- key: /gm) ?? []).length === 2 && !written.includes('portable-alpha'))
  // What the import that follows must carry over: the source routine's own steps and inputs.
  exported = { steps: saved.steps, inputs: saved.inputs }
  await drive.capture('routine exported as a file', () => drive.evaluate(`document.querySelector('.lc-automations')?.innerText`))
  check('source captures have no renderer errors', drive.record.every((entry) => entry.errors.length === 0))
  await drive.finish({ intro: `W7 dev build; ${route.model}; two inputs, one manual free run and file export.`, extra: `${checks - failures}/${checks} checks passed. Transfer file: ${filePath}` })
  drive = undefined

  drive = await startDrive({ name: 'routine-inputs-import', port: 9897, workspace, seed: seed('tm_import', 'Maple'), keep: true, sendsNothing: true, env: { LOCUST_ROUTINE_FILE_PATH: filePath, LOCUST_ROUTINE_FOLDER_PATH: workspace } })
  await drive.ready()
  await drive.resize(1200, 720)
  await drive.send('Page.bringToFront')
  await navigation(drive)
  const before = await drive.evaluate(`(async () => ({ routines: await window.desktop.listRoutines(), history: await window.desktop.getMissionHistory() }))()`)
  check('fresh import profile has no routines or missions', before.routines.ok && before.routines.data.routines.length === 0 && before.history.ok && before.history.data.missions.length === 0)
  await drive.evaluate(`(${click})('Import routine')`)
  await sleep(600)
  const preview = await drive.capture('import preview in a fresh profile', () => drive.evaluate(`document.querySelector('[role=dialog][aria-label="Import routine"]')?.innerText`))
  check('preview shows steps, inputs, connectors and a teammate choice', ['{{topic}}', '{{style}}', 'Topic', 'Style', 'Connectors it needs', 'Give it to', 'with no schedule, and runs nothing'].every((text) => preview?.toLowerCase().includes(text.toLowerCase())), preview)
  const pending = await drive.evaluate(`(async () => ({ routines: await window.desktop.listRoutines(), history: await window.desktop.getMissionHistory() }))()`)
  check('preview creates and runs nothing', pending.routines.data.routines.length === 0 && pending.history.data.missions.length === 0)
  // A dropdown drawn by Locust now, not a select, so it is chosen as one (0.834).
  await drive.evaluate(`(${CHOOSE})('Give routine to', 'Maple')`)
  await sleep(150)
  await drive.evaluate(`document.querySelector('[role=dialog] button.lc-primarybutton')?.click()`)
  await sleep(700)
  const after = await drive.capture('import saved without schedule or execution', () => drive.evaluate(`(async () => ({ routines: await window.desktop.listRoutines(), history: await window.desktop.getMissionHistory(), dialog: !!document.querySelector('[role=dialog]') }))()`))
  const imported = after.routines.ok ? after.routines.data.routines[0] : undefined
  check('import assigns the selected teammate and preserves steps and inputs', !after.dialog && imported?.teammateId === 'tm_import' && JSON.stringify(imported.steps) === JSON.stringify(exported.steps) && JSON.stringify(imported.inputs) === JSON.stringify(exported.inputs))
  check('import remains read-only, unscheduled and never run', imported?.route.mode === 'ask' && imported?.schedule === undefined && imported?.execution === undefined && imported?.runs === 0 && after.history.data.missions.length === 0)
  // Folder selection is checked without dispatching another model turn. The
  // dev-only seam supplies the native picker's result; typing a path is refused.
  await drive.evaluate(`document.querySelector('button[aria-label="Edit Portable notes"]')?.click()`)
  await sleep(200)
  await drive.evaluate(`(${click})('Add input')`)
  await sleep(200)
  await drive.evaluate(`(${SET})('[aria-label="Input 3 key"]', 'folder'); (${SET})('[aria-label="Input 3 label"]', 'Folder')`)
  await drive.evaluate(`(${CHOOSE})('Input 3 kind', 'Folder')`)
  await drive.evaluate(`(${SET})('textarea[aria-label="Step 1"]', ${JSON.stringify('Report {{topic}} in {{style}} with folder {{folder}}.')})`)
  await drive.evaluate(`(${click})('Save changes')`)
  await sleep(500)
  const typed = await drive.evaluate(`window.desktop.runRoutine(${JSON.stringify(imported.routineId)}, { topic: 'test', style: 'Brief', folder: ${JSON.stringify(workspace)} })`)
  check('a typed folder path cannot start a run', !typed.ok && /folder picker/.test(typed.error.message), JSON.stringify(typed))
  await drive.evaluate(`(${click})('Run')`)
  await sleep(200)
  const unpicked = await drive.capture('a required folder has a picker and blocks Run', () => drive.evaluate(`({ picker: [...document.querySelectorAll('button')].some((b) => b.innerText === 'Choose Folder'), typedField: !!document.querySelector('[aria-label="Value for Folder"]'), disabled: document.querySelector('[role=dialog] .lc-primarybutton')?.disabled })`))
  check('folder input offers a picker, no typed field, and blocks Run until chosen', unpicked.picker && !unpicked.typedField && unpicked.disabled, JSON.stringify(unpicked))
  await drive.evaluate(`(${click})('Choose Folder')`)
  await sleep(200)
  const picked = await drive.capture('picked folder is displayed before running', () => drive.evaluate(`({ folder: document.querySelector('[role=dialog] output')?.innerText, disabled: document.querySelector('[role=dialog] .lc-primarybutton')?.disabled })`))
  check('the host-selected folder fills the value and enables Run', picked.folder === workspace && picked.disabled === false, JSON.stringify(picked))
  await drive.evaluate(`(${click})('Cancel')`)
  const noRun = await drive.evaluate('window.desktop.getMissionHistory()')
  check('cancel after picking the folder still starts nothing', noRun.ok && noRun.data.missions.length === 0)
  check('import captures have no renderer errors', drive.record.every((entry) => entry.errors.length === 0))
} catch (error) {
  if (!(error instanceof Error && error.message === 'PACKAGED-STOP')) check('drive completes without an exception', false, error instanceof Error ? error.stack : String(error))
} finally {
  if (drive) await drive.finish({ intro: `W7 dev build; ${route.model}; fresh-profile routine import; no model run in this profile.`, extra: `${checks - failures}/${checks} checks passed. Transfer file: ${filePath}` })
}
say(`${checks - failures}/${checks} CHECKS PASSED; ${failures} FAILED`)
process.exitCode = failures === 0 ? 0 : 1
