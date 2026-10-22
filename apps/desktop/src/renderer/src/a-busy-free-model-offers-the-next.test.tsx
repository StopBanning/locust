import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type { PublicModel } from '../../shared/ipc.js'
import { ThreadItems } from './components/Thread.js'
import { buildThread } from './missionView.js'
import { nextFreeModel } from './status.js'

/**
 * ONE PRESS OFF A BUSY FREE MODEL (C9).
 *
 * On 2026-09-26 two of OpenCode's free models were limited for hours while
 * three others answered, and a new person starts on the first "-free" model
 * listed. The busy notice said to press Stop and pick another model; now it
 * names one -- the next free model after the run's -- and the press stops the
 * run, puts the chat box on it and hands the message back (App; the drive
 * checks that end).
 */
const model = (id: string, extra: Partial<PublicModel> = {}): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [], ...extra }) as PublicModel

const CATALOGUE = [
  model('opencode/big-pickle'),
  model('opencode/ling-3.0-flash-fin-free', { displayName: 'Ling 3.0 Flash Fin Free' }),
  model('opencode/beam-free', { displayName: 'Beam Free' }),
  model('mine/cheap-free', { own: true }),
  model('opencode/glm-5.3-flash-free', { displayName: 'GLM 5.3 Flash Free' })
]

// Best first (0.712, status.FREE_MODELS_BEST_FIRST): GLM 5.3 Flash, Beam, then Ling 3.0 where it is listed.
describe('the free model to offer', () => {
  it('is the next of OpenCode\'s free models after the one the run is on', () => {
    expect(nextFreeModel('opencode', 'opencode/glm-5.3-flash-free', CATALOGUE)?.id).toBe('opencode/beam-free')
  })

  it('skips a model of the person\'s own, and goes round after the last', () => {
    expect(nextFreeModel('opencode', 'opencode/beam-free', CATALOGUE)?.id).toBe('opencode/ling-3.0-flash-fin-free')
    expect(nextFreeModel('opencode', 'opencode/ling-3.0-flash-fin-free', CATALOGUE)?.id).toBe('opencode/glm-5.3-flash-free')
  })

  it('is nothing for a paid route, a model of their own, another runtime, or a lone free model', () => {
    expect(nextFreeModel('opencode', 'opencode/big-pickle', CATALOGUE)).toBeUndefined()
    expect(nextFreeModel('opencode', 'mine/cheap-free', CATALOGUE)).toBeUndefined()
    expect(nextFreeModel('claude', 'opencode/ling-3.0-flash-fin-free', CATALOGUE)).toBeUndefined()
    expect(nextFreeModel('opencode', 'opencode/ling-3.0-flash-fin-free', CATALOGUE.slice(0, 2))).toBeUndefined()
  })
})

function diagnostic(code: string): NormalizedRuntimeEvent {
  return {
    id: `d_${code}`,
    runId: 'run_1',
    missionId: 'mission_1',
    sequence: 1,
    type: 'adapter.diagnostic',
    occurredAt: '2026-09-26T19:00:00.000Z',
    sourceAdapter: 'opencode',
    payload: { level: 'warning', code, message: 'The model\'s provider answered "Rate limit exceeded", and OpenCode is trying again on its own.', terminal: false, evidence: { redacted: true } }
  } as unknown as NormalizedRuntimeEvent
}

describe('a busy notice in the thread', () => {
  it('is marked busy only for a provider that is turning requests away', () => {
    const busy = buildThread([diagnostic('opencode.provider_busy.runtime_error')], { running: true })
    const other = buildThread([diagnostic('opencode.runtime_error')], { running: true })
    expect(busy.find((item) => item.type === 'diagnostic')).toMatchObject({ busy: true })
    expect(other.find((item) => item.type === 'diagnostic')).not.toHaveProperty('busy')
  })

  it('carries the offer as a button under its words, and only the busy one does', () => {
    const offer = { label: 'Stop and switch to Beam Free', onPress: () => undefined }
    const draw = (code: string, withOffer: boolean) =>
      renderToStaticMarkup(
        <ThreadItems
          items={buildThread([diagnostic(code)], { running: true })}
          owner={undefined}
          activity="idle"
          workspacePath={undefined}
          decision={undefined}
          {...(withOffer ? { busyModel: offer } : {})}
        />
      )
    expect(draw('opencode.provider_busy.runtime_error', true)).toContain('class="lc-diagnostic__action">Stop and switch to Beam Free</button>')
    expect(draw('opencode.provider_busy.runtime_error', false)).not.toContain('lc-diagnostic__action')
    expect(draw('opencode.runtime_error', true)).not.toContain('lc-diagnostic__action')
  }, 10_000)
})
