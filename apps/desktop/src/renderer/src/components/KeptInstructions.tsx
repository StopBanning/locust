import { useState, type ReactElement } from 'react'
import type { PublicCompare } from '../../../shared/compare.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import {
  combineSetups,
  isKeptTune,
  keptInstructions,
  tuneLessonLine,
  TUNE_COMBINE_FALLBACK,
  TUNE_LESSON_COST,
  type PublicTuneLesson
} from '../../../shared/tune.js'

export interface NoteTuneOutcome {
  readonly lesson?: PublicTuneLesson | null
  readonly message?: string
}

/** What Combine both edits came back with (0.811): the combined words and a line on each edit, or why there are none. */
export type CombineTuneOutcome = { readonly instructions: string; readonly note: string } | { readonly message: string }

/**
 * KEEP IS NOT SAVE (0.809), AND REFLECTION RUNS ONLY ON PRESS (0.810).
 *
 * The person's explicit choice saves instructions for future turns, and Note what this tune showed starts one side
 * turn on the teammate's own route only when pressed, whether or not the person used the instructions.
 */
export function KeptInstructions({
  compare,
  teammate,
  onUse,
  lesson,
  noting,
  model,
  free,
  onNote,
  onRemoveLesson,
  onCombine
}: {
  readonly compare: PublicCompare
  readonly teammate: PublicTeammate | undefined
  readonly onUse: (teammate: PublicTeammate, instructions: string) => Promise<string | undefined>
  readonly lesson?: PublicTuneLesson | undefined
  readonly noting?: boolean | undefined
  readonly model?: string | undefined
  readonly free?: boolean | undefined
  readonly onNote?: ((compare: PublicCompare, usedInstructions: boolean) => Promise<NoteTuneOutcome | undefined>) | undefined
  readonly onRemoveLesson?: ((lessonId: string) => Promise<string | undefined>) | undefined
  readonly onCombine?: ((compare: PublicCompare) => Promise<CombineTuneOutcome>) | undefined
}): ReactElement | null {
  const [dismissed, setDismissed] = useState(false)
  const [applied, setApplied] = useState(false)
  const [busy, setBusy] = useState(false)
  const [localNoting, setLocalNoting] = useState(false)
  const [localLesson, setLocalLesson] = useState<PublicTuneLesson | null | undefined>(undefined)
  const [noteMessage, setNoteMessage] = useState<string>()
  const [problem, setProblem] = useState<string>()
  const [combining, setCombining] = useState(false)
  const [combined, setCombined] = useState<{ readonly instructions: string; readonly note: string }>()
  const [combineMessage, setCombineMessage] = useState<string>()
  if (!isKeptTune(compare, teammate)) return null
  // COMBINE BOTH EDITS (0.811): the kept draft's edit with the other draft's, written by one side turn, offered as
  // the kept instructions are -- read, then used or not; nothing is saved until Use is pressed.
  const setups = combineSetups(compare)
  const kept = keptInstructions(compare, teammate)
  const instructions = combined?.instructions ?? kept
  const offerInstructions = !applied && !dismissed && instructions !== undefined
  const shownLesson = localLesson === null ? undefined : (lesson ?? localLesson)
  const isNoting = noting === true || localNoting
  const routeLabel =
    model ??
    compare.slots[0]?.route.label?.replace(/ · (Current|How it works|What it knows)$/, '') ??
    teammate.route?.model ??
    'this model'
  const routeFree =
    free ?? (teammate.route?.runtime === 'opencode' && /(^|[-/])free$/i.test(teammate.route.model))
  // Either side turn holds the other: one at a time on the teammate's route.
  const sideBusy = busy || isNoting || combining
  const combine = (): void => {
    if (sideBusy) return
    setCombining(true)
    setCombineMessage(undefined)
    setProblem(undefined)
    const run: Promise<CombineTuneOutcome> = onCombine
      ? onCombine(compare)
      : window.desktop === undefined
        ? Promise.resolve({ message: 'Locust is not ready. Nothing was combined.' })
        : window.desktop.combineTuneEdits(compare.compareId).then((res) => (res.ok ? { instructions: res.data.instructions, note: res.data.note } : { message: res.message }))
    void run
      .then((outcome) => {
        if ('instructions' in outcome) {
          setCombined(outcome)
          setApplied(false)
          setDismissed(false)
        } else {
          setCombineMessage(outcome.message)
        }
      })
      .catch(() => setCombineMessage(TUNE_COMBINE_FALLBACK))
      .finally(() => setCombining(false))
  }
  const combineButton = setups === undefined || combined !== undefined ? null : (
    <button type="button" className="lc-ghostbutton" disabled={sideBusy} onClick={combine}>
      {combining ? 'Combining the two edits…' : 'Combine both edits'}
    </button>
  )
  const combineSaid = combineMessage === undefined ? null : (
    <span className="lc-tuneoffer__note" role="status">
      {combineMessage}
    </span>
  )
  // One row (0.809): what happened, then the two choices side by side; the words themselves fold out beneath.
  // Beneath it (0.810): Note what this tune showed with its cost on the button's line, or the lesson with Remove;
  // and Combine both edits (0.811) on that line too, its own when a lesson stands there.
  return (
    <div className="lc-tuneoffer">
      {applied && (
        <span className="lc-tuneoffer__say" role="status">
          These instructions take effect from the next turn. You can edit them in Edit teammate.
        </span>
      )}
      {offerInstructions && (
        <span className="lc-tuneoffer__say">
          {combined !== undefined && setups !== undefined
            ? `Combined ${setups.kept.name} with ${setups.other.name}: ${combined.note}`
            : `The setup you kept uses different instructions from ${teammate.name}’s own.`}
        </span>
      )}
      {offerInstructions && (
        <span className="lc-tuneoffer__actions">
          <button
            type="button"
            className="lc-ghostbutton"
            disabled={busy}
            onClick={() => {
              if (busy) return
              setBusy(true)
              void onUse(teammate, instructions)
                .then((why) => {
                  if (why === undefined) setApplied(true)
                  else setProblem(why)
                })
                .catch(() => setProblem('The instructions could not be saved. The teammate is unchanged.'))
                .finally(() => setBusy(false))
            }}
          >
            {combined === undefined ? `Use these instructions for ${teammate.name}` : `Use the combined instructions for ${teammate.name}`}
          </button>
          <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => setDismissed(true)}>
            Not now
          </button>
        </span>
      )}
      {offerInstructions && (
        <details className="lc-tuneoffer__review">
          <summary>Review the instructions</summary>
          <pre>{instructions.length === 0 ? 'No standing instructions.' : instructions}</pre>
        </details>
      )}
      {shownLesson !== undefined ? (
        <div className="lc-tuneoffer__lesson" role="status">
          <span className="lc-tuneoffer__lesson-text">{tuneLessonLine(shownLesson)}</span>
          <button
            type="button"
            className="lc-ghostbutton"
            disabled={sideBusy}
            onClick={() => {
              if (sideBusy) return
              setBusy(true)
              setProblem(undefined)
              const remove = onRemoveLesson
                ? onRemoveLesson(shownLesson.lessonId)
                : window.desktop === undefined
                  ? Promise.resolve(undefined)
                  : window.desktop.removeTuneLesson(shownLesson.lessonId).then((res) => (res.ok ? undefined : res.message))
              void remove
                .then((why) => {
                  if (why === undefined) {
                    setLocalLesson(null)
                    setNoteMessage(undefined)
                  } else {
                    setProblem(why)
                  }
                })
                .catch(() => setProblem('That tune lesson could not be removed. Your lessons are unchanged.'))
                .finally(() => setBusy(false))
            }}
          >
            Remove
          </button>
        </div>
      ) : null}
      {shownLesson !== undefined && combineButton !== null && (
        <div className="lc-tuneoffer__note-row">
          <span className="lc-tuneoffer__cost">{TUNE_LESSON_COST(routeLabel, routeFree)}</span>
          {combineButton}
          {combineSaid}
        </div>
      )}
      {shownLesson !== undefined ? null : (
        <div className="lc-tuneoffer__note-row">
          <span className="lc-tuneoffer__cost">{TUNE_LESSON_COST(routeLabel, routeFree)}</span>
          <button
            type="button"
            className="lc-ghostbutton"
            disabled={sideBusy}
            onClick={() => {
              if (sideBusy) return
              setLocalNoting(true)
              setLocalLesson(undefined)
              setProblem(undefined)
              setNoteMessage(undefined)
              const used = applied || kept === undefined
              const run = onNote
                ? onNote(compare, used)
                : window.desktop === undefined
                  ? Promise.resolve(undefined)
                  : window.desktop.noteTuneLesson(compare.compareId, used).then((res) =>
                      res.ok
                        ? { lesson: res.data.lesson, ...(res.data.message === undefined ? {} : { message: res.data.message }) }
                        : { lesson: null, message: res.message }
                    )
              void run
                .then((outcome) => {
                  if (outcome?.lesson !== undefined && outcome.lesson !== null) {
                    setLocalLesson(outcome.lesson)
                  } else if (outcome?.message !== undefined) {
                    setNoteMessage(outcome.message)
                  }
                })
                .catch(() => setNoteMessage('That reflection could not be read as JSON. Nothing was saved.'))
                .finally(() => setLocalNoting(false))
            }}
          >
            {isNoting ? 'Noting what this tune showed…' : 'Note what this tune showed'}
          </button>
          {combineButton}
          {noteMessage !== undefined && (
            <span className="lc-tuneoffer__note" role="status">
              {noteMessage}
            </span>
          )}
          {combineSaid}
        </div>
      )}
      {problem !== undefined && (
        <span className="lc-tone-red" role="alert">
          {problem}
        </span>
      )}
    </div>
  )
}
