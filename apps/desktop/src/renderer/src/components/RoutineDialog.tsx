import { useEffect, useRef, useState } from 'react'
import { BOT_SIZE } from '../botSizes.js'
import type { ReactElement } from 'react'
import { useModal } from '../useModal.js'

import type { PublicTeammate, RoutineHandOff, RoutineSchedule } from '../../../shared/ipc.js'
import { EVERY_HOURS_CHOICES, onceMoment, watchedFolderValid, WEEKDAYS_ONLY } from '../../../shared/routine-schedule.js'
import { stepTooLongNotice } from '../../../shared/step-budget.js'
import { MAX_ROUTINE_STEPS } from '../routines.js'
import { Select } from './Select.js'
import { TeammateBot } from './TeammateBot.js'
import { RoutineInputEditor } from './RoutineInputs.js'
import { inputsRefusal } from '../../../shared/routine-inputs.js'
import type { RoutineInput } from '../../../shared/routine-inputs.js'
import { HOOK_WARNING, HOOK_ASK, type RoutineHookState } from '../../../shared/routine-hook.js'

/** THE LINK IS A PASSWORD (2026-10-25). Shown in this editor on this computer only. A browser may save the hook kind and is told the link stays here: no password, no Copy, no New link. */
const LINK_ON_THIS_COMPUTER = 'The link is shown only in Locust on your computer.'
export function RoutineHookLink({ routineId }: { readonly routineId?: string }): ReactElement {
  const here = typeof window === 'undefined' || window.desktop?.remote === undefined
  const [state, setState] = useState<RoutineHookState>({ message: routineId === undefined ? 'Save the routine to make its link. Your phone, from anywhere must be on in Settings > Phone.' : 'Checking the phone relay…' })
  const [busy, setBusy] = useState(false)
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!here) return
    let live = true
    if (routineId !== undefined) void window.desktop?.readRoutineHook(routineId).then((answer) => { if (live) setState(answer.ok ? answer.data : { message: answer.error.message }) }).catch(() => { if (live) setState({ message: 'The link could not be read. Open this editor again to try.' }) })
    return () => { live = false }
  }, [routineId, here])
  if (!here) return <div className="lc-routinesched__days">
    <p className="lc-dialog__note lc-mono">{LINK_ON_THIS_COMPUTER}</p>
    <p className="lc-dialog__note lc-mono">{HOOK_WARNING}</p>
    <p className="lc-dialog__note lc-mono">{HOOK_ASK}</p>
  </div>
  const replace = async (): Promise<void> => {
    if (routineId === undefined || !window.desktop) return
    setBusy(true)
    setCopied(false)
    try {
      const answer = await window.desktop.newRoutineHook(routineId)
      setState(answer.ok ? answer.data : { message: answer.error.message })
    } catch { setState({ message: 'The relay did not confirm a new link. Check Settings > Phone and try again.' }) }
    finally { setBusy(false) }
  }
  return <div className="lc-routinesched__days">
    {state.url !== undefined && <>
      <input className="lc-input" aria-label="Routine link" type="text" readOnly value={state.url} />
      <button type="button" className="lc-ghostbutton" onClick={() => { void navigator.clipboard.writeText(state.url!).then(() => setCopied(true), () => setCopied(false)) }}>{copied ? 'Copied' : 'Copy'}</button>
      <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => { void replace() }}>New link</button>
    </>}
    {state.message !== undefined && <p className="lc-dialog__note lc-mono">{state.message}</p>}
    {state.url === undefined && state.canMake === true && routineId !== undefined && <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => { void replace() }}>Make link</button>}
    <p className="lc-dialog__note lc-mono">{HOOK_WARNING}</p>
    <p className="lc-dialog__note lc-mono">{HOOK_ASK}</p>
  </div>
}

/** Monday first, as a week reads; 0 is Sunday, as the schedule counts. */
const WEEK_ORDER = [1, 2, 3, 4, 5, 6, 0] as const
const DAY_NAMES = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as const

/** What "Once" starts with: tomorrow at 09:00, in the person's clock. */
function tomorrowAtNine(now = new Date()): string {
  const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1)
  const two = (value: number): string => String(value).padStart(2, '0')
  return `${String(day.getFullYear())}-${two(day.getMonth() + 1)}-${two(day.getDate())}T09:00`
}

/**
 * Saving a conversation as a routine, and correcting one.
 *
 * The steps arrive already filled in with what the person typed on each turn:
 * the point of the feature is that they do not have to write it again. They
 * are editable here because a correction is the whole way a routine improves
 * -- "it saves your workflow as a routine, takes your corrections" -- and the
 * record of what actually happened is untouched either way.
 */
export function RoutineDialog({
  teammate,
  chooseFrom,
  initialName,
  folder,
  initialSteps,
  initialSchedule,
  team,
  initialHandOffs,
  initialStepRoles,
  truncated,
  routeLabel,
  busy,
  error,
  onSave,
  onCancel,
  editing: editingSaved,
  modeName,
  modeReadsOnly,
  fresh = false,
  initialInCopy = false,
  initialGoalTries,
  checkCommand,
  running = false,
  initialInputs = [],
  routineId
}: {
  readonly routineId?: string
  /** A standing goal's fixes, when it has one (0.534). */
  readonly initialGoalTries?: number
  /** This folder's check command from Settings, or undefined for none (0.534). */
  readonly checkCommand?: string
  /** It works in a copy, kept or discarded by the person (0.533). */
  readonly initialInCopy?: boolean
  /** Written from nothing on the Routines screen (0.530), not saved from a conversation. */
  readonly fresh?: boolean
  /** A saved routine is being corrected (0.493); absent, read off the other props as before. */
  readonly editing?: boolean
  /** The mode it runs in, saved with it, in the picker's word: "Ask", "Edit", "Auto". */
  readonly modeName?: string
  /** That mode only reads (Ask, Plan): what a run may do to the folder is said from this, never assumed. */
  readonly modeReadsOnly?: boolean
  /** A run of it is going now: that run keeps the steps it started with. */
  readonly running?: boolean
  readonly initialInputs?: readonly RoutineInput[]
  readonly teammate: PublicTeammate | undefined
  /**
   * Who could run it, when the conversation had no owner to inherit.
   *
   * A conversation can be started with nobody picked -- that is half the
   * product -- and until now that was the one thing you could not save as a
   * routine. The steps were never the problem: they are the words the person
   * typed, which exist either way. What was missing is whose route replays
   * them, so the dialog asks, once, here.
   */
  readonly chooseFrom?: readonly PublicTeammate[]
  readonly initialName: string
  /** The folder a schedule runs it in (0.512): its own, or the one it is being made in. */
  readonly folder?: { readonly name: string; readonly path: string }
  readonly initialSteps: readonly string[]
  /** When it runs on its own, if it does. Undefined: only when a person presses Run. */
  readonly initialSchedule: RoutineSchedule | undefined
  /**
   * The whole team, so a step can be handed to another teammate (0.435, a
   * hand-off chain). Absent, or one teammate: no choice is drawn.
   */
  readonly team?: readonly PublicTeammate[]
  /** Who takes each step of a saved routine, in step order. */
  readonly initialHandOffs?: readonly RoutineHandOff[]
  /** The role a chain template named for each step, shown beside who was proposed for it. */
  readonly initialStepRoles?: readonly (string | undefined)[]
  /** The conversation had more turns than a routine may hold, and the draft was cut. */
  readonly truncated: boolean
  /** The route this will replay on, in the words the picker uses. */
  readonly routeLabel: string | undefined
  readonly busy: boolean
  readonly error: string | undefined
  readonly onSave: (input: {
    readonly name: string
    readonly steps: readonly string[]
    readonly inputs?: readonly RoutineInput[]
    readonly schedule: RoutineSchedule | undefined
    /** Set only when the person changed what a run may do (0.530). */
    readonly readsOnly?: boolean
    /** Where it works (0.533); sent on a new routine, and on an edit only when it moved. */
    readonly inCopy?: boolean
    /** Keep going until the check passes (0.534): fixes allowed, or null for none; absent, unchanged. */
    readonly untilCheck?: { readonly tries: number } | null
    readonly teammateId?: string
    /** Who takes each step, in step order; absent when no choice was offered. */
    readonly handOffs?: readonly RoutineHandOff[]
  }) => void
  readonly onCancel: () => void
}): ReactElement {
  const editing = editingSaved ?? (initialSteps.length > 0 && routeLabel === undefined)
  const title = editing ? 'Edit routine' : fresh ? 'New routine' : 'Save as routine'
  const [name, setName] = useState(initialName)
  const [steps, setSteps] = useState<readonly string[]>(initialSteps)
  const [inputs, setInputs] = useState<readonly RoutineInput[]>(initialInputs)
  const inputsChanged = JSON.stringify(inputs) !== JSON.stringify(initialInputs)
  // What a run may do to the folder (0.530): chosen here, not inherited unseen.
  const [readsOnly, setReadsOnly] = useState(modeReadsOnly === true)
  // Where a run that changes files works (0.533): the folder, or a copy the person keeps.
  const [inCopy, setInCopy] = useState(initialInCopy)
  const copies = !readsOnly && inCopy
  // A standing goal (0.534): keep going until the folder's check passes, at most this many fixes.
  const [goalTries, setGoalTries] = useState<number | undefined>(initialGoalTries)
  const hasCheck = checkCommand !== undefined && checkCommand.trim().length > 0
  const goal = !readsOnly && goalTries !== undefined ? goalTries : undefined
  const shownMode = readsOnly ? 'Ask' : modeReadsOnly === true ? 'Edit' : modeName
  // One entry per step, kept in step with every add and remove (0.435).
  const [handOffs, setHandOffs] = useState<readonly RoutineHandOff[]>(
    initialSteps.map((_, index) => initialHandOffs?.[index] ?? {})
  )
  // The role a chain template named for each step, kept in step with every add and remove.
  const [stepRoles, setStepRoles] = useState<readonly (string | undefined)[]>(initialStepRoles ?? [])
  const [schedule, setSchedule] = useState<RoutineSchedule | undefined>(initialSchedule)
  // The watched folder as typed, kept while it is not yet a valid one (0.522).
  const [filesFolder, setFilesFolder] = useState(initialSchedule?.kind === 'files' ? initialSchedule.folder : 'inbox')
  // Nobody is picked to begin with: a default here would put a teammate's
  // name on work they were never part of, which is the one thing the whole
  // ownerless path exists to avoid.
  const [runner, setRunner] = useState<string>('')
  const mustPick = chooseFrom !== undefined && chooseFrom.length > 0 && teammate === undefined
  // Nobody to pick (0.658): the note says to add a teammate first, so Save waits too. It was live and
  // answered "Choose which teammate runs this routine" with no list to choose from (Flash's review).
  const nobodyToRun = chooseFrom !== undefined && chooseFrom.length === 0 && teammate === undefined
  const scheduleKind = schedule?.kind ?? 'off'
  // Who runs the routine, and so who takes a step nobody else is named for.
  const ownerId = teammate?.teammateId ?? (runner.length > 0 ? runner : undefined)
  const ownerName = teammate?.name ?? chooseFrom?.find((entry) => entry.teammateId === runner)?.name
  // A chain from a template keeps its checker even for one teammate: every step is theirs, and the last still has to approve.
  const canHandOff = team !== undefined && (team.length > 1 || (initialHandOffs ?? []).some((entry) => entry.check === true))
  const asksFirst = handOffs.some((entry) => entry.askFirst === true) || (initialHandOffs ?? []).some((entry) => entry.askFirst === true)
  const setHandOff = (index: number, change: RoutineHandOff): void => {
    setHandOffs(steps.map((_, at) => (at === index ? change : handOffs[at] ?? {})))
  }

  const kept = steps.filter((step) => step.trim().length > 0)
  // The first step too long to send, by its number on screen (A5.1).
  const tooLongAt = steps.findIndex((step) => stepTooLongNotice(step.trim()) !== undefined)
  // Legacy routines may contain literal braces. Unchanged declarations stay untouched.
  const inputError = inputsRefusal(inputs, editing && initialInputs.length === 0 && !inputsChanged ? undefined : kept)
  const canSave = name.trim().length > 0 && kept.length > 0 && tooLongAt < 0 && inputError === undefined && !busy && !nobodyToRun && (!mustPick || runner.length > 0)
  // Focus in, Tab held inside, Escape closes, focus back to the opener.
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onCancel)

  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog" role="dialog" aria-modal="true" aria-label={editing ? 'Edit routine' : fresh ? 'New routine' : 'Save as routine'}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">{title}</span>
          <span className="lc-dialog__sub lc-mono">replayed step by step</span>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onCancel}>
            ×
          </button>
        </div>

        <div className="lc-dialog__body">
          <div className="lc-dialog__fields">
            <label className="lc-fieldlabel lc-mono" htmlFor="routine-name">
              Name
            </label>
            <input
              id="routine-name"
              className="lc-input"
              value={name}
              maxLength={80}
              onChange={(event) => setName(event.target.value)}
              placeholder="Nightly tidy"
            />
          </div>

          {teammate !== undefined && (
            <p className="lc-dialog__note lc-mono">
              <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={BOT_SIZE.pickerMark} /> {teammate.name} runs it
              {routeLabel === undefined ? '' : ` on ${routeLabel}`}
              {shownMode === undefined ? '' : `, in ${shownMode}`}.
              {/*
                * What a later run uses (0.493), said instead of "A routine saved
                * read-only stays read-only" -- which sat on every routine, the
                * ones that write included (Grok's 0.489 pass).
                */}
              {/*
                * "The model at the time" was read by Sol (0.535) as whatever the
                * message box showed; a model picked there counts only once a
                * message is sent on it. Said as what it is.
                */}
              {` Each run uses the model ${teammate.name} last worked on${teammate.route === undefined ? ' (this one, until they have worked on one)' : ''}${shownMode === undefined ? '' : `, and always ${shownMode}`}.`}
            </p>
          )}
          {modeName !== undefined && (
            <div className="lc-dialog__section">
              <span className="lc-fieldlabel lc-mono" id="routine-may-label">
                What a run may do
              </span>
              {/*
                ONE CHOICE, SAID AT SAVE (0.530). A routine ran in whatever mode its
                conversation happened to be in, and Sol's 0.528 pass watched a file
                routine write into the folder while its dialog promised it changed
                nothing. Now the person picks, and every sentence below follows it.
              */}
              <div className="lc-routinesched">
              <div className="lc-segmented" role="radiogroup" aria-labelledby="routine-may-label">
                {(
                  [
                    [true, 'Only read'],
                    [false, 'Change files']
                  ] as const
                ).map(([only, label]) => (
                  <button
                    key={label}
                    type="button"
                    role="radio"
                    aria-checked={readsOnly === only}
                    className={`lc-button${readsOnly === only ? ' is-active' : ''}`}
                    onClick={() => setReadsOnly(only)}
                  >
                    {label}
                  </button>
                ))}
              </div>
              </div>
              <p className="lc-dialog__note lc-mono">
                {readsOnly
                  ? 'It reads and answers; nothing in the folder changes.'
                  : copies
                    ? 'It changes files in a copy of the folder. When a run finishes, its changes wait under Routines: Keep writes them into the folder, Discard throws them away.'
                    : 'What a run changes lands in the folder straight away, with nothing to keep or undo first.'}
              </p>
              {/*
                WHERE IT WORKS (0.533). Sol's 0.528 pass: "I would not trust a file
                routine's no-change description after seeing it create files." A run
                that changes files can work in a copy instead, and nothing reaches the
                folder until the person keeps it.
              */}
              {!readsOnly && (
                <>
                {/*
                  WHEN ITS STEPS ARE DONE (0.534). A standing goal: the folder's own
                  check (Settings > Project folder) runs, and while it fails the
                  teammate is asked to fix what it says -- a few times at most, so a
                  check that can never pass stops instead of spending on.
                */}
                <span className="lc-fieldlabel lc-mono" id="routine-goal-label">
                  When its steps are done
                </span>
                <div className="lc-routinesched">
                  <div className="lc-segmented" role="radiogroup" aria-labelledby="routine-goal-label">
                    <button type="button" role="radio" aria-checked={goalTries === undefined} className={`lc-button${goalTries === undefined ? ' is-active' : ''}`} onClick={() => setGoalTries(undefined)}>
                      Stop
                    </button>
                    <button
                      type="button"
                      role="radio"
                      aria-checked={goalTries !== undefined}
                      disabled={!hasCheck && goalTries === undefined}
                      className={`lc-button${goalTries !== undefined ? ' is-active' : ''}`}
                      onClick={() => setGoalTries((held) => held ?? 3)}
                    >
                      Keep going until the check passes
                    </button>
                  </div>
                  {goalTries !== undefined && (
                    <label className="lc-routinesched__detail lc-mono">
                      at most
                      <Select className="lc-input lc-routinesched__pick" aria-label="Fixes at most" value={goalTries} onChange={(next) => setGoalTries(Number(next))} options={[1, 2, 3, 4, 5].map((count) => ({ value: String(count), label: String(count) }))} />
                      {goalTries === 1 ? 'fix' : 'fixes'}
                    </label>
                  )}
                </div>
                <p className="lc-dialog__note lc-mono">
                  {!hasCheck
                    ? 'This folder has no check command yet. Set one in Settings > Project folder, such as npm test, to keep going until it passes.'
                    : goalTries === undefined
                      ? `It stops when its steps are done. Or it can run \`${checkCommand ?? ''}\` then, and keep fixing until it passes.`
                      : `When its steps are done it runs \`${checkCommand ?? ''}\`. While that fails, it is asked to fix what it says, up to ${String(goalTries)} time${goalTries === 1 ? '' : 's'}; then it stops and says what still fails. Each fix is one more turn.`}
                </p>
                <div className="lc-routinesched">
                  <div className="lc-segmented" role="radiogroup" aria-label="Where it works">
                    {(
                      [
                        [false, 'In the folder'],
                        [true, 'In a copy, you keep']
                      ] as const
                    ).map(([copy, label]) => (
                      <button
                        key={label}
                        type="button"
                        role="radio"
                        aria-checked={inCopy === copy}
                        className={`lc-button${inCopy === copy ? ' is-active' : ''}`}
                        onClick={() => setInCopy(copy)}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                </div>
                </>
              )}
            </div>
          )}
          {running && (
            <p className="lc-dialog__note">
              It is running now. That run keeps the steps it started with; your changes apply from the next run.
            </p>
          )}

          {/*
            * ASKED, NOT GUESSED. A conversation with nobody picked has no
            * owner to inherit, and a routine runs on somebody's route -- so
            * the one missing fact is asked for here rather than defaulted to
            * whoever happens to be first in the roster.
            */}
          {mustPick && (
            <div className="lc-dialog__fields">
              <label className="lc-fieldlabel lc-mono" htmlFor="routine-runner">
                Who runs it
              </label>
              <Select
                id="routine-runner"
                className="lc-input"
                value={runner}
                onChange={setRunner}
                options={[
                  { value: '', label: 'Pick a teammate' },
                  ...(chooseFrom ?? []).map((entry) => ({ value: entry.teammateId, label: entry.name }))
                ]}
              />
              <p className="lc-dialog__note lc-mono">
                This conversation was not assigned to anyone. It replays on the teammate you pick, on their route.
              </p>
            </div>
          )}

          {/*
            * And the case where there is nobody to pick. Said rather than
            * silently unsavable: the app cannot invent a teammate, and a
            * disabled button with no sentence is the failure this whole file
            * keeps being about.
            */}
          {nobodyToRun && (
            <p className="lc-dialog__note lc-mono">
              A routine runs on a teammate&apos;s route, and there are no teammates yet. Add one, then save this again.
            </p>
          )}

          <div className="lc-dialog__section">
            <span className="lc-fieldlabel lc-mono">
              Steps <span className="lc-queued__note">{String(steps.length)} of {String(MAX_ROUTINE_STEPS)}</span>
            </span>
            {steps.map((step, index) => {
              /*
               * A step longer than the message can carry, said WHILE it is
               * typed.
               *
               * The store accepts 20,000 characters and the runtime prompt cap
               * is 12,000 for the step, the workspace briefing, anything
               * waiting from teammates and the roster trailer together. So a
               * step could be saved here and then never send -- and worse, an
               * over-long one used to make the host shed every waiting peer
               * message trying to fit, fail anyway, and send it over the cap
               * regardless (R4, docs/ROUTINES-RECHECK-2026-09-08.md). That
               * half is fixed in the host; this is the half that stops it
               * being saved in the first place.
               *
               * A block now, on SAVE (A5.1): Save stays off until every step
               * fits, and says which one does not. The cap on READ is not
               * lowered -- `parsedRoutine` validates length there, and a
               * smaller one would make routines people already saved
               * unreadable -- so this costs nobody a saved routine.
               */
              const tooLong = stepTooLongNotice(step)
              return (
                <div className="lc-routinestep" key={`step_${String(index)}`}>
                  <span className="lc-routinestep__n lc-mono">{index + 1}</span>
                  <textarea
                    className="lc-input lc-routinestep__text"
                    aria-label={`Step ${String(index + 1)}`}
                    value={step}
                    rows={2}
                    aria-invalid={tooLong !== undefined}
                    {...(tooLong === undefined ? {} : { 'aria-describedby': `step-too-long-${String(index)}` })}
                    onChange={(event) =>
                      setSteps(steps.map((held, at) => (at === index ? event.target.value : held)))
                    }
                  />
                  <button
                    type="button"
                    className="lc-ghostbutton lc-routinestep__drop"
                    aria-label={`Remove step ${String(index + 1)}`}
                    onClick={() => {
                      setSteps(steps.filter((_, at) => at !== index))
                      setHandOffs(handOffs.filter((_, at) => at !== index))
                      setStepRoles(stepRoles.filter((_, at) => at !== index))
                    }}
                  >
                    Remove
                  </button>
                  {/*
                    * WHO TAKES IT (0.435). A step can go to another teammate,
                    * who is given the answer of the step before; a checker's
                    * approval is what makes a run count as done.
                    *
                    * WAIT FOR MY OK FIRST (0.840), from step 2 on, with one
                    * teammate too: the run holds before the step, and Go ahead
                    * on its routine card starts it.
                    */}
                  {(canHandOff || index > 0) && (
                    <div className="lc-routinestep__who">
                      {canHandOff && <>
                      <Select
                        className="lc-input lc-routinestep__pick"
                        aria-label={`Who takes step ${String(index + 1)}`}
                        value={handOffs[index]?.teammateId ?? ''}
                        onChange={(next) => {
                          const { teammateId: _was, ...rest } = handOffs[index] ?? {}
                          setHandOff(index, next === '' ? rest : { ...rest, teammateId: next })
                        }}
                        options={[
                          { value: '', label: ownerName === undefined ? 'The teammate who runs it' : `${ownerName} (runs it)` },
                          ...team
                            .filter((entry) => entry.teammateId !== ownerId)
                            .map((entry) => ({ value: entry.teammateId, label: entry.name }))
                        ]}
                      />
                      <label className="lc-routinestep__check lc-mono">
                        <input
                          type="checkbox"
                          checked={handOffs[index]?.check === true}
                          onChange={(event) => {
                            const { check: _was, ...rest } = handOffs[index] ?? {}
                            setHandOff(index, event.target.checked ? { ...rest, check: true } : rest)
                          }}
                        />
                        Checker: must approve
                      </label>
                      </>}
                      {index > 0 && (
                        <label className="lc-routinestep__check lc-mono">
                          <input
                            type="checkbox"
                            checked={handOffs[index]?.askFirst === true}
                            onChange={(event) => {
                              const { askFirst: _was, ...rest } = handOffs[index] ?? {}
                              setHandOff(index, event.target.checked ? { ...rest, askFirst: true } : rest)
                            }}
                          />
                          Wait for my OK first
                        </label>
                      )}
                      {canHandOff && stepRoles[index] !== undefined && <span className="lc-routinestep__role lc-mono">Role: {stepRoles[index]}</span>}
                    </div>
                  )}
                  {tooLong !== undefined && (
                    <p className="lc-routinestep__over lc-tone-amber" id={`step-too-long-${String(index)}`}>
                      {tooLong}
                    </p>
                  )}
                </div>
              )
            })}
            {/*
              * Kept and disabled at the cap rather than unmounted. A control
              * that vanishes reads as a broken dialog; one that stays and
              * says why teaches the limit (design pass, gap 4).
              */}
            <button
              type="button"
              className="lc-ghostbutton"
              disabled={steps.length >= MAX_ROUTINE_STEPS}
              title={steps.length >= MAX_ROUTINE_STEPS ? `A routine holds ${String(MAX_ROUTINE_STEPS)} steps at most` : undefined}
              onClick={() => {
                setSteps([...steps, ''])
                setHandOffs([...steps.map((_, at) => handOffs[at] ?? {}), {}])
                setStepRoles([...steps.map((_, at) => stepRoles[at]), undefined])
              }}
            >
              Add a step
            </button>
          </div>

          <div className="lc-dialog__section">
            <span className="lc-fieldlabel lc-mono" id="routine-schedule-label">
              Runs on its own
            </span>
            <div className="lc-routinesched">
              <div className="lc-segmented" role="radiogroup" aria-labelledby="routine-schedule-label">
                {(
                  [
                    // Shorter since 0.522, so seven fit on one line in the dialog.
                    ['off', 'When I press Run'],
                    ['every', 'Every few hours'],
                    ['daily', 'Daily'],
                    ['weekly', 'On set days'],
                    ['once', 'Once'],
                    ['files', 'On a new file'],
                    ['hook', 'When a link is called']
                  ] as const
                ).map(([kind, label]) => (
                  <button
                    key={kind}
                    type="button"
                    role="radio"
                    aria-checked={scheduleKind === kind}
                    className={`lc-button${scheduleKind === kind ? ' is-active' : ''}`}
                    onClick={() => {
                      // Pressing the choice already made keeps what was set
                      // under it: a person who taps "Daily" twice has not
                      // asked for 09:00 back.
                      if (kind === scheduleKind) return
                      setSchedule(
                        kind === 'off'
                          ? undefined
                          : kind === 'every'
                            ? { kind: 'every', hours: 4 }
                            : kind === 'weekly'
                              ? { kind: 'weekly', days: WEEKDAYS_ONLY, at: '09:00' }
                              : kind === 'once'
                                ? { kind: 'once', on: tomorrowAtNine() }
                                : kind === 'files'
                                  ? { kind: 'files', folder: 'inbox' }
                                  : kind === 'hook' ? { kind: 'hook' } : { kind: 'daily', at: '09:00' }
                      )
                    }}
                  >
                    {label}
                  </button>
                ))}
              </div>
              {schedule?.kind === 'every' && (
                <label className="lc-routinesched__detail lc-mono">
                  every
                  <Select
                    className="lc-input lc-routinesched__pick"
                    aria-label="Hours between runs"
                    value={String(schedule.hours)}
                    onChange={(next) => setSchedule({ kind: 'every', hours: Number(next) })}
                    options={EVERY_HOURS_CHOICES.map((hours) => ({ value: String(hours), label: hours === 1 ? '1 hour' : `${String(hours)} hours` }))}
                  />
                </label>
              )}
              {schedule?.kind === 'daily' && (
                <label className="lc-routinesched__detail lc-mono">
                  at
                  <input
                    type="time"
                    className="lc-input lc-routinesched__pick"
                    aria-label="Time of day"
                    value={schedule.at}
                    onChange={(event) => {
                      if (/^\d\d:\d\d$/u.test(event.target.value)) setSchedule({ kind: 'daily', at: event.target.value })
                    }}
                  />
                </label>
              )}
            </div>
            {schedule?.kind === 'weekly' && (
              <div className="lc-routinesched lc-routinesched__days">
                <div className="lc-segmented" role="group" aria-label="Days it runs">
                  {WEEK_ORDER.map((day) => {
                    const on = schedule.days.includes(day)
                    return (
                      <button
                        key={day}
                        type="button"
                        aria-pressed={on}
                        className={`lc-button${on ? ' is-active' : ''}`}
                        onClick={() => {
                          const days = on ? schedule.days.filter((entry) => entry !== day) : [...schedule.days, day].sort((a, b) => a - b)
                          // At least one day: the last one stays on rather than leaving a schedule that never runs.
                          if (days.length > 0) setSchedule({ ...schedule, days })
                        }}
                      >
                        {DAY_NAMES[day]}
                      </button>
                    )
                  })}
                </div>
                <label className="lc-routinesched__detail lc-mono">
                  at
                  <input
                    type="time"
                    className="lc-input lc-routinesched__pick"
                    aria-label="Time of day"
                    value={schedule.at}
                    onChange={(event) => {
                      if (/^\d\d:\d\d$/u.test(event.target.value)) setSchedule({ ...schedule, at: event.target.value })
                    }}
                  />
                </label>
              </div>
            )}
            {schedule?.kind === 'files' && (
              <div className="lc-routinesched lc-routinesched__days">
                <label className="lc-routinesched__detail lc-mono">
                  in the folder
                  <input
                    className="lc-input lc-routinesched__pick"
                    aria-label="The folder it watches"
                    value={filesFolder}
                    onChange={(event) => {
                      setFilesFolder(event.target.value)
                      if (watchedFolderValid(event.target.value.trim())) setSchedule({ kind: 'files', folder: event.target.value.trim() })
                    }}
                  />
                </label>
                {!watchedFolderValid(filesFolder.trim()) && (
                  <span className="lc-routinesched__detail lc-mono">A folder inside the project, by name, such as inbox.</span>
                )}
                <p className="lc-dialog__note lc-mono">
                  A file that lands there starts a run once it stops changing, up to six an hour, and the run is told its name.
                  What is there already starts nothing.{' '}
                  {/*
                    SAID FROM THE MODE IT HAS (0.530). This read "It reads the file and changes
                    nothing, as every routine runs in Ask" under a line saying "always Edit", and
                    Sol's 0.528 pass watched an Edit run write receipts into the folder at once.
                    A routine runs in the mode saved with it; so does this sentence.
                  */}
                  {readsOnly
                    ? 'It reads the file and changes nothing.'
                    : copies
                      ? 'It may change files in its copy; nothing lands in the folder until you keep it.'
                      : 'It may change files, and what it changes lands in the folder straight away.'}
                </p>
              </div>
            )}
            {schedule?.kind === 'hook' && <RoutineHookLink {...(routineId === undefined ? {} : { routineId })} />}
            {schedule?.kind === 'once' && (
              <div className="lc-routinesched lc-routinesched__days">
                <label className="lc-routinesched__detail lc-mono">
                  on
                  <input
                    type="datetime-local"
                    className="lc-input lc-routinesched__pick"
                    aria-label="Date and time"
                    value={schedule.on}
                    onChange={(event) => {
                      if (onceMoment(event.target.value) !== undefined) setSchedule({ kind: 'once', on: event.target.value })
                    }}
                  />
                </label>
                {(onceMoment(schedule.on)?.getTime() ?? 0) <= Date.now() && (
                  <span className="lc-routinesched__detail lc-mono">That time has passed, so it will not run on its own.</span>
                )}
              </div>
            )}
            {schedule !== undefined && schedule.kind !== 'hook' && (
              <p className="lc-dialog__note lc-mono">
                Only while Locust is open, and only when {teammate?.name ?? 'the teammate'} is free. A run missed while
                Locust was closed happens once, the next time it is open.
                {folder !== undefined && <span title={folder.path}>{` On a schedule it runs in ${folder.name}.`}</span>}
              </p>
            )}
          </div>

          <RoutineInputEditor inputs={inputs} onChange={setInputs} />
          {inputError !== undefined && <p className="lc-dialog__error">{inputError}</p>}
          {truncated && (
            <p className="lc-dialog__note lc-mono">
              This conversation had more turns than a routine can hold, so the first {String(MAX_ROUTINE_STEPS)} are here.
            </p>
          )}
          <p className="lc-dialog__note lc-mono">
            Each step runs only after the one before it finishes. A step that fails ends the routine there.
            {steps.length > 1 ? ' A step set to wait for your OK starts when you go ahead on the routine’s card, and your phone and chat apps are told.' : ''}
            {canHandOff
              ? ' A step handed to another teammate is given the answer of the step before it; a checker must approve for the run to count as done.'
              : ''}
          </p>
          {error !== undefined && <p className="lc-dialog__error">{error}</p>}
          {tooLongAt >= 0 && <p className="lc-dialog__error">Shorten step {tooLongAt + 1} to save this routine.</p>}
        </div>

        <div className="lc-dialog__foot">
          <button type="button" className="lc-ghostbutton" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="lc-primarybutton"
            disabled={!canSave}
            onClick={() =>
              onSave({
                name: name.trim(),
                steps: kept.map((step) => step.trim()),
                ...(editing ? inputsChanged ? { inputs } : {} : { inputs }),
                schedule,
                // Only when it moved: an unchanged choice keeps the mode exactly as saved.
                ...(modeName === undefined || readsOnly === (modeReadsOnly === true) ? {} : { readsOnly }),
                // A copy only for a routine that may change files; sent when new, or when it moved.
                ...(editing ? (copies === initialInCopy ? {} : { inCopy: copies }) : copies ? { inCopy: true } : {}),
                // A goal: sent when new, or when it moved; null takes it away.
                ...(editing
                  ? goal === initialGoalTries ? {} : { untilCheck: goal === undefined ? null : { tries: goal } }
                  : goal === undefined ? {} : { untilCheck: { tries: goal } }),
                ...(runner.length === 0 ? {} : { teammateId: runner }),
                // Lined up with the steps that are kept (0.435). Sent for one teammate too while a step waits for an OK,
                // or once did, so turning the last one off is saved (0.840).
                ...(canHandOff || asksFirst
                  ? { handOffs: steps.flatMap((step, at) => (step.trim().length > 0 ? [handOffs[at] ?? {}] : [])) }
                  : {})
              })
            }
          >
            {editing ? 'Save changes' : 'Save routine'}
          </button>
        </div>
      </div>
    </div>
  )
}
