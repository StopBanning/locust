import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { CHECK_FIX_STOPPED, checkFixPrompt } from '../../shared/check-fix.js'
import { CheckFixLine } from './components/CheckFixLine.js'
import { EditCheckCard } from './components/EditCheckCard.js'
import { CheckAfterEditsRow } from './components/Screens.js'
import { Thread } from './components/Thread.js'
import { buildThread, fixOfTurn, turnPromptLine } from './missionView.js'

/*
 * FIX UNTIL THE CHECK PASSES (0.812). A turn Locust sent back to fix a failing check is Locust's: nobody typed it, so
 * no bubble in the person's voice -- a line that says what failed and which fix it is, with what Locust sent a click
 * away. And the choice that turns it on is the folder's, in Settings, beside the check itself.
 */

const fix1 = { kind: 'check-fix', fix: 1, of: 2, command: 'npm test' } as const
const fix2 = { kind: 'check-fix', fix: 2, of: 2, command: 'npm test' } as const
const sent1 = checkFixPrompt({ command: 'npm test', lines: ['FAIL adds two numbers'], first: false }, 1, 2)
const sent2 = checkFixPrompt({ command: 'npm test', lines: ['FAIL adds three numbers'], first: false }, 2, 2)

const answered = (missionId: string, text: string): readonly NormalizedRuntimeEvent[] =>
  [{ id: `${missionId}_said`, runId: `run_${missionId}`, missionId, sequence: 1, occurredAt: '2026-10-21T20:00:00.000Z', sourceAdapter: 'opencode', type: 'message.delta', payload: { itemId: `${missionId}_m`, text, final: true } }] as unknown as readonly NormalizedRuntimeEvent[]

const stopped = (missionId: string, message: string): NormalizedRuntimeEvent =>
  ({ id: `${missionId}_note`, runId: `run_${missionId}`, missionId, sequence: 2, occurredAt: '2026-10-21T20:00:05.000Z', sourceAdapter: 'opencode', type: 'adapter.diagnostic',
    payload: { level: 'warning', code: 'host.check_fix', message, terminal: false, evidence: { redacted: true } } }) as unknown as NormalizedRuntimeEvent

const thread = (input: { readonly earlierTurns: readonly unknown[]; readonly prompt: string; readonly startedBy?: unknown; readonly events: readonly NormalizedRuntimeEvent[] }): string =>
  renderToStaticMarkup(
    <Thread
      prompt={input.prompt}
      onOpenPeerRun={() => undefined}
      earlierTurns={input.earlierTurns as never}
      events={input.events as never}
      running={false}
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
      shownMissionId="m_fix2"
      peers={{ self: undefined, teammates: [], messages: [], notices: [] }}
      {...(input.startedBy === undefined ? {} : { startedBy: input.startedBy as never })}
      runMode="accept-edits"
    />
  )

describe('a turn sent back to fix the check is said as Locust\'s', () => {
  it('draws no bubble in the person\'s voice', () => {
    expect(turnPromptLine({ prompt: sent1, startedBy: fix1 })).toBeUndefined()
  })

  it('says what failed and which fix it is, for an earlier turn and the one on screen, with what was sent a click away', () => {
    const html = thread({
      earlierTurns: [
        { missionId: 'm_person', prompt: 'Add a sum function.', events: answered('m_person', 'Added sum.') },
        { missionId: 'm_fix1', prompt: sent1, events: answered('m_fix1', 'Fixed the off-by-one.'), startedBy: fix1 }
      ],
      prompt: sent2,
      startedBy: fix2,
      events: answered('m_fix2', 'Fixed the three-number case.')
    })
    expect(html).toContain('Add a sum function.')
    expect(html).toContain('</code> failed, so Locust sent it back</span>')
    expect(html).toContain('· fix 1 of 2')
    expect(html).toContain('</code> still failed, so Locust sent it back again</span>')
    expect(html).toContain('· fix 2 of 2')
    expect(html.match(/What Locust sent/g)).toHaveLength(2)
    // What was sent is the fold-out's, never a bubble: one bubble in the whole thread, the person's.
    expect(html.match(/class="lc-bubble"/g)).toHaveLength(1)
    expect(html).toContain('FAIL adds three numbers')
  })

  it('a routine’s own fix turn is Locust’s too, read from its words; a routine’s step stays the person’s', () => {
    const routine = { kind: 'routine', routineId: 'rt_x', step: 1 } as const
    // routine-runner.ts goalFixPrompt's words; a-failing-check-goes-back-to-fix.test.ts holds the runner to them.
    const fix = [
      'The check `node check.js` failed after the work above (fix 1 of 2). The end of its output:',
      '',
      'CHECK FAILED: answer.txt says 41',
      '',
      'Fix what it reports in this folder, then say in one or two sentences what you changed.'
    ].join('\n')
    expect(fixOfTurn({ prompt: fix, startedBy: routine })).toEqual({ command: 'node check.js', fix: 1, of: 2 })
    expect(turnPromptLine({ prompt: fix, startedBy: routine })).toBeUndefined()
    expect(turnPromptLine({ prompt: 'Create answer.txt holding 41.', startedBy: routine })).toBe('Create answer.txt holding 41.')
    // The same words typed by the person are the person's: only a routine's turn is read this way.
    expect(fixOfTurn({ prompt: fix })).toBeUndefined()
  })

  it('names whom it went back to, when the conversation has a teammate', () => {
    expect(renderToStaticMarkup(<CheckFixLine command="npm test" fix={1} of={2} prompt={sent1} teammateName="Wren" />)).toContain('failed, so Locust sent it back to Wren</span>')
    expect(renderToStaticMarkup(<CheckFixLine command="npm test" fix={2} of={2} prompt={sent2} teammateName="Wren" />)).toContain('still failed, so Locust sent it back to Wren again</span>')
  })

  it('says why it stopped under the turn it stopped after, when the conversation is opened again', () => {
    const items = buildThread([...answered('m_fix2', 'Tried again.'), stopped('m_fix2', CHECK_FIX_STOPPED.outOfFixes(2))], { running: false })
    expect(items.some((item) => item.type === 'diagnostic' && item.message === CHECK_FIX_STOPPED.outOfFixes(2))).toBe(true)
  })

  it('a pass after a fix is the fix\'s; after the person\'s own turn it is the turn\'s', () => {
    const check = { command: 'npm test', outcome: 'passed', newLines: [], unchanged: false, first: false } as const
    expect(renderToStaticMarkup(<EditCheckCard check={check} teammateName="Wren" fix={2} />)).toContain('passed after fix 2.')
    expect(renderToStaticMarkup(<EditCheckCard check={check} teammateName="Wren" />)).toContain('passed after this turn.')
  })
})

describe('the folder\'s choice, beside its check', () => {
  const row = (saved: string, fixTries?: number): string =>
    renderToStaticMarkup(
      <CheckAfterEditsRow saved={saved} onSave={() => undefined} {...(fixTries === undefined ? {} : { fixTries })} onFixTriesChange={() => undefined} />
    )

  it('is off until chosen: the failure is shown, with the button', () => {
    const html = row('npm test')
    expect(html).toContain('When it fails')
    expect(html).toMatch(/aria-checked="true"[^>]*>Show me</)
    expect(html).toContain('with a button to send it to the teammate')
    expect(html).not.toContain('at most')
  })

  it('when on, says how many fixes at most and that each is a turn', () => {
    const html = row('npm test', 3)
    expect(html).toMatch(/aria-checked="true"[^>]*>Send it back to fix</)
    expect(html).toMatch(/aria-checked="true" aria-label="3 fixes"/)
    expect(html).toContain('3 times at most, and it stops sooner when a fix changes nothing. Each fix is one more turn.')
    expect(row('npm test', 1)).toContain('1 time at most')
  })

  it('cannot be turned on before there is a check to run', () => {
    const html = row('')
    expect(html).toMatch(/disabled=""[^>]*>Send it back to fix</)
    expect(html).toContain('Save a check first.')
  })
})
