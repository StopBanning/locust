import { Fragment, useEffect, useMemo, useRef, useState } from 'react'
import { BOT_SIZE } from '../botSizes.js'
import type { ReactElement, ReactNode } from 'react'

import type { GroupMembership, PublicGroup, PublicRecoveredMission, PublicRoutine, PublicRuntimeStatus, PublicTeammate, PublicRoom } from '../../../shared/ipc.js'
import { roleLabelOf } from '../../../shared/ipc.js'
import type { LiveActivity } from '../faceState.js'
import { glancesAmong } from '../glances.js'
import type { Handoff } from '../glances.js'
import { isOwnRoute, routeChrome, routeModelName } from '../routeName.js'
import { isPutAway } from '../settled.js'
import type { SettledEntry } from '../settled.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { branchNameFor } from '../../../shared/worktree-name.js'
import mark from '../assets/locust-mark.svg'
import wordmark from '../assets/locust-wordmark.svg'
import {
  connectedRuntimeCount,
  facePresenceFor,
  missionPhaseView,
  missionsMatching,
  runtimeReach,
  shortMissionId,
  runtimeOfTeammate,
  stateInWords,
  teammateStatusView
} from '../status.js'
import { TeammateBot } from './TeammateBot.js'
import { LIST_PAGE, ListMore, WhenNear } from './ListWindow.js'
import { RuntimeMark } from './RuntimeMark.js'
import type { TeammateStatusView } from '../status.js'
import { Icon } from './Icon.js'
import { teammateTooltip } from '../teammateTooltip.js'
import { railCountBadge } from '../railFlyout.js'
import { conversationRows, heldFor, narrowingLine, ownerOf, unreadableSentence, withRoomsFolded } from '../conversationList.js'
import { RailFlyout } from './RailFlyout.js'
import { useLiveCallPlace } from '../liveCall.js'
import { routineStepLabel } from '../routines.js'

/**
 * A teammate's sidebar line is "<role> · <state>". When the state is the
 * news ("subagent working"), the role is the part allowed to be cut short.
 */
export function labelRole(label: string): string {
  const at = label.lastIndexOf(' · ')
  return at === -1 ? '' : label.slice(0, at)
}

export function labelState(label: string): string {
  const at = label.lastIndexOf(' · ')
  return at === -1 ? label : ' · ' + label.slice(at + 3)
}


export interface SidebarMission {
  /**
   * Who the mission belongs to, when the shell knows before the host has
   * recorded it -- a run that is still starting has no missionId to look up.
   */
  readonly ownerId?: string
  readonly missionId: string
  readonly title: string
  readonly phase: PublicRecoveredMission['phase'] | 'running'
  /** Which runtime this mission is on, when it is known. */
  readonly runtime?: PublicRecoveredMission['runtime']
  /** Its model, for the mark a conversation with no teammate wears (a model of your own has none). */
  readonly model?: string
  readonly integrityIssueCount: number
  /**
   * The mission that began this conversation, and the turn immediately
   * before this one. A reply is its own mission -- one run, one receipt --
   * so without these the sidebar lists an exchange as several entries while
   * the thread beside it shows it as one.
   */
  readonly rootId?: string
  readonly parentId?: string
  /** Every turn this row stands for, so selecting any of them lights it. */
  readonly memberIds?: readonly string[]
  /** Turns in the conversation; 1 is an ordinary single-run mission. */
  readonly turns?: number
  /** This turn's own words, before the collapse names the row by its first. */
  readonly words?: string
  /** What the person said in every turn, for search (`collapseConversations`). */
  readonly said?: string
  /** When this conversation last moved, for the rail flyout's age column. */
  readonly lastAt?: string
  /**
   * The routine whose replay began this conversation, when one did. A
   * routine replays a conversation's words, so its run wore the same title
   * as the conversation it was saved from, and the two rows could only be
   * told apart by opening them (0.271 design recheck, finding 2).
   */
  readonly routineId?: string
  /** This row stands for a comparison not yet decided (0.441, compareRows.ts): it opens the comparison. */
  readonly compareId?: string
  /** The folder this conversation belongs to (0.458): the sidebar lists it under that project. */
  readonly folderId?: string
  /** The conversation that sent this one, when a teammate started it for another (0.463): drawn under it. */
  readonly nestedUnder?: string
}


/** Whether a row is the conversation the workroom is showing. */
function isShown(mission: SidebarMission, selectedMissionId: string | undefined): boolean {
  if (selectedMissionId === undefined) return false
  return (mission.memberIds ?? [mission.missionId]).includes(selectedMissionId)
}

/**
 * The workspace sidebar.
 *
 * There is no teammate list yet and none is drawn: teammates arrive in P2 as a
 * real local store. Showing the reference's four sample teammates here would
 * put fictional colleagues beside a live mission, which is the one thing the
 * design spec forbids -- so the region is simply absent until it is real.
 */
/**
 * A sidebar group that folds.
 *
 * Colin, 2026-09-06: *"make them all collapsible dropdowns to give the user
 * more space"*. The heading keeps the same label type it always had -- this
 * adds the chevron and the count, not a new look -- and it is a real button,
 * so the whole row is the target rather than a caret a person has to hit.
 *
 * An empty group still draws: *"if no automations dont remove, just have it
 * as a holder, we want the user to know its possible even if none are setup"*.
 * A count of zero is information; a missing section is not.
 */
/**
 * How many faces the roster strip draws, and when it starts counting.
 *
 * Two thresholds, not one, from the design agent's ruling of 2026-09-15:
 * five teammates or fewer draws five faces and NO chip; six or more draws
 * four and then `+N`. The chip appears only when it has something to
 * report, which is the rule everywhere else in this app.
 *
 * The reason it is four rather than five once the chip exists is the whole
 * lesson of the first attempt. Their original six was a face-width
 * calculation that ignored its own neighbours: with the `+N` chip and the
 * `Team` pill sharing the row it wanted 311px of 267. Five measured at
 * exactly 267 of 267 -- true, and with no margin at all, which a wider count
 * like `+55` would spend. Four leaves room for the count to grow.
 */
const FACES_WITHOUT_CHIP = 5
const FACES_WITH_CHIP = 4

function SidebarSection({
  label,
  count,
  open,
  onToggle,
  children
}: {
  readonly label: string
  readonly count: number
  readonly open: boolean
  readonly onToggle: () => void
  readonly children?: ReactNode
}): ReactElement {
  return (
    <>
      <button
        type="button"
        className={`lc-sectionlabel lc-sectionlabel--fold${open ? ' is-open' : ''}`}
        aria-expanded={open}
        onClick={onToggle}
      >
        <Icon name={open ? 'chevron-down' : 'chevron-right'} size={11} />
        <span>{label}</span>
        <span className="lc-sectionlabel__count">{String(count)}</span>
      </button>
      {open && children}
    </>
  )
}

/** No conversation pinned. */
const NO_PINS: readonly string[] = []
/** Nothing settled or snoozed. */
const NOTHING_SETTLED: Readonly<Record<string, SettledEntry>> = {}

export function Sidebar({
  runtimes,
  missions,
  teammates,
  viewByTeammate,
  routineStepByTeammate,
  branchByTeammate = {},
  missionOwners,
  selectedMissionId,
  selectedTeammateId,
  onSelectMission,
  onMissionMenu,
  renamingMissionId,
  pinnedKeys = NO_PINS,
  settled = NOTHING_SETTLED,
  onRenameMission,
  onRenameDone,
  groups = [],
  groupMembers = {},
  unreadable = [],
  unreadableConversations = 0,
  onGroupMenu,
  renamingGroupId,
  onRenameGroup,
  onGroupRenameDone,
  onNewGroup,
  namingGroup = false,
  onNamingGroupDone,
  onAddMenu,
  addMenuOpen = false,
  onTeammateMenu,
  pendingApprovals,
  liveActivity,
  starting,
  recentlyDone,
  recentlyReceived,
  handoffs = [],
  onSelectTeammate,
  onOpenHub,
  onNewConversationWith,
  composerShown,
  onOpenSettings,
  onOpenMissions,
  onOpenTeammates,
  rooms,
  routines,
  currentRoomId,
  screenKey,
  onOpenRoom,
  onOpenRooms,
  onOpenAutomations,
  onOpenBoard,
  cloudTasks,
  background,
  benchmarks,
  onHome,
  compact = false
}: {
  readonly runtimes: readonly PublicRuntimeStatus[]
  readonly missions: readonly SidebarMission[]
  readonly teammates: readonly PublicTeammate[]
  /**
   * Every teammate's state, decided once in App. The sidebar draws it; it
   * does not work it out, because two surfaces working out one fact is how
   * they come to disagree.
   */
  readonly viewByTeammate: Readonly<Record<string, TeammateStatusView>>
  /** Which routine each teammate is replaying right now, if any. */
  readonly routineStepByTeammate: Readonly<Record<string, { readonly name: string; readonly step: number; readonly of: number }>>
  /** The branch each teammate's own tree is on, as git reports it; the name's branch until one is made (M17). */
  readonly branchByTeammate?: Readonly<Record<string, string>>
  readonly missionOwners: Readonly<Record<string, string>>
  readonly selectedMissionId: string | undefined
  /** Who the composer is addressing. Selecting a teammate makes them the next mission's owner. */
  readonly selectedTeammateId: string | undefined
  readonly onSelectMission: (missionId: string) => void
  /** Right-click on a mission row, so it can be acted on without opening it. */
  readonly onMissionMenu: (missionId: string, at: { readonly x: number; readonly y: number }) => void
  /** The conversation being renamed in place, if any. */
  readonly renamingMissionId?: string
  /** Conversations pinned to the top, by conversation key, newest pin first (0.729). */
  readonly pinnedKeys?: readonly string[]
  /** Conversations settled or snoozed out of the list, by conversation key (0.730, settled.ts). */
  readonly settled?: Readonly<Record<string, SettledEntry>>
  /** Commit a new name. An empty string clears it back to what was typed. */
  readonly onRenameMission?: (missionId: string, title: string) => void
  readonly onRenameDone?: () => void
  /** Named sets of conversations, in this folder. */
  readonly groups?: readonly PublicGroup[]
  /** Which group each conversation is in, by conversation id. */
  readonly groupMembers?: Readonly<Record<string, GroupMembership>>
  /**
   * Local files that exist and would not read -- `teammates`, `groups`,
   * `rooms`, `routines` -- which is not the same as any of them being empty.
   *
   * The store has told them apart since 0.146.0 and the host relayed it, and
   * the sidebar dropped the answer on the floor: `if (!response.ok) return`
   * left the list looking exactly like a folder with no groups. Astra,
   * 2026-09-16, with `groups.json` as a directory, truncated, and oversize:
   * "in all three cases the group disappeared and the conversation appeared
   * as an ordinary ungrouped row." The rooms lesson, applied a layer down
   * and then not applied at the top.
   */
  readonly unreadable?: readonly string[]
  /** Ledger files that exist and would not read; they have no row to draw. */
  readonly unreadableConversations?: number
  /** The header menu for a group: rename it, or remove it. */
  readonly onGroupMenu?: (groupId: string, at: { readonly x: number; readonly y: number }) => void
  /** The group being renamed in place, if any. */
  readonly renamingGroupId?: string
  readonly onRenameGroup?: (groupId: string, name: string) => void
  readonly onGroupRenameDone?: () => void
  /** Make a new group. Absent means the `+` offers none. */
  readonly onNewGroup?: (name: string) => void
  /** Whether to ask for a new group's name now; the host owns this, because
      the conversation menu can start it too. */
  readonly namingGroup?: boolean
  readonly onNamingGroupDone?: () => void
  /**
   * The `+`: New teammate, New room, New group, drawn as the right-click
   * menus are and hung from the button that asked for it.
   */
  readonly onAddMenu: (anchor: HTMLElement) => void
  /** Whether that menu is open, for the button to say so. */
  readonly addMenuOpen?: boolean
  /** Right-click on a teammate. Same menu shape as a mission row, on the row above them. */
  readonly onTeammateMenu: (teammateId: string, at: { readonly x: number; readonly y: number }) => void
  /** Approvals waiting on each teammate's live run, by teammate id. */
  readonly pendingApprovals: Readonly<Record<string, number>>
  /** What each teammate's live run is doing, by teammate id; absent means no live run. */
  readonly liveActivity: Readonly<Record<string, LiveActivity>>
  /** Teammates whose automatic reply the host is starting but has not started yet. */
  readonly starting: readonly string[]
  /** Teammates whose mission just finished, or who just received a message. */
  readonly recentlyDone: readonly string[]
  readonly recentlyReceived: readonly string[]
  /** Messages just handed between teammates: the two faces look at each other. */
  readonly handoffs?: readonly Handoff[]
  readonly onSelectTeammate: (teammateId: string) => void
  /**
   * Open the teammate's hub -- the conversation their replies to other
   * teammates land in -- or, for a teammate who has none yet, address them.
   * The face's click. Filtering the list to them lives in the hover card.
   */
  readonly onOpenHub: (teammateId: string) => void
  /** A blank page with that teammate on it, not their newest conversation. */
  readonly onNewConversationWith: (teammateId: string) => void
  /** Whether the composer is on screen; the empty state says "below" only then. */
  readonly composerShown: boolean
  readonly onOpenSettings: () => void
  /** With a teammate: their missions only, and never a toggle closed (L23). */
  readonly onOpenMissions: (teammateId?: string) => void
  readonly onOpenTeammates: () => void
  /** Rooms a person can write to at once; the one open now is highlighted. */
  readonly rooms: readonly PublicRoom[]
  /** Saved routines, listed under Routines. */
  readonly routines: readonly PublicRoutine[]
  readonly currentRoomId: string | undefined
  /** The screen the window shows; a new one closes a pinned teammate card (R27). */
  readonly screenKey?: string
  readonly onOpenRoom: (roomId: string) => void
  readonly onOpenRooms: () => void
  readonly onOpenAutomations: () => void
  /** The board (0.585): every conversation by what it needs from the person. Its place is drawn when the shell offers it. */
  readonly onOpenBoard?: () => void
  /**
   * This folder's cloud tasks (0.509), when it has any. A cloud task makes no
   * conversation, so with its panel closed nothing else in the window said it
   * was there.
   */
  readonly cloudTasks?: { readonly count: number; readonly onOpen: () => void }
  /** Background turns (W10): how many are still going, and the panel. */
  readonly background?: { readonly count: number; readonly onOpen: () => void }
  /** Your benchmarks (0.764): comparisons run again, once there is one. */
  readonly benchmarks?: { readonly count: number; readonly onOpen: () => void }
  /** Back to the home screen: nothing picked, nothing open. */
  readonly onHome: () => void
  /**
   * The 64px avatar rail is on. The rail draws no conversation rows -- four
   * pixels of a title is not a smaller list -- so hovering an avatar opens
   * that teammate's conversations as a flyout beside it, and clicking pins it
   * (design agent, 2026-09-08). None of that exists in the full sidebar, where
   * the rows are already drawn.
   */
  readonly compact?: boolean
  /** Every folder worked in, for the project headings (0.458). */
  readonly folders?: readonly { readonly id: string; readonly name: string; readonly installFolder?: true }[]
  /** The folder new conversations start in: its section comes first. */
  readonly currentFolderId?: string
}): ReactElement {
  const [query, setQuery] = useState('')
  // The conversation a call is in (0.750): its row says so, as the canvas's "On a call" did.
  const callPlace = useLiveCallPlace()
  const onCall = useMemo(() => new Set(callPlace.split(' ').filter((id) => id.length > 0)), [callPlace])
  // The rail flyout: who it is open for, whether a click pinned it, and where
  // its trigger sits on screen. Hover opens after 120ms of intent and closes
  // a moment after the pointer leaves both the avatar and the panel; a pin
  // survives the pointer and is undone by Esc, a click outside, or clicking
  // the same avatar again.
  const [railHovered, setRailHovered] = useState<string | undefined>(undefined)
  const [railPinned, setRailPinned] = useState<string | undefined>(undefined)
  const [railAnchor, setRailAnchor] = useState<{ readonly top: number; readonly left: number } | undefined>(undefined)
  const railOpenTimer = useRef<number | undefined>(undefined)
  const railCloseTimer = useRef<number | undefined>(undefined)
  const railSlots = useRef(new Map<string, HTMLElement>())
  const measureRail = (teammateId: string): void => {
    // From the trigger's MEASURED rect, never a row-pitch constant: the mock's
    // first draft guessed a pitch and drifted 4px further out with every
    // teammate down the rail, pointing at the wrong face by the sixth.
    const slot = railSlots.current.get(teammateId)
    if (slot === undefined) return
    const box = slot.getBoundingClientRect()
    // Top from the slot, so the panel lines up with the face it belongs to.
    // Left from the RAIL's edge, not the slot's: the slot is inset by the row
    // margins, and anchoring on it put the panel one pixel inside the rail
    // instead of eight past it (measured by drive-compact, 2026-09-08).
    const rail = slot.closest('.lc-sidebar')?.getBoundingClientRect()
    setRailAnchor({ top: Math.max(8, box.top - 6), left: (rail?.right ?? box.right) + 8 })
  }
  const railEnter = (teammateId: string): void => {
    /*
     * Hover opens the card in BOTH layouts now.
     *
     * The wide faces row first shipped with a native `title`, which Windows
     * draws as one unbroken line: "Message Wembley · Research & Briefs ·
     * Cursor Agent / cursor-grok-4.6-medium — click to show only their
     * conversations". Colin, 2026-09-15: "maybe the little profile cards
     * that we had built previously for the teammates can be hoverable as
     * opposed to this massive text line."
     *
     * The card already existed and already says all of it, laid out, with
     * their conversations under it. One answer to "who is this" rather than
     * two, and the tooltip was the worse of them.
     */
    window.clearTimeout(railCloseTimer.current)
    window.clearTimeout(railOpenTimer.current)
    railOpenTimer.current = window.setTimeout(() => {
      if (railPinned === undefined) {
        measureRail(teammateId)
        setRailHovered(teammateId)
      }
    }, 120)
  }
  const railLeave = (): void => {
    window.clearTimeout(railOpenTimer.current)
    if (railPinned !== undefined) return
    railCloseTimer.current = window.setTimeout(() => setRailHovered(undefined), 160)
  }
  const railPin = (teammateId: string): void => {
    // Pinning stays a RAIL affordance. Wide, a click on a face filters the
    // list, so a click that also pinned a panel over it would be two
    // answers to one press.
    if (!compact) return
    window.clearTimeout(railOpenTimer.current)
    window.clearTimeout(railCloseTimer.current)
    if (railPinned === teammateId) {
      setRailPinned(undefined)
      setRailHovered(undefined)
      return
    }
    measureRail(teammateId)
    setRailPinned(teammateId)
    setRailHovered(teammateId)
  }
  const railClose = (): void => {
    window.clearTimeout(railOpenTimer.current)
    window.clearTimeout(railCloseTimer.current)
    setRailPinned(undefined)
    setRailHovered(undefined)
  }
  /*
   * ANOTHER SCREEN CLOSES THE CARD (QA-2026-09-29 round 2, R27). A card
   * pinned by a click in the rail stayed open over whatever Ctrl+1 to Ctrl+5
   * or the palette opened next, and on Settings a click meant for
   * "Appearance" landed on it and opened All missions.
   */
  // Not for the conversation view: a face opens its teammate's conversation
  // AND its card in one press, so that change is the card's own.
  useEffect(() => {
    if (screenKey === undefined || screenKey === 'workroom') return
    window.clearTimeout(railOpenTimer.current)
    window.clearTimeout(railCloseTimer.current)
    setRailPinned(undefined)
    setRailHovered(undefined)
  }, [screenKey])
  /*
   * Which groups are folded. Named groups start FOLDED and Ungrouped starts
   * open, which is the design's order and is also right: a group is
   * something you chose to put away, so it costs one line until you want it,
   * and the list you actually scan stays at the top of the column.
   */
  const [foldedGroups, setFoldedGroups] = useState<ReadonlySet<string>>(new Set())
  /*
   * A new group is named before it exists.
   *
   * The alternative is creating "Untitled group" and making someone rename
   * it -- which rooms deliberately do, because a room made from an ask
   * should not cost a name first. A group is made on purpose from a menu, so
   * there is nothing to interrupt and the name is the whole act.
   */

  const [faceFilter, setFaceFilter] = useState<string>()
  /** How many of the long list's rows are drawn (ListWindow.tsx): a page, and more as it is scrolled. */
  const [listBudget, setListBudget] = useState(LIST_PAGE)
  // A new search or filter is a new list: it starts at one page again.
  useEffect(() => setListBudget(LIST_PAGE), [query, faceFilter])
  const railOpenFor = railPinned ?? railHovered
  /*
   * M35 (the code review): in the rail, Rename and New group did nothing --
   * their fields live in the wide list -- and the state they left behind
   * surfaced later as an autofocused field when the window widened. Rename
   * in the rail pins its teammate's card open, where the row becomes the
   * field. The rail draws no groups, so its menus offer no New group, and a
   * naming left over from the wide list is cleared.
   */
  useEffect(() => {
    if (!compact) return
    if (namingGroup === true) onNamingGroupDone?.()
    if (renamingMissionId === undefined) return
    const renamed = missions.find((mission) => (mission.memberIds ?? [mission.missionId]).includes(renamingMissionId))
    const owner = renamed === undefined ? undefined : ownerOf(renamed, missionOwners)
    if (owner === undefined) {
      onRenameDone?.()
      return
    }
    if (railPinned !== owner) {
      measureRail(owner)
      setRailPinned(owner)
      setRailHovered(owner)
    }
    // Only when the layout or the rename changes: re-pinning on every render
    // would undo a person closing the card.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [compact, renamingMissionId, namingGroup])

  // Which groups are open. All three start open, which is how the sidebar
  // has always read; folding is for making room, not a new default.
  const [openSections, setOpenSections] = useState({ rooms: true, teammates: true, missions: true, automations: true })
  const connected = connectedRuntimeCount(runtimes)
  /*
   * The flat list: every conversation in this folder, live first then newest
   * first, filtered by the same query the nested layout used.
   *
   * `faceFilter` is the roster row acting as a filter rather than a
   * container -- clicking a face narrows the list to that teammate and
   * clicking it again clears it. The teammate is still a property of every
   * row either way.
   */
  // A conversation answers to its teammate's name too: "Atlas" finds Atlas's (0.414).
  const ownerName = (mission: SidebarMission): string | undefined => {
    const owner = ownerOf(mission, missionOwners)
    return owner === undefined ? undefined : teammates.find((entry) => entry.teammateId === owner)?.name
  }
  /*
   * Settled and snoozed conversations are out of the list (0.730) -- unless it is the one open, or a search is
   * looking for it, which is how a person finds one to bring back. A snooze's end redraws the list then.
   */
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const ends = Object.values(settled).map((entry) => (entry.until === undefined ? NaN : Date.parse(entry.until))).filter((end) => end > Date.now())
    if (ends.length === 0) return
    const timer = setTimeout(() => setNow(new Date()), Math.min(Math.min(...ends) - Date.now() + 500, 2_147_000_000))
    return () => clearTimeout(timer)
  }, [settled, now])
  const searching = query.trim().length > 0
  const shownConversations = conversationRows(missionsMatching(missions, query, ownerName)).filter(
    (mission) =>
      (faceFilter === undefined || ownerOf(mission, missionOwners) === faceFilter)
      && (searching || isShown(mission, selectedMissionId) || !isPutAway(mission.lastAt, settled[mission.rootId ?? mission.missionId], now))
  )
  /*
   * Everything not in a group, keyed by the CONVERSATION rather than the
   * turn -- the same key the rename uses, because filing a reply somewhere
   * its exchange is not would be a group that lies about what it holds.
   *
   * A membership pointing at a group that has been removed is already
   * dropped by the store, so a row can never be missing from both lists.
   */
  /*
   * SIX FACES, then a chip.
   *
   * Measured at twelve teammates: the strip wanted 500px inside a 267px
   * column and the last face was drawn 172px OUTSIDE the sidebar.
   *
   * The design agent's answer, and the reason for the shape: no horizontal
   * scroll, "because a scrolling strip hides the thing it exists to expose".
   * A roster you have to drag sideways to read is not one you can scan, and
   * scanning is the whole job of the strip.
   *
   * Most recent first, so the six on screen are the six you are working with
   * rather than the six you happened to make first. A teammate with no work
   * sorts last and never above one that has some.
   *
   * The compact rail is untouched: it lists every teammate down a column
   * that scrolls, which is a different shape with a different constraint.
   */
  const facesByRecency = [...teammates].sort((left, right) => {
    const newest = (teammateId: string): number =>
      missions
        .filter((mission) => ownerOf(mission, missionOwners) === teammateId)
        .reduce((held, mission) => {
          const at = mission.lastAt === undefined ? 0 : Date.parse(mission.lastAt)
          return Number.isNaN(at) ? held : Math.max(held, at)
        }, 0)
    return newest(right.teammateId) - newest(left.teammateId)
  })
  /*
   * What is narrowing the list, said once, with the count.
   *
   * Undefined when nothing is -- an unfiltered list needs no sentence above
   * it, and a permanent one would be a control that is almost always saying
   * "everything".
   */
  const filteredTo = teammates.find((entry) => entry.teammateId === faceFilter)
  const narrowing = narrowingLine({
    faceName: filteredTo?.name,
    query,
    shown: shownConversations.length,
    inFace: missions.filter((mission) => ownerOf(mission, missionOwners) === faceFilter).length,
    all: missions.length
  })

  const shownFaces = facesByRecency.slice(
    0,
    facesByRecency.length <= FACES_WITHOUT_CHIP ? FACES_WITHOUT_CHIP : FACES_WITH_CHIP
  )
  const restOfTeam = facesByRecency.length - shownFaces.length
  // The strip is a row and the roster a column: a handoff between two faces
  // in either turns them toward each other along it.
  const stripGlances = glancesAmong(shownFaces.map((teammate) => teammate.teammateId), handoffs, 'row')
  const rosterGlances = glancesAmong(teammates.map((teammate) => teammate.teammateId), handoffs, 'column')

  /*
   * The list of conversations for one folder: its groups, then Ungrouped
   * (0.458). With one folder this is the whole list, drawn as it always was.
   */
  const showsGroup = (groupId: string, inFolder: readonly SidebarMission[], keepsEmpty: boolean): boolean =>
    inFolder.some((mission) => heldFor(mission, groupMembers)?.groupId === groupId)
    || (keepsEmpty && !shownConversations.some((mission) => heldFor(mission, groupMembers)?.groupId === groupId))
  /*
   * Conversations a teammate started for another, drawn under the one they
   * came from (0.463) -- only while that one is listed too; otherwise where
   * they always were.
   */
  const shownIds = new Set(shownConversations.map((mission) => mission.missionId))
  const childrenOf = new Map<string, SidebarMission[]>()
  for (const mission of shownConversations) {
    if (mission.nestedUnder === undefined || !shownIds.has(mission.nestedUnder)) continue
    childrenOf.set(mission.nestedUnder, [...(childrenOf.get(mission.nestedUnder) ?? []), mission])
  }
  const isNestedChild = (mission: SidebarMission): boolean => mission.nestedUnder !== undefined && shownIds.has(mission.nestedUnder)
  const rowAndChildren = (mission: SidebarMission): ReactElement => (
    <Fragment key={mission.missionId}>
      {conversationRow(mission)}
      {(childrenOf.get(mission.missionId) ?? []).map((child) => (
        <div className="lc-convnest" key={child.missionId}>
          {conversationRow(child)}
        </div>
      ))}
    </Fragment>
  )
  const conversationList = (allInFolder: readonly SidebarMission[], keepsEmpty: boolean): ReactElement => {
    /*
     * PINNED, AT THE TOP (0.729): what the person pinned, newest pin first, above projects and the rest, as
     * Claude's own list keeps its starred conversations -- and not listed again below.
     */
    const pinRank = (mission: SidebarMission): number => pinnedKeys.indexOf(mission.rootId ?? mission.missionId)
    const pinned = allInFolder.filter((mission) => !isNestedChild(mission) && pinRank(mission) >= 0).sort((left, right) => pinRank(left) - pinRank(right))
    const inFolder = allInFolder.filter((mission) => !isNestedChild(mission) && pinRank(mission) < 0)
    /*
     * PROJECTS, AS CLAUDE HAS THEM (Colin, 2026-09-29: "claude has
     * projects/and workspace folders, like they arent the same thing so
     * just use that setup"). A project is what the person files a
     * conversation under -- the groups they made while Locust was held to
     * one folder -- drawn with the folder look he chose ("its beautiful").
     * The folder a conversation WORKS in is the chat box's chip, and a
     * project holds conversations from any folder.
     */
    const drawnGroups = groups
    const folderUngrouped = withRoomsFolded(inFolder.filter((mission) => heldFor(mission, groupMembers) === undefined), rooms)
    /*
     * The long part of the list, drawn a page at a time (ListWindow.tsx) -- always far enough to hold the one open,
     * so the conversation on screen is never the row that is missing.
     */
    const openAt = folderUngrouped.findIndex((entry) =>
      entry.kind === 'room' ? entry.missions.some((mission) => isShown(mission, selectedMissionId)) : isShown(entry.mission, selectedMissionId)
    )
    const drawnUngrouped = folderUngrouped.slice(0, Math.max(listBudget, openAt + 1))
    const notDrawn = folderUngrouped.length - drawnUngrouped.length
    // "Ungrouped" only where a group is drawn beside it: in a folder with none it names the only thing there.
    const groupsShown = drawnGroups.some((group) => showsGroup(group.groupId, inFolder, keepsEmpty))
    return (
          <>
            {pinned.length > 0 && (
              <div className="lc-convgroup">
                <div className="lc-project__head is-plain">
                  <span className="lc-project__name">Pinned</span>
                </div>
                {pinned.map(rowAndChildren)}
              </div>
            )}
            {drawnGroups.filter((group) => showsGroup(group.groupId, inFolder, keepsEmpty)).map((group) => {
              const theirs = inFolder.filter(
                (mission) => heldFor(mission, groupMembers)?.groupId === group.groupId
              )
              const open = !foldedGroups.has(group.groupId)
              return (
                <div className="lc-convgroup" key={group.groupId}>
                  <div className="lc-convgroup__head">
                    {renamingGroupId === group.groupId ? (
                      /* The same shape a conversation rename uses: Enter
                         commits, Escape abandons, blur commits too. */
                      <input
                        className="lc-input lc-convgroup__rename"
                        defaultValue={group.name}
                        maxLength={60}
                        aria-label="Project name"
                        autoFocus
                        onKeyDown={(event) => {
                          if (event.key === 'Escape') onGroupRenameDone?.()
                          if (event.key === 'Enter') {
                            const next = event.currentTarget.value
                            onGroupRenameDone?.()
                            if (next.trim().length > 0 && next.trim() !== group.name) {
                              onRenameGroup?.(group.groupId, next)
                            }
                          }
                        }}
                        onBlur={(event) => {
                          const next = event.currentTarget.value
                          onGroupRenameDone?.()
                          if (next.trim().length > 0 && next.trim() !== group.name) {
                            onRenameGroup?.(group.groupId, next)
                          }
                        }}
                      />
                    ) : (
                    <button
                      type="button"
                      className={`lc-project__head${open ? ' is-open' : ''}`}
                      aria-expanded={open}
                      title={`${group.name}: a project. Click to fold or unfold it.`}
                      onClick={() =>
                        setFoldedGroups((current) => {
                          const next = new Set(current)
                          if (next.has(group.groupId)) next.delete(group.groupId)
                          else next.add(group.groupId)
                          return next
                        })
                      }
                    >
                      {/* A name, no icon, no count: Claude's sidebar, from Colin's screenshot of it
                          (2026-09-30: "remove the random numbers, literally set it up exactly how
                          claude code has their sidebar"). */}
                      <span className="lc-project__name">{group.name}</span>
                    </button>
                    )}
                    {onGroupMenu !== undefined && renamingGroupId !== group.groupId && (
                      <button
                        type="button"
                        className="lc-convgroup__menu"
                        aria-label={`Actions for ${group.name}`}
                        title="Rename or remove this project"
                        onClick={(event) => {
                          event.stopPropagation()
                          const box = event.currentTarget.getBoundingClientRect()
                          onGroupMenu(group.groupId, { x: box.right, y: box.bottom })
                        }}
                      >
                        <Icon name="dots" size={13} />
                      </button>
                    )}
                  </div>
                  {/*
                    * An empty group says what to do with it. It is the one
                    * container in this app worth drawing empty, because the
                    * person made it deliberately a moment ago and an empty
                    * fold would read as a mistake.
                    */}
                  {open && theirs.length === 0 && (
                    <p className="lc-convgroup__empty lc-row__meta">Nothing in here yet.</p>
                  )}
                  {open && theirs.map(rowAndChildren)}
                </div>
              )
            })}
            {/*
              * Ungrouped carries a heading only when a group exists to be
              * ungrouped FROM. With no groups at all this is the whole
              * sidebar and a label over it would name the only thing there.
              */}
            {/* Under Pinned with no projects beside it, the rest is Recents, as Claude's list names it (0.729). */}
            {(groupsShown || pinned.length > 0) && folderUngrouped.length > 0 && (
              <div className="lc-project__head is-plain">
                <span className="lc-project__name">{groupsShown ? 'Ungrouped' : 'Recents'}</span>
              </div>
            )}
            {drawnUngrouped.map((entry) => (entry.kind === 'room' ? roomRow(entry.room, entry.missions) : rowAndChildren(entry.mission)))}
            {notDrawn > 0 && <ListMore remaining={notDrawn} onMore={() => setListBudget((held) => held + LIST_PAGE)} />}
          </>
    )
  }
  const unowned = missions.filter((mission) => (mission.ownerId ?? missionOwners[mission.missionId]) === undefined)
  const shownUnowned = missionsMatching(unowned, query)
  /*
   * One row, drawn the same wherever it sits.
   *
   * Grouped and ungrouped conversations are the SAME thing in different
   * places, so they must not be two pieces of markup that drift -- the way
   * a conversation used to read one way nested under a teammate and another
   * way in the Missions section.
   */
  /**
   * A room, standing in the list for the conversations its posts started
   * (`withRoomsFolded`): its name, where its newest answer was, and a press
   * opens the room. The people mark keeps the face's footprint, so the title
   * starts on the same x as every other row.
   */
  const roomRow = (room: PublicRoom, answers: readonly SidebarMission[]): ReactElement => {
    const running = answers.some((mission) => mission.phase === 'running')
    const members = room.teammateIds
      .map((id) => teammates.find((entry) => entry.teammateId === id)?.name)
      .filter((name): name is string => name !== undefined)
    const here = currentRoomId === room.roomId
    return (
      <div className="lc-convrow" key={`room:${room.roomId}`}>
        <button
          type="button"
          className={`lc-conv lc-conv--room${here ? ' is-active' : ''}`}
          title={`${room.name}: a room with ${members.join(' and ')}`}
          aria-current={here ? 'true' : undefined}
          onClick={() => onOpenRoom(room.roomId)}
        >
          {running ? <span className="lc-conv__working" aria-hidden="true" data-orb="shimmer" /> : <span className="lc-dot lc-tone-blue is-quiet" />}
          <span className="lc-conv__room" aria-hidden="true">
            <Icon name="users" size={13} />
          </span>
          <span className={`lc-conv__title${running ? ' is-working' : ''}`}>{room.name}</span>
        </button>
      </div>
    )
  }

  const conversationRow = (mission: SidebarMission): ReactElement => {
              const owner = ownerOf(mission, missionOwners)
              const by = owner === undefined ? undefined : teammates.find((entry) => entry.teammateId === owner)
              /*
               * WHOSE, AND WHETHER A ROUTINE STARTED IT.
               *
               * Two rows with one title are one question asked twice -- by
               * two teammates, or by a person and then by the routine they
               * saved from it. The face says whose at a glance, and now says
               * it to a screen reader and on hover too; a routine's run
               * carries the Routines clock, so the replay is not mistaken for
               * the conversation it came from (0.271 design recheck, finding
               * 2). The routine's name is in the hover; a deleted routine is
               * still "a routine".
               */
              const routineName = routines.find((entry) => entry.routineId === mission.routineId)?.name
              const fromRoutine =
                mission.routineId === undefined
                  ? undefined
                  : routineName === undefined
                    ? 'from a routine'
                    : `from the routine ${routineName}`
              const hover = [mission.title, ...(by === undefined ? [] : [by.name]), ...(fromRoutine === undefined ? [] : [fromRoutine])].join(' · ')
              // The step a routine is on, when this is its conversation and it is running.
              const runningStep =
                mission.phase === 'running' && mission.routineId !== undefined && by !== undefined
                  ? routineStepByTeammate[by.teammateId]
                  : undefined
              /*
               * A SUBAGENT AT WORK, said on the conversation that sent it.
               *
               * Yurt's beta report (#17): in a Claude subagent run the sidebar
               * never said a subagent was working -- only the fold, afterwards,
               * said "asked 1 subagent". A subagent is part of its teammate's
               * turn, not a conversation of its own, so it does not get a row;
               * its parent's row says so while it works (Colin: "I'll let you
               * choose design choice").
               */
              const delegating = mission.phase === 'running' && by !== undefined && liveActivity[by.teammateId] === 'delegating'
              return (
                <div className="lc-convrow" key={mission.missionId}>
                  {/*
                    * Renaming replaces the row rather than sitting on top of
                    * it, the same shape the room header already uses: Enter
                    * commits, Escape abandons, and losing focus commits too,
                    * because a half-typed name left on screen with no way to
                    * finish is worse than either.
                    *
                    * The starting value is the name on screen. Clearing it
                    * and pressing Enter is how you get the typed sentence
                    * back -- nothing was ever overwritten to lose.
                    */}
                  {renamingMissionId !== undefined
                  && (mission.memberIds ?? [mission.missionId]).includes(renamingMissionId) ? (
                    <input
                      className="lc-input lc-conv__rename"
                      defaultValue={mission.title}
                      maxLength={120}
                      aria-label="Name this conversation"
                      autoFocus
                      onKeyDown={(event) => {
                        if (event.key === 'Escape') onRenameDone?.()
                        if (event.key === 'Enter') {
                          const next = event.currentTarget.value
                          onRenameDone?.()
                          if (next.trim() !== mission.title) onRenameMission?.(mission.missionId, next)
                        }
                      }}
                      onBlur={(event) => {
                        const next = event.currentTarget.value
                        onRenameDone?.()
                        if (next.trim() !== mission.title) onRenameMission?.(mission.missionId, next)
                      }}
                    />
                  ) : (
                  <button
                    type="button"
                    className={`lc-conv${isShown(mission, selectedMissionId) ? ' is-active' : ''}`}
                    title={hover}
                    // Alt+Down / Alt+Up walk the rows in the order they are drawn (conversationSteps.ts).
                    data-mission-id={mission.missionId}
                    aria-current={isShown(mission, selectedMissionId) ? 'true' : undefined}
                    onContextMenu={(event) => {
                      event.preventDefault()
                      onMissionMenu(mission.missionId, { x: event.clientX, y: event.clientY })
                    }}
                    onClick={() => onSelectMission(mission.missionId)}
                  >
                    {/*
                      * A dot only where there is something to say.
                      *
                      * This draws the mission's PHASE: lime running, red
                      * failed or interrupted, amber finished with an
                      * incomplete record, muted cancelled -- and blue for
                      * completed, which is almost every row almost all of
                      * the time.
                      *
                      * A coloured dot at the left edge of a list row is the
                      * universal unread mark, so a permanent blue one reads
                      * as a notification that will not clear. Colin,
                      * 2026-09-15, doing exactly that: "correct me if im
                      * wrong but this blue dot would be a notification, but
                      * it stays there when i click on the convo." The
                      * reading was right; the meaning was not.
                      *
                      * So the ordinary outcome goes quiet and the dot keeps
                      * its footprint, which holds every title on one x. What
                      * is left is true: a mark here means this one wants
                      * something.
                      */}
                    {/*
                      * A SPARK WHILE IT RUNS, A QUIET DOT WHEN IT IS DONE.
                      *
                      * Colin, 2026-09-20: *"instead of this dot next to the
                      * face we can just use a smaller orb to show that they
                      * are working that turns into a white/gray dot like
                      * claude code when done."* The orb he picked then, the
                      * dotted outline morphing circle to square, gave way on
                      * 2026-09-27 to Claude Code's own spinner -- "i think the
                      * one i suggested is getting outdated" (WorkingSpark.tsx).
                      *
                      * `working` rather than the live line's own state, on
                      * purpose. This row is a list entry for a whole
                      * conversation, not a report on the current tool: it
                      * claims only "this one is going", which is exactly what
                      * the spark says. Reaching for the specific state would
                      * make a sidebar row assert something it then has to
                      * keep up with.
                      *
                      * Every other phase keeps the dot, because a finished
                      * conversation is a state rather than an activity — and
                      * the tone still carries whether it ended well.
                      */}
                    {/*
                      * A SHIMMER, NOT A SPINNER (0.839). Colin, 2026-10-28, of Claude Code's new sidebar: "this rolling
                      * shimmer to show that the convo/chat is working ... its like a monochrome gradient ... i have lots
                      * of chats going now and its too distracting". With many running, a turning glyph on every row
                      * pulled the eye down the list; now a running row keeps the dot's footprint, empty, and a light
                      * band passes slowly through its title, as Claude Code's does.
                      */}
                    {mission.phase === 'running' ? (
                      <span className="lc-conv__working" aria-hidden="true" data-orb="shimmer" />
                    ) : (
                      <span
                        className={`lc-dot lc-tone-${missionPhaseView(mission.phase, mission.integrityIssueCount > 0).tone}${
                          missionPhaseView(mission.phase, mission.integrityIssueCount > 0).tone === 'blue' ? ' is-quiet' : ''
                        }`}
                      />
                    )}
                    {/*
                      * The face answers "which teammate" and costs no words.
                      * It is the same glyph as the roster above and the
                      * workroom header, so it identifies rather than labels
                      * -- which is what let the route line go.
                      */}
                    {/*
                      * No teammate: the model's own mark in the face's place,
                      * not an empty box (Colin, 10/02: "if a teammate isnt
                      * assigned we can just use the logo for whatever model is
                      * chosen"). A model of your own has no mark; it keeps the box.
                      */}
                    {by === undefined ? (
                      mission.runtime === undefined || mission.model === undefined || isOwnRoute(mission.model) ? (
                        <span className="lc-conv__nobody" aria-hidden="true" />
                      ) : (
                        <span className="lc-conv__runtime">
                          <RuntimeMark runtime={mission.runtime} size={13} label={runtimeDisplayName(mission.runtime)} />
                        </span>
                      )
                    ) : (
                      // Drawn when the row comes near the screen: a face is most of what a row costs (ListWindow.tsx).
                      <WhenNear size={BOT_SIZE.conversationOwner}>
                        <TeammateBot hue={by.hue} avatar={by.avatar} size={BOT_SIZE.conversationOwner} teammateId={by.teammateId} name={by.name} />
                      </WhenNear>
                    )}
                    {fromRoutine !== undefined && (
                      <span className="lc-conv__routine" role="img" aria-label={`${fromRoutine.charAt(0).toUpperCase()}${fromRoutine.slice(1)}`}>
                        <Icon name="clock" size={11} />
                      </span>
                    )}
                    {mission.compareId !== undefined && <span className="lc-conv__vs lc-mono" aria-label="A comparison">vs</span>}
                    <span className={`lc-conv__title${mission.phase === 'running' ? ' is-working' : ''}`}>{mission.title}</span>
                    {/*
                      * A routine's conversation says which step it is on
                      * while one runs, where its age would be -- the clock
                      * before the title already says it is a routine.
                      */}
                    {(mission.memberIds ?? [mission.missionId]).some((id) => onCall.has(id)) ? (
                      <span className="lc-conv__age lc-mono lc-conv__oncall"><Icon name="waveform" size={11} />on a call</span>
                    ) : delegating ? (
                      <span className="lc-conv__age lc-mono">subagent working</span>
                    ) : runningStep !== undefined ? (
                      <span className="lc-conv__age lc-mono" title={`${runningStep.name}: step ${String(runningStep.step)} of ${String(runningStep.of)}`}>
                        step {runningStep.step} of {runningStep.of}
                      </span>
                    ) : null}
                  </button>
                  )}
                  <button
                    type="button"
                    className="lc-teammate__missionmenu"
                    aria-label={`Actions for ${mission.title}`}
                    title="Actions — including Save as routine"
                    onClick={(event) => {
                      event.stopPropagation()
                      const box = event.currentTarget.getBoundingClientRect()
                      onMissionMenu(mission.missionId, { x: box.right, y: box.bottom })
                    }}
                  >
                    <Icon name="dots" size={13} />
                  </button>
                </div>
              )
              }

  return (
    <nav className="lc-sidebar" aria-label="Workspace">
      <div className="lc-sidebar__brand">
        {/* The logo is the way back to the home screen (Colin, 2026-09-05). */}
        <button type="button" className="lc-brand__lockup" onClick={onHome} title="Home" aria-label="Home">
          <img className="lc-brand__mark" src={mark} alt="" aria-hidden="true" />
          <img className="lc-brand__wordmark" src={wordmark} alt="Locust" />
        </button>
        {/*
          * ONE `+`, and it says what it would add: New teammate, New room,
          * New group (Colin, 2026-09-14: "consolidate room and teammate add
          * into one +, looks clunky").
          *
          * DRAWN AS THE RIGHT-CLICK MENUS ARE (0.311). Colin, 2026-09-24:
          * "make the + button for new teammate and group the same style as
          * our right click dropdowns, those are way cleaner". It was its own
          * `lc-menu` dropdown, with its own ways of closing and its own fixes
          * for hanging out of the 64px rail; now it is the one menu the rows
          * use (ContextMenu, drawn at the window, kept inside it, with keys),
          * opened from here and hung from this button.
          */}
        <span className="lc-control__anchor lc-sidebar__add">
          <button
            type="button"
            className="lc-iconbutton"
            aria-label="Add"
            title="Add"
            aria-haspopup="menu"
            aria-expanded={addMenuOpen}
            onClick={(event) => onAddMenu(event.currentTarget)}
          >
            <Icon name="plus" size={14} />
          </button>
        </span>
      </div>

      <div className="lc-search">
        <Icon name="search" size={13} />
        <input
          type="text"
          /*
           * CONVERSATIONS, because that is what this list holds.
           *
           * Astra's acceptance pass, 2026-09-14, finding 2: typing here left
           * the screen titled Missions showing all twelve, while the sidebar
           * said "No missions match that". Two surfaces, one word, one of
           * them not responding -- and the reasonable conclusion from the
           * page you are looking at is that search is broken.
           *
           * The sidebar collapses missions into conversations; the Missions
           * screen lists missions. Naming each for what it holds is the
           * smaller and truer fix, and the different noun is itself the
           * signal that the scopes differ. Sharing one query between them is
           * a product decision, not a wording repair.
           */
          placeholder="Search conversations"
          aria-label="Search conversations"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          // Escape empties the search, as it does Claude Code's prompt; an
          // empty box lets it through to whatever else Escape closes (0.414).
          onKeyDown={(event) => {
            if (event.key !== 'Escape' || query.length === 0) return
            event.preventDefault()
            event.stopPropagation()
            setQuery('')
          }}
          autoComplete="off"
        />
      </div>

      {/*
        * THE PLACES, AT THE TOP, one to a line -- Claude's sidebar, from
        * Colin's screenshot of it (2026-09-30): New, Artifacts, Routines,
        * Customize stacked under its switcher, the conversations below. They
        * were a row of thirds in the footer. `lc-sidebar__nav` stays on the
        * list for everything that finds these buttons by it.
        */}
      {/*
        * ROWS, WITH COUNTS (0.605). Colin, 2026-10-04, on the two-by-two grid
        * these had become: rows, each with how many it holds, and Board down
        * in the bottom bar beside Settings -- it is a view of the
        * conversations, not a fourth kind of place. A place with nothing in
        * it carries no number; "0" beside "Routines" is a nag.
        */}
      <div className="lc-sidebar__nav lc-sidebar__places">
        {/*
          * NO COUNTS (0.609). 0.605 gave each place its count, as rows; the rows
          * were "too much dead space" (Colin, 2026-10-04). In one row the counts
          * do not fit beside the words (every word was cut) and on the icon's
          * corner the "14" ran into the "C" of Conversations (the frames of this
          * change). The list below already shows what each holds; the row stays
          * three words.
          */}
        <button type="button" onClick={() => { railClose(); onOpenMissions() }} title="All conversations (Ctrl 1)">
          <Icon name="inbox" size={14} />
          <span>Conversations</span>
        </button>
        <button type="button" onClick={() => { railClose(); onOpenRooms() }} title="Rooms — ask several teammates at once (Ctrl 4)">
          <Icon name="users" size={14} />
          <span>Rooms</span>
        </button>
        <button type="button" onClick={() => { railClose(); onOpenAutomations() }} title="Routines — work that repeats">
          <Icon name="clock" size={14} />
          <span>Routines</span>
        </button>
      </div>
      {cloudTasks !== undefined && (
        <div className="lc-sidebar__nav lc-sidebar__places lc-sidebar__places--pinned">
          <button type="button" onClick={() => { railClose(); cloudTasks.onOpen() }} title="Cloud tasks -- what Codex Cloud is doing, or did, for this folder">
            <Icon name="cloud" size={14} />
            <span>Cloud tasks</span>
            <span className="lc-sidebar__count">{String(cloudTasks.count)}</span>
          </button>
        </div>
      )}
      {background !== undefined && (
        <div className="lc-sidebar__nav lc-sidebar__places lc-sidebar__places--pinned">
          <button type="button" onClick={() => { railClose(); background.onOpen() }} title="In the background -- Claude turns that keep going if you close Locust">
            <Icon name="clock" size={14} />
            <span>In the background</span>
            <span className="lc-sidebar__count">{String(background.count)}</span>
          </button>
        </div>
      )}
      {/* Your benchmarks (0.764): only once a comparison has been run again -- an empty place is a nag. */}
      {benchmarks !== undefined && (
        <div className="lc-sidebar__nav lc-sidebar__places lc-sidebar__places--pinned">
          <button type="button" onClick={() => { railClose(); benchmarks.onOpen() }} title="Benchmarks -- every comparison you ran again, and which model leads it">
            <Icon name="target" size={14} />
            <span>Benchmarks</span>
            <span className="lc-sidebar__count">{String(benchmarks.count)}</span>
          </button>
        </div>
      )}

      {/*
        * The roster, one row tall.
        *
        * It used to be 200px of the scroll: four teammates each spending
        * three lines on themselves -- name, role, and a route line truncated
        * in every one of them -- above the conversations people actually came
        * for. The face is the whole identity; the route line went to the Team
        * roster, which is where comparing models is the task.
        */}
      {!compact && teammates.length > 0 && (
        <div className="lc-faces">
          {shownFaces.map((teammate) => {
            /*
             * ON means "this is who you are with": the teammate the composer
             * addresses, whose conversation is open. It used to mean "the
             * list is filtered to them", and the click did both -- picked
             * the teammate AND narrowed the list -- which left no click for
             * the thing a face most obviously does: open their conversation.
             *
             * Colin, 2026-09-21: "We already have those avatars at the top
             * for the teammates, we can just make those clickable, to go to
             * their hub chat where all those teammate responses go to." So
             * the click opens the hub, and the filter moved into the card
             * that opens on hover, where the list of their conversations
             * already is.
             */
            const on = selectedTeammateId === teammate.teammateId
            const status = viewByTeammate[teammate.teammateId]
            return (
              <button
                key={teammate.teammateId}
                type="button"
                className={`lc-faces__one${on ? ' is-on' : ''}${faceFilter === teammate.teammateId ? ' is-filtering' : ''}`}
                aria-pressed={on}
                // No native `title`: the card that opens on hover says all of
                // this laid out, and a tooltip drawing the same facts as one
                // unbroken line underneath it is the worse of two answers.
                // The label stays for anyone who cannot see either.
                aria-label={`${teammate.name} — open their conversation`}
                // What the bot says by moving, in words. A description, not
                // part of the name: the name is what pressing it does, and
                // the drives find a face by it (drive-lib's teammateFace).
                aria-description={status === undefined ? undefined : stateInWords(status)}
                ref={(node) => {
                  if (node === null) railSlots.current.delete(teammate.teammateId)
                  else railSlots.current.set(teammate.teammateId, node)
                }}
                onMouseEnter={() => railEnter(teammate.teammateId)}
                onMouseLeave={railLeave}
                // Keyboard reaches it too: tabbing to a face is the same
                // question as pointing at one.
                onFocus={() => railEnter(teammate.teammateId)}
                onBlur={railLeave}
                onClick={() => onOpenHub(teammate.teammateId)}
                onContextMenu={(event) => {
                  event.preventDefault()
                  onTeammateMenu(teammate.teammateId, { x: event.clientX, y: event.clientY })
                }}
              >
                {/*
                  * The face MOVES again.
                  *
                  * The old roster row passed `activity` and `presence`
                  * through, which is what made a working teammate's face
                  * animate and a signed-out one look absent. The faces row
                  * was built without them, so the whole roster went still --
                  * Colin, 2026-09-15: "the teammates face used to also be
                  * animated on the sidebar, lets bring that back as well."
                  *
                  * It is the same component and the same state the rest of
                  * the app reads, so a face here cannot disagree with the
                  * same teammate's face in the workroom header.
                  */}
                <TeammateBot
                  hue={teammate.hue}
                  avatar={teammate.avatar}
                  size={BOT_SIZE.sidebarFaces}
                  teammateId={teammate.teammateId}
                  // Which model is who, on the one surface that names nobody's
                  // route in words (0.383) -- not on a model of their own.
                  {...(teammate.route === undefined || isOwnRoute(teammate.route.model) ? {} : { runtime: teammate.route.runtime })}
                  {...(status === undefined ? {} : { activity: status.activity, presence: facePresenceFor(status.status) })}
                  {...(stripGlances.has(teammate.teammateId) ? { glance: stripGlances.get(teammate.teammateId) } : {})}
                />
                {status !== undefined && (
                  <span className={`lc-faces__pip lc-tone-${status.tone}`} aria-hidden="true" />
                )}
              </button>
            )
          })}
          {/* What the six do not show, and the way to it. Counted rather
              than hidden: a strip that silently stops at six is one that
              lies about how many people are on the team. */}
          {restOfTeam > 0 && (
            <button
              type="button"
              className="lc-faces__more"
              onClick={onOpenTeammates}
              title={`${String(restOfTeam)} more on the team — open the roster`}
            >
              +{restOfTeam}
            </button>
          )}
          <button type="button" className="lc-faces__team" aria-current={screenKey === 'teammates' ? 'page' : undefined} onClick={onOpenTeammates} title="Team (Ctrl 2)">
            <Icon name="users" size={13} />
            <span>Team</span>
          </button>
        </div>
      )}
      {/* What the face filter is doing, in words, because a filtered list
        * that does not say it is filtered reads as a list that lost things. */}
      {/*
        * ONE sentence for everything narrowing the list.
        *
        * There are two filters -- a face and the search box -- and they
        * compose, so with both on there were two reasons the list was short
        * and only one of them was stated. The design agent's shape, and the
        * reason for it: "there's no way to be filtered without seeing why".
        *
        * It also carries the arithmetic. `3 of 14` is what tells you the
        * list is filtered rather than empty, which is the difference between
        * "I have no conversations" and "none of mine match this word".
        */}
      {!compact && narrowing !== undefined && (
        <div className="lc-faces__clear">
          <span className="lc-faces__clearsaid">{narrowing}</span>
          <button
            type="button"
            className="lc-faces__clearlink"
            onClick={() => {
              setFaceFilter(undefined)
              setQuery('')
            }}
          >
            Clear
          </button>
        </div>
      )}

      <div className="lc-sidebar__scroll">
        {/*
          * TWO LAYOUTS, because 268px and 64px are different problems.
          *
          * Wide, the list IS the sidebar: conversations flat and newest
          * first, with the teammate as a face on the row rather than a
          * folder around it. Measured on a light week -- 330px spent before
          * the first conversation became 0px, and 7 of 14 visible became 14.
          *
          * The rail is left exactly as it was, and deliberately. Four pixels
          * of a title is not a smaller list, it is a decoration that lies
          * about being one, so at 64px the avatar remains the teammate and
          * their conversations open in the flyout. Grok drove that rail on
          * 2026-09-15 and it holds; the measured win here is entirely in the
          * wide layout, and rebuilding a working rail to match a change it
          * does not share is how the last rail attempt broke.
          */}
        {compact ? (
          <>
          {/*
            * Rooms sit above the roster: a room is where several teammates
            * are written to at once, so it reads before any one of them. Only
            * DRAWN EVEN WITH NO ROOMS, which is the opposite of what this
            * comment used to say. It said the way in is the Rooms screen, "one
            * palette entry or Ctrl 4 away, so an empty section has nothing to
            * say here" -- and that is exactly backwards. An empty section's
            * whole job is to say the feature exists.
            *
            * The result was a feature nobody could reach without already having
            * used it: rooms appeared in the sidebar only once you had made one,
            * and the only ways to make a first one were a keyboard shortcut and
            * a palette entry. Colin, who owns the app and had watched a day of
            * work go into rooms, 2026-09-09: "sorry if this is dumb but how
            * does one create a room for teammates, i cant figure it out lol."
            * Not dumb -- there was nothing on screen to find.
            */}
          {/*
            * The same section as its neighbours: a fold and a count. It was a
            * bare label over the rows, so TEAMMATES 3 and ROUTINES 0 folded and
            * counted while ROOMS did neither (design agent, 2026-09-10).
            * Always drawn and foldable are not in tension -- the section is
            * always THERE; whether it is open is the person's.
            */}
          <SidebarSection
            label="Rooms"
            count={rooms.length}
            open={openSections.rooms}
            onToggle={() => setOpenSections((current) => ({ ...current, rooms: !current.rooms }))}
          >
            <>
              {rooms.map((room) => (
                <button
                  key={room.roomId}
                  type="button"
                  className={`lc-row lc-row--button lc-roomrow${currentRoomId === room.roomId ? ' is-selected' : ''}`}
                  aria-current={currentRoomId === room.roomId ? 'true' : undefined}
                  title={`Open ${room.name}`}
                  onClick={() => { railClose(); onOpenRoom(room.roomId) }}
                >
                  <Icon name="users" size={14} />
                  <span className="lc-row__text">
                    <span className="lc-row__name">{room.name}</span>
                    <span className="lc-row__meta">
                      {String(room.teammateIds.length)} teammate{room.teammateIds.length === 1 ? '' : 's'}
                      {room.posts.length > 0 ? ` · ${String(room.posts.length)} post${room.posts.length === 1 ? '' : 's'}` : ''}
                    </span>
                  </span>
                </button>
              ))}
              {/*
                * Two words, and the sentence on hover. It read "New room — ask
                * several teammates at once", which the rail ellipsised mid-word
                * -- on the one sentence carrying a feature nobody could find
                * (design agent, 2026-09-10). Since 0.62.0 a room is made from
                * the ask, so this row is the way BACK to rooms, not the way in,
                * and it no longer has to carry the whole explanation.
                */}
              <button type="button" className="lc-row lc-row--button lc-roomrow lc-roomrow--new" onClick={onOpenRooms} title="New room — ask several teammates at once, or tick two names in the message box">
                <Icon name="plus" size={12} />
                <span className="lc-row__text">
                  <span className="lc-row__meta">New room</span>
                </span>
              </button>
            </>
          </SidebarSection>
          <SidebarSection
            label="Teammates"
            count={teammates.length}
            open={openSections.teammates}
            onToggle={() => setOpenSections((current) => ({ ...current, teammates: !current.teammates }))}
          >
          {teammates.map((teammate) => {
            const owned = missions.filter(
              (mission) => (mission.ownerId ?? missionOwners[mission.missionId]) === teammate.teammateId
            )
            // What this teammate's rows show while a search is running. Their
            // status still comes from ALL their work: a teammate does not stop
            // working because someone typed in a box.
            const shownOwned = missionsMatching(owned, query, ownerName)
            // The runtime this teammate's own work is on, or is set to use.
            // Asking about Codex for everyone told a person their teammate
            // needed a sign-in while she was visibly working on Claude Code;
            // asking only their MISSIONS said "idle" for a teammate that had
            // never run and could not. See `runtimeOfTeammate`.
            /*
             * One answer per teammate, decided in App and handed down.
             *
             * This row used to call `teammateStatusView` itself with inputs
             * that had drifted from the ones the Team roster's map used -- so
             * the same teammate read "working" here and idle there, which Grok
             * caught on screen in 0.116.0. Nothing is computed here now.
             */
            const status = viewByTeammate[teammate.teammateId] ?? teammateStatusView({
              runtime: undefined,
              ...runtimeReach(runtimes, runtimes.length > 0),
              hasRunningMission: false,
              pendingApprovals: 0,
              roleLabel: roleLabelOf(teammate)
            })
            const selected = teammate.teammateId === selectedTeammateId
            return (
              <div
                key={teammate.teammateId}
                className={`lc-teammate lc-railslot${selected ? ' is-selected' : ''}`}
                ref={(node) => {
                  if (node === null) railSlots.current.delete(teammate.teammateId)
                  else railSlots.current.set(teammate.teammateId, node)
                }}
                onMouseEnter={() => railEnter(teammate.teammateId)}
                onMouseLeave={railLeave}
              >
                {/* The rail's one new mark: how many conversations sit behind
                    this face, drawn only past one. Hidden outside the rail. */}
                {railCountBadge(owned.length) !== undefined && (
                  <span className="lc-railbadge" aria-hidden="true">{railCountBadge(owned.length)}</span>
                )}
                {/*
                  * The hover says name, role AND model -- not just the name. In
                  * the compact rail all three are drawn as text and the rail
                  * hides text, so an avatar there was a coloured square with no
                  * way to find out whose it was (Colin, 2026-09-07).
                  */}
                <button
                  type="button"
                  className="lc-row lc-row--button"
                  aria-current={selected ? 'true' : undefined}
                  // No native title in the rail: the flyout says the same three
                  // facts as real text, and a tooltip fading in over a panel is
                  // two answers to one question. The full sidebar keeps it.
                  // The rail hides the row's words, the state among them, so
                  // a screen reader is told it here; the full row says it.
                  {...(compact
                    ? { 'aria-label': teammateTooltip(teammate), 'aria-description': stateInWords(status) }
                    : { title: teammateTooltip(teammate) })}
                  aria-expanded={compact ? railOpenFor === teammate.teammateId : undefined}
                  onClick={() => {
                    onSelectTeammate(teammate.teammateId)
                    railPin(teammate.teammateId)
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    onTeammateMenu(teammate.teammateId, { x: event.clientX, y: event.clientY })
                  }}
                >
                  <TeammateBot
                    hue={teammate.hue}
                    avatar={teammate.avatar}
                    size={BOT_SIZE.railRow}
                    activity={status.activity}
                    presence={facePresenceFor(status.status)}
                    teammateId={teammate.teammateId}
                    {...(rosterGlances.has(teammate.teammateId) ? { glance: rosterGlances.get(teammate.teammateId) } : {})}
                  />
                  <span className="lc-row__text">
                    <span className="lc-row__name">
                      {teammate.name}
                      {status.pulse && <span className={`lc-dot is-pulsing lc-tone-${status.tone} lc-namedot`} />}
                    </span>
                    {status.activity === 'delegating' ? (
                      // "subagent working" is the news; the role gives way to it
                      // when the line is short, instead of the other way round.
                      <span className="lc-row__meta is-delegating">
                        {/* One glyph app-wide for "there is another agent in this" -- the same one the helper row uses. */}
                        <span className="lc-teammate__delegating" aria-hidden="true">
                          <Icon name="users" size={11} />
                        </span>
                        {/* The role stays muted here too: it is identity and has no
                            state to borrow. This branch tinted the whole line while
                            the ordinary one had already been split (RULINGS 2026-09-10). */}
                        <span className="lc-row__metarole">{labelRole(status.label)}</span>
                        <span className={`lc-row__metastate lc-tone-${status.tone === 'muted' ? 'muted' : status.tone}`}>
                          {labelState(status.label)}
                        </span>
                      </span>
                    ) : (
                      /*
                       * The role in muted, the STATE in the tone -- never the
                       * whole line. "Finance Bro · thinking" was lime end to
                       * end, so the role was wearing the state's colour and a
                       * reader could not tell which word the lime was about
                       * (design agent, 2026-09-10). The delegating branch above
                       * already split it this way; the ordinary line now does
                       * too, through the same two helpers.
                       */
                      <span className="lc-row__meta">
                        <span className="lc-row__metarole">{labelRole(status.label)}</span>
                        <span className={`lc-row__metastate lc-tone-${status.tone === 'muted' ? 'muted' : status.tone}`}>
                          {labelState(status.label)}
                        </span>
                      </span>
                    )}
                    {/*
                      * Which step of which routine is running. Derived in the
                      * shell from the live runs, so it disappears when the work
                      * does rather than being cleared by hand.
                      */}
                    {teammate.route !== undefined && (
                      // Which model this teammate IS. People pit models against
                      // each other on purpose, and that only reads if each row
                      // says who is who without opening a thread. This line
                      // earns its place; the two below it were one line's worth
                      // of fact spread over two.
                      <span
                        className="lc-row__route lc-mono"
                        title={`${runtimeDisplayName(teammate.route.runtime)} / ${teammate.route.model}`}
                      >
                        {/*
                          * Spelled as a name, the same way the composer's chip
                          * spells it -- `Cursor / Grok 4.6`, not `Cursor Agent
                          * / cursor-grok-4.6-medium`, which says cursor twice
                          * and then spells a product in lowercase. The exact id
                          * is the tooltip. The runtime's mark goes first
                          * (0.383), except on a model of the person's own.
                          */}
                        {!isOwnRoute(teammate.route.model) && <RuntimeMark runtime={teammate.route.runtime} size={11} className="is-inline" />}
                        {routeChrome(teammate.route.runtime, teammate.route.model, routeModelName(teammate.route.runtime, teammate.route.model))}
                      </span>
                    )}
                    {/*
                      * ONE situational line, never two.
                      *
                      * The routine step and the worktree branch answer the same
                      * question -- where and how is this teammate working right
                      * now -- and the row was drawing both, so a teammate on its
                      * own branch replaying a routine stacked five lines in a
                      * 268px rail. Four teammates made it a wall of mono
                      * (design review, 2026-09-06).
                      *
                      * The routine step wins while one is running, because it is
                      * the thing that is changing; the branch is a standing
                      * fact and comes back when the routine finishes.
                      */}
                    {routineStepByTeammate[teammate.teammateId] !== undefined ? (
                      <span className="lc-row__route lc-mono" title={routineStepByTeammate[teammate.teammateId]!.name}>
                        {routineStepLabel(routineStepByTeammate[teammate.teammateId]!)}
                      </span>
                    ) : (
                      teammate.worktree === true && (
                        <span className="lc-row__route lc-mono" title="Works on its own branch, in its own worktree of the folder">
                          on {branchByTeammate[teammate.teammateId] ?? branchNameFor(teammate.name, teammate.teammateId)}
                        </span>
                      )
                    )}
                  </span>
                </button>
                {shownOwned.length > 0 && (
                  <div className="lc-teammate__missions">
                    {shownOwned.map((mission) => (
                      <div className="lc-teammate__missionrow" key={mission.missionId}>
                      <button
                        type="button"
                        className={`lc-teammate__mission${isShown(mission, selectedMissionId) ? ' is-active' : ''}`}
                        onContextMenu={(event) => {
                          event.preventDefault()
                          onMissionMenu(mission.missionId, { x: event.clientX, y: event.clientY })
                        }}
                        onClick={() => onSelectMission(mission.missionId)}
                      >
                        <span
                          className={`lc-dot lc-tone-${missionPhaseView(mission.phase, mission.integrityIssueCount > 0).tone}`}
                        />
                        <span className="lc-teammate__title">{mission.title}</span>
                        {(mission.turns ?? 1) > 1 && (
                          <span className="lc-teammate__turns lc-mono">{mission.turns}</span>
                        )}
                      </button>
                      {/*
                        * The same menu the right-click opens, with something to
                        * press. `Save as routine` lives in there and had no
                        * visible way in at all: a tester concluded routines did
                        * not exist (2026-09-07). Shown on hover and whenever it
                        * has focus, so it is reachable by keyboard and does not
                        * add a permanent object to every row.
                        */}
                      <button
                        type="button"
                        className="lc-teammate__missionmenu"
                        aria-label={`Actions for ${mission.title}`}
                        title="Actions — including Save as routine"
                        onClick={(event) => {
                          event.stopPropagation()
                          const box = event.currentTarget.getBoundingClientRect()
                          onMissionMenu(mission.missionId, { x: box.right, y: box.bottom })
                        }}
                      >
                        <Icon name="dots" size={13} />
                      </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
            })}
          </SidebarSection>

          {/*
            "Missions", not "Other missions". The old label drew a distinction
            that only makes sense from inside the code -- these are the ones no
            teammate owns -- and Colin read it the way anyone would: "just have
            it say missions lol, why other missions?"

            Drawn only when it holds something, which is the answer to his next
            question: are these two redundant? Not in content -- every
            conversation appears exactly once, under its teammate or here, and
            all of them stay on the sidebar either way. But once a person has
            teammates almost nothing is unowned, so this stood as a heading
            reading 0 for good. Automations keeps its empty holder because an
            empty Automations teaches what the app can do; an empty Missions
            teaches nothing.
          */}
          {shownUnowned.length > 0 && (
          <SidebarSection
            label="Conversations"
            count={shownUnowned.length}
            open={openSections.missions}
            onToggle={() => setOpenSections((current) => ({ ...current, missions: !current.missions }))}
          >
            <>
              {shownUnowned.map((mission) => {
                const view = missionPhaseView(mission.phase, mission.integrityIssueCount > 0)
                return (
                  <button
                    type="button"
                    key={mission.missionId}
                    className="lc-row lc-row--mission"
                    aria-current={isShown(mission, selectedMissionId)}
                    onClick={() => onSelectMission(mission.missionId)}
                    onContextMenu={(event) => {
                      event.preventDefault()
                      onMissionMenu(mission.missionId, { x: event.clientX, y: event.clientY })
                    }}
                  >
                    <span
                      className={`lc-dot lc-tone-${view.tone}${mission.phase === 'running' ? ' is-pulsing' : ''}`}
                    />
                    <span className="lc-row__text">
                      <span className="lc-row__name">{mission.title}</span>
                      <span className="lc-row__meta">
                        {view.label}
                        {(mission.turns ?? 1) > 1 && ` · ${String(mission.turns)} turns`} ·{' '}
                        <span className="lc-mono">{shortMissionId(mission.missionId)}</span>
                      </span>
                    </span>
                  </button>
                )
              })}
            </>
          </SidebarSection>
          )}

          <SidebarSection
            label="Routines"
            count={routines.length}
            open={openSections.automations}
            onToggle={() => setOpenSections((current) => ({ ...current, automations: !current.automations }))}
          >
            {routines.length === 0 ? (
              /*
                * The empty row is a DOOR, not a status.
                *
                * It read "Nothing saved yet", which is a fact about the shelf
                * and tells nobody that pressing it goes anywhere -- the same
                * mistake the Rooms row made until 0.60.0. It now says what is
                * behind it, which is the screen that lists the finished
                * conversations you can save.
                *
                * Not "New routine": a routine cannot be made from nothing, so a
                * button promising a blank one would lie about what it is.
                */
              <button
                type="button"
                className="lc-row lc-row--button lc-roomrow lc-roomrow--new"
                onClick={onOpenAutomations}
                title="Save a finished conversation so a teammate can replay it"
              >
                <Icon name="clock" size={12} />
                <span className="lc-row__text">
                  <span className="lc-row__meta">Save one from a finished conversation</span>
                </span>
              </button>
            ) : (
              routines.map((routine) => (
                <button
                  key={routine.routineId}
                  type="button"
                  className="lc-row lc-row--button"
                  title={`Open Routines · ${routine.name}`}
                  onClick={onOpenAutomations}
                >
                  <Icon name="clock" size={14} />
                  <span className="lc-row__text">
                    <span className="lc-row__name">{routine.name}</span>
                    <span className="lc-row__meta">
                      {String(routine.steps.length)} step{routine.steps.length === 1 ? '' : 's'}
                      {routine.schedule === undefined ? '' : ' · scheduled'}
                    </span>
                  </span>
                </button>
              ))
            )}
          </SidebarSection>

          </>
        ) : (          <div className="lc-convlist">
            {namingGroup && (
              <input
                className="lc-input lc-convgroup__rename"
                placeholder="Name this project"
                aria-label="Name this project"
                maxLength={60}
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === 'Escape') onNamingGroupDone?.()
                  if (event.key === 'Enter') {
                    const next = event.currentTarget.value
                    onNamingGroupDone?.()
                    if (next.trim().length > 0) onNewGroup?.(next)
                  }
                }}
                onBlur={() => onNamingGroupDone?.()}
              />
            )}
            {/*
              * Named groups first, folded; Ungrouped last and open.
              *
              * Colin's reference and also the right order: a group is
              * something you chose to put away, so it costs one line until
              * you want it, and the list you actually scan stays at the top
              * of the column.
              */}
            {(() => {
              const said = unreadableSentence(unreadable)
              return said === undefined ? null : (
                <div className="lc-sidebar__unreadable" role="status">
                  <span className="lc-sidebar__unreadable-happened">{said.happened}</span>
                  <span className="lc-sidebar__unreadable-safe lc-mono">{said.safe}</span>
                </div>
              )
            })()}
            {/*
              * And the conversations that exist and would not read, which
              * are a different thing from the stores above: All missions
              * already says "1 file could not be read" and this column said
              * nothing, so a person who deleted the one row they could see
              * of a chain whose other turn was unreadable thought the
              * conversation was gone (Grok, passes 11 and 12, with the
              * exact file). The row cannot be drawn -- there is nothing to
              * name it by -- so the count is, with the place that lists it.
              */}
            {unreadableConversations > 0 && (
              <div className="lc-sidebar__unreadable" role="status">
                <span className="lc-sidebar__unreadable-happened">
                  {unreadableConversations === 1
                    ? '1 conversation could not be read and is not listed here.'
                    : `${String(unreadableConversations)} conversations could not be read and are not listed here.`}
                </span>
                <span className="lc-sidebar__unreadable-safe lc-mono">Nothing is written over them. All conversations names the files.</span>
              </div>
            )}
            {/*
              * PROJECTS, then the rest (Colin, 2026-09-29). Every folder's
              * conversations in one list: a project holds conversations from
              * any folder, and the folder a conversation works in is the
              * chat box's chip, where it is switched.
              */}
            {conversationList(shownConversations, true)}
          </div>
        )}


        {/*
          * ONE card, drawn for whichever layout asked for it.
          *
          * It used to live inside the rail's branch, because the rail was the
          * only thing that opened it. The wide faces row opens it now too, so
          * a copy in each branch would be two panels that drift; this is the
          * same component, the same measured anchor, and the same close
          * behaviour in both.
          */}
        {railOpenFor !== undefined && railAnchor !== undefined && (() => {
          const open = teammates.find((entry) => entry.teammateId === railOpenFor)
          if (open === undefined) return null
          const theirs = missions.filter(
            (mission) => (mission.ownerId ?? missionOwners[mission.missionId]) === open.teammateId
          )
          const theirRuntime = runtimeOfTeammate(open, theirs)
          const status = teammateStatusView({
            runtime: theirRuntime === undefined ? undefined : runtimes.find((entry) => entry.id === theirRuntime),
            ...runtimeReach(runtimes, runtimes.length > 0),
            hasRunningMission:
              theirs.some((mission) => mission.phase === 'running') || starting.includes(open.teammateId),
            pendingApprovals: pendingApprovals[open.teammateId] ?? 0,
            roleLabel: roleLabelOf(open),
            ...(liveActivity[open.teammateId] === undefined ? {} : { liveActivity: liveActivity[open.teammateId] }),
            recentlyDone: recentlyDone.includes(open.teammateId),
            recentlyReceived: recentlyReceived.includes(open.teammateId)
          })
          return (
            <RailFlyout
              teammate={open}
              statusLabel={status.label}
              statusTone={status.tone === 'muted' ? 'muted' : status.tone}
              route={open.route === undefined ? undefined : routeChrome(open.route.runtime, open.route.model, routeModelName(open.route.runtime, open.route.model))}
              missions={theirs}
              selectedMissionId={selectedMissionId}
              top={railAnchor.top}
              left={railAnchor.left}
              pinned={railPinned !== undefined}
              onSelectMission={(missionId) => {
                onSelectMission(missionId)
                railClose()
              }}
              onMissionMenu={onMissionMenu}
              {...(renamingMissionId === undefined ? {} : { renamingMissionId })}
              {...(onRenameMission === undefined ? {} : { onRename: onRenameMission })}
              {...(onRenameDone === undefined ? {} : { onRenameDone })}
              onOpenMissions={() => {
                onOpenMissions(open.teammateId)
                railClose()
              }}
              onNewConversation={() => {
                // Was `onSelectTeammate`, which selects a teammate who is
                // already selected and opens the conversation that is
                // already open -- so the control did nothing at all.
                onNewConversationWith(open.teammateId)
                railClose()
              }}
              // The list filter is offered only where there is a list: the
              // rail draws no conversation rows, so narrowing them there
              // would narrow nothing anyone can see.
              filtered={faceFilter === open.teammateId}
              {...(compact
                ? {}
                : {
                    onFilter: () => {
                      setFaceFilter(faceFilter === open.teammateId ? undefined : open.teammateId)
                      railClose()
                    }
                  })}
              onPointerEnter={() => window.clearTimeout(railCloseTimer.current)}
              onPointerLeave={railLeave}
              onClose={railClose}
            />
          )
        })()}

        {/*
          Only one empty state, and only when it is true: no missions at all.
          When every mission belongs to a teammate they are already listed
          above, so an "other missions" section with a note in it would be a
          heading for nothing.
        */}
        {/*
          A search that matches nothing says so. Without this the sidebar
          simply empties, which reads as "you have no missions" rather than
          "none of them match".
        */}
        {query.trim().length > 0 && missionsMatching(missions, query, ownerName).length === 0 && (
          <p className="lc-sidebar__empty lc-row__meta">No conversations match that.</p>
        )}

        {/*
          The folding Missions section above carries its own one-liner, so
          this no longer draws a heading of its own -- with both, an empty
          sidebar showed "Missions" twice (seen in the settings drive,
          2026-09-06). What it still owns is the sentence that tells a first
          person what to do, which depends on the composer being on screen
          and on whether a teammate is picked.
        */}
        {missions.length === 0 && (
          <>
            <p className="lc-sidebar__empty lc-row__meta">
              {/*
                * "below" means the composer, which only the workroom has. On
                * Teammates and Settings this pointed at a box that was not on
                * the screen -- spotted while touring the screens, 2026-09-03.
                */}
              {!composerShown
                ? 'No conversations yet. They are kept here as they happen.'
                : teammates.length === 0
                  ? 'No conversations yet. Write below and it is kept on this machine as it happens.'
                  : // A message with nobody picked belongs to nobody now (0.21.6), so this
                    // must not promise the first teammate will pick it up -- the composer
                    // right under it said "Write a message…" while this said Juno would
                    // take it (user session, 2026-09-05). With a teammate actually
                    // picked, the promise is true and it says their name.
                    (() => {
                      const picked = teammates.find((teammate) => teammate.teammateId === selectedTeammateId)
                      return picked === undefined
                        ? 'No conversations yet. Pick a teammate, or write below and give it to one later.'
                        : `No conversations yet. Write below and ${picked.name} picks it up.`
                    })()}
            </p>
          </>
        )}
      </div>

      <div className="lc-sidebar__footer">
        {/*
          * An equal-thirds grid with min-width: 0. The row is a fixed 266px
          * and three labelled buttons laid out by content came to 371px of it
          * -- so the count was pushed off the edge entirely rather than
          * merely squeezed. Cells that share the width cannot overflow
          * whatever the labels ever say.
          */}
        {/*
          * TWO ROWS of the same equal thirds, not five cells in one.
          *
          * The one-row grid is a scar: laid out by content it came to 371px
          * inside a 266px column, and even at three equal cells the full word
          * "Teammates" starved to "Teamma…". Five cells would be ~47px each
          * and "Routines" does not survive that.
          *
          * The design agent's answer, 2026-09-15, picked rather than left to
          * implementation: keep the grid exactly as delivered and give it a
          * second row. Nothing is content-sized, so the scar cannot reopen.
          *
          * WHY THESE TWO ARE HERE AT ALL. 0.139.0 flattened the sidebar into
          * a conversation list and left Rooms and Routines with nowhere to
          * be -- measured after shipping it: no button, no label, and an
          * existing room not drawn anywhere in the wide sidebar. That is the
          * exact failure the design brief warned about, because it had
          * happened before: rooms reachable only from the palette, and Colin,
          * 2026-09-09, "sorry if this is dumb but how does one create a room
          * for teammates, i cant figure it out lol". A labelled footer button
          * beats a fold under 300px of roster; no button at all beats
          * nothing.
          */}
        {/*
          * The second row is Settings and the status, and no Teammates.
          *
          * Three answers to one collision, and the third is the right one.
          * 0.140.0 put Teammates, Settings and the count in equal thirds and
          * shipped truncated -- "Teamma..." and "6 connect..." -- on a fit
          * check that compared a label's RENDERED width to its cell, which a
          * truncated element always passes. I then gave the row two cells,
          * which fit but kept a door nobody needed.
          *
          * The design agent's answer, once Colin's faces row existed: take
          * `Teammates` out altogether. The faces and their `Team` pill ARE
          * the way to the roster, so a footer link is a second door to one
          * room -- and removing it frees two cells for the status line, which
          * then reads in full instead of "6 connect...". One removal, both
          * truncations gone.
          *
          * Measured: nav cells 74px against a widest label of 48px, status
          * 156px against 145px of ink. Nothing content-sized, so the
          * 371px-into-266px scar cannot reopen.
          */}
        <div className={`lc-sidebar__nav${onOpenBoard === undefined ? '' : ' lc-sidebar__nav--board'}`}>
          <button type="button" onClick={() => { railClose(); onOpenSettings() }} title="Settings (Ctrl 3)">
            <Icon name="settings" size={14} />
            <span>Settings</span>
          </button>
          {/* Board lives down here (0.605): a view of the conversations, beside the app's own controls, not a fourth place. */}
          {onOpenBoard !== undefined && (
            <button type="button" onClick={() => { railClose(); onOpenBoard() }} title="Board — every conversation by what it needs from you">
              <Icon name="columns" size={14} />
              <span>Board</span>
            </button>
          )}
          {/* The status takes what the row leaves. It stays mono and muted
              while the nav labels stay sentence case, so the row reads as one
              fact beside places rather than three peers. Beside Board it says
              "agents", and its title says "AI agents": the shorter line fits
              the third of the row it has (measured at 267px, 2026-10-04). */}
          <div className={`lc-connected ${onOpenBoard === undefined ? 'lc-connected--wide' : 'lc-connected--beside'}`} title={`${connected} AI agent${connected === 1 ? '' : 's'} ready`}>
            <span className={`lc-connected__dot${connected === 0 ? ' is-none' : ''}`} />
            <span>
              {connected} {onOpenBoard === undefined ? 'AI ' : ''}agent{connected === 1 ? '' : 's'} ready
            </span>
          </div>
        </div>
      </div>
    </nav>
  )
}
