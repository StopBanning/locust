import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { QUIET_FREE_MS, quietFreeRun } from './missionView.js'
import { Thread } from './components/Thread.js'

/*
 * A FREE MODEL THAT SAYS NOTHING IS OFFERED ANOTHER (2026-11-01). On 2026-10-31's evening every OpenCode free model
 * took a message and said nothing, for hours and without an error (the launch-clip filming; CI's first hour on a bare
 * Mac at 00:14). A failed run already offers a free model that answers (0.711). A turn that has had nothing back for two
 * minutes offers it while it runs: switching stops it and puts the message back in the chat box, sending nothing.
 */
const BEGAN = Date.parse('2026-11-01T05:00:00.000Z')
const at = (ms: number): string => new Date(BEGAN + ms).toISOString()
const event = (type: string, ms = 0): NormalizedRuntimeEvent => ({ type, occurredAt: at(ms), sourceAdapter: 'opencode', payload: {} }) as unknown as NormalizedRuntimeEvent
const free = (events: readonly NormalizedRuntimeEvent[]) => ({ runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free', events })

describe('a free model that has said nothing', () => {
  it('is quiet two minutes after its run began with nothing back, and not a moment before', () => {
    const events = [event('run.started'), event('adapter.diagnostic', 5_000)]
    expect(quietFreeRun(free(events), BEGAN + QUIET_FREE_MS)).toBe(true)
    expect(quietFreeRun(free(events), BEGAN + QUIET_FREE_MS - 1)).toBe(false)
  })

  it('is never quiet once anything came back: a word, a thought, a step or a tool', () => {
    for (const type of ['message.delta', 'step.started', 'tool.started', 'plan.updated']) {
      expect(quietFreeRun(free([event('run.started'), event(type, 90_000)]), BEGAN + 10 * QUIET_FREE_MS)).toBe(false)
    }
  })

  it('is only an OpenCode free model, and only once its run has begun', () => {
    const silent = [event('run.started')]
    const later = BEGAN + 10 * QUIET_FREE_MS
    expect(quietFreeRun({ ...free(silent), model: 'opencode/claude-opus-5.5' }, later)).toBe(false)
    expect(quietFreeRun({ ...free(silent), runtime: 'kilo', model: 'kilo/kilo-auto/free' }, later)).toBe(false)
    expect(quietFreeRun({ runtime: 'opencode', events: silent }, later)).toBe(false)
    // Still starting: the live line says so.
    expect(quietFreeRun(free([]), later)).toBe(false)
  })
})

describe('the offer, while the turn runs', () => {
  const thread = (extra: Record<string, unknown>): string =>
    renderToStaticMarkup(
      <Thread
        prompt="Where can I keep savings for three years?"
        onOpenPeerRun={() => undefined}
        earlierTurns={[]}
        events={[]}
        running={true}
        restoredMission={undefined}
        error={undefined}
        errorIsPersistence={false}
        startedAt={undefined}
        approvals={[]}
        onDecide={() => undefined}
        onAnswerQuestion={() => undefined}
        decidingIds={[]}
        cancelled={false}
        handoff={undefined}
        peers={{ self: undefined, teammates: [], messages: [], notices: [] }}
        {...extra}
      />
    )

  it('names the quiet model, says where the message goes, and offers the next one', () => {
    const html = thread({ quietModel: { model: 'Deepseek V4.1 Flash Free', label: 'Stop and switch to Big Pickle Free', onPress: () => undefined } })
    expect(html).toContain('Deepseek V4.1 Flash Free has said nothing in two minutes.')
    expect(html).toContain('your message goes back in the chat box')
    expect(html).toContain('Stop and switch to Big Pickle Free')
    expect(thread({})).not.toContain('has said nothing')
  })

  // The press is ONE PRESS OFF A BUSY FREE MODEL's (C9): it stops the run, moves the chat box on and hands the message back.
  it('is wired: the window offers it for the run on screen, with the busy model’s own press', () => {
    const app = readFileSync(join(__dirname, 'App.tsx'), 'utf8')
    expect(app).toContain('{...(quietOffer === undefined ? {} : { quietModel: quietOffer })}')
    const quiet = app.slice(app.indexOf('const quietOffer'), app.indexOf('A FREE MODEL OPENCODE DROPPED'))
    expect(quiet).toContain('label: busyOffer.label, onPress: busyOffer.onPress')
    const busy = app.slice(app.indexOf('const busyOffer'), app.indexOf('const quietOffer'))
    expect(busy).toMatch(/cancelMission\(\)\s+changeRoute\(\{ runtime: next\.runtime, model: next\.id \}\)\s+setHandBack\(/)
  })
})
