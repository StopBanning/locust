import { useState } from 'react'
import type { ReactElement } from 'react'

import type { NextStep } from '../../../shared/next-steps.js'

/**
 * THE TASKS A REPLY SUGGESTS, AS CARDS (2026-10-30, shared/next-steps.ts). Colin, of Claude Code's "2 suggested
 * tasks" under a reply: "very cool feature there, can we add that to locust as well?" Each card has the title, the
 * brief in two lines, a cross that puts it away, Do it here (the next turn of this conversation) and Start (a new
 * conversation with the same teammate); Start all starts each. Nothing runs until a press: the cards are words.
 *
 * Drawn only on the turn on screen, and only while it is not running; a turn that is still going has not finished
 * suggesting, and Do it here would have nowhere to go.
 */
export function NextSteps({ steps, onHere, onStart }: {
  readonly steps: readonly NextStep[]
  /** The brief as this conversation's next turn. Absent where it cannot be sent now. */
  readonly onHere?: (step: NextStep) => void
  /** A new conversation with the same teammate, on this brief. */
  readonly onStart?: (step: NextStep) => void
}): ReactElement | null {
  const [gone, setGone] = useState<ReadonlySet<string>>(() => new Set())
  const shown = steps.filter((step) => !gone.has(step.title))
  if (shown.length === 0) return null
  const away = (step: NextStep): void => setGone((before) => new Set([...before, step.title]))
  return (
    <section className="lc-nextsteps" aria-label="Suggested tasks">
      <div className="lc-nextsteps__head">
        <span className="lc-nextsteps__count">{shown.length === 1 ? '1 suggested task' : `${String(shown.length)} suggested tasks`}</span>
        {onStart !== undefined && shown.length > 1 && (
          <button
            type="button"
            className="lc-linkbutton"
            onClick={() => {
              for (const step of shown) onStart(step)
              setGone(new Set(steps.map((step) => step.title)))
            }}
          >
            Start all
          </button>
        )}
      </div>
      {shown.map((step) => (
        <div className="lc-nextstep" key={step.title}>
          <div className="lc-nextstep__words">
            <span className="lc-nextstep__title">{step.title}</span>
            <span className="lc-nextstep__brief" title={step.brief}>{step.brief}</span>
          </div>
          <button type="button" className="lc-iconbutton lc-nextstep__away" aria-label={`Put away: ${step.title}`} title="Put it away" onClick={() => away(step)}>
            ×
          </button>
          <div className="lc-nextstep__actions">
            {onHere !== undefined && (
              <button type="button" className="lc-button lc-nextstep__here" onClick={() => { away(step); onHere(step) }}>
                Do it here
              </button>
            )}
            {onStart !== undefined && (
              <button type="button" className="lc-button" onClick={() => { away(step); onStart(step) }}>
                Start
              </button>
            )}
          </div>
        </div>
      ))}
    </section>
  )
}
