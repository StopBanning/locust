// Scout keeps watch for what you ask, and keeps its hours (0.815, the first slice of the always-on plan).
//
//   node _tools/drive-scout-keeps-its-goal-and-hours.mjs [--packaged <exe>] [--tag <name>]
//
// Free models only: Scout reads with the free model, so nothing is spent. The folder is a git repository whose check
// is `node check.js`, failing until answer.txt says 42. Then:
//   1. Settings > General shows Scout's new rows: what to keep watch for, the folder's check (naming its command),
//      quiet hours, and waiting while you work -- at three window sizes.
//   2. A goal typed and saved is kept, and read into the next look's prompt (the look's own record says so).
//   3. With the folder's check on, Look now runs it: a card "The check node check.js fails: ..." waits on Home.
//   4. answer.txt fixed, Look now again: the card says the check passes again, and the failing card is gone.
//   5. Settings says what today's looks used, and quiet hours chosen are kept.

import { createHash } from 'node:crypto'
import { readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, git, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

const workspace = await scratchRepository('locust-drive-scout-goal-ws-')
await writeFile(join(workspace, 'check.js'), [
  "const said = require('fs').existsSync('answer.txt') ? require('fs').readFileSync('answer.txt', 'utf8').trim() : 'nothing'",
  "if (said !== '42') { console.log('CHECK FAILED: answer.txt must say 42, and it says ' + said); process.exit(1) }",
  "console.log('ok: answer.txt says 42')",
  ''
].join('\n'), 'utf8')
await writeFile(join(workspace, 'answer.txt'), '41\n', 'utf8')
await git(['add', '-A'], workspace)
await git(['commit', '-q', '-m', 'an answer and its check'], workspace)
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `scout-goal-${tag}`,
  port: 9873,
  workspace,
  outPath: join(recordRoot('scout-keeps-its-goal-and-hours-2026-10-22'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Docs & QA', createdAt: '2026-10-22T05:00:00.000Z', route: FREE_ROUTE }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, checkCommands: { [workspaceId]: 'node check.js' } }
  }
})

const state = async () => JSON.parse(String(await drive.evaluate('window.desktop.scout.state().then((s) => JSON.stringify(s))')))
const SECTION = `document.querySelector('[data-setting="scout"]')?.innerText.replace(/\\s+/g, ' ') ?? ''`
const OPEN_SETTINGS = `(async () => {
  document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
  for (let i = 0; i < 40 && !document.querySelector('.lc-settings'); i += 1) await new Promise((r) => setTimeout(r, 100))
  ;[...document.querySelectorAll('.lc-settings__navitem, button')].find((b) => b.innerText.trim() === 'General')?.click()
  for (let i = 0; i < 40 && !document.querySelector('[data-setting="scout"]'); i += 1) await new Promise((r) => setTimeout(r, 100))
  document.querySelector('[data-setting="scout"]')?.scrollIntoView({ block: 'start' })
  return document.querySelector('[data-setting="scout"]') ? 'open' : 'no Scout section'
})()`
const CLOSE_SETTINGS = `(() => { document.querySelector('button[title="Settings (Ctrl 3)"]')?.click(); return 'closed' })()`
const CARDS = `JSON.stringify([...document.querySelectorAll('[data-scout-card]')].map((card) => card.querySelector('.lc-scoutcard__said')?.innerText ?? ''))`
/** The newest look's prompt, from its own record in the profile's ledger. */
const lastLookPrompt = async () => {
  const ids = (await state()).lookIds
  const id = ids.at(-1)
  if (id === undefined) return ''
  const text = await readFile(join(drive.profile, 'mission-ledger', `${id}.jsonl`), 'utf8').catch(async () => {
    const files = (await readdir(join(drive.profile, 'mission-ledger'))).filter((file) => file.includes(id))
    return files[0] === undefined ? '' : readFile(join(drive.profile, 'mission-ledger', files[0]), 'utf8')
  })
  return /"prompt":"((?:[^"\\]|\\.)*)"/.exec(text)?.[1] ?? ''
}
const lookAndWait = async () => {
  const before = (await state()).lastLookAt
  await drive.evaluate('window.desktop.scout.look()')
  for (let i = 0; i < 120; i += 1) {
    const now = await state()
    if (now.lastLookAt !== before && !now.cards.some((card) => card.reading === true)) return now
    await sleep(500)
  }
  return state()
}

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const turned = JSON.parse(String(await drive.evaluate(`window.desktop.scout.turnOn(${JSON.stringify({ runtime: FREE_ROUTE.runtime, model: FREE_ROUTE.model })}).then((s) => JSON.stringify(s))`)))
  check('Scout is on, on the free model', turned?.settings?.on === true, JSON.stringify(turned).slice(0, 200))
  await lookAndWait()

  // 1. The new rows, at three sizes.
  const shown = String(await drive.capture('settings: what Scout keeps watch for, its check and its hours', async () => {
    say(`  ${String(await drive.evaluate(OPEN_SETTINGS))}`)
    return drive.evaluate(SECTION)
  }))
  check('the goal box is there, in the person’s words', shown.includes('What to keep watch for') && shown.includes('In your own words.'), shown.slice(0, 300))
  check('the folder’s check names its command', shown.includes('When the folder has changed, Scout runs node check.js'), shown)
  check('quiet hours and waiting while you work are there', shown.includes('Quiet hours') && shown.includes('Wait while you’re working'), shown)
  await drive.resize(1120, 720)
  await drive.capture('settings at 1120 by 720', () => drive.evaluate(`${OPEN_SETTINGS}.then(() => '')`).then(() => drive.evaluate(SECTION)))
  await drive.resize(860, 640)
  await drive.capture('settings at 860 wide', () => drive.evaluate(`${OPEN_SETTINGS}.then(() => '')`).then(() => drive.evaluate(SECTION)))
  await drive.resize(1440, 900)

  // 2. A goal typed and saved.
  const GOAL = 'Keep the answer check passing.'
  const saved = String(await drive.capture('a goal typed and saved', async () => {
    await drive.evaluate(`${OPEN_SETTINGS}.then(() => '')`)
    await drive.evaluate(`(() => {
      const box = document.querySelector('textarea[aria-label="What Scout keeps watch for"]')
      const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
      set.call(box, ${JSON.stringify(GOAL)})
      box.dispatchEvent(new Event('input', { bubbles: true }))
      return 'typed'
    })()`)
    await sleep(300)
    await drive.evaluate(`[...document.querySelectorAll('[data-setting="scout"] button')].find((b) => b.innerText.trim() === 'Save')?.click()`)
    for (let i = 0; i < 20 && (await state()).settings.goal !== GOAL; i += 1) await sleep(250)
    return drive.evaluate(SECTION)
  }))
  check('the goal is kept', (await state()).settings.goal === GOAL, JSON.stringify((await state()).settings))
  check('and Save has gone once it is saved', !saved.includes(' Save Cancel'), saved.slice(0, 400))

  // 3. The folder's check on: a look runs it, and the failure waits on Home.
  await drive.evaluate(`window.desktop.scout.set({ watches: { github: false, teammates: true, check: true } })`)
  const failed = await lookAndWait()
  check('the check ran and its failure is a card that needs you now', failed.cards.some((card) => card.said === 'The check node check.js fails: CHECK FAILED: answer.txt must say 42, and it says 41' && card.urgency === 'now'), JSON.stringify(failed.cards.map((card) => card.said)))
  const prompt = await lastLookPrompt()
  check('the look’s prompt carried the goal', prompt.includes(`The person asked you to keep watch for this: ${GOAL}`), prompt.slice(0, 300))
  await drive.evaluate(CLOSE_SETTINGS)
  await drive.capture('Home: the check fails', async () => { await sleep(500); return drive.evaluate(CARDS) })

  // 4. Fixed: the card turns.
  await writeFile(join(workspace, 'answer.txt'), '42\n', 'utf8')
  const passed = await lookAndWait()
  const checkCards = passed.cards.filter((card) => card.key.startsWith('check:')).map((card) => card.said)
  check('fixed, the check passes again, and the failing card has gone', checkCards.length === 1 && checkCards[0] === 'The check node check.js passes again.', JSON.stringify(checkCards))
  await drive.capture('Home: the check passes again', async () => { await sleep(500); return drive.evaluate(CARDS) })

  // 5. Today's count, and quiet hours kept.
  await drive.evaluate(`window.desktop.scout.set({ quiet: { from: 22, to: 7 } })`)
  const after = String(await drive.capture('settings: today’s looks and quiet hours', async () => {
    await drive.evaluate(`${OPEN_SETTINGS}.then(() => '')`)
    return drive.evaluate(SECTION)
  }))
  check('today’s looks are counted', /Today: \d+ looks?, /.test(after), after.slice(0, 500))
  check('quiet hours are kept', JSON.stringify((await state()).settings.quiet) === '{"from":22,"to":7}' && after.includes('10 PM to 7 AM'), JSON.stringify((await state()).settings.quiet))
} catch (error) {
  failures += 1
  say(`  [FAIL] the drive stopped: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Scout on ${FREE_ROUTE.model}; the folder's check is \`node check.js\`, failing until answer.txt says 42.` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
