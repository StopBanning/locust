// OpenCode 2 runs (2026-10-20): with OpenCode 2 first on PATH, an OpenCode teammate answers, edits, continues its own
// conversation and asks before acting in Approve each -- over `opencode acp`, which is how Locust runs it.
//
//   LOCUST_DRIVE_PATH_FIRST=<folder holding OpenCode 2's opencode.exe> [LOCUST_OPENCODE2_HOME=<its scratch home>] node _tools/drive-opencode-2-runs.mjs [--packaged <exe>]
//
// Free model only. OpenCode 2's own folders are a scratch home (XDG_*), never the person's: OpenCode 1 keeps its
// sessions in ~/.local/share/opencode, and OpenCode 2 is not to touch them from a drive.

import { existsSync, readFileSync, statSync } from 'node:fs'
import { mkdtemp } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sendAndWaitScript, sleep, startDrive } from './drive-lib.mjs'

if (process.env.LOCUST_DRIVE_PATH_FIRST === undefined) {
  say('needs LOCUST_DRIVE_PATH_FIRST: the folder holding OpenCode 2')
  process.exit(2)
}
const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
/*
 * OpenCode 2 runs ONE background service per machine, on a fixed port (127.0.0.1:45278), and its commands talk to it.
 * MEASURED 2026-10-20: a second home's `opencode models` failed, "Managed service port 45278 ... is already in use",
 * and the window listed no OpenCode models. So the home is the one whose service is running, when one is named
 * (LOCUST_OPENCODE2_HOME); a fresh one otherwise.
 */
const home = process.env.LOCUST_OPENCODE2_HOME ?? await mkdtemp(join(tmpdir(), 'locust-opencode-2-home-'))
const xdg = { XDG_DATA_HOME: join(home, 'data'), XDG_CONFIG_HOME: join(home, 'config'), XDG_STATE_HOME: join(home, 'state'), XDG_CACHE_HOME: join(home, 'cache') }
// OpenCode 1's own database: what the person's sessions are in. Read before and after, never opened.
const theirs = join(homedir(), '.local', 'share', 'opencode', 'opencode.db')
const theirsBefore = existsSync(theirs) ? statSync(theirs).mtimeMs : undefined
const workspace = await scratchRepository('locust-opencode-2-ws-')
// OpenCode 2's log, read only from where it stood when the drive began: a named home has a log of its own past.
const log = join(xdg.XDG_DATA_HOME, 'opencode', 'log', 'opencode.log')
const logFrom = existsSync(log) ? statSync(log).size : 0
const logSince = () => (existsSync(log) ? readFileSync(log).subarray(logFrom).toString('utf8').split('\n') : [])
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: 'opencode-2-runs',
  port: 9688,
  workspace,
  env: xdg,
  outPath: join(recordRoot('opencode-2-runs-2026-10-20'), packaged === undefined ? 'local' : 'packaged'),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-20T05:00:00.000Z', route: FREE_ROUTE }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const MODE = (name) => `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(b.innerText))
  if (!control) return 'no mode control'
  control.click(); await new Promise((r) => setTimeout(r, 400))
  const item = [...document.querySelectorAll('[role=menuitemradio]')].find((b) => b.innerText.trim().startsWith(${JSON.stringify(name)}))
  if (!item || item.disabled) { control.click(); return 'no ' + ${JSON.stringify(name)} }
  item.click(); await new Promise((r) => setTimeout(r, 400))
  return 'mode: ' + control.innerText.split(/\\s+/).join(' ').trim()
})()`
/** Send, decline every card the turn shows, and wait for its end. */
const declining = (text) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 120; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const button = document.querySelector('button[aria-label="Send"]')
    if (button && !button.disabled) { button.click(); break }
  }
  const cards = []
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    const approval = document.querySelector('[role=group][aria-label="Approval required"]')
    if (approval) {
      await new Promise((r) => setTimeout(r, 300))
      cards.push((approval.querySelector('.lc-approval__detail')?.innerText ?? approval.innerText).split(/\\s+/).join(' ').slice(0, 90))
      const deny = approval.querySelector('button.lc-denybutton')
      if (!deny) break
      deny.click()
      await new Promise((r) => setTimeout(r, 900))
      continue
    }
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) break
  }
  await new Promise((r) => setTimeout(r, 900))
  return JSON.stringify({ cards, thread: (document.querySelector('.lc-thread')?.innerText ?? '').split(/\\s+/).join(' ').slice(-300) })
})()`
const lastReply = `(() => { const replies = [...document.querySelectorAll('.lc-thread .lc-message--assistant, .lc-thread [data-role="assistant"]')]; return (replies.at(-1)?.innerText ?? document.querySelector('.lc-thread')?.innerText.slice(-400) ?? '').replace(/\\s+/g, ' ').trim() })()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  let row = ''
  for (let second = 0; second < 90 && !/Ready|not supported/i.test(row); second += 2) {
    await sleep(2000)
    row = String(await drive.evaluate(`([...document.querySelectorAll('.lc-runtimecell')].find((el) => /OpenCode/.test(el.querySelector('.lc-runtimecell__name')?.textContent ?? ''))?.innerText ?? '').replace(/\\s+/g, ' ').trim()`))
  }
  await drive.capture('Home, Connected accounts, with OpenCode 2 first on PATH', () => row)
  check('OpenCode 2 reads as ready, not as unsupported', /Ready/i.test(row) && !/not supported/i.test(row), row)

  say(`open Wren: ${String(await drive.evaluate(openTeammateScript('Wren')))}`)
  const edit = String(await drive.capture('Wren in Edit: write a file', () => drive.evaluate(sendAndWaitScript('Create a file named hello.txt in this folder containing exactly the word hi. Then reply with the single word: done.', { waitSeconds: 180 }))))
  const written = join(workspace, 'hello.txt')
  check('an Edit turn writes the file, in the workspace', existsSync(written) && /hi/.test(readFileSync(written, 'utf8')), edit)
  const reply = String(await drive.evaluate(lastReply))
  check('and its reply reaches the thread', /done/i.test(reply), reply)
  const thread = String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText ?? ''`))
  check('no step reads Used other', !thread.includes('Used other'), thread.slice(-500))
  check('a folder is not the command the footer says ran', !thread.includes(`ran ${workspace}`), thread.slice(-500))

  const follow = String(await drive.capture('a follow-up, in the same conversation', () => drive.evaluate(sendAndWaitScript('What is the name of the file you just created? Answer with the file name only.', { waitSeconds: 180 }))))
  check('a follow-up continues OpenCode 2’s own session', /hello\.txt/i.test(String(await drive.evaluate(lastReply))), follow)

  say(String(await drive.evaluate(MODE('Approve each'))))
  const asked = JSON.parse(String(await drive.capture('Approve each: a shell command, declined', () => drive.evaluate(declining('Run exactly this shell command with your shell tool: echo DENIED-RUN > denied.txt   If it is declined, do not try another way; say so in one sentence.')))))
  check('Approve each asks on a card before the command', asked.cards.length > 0, JSON.stringify(asked.cards))
  check('and the declined command did not run', !existsSync(join(workspace, 'denied.txt')), asked.thread)
  // OpenCode 2 ends a refused turn "cancelled": it reads as declined, never as a Stop the person did not press.
  check('and the turn reads as declined, not stopped', !/was stopped|unreported OpenCode command/i.test(asked.thread) && /declined/i.test(asked.thread), asked.thread)

  // Locust's command guard, as OpenCode 2's plugin folder: OpenCode's own log says it loaded it.
  const loaded = logSince().filter((line) => /loading plugin/.test(line))
  check('OpenCode 2 loaded Locust’s command guard', loaded.some((line) => /opencode-2-command-guard/.test(line)), loaded.join(' | ') || `no log at ${log}`)
  const warned = logSince().filter((line) => /plugin path must be a directory/.test(line))
  check('and was never handed OpenCode 1’s plugin file', warned.length === 0, warned.join(' | '))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. OpenCode 2 first on PATH (${process.env.LOCUST_DRIVE_PATH_FIRST}), its folders in a scratch home; Wren on ${FREE_ROUTE.model}.` })
}
const theirsAfter = existsSync(theirs) ? statSync(theirs).mtimeMs : undefined
check('OpenCode 1’s own sessions were not touched', theirsAfter === theirsBefore, `${String(theirsBefore)} -> ${String(theirsAfter)}`)
say(failures === 0 ? 'OPENCODE 2 RUNS: PASSED' : `${String(failures)} check(s) failed`)
process.exit(failures === 0 ? 0 : 1)
