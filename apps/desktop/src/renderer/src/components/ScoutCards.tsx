import { useEffect, useRef, useState } from 'react'
import type { ReactElement, Ref } from 'react'

import type { PublicTeammate } from '../../../shared/ipc.js'
import type { ScoutCard, ScoutDraft, ScoutFix, ScoutOpen, ScoutState } from '../../../shared/scout.js'
import { Icon } from './Icon.js'
import { TeammateBot } from './TeammateBot.js'
import { useModal } from '../useModal.js'

/*
 * WHAT SCOUT NOTICED, ON HOME (0.756, shared/scout.ts; the design: https://claude.ai/artifact/2BMzn1KUaZNnNvJzBHgEfG).
 * Each card leads with Locust's own sentence; what Scout's model adds -- its take, and the next step drafted for a
 * teammate -- sits under it as Scout's. A card that needs the person now is raised, in amber; the rest wait. Nothing
 * happens until a press: Send starts the drafted turn as that teammate's own, Change it first puts it in the box.
 */
const SHOWN = 3

export function ago(iso: string, now: Date): string {
  const minutes = Math.max(0, Math.round((now.getTime() - Date.parse(iso)) / 60_000))
  if (!Number.isFinite(minutes)) return ''
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${String(minutes)} min ago`
  const hours = Math.round(minutes / 60)
  return hours < 24 ? `${String(hours)} h ago` : `${String(Math.round(hours / 24))} d ago`
}

/** "2 files", "1 file". */
const filesWord = (count: number): string => `${String(count)} file${count === 1 ? '' : 's'}`
/** The files a fix touched, as many as fit on a line. */
const SHOWN_FILES = 4

/*
 * A FIX SCOUT PREPARED (0.828, main/scout-fix.ts), under a failing check's sentence: who is preparing it, then ready
 * -- how many files, and that the check passes in the copy -- with Review (its conversation), Apply and Skip; or what
 * was missing. Nothing reaches the folder before Apply.
 */
export function FixLine({ fix, cardId, onOpen }: { readonly fix: ScoutFix; readonly cardId: string; readonly onOpen: (open: ScoutOpen) => void }): ReactElement {
  const [busy, setBusy] = useState(false)
  const [failed, setFailed] = useState<string>()
  const press = (work: (() => Promise<unknown>) | undefined, fallback: string): void => {
    if (work === undefined) return
    setBusy(true); setFailed(undefined)
    void work().catch(() => { setFailed(fallback) }).finally(() => { setBusy(false) })
  }
  const scout = typeof window === 'undefined' ? undefined : window.desktop?.scout
  const review = (missionId: string | undefined, label = 'Review') => missionId === undefined ? null
    : <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { onOpen({ kind: 'conversation', missionId, label }) }}>{label}</button>
  const again = <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { press(scout?.fixAgain === undefined ? undefined : () => scout.fixAgain!(cardId), 'It could not be prepared again. Try once more.') }}>Prepare again</button>
  const files = (list: readonly string[]) => list.length === 0 ? null
    : <p className="lc-scoutfix__files lc-mono">{[...list.slice(0, SHOWN_FILES), ...(list.length > SHOWN_FILES ? [`and ${String(list.length - SHOWN_FILES)} more`] : [])].join(' · ')}</p>
  let said: string
  let acts: ReactElement | null = null
  let shown: ReactElement | null = null
  switch (fix.state) {
    case 'preparing':
      said = `${fix.name} is preparing a fix in a copy of the folder…`
      break
    case 'ready':
      said = `Fix ready: ${filesWord(fix.files.length)} changed${fix.added === undefined ? '' : ` (+${String(fix.added)} −${String(fix.removed ?? 0)})`}, the check passes.`
      shown = files(fix.files)
      acts = <>
        {review(fix.missionId)}
        <button type="button" className="lc-primarybutton" disabled={busy} onClick={() => { press(scout?.applyFix === undefined ? undefined : () => scout.applyFix!(cardId), 'The fix could not be applied. Nothing in your folder changed.') }}>Apply</button>
        <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { press(scout?.skipFix === undefined ? undefined : () => scout.skipFix!(cardId), 'It could not be skipped. Try again.') }}>Skip</button>
      </>
      break
    case 'applied':
      said = `Applied: ${filesWord(fix.files.length)} in your folder, beside your own changes. Opening a pull request is yours.`
      shown = files(fix.files)
      break
    case 'stale':
      said = `Not applied: the fix was made on an older copy, and you changed ${fix.files.length === 1 ? 'that file' : 'those files'} since.`
      shown = files(fix.files)
      acts = <>{review(fix.missionId)}{again}</>
      break
    case 'unfixed':
      said = `${fix.name} could not prepare a fix: ${fix.why}`
      acts = <>{review(fix.missionId, 'Open it')}{again}</>
      break
    case 'held':
      said = fix.why
      acts = again
      break
  }
  return <div className={`lc-scoutfix${fix.state === 'ready' ? ' is-ready' : ''}`} data-scout-fix={fix.state}>
    <p className={fix.state === 'preparing' ? 'lc-scoutcard__reading lc-mono' : 'lc-scoutfix__said'} role="status">{said}</p>
    {shown}
    {failed !== undefined && <p className="lc-scoutfix__said" role="alert">{failed}</p>}
    {acts !== null && <div className="lc-scoutcard__acts">{acts}</div>}
  </div>
}

function Card({ card, scout, now, onOpen, onSend, onDraftIntoBox, onDismiss, onTune, cardRef }: {
  readonly card: ScoutCard
  readonly scout: PublicTeammate | undefined
  readonly now: Date
  readonly onOpen: (open: ScoutOpen) => void
  readonly onSend: (draft: ScoutDraft, cardId: string) => void
  readonly onDraftIntoBox: (draft: ScoutDraft, cardId: string) => void
  readonly onDismiss: (cardId: string) => void
  readonly onTune?: (cardId: string) => void
  readonly cardRef?: Ref<HTMLElement>
}): ReactElement {
  const [editing, setEditing] = useState(false)
  const [postText, setPostText] = useState(card.post?.message ?? '')
  const [posting, setPosting] = useState(false)
  const [postError, setPostError] = useState<string>()
  const needsYou = card.urgency === 'now'
  const brief = card.key.startsWith('brief:')
  const week = card.key.startsWith('week:')
  // A fix under way, ready or applied is the next step (0.828): the handoff drafted for the same check is not offered beside it.
  const draft = card.fix !== undefined && ['preparing', 'ready', 'applied'].includes(card.fix.state) ? undefined : card.draft
  return <article ref={cardRef} className={`lc-card lc-scoutcard${needsYou ? ' is-pending is-amber' : card.urgency === 'later' ? ' is-terminal' : ''}`} data-scout-card={card.cardId}>
    <div className="lc-scoutcard__head">
      {scout !== undefined && <TeammateBot hue={scout.hue} avatar={scout.avatar} size={20} teammateId={scout.teammateId} hopsWhenDone={false} />}
      <span className={`lc-scoutcard__kind${needsYou ? ' lc-tone-amber' : ''}`}>{needsYou ? 'Needs you' : brief ? 'Morning brief' : week ? 'Your week' : draft !== undefined ? 'Noticed, and drafted' : 'Noticed'}</span>
      <span className="lc-scoutcard__when lc-mono">{`${scout?.name ?? 'Scout'} · ${ago(card.at, now)}`}</span>
    </div>
    <p className="lc-scoutcard__said">{card.said}</p>
    {/* A morning brief's lines (0.816), and a week's (0.817): each Locust's own count. */}
    {card.lines !== undefined && card.lines.length > 0 && <ul className="lc-scoutcard__lines">
      {card.lines.map((line, index) => <li key={String(index)}>{line}</li>)}
    </ul>}
    {card.quote !== undefined && <p className="lc-scoutcard__quote lc-mono">{card.quote}</p>}
    {card.chips !== undefined && card.chips.length > 0 && <div className="lc-scoutcard__chips">
      {card.chips.map((chip, index) => <span key={`${chip.text}-${String(index)}`} className={`lc-status lc-status--${chip.tone}`}>{chip.text}</span>)}
    </div>}
    {card.take !== undefined && <p className="lc-scoutcard__take"><span className="lc-scoutcard__who">{scout?.name ?? 'Scout'}:</span> {card.take}</p>}
    {card.reading === true && <p className="lc-scoutcard__reading lc-mono">{`${scout?.name ?? 'Scout'} is reading it…`}</p>}
    {card.fix !== undefined && <FixLine fix={card.fix} cardId={card.cardId} onOpen={onOpen} />}
    {draft !== undefined && <div className="lc-scoutcard__draft">
      <p className="lc-scoutcard__draftlabel lc-mono">{`Drafted for ${draft.name}`}</p>
      <p className="lc-scoutcard__drafttext">{draft.message}</p>
    </div>}
    {card.post !== undefined && <div className="lc-scoutcard__draft">
      {editing ? <><textarea aria-label="Comment to post" value={postText} onChange={(event) => { setPostText(event.target.value) }} />
        <button type="button" className="lc-control lc-control--boxed" onClick={() => {
          void window.desktop?.scout?.editPost(card.cardId, postText).then(() => { setEditing(false) }).catch(() => { setPostError('The edit could not be saved. Your comment has not been posted.') })
        }}>Keep this text</button></> : <p className="lc-scoutcard__drafttext">{card.post.message}</p>}
      {card.post.postedUrl !== undefined && <><p>Posted on GitHub.</p><button type="button" className="lc-control lc-control--boxed" onClick={() => { onOpen({ kind: 'url', url: card.post!.postedUrl!, label: 'Open the comment' }) }}>Open the comment</button></>}
      {card.post.failed !== undefined && <p>{card.post.failed}</p>}
      {postError !== undefined && <p>{postError}</p>}
    </div>}
    <div className="lc-scoutcard__acts">
      {card.post !== undefined && !card.post.attempted && !editing && <>
        <button type="button" className="lc-primarybutton" disabled={posting} onClick={() => {
          setPosting(true)
          void window.desktop?.scout?.post(card.cardId).catch(() => { setPostError('Posting could not be confirmed. Check GitHub before posting again.') }).finally(() => { setPosting(false) })
        }}>{posting ? 'Posting...' : `Post it on #${String(card.post.number)}`}</button>
        <button type="button" className="lc-control lc-control--boxed" onClick={() => { setPostText(card.post!.message); setEditing(true) }}>Change it first</button>
      </>}
      {draft !== undefined && <button type="button" className="lc-primarybutton" onClick={() => { onSend(draft, card.cardId) }}>{`Send to ${draft.name}`}</button>}
      {draft !== undefined && <button type="button" className="lc-control lc-control--boxed" onClick={() => { onDraftIntoBox(draft, card.cardId) }}>Change it first</button>}
      {card.open !== undefined && <button type="button" className={draft === undefined && needsYou ? 'lc-primarybutton' : 'lc-control lc-control--boxed'} onClick={() => { onOpen(card.open!) }}>{card.open.label}</button>}
      {/* A weekly tune-up's one suggestion (0.817): the press applies it. */}
      {card.tune !== undefined && onTune !== undefined && <button type="button" className="lc-primarybutton" onClick={() => { onTune(card.cardId) }}>{card.tune.label}</button>}
      <button type="button" className="lc-ghostbutton lc-scoutcard__dismiss" onClick={() => { onDismiss(card.cardId) }}>Dismiss</button>
    </div>
  </article>
}

export function ScoutCards({ state, scout, onOpen, onSend, onDraftIntoBox, onDismiss, onTune, now = new Date(), focusCardId }: {
  readonly state: ScoutState
  /** Scout's own teammate: its face and name. */
  readonly scout: PublicTeammate | undefined
  readonly onOpen: (open: ScoutOpen) => void
  readonly onSend: (draft: ScoutDraft, cardId: string) => void
  readonly onDraftIntoBox: (draft: ScoutDraft, cardId: string) => void
  readonly onDismiss: (cardId: string) => void
  /** Applies a weekly tune-up's suggestion (0.817). */
  readonly onTune?: (cardId: string) => void
  readonly now?: Date
  /** Brought here from the title bar or the Board: that card is in view, even past the first three. */
  readonly focusCardId?: string
}): ReactElement | null {
  const [all, setAll] = useState(false)
  const focusRef = useRef<HTMLElement>(null)
  useEffect(() => {
    focusRef.current?.scrollIntoView({ block: 'center' })
  }, [focusCardId])
  if (!state.settings.on || state.cards.length === 0) return null
  // What needs the person now comes first, whenever it was found.
  const ordered = [...state.cards.filter((card) => card.urgency === 'now'), ...state.cards.filter((card) => card.urgency !== 'now')]
  const shown = all ? ordered : ordered.slice(0, SHOWN)
  const visible = focusCardId === undefined || shown.some((card) => card.cardId === focusCardId)
    ? shown
    : [...ordered.filter((card) => card.cardId === focusCardId), ...shown]
  const name = scout?.name ?? 'Scout'
  return <section className="lc-scout" aria-label={`${name} noticed`}>
    <div className="lc-scout__head">
      <span className="lc-scout__heading"><Icon name="eye" size={13} />{` ${name} noticed · ${String(state.cards.length)}`}</span>
      {ordered.length > SHOWN && <button type="button" className="lc-ghostbutton" onClick={() => { setAll(!all) }}>{all ? 'Show fewer' : `Show all ${String(ordered.length)}`}</button>}
    </div>
    {state.modelSaid !== undefined && <p className="lc-scout__said lc-mono">{`${state.modelSaid} The cards below are in Locust’s words.`}</p>}
    {visible.map((card) => <Card key={card.cardId} card={card} scout={scout} now={now} onOpen={onOpen} onSend={onSend} onDraftIntoBox={onDraftIntoBox} onDismiss={onDismiss} {...(onTune === undefined ? {} : { onTune })} {...(card.cardId === focusCardId ? { cardRef: focusRef } : {})} />)}
  </section>
}

/*
 * KEEPING WATCH, WITH NOTHING TO SAY (0.768). Colin, 2026-10-13, of Scout: "you tell me they are working properly but
 * i have not seen anything, is that normal". Scout speaks only when something needs the person, so a look that found
 * nothing drew nothing at all, and a Scout keeping watch read as one that was not. One quiet line says it is on, when
 * it last looked, and offers a look now. The time moves on by itself, once a minute.
 */
export function ScoutWatching({ state, scout, onLookNow, looking, now: fixed }: {
  readonly state: ScoutState
  readonly scout: PublicTeammate | undefined
  readonly onLookNow: () => void
  readonly looking?: boolean
  readonly now?: Date
}): ReactElement {
  const [ticked, setTicked] = useState(() => new Date())
  useEffect(() => {
    if (fixed !== undefined) return undefined
    const timer = setInterval(() => { setTicked(new Date()) }, 60_000)
    return () => { clearInterval(timer) }
  }, [fixed])
  const now = fixed ?? ticked
  const name = scout?.name ?? 'Scout'
  return <section className="lc-scout lc-scout--watching" aria-label="Scout keeps watch">
    <div className="lc-scout__head">
      <span className="lc-scout__heading"><Icon name="eye" size={13} />{` ${name} keeps watch`}</span>
      <span className="lc-scout__looked lc-mono">{state.lastLookAt === undefined ? 'its first look is on its way' : `looked ${ago(state.lastLookAt, now)} · nothing needs you`}</span>
      <button type="button" className="lc-ghostbutton" disabled={looking === true} onClick={onLookNow}>{looking === true ? 'Looking…' : 'Look now'}</button>
    </div>
  </section>
}

/*
 * SCOUT'S WATCH, ON ITS OWN CARD (0.799). Colin, 2026-10-19: "find a way to fold away the scout watch in a more clean
 * way, like on the team bar". Scout is a teammate, so when nothing needs the person its line is the line under its
 * name on Home's team card, in the role's place, and Look now is the eye on that card. A line of its own above the team
 * cost Home a row. The time moves on by itself, once a minute.
 */
export function ScoutLooked({ state, looking, now: fixed }: {
  readonly state: ScoutState
  readonly looking: boolean
  readonly now?: Date
}): ReactElement {
  const [ticked, setTicked] = useState(() => new Date())
  useEffect(() => {
    if (fixed !== undefined) return undefined
    const timer = setInterval(() => { setTicked(new Date()) }, 60_000)
    return () => { clearInterval(timer) }
  }, [fixed])
  const now = fixed ?? ticked
  return <>{looking ? 'Looking…' : state.lastLookAt === undefined ? 'First look on its way' : `Looked ${ago(state.lastLookAt, now)} · all clear`}</>
}

/*
 * MEET SCOUT (0.756): the one press that adds it to the team, on the best free model, for a person with a team and a
 * folder. It says what Scout reads, what it sends and to whom, before it is on -- and that it never acts by itself.
 *
 * A DIALOG, OPENED FROM SCOUT'S PLACE ON THE TEAM (0.808). It stood on Home as a card four lines tall above the team,
 * and Home scrolled at 1205x803 (148px), 1366x768 (183px) and even 1440x900 (51px) -- against "is there a way to fit
 * all this in one screen with no scrolling" (Colin, 2026-10-19). Now the offer is a card in the team's grid, in the
 * slot Scout takes once it is on (HomeTeam's `invite`), and its words are here, read before the press that turns it on.
 */
export function ScoutInvite({ model, onTurnOn, onNotNow, onClose, busy, message }: {
  /** The model Scout would read with, as a person reads it: "Space Bunny through OpenCode, free". */
  readonly model: string
  readonly onTurnOn: () => void
  readonly onNotNow: () => void
  readonly onClose: () => void
  readonly busy?: boolean
  readonly message?: string
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose)
  return <div className="lc-scrim">
    <div ref={box} className="lc-dialog lc-scoutinvite" role="dialog" aria-modal="true" aria-label="Meet Scout">
      <div className="lc-dialog__head">
        <span className="lc-dialog__title">Meet Scout, a teammate that keeps watch</span>
        <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onClose}>
          <Icon name="close" size={13} />
        </button>
      </div>
      <div className="lc-dialog__body lc-scoutinvite__text">
        <p className="lc-scoutinvite__lede">Scout looks at your pull requests and their checks, the reviews asked of you and your teammates’ turns every 10 minutes, and tells you here what needs you, with the next step drafted. It never sends, merges or changes anything: you press.</p>
        <p className="lc-scoutinvite__sends">{`When something is new, Scout’s model reads it: titles, check names, a waiting teammate’s question. That model is ${model}. Change it like any teammate’s.`}</p>
        {message !== undefined && <p className="lc-scoutinvite__message" role="status">{message}</p>}
      </div>
      <div className="lc-dialog__foot">
        <button type="button" className="lc-button" onClick={onNotNow}>Not now</button>
        <button type="button" className="lc-primarybutton" disabled={busy === true} onClick={onTurnOn}>Turn on Scout</button>
      </div>
    </div>
  </div>
}
