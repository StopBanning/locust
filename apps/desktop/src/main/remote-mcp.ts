import { createHash, randomBytes, timingSafeEqual } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'

import type { RemoteMcpApp, RemoteMcpAsking, RemoteMcpState } from '../shared/remote-mcp.js'
import { mcpText, type McpToolResult } from './locust-mcp-host.js'
import { replaceFile } from './replace-file.js'

/**
 * OTHER APPS, FROM ANYWHERE (0.841). Claude on the web, Claude Duet, and any app that takes a remote MCP server, use
 * the person's Locust team at an address on Locust's relay (apps/relay/src/mcp.ts). The relay passes each request
 * here; this computer decides every one of them:
 *
 *   - An app signs in the way MCP asks (OAuth 2.1 with PKCE). Its sign-in becomes a card in Locust's window naming it,
 *     where it sends the person back to, and six digits the sign-in page shows too. Only Allow there makes a code.
 *   - The code, and the keys the app trades it for, are made here and kept here as hashes. The relay passes keys on
 *     and never holds one: it cannot let an app in. A key lasts a day; the app renews it with its refresh key, which
 *     lasts ninety days of not being used, and is replaced each time.
 *   - The tools are exactly what an app on this computer gets (locust-mcp-tools.ts, resources/locust-mcp-bridge.mjs):
 *     Ask mode unless the person turned on each teammate's own mode, and every approval card answered only in Locust.
 *   - Remove takes one app's keys away at once. Turning this off takes every app's.
 */

export const REMOTE_MCP_FILE = 'remote-mcp.json'
/** An access key's life: renewed by the app with its refresh key, while this computer is on. */
export const ACCESS_MS = 24 * 60 * 60_000
export const REFRESH_MS = 90 * 24 * 60 * 60_000
/** How long the person has to answer the card, and a code to be traded once allowed. */
export const ASKING_MS = 5 * 60_000
export const CODE_MS = 5 * 60_000
/** Apps connected at once: the one used longest ago goes, past this. */
export const MAX_APPS = 10
/** The longest answer sent back through the relay; a tool's answer past it is said to be too long. */
export const MAX_ANSWER_CHARS = 240 * 1024
const PROTOCOLS = ['2025-06-18', '2025-03-26', '2024-11-05'] as const

const schema = (properties: Record<string, unknown> = {}, required: readonly string[] = []) => ({ type: 'object', properties, required, additionalProperties: false })
const name = { type: 'string', minLength: 1, maxLength: 200 }
const message = { type: 'string', minLength: 1, maxLength: 8000 }
const effort = { type: 'string', minLength: 1, maxLength: 40 }
const READ_ONLY = { readOnlyHint: true }
/**
 * The tools, as the bridge an app on this computer runs lists them (resources/locust-mcp-bridge.mjs): word for word,
 * which a test checks, so an app is never told something different from anywhere.
 */
export const REMOTE_MCP_TOOLS = [
  { name: 'list_teammates', description: 'List Locust teammates: id, name, role, runtime, model, whether busy, and runs_in: the mode a turn started from here would use. Read only.', inputSchema: schema(), annotations: READ_ONLY },
  { name: 'start_conversation', description: 'Ask a Locust teammate by name or id. Ask mode (read only), unless the person has turned on "Use each teammate\'s own mode" in Locust; then the mode the teammate is set to there, and any approval is answered by the person in Locust\'s window, never from here. Optionally a model and effort for this turn alone, from list_models; left out, the teammate\'s own. The reply says the mode used. No automatic teammate handoffs. Shares remain visible in Locust. API/account usage and Locust monthly limits apply. Returns a conversation id; use read_reply to poll.', inputSchema: schema({ teammate: name, message, model: name, effort }, ['teammate', 'message']) },
  { name: 'send_message', description: 'Continue a conversation started by this server, in the same kind of mode as start_conversation, with no automatic teammate handoffs. Optionally a model and effort for this turn, as start_conversation takes them. No message is queued while it is working; poll read_reply before sending.', inputSchema: schema({ conversation_id: name, message, model: name, effort }, ['conversation_id', 'message']) },
  { name: 'read_reply', description: 'Read the newest finished answer, current activity while still working (including a wait for the person to approve an action in Locust), or Locust\'s own failure words. Read only.', inputSchema: schema({ conversation_id: name }, ['conversation_id']), annotations: READ_ONLY },
  { name: 'list_models', description: 'List the models a teammate\'s AI agent offers in this Locust, with the efforts each takes, and the model and effort it runs on now. Read only.', inputSchema: schema({ teammate: name }, ['teammate']), annotations: READ_ONLY },
  { name: 'list_background_runs', description: 'List Claude turns running in the background. Read only; never starts or stops them.', inputSchema: schema(), annotations: READ_ONLY },
  { name: 'list_routines', description: 'List Locust routines (saved multi-step workflows): each step\'s teammate and mode, the inputs a run asks for, whether it changes files, and whether run_routine can start it from here. Read only.', inputSchema: schema(), annotations: READ_ONLY },
  { name: 'run_routine', description: 'Run a Locust routine by id or name, with values for its inputs. A routine that changes files runs only if the person has turned on "Use each teammate\'s own mode" in Locust; any approval is answered by the person in Locust\'s window, never from here. A routine that asks for a folder runs only from Locust. API/account usage and Locust monthly limits apply. Returns the routine id; poll routine_status.', inputSchema: schema({ routine: name, values: { type: 'object', additionalProperties: { type: 'string', maxLength: 4000 }, maxProperties: 12 } }, ['routine']) },
  { name: 'routine_status', description: 'The step a routine started by run_routine is on, a wait for the person in Locust, or, once it ends, its last answer. Read only.', inputSchema: schema({ routine: name }, ['routine']), annotations: READ_ONLY }
] as const

/** What the relay passes on: an app's request, and what the PC sends back. */
export interface RemoteMcpAsk { readonly ask: string; readonly kind: 'connect' | 'token' | 'call'; readonly body: string; readonly auth?: string }
export interface RemoteMcpReply { readonly status: number; readonly body: string }

interface Grant {
  readonly id: string
  readonly name: string
  readonly clientId: string
  readonly host: string
  readonly added: number
  readonly used?: number
  readonly access: string
  readonly accessUntil: number
  readonly refresh: string
  readonly refreshUntil: number
}
interface Saved { readonly v: 1; readonly enabled: boolean; readonly grants: readonly Grant[] }
interface Code { readonly hash: string; readonly clientId: string; readonly redirect: string; readonly challenge: string; readonly name: string; readonly host: string; readonly until: number }
interface Asking extends RemoteMcpAsking { readonly clientId: string; readonly redirect: string; readonly challenge: string; readonly settle: (allowed: boolean) => void }

const hashOf = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const sameHash = (one: string, other: string): boolean => one.length === other.length && timingSafeEqual(Buffer.from(one), Buffer.from(other))
const reply = (status: number, body: unknown): RemoteMcpReply => ({ status, body: JSON.stringify(body) })
const oauthError = (error: string, words: string): RemoteMcpReply => reply(400, { error, error_description: words })
const VERIFIER = /^[A-Za-z0-9._~-]{43,128}$/
const KEY = /^[A-Za-z0-9_-]{43}$/

/** Where an app sends the person back, as the card names it: the host, never a whole address. */
export function hostOf(redirect: string): string {
  try {
    const url = new URL(redirect)
    return url.protocol === 'http:' ? 'an app on the computer it was started on' : url.host
  } catch { return 'an address it did not make clear' }
}

export function createRemoteMcp(options: {
  readonly file: string
  /** Locust's tool calls, as an app on this computer makes them. */
  readonly call: (name: string, args: unknown) => Promise<McpToolResult>
  /** Where apps connect, while From anywhere is on: undefined otherwise. */
  readonly address: () => string | undefined
  /** The relay is told it may take apps, or no longer. */
  readonly open: (on: boolean) => Promise<boolean>
  readonly refusal?: () => string | undefined
  /** Told whenever what Settings and the card show changed. */
  readonly changed?: (state: RemoteMcpState) => void
  /** An app asks to connect: said where the person may be, besides the card. */
  readonly asked?: (name: string) => void
  readonly now?: () => number
  readonly version?: string
}) {
  const now = options.now ?? Date.now
  let saved: Saved = { v: 1, enabled: false, grants: [] }
  let message: string | undefined
  const codes: Code[] = []
  const asking = new Map<string, Asking>()
  let writing: Promise<void> = Promise.resolve()
  let usedTimer: ReturnType<typeof setTimeout> | undefined

  const persist = (): Promise<void> => {
    const next = writing.then(async () => {
      await mkdir(dirname(options.file), { recursive: true })
      await writeFile(`${options.file}.tmp`, JSON.stringify(saved), 'utf8')
      await replaceFile(`${options.file}.tmp`, options.file)
    })
    writing = next.catch(() => undefined)
    return next
  }
  const state = (): RemoteMcpState => {
    const address = options.address()
    const shown = [...asking.values()].filter((one) => one.until > now())
    return {
      enabled: saved.enabled,
      ...(saved.enabled && address !== undefined ? { address } : {}),
      apps: saved.grants.map((grant): RemoteMcpApp => ({ id: grant.id, name: grant.name, host: grant.host, added: new Date(grant.added).toISOString(), ...(grant.used === undefined ? {} : { used: new Date(grant.used).toISOString() }) })),
      asking: shown.map(({ request, name: shownName, host, match, until }) => ({ request, name: shownName, host, match, until })),
      ...(message === undefined ? {} : { message })
    }
  }
  const tell = (): void => { options.changed?.(state()) }

  /** A card the person answers: Allow makes a code the app trades for keys; Deny, or no answer, makes none. */
  const connect = async (body: string): Promise<RemoteMcpReply> => {
    let asked: Record<string, unknown>
    try { asked = JSON.parse(body) as Record<string, unknown> } catch { return reply(400, { error: 'bad-ask' }) }
    const clientId = typeof asked.client_id === 'string' ? asked.client_id : ''
    const redirect = typeof asked.redirect === 'string' ? asked.redirect : ''
    const challenge = typeof asked.challenge === 'string' ? asked.challenge : ''
    const match = typeof asked.match === 'string' && /^\d{6}$/.test(asked.match) ? asked.match : ''
    const shownName = typeof asked.client === 'string' && asked.client.trim().length > 0 ? asked.client.slice(0, 80) : 'An app'
    if (!saved.enabled || options.refusal?.() !== undefined || clientId === '' || redirect === '' || !/^[A-Za-z0-9_-]{43}$/.test(challenge) || match === '') return reply(403, { error: 'denied' })
    const request = randomBytes(12).toString('base64url')
    const allowed = await new Promise<boolean>((settle) => {
      const until = now() + ASKING_MS
      const timer = setTimeout(() => { asking.delete(request); settle(false); tell() }, ASKING_MS)
      asking.set(request, { request, name: shownName, host: hostOf(redirect), match, until, clientId, redirect, challenge,
        settle: (yes) => { clearTimeout(timer); asking.delete(request); settle(yes); tell() } })
      tell()
      options.asked?.(shownName)
    })
    if (!allowed || !saved.enabled) return reply(403, { error: 'denied' })
    const code = randomBytes(32).toString('base64url')
    codes.push({ hash: hashOf(code), clientId, redirect, challenge, name: shownName, host: hostOf(redirect), until: now() + CODE_MS })
    return reply(200, { code })
  }

  const keys = async (grant: Omit<Grant, 'access' | 'accessUntil' | 'refresh' | 'refreshUntil'>): Promise<RemoteMcpReply> => {
    const access = randomBytes(32).toString('base64url')
    const refresh = randomBytes(32).toString('base64url')
    const made: Grant = { ...grant, access: hashOf(access), accessUntil: now() + ACCESS_MS, refresh: hashOf(refresh), refreshUntil: now() + REFRESH_MS }
    const others = saved.grants.filter((one) => one.id !== grant.id)
    // Past MAX_APPS, the app used longest ago goes.
    const kept = [...others].sort((one, other) => (other.used ?? other.added) - (one.used ?? one.added)).slice(0, MAX_APPS - 1)
    saved = { ...saved, grants: [...saved.grants.filter((one) => kept.includes(one)), made] }
    await persist()
    tell()
    return reply(200, { access_token: access, token_type: 'Bearer', expires_in: Math.floor(ACCESS_MS / 1000), refresh_token: refresh, scope: 'locust' })
  }

  /** RFC 6749 with PKCE: a code for keys, once; or a refresh key for new keys, the old ones gone. */
  const token = async (body: string): Promise<RemoteMcpReply> => {
    if (!saved.enabled || options.refusal?.() !== undefined) return oauthError('invalid_grant', 'This Locust no longer takes apps from anywhere.')
    let fields: URLSearchParams
    try {
      fields = body.trimStart().startsWith('{') ? new URLSearchParams(Object.entries(JSON.parse(body) as Record<string, string>).map(([key, value]): [string, string] => [key, String(value)])) : new URLSearchParams(body)
    } catch { return oauthError('invalid_request', 'The request could not be read.') }
    const grantType = fields.get('grant_type')
    if (grantType === 'authorization_code') {
      const code = fields.get('code') ?? ''
      const presented = hashOf(code)
      const at = codes.findIndex((one) => sameHash(one.hash, presented))
      if (at < 0) return oauthError('invalid_grant', 'That code is not one this Locust made, or it was used already.')
      // Good once, right or wrong: a code tried is gone.
      const found = codes.splice(at, 1)[0]!
      const verifier = fields.get('code_verifier') ?? ''
      if (found.until <= now()) return oauthError('invalid_grant', 'That code ran out. Connect again from the app.')
      if (fields.get('client_id') !== found.clientId || (fields.get('redirect_uri') ?? found.redirect) !== found.redirect) return oauthError('invalid_grant', 'That code was made for another app.')
      if (!VERIFIER.test(verifier) || !sameHash(createHash('sha256').update(verifier, 'ascii').digest('base64url'), found.challenge)) return oauthError('invalid_grant', 'The app could not prove it started this sign-in.')
      return keys({ id: randomBytes(9).toString('base64url'), name: found.name, clientId: found.clientId, host: found.host, added: now() })
    }
    if (grantType === 'refresh_token') {
      const presented = hashOf(fields.get('refresh_token') ?? '')
      const grant = saved.grants.find((one) => sameHash(one.refresh, presented))
      if (grant === undefined || grant.refreshUntil <= now()) return oauthError('invalid_grant', 'That refresh key is not one this Locust holds. Connect again from the app.')
      if (fields.get('client_id') !== null && fields.get('client_id') !== grant.clientId) return oauthError('invalid_grant', 'That refresh key was made for another app.')
      const { access: _a, accessUntil: _b, refresh: _c, refreshUntil: _d, ...rest } = grant
      return keys({ ...rest, used: now() })
    }
    return reply(400, { error: 'unsupported_grant_type' })
  }

  /** The app's last use, kept at most once a minute: a call is not a disk write. */
  const noteUse = (grant: Grant): void => {
    const at = now()
    saved = { ...saved, grants: saved.grants.map((one) => (one.id === grant.id ? { ...one, used: at } : one)) }
    if (usedTimer !== undefined) return
    usedTimer = setTimeout(() => { usedTimer = undefined; void persist().catch(() => undefined); tell() }, 60_000)
    usedTimer.unref?.()
  }

  const rpc = (id: unknown, result: unknown): RemoteMcpReply => {
    const text = JSON.stringify({ jsonrpc: '2.0', id, result })
    if (text.length <= MAX_ANSWER_CHARS) return { status: 200, body: text }
    return reply(200, { jsonrpc: '2.0', id, result: mcpText('That answer is too long to send through Locust’s relay. Read it in Locust.', true) })
  }
  const rpcError = (id: unknown, code: number, words: string): RemoteMcpReply => reply(200, { jsonrpc: '2.0', id: id ?? null, error: { code, message: words } })

  /** One MCP request, with the key it came with. JSON-RPC, one at a time, answered as JSON. */
  const call = async (auth: string | undefined, body: string): Promise<RemoteMcpReply> => {
    const presented = auth !== undefined && KEY.test(auth) ? hashOf(auth) : undefined
    const grant = presented === undefined ? undefined : saved.grants.find((one) => sameHash(one.access, presented))
    if (!saved.enabled || options.refusal?.() !== undefined || grant === undefined || grant.accessUntil <= now()) return reply(401, { error: 'invalid_token' })
    let request: Record<string, unknown>
    try {
      const value: unknown = JSON.parse(body)
      if (typeof value !== 'object' || value === null || Array.isArray(value)) return rpcError(null, -32600, 'Send one JSON-RPC request at a time.')
      request = value as Record<string, unknown>
    } catch { return rpcError(null, -32700, 'The request is not JSON.') }
    noteUse(grant)
    const method = request.method
    if (request.jsonrpc !== '2.0' || typeof method !== 'string') return rpcError(request.id, -32600, 'Invalid request.')
    // A notification is answered with nothing.
    if (request.id === undefined) return { status: 202, body: '' }
    const params = typeof request.params === 'object' && request.params !== null ? request.params as Record<string, unknown> : {}
    if (method === 'initialize') {
      const asked = PROTOCOLS.find((one) => one === params.protocolVersion)
      return rpc(request.id, { protocolVersion: asked ?? PROTOCOLS[0], capabilities: { tools: {} }, serverInfo: { name: 'locust', version: options.version ?? '1' } })
    }
    if (method === 'ping') return rpc(request.id, {})
    if (method === 'tools/list') return rpc(request.id, { tools: REMOTE_MCP_TOOLS })
    if (method === 'tools/call') {
      const tool = REMOTE_MCP_TOOLS.find((one) => one.name === params.name)
      if (tool === undefined) return rpcError(request.id, -32602, 'Unknown Locust tool.')
      const result = await options.call(tool.name, params.arguments ?? {}).catch(() => mcpText('Locust could not complete that request. Nothing was retried.', true))
      return rpc(request.id, result)
    }
    return rpcError(request.id, -32601, 'Method not found.')
  }

  return {
    async load(): Promise<RemoteMcpState> {
      try {
        const read = JSON.parse(await readFile(options.file, 'utf8')) as Partial<Saved>
        const grants = Array.isArray(read.grants) ? read.grants.filter((grant): grant is Grant => typeof grant === 'object' && grant !== null && typeof (grant as Grant).access === 'string' && typeof (grant as Grant).refresh === 'string') : []
        saved = { v: 1, enabled: read.enabled === true && options.refusal?.() === undefined, grants: grants.filter((grant) => grant.refreshUntil > now()) }
      } catch { /* Never turned on: off. */ }
      return state()
    },
    settings: (): RemoteMcpState => state(),
    /** The relay is told again, as the PC connects: it answers apps only while this says so. */
    sync: async (): Promise<void> => { await options.open(saved.enabled).catch(() => false) },
    async setEnabled(next: unknown): Promise<RemoteMcpState> {
      message = undefined
      if (typeof next !== 'boolean') return state()
      if (next) {
        const refused = options.refusal?.()
        if (refused !== undefined) { message = refused; return state() }
        if (options.address() === undefined) { message = 'Turn on From anywhere under Your phone first: apps reach Locust through the same relay.'; return state() }
        if (!(await options.open(true))) { message = 'The relay could not be reached. Nothing was turned on; try again in a moment.'; return state() }
        saved = { ...saved, enabled: true }
      } else {
        // Off takes every app's keys away, here and at the relay.
        for (const one of asking.values()) one.settle(false)
        codes.length = 0
        saved = { v: 1, enabled: false, grants: [] }
        await options.open(false).catch(() => false)
      }
      await persist().catch(() => { message = 'The setting could not be saved. It may change back when Locust restarts.' })
      tell()
      return state()
    },
    async remove(id: unknown): Promise<RemoteMcpState> {
      saved = { ...saved, grants: saved.grants.filter((grant) => grant.id !== id) }
      await persist().catch(() => undefined)
      tell()
      return state()
    },
    /** The person's answer to a card. */
    answerSignIn(request: unknown, allow: unknown): RemoteMcpState {
      const one = typeof request === 'string' ? asking.get(request) : undefined
      if (one !== undefined && typeof allow === 'boolean') one.settle(allow && saved.enabled)
      return state()
    },
    /** An app's request the relay passed on. */
    async answer(asked: RemoteMcpAsk): Promise<RemoteMcpReply> {
      if (asked.kind === 'connect') return connect(asked.body)
      if (asked.kind === 'token') return token(asked.body)
      return call(asked.auth, asked.body)
    },
    dispose(): void {
      clearTimeout(usedTimer)
      for (const one of asking.values()) one.settle(false)
    }
  }
}
