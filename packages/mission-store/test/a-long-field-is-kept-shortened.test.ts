import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { createFileMissionLedger, fitEventToLedger, LEDGER_MESSAGE_TEXT_LIMIT, LEDGER_TEXT_LIMIT } from '../src/index.js'

/**
 * A LONG FIELD IS KEPT SHORTENED, NEVER REFUSED (review 2026-10-15, C1 and D2).
 *
 * The ledger refuses a field over its limit, and the writer refuses what the
 * reader would, so one unbounded field stopped a whole run with "the mission
 * ledger could not be written". Ten normalizers each bounded their own fields
 * or did not: the 0.715 hunt's MU1 and CP1, then 0.777's ACP message chunk,
 * then -- by a field-size audit over the recorded transcripts -- Antigravity,
 * Muse, Copilot, OpenCode and Claude fields as well.
 *
 * The fit now happens once, where events meet the ledger. This proves it
 * against the class: every event type, every free-text and name field made
 * 20,000 characters, written to a real file and read back.
 */
const NOW = '2026-10-15T20:00:00.000Z'
const LONG = 'w'.repeat(20_000)
const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

const process = {
  exitCode: 0,
  signal: null,
  stderr: '',
  stderrTruncated: false,
  recordCount: 1,
  inputDeliveryFailed: false,
  outputLimitExceeded: false,
  forcedTerminationAttempted: false,
  terminationUnconfirmed: false,
  startedAt: NOW,
  finishedAt: NOW
}

function evidence(raw: unknown): Record<string, unknown> {
  return { transportSequence: 1, runtimeEventType: LONG, redacted: false, raw }
}

/** One of each event type, every string that is not vocabulary made long. */
function longPayloads(): ReadonlyArray<readonly [NormalizedRuntimeEvent['type'], Record<string, unknown>]> {
  const raw = { text: LONG, [LONG]: 1, list: Array.from({ length: 150 }, () => LONG.slice(0, 10)), deep: { a: { b: { c: { d: { e: { f: { g: { h: { i: 'x' } } } } } } } } } }
  return [
    ['run.started', { runtimeThreadId: LONG, evidence: evidence(raw) }],
    ['plan.updated', { itemId: LONG, plan: [{ content: LONG, status: 'pending' }], final: false, evidence: evidence(raw) }],
    ['message.delta', { itemId: LONG, operation: 'append', text: LONG, final: false, evidence: evidence(raw) }],
    ['step.started', { stepKind: 'reasoning', itemId: LONG, itemType: LONG, status: LONG, message: LONG, evidence: evidence(raw) }],
    ['step.completed', { stepKind: 'item', itemId: LONG, message: LONG, evidence: evidence(raw) }],
    ['tool.started', { itemId: LONG, toolKind: LONG, name: LONG, command: LONG, title: LONG, parentItemId: LONG, phase: 'started', evidence: evidence(raw) }],
    ['tool.completed', { itemId: LONG, toolKind: 'shell', name: 'Bash', output: { stdout: LONG, rows: Array.from({ length: 30 }, () => LONG) }, status: LONG, exitCode: 0, phase: 'completed', evidence: evidence(raw) }],
    ['route.limit_detected', { kind: 'quota-exhausted', message: LONG, evidence: evidence(raw) }],
    ['adapter.diagnostic', { level: 'warning', code: LONG, message: LONG, terminal: false, evidence: evidence(raw) }],
    ['run.completed', { runtimeThreadId: LONG, usage: { [LONG]: 1, note: LONG }, process: { ...process, stderr: 'e'.repeat(600_000) } }],
    ['run.failed', { kind: 'unknown', message: LONG, runtimeThreadId: LONG, runtimeTerminal: 'failed', process }]
  ]
}

describe('a long field', () => {
  it('is kept shortened in every event type, and read back', async () => {
    const root = await mkdtemp(join(tmpdir(), 'teammate-fit-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      missionId: 'mission_fit',
      runId: 'run_fit',
      prompt: 'Say something long.',
      runtime: 'acp',
      model: 'account-default',
      requestedRouteId: 'acp',
      resolvedRouteId: 'acp:agent',
      cliVersion: '1.0.0',
      workspaceId: 'ws_test',
      sandbox: 'read-only',
      mode: 'approve-each',
      executionPolicyVersion: 1,
      createdAt: NOW
    })
    const payloads = longPayloads()
    // The control: a walk over nothing would pass.
    expect(payloads.length).toBe(11)
    const events = payloads.map(([type, payload], index) => ({
      id: `run_fit:acp:${String(index + 1)}`,
      runId: 'run_fit',
      missionId: 'mission_fit',
      sequence: index + 1,
      occurredAt: NOW,
      sourceAdapter: 'acp',
      cliVersion: LONG,
      requestedRouteId: LONG,
      runtimeThreadId: LONG,
      type,
      payload
    })) as unknown as NormalizedRuntimeEvent[]

    await ledger.appendEvents('mission_fit', events)
    // Read by a fresh reader: what is on disk, not what this one remembers.
    const read = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_fit')
    expect(read?.events.map((event) => event.type)).toEqual(payloads.map(([type]) => type))
    // An answer this long is kept WHOLE now (0.869); every other free-text
    // field is still shortened, both ends kept, the middle said to be missing.
    const message = read?.events.find((event) => event.type === 'message.delta')
    expect((message?.payload as { text: string }).text).toBe(LONG)
    const said = read?.events.find((event) => event.type === 'adapter.diagnostic')
    const text = (said?.payload as { message: string }).message
    expect(text.length).toBeLessThanOrEqual(LEDGER_TEXT_LIMIT)
    expect(text.startsWith('www')).toBe(true)
    expect(text.endsWith('www')).toBe(true)
    expect(text).toContain('[shortened]')
  })

  it('keeps a long answer whole up to its own limit, and shortens one past it (0.869)', async () => {
    // Colin, 2026-11-07: a Codex report an app read over Locust's MCP server
    // arrived "with the middle cut out", cut here at 16,384.
    const root = await mkdtemp(join(tmpdir(), 'teammate-fit-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      missionId: 'mission_answer', runId: 'run_answer', prompt: 'Write the report.', runtime: 'codex', model: 'account-default',
      requestedRouteId: 'codex', resolvedRouteId: 'codex:cli', cliVersion: '1.0.0', workspaceId: 'ws_test', sandbox: 'read-only',
      mode: 'approve-each', executionPolicyVersion: 1, createdAt: NOW
    })
    const report = Array.from({ length: 1_000 }, (_, i) => `## Part ${String(i)}\n${'word '.repeat(16)}`).join('\n')
    expect(report.length).toBeGreaterThan(80_000)
    const tooLong = `START${'x'.repeat(LEDGER_MESSAGE_TEXT_LIMIT)}END`
    // Within the character limit, past a record's bytes: JSON writes each as six.
    const wide = '\u0001'.repeat(LEDGER_MESSAGE_TEXT_LIMIT - 10)
    const answer = (sequence: number, itemId: string, text: string) => ({
      id: `run_answer:codex:${String(sequence)}`, runId: 'run_answer', missionId: 'mission_answer', sequence, occurredAt: NOW,
      sourceAdapter: 'codex', type: 'message.delta', payload: { itemId, operation: 'replace', text, final: true, evidence: { redacted: false } }
    }) as unknown as NormalizedRuntimeEvent
    await ledger.appendEvents('mission_answer', [answer(1, 'a1', report), answer(2, 'a2', tooLong), answer(3, 'a3', wide)])
    const read = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_answer')
    const texts = read?.events.map((event) => (event.payload as { text: string }).text) ?? []
    expect(texts[0]).toBe(report)
    expect(texts[1]!.length).toBeLessThanOrEqual(LEDGER_MESSAGE_TEXT_LIMIT)
    expect(texts[1]!.startsWith('START')).toBe(true)
    expect(texts[1]!.endsWith('END')).toBe(true)
    expect(texts[1]).toContain('[shortened]')
    expect(texts[2]!.length).toBeLessThan(wide.length)
    expect(texts[2]).toContain('[shortened]')
  })

  it('is refused as before when it is not long but wrong', async () => {
    const root = await mkdtemp(join(tmpdir(), 'teammate-fit-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    await ledger.createMission({
      missionId: 'mission_wrong',
      runId: 'run_wrong',
      prompt: 'Say something.',
      runtime: 'acp',
      model: 'account-default',
      requestedRouteId: 'acp',
      resolvedRouteId: 'acp:agent',
      cliVersion: '1.0.0',
      workspaceId: 'ws_test',
      sandbox: 'read-only',
      mode: 'approve-each',
      executionPolicyVersion: 1,
      createdAt: NOW
    })
    // Vocabulary is not fitted: an operation nobody defined is still refused.
    const wrong = {
      id: 'run_wrong:acp:1', runId: 'run_wrong', missionId: 'mission_wrong', sequence: 1, occurredAt: NOW, sourceAdapter: 'acp',
      type: 'message.delta', payload: { itemId: 'm1', operation: 'prepend', text: 'hi', final: false, evidence: { redacted: false } }
    } as unknown as NormalizedRuntimeEvent
    await expect(ledger.appendEvents('mission_wrong', [wrong])).rejects.toThrow('not readable')
  })

  it('costs nothing when it already fits', () => {
    const event = {
      id: 'r:acp:1', runId: 'r', missionId: 'm', sequence: 1, occurredAt: NOW, sourceAdapter: 'acp',
      type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: 'hello', final: false, evidence: { redacted: false, raw: { a: [1, 'two'] } } }
    } as unknown as NormalizedRuntimeEvent
    expect(fitEventToLedger(event)).toBe(event)
  })
})
