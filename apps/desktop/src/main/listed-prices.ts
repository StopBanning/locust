import { readFile, stat } from 'node:fs/promises'
import { join } from 'node:path'

import type { OpenCodePrice } from '@teammate/runtime-adapters'

/**
 * A MODEL OF YOUR OWN, PRICED AS ITS PROVIDER LISTS IT (0.810).
 *
 * Colin's benchmark of 10/21 put Claude Haiku, Sonnet and Opus side by side
 * through his own API key, and "Cost a task" read tokens for each -- under a
 * heading that promised a price, for models whose tokens are listed at
 * forty times more on Opus than on Haiku. OpenCode prices the models it knows from
 * models.dev's list, which it keeps on this computer and refreshes as it
 * runs; a model of your own at that provider's own address, under the id
 * that provider lists, is the same model at the same price. So OpenCode is
 * told that price, and reports dollars for it as it does for its own.
 *
 * Only an exact match is priced: the provider whose own address the model is
 * served from, and the id it lists there. A gateway, a reseller's copy or a
 * server of your own is not -- what it charges is not on the list -- and two
 * providers at one address that price a model differently (a plan beside a
 * pay-as-you-go account) price it not at all.
 */

/**
 * Providers OpenCode reaches through an SDK of their own, so the list gives
 * no address for them: each one's OpenAI-compatible address, by host.
 */
const OWN_ADDRESS: Readonly<Record<string, string>> = {
  'api.anthropic.com': 'anthropic',
  'api.openai.com': 'openai',
  'generativelanguage.googleapis.com': 'google',
  'api.x.ai': 'xai',
  'api.mistral.ai': 'mistral',
  'api.groq.com': 'groq',
  'api.together.xyz': 'togetherai',
  'api.cerebras.ai': 'cerebras'
}

/** Where OpenCode keeps the list: its cache folder, which it finds as XDG says. */
export function openCodePriceListPath(env: NodeJS.ProcessEnv, home: string): string {
  const cache = env.XDG_CACHE_HOME !== undefined && env.XDG_CACHE_HOME.length > 0 ? env.XDG_CACHE_HOME : join(home, '.cache')
  return join(cache, 'opencode', 'models.json')
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const isPrice = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value) && value >= 0

/** A price in the shape OpenCode's config takes, or undefined for one that is not a whole price. */
function priceOf(value: unknown): OpenCodePrice | undefined {
  if (!isRecord(value) || !isPrice(value.input) || !isPrice(value.output)) return undefined
  const parts = (from: Record<string, unknown>): { readonly cache_read?: number; readonly cache_write?: number } => ({
    ...(isPrice(from.cache_read) ? { cache_read: from.cache_read } : {}),
    ...(isPrice(from.cache_write) ? { cache_write: from.cache_write } : {})
  })
  const tiers = Array.isArray(value.tiers)
    ? value.tiers.flatMap((tier: unknown) =>
        isRecord(tier) && isPrice(tier.input) && isPrice(tier.output) && isRecord(tier.tier) && tier.tier.type === 'context' && isPrice(tier.tier.size)
          ? [{ input: tier.input, output: tier.output, ...parts(tier), tier: { type: 'context' as const, size: tier.tier.size } }]
          : []
      )
    : []
  const over = value.context_over_200k
  return {
    input: value.input,
    output: value.output,
    ...parts(value),
    ...(tiers.length === 0 ? {} : { tiers }),
    ...(isRecord(over) && isPrice(over.input) && isPrice(over.output) ? { context_over_200k: { input: over.input, output: over.output, ...parts(over) } } : {})
  }
}

/** An address as it is compared: host in lower case, path without its last slash. */
function addressOf(value: string): { readonly host: string; readonly whole: string } | undefined {
  try {
    const url = new URL(value)
    const host = url.host.toLowerCase()
    return { host, whole: `${host}${url.pathname.replace(/\/+$/, '')}` }
  } catch {
    return undefined
  }
}

/**
 * The listed price of `model` at `baseUrl`, from OpenCode's copy of the list;
 * undefined unless exactly one price is listed for it there. A provider whose
 * own address is `baseUrl` itself is asked first, so a plan and an account
 * that share a host are told apart by their paths.
 */
export function listedPrice(list: unknown, baseUrl: string, model: string): OpenCodePrice | undefined {
  const asked = addressOf(baseUrl)
  if (asked === undefined || !isRecord(list)) return undefined
  const exact: string[] = []
  const sameHost: string[] = []
  for (const [id, provider] of Object.entries(list)) {
    if (!isRecord(provider)) continue
    const listed = typeof provider.api === 'string' ? addressOf(provider.api) : undefined
    if (listed?.whole === asked.whole) exact.push(id)
    else if (listed?.host === asked.host || OWN_ADDRESS[asked.host] === id) sameHost.push(id)
  }
  const pricesAt = (ids: readonly string[]): readonly OpenCodePrice[] =>
    ids.flatMap((id) => {
      const models = (list[id] as Record<string, unknown>).models
      const price = isRecord(models) && isRecord(models[model]) ? priceOf(models[model].cost) : undefined
      return price === undefined ? [] : [price]
    })
  const found = pricesAt(exact).length > 0 ? pricesAt(exact) : pricesAt(sameHost)
  const first = found[0]
  if (first === undefined) return undefined
  return found.every((price) => JSON.stringify(price) === JSON.stringify(first)) ? first : undefined
}

export interface PriceList {
  /** The listed price of a model at an address; undefined when it is not listed, or the list cannot be read. */
  priceAt(baseUrl: string, model: string): Promise<OpenCodePrice | undefined>
}

/**
 * OpenCode's list, read when a run of your own model starts and kept while
 * the file is unchanged: it is some megabytes, and OpenCode rewrites it
 * when it refreshes. No list -- OpenCode never run, or run offline -- is no
 * price, and the run reports tokens as it did before.
 */
export function createPriceList(options: {
  readonly path: string
  readonly readText?: (path: string) => Promise<string>
  readonly modifiedAt?: (path: string) => Promise<number>
}): PriceList {
  const readText = options.readText ?? ((path) => readFile(path, 'utf8'))
  const modifiedAt = options.modifiedAt ?? (async (path) => (await stat(path)).mtimeMs)
  let held: { readonly at: number; readonly list: unknown } | undefined
  return {
    async priceAt(baseUrl, model) {
      try {
        const at = await modifiedAt(options.path)
        if (held?.at !== at) held = { at, list: JSON.parse(await readText(options.path)) as unknown }
        return listedPrice(held.list, baseUrl, model)
      } catch {
        return undefined
      }
    }
  }
}
