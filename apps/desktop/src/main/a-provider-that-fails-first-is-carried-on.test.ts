import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { carryOnPrompt, carryOnReason, providerFailedFirst } from '../shared/carry-on.js'
import type { CarryOnResponse, PlanRunsOut } from '../shared/carry-on.js'
import { createCarryOn, endedOnPlanLimit, stoppedFor } from './carry-on.js'
import type { CarryOnStart } from './carry-on.js'

/*
 * A PROVIDER THAT FAILED FIRST IS CARRIED ON (0.839, the OpenBot scrub's take 3). OpenBot moves an agent to its next
 * model on a provider, sign-in or network error, not only a used-up plan. Locust now does the same, through the same
 * card and the same Without asking -- but only for a turn that had not run one tool, so the next agent starts clean.
 */

const failed = (kind: string, message: string, stderr?: string) => ({
  type: 'run.failed',
  sourceAdapter: 'claude',
  payload: { kind, message, ...(stderr === undefined ? {} : { process: { stderr } }) }
})
const signedOut = failed('authentication-failed', 'Invalid API key · Please run /login')
const busy = failed('unknown', 'Claude Code stopped: API Error: 529 Overloaded')
const offline = failed('process-failed', 'Codex invocation did not complete successfully', 'starting\nError: getaddrinfo ENOTFOUND api.openai.com\n')
const throttled = failed('temporary-rate-limit', 'Rate limited, try again shortly')
const refused = failed('safety-blocked', 'The request was refused')
const puzzling = failed('unknown', 'Claude Code stopped: something went wrong')
const tool = { type: 'tool.started', sourceAdapter: 'claude', payload: { toolCallId: 't1', name: 'Read' } }
const quota = { type: 'run.failed', sourceAdapter: 'codex', payload: { kind: 'quota-exhausted', message: "You've hit your usage limit." } }

function harness(input: { readonly events: readonly unknown[]; readonly phase?: string; readonly runtime?: string; readonly setting?: PlanRunsOut }) {
  const started: CarryOnStart[] = []
  const notes: string[] = []
  const carryOn = createCarryOn({
    ledger: {
      getMission: async () =>
        ({ phase: input.phase ?? 'failed', events: input.events, metadata: { missionId: 'mission_1', runId: 'run_1', runtime: input.runtime ?? 'claude', sandbox: 'workspace-write', mode: 'accept-edits' } }) as never
    },
    setting: async () => input.setting,
    sentByPerson: () => true,
    note: (_missionId, message) => notes.push(message),
    start: async (asked): Promise<CarryOnResponse> => {
      started.push(asked)
      return { ok: true, missionId: 'mission_2' }
    }
  })
  return { carryOn, started, notes }
}

describe('a provider that fails before the turn did anything is carried on', () => {
  it('is a sign-in that expired, a provider that is down or busy, a throttle, or no network', () => {
    for (const events of [[signedOut], [busy], [offline], [throttled]]) expect(providerFailedFirst(events)).toBe(true)
    expect(carryOnReason([busy])).toBe('provider')
  })

  it('is never a refusal, an error nobody can read, a run that finished, or one that had already run a tool', () => {
    expect(providerFailedFirst([refused])).toBe(false)
    expect(providerFailedFirst([puzzling])).toBe(false)
    expect(providerFailedFirst([busy, { type: 'run.completed', payload: {} }])).toBe(false)
    // Even a read: the next agent would find the turn begun, and could do it twice.
    expect(providerFailedFirst([tool, busy])).toBe(false)
    expect(providerFailedFirst([])).toBe(false)
  })

  it('leaves a used-up plan to its own reason, which still carries on as it did', () => {
    expect(carryOnReason([quota])).toBe('plan')
    expect(providerFailedFirst([quota])).toBe(false)
    expect(stoppedFor({ phase: 'failed', events: [busy] } as never)).toBe('provider')
    expect(stoppedFor({ phase: 'cancelled', events: [busy] } as never)).toBeUndefined()
    expect(endedOnPlanLimit({ phase: 'failed', events: [busy] } as never)).toBe(false)
  })

  it('one press starts the task from the start on the agent offered, told why', async () => {
    const h = harness({ events: [busy] })
    expect(await h.carryOn.carry('mission_1', { runtime: 'codex' })).toEqual({ ok: true, missionId: 'mission_2' })
    expect(h.started).toEqual([{ prompt: carryOnPrompt('claude', 'provider'), runtime: 'codex', model: undefined, mode: 'accept-edits', followUpOf: 'mission_1', from: 'claude' }])
    expect(carryOnPrompt('claude', 'provider')).toBe('Claude Code stopped on an error from its provider before it did anything. Do the task from the start, the way the task asks.')
    const same = harness({ events: [busy] })
    expect(await same.carryOn.carry('mission_1', { runtime: 'claude' })).toEqual({ ok: false, message: 'Claude Code is the agent that just failed.' })
    const begun = harness({ events: [tool, busy] })
    expect((await begun.carryOn.carry('mission_1', { runtime: 'codex' })).ok).toBe(false)
  })

  it('goes on by itself with Without asking on, and says so when the picked agent is the one that failed', async () => {
    const on = harness({ events: [offline], runtime: 'codex', setting: { runtime: 'claude', automatic: true } })
    await on.carryOn.onRunEnded({ missionId: 'mission_1' })
    expect(on.started.map((start) => start.runtime)).toEqual(['claude'])
    const itself = harness({ events: [signedOut], setting: { runtime: 'claude', automatic: true } })
    await itself.carryOn.onRunEnded({ missionId: 'mission_1' })
    expect(itself.started).toEqual([])
    expect(itself.notes).toEqual(['Claude Code is the agent picked to carry on, and it is the one whose provider failed, so nothing was carried on.'])
  })

  it('says why on the card and on the seam, and Settings says it covers both', () => {
    const renderer = join(__dirname, '../renderer/src/')
    const thread = readFileSync(`${renderer}components/Thread.tsx`, 'utf8')
    expect(thread).toContain('stopped on an error before it did anything, so another agent can do the task from the start.')
    const divider = readFileSync(`${renderer}components/HandoffDivider.tsx`, 'utf8')
    expect(divider).toContain("’s provider failed before it did anything, so Locust carried the work on with ${toLabel}.")
    const settings = readFileSync(`${renderer}components/Screens.tsx`, 'utf8')
    expect(settings).toContain('<h2 className="lc-settings__heading">When an agent cannot go on</h2>')
    expect(readFileSync(`${renderer}settingsPages.ts`, 'utf8')).toContain("'provider error'")
  })
})
