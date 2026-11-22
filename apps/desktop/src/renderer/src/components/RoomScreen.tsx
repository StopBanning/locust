import type { KeyboardEvent, ReactElement } from 'react'
import { BOT_SIZE } from '../botSizes.js'
import { useEffect, useRef, useState } from 'react'

import type { PublicRoom, PublicTeammate, RoomTaskRequest } from '../../../shared/ipc.js'
import { roleLabelOf } from '../../../shared/ipc.js'
import type { ThreadItem } from '../missionView.js'
import { ThreadItems } from './Thread.js'
import { MAX_ROOM_TEAMMATES } from '../../../shared/live-missions.js'
import { AgentText, LiveRegisterLine } from './ThreadItems.js'
import { Icon } from './Icon.js'
import type { ContextMenuState } from './ContextMenu.js'
import { TeammateBot } from './TeammateBot.js'
import { footLine } from '../roomExchange.js'
import { modelDisplayName, routeChrome } from '../routeName.js'
import { shortAgo } from '../railFlyout.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { useFollowBottom } from '../useFollowBottom.js'
import { QUIET_SECONDS_BEFORE_SAYING_SO } from '../quiet.js'
import { JumpToBottom } from './JumpToBottom.js'
import type { RoomExchange } from '../roomExchange.js'
import { mentionAt, mentionChoices, mentionCompleted, namesSaid, withoutMention } from '../roomMentions.js'
import { clock } from '../../../shared/clock.js'

/**
 * A room: a named set of teammates and the thread of what a person said to
 * all of them, with each one's answer under each post.
 *
 * A surface over records the window already holds (vision #2, "a surface,
 * not a new engine"): a post starts one ordinary mission per teammate on
 * that teammate's own route, the room remembers which, and the answer card
 * under a post is read from that mission -- its phase and its last words.
 * Each card opens the mission it was read from, so nothing here is a
 * summary a person cannot check.
 */

export interface RoomAnswer {
  readonly teammateId: string
  readonly missionId: string
  /** What the record says: starting, running, completed, failed, cancelled, interrupted -- or unknown. */
  readonly phase: string
  /** When the asking started: the first event, or the post for a run with none. */
  readonly startedAt: string | undefined
  /** The teammate's final words in that mission, when there are any yet. */
  readonly text: string | undefined
  /**
   * The whole turn, as the thread would draw it: every message, the work
   * fold with its tool calls, the plan and its progress, the live line.
   *
   * `text` above is the LAST final message and was all a room ever showed, so
   * a teammate that said three things showed one, and a teammate still
   * thinking showed "working...". Colin, 2026-09-12: "there are parts of the
   * chat that are missing... we already have this exact same system we just
   * need to be able to have it work with multiple in one chat."
   *
   * `text` stays for the one thing it is still better at: the collapsed
   * answers list a large room falls back to.
   */
  readonly items: readonly ThreadItem[]
  readonly runtime: string
  readonly model: string
}

/**
 * A room's card on the Rooms screen: who is in it, how much has been said,
 * and when (0.416).
 *
 * It read "2 teammates · 1 post" -- a count where a person looks for faces
 * they know, and nothing to say which room they used last (fresh-eyes area
 * 10). Three names fit; past that, two and how many more.
 */
export function roomCardMeta(
  room: Pick<PublicRoom, 'teammateIds' | 'posts' | 'createdAt'>,
  teammates: readonly Pick<PublicTeammate, 'teammateId' | 'name'>[],
  now: Date = new Date()
): string {
  const names = room.teammateIds
    .map((id) => teammates.find((entry) => entry.teammateId === id)?.name)
    .filter((name): name is string => name !== undefined)
  const who = names.length <= 3 ? names.join(' · ') : `${names.slice(0, 2).join(' · ')} +${String(names.length - 2)}`
  const said = room.posts.length === 0 ? 'nothing posted yet' : `${String(room.posts.length)} post${room.posts.length === 1 ? '' : 's'}`
  const ago = shortAgo(room.posts[room.posts.length - 1]?.at ?? room.createdAt, now)
  return [who, said, ago].filter((part): part is string => part !== undefined && part.length > 0).join(' · ')
}

/**
 * What is wrong with a room this size, while it can still be changed.
 *
 * This used to warn that a room bigger than `MAX_LIVE_MISSIONS` would only
 * start that many -- true when the cap was 4 and the room limit 8. The cap
 * has since been measured and raised to 8, and the two are now held equal by
 * `room-and-mission-caps-agree`, so that sentence can never be true again
 * and has gone.
 *
 * What replaced it is the fact that IS true past eight and was said nowhere:
 * the store refuses the room. Ticking a ninth teammate was allowed, and
 * Create room then failed with "A room needs between 1 and 8 teammates." --
 * offered and then refused, in the one place the number can still change.
 *
 * The design agent's note about register applies here and resolves the other
 * way. Amber says a person has something to do, and their objection was that
 * the cap is a fact about the machine while ticking is allowed. This is not
 * that: the room cannot be made, and unticking is the reader's to do. The
 * register was wrong because the fact was wrong.
 */
export function roomFullNote(ticked: number): string | undefined {
  if (ticked <= MAX_ROOM_TEAMMATES) return undefined
  return `A room holds ${String(MAX_ROOM_TEAMMATES)} teammates. Untick ${String(ticked - MAX_ROOM_TEAMMATES)} to make this one.`
}

/**
 * Why Create room is not available, in the order a person hits them.
 *
 * SAME RULE, THREE BRANCHES, AND ONLY ONE SPOKE. The button is disabled on
 * an empty name, on nobody ticked, and on more than eight ticked -- and the
 * third one explained itself in amber with the number to untick while the
 * first two were `opacity: 0.5` and silence. A person who has typed nothing
 * yet can work out the first; a person who has typed a name and is looking
 * at a row of teammates cannot tell whether the button is broken.
 *
 * `roomFullNote` keeps its own wording and its amber register: that one is
 * about a cap the machine enforces. These two are about a step not taken
 * yet, so they are quiet rather than amber -- see the register argument
 * above, which resolves the other way for them.
 */
export function roomBlockedReason(name: string, ticked: number): string | undefined {
  const full = roomFullNote(ticked)
  if (full !== undefined) return full
  if (name.trim().length === 0) return 'Name the room to create it.'
  if (ticked === 0) return 'Pick at least one teammate.'
  return undefined
}

/**
 * The people a post did not reach, as one line.
 *
 * A member with no mission used to get an ANSWER CARD with nothing in it --
 * an avatar, a name, `did not start` beside it, and then about 90px of void
 * where a route, a phase, an Open button and an answer belong. In a grid of
 * answers a card is a promise that an answer is inside it, so an empty one
 * reads as broken however it is coloured; and because a grid forces
 * equal-height cells, those two empty cards took their height from an
 * unrelated string in a neighbouring cell (the model name wrapping to two
 * lines) rather than from anything of their own.
 *
 * They share one fact, so they are one line.
 *
 * The reason is NOT in the record -- the refusal is a transient response, so
 * a reload has only the absence -- and this must not invent one. The cap is
 * named only when the cap can actually have been what bit: as many missions
 * started as are allowed to run.
 */
export function absentLine(absent: readonly { readonly name: string; readonly reason?: string }[]): string | undefined {
  if (absent.length === 0) return undefined
  const say = (names: readonly string[]): string =>
    names.length === 1
      ? String(names[0])
      : `${names.slice(0, -1).join(', ')} and ${String(names[names.length - 1] ?? '')}`
  const verb = (names: readonly string[]): string => (names.length === 1 ? 'was' : 'were')

  // Grouped by reason, so people turned away by the same thing are one
  // sentence rather than one each -- the mistake the composer note used to
  // make, repeated once per person in the smallest text on the screen.
  const byReason = new Map<string, string[]>()
  for (const entry of absent) byReason.set(entry.reason ?? '', [...(byReason.get(entry.reason ?? '') ?? []), entry.name])

  return [...byReason]
    .map(([reason, names]) =>
      reason === ''
        ? // No recorded reason: posts written before refusals were kept, and
          // anything the host declined to explain. Says less rather than
          // inventing why.
          `${say(names)} ${verb(names)} not asked.`
        : `${say(names)} ${verb(names)} not asked — ${reason.charAt(0).toLowerCase()}${reason.slice(1)}`
    )
    .join(' ')
}

/**
 * The host's refusals as one notice, said once per REASON.
 *
 * This was written inline as one sentence per refused teammate, joined with
 * a separator, so a room where the live cap turned two people away read:
 *
 *   Otto: Up to 4 missions can run at once. Wait for one to finish or stop
 *   it first. · Pike: Up to 4 missions can run at once. Wait for one to
 *   finish or stop it first.
 *
 * The same sentence twice, in the smallest text on the screen, 500px below
 * the cards it explains, and repeated once per person it happened to. It
 * also displaced the room's own note, which shares that slot.
 *
 * Grouped by message, because the message is what they actually share. Two
 * people turned away by the cap are one fact with two names; two turned away
 * for different reasons stay two lines.
 */
/**
 * Whether a post's answers are laid out as a grid or as a list.
 *
 * A grid forces EQUAL-HEIGHT CELLS, and answers are of wildly unequal
 * length, so every row is as tall as its longest cell. It is paid in
 * whitespace and it worsens with width, because a wider row has more
 * chances to contain one long answer. It is visible even when every answer
 * is one word: on 2026-09-09 two cards took their height from
 * `opencode/muse-spark-1.3-contributor-free` wrapping to two mono lines in
 * the card beside them.
 *
 * Survivable at six, where a row is three cells and one glance. Past that a
 * grid scrolls, and it is then costing the only thing it was for -- seeing
 * the room at once -- while still paying the whitespace. A list gives every
 * answer one column width and its own height, which is what prose wants,
 * and puts the room in the same shape as the thread.
 *
 * Counted on RENDERED answers rather than room members, because rows are
 * what break. A room of eight where three were never asked draws five.
 */
export const ANSWERS_BEFORE_A_LIST = 6

/**
 * Who is still waiting for a slot, as one recessed line.
 *
 * The design agent's answer to "queue, or don't", and the part of it that
 * makes the queue cheap: it needs ONE new state, not three.
 *
 * Waiting is the only real one -- nothing is happening, nothing is wrong,
 * nothing wants the reader -- so it is a roster line in the standing
 * register, not a card. No avatar, no box per person, no answer-shaped
 * container, because there is no answer and there is not going to be one
 * YET. A name leaves this line and becomes a card the moment its mission
 * starts, and that is the only transition the queue has to draw.
 *
 * "Next" is not a state, it is a position, and it changes with no action by
 * the reader -- drawing it would make the screen mutate to say which of two
 * identical waits is fractionally sooner. So the line is ordered and
 * position is left to be position.
 */
/**
 * An answer, whole.
 *
 * It used to fold past twelve rendered lines behind `Show the rest`. Colin
 * took it out, 2026-09-11: "just let them post uninhibited in chat."
 *
 * He is right about what a room IS. A folded answer is a summary the reader
 * did not ask for, and a room is the place two teammates argue in front of
 * you -- the argument is the content, so hiding two thirds of it and
 * offering a button is the wrong default for this surface. The thread has
 * its own fold for a turn's WORK, which is a different thing: that hides
 * tool calls, not prose.
 *
 * What the fold was really covering for is fixed rather than hidden now.
 * Long answers looked broken here because the live event window was eating
 * their beginnings (`messageFragments.ts`), so what a fold opened onto was
 * a reply starting mid-sentence.
 *
 * Kept as a component rather than inlined: the Markdown rendering is the
 * part that matters and it has its own reason, below.
 */
function RoomAnswerText({ text }: { readonly text: string }): ReactElement {
  return (
    /*
     * The thread's own reader, not a second one.
     *
     * A teammate answers in Markdown whether or not anyone asked, and this
     * used to be one plain paragraph -- so an argument drew literal
     * asterisks and hyphens where the same reply in the thread drew bold
     * and a list (MEASURED 2026-09-11: "**column alignment**" on screen).
     * What a teammate says reads the same wherever it is read.
     */
    <div className="lc-roomanswer__text">
      <AgentText text={text} streaming={false} />
    </div>
  )
}

/**
 * How long a member has been quiet, past which it is worth saying so.
 *
 * The number itself moved to `quiet.ts` on 2026-09-13, when the conversation
 * needed the same rule: one silence must not be described two ways. Still
 * exported from here because this is where it has been imported from.
 */
export { QUIET_SECONDS_BEFORE_SAYING_SO }

/**
 * What a member is doing, and for how long.
 *
 * The design agent's three facts, and the naming is the point: it says
 * ASKED, not `starting`. "Starting" is the app's word for its own dispatch
 * loop; the reader's fact is that we asked and nothing has come back.
 *
 *   asked · 2s
 *   asked · 45s · no word back yet        (past 20s, amber)
 *   running · 52s                          (at the first event)
 *
 * The elapsed count is the whole argument, and it is the same one as
 * elapsed-seconds instead of a progress bar: two seconds of silence is
 * normal, forty-five is alarming, and the only thing separating them is a
 * number we already have. Without it both are the word "starting".
 *
 * Past twenty seconds it adds a SENTENCE rather than changing phase --
 * nothing has changed, the run is not failing, it is quiet, and those are
 * different claims. It never goes red on its own: a launch that never speaks
 * ends as a failure through the normal path, with the runtime's own reason.
 */
function AnswerState({ phase, startedAt }: { readonly phase: string; readonly startedAt: string | undefined }): ReactElement {
  const [now, setNow] = useState(() => Date.now())
  const live = phase === 'running' || phase === 'starting'
  useEffect(() => {
    if (!live) return undefined
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [live])

  if (!live) return <span className="lc-roomanswer__phase lc-mono">{phase}</span>
  const began = startedAt === undefined ? undefined : Date.parse(startedAt)
  const seconds = began === undefined || Number.isNaN(began) ? undefined : Math.max(0, Math.round((now - began) / 1000))
  // `starting` means no event has arrived yet, which is the whole of "we
  // asked and nothing came back".
  const word = phase === 'starting' ? 'asked' : 'running'
  const quiet = word === 'asked' && seconds !== undefined && seconds >= QUIET_SECONDS_BEFORE_SAYING_SO
  return (
    <span className={`lc-roomanswer__phase lc-mono${quiet ? ' lc-tone-amber' : ''}`}>
      {word}
      {seconds !== undefined && ` · ${String(seconds)}s`}
      {quiet && ' · no word back yet'}
    </span>
  )
}

/**
 * One line about a post: what was asked, and what came of it.
 *
 * The design agent's rule, which is the trace line's rule: **count the
 * ordinary, name the exceptional.** A state one or two members are in gets
 * their names, because that is the fact you want; three or more goes back to
 * being a number, because four names is a list and a list is not a glance.
 *
 *   8 asked · all answered
 *   8 asked · 6 answered · Otto running · Sable failed
 *   8 asked · 5 answered · 3 failed
 *   6 asked · 2 answered · 4 running · 2 waiting for a slot
 *
 * A zero segment is absent rather than written as zero. And this is the
 * whole of "is anything wrong" -- a question asked ONCE, on arrival, which
 * needs a sentence that is true when you look rather than a view somebody
 * has to watch. Nobody watches a room while it runs; that was the argument
 * for the queue and it decides this too.
 */
export function postHeadline(
  members: readonly {
    readonly name: string
    readonly state: 'answered' | 'replied' | 'running' | 'failed' | 'waiting' | 'absent'
  }[]
): string {
  // `absent` is someone the post never reached, and `waiting` someone it has
  // not reached YET. Neither was asked, so neither is counted as asked, and
  // both are said elsewhere -- the absent line names them with the reason.
  const asked = members.filter((member) => member.state !== 'waiting' && member.state !== 'absent').length
  const parts: string[] = [`${String(asked)} asked`]
  const of = (state: string): readonly string[] =>
    members.filter((member) => member.state === state).map((member) => member.name)

  const answered = of('answered')
  if (answered.length > 0 && answered.length === asked && asked > 0) parts.push('all answered')
  else if (answered.length > 0) parts.push(`${String(answered.length)} answered`)

  // Spoke, still going. Said as one segment rather than counted twice, and
  // never folded into "answered": the whole value of this line is knowing
  // whether there is anything left to wait for.
  const replied = of('replied')
  if (replied.length > 0) {
    parts.push(
      replied.length <= 2
        ? `${replied.join(' and ')} replied, still working`
        : `${String(replied.length)} replied, still working`
    )
  }

  // Named at one or two, counted past that. `running` and `failed` are the
  // exceptions a person is looking for; `waiting` is always a count because
  // it is a queue position rather than something that happened.
  for (const [state, word] of [['running', 'running'], ['failed', 'failed']] as const) {
    const names = of(state)
    if (names.length === 0) continue
    parts.push(names.length <= 2 ? `${names.join(' and ')} ${word}` : `${String(names.length)} ${word}`)
  }
  const waiting = of('waiting')
  if (waiting.length > 0) parts.push(`${String(waiting.length)} waiting for a slot`)
  return parts.join(' · ')
}

export function waitingLine(names: readonly string[]): string | undefined {
  if (names.length === 0) return undefined
  return names.join(', ')
}

export function refusalNotice(refused: readonly { readonly name: string; readonly message: string }[]): string | undefined {
  if (refused.length === 0) return undefined
  const byReason = new Map<string, string[]>()
  for (const entry of refused) byReason.set(entry.message, [...(byReason.get(entry.message) ?? []), entry.name])
  return [...byReason].map(([message, names]) => `${names.join(', ')}: ${message}`).join(' · ')
}

export function RoomScreen({
  rooms,
  teammates,
  currentRoomId,
  answersFor,
  exchangeFor,
  exchangeCostText,
  onSelectRoom,
  onCreateRoom,
  onRemoveRoom,
  onMenu,
  onRenameRoom,
  onPost,
  onOpenMission,
  workspacePath,
  onTask,
  notice
}: {
  readonly rooms: readonly PublicRoom[]
  readonly teammates: readonly PublicTeammate[]
  readonly currentRoomId: string | undefined
  /** The answers under one post, read from the missions it started. */
  readonly answersFor: (room: PublicRoom, postId: string) => readonly RoomAnswer[]
  /**
   * The post as a CONVERSATION, when it became one.
   *
   * Undefined means nobody replied to anybody, and the grid of cards below is
   * the truth. Defined means the grid would be a lie -- it claims these
   * arrived in parallel and none is a reply to another -- so the post is
   * drawn as a sequence instead (design agent, 2026-09-11).
   */
  readonly exchangeFor?: (room: PublicRoom, postId: string) => RoomExchange | undefined
  /** What the exchange cost, in the runtime's own unit, already worded. */
  readonly exchangeCostText?: (room: PublicRoom, postId: string) => string | undefined
  readonly onSelectRoom: (roomId: string | undefined) => void
  readonly onCreateRoom: (name: string, teammateIds: readonly string[]) => Promise<string | undefined>
  readonly onRemoveRoom: (roomId: string) => void
  /** Opens the app's one context menu, so a room's header matches a conversation's. */
  readonly onMenu?: (menu: ContextMenuState) => void
  /** Give the room a name. Absent where renaming is not offered. */
  readonly onRenameRoom?: (roomId: string, name: string) => void
  /** `to`: the members the post is put to, when the person named someone; absent is everyone. */
  readonly onPost: (roomId: string, text: string, to?: readonly string[]) => Promise<string | undefined>
  readonly onOpenMission: (missionId: string) => void
  /**
   * The folder, so a file reads the same here as it does in the thread.
   *
   * It was undefined until 2026-09-13, which meant `relativePath` had nothing
   * to fold against and Astra measured one file appearing as a long absolute
   * path in the room and as `README.md` in the conversation. Two surfaces, one
   * fact, computed differently.
   */
  readonly workspacePath?: string
  /** A person moving the board. Resolves with the host's refusal, if any. */
  readonly onTask: (request: RoomTaskRequest) => Promise<string | undefined>
  /** The host's last word about a post or a room, when it had one. */
  readonly notice: string | undefined
}): ReactElement {
  const room = rooms.find((entry) => entry.roomId === currentRoomId)
  const [draftName, setDraftName] = useState('')
  /** The title is an input while this is on. */
  const [renaming, setRenaming] = useState(false)
  const [draftMembers, setDraftMembers] = useState<readonly string[]>([])
  const [draftText, setDraftText] = useState('')
  const [draftTask, setDraftTask] = useState('')
  /** The add field only exists once somebody asks for it. */
  const [adding, setAdding] = useState(false)
  const [boardError, setBoardError] = useState<string>()
  const [busy, setBusy] = useState(false)
  const [formError, setFormError] = useState<string>()
  /**
   * Who the next post is put to (0.371), by teammate id: empty is everyone.
   * Set by Reply on an answer or an @ in the box, shown as tiles in the box,
   * and cleared once the post is made -- naming someone is per post, as it
   * is in every chat that has it.
   */
  const [askTo, setAskTo] = useState<readonly string[]>([])
  const box = useRef<HTMLTextAreaElement>(null)
  /** Where the caret is in the box, so an @ is read where it is being typed. */
  const [caret, setCaret] = useState(0)
  /** The @ whose menu was put away with Escape, by where it starts. */
  const [mentionOff, setMentionOff] = useState<number>()
  /** Which member the arrow keys are on in the @ menu. */
  const [mentionIndex, setMentionIndex] = useState(0)

  // A room is a chat, so it scrolls like one: following the newest answer
  // while the person is at the bottom, and offering the way back as soon as
  // they are not. It did neither until 2026-09-13 -- a post to eight
  // teammates grew under the reader and never moved.
  const follow = useFollowBottom()
  // Opening a different room starts at its newest post, not wherever the last
  // one was left -- and asks everyone in it until someone is named there.
  useEffect(() => {
    follow.jumpNow()
    setAskTo([])
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentRoomId])

  const members = room === undefined ? [] : room.teammateIds.map((id) => teammates.find((entry) => entry.teammateId === id))

  /**
   * Put one teammate's answer at the top of the room.
   *
   * The answer to "what did Booty actually say" was never a better preview
   * -- the fold and Open were already right. What was missing was a way to
   * GET to Booty among eight without scrolling and reading names.
   *
   * Arithmetic on `scrollTop` rather than `scrollIntoView`, which fights a
   * scroll container: it scrolls every ancestor that can scroll, so in a
   * pane inside a pane it moves the wrong one and the room jumps under the
   * reader.
   */
  const jumpTo = (postId: string, teammateId: string): void => {
    const card = document.querySelector(`[data-answer="${postId}:${teammateId}"]`)
    const scroller = card?.closest('.lc-screen__scroll')
    if (card === null || !(card instanceof HTMLElement) || !(scroller instanceof HTMLElement)) return
    // The sticky header sits over the top of the scroll area, so the card
    // has to clear it or it lands underneath the thing that sent you there.
    const header = scroller.querySelector('.lc-posthead')
    const clearance = header instanceof HTMLElement ? header.offsetHeight : 0
    scroller.scrollTo({ top: card.offsetTop - scroller.offsetTop - clearance, behavior: 'smooth' })
  }

  const create = async (): Promise<void> => {
    setFormError(undefined)
    setBusy(true)
    const error = await onCreateRoom(draftName, draftMembers)
    setBusy(false)
    if (error !== undefined) {
      setFormError(error)
      return
    }
    setDraftName('')
    setDraftMembers([])
  }

  /** The refusal, if any: an add keeps its words when it was refused (L24). */
  const move = async (request: RoomTaskRequest): Promise<string | undefined> => {
    setBoardError(undefined)
    const error = await onTask(request)
    if (error !== undefined) setBoardError(error)
    return error
  }

  const post = async (): Promise<void> => {
    if (room === undefined || draftText.trim().length === 0) return
    const sending = draftText
    setFormError(undefined)
    setBusy(true)
    /*
     * The box empties NOW, not when every member has been asked.
     *
     * Asking a room is one runtime start per member and each can be a cold
     * boot, so the response is seconds away -- and until it came, a person's
     * own sentence sat under their cursor with "Posting..." beneath it,
     * reading as a message that had not gone (Colin, 2026-09-11). The post is
     * on disk before the first member is asked, and the host now says so; the
     * only thing this has to get right is putting the words back if it turns
     * out they never landed.
     */
    setDraftText('')
    // Only members still in the room: someone removed since they were named
    // is not asked, and nobody left named is everyone.
    const naming = askTo.filter((id) => room.teammateIds.includes(id))
    const error = await onPost(room.roomId, sending, naming.length === 0 ? undefined : naming)
    setBusy(false)
    if (error !== undefined) {
      setFormError(error)
      // Theirs to try again with, in the box they typed it in, to the same people.
      setDraftText((current) => (current.length === 0 ? sending : current))
      return
    }
    setAskTo([])
  }

  /** Put the next post to one teammate, from their answer: the Reply on a card. */
  const replyTo = (teammateId: string): void => {
    setAskTo([teammateId])
    box.current?.focus()
  }
  /*
   * ROOMS, ROUND TWO (0.399; the rooms-and-peers research, item 7).
   *
   * Build on one answer, or have one teammate merge them all. Both only
   * write the next post for you -- nothing is sent until you send it --
   * because every member is already briefed with the room's last posts and
   * their finished answers (0.370): naming the answer is all a post needs.
   */
  const buildOn = (name: string): void => {
    setAskTo([])
    setDraftText(`Build on ${name}\u2019s answer above: `)
    box.current?.focus()
  }
  const mergeBy = (teammateId: string): void => {
    setAskTo([teammateId])
    setDraftText('Merge the answers to my last post into one: keep what they agree on, say plainly where they differ, and name whose each part was.')
    box.current?.focus()
  }
  // Answers in columns, for comparing them across (0.399). This room, this visit.
  const [sideBySide, setSideBySide] = useState(false)

  // The @ menu: the room's members, by what follows the @ at the caret.
  const roomMembers = members.filter((member): member is PublicTeammate => member !== undefined)
  const mention = mentionAt(draftText, caret)
  const mentionOpen = mention !== undefined && mention.start !== mentionOff
  const choices = mentionOpen ? mentionChoices(roomMembers, askTo, mention.query) : []
  const activeChoice = choices[Math.min(mentionIndex, choices.length - 1)]
  const askedNames = askTo.map((id) => roomMembers.find((member) => member.teammateId === id)?.name ?? id)

  /** Name a member from the @ menu: the tile goes in, the @-word comes out. */
  const pick = (member: PublicTeammate): void => {
    if (mention === undefined) return
    const next = withoutMention(draftText, mention)
    setAskTo((current) => (current.includes(member.teammateId) ? current : [...current, member.teammateId]))
    setDraftText(next.text)
    setCaret(next.caret)
    setMentionIndex(0)
    // The caret goes back where the @ was, once React has put the text in.
    requestAnimationFrame(() => {
      box.current?.focus()
      box.current?.setSelectionRange(next.caret, next.caret)
    })
  }

  /** What was typed, and "@Wren " becoming its tile without the menu. */
  const typed = (text: string, at: number): void => {
    // A menu put away with Escape stays away for that @ only.
    if (mentionAt(text, at) === undefined) setMentionOff(undefined)
    setMentionIndex(0)
    const done = mentionCompleted(roomMembers, askTo, text, at)
    if (done === undefined) {
      setDraftText(text)
      setCaret(at)
      return
    }
    const next = withoutMention(text.slice(0, at - 1) + text.slice(at), done.mention)
    setAskTo((current) => [...current, done.member.teammateId])
    setDraftText(next.text)
    setCaret(next.caret)
    setMentionIndex(0)
    requestAnimationFrame(() => box.current?.setSelectionRange(next.caret, next.caret))
  }

  /** Keys in the room's box: the @ menu's first, then a tile's, then sending. */
  const keyDown = (event: KeyboardEvent<HTMLTextAreaElement>): void => {
    // Every key belongs to an input method while it is composing: Enter there
    // confirms a word and must not post it (the main box's rule, code review B4).
    if (event.nativeEvent.isComposing || event.keyCode === 229) return
    if (choices.length > 0) {
      const at = Math.min(mentionIndex, choices.length - 1)
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        setMentionIndex((at + (event.key === 'ArrowDown' ? 1 : choices.length - 1)) % choices.length)
        return
      }
      // Enter names the member highlighted: it must never post "@Wr".
      if ((event.key === 'Enter' && !event.shiftKey) || event.key === 'Tab') {
        event.preventDefault()
        if (activeChoice !== undefined) pick(activeChoice)
        return
      }
      if (event.key === 'Escape') {
        event.preventDefault()
        setMentionOff(mention?.start)
        return
      }
    }
    // Backspace at the very start takes the last name off, as a list of
    // names in any chat box does.
    const field = event.currentTarget
    if (event.key === 'Backspace' && askTo.length > 0 && field.selectionStart === 0 && field.selectionEnd === 0) {
      event.preventDefault()
      setAskTo((current) => current.slice(0, -1))
      return
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      void post()
    }
  }

  if (room === undefined) {
    return (
      <div className="lc-screen">
        <div className="lc-screen__header">
          <span className="lc-screen__title">Rooms</span>
          <span className="lc-screen__meta lc-mono">
            {rooms.length === 0 ? 'none yet' : `${String(rooms.length)} room${rooms.length === 1 ? '' : 's'}`} · one post, every teammate answers
          </span>
        </div>
        <div className="lc-screen__scroll">
          {rooms.length > 0 && (
            <section className="lc-settings__section">
              <h2 className="lc-settings__heading lc-settings__heading--section lc-mono">Your rooms</h2>
              <div className="lc-roomlist">
                {rooms.map((entry) => (
                  <button key={entry.roomId} type="button" className="lc-roomcard" onClick={() => onSelectRoom(entry.roomId)}>
                    <span className="lc-roomcard__name">{entry.name}</span>
                    <span className="lc-roomcard__meta lc-mono">{roomCardMeta(entry, teammates)}</span>
                  </button>
                ))}
              </div>
            </section>
          )}
          <section className="lc-settings__section">
            {/* A section label, not a second screen title. `.lc-screen__title`
                and `.lc-settings__heading` both set --lc-text-screen-title at
                weight 500, so "Rooms" and "New room" sat one above the other
                at equal rank with neither subordinate. The screen is Rooms;
                this names a block on it. */}
            <h2 className="lc-settings__heading lc-settings__heading--section lc-mono">New room</h2>
            {teammates.length === 0 ? (
              <p className="lc-settings__note">Make a teammate first; a room is a set of them.</p>
            ) : (
              /*
                * THE CARD, and the rule behind it.
                *
                * "A screen's content lives in a card; the pane is not a
                * surface" -- the roster, the runtime panel, the activity
                * fold, the receipt and the sidebar rows all obey it. This
                * form did not: the field, the chips and the button sat
                * straight on the pane with nothing binding them, so the
                * button read as unrelated to the field it belongs to.
                * MEASURED before the change: transparent background, no
                * border, no padding.
                *
                * Three bands, because they answer three different
                * questions: what a room IS, what this one will be, and the
                * one thing to press. The action band sits on a lighter
                * ground so the press is visibly the end of the form rather
                * than another field.
                */
              <form
                className="lc-roomform lc-roomcardform"
                onSubmit={(event) => {
                  event.preventDefault()
                  void create()
                }}
              >
                <p className="lc-roomcardform__about">
                  Name it and pick who is in it. A post goes to everyone in the room at once, each on their own
                  AI agent and model; their answers land here, and each one opens the conversation it came from.
                </p>
                <div className="lc-roomcardform__fields">
                {/*
                  * LABELS, in the machine voice the rest of the app uses for
                  * them. A placeholder is not a label: it disappears the
                  * moment a person types, so the field stops saying what it
                  * is exactly when there is something in it to misread. The
                  * placeholder stays as an example of a name rather than as
                  * the name of the field.
                  */}
                <label className="lc-roomcardform__label lc-mono" htmlFor="lc-room-name">
                  Room name
                </label>
                <input
                  id="lc-room-name"
                  className="lc-roomform__name"
                  value={draftName}
                  onChange={(event) => setDraftName(event.target.value)}
                  placeholder="Release, standup, research…"
                  aria-label="Room name"
                  maxLength={60}
                />
                <div className="lc-roomcardform__label lc-mono">Who is in it</div>
                <div className="lc-roomform__members" role="group" aria-label="Teammates in the room">
                  {teammates.map((teammate) => {
                    const on = draftMembers.includes(teammate.teammateId)
                    return (
                      <button
                        key={teammate.teammateId}
                        type="button"
                        role="checkbox"
                        aria-checked={on}
                        className={`lc-button${on ? ' is-active' : ''}`}
                        onClick={() =>
                          setDraftMembers((current) =>
                            on ? current.filter((id) => id !== teammate.teammateId) : [...current, teammate.teammateId]
                          )
                        }
                      >
                        <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={BOT_SIZE.pickerMark} activity="idle" presence="none" />
                        {teammate.name}
                        {/* A tick as well as the lime. `.lc-button.is-active`
                            is border, tint and text colour -- all three are
                            colour, and a chip that says it is picked ONLY by
                            being greener is a state a colour-blind reader
                            has to infer from its neighbours. */}
                        {on && <Icon name="check" size={13} />}
                      </button>
                    )
                  })}
                </div>
                {/* Said HERE, while the room is being built, because this
                  * is the only screen where the number can still be changed. */}
                {roomBlockedReason(draftName, draftMembers.length) !== undefined && (
                  <span
                    className={`lc-settings__note${
                      roomFullNote(draftMembers.length) === undefined ? '' : ' lc-tone-amber'
                    }`}
                  >
                    {roomBlockedReason(draftName, draftMembers.length)}
                  </span>
                )}
                </div>
                <div className="lc-roomform__actions lc-roomcardform__action">
                  {/* What is about to happen, beside the button that does
                      it: how many are in, and the fact that each answers on
                      its own route -- which is the thing about a room that
                      surprises people. */}
                  <span className="lc-roomcardform__summary">
                    {`${String(draftMembers.length)} of ${String(teammates.length)} teammate${teammates.length === 1 ? '' : 's'}`}
                    {' · each answers on its own model'}
                  </span>
                  <button
                    type="submit"
                    className="lc-button is-active"
                    // Not offered while it would be refused: the store turns
                    // a ninth teammate away, and finding that out by pressing
                    // the button is how it used to go.
                    disabled={
                      busy ||
                      draftName.trim().length === 0 ||
                      draftMembers.length === 0 ||
                      roomFullNote(draftMembers.length) !== undefined
                    }
                  >
                    Create room
                  </button>
                  {formError !== undefined && <span className="lc-settings__note lc-tone-red">{formError}</span>}
                </div>
              </form>
            )}
            {/*
              * Where the thing you are about to make will appear.
              *
              * Only when there are none: a person with rooms already knows
              * where they live, and repeating it under the form would be
              * telling them something they can see two inches away.
              */}
            {rooms.length === 0 && teammates.length > 0 && (
              <p className="lc-settings__note lc-roomcardform__after">
                No rooms yet. The first one you make appears in the sidebar under its own name.
              </p>
            )}
          </section>
        </div>
      </div>
    )
  }

  return (
    <div className="lc-screen lc-room">
      {/*
        * Back, then the room, then the one destructive thing.
        *
        * These three used to be three peers on one row with one gap between
        * them, so a boxed button, a title and a mono list of names read as a
        * single clump of text in the top-left corner -- Colin, 2026-09-11:
        * "the rooms button and text clump at the top is brutal." They are not
        * peers. The room's name is the heading and its members belong TO it,
        * a line under it, the way a mission's route sits under its title;
        * leaving is navigation and sits apart from both.
        */}
      <div className="lc-screen__header lc-room__header">
        <div className="lc-workroom__identity lc-room__identity">
          {/*
            * The name, which is also where it is changed.
            *
            * A room made from an ask starts as `Untitled room` on purpose --
            * charging a name before a room has a purpose is most of why
            * nobody made one (design agent, 2026-09-10) -- so the composer
            * promises "you can rename it there", and this is there. Click it.
            */}
          <div style={{ minWidth: 0 }}>
            {renaming ? (
              <input
                className="lc-input lc-room__rename"
                defaultValue={room.name}
                maxLength={60}
                aria-label="Room name"
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === 'Escape') setRenaming(false)
                  if (event.key === 'Enter') {
                    const next = event.currentTarget.value.trim()
                    setRenaming(false)
                    if (next.length > 0 && next !== room.name) onRenameRoom?.(room.roomId, next)
                  }
                }}
                onBlur={(event) => {
                  const next = event.currentTarget.value.trim()
                  setRenaming(false)
                  if (next.length > 0 && next !== room.name) onRenameRoom?.(room.roomId, next)
                }}
              />
            ) : (
              <button
                type="button"
                className="lc-workroom__name lc-room__name"
                onClick={() => setRenaming(true)}
                title="Rename this room"
                disabled={onRenameRoom === undefined}
              >
                {room.name}
              </button>
            )}
            <div className="lc-workroom__role lc-room__members">
              {members.filter((entry) => entry !== undefined).map((entry) => entry!.name).join(' · ')}
            </div>
          </div>
        </div>
        {/*
          * The same header a conversation has: who this is on the left, the
          * occasional actions behind one control on the right.
          *
          * It was a boxed `Rooms` button, the title, a mono list of names and
          * a boxed `Remove room` -- four things competing across the top, and
          * the one in red text was the destructive one. Colin, 2026-09-14:
          * "wtf is all this just make it like our regular teammates chat,
          * include the triple dot dropdown and activity if needed".
          *
          * Going back is in the menu rather than beside the title for the
          * reason the conversation header has no back button either: the
          * sidebar is how you move between things, and a room is not deeper
          * than a conversation. Activity is left out because "if needed" --
          * there is no inspector panel for a room to toggle.
          */}
        <div className="lc-workroom__actions">
          <button
            type="button"
            className={`lc-button${sideBySide ? ' is-active' : ''}`}
            aria-pressed={sideBySide}
            aria-label="Answers side by side"
            title={sideBySide ? 'Answers one under another' : 'Answers side by side, to compare them'}
            onClick={() => setSideBySide((current) => !current)}
          >
            <Icon name="diff" size={13} />
          </button>
          <button
            type="button"
            className="lc-button"
            aria-label="More actions"
            aria-haspopup="menu"
            title="More actions"
            onClick={(event) => {
              const at = event.currentTarget.getBoundingClientRect()
              onMenu?.({
                x: Math.round(Math.max(8, at.right - 220)),
                y: Math.round(at.bottom + 4),
                title: 'This room',
                items: [
                  { label: 'All rooms', onSelect: () => onSelectRoom(undefined) },
                  ...(onRenameRoom === undefined
                    ? []
                    : [{ label: 'Rename this room', onSelect: () => setRenaming(true) }]),
                  {
                    label: 'Remove this room',
                    // The same second press the header button asked for.
                    // Nothing about the asking changed by moving it here.
                    confirmLabel: 'Remove for good?',
                    danger: true,
                    onSelect: () => onRemoveRoom(room.roomId)
                  }
                ]
              })
            }}
          >
            <Icon name="dots" size={13} />
          </button>
        </div>
      </div>
      <div className="lc-screen__scroll lc-room__thread" ref={follow.ref} onScroll={follow.onScroll}>
        {/*
          * The board. A task is a line of text, an owner, a state, and the
          * mission that last touched it. Teammates move it with a block at
          * the end of a reply; this is where a person moves it by hand.
          */}
        {room.posts.length === 0 && (
          <p className="lc-settings__note">Nothing posted yet. Whatever you write below goes to everyone in the room, or type @ to ask one teammate.</p>
        )}
        {room.posts.map((entry) => {
          const answers = answersFor(room, entry.postId)
          const exchange = exchangeFor?.(room, entry.postId)
          const waiting = waitingLine(
            (entry.queued ?? []).map((id) => teammates.find((candidate) => candidate.teammateId === id)?.name ?? id)
          )
          // Only the members it was put to can be missing from it: a post to
          // Wren does not report Pip as "not asked" -- the person chose that.
          const absent = absentLine(
            (entry.to ?? room.teammateIds)
              .filter((id) => room.teammateIds.includes(id) || entry.missions[id] !== undefined)
              .filter((id) => answers.every((candidate) => candidate.teammateId !== id) && !(entry.queued ?? []).includes(id))
              .map((id) => ({
                name: teammates.find((candidate) => candidate.teammateId === id)?.name ?? id,
                ...(entry.refused?.[id] === undefined ? {} : { reason: entry.refused[id] })
              }))
          )
          return (
            <section key={entry.postId} className="lc-roompost">
              <div className="lc-roompost__you">
                <span className="lc-roompost__text">{entry.text}</span>
                <span className="lc-roompost__at lc-mono">
                  {/* Who it was put to, when the person named someone. */}
                  {entry.to !== undefined && entry.to.length > 0 && (
                    <span className="lc-roompost__to">
                      To {namesSaid(entry.to.map((id) => teammates.find((candidate) => candidate.teammateId === id)?.name ?? id))} ·{' '}
                    </span>
                  )}
                  {clock(new Date(entry.at))}
                </span>
              </div>
              {/*
                * One sticky line per post: the glance, and the way to one
                * answer. Chrome on the section, the same relationship the
                * workroom header has to its thread -- no card, no register,
                * and nothing in it that is not already in the records the
                * cards below are read from.
                */}
              <div className="lc-posthead">
                <span className="lc-posthead__counts lc-mono">
                  {postHeadline(
                    room.teammateIds.map((id) => {
                      const name = teammates.find((candidate) => candidate.teammateId === id)?.name ?? id
                      if ((entry.queued ?? []).includes(id)) return { name, state: 'waiting' as const }
                      const found = answers.find((candidate) => candidate.teammateId === id)
                      // No mission at all is NOT a failure -- nothing ran to
                      // fail. Seen in a screenshot 2026-09-13: the headline
                      // said "Booty failed" directly above the line saying
                      // "Booty was not asked.", which is one fact answered two
                      // ways. `absentLine` owns this case and says why; the
                      // headline counts only the people who were actually
                      // asked.
                      if (found === undefined) return { name, state: 'absent' as const }
                      if (found.phase === 'failed' || found.phase === 'cancelled') return { name, state: 'failed' as const }
                      // Having SPOKEN is not having FINISHED. Astra measured
                      // "2 asked · all answered" over two cards that both
                      // still said running: the text had arrived and the
                      // missions had not ended. A person reads "all answered"
                      // as "nothing left to wait for", which was false.
                      if (found.phase === 'completed') return { name, state: 'answered' as const }
                      if (found.text !== undefined) return { name, state: 'replied' as const }
                      return { name, state: 'running' as const }
                    })
                  )}
                </span>
                {/*
                  * The members as an index. The strip IS the roster, so a
                  * waiting member is here as a hollow pip rather than absent
                  * -- a name missing from it reads as someone not in the room.
                  */}
                <span className="lc-posthead__index" role="group" aria-label="Jump to an answer">
                  {room.teammateIds.map((id) => {
                    const teammate = teammates.find((candidate) => candidate.teammateId === id)
                    const found = answers.find((candidate) => candidate.teammateId === id)
                    const waiting = (entry.queued ?? []).includes(id)
                    const phase = waiting
                      ? 'waiting'
                      : // No mission is not a failure -- nothing ran to fail --
                        // the headline's own rule. A post put to Wren alone
                        // painted Pip's face red in its head (0.371).
                        found === undefined
                        ? 'not-asked'
                        : found.phase === 'failed' || found.phase === 'cancelled'
                          ? 'failed'
                          : // Terminal, same as the headline above -- a pip that
                            // said "answered" while the line said "still
                            // working" would be the same fact told two ways.
                            found.phase === 'completed'
                            ? 'answered'
                            : 'running'
                    const said = phase === 'not-asked' ? 'not asked' : phase
                    return (
                      <button
                        key={id}
                        type="button"
                        className={`lc-posthead__face is-${phase}`}
                        title={`${teammate?.name ?? id} · ${said}`}
                        aria-label={`${teammate?.name ?? id}, ${said}`}
                        onClick={() => jumpTo(entry.postId, id)}
                      >
                        {teammate !== undefined && (
                          <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={BOT_SIZE.postFace} activity="idle" presence="none" />
                        )}
                        <span className="lc-posthead__pip" aria-hidden="true" />
                      </button>
                    )
                  })}
                </span>
              </div>
              {exchange !== undefined ? (
                /*
                  * A post that became an argument is a SEQUENCE, first answers
                  * included -- not a grid with the rest tucked somewhere.
                  *
                  * A grid is a claim: these arrived in parallel and none is a
                  * reply to another. False the instant message three answers
                  * message two, and the reader then has to merge two shapes in
                  * their head to recover one conversation. And not a fold: a
                  * fold means work summarised, this is what they SAID, and
                  * Said does not collapse (design agent, 2026-09-11).
                  */
                <div className="lc-roomsaid">
                  {exchange.items.map((item) => {
                    const who = teammates.find((candidate) => candidate.teammateId === item.teammateId)
                    /*
                     * The face is who said it; its title is the name. A room
                     * used to print the name above every speaker's first
                     * line as well. Colin, 2026-09-22, on the conversation
                     * view that never did: "the name next to it isnt needed"
                     * -- the room's member row and the face's own title say it.
                     */
                    const face =
                      who === undefined ? null : (
                        <TeammateBot hue={who.hue} avatar={who.avatar} size={BOT_SIZE.roomSaid} activity="idle" presence="none" name={who.name} />
                      )
                    if (item.kind === 'said') {
                      return (
                        <div
                          key={item.key}
                          className={`lc-roomsaid__turn${item.startsSpeaker ? '' : ' is-continued'}`}
                          data-said={`${entry.postId}:${item.missionId}`}
                        >
                          <span className="lc-roomsaid__gutter">{item.startsSpeaker ? face : null}</span>
                          <div className="lc-roomsaid__body">
                            {/* The one case where a time is load-bearing:
                                this arrived after a NEWER post exists, so
                                without it the room looks like it inserted
                                a message into the past. */}
                            {item.startsSpeaker && item.showTime && item.at !== undefined && (
                              <span className="lc-roomsaid__who">
                                <span className="lc-roomsaid__at lc-mono">
                                  {clock(new Date(item.at))}
                                </span>
                              </span>
                            )}
                            <RoomAnswerText text={item.text} />
                          </div>
                        </div>
                      )
                    }
                    /*
                      * A turn happening right now.
                      *
                      * The drawing's `Wren  writing now / Wren is writing...`
                      * -- a live turn, in its place in the argument, with the
                      * face beside it like any other. This is the line that
                      * keeps a room from reading as finished while the host is
                      * still booting a runtime.
                      */
                    if (item.kind === 'replying') {
                      /*
                        * The thread's live line, in the room.
                        *
                        * It used to be a flat `Gem is replying…` beside a
                        * static dot, while the thread two clicks away animated
                        * and named the tool it was on. Colin, 2026-09-11:
                        * "lets make this behave more like our actual chat,
                        * where the animated ...'s appear and all the calls."
                        * Same component, so they cannot drift again.
                        *
                        * No run yet means `starting` -- the host has decided
                        * on this reply and the process does not exist, which
                        * is a true thing to say and the only one available.
                        */
                      return (
                        <div
                          key={item.key}
                          className="lc-roomsaid__turn is-live"
                          data-replying={item.teammateId}
                          data-register={item.live?.register ?? 'starting'}
                        >
                          <span className="lc-roomsaid__gutter">{face}</span>
                          <div className="lc-roomsaid__body">
                            <span className="lc-roomsaid__live">
                              <LiveRegisterLine
                                register={item.live?.register ?? 'starting'}
                                {...(item.live?.label === undefined ? {} : { label: item.live.label })}
                                {...(item.live?.detail === undefined ? {} : { detail: item.live.detail })}
                                startedAt={item.live?.startedAt ?? entry.at}
                                thinking={item.live?.thinking ?? true}
                              />
                            </span>
                          </div>
                        </div>
                      )
                    }
                    // Absence, drawn where it happened. Drop it and the reader
                    // watches the argument stop and blames the budget.
                    return (
                      <div key={item.key} className="lc-roomsaid__turn">
                        <span className="lc-roomsaid__gutter">{face}</span>
                        <p className="lc-roomsaid__absent">
                          {item.kind === 'silent'
                            ? `${item.name}\u2019s turn ended without a reply \u2014 the runtime finished and wrote nothing back. Nothing was changed.`
                            : `${item.name} is finishing another mission. Their reply is queued.`}
                        </p>
                      </div>
                    )
                  })}
                  {/* The budget, the cost and the ending, in one line, under
                      the thing they describe. Never amber: a rule working as
                      intended is not an alert. */}
                  <p className="lc-roomsaid__foot lc-mono">
                    {footLine(exchange.foot, exchangeCostText?.(room, entry.postId))}
                  </p>
                </div>
              ) : (
              <div className={`lc-roompost__answers${answers.length > ANSWERS_BEFORE_A_LIST ? ' is-list' : ''}${sideBySide && answers.length > 1 ? ' is-columns' : ''}`}>
                {/*
                  * WHOEVER REPLIED FIRST, first.
                  *
                  * The roster order is the order they were added to the room,
                  * which is not a fact about this post. Colin, 2026-09-12: "we
                  * can just have whoever replies first, appear in the chat".
                  * A teammate with no first event yet sorts last, because
                  * they have not spoken.
                  */}
                {[...room.teammateIds]
                  .sort((left, right) => {
                    const at = (id: string): string =>
                      answers.find((candidate) => candidate.teammateId === id)?.startedAt ?? '~'
                    return at(left) < at(right) ? -1 : at(left) > at(right) ? 1 : 0
                  })
                  .map((teammateId) => {
                  const teammate = teammates.find((candidate) => candidate.teammateId === teammateId)
                  const answer = answers.find((candidate) => candidate.teammateId === teammateId)
                  const name = teammate?.name ?? teammateId
                  // No mission, nothing said. They are named together underneath.
                  if (answer === undefined) return null
                  return (
                    <div key={teammateId} className="lc-roomanswer" data-answer={`${entry.postId}:${teammateId}`}>
                      <div className="lc-roomanswer__who">
                        {teammate !== undefined && (
                          <TeammateBot
                            hue={teammate.hue}
                            avatar={teammate.avatar}
                            size={BOT_SIZE.roomAnswer}
                            activity={answer.phase === 'running' || answer.phase === 'starting' ? 'thinking' : 'idle'}
                            presence={answer.phase === 'running' || answer.phase === 'starting' ? 'working' : 'none'}
                            name={name}
                          />
                        )}
                        {/* No name beside the face: it is the face's title (Colin, 2026-09-22). */}
                        {teammate === undefined && <span className="lc-roomanswer__name">{name}</span>}
                        <span className="lc-roomanswer__route lc-mono">
                          {/* The composer's spelling, not the catalog's full
                              display name and a raw model id. One route, one
                              name, on every surface (Grok's finding 1). */}
                          {routeChrome(
                            answer.runtime as MissionRuntimeId,
                            answer.model,
                            answer.model === 'account-default' ? 'default' : modelDisplayName(answer.runtime, answer.model)
                          )}
                        </span>
                        <span className="lc-roomanswer__sep lc-mono" aria-hidden="true">·</span>
                        {answer.phase === 'failed' ? (
                          <span className="lc-roomanswer__phase lc-mono lc-tone-red">{answer.phase}</span>
                        ) : (
                          <AnswerState phase={answer.phase} startedAt={answer.startedAt} />
                        )}
                        {/* What you can do with the answer, as one group: in columns
                            it takes its own line under the route (0.399). */}
                        <span className="lc-roomanswer__acts">
                        {/* The next post to this teammate only (0.371). */}
                        <button
                          type="button"
                          className="lc-ghostbutton"
                          title={`Put your next post to ${name} only`}
                          aria-label={`Reply to ${name}`}
                          onClick={() => replyTo(teammateId)}
                        >
                          Reply
                        </button>
                        {answer.phase === 'completed' && (
                          <button type="button" className="lc-ghostbutton" title={`Your next post, to everyone, building on ${name}\u2019s answer`} onClick={() => buildOn(name)}>
                            Build on
                          </button>
                        )}
                        {answer.phase === 'completed' && answers.filter((other) => other.phase === 'completed').length > 1 && (
                          <button type="button" className="lc-ghostbutton" title={`Your next post, to ${name} only: merge the answers into one`} onClick={() => mergeBy(teammateId)}>
                            Merge
                          </button>
                        )}
                        <button type="button" className="lc-ghostbutton" onClick={() => onOpenMission(answer.missionId)}>
                          Open
                        </button>
                        </span>
                      </div>
                      {/*
                        * The thread's own renderer, not a summary of it.
                        *
                        * This was one paragraph carrying the teammate's LAST
                        * final message. Everything else a turn does -- the
                        * other messages, the thinking, the tool calls, the
                        * plan and how far through it is -- existed and was
                        * simply not drawn here, which is what Colin saw
                        * "missing" and what the loading flash showed for a
                        * moment before the card settled on one message.
                        *
                        * `ThreadItems` is the same component the conversation
                        * uses, and it was already extracted to be rendered
                        * more than once. A room renders one per teammate.
                        */}
                      <ThreadItems
                        items={answer.items}
                        owner={teammate}
                        faces={false}
                        activity={answer.phase === 'running' || answer.phase === 'starting' ? 'thinking' : 'idle'}
                        workspacePath={workspacePath}
                        decision={undefined}
                      />
                    </div>
                  )
                })}
              </div>
              )}
              {waiting !== undefined && (
                <p className="lc-roomwaiting">
                  <span className="lc-roomwaiting__label lc-mono">Waiting for a slot</span>
                  <span className="lc-roomwaiting__names">{waiting}</span>
                  <span className="lc-roomwaiting__count lc-mono">{(entry.queued ?? []).length}</span>
                </p>
              )}
              {absent !== undefined && <p className="lc-roomabsent">{absent}</p>}
            </section>
          )
        })}
        {/*
          * The board sits with the newest messages, not above everything.
          *
          * Colin, 2026-09-14: "it should actually match how the design agent
          * set it up and appear where the user can actually see it?" It was
          * pinned at the top of the room, which means that in any room with a
          * conversation in it the board is scrolled off screen exactly when
          * somebody would act on it -- and the design's plan is drawn INLINE,
          * where the reading is, never as a strip above the page.
          *
          * At the end it is what you are looking at when you are caught up,
          * which is the moment a task list is worth anything. The room
          * follows its newest line (0.96.0), so this is on screen on open.
          */}
        <section className="lc-board" aria-label="Task board">
          {/*
            * The PLAN's header, not a section of its own.
            *
            * Colin, 2026-09-13: "I just really enjoyed that UI over the task
            * bar we setup. Would there be a way to have at least replace
            * that?" The rows already borrowed the plan's shape; what still
            * read as a bar was everything around them -- a bordered raised
            * card with a titled head, pinned above every room, drawn even
            * when the board was empty.
            *
            * A plan draws nothing when it has no steps, so neither does this:
            * an empty board is one quiet line to add the first task, and the
            * head arrives with the first one.
            */}
          {room.tasks.length > 0 && (
            <div className="lc-rail__meta lc-mono lc-board__head">
              TASKS · {room.tasks.filter((task) => task.state === 'done').length} of {room.tasks.length} done
            </div>
          )}
          {room.tasks.map((task) => {
            const owner = teammates.find((entry) => entry.teammateId === task.ownerId)
            return (
              /*
               * THE PLAN'S SHAPE, because it is the same kind of thing.
               *
               * Every row carried a state TAG, the text, an owner with a face
               * and a name, an Open button and three ghost buttons -- six
               * competing elements per line, all at full strength, for a list
               * of two. Colin, 2026-09-13: "that task bar at the top is a
               * disaster lets just scrap that for this plan ui asset."
               *
               * A plan step is a marker, the words, and one quiet note on the
               * right. The state is the MARKER now -- a tick when it is done,
               * a filled dot while someone has it, an empty one otherwise --
               * so the tag goes, the text carries the done state itself, and
               * the owner becomes the quiet note.
               */
              <div key={task.taskId} className={`lc-task lc-plan__step is-${task.state}`}>
                <span className="lc-plan__marker" aria-hidden="true">
                  {task.state === 'done' ? <Icon name="check" size={11} /> : <span className="lc-dot" />}
                </span>
                {/* Said for a reader who cannot see the marker. */}
                <span className="lc-sr">{task.state === 'in-hand' ? 'in hand' : task.state}</span>
                <span className="lc-task__text">{task.text}</span>
                <span className="lc-task__owner">
                  {owner === undefined ? (
                    <span className="lc-settings__note">unassigned</span>
                  ) : (
                    <>
                      <TeammateBot hue={owner.hue} avatar={owner.avatar} size={BOT_SIZE.pickerMark} activity="idle" presence="none" />
                      {owner.name}
                    </>
                  )}
                </span>
                <span className="lc-task__actions">
                  {/*
                    * ONE control, not four.
                    *
                    * Open, Assign, Done/Reopen and Remove were four boxed
                    * buttons on every row, so a five-task board drew twenty
                    * of them and the tasks themselves were the quietest thing
                    * in the list. Colin, 2026-09-14: "make all these a
                    * dropdown, its clutter and i want it to be mostly
                    * automatic anyway."
                    *
                    * The second half of that is the real point and it is why
                    * this is the right shape rather than a smaller version of
                    * the same thing: teammates move this board themselves,
                    * with `locust-task` blocks, and 0.116.0 tells them to
                    * claim a row the moment they start it. These controls are
                    * the manual override for when that goes wrong -- and an
                    * override should be reachable, not resident.
                    *
                    * Same menu the room header uses, so there is one way to
                    * ask this app for the occasional actions.
                    */}
                  <button
                    type="button"
                    className="lc-ghostbutton lc-task__more"
                    aria-label={`Actions for ${task.text}`}
                    aria-haspopup="menu"
                    title="Actions"
                    onClick={(event) => {
                      const at = event.currentTarget.getBoundingClientRect()
                      onMenu?.({
                        x: Math.round(Math.max(8, at.right - 220)),
                        y: Math.round(at.bottom + 4),
                        title: task.text,
                        items: [
                          ...(task.missionId === undefined
                            ? []
                            : [
                                {
                                  label: 'Open the conversation',
                                  onSelect: () => onOpenMission(task.missionId!)
                                }
                              ]),
                          ...(task.state === 'done'
                            ? [
                                {
                                  label: 'Reopen',
                                  onSelect: () => void move({ roomId: room.roomId, op: 'reopen', taskId: task.taskId })
                                }
                              ]
                            : [
                                {
                                  label: 'Mark done',
                                  onSelect: () => void move({ roomId: room.roomId, op: 'done', taskId: task.taskId })
                                }
                              ]),
                          // Assigning names people, so each one is its own
                          // row rather than a second menu to open.
                          ...members
                            .filter((entry) => entry !== undefined)
                            .filter((entry) => entry!.teammateId !== task.ownerId)
                            .map((entry) => ({
                              label: `Assign to ${entry!.name}`,
                              onSelect: () =>
                                void move({
                                  roomId: room.roomId,
                                  op: 'assign',
                                  taskId: task.taskId,
                                  ownerId: entry!.teammateId
                                })
                            })),
                          ...(task.ownerId === undefined
                            ? []
                            : [
                                {
                                  label: 'Leave it unassigned',
                                  onSelect: () =>
                                    void move({ roomId: room.roomId, op: 'assign', taskId: task.taskId })
                                }
                              ]),
                          {
                            label: 'Take it off the board',
                            confirmLabel: 'Remove for good?',
                            danger: true,
                            onSelect: () => void move({ roomId: room.roomId, op: 'remove', taskId: task.taskId })
                          }
                        ]
                      })
                    }}
                  >
                    <Icon name="dots" size={13} />
                  </button>
                </span>
              </div>
            )
          })}
          {/*
            * Adding a task is a LINE, not a bar.
            *
            * Colin, 2026-09-14, on an empty room: "i thought we completely
            * removed the add a task function and replaced it with the plan
            * ui no? this is definitely not what we cooked up lmfao." He was
            * looking at a bordered full-width input sitting at the top of a
            * room with nothing in it -- the loudest thing on the screen, for
            * the least important thing there.
            *
            * The plan came first in 0.99.0 and this did not follow it. A
            * plan has no input in it; it has steps. So the field is gone
            * until it is asked for, and what is left is one quiet line at the
            * END of the list, where a new step belongs.
            */}
          {/* Outside the add form, so a refused move, done or assign is said too (L24). */}
          {boardError !== undefined && <p className="lc-settings__note lc-tone-red" role="status">{boardError}</p>}
          {adding ? (
            <form
              className="lc-board__add"
              onSubmit={(event) => {
                event.preventDefault()
                if (draftTask.trim().length === 0) return
                void move({ roomId: room.roomId, op: 'add', text: draftTask }).then((refused) => {
                  // L24 (the code review): cleared and closed whatever the
                  // answer, so a refused task was thrown away with its reason.
                  if (refused !== undefined) return
                  setDraftTask('')
                  setAdding(false)
                })
              }}
            >
              <input
                className="lc-roomform__name"
                value={draftTask}
                onChange={(event) => setDraftTask(event.target.value)}
                onKeyDown={(event) => {
                  // Escape puts it away without leaving a half-typed step.
                  if (event.key === 'Escape') {
                    setDraftTask('')
                    setAdding(false)
                  }
                }}
                placeholder="What needs doing?"
                aria-label="Add a task"
                maxLength={200}
                autoFocus
              />
              <button type="submit" className="lc-button" disabled={draftTask.trim().length === 0}>
                Add
              </button>
            </form>
          ) : (
            <button type="button" className="lc-board__addlink" onClick={() => setAdding(true)}>
              + Add a task
            </button>
          )}
        </section>
        <JumpToBottom shown={follow.away} onClick={follow.toBottom} />
      </div>
      {/*
        * The thread's composer, not a second design.
        *
        * It used to be a bordered textarea with a wide filled "Post" button on
        * a row of its own underneath -- Colin, 2026-09-11: "the post button is
        * a little wonky, lets just make it the same as our regular chat box."
        * It is literally the same box now: `lc-composer` carries the field and
        * the round send control, and the room only adds what is true of a room
        * -- the line saying where a post goes.
        */}
      <form
        className="lc-composer lc-roomcompose"
        onSubmit={(event) => {
          event.preventDefault()
          void post()
        }}
      >
        {/*
          * The room's members, for an @ -- in the menu the main box draws its
          * commands in, above the field and sharing its edge. A mouse press
          * keeps the caret in the box, so picking with the mouse types on.
          */}
        {choices.length > 0 && (
          <div className="lc-slash lc-mentions" role="listbox" aria-label={`Ask someone in ${room.name}`}>
            {choices.map((member, index) => (
              <button
                key={member.teammateId}
                type="button"
                role="option"
                aria-selected={member === activeChoice}
                className={`lc-slash__item${member === activeChoice ? ' is-active' : ''}`}
                onMouseEnter={() => setMentionIndex(index)}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => pick(member)}
              >
                <span className="lc-slash__name lc-mentions__who">
                  <TeammateBot hue={member.hue} avatar={member.avatar} size={BOT_SIZE.pickerMark} activity="idle" presence="none" />
                  {member.name}
                </span>
                <span className="lc-slash__detail">{roleLabelOf(member)}</span>
              </button>
            ))}
          </div>
        )}
        {/*
          * Who this post goes to, when it is not everyone: a tile per
          * teammate, inside the box, the way an attached file is -- where a
          * person looks to see what is being sent. Removing the last one asks
          * everyone again.
          */}
        {askTo.length > 0 && (
          <div className="lc-attached lc-askto" aria-label={`Asking ${namesSaid(askedNames)} only`}>
            <span className="lc-askto__label lc-mono">To</span>
            {askTo.map((id, index) => {
              const member = roomMembers.find((entry) => entry.teammateId === id)
              const name = askedNames[index] ?? id
              return (
                <button
                  key={id}
                  type="button"
                  className="lc-attached__tile lc-askto__tile"
                  title={askTo.length === 1 ? `Only ${name} is asked. Remove to ask everyone in ${room.name}.` : `Remove ${name}`}
                  aria-label={`Stop asking ${name}`}
                  onClick={() => {
                    setAskTo((current) => current.filter((entry) => entry !== id))
                    box.current?.focus()
                  }}
                >
                  {member !== undefined && <TeammateBot hue={member.hue} avatar={member.avatar} size={BOT_SIZE.tileMark} activity="idle" presence="none" />}
                  <span className="lc-attached__name">{name}</span>
                  <Icon name="close" size={11} />
                </button>
              )
            })}
          </div>
        )}
        <div className="lc-composer__box">
          <textarea
            ref={box}
            className="lc-roomcompose__box"
            value={draftText}
            onChange={(event) => typed(event.target.value, event.target.selectionStart ?? event.target.value.length)}
            onSelect={(event) => setCaret(event.currentTarget.selectionStart ?? 0)}
            placeholder={askTo.length === 0 ? `Post to ${room.name}…` : `Ask ${namesSaid(askedNames)}…`}
            aria-label={askTo.length === 0 ? `Post to ${room.name}` : `Ask ${namesSaid(askedNames)} in ${room.name}`}
            rows={1}
            onKeyDown={keyDown}
          />
          <button
            type="submit"
            className="send-button lc-send"
            disabled={busy || draftText.trim().length === 0}
            aria-label={askTo.length === 0 ? `Post to ${room.name}` : `Ask ${namesSaid(askedNames)}`}
            title={`${askTo.length === 0 ? `Post to ${room.name}` : `Ask ${namesSaid(askedNames)}`} — Shift+Enter for a new line`}
          >
            <Icon name="arrow-up" size={15} />
          </button>
        </div>
        {/*
          * Only when there is something to say.
          *
          * This line was never empty. It narrated the board back at you --
          * "Jimothy took on X. Jimothy finished X. Jimothy handed X to Yurt."
          * -- directly under a board already showing X as IN HAND beside
          * Jimothy's face, and when it had no news it fell back to a standing
          * sentence about where a post goes. Colin, 2026-09-11: "that texxt
          * under the chat box needs to go away."
          *
          * Both halves were the same mistake in different directions: the
          * narration says what another register already says, and the
          * fallback is true forever, which is what makes it furniture rather
          * than information. The room header already names who is in the room.
          *
          * What survives is what a person cannot find anywhere else: that a
          * post is in flight, and that one was REFUSED.
          */}
        {(busy || notice !== undefined || formError !== undefined) && (
          <div className="lc-roomcompose__row">
            <span className="lc-settings__note">{busy ? 'Posting…' : notice ?? formError}</span>
          </div>
        )}
      </form>
    </div>
  )
}
