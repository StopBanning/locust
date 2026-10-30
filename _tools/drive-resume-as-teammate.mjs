// Is a mission resumed from its checkpoint resumed AS its teammate (H6)?
//
//   LOCUST_SPEND=1 node _tools/drive-resume-as-teammate.mjs [--packaged <exe>] [--tag <name>]
//
// The code review's H6: Resume from checkpoint started the continuation as
// nobody's -- in the project folder rather than the teammate's own branch,
// never assigned to them. Wren has Own branch on; an interrupted mission of
// hers is seeded in the ledger (started, never finished). "Resume from
// checkpoint" is pressed in the window. Then: where did the resumed Claude
// Code session run (Claude files a session under its working folder), and
// whose is the new mission (the profile's own owner record)? One Haiku turn.

import { mkdir, mkdtemp, readdir, readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, git, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `resume-as-teammate-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-resume-ws-')
const tree = join(workspace, '.locust', 'worktrees', 'tm_wren')
await mkdir(join(workspace, '.locust', 'worktrees'), { recursive: true })
await git(['worktree', 'add', '-q', '-b', 'locust/wren', tree, 'HEAD'], workspace)
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-resume-profile-'))
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
  spends: true,
  name: 'resume-as-teammate',
  port: 9559,
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
const dashed = (path) => path.replace(/[^A-Za-z0-9]/g, '-')
const sessionsIn = async (path) => (await readdir(join(homedir(), '.claude', 'projects', dashed(path))).catch(() => [])).filter((name) => name.endsWith('.jsonl'))

try {
  await drive.capture('launch: an interrupted mission of Wren’s', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(1500)
  const pressed = await drive.capture('open it and press Resume from checkpoint', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40; i += 1) {
      const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /RESUMED/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
      if (row) { row.click(); break }
      await new Promise((r) => setTimeout(r, 250))
    }
    for (let i = 0; i < 40; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = [...document.querySelectorAll('button')].find((b) => /Resume (from checkpoint|anyway)/.test(b.textContent ?? ''))
      if (button) { button.click(); return 'pressed ' + button.textContent.trim() }
    }
    return 'no Resume button'
  })()`))
  check('Resume from checkpoint was offered and pressed', /^pressed/.test(String(pressed)), String(pressed))
  await drive.capture('wait for the resumed run', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      if (i > 10 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'settled after ' + String(i / 2) + 's'
    }
    return 'still going'
  })()`))
  const inTree = await sessionsIn(tree)
  const inProject = await sessionsIn(workspace)
  check('the resumed run ran in Wren’s own branch', inTree.length > 0, `${String(inTree.length)} session(s) under the worktree, ${String(inProject.length)} under the project`)
  check('and not in the project folder', inProject.length === 0)
  // Whose it is, from the profile's own record of owners.
  let owners = {}
  for (const name of await readdir(profilePath)) {
    if (!name.endsWith('.json')) continue
    try {
      const parsed = JSON.parse(await readFile(join(profilePath, name), 'utf8'))
      if (parsed?.missionOwners !== undefined) owners = parsed.missionOwners
    } catch {
      // Not the file.
    }
  }
  const resumedOwned = Object.entries(owners).filter(([missionId, owner]) => missionId !== 'mission_interrupted' && owner === 'tm_wren')
  check('the resumed mission is recorded as Wren’s', resumedOwned.length === 1, JSON.stringify(owners))
  say(failures === 0 ? '\nRESUME AS TEAMMATE PASSED' : `\nRESUME AS TEAMMATE: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Wren (Own branch, Claude Haiku, Ask) with an interrupted mission; Resume from checkpoint.` })
}
