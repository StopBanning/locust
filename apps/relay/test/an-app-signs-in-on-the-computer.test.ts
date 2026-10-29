import { createHash, randomBytes } from 'node:crypto'
import { describe, expect, it } from 'vitest'
import { roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'
import { CONNECTS_PER_HOUR, MAX_CONNECTS_WAITING, redirectAllowed } from '../src/mcp.js'
import { mcpPage } from '../src/mcp-page.js'
import type { RelayFrame } from '../src/protocol.js'

/**
 * AN APP SIGNS IN ON THE COMPUTER (0.841, mcp.ts). The relay registers an app and sends its sign-in to the PC as a
 * card; it decides nothing itself, keeps no key, and its page says nothing an app could make it say.
 */
const hash = (text: string): string => createHash('sha256').update(text).digest('base64url')
async function fixture() {
  let now = 1_800_000_000_000
  const stored = new Map<string, unknown>()
  const frames: RelayFrame[] = []
  let attachment: Attachment = { as: 'new', at: now }
  const pc: Peer = { get attachment() { return attachment }, attach: (next) => { attachment = next }, send: (frame) => { frames.push(frame) }, close: () => undefined }
  const peers: Peer[] = [pc]
  const host: RoomHost = {
    store: { get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined, put: async (key, value) => { stored.set(key, structuredClone(value)) }, delete: async (keys) => { for (const key of keys) stored.delete(key) }, deleteAll: async () => { stored.clear() } },
    peers: () => peers, now: () => now, setAlarm: async () => undefined, random: (size) => randomBytes(size).toString('base64url'), sha256: async (text) => hash(text)
  }
  const core = roomCore(host)
  const say = (frame: unknown) => core.message(pc, JSON.stringify(frame))
  await say({ t: 'auth', as: 'pc', token: randomBytes(32).toString('base64url') })
  await say({ t: 'mcp-open', id: 'o' })
  const client = String((await core.mcp.register(JSON.stringify({ client_name: '<img src=x onerror=alert(1)>Claude', redirect_uris: ['https://claude.ai/cb'] }))).body.client_id)
  const signIn = () => core.mcp.authorize(new URLSearchParams({ response_type: 'code', client_id: client, redirect_uri: 'https://claude.ai/cb', code_challenge: 'c'.repeat(43), code_challenge_method: 'S256' }))
  return { core, say, frames, stored, signIn, tick: (ms: number) => { now += ms } }
}

describe('an app signing in', () => {
  it('is a card on the PC, with the page saying the app’s name as text only', async () => {
    const room = await fixture()
    const shown = await room.signIn()
    if (shown.page !== 'waiting') throw new Error(shown.page)
    const asked = room.frames.find((frame) => frame.t === 'mcp-ask')
    expect(asked).toMatchObject({ t: 'mcp-ask', kind: 'connect' })
    expect(JSON.parse(asked?.t === 'mcp-ask' ? asked.body : '{}')).toMatchObject({ client: 'img src=x onerror=alert(1)Claude', match: shown.match })
    const html = mcpPage(shown)
    expect(html).not.toContain('<img')
    expect(html).toContain(`${shown.match.slice(0, 3)} ${shown.match.slice(3)}`)
    // The relay kept no code: none was made.
    expect(JSON.stringify([...room.stored.values()])).not.toContain('"code"')
  })

  it('lets at most a few sign-ins wait, and a few an hour, so nobody can fill the window with cards', async () => {
    const room = await fixture()
    for (let at = 0; at < MAX_CONNECTS_WAITING; at += 1) expect((await room.signIn()).page).toBe('waiting')
    expect((await room.signIn()).page).toBe('busy')
    room.tick(5 * 60_000 + 1)
    for (let at = MAX_CONNECTS_WAITING; at < CONNECTS_PER_HOUR; at += 1) {
      const page = (await room.signIn()).page
      expect(page).toBe('waiting')
      room.tick(5 * 60_000 + 1)
    }
    expect((await room.signIn()).page).toBe('busy')
  })

  it('sends a person back only to an https or loopback address, and off forgets every app', async () => {
    expect(redirectAllowed('https://claude.ai/api/mcp/auth_callback')).toBe(true)
    expect(redirectAllowed('http://localhost:33418/callback')).toBe(true)
    expect(redirectAllowed('http://claude.ai/cb')).toBe(false)
    expect(redirectAllowed('javascript:alert(1)')).toBe(false)
    expect(redirectAllowed('https://claude.ai/cb#x')).toBe(false)
    const room = await fixture()
    await room.say({ t: 'mcp-close', id: 'c' })
    expect((await room.signIn()).page).toBe('off')
    expect(room.stored.has('mcp')).toBe(false)
  })
})
