import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { createFileMissionLedger } from '@teammate/mission-store'
import type { MissionLedger, MissionLedgerMetadata } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { namedByATeammate } from './mission-history.js'
import { filesNamedIn, namedFileKey, relativeFilesNamedIn } from './named-files.js'
import { decideTeammateFile, readRoots } from './reveal-file.js'
import { workspaceIdFor } from './workspace.js'

/**
 * A FILE A TEAMMATE WROTE OUTSIDE THE FOLDER OPENS (0.787).
 *
 * Colin, 2026-10-18, with a screenshot: Codex was asked to write its review
 * to D:\work\scratch\toaster-review2, wrote Codex.md there with its own
 * apply_patch, and handed it over by its whole path. The thread said "Not
 * shown as a file", and the "created a file" row above it would not open it
 * either. "claude code and the other apps still let you access the file."
 *
 * The events below are that turn's, from his ledger: the patch's target as a
 * Windows path, the handover with forward slashes, the reply in fragments.
 * The rule they pin: a whole path outside every folder Locust works in opens
 * when the teammate's own record names it, and only then.
 */

const WINDOWS = process.platform === 'win32'
const REPORT = WINDOWS ? 'D:\\work\\scratch\\toaster-review2\\Codex.md' : '/work/scratch/toaster-review2/Codex.md'
const HANDED = REPORT.replace(/\\/g, '/')
const NEVER_NAMED = WINDOWS ? 'C:\\Users\\someone\\.ssh\\id_rsa' : '/home/someone/.ssh/id_rsa'
const WORKSPACE = WINDOWS ? 'C:\\Users\\someone\\.claude' : '/home/someone/.claude'

let sequence = 0
const event = (type: NormalizedRuntimeEvent['type'], payload: Record<string, unknown>): NormalizedRuntimeEvent =>
  ({
    id: `run_review:${String(++sequence)}`,
    runId: 'run_review',
    missionId: 'mission_review',
    sequence,
    type,
    occurredAt: '2026-10-18T20:00:00.000Z',
    sourceAdapter: 'codex',
    payload: { ...payload, evidence: { redacted: false } }
  }) as NormalizedRuntimeEvent
const said = (text: string, final = false): NormalizedRuntimeEvent =>
  event('message.delta', { itemId: 'msg_1', operation: 'append', text, final })
const step = (type: 'tool.started' | 'tool.completed' | 'tool.failed', command: string, status?: string): NormalizedRuntimeEvent =>
  event(type, { itemId: 'item_patch', toolKind: 'fileChange', name: 'apply_patch', command, phase: type === 'tool.started' ? 'started' : 'completed', ...(status === undefined ? {} : { status }) })

const wrote = (): readonly NormalizedRuntimeEvent[] => [step('tool.started', REPORT), step('tool.completed', REPORT, 'completed')]
const handedOver = (): readonly NormalizedRuntimeEvent[] => [
  said('The review found four issues.\n\n<locust-'),
  said(`file>\n${HANDED} :: Review findings, evidence, fixes and verification limits\n</locust-file>`, true)
]
const theTurn = (): readonly NormalizedRuntimeEvent[] => [...wrote(), ...handedOver()]

describe('what a turn names as its own', () => {
  it('names the file its finished step wrote and the file it handed over, as one file', () => {
    expect(filesNamedIn(theTurn())).toEqual([namedFileKey(REPORT)])
    // The handover alone, in its own spelling, is the same file.
    expect(filesNamedIn(handedOver())).toEqual([namedFileKey(REPORT)])
    expect(filesNamedIn(wrote())).toEqual([namedFileKey(REPORT)])
  })

  it('names nothing for a step that was declined, failed, or never finished', () => {
    expect(filesNamedIn([step('tool.started', REPORT)])).toEqual([])
    expect(filesNamedIn([step('tool.failed', REPORT)])).toEqual([])
    expect(filesNamedIn([step('tool.completed', REPORT, 'declined')])).toEqual([])
  })

  it('does not read a shell command for paths, nor take a network path or a relative one', () => {
    expect(filesNamedIn([step('tool.completed', `powershell -Command "Get-Content '${REPORT}'"`, 'completed')])).toEqual([])
    expect(filesNamedIn([step('tool.completed', '\\\\server\\share\\report.md', 'completed')])).toEqual([])
    expect(filesNamedIn([said('<locust-file>\n//server/share/report.md\ndocs/report.md\n</locust-file>', true)])).toEqual([])
    expect(namedFileKey('\\\\?\\C:\\x.md')).toBeUndefined()
  })

  it('reads a handover only from what the teammate said, never a file block quoted in code', () => {
    expect(filesNamedIn([said(`Write it like this:\n\n\`\`\`\n<locust-file>\n${HANDED}\n</locust-file>\n\`\`\``, true)])).toEqual([])
  })
})

describe('the host, asked to open a file outside every folder it knows', () => {
  let root: string
  let ledger: MissionLedger

  beforeEach(async () => {
    sequence = 0
    root = await mkdtemp(join(tmpdir(), 'locust-named-'))
    ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      missionId: 'mission_review',
      runId: 'run_review',
      prompt: 'Review the Toaster changes and write the report to D:\\work\\scratch\\toaster-review2\\Codex.md',
      runtime: 'codex',
      model: 'gpt-6.1-sol',
      requestedRouteId: 'codex:gpt-6.1-sol',
      resolvedRouteId: 'codex:gpt-6.1-sol',
      cliVersion: '0.160.0',
      workspaceId: 'ws_claude',
      createdAt: '2026-10-18T20:00:00.000Z',
      sandbox: 'workspace-write',
      executionPolicyVersion: 1
    } as MissionLedgerMetadata)
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true }).catch(() => undefined)
  })

  it('opens the one the teammate wrote and handed over, in any spelling of it', async () => {
    await ledger.appendEvents('mission_review', theTurn())
    const named = (path: string): Promise<boolean> => namedByATeammate(ledger, path)
    const decision = await decideTeammateFile(REPORT, [WORKSPACE], named)
    expect(decision).toEqual({ ok: true, path: REPORT, named: true })
    expect(await named(HANDED)).toBe(true)
    if (WINDOWS) expect(await named(REPORT.toUpperCase())).toBe(true)
    // A read asks its link question of the file's own folder: it is in no root.
    expect(decision.ok && readRoots(decision, [WORKSPACE])).toEqual([dirname(REPORT)])
  })

  it('refuses a file no teammate named, the way it always has', async () => {
    await ledger.appendEvents('mission_review', theTurn())
    expect(await decideTeammateFile(NEVER_NAMED, [WORKSPACE], (path) => namedByATeammate(ledger, path))).toMatchObject({ ok: false, reason: 'outside' })
  })

  it('finds a file handed over while the turn is still running', async () => {
    const named = (path: string): Promise<boolean> => namedByATeammate(ledger, path)
    await ledger.appendEvents('mission_review', [said('Working on it. ')])
    expect(await named(REPORT)).toBe(false)
    // The same record, grown: the history's cache reads a changed file again.
    await ledger.appendEvents('mission_review', handedOver())
    expect(await named(REPORT)).toBe(true)
  })

  it('stops opening it once the conversation is deleted', async () => {
    await ledger.appendEvents('mission_review', theTurn())
    expect(await namedByATeammate(ledger, REPORT)).toBe(true)
    await ledger.deleteMission('mission_review')
    expect(await namedByATeammate(ledger, REPORT)).toBe(false)
  })

  it('never looks up a network path, a relative one, or one inside the folder', async () => {
    const asked: string[] = []
    const named = async (path: string): Promise<boolean> => {
      asked.push(path)
      return true
    }
    expect(await decideTeammateFile('\\\\server\\share\\report.md', [WORKSPACE], named)).toMatchObject({ ok: false })
    expect(await decideTeammateFile('//server/share/report.md', [WORKSPACE], named)).toMatchObject({ ok: false })
    expect(await decideTeammateFile('docs/report.md', [WORKSPACE], named)).toMatchObject({ ok: false })
    const inFolder = join(WORKSPACE, 'notes.md')
    expect(await decideTeammateFile(inFolder, [WORKSPACE], named)).toEqual({ ok: true, path: inFolder })
    expect(asked).toEqual([])
  })
})

/**
 * A REPORT HANDED OVER FROM THE FOLDER IT RAN IN (0.868).
 *
 * Colin, 2026-11-06, with a screenshot: Codex, working in now-coding, built in a worktree beside it and handed over
 * `../locust-wt/spotify-artist/REPORT.md`. The copy button said "Not copied" ("wont let me copy"): only a whole path
 * counted as named. A relative handover now counts, resolved against the one folder that turn ran in.
 */
describe('a file handed over by a path from the folder its turn ran in', () => {
  const FOLDER = WINDOWS ? 'C:\\work\\now-coding' : '/work/now-coding'
  const ELSEWHERE_FOLDER = WINDOWS ? 'C:\\work\\other' : '/work/other'
  const WORKTREE_REPORT = WINDOWS ? 'C:\\work\\locust-wt\\spotify-artist\\REPORT.md' : '/work/locust-wt/spotify-artist/REPORT.md'
  const handedRelative = (): readonly NormalizedRuntimeEvent[] => [
    said('Committed on `exec/spotify-artist`.\n\n<locust-file>\n../locust-wt/spotify-artist/REPORT.md :: Implementation and verification limits\n</locust-file>', true)
  ]
  let root: string
  let ledger: MissionLedger

  beforeEach(async () => {
    sequence = 0
    root = await mkdtemp(join(tmpdir(), 'locust-named-relative-'))
    ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      missionId: 'mission_review',
      runId: 'run_review',
      prompt: 'Build the Spotify artist pages in a worktree and write a report',
      runtime: 'codex',
      model: 'gpt-6.1-sol',
      requestedRouteId: 'codex:gpt-6.1-sol',
      resolvedRouteId: 'codex:gpt-6.1-sol',
      cliVersion: '0.160.0',
      workspaceId: workspaceIdFor(FOLDER),
      createdAt: '2026-11-06T20:00:00.000Z',
      sandbox: 'workspace-write',
      executionPolicyVersion: 1
    } as MissionLedgerMetadata)
    await ledger.appendEvents('mission_review', handedRelative())
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true }).catch(() => undefined)
  })

  it('keeps the relative path a file block hands over, and no whole path for it', () => {
    expect(relativeFilesNamedIn(handedRelative())).toEqual(['../locust-wt/spotify-artist/REPORT.md'])
    expect(filesNamedIn(handedRelative())).toEqual([])
    expect(relativeFilesNamedIn([said('<locust-file>\n//server/share/report.md\n</locust-file>', true)])).toEqual([])
  })

  it('copies it when the folder its turn ran in is one Locust knows', async () => {
    const named = (path: string): Promise<boolean> => namedByATeammate(ledger, path, [ELSEWHERE_FOLDER, FOLDER])
    expect(await decideTeammateFile(join(FOLDER, '..', 'locust-wt', 'spotify-artist', 'REPORT.md'), [FOLDER], named)).toEqual({ ok: true, path: WORKTREE_REPORT, named: true })
  })

  it('resolves it against that folder only, never another one it knows', async () => {
    // From the other folder the same words would name /work/locust-wt/... too, so ask for a file only another folder resolves to.
    const fromOther = WINDOWS ? 'C:\\locust-wt\\spotify-artist\\REPORT.md' : '/locust-wt/spotify-artist/REPORT.md'
    expect(await namedByATeammate(ledger, fromOther, [FOLDER, ELSEWHERE_FOLDER.replace(/other$/, 'deep/other')])).toBe(false)
    // And without the folder it ran in among the known ones, nothing counts.
    expect(await namedByATeammate(ledger, WORKTREE_REPORT, [ELSEWHERE_FOLDER])).toBe(false)
    expect(await namedByATeammate(ledger, WORKTREE_REPORT)).toBe(false)
  })
})
