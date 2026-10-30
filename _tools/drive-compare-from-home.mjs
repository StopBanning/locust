// Compare, for someone with no teammates (0.442).
//
//   node _tools/drive-compare-from-home.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only (OpenCode's free tier): spends nothing.
//
// Colin, 2026-09-28: "shouldnt compare mode be implemented in regular chats as
// well? You dont NEED a teammate assigned to complete the task and want new
// users to be able to try that out". A profile with NO teammates: Home's chat
// box offers Compare in its mode chip (Home's own button is gone); choosing it
// puts the chat box on Compare;
// two free models ticked, one ask, both columns answer at once with nobody's
// name on them; Keep this one leaves an ordinary conversation of nobody's.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { recordRoot, say, scratchRepository, sleep, startDrive, comparePick, comparePickScript, enterCompareScript, freeModels } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
// Two free models OpenCode serves today (freeModels), unless LOCUST_COMPARE_PICKS names them.
const PICKS = (process.env.LOCUST_COMPARE_PICKS?.split(',') ?? freeModels().slice(0, 2).map((model) => model.replace(/^opencode\//, '')))
const OUT = join(recordRoot('compare-from-home-2026-09-28'), `compare-from-home-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-drive-compare-home-ws-')
const drive = await startDrive({
  focused: true,
  name: `compare-from-home-${tag}`, port: 9777, workspace, outPath: OUT,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(4000)

  // Home's "Compare models" button is gone (by 0.842): with no teammate, a person reaches Compare from the chat
  // box's own mode chip, "Direct or compare". The chip is read as offered, then pressed as a person presses it.
  const offered = JSON.parse(String(await drive.capture('Home: the chat box offers Compare', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && !document.querySelector('.lc-control--chatmode'); i += 1) await new Promise((r) => setTimeout(r, 500))
    const chip = document.querySelector('.lc-control--chatmode')
    if (!chip) return JSON.stringify({ chip: false })
    chip.click()
    await new Promise((r) => setTimeout(r, 400))
    const items = [...document.querySelectorAll('[role="menu"][aria-label="Direct or compare"] [role="menuitemradio"], [role="menu"][aria-label="Direct or compare"] button')]
      .map((item) => item.innerText.trim().split('\\n')[0])
    chip.click()
    await new Promise((r) => setTimeout(r, 300))
    return JSON.stringify({ chip: true, items })
  })()`))))
  const entered = JSON.parse(String(await drive.evaluate(enterCompareScript())))
  const on = String(await drive.evaluate(`document.querySelector('.lc-control--chatmode')?.getAttribute('aria-label')?.replace('Chat mode: ', '') ?? ''`))
  const opened = { ...offered, entered: entered.how, on, labels: [] }
  // Ticked one at a time through drive-lib (2026-10-06): the picker names rows by display name, draws only those
  // in view, and closes after a pick, leaving a chip per column -- comparePickScript handles all three.
  for (const [index, id] of PICKS.entries()) {
    const got = JSON.parse(String(await drive.evaluate(comparePickScript(index, comparePick(id)))))
    if (got.picked) opened.labels = [...(opened.labels ?? []), got.label]
    else say(`  pick ${id}: ${got.why}`)
  }
  say(`  opened: ${JSON.stringify(opened)}`)
  check("Home's chat box offers Compare, with no teammate at all", opened.chip === true && opened.items.some((item) => /^Compare/.test(item)), JSON.stringify(opened))
  check('choosing it puts the chat box on Compare, ready for two models', ['picker', 'columns'].includes(opened.entered) && opened.on === 'Compare', JSON.stringify(opened))
  check('two free models tick', opened.labels?.length === 2, JSON.stringify(opened.labels))

  const answered = JSON.parse(String(await drive.capture('Both columns answer, nobody\'s', () => drive.evaluate(`(async () => {
    [...document.querySelectorAll('.lc-picker__foot--compare button')].find((b) => b.textContent.trim() === 'Done')?.click()
    await new Promise((r) => setTimeout(r, 500))
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'In one sentence: what is a pull request?')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    document.querySelector('button[aria-label="Send"]')?.click()
    let bothRan = false
    // Under the 400 s a single evaluate is given (drive-lib): past it the drive read "undefined", not the columns.
    for (let i = 0; i < 700; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      const states = [...document.querySelectorAll('.lc-compare__state')].map((el) => el.textContent.trim())
      if (states.length === 2 && states.every((state) => state === 'working')) bothRan = true
      if (i > 6 && states.length === 2 && states.every((state) => state !== 'working' && state !== 'waiting')) break
    }
    await new Promise((r) => setTimeout(r, 1200))
    return JSON.stringify({
      open: document.querySelector('.lc-compare') !== null,
      bothRan,
      heads: [...document.querySelectorAll('.lc-compare__head')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim()),
      cells: [...document.querySelectorAll('.lc-compare__cell')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim().length),
      rows: document.querySelectorAll('.lc-conv').length,
      vsRows: [...document.querySelectorAll('.lc-conv')].filter((row) => row.querySelector('.lc-conv__vs')).length
    })
  })()`))))
  say(`  answered: ${JSON.stringify(answered)}`)
  check('the comparison opens with two columns, both done', answered.open && answered.heads.length === 2 && answered.heads.every((head) => / done$/.test(head)), JSON.stringify(answered.heads))
  check('both columns ran at once, with nobody\'s name on them', answered.bothRan === true, JSON.stringify(answered))
  check('both answered', answered.cells.length === 2 && answered.cells.every((length) => length > 20), JSON.stringify(answered.cells))
  check('the sidebar lists it once, marked vs', answered.rows === 1 && answered.vsRows === 1, JSON.stringify(answered))

  const kept = JSON.parse(String(await drive.capture('Kept: an ordinary conversation of nobody\'s', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-compare__foot .lc-primarybutton')?.click()
    for (let i = 0; i < 40 && document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1200))
    return JSON.stringify({
      gone: document.querySelector('.lc-compare') === null,
      thread: (document.querySelector('.lc-thread')?.innerText ?? '').length,
      compared: document.querySelector('.lc-compared')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
      rows: document.querySelectorAll('.lc-conv').length
    })
  })()`))))
  say(`  kept: ${JSON.stringify(kept)}`)
  check('Keep this one leaves an ordinary conversation that says what it was compared with', kept.gone && kept.thread > 40 && /^Compared with .+\. Open the comparison$/.test(kept.compared) && kept.rows === 1, JSON.stringify(kept))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. A profile with no teammates; two free OpenCode models compared from Home.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
