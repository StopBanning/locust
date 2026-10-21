import { randomBytes } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { OpenCodeProvider } from '@teammate/runtime-adapters'

import type { PublicModel, PublicOwnModel } from '../shared/ipc.js'
import { replaceFile } from './replace-file.js'

/**
 * YOUR OWN MODELS (0.357).
 *
 * Colin, 2026-09-26: a company with a model of its own -- his father's is
 * building one -- should be able to "just insert their model". Any endpoint
 * that speaks the OpenAI chat API (a company gateway, vLLM, Ollama's /v1,
 * LM Studio) runs through Locust's OpenCode path as a provider of its own
 * (withOpenCodeProviders). This keeps the list, and the keys.
 *
 * A KEY IS KEPT BY THE OPERATING SYSTEM, never in the clear: encrypted with
 * Electron's safeStorage (DPAPI on Windows, the keychain on macOS) before it
 * touches the disk, decrypted only to start a run, and never sent to the
 * window -- the window is told only that one is kept. A machine that cannot
 * encrypt keeps no key, and says so, rather than writing it in the clear.
 */

/** What the operating system's own protection is asked to do with a key. */
export interface SecretBox {
  available(): boolean
  encrypt(text: string): Buffer
  decrypt(cipher: Buffer): string
}

interface StoredOwnModel {
  readonly ownId: string
  readonly name: string
  readonly baseUrl: string
  readonly model: string
  readonly createdAt: string
  /** The key as the operating system encrypted it, base64. */
  readonly key?: string
  /** A model that only chats: its runs get no tools (0.358). Absent: it takes tools. */
  readonly chatOnly?: true
}

interface StoredFile {
  readonly schemaVersion: 1
  readonly models: readonly StoredOwnModel[]
}

export const MAX_OWN_MODELS = 20
const OWN_PREFIX = 'own-'

export class OwnModelRefusal extends Error {}

/** The address as it is kept: http(s), no trailing slash, no spaces. */
export function ownModelAddress(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim().replace(/\/+$/, '')
  if (trimmed.length === 0 || trimmed.length > 500 || /\s/.test(trimmed)) return undefined
  try {
    const url = new URL(trimmed)
    return url.protocol === 'http:' || url.protocol === 'https:' ? trimmed : undefined
  } catch {
    return undefined
  }
}

/** A model id as endpoints name them: `acme-70b`, `llama3:8b`, `meta-llama/Llama-3.1-8B`. */
export function ownModelId(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed.length > 0 && trimmed.length <= 200 && /^[\w.:/@+-]+$/.test(trimmed) ? trimmed : undefined
}

function ownName(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed.length > 0 && trimmed.length <= 60 && !/[\u0000-\u001f\u007f]/.test(trimmed) ? trimmed : undefined
}

/** The model id a route carries for one of these: `own-<id>/<model>`. */
export function ownRouteModel(model: { readonly ownId: string; readonly model: string }): string {
  return `${OWN_PREFIX}${model.ownId}/${model.model}`
}

export function isOwnRouteModel(model: string | undefined): boolean {
  return model !== undefined && model.startsWith(OWN_PREFIX)
}

function hostOf(baseUrl: string): string {
  try {
    return new URL(baseUrl).host
  } catch {
    return baseUrl
  }
}

function publicOf(model: StoredOwnModel): PublicOwnModel {
  return {
    ownId: model.ownId,
    name: model.name,
    baseUrl: model.baseUrl,
    model: model.model,
    hasKey: model.key !== undefined,
    chatOnly: model.chatOnly === true,
    createdAt: model.createdAt
  }
}

function parsedStored(value: unknown): StoredOwnModel | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  const name = ownName(record.name)
  const baseUrl = ownModelAddress(record.baseUrl)
  const model = ownModelId(record.model)
  if (typeof record.ownId !== 'string' || !/^[a-f0-9]{8}$/.test(record.ownId) || name === undefined || baseUrl === undefined || model === undefined) return undefined
  if (typeof record.createdAt !== 'string' || Number.isNaN(Date.parse(record.createdAt))) return undefined
  return {
    ownId: record.ownId,
    name,
    baseUrl,
    model,
    createdAt: record.createdAt,
    ...(typeof record.key === 'string' && record.key.length > 0 ? { key: record.key } : {}),
    ...(record.chatOnly === true ? { chatOnly: true as const } : {})
  }
}

export interface OwnModelStore {
  list(): Promise<readonly PublicOwnModel[]>
  add(input: { readonly name: unknown; readonly baseUrl: unknown; readonly model: unknown; readonly key?: unknown; readonly chatOnly?: unknown }): Promise<PublicOwnModel>
  /** Whether a kept model only chats (0.358): what a failed first run, or a Test, finds out after it is added. */
  setChatOnly(ownId: unknown, chatOnly: unknown): Promise<PublicOwnModel>
  remove(ownId: unknown): Promise<void>
  /** The provider a run on this route model needs, key decrypted; undefined for a model no longer kept. */
  providerFor(routeModel: string): Promise<{ readonly id: string; readonly provider: OpenCodeProvider } | undefined>
  /** The key of a kept model, for a connection test. */
  keyOf(ownId: string): Promise<string | undefined>
  /** Each kept model as the picker lists it, under OpenCode. */
  catalog(): Promise<readonly PublicModel[]>
}

export function createOwnModelStore(options: {
  readonly rootDirectory: string
  readonly secrets: SecretBox
  readonly now?: () => Date
  readonly createId?: () => string
}): OwnModelStore {
  const path = join(options.rootDirectory, 'own-models.json')
  const now = options.now ?? (() => new Date())
  const createId = options.createId ?? (() => randomBytes(4).toString('hex'))
  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(work: () => Promise<T>): Promise<T> => {
    const next = queue.then(work, work)
    queue = next.catch(() => undefined)
    return next
  }

  const read = async (): Promise<StoredFile> => {
    let text: string
    try {
      text = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { schemaVersion: 1, models: [] }
      throw error
    }
    const parsed = JSON.parse(text) as { readonly schemaVersion?: unknown; readonly models?: unknown }
    if (parsed.schemaVersion !== 1 || !Array.isArray(parsed.models)) throw new Error('Your own models could not be read.')
    return { schemaVersion: 1, models: parsed.models.flatMap((entry) => parsedStored(entry) ?? []) }
  }
  const write = async (file: StoredFile): Promise<void> => {
    await mkdir(options.rootDirectory, { recursive: true })
    const temporary = `${path}.${String(process.pid)}.tmp`
    await writeFile(temporary, JSON.stringify(file), 'utf8')
    await replaceFile(temporary, path)
  }
  const decryptKey = (model: StoredOwnModel): string | undefined => {
    if (model.key === undefined) return undefined
    return options.secrets.decrypt(Buffer.from(model.key, 'base64'))
  }

  return {
    list: () => serialize(async () => (await read()).models.map(publicOf)),

    add: (input) =>
      serialize(async () => {
        const name = ownName(input.name)
        if (name === undefined) throw new OwnModelRefusal('Give it a name of up to 60 characters.')
        const baseUrl = ownModelAddress(input.baseUrl)
        if (baseUrl === undefined) throw new OwnModelRefusal('The address must be an http:// or https:// address, like https://llm.example.com/v1.')
        const model = ownModelId(input.model)
        if (model === undefined) throw new OwnModelRefusal('The model is the id the endpoint knows it by, like acme-70b or llama3:8b.')
        const key = typeof input.key === 'string' && input.key.trim().length > 0 ? input.key.trim() : undefined
        if (key !== undefined && key.length > 4000) throw new OwnModelRefusal('That key is longer than any key an endpoint issues.')
        if (key !== undefined && !options.secrets.available()) {
          throw new OwnModelRefusal('This machine cannot protect a key, so it was not kept. Add the model without one, or on a machine that can.')
        }
        const file = await read()
        if (file.models.length >= MAX_OWN_MODELS) throw new OwnModelRefusal(`Up to ${String(MAX_OWN_MODELS)} models of your own can be kept.`)
        if (file.models.some((kept) => kept.name.toLowerCase() === name.toLowerCase())) throw new OwnModelRefusal(`You already have a model called ${name}.`)
        const stored: StoredOwnModel = {
          ownId: createId(),
          name,
          baseUrl,
          model,
          createdAt: now().toISOString(),
          ...(key === undefined ? {} : { key: options.secrets.encrypt(key).toString('base64') }),
          ...(input.chatOnly === true ? { chatOnly: true as const } : {})
        }
        await write({ schemaVersion: 1, models: [...file.models, stored] })
        return publicOf(stored)
      }),

    setChatOnly: (ownId, chatOnly) =>
      serialize(async () => {
        if (typeof ownId !== 'string' || typeof chatOnly !== 'boolean') throw new OwnModelRefusal('That model could not be changed.')
        const file = await read()
        const kept = file.models.find((model) => model.ownId === ownId)
        if (kept === undefined) throw new OwnModelRefusal('That model is no longer kept.')
        const { chatOnly: _was, ...rest } = kept
        const changed: StoredOwnModel = chatOnly ? { ...rest, chatOnly: true } : rest
        await write({ schemaVersion: 1, models: file.models.map((model) => (model.ownId === ownId ? changed : model)) })
        return publicOf(changed)
      }),

    remove: (ownId) =>
      serialize(async () => {
        if (typeof ownId !== 'string') return
        const file = await read()
        await write({ schemaVersion: 1, models: file.models.filter((model) => model.ownId !== ownId) })
      }),

    providerFor: (routeModel) =>
      serialize(async () => {
        if (!isOwnRouteModel(routeModel)) return undefined
        const slash = routeModel.indexOf('/')
        const ownId = routeModel.slice(OWN_PREFIX.length, slash < 0 ? undefined : slash)
        const kept = (await read()).models.find((model) => model.ownId === ownId)
        if (kept === undefined) return undefined
        const apiKey = decryptKey(kept)
        return {
          id: `${OWN_PREFIX}${kept.ownId}`,
          provider: {
            name: kept.name,
            baseUrl: kept.baseUrl,
            models: [kept.model],
            ...(apiKey === undefined ? {} : { apiKey }),
            ...(kept.chatOnly === true ? { toolCalls: false } : {})
          }
        }
      }),

    keyOf: (ownId) =>
      serialize(async () => {
        const kept = (await read()).models.find((model) => model.ownId === ownId)
        return kept === undefined ? undefined : decryptKey(kept)
      }),

    catalog: () =>
      serialize(async () =>
        (await read()).models.map(
          (model): PublicModel => ({
            id: ownRouteModel(model),
            runtime: 'opencode',
            displayName: model.name,
            description: `Your model · ${hostOf(model.baseUrl)}${model.chatOnly === true ? ' · chat only' : ''}`,
            supportedEfforts: [],
            own: true,
            ...(model.chatOnly === true ? { chatOnly: true as const } : {})
          })
        )
      )
  }
}

/**
 * Whether an endpoint answers, asked the way every OpenAI-compatible server
 * lists what it serves: GET <address>/models. Said in words for the person
 * at the form -- including whether the model they typed is among the ones
 * it lists, which is the mistake a connection test exists to catch.
 */
export async function testOwnEndpoint(
  input: { readonly baseUrl: string; readonly model: string; readonly key?: string },
  fetcher: typeof fetch = fetch,
  timeoutMs = 8000,
  /*
   * THE MODEL ITSELF GETS A MINUTE (0.640). Listing is instant; answering is
   * not. A model on the person's own machine loads on its first request, and
   * a CPU reads even a short prompt slowly: a real llama.cpp server took 8 s
   * for this one-word request, past the old shared 8 s, and Test said nothing
   * about tools at all.
   */
  chatTimeoutMs = 60_000
): Promise<{ readonly ok: boolean; readonly said: string; readonly tools?: boolean }> {
  const listed = await listedModels(input, fetcher, timeoutMs)
  if (!listed.ok) return listed
  const tools = await takesTools(input, fetcher, chatTimeoutMs)
  if (tools === false) return { ok: true, said: `${listed.said} It cannot use tools, so it is set to chat only.`, tools: false }
  if (tools === true) return { ok: true, said: `${listed.said} It can use tools.`, tools: true }
  // Said, not left out: a Test that could not tell must say so.
  return { ok: false, said: `${listed.said} ${tools.said}, so Test could not tell whether it can use tools.` }
}

async function listedModels(
  input: { readonly baseUrl: string; readonly model: string; readonly key?: string },
  fetcher: typeof fetch,
  timeoutMs: number
): Promise<{ readonly ok: boolean; readonly said: string }> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetcher(`${input.baseUrl}/models`, {
      // Anthropic's OpenAI-compatible endpoint lists its models only with a version header ("anthropic-version: header
      // is required", 400, measured 2026-10-07); its chat route needs none. Other servers ignore a header they don't know.
      headers: { 'anthropic-version': '2023-06-01', ...(input.key === undefined ? {} : { authorization: `Bearer ${input.key}` }) },
      signal: controller.signal
    })
    if (response.status === 401 || response.status === 403) return { ok: false, said: 'It answered, and refused the key.' }
    if (!response.ok) return { ok: false, said: `It answered ${String(response.status)} ${response.statusText}.`.replace(/\s+\./, '.') }
    const body = (await response.json().catch(() => undefined)) as { readonly data?: readonly { readonly id?: unknown }[] } | undefined
    const ids = (body?.data ?? []).flatMap((entry) => (typeof entry.id === 'string' ? [entry.id] : []))
    if (ids.length === 0) return { ok: true, said: 'It answered. It did not list its models, so the model name could not be checked.' }
    return ids.includes(input.model)
      ? { ok: true, said: `It answered, and serves ${input.model}.` }
      : { ok: false, said: `It answered, but does not list ${input.model}. It lists ${ids.slice(0, 5).join(', ')}${ids.length > 5 ? ', …' : ''}.` }
  } catch (error) {
    return {
      ok: false,
      said: controller.signal.aborted ? `No answer in ${String(Math.round(timeoutMs / 1000))} seconds.` : `It could not be reached: ${error instanceof Error ? error.message : String(error)}.`
    }
  } finally {
    clearTimeout(timer)
  }
}

/**
 * Whether the model takes tools, asked the one way that settles it: a
 * request carrying one tool, answer capped at a single token. A model or
 * server without tool support refuses it outright -- 400 "does not support
 * tools" is Ollama's wording; vLLM's names `--enable-auto-tool-choice` -- and
 * a teammate's run would end the same way. When the answer says neither,
 * what happened instead, in words.
 */
async function takesTools(
  input: { readonly baseUrl: string; readonly model: string; readonly key?: string },
  fetcher: typeof fetch,
  timeoutMs: number
): Promise<boolean | { readonly said: string }> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetcher(`${input.baseUrl}/chat/completions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...(input.key === undefined ? {} : { authorization: `Bearer ${input.key}` }) },
      body: JSON.stringify({
        model: input.model,
        messages: [{ role: 'user', content: 'Reply with OK.' }],
        max_tokens: 1,
        tools: [{ type: 'function', function: { name: 'noop', description: 'Does nothing.', parameters: { type: 'object', properties: {} } } }]
      }),
      signal: controller.signal
    })
    if (response.ok) return true
    const text = await response.text().catch(() => '')
    if ((response.status === 400 || response.status === 422 || response.status === 501) && /tool/i.test(text)) return false
    return { said: `A one-word request got ${`${String(response.status)} ${response.statusText}`.trim()} back` }
  } catch (error) {
    return {
      said: controller.signal.aborted
        ? `It did not answer a one-word request in ${String(Math.round(timeoutMs / 1000))} second${Math.round(timeoutMs / 1000) === 1 ? '' : 's'}`
        : `A one-word request to it failed (${error instanceof Error ? error.message : String(error)})`
    }
  } finally {
    clearTimeout(timer)
  }
}
