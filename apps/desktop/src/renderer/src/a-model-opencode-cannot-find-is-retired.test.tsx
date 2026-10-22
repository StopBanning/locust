import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { PublicModel } from '../../shared/ipc.js'
import { Thread } from './components/Thread.js'
import { modelRetired, modelUnavailable, retiredFreeModels } from './missionView.js'
import { freeStartModel } from './status.js'

/*
 * 0.712, from the first hour on a bare Mac (_smoke/mac-first-hour-smoke.mjs,
 * 2026-10-09). OpenCode installed in 9 s through the npm Locust carries, and
 * the chat box was on Fledge Alpha: a fresh OpenCode 1.18.35 lists the models
 * its binary was built with until it next reads models.dev, and Fledge Alpha
 * -- first of the free ones once Exo was left out -- was among them. The first
 * message a new person sent ended on this line, with nothing to press.
 */
const SAID = 'OpenCode stopped: Model not found: opencode/fledge-alpha-free. Did you mean: exo-free, ling-3.0-flash-fin-free, ling-3.1-flash-free?'
const model = (id: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
// The fresh install's list, in its order.
const catalog = ['opencode/exo-free', 'opencode/fledge-alpha-free', 'opencode/ling-3.0-flash-fin-free', 'opencode/beam-free'].map(model)
const failedWith = (message: string): NormalizedRuntimeEvent =>
  ({ type: 'run.failed', payload: { message } }) as unknown as NormalizedRuntimeEvent
const run = (id: string, phase: 'completed' | 'failed', at: string, message?: string) => ({
  runtime: 'opencode' as const,
  model: id,
  phase,
  lastUpdatedAt: at,
  events: message === undefined ? [] : [failedWith(message)]
})
const shown = (retired: ReadonlySet<string>) => catalog.filter((entry) => !retired.has(entry.id))

describe('a model OpenCode cannot find is retired', () => {
  it('reads "Model not found" as retired, and only that', () => {
    expect(modelRetired({ message: SAID })).toBe(true)
    expect(modelUnavailable({ message: SAID })).toBe(false)
    expect(modelRetired({ message: 'Error: ENOENT: file not found: notes.md' })).toBe(false)
    expect(modelRetired({ message: 'OpenCode stopped: Upstream request failed: Model is unavailable.' })).toBe(false)
  })

  it('a new person never starts on Fledge Alpha: left out before anyone here has run it', () => {
    const retired = retiredFreeModels([])
    expect(retired.has('opencode/fledge-alpha-free')).toBe(true)
    expect(shown(retired).map((entry) => entry.id)).not.toContain('opencode/fledge-alpha-free')
    // The steadiest listed (status.FREE_MODELS_BEST_FIRST); and without it, still not Fledge Alpha.
    expect(freeStartModel('opencode', shown(retired), [])).toBe('opencode/beam-free')
    expect(freeStartModel('opencode', shown(retired).filter((entry) => !entry.id.startsWith('opencode/beam')), [])).toBe('opencode/ling-3.0-flash-fin-free')
  })

  it('the next model OpenCode drops is learned from its first failure, and is back once it answers', () => {
    const lost = 'OpenCode stopped: Model not found: opencode/beam-free. Did you mean: ling-3.1-flash-free?'
    expect(retiredFreeModels([run('opencode/beam-free', 'failed', '2026-10-10T01:00:00Z', lost)]).has('opencode/beam-free')).toBe(true)
    expect(
      retiredFreeModels([
        run('opencode/beam-free', 'failed', '2026-10-10T01:00:00Z', lost),
        run('opencode/beam-free', 'completed', '2026-10-11T01:00:00Z')
      ]).has('opencode/beam-free')
    ).toBe(false)
  })
})

describe('the offer under a failed free model says why', () => {
  const thread = (why?: 'retired' | 'down' | 'limit'): string =>
    renderToStaticMarkup(
      <Thread
        prompt="Say hello to me in five words."
        onOpenPeerRun={() => undefined}
        earlierTurns={[]}
        events={[]}
        running={false}
        restoredMission={undefined}
        error={SAID}
        errorIsPersistence={false}
        startedAt={undefined}
        approvals={[]}
        onDecide={() => undefined}
        onAnswerQuestion={() => undefined}
        decidingIds={[]}
        cancelled={false}
        handoff={undefined}
        peers={{ self: undefined, teammates: [], messages: [], notices: [] }}
        limitModel={{ label: 'Switch to Ling 3.0 Flash Fin Free', onPress: () => undefined, ...(why === undefined ? {} : { why }) }}
      />
    )

  it('a retired model is not "at its limit"', () => {
    const html = thread('retired')
    expect(html).toContain('OpenCode no longer offers this model.')
    expect(html).not.toContain('at its limit')
    expect(html).toContain('Switch to Ling 3.0 Flash Fin Free')
  })

  it('a provider that is down says so; a limit, or no reason given, stays a limit', () => {
    expect(thread('down')).toContain('provider is down for now.')
    expect(thread('limit')).toContain('This model is at its limit.')
    expect(thread()).toContain('This model is at its limit.')
  })
})
