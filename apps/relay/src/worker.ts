import icon256 from '../../desktop/resources/icon.png'
import icon512 from '../../desktop/resources/icon-512.png'
import { PHONE_SERVICE_WORKER, phoneManifest, phonePage } from '../../desktop/src/main/phone-page.js'
import { deskNotePage, deskPage } from '../../desktop/src/main/desk-page.js'

import { MAX_FRAME, ROOM_ID } from './protocol.js'
import { WINDOW_PATH, WINDOW_VERSION, windowDirectory, windowFile, windowType, type WindowEntry, type ZipRange } from './window-zip.js'
import type { RoomEnv } from './room.js'
import { githubBack, githubOffered, githubStart, type NotePage } from './github.js'
import { MAX_SYNC_OBJECT } from './sync.js'

export { Room } from './room.js'
export { Account } from './account.js'

/**
 * LOCUST'S RELAY (0.826), at app.locust.lol: the page a phone opens to reach Locust from anywhere, and the rooms that
 * page and the PC meet in (/v1/room/<id>, one Durable Object each). The page is the one the PC serves on the home
 * network, asking through the relay instead (apps/desktop/src/main/phone-page.ts). Since 0.827 it is also a
 * home-screen app (its manifest, service worker and icons) that can be sent alerts, signed with the relay's key.
 *
 * Open source with the rest of Locust, so anyone can run their own: LOCUST_RELAY points Locust at another copy.
 */
interface Env extends RoomEnv {
  readonly ROOMS: DurableObjectNamespace
  readonly GITHUB_WEB?: string
  readonly GITHUB_API?: string
  /** Where each release's window zip is (0.830): Locust's own downloads, unless a copy of the relay names another. */
  readonly DESK_DOWNLOADS?: string
}

/*
 * LOCUST IN A BROWSER (0.830). /desk/ is the way in (desk-page.ts); /desk/v/<version>/ is that release's own window,
 * read a file at a time from the zip the release put beside its installer (window-zip.ts). Only a release's own code
 * runs on this address: the computer a browser is paired with tells it which version to open, and nothing more.
 */
const DEFAULT_DOWNLOADS = 'https://dl.locust.lol/'
const directories = new Map<string, Promise<ReadonlyMap<string, WindowEntry> | undefined>>()

const zipRange = (url: string): ZipRange => async (start, end) => {
  const answer = await fetch(url, { headers: { range: end === undefined ? `bytes=-${String(start)}` : `bytes=${String(start)}-${String(end)}` } }).catch(() => undefined)
  if (answer === undefined || (answer.status !== 206 && answer.status !== 200)) return undefined
  const bytes = new Uint8Array(await answer.arrayBuffer())
  if (answer.status === 200) {
    // A server that sends the whole file for a range: the range is cut from it here.
    const total = bytes.length
    return end === undefined ? { bytes: bytes.subarray(Math.max(0, total - start)), total } : { bytes: bytes.subarray(start, end + 1), total }
  }
  const total = Number(/\/(\d+)$/.exec(answer.headers.get('content-range') ?? '')?.[1] ?? Number.NaN)
  return Number.isFinite(total) ? { bytes, total } : undefined
}

const deskHeaders = (url: URL): Record<string, string> => ({
  'content-type': 'text/html; charset=utf-8',
  'cache-control': 'no-cache',
  'x-robots-tag': 'noindex',
  ...security(url),
  'content-security-policy':
    `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' data: blob:; connect-src 'self' ${url.protocol === 'https:' ? 'wss' : 'ws'}://${url.host}; worker-src 'self' blob:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`
})

async function windowAsset(url: URL, env: Env, version: string, path: string): Promise<Response> {
  const notFound = (): Response => new Response('Not found.', { status: 404, headers: { 'cache-control': 'no-store' } })
  const type = windowType(path)
  if (type === undefined || !WINDOW_PATH.test(path)) return notFound()
  const zip = `${(env.DESK_DOWNLOADS ?? DEFAULT_DOWNLOADS).replace(/\/*$/, '/')}window/Locust-${version}-window.zip`
  const range = zipRange(zip)
  let directory = directories.get(version)
  if (directory === undefined) {
    directory = windowDirectory(range)
    directories.set(version, directory)
  }
  const entries = await directory
  if (entries === undefined) {
    directories.delete(version)
    return new Response(`Locust ${version}'s window is not on Locust's downloads.`, { status: 404, headers: { 'cache-control': 'no-store', 'content-type': 'text/plain; charset=utf-8' } })
  }
  const entry = entries.get(path)
  if (entry === undefined) return notFound()
  const bytes = await windowFile(range, entry)
  if (bytes === undefined) return new Response('The file could not be read.', { status: 502, headers: { 'cache-control': 'no-store' } })
  // A release's files never change: kept by the browser for good. Its page is asked for again, for its policy.
  if (path.endsWith('.html')) return new Response(bytes, { headers: deskHeaders(url) })
  return new Response(bytes, { headers: { 'content-type': type, 'cache-control': 'public, max-age=31536000, immutable', ...security(url) } })
}

const security = (url: URL): Record<string, string> => ({
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  ...(url.protocol === 'https:' ? { 'strict-transport-security': 'max-age=31536000' } : {})
})

const pageHeaders = (url: URL): HeadersInit => ({
  'content-type': 'text/html; charset=utf-8',
  'cache-control': 'no-cache',
  'x-robots-tag': 'noindex',
  ...security(url),
  'content-security-policy':
    `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self' ${url.protocol === 'https:' ? 'wss' : 'ws'}://${url.host}; img-src 'self' data:; manifest-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`
})

/** An app's sign-in metadata: only addresses, the same for every room, kept briefly. */
const metadataHeaders = (url: URL): Record<string, string> => ({ 'cache-control': 'public, max-age=300', 'access-control-allow-origin': '*', ...security(url) })

const file = (url: URL, body: BodyInit, type: string, cache = 'public, max-age=3600'): Response =>
  new Response(body, { headers: { 'content-type': type, 'cache-control': cache, ...security(url) } })

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)
    if (request.method === 'GET') {
      if (url.pathname === '/' || url.pathname === '/index.html') return new Response(phonePage({ via: 'relay', ...(env.VAPID_PUBLIC_KEY === undefined ? {} : { alertsKey: env.VAPID_PUBLIC_KEY }) }), { headers: pageHeaders(url) })
      // A service worker is checked for a newer one on every visit: never kept by a cache.
      if (url.pathname === '/sw.js') return file(url, PHONE_SERVICE_WORKER, 'text/javascript; charset=utf-8', 'no-cache')
      if (url.pathname === '/manifest.webmanifest') return file(url, phoneManifest(), 'application/manifest+json')
      if (url.pathname === '/icon-256.png') return file(url, icon256, 'image/png', 'public, max-age=86400')
      if (url.pathname === '/icon-512.png') return file(url, icon512, 'image/png', 'public, max-age=86400')
      if (url.pathname === '/desk' || url.pathname === '/desk/') return new Response(deskPage({ github: githubOffered(env) }), { headers: pageHeaders(url) })
      // Signing in with GitHub (0.838, github.ts).
      const note: NotePage = (said) => new Response(deskNotePage(said), { headers: { ...pageHeaders(url), 'cache-control': 'no-store' } })
      if (url.pathname === '/v1/github/start') return githubStart(url, env, note)
      if (url.pathname === '/v1/github/back') return githubBack(request, url, env, note)
      const asset = /^\/desk\/v\/([^/]+)\/(.+)$/.exec(url.pathname)
      if (asset !== null && WINDOW_VERSION.test(asset[1]!)) return windowAsset(url, env, asset[1]!, asset[2]!)
    }
    /*
     * OTHER APPS FROM ANYWHERE (0.841, mcp.ts). What an app reads first, to learn where to sign in: the room's MCP
     * address is the protected resource (RFC 9728), and the room itself is its sign-in server (RFC 8414), at the
     * addresses MCP's clients try. Both say only addresses; a room that is off answers them the same, and refuses at
     * the room.
     */
    if (request.method === 'GET') {
      const resource = /^\/\.well-known\/oauth-protected-resource\/v1\/room\/([^/]+)\/mcp$/.exec(url.pathname)?.[1]
      if (resource !== undefined && ROOM_ID.test(resource)) {
        return Response.json({ resource: `${url.origin}/v1/room/${resource}/mcp`, authorization_servers: [`${url.origin}/v1/room/${resource}`], bearer_methods_supported: ['header'], resource_name: 'Locust' }, { headers: metadataHeaders(url) })
      }
      const server = /^\/\.well-known\/(?:oauth-authorization-server|openid-configuration)\/v1\/room\/([^/]+)$/.exec(url.pathname)?.[1]
        ?? /^\/v1\/room\/([^/]+)\/\.well-known\/(?:oauth-authorization-server|openid-configuration)$/.exec(url.pathname)?.[1]
      if (server !== undefined && ROOM_ID.test(server)) {
        const issuer = `${url.origin}/v1/room/${server}`
        return Response.json({
          issuer,
          authorization_endpoint: `${issuer}/oauth/authorize`,
          token_endpoint: `${issuer}/oauth/token`,
          registration_endpoint: `${issuer}/oauth/register`,
          response_types_supported: ['code'],
          grant_types_supported: ['authorization_code', 'refresh_token'],
          code_challenge_methods_supported: ['S256'],
          token_endpoint_auth_methods_supported: ['none'],
          scopes_supported: ['locust']
        }, { headers: metadataHeaders(url) })
      }
    }
    const mcpRoom = /^\/v1\/room\/([^/]+)\/(?:oauth\/(?:register|authorize|wait|token)|mcp)$/.exec(url.pathname)?.[1]
    if (mcpRoom !== undefined && ROOM_ID.test(mcpRoom)) {
      if (Number(request.headers.get('content-length') ?? '0') > MAX_FRAME) return new Response('{"error":"too-big"}', { status: 413, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
      return env.ROOMS.get(env.ROOMS.idFromName(mcpRoom)).fetch(request)
    }
    // An account's sealed copies (docs/PLAN-2026-10-30-ACCOUNTS.md) are asked for at the PC's own room.
    const sync = /^\/v1\/room\/([^/]+)\/sync(?:\/[^/]+){0,2}$/.exec(url.pathname)?.[1]
    if (sync !== undefined && ROOM_ID.test(sync)) {
      if (Number(request.headers.get('content-length') ?? '0') > MAX_SYNC_OBJECT) return new Response('{"error":"too-big"}', { status: 413, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
      return env.ROOMS.get(env.ROOMS.idFromName(sync)).fetch(request)
    }
    // Called links (2026-10-25) go to the same room, with their own readable letters.
    const hook =/^\/v1\/room\/([^/]+)\/hook\/([^/]+)$/.exec(url.pathname)
    if (hook !== null && ROOM_ID.test(hook[1]!)) {
      if (request.method !== 'POST') return new Response('{"error":"method"}', { status: 405, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', allow: 'POST' } })
      if (Number(request.headers.get('content-length') ?? '0') > MAX_FRAME) return new Response('{"error":"too-big"}', { status: 413 })
      return env.ROOMS.get(env.ROOMS.idFromName(hook[1]!)).fetch(request)
    }
    // Scout's away look (0.829) posts to its room over HTTPS, from a GitHub workflow.
    const away = /^\/v1\/room\/([^/]+)\/away$/.exec(url.pathname)?.[1]
    if (away !== undefined && ROOM_ID.test(away)) {
      if (request.method !== 'GET' && request.method !== 'POST') return new Response('{"error":"bad-frame"}', { status: 405, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
      const length = Number(request.headers.get('content-length') ?? '0')
      if (!(length <= MAX_FRAME)) return new Response('{"error":"too-big"}', { status: 413, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
      return env.ROOMS.get(env.ROOMS.idFromName(away)).fetch(request)
    }
    const room = /^\/v1\/room\/([^/]+)$/.exec(url.pathname)?.[1]
    if (room !== undefined && ROOM_ID.test(room)) {
      if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return new Response('Expected a WebSocket.', { status: 426 })
      return env.ROOMS.get(env.ROOMS.idFromName(room)).fetch(request)
    }
    return new Response('Not found.', { status: 404, headers: { 'cache-control': 'no-store' } })
  }
}
