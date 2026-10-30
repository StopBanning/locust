// Compare changes (0.445): each model edits its own copy; Keep brings one in.
//
//   node _tools/drive-compare-changes.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only (OpenCode's free tier): spends nothing.
//
// Colin, 2026-09-28, asking for Compare: "a room where you can compare work
// between different models". Phase one compared answers; this is the work.
// In a git project, with no teammates: Compare models, two free models, the
// mode chip on Edit; one ask to fix cart.py. Each column changes ITS OWN copy
// -- the folder's cart.py is untouched while they work -- and its foot says
// how much it changed. Keep this one brings that column's change into the
// folder, uncommitted, and removes both copies and their branches.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { comparePick, freeModels, comparePickScript, compareDoneScript, enterCompareScript, git, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
// Mimo was rate-limited through 2026-10-06; Fledge Alpha answered every drive asked of it that day.
// Two free models OpenCode serves today (freeModels), unless LOCUST_COMPARE_PICKS names them.
const PICKS = (process.env.LOCUST_COMPARE_PICKS?.split(',') ?? freeModels().slice(0, 2).map((model) => model.replace(/^opencode\//, '')))
const OUT = join(recordRoot('compare-changes-2026-09-28'), `compare-changes-${tag}`)
await mkdir(OUT, { recursive: true })

const CART = 'def total(prices):\n    """The cart total: every price added up."""\n    return 0\n'
const workspace = await scratchRepository('locust-drive-compare-changes-ws-')
await writeFile(join(workspace, 'cart.py'), CART, 'utf8')
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'a cart that totals nothing'], workspace)
const inFolder = () => readFileSync(join(workspace, 'cart.py'), 'utf8').replace(/\r\n/g, '\n')
const status = () => execFileSync('git', ['status', '--porcelain'], { cwd: workspace, encoding: 'utf8' }).split(/\r?\n/).filter(Boolean)
const branches = () => execFileSync('git', ['branch', '--list', '--format=%(refname:short)'], { cwd: workspace, encoding: 'utf8' }).split(/\r?\n/).filter(Boolean)
// The columns' worktrees, wherever they are: since 0.493 outside the folder (a path under it led a model back to it).
const flat = (path) => path.replace(/\\/g, '/').toLowerCase()
const copies = () => execFileSync('git', ['worktree', 'list', '--porcelain'], { cwd: workspace, encoding: 'utf8' })
  .split(/\r?\n/).filter((line) => line.startsWith('worktree ')).map((line) => line.slice('worktree '.length).trim())
  .filter((path) => flat(path) !== flat(workspace))
const insideFolder = () => copies().filter((path) => flat(path).startsWith(`${flat(workspace)}/`))

const drive = await startDrive({
  focused: true,
  name: `compare-changes-${tag}`, port: 9779, workspace, outPath: OUT,
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

  // Compare set up as a person does it, through drive-lib's helpers (2026-10-06: the picker names rows by display
  // name and draws only those in view, and Compare models may arrive with models already in the box).
  const entered = JSON.parse(String(await drive.evaluate(enterCompareScript())))
  const labels = []
  for (const [index, id] of PICKS.entries()) {
    const got = JSON.parse(String(await drive.evaluate(comparePickScript(index, comparePick(id)))))
    if (got.picked) labels.push(got.label)
    else say(`  pick ${id}: ${got.why} (entered: ${entered.how})`)
  }
  await drive.evaluate(compareDoneScript())
  const picked = JSON.parse(String(await drive.capture('Two free models ticked; the mode chip set to Edit', () => drive.evaluate(`(async () => {
    const labels = ${JSON.stringify(labels)}
    const chip = document.querySelector('button[aria-label="Permission mode"]')
    const before = chip?.textContent.trim() ?? ''
    const chipDisabled = chip?.disabled ?? true
    chip?.click()
    await new Promise((r) => setTimeout(r, 400))
    const items = [...document.querySelectorAll('.lc-menu[aria-label="What the comparison does"] .lc-menu__item')].map((item) => item.innerText.replace(/\\s+/g, ' ').trim())
    ;[...document.querySelectorAll('.lc-menu[aria-label="What the comparison does"] .lc-menu__item')].find((item) => item.querySelector('.lc-menu__name')?.textContent.trim() === 'Auto')?.click()
    await new Promise((r) => setTimeout(r, 400))
    return JSON.stringify({ labels, before, chipDisabled, items, after: document.querySelector('button[aria-label="Permission mode"]')?.textContent.trim() ?? '' })
  })()`))))
  say(`  picked: ${JSON.stringify(picked)}`)
  check('two free models tick', picked.labels?.length === 2, JSON.stringify(picked.labels))
  check('the mode chip offers Ask or Auto for the comparison, and takes Auto', picked.chipDisabled === false && picked.items?.length === 2 && picked.after === 'Auto', JSON.stringify(picked))

  const worked = JSON.parse(String(await drive.capture('Both columns changed their own copies; the feet say how much', () => drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'In cart.py, make total(prices) return the sum of the prices. Change only cart.py, and keep the docstring.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    document.querySelector('button[aria-label="Send"]')?.click()
    let both = false
    for (let i = 0; i < 1400; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      const states = [...document.querySelectorAll('.lc-compare__state')].map((el) => el.textContent.trim())
      if (states.length === 2 && states.every((state) => state === 'working')) both = true
      if (i > 6 && states.length === 2 && states.every((state) => state !== 'working' && state !== 'waiting')) break
    }
    // The feet are read once the columns settle.
    for (let i = 0; i < 40 && [...document.querySelectorAll('.lc-compare__numbers')].some((el) => !/in \\d+ files?|no changes/.test(el.textContent)); i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify({
      both,
      bar: document.querySelector('.lc-compare__bar')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
      heads: [...document.querySelectorAll('.lc-compare__head')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim()),
      feet: [...document.querySelectorAll('.lc-compare__numbers')].map((el) => el.textContent.trim()),
      keepTitle: document.querySelector('.lc-compare__foot .lc-primarybutton')?.title ?? '',
      tallies: document.querySelectorAll('.lc-compare .lc-activity__counts').length,
      internal: (document.querySelector('.lc-compare')?.innerText ?? '').includes('.locust/compare')
    })
  })()`))))
  say(`  worked: ${JSON.stringify(worked)}`)
  check('the bar says each changes its own copy and only the kept one comes in', /Each changes its own copy of your project; only the one you keep comes into your folder\./.test(worked.bar), worked.bar)
  check('both columns ran at once and are done', worked.both === true && worked.heads.length === 2 && worked.heads.every((head) => / done$/.test(head)), JSON.stringify(worked.heads))
  check('each foot says what its model changed', worked.feet.length === 2 && worked.feet.every((foot) => /^\+\d+ −\d+ in 1 file/.test(foot)), JSON.stringify(worked.feet))
  check("the folder's own cart.py is untouched while they compare, and nothing in it changed", inFolder() === CART && status().length === 0, JSON.stringify({ status: status() }))
  check('each column has its own copy, on its own branch', copies().length === 2 && branches().filter((branch) => branch.startsWith('locust/compare-')).length === 2, JSON.stringify({ copies: copies(), branches: branches() }))
  check('no column works inside the folder: its path does not lead back to it (Sol, 0.491)', insideFolder().length === 0, JSON.stringify({ inside: insideFolder() }))
  check("no row shows a copy's own path: each file reads as the folder's", worked.internal === false, JSON.stringify({ internal: worked.internal }))
  check("a column's summary does not repeat a running tally beside the foot (0.453)", worked.tallies === 0, JSON.stringify({ tallies: worked.tallies }))
  check('Keep says where the changes go', /Its changes come into your folder, not committed/.test(worked.keepTitle), worked.keepTitle)

  const kept = JSON.parse(String(await drive.capture("Kept: its change is in the folder, uncommitted, and the copies are gone", () => drive.evaluate(`(async () => {
    const keptCell = document.querySelector('.lc-compare__cell')?.innerText ?? ''
    document.querySelector('.lc-compare__foot .lc-primarybutton')?.click()
    for (let i = 0; i < 60 && document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1200))
    return JSON.stringify({
      gone: document.querySelector('.lc-compare') === null,
      problem: document.querySelector('.lc-compare__problem')?.textContent ?? '',
      compared: document.querySelector('.lc-compared')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
      rows: document.querySelectorAll('.lc-conv').length,
      keptCell: keptCell.length
    })
  })()`))))
  say(`  kept: ${JSON.stringify(kept)}`)
  const after = inFolder()
  check('Keep leaves the ordinary conversation, saying its change came into the folder', kept.gone && /^Compared with .+\. Its changes came into your folder: cart\.py\. Open the comparison$/.test(kept.compared) && kept.rows === 1, JSON.stringify(kept))
  check("the kept column's change is in the folder, not committed", after !== CART && /sum|\+=|for /.test(after) && status().join(',') === ' M cart.py' && execFileSync('git', ['log', '--oneline'], { cwd: workspace, encoding: 'utf8' }).split(/\r?\n/).filter(Boolean).length === 2, JSON.stringify({ status: status(), cart: after.slice(0, 160) }))
  check('both copies and their branches are removed', copies().length === 0 && branches().every((branch) => !branch.startsWith('locust/compare-')), JSON.stringify({ copies: copies(), branches: branches() }))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. A git project with a cart.py that totals nothing, no teammates; two free OpenCode models compared in Edit, one kept.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
