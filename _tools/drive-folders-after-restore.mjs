// A real backup is applied to a disposable profile; the restore card moves its Windows folder.
// The continued turn uses only the free OpenCode route. No installed app or profile is touched.
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { FREE_ROUTE, startDrive, say } from './drive-lib.mjs'

const old = 'C:\\work\\demo'
const hash = value => createHash('sha256').update(value).digest('hex')
const id = `ws_${hash(old).slice(0, 32)}`
const root = await mkdtemp(join(tmpdir(), 'locust-folder-restore-drive-'))
const local = await mkdtemp(join(homedir(), 'work', 'folder-restore-drive-'))
// A unique parent avoids colliding with any real demo project.
const target = join(local, 'demo')
await mkdir(target)
const profile = join(root, 'profile')
const backup = join(root, 'backup')
await mkdir(profile)
const at = '2026-10-30T12:00:00.000Z'
const metadata = { missionId: 'mission_restored', runId: 'run_restored', prompt: 'Remember this conversation.', runtime: 'opencode', cliVersion: null, model: FREE_ROUTE.model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', workspaceId: id, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at, mode: 'ask' }
const events = [
  { type: 'run.started', payload: { runtimeThreadId: 'old-computer-session', evidence: { redacted: true } } },
  { type: 'message.delta', payload: { itemId: 'answer', operation: 'append', text: 'Ready.', final: true, evidence: { redacted: true } } },
  { type: 'run.completed', payload: { process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } } }
]
const ledger = [
  { schemaVersion: 15, recordType: 'mission.created', ledgerSequence: 1, occurredAt: at, metadata },
  ...events.map((event, index) => ({ schemaVersion: 15, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: at, event: { id: `event_${index}`, runId: metadata.runId, missionId: metadata.missionId, sequence: index + 1, occurredAt: at, sourceAdapter: 'opencode', ...event } }))
].map(value => JSON.stringify(value) + '\n').join('')
const files = {
  'folders.json': JSON.stringify({ schemaVersion: 1, folders: [{ id, path: old, name: 'demo' }] }),
  'teammates.json': JSON.stringify({ schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, folder: old, route: FREE_ROUTE }], missionOwners: { mission_restored: 'tm_wren' }, settings: { swarm: false, relay: false, memoryMode: 'off', autoMode: false } }),
  'mission-ledger/mission_restored.jsonl': ledger
}
const entries = []
for (const [path, text] of Object.entries(files)) {
  const to = join(backup, 'profile', path)
  await mkdir(dirname(to), { recursive: true })
  await writeFile(to, text)
  entries.push({ path, bytes: Buffer.byteLength(text), sha256: hash(text) })
}
await writeFile(join(backup, 'locust-backup.json'), JSON.stringify({ schema: 1, app: 'Locust', appVersion: '0.1.0', createdAt: at, counts: { teammates: 1, routines: 0, memories: 0, groups: 0, rooms: 0, compares: 0, folders: 1, conversations: 1 }, files: entries, leftOut: [] }))
await writeFile(join(profile, 'pending-restore.json'), JSON.stringify({ folder: backup }))
let drive
try {
  drive = await startDrive({ name: 'folders-after-restore', port: 9793, workspace: target, profilePath: profile, holdCursor: false })
  await drive.ready()
  await drive.resize(1280, 900)
  const history = JSON.parse(await drive.evaluate('(async () => JSON.stringify(await window.desktop.getMissionHistory()))()'))
  if (!history.ok || !history.data.missions.some(mission => mission.missionId === 'mission_restored' && mission.phase === 'completed')) throw new Error('The backup conversation was not readable before applying a move')
  const offered = await drive.capture('the restored folder is offered', () => drive.evaluate(`document.querySelector('[aria-label="Find restored folders"]')?.textContent ?? ''`))
  if (!String(offered).includes(old) || !String(offered).includes(target)) throw new Error(`Expected old and matching local folders: ${offered}`)
  await drive.capture('one click applies the matching folder', () => drive.evaluate(`[...document.querySelectorAll('[aria-label="Find restored folders"] button')].find(button => button.textContent === 'Apply folders').click()`))
  await drive.waitFor(`document.querySelector('[aria-label="Find restored folders"]') === null`)
  const stored = JSON.parse(await readFile(join(profile, 'folders.json'), 'utf8'))
  if (stored.folders.find(folder => folder.id === id)?.path !== target) throw new Error('Old folder id was not moved')
  const roster = JSON.parse(await readFile(join(profile, 'teammates.json'), 'utf8'))
  if (roster.teammates[0].folder !== target) throw new Error('Teammate did not follow')
  const movedLedger = await readFile(join(profile, 'mission-ledger/mission_restored.jsonl'), 'utf8')
  if (!movedLedger.startsWith(ledger) || !movedLedger.includes('host.folder_moved')) throw new Error('History was rewritten or the move was not recorded')
  let continuedId
  try {
    const response = JSON.parse(await drive.capture('the old conversation continues', () => drive.evaluate(`(async () => JSON.stringify(await window.desktop.startCodexMission(${JSON.stringify({ prompt: 'Reply with Ready. Do not call tools or edit files.', runtime: 'opencode', model: FREE_ROUTE.model, mode: 'ask', teammateId: 'tm_wren', followUpOf: 'mission_restored' })})))()`)))
    if (!response.ok) throw new Error(JSON.stringify(response))
    continuedId = response.data.missionId
    await drive.waitFor(`(async () => {
      const history = await window.desktop.getMissionHistory()
      if (!history.ok) return false
      const mission = history.data.missions.find(m => m.missionId === ${JSON.stringify(continuedId)})
      if (!mission) return false
      if (mission.phase === 'completed') return true
      if (mission.phase === 'failed' || mission.phase === 'cancelled') {
        throw new Error('Mission ' + mission.missionId + ' ended in ' + mission.phase + ': ' + (mission.hostFailureMessage ?? 'no error message'))
      }
      return false
    })()`, { timeoutMs: 180_000, what: 'continuation on free OpenCode' })
    const continued = await readFile(join(profile, 'mission-ledger', `${continuedId}.jsonl`), 'utf8')
    if (!continued.includes(`ws_${hash(target).slice(0, 32)}`)) throw new Error('Continued turn was not recorded in the chosen folder')
    say('RESTORED FOLDERS PASSED: old id, teammate, append-only record and continued turn')
  } catch (error) {
    if (drive && continuedId) {
      const continuedFile = join(profile, 'mission-ledger', `${continuedId}.jsonl`)
      say('CONTINUED LEDGER: ' + (await readFile(continuedFile, 'utf8').catch(() => 'no continued ledger file')))
      const hist = await drive.evaluate('(async () => JSON.stringify(await window.desktop.getMissionHistory()))()').catch(e => String(e))
      say(`HISTORY: ${hist}`)
    }
    throw error
  }
} catch (error) {
  if (drive) {
    say(String(await drive.evaluate(`document.querySelector('[aria-label="Find restored folders"]')?.textContent ?? 'no card'`)))
    say(await readFile(join(profile, 'mission-ledger/mission_restored.jsonl'), 'utf8'))
  }
  throw error
} finally {
  if (drive) await drive.finish({ intro: 'A throwaway backup restored from Windows; one Apply folders click; free OpenCode continuation.' })
  await rm(root, { recursive: true, force: true })
  await rm(local, { recursive: true, force: true })
}
