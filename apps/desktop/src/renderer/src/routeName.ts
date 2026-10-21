import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import { defaultEffort, modelFamily, modelLabelFor } from './status.js'
import { effortName } from './effortLevels.js'
import { splitEffort } from './effortScale.js'
import type { PublicModel } from '../../shared/ipc.js'
import { runtimeDisplayName } from '../../shared/runtimes.js'
import { claudeRouteModelName } from '../../shared/claude-models.js'
import { antigravityTierName } from '../../shared/antigravity-models.js'

/**
 * How a route reads when it is CHROME rather than prose.
 *
 * The composer chip and the mission header name the same route the Settings
 * screen does, and they are not the same job. Settings is telling you which
 * program this is, so `Cursor Agent` and `Codex CLI` earn their second word.
 * A chip sitting under a text box is a label on a control you already chose;
 * the second word is filler there, and the model beside it was the raw id --
 * `Cursor Agent / cursor-grok-4.6`, which says "cursor" twice and then spells
 * a product name in lowercase with a hyphen in it.
 *
 * Colin, 2026-09-11: "this is way too much filler text, it should just say
 * Cursor / Grok 4.6, Claude / Fable 5.1, Codex / GPT-6 Astra".
 *
 * The full id is not thrown away -- it stays in the control's tooltip and in
 * the receipt, which are where a person goes to find the exact string to
 * type somewhere else.
 */

const SHORT_NAMES: Partial<Record<MissionRuntimeId, string>> = {
  codex: 'Codex',
  claude: 'Claude',
  cursor: 'Cursor',
  gemini: 'Gemini',
  copilot: 'Copilot'
}

/** `Cursor`, not `Cursor Agent`. Runtimes whose name is already one word keep it. */
export function shortRuntimeName(runtime: MissionRuntimeId): string {
  return SHORT_NAMES[runtime] ?? runtimeDisplayName(runtime)
}

/*
 * Written the way the makers write them, which is the only reason this is a
 * list rather than a rule: `GPT-4`, `GPT-5` keep the hyphen and `Grok 4`,
 * `Claude 3`, `Gemini 2.5` do not. Nothing here invents a version or a name
 * -- every word comes from the id -- so an id this does not recognise still
 * reads as itself, only spaced and capitalised.
 */
const ACRONYMS: ReadonlySet<string> = new Set(['gpt', 'cli', 'ai'])

const capitalised = (token: string): string =>
  ACRONYMS.has(token.toLowerCase()) ? token.toUpperCase() : token.charAt(0).toUpperCase() + token.slice(1)

const SHORT_NUMBER = /^\d{1,2}$/

/**
 * Whether `tokens[index]` is the minor half of a version written with a
 * hyphen: `claude-opus-5-5` is Opus 5.5, and `claude-3-5-sonnet` Claude 3.5.
 * Anthropic's ids -- and Cursor's copies of them -- write the point as a
 * hyphen, and a mission on Cursor's `claude-opus-5-5-medium` read "Claude Opus
 * 5 5 Medium" (2026-09-23). Exactly two short numbers after a word: a third,
 * or a longer number before them, is a date (`gpt-4o-2024-08-06`), not a
 * version.
 */
function isMinorVersion(tokens: readonly string[], index: number): boolean {
  if (index < 2 || !SHORT_NUMBER.test(tokens[index] ?? '') || !SHORT_NUMBER.test(tokens[index - 1] ?? '')) return false
  if (/^\d+$/.test(tokens[index - 2] ?? '')) return false
  return !SHORT_NUMBER.test(tokens[index + 1] ?? '')
}

function spellOut(id: string): string {
  const tokens = id.split('-')
  let out = ''
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index] ?? ''
    if (index === 0) {
      out = capitalised(token)
      continue
    }
    if (isMinorVersion(tokens, index)) {
      out += `.${token}`
      continue
    }
    // `GPT-6`, but `Grok 4.6`: the hyphen survives only where the makers keep
    // one, which is between an acronym and the number that versions it.
    const previous = tokens[index - 1] ?? ''
    const keepsHyphen = ACRONYMS.has(previous.toLowerCase()) && /^[0-9]/.test(token)
    out += (keepsHyphen ? '-' : ' ') + capitalised(token)
  }
  return out
}

/**
 * `cursor-grok-4.6` beside Cursor reads `Grok 4.6`.
 *
 * Three things happen, in order, and each is a repetition being removed
 * rather than a name being invented:
 *
 *   1. A provider that merely repeats the runtime goes (`modelLabelFor`, for
 *      OpenCode's `opencode/ling-3.0-...`). A provider that does NOT repeat
 *      it is real information about whose model this is, and stays.
 *   2. A runtime that repeats itself INSIDE the id goes the same way --
 *      Cursor names every model `cursor-<model>` and the chip already says
 *      Cursor.
 *   3. What is left is spelled as a name rather than an identifier.
 */
/**
 * The names of the person's own models, by route id (0.357).
 *
 * A model of your own runs as `own-1a2b3c4d/acme-70b` -- the provider id
 * OpenCode is given -- and every surface names a route from its id alone,
 * which would spell that as "Own 1a2b3c4d/Acme 70b". The catalog is where
 * the name the person typed lives; the window hands it here each time it
 * reads the catalog, so the chip, the picker and a mission row all say
 * "Acme Chat". A route whose model has since been removed says its model
 * id, spelled, and never the internal half.
 */
const ownModelNames = new Map<string, string>()
/** An agent the person added (0.777), by the route's model: its name, and the model picked on it. */
const acpRouteNames = new Map<string, string>()
const OWN_ROUTE_MODEL = /^own-[a-f0-9]{8}\/(.+)$/
/** An id that spells out as a name: words and versions joined by hyphens, no provider, no spaces. */
const SPELLABLE_ID = /^[A-Za-z0-9.]+(?:-[A-Za-z0-9.]+)+$/

/**
 * A NAME THAT IS ONLY AN ID (0.810). A model added with its own id as its name -- "claude-haiku-5-5", as the API
 * benchmark of 10/21 added three -- read as the id on every row of Compare and the benchmark (Colin's screenshot).
 * Like a catalog's id (rememberOwnModels), it is an identifier, not a spelling: it reads "Claude Haiku 5.5". Any
 * other name stands as the person wrote it.
 */
export function shownRouteLabel(route: { readonly model: string; readonly label?: string }): string | undefined {
  const label = route.label?.trim()
  if (label === undefined || label.length === 0) return undefined
  const bare = OWN_ROUTE_MODEL.exec(route.model)?.[1] ?? route.model
  return label === bare && SPELLABLE_ID.test(label) ? spellOut(label) : label
}

/**
 * THE RUNTIME'S OWN SPELLING (0.528). The picker lists a model by the name its
 * runtime gave it -- Codex's "GPT-6-Luna" -- and the chip spelled the id,
 * "GPT-6 Luna": one model, two names, side by side in Compare's columns. A
 * catalog name that is the SAME name as the spelled id (differing only in
 * case, hyphens and spaces) is the maker's styling and is used; a different
 * name -- an alias's version, a description -- never replaces the spelling.
 */
const catalogSpellings = new Map<string, string>()
/**
 * What each Claude alias means today, by the name the catalog gives it (0.697):
 * Claude Code's own list once its handshake has answered, so `haiku` reads
 * "Haiku 5.5" on every chip the day Claude Code knows it.
 */
const claudeAliasNames = new Map<string, string>()
const sameName = (a: string): string => a.toLowerCase().replace(/[\s-]+/g, ' ').trim()

export function rememberOwnModels(models: readonly { readonly id: string; readonly displayName: string; readonly own?: true; readonly runtime?: string; readonly older?: boolean; readonly agentName?: string }[]): void {
  ownModelNames.clear()
  acpRouteNames.clear()
  for (const model of models) {
    if (model.runtime !== 'acp' || model.agentName === undefined) continue
    acpRouteNames.set(model.id, model.id.includes('::') ? `${model.agentName} · ${model.displayName}` : model.agentName)
  }
  catalogSpellings.clear()
  claudeAliasNames.clear()
  for (const model of models) {
    if (model.runtime === 'claude' && model.older !== true && !model.id.startsWith('claude-') && model.id !== 'account-default') claudeAliasNames.set(model.id, model.displayName)
    if (model.own === true) ownModelNames.set(model.id, shownRouteLabel({ model: model.id, label: model.displayName }) ?? model.displayName)
    // A catalog "name" that is the id, or the id without its provider (OpenCode's
    // `nemotron-3-ultra-free`), is an identifier, not a spelling: never taken.
    // Nor is `account-default`, Locust's own word for "send no model": its catalog label
    // ("Account default") would have re-cased every list row that says "Account Default".
    else if (model.runtime !== undefined && model.id !== 'account-default' && model.displayName !== model.id && !model.id.endsWith(`/${model.displayName}`)) catalogSpellings.set(`${model.runtime}:${model.id}`, model.displayName)
  }
}

/** A route on one of the person's own models: `own-<8 hex>/<model>`. */
export function isOwnRoute(modelId: string): boolean {
  return OWN_ROUTE_MODEL.test(modelId)
}

/** Locust's own word for "send no model; the account picks" (status.ts ACCOUNT_DEFAULT_MODEL). */
const ACCOUNT_DEFAULT_ID = 'account-default'

/**
 * A route as chrome, "Runtime / Model" -- and a model of the person's own by
 * its own name alone (0.361).
 *
 * Every model of your own runs through OpenCode, so the chip read "OpenCode
 * / Acme Chat": a company that brought its own model met another product's
 * name in front of it, on every surface that names a route. OpenCode is
 * still what runs it, and it stays where exact strings live -- the chip's
 * tooltip, Details, the receipt -- but on chrome the name the person gave
 * the model is the whole name.
 *
 * `model` is the name the surface already spells (a route's, or a past
 * mission's); only whether the runtime's word goes in front is decided here.
 */
export function routeChrome(runtime: MissionRuntimeId, modelId: string, model: string, separator = ' / '): string {
  if (isOwnRoute(modelId)) return model
  // The account's own default is the runtime's name alone: "Codex", not "Codex / Account Default" on every row
  // (declutter, 0.723). What follows the model's name -- a level, " · Medium" -- stays.
  if (modelId === ACCOUNT_DEFAULT_ID) {
    const rest = model.replace(/^(?:Account Default|default)(?: · )?/i, '')
    return rest.length === 0 ? shortRuntimeName(runtime) : `${shortRuntimeName(runtime)}${separator}${rest}`
  }
  return `${shortRuntimeName(runtime)}${separator}${model}`
}

export function modelDisplayName(runtime: string, modelId: string): string {
  // An agent the person added (0.777): its own name, then the model picked on it -- never `acp_goose::...`.
  if (runtime === 'acp') {
    const known = acpRouteNames.get(modelId)
    if (known !== undefined) return known
    const parsed = /^acp_([a-z0-9-]+)(?:::(.+))?$/.exec(modelId)
    if (parsed !== null) return parsed[2] === undefined ? spellOut(parsed[1]!) : `${spellOut(parsed[1]!)} · ${modelDisplayName('opencode', parsed[2])}`
  }
  const own = OWN_ROUTE_MODEL.exec(modelId)
  if (own !== null) return ownModelNames.get(modelId) ?? spellOut(own[1]!)
  // `auto` is the runtime choosing its own model (Copilot's, Cursor's). Said
  // bare, beside a mode, it read as the Auto MODE -- "Auto · running" over an
  // Approve-each turn (drive-copilot-approve-each, 0.377). It says it is one.
  if (modelId.trim().toLowerCase() === 'auto') return 'Auto model'
  const named = modelLabelFor(runtime, modelId)
  const slash = named.indexOf('/')
  if (slash > 0) {
    return `${spellOut(named.slice(0, slash))}/${spellOut(named.slice(slash + 1))}`
  }
  const prefix = `${runtime.toLowerCase()}-`
  const bare = named.toLowerCase().startsWith(prefix) ? named.slice(prefix.length) : named
  // An id that is ONLY the runtime's name has nothing left to show; the id
  // itself is then the honest label.
  const spelled = bare.length === 0 ? spellOut(named) : spellOut(bare)
  const theirs = catalogSpellings.get(`${runtime}:${modelId}`)
  return theirs !== undefined && sameName(theirs) === sameName(spelled) ? theirs : spelled
}

/**
 * The model a ROUTE will run, for the controls that pick or show one: the
 * composer's chip, the picker, a teammate's route.
 *
 * For Claude Code that is the version the alias means -- `Claude / Opus 5.5`,
 * the chip Colin wrote out on 2026-09-11 (*"Claude / Fable 5.1"*) -- taken
 * from what a finished run on that route reported (`earned`) and otherwise
 * from Claude Code's own alias table. A past mission's row keeps
 * `modelDisplayName`: `opus` in August meant whatever it meant then, and
 * today's version would be a claim about a run nobody measured.
 */
export function routeModelName(runtime: string, modelId: string, earned?: string): string {
  if (runtime === 'claude') {
    const named = claudeRouteModelName(modelId, earned, claudeAliasNames.get(modelId))
    if (named !== undefined) return named
  }
  // An Antigravity tier by the model it runs -- "Gemini 3.8 Flash", not the
  // tier's bare word (0.384; see shared/antigravity-models.ts).
  if (runtime === 'antigravity') {
    const named = antigravityTierName(modelId)
    if (named !== undefined) return named
  }
  return modelDisplayName(runtime, modelId)
}

/**
 * A model name with its trailing "Free" split off, so the route chip can draw
 * it as a tag that is never cut. The chip truncates names at 18 characters,
 * and "Muse Spark 1.3 Contributor Free" lost exactly the word that says the
 * route costs nothing (Yurt's beta report, 2026-09-23, #12).
 */
export function freeTagOf(label: string): { readonly name: string; readonly free: boolean } {
  return / free$/i.test(label) && label.length > 5 ? { name: label.slice(0, -5), free: true } : { name: label, free: false }
}

/**
 * A route's model by name, a Cursor family by the family: "Gemini 4 Argon", not
 * "Gemini 4 Argon Low" -- the id it stands on names one level, and the level is
 * its own control. A compare column named the id's level beside an effort set
 * to High, and went out labelled "Gemini 4 Argon Low · High" (2026-10-16).
 */
export function routeFamilyName(
  route: { readonly runtime: string; readonly model: string },
  resolved: ReadonlyMap<string, string>,
  models: readonly PublicModel[]
): string {
  const family = modelFamily(models, route.runtime, route.model)
  return routeModelName(route.runtime, family?.variants !== undefined ? family.displayName : route.model, resolved.get(`${route.runtime}:${route.model}`))
}

/**
 * The effort a route runs at, found the way the composer's chip finds it
 * (0.386): the level chosen for it; else the level a variant id carries --
 * Cursor's `grok-4.7-medium` IS medium; else the model's default. Only for a
 * model the catalog says has levels, and only once the catalog has answered:
 * before that, nothing is said rather than a level guessed.
 */
export function routeEffortOf(
  route: { readonly runtime: string; readonly model: string; readonly effort?: string },
  models: readonly PublicModel[]
): string | undefined {
  const family = modelFamily(models, route.runtime, route.model)
  if (family === undefined || family.supportedEfforts.length === 0) return undefined
  const ofId = Object.entries(family.variants ?? {}).find(([, id]) => id === route.model)?.[0]
  return route.effort ?? ofId ?? defaultEffort(family.supportedEfforts, family.defaultEffort)
}

/**
 * A teammate's route on Home's card (0.384): the model by the name it RUNS as
 * -- "Opus 5.5", "Gemini 3.8 Flash", as the Team screen and the chip name it
 * (routeModelName) -- where Home had spelled the alias, "Claude · Opus"
 * (Colin's frame, 2026-09-26). The whole route, runtime and all, stays for the
 * card's hover and its accessible name; the line itself shows the runtime's
 * mark and the model.
 *
 * THE LEVEL IS THE HOVER, NOT THE LINE (0.386). Colin, 2026-09-27, over his
 * Home: "some teammates on home page not showing model effort" -- Robin's
 * card read "Grok 4.7 Medium" only because Cursor writes the level into the
 * model's id, and the two on Opus said nothing of theirs. Then: "its honestly
 * up to you if effort level is even worth showing on the home page ...
 * whatever is best for user and design". So every card says the same kind of
 * thing: the model, by name -- a Cursor family by the family, "Grok 4.7", as
 * the chip names it -- and the level it runs at is in the hover and the
 * accessible name, "Cursor · Grok 4.7 · Medium". Home is who is on the team;
 * how hard one thinks is a knob, set and shown beside the message box.
 */
export function homeRouteOf(
  route: { readonly runtime: MissionRuntimeId; readonly model: string; readonly effort?: string },
  resolved: ReadonlyMap<string, string>,
  models: readonly PublicModel[] = []
): { readonly route: string; readonly model: string; readonly runtime?: MissionRuntimeId } {
  const name = routeFamilyName(route, resolved, models)
  const effort = routeEffortOf(route, models)
  const level = effort === undefined ? undefined : splitEffort(effort)
  const withLevel = level === undefined ? name : `${name} · ${effortName(level.base)}${level.fast ? ' · Fast' : ''}`
  return { route: routeChrome(route.runtime, route.model, withLevel, ' · '), model: name, ...(isOwnRoute(route.model) ? {} : { runtime: route.runtime }) }
}
