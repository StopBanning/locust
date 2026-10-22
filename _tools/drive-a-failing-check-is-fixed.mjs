// FIX UNTIL THE CHECK PASSES (0.812): a failing check goes back to the teammate, and the loop ends when it passes.
//
//   node _tools/drive-a-failing-check-is-fixed.mjs [--packaged <exe>] [--tag <name>]
//
// The folder's check (set as this folder's command, as a person does in Settings) runs math.js and report.js. The
// person asks Cedar to change sum() to take two numbers and to change only math.js for now -- which breaks
// report.js, its caller. With "When it fails: Send it back to fix, at most 2" chosen in Settings, the failure goes
// back to Cedar as Locust's turn, never drawn as the person's; the fix changes report.js, the check passes, and the
// loop ends there. Reloaded, the conversation still says which turn was Locust's. A free OpenCode teammate, in Edit:
// spends nothing.

import { createHash } from 'node:crypto'
import { execFile } from 'node:child_process'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, git, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const MODEL = FREE_ROUTE.model

const CHECK = [
  "const failures = []",
  "let pair",
  "try { pair = require('./math.js').sum(2, 3) } catch (error) { pair = 'an error: ' + error.message }",
  "if (pair !== 5) failures.push('FAIL sum(2, 3): expected 5, got ' + String(pair))",
  "let total",
  "try { total = require('./report.js').total([{ amount: 2 }, { amount: 3 }, { amount: 5 }]) } catch (error) { total = 'an error: ' + error.message }",
  "if (total !== 10) failures.push('FAIL report total: expected 10, got ' + String(total))",
  "if (failures.length > 0) { console.log(failures.join('\\n')); process.exit(1) }",
  "console.log('ok: 2 checks passed')",
  ''
].join('\n')

const workspace = await scratchRepository('locust-drive-checkfix-ws-')
await writeFile(join(workspace, 'math.js'), 'exports.sum = (values) => values.reduce((total, value) => total + value, 0)\n', 'utf8')
await writeFile(join(workspace, 'report.js'), "const { sum } = require('./math.js')\n\nexports.total = (orders) => sum(orders.map((order) => order.amount))\n", 'utf8')
await writeFile(join(workspace, 'check.js'), CHECK, 'utf8')
await git(['add', '-A'], workspace)
await git(['commit', '-q', '-m', 'math, report and their check'], workspace)

const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-checkfix-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const CEDAR = { teammateId: 'tm_cedar', name: 'Cedar', hue: 'lime', role: 'Custom', roleTitle: 'Numbers', createdAt: '2026-10-21T05:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'accept-edits' } }
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `check-fix-${tag}`,
  port: 9861,
  workspace,
  profilePath,
  outPath: join(recordRoot('a-failing-check-is-fixed-2026-10-21'), tag),
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
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const run = (file, args, cwd) => new Promise((resolve) => {
  execFile(file, args, { cwd, windowsHide: true }, (error, stdout, stderr) => resolve({ code: error === null ? 0 : (error.code ?? 1), out: `${stdout}${stderr}`.trim() }))
})
const checkBefore = await readFile(join(workspace, 'check.js'), 'utf8')

const OPEN_FOLDER_SETTINGS = `(async () => {
  document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
  await new Promise((r) => setTimeout(r, 900))
  ;[...document.querySelectorAll('.lc-settings__navitem, button')].find((b) => b.innerText.trim() === 'Project folder')?.click()
  await new Promise((r) => setTimeout(r, 900))
  const row = document.querySelector('input[aria-label="Check after edits"]')?.closest('.lc-policyrow')
  row?.scrollIntoView({ block: 'center' })
  return row === undefined || row === null ? 'NO CHECK AFTER EDITS ROW' : 'open'
})()`
const ROW = `(() => {
  const row = document.querySelector('input[aria-label="Check after edits"]')?.closest('.lc-policyrow')
  if (!row) return 'NO ROW'
  const on = [...row.querySelectorAll('[role=radio][aria-checked=true]')].map((b) => b.getAttribute('aria-label') ?? b.innerText.trim())
  return JSON.stringify({ command: row.querySelector('input').value, on, text: row.innerText.replace(/\\s+/g, ' ').trim() })
})()`
const CLOSE_SETTINGS = `(async () => {
  document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
  await new Promise((r) => setTimeout(r, 700))
  return document.querySelector('.lc-settings') ? 'still open' : 'closed'
})()`
// The thread's text, and how many of its bubbles there are: only the person's message is one.
const THREAD = `(() => JSON.stringify({
  bubbles: document.querySelectorAll('.lc-thread .lc-bubble').length,
  lines: [...document.querySelectorAll('.lc-thread .lc-checkfixline')].map((line) => line.innerText.replace(/\\s+/g, ' ').trim()),
  cards: [...document.querySelectorAll('.lc-thread .lc-editcheck')].map((card) => card.innerText.replace(/\\s+/g, ' ').trim()),
  text: document.querySelector('.lc-thread')?.innerText.replace(/\\s+/g, ' ').trim().slice(-1600) ?? ''
}))()`
// Until no run is going, approving a command it asks for once (this drive's own fixture).
const SETTLE = (seconds) => `(async () => {
  for (let i = 0; i < ${String(seconds * 2)}; i += 1) {
    await new Promise((r) => setTimeout(r, 500))
    const approve = [...document.querySelectorAll('[role=group][aria-label="Approval required"] button')].find((b) => /^Approve once/.test(b.innerText.trim()))
    if (approve) { approve.click(); await new Promise((r) => setTimeout(r, 900)); continue }
    if (i > 4 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'settled after ' + String(i / 2) + 's'
  }
  return 'still running after ${String(seconds)}s'
})()`

try {
  await drive.ready()
  await drive.resize(1209, 770)

  const off = String(await drive.capture('settings: the check, and what a failure does (off)', async () => {
    await drive.evaluate(OPEN_FOLDER_SETTINGS)
    return drive.evaluate(ROW)
  }))
  check('the row starts on Show me, with the seeded check', /"command":"node check\.js"/.test(off) && /"on":\["Show me"\]/.test(off), off)

  const on = String(await drive.capture('settings: send it back to fix, at most 2', async () => {
    await drive.evaluate(`(async () => {
      const row = document.querySelector('input[aria-label="Check after edits"]')?.closest('.lc-policyrow')
      ;[...(row?.querySelectorAll('[role=radio]') ?? [])].find((b) => b.innerText.trim() === 'Send it back to fix')?.click()
      await new Promise((r) => setTimeout(r, 1200))
    })()`)
    return drive.evaluate(ROW)
  }))
  check('choosing it shows the most, 2 by default, and says each fix is a turn', /"on":\["Send it back to fix","2 fixes"\]/.test(on) && /2 times at most/.test(on) && /Each fix is one more turn/.test(on), on)

  await drive.resize(860, 640)
  await drive.capture('settings at 860 wide', () => drive.evaluate(ROW))
  await drive.resize(1209, 770)

  const stored = JSON.parse(await readFile(join(drive.profile, 'teammates.json'), 'utf8'))
  check('it is kept for this folder only, on disk', stored.settings?.checkFixes?.[workspaceId] === 2 && Object.keys(stored.settings?.checkFixes ?? {}).length === 1, JSON.stringify(stored.settings?.checkFixes))
  await drive.evaluate(CLOSE_SETTINGS)
  await drive.evaluate(OPEN_FOLDER_SETTINGS)
  const reread = String(await drive.evaluate(ROW))
  check('and read back when Settings opens again', /"on":\["Send it back to fix","2 fixes"\]/.test(reread), reread)
  await drive.evaluate(CLOSE_SETTINGS)

  const asked = String(await drive.capture('ask Cedar for a change that breaks its caller', async () => {
    const opened = await drive.evaluate(openTeammateScript('Cedar'))
    const route = await drive.evaluate(`document.querySelector('.lc-composer, form.command-dock')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? ''`)
    if (!/deepseek/i.test(String(route))) throw new Error(`not on the free route before sending: ${String(route)}`)
    const sent = await drive.evaluate(sendAndWaitScript('In math.js, change sum so it takes two numbers, sum(a, b), and returns a + b. Change only math.js for now, and do not run any commands.', { settle: false }))
    return `${String(opened)} | ${String(sent)} | ${String(await drive.evaluate(SETTLE(360)))}`
  }))
  check('the person\'s turn ran and ended', /settled after/.test(asked), asked)

  const sentBack = String(await drive.capture('the failure goes back to Cedar as Locust\'s turn', async () => {
    const seen = await drive.evaluate(`(async () => {
      for (let i = 0; i < 120; i += 1) {
        if (document.querySelector('.lc-thread .lc-checkfixline')) return 'line after ' + String(i / 2) + 's'
        await new Promise((r) => setTimeout(r, 500))
      }
      return 'NO FIX LINE'
    })()`)
    return `${String(seen)} || ${String(await drive.evaluate(THREAD))}`
  }))
  check('a fix line appeared, naming the check, the teammate and the fix', /line after/.test(sentBack) && /node check\.js failed, so Locust sent it back to Cedar · fix 1 of 2/.test(sentBack), sentBack)

  const ended = String(await drive.capture('the fix ends and the check passes', async () => {
    const settled = await drive.evaluate(SETTLE(420))
    const card = await drive.evaluate(`(async () => {
      for (let i = 0; i < 240; i += 1) {
        const text = [...document.querySelectorAll('.lc-thread .lc-editcheck')].map((c) => c.innerText.replace(/\\s+/g, ' ').trim()).join(' ;; ')
        if (/passed after|fails|failed|could not|ran past/.test(text) && !/Running the check/.test(text)) return text
        await new Promise((r) => setTimeout(r, 500))
      }
      return 'NO SETTLED CHECK CARD'
    })()`)
    return `${String(settled)} || ${String(card)} || ${String(await drive.evaluate(THREAD))}`
  }))
  check('the check passed after fix 1, and nothing more was sent', /The check node check\.js passed after fix 1\./.test(ended) && !/fix 2 of 2/.test(ended), ended)
  const shown = JSON.parse(String(await drive.evaluate(THREAD)))
  check('only the person\'s message is a bubble; Locust\'s turn is a line', shown.bubbles === 1 && shown.lines.length === 1, JSON.stringify({ bubbles: shown.bubbles, lines: shown.lines }))

  const told = String(await drive.capture('what Locust sent, opened', () => drive.evaluate(`(async () => {
    const details = document.querySelector('.lc-thread .lc-checkfixline details')
    if (!details) return 'NO DETAILS'
    details.querySelector('summary').click()
    await new Promise((r) => setTimeout(r, 500))
    details.scrollIntoView({ block: 'center' })
    return details.innerText.replace(/\\s+/g, ' ').trim()
  })()`)))
  check('it carries what the check printed, and the rule against weakening tests', /FAIL report total: expected 10/.test(told) && /Do not weaken, skip or delete tests/.test(told), told)

  for (const [width, height] of [[1440, 900], [860, 640]]) {
    await drive.resize(width, height)
    await drive.capture(`the conversation at ${String(width)} wide`, () => drive.evaluate(THREAD))
  }
  await drive.resize(1209, 770)

  const checkAfter = await readFile(join(workspace, 'check.js'), 'utf8')
  check('the check itself was not touched', checkAfter === checkBefore)
  const now = await run('node', ['check.js'], workspace)
  check('the check passes in the folder now', now.code === 0 && /ok: 2 checks passed/.test(now.out), now.out)
  const report = await readFile(join(workspace, 'report.js'), 'utf8')
  const diff = await run('git', ['diff', '--stat'], workspace)
  check('the fix changed report.js, the caller the change broke', report !== "const { sum } = require('./math.js')\n\nexports.total = (orders) => sum(orders.map((order) => order.amount))\n", diff.out)

  const reopened = String(await drive.capture('reloaded: the conversation still says which turn was Locust\'s', async () => {
    await drive.evaluate('location.reload()')
    await sleep(4000)
    await drive.ready()
    await drive.evaluate(`(async () => {
      ;[...document.querySelectorAll('.lc-convrow button.lc-conv, .lc-conv')][0]?.click()
      await new Promise((r) => setTimeout(r, 2000))
    })()`)
    return drive.evaluate(THREAD)
  }))
  const after = JSON.parse(reopened)
  check('after a reload: one bubble, the fix line, and no prompt drawn as the person\'s', after.bubbles === 1 && after.lines.length === 1 && !/failed after your last turn/.test(after.text.replace(/What Locust sent.*$/, '')), reopened)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Cedar on ${MODEL}, in Edit. The folder's check is \`node check.js\`; Settings is set to send a failure back to fix, at most 2. The ask changes math.js's sum() and breaks report.js, its caller.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
