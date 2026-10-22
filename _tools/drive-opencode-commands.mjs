// OpenCode's own slash commands in the / menu (0.427).
//
//   node _tools/drive-opencode-commands.mjs [--packaged <exe>] [--tag <name>]
//
// Free model (LOCUST_FREE_MODEL, Nemotron by default): spends nothing.
//
// The folder has one committed file and an uncommitted change that breaks it
// (add returns a - b). Before any run, the / menu must already list
// OpenCode's commands under its name. Ada (Ask) picks /review with Enter and
// sends it: OpenCode reviews the uncommitted change, reading it through
// Locust's git tool (0.818). Asked to write through a shell redirect, Ada
// writes nothing, in the folder or outside it. Bea (Approve each) sends
// /init with an argument: it goes through OpenCode's server, stops for the
// AGENTS.md it wants to write, and writes it once approved.
//
// PRIVACY. OpenCode lists every skill on the machine. The whole list is read
// as counts and yes/no answers; the menu is pictured only while every row
// showing is a known built-in.

import { existsSync } from 'node:fs'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'

import { git, openTeammateScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const MODEL = process.env.LOCUST_FREE_MODEL ?? 'opencode/nemotron-3-ultra-free'
const OUT = join(recordRoot('opencode-commands-2026-09-28'), `opencode-commands-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-drive-oc-cmds-ws-')
await writeFile(join(workspace, 'calc.py'), 'def add(a, b):\n    return a + b\n', 'utf8')
await git(['add', 'calc.py'], workspace)
await git(['commit', '-q', '-m', 'add calc'], workspace)
await writeFile(join(workspace, 'calc.py'), 'def add(a, b):\n    return a - b\n', 'utf8')

const SAFE = new Set(['ask', 'plan', 'edit', 'approve', 'model', 'swarm', 'stop', 'auto', 'init', 'review'])

const drive = await startDrive({
  name: `opencode-commands-${tag}`, port: 9764, workspace, outPath: OUT, launchElsewhere: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-09-28T01:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'ask' } },
      { teammateId: 'tm_bea', name: 'Bea', hue: 'lime', role: 'Docs & QA', createdAt: '2026-09-28T01:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'approve-each' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

const type = (text) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  field.focus()
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 400))
  return 'typed'
})()`
const press = (key) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  field.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(key)}, bubbles: true, cancelable: true }))
  await new Promise((r) => setTimeout(r, 500))
  return document.querySelector('form.command-dock textarea').value
})()`
const NAME_OF = `(row) => {
  const whole = row.querySelector('.lc-slash__name')?.textContent ?? ''
  const hint = row.querySelector('.lc-slash__hint')?.textContent ?? ''
  return whole.slice(0, whole.length - hint.length).trim().replace(/^\\//, '')
}`
const menu = `(() => {
  const rows = [...document.querySelectorAll('.lc-slash .lc-slash__item')]
  const names = rows.map(${NAME_OF})
  const groups = [...document.querySelectorAll('.lc-slash .lc-slash__group')].map((g) => g.textContent.trim())
  const safe = names.every((name) => ${JSON.stringify([...SAFE])}.includes(name))
  return JSON.stringify({ open: document.querySelector('.lc-slash') !== null, count: rows.length, safe, names: safe ? names : undefined, groups,
    has: Object.fromEntries(['init', 'review'].map((name) => [name, names.includes(name)])) })
})()`
/** Wait, re-reading the menu, until OpenCode's group shows or half a minute passes. */
const menuWithGroup = `(async () => {
  let read
  for (let i = 0; i < 60; i += 1) {
    read = JSON.parse(${menu})
    if (read.groups.includes('OpenCode')) break
    await new Promise((r) => setTimeout(r, 500))
  }
  return JSON.stringify(read)
})()`
/** Send what is in the box with Enter, answer every card with Approve once, and wait for the end. */
const sendAnswering = `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }))
  const cards = []
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    const approval = document.querySelector('[role=group][aria-label="Approval required"]')
    if (approval) {
      await new Promise((r) => setTimeout(r, 300))
      cards.push((approval.querySelector('.lc-approval__detail')?.innerText ?? approval.innerText).split(/\\s+/).join(' ').slice(0, 90))
      const button = [...approval.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))
      if (!button) break
      button.click()
      await new Promise((r) => setTimeout(r, 900))
      continue
    }
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) break
  }
  await new Promise((r) => setTimeout(r, 2500))
  return JSON.stringify({ cards, thread: (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-3000) })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2000)
  await drive.evaluate(openTeammateScript('Ada'))
  await sleep(800)

  // Before any run: the list comes from OpenCode's server, asked at launch.
  await drive.evaluate(type('/'))
  const first = JSON.parse(String(await drive.evaluate(menuWithGroup)))
  await drive.evaluate(press('Escape'))
  say(`  before any run: ${JSON.stringify({ ...first, names: undefined })}`)
  check("OpenCode's commands are listed under its name before any run", first.groups.includes('OpenCode') && first.has.init && first.has.review, JSON.stringify({ ...first, names: undefined }))

  const narrowed = JSON.parse(String(await drive.evaluate(`(async () => { await ${type('/rev')}; return ${menu} })()`)))
  if (narrowed.safe) await drive.capture('Typing /rev', () => drive.evaluate(menu))
  else say('  (the /rev menu shows a name that is not a known built-in; not pictured)')
  check('/rev narrows to /review, under OpenCode', narrowed.has.review && narrowed.groups.includes('OpenCode'), JSON.stringify({ ...narrowed, names: undefined }))
  let boxed
  if (narrowed.has.review) boxed = String(await drive.evaluate(press('Enter')))
  else {
    await drive.evaluate(type('/review '))
    boxed = '/review (typed whole)'
  }
  check('Enter writes "/review " into the box', boxed === '/review ', boxed)

  const reviewed = JSON.parse(String(await drive.capture('/review sent', () => drive.evaluate(sendAnswering))))
  say(`  review: ${reviewed.thread.slice(-300)}`)
  check('OpenCode reviewed the uncommitted change: add now subtracts', /calc|\badd\b|subtract|a - b|minus/i.test(reviewed.thread.slice(reviewed.thread.indexOf('/review') + 7)), reviewed.thread.slice(-300))

  // Ask writes nothing, a redirect included (0.818): 0.744's git allows let `git status > status.txt` through.
  const outside = join(dirname(workspace), `ask-mode-outside-${String(process.pid)}.txt`)
  await drive.evaluate(type(`Run exactly these two shell commands with your bash tool, one at a time, as written: git status > status.txt   and then   git log -1 > ../${basename(outside)}   Then say in one sentence whether each one ran.`))
  const redirected = JSON.parse(String(await drive.capture('Ask asked to write through a redirect', () => drive.evaluate(sendAnswering))))
  const wrote = [join(workspace, 'status.txt'), outside].filter((file) => existsSync(file))
  check('Ask mode wrote nothing, in the folder or outside it', wrote.length === 0, `${wrote.map((file) => basename(file)).join(', ') || 'nothing written'} || ${redirected.thread.slice(-240)}`)
  await Promise.all(wrote.map((file) => rm(file, { force: true })))

  await drive.evaluate(openTeammateScript('Bea'))
  await sleep(1000)
  const mode = String(await drive.evaluate(`[...document.querySelectorAll('.lc-control')].map((b) => b.innerText.trim()).find((t) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)/.test(t)) ?? ''`))
  check('Bea is in Approve each', /^Approve/.test(mode), mode)
  await drive.evaluate(type('/init Keep it under five lines.'))
  const inited = JSON.parse(String(await drive.capture('/init sent in Approve each', () => drive.evaluate(sendAnswering))))
  say(`  init: cards=${JSON.stringify(inited.cards)} thread=${inited.thread.slice(-200)}`)
  const written = await readFile(join(workspace, 'AGENTS.md'), 'utf8').catch(() => '')
  check('it stopped to ask before writing, and AGENTS.md was written once approved', inited.cards.length > 0 && written.length > 0, JSON.stringify({ cards: inited.cards.length, lines: written.split('\n').length }))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. Ada (Ask) and Bea (Approve each) on OpenCode / ${MODEL}; the folder has an uncommitted change that breaks calc.py. The menu is pictured only when every row is a known name.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
