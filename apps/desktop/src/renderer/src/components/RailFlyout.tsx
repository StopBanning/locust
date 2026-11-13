import { useEffect, useRef } from 'react'
import type { ReactElement } from 'react'

import type { PublicTeammate } from '../../../shared/ipc.js'
import { missionPhaseView } from '../status.js'
import { railEmptyLine, railRows, shortAgo } from '../railFlyout.js'
import { Icon } from './Icon.js'
import { TeammateBot } from './TeammateBot.js'
import { RuntimeMark } from './RuntimeMark.js'
import { isOwnRoute } from '../routeName.js'
import type { SidebarMission } from './Sidebar.js'

/**
 * A teammate's conversations, drawn beside the 64px rail.
 *
 * The rail draws no conversation rows because it has no room to draw them
 * honestly. This is the full sidebar's teammate row at full width, one pixel
 * outside a rail that cannot hold it (design agent, 2026-09-08): name, role,
 * state and route as real text, then the conversations as real rows with a
 * phase dot, a turn count and an age.
 *
 * It is CHROME, not thread content, so the three-register rule does not reach
 * it and it adds no twentieth species. It wears the same shell as the route
 * picker and the context menu.
 *
 * Positioned from the trigger's MEASURED rect -- the parent passes `top` and
 * `left` in viewport pixels. The mock's first draft guessed a row pitch and
 * drifted four pixels further out of line with every teammate down the rail;
 * a measurement cannot.
 */
export function RailFlyout({
  teammate,
  statusLabel,
  statusTone,
  route,
  missions,
  selectedMissionId,
  top,
  left,
  pinned,
  onSelectMission,
  onMissionMenu,
  renamingMissionId,
  onRename,
  onRenameDone,
  onOpenMissions,
  onNewConversation,
  filtered = false,
  onFilter,
  onPointerEnter,
  onPointerLeave,
  onClose
}: {
  readonly teammate: PublicTeammate
  /** `Code & Migrations · working`, as the full sidebar draws it. */
  readonly statusLabel: string
  readonly statusTone: string
  readonly route: string | undefined
  readonly missions: readonly SidebarMission[]
  readonly selectedMissionId: string | undefined
  readonly top: number
  readonly left: number
  readonly pinned: boolean
  readonly onSelectMission: (missionId: string) => void
  readonly onMissionMenu: (missionId: string, at: { readonly x: number; readonly y: number }) => void
  /**
   * M35 (the code review): Rename, chosen in the rail, renamed nothing -- the
   * field lived only in the wide list. Here the row becomes the field, with
   * the wide row's rules: Enter commits, Escape abandons, losing focus commits.
   */
  readonly renamingMissionId?: string
  readonly onRename?: (missionId: string, name: string) => void
  readonly onRenameDone?: () => void
  readonly onOpenMissions: () => void
  readonly onNewConversation: () => void
  /**
   * Whether the sidebar's list is narrowed to this teammate, and the switch
   * for it. This is where the face's old click went (0.234): the face now
   * OPENS their conversation, and the card that already lists their
   * conversations is the natural home for "only theirs". Absent in the rail,
   * which draws no list to narrow.
   */
  readonly filtered?: boolean
  readonly onFilter?: () => void
  readonly onPointerEnter: () => void
  readonly onPointerLeave: () => void
  readonly onClose: () => void
}): ReactElement {
  const root = useRef<HTMLDivElement | null>(null)
  const { shown, countLabel } = railRows(missions)

  // Esc, or a click anywhere outside, unpins. Only while pinned: a hover-open
  // panel closes by the pointer leaving, and a document listener for that
  // would fight the hover intent that opened it.
  useEffect(() => {
    if (!pinned) return
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') onClose()
    }
    const onDown = (event: MouseEvent): void => {
      const target = event.target
      if (target instanceof Node && root.current?.contains(target)) return
      if (target instanceof Element && target.closest('.lc-railslot') !== null) return
      onClose()
    }
    document.addEventListener('keydown', onKey)
    document.addEventListener('mousedown', onDown)
    return () => {
      document.removeEventListener('keydown', onKey)
      document.removeEventListener('mousedown', onDown)
    }
  }, [pinned, onClose])

  return (
    <div
      ref={root}
      className="lc-railflyout"
      role="dialog"
      aria-label={`${teammate.name}'s conversations`}
      style={{ top, left }}
      onMouseEnter={onPointerEnter}
      onMouseLeave={onPointerLeave}
    >
      <div className="lc-railflyout__head">
        <TeammateBot hue={teammate.hue} avatar={teammate.avatar} size={30} teammateId={teammate.teammateId} />
        <span className="lc-railflyout__who">
          <span className="lc-railflyout__name">{teammate.name}</span>
          <span className={`lc-railflyout__status lc-tone-${statusTone}`}>{statusLabel}</span>
          {route !== undefined && (
            <span className="lc-railflyout__route lc-mono">
              {teammate.route !== undefined && !isOwnRoute(teammate.route.model) && <RuntimeMark runtime={teammate.route.runtime} size={11} className="is-inline" />}
              {route}
            </span>
          )}
        </span>
      </div>

      <div className="lc-railflyout__rule" />

      {missions.length === 0 ? (
        <p className="lc-railflyout__empty">{railEmptyLine(teammate.name)}</p>
      ) : (
        <>
          <div className="lc-railflyout__label">
            <span className="lc-fieldlabel">Conversations</span>
            <span className="lc-mono lc-railflyout__count">{countLabel}</span>
          </div>
          <div className="lc-railflyout__list">
            {shown.map((mission) => {
              const view = missionPhaseView(mission.phase, mission.integrityIssueCount > 0)
              const live = mission.phase === 'running'
              const active = selectedMissionId !== undefined && (mission.memberIds ?? [mission.missionId]).includes(selectedMissionId)
              const age = shortAgo(mission.lastAt)
              if (renamingMissionId !== undefined && (mission.memberIds ?? [mission.missionId]).includes(renamingMissionId)) {
                const commit = (next: string): void => {
                  onRenameDone?.()
                  if (next.trim() !== mission.title) onRename?.(mission.missionId, next)
                }
                return (
                  <input
                    key={mission.missionId}
                    className="lc-input lc-conv__rename lc-railflyout__rename"
                    defaultValue={mission.title}
                    maxLength={120}
                    aria-label="Name this conversation"
                    autoFocus
                    onKeyDown={(event) => {
                      if (event.key === 'Escape') onRenameDone?.()
                      if (event.key === 'Enter') commit(event.currentTarget.value)
                    }}
                    onBlur={(event) => commit(event.currentTarget.value)}
                  />
                )
              }
              return (
                <button
                  key={mission.missionId}
                  type="button"
                  className={`lc-railflyout__row${live || active ? ' is-live' : ''}`}
                  onClick={() => onSelectMission(mission.missionId)}
                  // The same mission menu the full sidebar's rows open, so Save
                  // as routine is reachable in the rail -- the action a tester
                  // concluded did not exist.
                  onContextMenu={(event) => {
                    event.preventDefault()
                    onMissionMenu(mission.missionId, { x: event.clientX, y: event.clientY })
                  }}
                >
                  <span className={`lc-dot lc-tone-${view.tone}${live ? ' is-pulsing' : ''}`} />
                  {mission.appStarted && (
                    <span className="lc-conv__routine lc-conv__app" role="img" aria-label="Started by another app through Locust" title="Started by another app through Locust">
                      <Icon name="plug" size={11} />
                    </span>
                  )}
                  <span className="lc-railflyout__title">{mission.title}</span>
                  {(mission.turns ?? 1) > 1 && <span className="lc-railflyout__turns lc-mono">{mission.turns}</span>}
                  {age !== undefined && <span className="lc-railflyout__age lc-mono">{age}</span>}
                </button>
              )
            })}
          </div>
        </>
      )}

      <div className="lc-railflyout__rule" />
      <div className="lc-railflyout__foot">
        <button type="button" className="lc-railflyout__action" onClick={onOpenMissions}>
          <Icon name="inbox" size={14} />
          <span>All of {teammate.name}&apos;s conversations</span>
          <span className="lc-mono lc-railflyout__key">Ctrl 1</span>
        </button>
        <button type="button" className="lc-railflyout__action" onClick={onNewConversation}>
          <Icon name="plus" size={14} />
          <span>New conversation with {teammate.name}</span>
        </button>
        {onFilter !== undefined && (
          <button type="button" className="lc-railflyout__action" aria-pressed={filtered} onClick={onFilter}>
            <Icon name="search" size={14} />
            <span>{filtered ? 'Show everyone’s conversations' : `Only ${teammate.name}’s in the list`}</span>
          </button>
        )}
      </div>
    </div>
  )
}
