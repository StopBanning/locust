import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { carryOnPrompt } from '../../shared/carry-on.js'
import { HandoffDivider } from './components/HandoffDivider.js'
import { Thread } from './components/Thread.js'
import { turnPromptLine } from './missionView.js'

/*
 * A turn Locust carried on after a plan ran out (0.774) is Locust's: no bubble in the person's voice, a divider that
 * says why the work moved, and -- on the stopped run's card -- one press that names the agent it moves to.
 */

const ranOut: NormalizedRuntimeEvent[] = [
  { id: 'failed', runId: 'run_1', missionId: 'mission_1', sequence: 1, occurredAt: '2026-10-15T12:00:01.000Z', sourceAdapter: 'codex', type: 'run.failed',
    payload: { kind: 'quota-exhausted', message: "You've hit your usage limit.", runtimeTerminal: 'failed', process: {
      exitCode: 1, signal: null, stderr: '', stderrTruncated: false, recordCount: 1,
      inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0,
      forcedTerminationAttempted: false, terminationUnconfirmed: false,
      startedAt: '2026-10-15T12:00:00.000Z', finishedAt: '2026-10-15T12:00:01.000Z'
    } } } as unknown as NormalizedRuntimeEvent
]

const render = (carryOn: { readonly label: string; readonly busy: boolean; readonly refused?: string; readonly why?: 'plan' | 'provider' }): string => renderToStaticMarkup(
  <Thread prompt="Write the release notes" onOpenPeerRun={() => undefined} earlierTurns={[]} events={ranOut}
    running={false} restoredMission={undefined} error="You've hit your usage limit." errorIsPersistence={false} startedAt={undefined}
    approvals={[]} onDecide={() => undefined} onAnswerQuestion={() => undefined} decidingIds={[]} cancelled={false}
    handoff={undefined} peers={{ self: undefined, teammates: [], messages: [], notices: [] }}
    carryOn={{ why: 'plan', ...carryOn, from: 'codex', onPress: () => undefined }} />
)

describe('a carried-on turn is said as Locust\'s', () => {
  it('draws no bubble in the person\'s voice', () => {
    expect(turnPromptLine({ prompt: carryOnPrompt('codex'), startedBy: { kind: 'carry-on', from: 'codex' } })).toBeUndefined()
  })

  it('the divider says the plan ran out and where the work went', () => {
    const html = renderToStaticMarkup(<HandoffDivider from="codex" to="claude" at="14:05" unsettledCount={0} omittedBriefing={[]} carriedOn />)
    expect(html).toContain('Codex ran out of its plan, so Locust carried the work on with Claude Code.')
    const plain = renderToStaticMarkup(<HandoffDivider from="codex" to="claude" at="14:05" unsettledCount={0} omittedBriefing={[]} />)
    expect(plain).not.toContain('ran out of its plan')
    // 0.839: a provider that failed before the turn did anything is said as that, not as a plan.
    const provider = renderToStaticMarkup(<HandoffDivider from="codex" to="claude" at="14:05" unsettledCount={0} omittedBriefing={[]} carriedOn carriedFor="provider" />)
    expect(provider).toContain('Codex’s provider failed before it did anything, so Locust carried the work on with Claude Code.')
    expect(provider).not.toContain('ran out of its plan')
  })

  it('the stopped run says its provider failed first, and that another agent can start it clean (0.839)', () => {
    const html = render({ label: 'Carry on with Claude Code', busy: false, why: 'provider' })
    expect(html).toContain('stopped on an error before it did anything, so another agent can do the task from the start.')
    expect(html).not.toContain('ran out of its plan')
  })

  it('the stopped run offers one press that names the agent, and says why a press was refused', () => {
    const html = render({ label: 'Carry on with Claude Code', busy: false })
    expect(html).toContain('Codex CLI ran out of its plan. What it did is kept, and the next agent is told what was done and what is left.')
    expect(html).toContain('Carry on with Claude Code')
    expect(render({ label: 'Carry on with Claude Code', busy: true })).toContain('Starting…')
    expect(render({ label: 'Carry on with Claude Code', busy: false, refused: 'This turn was already carried on.' })).toContain('This turn was already carried on.')
  })
})
