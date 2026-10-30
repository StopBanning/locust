// Does a conversation's header offer Changes only in a folder that is a git repository? (2026-10-16)
//
//   node _tools/drive-a-folder-without-git-offers-no-changes.mjs [--packaged <Locust.exe>]
//
// The clean-machine drive found that on a computer without Git the Changes button only ever opened "This folder is
// not a git repository". Two folders, each with a seeded, finished conversation that changed app.js (nothing is sent,
// nothing spent):
//   plain -- no git: the header is drawn (Activity is there) but has no Changes, and no Commit.
//   git   -- a repository with app.js changed: Changes is there and its panel shows the change.
// Control on 2026-10-16: packaged 0.781 FAILED "a folder without git offers no Changes".

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const git = (cwd, ...args) => execFileSync('git', args, { cwd, encoding: 'utf8', windowsHide: true })
const SERVER = (port) => `const http = require('http')\n\nconst port = ${String(port)}\n\nhttp.createServer((req, res) => res.end('ok')).listen(port)\n`

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}

async function folderWithConversation(kind) {
  const workspace = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), `locust-drive-changes-${kind}-ws-`))
  if (kind === 'git') {
    git(workspace, 'init', '-q', '-b', 'main')
    git(workspace, 'config', 'user.email', 'drive@locust.test')
    git(workspace, 'config', 'user.name', 'Locust drive')
    await writeFile(join(workspace, 'app.js'), SERVER(3000), 'utf8')
    git(workspace, 'add', '.')
    git(workspace, 'commit', '-q', '-m', 'A small server')
  }
  await writeFile(join(workspace, 'app.js'), SERVER(3001), 'utf8')
  const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), `locust-drive-changes-${kind}-profile-`))
  const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
  const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
  const missionId = `mission_5e000000-0000-4000-8000-00078200000${kind === 'git' ? '1' : '2'}`
  const runId = `run_5e0782${kind === 'git' ? 'a' : 'b'}`
  const at = new Date(Date.now() - 600_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: 'In app.js, change the port from 3000 to 3001. Change nothing else.',
    runtime: 'opencode', model: FREE_ROUTE.model, requestedRouteId: 'opencode', resolvedRouteId: `opencode:${FREE_ROUTE.model}`, cliVersion: null,
    workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
  })
  const process_ = { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at }
  const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'Changed the port in app.js to 3001.', final: true }),
    event(2, 'run.completed', { runtimeTerminal: 'completed', process: process_ })
  ])
  await ledger.flush?.()
  return { workspace, profilePath, missionId }
}

const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Developer', createdAt: '2026-10-15T01:00:00.000Z', route: FREE_ROUTE }
const HEADER = `JSON.stringify({
  changes: [...document.querySelectorAll('button')].some((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Changes'),
  activity: [...document.querySelectorAll('button')].some((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Activity'),
  commit: [...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Commit'),
  panel: Boolean(document.querySelector('.lc-changes'))
})`

for (const [index, kind] of ['plain', 'git'].entries()) {
  const { workspace, profilePath, missionId } = await folderWithConversation(kind)
  say(`=== ${kind} folder: ${workspace}`)
  const drive = await startDrive({
    ...(packaged === undefined ? {} : { packaged }),
    name: `changes-button-${kind}-${tag}`,
    port: 9787 + index,
    workspace,
    profilePath,
    outPath: join(recordRoot('a-folder-without-git-offers-no-changes-2026-10-16'), `${kind}-${tag}`),
    seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
  })
  try {
    await drive.ready()
    await drive.resize(1440, 900)
    const header = JSON.parse(String(await drive.capture(`the conversation in the ${kind} folder`, () => drive.evaluate(`(async () => {
      ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /port/i.test(r.innerText))?.click()
      for (let i = 0; i < 40 && ![...document.querySelectorAll('button')].some((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Activity'); i += 1) await new Promise((r) => setTimeout(r, 250))
      await new Promise((r) => setTimeout(r, 2500))
      return ${HEADER}
    })()`))))
    check(`the ${kind} folder's conversation header is drawn (Activity is there)`, header.activity, JSON.stringify(header))
    if (kind === 'plain') {
      check('a folder without git offers no Changes', !header.changes, JSON.stringify(header))
      check('nor Commit', !header.commit, JSON.stringify(header))
    } else {
      check('a repository offers Changes', header.changes, JSON.stringify(header))
      const panel = String(await drive.capture('Changes opened in the repository', () => drive.evaluate(`(async () => {
        ;[...document.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Changes')?.click()
        for (let i = 0; i < 40 && !document.querySelector('.lc-changes .lc-diff__row.is-add'); i += 1) await new Promise((r) => setTimeout(r, 250))
        return (document.querySelector('.lc-changes')?.innerText ?? 'no panel').replace(/\\s+/g, ' ').slice(0, 300)
      })()`)))
      check('and its panel shows the change to app.js', /app\.js/.test(panel) && /3001/.test(panel), panel)
    }
  } catch (error) {
    failures += 1
    say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    await drive.finish({ intro: `${packaged ?? 'out/'}: a seeded conversation in a ${kind} folder; nothing sent.` })
  }
  await sleep(1000)
}
say(failures === 0 ? 'CHANGES BUTTON DRIVE PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
