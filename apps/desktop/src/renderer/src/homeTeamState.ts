import { faceLabel } from './faceState.js'
import type { NeedsYouItem } from './needsYou.js'
import type { TeammateStatusView } from './status.js'
import { agoLabel } from './teammateWork.js'

/**
 * What a waiting teammate's card says under the name (0.610): the question,
 * or the action it wants to take -- "Wants to run pnpm test" -- in the place
 * the role usually has. One line; the card cuts it with an ellipsis.
 */
export function waitingLine(item: NeedsYouItem): string | undefined {
  if (item.kind === 'decision') return item.what
  if (item.kind !== 'approval') return undefined
  if (item.asking) return item.what
  return item.command ? `Wants to run ${item.what}` : `Wants your approval: ${item.what}`
}

/** What a team card says beside the name, in which colour, and how it is read aloud. */
export interface TeamCardState {
  readonly word: string
  readonly tone: 'live' | 'amber' | 'red'
  readonly spoken: string
}

const lowerFirst = (text: string): string => (/^[A-Z][a-z]/.test(text) ? `${text.charAt(0).toLowerCase()}${text.slice(1)}` : text)

/**
 * What an idle teammate last did, for the card's hover (0.609): "last worked
 * 3 hours ago", or "no work yet" before a first conversation. It was drawn
 * beside the name in 0.606 ("3h ago", "no work yet") and read as debris
 * (Colin, 2026-10-04: "looks trashy as well"); the card is calm when the
 * teammate is, and the fact is a hover away.
 */
export function lastWorkedPhrase(lastAt: string | undefined, now: Date = new Date()): string {
  const ago = lastAt === undefined ? undefined : agoLabel(lastAt, now)
  return ago === undefined ? 'no work yet' : `last worked ${ago}`
}

/**
 * THE STATE ON A TEAM CARD (0.606). A card said "working" or nothing, so a
 * teammate waiting on an answer, one blocked on a sign-in and one idle since
 * Tuesday all read the same, and the cards read as static -- the critique
 * Colin nodded at on 2026-10-04. The same fact the sidebar face and the
 * header already draw (`teammateStatusView`) is said once more where a
 * person looks for who to talk to: the live word (working, thinking,
 * replying) in the live colour, "waiting on you" in amber, the sign-in or
 * install in red. Idle says nothing here (0.609; see `lastWorkedPhrase`).
 */
export function teamCardState(view: TeammateStatusView | undefined): TeamCardState | undefined {
  if (view === undefined) return undefined
  if (view.status === 'blocked') {
    // Still being checked (the amber case): a moment, not a state. Every card
    // went red-amber "AI agent not answ…" for the seconds after launch while
    // the strip said "checking 7 on this machine" (a dev frame of 0.609).
    if (view.tone === 'amber') return undefined
    const word = lowerFirst(view.label)
    return { word, tone: 'red', spoken: word }
  }
  if (view.status === 'approval-needed') return { word: 'waiting on you', tone: 'amber', spoken: 'waiting on you' }
  if (view.status === 'working') {
    const word = faceLabel(view.activity)
    return { word, tone: 'live', spoken: `${word} now` }
  }
  return undefined
}
