// A routine keeps going until the folder's check passes (0.534, a standing goal).
//
//   node _tools/drive-a-routine-until-its-check-passes.mjs [--packaged <exe>] [--tag <name>]
//
// The project's check, set as this folder's command, passes only when
// answer.txt says 42. The routine's step asks for 41 -- so its first check
// fails, the same teammate is asked to fix what the check printed, and the
// check runs again. It works in a copy, so the passing answer then waits on
// the card for Keep, and the folder is untouched until then. A free OpenCode
// teammate (Fledge Alpha), in Edit: spends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const MODEL = FREE_ROUTE.model
const workspace = await scratchRepository('locust-drive-goal-ws-')
await writeFile(join(workspace, 'check.js'), [
  "const { readFileSync } = require('node:fs')",
  "let said = ''",
  "try { said = readFileSync('answer.txt', 'utf8').trim() } catch { said = '(no answer.txt)' }",
  "if (said !== '42') { console.log('CHECK FAILED: answer.txt must say exactly 42, and it says ' + said); process.exit(1) }",
  "console.log('CHECK PASSED')",
  ''
].join('\n'), 'utf8')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-goal-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const CEDAR = { teammateId: 'tm_cedar', name: 'Cedar', hue: 'lime', role: 'Custom', roleTitle: 'Numbers', createdAt: '2026-10-01T05:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'accept-edits' } }
await writeFile(join(profilePath, 'routines.json'), JSON.stringify({
  schemaVersion: 1,
  routines: [{
    routineId: 'rt_goal0000000000000000000',
    name: 'Answer file',
    teammateId: CEDAR.teammateId,
    route: CEDAR.route,
    steps: ['Create a file named answer.txt in the folder you are working in, holding exactly the number 41 and nothing else. Do not run any check yourself. Reply with the word WRITTEN.'],
    learnedFrom: [],
    createdAt: new Date(Date.now() - 3_600_000).toISOString(),
    runs: 0,
    workspaceId,
    inCopy: true,
    untilCheck: { tries: 2 }
  }]
}), 'utf8')
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `routine-goal-${tag}`,
  port: 9856,
  workspace,
  profilePath,
  outPath: join(recordRoot('a-routine-until-its-check-passes-2026-10-01'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [CEDAR],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, checkCommands: { [workspaceId]: 'node check.js' } }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const row = `[...document.querySelectorAll('.lc-routinerow:not(.lc-routineadd)')].find((r) => /Answer file/.test(r.innerText))`
try {
  await drive.ready()
  await drive.resize(1209, 770)
  const dialog = String(await drive.capture('the routine, opened: its goal', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1200))
    ;(${row})?.querySelector('button[aria-label^="Edit"]')?.click()
    await new Promise((r) => setTimeout(r, 800))
    const text = document.querySelector('[role=dialog]')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no dialog'
    ;[...(document.querySelector('[role=dialog]')?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Cancel')?.click()
    await new Promise((r) => setTimeout(r, 400))
    return text
  })()`)))
  check('the dialog names the check and the fixes it may make', /When its steps are done it runs `node check\.js`\. While that fails, it is asked to fix what it says, up to 2 times/.test(dialog), dialog)
  const ran = String(await drive.capture('run, then the goal: check, fix, check', () => drive.evaluate(`(async () => {
    ;[...((${row})?.querySelectorAll('button') ?? [])].find((b) => b.innerText.trim() === 'Run')?.click()
    for (let i = 0; i < 900; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      ;[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()
      const now = ${row}
      if (now?.querySelector('.lc-routinechanges')) return 'waiting: ' + now.querySelector('.lc-routinechanges').innerText.replace(/\\s+/g, ' ').trim()
      if (now?.querySelector('.lc-recovery')) return 'held: ' + now.querySelector('.lc-recovery').innerText.replace(/\\s+/g, ' ').trim()
    }
    return 'never finished'
  })()`)))
  // The fix turn is Locust's line (0.813, CheckFixLine), never a message from the person; what it sent is opened to read.
  const thread = String(await drive.capture('the conversation: the step, the failed check, the fix', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')][0]?.click()
    await new Promise((r) => setTimeout(r, 1500))
    const sent = document.querySelector('.lc-thread .lc-checkfixline details')
    if (sent) { sent.querySelector('summary').click(); await new Promise((r) => setTimeout(r, 400)) }
    return JSON.stringify({
      bubbles: [...document.querySelectorAll('.lc-thread .lc-bubble')].map((bubble) => bubble.innerText.replace(/\\s+/g, ' ').trim()),
      text: document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''
    })
  })()`)))
  const shown = JSON.parse(thread)
  check('the first check failed and the same teammate was asked to fix what it printed, as Locust’s line', /node check\.js failed, so Locust sent it back to Cedar · fix 1 of 2/.test(shown.text) && /The check `node check\.js` failed after the work above \(fix 1 of 2\)/.test(shown.text) && /answer\.txt must say exactly 42, and it says 41/.test(shown.text), shown.text.slice(-900))
  check('the fix is never drawn as the person’s message', !shown.bubbles.some((bubble) => /failed after the work above/.test(bubble)), JSON.stringify(shown.bubbles))
  check('it passed after the fix, and the conversation says so', /finished: the check `node check\.js` passed after 1 fix/.test(shown.text), shown.text.slice(-500))
  check('the passing answer waits on the card, in its copy', /^waiting: /.test(ran) && /answer\.txt/.test(ran), ran)
  check('and the folder has no answer.txt until Keep', !(await stat(join(workspace, 'answer.txt')).then(() => true, () => false)))
  const kept = String(await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1000))
    ;[...((${row})?.querySelectorAll('.lc-routinechanges button') ?? [])].find((b) => b.innerText.trim() === 'Keep')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return document.querySelector('.lc-claim')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''
  })()`))
  const answer = (await readFile(join(workspace, 'answer.txt'), 'utf8').catch(() => '')).trim()
  check('Keep writes the passing answer into the folder', answer === '42', `${kept} | answer.txt: ${answer || '(none)'}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A routine for Cedar on ${MODEL}, in a copy, until \`node check.js\` passes (2 fixes at most); its step writes the wrong answer.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
