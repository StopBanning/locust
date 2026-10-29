import type { Peer, RoomHost } from './room-core.js'
import { TOKEN, type RelayFrame } from './protocol.js'

/**
 * OTHER APPS, FROM ANYWHERE (0.841). Claude on the web, Claude Duet, and any app that takes a remote MCP server, reach
 * a person's Locust at <relay>/v1/room/<room>/mcp. Locust's own MCP server runs on the person's computer and nowhere
 * else (main/locust-mcp-host.ts): an app on Anthropic's computers could not reach it.
 *
 * The relay passes each request to the PC over the connection it already keeps, and the PC answers it. The relay
 * decides nothing about who may come in:
 *
 *   - An app connects the way MCP's sign-in asks (OAuth 2.1 with PKCE): it registers itself, then sends the person to
 *     an address here, and the PC puts a card in Locust's window naming the app and a six-digit match. The person
 *     allows or denies it THERE, on their own computer, never on this page.
 *   - The code the app is handed, the keys it gets for it, and every key's check, are the PC's (main/remote-mcp.ts).
 *     The relay keeps no key of its own and cannot make one.
 *   - What the relay can read: these requests are an app's plain HTTPS, so their words reach it the way a called
 *     link's do. They are never kept: a request waits in memory for its answer, at most MCP_ASK_MS.
 *   - Off is off: a room the PC has not opened answers nothing but 404, and turning it off forgets every app.
 */

/** An ask the PC answers by name: 12 random bytes. */
export const MCP_ASK = /^[A-Za-z0-9_-]{16}$/
/** A registered app's id: 16 random bytes. */
export const MCP_CLIENT = /^[A-Za-z0-9_-]{22}$/
/** The longest request or answer passed on. A tool's answer past this is cut by the PC first. */
export const MAX_MCP_BODY = 256 * 1024
/** How long a request waits for the PC's answer. */
export const MCP_ASK_MS = 30_000
/** How long the person has to answer the card in Locust. */
export const CONNECT_MS = 5 * 60_000
/** Apps one room keeps registered: the oldest goes first past this. */
export const MAX_MCP_CLIENTS = 30
/** Cards waiting at once: more is someone else trying, and is refused. */
export const MAX_CONNECTS_WAITING = 2
export const CONNECTS_PER_HOUR = 10
export const REGISTERS_PER_HOUR = 30
/** A busy app polls; a looping one is stopped before it costs the person's plan. */
export const MCP_CALLS_PER_HOUR = 1_200
const HOUR_MS = 60 * 60_000
const KEY = 'mcp'

export interface McpClient { readonly id: string; readonly name: string; readonly redirects: readonly string[]; readonly at: number }
export interface McpConnect {
  readonly ask: string
  readonly client: string
  readonly redirect: string
  readonly state?: string
  readonly until: number
  readonly match: string
  /** The PC's code, once the person allowed it; true once they denied it. */
  readonly code?: string
  readonly denied?: true
}
interface McpState { readonly since: number; readonly clients: readonly McpClient[]; readonly connects: readonly McpConnect[] }

export interface McpAnswer { readonly status: number; readonly body: Readonly<Record<string, unknown>> }
/** What the page an app sends the person to shows. */
export type McpPage =
  | { readonly page: 'waiting'; readonly ask: string; readonly match: string; readonly client: string }
  /** A sign-in that is not one: no redirect is made for it, as OAuth asks. */
  | { readonly page: 'bad'; readonly words: string }
  | { readonly page: 'off' | 'offline' | 'busy' }
  /** An error the app is told by its own address, as OAuth asks for a request that came from a known app. */
  | { readonly page: 'redirect'; readonly to: string }

/** An address an app may be sent back to: https, or a loopback one on the person's own computer (Claude Code's). */
export function redirectAllowed(value: string): boolean {
  if (value.length > 500) return false
  let url: URL
  try { url = new URL(value) } catch { return false }
  if (url.hash !== '' || url.username !== '' || url.password !== '') return false
  if (url.protocol === 'https:') return true
  return url.protocol === 'http:' && (url.hostname === 'localhost' || url.hostname === '127.0.0.1' || url.hostname === '[::1]')
}

/** An app's own name, as the card will say it: printable, short, never markup. */
function clientName(value: unknown): string {
  const text = typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f<>]/g, '').trim() : ''
  return text.length === 0 ? 'An app' : text.slice(0, 80)
}

/** `to` with these added to its query, as OAuth sends an app back. */
function back(to: string, fields: Readonly<Record<string, string | undefined>>): string {
  const url = new URL(to)
  for (const [key, value] of Object.entries(fields)) if (value !== undefined) url.searchParams.set(key, value)
  return url.href
}

const CHALLENGE = /^[A-Za-z0-9_-]{43}$/

export function mcpRoom(host: RoomHost & { readonly exists: () => Promise<boolean>; readonly pc: () => Peer | undefined }) {
  const read = (): Promise<McpState | undefined> => host.store.get<McpState>(KEY)
  const write = (next: McpState): Promise<void> => host.store.put(KEY, next)
  /*
   * COUNTED IN MEMORY, as wrong called-link passwords are (room-core.ts): a count in storage would cost a write per
   * call. A room that sleeps forgets them; one being called does not sleep.
   */
  const registered: number[] = []
  const connected: number[] = []
  const called: number[] = []
  const under = (times: number[], most: number): boolean => {
    const now = host.now()
    while (times.length > 0 && now - times[0]! >= HOUR_MS) times.shift()
    if (times.length >= most) return false
    times.push(now)
    return true
  }
  const ask = (peer: Peer, frame: Extract<RelayFrame, { readonly t: 'mcp-ask' }>): void => peer.send(frame)
  const on = async (): Promise<boolean> => (await host.exists()) && (await read()) !== undefined

  return {
    /** The PC turned it on: the room answers apps from now. */
    async open(): Promise<void> {
      if ((await read()) === undefined) await write({ since: host.now(), clients: [], connects: [] })
    },
    /** Off forgets every app and every card. The PC forgets their keys. */
    async close(): Promise<void> {
      await host.store.delete([KEY])
    },
    on,

    /** RFC 7591: an app says its name and where to send the person back, and is given an id. */
    async register(body: string): Promise<McpAnswer> {
      const state = await read()
      if (state === undefined || !(await host.exists())) return { status: 404, body: { error: 'not-found' } }
      let asked: Record<string, unknown>
      try {
        const value: unknown = JSON.parse(body)
        if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('not an object')
        asked = value as Record<string, unknown>
      } catch { return { status: 400, body: { error: 'invalid_client_metadata', error_description: 'The registration is not a JSON object.' } } }
      const redirects = asked.redirect_uris
      if (!Array.isArray(redirects) || redirects.length === 0 || redirects.length > 5 || !redirects.every((one) => typeof one === 'string' && redirectAllowed(one))) {
        return { status: 400, body: { error: 'invalid_redirect_uri', error_description: 'Give one to five https addresses, or loopback ones, to send the person back to.' } }
      }
      if (!under(registered, REGISTERS_PER_HOUR)) return { status: 429, body: { error: 'too-fast' } }
      const client: McpClient = { id: host.random(16), name: clientName(asked.client_name), redirects: redirects as string[], at: host.now() }
      await write({ ...state, clients: [...state.clients, client].slice(-MAX_MCP_CLIENTS) })
      return {
        status: 201,
        body: {
          client_id: client.id,
          client_name: client.name,
          redirect_uris: client.redirects,
          client_id_issued_at: Math.floor(client.at / 1000),
          token_endpoint_auth_method: 'none',
          grant_types: ['authorization_code', 'refresh_token'],
          response_types: ['code']
        }
      }
    },

    /** The address an app sends the person to: a card in Locust for them to answer, and this page to wait on. */
    async authorize(params: URLSearchParams): Promise<McpPage> {
      const state = await read()
      if (state === undefined || !(await host.exists())) return { page: 'off' }
      const client = state.clients.find((one) => one.id === params.get('client_id'))
      const redirect = params.get('redirect_uri') ?? (client?.redirects.length === 1 ? client.redirects[0] : undefined)
      if (client === undefined) return { page: 'bad', words: 'The app that sent you here is not one this Locust knows. Connect it again from the app.' }
      if (redirect === undefined || !client.redirects.includes(redirect)) return { page: 'bad', words: 'The app asked to send you back somewhere it did not register. Nothing was asked of your computer.' }
      const given = params.get('state') ?? undefined
      const refuse = (error: string): McpPage => ({ page: 'redirect', to: back(redirect, { error, state: given }) })
      if (params.get('response_type') !== 'code') return refuse('unsupported_response_type')
      const challenge = params.get('code_challenge') ?? ''
      if (params.get('code_challenge_method') !== 'S256' || !CHALLENGE.test(challenge)) return refuse('invalid_request')
      if (given !== undefined && given.length > 500) return refuse('invalid_request')
      const pc = host.pc()
      if (pc === undefined) return { page: 'offline' }
      const open = state.connects.filter((connect) => connect.until > host.now())
      if (open.filter((connect) => connect.code === undefined && connect.denied !== true).length >= MAX_CONNECTS_WAITING || !under(connected, CONNECTS_PER_HOUR)) return { page: 'busy' }
      // Six digits, shown on this page and on the card: the person checks they are answering this sign-in.
      const match = Array.from(host.random(9), (character) => String(character.charCodeAt(0) % 10)).join('').slice(0, 6)
      const connect: McpConnect = { ask: host.random(12), client: client.id, redirect, ...(given === undefined ? {} : { state: given }), until: host.now() + CONNECT_MS, match }
      await write({ ...state, connects: [...open, connect] })
      ask(pc, { t: 'mcp-ask', ask: connect.ask, kind: 'connect', body: JSON.stringify({ client: client.name, client_id: client.id, redirect, challenge, match }) })
      return { page: 'waiting', ask: connect.ask, match, client: client.name }
    },

    /** What the waiting page is told: still waiting, or where to go now. A decided sign-in is told once. */
    async wait(askId: string): Promise<McpAnswer> {
      const state = await read()
      const connect = state?.connects.find((one) => one.ask === askId)
      if (state === undefined || connect === undefined) return { status: 404, body: { state: 'gone' } }
      if (connect.code === undefined && connect.denied !== true && connect.until > host.now()) return { status: 200, body: { state: 'waiting' } }
      await write({ ...state, connects: state.connects.filter((one) => one !== connect && one.until > host.now()) })
      if (connect.code !== undefined) return { status: 200, body: { state: 'allowed', to: back(connect.redirect, { code: connect.code, state: connect.state }) } }
      if (connect.denied === true) return { status: 200, body: { state: 'denied', to: back(connect.redirect, { error: 'access_denied', state: connect.state }) } }
      return { status: 200, body: { state: 'expired' } }
    },

    /** The PC answered a card: its code, or a refusal. True when the answer was for a card. */
    async answered(askId: string, status: number, body: string): Promise<boolean> {
      const state = await read()
      const connect = state?.connects.find((one) => one.ask === askId)
      if (state === undefined || connect === undefined) return false
      if (connect.code !== undefined || connect.denied === true) return true
      let code: unknown
      try { code = (JSON.parse(body) as Record<string, unknown>).code } catch { code = undefined }
      const decided: McpConnect = status === 200 && typeof code === 'string' && TOKEN.test(code) ? { ...connect, code } : { ...connect, denied: true }
      await write({ ...state, connects: state.connects.map((one) => (one === connect ? decided : one)) })
      return true
    },

    /**
     * A key's exchange or a tool call, for the PC to answer: undefined once it is on its way, or why it was not sent.
     * The bearer is passed as it came; the PC alone can tell a good one.
     */
    async forward(askId: string, kind: 'token' | 'call', body: string, auth?: string): Promise<McpAnswer | undefined> {
      if (!(await on())) return { status: 404, body: { error: 'not-found' } }
      if (new TextEncoder().encode(body).length > MAX_MCP_BODY) return { status: 413, body: { error: 'too-big' } }
      const pc = host.pc()
      if (pc === undefined) return { status: 503, body: { error: 'pc-offline' } }
      if (!under(called, MCP_CALLS_PER_HOUR)) return { status: 429, body: { error: 'too-fast' } }
      ask(pc, { t: 'mcp-ask', ask: askId, kind, body, ...(auth === undefined ? {} : { auth }) })
      return undefined
    }
  }
}
