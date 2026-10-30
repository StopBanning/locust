// A pull request link in a reply wears its state (exec/live-pr-status).
//
//   node _tools/drive-live-pr-status.mjs
//
// A seeded reply links two pull requests and an issue. gh is a stand-in:
// 42 is merged, 7 is open with a failing check. The issue stays a plain link.
// Both themes, at 1440x900 and 1205x803. Sends nothing; no model runs.

import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const merged = JSON.stringify({
  state: 'MERGED', isDraft: false, mergedAt: '2026-10-18T12:00:00Z', title: 'Quiet the header',
  statusCheckRollup: [{ __typename: 'CheckRun', conclusion: 'SUCCESS', status: 'COMPLETED', name: 'CI' }]
})
const failing = JSON.stringify({
  state: 'OPEN', isDraft: false, mergedAt: null, title: 'Fix the cart',
  statusCheckRollup: [
    { __typename: 'CheckRun', conclusion: 'FAILURE', status: 'COMPLETED', name: 'CI' },
    { __typename: 'CheckRun', conclusion: 'SUCCESS', status: 'COMPLETED', name: 'Lint' }
  ]
})
const csharp = `using System;
public class G {
  public static int Main(string[] args) {
    string number = "";
    for (int i = 0; i < args.Length; i++) if (args[i] == "view" && i + 1 < args.Length) number = args[i + 1];
    if (number == "42") { Console.Write(${JSON.stringify(merged)}); return 0; }
    if (number == "7") { Console.Write(${JSON.stringify(failing)}); return 0; }
    Console.Write("[]");
    return 0;
  }
}
`
const bin = await mkdtemp(join(tmpdir(), 'locust-prlink-gh-'))
const source = join(bin, 'gh.cs')
await writeFile(source, csharp, 'utf8')
const csc = 'C:\\Windows\\Microsoft.NET\\Framework64\\v4.0.30319\\csc.exe'
await new Promise((resolve, reject) => {
  execFile(csc, ['/nologo', '/out:' + join(bin, 'gh.exe'), source], { windowsHide: true }, (error, stdout, stderr) => {
    if (error) reject(new Error(String(stderr || stdout || error)))
    else resolve(undefined)
  })
})
const gh = (args) => new Promise((resolve, reject) => {
  execFile(join(bin, 'gh.exe'), args, { windowsHide: true }, (error, stdout, stderr) => {
    if (error) reject(new Error(String(stderr || stdout || error)))
    else resolve(String(stdout))
  })
})
for (const [number, text] of [['42', merged], ['7', failing]]) {
  const out = await gh(['pr', 'view', number, '--repo', 'acme/app', '--json', 'state,isDraft,mergedAt,statusCheckRollup,title'])
  JSON.parse(out)
  if (out !== text) throw new Error(`gh stand-in for ${number} returned ${out}`)
}
say('gh stand-in answers 42 merged and 7 open with a failing check')

const workspace = await scratchRepository('locust-drive-prlink-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-prlink-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const missionId = 'mission_10000000-0000-4000-8000-000000000042'
const runId = 'run_10000000-0000-4000-8000-000000000042'
const at = new Date(Date.now() - 60_000).toISOString()
const reply = 'Opened [https://github.com/acme/app/pull/42](https://github.com/acme/app/pull/42) and [https://github.com/acme/app/pull/7](https://github.com/acme/app/pull/7). See [https://github.com/acme/app/issues/3](https://github.com/acme/app/issues/3).'
await ledger.createMission({
  missionId, runId, prompt: 'Where did the two pull requests land?',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
const normalizer = adapters.createCodexEventNormalizer({
  runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at)
})
await ledger.appendEvents(missionId, [
  ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
  ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: reply } }) }),
  ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
  ...normalizer.finish({
    exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false,
    forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0,
    startedAt: at, finishedAt: at
  })
])
say('seeded one reply linking pull/42, pull/7 and issues/3')

process.env.LOCUST_DRIVE_PATH_FIRST = bin
const drive = await startDrive({
  name: 'live-pr-status',
  port: 9531,
  workspace,
  profilePath,
  sendsNothing: true,
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

const readChips = `(() => {
  document.querySelector('.lc-prlink')?.scrollIntoView({ block: 'center' })
  const chips = [...document.querySelectorAll('.lc-prlink')].map((el) => ({
    state: el.getAttribute('data-state'),
    checks: el.getAttribute('data-checks'),
    text: el.querySelector('.lc-prlink__state')?.textContent ?? '',
    title: el.getAttribute('title') ?? '',
    mark: el.querySelector('.lc-prlink__mark') !== null
  }))
  const links = [...document.querySelectorAll('.lc-link')].map((el) => ({
    title: el.getAttribute('title') ?? '',
    chip: el.nextElementSibling?.classList.contains('lc-prlink') === true
  }))
  return JSON.stringify({ chips, links, theme: document.documentElement.dataset.theme ?? '' })
})()`

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const expectChips = (seen, theme) => {
  const parsed = JSON.parse(seen)
  check(`${theme}: two chips`, parsed.chips.length === 2 && parsed.theme === theme, JSON.stringify(parsed.chips))
  const mergedChip = parsed.chips.find((chip) => chip.state === 'merged')
  const openChip = parsed.chips.find((chip) => chip.state === 'open')
  check(`${theme}: 42 is Merged, with no check mark`, mergedChip?.text === 'Merged' && mergedChip?.checks === 'none' && mergedChip?.title === 'Merged' && mergedChip?.mark === false, JSON.stringify(mergedChip))
  check(`${theme}: 7 is Open, one check failing`, openChip?.text === 'Open' && openChip?.checks === 'failing' && openChip?.title === 'Open · 1 of 2 checks passed · 1 failing' && openChip?.mark === true, JSON.stringify(openChip))
  const issue = parsed.links.find((link) => link.title.includes('/issues/3'))
  const pull42 = parsed.links.find((link) => link.title.includes('/pull/42'))
  const pull7 = parsed.links.find((link) => link.title.includes('/pull/7'))
  check(`${theme}: the issue is a plain link`, issue?.chip === false, JSON.stringify(issue))
  check(`${theme}: both pull requests wear a chip`, pull42?.chip === true && pull7?.chip === true, JSON.stringify(parsed.links))
}

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const opened = String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 40; i += 1) {
      const row = document.querySelector('[data-mission-id="${missionId}"]')
      if (row) { row.click(); return 'opened' }
      await new Promise((r) => setTimeout(r, 250))
    }
    return 'no row'
  })()`))
  check('the seeded conversation opened', opened === 'opened', opened)
  await drive.waitFor(`document.querySelectorAll('.lc-prlink').length === 2`, { timeoutMs: 20_000, what: 'two pull request chips' })
  const shot = async (theme, width, height) => {
    await drive.resize(width, height)
    await drive.evaluate(`window.desktop.setTheme(${JSON.stringify(theme)})`)
    await drive.waitFor(`document.documentElement.dataset.theme === ${JSON.stringify(theme)}`, { timeoutMs: 5000, what: `${theme} theme` })
    await sleep(250)
    return String(await drive.capture(`${theme} ${String(width)}x${String(height)}`, () => drive.evaluate(readChips)))
  }
  expectChips(await shot('dark', 1440, 900), 'dark')
  expectChips(await shot('light', 1440, 900), 'light')
  expectChips(await shot('dark', 1205, 803), 'dark')
  expectChips(await shot('light', 1205, 803), 'light')
  say(failures === 0 ? '\nLIVE PR STATUS PASSED' : `\nLIVE PR STATUS: ${String(failures)} FAILED`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A reply links two pull requests and an issue. The pull requests wear Merged and Open; the issue stays a plain link. Both themes, at 1440x900 and 1205x803.' })
  if (failures > 0) process.exitCode = 1
}
