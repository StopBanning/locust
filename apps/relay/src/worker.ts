import { phonePage } from '../../desktop/src/main/phone-page.js'

import { ROOM_ID } from './protocol.js'

export { Room } from './room.js'

/**
 * LOCUST'S RELAY (0.826), at app.locust.lol: the page a phone opens to reach Locust from anywhere, and the rooms that
 * page and the PC meet in (/v1/room/<id>, one Durable Object each). The page is the one the PC serves on the home
 * network, asking through the relay instead (apps/desktop/src/main/phone-page.ts).
 *
 * Open source with the rest of Locust, so anyone can run their own: LOCUST_RELAY points Locust at another copy.
 */
interface Env {
  readonly ROOMS: DurableObjectNamespace
}

const pageHeaders = (url: URL): HeadersInit => ({
  'content-type': 'text/html; charset=utf-8',
  'cache-control': 'no-cache',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  'x-robots-tag': 'noindex',
  ...(url.protocol === 'https:' ? { 'strict-transport-security': 'max-age=31536000' } : {}),
  'content-security-policy':
    `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self' ${url.protocol === 'https:' ? 'wss' : 'ws'}://${url.host}; img-src 'self' data:; manifest-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`
})

const page = phonePage({ via: 'relay' })

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)
    if (request.method === 'GET' && (url.pathname === '/' || url.pathname === '/index.html')) return new Response(page, { headers: pageHeaders(url) })
    const room = /^\/v1\/room\/([^/]+)$/.exec(url.pathname)?.[1]
    if (room !== undefined && ROOM_ID.test(room)) {
      if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return new Response('Expected a WebSocket.', { status: 426 })
      return env.ROOMS.get(env.ROOMS.idFromName(room)).fetch(request)
    }
    return new Response('Not found.', { status: 404, headers: { 'cache-control': 'no-store' } })
  }
}
