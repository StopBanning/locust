// What waits on you, in one place (0.373).
//
//   node _tools/drive-needs-you.mjs [--packaged <exe>] [--tag <name>]
//
// Three things wait on a person, and each used to be found only where it
// happened: a question a teammate stopped on, a run paused for approval, a
// memory suggestion. The title bar now says how many ("2 need you") and its
// list opens where each is answered.
//
// Wren (a free model, Accept edits) is asked to stop on a question. Pip (a
// free model, Approve each) is asked to run one shell command and is left
// waiting. One memory suggestion is seeded. The drive reads the chip and the
// list from somewhere else in the app each time, follows each row to where
// it is answered, answers it, and watches the count come down to nothing.
//
// Free models only: nothing is spent.

import { createHash } from 'node:crypto'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('needs-you-2026-09-26'), `needs-you-${tag}`)
await mkdir(OUT, { recursive: true })

const WREN_MODEL = process.env.LOCUST_ROOM_MODEL_A ?? 'opencode/nemotron-3-ultra-free'
// The drives' free model (FREE_ROUTE): Pip's was LongCat by name until OpenCode dropped it (2026-10-22).
const PIP_MODEL = process.env.LOCUST_ROOM_MODEL_B ?? FREE_ROUTE.model
const workspace = await scratchRepository('locust-needs-you-ws-')
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const T0 = '2026-09-26T05:00:00.000Z'
const drive = await startDrive({
  name: `needs-you-${tag}`,
  port: 9674,
  workspace,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: { runtime: 'opencode', model: WREN_MODEL, mode: 'accept-edits' } },
      { teammateId: 'tm_pip', name: 'Pip', hue: 'violet', role: 'Docs & QA', createdAt: T0, route: { runtime: 'opencode', model: PIP_MODEL, mode: 'accept-edits' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'auto', autoMode: false }
  },
  files: {
    'memories.json': {
      schemaVersion: 1,
      memories: [
        { memoryId: 'mem_kept', text: 'Deploys go out on Thursdays.', scope: 'workspace', workspaceId, workspaceName: 'scratch', by: { name: 'you' }, createdAt: T0, status: 'kept', enabled: true },
        { memoryId: 'mem_waiting', text: 'The staging site is at staging.tides.test.', scope: 'workspace', workspaceId, workspaceName: 'scratch', by: { teammateId: 'tm_wren', name: 'Wren' }, createdAt: T0, status: 'proposed', enabled: true }
      ]
    }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

const CHIP = `(() => (document.querySelector('.lc-needsyou')?.innerText ?? '').replace(/\\s+/g, ' ').trim())()`
const chip = async () => String(await drive.evaluate(CHIP))
const home = () => drive.evaluate(`(async () => { document.querySelector('.lc-brand__lockup')?.click(); await new Promise(r => setTimeout(r, 800)); return 'home' })()`)
/**
 * Open the list fresh, and read its rows; the menu is left open. A list still
 * open from an earlier step is closed first: a row that was never there left it
 * open, a second click shut it, and reading it as it stood showed the rows of
 * when it opened -- one model that did not ask read as nine failures (2026-10-07).
 */
const LIST = `(async () => {
  const button = document.querySelector('.lc-needsyou')
  if (button === null) return JSON.stringify({ rows: [] })
  if (document.querySelector('.lc-context') !== null) {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await new Promise(r => setTimeout(r, 300))
  }
  button.click()
  await new Promise(r => setTimeout(r, 500))
  const rows = [...document.querySelectorAll('.lc-context .lc-context__item')].map((row) => (row.querySelector('.lc-context__label')?.textContent ?? row.innerText).trim())
  return JSON.stringify({ rows })
})()`
const pick = (pattern) => `(async () => {
  const row = [...document.querySelectorAll('.lc-context .lc-context__item')].find((item) => ${pattern}.test(item.innerText))
  if (!row) return 'no row matching ${pattern}'
  row.click()
  await new Promise(r => setTimeout(r, 1200))
  return 'picked'
})()`
const closeMenu = () => drive.evaluate(`(async () => { document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 300)) })()`)

/** Send in the open conversation; wait for the run to end, or for an approval card when told to. */
const send = (text, { untilApproval = false } = {}) => drive.evaluate(`(async () => {
  const field = document.querySelector('form.command-dock textarea')
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 120; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const button = document.querySelector('button[aria-label="Send"]')
    if (button && !button.disabled) { button.click(); break }
  }
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    if (${untilApproval} && document.querySelector('[role=group][aria-label="Approval required"]')) return 'approval waiting'
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended'
  }
  return 'timed out'
})()`)
const MODE_APPROVE_EACH = `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(b.innerText))
  if (!control) return 'no mode control'
  control.click(); await new Promise((r) => setTimeout(r, 400))
  const approve = [...document.querySelectorAll('[role=menuitemradio]')].find((b) => /^Approve each/.test(b.innerText.trim()))
  if (!approve || approve.disabled) { control.click(); return 'approve each not offered' }
  approve.click(); await new Promise((r) => setTimeout(r, 400))
  return 'mode: ' + control.innerText.split(/\\s+/).join(' ').trim()
})()`

try {
  await drive.ready()
  await drive.resize(1215, 800)
  check('a waiting memory suggestion is counted from the start', (await chip()) === '1 needs you', `"${await chip()}"`)

  // A question: Wren stops on one.
  await drive.evaluate(openTeammateScript('Wren'))
  const asked = await send('Before you do anything, ask me whether the new file should be called notes.md or todo.md -- use your question block, and stop there. Change nothing.')
  const card = String(await drive.evaluate(`(() => document.querySelector('.lc-decision__question')?.textContent?.trim() ?? 'no card')()`))
  check('Wren stopped on a question', asked === 'ended' && card !== 'no card', `${String(asked)} || ${card}`)

  await home()
  check('the question is counted from elsewhere in the app', (await chip()) === '2 need you', `"${await chip()}"`)
  const listed = JSON.parse(String(await drive.capture('the list, opened from Home', () => drive.evaluate(LIST))))
  check('the list says Wren asked, then the memory suggestion', listed.rows.length === 2 && /^Wren asked: /.test(listed.rows[0] ?? '') && listed.rows[1] === '1 memory suggestion waiting', JSON.stringify(listed.rows))
  await drive.evaluate(pick('/^Wren asked/'))
  const opened = String(await drive.evaluate(`(() => document.querySelector('.lc-decision') !== null ? 'the question card is on screen' : 'no question card')()`))
  check('its row opens Wren’s conversation, at the question', opened === 'the question card is on screen', opened)

  // A paused run: Pip asks to run a command, and waits.
  await drive.evaluate(openTeammateScript('Pip'))
  const mode = String(await drive.evaluate(MODE_APPROVE_EACH))
  const paused = await send('Run exactly this shell command with your bash tool: echo NEEDS-YOU > needs.txt   Then say in one sentence whether it ran.', { untilApproval: true })
  check('Pip, in Approve each, is waiting for approval', /^mode: Approve/.test(mode) && paused === 'approval waiting', `${mode} || ${String(paused)}`)
  await drive.evaluate(openTeammateScript('Wren'))
  check('the paused run is counted, from Wren’s conversation', (await chip()) === '3 need you', `"${await chip()}"`)
  const three = JSON.parse(String(await drive.capture('the list with a paused run first', () => drive.evaluate(LIST))))
  check('the paused run is first in the list, naming the command', /^Pip wants to run: .*NEEDS-YOU/.test(three.rows[0] ?? ''), JSON.stringify(three.rows))
  await drive.evaluate(pick('/^Pip wants to run/'))
  const approval = String(await drive.evaluate(`(async () => {
    const card = document.querySelector('[role=group][aria-label="Approval required"]')
    if (card === null) return 'no approval card'
    const approve = [...card.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))
    approve?.click()
    for (let i = 0; i < 400; i += 1) {
      await new Promise(r => setTimeout(r, 400))
      const next = document.querySelector('[role=group][aria-label="Approval required"]')
      if (next) [...next.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))?.click()
      if (i > 5 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'approved, and the run ended'
    }
    return 'still running'
  })()`))
  check('its row opens Pip’s conversation at the approval, and approving ends the run', approval === 'approved, and the run ended', approval)
  check('answered, it leaves the count', (await chip()) === '2 need you', `"${await chip()}"`)

  // The question, answered from its card.
  await drive.evaluate(openTeammateScript('Wren'))
  const answered = String(await drive.evaluate(`(async () => {
    const option = document.querySelector('.lc-decision__option')
    if (option === null) return 'no option'
    const label = option.innerText.trim()
    option.click()
    for (let i = 0; i < 20; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      if (document.querySelector('button[aria-label^="Stop the running"]')) return 'answered: ' + label
    }
    return 'answered, no run started: ' + label
  })()`))
  await sleep(1500)
  check('answering the question takes it off the count', /^answered: /.test(answered) && (await chip()) === '1 needs you', `${answered} || "${await chip()}"`)

  // The memory suggestion, from the list.
  await drive.evaluate(LIST)
  await drive.evaluate(pick('/memory suggestion/'))
  const memory = String(await drive.evaluate(`(async () => {
    const heading = document.querySelector('.lc-screen__title')?.textContent?.trim() ?? ''
    const row = document.querySelector('.lc-memory.is-proposed')
    const keep = row === null ? null : [...row.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Keep')
    keep?.click()
    await new Promise(r => setTimeout(r, 1200))
    return heading + ' || kept: ' + String(keep !== undefined && keep !== null)
  })()`))
  await sleep(800)
  await drive.capture('everything answered', () => drive.evaluate(CHIP))
  check('its row opens the Memory screen; kept, the chip is gone', /^Memory \|\| kept: true$/i.test(memory) && (await chip()) === '', `${memory} || chip "${await chip()}"`)
  await closeMenu()
  say(failures === 0 ? '\nNEEDS YOU PASSED' : `\nNEEDS YOU: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Wren on ${WREN_MODEL} (Accept edits), Pip on ${PIP_MODEL} (Approve each), one memory suggestion seeded. A question, a paused run and a suggestion: counted in the title bar, listed, followed and answered.` })
}
