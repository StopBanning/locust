// Does a refused Resume from checkpoint say why, on the card (M28)?
//
//   node _tools/drive-resume-refusal-shown.mjs [--packaged <exe>] [--tag <name>]
//
// The code review's M28: a refused Resume from checkpoint was stored as a run
// nothing showed, so the button just seemed dead. An interrupted mission of
// Wren's is seeded; the chat box is put on a route the host refuses before
// anything starts; Resume is pressed. Nothing may start (the ledger is
// counted), and the card must say why. Sends nothing.

import { mkdir, mkdtemp, readdir } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, git, pickRouteScript, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `resume-refusal-shown-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-resref-ws-')
const tree = join(workspace, '.locust', 'worktrees', 'tm_wren')
await mkdir(join(workspace, '.locust', 'worktrees'), { recursive: true })
await git(['worktree', 'add', '-q', '-b', 'locust/wren', tree, 'HEAD'], workspace)
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-resref-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const at = new Date(Date.now() - 10 * 60_000).toISOString()
await ledger.createMission({
  missionId: 'mission_interrupted', runId: 'run_interrupted', prompt: 'Reply with the single word RESUMED. Do nothing else.',
  runtime: 'claude', model: 'haiku', requestedRouteId: 'claude', resolvedRouteId: 'claude-account:haiku', cliVersion: null,
  workspaceId, sandbox: 'read-only', mode: 'ask', executionPolicyVersion: 1, createdAt: at
})
// Started, never finished: what an app closed mid-run leaves behind.
const normalizer = adapters.createClaudeEventNormalizer({ runId: 'run_interrupted', missionId: 'mission_interrupted', cliVersion: '2.1.281', now: () => new Date(at) })
await ledger.appendEvents('mission_interrupted', normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'system', subtype: 'init', session_id: 'session_interrupted', model: 'haiku', tools: [] }) }))

// The checkpoint a shutdown writes, which is what makes it resumable.
await ledger.createCheckpoint('mission_interrupted', 'shutdown')
const T0 = '2026-09-05T05:00:00.000Z'
const drive = await startDrive({
  sendsNothing: true,
  name: 'resume-refusal-shown',
  port: 9565,
  workspace,
  profilePath,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, worktree: true, route: { runtime: 'claude', model: 'haiku', mode: 'ask' } }],
    missionOwners: { mission_interrupted: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
try {
  await drive.capture('launch: Wren, with an interrupted mission', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(1500)
  // A route the host refuses before anything starts: Cursor in Ask, which
  // Windows cannot hold read-only. Deterministic, and it spends nothing.
  await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /RESUMED/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
    row?.click()
    await new Promise((r) => setTimeout(r, 1500))
  })()`)
  const route = await drive.evaluate(pickRouteScript({ group: '/cursor/i', search: 'auto', row: '/auto/i' }))
  check('the chat box is on Cursor', /cursor/i.test(String(route)), String(route))
  // Ask, chosen from the mode menu as a person does.
  const asked = String(await drive.evaluate(`(async () => {
    const button = [...document.querySelectorAll('form.command-dock button')].find((b) => b.getAttribute('aria-haspopup') === 'menu' && /Edit|Ask|Auto|Plan|Approve/.test(b.textContent ?? ''))
    button?.click()
    await new Promise((r) => setTimeout(r, 400))
    const ask = [...document.querySelectorAll('[role="menuitemradio"]')].find((item) => /^\\s*Ask/.test(item.textContent ?? ''))
    ask?.click()
    await new Promise((r) => setTimeout(r, 400))
    return button?.textContent?.trim() ?? 'no mode button'
  })()`))
  // Informational: the menu does not offer Ask for Cursor on Windows; the host refuses the resume either way.
  say(`  mode: ${asked}`)
  const missionsBefore = (await readdir(join(profilePath, 'mission-ledger')).catch(() => [])).length
  const pressed = await drive.capture('open the interrupted mission and press Resume', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = [...document.querySelectorAll('button')].find((b) => /Resume (from checkpoint|anyway)/.test(b.textContent ?? ''))
      if (button && !button.disabled) { button.click(); await new Promise((r) => setTimeout(r, 2500)); return 'pressed' }
    }
    return 'no Resume button'
  })()`))
  check('Resume was pressed', pressed === 'pressed', String(pressed))
  const missionsAfter = (await readdir(join(profilePath, 'mission-ledger')).catch(() => [])).length
  check('nothing was started', missionsAfter === missionsBefore, `${String(missionsBefore)} -> ${String(missionsAfter)} ledger entries`)
  const shown = String(await drive.evaluate(`document.querySelector('.lc-resume__refusal')?.textContent?.trim() ?? document.querySelector('.lc-resume')?.innerText.replace(/\\s+/g, ' ') ?? 'no card'`))
  check('the card says why it did not resume', /^Not resumed:/.test(shown), shown)
  say(failures === 0 ? '\nRESUME REFUSAL SHOWN PASSED' : `\nRESUME REFUSAL SHOWN: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Resume from checkpoint on Wren's interrupted mission with the chat box on Cursor in Ask (refused on Windows before anything starts).` })
}
