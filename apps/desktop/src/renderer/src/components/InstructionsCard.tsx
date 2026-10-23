import { useState, type ReactElement } from 'react'

import { MAX_TEAMMATE_INSTRUCTIONS } from '../../../shared/ipc.js'
import { secretIn } from '../../../shared/secrets.js'
import { Icon } from './Icon.js'

/*
 * WHAT AN INTERVIEW WROTE (0.831, shared/interview.ts): under the teammate's last answer of an interview, the
 * instructions it wrote from the person's answers, in a box they can change, and Save. Nothing is kept until they
 * press it; a key in them is refused here, since instructions go to the teammate's model on every turn.
 */
/** Why these instructions are not saved: a key in them, which the teammate's model would be given every turn. */
export function instructionsRefusal(name: string, text: string): string | undefined {
  const kind = secretIn(text)
  return kind === undefined ? undefined : `They hold ${kind}, and ${name}’s model is given them on every turn, so they were not saved. Take it out first.`
}

export function InstructionsCard({ name, written, current, onSave }: {
  readonly name: string
  readonly written: string
  /** The teammate's instructions now, when it has some: the saved ones take their place. */
  readonly current?: string
  /** Resolves with why they were not saved, or nothing when they were. */
  readonly onSave: (instructions: string) => Promise<string | undefined>
}): ReactElement {
  const [text, setText] = useState(written)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string>()
  const [saved, setSaved] = useState(current !== undefined && current.trim() === written.trim())
  const trimmed = text.trim()
  if (saved) {
    return (
      <div className="lc-diagnostic lc-tone-muted lc-interview" role="status">
        <Icon name="check" size={12} />
        <span>{`Saved as ${name}’s instructions. ${name} follows them from its next turn; change them any time in ${name}’s settings.`}</span>
      </div>
    )
  }
  return (
    <div className="lc-diagnostic lc-tone-muted lc-interview" role="group" aria-label={`${name}’s instructions from the interview`}>
      <Icon name="pencil" size={12} />
      <div className="lc-editcheck__body lc-interview__body">
        <span>{`${name} wrote its own instructions from your answers. Change anything, then save them: every turn ${name} takes is given these.`}</span>
        <textarea className="lc-input lc-interview__text" aria-label={`${name}’s instructions`} rows={6} maxLength={MAX_TEAMMATE_INSTRUCTIONS} value={text} disabled={busy}
          onChange={(event) => { setText(event.target.value); setProblem(undefined) }} />
        {current !== undefined && current.trim().length > 0 && <span className="lc-interview__note">{`They take the place of the instructions ${name} has now.`}</span>}
        {problem !== undefined && <span className="lc-interview__note lc-tone-red" role="alert">{problem}</span>}
        <button type="button" className="lc-button" disabled={busy || trimmed.length === 0} onClick={() => {
          const refused = instructionsRefusal(name, trimmed)
          if (refused !== undefined) { setProblem(refused); return }
          setBusy(true)
          void onSave(trimmed)
            .then((why) => { if (why === undefined) setSaved(true); else setProblem(why) })
            .catch(() => setProblem(`They could not be saved. ${name}’s instructions are unchanged.`))
            .finally(() => setBusy(false))
        }}>{`Save as ${name}’s instructions`}</button>
      </div>
    </div>
  )
}
