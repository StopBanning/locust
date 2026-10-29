import { createHash, randomBytes } from 'node:crypto'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import { roomCore, type Attachment, type Peer, type RoomHost } from '../../../relay/src/room-core.js'
import type { RelayFrame } from '../../../relay/src/protocol.js'
import { mcpText } from './locust-mcp-host.js'
import { createRemoteMcp, REMOTE_MCP_TOOLS } from './remote-mcp.js'

/**
 * AN APP USES THE TEAM FROM ANYWHERE (0.841). Claude on the web connects to Locust through the relay the way MCP signs
 * in: it registers, sends the person to the room's sign-in, a card in Locust's window names it with six digits the
 * page shows too, and only Allow there makes a code. The PC makes and checks every key; the relay only passes them on.
 * Here the relay's room (apps/relay/src/room-core.ts, mcp.ts) and the PC's side (remote-mcp.ts) meet in one process,
 * with the room's held requests (room.ts) done the same way: an ask's answer is waited for by name.
 */
const hash = (text: string): string => createHash('sha256').update(text).digest('base64url')
const folders: string[] = []
afterEach(async () => { await Promise.all(folders.splice(0).map((folder) => rm(folder, { recursive: true, force: true }))) })

async function world() {
  const folder = await mkdtemp(join(tmpdir(), 'locust-remote-mcp-'))
  folders.push(folder)
  let now = 1_800_000_000_000
  const stored = new Map<string, unknown>()
  const waiting = new Map<string, (reply: { status: number; body: string }) => void>()
  let attachment: Attachment = { as: 'new', at: now }
  const pending: Promise<unknown>[] = []
  // The PC's line to the relay: what the relay asks is answered by the PC module, as phone-anywhere.ts does.
  const pc: Peer = {
    get attachment() { return attachment },
    attach: (next) => { attachment = next },
    send: (frame: RelayFrame) => {
      if (frame.t === 'mcp-ask') pending.push(remote.answer({ ask: frame.ask, kind: frame.kind, body: frame.body, ...(frame.auth === undefined ? {} : { auth: frame.auth }) })
        .then((answer) => core.message(pc, JSON.stringify({ t: 'mcp-answer', ask: frame.ask, status: answer.status, body: answer.body }))))
    },
    close: () => undefined
  }
  const peers: Peer[] = [pc]
  const host: RoomHost = {
    store: { get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined, put: async (key, value) => { stored.set(key, structuredClone(value)) }, delete: async (keys) => { for (const key of keys) stored.delete(key) }, deleteAll: async () => { stored.clear() } },
    peers: () => peers, now: () => now, setAlarm: async () => undefined, random: (size) => randomBytes(size).toString('base64url'), sha256: async (text) => hash(text),
    answered: (ask, status, body) => { waiting.get(ask)?.({ status, body }) }
  }
  const core = roomCore(host)
  const calls: [string, unknown][] = []
  const remote = createRemoteMcp({
    file: join(folder, 'remote-mcp.json'),
    call: async (name, args) => { calls.push([name, args]); return mcpText(JSON.stringify([{ id: 'tm_wren', name: 'Wren' }])) },
    address: () => 'https://app.locust.lol/v1/room/r/mcp',
    open: async (on) => { await core.message(pc, JSON.stringify({ t: on ? 'mcp-open' : 'mcp-close', id: 'o' })); return true },
    now: () => now
  })
  await core.message(pc, JSON.stringify({ t: 'auth', as: 'pc', token: randomBytes(32).toString('base64url') }))
  /** room.ts's held request: registered under its name, sent, and its answer waited for. */
  const held = async (kind: 'token' | 'call', body: string, auth?: string) => {
    const ask = randomBytes(12).toString('base64url')
    const answered = new Promise<{ status: number; body: string }>((resolve) => waiting.set(ask, resolve))
    const refused = await core.mcp.forward(ask, kind, body, auth)
    if (refused !== undefined) return { status: refused.status, body: JSON.stringify(refused.body) }
    const reply = await answered
    waiting.delete(ask)
    return reply
  }
  const settle = async (): Promise<void> => { while (pending.length > 0) await pending.shift() }
  const verifier = randomBytes(32).toString('base64url')
  const challenge = createHash('sha256').update(verifier).digest('base64url')
  const redirect = 'https://claude.ai/api/mcp/auth_callback'
  /** The app's part: register, then send the person to sign in. */
  const signIn = async (state = 'st_1') => {
    const registered = await core.mcp.register(JSON.stringify({ client_name: 'Claude', redirect_uris: [redirect] }))
    const clientId = String(registered.body.client_id)
    const page = await core.mcp.authorize(new URLSearchParams({ response_type: 'code', client_id: clientId, redirect_uri: redirect, code_challenge: challenge, code_challenge_method: 'S256', state }))
    return { registered, clientId, page }
  }
  const exchange = (code: string, clientId: string, proof = verifier) =>
    held('token', new URLSearchParams({ grant_type: 'authorization_code', code, code_verifier: proof, client_id: clientId, redirect_uri: redirect }).toString())
  const rpc = async (token: string, method: string, params?: unknown, id: number | null = 1) => {
    const reply = await held('call', JSON.stringify({ jsonrpc: '2.0', ...(id === null ? {} : { id }), method, ...(params === undefined ? {} : { params }) }), token)
    return { status: reply.status, json: reply.body === '' ? undefined : JSON.parse(reply.body) as Record<string, any> }
  }
  return { core, remote, held, settle, signIn, exchange, rpc, calls, stored, folder, peers, verifier, tick: (ms: number) => { now += ms } }
}

/** The whole sign-in, allowed: the code from the page's redirect, traded for keys. */
async function connected(room: Awaited<ReturnType<typeof world>>) {
  const { clientId, page } = await room.signIn()
  if (page.page !== 'waiting') throw new Error(`no card: ${page.page}`)
  await new Promise((resolve) => setTimeout(resolve, 0))
  const card = room.remote.settings().asking[0]!
  room.remote.answerSignIn(card.request, true)
  await room.settle()
  const waited = await room.core.mcp.wait(page.ask)
  const code = new URL(String(waited.body.to)).searchParams.get('code')!
  const keys = JSON.parse((await room.exchange(code, clientId)).body) as Record<string, string>
  return { clientId, code, keys, card, page, waited }
}

describe('an app from anywhere', () => {
  it('signs in only when the person allows its card, which shows the six digits its page does', async () => {
    const room = await world()
    expect((await room.signIn()).page).toEqual({ page: 'off' })
    await room.remote.setEnabled(true)
    const { registered, page } = await room.signIn()
    expect(registered).toMatchObject({ status: 201, body: { client_name: 'Claude', token_endpoint_auth_method: 'none' } })
    if (page.page !== 'waiting') throw new Error('no card')
    await new Promise((resolve) => setTimeout(resolve, 0))
    const card = room.remote.settings().asking[0]!
    expect(card).toMatchObject({ name: 'Claude', host: 'claude.ai', match: page.match })
    expect(page.match).toMatch(/^\d{6}$/)
    // Nothing is decided by waiting, or on the page.
    expect(await room.core.mcp.wait(page.ask)).toEqual({ status: 200, body: { state: 'waiting' } })
    room.remote.answerSignIn(card.request, true)
    await room.settle()
    const waited = await room.core.mcp.wait(page.ask)
    expect(waited.body.state).toBe('allowed')
    const back = new URL(String(waited.body.to))
    expect(back.origin + back.pathname).toBe('https://claude.ai/api/mcp/auth_callback')
    expect(back.searchParams.get('state')).toBe('st_1')
    expect(back.searchParams.get('code')).toMatch(/^[A-Za-z0-9_-]{43}$/)
    // Told once.
    expect((await room.core.mcp.wait(page.ask)).body).toEqual({ state: 'gone' })
    expect(room.remote.settings().asking).toEqual([])
  })

  it('sends the app its own word for no when the person denies it, and makes no code', async () => {
    const room = await world()
    await room.remote.setEnabled(true)
    const { page } = await room.signIn('st_2')
    if (page.page !== 'waiting') throw new Error('no card')
    await new Promise((resolve) => setTimeout(resolve, 0))
    room.remote.answerSignIn(room.remote.settings().asking[0]!.request, false)
    await room.settle()
    const waited = await room.core.mcp.wait(page.ask)
    expect(waited.body.state).toBe('denied')
    const back = new URL(String(waited.body.to))
    expect(back.searchParams.get('error')).toBe('access_denied')
    expect(back.searchParams.get('code')).toBeNull()
  })

  it('trades the code for keys once, and only with the proof the app started with', async () => {
    const room = await world()
    await room.remote.setEnabled(true)
    const { clientId, page } = await room.signIn()
    if (page.page !== 'waiting') throw new Error('no card')
    await new Promise((resolve) => setTimeout(resolve, 0))
    room.remote.answerSignIn(room.remote.settings().asking[0]!.request, true)
    await room.settle()
    const code = new URL(String((await room.core.mcp.wait(page.ask)).body.to)).searchParams.get('code')!
    // A wrong proof spends the code: it is good once, right or wrong.
    const wrong = await room.exchange(code, clientId, randomBytes(32).toString('base64url'))
    expect(wrong.status).toBe(400)
    expect(JSON.parse(wrong.body).error).toBe('invalid_grant')
    expect((await room.exchange(code, clientId)).status).toBe(400)
  })

  it('answers MCP with the same tools and rules as an app on this computer, for a good key only', async () => {
    const room = await world()
    await room.remote.setEnabled(true)
    const { keys } = await connected(room)
    expect(keys).toMatchObject({ token_type: 'Bearer', expires_in: 86_400, scope: 'locust' })
    const init = await room.rpc(keys.access_token!, 'initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude' } })
    expect(init.json?.result).toMatchObject({ protocolVersion: '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'locust' } })
    expect((await room.rpc(keys.access_token!, 'notifications/initialized', undefined, null)).status).toBe(202)
    const listed = await room.rpc(keys.access_token!, 'tools/list')
    expect(listed.json?.result.tools.map((tool: { name: string }) => tool.name)).toEqual(['list_teammates', 'start_conversation', 'send_message', 'read_reply', 'list_models', 'list_background_runs', 'list_routines', 'run_routine', 'routine_status'])
    const called = await room.rpc(keys.access_token!, 'tools/call', { name: 'list_teammates', arguments: {} })
    expect(called.json?.result.content[0].text).toContain('Wren')
    expect(room.calls).toEqual([['list_teammates', {}]])
    // Only the listed tools: never Locust's own command-line ones.
    expect((await room.rpc(keys.access_token!, 'tools/call', { name: 'cli/start', arguments: {} })).json?.error.code).toBe(-32602)
    expect(room.calls).toHaveLength(1)
    expect((await room.rpc(randomBytes(32).toString('base64url'), 'tools/list')).status).toBe(401)
    // A day on, the key is spent; the refresh key makes new ones, and the old refresh key is gone.
    room.tick(24 * 60 * 60_000 + 1)
    expect((await room.rpc(keys.access_token!, 'tools/list')).status).toBe(401)
    const renewed = await room.held('token', new URLSearchParams({ grant_type: 'refresh_token', refresh_token: keys.refresh_token! }).toString())
    const fresh = JSON.parse(renewed.body) as Record<string, string>
    expect((await room.rpc(fresh.access_token!, 'tools/list')).status).toBe(200)
    expect((await room.held('token', new URLSearchParams({ grant_type: 'refresh_token', refresh_token: keys.refresh_token! }).toString())).status).toBe(400)
  })

  it('keeps only hashes of the keys, and Remove or Off takes them away at once', async () => {
    const room = await world()
    await room.remote.setEnabled(true)
    const { keys } = await connected(room)
    const file = await readFile(join(room.folder, 'remote-mcp.json'), 'utf8')
    expect(file).not.toContain(keys.access_token)
    expect(file).not.toContain(keys.refresh_token)
    expect(JSON.stringify([...room.stored.values()])).not.toContain(keys.access_token)
    const app = room.remote.settings().apps[0]!
    expect(app).toMatchObject({ name: 'Claude', host: 'claude.ai' })
    await room.remote.remove(app.id)
    expect((await room.rpc(keys.access_token!, 'tools/list')).status).toBe(401)
    const again = await connected(room)
    await room.remote.setEnabled(false)
    expect(room.remote.settings()).toMatchObject({ enabled: false, apps: [] })
    // The relay forgot the room's apps: it takes no sign-in and passes no call.
    expect((await room.core.mcp.register(JSON.stringify({ redirect_uris: ['https://claude.ai/x'] }))).status).toBe(404)
    expect((await room.rpc(again.keys.access_token!, 'tools/list')).status).toBe(404)
  })

  it('asks nothing of a computer that is off, and refuses a sign-in that is not one', async () => {
    const room = await world()
    await room.remote.setEnabled(true)
    const { clientId } = await room.signIn()
    const params = (over: Record<string, string>) => new URLSearchParams({ response_type: 'code', client_id: clientId, redirect_uri: 'https://claude.ai/api/mcp/auth_callback', code_challenge: 'a'.repeat(43), code_challenge_method: 'S256', ...over })
    expect((await room.core.mcp.authorize(params({ client_id: 'nobody' }))).page).toBe('bad')
    expect((await room.core.mcp.authorize(params({ redirect_uri: 'https://evil.example/cb' }))).page).toBe('bad')
    expect(await room.core.mcp.authorize(params({ code_challenge_method: 'plain' }))).toMatchObject({ page: 'redirect' })
    expect((await room.core.mcp.register(JSON.stringify({ redirect_uris: ['http://evil.example/cb'] }))).status).toBe(400)
    room.peers.length = 0
    expect((await room.core.mcp.authorize(params({}))).page).toBe('offline')
    expect((await room.held('call', '{}', randomBytes(32).toString('base64url'))).status).toBe(503)
  })
})

describe('the tools an app from anywhere is offered', () => {
  it('are word for word the ones an app on this computer is offered', async () => {
    const bridge = await readFile(fileURLToPath(new URL('../../resources/locust-mcp-bridge.mjs', import.meta.url)), 'utf8')
    for (const tool of REMOTE_MCP_TOOLS) {
      expect(bridge).toContain(`{ name: '${tool.name}', description: '${tool.description.replaceAll("'", "\\'")}'`)
      expect(/READS = new Set\(\[([^\]]*)\]\)/.exec(bridge)![1]!.includes(`'${tool.name}'`)).toBe('annotations' in tool)
    }
    const start = bridge.indexOf('const TOOLS = [')
    const listed = bridge.slice(start, bridge.indexOf('\n]', start))
    expect([...listed.matchAll(/^ {2}\{ name: '([a-z_]+)'/gm)].map((match) => match[1])).toEqual(REMOTE_MCP_TOOLS.map((tool) => tool.name))
  })
})
