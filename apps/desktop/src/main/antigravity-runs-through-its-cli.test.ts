import { describe, expect, it } from 'vitest'

import { whatItMayDo } from '../shared/may-do.js'
import { hostReadsEventsOf } from '../shared/runtimes.js'
import { agyRoute, resumableThreadOf } from './codex-mission.js'

/**
 * ANTIGRAVITY RUNS THROUGH ITS CLI (0.540). Google moved personal accounts
 * from Gemini CLI to Antigravity CLI; `agy` runs headless with a stream the
 * host reads whole, so Antigravity is one more runtime the runner drives.
 */
describe('Antigravity through its CLI', () => {
  it('is a runtime whose events the host reads', () => {
    expect(hostReadsEventsOf('antigravity')).toBe(true)
    expect(hostReadsEventsOf('gemini')).toBe(false)
  })

  it('carries an app tier, or an id with its effort on the end, to a model and an effort; the effort chosen in the box wins', () => {
    expect(agyRoute('flash', undefined)).toEqual({ model: 'gemini-3.8-flash', effort: 'medium' })
    expect(agyRoute('pro', undefined)).toEqual({ model: 'gemini-3.1-pro', effort: 'high' })
    expect(agyRoute('flash_lite', undefined)).toEqual({ model: 'gemini-3.8-flash', effort: 'low' })
    expect(agyRoute('gemini-3.8-flash-low', undefined)).toEqual({ model: 'gemini-3.8-flash', effort: 'low' })
    expect(agyRoute('gemini-3.8-flash-low', 'high')).toEqual({ model: 'gemini-3.8-flash', effort: 'high' })
    expect(agyRoute('claude-opus-4-6-thinking', undefined)).toEqual({ model: 'claude-opus-4-6-thinking' })
    // A Gemini model saved bare goes with Medium (0.665): agy refuses one with no effort, and a teammate's
    // automatic reply sent none ("--model gemini-3.8-flash requires --effort").
    expect(agyRoute('gemini-3.8-flash', undefined)).toEqual({ model: 'gemini-3.8-flash', effort: 'medium' })
    expect(agyRoute('gemini-3.8-flash', 'low')).toEqual({ model: 'gemini-3.8-flash', effort: 'low' })
    expect(agyRoute(undefined, undefined)).toEqual({})
  })

  it('sends an Orchestrator model with no effort at all, even one left over from another model', () => {
    // agy 1.5: "--effort is not supported for model gemini-pro-orchestrator-neon". Its planner and executor
    // efforts are agy's own settings, chosen in agy's /model; measured 22 Nov, the bare model runs.
    expect(agyRoute('gemini-pro-orchestrator-neon', undefined)).toEqual({ model: 'gemini-pro-orchestrator-neon' })
    expect(agyRoute('gemini-pro-orchestrator-g1-pro', 'medium')).toEqual({ model: 'gemini-pro-orchestrator-g1-pro' })
  })
})

describe('a reply to a conversation Antigravity began elsewhere (0.541, Colin: "trajectory not found")', () => {
  const APP_ID = '920da525-dde6-4610-9d16-9444d0422935'
  const mission = (metadata: Record<string, unknown>, events: readonly Record<string, unknown>[]) =>
    ({ metadata: { missionId: 'mission_1', runtime: 'antigravity', ...metadata }, events }) as never

  it('starts the CLI fresh after a turn the app ran', () => {
    const app = mission({ resolvedRouteId: 'antigravity:hub' }, [{ type: 'run.completed', payload: { runtimeThreadId: APP_ID } }])
    expect(resumableThreadOf(app, 'antigravity')).toBeUndefined()
  })

  it('starts fresh after the 0.540 failure, which named the app id but not the ended session', () => {
    const failed = mission({ resolvedRouteId: 'antigravity', continuesFrom: { missionId: 'mission_0', runtimeThreadId: APP_ID } }, [
      { type: 'step.started', payload: { runtimeThreadId: APP_ID } },
      { type: 'run.failed', payload: { runtimeThreadId: APP_ID, message: `Antigravity could not run it: failed to send message: trajectory not found: ${APP_ID}` } }
    ])
    expect(resumableThreadOf(failed, 'antigravity')).toBeUndefined()
  })

  it('still resumes a conversation the CLI itself holds, and a turn stopped before it named one', () => {
    expect(resumableThreadOf(mission({ resolvedRouteId: 'antigravity' }, [{ type: 'run.completed', payload: { runtimeThreadId: 'cli-1' } }]), 'antigravity')).toBe('cli-1')
    expect(resumableThreadOf(mission({ continuesFrom: { missionId: 'mission_0', runtimeThreadId: 'cli-1' } }, [{ type: 'run.cancelled', payload: {} }]), 'antigravity')).toBe('cli-1')
  })

  it('a failure that ended any session is not revived through what it continued', () => {
    const ended = mission({ runtime: 'opencode', continuesFrom: { missionId: 'mission_0', runtimeThreadId: 'ses_1' } }, [{ type: 'run.failed', payload: { sessionEnded: true } }])
    expect(resumableThreadOf(ended, 'opencode')).toBeUndefined()
  })
})

describe('what an Antigravity run may do, as measured', () => {
  it('runs commands only in Auto', () => {
    const said = (sandbox: 'read-only' | 'workspace-write' | 'full-access') => JSON.stringify(whatItMayDo('antigravity', sandbox))
    expect(said('full-access')).toMatch(/command/i)
    expect(said('workspace-write')).not.toMatch(/any command/i)
  })
})
