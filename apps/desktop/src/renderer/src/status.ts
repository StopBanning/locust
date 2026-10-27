import { FREE_START_RUNTIME, installSentence } from '../../shared/runtime-install.js'
import { FREE_MODELS_BEST_FIRST } from '../../shared/free-models.js'
import { integrationOf, RUNTIME_INTEGRATION } from '../../shared/runtime-integration.js'
import type { IntegrationLevel } from '../../shared/runtime-integration.js'
import { hostCanRunMission, isMissionRuntime, runtimeDisplayName } from '../../shared/runtimes.js'
import { faceLabel, teammateActivity } from './faceState.js'
import type { FaceActivity, LiveActivity } from './faceState.js'
import type { MissionMode, PublicModel, PublicRecoveredMission, PublicRuntimeStatus } from '../../shared/ipc.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'

/**
 * Every status word the shell shows, derived from discovery and mission state
 * and from nothing else.
 *
 * This module exists because the product's whole claim is that you can always
 * tell which runtime and model are really in play. A label that can drift from
 * what discovery proved -- a hard-coded `LIVE`, an optimistic default, a chip
 * that survives a failed probe -- turns that claim into decoration. So the
 * labels are pure functions with tests, and the tests are mutation-checked.
 *
 * The load-bearing invariant, stated once: **nothing renders as LIVE or READY
 * unless discovery reported the runtime ready.** Everything else here is
 * presentation.
 */

/** Route-picker and settings status tags, in the design's vocabulary. */
export type RouteTag =
  | 'ACTIVE'
  | 'READY'
  /** Signed in, and the last run on it ended on the account's own usage limit. */
  | 'AT LIMIT'
  | 'PREVIEW'
  | 'SIGN IN'
  | 'NOT INSTALLED'
  | 'UNAVAILABLE'
  /** Installed, but its probe did not answer in time; discovery asks again. */
  | 'CHECKING'
  | 'API'
  | 'LOCAL'
  | 'PLANNED'
  | 'EXPERIMENTAL'

// The integration map moved to shared/ so the main process can read it too.
export { integrationOf, RUNTIME_INTEGRATION }
export type { IntegrationLevel }

export interface RouteRowStatus {
  readonly tag: RouteTag
  /** Whether the row may be chosen as the active route right now. */
  readonly selectable: boolean
  /** Sentence shown under the row. Always says why, never just what. */
  readonly detail: string
}

/**
 * The one function that may say a runtime is usable. `ready` comes from a
 * probe that actually ran; `status` distinguishes why a runtime is not.
 */
/**
 * The route a mission starts on when nobody has chosen one.
 *
 * `codex/account-default` was hard-coded in three places -- the composer's
 * seeded route, a room post for a teammate with no route of its own, and the
 * relay's borrowed route -- and nothing ever moved any of them. So a person
 * who installed the runtime the app told them to install still had a composer
 * pointing at Codex, which was not on their machine: the box read "Install a
 * coding agent and sign in to start a mission…", Enter did nothing, and no
 * part of the screen said why (QA, 2026-09-06, reproduced live). Their first
 * room post would have refused every teammate for the same reason.
 *
 * Preference order, and each part is deliberate:
 *
 * 1. A runtime that can actually run right now. A default naming something
 *    absent is the whole defect.
 * 2. Among those, Claude Code, then Codex, then OpenCode (0.307). Colin,
 *    2026-09-23, with the beta's pitch -- "full model control in a great
 *    UI, not free. Most users will be on Claude or Codex plans; OpenCode is
 *    a bonus" -- and "don't start on OpenCode when Claude or Codex is signed
 *    in": "will run with your default setup". Usable already means signed
 *    in and answering, so a Claude Code that is installed and signed out is
 *    passed over. OpenCode, which needs no account, is still the start for
 *    anyone without either -- the first-run screen recommends it by name.
 *    It used to come first for everyone, which put a plan user's first
 *    message on a free model they had not chosen.
 * 3. Failing everything, still OpenCode -- the runtime the first-run screen
 *    is at that moment telling them to install. This used to fall back to
 *    Codex "as a stable answer for a machine mid-probe", and a first outside
 *    tester read the result exactly as it sounds (2026-09-07): a welcome
 *    screen selling OpenCode above a composer claiming `Codex CLI /
 *    account-default`, on a machine with no CLIs at all. A default that names
 *    something absent is the defect this function exists to fix, and naming
 *    the one the app is recommending is at least a claim the next click can
 *    make true.
 *
 * The MODEL is left as `account-default` here on purpose: which model to
 * prefer is the catalogue's business, not discovery's, and the picker is what
 * knows a free one exists.
 */
/** Who a fresh profile starts on, when each can run (see defaultRoute). */
export const START_ORDER: readonly MissionRuntimeId[] = ['claude', 'codex', FREE_START_RUNTIME]

export function defaultRoute(runtimes: readonly PublicRuntimeStatus[]): {
  readonly runtime: MissionRuntimeId
  readonly model: string
} {
  // `hostCanRunMission` takes a mission runtime; the settings list carries a
  // wider set of ids (omniroute chooses a route rather than being one), so the
  // narrowing happens here rather than at every call site.
  const usable = runtimes.filter(
    (runtime) => runtimeIsUsable(runtime) && isMissionRuntime(runtime.id) && hostCanRunMission(runtime.id)
  )
  const chosen = START_ORDER.map((id) => usable.find((runtime) => runtime.id === id)).find((runtime) => runtime !== undefined) ?? usable[0]
  return {
    runtime: (chosen?.id as MissionRuntimeId | undefined) ?? FREE_START_RUNTIME,
    model: ACCOUNT_DEFAULT_MODEL
  }
}

/**
 * The model a route carries before the catalogue has been read.
 *
 * It is a real thing to run -- "whatever this account gives you" -- but it is
 * not a row in any list, so a route left on it has no ACTIVE row in the
 * picker, and everything the picker hangs on that row (the effort levels,
 * most of all) has nothing to attach to. Measured on a fresh profile
 * 2026-09-07: 0 ACTIVE rows, 0 effort chips.
 */
export const ACCOUNT_DEFAULT_MODEL = 'account-default'


export function runtimeIsUsable(runtime: PublicRuntimeStatus): boolean {
  return runtime.ready && runtime.status === 'ready'
}

/**
 * WHETHER ANY RUNTIME IS USABLE, OR INSTALLED -- AND NOTHING WHILE IT IS NOT
 * KNOWN YET (0.563). Discovery lists every runtime it knows, installed or not,
 * so its first answer is never empty; before it, the window holds an empty
 * list, and `[].some(...)` said "nothing is usable". Every teammate was
 * BLOCKED for the first seconds of every launch: a screen showed red crosses
 * for eyes, and a pet slumped in its failure (found by drive-pet-looks.mjs).
 * Not known is not blocked: until discovery answers, this says nothing, and a
 * teammate's face is whatever else it is.
 */
export function runtimeReach(
  runtimes: readonly PublicRuntimeStatus[],
  known: boolean
): { readonly anyRuntimeUsable?: boolean; readonly anyRuntimeInstalled?: boolean } {
  if (!known) return {}
  const usable = runtimes.some(runtimeIsUsable)
  // A partial sweep can prove that one works, but cannot prove none work yet.
  const pending = runtimes.some((entry) => entry.checking === true)
  return {
    ...(usable || !pending ? { anyRuntimeUsable: usable } : {}),
    anyRuntimeInstalled: runtimes.some((entry) => entry.installed)
  }
}

/**
 * The tag a route row wears, given what discovery proved and whether this is
 * the row the composer is set to.
 *
 * ACTIVE is drawn in the same lime as READY and reads as a stronger claim, so
 * it may only sit on a row that could actually run. A fresh install defaults
 * the route to Codex; before this, that row wore a lime ACTIVE tag directly
 * above the sentence "Codex CLI was not found on this machine."
 */
/**
 * What a signed-in runtime says about itself. Exported because the welcome
 * screen states it ONCE above the list instead of six times inside it, and a
 * second copy of the sentence would drift from this one (design pass,
 * 2026-09-05).
 */
export const SIGNED_IN_DETAIL = 'Signed in on this machine, using your own account.'

/**
 * What a runtime says when it is ready WITHOUT an account.
 *
 * One runtime is in this state and it is the important one: OpenCode's free
 * model runs with no sign-in, which is the entire on-ramp for a person who
 * has just installed the app. Saying "signed in, using your own account"
 * about it is both false and discouraging in the same breath.
 */
export const NO_ACCOUNT_DETAIL = 'Ready. Its free model needs no account.'

export function routeRowTag(status: RouteRowStatus, isActive: boolean): RouteTag {
  // AT LIMIT outranks ACTIVE: the active route being the one that just
  // refused to run is exactly what a person needs to see.
  if (status.tag === 'AT LIMIT') return status.tag
  return isActive && status.selectable ? 'ACTIVE' : status.tag
}

/**
 * @param limited  The runtime's own words the last time a run on it ended on
 *   the account's usage limit, while nothing on it has completed since.
 *   READY means "signed in"; it had been saying so beside a runtime that had
 *   just refused two missions in a row (user session 1, 2026-09-05), and the
 *   picker offered every model of it as READY. The row stays selectable --
 *   the limit is the account's and may lift on the provider's clock -- but
 *   the tag and the sentence say what happened.
 */
export function routeRowStatus(
  runtime: PublicRuntimeStatus,
  integration: IntegrationLevel,
  isActive: boolean,
  limited?: string
): RouteRowStatus {
  const status = baseRouteRowStatus(runtime, integration, isActive)
  if (limited === undefined || !status.selectable) return status
  return {
    tag: 'AT LIMIT',
    selectable: true,
    detail: `Signed in, but the last run on it ended on your account's usage limit: ${limited}`
  }
}

function baseRouteRowStatus(
  runtime: PublicRuntimeStatus,
  integration: IntegrationLevel,
  isActive: boolean
): RouteRowStatus {
  if (integration === 'planned') {
    return {
      tag: 'PLANNED',
      selectable: false,
      detail: 'Not built yet. Shown so the roadmap is visible, not because it works.'
    }
  }
  if (!runtime.installed) {
    return {
      tag: 'NOT INSTALLED',
      selectable: false,
      // "Install it and sign in" is true and it is a dead end: it names the
      // problem and leaves the person to go and find the answer, which is the
      // wall anyone opening Locust for the first time hits before they have
      // seen it do anything. This says which package, and what signing in
      // takes -- or that nothing does.
      detail: installSentence(runtime.id, runtime.displayName)
    }
  }
  if (runtime.status === 'auth-required' || runtime.auth === 'unauthenticated') {
    return {
      tag: 'SIGN IN',
      selectable: false,
      detail: `Installed, but ${runtime.displayName} is not signed in.`
    }
  }
  if (runtime.status === 'unsupported') {
    // A version Locust cannot run yet (0.805, OpenCode 2): a fact to act on, not a check to wait out.
    return {
      tag: 'UNAVAILABLE',
      selectable: false,
      detail: runtime.unsupportedNote ?? `This version of ${runtime.displayName} is not supported yet.`
    }
  }
  if (!runtimeIsUsable(runtime)) {
    // Installed but not answering is a moment, not a verdict: Claude Code's
    // first version probe can outlast the 5 s window on a cold start, and
    // Colin's launch screen said UNAVAILABLE for a runtime that was fine a
    // minute later (2026-09-05). The shell asks again; the tag says so.
    const reason =
      runtime.checking === true
        ? 'is still being checked'
        : runtime.status === 'offline'
          ? 'could not be reached'
          : runtime.status === 'probe-failed'
            ? 'did not answer its version probe in time'
            : 'is not ready'
    return {
      tag: 'CHECKING',
      selectable: false,
      detail: `${runtime.displayName} ${reason}. Locust asks again shortly.`
    }
  }
  // Ready, and real, but on a surface nobody promised: selectable, and said.
  // Antigravity through its CLI is a published, measured route: no tag (0.541).
  if (integration === 'experimental' && runtime.throughCli !== true) {
    return {
      tag: 'EXPERIMENTAL',
      selectable: true,
      detail: `${runtime.displayName} is driven through an unpublished interface of the running app. It works today and may break with an update.`
    }
  }
  // Ready, and the adapter is only partly built: selectable, never called live.
  if (integration === 'preview') {
    return {
      tag: 'PREVIEW',
      selectable: true,
      detail: runtime.id === 'kilo'
        ? 'Preview. Auto Free needs no account. A turn through Kilo did not finish, and paid models and sign-in have not been measured.'
        : `Signed in and found. Locust's support for ${runtime.displayName} has not been tried through a whole run yet, so parts of a run may not show.`
    }
  }
  return {
    tag: isActive ? 'ACTIVE' : 'READY',
    selectable: true,
    detail: runtime.auth === 'not-applicable' ? NO_ACCOUNT_DETAIL : SIGNED_IN_DETAIL
  }
}

/**
 * Count for the sidebar's "N connected" line.
 *
 * TWO truths have to agree before a runtime may be counted. `runtimeIsUsable`
 * says discovery found the CLI installed and signed in. `integrationOf` says
 * how far THIS build's support for it goes. A runtime can satisfy the first
 * and not the second -- Gemini CLI probes at 0.58.0 and reports itself signed
 * in, while the same screen says "Not built yet. Shown so the roadmap is
 * visible, not because it works."
 *
 * **This is a guard, not a repair, and the difference is worth recording.** It
 * was written on 2026-09-05 while chasing what looked like a miscount -- the
 * footer said 6 where a probe counted 5 -- and the app turned out to be right:
 * the sixth was Antigravity, whose adapter is EXPERIMENTAL and does complete
 * missions, and the probe had been counting the literal word READY. No
 * miscount has ever been observed. What remains true is that nothing here
 * previously stopped a discovered-but-unbuilt runtime from being counted, and
 * `integrationOf` is the one place that knows the difference.
 *
 * Counted through `integrationOf` rather than a second list, for the reason
 * that map exists: a runtime this build does not know is `planned`, so a new
 * one cannot start being counted before anybody has written its adapter.
 */
/**
 * DECIDED 2026-09-05 (Colin: "if antigravity works it works"). An
 * EXPERIMENTAL route counts when discovery says it can run right now --
 * Antigravity only answers while its own app is open with this folder, so
 * the count reflects that moment honestly and drops back when it does not.
 * What is NOT counted is a runtime this build cannot drive at all: `planned`
 * is filtered by name, which is why a design pass's report that Gemini was
 * being counted did not hold.
 */
export function connectedRuntimeCount(runtimes: readonly PublicRuntimeStatus[]): number {
  return runtimes.filter((runtime) => runtimeIsUsable(runtime) && integrationOf(runtime.id) !== 'planned')
    .length
}

/** Sidebar / roster status vocabulary from the design. */
export type TeammateStatus = 'working' | 'approval-needed' | 'idle' | 'blocked'

export interface TeammateStatusView {
  readonly status: TeammateStatus
  /** What their face does, decided here so every surface draws the same thing. */
  readonly activity: FaceActivity
  readonly label: string
  /** Semantic token name, never a literal color. */
  readonly tone: 'live' | 'amber' | 'muted' | 'red'
  readonly pulse: boolean
}

/**
 * A teammate's state in words, for a face that stands with none beside it --
 * the faces strip, the compact rail. There a bot said "working" or "waiting
 * on you" by moving, a dot and a colour, and a screen reader heard only the
 * name (the harness pass, 2026-09-23; Yurt's #4 took colour away as well).
 * Blocked says why, since "blocked" alone is no use to act on.
 */
export function stateInWords(view: TeammateStatusView): string {
  return view.status === 'blocked' ? view.label : faceLabel(view.activity)
}

/**
 * Which runtime a teammate is judged against, decided in ONE place.
 *
 * Their running mission's, then any past mission's, then the route they are
 * SET to use. That last step is Astra's Finding 1 (2026-09-14) and it was
 * missing from all three surfaces identically, because all three wrote this
 * chain out by hand.
 *
 * What she saw: a brand-new Claude teammate on a profile where Claude is not
 * signed in. Settings said, correctly and in red, "Installed, but Claude Code
 * is not signed in", with `run claude` beside it. The sidebar said
 * "Docs & QA - idle". Idle is the word for a teammate with nothing to do; this
 * one could not do anything, the app knew, and the surface a person looks at
 * first said the wrong word.
 *
 * The cause is that a teammate's runtime was read from its MISSIONS, and a
 * teammate that has never run has none -- so the sign-in check was skipped for
 * exactly the teammates a person is most likely to be looking at, the ones
 * they just made. 0.109.0 made all three surfaces draw the same dot; this is
 * the state behind the dot, which was wrong everywhere equally.
 */
export function runtimeOfTeammate(
  teammate: { readonly route?: { readonly runtime: string } },
  owned: readonly { readonly phase: string; readonly runtime?: string }[]
): string | undefined {
  return (
    owned.find((mission) => mission.phase === 'running')?.runtime ??
    owned.find((mission) => mission.runtime !== undefined)?.runtime ??
    teammate.route?.runtime
  )
}

export function teammateStatusView(input: {
  /**
   * The runtime this teammate's own work is on, when they have any. A
   * teammate with no missions has no runtime of their own, and judging every
   * teammate against one hard-coded runtime told people their teammate needed
   * a sign-in while she was visibly working on another one.
   */
  readonly runtime: PublicRuntimeStatus | undefined
  /** Whether ANY runtime could take work right now. */
  readonly anyRuntimeUsable?: boolean
  /**
   * Whether ANY runtime is on this machine at all -- which is a different
   * question from whether one is usable, and the difference is the whole of
   * Grok's finding 2 (2026-09-15).
   */
  readonly anyRuntimeInstalled?: boolean
  readonly hasRunningMission: boolean
  readonly pendingApprovals: number
  readonly roleLabel: string
  /** What their live run is doing right now; `idle` when there is none. */
  readonly liveActivity?: LiveActivity
  readonly recentlyDone?: boolean
  readonly recentlyReceived?: boolean
}): TeammateStatusView {
  // A pending check proves neither a missing tool nor a sign-in wall.
  const ownRuntimeBlocked = input.runtime !== undefined && input.runtime.checking !== true && !runtimeIsUsable(input.runtime)
  const activity = teammateActivity({
    blocked: input.anyRuntimeUsable === false || ownRuntimeBlocked,
    waitingOnYou: input.pendingApprovals > 0,
    live: input.hasRunningMission ? (input.liveActivity ?? 'working') : 'idle',
    recentlyDone: input.recentlyDone === true,
    recentlyReceived: input.recentlyReceived === true
  })
  /*
   * Nobody can work when nothing is usable, whatever this teammate has done
   * before -- but WHY nothing is usable decides what the row should say.
   *
   * Grok's pass 4, 2026-09-15, driving the case this app is about to meet:
   * a machine with no coding CLI at all. Settings said OpenCode **NOT
   * INSTALLED**; the roster row beside it said **Runtime sign-in required**,
   * in red. "There is nothing to sign in to. The CLI is not there."
   *
   * Two facts wore one sentence, and the wrong one was on the screen a
   * person stares at after naming their first teammate -- while the screen
   * that knew the truth was two clicks away. Telling someone to sign in to
   * software they have not installed is an instruction they cannot follow.
   */
  if (input.anyRuntimeUsable === false) {
    return input.anyRuntimeInstalled === false
      ? { status: 'blocked', activity, label: 'No AI agent installed', tone: 'red', pulse: false }
      : { status: 'blocked', activity, label: 'Sign-in needed', tone: 'red', pulse: false }
  }
  // A teammate whose OWN runtime cannot run is blocked, even if a mission
  // looks active in the renderer -- the sign-in wall outranks optimistic
  // local state. One with no runtime AND no route is simply idle; see
  // `runtimeOfTeammate` for why "no missions yet" used to mean the same
  // thing and should not have.
  if (input.runtime !== undefined && ownRuntimeBlocked) {
    // Same distinction, for the teammate's OWN runtime: absent is not
    // unsigned. `installed` is the runtime's own answer, so this needs
    // nothing passed in.
    // And not answering is not signed out (L18): an installed runtime whose
    // probe timed out or could not be reached is asked again, as Settings
    // says -- telling the person to sign in sends them to fix nothing.
    const notAnswering = input.runtime.installed && (input.runtime.status === 'probe-failed' || input.runtime.status === 'offline')
    return {
      status: 'blocked',
      activity,
      label: !input.runtime.installed
        ? 'AI agent not installed'
        : input.runtime.status === 'unsupported'
          ? 'This AI agent version is not supported yet'
          : notAnswering
          ? 'AI agent not answering — checking again'
          : 'Sign-in needed',
      tone: notAnswering ? 'amber' : 'red',
      pulse: false
    }
  }
  if (input.pendingApprovals > 0) {
    return { status: 'approval-needed', activity, label: `${input.roleLabel} · ${faceLabel(activity)}`, tone: 'amber', pulse: false }
  }
  // Copy follows state: the word beside the face is the face's own word.
  if (input.hasRunningMission) {
    return { status: 'working', activity, label: `${input.roleLabel} · ${faceLabel(activity)}`, tone: 'live', pulse: true }
  }
  return { status: 'idle', activity, label: `${input.roleLabel} · ${faceLabel(activity)}`, tone: 'muted', pulse: false }
}

export interface MissionPhaseView {
  readonly label: string
  readonly tone: 'live' | 'blue' | 'amber' | 'red' | 'muted'
  /** Uppercase tag for the missions table. */
  readonly tag: string
}

export function missionPhaseView(
  phase: PublicRecoveredMission['phase'] | 'running',
  hasIntegrityIssues = false
): MissionPhaseView {
  if (phase === 'running') return { label: 'Running', tone: 'live', tag: 'RUNNING' }
  if (phase === 'interrupted') {
    return { label: 'Interrupted', tone: 'red', tag: 'INTERRUPTED' }
  }
  if (phase === 'failed') return { label: 'Failed', tone: 'red', tag: 'FAILED' }
  if (phase === 'cancelled') return { label: 'Cancelled', tone: 'muted', tag: 'CANCELLED' }
  // Completed, but the ledger could not be read to its end: the run finished
  // and its record did not, and the design's receipt card must not print
  // `verified` over that.
  if (hasIntegrityIssues) {
    return { label: 'Completed · receipt incomplete', tone: 'amber', tag: 'COMPLETED' }
  }
  return { label: 'Completed', tone: 'blue', tag: 'COMPLETED' }
}

/**
 * A row's tone in a LIST of conversations (design pass, 0.715): a finished
 * turn is the ordinary case, and drawn blue on every row -- a blue dot and a
 * blue COMPLETED on all sixteen of the everyday list -- it was the loudest
 * thing on the screen, and the rows that need a look (running, stopped short,
 * a record that did not read to its end) had nothing left to stand out with.
 * So a list draws the settled case quiet and keeps the colours for those.
 * A single turn's receipt still says Completed in blue: there it is the news.
 */
export function listTone(view: MissionPhaseView): MissionPhaseView['tone'] {
  return view.tone === 'blue' && view.label === 'Completed' ? 'muted' : view.tone
}

/**
 * The word the receipt card prints against the ledger path. `verified` is a
 * claim about durability, so it requires zero integrity issues -- not merely
 * that a mission was recovered.
 */
/**
 * What was verified, said on the face of it.
 *
 * This returned a bare `verified`, in green, beside `COMPLETED`. Astra's audit
 * of 0.87.1: "it does not name what was verified on its collapsed face... a
 * completed but incorrect artifact can have an intact record without appearing
 * endorsed as correct work."
 *
 * That is the one claim this app must never make by accident. The receipt is
 * about DURABILITY -- that the record of this mission is whole and recovers --
 * and it says nothing at all about whether the work was any good. A green
 * `verified` under a finished turn invites exactly the other reading.
 *
 * The Missions header has always said the same words for the same fact
 * (`Screens.tsx`), so this is also two surfaces agreeing on their own words
 * rather than a new phrase.
 *
 * `record readable`, not `ledger verified` (QA-2026-09-29 round 2, R24): the
 * check is that every file parses and its sequence has no gap. A reply edited
 * at rest, or a whole mission file deleted, reads back clean, so "verified"
 * promised tamper evidence the check does not give.
 */
export function ledgerVerificationLabel(integrityIssueCount: number): 'record readable' | 'record incomplete' {
  return integrityIssueCount === 0 ? 'record readable' : 'record incomplete'
}

/** Short mission id for mono provenance: real UUID prefix, never a fake counter. */
export function shortMissionId(missionId: string): string {
  const bare = missionId.startsWith('mission_') ? missionId.slice('mission_'.length) : missionId
  return bare.slice(0, 8)
}

/** Checkpoint display id. Epochs are 1-based in the ledger. */
export function checkpointLabel(epoch: number): string {
  return `ck_${epoch}`
}

/**
 * Whether the route control can hand the running mission to another runtime.
 *
 * A handoff is addressed by runId, and there is a real window -- between the
 * user submitting and the host's receipt coming back -- where a mission is
 * visibly running but has no id yet. A control offered in that window looks
 * available and silently does nothing, which is the exact failure this shell
 * refuses everywhere else. So the state is named, and the control says why.
 *
 * `switching` is a handoff already in flight: a second one would race the
 * first, and the first is irreversible.
 */
export type HandoffAvailability = 'idle' | 'starting' | 'available' | 'switching'

export function handoffAvailability(
  running: boolean,
  hasRunId: boolean,
  switching: boolean
): HandoffAvailability {
  if (switching) return 'switching'
  if (!running) return 'idle'
  return hasRunId ? 'available' : 'starting'
}

/** What the route control says about itself. `undefined` where a title adds nothing. */
export function handoffTitle(availability: HandoffAvailability): string | undefined {
  if (availability === 'available') return 'Hand this to another AI agent'
  if (availability === 'starting') return 'Waiting for it to start before it can be handed over'
  if (availability === 'switching') return 'Handing it over'
  return undefined
}

/**
 * A face moves only while its teammate is doing something. Everything else --
 * idle, waiting on an approval, blocked -- is still, and says so through the
 * presence dot and the label instead.
 */
export function facePresenceFor(status: TeammateStatus): 'working' | 'approval' | 'blocked' | 'none' {
  if (status === 'working') return 'working'
  if (status === 'approval-needed') return 'approval'
  if (status === 'blocked') return 'blocked'
  return 'none'
}

/**
 * How many rows of one runtime's models the picker shows before it stops and
 * says how many more there are.
 *
 * A route group is usually a handful of rows. One runtime here lists 217, and
 * an unannounced wall of them buries every other runtime below it -- the list
 * scrolls, so the rows underneath are not visibly there at all. The cap is a
 * presentation choice and must never read as "this is all there is", so what
 * it holds back is COUNTED and stated, and typing in the picker's search
 * lifts it entirely.
 */
export const ROUTE_GROUP_LIMIT = 6

export interface CappedRouteRows<TRow> {
  readonly rows: readonly TRow[]
  /** Rows held back per group, by group name. A group at the cap is absent. */
  readonly hiddenByGroup: ReadonlyMap<string, number>
}

/**
 * Cap each group to `limit` rows, keeping the order they arrived in.
 *
 * Two rules make the cap safe to show:
 * - The ACTIVE row is always kept, even when it sits below the cut. A picker
 *   that hides the route you are on cannot be read as a picker at all.
 * - Nothing is dropped silently: every group over the cap reports how many
 *   rows it is not showing, and the caller must say so.
 *
 * While a search is running the cap is WIDER, not gone. It used to be lifted
 * entirely -- "the person has narrowed the list themselves" -- and one letter
 * then produced 92 rows (0.21.2 QA): a search for `a` narrows nothing, and a
 * wall of matches is the thing the cap exists to prevent. What made the cap
 * honest before still makes it honest here: the rows held back are counted
 * and the picker says so, so a narrowing is visible rather than a lie.
 */
export const ROUTE_SEARCH_GROUP_LIMIT = ROUTE_GROUP_LIMIT * 2

export function capRouteRows<TRow extends { readonly group: string; readonly tag: RouteTag }>(
  rows: readonly TRow[],
  limit: number,
  searching: boolean
): CappedRouteRows<TRow> {
  if (searching) limit = Math.max(limit, ROUTE_SEARCH_GROUP_LIMIT)
  const counts = new Map<string, number>()
  const kept: TRow[] = []
  const hidden = new Map<string, number>()
  for (const row of rows) {
    const seen = counts.get(row.group) ?? 0
    counts.set(row.group, seen + 1)
    if (seen < limit) {
      kept.push(row)
      continue
    }
    // Below the cut. The active row displaces the last kept row of its group
    // rather than being hidden, so the count shown stays exactly `limit`.
    if (row.tag === 'ACTIVE') {
      const last = kept.map((entry) => entry.group).lastIndexOf(row.group)
      if (last >= 0) {
        kept.splice(last, 1, row)
        hidden.set(row.group, (hidden.get(row.group) ?? 0) + 1)
        continue
      }
    }
    hidden.set(row.group, (hidden.get(row.group) ?? 0) + 1)
  }
  return { rows: kept, hiddenByGroup: hidden }
}

/** Bytes, in the largest unit that keeps the number readable. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return 'unknown'
  if (bytes < 1024) return `${String(Math.round(bytes))} B`
  const units = ['KB', 'MB', 'GB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value < 10 ? value.toFixed(1) : String(Math.round(value))} ${units[unit]}`
}

/**
 * What a prune preview says out loud, before anyone agrees to it.
 *
 * The count of what would go is never the whole story: some old missions are
 * kept because a conversation being kept continues from them, and some
 * because they are running. A preview that mentioned only the deletions would
 * read as though the rest had been missed.
 */
export function prunePreviewSummary(preview: {
  readonly deleted: readonly string[]
  readonly keptForContinuity: readonly string[]
  readonly keptAsRunning: readonly string[]
}): string {
  const missions = (count: number): string => `${String(count)} turn${count === 1 ? '' : 's'}`
  if (preview.deleted.length === 0) {
    const because: string[] = []
    if (preview.keptForContinuity.length > 0) {
      because.push(`${missions(preview.keptForContinuity.length)} still part of a conversation you are keeping`)
    }
    if (preview.keptAsRunning.length > 0) because.push(`${missions(preview.keptAsRunning.length)} running right now`)
    return because.length === 0
      ? 'Nothing is old enough to delete.'
      : `Nothing would be deleted: ${because.join(', and ')}.`
  }
  const kept: string[] = []
  if (preview.keptForContinuity.length > 0) {
    kept.push(`${missions(preview.keptForContinuity.length)} kept as part of a conversation you are keeping`)
  }
  if (preview.keptAsRunning.length > 0) kept.push(`${missions(preview.keptAsRunning.length)} kept because they are running`)
  return `Delete ${missions(preview.deleted.length)} for good${kept.length === 0 ? '' : `, with ${kept.join(' and ')}`}.`
}

/**
 * Whether a permission mode can actually run on a runtime.
 *
 * Per-action approvals need a runtime that can stop and ask, which today is
 * the Codex app-server alone. Offering the mode anyway meant a person could
 * sit in it with another route selected and have every message refused before
 * it started -- which is exactly what happened to the first person to try it.
 * A control that cannot do its job says so instead.
 */
/**
 * WHAT EACH RUNTIME CAN HONESTLY BE ASKED TO DO.
 *
 * This was a chain of `if` statements ending in `return true`, each clause a
 * war story worth keeping -- and the fall-through was the defect. A runtime
 * added to `MissionRuntimeId` inherited FOUR modes it had never been tested
 * in, silently, because saying nothing meant saying yes.
 *
 * MEASURED 2026-09-21, by adding `muse` to the union: typecheck clean, 3,689
 * tests green, and the app immediately offered Ask, Accept edits, Auto and
 * Plan on a runtime with no adapter, no command builder and no event
 * normalizer. Nothing asked. That is the whole argument for this table, made
 * by accident while building something else.
 *
 * The type is an EXHAUSTIVE `Record`, which is the point: the next runtime
 * added to the union will not compile until somebody says what it can do.
 * A declaration is cheap; discovering the answer from a screenshot six weeks
 * later is what this project actually did for six runtimes
 * (`FINDING-2026-09-20-THE-ORBS-DEPEND-ON-WHAT-A-RUNTIME-STREAMS.md`).
 *
 * `evidence` is not decoration. Builder.io's conformance suite proves two
 * things of an adapter -- that it does not advertise what it cannot do, and
 * that an un-advertised capability THROWS rather than silently no-ops. This
 * is the first half. `unproven` means the row is a placeholder and the
 * runtime offers nothing, which is the only honest default.
 */
export interface RuntimeCapabilities {
  /**
   * The modes this runtime can actually run. `plan` is deliberately absent:
   * it is available exactly where read-only containment is real, so it is
   * DERIVED from `ask` below rather than declared twice and allowed to drift.
   */
  readonly modes: readonly Exclude<MissionMode, 'plan'>[]
  /** Modes that are not real on a given platform, whatever the list says. */
  readonly notOn?: Readonly<Record<string, readonly Exclude<MissionMode, 'plan'>[]>>
  readonly evidence: 'measured' | 'unproven'
}

const EVERY_ORDINARY_MODE = ['ask', 'accept-edits', 'auto'] as const

export const RUNTIME_CAPABILITIES: Readonly<Record<MissionRuntimeId, RuntimeCapabilities>> = {
  // The only runtime with a real per-call approval: `approve-each` exists
  // because Codex asks before each tool and waits for an answer.
  codex: { modes: [...EVERY_ORDINARY_MODE, 'approve-each'], evidence: 'measured' },
  /*
   * Claude was briefly Ask-only here, on the reading that it could not edit.
   * It can: `--permission-mode acceptEdits` with the editing tools named. The
   * contradiction that prompted it -- "Accept edits" in the composer beside
   * "read-only" in the header -- was fixed properly by making the mode decide
   * the argv, in `createClaudePrintCommand`. Removing the capability had been
   * the wrong repair for the right complaint.
   *
   * Approve each (0.837): its own default mode asks Locust's permission host before every edit and every command,
   * MEASURED 2026-10-26 on 2.1.311 (commands.ts, createClaudePrintCommand).
   */
  claude: { modes: [...EVERY_ORDINARY_MODE, 'approve-each'], evidence: 'measured' },
  /*
   * Cursor's read-only mode is only real where its sandbox can run. On
   * Windows the host refuses such a mission rather than record a containment
   * it cannot keep -- so offering the mode there would be offering a refusal,
   * and every message sent under it came back as an error.
   */
  // Ask on Windows too (0.485): Cursor's own `--mode ask` held against three
  // pushed writes, measured 2026-09-30 (commands.ts cursorCanEnforceReadOnly).
  cursor: { modes: [...EVERY_ORDINARY_MODE], evidence: 'measured' },
  /*
   * NOTHING, AND IT TOOK A SEPARATE COMMIT TO SAY SO.
   *
   * Gemini is listed PLANNED / "Not built yet", no mission can run under it,
   * and its event stream has never been captured -- Google refuses the CLI
   * to consumer accounts, so nobody here can capture one. It claimed three
   * modes anyway, inherited from the old `modeRunsOn` fall-through where
   * saying nothing meant yes.
   *
   * It was left alone when the table was written, deliberately: that commit
   * was a refactor carrying a control test proving every runtime answered
   * exactly as before, and correcting a claim inside it would have made the
   * control lie about what it had checked. Flagging it was the whole value
   * of writing the table down. This is the separate commit it was flagged
   * for.
   *
   * Muse went the other way on the same day -- `modes: []` until a run
   * proved two, then exactly those two. Same rule, both directions: a mode
   * is claimed when a run has proved it and not before.
   */
  gemini: { modes: [], evidence: 'unproven' },
  // Approve-each through `opencode serve`, which stops and asks (A6.7,
  // measured 2026-09-25): `run` could only reject what it would have asked.
  opencode: { modes: [...EVERY_ORDINARY_MODE, 'approve-each'], evidence: 'measured' },
  // Approve-each through the Agent Client Protocol (`copilot --acp`, 0.377):
  // Copilot stops and asks before each action -- measured 2026-09-26, and
  // driven (drive-copilot-approve-each). Every other mode stays on `-p`.
  copilot: { modes: [...EVERY_ORDINARY_MODE, 'approve-each'], evidence: 'measured' },
  /*
   * Antigravity runs its agent under its own policy and Locust holds nothing
   * there: no handle keeps it read-only, and none bounds it for Auto. The one
   * mode it can honestly offer is the one it offers.
   */
  // Through Antigravity CLI (0.540, measured 2026-10-02): read-only refuses
  // writes and commands, Edit allows edits, Auto allows all.
  antigravity: { modes: [...EVERY_ORDINARY_MODE], evidence: 'measured' },
  /*
   * TWO MODES, AND THE THIRD IS LEFT OFF DELIBERATELY.
   *
   * Colin, 2026-09-21: *"we have no control over whether opencode continues
   * to support muse so its better that we integrate it ourself"*. This row
   * was empty until a run had actually happened. One has, twice, through the
   * real command builder, the real process runner and the real normalizer --
   * `node _tools/drive-muse-echo.mjs`, on the free echo provider, once
   * read-only and once workspace-write. Both reached `run.completed`, and
   * the prompt came back in the answer, so the prompt file arrived.
   *
   * `auto` was left off while `createMuseExecCommand` kept Muse's own
   * sandbox on in every mode: it would have been Accept edits under a name
   * that promises more. 0.773 claims it, because Auto now passes
   * `--disable-sandbox` -- measured 2026-10-15: `Write-Output hello` ran to
   * exit 0 on a PC whose Muse sandbox was never set up, where Edit's every
   * command failed before it started. A Locust user is never asked to run an
   * elevated setup to get a teammate that can run commands.
   */
  muse: { modes: [...EVERY_ORDINARY_MODE], evidence: 'measured' },
  /*
   * Ask and Plan map to Kilo's ask agent (unnamed tools denied). Edit and
   * Auto map to its code mode. Approve each is not offered: code mode runs
   * what its own permissions allow without a card. Those mappings are in
   * the launch path. A finished turn is not: Auto Free through `kilo acp`
   * did not return (2026-10-26), so the evidence stays unproven. Paid models
   * and sign-in were not measured.
   */
  kilo: { modes: [...EVERY_ORDINARY_MODE], evidence: 'unproven' },
  /*
   * AN AGENT THE PERSON ADDED (0.777): Approve each and nothing else. It is the one mode the Agent Client Protocol
   * itself makes true -- every action the agent asks to take arrives as session/request_permission, answered on a card
   * -- whatever the agent is. Its own modes (OpenCode's build and plan, Goose's auto) are its own ids with its own
   * meanings, never measured here, so none is claimed. Measured by _tools/drive-your-own-acp-agent.mjs.
   */
  acp: { modes: ['approve-each'], evidence: 'measured' }
}

export function modeRunsOn(
  mode: MissionMode,
  runtime: MissionRuntimeId,
  platform?: string
): boolean {
  // Plan is available exactly where read-only containment is real. A plan that
  // could edit the workspace is a promise the app cannot keep.
  if (mode === 'plan') return modeRunsOn('ask', runtime, platform)
  const facts = RUNTIME_CAPABILITIES[runtime]
  if (!facts.modes.includes(mode)) return false
  const barred = platform === undefined ? undefined : facts.notOn?.[platform]
  return barred === undefined || !barred.includes(mode)
}

/** The modes a route can actually run, in the order they are offered. */
/**
 * A mission mode in the words the composer uses for it.
 *
 * The roster card printed the literal string `read-only` for every teammate,
 * whatever mode they had actually run in -- a fact the card never had. Once a
 * runtime could edit, that line was simply false. MEASURED 2026-09-03 while
 * reading the Teammates screen.
 */
/**
 * Every mode, and every phrasing of it, in ONE table.
 *
 * There were four, in four files: the composer's `MODES` rows, `modeLabel`
 * here, `modeSentence` in `IdleTeammate.tsx`, `modeSummary` in
 * `NewTeammateDialog.tsx`. Nothing made them agree and they already did not --
 * the same mode was `Ask` in one place and `ask` in another, and only two of
 * the four said Ask is read-only, which is the single most load-bearing fact
 * about it (Grok's audit, 2026-09-13). Four tables is four chances to be wrong
 * about what a run may touch, and this app has been wrong about that before:
 * the histories on `modeSummary` and on the composer's connector sentence are
 * both records of shipping a permission claim that the run did not have.
 *
 * The registers stay different on purpose -- a picker row, a receipt line, a
 * summary card and a second-person sentence are not the same writing. What is
 * shared is the FACTS, and they are these.
 */
export interface ModeFacts {
  readonly mode: MissionMode
  /** Title case, the way a control names it. */
  readonly name: string
  /** The clause a receipt appends to say what it may touch, where one adds anything. */
  readonly scope?: string
  /** A short lowercase clause, for a summary card that leads with the name. */
  readonly short: string
  /** One full third-person sentence, for a picker row that has the room. */
  readonly consequence: string
  /** Said to the person, second person, where they are about to choose. */
  readonly sentence: string
  /**
   * The chip's word: the slash command's own name for the mode.
   *
   * Colin, 2026-09-23, with a frame of the metal composer wrapping onto two
   * lines: "maybe we can reword accept edits to make room". The menu keeps the
   * full names and what each one does; the chip on the row says it in the
   * word `/edit` already uses.
   */
  readonly chip: string
}

/** In the order the composer offers them. */
export const MODE_FACTS: readonly ModeFacts[] = [
  {
    mode: 'ask',
    name: 'Ask',
    chip: 'Ask',
    scope: 'read-only',
    short: 'reads and explains, every write refused',
    consequence: 'Reads and explains. It cannot change any file.',
    sentence: 'In Ask mode nothing is changed: every write is refused.'
  },
  {
    mode: 'accept-edits',
    // The chip's word, everywhere a person reads the mode (0.309). A practice
    // tester on 0.306: "The compact mode control says Edit while its menu
    // says Accept edits." It was "Accept edits" -- Claude Code's own term --
    // in the menu and in every message, and "Edit" on the chip beside them.
    name: 'Edit',
    chip: 'Edit',
    short: 'may change files in this workspace',
    consequence: 'May edit files inside this workspace folder, and nowhere else.',
    sentence:
      'Edit is on, so it may change files here; switch to Ask below to keep it read-only.'
  },
  {
    mode: 'plan',
    name: 'Plan',
    chip: 'Plan',
    scope: 'read-only',
    short: 'answers with the steps it would take, changes nothing',
    consequence: 'Answers with the steps it would take, and changes nothing.',
    sentence: 'In Plan mode it answers with the steps it would take and changes nothing.'
  },
  {
    mode: 'approve-each',
    name: 'Approve each action',
    chip: 'Approve',
    short: 'asks before every command or change',
    consequence: 'Stops and asks before every command or file change.',
    sentence: 'Every change waits for your approval before it lands.'
  },
  {
    mode: 'auto',
    name: 'Auto',
    chip: 'Auto',
    scope: 'whole machine',
    short: 'may edit anything on this machine',
    consequence:
      'Runs without asking and may change files anywhere on this machine, not only this folder.',
    sentence:
      'Auto is on: it runs without asking and may change files anywhere on this machine, not only in this folder.'
  }
]

export function modeFacts(mode: MissionMode): ModeFacts {
  // The union is closed and the table covers it, which `mode-says-one-thing`
  // asserts. The fallback exists so a mode added to the union without a row
  // renders SOMETHING rather than crashing the surface that drew it.
  return MODE_FACTS.find((facts) => facts.mode === mode) ?? MODE_FACTS[0]!
}

/**
 * A mission mode in the words a receipt uses for it.
 *
 * The roster card printed the literal string `read-only` for every teammate,
 * whatever mode they had actually run in -- a fact the card never had. Once a
 * runtime could edit, that line was simply false. MEASURED 2026-09-03 while
 * reading the Teammates screen.
 *
 * Lowercase, because it sits mid-sentence in a receipt; the scope clause is
 * appended only where it adds something the name does not already carry.
 */
export function modeLabel(mode: MissionMode | undefined): string {
  if (mode === undefined) return 'not set yet'
  const facts = modeFacts(mode)
  const name = facts.name.toLowerCase()
  return facts.scope === undefined ? name : `${name} · ${facts.scope}`
}

/** The summary card's line: the name, then what it does. */
export function modeSummary(mode: MissionMode): string {
  const facts = modeFacts(mode)
  return `${facts.name} · ${facts.short}`
}

/** Said to the person on the surface where they are about to choose. */
export function modeSentence(mode: MissionMode): string {
  return modeFacts(mode).sentence
}

/**
 * Auto is absent unless the workspace switched it on, and absent rather than
 * disabled: a greyed-out row invites the question "how do I get that?" on the
 * one control where the answer should be a deliberate trip to Settings.
 * Omitting the option is also what makes this fail closed -- a caller that
 * does not pass the switch cannot offer the mode by forgetting to.
 */
/**
 * What a run was allowed, in the words the header uses. Read from the run's
 * own start receipt, never from the mode the composer happens to show now --
 * and Auto needs its own phrase, because a run that could touch the whole
 * machine rendered as "may edit the workspace" would understate what happened.
 */
export function sandboxPhrase(sandbox: 'read-only' | 'workspace-write' | 'full-access' | undefined): string {
  if (sandbox === 'full-access') return 'may edit anything on this machine'
  if (sandbox === 'workspace-write') return 'may edit the workspace'
  return 'read-only'
}

/**
 * Which sign-in a run went through, in words.
 *
 * The record's route id is `<runtime>-account:default` for every runtime but
 * Antigravity (`antigravity:hub`): it names the account the runtime was
 * signed into, never a plan or a price. OpenCode's free models need no
 * account at all, and saying "your account" over one would be the opposite
 * of the promise the welcome screen makes.
 */
export function accountPhrase(route: { readonly runtime: MissionRuntimeId; readonly model: string; readonly resolvedRouteId: string }): string {
  if (route.resolvedRouteId === 'antigravity:hub') return 'through the Antigravity app'
  if (/-free$/i.test(route.model)) return 'none — a free model'
  return `your ${runtimeDisplayName(route.runtime)} sign-in`
}

/**
 * Who the composer should address once a conversation is opened.
 *
 * Opening a mission used to change the thread and leave the selection alone,
 * so clicking a teammate's message in an exchange showed THEIR run under a
 * header with their name while the composer still said "Message Wren..." and
 * Wren's card stayed lit -- three surfaces, two answers, on the one control
 * whose whole job is saying where the next message goes (0.35.0 targeted QA).
 *
 * A mission with no recorded owner keeps the current selection rather than
 * clearing it: an unowned raw conversation is a real state, and blanking the
 * composer would be a second wrong answer instead of the first.
 */
export function ownerToSelect(
  missionId: string,
  owners: Readonly<Record<string, string>>,
  current: string | undefined,
  /**
   * The owner the run itself carries, which is what the HEADER prefers. The
   * two read from different places and agreed everywhere anyone could trace,
   * because every path that sets one writes the other -- except the window
   * where a run has started and the host has not recorded it yet (QA,
   * 2026-09-06, source-reviewed). Taking the same first answer here closes
   * the window instead of relying on the two staying in step.
   */
  fromRun?: string
): string | undefined {
  /*
   * A CONVERSATION NOBODY OWNS IS ADDRESSED TO NOBODY (0.551). This kept the
   * pick from before, so Colin opened a conversation with no teammate, typed,
   * and the message went to Boss -- queued "Sends when Boss finishes" behind
   * Boss's other conversation, on Boss's route. The box writes in the
   * conversation on screen.
   */
  void current
  return fromRun ?? owners[missionId]
}

/**
 * Every mode a route can run, Auto included.
 *
 * Auto was briefly hidden until the workspace switch was on, so choosing it
 * meant a trip to Settings first. Colin, 2026-09-06: "always allow auto to be
 * chosen from the permission dropdown, we want the user experience to be
 * fluid." So it is always offered, and picking it turns the switch on -- the
 * menu item IS the switch, with its consequence stated beside it in amber.
 * Settings keeps the same switch for seeing the state and taking it back, and
 * the host still asks the switch as each run starts, so nothing here is what
 * finally decides whether a run gets it.
 */
export function modesFor(runtime: MissionRuntimeId, platform?: string): readonly MissionMode[] {
  return (['ask', 'plan', 'accept-edits', 'approve-each', 'auto'] as const).filter((mode) =>
    modeRunsOn(mode, runtime, platform)
  )
}

/** Why a mode is unavailable here, for the menu to say out loud. */
export function modeUnavailableReason(
  mode: MissionMode,
  runtime: MissionRuntimeId,
  platform?: string
): string | undefined {
  if (modeRunsOn(mode, runtime, platform)) return undefined
  if (mode === 'approve-each') return `Claude Code, Codex CLI, OpenCode and Copilot CLI only. ${runtimeLabel(runtime)} cannot stop and ask yet.`
  if (mode === 'auto') return `${runtimeLabel(runtime)} runs its own agent under its own permissions; Locust has no handle to widen.`
  if (runtime === 'antigravity') return 'Antigravity CLI runs this mode; without it, Antigravity runs only in Edit.'
  return 'Cursor Agent cannot be kept read-only on Windows: that needs macOS or Linux, and its plan mode alone does not stop it editing files.'
}

/**
 * A runtime's name, from the one table that has to list them all.
 *
 * This was an `if` chain ending in `return 'Codex CLI'`, which is the same
 * fall-through that let `modeRunsOn` grant Muse four untested modes: a
 * runtime nobody added here was not missing, it was renamed. MEASURED on
 * 2026-09-21 by driving the packaged build -- with Muse Code chosen, the
 * mode menu said **"Codex CLI only. Codex CLI cannot stop and ask yet"** and
 * **"Codex CLI runs its own agent under its own permissions"**, about a
 * runtime the person had just picked and a runtime that was not in play.
 *
 * `runtimeDisplayName` is an exhaustive record, so the compiler asks for the
 * next runtime instead of quietly calling it Codex.
 */
function runtimeLabel(runtime: MissionRuntimeId): string {
  return runtimeDisplayName(runtime)
}

/**
 * The model families worth surfacing first when nothing else distinguishes
 * them.
 *
 * This is a CURATED JUDGEMENT, not a measurement: nothing in this app counts
 * how often a model is used anywhere but on this machine. It exists because a
 * runtime that lists seventy models alphabetically buries the ones most
 * people came for. It only ever affects ORDER -- no row is hidden, nothing is
 * labelled "best", and a model absent from this list is offered exactly as
 * readily as one on it.
 */
const FLAGSHIP_MODELS: readonly RegExp[] = [
  /^auto$/i,
  /grok-4\.6/i,
  /composer-2/i,
  /opus-5/i,
  /fable/i,
  /sonnet-5/i,
  /gpt-5\.3-codex/i,
  /gemini-3/i,
  // A newer family sits above the one it replaces. Added 2026-09-05, the day
  // GPT-6 Astra appeared: the app READS its models from each runtime, so a
  // new one is offered the moment the runtime reports it and needs no code
  // at all -- this list is the single exception, and only decides where a
  // row sits among seventy. Left out, Astra sorted below GPT-5.6.
  /gpt-6/i,
  /gpt-5\.6/i
]

/** Where a model sits in the curated list, or nowhere. */
export function flagshipRank(modelId: string): number | undefined {
  const index = FLAGSHIP_MODELS.findIndex((pattern) => pattern.test(modelId))
  return index === -1 ? undefined : index
}

/**
 * Order the picker's rows within each runtime: what this person has actually
 * run, newest first; then the flagship families; then everything else, in the
 * order the runtime listed them.
 *
 * The first rule comes from their own ledger and is a claim the app can back.
 * The second is a judgement, and is second for that reason.
 */
/** How many routes the Recent group offers before it is a list of its own. */
export const RECENT_ROUTE_LIMIT = 4

/**
 * The routes this person actually moves between, lifted to the top.
 *
 * Recency already sorts rows WITHIN a runtime, which helps when you stay on
 * one. It does nothing for the move this product exists for -- putting two
 * models on the same work -- because the other runtime's group sits below
 * six rows of the one you are on, and the list shows less than half its
 * height at a time. MEASURED 2026-09-03: 27 rows across 7 groups, scrolling
 * at 330px of 751.
 *
 * Only routes that are still offered appear, so a model a runtime has stopped
 * advertising cannot be resurrected here. Three cases withhold it entirely,
 * all of them the same rule: a shortcut has to shorten something.
 *
 * - Fewer than two recents. One recent route is the one you are already on.
 * - A list short enough to see at once. The justification for the group was
 *   27 rows scrolling at 330px; below that there is nothing to skip past.
 * - Recents that are half the list or more. With two routes total, a Recent
 *   group duplicates the entire picker and explains nothing.
 *
 * The last two came from the design pass pushing back on the feature rather
 * than styling around it, which was the right call.
 */
export function recentRouteRows<TRow extends { readonly key: string; readonly group: string }>(
  rows: readonly TRow[],
  recent: readonly string[],
  limit: number = RECENT_ROUTE_LIMIT
): readonly TRow[] {
  const byKey = new Map(rows.map((row) => [row.key, row]))
  const picked: TRow[] = []
  for (const key of recent) {
    const row = byKey.get(key)
    if (row === undefined) continue
    picked.push({ ...row, group: 'Recent', key: `recent:${key}` })
    if (picked.length >= limit) break
  }
  if (picked.length < 2) return []
  // Short enough to take in at once, or so much of the list that the shortcut
  // is a second copy of it.
  if (rows.length <= ROUTE_GROUP_LIMIT + 2) return []
  if (picked.length * 2 >= rows.length) return []
  return picked
}

/** The picker's group for the models a person added themselves. */
export const OWN_MODELS_GROUP = 'Your models'

export function orderRouteRows<
  TRow extends {
    readonly key: string
    readonly group: string
    readonly model?: string
    readonly tag?: string
  }
>(
  rows: readonly TRow[],
  recent: readonly string[]
): readonly TRow[] {
  const rank = new Map(recent.map((key, index) => [key, index]))
  const groups: string[] = []
  for (const row of rows) if (!groups.includes(row.group)) groups.push(row.group)
  /*
   * GROUPS ARE BANDED BY WHETHER THEY CAN RUN, then by discovery order.
   *
   * They used to keep discovery's order outright -- "only rows move" -- and
   * discovery's order has nothing to do with what is installed. Sol's beta
   * review, 2026-09-21, finding 2, opened the picker on a machine where
   * exactly one runtime worked:
   *
   *   CODEX CLI · YOUR ACCOUNT   NOT INSTALLED
   *   CLAUDE CODE                NOT INSTALLED
   *   CURSOR AGENT               NOT INSTALLED
   *   ... OpenCode, the only thing that works, below the fold
   *   GEMINI CLI                 PLANNED -- "Not built yet."
   *
   * Three headings a person cannot use, and a thing that does not exist, in
   * the one control they open to send a message. `runtimeListOrder` fixed
   * exactly this for the Settings list on 2026-09-15 and its comment claimed
   * "the picker already groups connected-first". It did not; nothing did.
   *
   * A group takes the band of its BEST row, so one usable model keeps a
   * runtime up top, and `RUNTIME_BAND` is the same table Settings sorts by --
   * one definition of "first", not two that drift.
   */
  const bandOf = (group: string): number => {
    let best = 9
    for (const row of rows) {
      if (row.group !== group) continue
      // ACTIVE only ever lands on a selectable row (`routeRowTag`), so it is
      // a READY row wearing a different word.
      const band = row.tag === 'ACTIVE' ? 0 : RUNTIME_BAND[row.tag ?? ''] ?? 3
      if (band < best) best = band
    }
    return best
  }
  const groupBand = new Map(groups.map((group) => [group, bandOf(group)]))
  const score = (row: TRow): number => {
    // The route you are ON sorts first in its group. It used to sort by the
    // same rules as everything else, so "account-default" -- not recently
    // used and not a flagship name -- fell into the bottom band, below six
    // models and behind a "1 more model · type to search them" line. The row
    // the composer is pointing at is the last thing that should need finding.
    if (row.tag === 'ACTIVE') return -1
    // Kilo's Auto Free stays above flagship names, which would otherwise fill
    // the six visible rows with paid gateway models.
    if (row.model === 'kilo/kilo-auto/free') return 500_000
    const used = rank.get(row.key)
    if (used !== undefined) return used
    const flagship = flagshipRank(row.model ?? row.key)
    // Everything used sorts above everything merely notable, which sorts
    // above the rest; the offsets keep those three bands apart whatever the
    // counts are.
    return flagship === undefined ? 2_000_000 : 1_000_000 + flagship
  }
  return [...rows].sort((left, right) => {
    /*
     * What can run, then ALPHABETICAL inside that band. See `bandOf`.
     *
     * This fell back to `groups.indexOf` -- discovery's arrival order -- and
     * Gemini's handoff pass caught it on a machine with all seven runtimes
     * present: Claude Code sorted below Codex CLI in the same band, purely
     * because Codex answered its probe first.
     * 
     * Worth taking for a reason beyond taste. `runtimeListOrder`, which
     * Settings uses, has sorted `left.displayName.localeCompare(right.
     * displayName)` inside each band since 2026-09-15. Two lists of the same
     * six runtimes, two different answers to 'what comes first' -- and I
     * wrote 'alphabetical inside each band' into the handoff brief myself,
     * then implemented the other thing. Fewer clocks, not fewer numbers.
     */
    const byBand = (groupBand.get(left.group) ?? 9) - (groupBand.get(right.group) ?? 9)
    if (byBand !== 0) return byBand
    // The person's own models lead their band (0.357): a model you added is
    // the one you came to use, and "Your models" would otherwise sort last.
    const own = Number(right.group === OWN_MODELS_GROUP) - Number(left.group === OWN_MODELS_GROUP)
    if (own !== 0) return own
    const byGroup = left.group.localeCompare(right.group)
    if (byGroup !== 0) return byGroup
    return score(left) - score(right)
  })
}

/**
 * Which missions a search shows.
 *
 * The sidebar's search field was drawn, labelled and completely inert: it
 * held no state and filtered nothing, which is the exact shape this codebase
 * refuses elsewhere -- a control that cannot do its job should say so rather
 * than appear and fail.
 *
 * Matching is on the words a person can see: a mission's title, and its short
 * id, so the id in a receipt can be pasted straight in.
 */
export function missionsMatching<TRow extends { readonly title: string; readonly missionId: string; readonly said?: string }>(
  rows: readonly TRow[],
  query: string,
  /**
   * Another name a row answers to -- the sidebar passes the teammate's, so
   * typing "Atlas" finds Atlas's conversations (0.414).
   */
  alsoCalled?: (row: TRow) => string | undefined
): readonly TRow[] {
  const needle = query.trim().toLowerCase()
  if (needle.length === 0) return rows
  return rows.filter((row) =>
    row.title.toLowerCase().includes(needle)
    || row.missionId.toLowerCase().includes(needle)
    || (row.said?.toLowerCase().includes(needle) ?? false)
    || (alsoCalled?.(row)?.toLowerCase().includes(needle) ?? false)
  )
}

/**
 * One row per CONVERSATION, not one per turn.
 *
 * A mission is one run: one process, one receipt, one durable record, and a
 * reply is a second run because it is a second process. That is right for the
 * ledger and wrong for the sidebar, which was listing every reply as its own
 * entry while the thread beside it showed them as the single exchange they
 * were. So the chain is collapsed here, for display only -- nothing about how
 * missions are recorded changes.
 *
 * The row a person clicks is the LEAF of the chain: the turn nothing else
 * continues from, which is where the conversation actually is. Its name comes
 * from the ROOT, because that is what they typed to start it. The phase is
 * live if any turn is live, so a conversation whose newest turn is running
 * reads as running.
 */
export interface ConversationRowExtras {
  /** Every mission in the chain, so selecting any turn lights this row. */
  readonly memberIds: readonly string[]
  /** How many turns it holds. 1 means an ordinary single-run mission. */
  readonly turns: number
  /**
   * What the person said in every turn, for search. The row is named by its
   * first turn, so a word from a follow-up -- "also check the signup form" --
   * found nothing: fresh-eyes area 8, 0.413 (0.414).
   */
  readonly said: string
}

/**
 * What a route row is searched by. Model ids arrive as `muse-spark-1.3` and
 * `gpt-5.6-luna`; a person types "muse spark" or "gpt 5". Punctuation is
 * folded to spaces on both sides so the search matches how names are said,
 * not how a CLI happened to spell them.
 */
/**
 * The effort level a route uses when nobody has chosen one.
 *
 * The control is meant to read as a constant -- Colin, 2026-09-07: "have it
 * be a constant" -- and a control showing nothing was how "we still have no
 * effort control" stayed true even after the levels existed.
 *
 * This is used in TWO places on purpose: the chip's display in Composer.tsx
 * and the value `swarmEffortFor` hands the run. It has to be both, and for
 * one release it was only the first -- the chip stated "medium" while the run
 * was started with no effort argument at all.
 *
 * `medium` when the model offers it, because every runtime here treats it as
 * the ordinary setting. Otherwise the middle of what was reported, which for
 * a two-level model is the higher one -- the same bias as picking `medium`
 * out of five. Undefined only when the model reports no levels at all, and
 * then there is nothing to show.
 *
 * All of that is Locust's guess, and it comes second to the runtime's own
 * answer where the catalogue has one (`PublicModel.defaultEffort`): Cursor
 * lists Kimi K3's default as its max variant and Opus 4.6's as high, and
 * "the middle" would have said high and max.
 */
export function defaultEffort(supportedEfforts: readonly string[], runtimeDefault?: string): string | undefined {
  if (supportedEfforts.length === 0) return undefined
  if (runtimeDefault !== undefined && supportedEfforts.includes(runtimeDefault)) return runtimeDefault
  if (supportedEfforts.includes('medium')) return 'medium'
  return supportedEfforts[Math.floor(supportedEfforts.length / 2)]
}

/**
 * The effort to use after a model switch.
 *
 * Carried across when the new model advertises it, so choosing a different
 * model does not silently undo a choice about how hard it should think.
 * Cleared to that model's default otherwise, never to nothing: sending a
 * level a model never advertised is the thing this guards against, and
 * showing an empty control is what it used to do instead.
 */
export function effortAfterRouteChange(
  current: string | undefined,
  supportedEfforts: readonly string[],
  runtimeDefault?: string
): string | undefined {
  if (current !== undefined && supportedEfforts.includes(current)) return current
  return defaultEffort(supportedEfforts, runtimeDefault)
}

/**
 * KEEP CARRIES THE WHOLE CHOICE (0.622). What the chat box becomes when one
 * column of a comparison is kept: the column's runtime and model, and the
 * effort it ran at -- by the rule a model change follows, so a model that
 * does not offer that effort lands on its own default, and one with no
 * levels has none. There is no mode in it, on purpose: a comparison's Auto
 * worked in copies of the folder, and the conversation goes on in the folder
 * itself, at the chat box's own mode.
 */
export function routeAfterKeep(
  kept: { readonly runtime: string; readonly model: string; readonly effort?: string },
  models: readonly PublicModel[]
): { readonly runtime: MissionRuntimeId; readonly model: string; readonly effort: string | undefined } {
  const family = modelFamily(models, kept.runtime, kept.model)
  return {
    runtime: kept.runtime as MissionRuntimeId,
    model: kept.model,
    effort: effortAfterRouteChange(kept.effort, family?.supportedEfforts ?? [], family?.defaultEffort)
  }
}

/**
 * The bar's sentence once a column is kept. It says only what is true: which
 * model the conversation carries on with, and the mode it carries on in --
 * the chat box's own, not the comparison's. When the comparison ran in Auto
 * (in copies) and the conversation does not, it says so, so nobody reads
 * "carries on with it" as "carries on in Auto".
 */
export function keptSentence(
  keptName: string | undefined,
  conversationMode: MissionMode | undefined,
  comparisonMode?: MissionMode
): string {
  const head = `You kept ${keptName ?? 'one'}; the conversation carries on with it`
  if (conversationMode === undefined) return `${head}.`
  const name = modeFacts(conversationMode).name
  const ranElsewhere =
    comparisonMode !== undefined && comparisonMode !== conversationMode
      ? `, not in ${modeFacts(comparisonMode).name} as the comparison did, in copies of your folder`
      : ''
  return `${head} in ${name}${ranElsewhere}.`
}

/**
 * A model id as it should read beside the runtime that serves it.
 *
 * OpenCode aggregates providers, so its ids carry one:
 * `anthropic/claude-sonnet-4`, `opencode/ling-3.0-flash-fin-free`. The
 * provider is real information and stays -- EXCEPT when it merely repeats
 * the runtime already named next to it, which is how the composer came to
 * read `OpenCode / opencode/ling-3.0-flash-fin-free` and truncate to
 * `OpenCode / opencode/big-pic...`: the runtime twice, and the part that
 * identifies the model cut off (outside tester, 2026-09-07).
 */
export function modelLabelFor(runtime: string, modelId: string): string {
  const slash = modelId.indexOf('/')
  if (slash <= 0) return modelId
  const provider = modelId.slice(0, slash)
  return provider.toLowerCase() === runtime.toLowerCase() ? modelId.slice(slash + 1) : modelId
}

export function routeSearchText(text: string): string {
  return text.toLowerCase().replace(/[-_/.:]+/g, ' ').replace(/\s+/g, ' ').trim()
}

/**
 * Whether a run belongs in the mission list at all.
 *
 * A send the host refused never became a mission: no id was ever assigned to
 * it and it has already settled. Listing one files a permanent row under an
 * internal key, which looks like a mission, answers "Copy mission id" with
 * that key, and can never be reopened -- Codex's QA pass on 0.14 caught it as
 * a prompt that "appeared submitted" but "was never recorded as a mission".
 * A run that is still starting has no id yet either, and that one DOES belong:
 * it is how a teammate reads as working before the first receipt.
 */
export function listedAsMission(run: {
  readonly missionId: string | undefined
  readonly active: boolean
}): boolean {
  return run.missionId !== undefined || run.active
}

export function collapseConversations<
  TRow extends {
    readonly missionId: string
    readonly title: string
    readonly phase: string
    readonly integrityIssueCount: number
    readonly rootId?: string
    readonly parentId?: string
    /** This turn's own words; `title` is already the conversation's name. */
    readonly words?: string
  }
>(rows: readonly TRow[]): readonly (TRow & ConversationRowExtras)[] {
  const order: string[] = []
  const groups = new Map<string, TRow[]>()
  for (const row of rows) {
    const key = row.rootId ?? row.missionId
    const held = groups.get(key)
    if (held === undefined) {
      order.push(key)
      groups.set(key, [row])
    } else {
      held.push(row)
    }
  }
  return order.map((key) => {
    const members = groups.get(key) ?? []
    const continued = new Set(members.map((row) => row.parentId).filter((id): id is string => id !== undefined))
    // The leaf is the turn nothing continues from. A hand-edited ledger could
    // leave none, so the last row read is the fallback rather than a crash.
    const leaf = members.find((row) => !continued.has(row.missionId)) ?? members[members.length - 1]!
    const root = members.find((row) => row.missionId === key) ?? leaf
    const running = members.find((row) => row.phase === 'running')
    return {
      ...leaf,
      title: root.title,
      phase: running === undefined ? leaf.phase : running.phase,
      integrityIssueCount: Math.max(...members.map((row) => row.integrityIssueCount)),
      memberIds: members.map((row) => row.missionId),
      turns: members.length,
      said: members.map((row) => row.words ?? row.title).join(' · ')
    }
  })
}

/**
 * The model and effort a mission is actually started with.
 *
 * Some runtimes take an effort as a flag. Cursor encodes it in the model id
 * instead and lists every combination as its own model; the catalog folds
 * those into one row with `variants`, so the chosen effort has to be turned
 * back into the id it names -- and then NOT sent beside it. Sending both put
 * "cursor-grok-4.6-high-fast" and "medium" in front of a builder that refuses
 * any effort for Cursor, and every Cursor run with an effort picked failed
 * with "cannot be started with the options chosen" (Colin, 2026-09-06).
 */
/**
 * Whether this route's effort is already inside its model id.
 *
 * Cursor lists every effort as its own model (`cursor-grok-4.6-high-fast`,
 * `composer-2.5-fast`) and its command builder refuses a separate effort
 * outright. `startRoute` handles that for a mission the person starts. A
 * routine stores its route instead, and storing an effort beside an id that
 * already carries one put both in front of that builder: every routine taught
 * on Cursor threw on replay and never opened a mission (measured 2026-09-07,
 * `routine-smoke`, stored route
 * `{model: "composer-2.5-fast", effort: "fast"}`).
 *
 * Asked of the id rather than of the runtime, because the runtime is not the
 * thing that decides it -- a family that folds variants does, and the next
 * runtime to fold them should not need this file edited.
 */
/**
 * Whether the runtime we tell a new person needs no account STILL has
 * something free to run.
 *
 * `FirstLaunch` promises, unconditionally: *"OpenCode needs no account — one
 * install and you have a working teammate."* Both halves are true today, but
 * they are different KINDS of claim. "Needs no account" is about auth and is
 * a property of the runtime's design. "You have a working teammate" depends
 * on OpenCode continuing to publish a free model -- a decision somebody else
 * makes, about which Colin asked directly (2026-09-21): *"i know its
 * available on opencode but idk how long that will continue for."*
 *
 * Nothing checked it. The app would have gone on promising a free teammate
 * to someone who installs OpenCode and finds nothing they can run -- the
 * worst moment to be wrong, because it is the first one.
 *
 * THREE ANSWERS, and the third is the point:
 *
 * - `yes`   - OpenCode is ready and lists at least one `-free` model.
 * - `no`    - OpenCode is ready, its list was read, and none of it is free.
 * - `unknown` - it is not installed yet, or its list has not been read.
 *
 * **`unknown` must never weaken the promise.** At first launch OpenCode is
 * usually not installed, so `unknown` IS the ordinary path; treating absence
 * of evidence as disproof would quietly retire the onboarding sentence for
 * everybody. The copy changes only on `no`, which is positive evidence.
 */
export function freeStartStillFree(
  runtimes: readonly PublicRuntimeStatus[],
  models: readonly PublicModel[]
): 'yes' | 'no' | 'unknown' {
  const runtime = runtimes.find((entry) => entry.id === FREE_START_RUNTIME)
  if (runtime?.status !== 'ready') return 'unknown'
  const listed = models.filter((model) => model.runtime === FREE_START_RUNTIME)
  // A ready runtime whose list we never read tells us nothing about its
  // prices. That is the same distinction the rest of this file keeps: no
  // answer is not a bad answer.
  if (listed.length === 0) return 'unknown'
  return listed.some((model) => model.id.endsWith('-free')) ? 'yes' : 'no'
}

export function effortIsInModelId(
  models: readonly PublicModel[],
  runtime: MissionRuntimeId,
  modelId: string
): boolean {
  return models.some(
    (model) =>
      model.runtime === runtime &&
      model.variants !== undefined &&
      (model.id === modelId || Object.values(model.variants).includes(modelId))
  )
}

/**
 * The catalogue row a model id belongs to -- by its own id, or by being one of
 * that family's effort variants.
 *
 * Two places needed this and each had written `find(m => m.id === modelId)`,
 * which is wrong for exactly the same reason in both: a Cursor family never
 * listed without an effort takes the FIRST VARIANT SEEN as its id, so the id
 * in hand is routinely a sibling of the family's own. In `startRoute` that
 * sent a loose effort to a runtime that refuses one (Colin, 2026-09-07). In
 * the composer it meant no family matched, `supportedEfforts` came back empty,
 * and the effort chip silently disappeared -- which is why the control count
 * moved between three and four across otherwise identical drives.
 */
export function modelFamily(
  models: readonly PublicModel[],
  runtime: string,
  modelId: string
): PublicModel | undefined {
  return models.find(
    (entry) =>
      entry.runtime === runtime &&
      (entry.id === modelId || Object.values(entry.variants ?? {}).includes(modelId))
  )
}

export function startRoute(
  models: readonly PublicModel[],
  runtime: MissionRuntimeId,
  modelId: string,
  effort: string | undefined
): { readonly model: string; readonly effort?: string } {
  if (effort === undefined) return { model: modelId }
  // Found by id OR by being one of a family's variants.
  //
  // Matching on `id` alone was the whole of this bug (Colin, 2026-09-07:
  // "before that model and effort was working perfectly fine"). A Cursor
  // family that is never listed without an effort takes the FIRST VARIANT SEEN
  // as its id -- `cursorModelsFrom` says so in as many words -- so the id in
  // hand is routinely `cursor-grok-4.6-low` while the person has chosen
  // `medium`. No family matched, the effort fell through as a separate value,
  // and `createCursorPrintCommand` refuses any effort: "Cursor Agent takes no
  // effort level. Nothing was recorded."
  //
  // Looking the family up through its variants as well makes the two paths
  // agree, and re-resolves the id to the variant the effort actually names.
  const model = modelFamily(models, runtime, modelId)
  // A model that advertises NO efforts must never be sent one.
  //
  // Colin, 2026-09-08: choosing OpenCode while an effort was set failed the run
  // outright -- "That runtime cannot be started with the options chosen.
  // OpenCode takes no effort level. Nothing was recorded." The effort was a
  // leftover from the previous route, and every place that changes a route has
  // to remember to clear it: the picker does, adopting a teammate's own route
  // did not. This is the one path EVERY start takes, so it is the honest place
  // to enforce it rather than a fourth caller remembering.
  //
  // Only when the family is actually KNOWN to have none. A model missing from
  // the catalogue (an account default, or a probe that has not answered yet)
  // says nothing about efforts, and dropping one there would quietly downgrade
  // a Claude run that asked for `high`.
  if (model !== undefined && model.supportedEfforts.length === 0) return { model: modelId }
  // OpenCode takes ANY variant name and silently ignores one the model does
  // not list (measured 2026-09-25), so a leftover effort from another model
  // would be shown as applied and never be. Its lists are per model and
  // complete, so an effort outside one is dropped here (A6.5).
  if (runtime === 'opencode' && model !== undefined && !model.supportedEfforts.includes(effort)) {
    return { model: modelId }
  }
  const variant = model?.variants?.[effort]
  if (variant !== undefined) return { model: variant }
  // A model with variants and no variant for this effort: the effort names
  // nothing this runtime can honour, so it is dropped rather than refused.
  if (model?.variants !== undefined) return { model: modelId }
  return { model: modelId, effort }
}

/**
 * The order runtimes read in on a settings list.
 *
 * `READY` → `EXPERIMENTAL` → `SIGN IN` → `NOT INSTALLED` → `PLANNED`, and
 * alphabetical inside each band so the order is stable between launches.
 *
 * Frame pass, 2026-09-15: the list ran Codex READY, Claude READY, Cursor
 * READY, **Gemini PLANNED**, OpenCode READY, Copilot READY, **OmniRoute
 * PLANNED**, Antigravity EXPERIMENTAL. Two things that do not exist sat in
 * the middle of six that do, so a person scanning for what they can use had
 * to read every tag. The picker already groups connected-first; Settings was
 * the one list not doing it.
 */
const RUNTIME_BAND: Readonly<Record<string, number>> = {
  READY: 0,
  EXPERIMENTAL: 1,
  'SIGN IN': 2,
  'AT LIMIT': 2,
  'NOT INSTALLED': 3,
  PLANNED: 4
}

export function runtimeListOrder(
  runtimes: readonly PublicRuntimeStatus[],
  tagOf: (runtime: PublicRuntimeStatus) => string
): readonly PublicRuntimeStatus[] {
  return [...runtimes].sort((left, right) => {
    const band = (RUNTIME_BAND[tagOf(left)] ?? 3) - (RUNTIME_BAND[tagOf(right)] ?? 3)
    return band !== 0 ? band : left.displayName.localeCompare(right.displayName)
  })
}

/**
 * What we knew, kept until something better is known.
 *
 * Discovery re-runs: fifteen seconds in, again while anything is still
 * checking, and every time the window is focused. Each answer replaced the
 * last wholesale — so a runtime that had reported `ready · 2.1.270` went
 * back to `CHECKING` with no version the moment a re-sweep began, and the
 * welcome grid flickered between three different kinds of thing in one
 * column. Colin, 2026-09-14: *"randomly checking for cursor and claude...
 * some showing ready, some showing version code, and the box just popped up
 * again randomly."*
 *
 * It got worse when probes were staggered, because a sweep now takes long
 * enough to watch — but the bug was always there, and it is the same one
 * this project keeps meeting: **absence of a fresh answer is not evidence
 * that the old one stopped being true.**
 *
 * So a probe that has not come back yet — `probe-failed` and `offline` are
 * the two states that mean "no answer", not "a bad answer" — keeps whatever
 * the last completed sweep established. Every DEFINITE outcome wins
 * immediately, including the bad ones: signed out, not installed and ready
 * all replace what came before, because each of those is an answer.
 */
const NO_ANSWER_YET: ReadonlySet<string> = new Set(['probe-failed', 'offline'])

/**
 * THE AGENTS AN ANSWER STILL CALLS "BEING CHECKED" WHOSE CHECK HAS FINISHED (0.639).
 *
 * Since 0.634 the first screen waits for no agent past two seconds: a slow one
 * is shown as being checked, and when its check lands the host stores the
 * answer and the window, told the check finished, asks again. Two orders
 * lost that: the finish arrived before the window held any list (so it named
 * nobody being checked), or the window's ask reached the host a tick before
 * the answer was stored (so it came back "being checked" and nothing else
 * would ever say otherwise). Measured on the packaged 0.638: OpenCode
 * answered two seconds in, and its nine free models in the picker said
 * CHECKING forty seconds later, reopened or not, while the host's own list
 * said ready. The window keeps the ids whose checks have finished; an answer
 * that still calls one of them being checked is read again.
 */
export function staleChecking(runtimes: readonly PublicRuntimeStatus[], finished: ReadonlySet<string>): readonly string[] {
  return runtimes.filter((entry) => entry.checking === true && entry.status !== 'ready' && finished.has(entry.id)).map((entry) => entry.id)
}

export function keepWhatWasKnown(
  previous: readonly PublicRuntimeStatus[],
  next: readonly PublicRuntimeStatus[]
): readonly PublicRuntimeStatus[] {
  const before = new Map(previous.map((runtime) => [runtime.id, runtime]))
  return next.map((runtime) => {
    const held = before.get(runtime.id)
    if (held === undefined) return runtime
    // Still out. Keep the last thing this runtime actually said.
    if (NO_ANSWER_YET.has(runtime.status) && !NO_ANSWER_YET.has(held.status)) return held
    // Answered, but without a version it had told us before. A version does
    // not stop being true because one probe did not print it.
    if (runtime.version === null && held.version !== null && runtime.installed && held.installed) {
      return { ...runtime, version: held.version }
    }
    return runtime
  })
}

/**
 * The model a never-used profile should sit on once the free runtime is ready.
 *
 * `defaultRoute` deliberately answers only half of this: it picks the RUNTIME
 * and leaves the model on `account-default`, because which model to prefer is
 * the catalogue's business and discovery has not read the catalogue yet. This
 * is the other half, asked once the catalogue exists.
 *
 * Sol's beta review, 2026-09-21, finding 1 -- and the only one of the nine
 * that can cost money:
 *
 *   "After Install, the composer is Account Default... The one runtime that
 *   needs no account should come up on a named free model, so the first Send
 *   cannot spend."
 *
 * `account-default` means "whatever this account gives you". On a signed-in
 * account that is a billable model, chosen by somebody else, sitting behind
 * the first Enter a new person presses. The app recommends OpenCode by name
 * on the welcome screen precisely because it needs no account; landing them
 * on an unnamed route afterwards gives that back.
 *
 * THE BEST FIRST OF THE LISTED FREE MODELS (0.712; it was the first listed).
 * Catalogue order looked like OpenCode's own answer to "what first", but it
 * is only the alphabet, and its first three free models were, in turn, one
 * OpenCode had retired (Exo, 0.710), one its server no longer had (Fledge
 * Alpha, "Model not found", the first message on a bare Mac, 0.712), and one
 * down since 2026-10-05 (Ling 3.0, which OpenCode retries for two and a half
 * minutes before it gives up). So the listed free models are taken in
 * FREE_MODELS_BEST_FIRST's order, which names the ones the drive sweeps have
 * found answering. It only orders what OpenCode lists -- it never adds a
 * model, so a name gone stale costs nothing -- and a model not named keeps its
 * catalogue place after them. Retired and down models are left out as before.
 *
 * BUT ONE THAT ANSWERED HERE FIRST (0.517). The first listed (Ling) said
 * "Endpoint is unavailable" all of 2026-09-30 while others answered, and
 * every "Use a free model" landed on it. `answered` is the free models that
 * finished a run on this machine, newest first (`freeModelsThatAnswered`);
 * the newest still listed wins, and the catalogue's first is only for a
 * person who has not run one yet.
 *
 * `undefined` means LEAVE IT ALONE: no free model listed, the catalogue is
 * unread, or this is not the free-start runtime. Same rule as everywhere else
 * in this file -- no answer is not a bad answer.
 */
export function freeStartModel(
  runtime: MissionRuntimeId,
  models: readonly PublicModel[],
  answered: readonly string[] = [],
  /** Down right now (missionView.downFreeModels, 0.711): passed over while another is listed. */
  down: ReadonlySet<string> = NONE_DOWN
): string | undefined {
  if (runtime !== FREE_START_RUNTIME) return undefined
  const listed = freeModelsBestFirst(models)
  const up = listed.filter((model) => !down.has(model.id))
  return answered.find((id) => up.some((model) => model.id === id)) ?? up[0]?.id ?? listed[0]?.id
}

const NONE_DOWN: ReadonlySet<string> = new Set()

// OpenCode's free models, steadiest first (0.712): kept in shared/free-models.ts since 0.756, where Scout reads it too.
export { FREE_MODELS_BEST_FIRST }

/**
 * A FREE MODEL OPENCODE NO LONGER LISTS (0.819). OpenCode drops free models on its own clock -- Ling, then Space Bunny
 * and LongCat by 2026-10-22 -- and a teammate saved on one failed at its next send, "Model not found", before Locust
 * offered another. Once the catalogue has been read, a free model of the free-start runtime that it no longer lists,
 * or that failed here as retired (`models` is the window's list less those), is known before the send.
 *
 * False before the catalogue is read, and for every other route: a paid model, or one of the person's own, is theirs.
 */
export function droppedFreeModel(route: { readonly runtime: string; readonly model?: string }, models: readonly PublicModel[]): boolean {
  if (route.runtime !== FREE_START_RUNTIME || route.model === undefined || !/^opencode\/.+-free$/.test(route.model)) return false
  const listed = freeModelsBestFirst(models)
  return listed.length > 0 && !listed.some((model) => model.id === route.model)
}

/** The listed free models of the free-start runtime, FREE_MODELS_BEST_FIRST's first, then the rest as listed. */
export function freeModelsBestFirst(models: readonly PublicModel[]): readonly PublicModel[] {
  const rank = (id: string): number => {
    const at = FREE_MODELS_BEST_FIRST.indexOf(id)
    return at < 0 ? FREE_MODELS_BEST_FIRST.length : at
  }
  return models
    .map((model, index) => ({ model, index }))
    .filter(({ model }) => model.runtime === FREE_START_RUNTIME && model.own !== true && model.id.endsWith('-free'))
    .sort((a, b) => rank(a.model.id) - rank(b.model.id) || a.index - b.index)
    .map(({ model }) => model)
}

/**
 * The free model to offer when this one's provider is busy (C9): the next of
 * OpenCode's free models, best first (freeModelsBestFirst, 0.712; it was the
 * catalogue's order), after the one the run is on -- round to the first after
 * the last. On 2026-09-26 two free models were
 * limited for hours while three others answered, and a new person starts on
 * the first of them. Undefined when the run is not on one of OpenCode's free
 * models, or there is no other: a paid route, or a model of the person's own,
 * is theirs to leave. The next may be busy too; its notice then offers the
 * one after it.
 */
export function nextFreeModel(
  runtime: MissionRuntimeId,
  model: string | undefined,
  models: readonly PublicModel[],
  /** Down right now (0.711): skipped, unless every other is down too. */
  down: ReadonlySet<string> = NONE_DOWN
): PublicModel | undefined {
  const free = freeModelsBestFirst(models)
  const at = free.findIndex((entry) => entry.id === model)
  if (runtime !== FREE_START_RUNTIME || at < 0 || free.length < 2) return undefined
  for (let step = 1; step < free.length; step += 1) {
    const next = free[(at + step) % free.length]!
    if (!down.has(next.id)) return next
  }
  return free[(at + 1) % free.length]
}

/**
 * Why Send is disabled, in the person's own terms — or `undefined` when it is
 * not.
 *
 * Sol's beta review, 2026-09-21, finding 6: typed `hello` with no runtime
 * installed, Send stayed grey, and the only thing the control said about
 * itself was *"Start mission — Shift+Enter for a new line."* A disabled
 * button describing the thing it will not do is the shape of the first-run
 * wall this app has hit before (QA, 2026-09-06: install what you were told
 * to, come back, type, press Enter, nothing, and no part of the screen says
 * why).
 *
 * ORDER IS THE WHOLE DESIGN. Every clause below can be true at once on a
 * fresh profile — nothing installed AND nothing typed — and only the FIRST
 * one is worth saying, because it is the one the person cannot fix by doing
 * the obvious thing. "Write a message first" to someone who has written a
 * message and has no runtime is the original defect with more words.
 *
 * `empty` is last and deliberately quiet: a person staring at an empty box
 * knows the box is empty, so this is the only clause that is not news.
 */
export function sendBlockedReason(blocked: {
  readonly nothingInstalled: boolean
  readonly runtimeReady: boolean
  readonly routeCanRun: boolean
  readonly busy: boolean
  readonly empty: boolean
}): string | undefined {
  if (blocked.nothingInstalled) return 'Nothing can run yet — install an AI agent from the list above.'
  if (!blocked.runtimeReady) return 'The model this is set to is not ready on this machine. Pick another above, or set it up in Settings.'
  if (!blocked.routeCanRun) return 'This mode cannot run on the model this is set to. Pick another mode or model above.'
  if (blocked.busy) return 'Something else is starting. This goes as soon as it settles.'
  if (blocked.empty) return 'Write a message first.'
  return undefined
}

/**
 * What the collapsed row of "other" coding agents honestly says about them.
 *
 * It used to say one thing about all of them: *"{n} others Locust can drive —
 * they each need their own account."* Sol's beta review, 2026-09-21, finding
 * 7, expanded it and counted: Codex, Claude and Copilot install from here and
 * then want an account; Cursor Agent and Antigravity are a **Get it** link to
 * somebody else's installer. One sentence, two different jobs, and a person
 * deciding whether to press it was told the wrong one about two of five.
 *
 * This is the second time that sentence has been wrong about the rows behind
 * it. Fable found the first on 0.198.0 — four installed, hung CLIs sitting
 * behind *"they each need their own account"*, when they needed no account
 * and were already on the machine. Both failures are the same failure: a
 * fixed sentence about a list that varies. So it is derived now.
 *
 * Said as INSTALL METHOD, which is what the next click actually is. The
 * account only survives in the case where it is true of every row.
 */
export function deferredOthersSentence(
  others: readonly { readonly installsFromHere: boolean }[]
): string {
  const count = others.length
  const head = `${String(count)} other${count === 1 ? '' : 's'} Locust can drive`
  const fromHere = others.filter((other) => other.installsFromHere).length
  if (fromHere === count) return `${head} — each installs from here, then signs in.`
  if (fromHere === 0) return `${head} — each installs from its own vendor.`
  return `${head} — some install from here, some from their own vendor.`
}
