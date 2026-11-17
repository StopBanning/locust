import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { MISSION_LEDGER_SCHEMA_VERSION, boundedApproval, createFileMissionLedger } from '../src/index.js'
import type { MissionApproval, MissionLedgerMetadata } from '../src/index.js'

/*
 * 0.576, ledger v20. Saving a conversation's record (0.575) found the ledger
 * held no approval at all: a declined call kept its own words, and an allowed
 * one left no trace. The host now writes each card the person answered, and
 * how, onto the mission it was asked in.
 */

const NOW = '2026-10-04T01:00:00.000Z'
const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

async function ledgerWithMission(schemaVersion?: number): Promise<{ root: string; ledger: ReturnType<typeof createFileMissionLedger> }> {
  const root = await mkdtemp(join(tmpdir(), 'teammate-approval-'))
  roots.push(root)
  const metadata = {
    missionId: 'mission_1',
    runId: 'run_1',
    prompt: 'Tidy the build folder.',
    runtime: 'codex',
    model: 'account-default',
    requestedRouteId: 'codex',
    resolvedRouteId: 'codex-account:default',
    cliVersion: '0.160.0',
    workspaceId: 'ws_test',
    sandbox: 'workspace-write',
    executionPolicyVersion: 1,
    createdAt: NOW
  }
  if (schemaVersion === undefined) {
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission(metadata as unknown as MissionLedgerMetadata)
    return { root, ledger }
  }
  await writeFile(join(root, 'mission_1.jsonl'), `${JSON.stringify({ schemaVersion, recordType: 'mission.created', ledgerSequence: 1, occurredAt: NOW, metadata })}\n`, 'utf8')
  return { root, ledger: createFileMissionLedger({ rootDirectory: root }) }
}

const ALLOWED: MissionApproval = {
  approvalId: 'ap_1',
  kind: 'command',
  asked: 'Run a command\nnpm test',
  answer: 'allowed',
  by: 'card',
  askedAt: '2026-10-04T00:59:50.000Z',
  occurredAt: NOW
}
const DENIED_BY_RULE: MissionApproval = {
  approvalId: 'ap_2',
  kind: 'command',
  asked: 'Run a command\ngit push origin main',
  answer: 'denied',
  by: 'saved-rule',
  words: 'never run git push for Wren in this folder.',
  askedAt: '2026-10-04T01:00:10.000Z',
  occurredAt: '2026-10-04T01:00:11.000Z'
}

describe('a card the person answered is kept on its mission', () => {
  it('comes back from the file exactly as written, in order, with the schema version that holds it', async () => {
    const { root, ledger } = await ledgerWithMission()
    await ledger.appendApproval('mission_1', ALLOWED)
    await ledger.appendApproval('mission_1', DENIED_BY_RULE)
    await ledger.flush()

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.approvals).toEqual([ALLOWED, DENIED_BY_RULE])
    // Written at 27, the newest the Rust app reads: only a mission on Apple Intelligence is written at 28 (0.878).
    expect(recovered?.schemaVersion).toBe(27)
    expect(recovered?.issues).toEqual([])
  }, 20_000)

  it('sits between a run\'s events without disturbing their numbering', async () => {
    const { root, ledger } = await ledgerWithMission()
    const event = (sequence: number) => ({
      id: `run_1:${String(sequence)}`, runId: 'run_1', missionId: 'mission_1', sequence, occurredAt: NOW, sourceAdapter: 'codex',
      type: 'message.delta', payload: { itemId: 'answer', operation: 'append', text: 'hi', final: false, evidence: { redacted: true } }
    })
    await ledger.appendEvents('mission_1', [event(1)] as never)
    await ledger.appendApproval('mission_1', ALLOWED)
    await ledger.appendEvents('mission_1', [event(2)] as never)
    await ledger.flush()

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.events.map((one) => one.sequence)).toEqual([1, 2])
    expect(recovered?.approvals).toHaveLength(1)
    expect(recovered?.issues).toEqual([])
  }, 20_000)

  it('is refused on a mission written before version 20, which its own readers would stop at', async () => {
    const { ledger } = await ledgerWithMission(19)
    await expect(ledger.appendApproval('mission_1', ALLOWED)).rejects.toThrow('cannot hold approvals')
  }, 20_000)

  it('is read as damage in a file written before version 20', async () => {
    const { root } = await ledgerWithMission(19)
    await writeFile(join(root, 'mission_1.jsonl'), `${JSON.stringify({ schemaVersion: 19, recordType: 'mission.approval', ledgerSequence: 2, occurredAt: NOW, approval: ALLOWED })}\n`, { flag: 'a' })

    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')

    expect(recovered?.approvals).toEqual([])
    expect(recovered?.issues.map((issue) => issue.code)).toEqual(['invalid-record'])
  }, 20_000)

  it('refuses an answer or an answerer it does not know', async () => {
    const { ledger } = await ledgerWithMission()
    await expect(ledger.appendApproval('mission_1', { ...ALLOWED, answer: 'maybe' as never })).rejects.toThrow('Approval answer is invalid')
    await expect(ledger.appendApproval('mission_1', { ...ALLOWED, by: 'the teammate' as never })).rejects.toThrow('Approval answerer is invalid')
  }, 20_000)

  // v21 (0.616, the PRD's R8): a request the person's own Always, on an earlier
  // card of the run, allowed with no card of its own.
  const BY_EARLIER_ALWAYS: MissionApproval = {
    approvalId: 'ap_3',
    kind: 'command',
    asked: 'Run a command\necho two',
    answer: 'allowed',
    by: 'earlier-always',
    words: 'The Always given earlier in this run allows every command it runs.',
    askedAt: '2026-10-04T01:00:20.000Z',
    occurredAt: '2026-10-04T01:00:20.000Z'
  }

  it('keeps an answer by an earlier Always on a v21 mission, beside the card that gave it', async () => {
    const { root, ledger } = await ledgerWithMission()
    await ledger.appendApproval('mission_1', { ...ALLOWED, answer: 'allowed-always' })
    await ledger.appendApproval('mission_1', BY_EARLIER_ALWAYS)
    await ledger.flush()
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.approvals.map((approval) => approval.by)).toEqual(['card', 'earlier-always'])
    expect(recovered?.issues).toEqual([])
  }, 20_000)

  it('refuses an answer by an earlier Always on a v20 mission, whose readers would stop at it', async () => {
    const { ledger } = await ledgerWithMission(20)
    await ledger.appendApproval('mission_1', ALLOWED)
    await expect(ledger.appendApproval('mission_1', BY_EARLIER_ALWAYS)).rejects.toThrow('cannot hold an answer by an earlier Always')
  }, 20_000)

  it('reads an answer by an earlier Always in a v20 file as damage, not as an answer', async () => {
    const { root } = await ledgerWithMission(20)
    await writeFile(join(root, 'mission_1.jsonl'), `${JSON.stringify({ schemaVersion: 20, recordType: 'mission.approval', ledgerSequence: 2, occurredAt: BY_EARLIER_ALWAYS.occurredAt, approval: BY_EARLIER_ALWAYS })}\n`, { flag: 'a' })
    const recovered = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_1')
    expect(recovered?.approvals).toEqual([])
    expect(recovered?.issues.map((issue) => issue.code)).toEqual(['invalid-record'])
  }, 20_000)

  it('is bounded by the host the way the reader accepts it', () => {
    const long = boundedApproval({ ...ALLOWED, asked: 'x'.repeat(5_000), words: `  ${'y'.repeat(3_000)}  ` })
    expect(long.asked).toHaveLength(2_000)
    expect(long.words).toHaveLength(1_000)
    const empty = boundedApproval({ ...ALLOWED, asked: '   ', words: '  ' })
    expect(empty.asked).toBe('(the card said nothing more)')
    expect(empty.words).toBeUndefined()
  })
})
