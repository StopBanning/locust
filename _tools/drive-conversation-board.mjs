// The board: every conversation by what it needs from you (0.585).
//
//   node _tools/drive-conversation-board.mjs [--packaged <exe>] [--tag <name>]
//
// Free OpenCode, in Approve each; spends nothing. One seeded finished
// conversation, then one live turn walked round the whole board:
//   1. Board, quiet: "all quiet" and the seeded conversation under Done.
//   2. Wren is asked to write a file: the card arrives; the Board shows the
//      conversation under NEEDS YOU with what it is waiting for.
//   3. The card opens from its Board card; approved once; back on the Board
//      it is under WORKING.
//   4. The run ends while the Board is on screen: it moves to READY TO LOOK
//      AT, and opening it takes it out again.
//   5. (0.835) Wren is asked to run a command: the Board shows the command
//      whole with Allow once and Deny, and Allow once on the Board answers it.
// Captured at 1440x900, and the Needs-you board again at 1120x720.

import { mkdir, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('conversation-board-2026-10-04'), `conversation-board-${tag}`)
await mkdir(OUT, { recursive: true })

const root = new URL('..', import.meta.url).pathname.slice(1)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-board-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-board-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z', route: { ...FREE_ROUTE, mode: 'approve-each' } }
// Atlas owns yesterday's finished conversation, so Done has something to list and Wren's
// composer starts a NEW conversation (opening Wren's latest would make the task a follow-up turn on it).
const ATLAS = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa2', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: '2026-08-10T09:00:00.000Z', route: { ...FREE_ROUTE, mode: 'ask' } }
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const seededId = 'mission_10000000-0000-4000-8000-000000000001'
const at = new Date(Date.now() - 20 * 3600_000).toISOString()
await ledger.createMission({
  missionId: seededId, runId: 'run_100001', prompt: 'Summarise yesterday in one line.',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
let tick = 0
const normalizer = adapters.createCodexEventNormalizer({ runId: 'run_100001', missionId: seededId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
await ledger.appendEvents(seededId, [
  ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_board' }) }),
  ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Yesterday: the invoices moved and the tests passed.' } }) }),
  ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 300, cached_input_tokens: 0, output_tokens: 20 } }) }),
  ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
])

const drive = await startDrive({
  name: `conversation-board-${tag}`, port: 9795, workspace, profilePath, outPath: OUT, ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN, ATLAS], missionOwners: { [seededId]: ATLAS.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
// Board lives in the sidebar's foot beside Settings since 0.605, not among its places.
const OPEN_BOARD = `(() => { const b = [...document.querySelectorAll('.lc-sidebar__nav button, .lc-sidebar__places button')].find((el) => el.innerText.trim() === 'Board'); if (!b) return 'no Board button'; b.click(); return 'opened' })()`
const BOARD = `JSON.stringify((() => {
  const board = document.querySelector('.lc-board')
  if (!board) return { shown: false }
  const sections = [...board.querySelectorAll('.lc-boardsection')].map((section) => ({
    title: section.getAttribute('aria-label'),
    cards: [...section.querySelectorAll('.lc-convcard, .lc-boardwait--ask')].map((card) => ({ title: card.querySelector('.lc-convcard__title')?.textContent ?? '', state: card.querySelector('.lc-convcard__state')?.textContent ?? '', who: card.querySelector('.lc-convcard__who')?.textContent ?? '', face: card.querySelector('.lc-bot') !== null, ask: card.dataset.boardAsk === undefined ? undefined : { command: card.querySelector('.lc-boardwait__command')?.textContent ?? '', where: card.querySelector('.lc-boardwait__where')?.textContent ?? '', acts: [...card.querySelectorAll('.lc-boardwait__acts button')].map((b) => b.textContent.trim()) } }))
  }))
  const overflow = [...board.querySelectorAll('.lc-convcard, .lc-boardwait')].some((card) => card.scrollWidth > card.clientWidth + 1)
  return { shown: true, meta: board.querySelector('.lc-screen__meta')?.textContent ?? '', note: board.querySelector('.lc-screen__note')?.textContent.slice(0, 60) ?? '', sections, overflow }
})())`
const running = async () => String(await drive.evaluate(`String(document.querySelector('button[aria-label^="Stop the running"]') !== null)`)) === 'true'
const cardShown = async () => String(await drive.evaluate(`String(document.querySelector('.lc-approval__actions') !== null)`)) === 'true'
const column = (board, title) => board.sections?.find((section) => section.title === title)

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2500)
  // 1. Quiet.
  check('the sidebar has a Board button', String(await drive.evaluate(OPEN_BOARD)) === 'opened')
  await sleep(600)
  const quiet = JSON.parse(String(await drive.capture('the board, quiet', () => drive.evaluate(BOARD))))
  check('quiet: it says so, and lists the finished conversation under Done', quiet.shown && /all quiet/.test(quiet.meta) && column(quiet, 'Done')?.cards.some((card) => /Summarise yesterday/.test(card.title)), JSON.stringify(quiet).slice(0, 240))
  check('quiet: the Done card wears its owner', column(quiet, 'Done')?.cards[0]?.face === true && column(quiet, 'Done')?.cards[0]?.who === 'Atlas', JSON.stringify(column(quiet, 'Done')?.cards[0]))

  // 2. A card: Needs you. The Board has no composer: the teammate's conversation is opened first.
  await drive.evaluate(openTeammateScript('Wren'))
  await sleep(800)
  check('the composer is on screen before sending', String(await drive.evaluate(`String(document.querySelector('form.command-dock textarea') !== null)`)) === 'true')
  const sent = String(await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Create a file hello.txt containing the single line: hi. Change nothing else. Then reply with the word DONE.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); return 'sent' }
    }
    return 'not sent'
  })()`))
  check('the task is sent', sent === 'sent', sent)
  await sleep(4000)
  await drive.capture('after sending', () => drive.evaluate(`JSON.stringify({ running: document.querySelector('button[aria-label^="Stop the running"]') !== null, rows: [...document.querySelectorAll('button.lc-conv')].map((r) => r.innerText.replace(/\\s+/g, ' ').slice(0, 40)), thread: (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 300) })`))
  let card = false
  for (let i = 0; i < 240 && !card; i += 1) {
    await sleep(500)
    card = await cardShown()
    if (!card && i > 20 && !(await running())) break
  }
  check('a card arrives', card)
  await drive.evaluate(OPEN_BOARD)
  await sleep(800)
  const needs = JSON.parse(String(await drive.capture('the board: needs you', () => drive.evaluate(BOARD))))
  const waiting = column(needs, 'Needs you')?.cards[0]
  check('NEEDS YOU lists the conversation, with what it is waiting for', waiting !== undefined && /Create a file/.test(waiting.title) && waiting.state.length > 0 && waiting.state !== 'working', JSON.stringify(waiting))
  check('the header counts it', /1 waiting on you/.test(needs.meta), needs.meta)
  check('no card overflows its column', needs.overflow === false)
  await drive.resize(1120, 720)
  await sleep(700)
  const small = JSON.parse(String(await drive.capture('the board at 1120x720', () => drive.evaluate(BOARD))))
  check('1120x720: the same board, nothing overflowing', small.shown && column(small, 'Needs you') !== undefined && small.overflow === false, JSON.stringify({ sections: small.sections?.map((s) => s.title), overflow: small.overflow }))
  await drive.resize(1440, 900)
  await sleep(500)

  // 3. Open from the board, approve, back: Working.
  const opened = String(await drive.evaluate(`(() => { const c = document.querySelector('.lc-board .lc-boardsection[data-column="needs-you"] :is(.lc-convcard, .lc-boardwait--ask .lc-boardwait__open)'); if (!c) return 'no card'; c.click(); return 'clicked' })()`))
  await sleep(900)
  check('its Board card opens the conversation, on its card', opened === 'clicked' && (await cardShown()) && String(await drive.evaluate(`String(document.querySelector('.lc-board') === null)`)) === 'true')
  await drive.evaluate(`[...document.querySelectorAll('.lc-approval__actions button')].find((b) => /^Approve once$|^Allow once$/.test(b.innerText.trim()))?.click()`)
  await sleep(1200)
  await drive.evaluate(OPEN_BOARD)
  await sleep(800)
  const working = JSON.parse(String(await drive.capture('the board: working', () => drive.evaluate(BOARD))))
  // The Board has no Stop button: the run is still going while the Board itself lists it under Working.
  const listedUnder = (board, title) => column(board, title)?.cards.some((c) => /Create a file/.test(c.title)) === true
  // Still going = under Needs you (the card just answered, clearing) or Working.
  const stillGoing = (board) => listedUnder(board, 'Needs you') || listedUnder(board, 'Working')
  if (!stillGoing(working)) say('  (the run had already ended before the Board was reopened: Working could not be seen)')
  else check('WORKING (or the clearing card) lists it once approved', true, JSON.stringify(working.sections?.map((s) => [s.title, s.cards.map((c) => c.state)])))

  // 4. It ends while the Board is on screen: Ready to look at; opening it clears that.
  // A free model sometimes asks a second card (a listing before the write): answer it from the
  // Board the way a person would -- open the card, Approve once, come back -- and keep waiting.
  for (let i = 0; i < 360; i += 1) {
    const now = JSON.parse(String(await drive.evaluate(BOARD)))
    if (!stillGoing(now)) break
    if (listedUnder(now, 'Needs you')) {
      await drive.evaluate(`document.querySelector('.lc-board .lc-boardsection[data-column="needs-you"] :is(.lc-convcard, .lc-boardwait--ask .lc-boardwait__open)')?.click()`)
      await sleep(900)
      await drive.evaluate(`[...document.querySelectorAll('.lc-approval__actions button')].find((b) => /^Approve once$|^Allow once$/.test(b.innerText.trim()))?.click()`)
      await sleep(700)
      await drive.evaluate(OPEN_BOARD)
      await sleep(600)
      continue
    }
    await sleep(500)
  }
  await sleep(1500)
  const ready = JSON.parse(String(await drive.capture('the board: ready to look at', () => drive.evaluate(BOARD))))
  check('READY TO LOOK AT lists the finish the person was not looking at', column(ready, 'Ready to look at')?.cards.some((c) => /Create a file/.test(c.title)) === true, JSON.stringify(ready.sections?.map((s) => [s.title, s.cards.map((c) => c.title.slice(0, 20))])))
  await drive.evaluate(`document.querySelector('.lc-board .lc-boardsection[data-column="to-look-at"] .lc-convcard')?.click()`)
  await sleep(1200)
  await drive.evaluate(OPEN_BOARD)
  await sleep(800)
  const after = JSON.parse(String(await drive.capture('the board, after looking', () => drive.evaluate(BOARD))))
  check('looked at: it leaves Ready to look at', column(after, 'Ready to look at') === undefined, JSON.stringify(after.sections?.map((s) => s.title)))

  // 5. A command, answered on the Board (0.835).
  await drive.evaluate(openTeammateScript('Wren'))
  await sleep(800)
  const asked = String(await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Run exactly this shell command once: git status --short. Then reply with the word LISTED and nothing else.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); return 'sent' }
    }
    return 'not sent'
  })()`))
  check('the command task is sent', asked === 'sent', asked)
  let asking
  for (let i = 0; i < 240 && asking === undefined; i += 1) {
    await sleep(500)
    if (await cardShown()) {
      await drive.evaluate(OPEN_BOARD)
      await sleep(800)
      asking = column(JSON.parse(String(await drive.evaluate(BOARD))), 'Needs you')?.cards.find((c) => c.ask !== undefined)
      if (asking === undefined) { await drive.evaluate(openTeammateScript('Wren')); await sleep(500) }
    }
  }
  await drive.capture('the board: a command asked, answered here', () => drive.evaluate(BOARD))
  check('the Board shows the command whole, where it runs, with Allow once and Deny', asking !== undefined && asking.ask.command.length > 0 && /^in /.test(asking.ask.where) && asking.ask.acts.includes('Allow once') && asking.ask.acts.includes('Deny') && !asking.ask.acts.some((a) => /Always/.test(a)), JSON.stringify(asking))
  // The command may come in the conversation already open, under that conversation's own title.
  const askedTitle = asking?.title
  let answered = 0
  for (let i = 0; i < 480; i += 1) {
    const now = JSON.parse(String(await drive.evaluate(BOARD)))
    const ask = column(now, 'Needs you')?.cards.find((c) => c.ask !== undefined && c.ask.acts.includes('Allow once'))
    if (ask !== undefined) {
      await drive.evaluate(`[...document.querySelectorAll('.lc-board .lc-boardwait--ask .lc-boardwait__acts button')].find((b) => b.textContent.trim() === 'Allow once')?.click()`)
      answered += 1
      await sleep(1500)
      continue
    }
    const other = column(now, 'Needs you')?.cards.find((c) => c.title === askedTitle)
    if (other !== undefined) {
      // A request the Board does not answer in place: the conversation's own card does.
      await drive.evaluate(`document.querySelector('.lc-board .lc-boardsection[data-column="needs-you"] :is(.lc-convcard, .lc-boardwait--ask .lc-boardwait__open)')?.click()`)
      await sleep(900)
      await drive.evaluate(`[...document.querySelectorAll('.lc-approval__actions button')].find((b) => /^Approve once$|^Allow once$/.test(b.innerText.trim()))?.click()`)
      await sleep(700)
      await drive.evaluate(OPEN_BOARD)
      await sleep(600)
      continue
    }
    if (column(now, 'Ready to look at')?.cards.some((c) => c.title === askedTitle) || column(now, 'Done')?.cards.some((c) => c.title === askedTitle)) break
    await sleep(500)
  }
  const done = JSON.parse(String(await drive.capture('the board: the command answered from it', () => drive.evaluate(BOARD))))
  check('Allow once on the Board answers it, and the turn goes on to finish', answered > 0 && (column(done, 'Ready to look at')?.cards.some((c) => c.title === askedTitle) || column(done, 'Done')?.cards.some((c) => c.title === askedTitle)) === true, JSON.stringify({ answered, sections: done.sections?.map((s) => [s.title, s.cards.map((c) => c.title.slice(0, 24))]) }))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. The Board: one seeded conversation, one free turn in Approve each walked through Needs you, Working, Ready to look at.`, extra: `Checks failed: ${String(failures)}` })
  say(failures === 0 ? 'PASSED' : `FAILED (${String(failures)})`)
  process.exitCode = failures === 0 ? 0 : 1
}
