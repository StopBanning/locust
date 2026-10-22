import type { ReactElement } from 'react'

import type { PublicRoom, PublicTeammate } from '../../../shared/ipc.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { BOT_SIZE } from '../botSizes.js'
import { conversationBoard, conversationColumn } from '../conversationBoard.js'
import { conversationKeys } from '../conversationList.js'
import type { ConversationBoardFacts, ConversationBoardKey, ConversationCard } from '../conversationBoard.js'
import { isOwnRoute } from '../routeName.js'
import { agoLabel } from '../teammateWork.js'
import { ArmedButton } from './ArmedButton.js'
import { Icon } from './Icon.js'
import { RuntimeMark } from './RuntimeMark.js'
import type { SidebarMission } from './Sidebar.js'
import { TeammateBot } from './TeammateBot.js'

/**
 * Something waiting on the person that is not a conversation: a Scout card
 * that needs you now, or a routine whose copy is waiting for Keep or
 * Discard. Drawn above the conversation cards in Needs you. The actions are
 * the ones that card already has where it lives; the rest of the card opens
 * that place.
 */
export type BoardWait =
  | {
      readonly kind: 'scout'
      readonly cardId: string
      /** What Scout said, in one line. */
      readonly said: string
      /** Scout's name. */
      readonly who: string
      readonly face?: PublicTeammate
      /** Set when the card has a draft: Send and Change it first name this teammate. */
      readonly sendTo?: string
    }
  | {
      readonly kind: 'routine'
      readonly routineId: string
      readonly name: string
      /** "Waiting for you", and how many files when there are any. */
      readonly line: string
      readonly face?: PublicTeammate
    }

/**
 * THE BOARD (0.585): every conversation, in columns by what it needs from
 * the person -- Needs you, Working, Ready to look at, Done -- over every
 * runtime at once. Devin Desktop's Command Center is the reference; Locust
 * can draw it across Claude Code, Codex, Cursor, OpenCode, Copilot and
 * Antigravity, which Devin cannot. The columns are decided in
 * `conversationBoard.ts`; this draws them with the team board's sections
 * (`.lc-boardsection`), so the two boards read the same.
 *
 * A card is the sidebar row with its face, its title and one line of state:
 * what it is waiting for, that it is working, when it finished. Pressing it
 * opens the conversation, which is also what takes a finish out of Ready to
 * look at. A Scout card that needs you now, and a routine waiting for Keep
 * or Discard, sit above those conversations in Needs you, with the actions
 * they already have. A quiet app -- nothing waiting, nothing working, nothing
 * to look at -- says so and lists what is done, rather than drawing one column.
 */
export function BoardScreen({
  missions,
  rooms,
  trashed,
  facts,
  waitingFor,
  teammates,
  missionOwners,
  onOpen,
  now,
  waiting = [],
  onOpenScout,
  onSendScout,
  onChangeScout,
  onDismissScout,
  onOpenRoutine,
  onSettleRoutine
}: {
  readonly missions: readonly SidebarMission[]
  readonly rooms: readonly PublicRoom[]
  readonly trashed: ReadonlySet<string>
  readonly facts: ConversationBoardFacts
  /** What a conversation in Needs you is waiting for, in one line, by turn id. */
  readonly waitingFor: ReadonlyMap<string, string>
  readonly teammates: readonly PublicTeammate[]
  readonly missionOwners: Readonly<Record<string, string>>
  readonly onOpen: (missionId: string) => void
  readonly now?: Date
  /** Scout cards and routines waiting on the person, above the conversations. */
  readonly waiting?: readonly BoardWait[]
  readonly onOpenScout?: (cardId: string) => void
  readonly onSendScout?: (cardId: string) => void
  readonly onChangeScout?: (cardId: string) => void
  readonly onDismissScout?: (cardId: string) => void
  readonly onOpenRoutine?: (routineId: string) => void
  readonly onSettleRoutine?: (routineId: string, decision: 'keep' | 'discard' | 'open') => void
}): ReactElement {
  const board = conversationBoard(missions, rooms, trashed, facts)
  const ownerOf = (row: SidebarMission): PublicTeammate | undefined => {
    const id = row.ownerId ?? missionOwners[row.missionId] ?? (row.memberIds ?? []).map((member) => missionOwners[member]).find((owner) => owner !== undefined)
    return id === undefined ? undefined : teammates.find((teammate) => teammate.teammateId === id)
  }
  const titleOf = (card: ConversationCard): string | undefined => {
    const parent = card.parentId === undefined ? undefined : missions.find((row) => row.missionId === card.parentId)
    return parent === undefined ? undefined : `under ${parent.title}`
  }
  const stateOf = (card: ConversationCard, key: ConversationBoardKey): string => {
    const row = card.row
    const ago = row.lastAt === undefined ? undefined : agoLabel(row.lastAt, now)
    if (key === 'needs-you') {
      const what = (row.memberIds ?? [row.missionId]).map((id) => waitingFor.get(id)).find((line) => line !== undefined) ?? waitingFor.get(row.missionId)
      return what ?? 'waiting on you'
    }
    if (key === 'working') return 'working'
    const word = row.phase === 'completed' ? (key === 'to-look-at' ? 'finished' : 'done') : row.phase === 'failed' ? 'failed' : row.phase === 'running' ? 'working' : 'stopped'
    return ago === undefined ? word : `${word} · ${ago}`
  }
  const card = (entry: ConversationCard, key: ConversationBoardKey): ReactElement => {
    const row = entry.row
    const by = ownerOf(row)
    const under = titleOf(entry)
    return (
      <button type="button" key={row.missionId} className="lc-convcard" data-column={key} onClick={() => onOpen(row.missionId)} title={row.title}>
        {by === undefined ? (
          row.runtime === undefined || row.model === undefined || isOwnRoute(row.model) ? (
            <span className="lc-conv__nobody" aria-hidden="true" />
          ) : (
            <span className="lc-conv__runtime">
              <RuntimeMark runtime={row.runtime} size={13} label={runtimeDisplayName(row.runtime)} />
            </span>
          )
        ) : (
          <TeammateBot hue={by.hue} avatar={by.avatar} size={BOT_SIZE.conversationOwner} teammateId={by.teammateId} name={by.name} />
        )}
        <span className="lc-convcard__title">{row.title}</span>
        <span className="lc-convcard__meta lc-mono">
          {by !== undefined && <span className="lc-convcard__who">{by.name}</span>}
          {under !== undefined && <span className="lc-convcard__under">{under}</span>}
          <span className="lc-convcard__state">{stateOf(entry, key)}</span>
        </span>
      </button>
    )
  }
  const faceOf = (mate: PublicTeammate | undefined): ReactElement =>
    mate === undefined ? (
      <span className="lc-conv__nobody" aria-hidden="true" />
    ) : (
      <TeammateBot hue={mate.hue} avatar={mate.avatar} size={BOT_SIZE.conversationOwner} teammateId={mate.teammateId} name={mate.name} />
    )
  const waitCard = (entry: BoardWait): ReactElement => {
    if (entry.kind === 'scout') {
      return (
        <div key={`scout:${entry.cardId}`} className="lc-boardwait" data-column="needs-you" data-board-scout={entry.cardId}>
          <button type="button" className="lc-boardwait__open" onClick={() => onOpenScout?.(entry.cardId)}>
            {faceOf(entry.face)}
            <span className="lc-convcard__title">{entry.said}</span>
            <span className="lc-convcard__meta lc-mono">
              <span className="lc-convcard__who">{entry.who}</span>
              <span className="lc-convcard__state">Needs you</span>
            </span>
          </button>
          <div className="lc-boardwait__acts">
            {entry.sendTo !== undefined && (
              <button type="button" className="lc-primarybutton" onClick={() => onSendScout?.(entry.cardId)}>{`Send to ${entry.sendTo}`}</button>
            )}
            {entry.sendTo !== undefined && (
              <button type="button" className="lc-control lc-control--boxed" onClick={() => onChangeScout?.(entry.cardId)}>
                Change it first
              </button>
            )}
            <button type="button" className="lc-ghostbutton" onClick={() => onDismissScout?.(entry.cardId)}>
              Dismiss
            </button>
          </div>
        </div>
      )
    }
    return (
      <div key={`routine:${entry.routineId}`} className="lc-boardwait" data-column="needs-you" data-board-routine={entry.routineId}>
        <button type="button" className="lc-boardwait__open" onClick={() => onOpenRoutine?.(entry.routineId)}>
          {faceOf(entry.face)}
          <span className="lc-convcard__title">{entry.name}</span>
          <span className="lc-convcard__meta lc-mono">
            {entry.face !== undefined && <span className="lc-convcard__who">{entry.face.name}</span>}
            <span className="lc-convcard__state">{entry.line}</span>
          </span>
        </button>
        <div className="lc-boardwait__acts">
          <button type="button" className="lc-primarybutton" onClick={() => onSettleRoutine?.(entry.routineId, 'keep')}>
            Keep
          </button>
          <ArmedButton
            className="lc-ghostbutton"
            ariaLabel={`Discard the changes from ${entry.name}`}
            armedLabel="Discard for good?"
            onConfirm={() => onSettleRoutine?.(entry.routineId, 'discard')}
          >
            Discard
          </ArmedButton>
          <button type="button" className="lc-linkbutton" onClick={() => onSettleRoutine?.(entry.routineId, 'open')}>
            Open the copy
          </button>
        </div>
      </div>
    )
  }
  const needsYouMembers = board?.find((section) => section.key === 'needs-you')?.members ?? []
  const waitingCount = needsYouMembers.length + waiting.length
  const working = board?.find((section) => section.key === 'working')?.members.length ?? 0
  const meta = board === undefined && waiting.length === 0
    ? 'all quiet'
    : [waitingCount > 0 ? `${String(waitingCount)} waiting on you` : undefined, working > 0 ? `${String(working)} working` : undefined].filter((part) => part !== undefined).join(' · ') || 'nothing running'
  const quiet = board === undefined
    ? missions.filter((row) => conversationColumn(row, facts) === 'done' && !conversationKeys(row).some((id) => trashed.has(id)))
    : []
  const needsYouColumn = (members: readonly ConversationCard[]): ReactElement => (
    <section key="needs-you" className="lc-boardsection lc-boardsection--needs-you" data-column="needs-you" aria-label="Needs you">
      <h2 className="lc-boardsection__title">
        <span className="lc-boardsection__mark" aria-hidden="true" />
        Needs you
        <span className="lc-boardsection__count lc-mono">{members.length + waiting.length}</span>
      </h2>
      <div className="lc-convgrid">
        {waiting.map(waitCard)}
        {members.map((entry) => card(entry, 'needs-you'))}
      </div>
    </section>
  )
  const column = (section: { readonly key: ConversationBoardKey; readonly title: string; readonly members: readonly ConversationCard[] }): ReactElement =>
    section.key === 'needs-you' ? (
      needsYouColumn(section.members)
    ) : (
      <section key={section.key} className={`lc-boardsection lc-boardsection--${section.key}`} data-column={section.key} aria-label={section.title}>
        <h2 className="lc-boardsection__title">
          <span className="lc-boardsection__mark" aria-hidden="true" />
          {section.title}
          <span className="lc-boardsection__count lc-mono">{section.members.length}</span>
        </h2>
        <div className="lc-convgrid">{section.members.map((entry) => card(entry, section.key))}</div>
      </section>
    )
  const columns = board === undefined ? [] : board.map(column)
  const withWaits = waiting.length > 0 && (board === undefined || !board.some((section) => section.key === 'needs-you'))
    ? [needsYouColumn([]), ...columns]
    : columns
  return (
    <section className="lc-screen lc-board" aria-label="Board">
      <div className="lc-screen__header">
        <span className="lc-screen__title">Board</span>
        <span className="lc-screen__meta lc-mono">{meta}</span>
      </div>
      <div className="lc-screen__scroll">
        {board === undefined && waiting.length === 0 ? (
          <>
            <p className="lc-screen__note">
              <Icon name="columns" size={12} /> Nothing is waiting on you and nothing is running. Conversations land here by what they need from you: a card to answer, work under way, a finish you have not looked at.
            </p>
            {quiet.length > 0 && (
              <section className="lc-boardsection lc-boardsection--done" data-column="done" aria-label="Done">
                <h2 className="lc-boardsection__title">
                  <span className="lc-boardsection__mark" aria-hidden="true" />
                  Done
                  <span className="lc-boardsection__count lc-mono">{quiet.length}</span>
                </h2>
                <div className="lc-convgrid">{quiet.slice(0, 12).map((row) => card({ row }, 'done'))}</div>
              </section>
            )}
          </>
        ) : (
          <>
            <div className="lc-teamboard lc-convboard">{withWaits}</div>
            {board === undefined && quiet.length > 0 && (
              <section className="lc-boardsection lc-boardsection--done" data-column="done" aria-label="Done">
                <h2 className="lc-boardsection__title">
                  <span className="lc-boardsection__mark" aria-hidden="true" />
                  Done
                  <span className="lc-boardsection__count lc-mono">{quiet.length}</span>
                </h2>
                <div className="lc-convgrid">{quiet.slice(0, 12).map((row) => card({ row }, 'done'))}</div>
              </section>
            )}
          </>
        )}
      </div>
    </section>
  )
}
