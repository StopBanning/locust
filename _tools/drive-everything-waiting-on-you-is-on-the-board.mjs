// Everything waiting on you is in one place (0.816, Grok's handoff): a Scout card that needs you now, and a routine
// waiting for Keep or Discard, join the title bar's count and list and the Board's Needs you column.
//
//   node _tools/drive-everything-waiting-on-you-is-on-the-board.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only: nothing is spent. The folder's check, `node check.js`, fails until answer.txt says 42. Scout,
// watching the check, raises "The check node check.js fails" as a card that needs you now. A routine that works in a
// copy writes answer.txt there and waits for Keep. Then:
//   1. The Board's Needs you column shows both, above the conversations, each with its own buttons -- at three sizes.
//   2. The title bar counts them and lists them, each row saying what it is.
//   3. Keep on the Board writes the answer into the folder, and the routine leaves the column.
//   4. Dismiss on the Board takes Scout's card away, there and on Home.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

const workspace = await scratchRepository('locust-drive-board-ws-')
await writeFile(join(workspace, 'check.js'), [
  "const fs = require('fs')",
  "const said = fs.existsSync('answer.txt') ? fs.readFileSync('answer.txt', 'utf8').trim() : 'nothing'",
  "if (said !== '42') { console.log('CHECK FAILED: answer.txt must say 42, and it says ' + said); process.exit(1) }",
  "console.log('ok: answer.txt says 42')",
  ''
].join('\n'), 'utf8')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-board-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const CEDAR = { teammateId: 'tm_cedar', name: 'Cedar', hue: 'lime', role: 'Custom', roleTitle: 'Numbers', createdAt: '2026-10-22T05:00:00.000Z', route: { runtime: 'opencode', model: FREE_ROUTE.model, mode: 'accept-edits' } }
await writeFile(join(profilePath, 'routines.json'), JSON.stringify({
  schemaVersion: 1,
  routines: [{
    routineId: 'rt_board000000000000000000',
    name: 'Answer file',
    teammateId: CEDAR.teammateId,
    route: CEDAR.route,
    steps: ['Create a file named answer.txt in the folder you are working in, holding exactly the number 42 and nothing else. Do not run any check yourself. Reply with the word WRITTEN.'],
    learnedFrom: [],
    createdAt: new Date(Date.now() - 3_600_000).toISOString(),
    runs: 0,
    workspaceId,
    inCopy: true
  }]
}), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `board-waits-${tag}`,
  port: 9877,
  workspace,
  profilePath,
  outPath: join(recordRoot('everything-waiting-on-you-is-on-the-board-2026-10-22'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [CEDAR],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, checkCommands: { [workspaceId]: 'node check.js' } }
  }
})

const state = async () => JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then((s) => JSON.stringify(s))')))
const routineRow = `[...document.querySelectorAll('.lc-routinerow:not(.lc-routineadd)')].find((r) => /Answer file/.test(r.innerText))`
// Its own statement wherever it lands: a line that starts with [ would carry on the line above it.
const OPEN_ROUTINES = `;[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()`
const OPEN_BOARD = `(async () => {
  ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Board')?.click()
  for (let i = 0; i < 40 && !document.querySelector('.lc-convboard'); i += 1) await new Promise((r) => setTimeout(r, 100))
  return document.querySelector('.lc-convboard') ? 'board' : 'no board'
})()`
const NEEDS_YOU = `JSON.stringify({
  column: document.querySelector('.lc-boardsection--needs-you')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  scout: [...document.querySelectorAll('[data-board-scout]')].map((card) => ({ text: card.innerText.replace(/\\s+/g, ' ').trim(), buttons: [...card.querySelectorAll('button')].map((b) => b.innerText.trim()).filter(Boolean) })),
  routines: [...document.querySelectorAll('[data-board-routine]')].map((card) => ({ text: card.innerText.replace(/\\s+/g, ' ').trim(), buttons: [...card.querySelectorAll('button')].map((b) => b.innerText.trim()).filter(Boolean) }))
})`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  // Scout, watching the folder's check, which fails.
  await drive.evaluate(`window.desktop.scout.turnOn(${JSON.stringify({ runtime: FREE_ROUTE.runtime, model: FREE_ROUTE.model })})`)
  await drive.evaluate(`window.desktop.scout.set({ watches: { github: false, teammates: true, check: true } })`)
  await drive.evaluate('window.desktop.scout.look()')
  let scout = await state()
  for (let i = 0; i < 120 && !scout.cards.some((card) => card.urgency === 'now'); i += 1) { await sleep(500); scout = await state() }
  check('Scout raised the failing check as a card that needs you now', scout.cards.some((card) => card.urgency === 'now' && /^The check node check\.js fails/.test(card.said)), JSON.stringify(scout.cards.map((card) => card.said)))

  // The routine, run in its copy, waits for Keep.
  const ran = String(await drive.evaluate(`(async () => {
    ${OPEN_ROUTINES}
    await new Promise((r) => setTimeout(r, 1200))
    ;[...((${routineRow})?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Run')?.click()
    for (let i = 0; i < 600; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      ${OPEN_ROUTINES}
      const now = ${routineRow}
      if (now?.querySelector('.lc-routinechanges')) return 'waiting: ' + now.querySelector('.lc-routinechanges').innerText.replace(/\\s+/g, ' ').trim()
    }
    return 'never waited'
  })()`))
  check('the routine’s copy waits for Keep', ran.startsWith('waiting: ') && ran.includes('answer.txt'), ran)

  // 1. Both on the Board, at three sizes.
  const board = JSON.parse(String(await drive.capture('the Board: everything waiting on you', async () => {
    say(`  ${String(await drive.evaluate(OPEN_BOARD))}`)
    await sleep(600)
    return drive.evaluate(NEEDS_YOU)
  })))
  check('the Board’s Needs you shows Scout’s card, with Dismiss', board.scout.length === 1 && /The check node check\.js fails/.test(board.scout[0].text) && board.scout[0].buttons.includes('Dismiss'), JSON.stringify(board.scout))
  check('and the routine, with Keep, Discard and Open the copy', board.routines.length === 1 && /Answer file/.test(board.routines[0].text) && ['Keep', 'Discard', 'Open the copy'].every((label) => board.routines[0].buttons.includes(label)), JSON.stringify(board.routines))
  await drive.resize(1120, 720)
  await drive.capture('the Board at 1120 by 720', async () => { await sleep(500); return drive.evaluate(NEEDS_YOU) })
  await drive.resize(860, 640)
  await drive.capture('the Board at 860 wide', async () => { await sleep(500); return drive.evaluate(NEEDS_YOU) })
  await drive.resize(1440, 900)

  // 2. The title bar counts them, and lists what each is.
  const menu = String(await drive.capture('the title bar’s list', async () => {
    await drive.evaluate(`document.querySelector('.lc-needsyou')?.click()`)
    await sleep(500)
    return drive.evaluate(`JSON.stringify({ chip: document.querySelector('.lc-needsyou')?.innerText.replace(/\\s+/g, ' ').trim() ?? '', menu: document.querySelector('[role="menu"]')?.innerText.replace(/\\s+/g, ' ').trim() ?? '' })`)
  }))
  const listed = JSON.parse(menu)
  check('the title bar counts both', /\b2 needs? you\b/.test(listed.chip), listed.chip)
  check('its list says Scout noticed, and the routine is waiting', /Scout noticed: The check node check\.js fails/.test(listed.menu) && /Answer file: Waiting for you/.test(listed.menu), listed.menu)
  await drive.evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)

  // 3. Keep, from the Board.
  await drive.evaluate(OPEN_BOARD)
  await drive.evaluate(`[...(document.querySelector('[data-board-routine]')?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Keep')?.click()`)
  let answer = ''
  for (let i = 0; i < 40 && answer !== '42'; i += 1) { await sleep(250); answer = (await readFile(join(workspace, 'answer.txt'), 'utf8').catch(() => '')).trim() }
  check('Keep on the Board writes the answer into the folder', answer === '42', answer || '(no answer.txt)')
  let after = JSON.parse(String(await drive.evaluate(NEEDS_YOU)))
  for (let i = 0; i < 20 && after.routines.length > 0; i += 1) { await sleep(250); after = JSON.parse(String(await drive.evaluate(NEEDS_YOU))) }
  check('and the routine leaves the column', after.routines.length === 0, JSON.stringify(after.routines))

  // 4. Dismiss, from the Board.
  await drive.evaluate(`[...(document.querySelector('[data-board-scout]')?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Dismiss')?.click()`)
  for (let i = 0; i < 20 && after.scout.length > 0; i += 1) { await sleep(250); after = JSON.parse(String(await drive.evaluate(NEEDS_YOU))) }
  check('Dismiss on the Board takes Scout’s card away', after.scout.length === 0 && !(await state()).cards.some((card) => /^The check node check\.js fails/.test(card.said)), JSON.stringify(after))
  await drive.capture('the Board, nothing waiting', async () => { await sleep(500); return drive.evaluate(NEEDS_YOU) })
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Scout on ${FREE_ROUTE.model} watching \`node check.js\`; a copy routine for Cedar writes answer.txt.` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
