import { createHash, randomBytes, timingSafeEqual } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http'
import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os'
import { join } from 'node:path'

import type { PhoneSettings, PhoneState, PhoneTurn } from '../shared/phone.js'
import { privateMcpFile } from './locust-mcp-host.js'
import { replaceFile } from './replace-file.js'

/**
 * The key, compared in constant time over digests of equal length. Node reads
 * header bytes as Latin-1, so a supplied key of the right LENGTH in characters
 * could be twice as many bytes in UTF-8, and `timingSafeEqual` throws on
 * unequal lengths -- before the wrong-key count, with no answer sent (review
 * 2026-10-15, C2). Digests are always 32 bytes, so nothing here can throw.
 */
function sameKey(supplied: string, expected: string): boolean {
  const digest = (value: string): Buffer => createHash('sha256').update(value, 'latin1').digest()
  return timingSafeEqual(digest(supplied), digest(expected))
}

/*
 * YOUR PHONE (0.775, shared/phone.ts): one small page on the home network. Off means no socket. On, the page is
 * served to anyone who asks (it holds nothing), and everything it shows or does needs the key from the QR code.
 */
export const PHONE_FILE = 'phone.json'
/** A fixed port, so a phone's saved address still works after Locust restarts; any free one when it is taken. */
export const PHONE_PORT = 47740
/** The largest request a phone may send, either way it comes. */
export const MAX_BODY = 32 * 1024
const MAX_MESSAGE = 20_000
/** Wrong keys from one address before it is refused for a while: a 192-bit key is not guessed, but nobody gets to try. */
const MAX_WRONG = 20
const WRONG_WINDOW_MS = 10 * 60_000

export type PhoneSent = { readonly ok: true } | { readonly ok: false; readonly message: string }

export interface PhoneCalls {
  readonly state: () => Promise<PhoneState>
  readonly turns: (missionId: string) => Promise<readonly PhoneTurn[] | undefined>
  readonly decide: (approvalId: string, approve: boolean) => Promise<boolean>
  readonly send: (missionId: string, text: string) => Promise<PhoneSent>
}

/**
 * The addresses a phone on the same network can reach: IPv4, private ranges (and Tailscale's), never a virtual
 * switch's (Hyper-V, WSL, VirtualBox, VMware, Docker), which a phone cannot reach. Home Wi-Fi and Ethernet first.
 */
export function phoneAddresses(interfaces: NodeJS.Dict<NetworkInterfaceInfo[]> = networkInterfaces()): readonly string[] {
  const found: { readonly address: string; readonly rank: number }[] = []
  for (const [name, entries] of Object.entries(interfaces)) {
    if (entries === undefined || /vethernet|virtualbox|vmware|hyper-v|wsl|docker|loopback|vbox/i.test(name)) continue
    for (const entry of entries) {
      if (entry.family !== 'IPv4' || entry.internal) continue
      const [a, b] = entry.address.split('.').map(Number) as [number, number]
      const home = a === 192 && b === 168 ? 0 : a === 10 ? 1 : a === 172 && b >= 16 && b <= 31 ? 2 : a === 100 && b >= 64 && b <= 127 ? 3 : undefined
      if (home === undefined) continue
      found.push({ address: entry.address, rank: home + (/wi-?fi|wlan|wireless|ethernet/i.test(name) ? 0 : 0.5) })
    }
  }
  return [...new Set(found.sort((x, y) => x.rank - y.rank).map((entry) => entry.address))]
}

/** A browser on the network names this computer by its address; a name is a page trying DNS rebinding, refused. */
const HOST = /^(?:\d{1,3}(?:\.\d{1,3}){3}|localhost|127\.0\.0\.1)(?::\d{1,5})?$/

const PAGE_HEADERS = {
  'content-type': 'text/html; charset=utf-8',
  'cache-control': 'no-store',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  'content-security-policy':
    "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; manifest-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
} as const

async function readBody(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = []
  let size = 0
  for await (const chunk of req) {
    size += Buffer.byteLength(chunk)
    if (size > MAX_BODY) throw new Error('too large')
    chunks.push(Buffer.from(chunk))
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8')) as unknown
}

const json = (res: ServerResponse, status: number, value: unknown): void => {
  if (res.destroyed) return
  res.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' }).end(JSON.stringify(value))
}

export interface PhoneAnswer {
  readonly status: number
  readonly data: unknown
}

/**
 * What Locust answers a phone, however the request came (0.826): the page asks the same paths on the home network
 * and through the relay (phone-anywhere.ts). Undefined for a request it does not know; each way says that its own way.
 */
export async function answerPhone(calls: PhoneCalls, method: 'GET' | 'POST', path: string, input?: unknown): Promise<PhoneAnswer | undefined> {
  const url = new URL(path, 'http://phone')
  if (method === 'GET') {
    if (url.pathname === '/api/state') return { status: 200, data: await calls.state() }
    if (url.pathname === '/api/turns') {
      const turns = await calls.turns(url.searchParams.get('id') ?? '')
      return turns === undefined ? { status: 404, data: { message: 'That conversation is not in Locust any more.' } } : { status: 200, data: { turns } }
    }
    return undefined
  }
  const fields = typeof input === 'object' && input !== null ? (input as Record<string, unknown>) : {}
  if (url.pathname === '/api/decide' && typeof fields.approvalId === 'string' && typeof fields.approve === 'boolean') {
    return { status: 200, data: { ok: await calls.decide(fields.approvalId, fields.approve) } }
  }
  if (url.pathname === '/api/send' && typeof fields.missionId === 'string' && typeof fields.text === 'string') {
    const text = fields.text.trim()
    if (text.length === 0 || text.length > MAX_MESSAGE) return { status: 400, data: { ok: false, message: 'Write a message first.' } }
    return { status: 200, data: await calls.send(fields.missionId, text) }
  }
  return undefined
}

/** The key is checked from the headers before any body is read. */
export function phoneRequestHandler(key: () => string | undefined, page: () => string, calls: PhoneCalls, now: () => number = Date.now) {
  const wrong = new Map<string, number[]>()
  return (req: IncomingMessage, res: ServerResponse): void => {
    if (!HOST.test(req.headers.host ?? '')) { res.writeHead(421, { connection: 'close' }).end(); return }
    const url = new URL(req.url ?? '/', 'http://phone')
    if (req.method === 'GET' && (url.pathname === '/' || url.pathname === '/index.html')) {
      res.writeHead(200, PAGE_HEADERS).end(page())
      return
    }
    if (!url.pathname.startsWith('/api/')) { res.writeHead(404).end(); return }
    const from = req.socket.remoteAddress ?? ''
    const recent = (wrong.get(from) ?? []).filter((at) => now() - at < WRONG_WINDOW_MS)
    if (recent.length >= MAX_WRONG) { json(res, 429, { message: 'Too many wrong keys. Scan the code in Locust again in a few minutes.' }); return }
    const expected = key()
    const auth = req.headers.authorization
    const supplied = typeof auth === 'string' && auth.startsWith('Bearer ') ? auth.slice(7) : ''
    if (expected === undefined || !sameKey(supplied, expected)) {
      wrong.set(from, [...recent, now()])
      json(res, 401, { message: 'This phone is not paired. Scan the code in Locust’s Settings again.' })
      return
    }
    void (async () => {
      if (req.method === 'GET') {
        const answer = await answerPhone(calls, 'GET', url.pathname + url.search)
        if (answer !== undefined) { json(res, answer.status, answer.data); return }
      }
      if (req.method !== 'POST') { res.writeHead(405).end(); return }
      // A form on another page cannot send JSON with a key; this refuses whatever else arrives.
      if (!(req.headers['content-type'] ?? '').startsWith('application/json')) { res.writeHead(415).end(); return }
      let input: unknown
      try { input = await readBody(req) } catch { json(res, 400, { message: 'That message could not be read.' }); return }
      if (key() !== expected) { json(res, 401, { message: 'This phone was unpaired.' }); return }
      const answer = await answerPhone(calls, 'POST', url.pathname, input)
      if (answer !== undefined) { json(res, answer.status, answer.data); return }
      res.writeHead(404).end()
    })().catch(() => json(res, 500, { message: 'Locust could not do that. Nothing was sent.' }))
  }
}

export function createPhoneHost(options: {
  readonly directory: string
  readonly page: () => string
  readonly calls: PhoneCalls
  readonly refusal?: () => string | undefined
  readonly addresses?: () => readonly string[]
  readonly port?: number
  readonly protectFile?: (path: string) => Promise<void>
}) {
  const file = join(options.directory, PHONE_FILE)
  const addresses = options.addresses ?? (() => phoneAddresses())
  let server: Server | undefined
  let port: number | undefined
  let key: string | undefined
  let queue: Promise<unknown> = Promise.resolve()
  let disposed = false

  const settings = (message = options.refusal?.()): PhoneSettings => {
    const found = addresses()
    const url = server !== undefined && key !== undefined && port !== undefined && found[0] !== undefined
      ? `http://${found[0]}:${String(port)}/#${key}` : undefined
    return {
      enabled: server !== undefined,
      ...(url === undefined ? {} : { url }),
      addresses: port === undefined ? [] : found.map((address) => `http://${address}:${String(port)}`),
      ...(message === undefined ? {} : { message })
    }
  }
  const save = async (enabled: boolean, saved: string | undefined = key): Promise<void> => {
    await mkdir(options.directory, { recursive: true })
    const temp = file + '.tmp'
    await rm(temp, { force: true })
    await writeFile(temp, '', { mode: 0o600, flag: 'wx' })
    // The key is written only once the file is the account's alone, as the MCP connection file is.
    await (options.protectFile ?? privateMcpFile)(temp)
    await writeFile(temp, JSON.stringify({ enabled, key: saved }), { mode: 0o600 })
    await replaceFile(temp, file)
  }
  const stop = async (): Promise<void> => {
    const listener = server
    server = undefined
    port = undefined
    if (listener === undefined) return
    listener.closeAllConnections()
    await new Promise<void>((resolve) => listener.close(() => resolve()))
  }
  const listen = (listener: Server, at: number): Promise<void> => new Promise((resolve, reject) => {
    listener.once('error', reject)
    // Every network this computer is on: the firewall decides which may reach it (Windows asks the person once).
    listener.listen(at, '0.0.0.0', () => { listener.off('error', reject); resolve() })
  })
  const start = async (): Promise<void> => {
    if (server !== undefined) return
    key ??= randomBytes(24).toString('base64url')
    const listener = createServer(phoneRequestHandler(() => key, options.page, options.calls))
    listener.requestTimeout = 30_000
    listener.headersTimeout = 10_000
    listener.keepAliveTimeout = 5_000
    try {
      await listen(listener, options.port ?? PHONE_PORT)
    } catch {
      await listen(listener, 0)
    }
    const address = listener.address()
    if (typeof address !== 'object' || address === null) { listener.close(); throw new Error('no port') }
    server = listener
    port = address.port
  }
  const serial = (work: () => Promise<PhoneSettings>): Promise<PhoneSettings> => {
    const task = queue.then(work)
    queue = task.catch(() => undefined)
    return task
  }
  return {
    settings: async (): Promise<PhoneSettings> => { await queue; return settings() },
    load: () => serial(async () => {
      if (disposed || server !== undefined) return settings()
      if (options.refusal?.() !== undefined) return settings()
      try {
        const saved = JSON.parse(await readFile(file, 'utf8')) as { readonly enabled?: unknown; readonly key?: unknown }
        if (typeof saved.key === 'string' && /^[A-Za-z0-9_-]{32}$/.test(saved.key)) key = saved.key
        if (saved.enabled === true) await start()
      } catch { /* Missing or unreadable stays off. */ }
      return settings()
    }),
    setEnabled: (enabled: unknown) => serial(async () => {
      if (disposed || typeof enabled !== 'boolean') return settings('That setting was not changed.')
      if (enabled && options.refusal?.() !== undefined) return settings()
      try {
        if (enabled) await start()
        else {
          await stop()
          // Off unpairs: the next time it is on, it has a new key and a phone scans again.
          key = undefined
        }
        await save(enabled)
        return settings()
      } catch {
        await stop()
        return settings('Locust could not open its page on your network. It is still off.')
      }
    }),
    /** A new key: every paired phone is unpaired at once, and the code changes. */
    forget: () => serial(async () => {
      if (disposed) return settings()
      const next = randomBytes(24).toString('base64url')
      try { await save(server !== undefined, next) } catch { return settings('The new code could not be saved. The old one still works.') }
      key = next
      return settings()
    }),
    dispose: async (): Promise<void> => { disposed = true; await queue.catch(() => undefined); await stop() }
  }
}
