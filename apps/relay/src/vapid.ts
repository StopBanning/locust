/**
 * THE RELAY'S SIGNATURE ON A PUSH (0.827, RFC 8292). A push service takes a message only from a sender that signs
 * for it, and a browser's subscription is tied to one sender's public key. The relay is that sender: it holds the
 * private key (a Cloudflare secret, never in the code) and signs a short-lived token for each push service it posts
 * to. What it signs for is a message the PC already encrypted for one browser (apps/desktop/src/main/web-push.ts).
 */

export { isPushEndpoint } from './protocol.js'

const base64url = (bytes: Uint8Array): string => {
  let text = ''
  for (const byte of bytes) text += String.fromCharCode(byte)
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

export interface VapidKeys {
  /** The private key as a JWK (EC, P-256). */
  readonly privateJwk: JsonWebKey
  /** The public key, uncompressed, base64url: what the page hands the browser when it subscribes. */
  readonly publicKey: string
  /** Who runs this relay, as a push service asks: an https address, never a person's email. */
  readonly subject: string
}

/** The Authorization header for one push service, good for twelve hours. */
export async function vapidAuthorization(endpoint: string, keys: VapidKeys, now: number): Promise<string> {
  const header = base64url(new TextEncoder().encode(JSON.stringify({ typ: 'JWT', alg: 'ES256' })))
  const claims = base64url(new TextEncoder().encode(JSON.stringify({ aud: new URL(endpoint).origin, exp: Math.floor(now / 1000) + 12 * 3600, sub: keys.subject })))
  const key = await crypto.subtle.importKey('jwk', keys.privateJwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign'])
  // WebCrypto signs ES256 as r || s, which is what a JWT wants.
  const signature = new Uint8Array(await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, key, new TextEncoder().encode(`${header}.${claims}`)))
  return `vapid t=${header}.${claims}.${base64url(signature)}, k=${keys.publicKey}`
}
