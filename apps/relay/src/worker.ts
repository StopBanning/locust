import icon256 from '../../desktop/resources/icon.png'
import icon512 from '../../desktop/resources/icon-512.png'
import { PHONE_SERVICE_WORKER, phoneManifest, phonePage } from '../../desktop/src/main/phone-page.js'

import { MAX_FRAME, ROOM_ID } from './protocol.js'
import type { RoomEnv } from './room.js'

export { Room } from './room.js'

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
