import { createAppServerClient, MUSE_EFFORTS, openCodeUsesAcp, usageWindowFromSnapshot } from '@teammate/runtime-adapters'
import type { ClaudeListedModel, MissionRuntimeId, RuntimeDiscovery } from '@teammate/runtime-adapters'

import type { PublicModel, ModelCatalogResponse } from '../shared/ipc.js'
import { CLAUDE_ALIAS_DEFAULTS, CLAUDE_OLDER_MODELS, claudeModelName } from '../shared/claude-models.js'
import type { AppServerRunProcess as AppServerProcess } from '@teammate/runtime-adapters'
import { antigravityUsageText, readAntigravityUsage } from './antigravity-usage.js'
import { readOpenCodeAcpModels } from './opencode-acp-models.js'

/**
 * The model catalog.
 *
 * `model/list` is an app-server call, so reading it means briefly starting one.
 * That is worth doing once: it is the only source of real model ids and of each
 * model's supported reasoning efforts, and without it the shell would keep
 * showing `account-default` and an effort menu that cannot know what it may
 * offer.
 *
 * It fails SOFT. A catalog that cannot be read means the picker falls back to
 * the account default -- which is what the exec transport uses anyway -- rather
 * than the app refusing to start a mission.
 */

const PROBE_TIMEOUT_MS = 20_000
/** How long a successful read stays good. Models change on release, not hourly. */
const CACHE_MS = 10 * 60 * 1000
/** How long the model list waits for Antigravity's usage before going without it (0.709). */
const USAGE_WAIT_MS = 1_500
const MAX_MODELS = 40
/**
 * The route a fresh profile starts on, before any model is chosen.
 *
 * Stated here rather than imported: the main process does not reach into
 * the renderer, and this id is protocol-level either way.
 */
const ACCOUNT_DEFAULT_MODEL = 'account-default'

export interface ModelCatalogOptions {
  readonly discover: () => Promise<readonly RuntimeDiscovery[]>
  readonly spawn: (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string) => AppServerProcess
  /** A new empty temporary directory, only for OpenCode 2's no-prompt catalog session. */
  readonly opencodeScratch?: () => Promise<string>
  readonly now?: () => number
  /** The models Claude Code's own handshake listed this session, when it has (0.697). */
  readonly claudeListed?: () => readonly ClaudeListedModel[] | undefined
  /** Antigravity's usage, without a model turn (0.685). Injected by tests; the real reader otherwise. */
  readonly readAntigravity?: (runtime: RuntimeDiscovery) => Promise<string | undefined>
  /** A usage reading arrived after the list it belonged to went out: the window should read again. */
  readonly onLateUsage?: () => void
}

export interface ModelCatalog {
  read(): Promise<ModelCatalogResponse>
  /** Drop what was read: an agent just changed under it (runtime-updates.ts). */
  forget(): void
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/**
 * Read the protocol's model list into the product's own shape. Every field is
 * checked: a model without an id is unusable, and a model whose efforts cannot
 * be read is reported as supporting none rather than as supporting all.
 */
export function parseModels(result: unknown): readonly PublicModel[] {
  const data = isObject(result) && Array.isArray(result.data) ? result.data : []
  const models: PublicModel[] = []
  for (const entry of data.slice(0, MAX_MODELS)) {
    if (!isObject(entry)) continue
    const id = typeof entry.id === 'string' ? entry.id : undefined
    if (id === undefined || id.length === 0) continue
    if (entry.hidden === true) continue
    const efforts: string[] = []
    if (Array.isArray(entry.supportedReasoningEfforts)) {
      for (const effort of entry.supportedReasoningEfforts) {
        const name = isObject(effort) ? effort.reasoningEffort : effort
        if (typeof name === 'string' && name.length > 0 && !efforts.includes(name)) efforts.push(name)
      }
    }
    models.push({
      id,
      runtime: 'codex',
      displayName: typeof entry.displayName === 'string' && entry.displayName.length > 0 ? entry.displayName : id,
      description: typeof entry.description === 'string' ? entry.description.slice(0, 200) : '',
      supportedEfforts: efforts
    })
  }
  return models
}

/**
 * The account default, as a row you can actually select, for one runtime.
 *
 * It is the route a fresh profile starts on, and it was the only route in the
 * app that named a model no list contained -- so the picker drew no ACTIVE row
 * for it, and the effort levels, which hang off that row, had nowhere to go.
 *
 * Built PER RUNTIME. `account-default` is not a Codex idea: codex-mission.ts
 * calls it "the shell's word for send no --model", and every runtime honours
 * it that way. Building it for Codex alone meant that once discovery started
 * preferring OpenCode on a fresh machine, the route a new person landed on
 * had no row again -- the same defect one runtime to the left.
 *
 * The levels offered are the ones EVERY listed model of that runtime agrees
 * on. An intersection rather than a union, because the account decides which
 * model answers, and a level only some of them accept would be a control that
 * silently does nothing. Effort itself is safe to offer here: it travels
 * separately from the model on every runtime that takes one.
 *
 * Undefined when that runtime listed nothing -- then there is no honest claim
 * to make about what the account supports, and the picker is right to stay
 * quiet.
 *
 * The intersection is also what keeps this SAFE on the runtimes whose command
 * builders refuse an effort outright ("Cursor Agent takes no effort level",
 * and the same for Gemini, OpenCode and Copilot). Each of those is a runtime
 * whose models report no levels, so the intersection is empty, so no chip is
 * drawn and nothing is sent. A union would have offered a level on the very
 * runtimes that throw on one.
 */
export function accountDefaultModel(
  models: readonly PublicModel[],
  runtime: MissionRuntimeId = 'codex'
): PublicModel | undefined {
  // Current models only: an older, fixed version is not what the account
  // answers with, and two of Claude's report no levels at all -- counted, they
  // would empty the account default's levels for everyone.
  const listed = models.filter(
    (model) => model.runtime === runtime && model.id !== ACCOUNT_DEFAULT_MODEL && model.older !== true
  )
  if (listed.length === 0) return undefined
  const shared = listed
    .map((model) => model.supportedEfforts)
    .reduce<readonly string[]>(
      (kept, efforts) => kept.filter((effort) => efforts.includes(effort)),
      listed[0]?.supportedEfforts ?? []
    )
  return {
    id: ACCOUNT_DEFAULT_MODEL,
    runtime,
    displayName: 'Account default',
    description: 'Whatever model your account uses. The effort levels below apply to all of them.',
    supportedEfforts: shared
  }
}

/**
 * Every runtime's account-default row, in front of the models they belong to.
 *
 * One per runtime that listed anything, because `defaultRoute` can stamp
 * `account-default` on any of them and each one needs a row of its own to be
 * the ACTIVE one.
 */
export function withAccountDefaults(models: readonly PublicModel[]): readonly PublicModel[] {
  // Idempotent: a runtime that already carries its row is left alone. Two
  // call sites reach this (the probe, and the early return when Codex is not
  // ready), and a list that went through both would otherwise show the same
  // "Account default" twice.
  const already = new Set(
    models.filter((model) => model.id === ACCOUNT_DEFAULT_MODEL).map((model) => model.runtime)
  )
  const defaults = [...new Set(models.map((model) => model.runtime))]
    .filter((runtime) => !already.has(runtime))
    .map((runtime) => accountDefaultModel(models, runtime))
    .filter((row): row is PublicModel => row !== undefined)
  return [...defaults, ...models]
}

/**
 * Aliases Claude Code accepts that its `--help` does not name.
 *
 * The help gives EXAMPLES -- "an alias for the latest model (e.g. 'fable',
 * 'opus', or 'sonnet')" -- and the picker offered exactly the examples, so
 * the cheapest family could not be chosen in Locust at all. Measured, not
 * assumed: `claude --model haiku` on Claude Code 2.1.280 ran as
 * claude-haiku-4-5-20251001 (2026-09-22, the background-task capture).
 * Offered only beside advertised aliases, so a CLI whose help stops naming
 * aliases at all is not handed one by this list.
 */
export const CLAUDE_ALIASES_MEASURED: readonly string[] = ['haiku']

/**
 * Claude Code's models, from what its CLI advertised at discovery. An alias
 * resolves to the newest model of that family on the runtime's side, which is
 * why it is offered as the alias rather than as a version this build guessed.
 */
export function claudeModelsFrom(runtimes: readonly RuntimeDiscovery[], listed: readonly ClaudeListedModel[] = []): readonly PublicModel[] {
  const claude = runtimes.find((entry) => entry.id === 'claude')
  const hints = claude?.modelHints
  if (claude?.readiness !== 'ready' || hints === undefined) return []
  /*
   * What Claude Code's own picker lists (0.697), when its handshake has
   * answered: the alias rows by the name and efforts it gives them, and its
   * fixed versions first among the older ones. Before it answers, and on a
   * Claude Code too old to list them, the copied table below stands in.
   */
  const listedAlias = new Map(listed.filter((model) => !model.value.startsWith('claude-') && model.value !== 'default').map((model) => [model.value, model]))
  const aliases = hints.aliases.length === 0
    ? []
    : [...new Set([...hints.aliases, ...CLAUDE_ALIASES_MEASURED, ...listedAlias.keys()])]
  const current: PublicModel[] = aliases.map((alias) => {
    const family = `${alias.charAt(0).toUpperCase()}${alias.slice(1)}`
    const meant = CLAUDE_ALIAS_DEFAULTS[alias]
    const own = listedAlias.get(alias)
    return {
      id: alias,
      runtime: 'claude',
      // The version the alias means: Claude Code's own name for it when it
      // listed one -- "Haiku 5.5" -- else from the copied model registry
      // (see shared/claude-models.ts). A run that reports something else
      // wins on its own route (the picker and the chip read the resolved
      // name first). An alias neither knows keeps its own name rather than
      // a made-up version.
      displayName: own?.displayName ?? (meant === undefined ? undefined : claudeModelName(meant)) ?? family,
      // What the alias is FOR: it moves with the family, which is the one
      // thing a pinned version would not do.
      description: `Always the newest ${family}`,
      supportedEfforts: own === undefined ? hints.efforts : (own.efforts ?? [])
    }
  })
  // The fixed versions, folded under the current ones. Offered only beside
  // advertised aliases: a CLI that names none may not take full names either.
  // A version an alias already means is not repeated, and one version listed
  // twice (Claude Code's `claude-haiku-4-5-20251001`, the table's own) is
  // offered once, under Claude Code's id.
  const meansNow = new Set([
    ...[...listedAlias.values()].map((model) => claudeModelName(model.resolvedModel) ?? model.resolvedModel),
    ...aliases.map((alias) => listedAlias.has(alias) ? undefined : claudeModelName(CLAUDE_ALIAS_DEFAULTS[alias] ?? alias)).filter((name): name is string => name !== undefined)
  ])
  const fixed = [
    ...listed.filter((model) => model.value.startsWith('claude-')).map((model) => ({ id: model.value, name: model.displayName, efforts: model.efforts ?? [] })),
    ...CLAUDE_OLDER_MODELS.map((model) => ({ id: model.id, name: claudeModelName(model.id) ?? model.id, efforts: model.efforts }))
  ]
  const offered = new Set<string>()
  const older: PublicModel[] = current.length === 0
    ? []
    : fixed.filter((model) => {
        if (meansNow.has(model.name) || offered.has(model.name)) return false
        offered.add(model.name)
        return true
      }).map((model) => ({
        id: model.id,
        runtime: 'claude',
        displayName: model.name,
        description: 'This version, always',
        supportedEfforts: model.efforts,
        older: true
      }))
  return [...current, ...older]
}

/**
 * The effort words Cursor writes into a model id -- the same words, in the
 * same order, as the effort control's scale (`effortScale.ts`).
 *
 * `none`, `minimal` and `max` were missing until 2026-09-23, so
 * `claude-opus-5-5-max` was a model of its own beside `claude-opus-5-5`, and
 * `kimi-k3-max` -- which Cursor lists as plain "Kimi K3", its default -- sat
 * beside a second "Kimi K3" made of the low and high variants.
 */
const CURSOR_EFFORT_WORDS: readonly string[] = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max']

/**
 * Split `cursor-grok-4.6-high-fast` into the model and the effort.
 *
 * Cursor lists every effort of every model as its own entry -- 241 of them on
 * a real account -- which turned the picker into a wall nobody could read.
 * They are one model with an effort each, which is what the picker already
 * knows how to show, and what the effort control exists for.
 *
 * The id's last words, read from the end: an optional `fast`, then the
 * effort (`xhigh`, or GPT-5.5's `extra-high`, which is the same level), with
 * `thinking` on either side of it -- Cursor writes `claude-opus-5-thinking-high`
 * and `claude-4.6-opus-high-thinking` for the same kind of model. `thinking`
 * stays in the family: Cursor names those as models of their own ("Claude
 * Opus 5 1M Thinking"), not as a level. A word is only taken while another is
 * left, so a name that IS an effort word ("fast") is a name.
 */
export function splitCursorModelId(id: string): { readonly family: string; readonly effort?: string } {
  const words = id.split('-')
  const last = (): string => words[words.length - 1] ?? ''
  let fast = false
  let thinking = false
  let level: string | undefined
  if (words.length > 1 && last() === 'fast') {
    fast = true
    words.pop()
  }
  if (words.length > 1 && last() === 'thinking') {
    thinking = true
    words.pop()
  }
  if (words.length > 2 && words[words.length - 2] === 'extra' && last() === 'high') {
    level = 'xhigh'
    words.splice(-2)
  } else if (words.length > 1 && CURSOR_EFFORT_WORDS.includes(last())) {
    level = words.pop()
  }
  if (!thinking && level !== undefined && words.length > 1 && last() === 'thinking') {
    thinking = true
    words.pop()
  }
  const effort = level === undefined ? (fast ? 'fast' : undefined) : fast ? `${level}-fast` : level
  if (effort === undefined) return { family: id }
  const family = `${words.join('-')}${thinking ? '-thinking' : ''}`
  return { family, effort }
}

/** A Cursor display name as words: its zero-width padding and doubled spaces gone. */
function cursorNameWords(name: string): readonly string[] {
  return name.replace(/[​-‍﻿]/g, '').trim().split(/\s+/).filter((word) => word.length > 0)
}

/**
 * THE NAME A FAMILY'S VARIANTS SHARE, in Cursor's own words.
 *
 * Cursor lists `claude-opus-5-5-low` as "Claude Opus 5.5 1M Low",
 * `claude-opus-5-5-medium` as "Claude Opus 5.5 1M" and `-xhigh` as
 * "Claude Opus 5.5 1M Extra High": the effort is a word it adds to the
 * model's name, before any trailing part ("Claude Fable 5.1 1M Low (NO
 * ZDR)", "Claude Opus 5 1M Low Thinking"). The words every variant has at the
 * start, and the ones every variant has at the end, are the model's own name
 * -- the name Cursor gives the variant it treats as the default, when it lists
 * one that way. Nothing is invented: every word is Cursor's.
 *
 * Undefined when the variants share no word at all, and the caller keeps what
 * it had.
 */
export function sharedCursorName(names: readonly string[]): string | undefined {
  const lists = names.map(cursorNameWords)
  const first = lists[0]
  if (first === undefined) return undefined
  let head = 0
  while (head < first.length && lists.every((words) => words[head] === first[head])) head += 1
  let tail = 0
  // A shared ending is counted only where it does not reach back into the
  // shared start of any variant, so no word is used twice.
  while (
    tail < first.length - head &&
    lists.every((words) => words.length - 1 - tail >= head && words[words.length - 1 - tail] === first[first.length - 1 - tail])
  ) {
    tail += 1
  }
  const shared = [...first.slice(0, head), ...first.slice(first.length - tail)]
  return shared.length === 0 ? undefined : shared.join(' ')
}

/**
 * Cursor Agent's models, grouped. Each row is one model; its efforts are the
 * variants Cursor actually listed, and `variants` says which concrete id each
 * one means, because the effort travels inside the id rather than as a flag.
 */
export function cursorModelsFrom(runtimes: readonly RuntimeDiscovery[]): readonly PublicModel[] {
  const cursor = runtimes.find((entry) => entry.id === 'cursor')
  const listed = cursor?.modelHints?.models
  if (cursor?.readiness !== 'ready' || listed === undefined) return []

  const families = new Map<string, {
    plainName?: string
    defaultId: string
    readonly variantNames: string[]
    readonly fastNames: string[]
    readonly variants: Record<string, string>
    readonly efforts: string[]
  }>()
  for (const model of listed) {
    const { family, effort } = splitCursorModelId(model.id)
    const held = families.get(family) ?? {
      // Until a plain variant turns up, the first one seen stands in, so a
      // family that only ever appears with an effort is still selectable.
      defaultId: model.id,
      variantNames: [],
      fastNames: [],
      variants: {},
      efforts: []
    }
    if (effort === undefined) {
      held.plainName = model.displayName
      held.defaultId = model.id
    } else {
      held.variants[effort] = model.id
      if (!held.efforts.includes(effort)) held.efforts.push(effort)
      // A fast variant is the same model sooner, and Cursor puts "Fast" LAST
      // ("Claude Opus 5 1M Thinking Fast"), which would hide the shared
      // ending the rest have. It names the family only if nothing else does.
      if (effort.endsWith('fast')) held.fastNames.push(model.displayName)
      else held.variantNames.push(model.displayName)
    }
    families.set(family, held)
  }

  return [...families.entries()].map(([family, held]) => {
    /*
     * A family Cursor lists only through its variants is named by what those
     * variants share -- "Claude Opus 5.5 1M", Cursor's own name for it.
     *
     * It used to be the family's id, on the reasoning that one variant's name
     * would claim that variant's effort; and the picker prints a name that is
     * not the row's id as it stands, so every such row read as an
     * identifier: `claude-opus-5-5`, `cursor-grok-4.6`, `gpt-5.5`, twenty-six
     * of Cursor's rows on a real account (2026-09-23). The shared words carry
     * no effort, because the effort is the word the variants differ by.
     */
    const shared = sharedCursorName(held.variantNames) ?? sharedCursorName([...held.variantNames, ...held.fastNames])
    /*
     * CURSOR'S OWN DEFAULT, where it says one: the variant it lists under the
     * bare name. "Claude Opus 5.5 1M" is `-medium`, "Kimi K3" is `-max`,
     * "GLM 5.2" is `-high`. That variant stands for the family, and its level
     * is the one a new route starts on -- the level Cursor itself would run,
     * where Locust's own rule (`medium`, else the middle) would have put Kimi
     * on high and Opus 4.6, whose levels are high and max, on max.
     */
    const named =
      held.plainName === undefined && shared !== undefined
        ? Object.entries(held.variants).find(
            ([effort, id]) =>
              !effort.endsWith('fast') &&
              cursorNameWords(listed.find((model) => model.id === id)?.displayName ?? '').join(' ') === shared
          )
        : undefined
    return {
      id: named?.[1] ?? held.defaultId,
      runtime: 'cursor' as const,
      displayName: held.plainName ?? shared ?? family,
      // Nothing to say beyond the name: "Listed by cursor-agent --list-models"
      // stood on every one of 49 rows. The picker says the levels instead.
      description: '',
      supportedEfforts: held.efforts,
      ...(Object.keys(held.variants).length === 0 ? {} : { variants: held.variants }),
      ...(named === undefined ? {} : { defaultEffort: named[0] })
    }
  })
}

/**
 * A Cursor route's level, put into the id it names, at the start of the run.
 *
 * The window does this too (`startRoute`), but only with the models it has
 * been told: a teammate saved as "Gemini 4 Argon, High" is `gemini-4-argon-low`
 * plus `high`, and sent before Cursor's list reached the window it went out as
 * both, and Cursor's builder refused it -- "Cursor Agent takes no effort
 * level" (2026-10-16, a teammate's first message seconds after launch). The
 * run's own discovery has the list, so the level becomes its id here. A level
 * the family does not list is left as it came, and the builder says so.
 */
export function cursorChoiceFor(
  models: readonly PublicModel[],
  model: string | undefined,
  effort: string | undefined
): { readonly model?: string; readonly effort?: string } {
  if (effort === undefined || model === undefined) return { ...(model === undefined ? {} : { model }), ...(effort === undefined ? {} : { effort }) }
  const family = models.find((entry) => entry.id === model || Object.values(entry.variants ?? {}).includes(model))
  const id = family?.variants?.[effort]
  return id === undefined ? { model, effort } : { model: id }
}

/**
 * OpenCode's models, from `opencode models` at discovery: one `provider/model`
 * id per line. The ones ending in `-free` cost nothing and need no sign-in,
 * which is the whole reason this runtime is here; they are named as free.
 */
export function opencodeModelsFrom(runtimes: readonly RuntimeDiscovery[]): readonly PublicModel[] {
  const opencode = runtimes.find((entry) => entry.id === 'opencode')
  if (openCodeUsesAcp(opencode?.version)) return []
  const listed = opencode?.modelHints?.models
  if (opencode?.readiness !== 'ready' || listed === undefined) return []
  return listed.map((model) => {
    const free = model.id.endsWith('-free')
    return {
      id: model.id,
      runtime: 'opencode' as const,
      displayName: model.displayName,
      // Said as a person would (0.495): "listed by opencode models" read as a log line (Grok, 0.489).
      description: free ? 'Free · no sign-in' : 'Through OpenCode',
      // Its variants, which `run --variant` takes (A6.5); none listed, none offered.
      supportedEfforts: [...(model.efforts ?? [])]
    }
  })
}

/**
 * Copilot has no cheap way to list models: the ones an account may use are
 * only reported inside a run. So the catalog offers `auto`, which is real --
 * Copilot picks -- and never a name this build guessed.
 */
export function copilotModelsFrom(runtimes: readonly RuntimeDiscovery[]): readonly PublicModel[] {
  const copilot = runtimes.find((entry) => entry.id === 'copilot')
  if (copilot?.readiness !== 'ready') return []
  return [
    {
      id: 'auto',
      runtime: 'copilot' as const,
      displayName: 'Auto',
      description: 'Copilot picks the model your plan allows',
      supportedEfforts: []
    }
  ]
}

/**
 * Muse Code's models (0.759). Its CLI lists none, so this offered nothing and every Muse teammate ran on whatever the
 * CLI chose. MEASURED 2026-10-12 on Muse Code 1.4.6 the one way it allows: a run on a name Meta does not serve ends
 * "does not exist or you lack access" before any model is asked, and these two reached the model -- Muse Wonder 1.0,
 * out that day and what 1.4.6 runs when no model is named, and Muse Spark 1.3. Its efforts are its own eight.
 */
export function museModelsFrom(runtimes: readonly RuntimeDiscovery[]): readonly PublicModel[] {
  const muse = runtimes.find((entry) => entry.id === 'muse')
  if (muse?.readiness !== 'ready') return []
  const model = (id: string, displayName: string, description: string): PublicModel => ({
    id,
    runtime: 'muse' as const,
    displayName,
    description,
    supportedEfforts: MUSE_EFFORTS,
    // Muse Code's own default (`muse exec --help`: "default: high").
    defaultEffort: 'high'
  })
  return [
    model('muse-wonder-1.0', 'Muse Wonder 1.0', "Meta's newest, and Muse Code's default"),
    model('muse-spark-1.3', 'Muse Spark 1.3', 'The Muse before it')
  ]
}

/**
 * Antigravity's three model tiers, exactly as its agent API names them. The
 * names are the host's, from the measured `--model=<flash_lite|flash|pro>`.
 */
export function antigravityModelsFrom(runtimes: readonly RuntimeDiscovery[]): readonly PublicModel[] {
  const antigravity = runtimes.find((entry) => entry.id === 'antigravity')
  const listed = antigravity?.modelHints?.models
  if (antigravity?.readiness !== 'ready' || listed === undefined) return []
  return listed.map((model) => ({
    id: model.id,
    runtime: 'antigravity' as const,
    displayName: model.displayName,
    // A tier from the app route says so; the CLI's models are named whole (0.540).
    description: model.description ?? (antigravity.executable?.commandName === 'agy' ? 'Through Antigravity CLI' : 'Antigravity model tier'),
    // Low / Medium / High beside the model, not as models of their own (0.540).
    supportedEfforts: model.efforts ?? []
  }))
}

/**
 * Whether a ready runtime that lists its own models at discovery listed none
 * this time (0.552). OpenCode, Cursor and Claude Code each answer from a CLI
 * probe that can fail on a busy machine while the runtime itself is fine.
 */
export function listsItsModelsButListedNone(runtimes: readonly RuntimeDiscovery[]): boolean {
  return runtimes.some((entry) =>
    (entry.id === 'opencode' || entry.id === 'cursor' || entry.id === 'claude')
    && entry.readiness === 'ready'
    && (entry.modelHints?.models === undefined || entry.modelHints.models.length === 0)
    && !(entry.id === 'claude' && (entry.modelHints?.aliases?.length ?? 0) > 0))
    // An agent installed and not answered yet: the sweep went on without it
    // (sweep-settle.ts), so a list without its models is not the list. Antigravity's
    // check that ran out of time reads the same way.
    || runtimes.some((entry) =>
      entry.availability === 'available'
      && ((entry.id === 'antigravity' && entry.readiness === 'unknown')
        || entry.diagnostics?.some((note) => note.code === 'check-pending') === true))
}

export function createModelCatalog(options: ModelCatalogOptions): ModelCatalog {
  const now = options.now ?? (() => Date.now())
  let cached: { readonly at: number; readonly response: ModelCatalogResponse } | undefined
  let inFlight: Promise<ModelCatalogResponse> | undefined
  /** A ready runtime whose own list came back empty on the last probe (0.552). */
  let missingAList = false
  /*
   * ANTIGRAVITY'S USAGE NO LONGER HOLDS THE LIST (0.709). `agy -p /usage`
   * took 7.3 s on Colin's machine on 2026-10-08, and every runtime's models
   * waited for it: OpenCode's free rows reached the picker 14 s after
   * "ready", Home's "Use a free model" was missing for that long, and drives
   * that pick a free model gave up. The list waits a moment for the reading;
   * a reading that comes later is kept here, joins every answer from then on
   * (a cached one too), and tells the window to read again.
   */
  let antigravityReading: string | undefined
  const withAntigravity = (response: ModelCatalogResponse): ModelCatalogResponse =>
    !response.ok || antigravityReading === undefined
      ? response
      : { ...response, data: { ...response.data, usageWindows: { ...response.data.usageWindows, antigravity: antigravityReading } } }

  const probe = async (): Promise<ModelCatalogResponse> => {
    const runtimes = await options.discover()
    missingAList = listsItsModelsButListedNone(runtimes)
    const opencode = runtimes.find((entry) => entry.id === 'opencode')
    // Start independently of Codex's own model read. A slow ACP handshake
    // must not put another twenty-second probe behind it.
    const opencodeAsked = openCodeUsesAcp(opencode?.version) && opencode?.readiness === 'ready'
      ? readOpenCodeAcpModels(opencode, { spawn: options.spawn, ...(options.opencodeScratch === undefined ? {} : { scratch: options.opencodeScratch }) })
      : Promise.resolve([])
    // Each runtime's models come from its own source and fail on their own:
    // Claude's and Cursor's from what their CLIs advertised at discovery,
    // Codex's from a live server read.
    const advertisedModels = [
      ...claudeModelsFrom(runtimes, options.claudeListed?.() ?? []),
      ...cursorModelsFrom(runtimes),
      ...opencodeModelsFrom(runtimes),
      ...copilotModelsFrom(runtimes),
      ...antigravityModelsFrom(runtimes),
      ...museModelsFrom(runtimes)
    ]
    const antigravity = runtimes.find((entry) => entry.id === 'antigravity')
    const readAntigravity = options.readAntigravity ?? (async (runtime: RuntimeDiscovery) =>
      runtime.executable === undefined ? undefined : antigravityUsageText(await readAntigravityUsage(runtime.executable, runtime.version)))
    let answered = false
    const usage = antigravity?.readiness === 'ready' && antigravity.executable?.commandName === 'agy'
      ? readAntigravity(antigravity).catch(() => undefined).then((reading) => {
          if (reading === undefined) return
          antigravityReading = `${reading} · as of ${new Date(now()).toISOString()}`
          if (answered) options.onLateUsage?.()
        })
      : Promise.resolve()
    // Independent of Codex: a missing server or an older method loses no other reading.
    const withUsage = async (response: ModelCatalogResponse): Promise<ModelCatalogResponse> => {
      const opencodeAcpModels = await opencodeAsked
      if (openCodeUsesAcp(opencode?.version) && opencode?.readiness === 'ready') {
        missingAList = listsItsModelsButListedNone(runtimes.filter((entry) => entry.id !== 'opencode')) || opencodeAcpModels.length === 0
      }
      const combined: ModelCatalogResponse = opencodeAcpModels.length === 0 ? response : {
        ok: true,
        data: {
          ...(response.ok ? response.data : {}),
          models: withAccountDefaults([...(response.ok ? response.data.models : []), ...opencodeAcpModels])
        }
      }
      await Promise.race([usage, new Promise<void>((resolve) => setTimeout(resolve, USAGE_WAIT_MS))])
      answered = true
      return withAntigravity(combined)
    }
    const codex = runtimes.find((entry) => entry.id === 'codex')
    if (codex?.readiness !== 'ready' || codex.executable === undefined) {
      return withUsage(advertisedModels.length > 0
        ? { ok: true, data: { models: withAccountDefaults(advertisedModels) } }
        : { ok: false, error: { code: 'MODELS_UNAVAILABLE', message: 'Codex CLI is not ready.' } })
    }

    const child = options.spawn(codex.executable.executablePath, [
      ...codex.executable.prefixArgs,
      'app-server'
    ], codex.executable.env)
    const client = createAppServerClient({
      transport: { send: (line) => child.write(line), close: () => child.kill() },
      onNotification: () => undefined,
      // Nothing should be able to ask this probe for permission: it starts no
      // turn. Refusing is the only correct answer.
      onRequest: async () => ({ decision: 'reject' }),
      requestTimeoutMs: PROBE_TIMEOUT_MS
    })
    child.onData((chunk) => client.accept(chunk))

    try {
      await client.request('initialize', { clientInfo: { name: 'locust', version: '0.1.0' } })
      client.notify('initialized')
      const result = await client.request('model/list', {})
      // The route a fresh profile starts on gets a row of its own, for every
      // runtime, so the picker has something to mark ACTIVE and the effort
      // levels have somewhere to hang whichever runtime discovery settled on.
      const models = withAccountDefaults([...parseModels(result), ...advertisedModels])
      if (models.length === 0) {
        return withUsage({ ok: false, error: { code: 'MODELS_UNAVAILABLE', message: 'No models were reported.' } })
      }
      /*
       * AND WHAT THE ACCOUNT HAS USED (0.390), in the same server session: no
       * turn spent. Colin, 2026-09-27, over his Home: "codex usage isnt
       * showing" -- a reading only ever came from a run, and his Codex runs
       * were all before 0.388 kept one. `account/rateLimits/read` returns the
       * snapshot a turn would push (its shape read from `codex app-server
       * generate-json-schema`, 0.157.1). An older Codex without the method,
       * or an account that will not say, just leaves the reading out.
       */
      let usageWindows: Record<string, string> | undefined
      try {
        const limits = await client.request('account/rateLimits/read', {})
        const snapshot = typeof limits === 'object' && limits !== null ? (limits as { readonly rateLimits?: unknown }).rateLimits : undefined
        const reading = usageWindowFromSnapshot(snapshot)
        // Read from the account itself, so it counts use outside Locust too (0.406).
        if (reading !== undefined) usageWindows = { codex: `${reading} · as of ${new Date().toISOString()}` }
      } catch {
        // No reading, and nothing else lost.
      }
      return withUsage({ ok: true, data: { models, ...(usageWindows === undefined ? {} : { usageWindows }) } })
    } catch {
      return withUsage(advertisedModels.length > 0
        ? { ok: true, data: { models: withAccountDefaults(advertisedModels) } }
        : {
            ok: false,
            error: { code: 'MODELS_UNAVAILABLE', message: 'The model list could not be read.' }
          })
    } finally {
      // Always take the server down. This probe exists to answer one question.
      client.dispose('model catalog read finished')
      child.kill()
    }
  }

  return {
    forget(): void {
      cached = undefined
    },
    read(): Promise<ModelCatalogResponse> {
      const held = cached
      if (held !== undefined && now() - held.at < CACHE_MS) return Promise.resolve(withAntigravity(held.response))
      // One probe at a time. Two windows asking at once must not start two
      // servers, and the second caller gets the first one's answer.
      if (inFlight !== undefined) return inFlight
      const running = probe()
        .then((response) => {
          // Only a success is cached: a transient failure must not pin the
          // picker to "unavailable" for the next ten minutes. Nor a success
          // MISSING a ready runtime's list (0.552): Colin, 10/02, "only
          // account default is popping up for opencode" -- a slow launch's
          // `opencode models` answered nothing, and every re-read the window
          // made for ten minutes got the same answer back from here.
          if (response.ok && !missingAList) cached = { at: now(), response }
          return response
        })
        .finally(() => {
          inFlight = undefined
        })
      inFlight = running
      return running
    }
  }
}
