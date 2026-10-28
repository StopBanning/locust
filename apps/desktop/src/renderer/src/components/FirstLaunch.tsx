import { useEffect, useRef, useState } from 'react'
import type { ReactElement, ReactNode } from 'react'

import type { PublicRuntimeStatus, TubePreference } from '../../../shared/ipc.js'
import { connectedRuntimeCount, deferredOthersSentence, integrationOf, routeRowStatus, runtimeIsUsable } from '../status.js'
import { FREE_START_RUNTIME, installCommand, installSentence, runtimeInstallFacts, signInCommand } from '../../../shared/runtime-install.js'
import { COVER_HEIGHT, COVER_REACH, HomeCover, coverGrowFor, coverRoomFor, coverScale, homeIsShort, sectionsAtNormalSpacing, type HomeSpacingSeen } from './HomeCover.js'
import { HomeTeam } from './HomeTeam.js'
import { AwayList } from './AwayList.js'
import type { AwaySummary } from '../../../shared/away.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import { RuntimeMark } from './RuntimeMark.js'
import { AgentMark } from './AgentMark.js'
import type { HomeTeamInvite, HomeTeammate, HomeTeamWatch } from './HomeTeam.js'
import { TeamTemplates } from './TeamTemplates.js'
import type { TeamTemplate } from '../../../shared/team-templates.js'
import { Icon } from './Icon.js'
import { StarterList } from './StarterList.js'
import type { Starter } from './StarterList.js'
import { SignInButton } from './SignInButton.js'
import { UsageChip, UsageLine, useUsageSummary } from './UsageLine.js'
import type { UsageReadResponse } from '../../../shared/ipc.js'
import type { UsageRange } from '../../../shared/usage.js'

/**
 * First run, and the empty state generally.
 *
 * Built to panel A of `FIRST-RUN-2026-09-04.md`, which Colin ratified on
 * 2026-09-05 ("do it exactly this way besides that send button"): the mark
 * in its own bordered card, two mono lines of claim, and the runtimes as ONE
 * panel in TWO COLUMNS with hairline dividers -- a dot, a name, a version,
 * nothing else. The two columns are also what stops six rows stacking into a
 * tower that pushes the composer off the bottom of the window.
 *
 * What the reference does not know about, and this does: a runtime can be
 * installed but signed out, at its account limit, or experimental. READY
 * needs no tag once its dot is green, so only the exception is tagged --
 * which is the reference's own rule, applied to more exceptions than it drew.
 */
/**
 * Ask the host to open one of the addresses it allows.
 *
 * These used to be `<a target="_blank">`, which does nothing in this app:
 * the host denies `window.open` outright and cancels navigation away from
 * its own URL, on purpose, because `openExternal` bypasses every egress
 * rule the packaged build has. So every "Get it" link ever shipped was
 * dead -- it looked like a link, it had a cursor, and nothing happened.
 * Found by the first outside tester on 0.55.0.
 *
 * A button rather than an anchor, because that is what it is: it makes a
 * request the host may refuse, and it never navigates.
 */
function openLink(url: string, onRefused: (message: string) => void): void {
  const bridge = window.desktop
  if (bridge === undefined) return
  void bridge
    .openLink(url)
    // The host refuses addresses it will not hand to a browser and says so.
    // Dropping that answer is how a refused press and a press that worked
    // came to look identical (Grok, 2026-09-14, finding 3).
    .then((answer) => {
      if (!answer.ok) onRefused(answer.message)
    })
    .catch(() => onRefused('That link could not be opened. Nothing on this machine changed.'))
}

/** Where a runtime that is not a package comes from. */
function vendorUrl(runtime: string): string | undefined {
  const facts = runtimeInstallFacts(runtime)
  return facts !== undefined && facts.install.kind === 'vendor' ? facts.install.url : undefined
}



export function FirstLaunch({
  scout,
  teamWatch,
  teamInvite,
  runtimes,
  limitedRuntimes,
  usageWindows,
  readUsage,
  usageKey,
  onOpenUsage,
  discoveryPhase,
  tube,
  swarmCalls = 0,
  coverStatus,
  coverActivity,
  freeStart = 'unknown',
  onUseFree,
  workspacePath,
  teammateCount,
  onChooseFolder,
  onNewTeammate,
  onAddTeamFromCard,
  rosterUnreadable = false,
  onInstall,
  installing,
  installLine,
  installLog,
  installFailure,
  npmMissing = false,
  npmIsBundled = false,
  npmDidNotAnswer = false,
  checkingGaveUp = false,
  onCheckAgain,
  onCompare,
  onStarter,
  onCompareStarter,
  onStartFromIssue,
  workspaceMade = false,
  team = [],
  onMessageTeammate,
  onUseTemplate,
  away,
  awayTeammates = [],
  onOpenAway,
  awaySaid,
  onOpenRoutines,
  onAwaySeen
}: {
  readonly runtimes: readonly PublicRuntimeStatus[]
  /** Runtimes whose last run ended on the account's usage limit, with its own words. */
  readonly limitedRuntimes: ReadonlyMap<string, string>
  /** The latest usage reading per runtime, from what runs reported (0.388). */
  readonly usageWindows?: ReadonlyMap<string, string>
  /** Usage across every agent and model, for the line under the accounts (0.714). */
  readonly readUsage?: (range: UsageRange) => Promise<UsageReadResponse>
  /** Changes when the record does, so the usage line counts the turn that just ended. */
  readonly usageKey?: string
  /** Opens the Usage dialog. */
  readonly onOpenUsage?: () => void
  /**
   * Whether the no-account runtime still lists something free. Absent reads
   * as `unknown`, which keeps the promise: this is a correction on disproof,
   * not a hedge on silence.
   */
  readonly freeStart?: 'yes' | 'no' | 'unknown'
  /** Puts the chat box on OpenCode's free model (0.514); absent when it already is. */
  readonly onUseFree?: () => void
  readonly discoveryPhase: 'loading' | 'ready' | 'error'
  /** The boot screen's preference; the lockup's lighting follows it. Absent reads as full. */
  readonly tube?: TubePreference
  /** How many times swarm has been turned on this session: the cover flies the swarm for a new one. */
  readonly swarmCalls?: number
  /** Who is working and who waits on you, for the cover's glass (0.610, glassStatus). */
  readonly coverStatus?: string
  /** Changes whenever a teammate's live work arrives, including a streaming delta. */
  readonly coverActivity?: unknown
  /** The folder the teammates work in; undefined when none is chosen. */
  readonly workspacePath: string | undefined
  readonly teammateCount: number
  /** The team, for the cards Home leads with (HomeTeam). */
  readonly team?: readonly HomeTeammate[]
  /** Pressing a teammate's card: talk to them. */
  readonly onMessageTeammate?: (teammateId: string) => void
  /** What Scout noticed, or the invitation to turn it on (0.756, ScoutCards.tsx): first on Home when there is any. */
  readonly scout?: ReactNode
  /** Scout keeping watch with nothing to say (0.799): its line and Look now on its own team card (HomeTeam). */
  readonly teamWatch?: HomeTeamWatch
  /** Scout, offered on the team's grid until it is decided (0.808). */
  readonly teamInvite?: HomeTeamInvite
  /** Since you were away (0.590): what ended while nobody was here, when there is something to say. */
  readonly away?: AwaySummary
  readonly awayTeammates?: readonly PublicTeammate[]
  readonly onOpenAway?: (missionId: string) => void
  /** What each of those runs came to, in a line (0.758, AwayList). */
  readonly awaySaid?: (missionId: string) => string | undefined
  readonly onOpenRoutines?: () => void
  readonly onAwaySeen?: () => void
  readonly onChooseFolder: () => void
  /** Opens the New teammate form: the home screen's way to the product's core action. */
  readonly onNewTeammate?: () => void
  /** Add the team on a Locust team card (0.398). */
  readonly onAddTeamFromCard?: () => void
  /**
   * The teammates file exists and would not read (QA-2026-09-29 round 2,
   * R26). It looked exactly like a first launch -- "Start with a team" --
   * and the person read their team as gone.
   */
  readonly rosterUnreadable?: boolean
  /** Makes a whole team from a template (TeamTemplates); resolves with why it could not. */
  readonly onUseTemplate?: (templateId: TeamTemplate['templateId']) => Promise<string | undefined>
  /** Run the install for a runtime. Absent means the panel offers none. */
  readonly onInstall?: (runtime: string) => void
  /** The runtime being installed right now; every other button waits on it. */
  readonly installing?: string
  /** The last line npm printed, with how long it has been going. */
  readonly installLine?: string
  /**
   * Everything npm has said for the install now running. The line above is
   * the last of these; this is what `Show output` opens onto.
   */
  readonly installLog?: readonly string[]
  /** What went wrong, and what to do about it. */
  readonly installFailure?: {
    readonly what: string
    readonly next: string
    readonly restart?: boolean
    /** The line the app would have run, kept reachable however it went wrong. */
    readonly command?: string
  }
  /** Nothing can run an install: no npm on the machine, and none shipped. */
  readonly npmMissing?: boolean
  /**
   * The install will run on the npm this app carries, because the machine
   * has no Node. The buttons work; what differs is where the CLI ends up
   * reachable from, which the note below says out loud.
   */
  readonly npmIsBundled?: boolean
  /**
   * npm IS on this machine and did not answer in five seconds. Changes the
   * sentence, never the behaviour: the bundled path is right either way.
   */
  readonly npmDidNotAnswer?: boolean
  /**
   * Discovery has asked three more times and an installed CLI still has not
   * answered. Until then CHECKING is honest; after it, the row has to say
   * what is true and offer the one thing that can change it.
   */
  readonly checkingGaveUp?: boolean
  /** Ask discovery again, from the top: the one repair the app can perform for a CLI that never answered. */
  readonly onCheckAgain?: () => void
  /**
   * Compare (0.442): opens the model picker already switched to Compare.
   * Colin, 2026-09-28: "You dont NEED a teammate assigned ... want new
   * users to be able to try that out" -- so it is on Home, by the agents.
   */
  readonly onCompare?: () => void
  /** A starter's words in the box for the teammate on screen, in the mode it suits (2026-10-19, StarterList.tsx). */
  readonly onStarter?: (starter: Starter) => void
  /** The same words in a comparison of two models: of the work, each in its own copy, or of the answers. */
  readonly onCompareStarter?: (starter: Starter) => void
  /** Open the folder's issues. Passed only when this folder is a GitHub repository gh can read. */
  readonly onStartFromIssue?: () => void
  /**
   * Locust INVENTED the folder it is about to work in, because the one it was
   * launched from was refused. The screen has to say so; see the card below.
   */
  readonly workspaceMade?: boolean
}): ReactElement {
  // Stays open across subsequent installs once a person opens it, which is
  // what the design asks for: someone who wanted the trace once wants it
  // for the next one too.
  const [outputOpen, setOutputOpen] = useState(false)
  /** What the host said when it would not open a link. Cleared on the next press. */
  const [linkRefusal, setLinkRefusal] = useState<string>()
  /*
   * The other agents, while none of them can help yet.
   *
   * Closed to begin with and opened by a press -- never opened on the app's
   * behalf, because the whole point is that the person decides when they want
   * the list. Once anything is connected this state stops being consulted.
   */
  const [othersOpen, setOthersOpen] = useState(false)
  /** The folded agent list, opened by its Show all (see `folded`). */
  const [agentsOpen, setAgentsOpen] = useState(false)
  // Signed-in first, exceptions last -- the reference's own order, and the
  // one that reads: a person scanning this wants "what can I use" before
  // "what is not built yet". Discovery's order is alphabetical by id, which
  // put PLANNED runtimes in the middle of the working ones.
  const rank = (row: { readonly connected: boolean; readonly status: { readonly tag: string } }): number =>
    row.connected ? 0 : row.status.tag === 'PLANNED' ? 2 : 1
  const rows = runtimes
    .map((runtime) => ({
      runtime,
      status: routeRowStatus(runtime, integrationOf(runtime.id), false, limitedRuntimes.get(runtime.id)),
      // The SAME question the rail footer asks -- "can this run work for me
      // right now" -- so the two numbers on this screen cannot disagree.
      // They did: the claim line counted READY tags (5) while the footer
      // counted usable runtimes (6), because Antigravity is connected and
      // wears EXPERIMENTAL rather than READY. One screen, one definition;
      // the caveat rides on the tag, where it belongs.
      connected: runtimeIsUsable(runtime) && integrationOf(runtime.id) !== 'planned'
    }))
    .sort((left, right) => rank(left) - rank(right))
  // A planned runtime cannot be connected by anyone, so it is not in the
  // count's denominator and not in the panel: it is named once, under it
  // (design review, 2026-09-05: "a progress meter the user can't complete").
  const connected = connectedRuntimeCount(runtimes)
  const shownAll = rows.filter((row) => integrationOf(row.runtime.id) !== 'planned')
  // On a machine with nothing connected, the row that is a complete answer
  // leads. Once anything works, rank() already puts connected first and this
  // steps out of the way.
  const shown =
    connected === 0
      ? [...shownAll].sort((left, right) =>
          left.runtime.id === FREE_START_RUNTIME ? -1 : right.runtime.id === FREE_START_RUNTIME ? 1 : 0
        )
      : shownAll
  /*
   * THE FIRST TEAMMATE, ASKED FOR BY NAME (0.309). A practice tester on
   * 0.306: "Claude looked for how to make a teammate and found it only under
   * an unlabeled sidebar +; the three prominent mascot faces on Home looked
   * like the entry point but were decorative." So while there is no
   * teammate, the home screen offers one, with the sentence that says what it
   * is -- which sat at the foot of the form, below the fold.
   */
  const offerFirstTeammate = !rosterUnreadable && teammateCount === 0 && onNewTeammate !== undefined && discoveryPhase === 'ready'
  // A build stamp with a commit hash is a fact for a changelog, not a
  // status panel: "2026.09.02-c22c1a3" reads as its date.
  const shortVersion = (version: string | null | undefined): string => (version === undefined || version === null ? '' : version.replace(/-[0-9a-f]{6,}$/i, ''))

  /*
   * When the screen is taller than the pane (1120x720 with six runtimes and
   * the changelog banner), keep it at its END, where the Install button and
   * the composer are, and let the lockup be what scrolls away above. The
   * list grows as discovery reports, so this follows the content's height
   * rather than running once; a person who has scrolled up is left where
   * they are.
   */
  const pane = useRef<HTMLDivElement>(null)
  /*
   * SCROLLED, SO THE TOP FADES (0.403). What scrolls away above -- the cover,
   * at 1120x720 -- was cut by the pane's edge mid-drawing: the sign's box
   * sliced through, or one thin line of it left at the top, which read as a
   * glitch rather than as more above (fresh-eyes check, Home). A scrolled
   * pane fades its top edge instead.
   */
  const [scrolled, setScrolled] = useState(false)
  useEffect(() => {
    const el = pane.current
    if (el === null || typeof ResizeObserver === 'undefined') return
    // From the top (0.583): Home is centred now, and a page that still runs
    // over opens at its cover. It opened at the END, so the cover was cut off
    // until scrolled up -- Colin: "i have to always scroll up to have our home
    // page centered". Once the person scrolls to the end it is followed there.
    let atEnd = false
    const onScroll = (): void => {
      atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 4
      setScrolled(el.scrollTop > 0)
    }
    const follow = (): void => {
      if (atEnd) el.scrollTop = el.scrollHeight
      setScrolled(el.scrollTop > 0)
    }
    el.addEventListener('scroll', onScroll)
    const observer = new ResizeObserver(follow)
    observer.observe(el)
    if (el.firstElementChild !== null) observer.observe(el.firstElementChild)
    follow()
    return () => {
      el.removeEventListener('scroll', onScroll)
      observer.disconnect()
    }
  }, [])

  /*
   * THE COVER GROWS INTO SPARE HEIGHT (see `coverGrowFor`). Measured from what
   * the page holds WITHOUT the cover's own growth -- everything else in the
   * column, plus the cover at its width's size -- so a cover that grows does
   * not shrink the room it was given and the two never chase each other. A
   * little of the spare height is always left as air above it, and a window
   * with none to spare draws the cover exactly as before.
   */
  // Usage for the line under the accounts (0.714): read once, again when the record moves.
  const usageSummary = useUsageSummary(readUsage, usageKey ?? '')
  const inner = useRef<HTMLDivElement>(null)
  const [coverGrow, setCoverGrow] = useState(1)
  // Short of height even with the smallest cover: tighter padding and gaps (0.583, `homeIsShort`).
  const [tight, setTight] = useState(false)
  // The sections' height at each spacing, for one count of sections (0.838, `sectionsAtNormalSpacing`).
  const spacing = useRef<HomeSpacingSeen>({ count: -1 })
  useEffect(() => {
    const paneEl = pane.current
    const innerEl = inner.current
    if (paneEl === null || innerEl === null || typeof ResizeObserver === 'undefined') return
    const measure = (): void => {
      const cover = innerEl.querySelector<HTMLElement>('.lc-cover')
      if (cover === null) return
      const padding = getComputedStyle(paneEl)
      const usable = paneEl.clientHeight - parseFloat(padding.paddingTop) - parseFloat(padding.paddingBottom)
      // The cover's footprint: its box and the bots' reach above it (COVER_REACH, 0.583).
      const base = (COVER_HEIGHT + COVER_REACH) * coverScale(cover.clientWidth)
      /*
       * EVERYTHING ELSE, MEASURED FROM ITSELF (0.516). This was the column's
       * height less the cover's, which also took in the space around the
       * cover -- and that moves when the cover grows. Colin, 2026-10-01, Home
       * at ~1200x770: "stuttering and shaking like crazy". Traced
       * (drive-home-holds-still): "everything else" read 314px with the cover
       * at 189 and 319px with it at 216, so each size argued for the other,
       * 20+ times in two seconds. The other sections, their margins and the
       * column's gaps and padding do not move with the cover.
       */
      const column = getComputedStyle(innerEl)
      const gap = parseFloat(column.rowGap) || 0
      const children = [...innerEl.children] as HTMLElement[]
      const sections = children
        .filter((child) => child !== cover && !child.contains(cover))
        .reduce((sum, child) => {
          const style = getComputedStyle(child)
          return sum + child.offsetHeight + (parseFloat(style.marginTop) || 0) + (parseFloat(style.marginBottom) || 0)
        }, 0)
        + (parseFloat(column.paddingTop) || 0) + (parseFloat(column.paddingBottom) || 0)
      const others = sections + gap * Math.max(0, children.length - 1)
      // Decided at the normal spacing, even while tight (0.838, `sectionsAtNormalSpacing`).
      const normal = sectionsAtNormalSpacing(spacing.current, children.length, sections, paneEl.classList.contains('is-tight'))
      spacing.current = normal.seen
      setTight(homeIsShort(normal.sections, children.length, base, paneEl.clientHeight))
      setCoverGrow(coverGrowFor(coverRoomFor(usable, others, base), base))
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(paneEl)
    observer.observe(innerEl)
    return () => observer.disconnect()
  }, [])

  /*
   * THE STEP THAT CAN BE TAKEN COMES FIRST (0.780). With nothing installed, the one thing Home can do is install
   * the free agent, and its row sat under the team templates and the folder card: below the fold in Locust's own
   * 1215x800 window and at 1024x680. So the first screen offered three teams that could not run yet and hid the
   * press that makes them run (clean-machine drive, packaged 0.779, 2026-10-16). Until something is installed the
   * accounts lead; once anything is, the order is the one it always was. Installed, not connected: agents being
   * checked at launch are installed, so a returning Home never reorders while they answer.
   */
  const accountsLead = discoveryPhase === 'ready' && runtimes.every((runtime) => !runtime.installed)
  const teamAndFolder = (
    <>
      {offerFirstTeammate && onUseTemplate !== undefined && onNewTeammate !== undefined && (
        <TeamTemplates onUse={onUseTemplate} onNewTeammate={onNewTeammate} {...(onAddTeamFromCard === undefined ? {} : { onAddFromImage: onAddTeamFromCard })} />
      )}

      {offerFirstTeammate && onUseTemplate === undefined && (
        <div className="lc-firstteammate">
          <button type="button" className="lc-button lc-firstteammate__button" onClick={onNewTeammate}>
            <Icon name="plus" size={13} /> New teammate
          </button>
          <span className="lc-firstteammate__about">
            A name, a face and a model of its own, and a place for its conversations. It grants no new access.
          </span>
        </div>
      )}

      {/*
        * The folder card itself lives in Settings and on the composer's own
        * chip -- this screen is about what is connected (Colin, 2026-09-05).
        * What stays is the one case where the screen would otherwise invite
        * a mission that cannot start: no folder chosen, which is what an
        * installed app launched from its own install folder begins as.
        */}
      {workspacePath === undefined && (
        <div className="lc-folder is-missing">
          <div className="lc-folder__text">
            <div className="lc-folder__label">No folder chosen</div>
            <div className="lc-folder__path">
              Every teammate works inside one project folder. Pick it before the first conversation.
            </div>
          </div>
          <button type="button" className="lc-button" onClick={onChooseFolder}>
            Choose folder
          </button>
        </div>
      )}

      {/*
        * THE FOLDER LOCUST INVENTED, SAID WHERE SOMEBODY IS LOOKING.
        *
        * A fresh install is launched from the Start menu, which starts it
        * in its own install folder; the host refuses that folder on
        * purpose, so the app makes `Documents\Locust` and works there. The
        * person therefore has a teammate standing in an empty folder they
        * did not choose, while the whole promise of the product is that a
        * teammate works in THEIR project.
        *
        * It was said in exactly one place: the composer chip's hover
        * tooltip. Nobody hovers a chip in their first ten seconds, and the
        * chip reads "Locust" -- which is also the application name and the
        * window title, so it does not read as a folder at all, let alone
        * one worth changing. Found by driving a first launch nobody had
        * ever driven (`_tools/first-launch-folder-drive.mjs`, 2026-09-19).
        *
        * Not an alarm, and not `is-missing`: the app did something
        * reasonable and this is the sentence that makes it visible. It
        * disappears the moment a folder is chosen, because `workspaceMade`
        * is false for any folder a person picked.
        */}
      {workspacePath !== undefined && workspaceMade && (
        <div className="lc-folder">
          <div className="lc-folder__text">
            <div className="lc-folder__label">Locust made a folder to work in</div>
            <div className="lc-folder__path" title={workspacePath}>
              Teammates will work in {workspacePath} — it is new and empty. Point them at your own project instead.
            </div>
          </div>
          <button type="button" className="lc-button" onClick={onChooseFolder}>
            Choose folder
          </button>
        </div>
      )}
    </>
  )

  /*
   * THE ACCOUNTS, THEN THE TEAM (0.799). Colin, 2026-10-19, Home at 1205x803 scrolled: "maybe if we move connected
   * accounts and usage in between the teammates and crt?" With a team, the accounts and usage sit under the cover
   * and the starters under the team, so a person reads what is connected, who is on the team, and what to begin.
   * Before a team exists the order is the one it was: the templates first, the accounts after.
   */
  let startersShown = false
  const accountsBlock = discoveryPhase !== 'ready' ? null : (() => {
    /*
     * Five rows reading CHECKING were five rows saying nothing at the
     * moment a person is deciding what to click, and the transient
     * state at that. The dot carries checking; the head note says it
     * once with a count; words in the rows are for results -- and the
     * result a person is choosing between is what each one costs.
     */
    const isChecking = (row: (typeof shown)[number]): boolean =>
      row.runtime.installed && row.status.tag === 'CHECKING' && !checkingGaveUp && !row.connected
    const checkingAny = shown.some(isChecking)
    const stuckAny = shown.some((row) => row.runtime.installed && row.status.tag === 'CHECKING' && checkingGaveUp && !row.connected)
    // One thing happening, said once at the head: while an install
    // runs, the other rows' actions step out rather than sit dimmed
    // with no reason (Colin, 2026-09-19, with the frame: "some of the
    // installs were bugged").
    const installingName = installing === undefined ? undefined : shown.find((row) => row.runtime.id === installing)?.runtime.displayName ?? installing
    /*
     * "ON THIS MACHINE" HAS TO MEAN ON THIS MACHINE.
     *
     * Both of these counted `shown.length`, which is the CATALOGUE --
     * every agent Locust can drive, installed or not. So a box with
     * five hung CLIs and no Antigravity read *6 on this machine · none
     * answering*, and Antigravity is not on this machine (Grok, pass
     * 16). The arithmetic of rows-plus-deferred came to six and the
     * words did not.
     *
     * `known` is right for the other branch and stays: a machine with
     * nothing installed genuinely has six KNOWN and none installed.
     * The word is what changes with the claim.
     */
    const onThisMachine = shown.filter((row) => row.runtime.installed).length
    const headNote = installingName !== undefined
      ? `installing ${installingName}…`
      : checkingAny
      ? `checking ${String(onThisMachine)} on this machine`
      : connected > 0
        ? `${String(connected)} ready`
        : stuckAny
          ? `${String(onThisMachine)} on this machine · none answering`
          : `${String(shown.length)} known · none installed yet`
    /*
     * ONE STEP FIRST, AND THE CATALOGUE WHEN IT IS ASKED FOR.
     *
     * Colin, 2026-09-19: "i really want a new user without tech
     * savvyness to be able to just use the software off rip, maybe even
     * ask them how to get the other agents working." The other agents
     * come AFTER, in his own sentence.
     *
     * Measured on the first frame anybody has taken of this screen on a
     * machine with nothing installed (`_tools/bare-machine-frame.mjs`,
     * 2026-09-19): six rows, of which five name a product attached to
     * an account or a subscription the person does not have -- a
     * ChatGPT account, an Anthropic account, a Cursor account, a GitHub
     * Copilot subscription. Every one of those five is a reason not to
     * press anything, on a screen whose sentence one line above has
     * already said which one to press.
     *
     * The on-ramp emphasis was already right and is untouched. What
     * changes is only that the five are BEHIND A PRESS while none of
     * them can help, and the count stays visible in the head note, so
     * nothing is hidden -- it is deferred, and says so.
     *
     * The instant anything is connected this stops applying and the
     * list is the list again: by then the question "what else can this
     * drive" is a real question, which is the one the catalogue
     * answers well.
     */
    /*
     * A ROW THAT IS ON THIS MACHINE IS NEVER DEFERRED, whatever state
     * it is in.
     *
     * The rule above is about products a person does not have and
     * cannot use yet. An installed CLI is the opposite of that case: it
     * is theirs, it has a state, and the state may be the one thing on
     * the screen they need.
     *
     * Fable found this on 0.198.0, the day the deferral shipped, on a
     * Linux box with four CLIs installed and hung. The screen drew ONE
     * row while the head note read *6 on this machine · none
     * answering*, the composer said *A coding agent is installed but
     * not answering — Check again above*, and the four that were
     * installed and hanging sat behind the sentence *5 others Locust
     * can drive — they each need their own account*. Every word of that
     * was wrong about them: they need no account, they are here, and
     * their Check again was a press away behind a control that gave no
     * reason to press it.
     *
     * My own measurement could not have caught it: the frame I designed
     * this against was taken on a machine with nothing installed, where
     * "not connected" and "not installed" are the same rows. They are
     * not the same rows.
     *
     * `installed` and not `connected`: a signed-out Codex is installed,
     * and the command that signs it in is on its row.
     */
    // How much has been done, across every agent (0.714): under the accounts, once there is anything to count.
    const usageLine = onOpenUsage === undefined ? null : <UsageLine summary={usageSummary} onOpen={onOpenUsage} />
    const usageChip = onOpenUsage === undefined ? null : <UsageChip summary={usageSummary} onOpen={onOpenUsage} />
    const deferOthers = connected === 0 && !checkingAny && installing === undefined && !othersOpen
    const drawn = deferOthers
      ? shown.filter((row) => row.runtime.id === FREE_START_RUNTIME || row.runtime.installed)
      : shown
    const deferred = shown.length - drawn.length
    /*
     * FOLDED TO ONE LINE when it has nothing to ask of the person: a team
     * exists (Home leads with it), something is connected, and every
     * agent on this machine is ready -- nothing checking, stuck,
     * installing or signed out. Then seven rows of names and versions
     * are a status panel on the first screen (first-impressions drive,
     * packaged 0.349), and one line says the same. The moment any of
     * that changes the list is the list again, rows and buttons and
     * all, because then it is the thing to act on; and Show all opens
     * it any time.
     *
     * With nobody on the team, only when EVERY agent is ready (0.360).
     * Then there is nothing to add and nothing to fix, and the seven
     * rows beside the team templates pushed the page past a 1440x900
     * window -- 25px over, 157px at 1366x768, the cover's middle bot
     * cut at the top (probe-home-fits, packaged 0.359). While any
     * agent could still be added, the rows stay: before a team exists
     * the list is how a new person learns what else Locust can drive
     * (Colin, 2026-09-19: "maybe even ask them how to get the other
     * agents working").
     */
    /*
     * A CHECK STILL RUNNING DOES NOT HOLD THE LIST OPEN, ONCE THERE IS A TEAM
     * (design pass, 2026-10-19). OpenCode's first start installs its plugin
     * for 48-89 s, and for that minute Home showed all seven rows -- six of
     * them Ready -- above the team, because one dot was still grey. A check
     * is a moment, not a thing to act on: the line says it ("6 ready ·
     * checking OpenCode") and the list opens again only if the answer needs
     * the person. Before a team, the list is still how a person meets the
     * agents, and it waits for every answer.
     */
    const foldsWhileChecking = team.length > 0
    const everyInstalledReady = shown.every((row) => !row.runtime.installed || row.connected || (foldsWhileChecking && isChecking(row)))
    const everyAgentReady = shown.every((row) => row.connected)
    const folded =
      !agentsOpen && connected > 0 && (foldsWhileChecking || !checkingAny) && !stuckAny && installingName === undefined && everyInstalledReady && (team.length > 0 || everyAgentReady)
    if (folded) {
      startersShown = true
      const ready = shown.filter((row) => row.connected)
      const stillChecking = shown.filter(isChecking)
      const checkingSaid =
        stillChecking.length === 0 ? undefined : stillChecking.length === 1 ? `checking ${stillChecking[0]?.runtime.displayName ?? ''}` : `checking ${String(stillChecking.length)} more`
      return (
        <>
        {/* The accounts and their usage, one block: the line costs no gap of Home's (0.714). */}
        <div className="lc-accountsblock">
        <div className="lc-agenthead is-folded">
          <span className="lc-agenthead__label">Connected accounts</span>
          <span className="lc-agenthead__note is-green">{`${String(connected)} ready`}</span>
          {checkingSaid !== undefined && <span className="lc-agenthead__note">{`· ${checkingSaid}`}</span>}
          {/*
            * THEIR MARKS, NOT A LIST OF NAMES (0.383). "Codex CLI ·
            * Claude Code · Cursor Agent · ..." was a sentence to read
            * on a line that says nothing needs you; five marks are
            * seen at once, as Orca shows its agents (Colin,
            * 2026-09-26: logos "to signify which model is being used
            * by what"). Each mark is named for a screen reader and on
            * hover, and the line's title lists them all.
            */}
          {/*
            * THE MARKS, AND ON HOVER WHAT EACH KNOWS (AgentMark, 0.389):
            * the row is as calm as it was before usage rings (0.388),
            * and pointing at a mark opens its name, state and usage.
            */}
          <span className="lc-agenthead__marks">
            {ready.map((row) => {
              const usage = usageWindows?.get(row.runtime.id)
              return (
                <AgentMark
                  key={row.runtime.id}
                  runtime={row.runtime.id}
                  name={row.runtime.displayName}
                  state={row.runtime.version === undefined || row.runtime.version === null ? 'Ready' : `Ready · ${shortVersion(row.runtime.version)}`}
                  {...(usage === undefined ? {} : { usage })}
                />
              )
            })}
          </span>
          {usageChip}
          <button type="button" className="lc-agenthead__more" onClick={() => setAgentsOpen(true)}>
            Show all
          </button>
        </div>
        {usageLine}
        </div>
        </>
      )
    }
    return (
      <>
        <div className="lc-agenthead">
          <span className="lc-agenthead__label">Connected accounts</span>
          <span className={`lc-agenthead__note${connected > 0 && !checkingAny ? ' is-green' : ''}`}>{headNote}</span>
        </div>
        <div className="lc-runtimepanel">
          {drawn.map((row) => {
            const { runtime, status, connected: usable } = row
            const facts = runtimeInstallFacts(runtime.id)
            const checking = isChecking(row)
            const stuck = runtime.installed && status.tag === 'CHECKING' && checkingGaveUp && !usable
            const signIn = !usable && status.tag === 'SIGN IN'
            // The recommendation, carried by the row's own emphasis while
            // nothing is connected: a lit dot, the name at full weight,
            // the fact in green, the one filled button.
            const onRamp = !usable && !checking && !stuck && connected === 0 && runtime.id === FREE_START_RUNTIME
            /*
             * THE STATE FIRST: installed or not, signed in or not.
             *
             * Colin, 2026-09-22: "can that beginning screen know the
             * difference if the user needs to install or sign in?" It
             * did, but only in the BUTTON -- the words beside it were
             * "needs a Cursor account" on a runtime that was not on the
             * machine and on one that was installed and signed out
             * alike. The sentence now leads with which it is, and the
             * account it will want comes after, where truncation in a
             * narrow cell takes it rather than the state.
             */
            const need: { readonly state?: string; readonly detail?: string; readonly free?: boolean } | undefined =
              usable || checking
                ? undefined
                : runtime.status === 'unsupported'
                  // A version Locust does not run yet (0.805, OpenCode 2): the state in the cell, why and what to do on hover.
                  ? { state: 'not supported yet', ...(runtime.unsupportedNote === undefined ? {} : { detail: runtime.unsupportedNote }) }
                  : stuck
                  ? { detail: 'did not answer its version check' }
                  : signIn
                    ? { state: 'not signed in', detail: facts?.account !== undefined ? `installed; signs in with ${facts.account}` : 'installed' }
                    : runtime.installed
                      ? facts?.account !== undefined
                        ? { detail: `needs ${facts.account}` }
                        : undefined
                      : facts?.account !== undefined
                        ? { state: 'not installed', detail: `needs ${facts.account}` }
                        : facts?.install.kind === 'vendor'
                          ? { state: 'not installed', detail: `from ${new URL(facts.install.url).host}` }
                          : { state: 'not installed', detail: 'no account needed', free: true }
            const needSaid = need === undefined ? undefined : [need.state, need.detail].filter((part) => part !== undefined).join(' · ')
            const dot = usable ? ' is-green' : checking ? ' is-checking' : signIn ? ' is-red' : onRamp ? ' is-lime' : ' is-muted'
            const waiting = installing !== undefined
            return (
              <div className={`lc-runtimecell${usable ? ' is-ready' : ''}${onRamp ? ' is-onramp' : ''}`} key={runtime.id}>
                <span className={`lc-runtimecell__dot${dot}`} />
                <span className="lc-runtimecell__name" title={status.detail}>
                  <RuntimeMark runtime={runtime.id} size={13} muted={!usable} className="is-inline" />
                  {runtime.displayName}
                </span>
                {need !== undefined && (
                  /*
                    * The STATE alone in the cell, the rest on hover. Two
                    * columns leave the text about twenty characters, and
                    * "installed · not signe…" cut off exactly the half
                    * that told the two cases apart (frame, 2026-09-22).
                    * "not installed" / "not signed in" is parallel, fits,
                    * and is what decides the button beside it.
                    */
                  <span className="lc-runtimecell__need" title={needSaid}>
                    {need.state !== undefined ? (
                      <>
                        <span className="lc-runtimecell__state">{need.state}</span>
                        {/* The one detail worth its width: why this row is the recommendation. */}
                        {need.free === true && <span className="lc-runtimecell__free"> · {need.detail}</span>}
                      </>
                    ) : (
                      need.detail
                    )}
                  </span>
                )}
                {usable ? (
                  <span className="lc-runtimecell__version">
                    {runtime.version === undefined || runtime.version === null ? 'Ready' : `Ready · ${shortVersion(runtime.version)}`}
                  </span>
                ) : installing === runtime.id ? (
                  <span className="lc-runtimecell__tag">Installing…</span>
                ) : checking ? null : signIn && signInCommand(runtime.id) !== undefined ? (
                  <SignInButton runtime={runtime.id} />
                ) : stuck ? (
                  /*
                   * "Check again", not "Install again" (design agent,
                   * 2026-09-18): that CLI is installed, so installing it
                   * again is not the repair. Asking again is the one the
                   * app can perform; the title says what was tried.
                   */
                  <button
                    type="button"
                    className="lc-runtimecell__quiet"
                    title={`${runtime.displayName} is on this machine but did not answer its version check in 5 seconds, four times. Check again asks once more.`}
                    onClick={() => onCheckAgain?.()}
                  >
                    Check again
                  </button>
                ) : waiting ? null : !runtime.installed && installCommand(runtime.id) !== undefined && onInstall !== undefined ? (
                  <button
                    type="button"
                    className={runtime.id === FREE_START_RUNTIME ? 'lc-runtimecell__install is-primary' : 'lc-runtimecell__quiet'}
                    disabled={waiting || npmMissing}
                    title={
                      npmMissing
                        ? 'Node.js is not on this machine, and this installs through npm.'
                        : waiting
                          ? `Waiting for the ${installing ?? ''} install to finish`
                          : installSentence(runtime.id, runtime.displayName)
                    }
                    onClick={() => onInstall(runtime.id)}
                  >
                    Install
                  </button>
                ) : !runtime.installed && vendorUrl(runtime.id) !== undefined ? (
                  // A vendor download is quiet text: it is not the
                  // recommendation, and it was the only boxed button on
                  // the shipped screen.
                  <button
                    type="button"
                    className="lc-runtimecell__quiet"
                    onClick={() => {
                      setLinkRefusal(undefined)
                      openLink(vendorUrl(runtime.id) ?? '', setLinkRefusal)
                    }}
                    title={`Open ${vendorUrl(runtime.id) ?? ''} in your browser`}
                  >
                    Get it ↗
                  </button>
                ) : (
                  <span className="lc-runtimecell__tag">{status.tag}</span>
                )}
              </div>
            )
          })}
        </div>
        {/*
          * Deferred, and saying so. Not a chevron on its own: the
          * sentence is the affordance, and it names what pressing it
          * gets you rather than making that a guess.
          */}
        {deferred > 0 && (
          <button type="button" className="lc-agentmore" onClick={() => setOthersOpen(true)}>
            {/*
              * Derived from the rows it is hiding. See
              * `deferredOthersSentence` -- this sentence has now been
              * wrong about them twice, both times by being fixed.
              */}
            {deferredOthersSentence(
              shown
                .filter((row) => !drawn.includes(row))
                .map((row) => ({ installsFromHere: installCommand(row.runtime.id) !== undefined }))
            )}
          </button>
        )}
        {usageLine}
      </>
    )
  })()
  const startersBlock = startersShown && onStarter !== undefined && onCompareStarter !== undefined
    ? <StarterList onStart={onStarter} onCompareStart={onCompareStarter} {...(onCompare === undefined ? {} : { onCompare })} {...(onStartFromIssue === undefined ? {} : { onIssue: onStartFromIssue })} />
    : null
  const accountsAbove = team.length > 0

  return (
    <div className={`lc-empty${scrolled ? ' is-scrolled' : ''}${tight ? ' is-tight' : ''}`} ref={pane}>
      <div className="lc-empty__inner" ref={inner}>
        {/* The design system's cover: the lockup lighting up once the runtimes have answered, and the teammates. */}
        <HomeCover ready={discoveryPhase === 'ready'} tube={tube ?? 'full'} swarmCalls={swarmCalls} grow={coverGrow} activity={coverActivity} {...(coverStatus === undefined ? {} : { status: coverStatus })} />

        {/*
          * The only words on the screen, both carrying information: what
          * discovery found, and what the app promises about it. Mono, because
          * that reads as machine output rather than marketing -- which is
          * what a local-first tool should sound like.
          */}
        {/*
          * While discovery has not answered, or could not: one mono line of
          * state under the lockup. Once it has answered, the count moves
          * into the list's own head note and the privacy line stays on the
          * boot screen, where it already is (design agent, 2026-09-19).
          */}
        {discoveryPhase !== 'ready' && (
          <p className="lc-claim">
            {discoveryPhase === 'loading'
              ? 'looking for the AI agents on this machine'
              : 'Locust could not look for AI agents — nothing of yours was read'}
          </p>
        )}
        {/*
          * A fresh machine: six tags and no next step read as a wall (first-run
          * drive, 2026-09-06). One sentence says what Locust runs and what to do.
          */}
        {/*
          * The first screen, redrawn (design agent, 2026-09-19). Measured off
          * the shipped frame: six centred blocks of six widths began at 194,
          * 112, 132, 211, 32 and 32px from the pane's left edge -- six left
          * edges, so every line arrived as its own announcement, which is
          * what four beta passes called "the first five minutes argue". One
          * left-aligned column, shared with the composer, makes them a
          * sequence: which do I pick, pick it, what happens when I click,
          * and then what. Prose, not mono: this is a list with words around
          * it, not a lockup.
          */}
        {/* For a first run only (0.610): on a returning person's Home it was first-run copy above their own team (Colin's mockup, 2026-10-04). */}
        {discoveryPhase === 'ready' && teammateCount === 0 && (
          <p className="lc-intro">
            {/*
              * One line (Colin, 2026-09-23: "lets make the text under the tv
              * be a bit cleaner, maybe we can fix up the wordiness as well"),
              * centred under the machine it captions (2026-09-24: "text is way
              * off center" -- it began at the column's edge; shell.css).
              * It was two sentences, the second with a bold white phrase that
              * broke across the line. What is left: whose agents and whose
              * accounts, and the one way in that needs neither.
              *
              * The second sentence is a promise about SOMEBODY ELSE'S price
              * list, so it is checked before it is made: it goes only on
              * positive evidence that OpenCode has nothing free left -- never
              * on "we have not looked", which is the ordinary state at first
              * launch. See `freeStartStillFree`.
              */}
            {/*
              * "AI agents", not "coding agents" (2026-09-26). The tools are
              * coding agents by their makers' name for them, and they do far
              * more than code; Locust is for "quite literally ANY ai user" --
              * code, money, design (Colin) -- and a person here for the second
              * two was told on the first line that this was not for them.
              */}
            {freeStart === 'no' ? 'Your AI agents, on your own accounts.' : 'Your AI agents, on your own accounts. OpenCode works without one.'}
            {/*
              * THE SENTENCE, PRESSABLE (0.514). A first-hour pass on 0.512:
              * Home said OpenCode works without an account while the chat box
              * sat on Claude, and nothing to press said "start free". Shown
              * only when it would change something: OpenCode has a free model
              * and the chat box is not already on OpenCode.
              */}
            {freeStart === 'yes' && onUseFree !== undefined && (
              <>
                {' '}
                <button type="button" className="lc-linkbutton" onClick={onUseFree}>
                  Use a free model
                </button>
              </>
            )}
          </p>
        )}

        {accountsAbove && accountsBlock}

        {/* What Scout noticed (0.756): what needs the person now leads, before what ended while they were away. */}
        {scout}

        {/* The morning-after view (0.590): first, because it is what a person back at the window came to see. */}
        {away !== undefined && onOpenAway !== undefined && onAwaySeen !== undefined && (
          <AwayList summary={away} teammates={awayTeammates} onOpen={onOpenAway} onSeen={onAwaySeen} {...(onOpenRoutines === undefined ? {} : { onOpenRoutines })} {...(awaySaid === undefined ? {} : { saidOf: awaySaid })} />
        )}

        {discoveryPhase === 'ready' && team.length > 0 && onMessageTeammate !== undefined && (
          <HomeTeam team={team} onMessage={onMessageTeammate} {...(onNewTeammate === undefined ? {} : { onNewTeammate })} {...(teamWatch === undefined ? {} : { watch: teamWatch })} {...(teamInvite === undefined ? {} : { invite: teamInvite })} />
        )}

        {rosterUnreadable && (
          <div className="lc-rosternotice" role="status">
            <p className="lc-rosternotice__happened">Your teammates file could not be read, so your team is not shown.</p>
            <p className="lc-rosternotice__safe lc-mono">Nothing was changed. The file is kept as it is and nothing is saved over it until it reads again.</p>
          </div>
        )}

        {!accountsLead && teamAndFolder}

        {!accountsAbove && accountsBlock}

        {/*
          * STARTERS, ONE ROW EACH, COMPARE ON EVERY ONE (2026-10-19, StarterList.tsx): what a conversation can begin
          * with, pressing one fills the box for the teammate, and its Compare fills the same words for two models.
          */}
        {startersBlock}

        {/*
          * ONE line under the panel, because only one install ever runs.
          *
          * At rest it is the command that would run, said before anything
          * happens rather than after -- an app that is about to run
          * `npm install -g` shows the line unprompted. While an install runs
          * it carries npm's last line, which is what makes forty seconds look
          * alive without turning the screen into a terminal. There is no
          * progress bar: npm reports nothing that honestly becomes a
          * percentage, so the screen shows the number it actually has.
          */}
        {/*
          * Said whenever npm is missing, not only when NOTHING is connected.
          *
          * This was gated on `connected === 0`, so the moment one runtime
          * happened to be found -- Codex signs itself in on many machines --
          * the sentence explaining why every Install button is switched off
          * disappeared, and the reason survived only in a tooltip. The first
          * outside tester on 0.55.0 had exactly that: Codex connected, every
          * other row offering a button that "does nothing", and no visible
          * reason anywhere. A disabled control has to say why it is disabled
          * on the screen, not on hover.
          */}
        {discoveryPhase === 'ready' && npmMissing && (
          <p className="lc-installnote lc-tone-amber">
            {/*
              * Not "below". The note renders under the list, so the buttons
              * it is about are ABOVE it -- pointing the wrong way at the one
              * moment a person is already lost (Grok, pass 9). Naming no
              * direction is better than naming one: there is exactly one set
              * of Install buttons on this screen.
              */}
            Node.js is not on this machine, so the Install buttons cannot run.{' '}
            <button type="button" className="lc-linkbutton" onClick={() => {
                setLinkRefusal(undefined)
                openLink('https://nodejs.org', setLinkRefusal)
              }}>
              Get Node.js ↗
            </button>
          </p>
        )}
        {/*
          * Node is absent and the buttons still work, which is a surprise
          * worth explaining before it becomes one. Ian downloaded Locust
          * and nothing worked until he installed Node; since 0.178.0 the
          * app carries its own npm and runs it with its own binary as the
          * Node, so the install goes through. The part that does NOT change
          * is the person's terminal: npm writes launcher shims that call
          * node by name, so outside Locust the CLI still needs one. Saying so
          * here costs one sentence; being discovered costs an evening.
          */}
        {/*
          * And only while there is an Install button for it to be about.
          * With every row still CHECKING there is none, and a sentence about
          * how an install will go, under a list that offers no install, was
          * one of the three things Colin's frame had arguing at once.
          */}
        {/*
          * AND ONLY ONCE AN INSTALL IS ACTUALLY UNDER WAY (2026-09-19).
          *
          * The condition above already stopped it appearing over a list that
          * offered no install. It still put two sentences of technical
          * explanation on the screen BEFORE the person had pressed anything,
          * and it was the longest run of prose there -- measured on the first
          * frame anybody has captured of this screen on a machine with
          * nothing installed (`_tools/bare-machine-frame.mjs`).
          *
          * The sentence exists to REASSURE: the install will work anyway.
          * That is a good thing to say to somebody watching an install, and
          * the wrong thing to say to somebody who has not started one --
          * "Node.js is not on this machine" reads as a problem statement to
          * anyone who does not already know what Node.js is, which is exactly
          * the person this screen is being rebuilt for.
          *
          * So it now waits for the press. An install that fails still says
          * everything, through `installFailure` below, which carries the
          * command and the repair.
          */}
        {/*
          * TWO SENTENCES, TWO MOMENTS -- and they were one paragraph.
          *
          * Sol's beta review, 2026-09-21, finding 8: OpenCode finished, the
          * row said Ready, and this paragraph was still under it, opening
          * with *"Node.js is not on this machine"* on a screen that had just
          * succeeded. Sol: clear the install note when the install finishes.
          *
          * The obvious fix -- hold the whole thing to `installing` -- is
          * wrong, and `install-buttons-say-why.test.ts` caught it. Ian's case
          * is the opposite one: he installed through us, went to his OWN
          * terminal afterwards, and the CLI was not there. His sentence has
          * to OUTLIVE the spinner, which is exactly when Sol's has to go.
          *
          * So they are split by who is being spoken to. The bundled-npm
          * reassurance belongs to the person watching a progress line and
          * ends with it; the terminal note belongs to the person who has
          * finished and stays. Neither piece of evidence loses.
          */}
        {discoveryPhase === 'ready' && installing === undefined && connected > 0 && npmIsBundled && (
          <p className="lc-installnote">
            Agents installed from here work inside Locust. To use one in your own terminal too, install{' '}
            <button
              type="button"
              className="lc-linkbutton"
              onClick={() => {
                setLinkRefusal(undefined)
                openLink('https://nodejs.org', setLinkRefusal)
              }}
            >
              Node.js ↗
            </button>
          </p>
        )}
        {discoveryPhase === 'ready' && installing !== undefined && npmIsBundled && rows.some((row) => !row.runtime.installed && installCommand(row.runtime.id) !== undefined) && (
          <p className="lc-installnote">
            {/*
              * WHICH of the two it was. The probe answers "not usable" for a
              * machine with no Node AND for an npm that is right there and
              * never answers -- nvm-windows with no version picked, a
              * corporate wrapper waiting on a proxy. One sentence covered
              * both, and Fable measured it on a box with Node on PATH and
              * only npm hanging: the screen told them Node was not installed
              * (pass 2, finding 3). The host always knew; it stopped saying.
              */}
            {npmDidNotAnswer
              ? 'npm is on this machine but did not answer, so Locust installs with the copy of npm it carries.'
              : 'Node.js is not on this machine, so Locust installs with the copy of npm it carries.'}{' '}
            The CLI will work here. To use it in your own terminal too, install{' '}
            <button type="button" className="lc-linkbutton" onClick={() => {
                setLinkRefusal(undefined)
                openLink('https://nodejs.org', setLinkRefusal)
              }}>
              Node.js ↗
            </button>
          </p>
        )}
        {linkRefusal !== undefined && (
          <p className="lc-installnote lc-tone-amber" role="status">
            {linkRefusal}
          </p>
        )}
        {installFailure !== undefined && (
          <div className="lc-installnote lc-installnote--failed" role="alert">
            <div className="lc-installnote__what">{installFailure.what}</div>
            <div className="lc-installnote__next">{installFailure.next}</div>
            {(installLog?.length ?? 0) > 0 && (
              <>
                {/*
                  * On a failure the trace is the thing worth reading, so the
                  * disclosure is here too -- and the LAST lines usually name
                  * the cause, which is what the unknown-failure copy tells
                  * people to look for.
                  */}
                <button
                  type="button"
                  className="lc-ghostbutton"
                  aria-expanded={outputOpen}
                  onClick={() => setOutputOpen(!outputOpen)}
                >
                  {outputOpen ? 'Hide output' : 'Show output'}
                </button>
                {outputOpen && (
                  <pre className="lc-installoutput lc-mono">{installLog?.join('\n')}</pre>
                )}
              </>
            )}
            {/*
              * THE COMMAND NEVER DISAPPEARS. It stops being the only option;
              * on a failure it is the option that works, because a person who
              * cannot read an npm trace can paste that line to someone who
              * can (FIRST-RUN-INSTALL-DESIGN, 2026-09-06).
              */}
            {installFailure.command !== undefined && (
              <div className="lc-installnote__command">
                <code className="lc-mono">{installFailure.command}</code>
                <button
                  type="button"
                  className="lc-runtimecell__install"
                  onClick={() => {
                    void navigator.clipboard?.writeText(installFailure.command ?? '').catch(() => undefined)
                  }}
                >
                  Copy
                </button>
              </div>
            )}
          </div>
        )}
        {installLine !== undefined && installFailure === undefined && (
          <div className="lc-installnote">
            {/*
              * What is being run, while it runs. It used to appear only after
              * a failure, so a person watching a 90-second install had no way
              * to know what it was doing or to run it themselves instead.
              */}
            {installing !== undefined && installCommand(installing) !== undefined && (
              <code className="lc-installnote__running lc-mono">
                {/*
                  * The command that RUNS, not the one a person would type.
                  * With no Node on the machine the app runs its own npm with
                  * its own binary, allows the package's install script, and
                  * installs into its own folder; the screen said plain
                  * `npm install -g …`, which fails in that person's terminal
                  * for the reason the note above gives (Fable, pass 1).
                  */}
                {npmIsBundled
                  ? `${installCommand(installing) ?? ''} --allow-scripts=${(() => { const facts = runtimeInstallFacts(installing); return facts?.install.kind === 'npm' ? facts.install.packageName : '' })()} --prefix <Locust's own folder>  (with the npm Locust carries)`
                  : installCommand(installing)}
              </code>
            )}
            <div className="lc-installnote__live">
              <span className="lc-mono lc-installnote__lastline">{installLine}</span>
              {(installLog?.length ?? 0) > 0 && (
                <button
                  type="button"
                  className="lc-ghostbutton"
                  aria-expanded={outputOpen}
                  onClick={() => setOutputOpen(!outputOpen)}
                >
                  {outputOpen ? 'Hide output' : 'Show output'}
                </button>
              )}
            </div>
            {/*
              * npm's own words, bounded and scrollable. The elapsed count in
              * the line above is the honest substitute for a progress bar --
              * npm reports nothing that can become a percentage -- and this
              * is for the person who wants to know WHAT it is doing, or who
              * needs to hand the trace to somebody else.
              */}
            {outputOpen && (
              <pre className="lc-installoutput lc-mono">{installLog?.join('\n')}</pre>
            )}
          </div>
        )}

        {accountsLead && teamAndFolder}

        {/*
          * The privacy claim is made once, in the claim line above; the
          * instruction to pick a teammate is the sidebar's (its missions
          * empty state says it) -- the review found both said twice.
          */}
        {/*
          * "coming soon" is not drawn here any more. Design agent,
          * 2026-09-18: it names two things a person cannot install on a
          * screen whose one job is installing something -- the only line on
          * the page that cannot be acted on. Settings lists the same
          * runtimes as PLANNED rows, where a person who is already working
          * and wants more is the audience.
          */}
      </div>
    </div>
  )
}
