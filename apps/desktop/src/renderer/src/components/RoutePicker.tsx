import { accountUsageKey } from '../../../shared/agent-accounts.js'
import { useAgentAccounts } from '../agentAccounts.js'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent, ReactElement, ReactNode } from 'react'

import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { PublicModel, PublicRuntimeStatus } from '../../../shared/ipc.js'
import { OWN_MODELS_GROUP, ROUTE_GROUP_LIMIT, capRouteRows, integrationOf, modelFamily, orderRouteRows, recentRouteRows, routeRowStatus, routeRowTag, routeSearchText } from '../status.js'
import type { RouteTag } from '../status.js'
import { isOwnRoute, modelDisplayName, routeFamilyName, routeModelName } from '../routeName.js'
import { isTypingTarget, menuRowFromDigit } from '../menuDigit.js'
import { RuntimeMark } from './RuntimeMark.js'
import { levelsLine } from '../effortScale.js'
import { effortName } from '../effortLevels.js'
import { FREE_START_RUNTIME } from '../../../shared/runtime-install.js'
import { MAX_COMPARE_SLOTS, MIN_COMPARE_SLOTS } from '../../../shared/compare.js'
import { policyModelAllowed } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'

export interface RouteChoice {
  readonly account?: string
  readonly runtime: MissionRuntimeId
  /** The model identifier a mission would actually be started with. */
  readonly model: string
}

/** A model picked for a comparison (0.441, shared/compare.ts), with the name its row shows. */
export interface ComparePick extends RouteChoice {
  readonly label: string
  /** This column's effort, chosen on its own chip; absent runs the model's default (0.490). */
  readonly effort?: string
}

/**
 * COMPARE IN THE PICKER (0.441). The picker the person already knows, with a
 * One / Compare switch: in Compare a row ticks instead of choosing, up to
 * three, and the foot says how many times the ask will run.
 */
export interface ComparePicking {
  readonly on: boolean
  readonly picks: readonly ComparePick[]
  readonly onMode: (on: boolean) => void
  readonly onToggle: (pick: ComparePick) => void
  /** Whether switching to Compare starts on two models already chosen (0.460); if not, the picker opens. */
  readonly prefills?: boolean
  /** Put this model in that column instead, keeping the others where they are (0.460). */
  readonly onReplace?: (index: number, pick: ComparePick) => void
  /** Set one column's effort (0.490): each model is compared at the level chosen for it. */
  readonly onEffort?: (index: number, effort: string) => void
  /** Why a model cannot be compared (it cannot be held read-only here), or nothing. */
  readonly refusal: (choice: RouteChoice) => string | undefined
  /** Each model changes its own copy of the project, rather than only answering (0.445). */
  readonly changes?: boolean
  readonly onChanges?: (changes: boolean) => void
  /** Why this folder cannot compare changes (not a git project), or nothing. */
  readonly changesRefusal?: string
  /** The folder is too big to copy, so a comparison that edits works in it directly (0.675): Auto says so. */
  readonly inPlace?: boolean
  /** Hide the names until one is kept (0.449). */
  readonly blind?: boolean
  readonly onBlind?: (blind: boolean) => void
  /** How each model has done in your decided comparisons, `runtime:model` (0.449). */
  readonly record?: ReadonlyMap<string, { readonly kept: number; readonly compared: number }>
}

const samePick = (a: RouteChoice, b: RouteChoice): boolean => a.runtime === b.runtime && a.model === b.model && a.account === b.account

/**
 * Words a detail may use while still only naming the group it sits under.
 * "Through Antigravity CLI" and "Antigravity model tier" say the heading
 * again; "Free · no sign-in" and a reset time do not.
 */
const DETAIL_FILLER = new Set(['through', 'via', 'cli', 'model', 'models', 'tier', 'tiers', 'the', 'a', 'an', 'its', 'and', 'of', 'on', 'with', 'your', 'own', 'account'])

/** True when the line only repeats the runtime the group heading already names. */
export function rowDetailRepeatsHeading(detail: string, group: string): boolean {
  const words = (text: string): string[] => text.toLowerCase().split(/[^a-z0-9]+/).filter((word) => word.length > 0)
  const said = words(detail).filter((word) => !DETAIL_FILLER.has(word))
  if (said.length === 0) return detail.trim().length > 0
  const named = new Set(words((group.split(' · ')[0] ?? group)))
  return said.every((word) => named.has(word))
}

interface RouteRow {
  readonly account?: string
  readonly key: string
  readonly group: string
  readonly runtime: MissionRuntimeId
  readonly model: string
  readonly label: string
  /** The row's one line: what the catalogue says, and what it resolved to. */
  readonly detail: string
  /** Everything known about it, effort levels included, for the row's hover. */
  readonly fullDetail: string
  readonly tag: RouteTag
  readonly selectable: boolean
  /** An older, fixed version: shown under its runtime's fold, or by a search. */
  readonly older?: boolean
}

/**
 * Rows come from discovery, and models come from the runtime's own catalog.
 * There are still no invented entries: when the catalog cannot be read the row
 * is the account default, which is exactly what the process is launched with.
 */
function buildRows(
  runtimes: readonly PublicRuntimeStatus[],
  models: readonly PublicModel[],
  active: RouteChoice,
  resolved: ReadonlyMap<string, string>,
  recent: readonly string[],
  limited: ReadonlyMap<string, string>
): readonly RouteRow[] {
  const rows: RouteRow[] = []
  for (const runtime of runtimes) {
    if (runtime.id === 'omniroute') continue
    const integration = integrationOf(runtime.id)
    const status = routeRowStatus(runtime, integration, false, limited.get(runtime.id))
    /*
     * THE ACCOUNT SUFFIX, ONLY WHERE THERE IS AN ACCOUNT.
     *
     * Every group header said `· your account` unconditionally, OpenCode's
     * included -- and OpenCode is the runtime the welcome screen recommends
     * BY NAME because it needs no account, whose free model this app's whole
     * on-ramp rests on. Gemini's handoff pass, 2026-09-21.
     * 
     * It is the same sentence-that-does-not-match-its-list defect as the
     * welcome screen's collapsed `they each need their own account`, on the
     * next control along, and it lands on the one person least able to tell
     * it is wrong: someone deciding whether picking this route will cost
     * them money.
     */
    const group = runtime.id === FREE_START_RUNTIME || runtime.id === 'kilo' ? runtime.displayName : `${runtime.displayName} · your account`

    // Every catalog model names its runtime -- Codex's from a live server
    // read, Claude's from what its CLI advertised -- and is offered only
    // there. A model under the wrong runtime would be a claim nothing checked.
    const forRuntime = models.filter((model) => model.runtime === runtime.id)
    const entries =
      forRuntime.length > 0
        ? forRuntime.map((model) => {
            const efforts =
              model.supportedEfforts.length > 0
                ? `${model.supportedEfforts.length} effort level${model.supportedEfforts.length === 1 ? '' : 's'} · ${model.supportedEfforts.join(', ')}`
                : 'no effort levels reported'
            // The name the runtime itself reported the last time a mission ran
            // on this route. Absent until one has, which is the honest state.
            const name = resolved.get(`${runtime.id}:${model.id}`)
            const measured = name === undefined ? efforts : `${name} · ${efforts}`
            // What the CATALOGUE said about this model, first. For OpenCode's
            // free models that is "Free · no sign-in", which its own comment
            // calls the whole reason the runtime is here -- and it reached
            // nobody, because a row's detail was built from the effort count
            // and the resolved name alone. A person with no account read
            // eleven names and could not tell which cost nothing (QA,
            // 2026-09-06). Effort levels still follow it; they are the more
            // technical half and the less urgent one.
            const described = model.description
            return {
              model: model.id,
              /*
               * A ROW THAT IS STILL AN IDENTIFIER GETS SPELLED OUT.
               *
               * The catalogue falls back to the model's id when a runtime
               * reports no display name for it, so the picker printed
               * muse-spark-1.3-contributor-free in a list where every
               * other row read as a proper name -- and the composer chip
               * directly beside it read "Muse Spark 1.3 Contributor Free",
               * because the chip has gone through modelDisplayName since
               * Grok found the same inconsistency in mission rows (pass 1,
               * finding 1). The picker was the one untreated spot.
               *
               * Only when the label IS the id. A name the runtime actually
               * gave is left exactly as the runtime wrote it: this spells
               * identifiers, it does not restyle anybody's product name.
               */
              // Claude's rows name the version their alias runs -- what a
              // finished run on the route reported, else Claude Code's own
              // table (see routeModelName) -- the same words as the chip.
              label:
                runtime.id === 'claude'
                  ? routeModelName(runtime.id, model.id, name)
                  : model.displayName === model.id || model.id.endsWith(`/${model.displayName}`)
                    ? // OpenCode's catalogue names a model by its id without
                      // the provider: `ling-3.0-flash-fin-free` for
                      // `opencode/ling-3.0-flash-fin-free`, which the picker
                      // drew raw while the chip said "Ling 3.0 Flash Fin"
                      // (seen on the packaged 0.482).
                      modelDisplayName(runtime.id, model.id)
                    : model.displayName,
              /*
               * ONE LINE PER MODEL, the way Claude Code's and Codex's own
               * pickers draw them.
               *
               * The line used to be the description, the resolved name AND
               * every effort level, and at the picker's width it wrapped to
               * three lines: about four models fit in view (frames,
               * 2026-09-22), in the control this product is built around.
               * The effort levels are the part a person does not choose a
               * model by -- the effort control beside the composer lists them
               * for the model that is chosen -- so the row keeps what the
               * catalogue says and what it resolved to, and the whole detail
               * moves to the row's hover. A model the catalogue says nothing
               * about keeps its effort levels as its line, rather than none.
               */
              detail:
                [described, name].filter((part): part is string => part !== undefined && part.length > 0).join(' · ')
                || levelsLine(model.supportedEfforts, effortName)
                || efforts,
              fullDetail: described === undefined || described.length === 0 ? measured : `${described} · ${measured}`,
              older: model.older === true,
              own: model.own === true
            }
          })
        : // The catalogue could not be read for this runtime, so there is one
          // row and it is the account's own default. It is labelled the way
          // the catalogue labels it -- a person reading a lowercase
          // `account-default` on the only ACTIVE row is reading a placeholder
          // that leaked (outside review, 2026-09-07).
          [{ model: 'account-default', label: 'Account default', detail: status.detail, fullDetail: status.detail, older: false, own: false }]

    for (const entry of entries) {
      // A route saved on one of a family's variants is still on that row: a
      // teammate on `claude-opus-5-5-high` is on Claude Opus 5.5, whichever
      // variant the row itself stands on.
      const isActive =
        runtime.id === active.runtime &&
        (entry.model === active.model || modelFamily(models, runtime.id, active.model)?.id === entry.model)
      rows.push({
        key: `${runtime.id}:${entry.model}`,
        // A model the person added runs on OpenCode and is listed as theirs.
        group: entry.own ? OWN_MODELS_GROUP : group,
        runtime: runtime.id as MissionRuntimeId,
        model: entry.model,
        label: entry.label,
        detail: entry.detail,
        fullDetail: entry.fullDetail,
        // Carried explicitly: this object is rebuilt field by field, so a
        // property added to the entry above is dropped here unless it is
        // named -- which is exactly what happened first (drive, 2026-09-06:
        // the row's own detail said "5 effort levels" while the control
        // under it drew none).
        tag: routeRowTag(status, isActive),
        selectable: status.selectable,
        // An ACTIVE older version is shown with the current ones: the route in
        // use is never folded out of sight.
        older: entry.older && !isActive
      })
    }
  }
  /*
   * YOUR OWN AGENTS (0.777): each a group of its own, named by the agent, with the models its Test listed -- or one
   * row, its own default, when it offered none. Never discovered, so there is no status to read: an agent that will
   * not start says so when the turn is sent, in its own words.
   */
  for (const model of models) {
    if (model.runtime !== 'acp') continue
    const isActive = active.runtime === 'acp' && active.model === model.id
    rows.push({
      key: `acp:${model.id}`,
      group: `${model.agentName ?? 'Your agent'} · your own agent`,
      runtime: 'acp',
      model: model.id,
      label: model.displayName,
      detail: model.description,
      fullDetail: model.description,
      tag: isActive ? 'ACTIVE' : 'READY',
      selectable: true
    })
  }
  // Within each runtime: what this person has run, then the flagship
  // families, then the rest as the runtime listed them. The routes they move
  // between are then lifted to a group of their own at the top, because the
  // move this product exists for is between runtimes, not within one.
  const ordered = olderLast(orderRouteRows(rows, recent))
  return [...recentRouteRows(ordered, recent), ...ordered]
}

/** Older versions at the end of their own runtime's group, in order, where the fold opens. */
function olderLast<TRow extends { readonly group: string; readonly older?: boolean }>(rows: readonly TRow[]): readonly TRow[] {
  const groups = new Map<string, number>()
  for (const row of rows) if (!groups.has(row.group)) groups.set(row.group, groups.size)
  return rows
    .map((row, index) => ({ row, index }))
    .sort(
      (a, b) =>
        (groups.get(a.row.group) ?? 0) - (groups.get(b.row.group) ?? 0) ||
        Number(a.row.older === true) - Number(b.row.older === true) ||
        a.index - b.index
    )
    .map((entry) => entry.row)
}

/**
 * THE FOLD FOR OLDER VERSIONS.
 *
 * Colin, 2026-09-22: "folded claude models is great, accessible but not
 * crowding". Unsearched, a runtime's older versions are one row under its
 * current models -- "Older versions  8" -- that opens them in place. A
 * search reaches into the fold without opening it: typing "opus 4.8" finds
 * Opus 4.8. A recent shortcut to one stays in Recent, where it was put.
 */
export function unfoldedRows<TRow extends { readonly group: string; readonly older?: boolean }>(
  rows: readonly TRow[],
  searching: boolean
): readonly TRow[] {
  return searching ? rows : rows.filter((row) => row.older !== true || row.group === 'Recent')
}

/** Each group's older versions, for its fold. */
export function olderByGroup<TRow extends { readonly group: string; readonly older?: boolean }>(
  rows: readonly TRow[]
): ReadonlyMap<string, readonly TRow[]> {
  const byGroup = new Map<string, TRow[]>()
  for (const row of rows) {
    if (row.older !== true || row.group === 'Recent') continue
    byGroup.set(row.group, [...(byGroup.get(row.group) ?? []), row])
  }
  return byGroup
}

/**
 * What to say when the picker is showing no rows.
 *
 * "Nothing matches that" is a claim about the SEARCH, and it must not be made
 * when the search is not why the list is empty.
 *
 * Seen walking the packaged 0.54.0 build as a new person would (2026-09-09):
 * open the picker, type `free`, and it answered "Nothing matches that." while
 * the status line read "6 runtimes connected". The model genuinely exists --
 * every other drive here selects it by that word -- but the catalog had not
 * arrived, and a person typing fast is told a model does not exist. It sent
 * that whole walkthrough to the wrong runtime without anyone noticing.
 *
 * The SAME defect 0.50.1 fixed on the effort chip, in a second place:
 * "effort · fixed" was drawn whenever the effort list was empty, which is also
 * what an unloaded catalog looks like. Neither surface may speak from having
 * no information.
 *
 * Extracted so it can be tested: the picker keeps its search in its own state,
 * so no test can reach this branch by rendering the component with a query.
 */
export function pickerEmptyMessage(modelCount: number): string {
  return modelCount === 0 ? 'Still reading the model list…' : 'Nothing matches that.'
}

export function RoutePicker({
  runtimes,
  models,
  resolvedModels,
  active,
  recentRoutes,
  onSelect,
  onClose,
  notice,
  limitedRuntimes,
  compare,
  slot,
  effort,
  openAt = 'model'
}: {
  readonly runtimes: readonly PublicRuntimeStatus[]
  /** Runtimes whose last run ended on the account's usage limit, with its own words. */
  readonly limitedRuntimes: ReadonlyMap<string, string>
  readonly models: readonly PublicModel[]
  /** What each route's model resolved to last time, keyed `runtime:model`. */
  readonly resolvedModels: ReadonlyMap<string, string>
  /** Routes this person has run, newest first, as `runtime:model`. */
  readonly recentRoutes: readonly string[]
  readonly active: RouteChoice
  readonly onSelect: (choice: RouteChoice) => void
  readonly onClose: () => void
  /**
   * Shown above the rows when picking has a consequence beyond the next
   * mission. The picker looks identical whether it is choosing a route or
   * moving a live run, so the difference has to be stated, not implied.
   */
  readonly notice?: string
  /**
   * ONE COLUMN OF A COMPARISON (0.460, as Arena draws it: a dropdown per
   * model). The picker chooses the model for that column alone: a model that
   * cannot join, or is already in another column, cannot be picked, and with
   * three columns this one can be removed.
   */
  readonly slot?: {
    readonly refusal: (choice: RouteChoice) => string | undefined
    readonly taken: readonly RouteChoice[]
    readonly onRemove?: () => void
  }
  /**
   * Effort and swarm live here now, not on the composer.
   *
   * Effort is a property of the ROUTE, not a peer of it: on most routes the
   * old chip read "effort · fixed" -- a control whose value is "there is no
   * value here" -- while the picker was already listing each model's levels
   * in its detail line. And swarm DISABLED effort to hold it at the model
   * maximum, so two adjacent chips in the composer were one setting, with one
   * silently switching the other off (design review, 2026-09-06).
   *
   * Together they take the composer from seven controls to four.
   */
  /** The maximum level swarm would hold every mission at, when one is known. */
  /** Compare (0.441): absent where a comparison cannot start (a run is live, or a comparison is on screen). */
  readonly compare?: ComparePicking
  /** The effort slider, at the foot of the list, so model and effort are one control. */
  readonly effort?: ReactNode
  /** Where focus goes when the list opens. Effort is the slider at the foot. */
  readonly openAt?: 'model' | 'effort'
}): ReactElement {
  const accounts = useAgentAccounts()
  const comparing = compare?.on === true
  const policy = useOrganizationPolicy()
  const [query, setQuery] = useState('')
  const [olderOpen, setOlderOpen] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (openAt === 'effort') {
      const slider = document.querySelector<HTMLElement>('[data-effort-focus] input, [data-effort-focus] button')
      if (slider !== null) {
        slider.focus()
        return
      }
    }
    inputRef.current?.focus()
  }, [openAt])

  const rows = useMemo(
    () => {
      const base = buildRows(runtimes, models, active.account === undefined ? active : { runtime: active.runtime, model: '' }, resolvedModels, recentRoutes, limitedRuntimes)
      const added = policy?.policy.accounts === false ? [] : accounts.flatMap(a => {
        const ownRuntimes = runtimes.filter(r => r.id === a.runtime).map(r => ({ ...r, ready: r.installed, status: 'ready' as const, auth: 'authenticated' as const }))
        const ownLimits = new Map([[a.runtime, limitedRuntimes.get(accountUsageKey(a.runtime, a.id))!]])
        return buildRows(ownRuntimes, models, active.account === a.id ? active : { runtime: active.runtime, model: '' }, resolvedModels, [], ownLimits).map(row => ({ ...row, key: `${row.key}:${a.id}`, account: a.id, tag: a.state === 'ready' ? row.tag : 'SIGN IN' as RouteTag, group: `${row.group.replace(/ \u00b7 your account$/, '')} \u00b7 ${a.name}`, label: `${row.label} \u00b7 ${a.name}` }))
      })
      return [...base, ...added].filter(row => policyModelAllowed(policy, row.runtime, row.model))
    },
    [runtimes, models, active, resolvedModels, recentRoutes, limitedRuntimes, policy, accounts]
  )
  const needle = routeSearchText(query)
  const searching = needle.length > 0

  /*
   * THE PICKER OPENS ON WHAT IS CHOSEN (0.411). It opened at the top of its
   * list, Claude Code's models, whatever the teammate was on: a teammate on
   * an OpenCode free model had to scroll to find its own route (fresh-eyes
   * check, the chat box). Claude Code's picker opens on the current model.
   * Once per opening, when the chosen row is out of view, and never while
   * searching; rows load as discovery answers, so it waits for the row.
   */
  const listRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent): void => {
      const index = menuRowFromDigit(
        event.key,
        { ctrl: event.ctrlKey, meta: event.metaKey, alt: event.altKey, shift: event.shiftKey },
        isTypingTarget(event.target)
      )
      if (index === undefined) return
      const row = listRef.current?.querySelectorAll<HTMLButtonElement>('.lc-picker__row:not(:disabled)')[index]
      if (row === undefined) return
      event.preventDefault()
      row.click()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
  const shownChosen = useRef(false)
  useEffect(() => {
    if (shownChosen.current || searching) return
    const list = listRef.current
    const row = list?.querySelector<HTMLElement>('.lc-picker__row.is-active')
    if (list === null || list === undefined || row === null || row === undefined) return
    shownChosen.current = true
    const box = list.getBoundingClientRect()
    const at = row.getBoundingClientRect()
    if (at.top >= box.top && at.bottom <= box.bottom) return
    list.scrollTop += at.top - box.top - (box.height - at.height) / 2
  }, [rows, searching])
  const matched = useMemo(
    () =>
      unfoldedRows(
        searching ? rows.filter((row) => routeSearchText(`${row.group} ${row.label}`).includes(needle)) : rows,
        searching
      ),
    [rows, needle, searching]
  )
  const folds = useMemo(() => olderByGroup(rows), [rows])
  // One runtime can list hundreds of models. Every group is capped until the
  // person searches, and each capped group says how many it is not showing.
  const { rows: shown, hiddenByGroup } = useMemo(
    () => capRouteRows(matched, ROUTE_GROUP_LIMIT, needle.length > 0),
    [matched, needle]
  )

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
      return
    }
    /*
     * ENTER TAKES THE ONE MODEL LEFT (0.495, Boss's review). Typing a name
     * down to a single row and pressing Enter did nothing. The row pressed is
     * the one on screen, so it follows every rule the row itself does --
     * refused, full or unselectable rows are not buttons that can be pressed.
     */
    if (event.key === 'Enter' && searching) {
      const left = [...(listRef.current?.querySelectorAll<HTMLButtonElement>('.lc-picker__row:not(.is-recent):not(:disabled)') ?? [])]
      if (left.length === 1) {
        event.preventDefault()
        left[0]!.click()
      }
    }
  }

  /** One model row, the same wherever it is drawn: in its group or under a fold. */
  const drawRow = (row: RouteRow): ReactElement => {
    const isActive = !comparing && (row.account === undefined ? row.tag === 'ACTIVE' : row.account === active.account && row.runtime === active.runtime && row.model === active.model)
    const recent = row.group === 'Recent'
    const choice = { runtime: row.runtime, model: row.model, ...(row.account === undefined ? {} : { account: row.account }) }
    const picked = comparing && (compare?.picks ?? []).some((pick) => samePick(pick, choice))
    const refused = comparing
      ? compare?.refusal(choice)
      : slot === undefined
        ? undefined
        : slot.refusal(choice) ?? (slot.taken.some((other) => samePick(other, choice)) ? 'Already in this comparison.' : undefined)
    const full = comparing && !picked && (compare?.picks.length ?? 0) >= MAX_COMPARE_SLOTS
    // Choosing what to compare: the model's own record, where it has one (0.449).
    const past = comparing ? compare?.record?.get(`${row.runtime}:${row.model}`) : undefined
    const spoken = past === undefined ? row.detail : `kept ${String(past.kept)} of ${String(past.compared)}`
    // A line that only names the group again is the heading, said twice.
    // A limit, a reset, an endpoint, or a comparison record is news, and stays.
    const showDetail = !recent && spoken.trim().length > 0 && (past !== undefined || !rowDetailRepeatsHeading(spoken, row.group))
    // READY is the dot. Every other tag says something the dot does not.
    const showTag = !recent && row.tag !== 'READY'
    // Which runtime the canonical row below sits under, so a recent row can
    // say why it appears twice.
    const pointsAt = recent
      ? rows.find((other) => other.key === row.key.replace(/^recent:/, ''))?.group.replace(/ · your account$/, '')
      : undefined
    return (
      <button
        type="button"
        className={`lc-picker__row${isActive ? ' is-active' : ''}${recent ? ' is-recent' : ''}${picked ? ' is-picked' : ''}${!recent && !showDetail ? ' is-plain' : ''}`}
        disabled={!row.selectable || refused !== undefined || full}
        aria-current={isActive}
        {...(comparing ? { 'aria-pressed': picked } : {})}
        // Its name and its state, and nothing twice (0.495): a screen reader read the
        // row's every word -- "Nemotron 3.5 Lightning Free Free · no sign-in ... READY" (Grok).
        aria-label={`${row.label}${isActive ? ', in use' : row.tag === 'READY' || row.tag === undefined ? '' : `, ${String(row.tag).toLowerCase()}`}${refused !== undefined ? `. ${refused}` : ''}`}
        // The whole of it, on hover: the row itself is one line.
        title={refused ?? (full ? `Three at most. Untick one to pick ${row.label}.` : `${row.label} · ${row.fullDetail}`)}
        onClick={() => {
          if (comparing && compare !== undefined) {
            // Named the way the chip names a route: "Nemotron 3 Ultra Free", not its id.
            compare.onToggle({ ...choice, label: `${routeFamilyName(row, resolvedModels, models)}${row.account === undefined ? '' : ` \u00b7 ${accounts.find(a => a.id === row.account)?.name ?? 'Removed account'}`}` })
            return
          }
          onSelect(choice)
          onClose()
        }}
      >
        {comparing && <span className="lc-picker__check" aria-hidden="true">{picked ? '✓' : ''}</span>}
        <span
          className={`lc-dot ${
            row.tag === 'ACTIVE'
              ? 'lc-tone-lime'
              : row.tag === 'READY'
                ? 'lc-tone-green'
                : row.tag === 'PREVIEW' || row.tag === 'EXPERIMENTAL' || row.tag === 'AT LIMIT'
                  ? 'lc-tone-amber'
                  : 'lc-tone-muted'
          }`}
        />
        <span className="lc-picker__text">
          <span className="lc-picker__label">{row.label}</span>
          {/* A shortcut row carries neither detail nor tag: the
            * canonical row below owns those, so ACTIVE appears exactly
            * once on screen. */}
          {showDetail && <span className="lc-picker__detail lc-mono">{spoken}</span>}
        </span>
        {recent ? (
          pointsAt === undefined ? null : (
            <span className="lc-picker__pointer lc-mono">
              {'↓ '}
              {!isOwnRoute(row.model) && <RuntimeMark runtime={row.runtime} size={11} className="is-inline" />}
              {pointsAt}
            </span>
          )
        ) : showTag ? (
        <span
          className={`lc-picker__tag lc-mono ${
            row.tag === 'ACTIVE'
              ? 'lc-tone-lime'
              : row.tag === 'PREVIEW' || row.tag === 'EXPERIMENTAL'
                ? 'lc-tone-amber'
                : row.tag === 'SIGN IN'
                  ? 'lc-tone-red'
                  : 'lc-tone-muted'
          }`}
        >
          {row.tag}
        </span>
        ) : null}
      </button>
    )
  }

  let lastGroup: string | undefined

  return (
    <div className="lc-picker" role="dialog" aria-label="Choose runtime and model" onKeyDown={onKeyDown}>
      <div className="lc-picker__head">
        <span className="lc-mono lc-separator">/</span>
        <input
          ref={inputRef}
          className="lc-picker__input"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search runtimes and models"
          aria-label="Search runtimes and models"
          autoComplete="off"
        />
        {/* Direct, Compare or Blind is chosen in the chat mode chip now (0.451), not here. */}
        {/*
          * Swarm, as a pill beside the search rather than a row of its own.
          * It is a statement about how every mission runs, so it sits with
          * the thing it is about -- and the reference draws it small and to
          * the right, not full width (`Locust UI Review 2026-09-06.dc.html`).
          * The mark stays as its glyph.
          */}
        {/*
          * No swarm toggle here any more. The mark is back on the composer
          * where Colin wants it -- "it just looks cool, its our logo, and
          * feels like something the user should know is on" -- and two
          * toggles for one setting is the confusion the design review named
          * in the first place. What stays is the CONSEQUENCE, one line down:
          * the effort levels grey out and say who is holding them.
          */}
      </div>
      {notice !== undefined && <div className="lc-picker__notice">{notice}</div>}
      {(policy?.policy.allowedAgents !== undefined || policy?.policy.allowedModels !== undefined) && <ManagedPolicyNote />}
      <div className="lc-picker__list" ref={listRef}>
        {shown.map((row, index) => {
          const header = row.group === lastGroup ? undefined : row.group
          lastGroup = row.group
          // The last row of a capped group carries the count it held back.
          const hidden = shown[index + 1]?.group === row.group ? 0 : hiddenByGroup.get(row.group) ?? 0
          const recent = row.group === 'Recent'
          // The fold closes a runtime's own group, under its current models.
          const fold = !searching && shown[index + 1]?.group !== row.group ? folds.get(row.group) : undefined
          return (
            <div key={row.key} className={recent ? 'lc-picker__tray' : undefined}>
              {header !== undefined && (
                <div className="lc-picker__group">
                  {/* The runtime's mark on its own group (0.383); not on
                      Recent, which mixes runtimes, nor on the person's own
                      models, which are named as theirs. */}
                  {!recent && row.group !== OWN_MODELS_GROUP && <RuntimeMark runtime={row.runtime} size={12} className="is-inline" />}
                  {header}
                  {recent && <span className="lc-picker__grouphint">shortcuts to rows below</span>}
                </div>
              )}
              {drawRow(row)}
              {/*
                * No effort control here. It lived under the selected model
                * for one release and was removed on Colin's word
                * (2026-09-07): "just go back to effort being separate,
                * completely remove it from the model page". Each row's
                * detail line still NAMES the levels a model reports, which
                * is information; choosing between them is the composer's
                * job, next to the model it applies to.
                */}
              {hidden > 0 && (
                <p className="lc-picker__more lc-mono">
                  {hidden} more {hidden === 1 ? 'model' : 'models'} {needle.length > 0 ? 'match · keep typing' : '· type to search them'}
                </p>
              )}
              {fold !== undefined && fold.length > 0 && (
                <>
                  <button
                    type="button"
                    className="lc-picker__fold"
                    aria-expanded={olderOpen}
                    onClick={() => setOlderOpen((open) => !open)}
                  >
                    <span className={`lc-picker__foldmark${olderOpen ? ' is-open' : ''}`} aria-hidden="true" />
                    Older versions
                    <span className="lc-picker__foldcount lc-mono">{fold.length}</span>
                  </button>
                  {olderOpen && fold.map((older) => <div key={older.key}>{drawRow(older)}</div>)}
                </>
              )}
            </div>
          )
        })}
        {/*
          * "Nothing matches that" is a claim about the SEARCH, and it must not
          * be made when the search is not why the list is empty.
          *
          * Seen walking the packaged 0.54.0 build as a new person would
          * (2026-09-09): open the picker, type `free`, and it answers "Nothing
          * matches that." while the status line reads "6 runtimes connected".
          * The model genuinely exists -- every other drive picks it by that
          * word -- but the catalog had not arrived yet, and a person typing
          * fast is told the model does not exist. It sent that whole
          * walkthrough to the wrong runtime.
          *
          * This is the SAME defect 0.50.1 fixed on the effort chip, in a
          * second place: "effort · fixed" was drawn whenever the effort list
          * was empty, which is also what an unloaded catalog looks like. An
          * empty list is not an answer, and neither of these surfaces may
          * speak from having no information.
          */}
        {shown.length === 0 && <p className="lc-inspector__empty">{pickerEmptyMessage(models.length)}</p>}
      </div>
      {comparing && compare !== undefined ? (
        <div className="lc-picker__foot lc-picker__foot--compare">
          <span>
            {compare.picks.length < MIN_COMPARE_SLOTS
              ? `Pick ${compare.picks.length === 0 ? 'two or three models' : 'one or two more'}. ${compare.changes === true ? 'Each works in its own copy of your folder.' : 'Each answers the same ask without changing files.'}`
              : `${String(compare.picks.length)} picked. Your ask runs ${compare.picks.length === 2 ? 'twice' : 'three times'}, once on each.`}
          </span>
          <button type="button" className="lc-primarybutton" disabled={compare.picks.length < MIN_COMPARE_SLOTS} onClick={onClose}>
            Done
          </button>
        </div>
      ) : slot?.onRemove !== undefined ? (
        <div className="lc-picker__foot lc-picker__foot--compare">
          <span>Compare two instead of three.</span>
          <button type="button" className="lc-button" onClick={() => { slot.onRemove?.(); onClose() }}>
            Remove this model
          </button>
        </div>
      ) : (
        <div className="lc-picker__foot">
          {effort}
          <span>Privacy and permissions live in Settings.</span>
        </div>
      )}
    </div>
  )
}
