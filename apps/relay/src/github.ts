import { accountName, type AccountRoom } from './account.js'
import { GITHUB_LOGIN, ROOM_ID, TOKEN } from './protocol.js'

/*
 * SIGNING IN WITH GITHUB (0.838). Two ways through GitHub's sign-in, both ending at /v1/github/back:
 *
 * - From the PC: Settings sends the person to /v1/github/start?room=<room>&link=<link>. Signed in, the room is tied to
 *   the account (room-core.ts githubLinked), and the account's list of rooms gains it (account.ts).
 * - From any browser: /v1/github/start alone. Signed in, each room the account opens hands this browser a ticket, and
 *   the browser goes to the phone page with it (phone-page.ts, #g=), which pairs the way a code does. The PC checks
 *   the account with GitHub itself (phone-anywhere.ts) before it lets the browser in.
 *
 * The relay asks GitHub for no permissions: only who the person is. It keeps the account's number and login, never
 * GitHub's token, which goes on to the browser and from there, sealed, to the PC.
 */
export interface GithubEnv {
  readonly ROOMS: DurableObjectNamespace
  readonly ACCOUNTS?: DurableObjectNamespace
  /** The OAuth app's id (wrangler.jsonc) and secret (`wrangler secret put GITHUB_CLIENT_SECRET`). */
  readonly GITHUB_CLIENT_ID?: string
  readonly GITHUB_CLIENT_SECRET?: string
  /** GitHub's sign-in and API, unless a drive stands its own in their place. */
  readonly GITHUB_WEB?: string
  readonly GITHUB_API?: string
}

export const githubOffered = (env: Pick<GithubEnv, 'ACCOUNTS' | 'GITHUB_CLIENT_ID' | 'GITHUB_CLIENT_SECRET'>): boolean =>
  env.ACCOUNTS !== undefined && (env.GITHUB_CLIENT_ID ?? '') !== '' && (env.GITHUB_CLIENT_SECRET ?? '') !== ''

/** What the Worker shows: a short page, built by the desk page's own look (desk-page.ts). */
export type NotePage = (note: { readonly title: string; readonly words: string; readonly links?: readonly { readonly label: string; readonly href: string }[] }) => Response

const STATE = /^[A-Za-z0-9_-]{43}$/
const COOKIE_MS = 10 * 60
const cookieName = (url: URL): string => (url.protocol === 'https:' ? '__Host-locust-github' : 'locust-github')

const random = (): string => {
  let text = ''
  for (const byte of crypto.getRandomValues(new Uint8Array(32))) text += String.fromCharCode(byte)
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

const web = (env: GithubEnv): string => (env.GITHUB_WEB ?? 'https://github.com').replace(/\/+$/, '')
const api = (env: GithubEnv): string => (env.GITHUB_API ?? 'https://api.github.com').replace(/\/+$/, '')

const TRY_AGAIN = { label: 'Try again', href: '/desk/' }
const DOWNLOAD = { label: 'Download Locust', href: 'https://locust.lol' }

/** Off to GitHub, with a state only this browser holds, and the PC's link when the PC sent the person. */
export function githubStart(url: URL, env: GithubEnv, note: NotePage): Response {
  if (!githubOffered(env)) return note({ title: 'GitHub sign-in is not on here', words: 'This copy of Locust’s relay does not offer signing in with GitHub. Pair this browser with a code from Locust on your computer instead.', links: [TRY_AGAIN] })
  const room = url.searchParams.get('room')
  const link = url.searchParams.get('link')
  const linking = room !== null || link !== null
  if (linking && (room === null || !ROOM_ID.test(room) || link === null || !TOKEN.test(link))) return note({ title: 'That link is not whole', words: 'Go back to Locust on your computer and choose Sign in with GitHub again.' })
  const state = random()
  const back = `${url.origin}/v1/github/back`
  const to = `${web(env)}/login/oauth/authorize?${new URLSearchParams({ client_id: env.GITHUB_CLIENT_ID!, redirect_uri: back, state, allow_signup: 'true' }).toString()}`
  const kept = linking ? `${state}.${room!}.${link!}` : state
  return new Response(null, {
    status: 302,
    headers: {
      location: to,
      'cache-control': 'no-store',
      'set-cookie': `${cookieName(url)}=${kept}; Path=/; Max-Age=${String(COOKIE_MS)}; HttpOnly; SameSite=Lax${url.protocol === 'https:' ? '; Secure' : ''}`
    }
  })
}

/** GitHub's own reason a sign-in gave no account (2026-10-29): its error code, or what came back instead. */
interface SignInFailed { readonly failed: string }

/**
 * What the person is told for each of GitHub's reasons. The first live sign-in said only "could not be read", which
 * named no one to fix it: a refused secret is the relay's to fix, a code that ran out is a fresh try.
 */
function failedWords(reason: string): string {
  if (reason === 'bad_verification_code') return 'GitHub said this sign-in’s code ran out or was used already. Start the sign-in again.'
  if (reason === 'incorrect_client_credentials') return 'GitHub refused this relay’s own sign-in key, so nobody can sign in here until it is set again. Pair with a code from Locust on your computer meanwhile.'
  if (reason === 'redirect_uri_mismatch') return 'GitHub does not have this address as the one to come back to. Pair with a code from Locust on your computer meanwhile.'
  return 'GitHub’s answer gave no account. Start the sign-in again; if it says this again, pair with a code from Locust on your computer.'
}

/** GitHub's word for who signed in: its token and the account, or GitHub's reason it gave none. */
async function whoSignedIn(env: GithubEnv, code: string, back: string): Promise<{ readonly token: string; readonly user: number; readonly login: string } | SignInFailed> {
  const exchange = await fetch(`${web(env)}/login/oauth/access_token`, {
    method: 'POST',
    headers: { accept: 'application/json', 'content-type': 'application/json', 'user-agent': 'locust-relay' },
    body: JSON.stringify({ client_id: env.GITHUB_CLIENT_ID, client_secret: env.GITHUB_CLIENT_SECRET, code, redirect_uri: back })
  }).catch(() => undefined)
  if (exchange === undefined) return { failed: 'unreachable' }
  if (!exchange.ok) return { failed: `exchange-${String(exchange.status)}` }
  const exchanged = await exchange.json().catch(() => undefined) as Record<string, unknown> | undefined
  const token = exchanged?.access_token
  if (typeof token !== 'string' || !/^[A-Za-z0-9_]{20,255}$/.test(token)) {
    // Only GitHub's own code, which is a word from its documented list: never anything that came with it.
    const said = typeof exchanged?.error === 'string' && /^[a-z_]{1,60}$/.test(exchanged.error) ? exchanged.error : 'no-token'
    return { failed: said }
  }
  const asked = await fetch(`${api(env)}/user`, { headers: { authorization: `Bearer ${token}`, accept: 'application/vnd.github+json', 'user-agent': 'locust-relay' } }).catch(() => undefined)
  if (asked === undefined || !asked.ok) return { failed: `user-${asked === undefined ? 'unreachable' : String(asked.status)}` }
  const user = await asked.json().catch(() => undefined) as Record<string, unknown> | undefined
  if (typeof user?.id !== 'number' || !Number.isSafeInteger(user.id) || user.id <= 0 || typeof user.login !== 'string' || !GITHUB_LOGIN.test(user.login)) return { failed: 'user-unreadable' }
  return { token, user: user.id, login: user.login }
}

const roomCall = (env: GithubEnv, room: string, path: string, body: unknown): Promise<Response> =>
  env.ROOMS.get(env.ROOMS.idFromName(room)).fetch(new Request(`https://room${path}`, { method: 'POST', body: JSON.stringify(body) }))
const accountCall = (env: Pick<GithubEnv, 'ACCOUNTS'>, user: number, path: string, body?: unknown): Promise<Response> =>
  env.ACCOUNTS!.get(env.ACCOUNTS!.idFromName(accountName(user))).fetch(new Request(`https://account${path}`, body === undefined ? {} : { method: 'POST', body: JSON.stringify(body) }))

/** The rooms an account opens forget it, through the room's own host (room.ts). */
export const forgetAccountRoom = (env: Pick<GithubEnv, 'ACCOUNTS'>, user: number, room: string): Promise<void> =>
  accountCall(env, user, '/remove', { room }).then(() => undefined)

export async function githubBack(request: Request, url: URL, env: GithubEnv, note: NotePage): Promise<Response> {
  if (!githubOffered(env)) return note({ title: 'GitHub sign-in is not on here', words: 'This copy of Locust’s relay does not offer signing in with GitHub.', links: [TRY_AGAIN] })
  const cookie = new RegExp(`(?:^|;\\s*)${cookieName(url).replace(/[-_]/g, '\\$&')}=([^;]+)`).exec(request.headers.get('cookie') ?? '')?.[1] ?? ''
  const [state, room, link] = cookie.split('.')
  const code = url.searchParams.get('code')
  // The state is checked before anything else: a sign-in this browser did not start is not finished here.
  if (state === undefined || !STATE.test(state) || url.searchParams.get('state') !== state || code === null || code.length > 200) {
    return note({ title: 'GitHub sign-in did not finish', words: url.searchParams.get('error') === 'access_denied' ? 'GitHub was told not to sign you in here. Nothing changed.' : 'This sign-in started somewhere else, or it ran out. Start it again.', links: [TRY_AGAIN] })
  }
  const who = await whoSignedIn(env, code, `${url.origin}/v1/github/back`)
  // GitHub's reason is said with it, in its own words, so the next try says what to fix.
  if ('failed' in who) return note({ title: 'GitHub did not say who you are', words: `${failedWords(who.failed)} (GitHub: ${who.failed})`, links: [TRY_AGAIN] })
  const cleared = `${cookieName(url)}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax${url.protocol === 'https:' ? '; Secure' : ''}`
  const withCleared = (response: Response): Response => { response.headers.append('set-cookie', cleared); return response }

  if (room !== undefined && link !== undefined) {
    const tied = await roomCall(env, room, '/github/linked', { room, link, user: who.user, login: who.login })
    if (tied.status !== 200) return withCleared(note({ title: 'That sign-in ran out', words: 'Go back to Locust on your computer and choose Sign in with GitHub again.' }))
    const kept = await tied.json() as { readonly key: string; readonly name: string }
    await accountCall(env, who.user, '/add', { room, key: kept.key, name: kept.name })
    return withCleared(note({ title: `Signed in as ${who.login}`, words: `Now any browser where you sign in to GitHub as ${who.login} opens Locust on ${kept.name}, with no code. You can close this tab.` }))
  }

  const listed = await accountCall(env, who.user, '/rooms').then((answer) => answer.json(), () => ({ rooms: [] })) as { readonly rooms: readonly AccountRoom[] }
  const ways: { readonly name: string; readonly href: string }[] = []
  // A room that let the account go is dropped from its list; one that would not answer now is only waited for.
  let waiting = 0
  for (const one of listed.rooms) {
    const answer = await roomCall(env, one.room, '/github/ticket', { user: who.user }).catch(() => undefined)
    if (answer?.status === 404) { await forgetAccountRoom(env, who.user, one.room).catch(() => undefined); continue }
    if (answer?.status !== 200) { waiting += 1; continue }
    const given = await answer.json() as { readonly ticket: string; readonly key: string; readonly name: string }
    ways.push({ name: given.name, href: `/#g=${one.room}.${given.key}.${given.ticket}.${who.token}` })
  }
  if (ways.length === 0) {
    return withCleared(note({
      title: waiting === 0 ? 'No computer is signed in yet' : 'Try again in a while',
      words: waiting === 0
        ? `No Locust is signed in to GitHub as ${who.login}. On your computer, open Locust, then Settings › General › Your phone, and choose Sign in with GitHub. Then sign in here again.`
        : 'Locust’s relay could not let this account in just now, or has let it in many times in the last hour. Try again later, or pair this browser with a code from Locust on your computer.',
      links: [TRY_AGAIN, DOWNLOAD]
    }))
  }
  if (ways.length === 1) return withCleared(new Response(null, { status: 302, headers: { location: ways[0]!.href, 'cache-control': 'no-store' } }))
  return withCleared(note({ title: 'Which computer?', words: `${who.login} is signed in on more than one Locust. Choose the one to open here.`, links: ways.map((way) => ({ label: way.name, href: way.href })) }))
}
