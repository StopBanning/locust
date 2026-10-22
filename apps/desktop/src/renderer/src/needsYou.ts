import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { parseDecision } from '../../shared/decision.js'
import { assistantMessages } from './missionView.js'

/**
 * NEEDS YOU (0.373): everything waiting on the person, in one place.
 *
 * A teammate that stops for an approval, or ends its turn by asking a
 * question, waits for the person -- and the person only learned of it by
 * opening that conversation, or from a desktop notification when the window
 * was behind another app. With two teammates working, a question asked in the
 * conversation not on screen sat unanswered while its asker did nothing.
 * Orca gathers every agent that needs attention in one feed, and Vibe Kanban
 * raises a hand on the card (docs/RESEARCH-2026-09-26-ROOMS-AND-PEERS.md):
 * this is Locust's version, one line in the title bar beside what is
 * running, shown only when something waits.
 *
 * In the order a person should answer them: a paused run first -- it is
 * holding a process open -- then a question a teammate stopped on, then a
 * Scout card that needs you now, then a routine whose copy is waiting for
 * Keep or Discard, then memory suggestions, which hold nothing up.
 *
 * The Scout card and the routine are not conversations. They already sit on
 * Home and on Routines; this list only counts them, and the title bar opens
 * those screens. A card that is `soon` or `later`, or one already dismissed,
 * is not here -- dismissed cards are gone from the list Scout holds.
 */

export type NeedsYouItem =
  | {
      readonly kind: 'approval'
      readonly key: string
      readonly missionId: string
      readonly teammateId: string | undefined
      readonly name: string
      /** What it wants to do, or what it asks, in one line. */
      readonly what: string
      /** A question the runtime put, rather than an action it wants to take. */
      readonly asking: boolean
      /** `what` is the exact command it wants to run. */
      readonly command: boolean
    }
  | {
      readonly kind: 'decision'
      readonly key: string
      readonly missionId: string
      readonly teammateId: string | undefined
      readonly name: string
      /** The question, in one line. */
      readonly what: string
    }
  | { readonly kind: 'memory'; readonly key: 'memory'; readonly count: number }
  | {
      readonly kind: 'scout'
      readonly key: string
      readonly cardId: string
      /** Scout's name, as the team knows it. */
      readonly name: string
      /** What Scout said, in one line. */
      readonly what: string
    }
  | {
      readonly kind: 'routine'
      readonly key: string
      readonly routineId: string
      /** The routine's name. */
      readonly name: string
      /** That it is waiting, and how many files, in one line. */
      readonly what: string
    }

/** A line in the list: long enough to recognise, short enough to be a menu row. */
export const NEEDS_YOU_TEXT = 90

function oneLine(text: string): string {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length <= NEEDS_YOU_TEXT ? flat : `${flat.slice(0, NEEDS_YOU_TEXT - 1).trimEnd()}…`
}

/**
 * The question a finished turn stopped on, or nothing.
 *
 * The same reading the thread's decision card makes (missionView.ts): the
 * LAST message the runtime marked final, once the run has completed.
 */
export function openQuestion(events: readonly NormalizedRuntimeEvent[]): string | undefined {
  const known = asked.get(events)
  if (known !== undefined) return known === null ? undefined : known
  const last = events.some((event) => event.type === 'run.completed') ? assistantMessages(events).filter((message) => message.final).at(-1) : undefined
  const question = last === undefined ? undefined : parseDecision(last.text)?.question
  asked.set(events, question ?? null)
  return question
}

/**
 * Read once per record. The list is rebuilt whenever a run streams, and a
 * finished turn's events are the same object from one read to the next (the
 * window keeps them by digest), so each is parsed the first time only.
 */
const asked = new WeakMap<readonly NormalizedRuntimeEvent[], string | null>()

export function needsYou(input: {
  /** Runs paused for the person, in the order they arrived. */
  readonly approvals: readonly { readonly approvalId: string; readonly runId: string; readonly missionId: string; readonly kind: string; readonly summary: string; readonly detail: string }[]
  readonly ownerOfRun: (runId: string) => string | undefined
  /**
   * One per conversation, each its newest turn (`collapseConversations`):
   * a question on an older turn was answered by the turn after it.
   */
  readonly conversations: readonly { readonly missionId: string; readonly phase: string }[]
  readonly ownerOfMission: (missionId: string) => string | undefined
  /** The turn's record, when the window holds it. */
  readonly eventsOf: (missionId: string) => readonly NormalizedRuntimeEvent[] | undefined
  readonly nameOf: (teammateId: string) => string | undefined
  /** Memory suggestions waiting on the Memory screen. */
  readonly memoryWaiting: number
  /**
   * Scout's cards, as the window already holds them. Only urgency `now`
   * counts; `soon` and `later` stay on Home. A dismissed card is not in
   * this list.
   */
  readonly scoutCards?: readonly { readonly cardId: string; readonly urgency: string; readonly said: string }[]
  /** Scout's own name. Absent: the row says Scout. */
  readonly scoutName?: string
  /**
   * Every routine. One with `files` set has a copy run waiting for Keep or
   * Discard (`files` is how many changed, and may be 0).
   */
  readonly routines?: readonly { readonly routineId: string; readonly name: string; readonly files?: number }[]
}): readonly NeedsYouItem[] {
  const who = (teammateId: string | undefined): string => (teammateId === undefined ? undefined : input.nameOf(teammateId)) ?? 'A teammate'
  const items: NeedsYouItem[] = []
  for (const request of input.approvals) {
    const teammateId = input.ownerOfRun(request.runId)
    /*
     * The command itself, where there is one: "Run a command" names nothing
     * a person can decide on (drive-needs-you, 0.373). A desktop notification
     * never shows it -- a lock screen is no place for a secret -- but this
     * list is inside the app, where the card it opens shows it anyway.
     */
    const command = request.kind === 'command' && request.detail.trim().length > 0
    items.push({
      kind: 'approval',
      key: `approval:${request.approvalId}`,
      missionId: request.missionId,
      teammateId,
      name: who(teammateId),
      what: oneLine(command ? request.detail : request.summary),
      asking: request.kind === 'question',
      command
    })
  }
  for (const conversation of input.conversations) {
    if (conversation.phase !== 'completed') continue
    const events = input.eventsOf(conversation.missionId)
    const question = events === undefined ? undefined : openQuestion(events)
    if (question === undefined) continue
    const teammateId = input.ownerOfMission(conversation.missionId)
    items.push({ kind: 'decision', key: `decision:${conversation.missionId}`, missionId: conversation.missionId, teammateId, name: who(teammateId), what: oneLine(question) })
  }
  const scoutName = input.scoutName?.trim() || 'Scout'
  for (const card of input.scoutCards ?? []) {
    if (card.urgency !== 'now' || card.cardId.trim().length === 0) continue
    items.push({ kind: 'scout', key: `scout:${card.cardId}`, cardId: card.cardId, name: scoutName, what: oneLine(card.said) })
  }
  for (const routine of input.routines ?? []) {
    if (routine.files === undefined || routine.routineId.trim().length === 0) continue
    items.push({ kind: 'routine', key: `routine:${routine.routineId}`, routineId: routine.routineId, name: routine.name, what: routineWaitLine(routine.files) })
  }
  if (input.memoryWaiting > 0) items.push({ kind: 'memory', key: 'memory', count: input.memoryWaiting })
  return items
}

/** The one line under a routine that is waiting: the words its own card uses. */
function routineWaitLine(files: number): string {
  if (files <= 0) return 'Waiting for you'
  return `Waiting for you · ${String(files)} file${files === 1 ? '' : 's'} in its copy`
}

/** A paused run or a question. Scout and a waiting routine are not a teammate's face. */
export function isConversationWait(item: NeedsYouItem): item is Extract<NeedsYouItem, { kind: 'approval' | 'decision' }> {
  return item.kind === 'approval' || item.kind === 'decision'
}

/** Where the title bar opens this, which is where it is answered. */
export function needsYouPlace(item: NeedsYouItem):
  | { readonly to: 'memory' }
  | { readonly to: 'home'; readonly cardId: string }
  | { readonly to: 'routines'; readonly routineId: string }
  | { readonly to: 'conversation'; readonly missionId: string } {
  if (item.kind === 'memory') return { to: 'memory' }
  if (item.kind === 'scout') return { to: 'home', cardId: item.cardId }
  if (item.kind === 'routine') return { to: 'routines', routineId: item.routineId }
  return { to: 'conversation', missionId: item.missionId }
}

/**
 * How many things each teammate waits on the person for: runs paused for an
 * approval AND questions it stopped on (0.564). The face counted approvals
 * only, so a teammate that ended its turn on a question looked idle while the
 * title bar said it needed you (Sol's 0.562 beta, act 3). A Scout card and a
 * routine waiting for Keep or Discard are counted in the title bar, not on a
 * face -- the teammate is not holding a run open for them.
 */
export function waitingByTeammate(items: readonly NeedsYouItem[]): Map<string, number> {
  const counts = new Map<string, number>()
  for (const item of items) {
    if (!isConversationWait(item) || item.teammateId === undefined) continue
    counts.set(item.teammateId, (counts.get(item.teammateId) ?? 0) + 1)
  }
  return counts
}

/** The item as a row of the menu it opens. */
export function needsYouLabel(item: NeedsYouItem): string {
  if (item.kind === 'memory') return `${String(item.count)} memory suggestion${item.count === 1 ? '' : 's'} waiting`
  if (item.kind === 'scout') return `${item.name} noticed: ${item.what}`
  if (item.kind === 'routine') return `${item.name}: ${item.what}`
  if (item.kind === 'decision') return `${item.name} asked: ${item.what}`
  if (item.asking) return `${item.name} is asking: ${item.what}`
  return item.command ? `${item.name} wants to run: ${item.what}` : `${item.name} needs your approval: ${item.what}`
}

/** The chip: how many things wait, as a sentence fragment. */
export function needsYouChip(count: number): string {
  return count === 1 ? '1 needs you' : `${String(count)} need you`
}
