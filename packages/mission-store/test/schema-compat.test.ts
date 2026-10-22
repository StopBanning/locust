import { createCodexEventNormalizer } from '@teammate/runtime-adapters'
import type { NormalizedRuntimeEvent, RuntimeProcessCompletion } from '@teammate/runtime-adapters'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import {
  createFileMissionLedger,
  MISSION_LEDGER_SCHEMA_VERSION,
  SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS
} from '../src/index.js'
import type { MissionLedgerMetadata } from '../src/index.js'

const NOW = '2026-08-31T16:00:00.000Z'
const roots: string[] = []

async function temporaryRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'teammate-schema-'))
  roots.push(root)
  return root
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

function v1Metadata(overrides: Record<string, unknown> = {}): Record<string, unknown> {
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

function completion(): RuntimeProcessCompletion {
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
    startedAt: NOW,
    finishedAt: NOW
  }
}

function codexEvents(): readonly NormalizedRuntimeEvent[] {
  const codex = createCodexEventNormalizer({
    runId: 'run_1',
    missionId: 'mission_1',
    requestedRouteId: 'codex',
    resolvedRouteId: 'codex-account:default',
    cliVersion: '0.151.0-alpha.7.2',
    now: () => new Date(NOW)
  })
  return [
    ...codex.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_1' }) }),
    ...codex.accept({
      sequence: 2,
      raw: JSON.stringify({
        type: 'item.completed',
        item: { id: 'answer', type: 'agent_message', text: 'Durable result' }
      })
    }),
    ...codex.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
    ...codex.finish(completion())
  ]
}

/** A ledger file exactly as version 1 of this package wrote them. */
async function writeV1Ledger(
  root: string,
  events: readonly NormalizedRuntimeEvent[],
  metadataOverrides: Record<string, unknown> = {}
): Promise<string> {
  const metadata = v1Metadata(metadataOverrides)
  const lines = [
    JSON.stringify({
      schemaVersion: 1,
      recordType: 'mission.created',
      ledgerSequence: 1,
      occurredAt: metadata.createdAt,
      metadata
    }),
    ...events.map((event, index) =>
      JSON.stringify({
        schemaVersion: 1,
        recordType: 'mission.event',
        ledgerSequence: index + 2,
        occurredAt: event.occurredAt,
        event
      })
    )
  ]
  const path = join(root, 'mission_1.jsonl')
  await writeFile(path, `${lines.join('\n')}\n`, 'utf8')
  return path
}

describe('ledger schema versions', () => {
  it('writes new missions at the current version', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata()
    } as unknown as MissionLedgerMetadata)

    const header = JSON.parse((await readFile(join(root, 'mission_1.jsonl'), 'utf8')).split('\n')[0] ?? '{}')

    // v17 added the person's check result after a turn (mission.edit_check);
    // v18 the terminal starter, a turn brought back from the runtime's own
    // terminal (0.391); v19 the side starter, a question asked on a fork of
    // a conversation's session (0.461); v20 the cards a person answered
    // (mission.approval, 0.576); v21 an answer by the person's own Always on an
    // earlier card of the run (0.616); v22 a tool call made by a helper the
    // teammate sent out (parentItemId, helper visibility); v23 a turn another
    // app asked for (mcp); v24 a turn carried on after a plan ran out (0.774); v25 an agent the person added (0.777);
    // v26 a failing check sent back to fix (0.812).
    expect(MISSION_LEDGER_SCHEMA_VERSION).toBe(26)
    expect(header.schemaVersion).toBe(26)
    expect(SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26])
    /*
     * THE PAIR THAT DRIFTED, checked as a pair.
     *
     * `isSupportedSchemaVersion` was sixteen hand-written `value === n`
     * clauses beside this array, and bumping the array alone left the reader
     * refusing the writer's own files. Reading every supported version back
     * through a real file is the only check that would have caught it.
     */
    expect(SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS).toContain(MISSION_LEDGER_SCHEMA_VERSION)
  })

  it('still recovers a mission recorded before the version bump', async () => {
    const root = await temporaryRoot()
    const events = codexEvents()
    await writeV1Ledger(root, events)

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    // The whole point of the compatibility list: a version bump must not read
    // to a user as their history disappearing.
    expect(recovered?.issues).toEqual([])
    expect(recovered?.phase).toBe('completed')
    expect(recovered?.events).toHaveLength(events.length)
    expect(recovered?.metadata.runtime).toBe('codex')
  })

  it('appends to a version-1 mission in version 1, keeping the file walkable', async () => {
    const root = await temporaryRoot()
    const events = codexEvents()
    await writeV1Ledger(root, events.slice(0, 2))

    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.appendEvents('mission_1', events.slice(2))

    const lines = (await readFile(join(root, 'mission_1.jsonl'), 'utf8')).trimEnd().split('\n')
    // A v2 record inside a v1 file would stop recovery at the first one, so the
    // appended half of the mission would vanish on the next launch.
    expect(lines.map((line) => JSON.parse(line).schemaVersion)).toEqual(lines.map(() => 1))

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.issues).toEqual([])
    expect(recovered?.events).toHaveLength(events.length)
    expect(recovered?.phase).toBe('completed')
  })

  it('stops recovery when a record disagrees with its file version', async () => {
    const root = await temporaryRoot()
    const events = codexEvents()
    await writeV1Ledger(root, events)
    const lines = (await readFile(join(root, 'mission_1.jsonl'), 'utf8')).trimEnd().split('\n')
    const third = JSON.parse(lines[2] ?? '{}')
    third.schemaVersion = 2
    lines[2] = JSON.stringify(third)
    await writeFile(join(root, 'mission_1.jsonl'), `${lines.join('\n')}\n`, 'utf8')

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.events).toHaveLength(1)
    expect(recovered?.issues.map((issue) => issue.code)).toContain('invalid-record')
  })

  it('refuses a version-1 file describing a runtime version 1 could not write', async () => {
    const root = await temporaryRoot()
    await writeV1Ledger(root, [], { runtime: 'claude' })

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered).toBeUndefined()
  })

  it('refuses a pre-v4 file claiming to continue another mission', async () => {
    const root = await temporaryRoot()
    // No writer before v4 could produce a continuation, so a v1 file carrying
    // one was hand-edited. Reading it would let an invented link decide which
    // missions the UI stitches together as one piece of work.
    await writeV1Ledger(root, [], {
      continuesFrom: { missionId: 'mission_other', checkpointEpoch: 1, reason: 'route-switch' }
    })

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered).toBeUndefined()
  })

  it('round-trips a continuation through the durable record', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata({
        continuesFrom: { missionId: 'mission_before', checkpointEpoch: 2, reason: 'route-switch' }
      })
    } as unknown as MissionLedgerMetadata)

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.continuesFrom).toEqual({
      missionId: 'mission_before',
      checkpointEpoch: 2,
      reason: 'route-switch'
    })
  })

  // 0.519: what a handoff's brief left out, kept so the divider can say it after a reopen.
  it('round-trips what a handoff left out, and refuses a malformed list', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata({
        continuesFrom: { missionId: 'mission_before', checkpointEpoch: 2, reason: 'route-switch', leftOut: ['summary', 'earlier'] }
      })
    } as unknown as MissionLedgerMetadata)
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.continuesFrom?.leftOut).toEqual(['summary', 'earlier'])

    const other = createFileMissionLedger({ rootDirectory: await temporaryRoot() })
    await expect(other.createMission({
      ...v1Metadata({ continuesFrom: { missionId: 'mission_before', checkpointEpoch: 2, reason: 'route-switch', leftOut: ['x'.repeat(41)] } })
    } as unknown as MissionLedgerMetadata)).rejects.toThrow()
  })

  it('refuses a continuation that names an unsafe mission id', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })

    // A path-traversing id in a continuation is the same hazard as one in a
    // mission id: it decides which file a reader is pointed at.
    await expect(ledger.createMission({
      ...v1Metadata({
        continuesFrom: { missionId: '../escape', checkpointEpoch: 1, reason: 'route-switch' }
      })
    } as unknown as MissionLedgerMetadata)).rejects.toThrow()
  })

  it('refuses a continuation with an impossible checkpoint epoch', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })

    // Epochs start at 1. A zero would point at a checkpoint that never existed.
    await expect(ledger.createMission({
      ...v1Metadata({
        continuesFrom: { missionId: 'mission_before', checkpointEpoch: 0, reason: 'route-switch' }
      })
    } as unknown as MissionLedgerMetadata)).rejects.toThrow()
  })

  it('round-trips a peer link through the durable record', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({ ...v1Metadata() } as unknown as MissionLedgerMetadata)
    await ledger.appendPeerLinks('mission_1', [
      { direction: 'received', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW },
      { direction: 'posted', messageId: 'wm_2', peerTeammateId: 'tm_atlas', occurredAt: NOW }
    ])

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.peerLinks).toEqual([
      { direction: 'received', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW },
      { direction: 'posted', messageId: 'wm_2', peerTeammateId: 'tm_atlas', occurredAt: NOW }
    ])
    expect(recovered?.ledgerSequence).toBe(3)
  })

  it('refuses a peer link in a file written before version 5', async () => {
    const root = await temporaryRoot()
    // No writer before v5 could produce one. A v1 file carrying a peer link
    // was hand-edited, and reading it would let an invented cross-reference
    // decide which workroom message a mission is shown as having received.
    await writeV1Ledger(root, [])
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 1,
        recordType: 'mission.peer',
        ledgerSequence: 2,
        occurredAt: NOW,
        link: { direction: 'received', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW }
      })}\n`,
      { flag: 'a' }
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.peerLinks).toEqual([])
    expect(recovered?.issues.map((issue) => issue.code)).toEqual(['invalid-record'])
  })

  it('refuses to append a peer link to a mission written before version 5', async () => {
    const root = await temporaryRoot()
    await writeV1Ledger(root, [])
    const ledger = createFileMissionLedger({ rootDirectory: root })

    // The file's version is fixed by its header; a record its own readers stop
    // at must not be written into it.
    await expect(ledger.appendPeerLinks('mission_1', [
      { direction: 'received', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW }
    ])).rejects.toThrow('cannot hold peer links')
  })

  it('refuses a peer link with an unsafe id or an unknown direction', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({ ...v1Metadata() } as unknown as MissionLedgerMetadata)

    await expect(ledger.appendPeerLinks('mission_1', [
      { direction: 'received', messageId: '../escape', peerTeammateId: 'tm_atlas', occurredAt: NOW }
    ])).rejects.toThrow()
    await expect(ledger.appendPeerLinks('mission_1', [
      { direction: 'broadcast' as 'posted', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW }
    ])).rejects.toThrow()
  })

  it('round-trips a follow-up continuation, session handle and all', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata({
        continuesFrom: {
          missionId: 'mission_before',
          checkpointEpoch: 1,
          reason: 'follow-up',
          runtimeThreadId: 'session-abc'
        }
      })
    } as unknown as MissionLedgerMetadata)

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.continuesFrom).toEqual({
      missionId: 'mission_before',
      checkpointEpoch: 1,
      reason: 'follow-up',
      runtimeThreadId: 'session-abc'
    })
  })

  it('refuses a follow-up in a file written before version 6', async () => {
    const root = await temporaryRoot()
    // A v5 file, deliberately: v4 and v5 may carry a continuation, so the
    // only thing standing between this file and a reader is the v6 rule. (A
    // v1 file would be refused for carrying ANY continuation, which would
    // let this test pass with the v6 rule deleted.) No writer before v6
    // could call a continuation a follow-up, so this one was hand-edited,
    // and believing it would draw an ordinary reply as though it continued
    // some other conversation.
    const metadata = v1Metadata({
      continuesFrom: { missionId: 'mission_other', checkpointEpoch: 1, reason: 'follow-up' }
    })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 5,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered).toBeUndefined()
  })

  it('refuses a Gemini mission in a file written before version 7', async () => {
    const root = await temporaryRoot()
    // A v6 file: every other rule accepts it, so only the v7 rule can refuse
    // it. No writer before v7 knew Gemini, so this header was hand-edited.
    const metadata = v1Metadata({ runtime: 'gemini', model: 'gemini-3-flash' })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 6,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}\n`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips who started a run at the current version', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(
      v1Metadata({ startedBy: { kind: 'relay', hop: 2 } }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.startedBy).toEqual({ kind: 'relay', hop: 2 })
  })

  it('refuses a host-started run in a file written before version 10', async () => {
    const root = await temporaryRoot()
    // A v9 file, deliberately: every other rule accepts it, so only the v10
    // rule can refuse it. No writer before v10 could say a run was started by
    // anything but a person, so this one was hand-edited -- and believing it
    // would let a file rename a mission using text it supplied.
    const metadata = v1Metadata({ startedBy: { kind: 'relay', hop: 1 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 9,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('refuses a starter it cannot read rather than dropping the field', async () => {
    const root = await temporaryRoot()
    // hop 0 is not a hop. A record whose starter is malformed must be refused
    // outright: quietly ignoring it would present a host-started run as one a
    // person began, which is the exact confusion the field exists to end.
    const metadata = v1Metadata({ startedBy: { kind: 'relay', hop: 0 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 10,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips a routine starter at the current version', async () => {
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ startedBy: { kind: 'routine', routineId: 'rt_abc', step: 2 } }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.startedBy).toEqual({ kind: 'routine', routineId: 'rt_abc', step: 2 })
  })

  it('round-trips a terminal starter at the current version (v18)', async () => {
    // A turn the person had in the runtime's own terminal, brought back
    // (docs/PLAN-TERMINAL-CATCH-UP-2026-09-27.md).
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ startedBy: { kind: 'terminal', exchange: 2 } }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.startedBy).toEqual({ kind: 'terminal', exchange: 2 })
  })

  it('refuses a terminal starter whose exchange is below 1', async () => {
    const root = await temporaryRoot()
    await expect(
      createFileMissionLedger({ rootDirectory: root }).createMission(
        v1Metadata({ startedBy: { kind: 'terminal', exchange: 0 } }) as unknown as MissionLedgerMetadata
      )
    ).rejects.toThrow()
  })

  it('refuses a terminal starter in a file written before version 18', async () => {
    const root = await temporaryRoot()
    // No writer before v18 brought a terminal's turns back.
    const metadata = v1Metadata({ startedBy: { kind: 'terminal', exchange: 1 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({ schemaVersion: 17, recordType: 'mission.created', ledgerSequence: 1, occurredAt: metadata.createdAt, metadata })}\n`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips a side starter at the current version (v19)', async () => {
    // A question asked on a fork of a conversation's session (0.461).
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ startedBy: { kind: 'side', of: 'mission_main', question: 2 } }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.startedBy).toEqual({ kind: 'side', of: 'mission_main', question: 2 })
  })

  it('refuses a side starter with no conversation, or a question below 1', async () => {
    const ledger = createFileMissionLedger({ rootDirectory: await temporaryRoot() })
    await expect(ledger.createMission(v1Metadata({ startedBy: { kind: 'side', of: '', question: 1 } }) as unknown as MissionLedgerMetadata)).rejects.toThrow()
    await expect(ledger.createMission(v1Metadata({ startedBy: { kind: 'side', of: 'mission_main', question: 0 } }) as unknown as MissionLedgerMetadata)).rejects.toThrow()
  })

  it('refuses a side starter in a file written before version 19', async () => {
    const root = await temporaryRoot()
    const metadata = v1Metadata({ startedBy: { kind: 'side', of: 'mission_main', question: 1 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({ schemaVersion: 18, recordType: 'mission.created', ledgerSequence: 1, occurredAt: metadata.createdAt, metadata })}\n`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('refuses a routine starter with no routine id or a step below 1', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await expect(
      ledger.createMission(v1Metadata({ startedBy: { kind: 'routine', routineId: '', step: 1 } }) as unknown as MissionLedgerMetadata)
    ).rejects.toThrow()
    await expect(
      ledger.createMission(v1Metadata({ startedBy: { kind: 'routine', routineId: 'rt_abc', step: 0 } }) as unknown as MissionLedgerMetadata)
    ).rejects.toThrow()
  })

  it('round-trips the mode a run was asked for, beside what it was allowed', async () => {
    // The record has always said what a run was ALLOWED and never what was
    // ASKED FOR, and they are not the same question: `ask` and `plan` are
    // both read-only, so a plan reopened after a restart came back as an
    // ordinary read-only run (QA, 2026-09-06).
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ sandbox: 'read-only', mode: 'plan' }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.mode).toBe('plan')
    expect(recovered?.metadata.sandbox).toBe('read-only')
  })

  it('reads a mission with no mode as one, rather than inventing a default', async () => {
    // Every mission written before 15 is in this position, and guessing `ask`
    // would put a word in the record that nobody chose.
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ sandbox: 'read-only' }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.mode).toBeUndefined()
  })

  it('refuses a mode in a file written before version 15', async () => {
    // A v14 reader would drop it and be wrong about what the run was for,
    // quietly -- the same reason every earlier widening moved the number.
    const root = await temporaryRoot()
    const metadata = v1Metadata({ sandbox: 'read-only', mode: 'plan' })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 14,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('keeps a mission on an agent the person added, and refuses one in a file written before version 25', async () => {
    // 0.777: `acp` is any program speaking the Agent Client Protocol, by the person's own command line.
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ runtime: 'acp', model: 'acp_goose', sandbox: 'workspace-write', mode: 'approve-each' }) as unknown as MissionLedgerMetadata
    )
    const kept = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(kept?.metadata.runtime).toBe('acp')
    expect(kept?.schemaVersion).toBe(MISSION_LEDGER_SCHEMA_VERSION)

    const older = await temporaryRoot()
    const metadata = v1Metadata({ runtime: 'acp', model: 'acp_goose', sandbox: 'workspace-write' })
    await writeFile(
      join(older, 'mission_1.jsonl'),
      `${JSON.stringify({ schemaVersion: 24, recordType: 'mission.created', ledgerSequence: 1, occurredAt: metadata.createdAt, metadata })}
`,
      'utf8'
    )
    expect(await createFileMissionLedger({ rootDirectory: older }).getMission('mission_1')).toBeUndefined()
  })

  it('keeps a turn Locust sent back to fix a failing check, and refuses one in a file written before version 26', async () => {
    // 0.812: nobody typed it -- the person's check failed after the turn before, and the folder said to send it back.
    const starter = { kind: 'check-fix', fix: 1, of: 2, command: 'npm test' }
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ sandbox: 'workspace-write', mode: 'accept-edits', startedBy: starter }) as unknown as MissionLedgerMetadata
    )
    const kept = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(kept?.metadata.startedBy).toEqual(starter)
    expect(kept?.schemaVersion).toBe(26)

    const older = await temporaryRoot()
    const metadata = v1Metadata({ sandbox: 'workspace-write', startedBy: starter })
    await writeFile(
      join(older, 'mission_1.jsonl'),
      `${JSON.stringify({ schemaVersion: 25, recordType: 'mission.created', ledgerSequence: 1, occurredAt: metadata.createdAt, metadata })}
`,
      'utf8'
    )
    expect(await createFileMissionLedger({ rootDirectory: older }).getMission('mission_1')).toBeUndefined()

    // A fix counts from 1, is one of at most five, never past its own most, and names its check.
    for (const bad of [{ ...starter, fix: 0 }, { ...starter, of: 6 }, { ...starter, fix: 3, of: 2 }, { ...starter, command: '' }, { ...starter, command: 'x'.repeat(501) }]) {
      await expect(
        createFileMissionLedger({ rootDirectory: await temporaryRoot() }).createMission(
          v1Metadata({ sandbox: 'workspace-write', startedBy: bad }) as unknown as MissionLedgerMetadata
        )
      ).rejects.toThrow(/starter|command/i)
    }
  })

  it('refuses a mode that is not one of the five', async () => {
    const root = await temporaryRoot()
    await expect(
      createFileMissionLedger({ rootDirectory: root }).createMission(
        v1Metadata({ mode: 'whatever' }) as unknown as MissionLedgerMetadata
      )
    ).rejects.toThrow(/mode/i)
  })

  it('round-trips a full-access mission at the current version', async () => {
    // The Auto mode a person switches on: a run that was not confined to the
    // workspace folder. The permission a run had is the last thing a record
    // may be vague about, so it is written and read as its own word.
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ sandbox: 'full-access' }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.sandbox).toBe('full-access')
  })

  it('refuses a full-access mission in a file written before version 14', async () => {
    // A v13 file knew read-only and workspace-write. Letting a full-access
    // mission through under that number would have an older reader draw a run
    // that could touch the whole machine as one confined to a folder.
    const root = await temporaryRoot()
    const metadata = v1Metadata({ sandbox: 'full-access' })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 13,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('refuses a routine starter in a file written before version 13', async () => {
    const root = await temporaryRoot()
    // A v12 file knew relay and resume; a routine starter there was hand-edited.
    const metadata = v1Metadata({ startedBy: { kind: 'routine', routineId: 'rt_abc', step: 1 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 12,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips a resume starter at the current version', async () => {
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({ startedBy: { kind: 'resume', epoch: 3 } }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.startedBy).toEqual({ kind: 'resume', epoch: 3 })
  })

  it('refuses a resume starter in a file written before version 11', async () => {
    const root = await temporaryRoot()
    // A v10 file: it knew `startedBy`, but `relay` was the only kind it could
    // write. A resume starter there was hand-edited.
    const metadata = v1Metadata({ startedBy: { kind: 'resume', epoch: 1 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 10,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('refuses a resume starter whose epoch is not an epoch', async () => {
    const root = await temporaryRoot()
    const metadata = v1Metadata({ startedBy: { kind: 'resume', epoch: 0 } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 11,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips the command the host ran', async () => {
    const root = await temporaryRoot()
    await createFileMissionLedger({ rootDirectory: root }).createMission(
      v1Metadata({
        command: { executablePath: 'C:\cmd.exe', args: ['-p', '<prompt>', '--no-color'] }
      }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.command?.args).toEqual(['-p', '<prompt>', '--no-color'])
  })

  it('refuses a command in a file written before version 12', async () => {
    const root = await temporaryRoot()
    const metadata = v1Metadata({ command: { executablePath: 'C:\cmd.exe', args: [] } })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 11,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('refuses a command carrying an argument it cannot vouch for', async () => {
    const root = await temporaryRoot()
    // A NUL in an argument is the kind of byte that rides into every reader of
    // the file. Refused outright rather than cleaned, like every other record.
    const metadata = v1Metadata({
      command: { executablePath: 'C:\cmd.exe', args: [`--flag${String.fromCharCode(0)}`] }
    })
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        schemaVersion: 12,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}
`,
      'utf8'
    )

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('round-trips a Cursor Agent mission at the current version', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(
      v1Metadata({ runtime: 'cursor', model: 'composer-1' }) as unknown as MissionLedgerMetadata
    )

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.runtime).toBe('cursor')
  })

  it('refuses a continuation reason it does not know', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await expect(ledger.createMission({
      ...v1Metadata({
        continuesFrom: { missionId: 'mission_before', checkpointEpoch: 1, reason: 'guessed' }
      })
    } as unknown as MissionLedgerMetadata)).rejects.toThrow()
  })

  it('rejects a file whose version this reader does not know', async () => {
    const root = await temporaryRoot()
    const metadata = v1Metadata()
    await writeFile(
      join(root, 'mission_1.jsonl'),
      `${JSON.stringify({
        // One past the newest this reader knows. Bump when the schema does.
        schemaVersion: 27,
        recordType: 'mission.created',
        ledgerSequence: 1,
        occurredAt: metadata.createdAt,
        metadata
      })}\n`,
      'utf8'
    )

    const snapshot = await createFileMissionLedger({ rootDirectory: root }).listMissions()

    expect(snapshot.issues.map((issue) => issue.code)).toContain('unsupported-schema')
    expect(snapshot.missions).toEqual([])
  })
})

describe('runtime agreement', () => {
  it('accepts a Claude mission and its Claude events', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata({ runtime: 'claude', model: 'claude-sonnet-5', requestedRouteId: 'claude' })
    } as unknown as MissionLedgerMetadata)

    const claudeEvent = {
      ...codexEvents()[0],
      sourceAdapter: 'claude'
    } as unknown as NormalizedRuntimeEvent
    await ledger.appendEvents('mission_1', [claudeEvent])

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.issues).toEqual([])
    expect(recovered?.metadata.runtime).toBe('claude')
    expect(recovered?.metadata.model).toBe('claude-sonnet-5')
    expect(recovered?.events).toHaveLength(1)
  })

  it('refuses an event from a runtime the mission is not running', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({ ...v1Metadata() } as unknown as MissionLedgerMetadata)

    const foreign = {
      ...codexEvents()[0],
      sourceAdapter: 'claude'
    } as unknown as NormalizedRuntimeEvent

    // A Claude event inside a Codex mission means one of the two records is
    // wrong, and a ledger that accepts both can no longer say which.
    await expect(ledger.appendEvents('mission_1', [foreign])).rejects.toThrow(
      /not readable by the ledger reader/
    )
  })
})

describe('sandbox widening (v3)', () => {
  it('records a workspace-write mission at the current version', async () => {
    const root = await temporaryRoot()
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      ...v1Metadata({ sandbox: 'workspace-write' })
    } as unknown as MissionLedgerMetadata)

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.metadata.sandbox).toBe('workspace-write')
  })

  it('refuses a pre-v3 file claiming a mission was allowed to write', async () => {
    const root = await temporaryRoot()
    // Versions before 3 could only record read-only runs, so this file was
    // hand-edited. Believing it would render a write run's permissions wrongly.
    await writeV1Ledger(root, [], { sandbox: 'workspace-write' })

    expect(await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')).toBeUndefined()
  })

  it('still reads v1 and v2 read-only missions', async () => {
    const root = await temporaryRoot()
    await writeV1Ledger(root, [])
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.metadata.sandbox).toBe('read-only')
  })
})
