import { Fragment, useContext, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import { callLength, type CallPiece, type SavedCallLine } from '../../../shared/calls.js'
import { CALL_TODO_WORDS, CallTodosContext, todoCount, type TodoShown } from '../callTodos.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import type { VoiceCallRole } from '../../../shared/voice-call.js'
import { CONNECTOR_NOT_UNDOABLE, dataSentLine } from '../../../shared/approval-data.js'
import { answerLiveCallAsk, type LiveCallAsk } from '../liveCall.js'
import { FRESH_CARD_MS } from './ApprovalCard.js'
import { Icon } from './Icon.js'
import { TeammateBot } from './TeammateBot.js'

/*
 * WHAT WAS SAID ON A CALL, IN THE THREAD (0.747). The canvas Colin passed on 2026-10-11: a call with a teammate
 * lives in that teammate's conversation -- your words as your bubbles, theirs beside their face, each marked as said
 * rather than typed -- among the turns the call started, which are already in the record. Live, it grows as you
 * talk; ended, it folds to one line that opens again.
 */

/** A piece of a call as the thread draws it: the call, live or kept, and where its words go. */
export interface CallPieceShown extends CallPiece {
  /** On now: drawn open, growing, with words still being said. */
  readonly live: boolean
  readonly partial?: Readonly<Record<VoiceCallRole, string>>
  /** A connector's questions waiting on the person, on the call's last piece (0.749). */
  readonly asks?: readonly LiveCallAsk[]
}

const at = (time: number): string => new Date(time).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })

function SaidMark(): ReactElement {
  return <span className="lc-callsaid__mark"><Icon name="mic" size={11} />Said</span>
}

function Line({ line, teammate, partial }: { readonly line: SavedCallLine; readonly teammate: PublicTeammate | undefined; readonly partial?: boolean }): ReactElement {
  if (line.role === 'you') {
    return <div className={`lc-callsaid is-you${partial === true ? ' is-partial' : ''}`}>
      <div className="lc-callsaid__bubble">{line.text}</div>
      {partial !== true && <SaidMark />}
    </div>
  }
  return <div className={`lc-callsaid is-voice${partial === true ? ' is-partial' : ''}`}>
    <span className="lc-callsaid__gutter">
      {teammate !== undefined && <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={20} teammateId={teammate.teammateId} hopsWhenDone={false} />}
    </span>
    <div className="lc-callsaid__body">
      <p className="lc-callsaid__text">{line.text}</p>
      {partial !== true && <SaidMark />}
    </div>
  </div>
}

/** "subject" -> "Subject": a connector names its values in lower case; the card's other rows are capitalised. */
const rowName = (name: string): string => `${name.charAt(0).toUpperCase()}${name.slice(1)}`

/*
 * A CONNECTOR'S QUESTION, ON A CARD IN THE CONVERSATION (0.749). Canvas board 2, "Waiting on you: Allow is a card in
 * the thread": on a call with a teammate, what a connector asks before it sends or changes anything is the same card
 * as every other approval in Locust -- the action, each value it would send, what leaves this machine, whether it can
 * be undone -- and only the card allows it. A yes said on the call is words, not an answer.
 */
function CallAskCard({ ask, name }: { readonly ask: LiveCallAsk; readonly name: string }): ReactElement {
  const [busy, setBusy] = useState(false)
  // A card that has just appeared takes no answer: a double click's second half (ApprovalCard's N9).
  const shownAt = useRef(Date.now())
  const answer = (allowed: boolean): void => {
    if (busy || Date.now() - shownAt.current < FRESH_CARD_MS) return
    setBusy(true)
    void answerLiveCallAsk(ask.id, allowed).then((reached) => { if (!reached) setBusy(false) })
  }
  return <div className="lc-card is-pending is-amber lc-callask" role="group" aria-label={`Allow ${ask.what}?`} data-call-ask={ask.id}>
    <div className="lc-card__head">
      <span className="lc-approval__title"><Icon name="shield" size={13} />{' '}Approval required — exact action</span>
      <span className="lc-rail__meta">{`On the call with ${name}`}</span>
    </div>
    <dl className="lc-receipt">
      <dt>Action</dt>
      <dd>{ask.what}</dd>
      {ask.fields.length === 0 && ask.detail.length > 0 && <><dt>Exact</dt><dd className="lc-mono lc-approval__detail">{ask.detail}</dd></>}
      {ask.fields.map((field, index) => <Fragment key={`${field.name}-${String(index)}`}><dt>{rowName(field.name)}</dt><dd>{field.value}</dd></Fragment>)}
      <dt>Data sent</dt>
      <dd>{dataSentLine('connector', '')}</dd>
      <dt>Reversible</dt>
      <dd>{CONNECTOR_NOT_UNDOABLE}</dd>
    </dl>
    <div className="lc-approval__actions">
      <button type="button" className="lc-primarybutton" disabled={busy} onClick={() => { answer(true) }}>Allow once</button>
      <button type="button" className="lc-denybutton" disabled={busy} onClick={() => { answer(false) }}>Deny</button>
    </div>
    <p className="lc-approval__note">Nothing has happened yet. Saying yes on the call does not allow it; Allow once does.</p>
  </div>
}

/*
 * A CALL'S TO-DOS (0.755, callTodos.ts): who has each, what was asked, how it is going, and -- for one whose turn is
 * not already on screen -- a press that opens its conversation. The strip lists them during a call from Home and sums
 * them up when it ends; a call with a teammate sums them up under its place in their conversation.
 */
export function CallTodoList({ handed, label, here, openable = true }: {
  readonly handed: readonly TodoShown[]
  readonly label: string
  /** The turns of the conversation on screen: a to-do among them is right there, and has nothing to open. */
  readonly here?: ReadonlySet<string>
  /** False while the call is on: opening a conversation would leave the one the call lives in, and end it. */
  readonly openable?: boolean
}): ReactElement {
  const { states, open } = useContext(CallTodosContext)
  return <section className="lc-calltodos" aria-label={label}>
    <p className="lc-calltodos__label lc-mono">{label}</p>
    <ul className="lc-calltodos__list">
      {handed.map((todo) => {
        const missionId = todo.missionId
        const words = CALL_TODO_WORDS[(missionId === undefined ? undefined : states.get(missionId)) ?? 'starting']
        const opens = openable && missionId !== undefined && here?.has(missionId) !== true
        return <li key={todo.key ?? missionId} className="lc-calltodo" data-call-todo={missionId}>
          <span className="lc-calltodo__who">{todo.name}</span>
          <span className="lc-calltodo__task" title={todo.task}>{todo.task}</span>
          <span className={`lc-status lc-status--${words.tone}`}>{words.label}</span>
          {opens && <button type="button" className="lc-control lc-control--boxed lc-calltodo__open" onClick={() => { open(missionId) }} aria-label={`Open ${todo.name}’s conversation: ${todo.task}`}>Open</button>}
        </li>
      })}
    </ul>
  </section>
}

export function CallWords({ piece, teammate, open, onToggle, here }: {
  readonly piece: CallPieceShown
  readonly teammate: PublicTeammate | undefined
  /** An ended call shows its words only when opened. */
  readonly open: boolean
  readonly onToggle: () => void
  /** The turns of the conversation on screen, for the summary (0.755): a to-do among them has nothing to open. */
  readonly here?: ReadonlySet<string>
}): ReactElement {
  const { call, live } = piece
  const shut = !live && !open
  const said = call.lines.length
  const partial = live && piece.last ? piece.partial : undefined
  /*
   * The summary sums up: two or more to-dos, or one in another teammate's conversation. A call that asked one thing
   * of this teammate has its turn right above, saying how it is going -- a card repeating it was clutter (the drive's
   * frame, 2026-10-12).
   */
  const handed = call.handed ?? []
  const summed = handed.length > 1 || handed.some((todo) => here?.has(todo.missionId) !== true)
  return <>
    {piece.first && (shut
      ? <button type="button" className="lc-callrow" onClick={onToggle} aria-expanded={false}>
        <span className="lc-callrow__mark"><Icon name="waveform" size={14} /></span>
        <span className="lc-callrow__words">
          <span className="lc-callrow__title">{`Call with ${call.name} · ${callLength(call)}`}</span>
          <span className="lc-callrow__note">{`${String(said)} ${said === 1 ? 'line' : 'lines'} said${call.turns.length > 0 ? ` · asked ${call.name} ${String(call.turns.length)} ${call.turns.length === 1 ? 'time' : 'times'}` : ''}`}</span>
        </span>
        <span className="lc-callrow__open">Show what was said</span>
      </button>
      : <div className="lc-calldivider lc-mono">
        <span className="lc-calldivider__rule" />
        <span className="lc-calldivider__words">
          {live && <span className="lc-calldivider__dot" aria-hidden="true" />}
          {`Call with ${call.name} · ${at(call.startedAt)}`}
        </span>
        {!live && <button type="button" className="lc-calldivider__hide" onClick={onToggle} aria-expanded={true}>Hide</button>}
        <span className="lc-calldivider__rule" />
      </div>)}
    {!shut && piece.lines.map((line, index) => <Line key={`${String(line.at)}-${String(index)}`} line={line} teammate={teammate} />)}
    {partial !== undefined && (['you', 'voice'] as const).map((role) => partial[role].trim().length > 0
      && <Line key={`partial-${role}`} line={{ role, text: partial[role].trim(), at: Date.now() }} teammate={teammate} partial />)}
    {live && piece.last && (piece.asks ?? []).map((ask) => <CallAskCard key={ask.id} ask={ask} name={call.name} />)}
    {!shut && !live && piece.last && <div className="lc-thread__marker lc-mono">{`Call ended · ${callLength(call)}`}</div>}
    {/* The summary, where the call ends, open or folded (0.755): what it handed out and how each is going. */}
    {!live && piece.last && summed && <CallTodoList handed={handed} label={`From the call · ${todoCount(handed.length)}`} {...(here === undefined ? {} : { here })} />}
  </>
}
