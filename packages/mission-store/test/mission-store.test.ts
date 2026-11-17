import {
  createCodexEventNormalizer
} from '@teammate/runtime-adapters'
import type {
  NormalizedRuntimeEvent,
  RuntimeProcessCompletion
} from '@teammate/runtime-adapters'
import { appendFile, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import {
  createFileMissionLedger,
  MISSION_LEDGER_SCHEMA_VERSION
} from '../src/index.js'
import type { MissionLedgerMetadata } from '../src/index.js'

const NOW = '2026-08-31T16:00:00.000Z'
const roots: string[] = []

async function temporaryRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'teammate-mission-ledger-'))
  roots.push(root)
  return root
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

function metadata(overrides: Partial<MissionLedgerMetadata> = {}): MissionLedgerMetadata {
  return {
    missionId: 'mission_1',
    runId: 'run_1',
    prompt: 'Inspect the workspace without changing it.',
    runtime: 'codex',
    model: 'account-default',
    requestedRouteId: 'codex',
    resolvedRouteId: 'codex-account:default',
    cliVersion: '0.151.0-alpha.7.2',
    workspaceId: 'ws_test',
    sandbox: 'read-only',
    executionPolicyVersion: 1,
    createdAt: NOW,
    ...overrides
  }
}

function completion(overrides: Partial<RuntimeProcessCompletion> = {}): RuntimeProcessCompletion {
  return {
    exitCode: 0,
    signal: null,
    stderr: '',
    stderrTruncated: false,
    recordCount: 3,
    cancelled: false,
    forcedTerminationAttempted: false,
    terminationUnconfirmed: false,
    inputDeliveryFailed: false,
    outputLimitExceeded: false,
    oversizedRecordsDropped: 0,
    startedAt: NOW,
    finishedAt: NOW,
    ...overrides
  }
}

function successfulEvents(): readonly NormalizedRuntimeEvent[] {
  const normalizer = createCodexEventNormalizer({
    runId: 'run_1',
    missionId: 'mission_1',
    requestedRouteId: 'codex',
    resolvedRouteId: 'codex-account:default',
    cliVersion: '0.151.0-alpha.7.2',
    now: () => new Date(NOW)
  })
  const events = [
    ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_1' }) }),
    ...normalizer.accept({
      sequence: 2,
      raw: JSON.stringify({
        type: 'item.completed',
        item: { id: 'answer', type: 'agent_message', text: 'Durable result' }
      })
    }),
    ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
    ...normalizer.finish(completion())
  ]
  return events
}

describe('file mission ledger', () => {
  it('persists a versioned append-only mission and recovers its terminal phase', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()

    await ledger.createMission(metadata())
    await ledger.appendEvents('mission_1', events.slice(0, 2))
    await ledger.appendEvents('mission_1', events.slice(2))
    const snapshot = await ledger.listMissions()

    expect(snapshot.issues).toEqual([])
    expect(snapshot.missions).toHaveLength(1)
    expect(snapshot.missions[0]).toMatchObject({
      metadata: metadata(),
      phase: 'completed',
      ledgerSequence: events.length + 1,
      events
    })
    const lines = (await readFile(join(root, 'mission_1.jsonl'), 'utf8')).trimEnd().split('\n')
    expect(lines).toHaveLength(events.length + 1)
    expect(JSON.parse(lines[0] ?? '{}')).toMatchObject({
      // 27, the newest the Rust app reads: only a mission on Apple Intelligence is written at 28 (0.878).
      schemaVersion: 27,
      recordType: 'mission.created',
      ledgerSequence: 1
    })
    expect(lines.map((line) => JSON.parse(line).ledgerSequence)).toEqual(
      Array.from({ length: events.length + 1 }, (_, index) => index + 1)
    )
  })

  it('serializes concurrent appends without losing ledger or event order', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()
    await ledger.createMission(metadata())

    await Promise.all([
      ledger.appendEvents('mission_1', events.slice(0, 2)),
      ledger.appendEvents('mission_1', events.slice(2))
    ])

    const recoveredLedger = createFileMissionLedger({ rootDirectory: root })
    const recovered = await recoveredLedger.getMission('mission_1')
    expect(recovered?.events.map((event) => event.sequence)).toEqual([1, 2, 3, 4])
    expect(recovered?.phase).toBe('completed')
  })

  it('rejects a mismatched or noncontiguous event before writing it', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const [first, second] = successfulEvents()
    await ledger.createMission(metadata())
    if (first === undefined || second === undefined) throw new Error('Fixture events missing')

    await expect(ledger.appendEvents('mission_1', [{ ...second, sequence: 2 }])).rejects.toThrow(
      'Mission event sequence is invalid'
    )
    await expect(ledger.appendEvents('mission_1', [{ ...first, missionId: 'mission_other' }])).rejects.toThrow(
      'Mission event correlation is invalid'
    )
    expect((await ledger.getMission('mission_1'))?.events).toEqual([])
  })

  it('ignores an incomplete crash tail and reports the recovered prefix', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()
    await ledger.createMission(metadata())
    await ledger.appendEvents('mission_1', events.slice(0, 2))
    await appendFile(join(root, 'mission_1.jsonl'), '{"schemaVersion":1,"recordType":"mission.event"', 'utf8')

    const recoveredLedger = createFileMissionLedger({ rootDirectory: root })
    const recovered = await recoveredLedger.getMission('mission_1')
    expect(recovered?.events).toHaveLength(2)
    expect(recovered?.phase).toBe('interrupted')
    expect(recovered?.issues).toContainEqual(expect.objectContaining({ code: 'truncated-tail' }))
    await expect(recoveredLedger.appendEvents('mission_1', events.slice(2))).rejects.toThrow(
      'Mission ledger is unavailable'
    )
  })

  it('records a generic host launch failure as a failed recovered mission', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata())
    await ledger.appendHostFailure('mission_1', {
      code: 'runtime-start-failed',
      message: 'The Codex process could not be started safely.',
      occurredAt: NOW
    })

    const recovered = await ledger.getMission('mission_1')
    expect(recovered?.phase).toBe('failed')
    expect(recovered?.hostFailures).toEqual([{
      code: 'runtime-start-failed',
      message: 'The Codex process could not be started safely.',
      occurredAt: NOW
    }])
  })

  it('does not trust unsupported or malformed ledger files', async () => {
    const root = await temporaryRoot()
    await writeFile(join(root, 'mission_future.jsonl'), `${JSON.stringify({
      schemaVersion: 99,
      recordType: 'mission.created',
      ledgerSequence: 1,
      metadata: metadata({ missionId: 'mission_future', runId: 'run_future' })
    })}\n`, 'utf8')
    await writeFile(join(root, 'mission_malformed.jsonl'), 'not-json\n', 'utf8')
    const ledger = createFileMissionLedger({ rootDirectory: root })

    const snapshot = await ledger.listMissions()
    expect(snapshot.missions).toEqual([])
    expect(snapshot.issues.map(({ code }) => code).sort()).toEqual([
      'invalid-record',
      'unsupported-schema'
    ])
  })
it('stops recovery at a noncontiguous body record and refuses to extend the corrupt ledger', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()
    await ledger.createMission(metadata())
    await ledger.appendEvents('mission_1', events.slice(0, 2))
    await appendFile(join(root, 'mission_1.jsonl'), `${JSON.stringify({
      schemaVersion: MISSION_LEDGER_SCHEMA_VERSION,
      recordType: 'mission.event',
      ledgerSequence: 5,
      occurredAt: NOW,
      event: events[2]
    })}
`, 'utf8')

    const recoveredLedger = createFileMissionLedger({ rootDirectory: root })
    const recovered = await recoveredLedger.getMission('mission_1')
    expect(recovered?.events).toHaveLength(2)
    expect(recovered?.ledgerSequence).toBe(3)
    expect(recovered?.issues).toContainEqual(expect.objectContaining({ code: 'invalid-record' }))
    await expect(recoveredLedger.appendEvents('mission_1', events.slice(2))).rejects.toThrow(
      'Mission ledger is unavailable'
    )
  })

  it('detects external modification on append and fails closed afterwards', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()
    await ledger.createMission(metadata())
    await ledger.appendEvents('mission_1', events.slice(0, 2))
    await appendFile(join(root, 'mission_1.jsonl'), `{"intruder":true}
`, 'utf8')

    await expect(ledger.appendEvents('mission_1', events.slice(2, 3))).rejects.toThrow(
      'Mission ledger changed outside the active writer'
    )
    // The failed append dropped its cached offsets, so the retry re-reads the
    // file, sees the foreign record, and still refuses to extend the ledger.
    await expect(ledger.appendEvents('mission_1', events.slice(2, 3))).rejects.toThrow(
      'Mission ledger is unavailable'
    )
    const recovered = await ledger.getMission('mission_1')
    expect(recovered?.events).toHaveLength(2)
    expect(recovered?.issues).toContainEqual(expect.objectContaining({ code: 'invalid-record' }))
  })

  it('orders history by recency and honors the list limit', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata({ missionId: 'mission_a', runId: 'run_a', createdAt: '2026-08-31T10:00:00.000Z' }))
    await ledger.createMission(metadata({ missionId: 'mission_b', runId: 'run_b', createdAt: '2026-08-31T12:00:00.000Z' }))
    await ledger.createMission(metadata({ missionId: 'mission_c', runId: 'run_c', createdAt: '2026-08-31T11:00:00.000Z' }))

    const limited = await ledger.listMissions({ limit: 2 })
    expect(limited.missions.map((mission) => mission.metadata.missionId)).toEqual(['mission_b', 'mission_c'])
    const all = await ledger.listMissions()
    expect(all.missions.map((mission) => mission.metadata.missionId)).toEqual(['mission_b', 'mission_c', 'mission_a'])
  })
  it('refuses an event its own reader would reject, rather than truncating recovery later', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const events = successfulEvents()
    // A message event is the one the reader constrains hardest: itemId is
    // capped at 512 characters and no persisted string may contain NUL.
    const message = events.find((event) => event.type === 'message.delta')
    if (message === undefined) throw new Error('Fixture message event missing')
    await ledger.createMission(metadata())

    // Since review 2026-10-15 (D2) a field that is only too LONG, or carries a
    // NUL, is fitted where events meet the ledger (fit.ts) rather than
    // refused: that refusal stopped whole runs. What is WRONG rather than
    // long is still refused, which is what this test is about.
    const emptyIdentity = {
      ...message,
      sequence: 1,
      payload: { ...message.payload, itemId: '   ' }
    } as typeof message
    const unknownOperation = {
      ...message,
      sequence: 1,
      payload: { ...message.payload, operation: 'prepend' }
    } as unknown as typeof message

    await expect(ledger.appendEvents('mission_1', [emptyIdentity])).rejects.toThrow(
      'Mission event is not readable by the ledger reader'
    )
    await expect(ledger.appendEvents('mission_1', [unknownOperation])).rejects.toThrow(
      'Mission event is not readable by the ledger reader'
    )

    // Nothing was written, so the mission is still appendable and still whole.
    expect((await ledger.getMission('mission_1'))?.events).toEqual([])
    await ledger.appendEvents('mission_1', events)
    const recovered = await ledger.getMission('mission_1')
    expect(recovered?.events).toHaveLength(events.length)
    expect(recovered?.phase).toBe('completed')
    expect(recovered?.issues).toEqual([])
  })
})

describe('recency-aware scanning past the file cap', () => {
  it('reads the most recently updated ledgers rather than whichever sort first by name', async () => {
    const root = await temporaryRoot()
    const { utimes, writeFile } = await import('node:fs/promises')

    // 2,001 files against a cap of 2,000 (500 until QA round 2, R28), so the selection path actually runs --
    // a handful of files would leave it unexercised and this test unable to
    // fail. Names are chosen so name order and recency order DISAGREE: the
    // wanted mission sorts LAST by name and is the newest by mtime.
    const base = Math.floor(Date.now() / 1000)
    const write = async (missionId: string, modifiedAt: number): Promise<void> => {
      const own = metadata({ missionId, runId: 'run_1' })
      const header = JSON.stringify({
        schemaVersion: MISSION_LEDGER_SCHEMA_VERSION,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: own.createdAt,
        metadata: own
      })
      const path = join(root, `${missionId}.jsonl`)
      await writeFile(path, `${header}\n`, 'utf8')
      await utimes(path, modifiedAt, modifiedAt)
    }

    await Promise.all(
      Array.from({ length: 2_000 }, (_, index) =>
        write(`mission_a${String(index).padStart(4, '0')}`, base - 10_000)
      )
    )
    await write('mission_zzz_newest', base)

    const snapshot = await createFileMissionLedger({ rootDirectory: root }).listMissions({ limit: 5 })

    // Sorting by name and slicing would drop exactly this one.
    expect(snapshot.missions.some((mission) => mission.metadata.missionId === 'mission_zzz_newest')).toBe(true)
    // And the issue has to describe the rule that was actually applied, because
    // it is what a user reads to understand an incomplete list.
    const capIssue = snapshot.issues.find((issue) => issue.code === 'file-limit-exceeded')
    expect(capIssue?.message).toContain('most recently updated')
  }, 60_000) // 2,001 files: a hosted Windows runner took 7.5 s where the default allows 5.
})

describe('deleting a mission', () => {
  it('removes the record for good and reports whether there was one', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata())

    expect(await ledger.deleteMission('mission_1')).toBe(true)
    expect(await ledger.getMission('mission_1')).toBeUndefined()
    expect((await ledger.listMissions()).missions).toEqual([])
    // Gone is gone: a second delete finds nothing, and says so plainly.
    expect(await ledger.deleteMission('mission_1')).toBe(false)
  })

  it('refuses to append to a mission that was deleted, rather than resurrecting it', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata())
    await ledger.deleteMission('mission_1')

    await expect(
      ledger.appendHostFailure('mission_1', {
        code: 'runtime-transport-failed',
        message: 'late',
        occurredAt: NOW
      })
    ).rejects.toThrow()
    expect(await ledger.getMission('mission_1')).toBeUndefined()

    // The part that is easy to get wrong: the writer caches offsets and
    // sequences per mission. If deletion left those behind, a mission created
    // again under the same id would be written against the OLD file's
    // bookkeeping and fail as "changed outside the active writer". The same
    // id must be as fresh as the first time.
    await ledger.createMission(metadata())
    await ledger.appendHostFailure('mission_1', {
      code: 'runtime-transport-failed',
      message: 'fresh',
      occurredAt: NOW
    })
    const reborn = await ledger.getMission('mission_1')
    expect(reborn?.issues).toEqual([])
    expect(reborn?.hostFailures.map((failure) => failure.message)).toEqual(['fresh'])
  })

  it('leaves every other mission untouched', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata())
    await ledger.createMission(metadata({ missionId: 'mission_2', runId: 'run_2' }))

    await ledger.deleteMission('mission_1')

    expect((await ledger.listMissions()).missions.map((m) => m.metadata.missionId)).toEqual(['mission_2'])
  })

  it('refuses an id that could name any other file', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await expect(ledger.deleteMission('../escape')).rejects.toThrow()
  })
})
