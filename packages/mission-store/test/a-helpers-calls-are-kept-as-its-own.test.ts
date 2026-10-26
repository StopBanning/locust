import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { MISSION_LEDGER_SCHEMA_VERSION, createFileMissionLedger } from '../src/index.js'
import type { MissionLedgerMetadata } from '../src/index.js'

/*
 * Ledger v22 (helper visibility, 2026-10-05). A Claude Code helper's own calls
 * are kept, each naming the helper's row as `parentItemId`. A v21 reader would
 * draw them as the teammate's own calls, so the number moved; a ledger written
 * before it must still read, whole, with no child rows.
 */

const NOW = '2026-10-05T05:00:00.000Z'
const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

const metadata = {
  missionId: 'mission_1',
  runId: 'run_1',
  prompt: 'Use a helper to count the files here.',
  runtime: 'claude',
  model: 'claude-haiku-4-5',
  requestedRouteId: 'claude',
  resolvedRouteId: 'claude-account:default',
  cliVersion: '2.1.290',
  workspaceId: 'ws_test',
  sandbox: 'workspace-write',
  executionPolicyVersion: 1,
  createdAt: NOW
}

function tool(
  sequence: number,
  type: 'tool.started' | 'tool.completed',
  itemId: string,
  name: string,
  extra: Record<string, unknown> = {}
): NormalizedRuntimeEvent {
  return {
    id: `run_1:claude:${String(sequence)}`,
    runId: 'run_1',
    missionId: 'mission_1',
    sequence,
    occurredAt: NOW,
    sourceAdapter: 'claude',
    type,
    payload: {
      itemId,
      toolKind: 'tool_use',
      name,
      phase: type === 'tool.started' ? 'started' : 'completed',
      evidence: { redacted: true },
      ...extra
    }
  } as unknown as NormalizedRuntimeEvent
}

/** The teammate's Agent call, as every ledger before v22 recorded a helper: one row, nothing under it. */
const HELPER_ONLY = [
  tool(1, 'tool.started', 'toolu_agent', 'Agent', { command: 'Count the files' }),
  tool(2, 'tool.completed', 'toolu_agent', 'Agent', { command: 'Count the files', status: 'Explore', output: 'There are 7 files.' })
]

/** The same run on v22: the helper's own Glob, between its start and its report. */
const WITH_CHILD = [
  HELPER_ONLY[0]!,
  tool(2, 'tool.started', 'toolu_glob', 'Glob', { command: '**/*', parentItemId: 'toolu_agent' }),
  tool(3, 'tool.completed', 'toolu_glob', 'Glob', { command: '**/*', parentItemId: 'toolu_agent', output: 'a.txt\nb.txt' }),
  { ...HELPER_ONLY[1]!, id: 'run_1:claude:4', sequence: 4 } as NormalizedRuntimeEvent
]

async function rawLedger(schemaVersion: number, events: readonly NormalizedRuntimeEvent[]): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'teammate-helper-'))
  roots.push(root)
  const lines = [
    JSON.stringify({ schemaVersion, recordType: 'mission.created', ledgerSequence: 1, occurredAt: NOW, metadata }),
    ...events.map((event, index) =>
      JSON.stringify({ schemaVersion, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: NOW, event })
    )
  ]
  await writeFile(join(root, 'mission_1.jsonl'), `${lines.join('\n')}\n`, 'utf8')
  return root
}

const childrenOf = (events: readonly NormalizedRuntimeEvent[]): readonly string[] =>
  events.flatMap((event) => {
    const parent = (event.payload as { readonly parentItemId?: unknown }).parentItemId
    return typeof parent === 'string' ? [`${event.type} ${(event.payload as { readonly itemId: string }).itemId} under ${parent}`] : []
  })

describe("a helper's calls in the ledger (v22)", () => {
  it('reads a ledger written before the bump whole, with no child rows and no error', async () => {
    const root = await rawLedger(21, HELPER_ONLY)
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.issues).toEqual([])
    expect(recovered?.events).toHaveLength(2)
    expect(childrenOf(recovered?.events ?? [])).toEqual([])
  }, 20_000)

  it("writes a helper's calls at v22 and reads them back under the helper's row", async () => {
    const root = await mkdtemp(join(tmpdir(), 'teammate-helper-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata as unknown as MissionLedgerMetadata)
    await ledger.appendEvents('mission_1', WITH_CHILD)
    await ledger.flush()
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    // Written at whatever is current; v22 is the first that keeps a helper's calls.
    expect(MISSION_LEDGER_SCHEMA_VERSION).toBe(27)
    expect(recovered?.schemaVersion).toBe(27)
    expect(recovered?.issues).toEqual([])
    expect(childrenOf(recovered?.events ?? [])).toEqual([
      'tool.started toolu_glob under toolu_agent',
      'tool.completed toolu_glob under toolu_agent'
    ])
  }, 20_000)

  it('refuses a parent that names nothing, so a reader never has to guess at one', async () => {
    const root = await mkdtemp(join(tmpdir(), 'teammate-helper-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata as unknown as MissionLedgerMetadata)
    await expect(
      ledger.appendEvents('mission_1', [tool(1, 'tool.started', 'toolu_glob', 'Glob', { parentItemId: '' })])
    ).rejects.toThrow('not readable by the ledger reader')
  }, 20_000)
})
