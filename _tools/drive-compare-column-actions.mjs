// Compare's column actions (0.444): Try again, Copy, Focus.
//
//   node _tools/drive-compare-column-actions.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only (OpenCode's free tier): spends nothing.
//
// Colin, 2026-09-28, on arena.ai: "if theres anything you want to take from
// this ... feel free". Arena gives each column Regenerate, Copy and Expand.
// Here: two free models compared from Home; Locust is closed while both are
// still answering, the way a person quits mid-answer; on the next start
// neither column has an answer, so each offers Try again where Keep would be.
// Both are tried again and finish, the sidebar still lists ONE comparison,
// Copy puts a column's words on the clipboard, and Focus gives one column
// the width with the other a rail -- and back.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { recordRoot, say, scratchRepository, sleep, startDrive, comparePick, comparePickScript, enterCompareScript, freeModels } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
// Two free models OpenCode serves today (freeModels), unless LOCUST_COMPARE_PICKS names them.
const PICKS = (process.env.LOCUST_COMPARE_PICKS?.split(',') ?? freeModels().slice(0, 2).map((model) => model.replace(/^opencode\//, '')))
const OUT = join(recordRoot('compare-column-actions-2026-09-28'), `compare-column-actions-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-drive-compare-actions-ws-')
let drive = await startDrive({
  focused: true,
  name: `compare-column-actions-${tag}`, port: 9778, workspace, outPath: OUT, keep: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const HEADS = `[...document.querySelectorAll('.lc-compare__head')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim())`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(4000)
  // Home's "Compare models" button is gone: a person reaches Compare from the chat box's mode chip (Direct or
  // compare), which drive-lib's enterCompareScript presses.
  const entered = JSON.parse(String(await drive.evaluate(enterCompareScript())))
  say(`  entered: ${entered.how}`)
  // CHANGELOG 0.460.0: "Compare picks its models like Arena." Choose both columns
  // before sending; choosing them after the ask only edits a later composer, not the run.
  const labels = []
  for (const [index, id] of PICKS.entries()) {
    const got = JSON.parse(String(await drive.evaluate(comparePickScript(index, comparePick(id)))))
    if (got.picked) labels.push(got.label)
    else say(`  pick ${id}: ${got.why}`)
  }
  const started = JSON.parse(String(await drive.capture('Two free models asked; Locust is closed while both are answering', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-picker__foot--compare button')].find((b) => b.textContent.trim() === 'Done')?.click()
    await new Promise((r) => setTimeout(r, 500))
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'In three short sentences: what is a git worktree, and when would you use one?')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    document.querySelector('button[aria-label="Send"]')?.click()
    let both = false
    for (let i = 0; i < 120 && !both; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const states = [...document.querySelectorAll('.lc-compare__state')].map((el) => el.textContent.trim())
      both = states.length === 2 && states.every((state) => state === 'working')
    }
    return JSON.stringify({ labels: ${JSON.stringify(labels)}, both, heads: ${HEADS} })
  })()`))))
  say(`  started: ${JSON.stringify(started)}`)
  check('two free models tick, and both columns are answering', started.labels?.length === 2 && started.both === true, JSON.stringify(started))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  // Closed mid-answer: the app and its runs are ended, the profile kept.
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. A profile with no teammates; two free OpenCode models compared from Home, and Locust closed while both answered.`, extra: `Checks failed so far: ${String(failures)}` })
}

const profilePath = drive.profile
await mkdir(join(OUT, 'after-relaunch'), { recursive: true })
drive = await startDrive({ name: `compare-column-actions-${tag}-again`, port: 9778, workspace, outPath: join(OUT, 'after-relaunch'), profilePath, stepFrom: 1, ...(packaged === undefined ? {} : { packaged }) })
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(3000)
  const reopened = JSON.parse(String(await drive.capture('Reopened: neither column has an answer, so each offers Try again', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      row = [...document.querySelectorAll('.lc-conv')].find((one) => one.querySelector('.lc-conv__vs'))
      if (!row) await new Promise((r) => setTimeout(r, 250))
    }
    row?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return JSON.stringify({
      open: document.querySelector('.lc-compare') !== null,
      heads: ${HEADS},
      feet: [...document.querySelectorAll('.lc-compare__foot button')].map((b) => b.textContent.trim()),
      rows: document.querySelectorAll('.lc-conv').length
    })
  })()`))))
  say(`  reopened: ${JSON.stringify(reopened)}`)
  check('reopened, each column offers Try again and neither offers Keep', reopened.open && reopened.feet.length === 2 && reopened.feet.every((label) => label === 'Try again'), JSON.stringify(reopened))

  const tried = JSON.parse(String(await drive.capture('Both tried again: both answer, and it is still one comparison', () => drive.evaluate(`(async () => {
    for (const button of [...document.querySelectorAll('.lc-compare__foot button')].filter((b) => b.textContent.trim() === 'Try again')) {
      button.click()
      for (let i = 0; i < 40 && [...document.querySelectorAll('.lc-compare__foot button')].some((b) => b.textContent.trim() === 'Starting…'); i += 1) await new Promise((r) => setTimeout(r, 250))
      await new Promise((r) => setTimeout(r, 400))
    }
    let working = 0
    // Under the 400 s a single evaluate is given (drive-lib): past it the drive read "undefined", not the columns.
    for (let i = 0; i < 700; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      const states = [...document.querySelectorAll('.lc-compare__state')].map((el) => el.textContent.trim())
      working = Math.max(working, states.filter((state) => state === 'working').length)
      if (i > 6 && states.length === 2 && states.every((state) => state !== 'working')) break
    }
    await new Promise((r) => setTimeout(r, 1200))
    return JSON.stringify({
      working,
      heads: ${HEADS},
      cells: [...document.querySelectorAll('.lc-compare__cell')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim().length),
      feet: [...document.querySelectorAll('.lc-compare__foot button')].map((b) => b.textContent.trim()),
      problem: document.querySelector('.lc-compare__problem')?.textContent ?? '',
      rows: document.querySelectorAll('.lc-conv').length,
      vsRows: [...document.querySelectorAll('.lc-conv')].filter((row) => row.querySelector('.lc-conv__vs')).length
    })
  })()`))))
  say(`  tried: ${JSON.stringify(tried)}`)
  check('Try again ran both columns again', tried.working >= 1 && tried.problem === '', JSON.stringify(tried))
  check('both are done, each with one answer in its place under the ask', tried.heads.every((head) => / done$/.test(head)) && tried.cells.length === 2 && tried.cells.every((length) => length > 20), JSON.stringify(tried))
  check('each now offers Keep this one', tried.feet.length === 2 && tried.feet.every((label) => label === 'Keep this one'), JSON.stringify(tried.feet))
  check('the sidebar still lists one comparison: the answers that were replaced are folded in', tried.rows === 1 && tried.vsRows === 1, JSON.stringify(tried))

  const copied = JSON.parse(String(await drive.capture("Copy: the first column's answer", () => drive.evaluate(`(async () => {
    // What the app hands the clipboard, caught on its way: the drive never
    // touches the person's own clipboard (the page cannot read it back anyway).
    let text = ''
    navigator.clipboard.writeText = async (value) => { text = String(value) }
    const button = document.querySelector('.lc-compare__head .lc-compare__tool[aria-label^="Copy"]')
    button?.click()
    await new Promise((r) => setTimeout(r, 300))
    const cell = document.querySelector('.lc-compare__cell')?.innerText ?? ''
    return JSON.stringify({ title: button?.title ?? '', length: text.length, head: text.slice(0, 60), inCell: text.length > 0 && cell.replace(/\\s+/g, ' ').includes(text.slice(0, 40).replace(/\\s+/g, ' ')) })
  })()`))))
  say(`  copied: ${JSON.stringify(copied)}`)
  check("Copy puts that column's words on the clipboard and says so", copied.title === 'Copied.' && copied.length > 20 && copied.inCell, JSON.stringify(copied))

  const widths = `[...document.querySelectorAll('.lc-compare__head')].map((el) => Math.round(el.getBoundingClientRect().width))`
  const focused = JSON.parse(String(await drive.capture('Focus: the second column has the width, the first is a rail', () => drive.evaluate(`(async () => {
    const before = ${widths}
    ;[...document.querySelectorAll('.lc-compare__head')][1]?.querySelector('.lc-compare__tool[aria-pressed]')?.click()
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({
      before,
      after: ${widths},
      rail: document.querySelector('.lc-compare__head.is-rail')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
      railCells: [...document.querySelectorAll('.lc-compare__cell.is-rail')].map((el) => el.innerText.trim().length),
      feet: [...document.querySelectorAll('.lc-compare__foot button')].map((b) => b.textContent.trim()),
      overflow: document.querySelector('.lc-compare__scroll').scrollWidth - document.querySelector('.lc-compare__scroll').clientWidth
    })
  })()`))))
  say(`  focused: ${JSON.stringify(focused)}`)
  check('Focus gives one column the width and keeps the other as a rail with its name', focused.after[0] < 200 && focused.after[1] > focused.before[1] + 300 && focused.rail.length > 3 && focused.railCells.every((length) => length === 0), JSON.stringify(focused))
  check('the rail offers no Keep; the focused column does, and nothing scrolls sideways', focused.feet.length === 1 && focused.feet[0] === 'Keep this one' && focused.overflow <= 1, JSON.stringify(focused))

  const swapped = JSON.parse(String(await drive.capture('The rail pressed: the first column has the width', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-compare__head.is-rail')?.click()
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({ after: ${widths}, rail: document.querySelector('.lc-compare__head.is-rail')?.innerText.replace(/\\s+/g, ' ').trim() ?? '' })
  })()`))))
  say(`  swapped: ${JSON.stringify(swapped)}`)
  check('pressing the rail focuses it instead', swapped.after[1] < 200 && swapped.after[0] > 600, JSON.stringify(swapped))

  const back = JSON.parse(String(await drive.capture('Every answer side by side again', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-compare__tool[aria-pressed="true"]')?.click()
    await new Promise((r) => setTimeout(r, 500))
    return JSON.stringify({ after: ${widths}, rails: document.querySelectorAll('.is-rail').length })
  })()`))))
  say(`  back: ${JSON.stringify(back)}`)
  check('its own button shows every answer again, side by side', back.rails === 0 && Math.abs(back.after[0] - back.after[1]) <= 2, JSON.stringify(back))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Relaunched on the same profile.', extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
