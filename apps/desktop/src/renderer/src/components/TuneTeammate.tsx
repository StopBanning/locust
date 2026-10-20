import { useRef, type ReactElement } from 'react'
import { MAX_TEAMMATE_INSTRUCTIONS } from '../../../shared/ipc.js'
import type { TeammateRoute } from '../../../shared/ipc.js'
import { TUNE_DRAFT_COST, type TuneDrafts } from '../../../shared/tune.js'
import { useModal } from '../useModal.js'

export interface TuneCard {
  readonly id: number
  readonly teammateId: string
  readonly route: TeammateRoute
  readonly name: string
  readonly model: string
  readonly free: boolean
  readonly prompt: string
  readonly current: string
  readonly drafts: TuneDrafts
  readonly drafting: boolean
  readonly draftRunId?: string
  readonly starting?: boolean
  readonly problem?: string
}

/** TUNE THIS TEAMMATE (0.808): review the edits before three trials, and never save instructions here. */
export function TuneTeammate({ card, onChange, onRun, onCancel }: {
  readonly card: TuneCard
  readonly onChange: (edit: (current: TuneCard) => TuneCard) => void
  readonly onRun: () => void
  readonly onCancel: () => void
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-tune" role="dialog" aria-modal="true" aria-label={`Tune ${card.name}`}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Tune this teammate</span>
          <span className="lc-dialog__sub">{card.name}</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" disabled={card.starting} onClick={onCancel}>×</button>
        </div>
        <div className="lc-dialog__body lc-tune__body">
          <label className="lc-tune__ask">Ask to run again
            <textarea className="lc-input" aria-label="Ask to run again" rows={2} maxLength={8000} value={card.prompt} disabled={card.starting}
              onChange={(event) => { const prompt = event.target.value; onChange((current) => ({ ...current, prompt })) }} />
          </label>
          <p className="lc-dialog__note">{TUNE_DRAFT_COST} {card.drafting ? 'Drafting the two edits…' : 'Review and edit all three before running. Keeping Current is a valid outcome.'}</p>
          <div className="lc-tune__columns">
            {(['current', 'how', 'knows'] as const).map((key) => {
              const title = key === 'current' ? 'Current' : key === 'how' ? 'How it works' : 'What it knows'
              const text = key === 'current' ? card.current : card.drafts[key].instructions
              return <label className="lc-tune__column" key={key}>
                <span className="lc-tune__title">{title}</span>
                <span className="lc-dialog__note lc-tune__why">{key === 'current' ? 'The teammate’s own standing instructions. Empty is allowed.' : card.drafts[key].why}</span>
                <textarea className="lc-input lc-tune__instructions" aria-label={`${title} instructions`} rows={8} maxLength={MAX_TEAMMATE_INSTRUCTIONS}
                  value={text} disabled={card.starting || (key !== 'current' && card.drafting)}
                  onChange={(event) => { const text = event.target.value; onChange((current) => key === 'current' ? { ...current, current: text } : { ...current, drafts: { ...current.drafts, [key]: { ...current.drafts[key], instructions: text } } }) }} />
                <span className="lc-dialog__note lc-mono">{text.length} / {MAX_TEAMMATE_INSTRUCTIONS}</span>
              </label>
            })}
          </div>
          <p className="lc-tune__cost">Three runs of {card.model}{card.free ? ' — free.' : ' — counts against your plan three times.'} Each works in its own copy where Compare supports one; you choose what to keep.</p>
          {card.problem !== undefined && <p className="lc-dialog__error" role="alert">{card.problem}</p>}
        </div>
        <div className="lc-dialog__foot">
          <button type="button" className="lc-ghostbutton" disabled={card.starting} onClick={onCancel}>Cancel</button>
          <button type="button" className="lc-primarybutton" disabled={card.drafting || card.starting || card.prompt.trim().length === 0} onClick={onRun}>
            {card.starting ? 'Starting…' : 'Run all three'}
          </button>
        </div>
      </div>
    </div>
  )
}
