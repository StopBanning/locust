import { useContext, useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { MissionApprovalDecision, MissionApprovalRequest, MissionQuestion } from '../../../shared/ipc.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { COMMAND_EFFECTS_UNKNOWN, CONNECTOR_NOT_UNDOABLE, commandEffectsUnknown, dataSentLine } from '../../../shared/approval-data.js'
import { commandReach } from '../../../shared/command-reach.js'
import { CommandText } from './CommandText.js'
import { Icon } from './Icon.js'
import { RuntimeMark } from './RuntimeMark.js'
import { DiffView } from './DiffView.js'
import { DiffNotesContext } from './DiffNotes.js'
import { fileCounts, parseUnifiedDiff } from '../diff.js'
import { ApprovalRuleContext } from '../approvalRuleContext.js'
import { numberKeyAnswer } from '../questionKeys.js'

/** How long a card that has just appeared takes no approval (N9): a double click's second half. */
export const FRESH_CARD_MS = 500


/**
 * A question, answered rather than authorized.
 *
 * The protocol files answers under each question's OWN id, and identifies a
 * chosen option by its LITERAL LABEL -- options carry no id and no index, and
 * the server matches the strings it gets back against the labels it sent. So
 * the label is what travels, and sending "3" for the third option answers with
 * the string "3".
 *
 * There is deliberately no "Always allow this session" here. The server has no
 * session-grant field for a question; offering one promised something the
 * protocol has no way to mean.
 */
function QuestionForm({
  questions,
  onAnswer,
  skippable = false,
  busy
}: {
  readonly questions: readonly MissionQuestion[]
  readonly onAnswer: (answers: Readonly<Record<string, readonly string[]>>) => void
  /** The runtime takes "no answer" as an answer -- Antigravity's own card has Skip. */
  readonly skippable?: boolean
  readonly busy: boolean
}): ReactElement {
  const [chosen, setChosen] = useState<Readonly<Record<string, string>>>({})
  const [typed, setTyped] = useState<Readonly<Record<string, string>>>({})
  const box = useRef<HTMLDivElement>(null)
  const send = useRef<HTMLButtonElement>(null)
  /*
   * NUMBER KEYS (0.801), as Claude Code's own question does: 1 to 9 choose that option of the question being
   * answered, and once every question has one the focus is on Send, so Enter sends. Never from a text field --
   * "2" typed into the chat box or the Something else field is a 2 -- and only the first card on the screen takes
   * them. A card that appears takes the focus only from an empty chat box or from nothing: a message being written
   * is not interrupted.
   */
  useEffect(() => {
    const element = box.current
    if (element === null || busy) return undefined
    const active = document.activeElement
    const idle = active === null || active === document.body || (active instanceof HTMLTextAreaElement && active.value.trim().length === 0 && active.closest('form.command-dock') !== null)
    if (idle && document.querySelector('.lc-questions') === element) element.querySelector<HTMLButtonElement>('.lc-question__option')?.focus({ preventScroll: true })
    return undefined
  }, [busy])
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const element = box.current
      if (element === null || busy || document.querySelector('.lc-questions') !== element) return
      const target = event.target instanceof HTMLElement ? event.target : null
      const picked = numberKeyAnswer({
        key: event.key,
        modified: event.ctrlKey || event.metaKey || event.altKey,
        typing: target !== null && (target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement),
        questions: questions.map((question) => ({ id: question.id, options: question.options.map((option) => option.label) })),
        chosen,
        focusedQuestion: target === null || !element.contains(target) ? undefined : target.closest<HTMLElement>('[data-question]')?.dataset.question
      })
      if (picked === undefined) return
      event.preventDefault()
      const next = { ...chosen, [picked.questionId]: picked.label }
      setChosen(next)
      // The next question still unanswered, else Send: Enter then does what the card is for.
      const open = questions.find((question) => question.options.length > 0 && next[question.id] === undefined)
      requestAnimationFrame(() => {
        if (open === undefined) send.current?.focus({ preventScroll: true })
        else element.querySelector<HTMLButtonElement>(`[data-question="${CSS.escape(open.id)}"] .lc-question__option`)?.focus({ preventScroll: true })
      })
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [busy, chosen, questions])

  const answersFor = (question: MissionQuestion): readonly string[] => {
    const values: string[] = []
    const pick = chosen[question.id]
    if (pick !== undefined) values.push(pick)
    const free = (typed[question.id] ?? '').trim()
    if (free.length > 0) values.push(free)
    return values
  }
  // Every question, or none: a partial answer is a question silently dropped,
  // and the model is told nothing rather than told less.
  const ready = questions.every((question) => answersFor(question).length > 0)

  return (
    <div className="lc-questions" ref={box}>
      {questions.map((question) => (
        <div key={question.id} className="lc-question" data-question={question.id}>
          {question.header !== null && question.header.length > 0 && (
            <p className="lc-question__header lc-mono">{question.header}</p>
          )}
          <p className="lc-question__text">{question.question}</p>
          {question.options.length > 0 && (
            <div className="lc-question__options" role="radiogroup" aria-label={question.question}>
              {question.options.map((option, index) => (
                <button
                  key={option.label}
                  type="button"
                  role="radio"
                  aria-checked={chosen[question.id] === option.label}
                  {...(index < 9 ? { 'aria-keyshortcuts': String(index + 1) } : {})}
                  className={`lc-question__option${chosen[question.id] === option.label ? ' is-chosen' : ''}`}
                  disabled={busy}
                  onClick={() => setChosen((current) => ({ ...current, [question.id]: option.label }))}
                >
                  {index < 9 && <span className="lc-question__key lc-mono" aria-hidden="true">{String(index + 1)}</span>}
                  <span className="lc-question__body">
                    <span className="lc-question__label">{option.label}</span>
                    {option.description !== null && option.description.length > 0 && (
                      <span className="lc-question__desc">{option.description}</span>
                    )}
                  </span>
                </button>
              ))}
            </div>
          )}
          {/* Free text where the question takes it: no options at all, or
              `isOther`, which is the protocol saying an unlisted answer is
              allowed. A secret answer is never echoed to the screen. */}
          {(question.options.length === 0 || question.isOther) && (
            <input
              className="lc-question__free"
              type={question.isSecret ? 'password' : 'text'}
              value={typed[question.id] ?? ''}
              disabled={busy}
              aria-label={question.options.length === 0 ? question.question : 'Something else'}
              placeholder={question.options.length === 0 ? 'Your answer' : 'Something else…'}
              onChange={(event) => setTyped((current) => ({ ...current, [question.id]: event.target.value }))}
            />
          )}
        </div>
      ))}
      <div className="lc-approval__actions">
        <button
          ref={send}
          type="button"
          className="lc-primarybutton"
          disabled={busy || !ready}
          onClick={() => {
            const answers: Record<string, readonly string[]> = {}
            for (const question of questions) answers[question.id] = answersFor(question)
            onAnswer(answers)
          }}
        >
          {questions.length > 1 ? 'Send answers' : 'Send answer'}
        </button>
        {skippable && (
          <button
            type="button"
            className="lc-ghostbutton"
            disabled={busy}
            // Every question left alone: the runtime is told it was skipped.
            onClick={() => onAnswer(Object.fromEntries(questions.map((question) => [question.id, []])))}
          >
            Skip
          </button>
        )}
      </div>
      <p className="lc-approval__note">
        {/* No "Always": a question has no session grant in the protocol, and a
            button that cannot mean what it says is worse than one fewer. */}
        {skippable
          ? 'This goes back to the runtime as your answer. Skip tells it you would rather not say.'
          : 'This goes back to the runtime as your answer. Stop the run below if you would rather not answer.'}
      </p>
    </div>
  )
}

/**
 * A question Locust can show but not answer: the runtime takes the answer in
 * its own window (Antigravity, when its waiting step could not be found).
 * No buttons -- a button here would claim an answer can go somewhere it
 * cannot.
 */
function QuestionElsewhere({ questions, where }: { readonly questions: readonly MissionQuestion[]; readonly where: string }): ReactElement {
  return (
    <div className="lc-questions">
      {questions.map((question) => (
        <div key={question.id} className="lc-question">
          <p className="lc-question__text">{question.question}</p>
          {question.options.length > 0 && (
            <ul className="lc-question__listed">
              {question.options.map((option) => (
                <li key={option.label}>{option.label}</li>
              ))}
            </ul>
          )}
        </div>
      ))}
      <p className="lc-approval__note">
        Answer it in {where}&rsquo;s own window. Locust carries on as soon as {where} has your answer.
      </p>
    </div>
  )
}

/**
 * The approval card.
 *
 * The design's rule for this surface is that it must answer four questions
 * before anyone can reasonably say yes: what would happen, where, whether it
 * can be undone, and who is asking. Everything shown here comes from the
 * runtime's own request -- when it did not say what it wants to do, the card
 * says THAT rather than filling the gap with a friendly summary.
 */
export function ApprovalCard({
  request,
  onDecide,
  onAnswer,
  busy,
  askedBy
}: {
  readonly request: MissionApprovalRequest
  /**
   * The helper that asked, when the teammate did not ask itself: "the
   * Explore helper of Wren" (helper visibility, `helperAskedBy`). Absent:
   * the teammate asked, and the card reads as it always has.
   */
  readonly askedBy?: string
  /** `reason`: why a denial was made, when the person said (0.374). */
  readonly onDecide: (decision: MissionApprovalDecision, reason?: string) => void
  /**
   * Answer a QUESTION, keyed by question id.
   *
   * Separate from `onDecide` because the two are different acts with different
   * payloads: an action is authorized, a question is answered. Sending a
   * decision for a question is what discarded the person's reply -- the server
   * cannot deserialize it, so it substituted an empty answer and told the model
   * they had said nothing.
   */
  readonly onAnswer: (answers: Readonly<Record<string, readonly string[]>>) => void
  readonly busy: boolean
}): ReactElement {
  const isQuestion = request.kind === 'question'
  // What this card can save as a rule, when one can hold it exactly (0.521).
  const rule = useContext(ApprovalRuleContext)(request)
  const [never, setNever] = useState(false)
  const [ruleSaid, setRuleSaid] = useState<string>()
  const [saving, setSaving] = useState(false)
  const saveRule = (effect: 'allow' | 'deny', why?: string): void => {
    if (rule === undefined || saving) return
    setSaving(true)
    setRuleSaid(undefined)
    void rule.save(effect, why).then((problem) => {
      setSaving(false)
      if (problem !== undefined) setRuleSaid(problem)
    })
  }
  const questions = request.questions ?? []
  /*
   * DENY, AND SAY WHY (0.374).
   *
   * A bare denial left the teammate to guess what was wrong, and the guess
   * was usually the same action by another route. Vibe Kanban's Deny opens
   * "Let the agent know why". Here Deny opens one line for the reason; Enter
   * with nothing in it still denies at once, so a plain no costs one key more
   * and an explained one no extra click. The reason reaches the teammate with
   * the denial on Claude and OpenCode, and as its next input on Codex.
   */
  /*
   * A NEW CARD IGNORES A CLICK MEANT FOR THE LAST ONE (QA-2026-09-29 round 2,
   * N9). A double click on "Approve once" also approved the NEXT card when
   * the run asked again within about 150 ms: the second click of the pair
   * landed on a card the person had not read. For its first half second a
   * card takes no approval.
   */
  const shownAt = useRef(Date.now())
  const approve = (decision: MissionApprovalDecision): void => {
    if (Date.now() - shownAt.current < FRESH_CARD_MS) return
    onDecide(decision)
  }
  const [denying, setDenying] = useState(false)
  const [reason, setReason] = useState('')
  const stopDenying = (): void => {
    setDenying(false)
    setReason('')
  }
  const submitDenial = (): void => {
    const said = reason.replace(/\s+/g, ' ').trim()
    if (never && rule !== undefined) {
      saveRule('deny', said.length === 0 ? undefined : said)
      return
    }
    onDecide('deny', said.length === 0 ? undefined : said)
  }
  // The change itself, when Codex sent it with the item (parity row 32).
  // Drawn with the same viewer the activity fold uses, so an approval and
  // its record read the same.
  const files = request.patch === undefined ? [] : parseUnifiedDiff(request.patch.text)
  // What would LEAVE this machine -- the question the card never answered, and
  // the only one of the four a person cannot work out for themselves. Claimed
  // as "nothing" for a file change, where that is provable, and never for a
  // command, where it is not. See shared/approval-data.ts.
  // The asking route's own words first, when it knows better than the kind (R15, R37).
  const dataSent = request.dataSentSays ?? dataSentLine(request.kind, request.detail)
  const reversible = request.reversibleSays ?? (
    request.kind === 'command'
      ? 'Unknown — a command can do anything the workspace sandbox allows.'
      : request.kind === 'file-change'
        ? 'Yes for tracked files, if the workspace is under version control.'
        : request.kind === 'connector'
          ? CONNECTOR_NOT_UNDOABLE
          : 'Nothing is changed by answering.')
  // A command that acts on programs this run did not start (0.578): the
  // arena's `taskkill //F //IM python.exe` stops every Python on the computer,
  // and the card said only "Run a command". See shared/command-reach.ts.
  const reach = request.kind === 'command' ? commandReach(request.detail) : undefined
  // Both rows would say Unknown: one row says it once (0.735, shared/approval-data.ts).
  const effectsUnknown = commandEffectsUnknown(request)

  // A card that waits on the person is brought into view when it appears.
  // Its buttons sat below the fold while the run said "waiting on you"
  // (seen driving the app, 2026-09-05).
  /*
   * This card no longer scrolls itself into view, and the thread does it.
   *
   * It used to call `scrollIntoView` on mount and again 300ms later, which
   * fixed a real defect -- its buttons sat under the composer while the run
   * said "waiting on you" (drive, 2026-09-05). But it moved the viewport
   * UNCONDITIONALLY, so a second approval in one run, or one arriving while
   * the person was scrolled up reading an earlier diff, took the screen away
   * mid-sentence. Flagged by the design agent, 2026-09-08.
   *
   * Guarding it with "only if they are at the bottom" does not work here
   * either: the card itself adds the height, so by the time this effect runs
   * the answer is always no. The thread's own follow (`stickToBottom.ts`)
   * remembers where the person was BEFORE the content grew, which is the only
   * place that question can be answered honestly -- and it scrolls to the very
   * bottom, so the buttons are in view for exactly the case the original fix
   * was for. One mechanism, in the one place that has the facts.
   */
  return (
    <div className="lc-card is-pending is-amber" role="group" aria-label="Approval required">
      <div className="lc-card__head">
        <span className="lc-approval__title">
          <Icon name="shield" size={13} />{' '}
          {isQuestion ? 'The runtime is asking you something' : 'Approval required — exact action'}
        </span>
        {/*
          * The header used to read "Codex CLI · this workspace" as a constant,
          * which contradicted the card's own Where row four lines down: on a
          * worktree teammate that row shows the worktree path while this said
          * "this workspace". A card whose two halves disagree about where an
          * action would happen is worse than one that says less, so when the
          * request names a folder the header defers to the Where row rather
          * than restating it wrongly.
          */}
        {/* Named by the request, not assumed: a Claude Code connector
            permission wore "Codex CLI" in its first drive (2026-09-10). */}
        <span className="lc-rail__meta">
          {/* Which program is asking, at a glance (0.383). */}
          <RuntimeMark runtime={request.runtime ?? 'codex'} size={12} className="is-inline" />
          {runtimeDisplayName(request.runtime ?? 'codex')}
          {request.cwd === null ? ' · this workspace' : ''}
        </span>
      </div>

      <dl className="lc-receipt">
        {askedBy !== undefined && (
          <>
            <dt>Asked by</dt>
            <dd>{askedBy}</dd>
          </>
        )}
        <dt>Action</dt>
        <dd>{request.summary}</dd>
        {request.detail.length > 0 && (
          <>
            <dt>{isQuestion ? 'Question' : 'Exact'}</dt>
            <dd className="lc-mono lc-approval__detail">{request.kind === 'command' ? <CommandText text={request.detail} /> : request.detail}</dd>
          </>
        )}
        {reach !== undefined && (
          <>
            <dt>Reaches</dt>
            <dd className="lc-approval__reach" data-reach={reach.kind}>{reach.said}</dd>
          </>
        )}
        <dt>Where</dt>
        <dd className="lc-mono">{request.cwd ?? 'the project folder'}</dd>
        {effectsUnknown && (
          <>
            <dt>Effects</dt>
            <dd>{COMMAND_EFFECTS_UNKNOWN}</dd>
          </>
        )}
        {!effectsUnknown && dataSent !== undefined && (
          <>
            <dt>Data sent</dt>
            <dd>{dataSent}</dd>
          </>
        )}
        {!isQuestion && !effectsUnknown && (
          <>
            <dt>Reversible</dt>
            <dd>{reversible}</dd>
          </>
        )}
      </dl>

      {request.patch !== undefined && (
        // A change waiting for approval is approved or denied, not annotated:
        // no "+" on its lines (DiffNotes.tsx).
        <DiffNotesContext.Provider value={undefined}>
        <div className="lc-approval__patch">
          <div className="lc-approval__patchhead lc-mono">
            {/* Whose change, by the runtime asking: an OpenCode edit read
                "as Codex would apply it" (beta pass on 0.390, 2026-09-27). */}
            <span>{`The change, as ${runtimeDisplayName(request.runtime ?? 'codex')} would apply it`}</span>
            <span className="lc-activity__counts">
              <span className="lc-diff__addmark">+{request.patch.added}</span>
              <span className="lc-diff__delmark">−{request.patch.removed}</span>
            </span>
          </div>
          {files.length === 0 ? (
            <p className="lc-settings__note">Codex sent a change this build could not read as a diff.</p>
          ) : (
            files.map((file) => (
              <div key={file.path} className="lc-approval__file">
                {/* The viewer draws hunks; the file they belong to is said here. */}
                <div className="lc-filerow is-static">
                  <Icon name="file" size={14} />
                  <span className="lc-filerow__path">{file.path}</span>
                  <span className="lc-filerow__status">{file.status}</span>
                </div>
                <DiffView file={file} truncated={request.patch!.truncated} reported={request.patch!.truncated ? { added: request.patch!.added, removed: request.patch!.removed } : fileCounts(file)} />
              </div>
            ))
          )}
        </div>
        </DiffNotesContext.Provider>
      )}

      {/*
        * A question is ANSWERED; an action is authorized. They were the same
        * three buttons, and for a question all three were malformed -- the
        * server could not deserialize a decision as an answer, so it logged the
        * failure, substituted an empty answer map, and told the model the
        * person had said nothing. Whatever they picked went nowhere.
        *
        * `questions` is present only when the request carried them, so a
        * question this build cannot read still falls through to the
        * authorization controls rather than drawing a form with no fields.
        */}
      {isQuestion && questions.length > 0 && request.answerIn !== undefined ? (
        <QuestionElsewhere questions={questions} where={request.answerIn} />
      ) : isQuestion && questions.length > 0 ? (
        <QuestionForm questions={questions} onAnswer={onAnswer} skippable={request.skippable === true} busy={busy} />
      ) : (
        <>
          {denying ? (
            <form
              className="lc-approval__actions lc-approval__deny"
              onSubmit={(event) => {
                event.preventDefault()
                submitDenial()
              }}
            >
              <input
                className="lc-input lc-approval__reason"
                value={reason}
                onChange={(event) => setReason(event.target.value)}
                onKeyDown={(event) => {
                  // Enter is said, not left to the form's own submission: a key
                  // with no character behind it (drive-deny-with-reason) did not
                  // submit, and an input method's Enter must confirm a word.
                  if (event.nativeEvent.isComposing || event.keyCode === 229) return
                  if (event.key === 'Enter') {
                    event.preventDefault()
                    if (!busy) submitDenial()
                  }
                  if (event.key === 'Escape') stopDenying()
                }}
                placeholder="Why? Optional. The teammate reads it and can change course."
                aria-label="Why you are denying it"
                maxLength={500}
                autoFocus
              />
              <button type="submit" className="lc-denybutton" disabled={busy || saving}>
                {reason.trim().length === 0 ? 'Deny' : 'Deny and say why'}
              </button>
              {rule !== undefined && (
                <label className="lc-approval__never" title={rule.denySentence}>
                  <input type="checkbox" checked={never} onChange={(event) => setNever(event.target.checked)} />
                  Never allow this (saves a rule)
                </label>
              )}
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={stopDenying}>
                Back
              </button>
            </form>
          ) : (
            <div className="lc-approval__actions">
              <button
                type="button"
                className="lc-primarybutton"
                disabled={busy}
                onClick={() => approve('approve-once')}
              >
                {isQuestion ? 'Allow once' : 'Approve once'}
              </button>
              {/*
                * ASKED EACH TIME (0.579). A command that reaches programs this
                * run did not start is not offered for the rest of the run or
                * as a saved rule. "Always" is the runtime's own pattern --
                * OpenCode answered `taskkill /IM locust-no-such-program.exe`
                * with "taskkill *", every taskkill for the run -- and Locust
                * cannot narrow it, so it is not offered.
                */}
              {reach === undefined && (
                <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => approve('approve-always')}>
                  Always allow this session
                </button>
              )}
              {rule !== undefined && reach === undefined && (
                <button type="button" className="lc-ghostbutton" disabled={busy || saving} title={rule.allowSentence} onClick={() => saveRule('allow')}>
                  Yes, and don&rsquo;t ask again
                </button>
              )}
              <button type="button" className="lc-denybutton" disabled={busy} onClick={() => setDenying(true)}>
                Deny…
              </button>
            </div>
          )}
          <p className="lc-approval__note">
            {/*
              "Always" is scoped to this session on purpose, and says so. A grant
              that outlives the run is a Settings decision, not one to take here.
            */}
            {/* What Always lets through, when the route can say (R35). */}
            {/* Asked although Always was given earlier (0.616): the host says why, in place of the general line. */}
            {request.askedAgain !== undefined
              ? `Nothing has happened yet. ${request.askedAgain}`
              : reach !== undefined
              ? 'Nothing has happened yet. A command that reaches beyond this run is asked about every time.'
              : request.alwaysCovers === undefined
                ? 'Nothing has happened yet. “Always” lasts until this run ends.'
                : `Nothing has happened yet. “Always” allows ${request.alwaysCovers}, until this run ends.`}
            {/* Where Locust keeps the Always (0.616), a rule saying no still wins over it; Codex keeps its own. */}
            {request.askedAgain === undefined && reach === undefined && request.alwaysKey !== undefined && ' Your saved rules still come first.'}
            {/* A saved rule outlives the run, so what it would save is said before it is pressed (0.521). */}
            {rule !== undefined && reach === undefined && ` “Don’t ask again” saves a rule: ${rule.allowSentence} Settings > Teammates lists your rules.`}
          </p>
          {ruleSaid !== undefined && <p className="lc-approval__note" role="status">{ruleSaid}</p>}
        </>
      )}
    </div>
  )
}
