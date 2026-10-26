import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { MissionMode, TeammateRoute } from './ipc.js'
import { KILO_AUTO_FREE_MODEL } from './kilo-models.js'
import { isMissionRuntime } from './runtimes.js'

/** Kilo's saved "account default" is the session's paid image model. The chip and the start use Auto Free. */
function kiloOnAutoFree<T extends { readonly runtime: string; readonly model?: string }>(route: T): T {
  if (route.runtime !== 'kilo') return route
  if (route.model !== undefined && route.model !== 'account-default' && route.model.trim().length > 0) return route
  return { ...route, model: KILO_AUTO_FREE_MODEL }
}

export interface StartRouteInput {
  readonly runtime?: unknown
  readonly model?: unknown
  readonly effort?: unknown
  readonly account?: unknown
  readonly modelChoice?: unknown
  readonly routeOverrideFor?: unknown
}

export interface StartRoute {
  readonly runtime: MissionRuntimeId
  readonly model?: string
  readonly effort?: string
  readonly account?: string
}

/**
 * Discovery is not a person's model choice. The early-send race measured on
 * 2026-09-10 ran a Codex teammate on OpenCode. Resolve the saved identity at
 * the host, except for a picker choice explicitly addressed to THIS teammate.
 * An unassigned mission has no saved authority and keeps the old defaults.
 * Mode is deliberately absent: a saved Accept-edits route must not widen an
 * explicit Ask request. Permission validation remains where it already is.
 */
export function routeAtStart(
  input: StartRouteInput,
  teammate: { readonly teammateId: string; readonly route?: TeammateRoute } | undefined
): StartRoute {
  const requestedModel = typeof input.model === 'string' ? input.model : undefined
  const requested: StartRoute = kiloOnAutoFree({
    runtime: isMissionRuntime(input.runtime) ? input.runtime : 'codex',
    ...(typeof input.account === 'string' ? { account: input.account } : {}),
    ...(requestedModel === undefined ? {} : { model: requestedModel }),
    ...(typeof input.effort === 'string' ? { effort: input.effort } : {})
  })
  const savedRoute = teammate?.route
  const saved = savedRoute === undefined ? undefined : {
    ...savedRoute,
    model: savedRoute.runtime === 'kilo' && (savedRoute.model === 'account-default' || savedRoute.model.trim().length === 0)
      ? KILO_AUTO_FREE_MODEL
      : savedRoute.model
  }
  if (saved === undefined || input.routeOverrideFor === teammate?.teammateId) return requested
  // Cursor encodes effort in the concrete model id. Matching the original
  // picker identity lets that transformation survive without allowing a
  // discovery fallback's model (or effort) to replace the saved identity.
  if (requested.runtime === saved.runtime && (input.modelChoice ?? input.model) === saved.model) {
    return { ...requested, model: requested.model ?? saved.model, ...(saved.account === undefined ? {} : { account: saved.account }) }
  }
  return {
    runtime: saved.runtime,
    ...(saved.account === undefined ? {} : { account: saved.account }),
    model: saved.model,
    ...(saved.effort === undefined ? {} : { effort: saved.effort })
  }
}

export interface PickerRoute { readonly account?: string; readonly runtime: MissionRuntimeId; readonly model: string }

/**
 * Keep the chip and pending header on the same choice while effects settle.
 *
 * MODELS STAY WHERE YOU PUT THEM (0.552). Colin, 10/02: "everytime i switch
 * to a model and go to a new chat it never saves to that model or that
 * teammate". Picks were held per TEAMMATE and the teammate's saved route beat
 * the conversation's own, so a reopened conversation showed the teammate's
 * model and a pick in one chat followed them into every other. Claude Code's
 * rule instead: a conversation keeps its model; a new one starts on the
 * default. So, in order:
 * - a pick made HERE (`key`: this conversation, or this new chat);
 * - the conversation's own route, what its last turn ran on;
 * - for a new chat with a teammate, the teammate's model;
 * - otherwise the bare route (a new chat with nobody).
 */
export function composerRouteFor(
  fallback: PickerRoute,
  teammate: { readonly teammateId: string; readonly route?: TeammateRoute } | undefined,
  explicit: ReadonlyMap<string, PickerRoute>,
  key?: string,
  conversation?: PickerRoute
): PickerRoute {
  const picked = key === undefined ? undefined : explicit.get(key)
  if (picked !== undefined) return kiloOnAutoFree(picked)
  if (conversation !== undefined) return kiloOnAutoFree(conversation)
  if (teammate === undefined) return kiloOnAutoFree(fallback)
  return kiloOnAutoFree(teammate.route ?? fallback)
}

/** Where the box's pick is held: the conversation on screen, or the new chat it starts (0.552). */
export function pickKeyFor(conversation: string | undefined, teammateId: string | undefined): string {
  if (conversation !== undefined) return conversation
  return teammateId === undefined ? 'new' : `new:${teammateId}`
}

/**
 * A run started FOR a teammate who is not the one on screen yet (H9).
 *
 * "Ask <reviewer> for a review" selected the reviewer and then started the
 * run from a closure made before the selection -- so it went out as the
 * AUTHOR, on the author's route and mode, in the author's conversation, and
 * the reviewer never ran it (code review H9, reproduced from the probe's own
 * capture). The tidy pass (A1.2) copied the pattern and went out as nobody's.
 *
 * So the teammate is carried whole, from their own record: their id, the
 * route the composer would show for them (a picker choice made for them
 * wins, as it does in the composer), their saved mode and effort. Nothing is
 * read from state that has not caught up with selecting them.
 */
export interface StartAs {
  readonly teammateId: string
  readonly route: PickerRoute
  readonly mode: MissionMode
  readonly effort: string | undefined
  /** This run's mode is for this run only: the teammate's saved route stays as it was (0.514). */
  readonly oneOff?: true
}

/**
 * A REVIEW READS; IT DOES NOT EDIT (0.514). Two long passes (0.509, 0.512)
 * found "Ask <reviewer> for a review" running in the reviewer's saved Edit
 * mode. The reviewer contract already says "edit nothing"; Ask makes the
 * runtime hold to it, as Claude Code's own review is read-only. For this run
 * only -- the reviewer's saved mode is theirs. Antigravity's app cannot be
 * held to Ask (it runs its own agent), so there it keeps the mode it has.
 * Through its CLI it can: agy in Ask refused `git status` (measured 10/02).
 */
export function asReviewer(as: StartAs, antigravityCli = false): StartAs {
  if ((as.route.runtime === 'antigravity' && !antigravityCli) || as.mode === 'ask' || as.mode === 'plan') return as
  return { ...as, mode: 'ask', oneOff: true }
}

export function startAs(
  teammate: { readonly teammateId: string; readonly route?: TeammateRoute },
  composer: { readonly route: PickerRoute; readonly mode: MissionMode; readonly effort: string | undefined },
  explicit: ReadonlyMap<string, PickerRoute>
): StartAs {
  // Just the picker's two fields: a saved route also carries its mode and
  // effort, which are said once, below, where they belong. A run started FOR
  // a teammate is a new conversation, so it starts on their model; a pick
  // for their new chat (`new:<id>`) still wins, as it does in the box.
  const route = composerRouteFor(composer.route, teammate, explicit, pickKeyFor(undefined, teammate.teammateId))
  return {
    teammateId: teammate.teammateId,
    route: { runtime: route.runtime, model: route.model, ...(route.account === undefined ? {} : { account: route.account }) },
    mode: teammate.route?.mode ?? composer.mode,
    effort: teammate.route === undefined ? composer.effort : teammate.route.effort
  }
}
