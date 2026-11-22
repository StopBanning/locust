import type { ReactElement } from 'react'

import type { PublicTeammate } from '../../../shared/ipc.js'
import { awayCounts, awayLine } from '../../../shared/away.js'
import type { AwayItem, AwayMissed, AwaySummary } from '../../../shared/away.js'
import { Icon } from './Icon.js'
import { clock } from '../../../shared/clock.js'

/**
 * SINCE YOU WERE AWAY, on Home (0.590, PRD R17).
 *
 * The morning-after view: what ended while nobody was at the window, in
 * four kinds and no more -- ran, failed, missed (a routine's slot that passed
 * while Locust was closed), waiting on you. Each run is a row that opens its
 * record; a miss names its routine and the time it was due. "Got it" takes
 * the list away and moves the mark, so the next absence starts from now.
 * Drawn only when there is something to say: the reducer returns nothing
 * otherwise, and Home keeps its shape.
 */

const SHOWN = 6

function when(iso: string, now: Date): string {
  const at = new Date(iso)
  if (!Number.isFinite(at.getTime())) return ''
  const sameDay = at.toDateString() === now.toDateString()
  const time = clock(at)
  if (sameDay) return time
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (at.toDateString() === yesterday.toDateString()) return `yesterday ${time}`
  return `${at.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })} ${time}`
}

export function AwayList({
  summary,
  teammates,
  onOpen,
  onOpenRoutines,
  onSeen,
  saidOf,
  now = new Date()
}: {
  readonly summary: AwaySummary
  readonly teammates: readonly PublicTeammate[]
  /**
   * What a run came to, in a line (0.758): its answer's first line, or why it failed. A row said only that a routine
   * had run -- "Polish overnight · finished 6:12" -- and what it did was a press away (Colin, 2026-10-12, of a night's
   * polish: "how does the user see this, i had no idea this even happened").
   */
  readonly saidOf?: (missionId: string) => string | undefined
  readonly onOpen: (missionId: string) => void
  /** The Routines screen, where a miss is explained and the routine can be run. */
  readonly onOpenRoutines?: () => void
  readonly onSeen: () => void
  readonly now?: Date
}): ReactElement {
  const nameOf = (item: AwayItem): string | undefined => teammates.find((teammate) => teammate.teammateId === item.ownerId)?.name
  const run = (item: AwayItem, kind: 'ran' | 'failed'): ReactElement => {
    const said = saidOf?.(item.missionId)
    return (
    <li key={`${kind}-${item.missionId}`}>
      <button type="button" className="lc-away__row" data-kind={kind} onClick={() => onOpen(item.missionId)} title={said === undefined ? item.title : `${item.title}\n${said}`}>
        <span className={`lc-away__mark lc-away__mark--${kind}`} aria-hidden="true" />
        <span className="lc-away__text">
          <span className="lc-away__title">{item.title}</span>
          {said !== undefined && <span className="lc-away__said">{said}</span>}
        </span>
        <span className="lc-away__meta lc-mono">
          {nameOf(item) !== undefined && <span className="lc-away__who">{nameOf(item)}</span>}
          <span className="lc-away__when">{kind === 'failed' ? 'failed' : 'finished'} {when(item.at, now)}</span>
        </span>
      </button>
    </li>
    )
  }
  const miss = (item: AwayMissed): ReactElement => (
    <li key={`missed-${item.routineId}-${item.dueAt}`}>
      <button type="button" className="lc-away__row" data-kind="missed" onClick={onOpenRoutines} disabled={onOpenRoutines === undefined} title={`${item.name}: due ${when(item.dueAt, now)}, Locust was closed`}>
        <span className="lc-away__mark lc-away__mark--missed" aria-hidden="true" />
        <span className="lc-away__title">{item.name}</span>
        <span className="lc-away__meta lc-mono">
          <span className="lc-away__when">missed, due {when(item.dueAt, now)}</span>
        </span>
      </button>
    </li>
  )
  const rows: ReactElement[] = [
    ...summary.failed.slice(0, SHOWN).map((item) => run(item, 'failed')),
    ...summary.missed.slice(0, SHOWN).map(miss),
    ...summary.ran.slice(0, SHOWN).map((item) => run(item, 'ran'))
  ]
  const more = summary.failed.length + summary.missed.length + summary.ran.length - rows.length
  const counts = awayCounts(summary)
  return (
    <section className="lc-away" aria-label="Since you were away">
      <div className="lc-away__head">
        <span className="lc-away__heading">
          <Icon name="clock" size={13} /> Since you were away
        </span>
        <span className="lc-away__line lc-mono">{(awayLine(counts) ?? '').replace(/^Since you were away: /, '')}</span>
        <button type="button" className="lc-ghostbutton lc-away__seen" onClick={onSeen}>
          Got it
        </button>
      </div>
      {counts.waiting > 0 && (
        <p className="lc-away__waiting">
          {counts.waiting === 1 ? 'One conversation is waiting on you -- the sidebar marks it.' : `${String(counts.waiting)} conversations are waiting on you -- the sidebar marks them.`}
        </p>
      )}
      {rows.length > 0 && <ul className="lc-away__list">{rows}</ul>}
      {more > 0 && <p className="lc-away__more lc-mono">and {String(more)} more in the sidebar</p>}
    </section>
  )
}
