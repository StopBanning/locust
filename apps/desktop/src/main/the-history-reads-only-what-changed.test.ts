import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'
import { createFileMissionLedger } from '@teammate/mission-store'
import type { MissionLedger, MissionLedgerMetadata } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { readMissionHistory } from './mission-history.js'

/**
 * THE HISTORY READS ONLY WHAT CHANGED.
 *
 * Every run end asked for the whole history and every ledger file was parsed
 * again: 490-560 ms on Colin's 138-mission ledger, measured in the app
 * (2026-09-22), with the main process slow to answer anything else. The
 * files are kept by size and modified time; an unchanged one is not read.
 *
 * The whole point is that nothing a person sees changes, so the first test
 * holds the kept answer to the old one field for field.
 */
const roots: string[] = []
afterEach(async () => {
  // Only directories this file made with mkdtemp.
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

const at = (minute: number): string => new Date(Date.UTC(2026, 8, 22, 12, minute)).toISOString()

function metadata(index: number): MissionLedgerMetadata {
  return {
    missionId: `mission_${String(index).padStart(3, '0')}`,
    runId: `run_${String(index)}`,
    prompt: `Question ${String(index)}`,
    runtime: 'opencode',
    model: 'opencode/muse-spark-1.3-contributor-free',
    requestedRouteId: 'opencode',
    resolvedRouteId: 'opencode-account:default',
    cliVersion: 'test',
    workspaceId: 'ws_test',
    sandbox: 'read-only',
    executionPolicyVersion: 1,
    createdAt: at(index)
  }
}

function answer(index: number, sequence: number, minute: number): NormalizedRuntimeEvent {
  return {
    id: `event_${String(index)}_${String(sequence)}`,
    runId: `run_${String(index)}`,
    missionId: `mission_${String(index).padStart(3, '0')}`,
    sequence,
    occurredAt: at(minute),
    sourceAdapter: 'opencode',
    type: 'message.delta',
    payload: { itemId: `answer_${String(index)}`, operation: 'append', text: `part ${String(sequence)} `, final: false, evidence: { redacted: true } }
  } as unknown as NormalizedRuntimeEvent
}

/** Twenty-five missions: more than the newest twenty that carry events. */
async function ledgerWithMissions(): Promise<MissionLedger> {
  const root = await mkdtemp(join(tmpdir(), 'locust-history-cache-'))
  roots.push(root)
  const ledger = createFileMissionLedger({ rootDirectory: root })
  for (let index = 1; index <= 25; index += 1) {
    await ledger.createMission(metadata(index))
    await ledger.appendEvents(metadata(index).missionId, [answer(index, 1, index), answer(index, 2, index)])
  }
  await ledger.flush()
  return ledger
}

/** Count the files the next history read opens. */
function countReads(ledger: MissionLedger): { readonly ids: string[] } {
  const original = ledger.readMission
  if (original === undefined) throw new Error('this ledger cannot read one mission')
  const seen = { ids: [] as string[] }
  ;(ledger as { readMission: NonNullable<MissionLedger['readMission']> }).readMission = (missionId: string) => {
    seen.ids.push(missionId)
    return original.call(ledger, missionId)
  }
  return seen
}

/** The same ledger with the new methods hidden: the old whole-listing path. */
function listingOnly(ledger: MissionLedger): MissionLedger {
  return {
    ...ledger,
    missionFiles: undefined,
    readMission: undefined,
    listMissions: ledger.listMissions.bind(ledger),
    getMission: ledger.getMission.bind(ledger)
  } as unknown as MissionLedger
}

describe('the history reads only what changed', () => {
  it('answers exactly what the whole listing answered', async () => {
    const ledger = await ledgerWithMissions()
    const kept = await readMissionHistory(ledger, undefined, 'C:\\work')
    const whole = await readMissionHistory(listingOnly(ledger), undefined, 'C:\\work')
    expect(kept.ok && whole.ok).toBe(true)
    if (!kept.ok || !whole.ok) return
    expect(kept.data).toEqual(whole.data)
    // And the premise: some rows came WITHOUT events, and still count them.
    const light = kept.data.missions.filter((mission) => mission.events.length === 0)
    expect(light.length).toBeGreaterThan(0)
    expect(light.every((mission) => mission.eventCount > 0)).toBe(true)
  }, 30_000)

  it('reads no file twice when nothing changed', async () => {
    const ledger = await ledgerWithMissions()
    await readMissionHistory(ledger, undefined, 'C:\\work')
    const read = countReads(ledger)
    await readMissionHistory(ledger, undefined, 'C:\\work')
    expect(read.ids).toEqual([])
  }, 30_000)

  it('reads the one file that changed, and the answer carries the change', async () => {
    const ledger = await ledgerWithMissions()
    await readMissionHistory(ledger, undefined, 'C:\\work')
    const read = countReads(ledger)
    await ledger.appendEvents('mission_003', [answer(3, 3, 59)])
    await ledger.flush()
    const after = await readMissionHistory(ledger, undefined, 'C:\\work')
    expect(read.ids).toEqual(['mission_003'])
    // Its newest turn moved it to the top, with its third event on it.
    expect(after.ok && after.data.missions[0]?.missionId).toBe('mission_003')
    expect(after.ok && after.data.missions[0]?.eventCount).toBe(3)
  }, 30_000)
})

 describe('an old question still needs an answer', () => {
  it('keeps its question on light history rows on the first and cached reads', async () => {
    const ledger = await ledgerWithMissions()
    const ask = { ...answer(1, 3, 1), payload: { itemId: 'ask', operation: 'append', final: true, evidence: { redacted: true }, text: '<locust-ask>\nWhich database?\n- SQLite :: local\n- Postgres :: server\n</locust-ask>' } } as NormalizedRuntimeEvent
    const done = { ...answer(1, 4, 1), type: 'run.completed', payload: { process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 1, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at(1), finishedAt: at(1) } } } as NormalizedRuntimeEvent
    await ledger.appendEvents(metadata(1).missionId, [ask, done])
    await ledger.flush()
    for (let read = 0; read < 2; read++) {
      const result = await readMissionHistory(ledger, undefined, '/work')
      expect(result.ok).toBe(true)
      if (!result.ok) return
      const old = result.data.missions.find((mission) => mission.missionId === metadata(1).missionId)
      expect(old?.events).toEqual([])
      expect(old?.waitingQuestion).toBe('Which database?')
    }
  })
})
