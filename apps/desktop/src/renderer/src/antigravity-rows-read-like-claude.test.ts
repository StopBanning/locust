import { describe, expect, it } from 'vitest'

import { activityEntries, activityTrace, buildSignalRail, buildThread, thoughtHeadline, traceOutcome } from './missionView.js'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

/**
 * ANTIGRAVITY'S ROWS READ LIKE CLAUDE'S (0.541).
 *
 * Colin, on Boss in Edit, 2026-10-02: the fold read `manage_task done` -- the
 * runtime's own tool id -- and Antigravity's refused `git status` read
 * "failed" and "it exited non-zero". It never ran: Edit does not let
 * Antigravity run commands. The shape below is what agy-events.ts emits.
 */
let sequence = 0
function event(type: string, payload: unknown): NormalizedRuntimeEvent {
  sequence += 1
  return {
    id: `run:antigravity:${String(sequence)}`,
    runId: 'run',
    sequence,
    occurredAt: new Date(Date.UTC(2026, 9, 2, 12, 0, sequence)).toISOString(),
    sourceAdapter: 'antigravity',
    type,
    payload
  } as unknown as NormalizedRuntimeEvent
}

const events = [
  event('run.started', {}),
  event('tool.started', { itemId: 'tool_1', toolKind: 'manage_task', name: 'manage_task', phase: 'started' }),
  event('tool.completed', { itemId: 'tool_1', toolKind: 'manage_task', name: 'manage_task', phase: 'completed' }),
  event('tool.started', { itemId: 'tool_2', toolKind: 'run_command', name: 'run_command', command: 'git status', phase: 'started' }),
  event('tool.failed', { itemId: 'tool_2', toolKind: 'run_command', name: 'run_command', command: 'git status', phase: 'completed', status: 'refused', output: 'Not allowed in this mode.' }),
  event('run.completed', {})
]
const details = (() => {
  const activity = buildThread(events, { running: false }).find((item) => item.type === 'activity')
  return activity?.type === 'activity' ? activity.details : []
})()

describe("Antigravity's thought (0.542)", () => {
  it('reads "Thought for Ns" from the time agy gives the step, though the step arrives already over', () => {
    // Both arrive at once, as agy sends them: its own timing is the length.
    const thought = [
      event('run.started', {}),
      event('step.started', { stepKind: 'reasoning', itemId: 'thought_1' }),
      event('step.completed', { stepKind: 'reasoning', itemId: 'thought_1', durationMs: 4385 }),
      event('tool.started', { itemId: 'tool_2', toolKind: 'view_file', name: 'view_file', command: 'note.txt', phase: 'started' }),
      event('tool.completed', { itemId: 'tool_2', toolKind: 'view_file', name: 'view_file', command: 'note.txt', phase: 'completed' }),
      event('run.completed', {})
    ]
    const activity = buildThread(thought, { running: false }).find((item) => item.type === 'activity')
    const steps = activity?.type === 'activity' ? activity.details : []
    expect(steps[0]).toMatchObject({ kind: 'reasoning', durationMs: 4385, output: '' })
  })
})

describe("Antigravity's wait on a helper (0.833)", () => {
  it('reads as a wait on that helper, at the length agy timed, with what its list said under it', () => {
    const waited = [
      event('run.started', {}),
      event('step.started', { stepKind: 'reasoning', itemId: 'thought_624' }),
      event('step.completed', { stepKind: 'reasoning', itemId: 'thought_624', durationMs: 309_291, message: '**Waiting on its helper Soak & Sweep Runner**\nIts list says the helper is waiting for dependents, with no step since 7:18 AM.' }),
      event('adapter.diagnostic', { level: 'warning', code: 'antigravity.rereading', message: 'Antigravity has read this whole conversation again 3 times this turn, about 230,000 tokens each, with none of it reused: 691,000 tokens of your quota so far. If it is only waiting, press Stop and send "continue" later.', terminal: false }),
      event('run.completed', {})
    ]
    const thread = buildThread(waited, { running: false })
    const activity = thread.find((item) => item.type === 'activity')
    const steps = activity?.type === 'activity' ? activity.details : []
    expect(steps[0]).toMatchObject({ kind: 'reasoning', durationMs: 309_291 })
    expect(thoughtHeadline(steps[0]?.output ?? '')).toBe('Waiting on its helper Soak & Sweep Runner')
    expect(JSON.stringify(thread)).toContain('press Stop and send')
  })
})

describe("Antigravity's rows", () => {
  it('a check on a background command reads as one, never as manage_task', () => {
    // Measured 0.542: manage_task is {Action: "status", TaskId} on a command agy sent away.
    const rows = activityEntries(details)
    const check = rows.find((entry) => entry.kind === 'tool')
    expect(check?.kind === 'tool' ? check.name : undefined).toBe('Checked on a command')
    expect(JSON.stringify(rows)).not.toContain('manage_task')
  })

  it('a command the mode refused reads refused, never failed or non-zero', () => {
    const shell = activityEntries(details).find((entry) => entry.kind === 'shell')
    expect(shell?.kind === 'shell' ? shell.refused : undefined).toBe('Not allowed in this mode.')
    const line = activityTrace(details, events, traceOutcome(events, false)).map((segment) => segment.text).join(' · ')
    expect(line).toContain('1 refused')
    expect(line).not.toMatch(/non-zero|ran git status/)
  })
})

describe("Antigravity's refusal in Activity (0.545, Sol on 0.544)", () => {
  it('a refused command did not run, and a runtime note is its sentence, not its code', () => {
    const withNote = [
      ...events.slice(0, -1),
      event('adapter.diagnostic', { level: 'warning', code: 'antigravity.denied_actions', message: 'It stopped there. Antigravity was not allowed to run a command in this mode.', terminal: false }),
      events.at(-1)!
    ]
    const names = buildSignalRail(withNote, { running: false }).map((row) => row.name)
    expect(names).toContain('Did not run git status')
    expect(names.join(' | ')).not.toMatch(/Ran git status|antigravity\.denied_actions/)
    expect(names.some((name) => name.startsWith('It stopped there.'))).toBe(true)
  })
})
