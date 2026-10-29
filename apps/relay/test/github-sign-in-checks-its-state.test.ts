import { randomBytes } from 'node:crypto'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { Account, changeRooms, MAX_ACCOUNT_ROOMS, type AccountRoom } from '../src/account.js'
import { githubBack, githubStart, type GithubEnv, type NotePage } from '../src/github.js'

/**
 * GITHUB SIGN-IN CHECKS ITS STATE (0.838, src/github.ts). The Worker's two ways through GitHub's sign-in: from the PC,
 * which ties a room to the account, and from any browser, which is handed a ticket into each room the account opens.
 * GitHub is stood in for; a sign-in this browser did not start is not finished, and GitHub is not asked anything.
 */
const ROOM = randomBytes(16).toString('base64url')
const OTHER_ROOM = randomBytes(16).toString('base64url')
const KEY = randomBytes(32).toString('base64url')
const LINK = randomBytes(32).toString('base64url')
const TICKET = randomBytes(32).toString('base64url')
const TOKEN = `gho_${'t'.repeat(36)}`

const note: NotePage = (said) => Response.json(said)

/** A Durable Object's storage, in memory. */
function storage() {
  const kept = new Map<string, unknown>()
  return {
    get: async <T>(key: string) => structuredClone(kept.get(key)) as T | undefined,
    put: async (key: string, value: unknown) => { kept.set(key, structuredClone(value)) },
    delete: async (keys: string[]) => { let n = 0; for (const key of keys) if (kept.delete(key)) n += 1; return n },
    deleteAll: async () => { kept.clear() },
    setAlarm: async () => undefined
  }
}

/** The Worker's two namespaces: each room answers as the test says, and accounts are the real Account. */
function anEnv(rooms: Record<string, (path: string, body: Record<string, unknown>) => Response>, offered = true) {
  const accounts = new Map<string, Account>()
  const calls: { room: string; path: string; body: Record<string, unknown> }[] = []
  const env: GithubEnv = {
    ROOMS: {
      idFromName: (name) => name,
      get: (id) => ({
        fetch: async (request) => {
          const path = new URL(request.url).pathname
          const body = await request.json() as Record<string, unknown>
          calls.push({ room: String(id), path, body })
          return rooms[String(id)]?.(path, body) ?? new Response('{}', { status: 404 })
        }
      })
    },
    ACCOUNTS: {
      idFromName: (name) => name,
      get: (id) => {
        const name = String(id)
        if (!accounts.has(name)) accounts.set(name, new Account({ storage: storage() } as unknown as DurableObjectState))
        return accounts.get(name)!
      }
    },
    GITHUB_CLIENT_ID: 'Iv1.test',
    GITHUB_CLIENT_SECRET: offered ? 'not-a-real-secret' : '',
    GITHUB_WEB: 'https://github.test',
    GITHUB_API: 'https://api.github.test'
  }
  const roomsOf = async (user: number): Promise<readonly AccountRoom[]> =>
    ((await (await env.ACCOUNTS!.get(env.ACCOUNTS!.idFromName(`github:${String(user)}`)).fetch(new Request('https://account/rooms'))).json()) as { rooms: AccountRoom[] }).rooms
  return { env, calls, roomsOf }
}

/** GitHub, stood in for: the code `good` is octo, number 42. */
const asked: string[] = []
beforeEach(() => {
  asked.length = 0
  vi.stubGlobal('fetch', async (input: string, init?: RequestInit) => {
    asked.push(input)
    if (input === 'https://github.test/login/oauth/access_token') {
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>
      expect(body).toMatchObject({ client_id: 'Iv1.test' })
      // GitHub's own answers: 200 with an error code, as it really sends them.
      if (body.client_secret !== 'not-a-real-secret') return Response.json({ error: 'incorrect_client_credentials', error_description: `The client_id and/or client_secret passed are incorrect: ${String(body.client_secret)}` })
      return Response.json(body.code === 'good' ? { access_token: TOKEN, token_type: 'bearer', scope: '' } : { error: 'bad_verification_code' })
    }
    if (input === 'https://api.github.test/user') {
      return (init?.headers as Record<string, string>).authorization === `Bearer ${TOKEN}` ? Response.json({ id: 42, login: 'octo' }) : new Response('{}', { status: 401 })
    }
    return new Response('not stood in for', { status: 500 })
  })
})
afterEach(() => { vi.unstubAllGlobals() })

const read = async (response: Response): Promise<Record<string, unknown>> => response.json() as Promise<Record<string, unknown>>
/** The browser goes to GitHub and comes back with its cookie and GitHub's state. */
async function signIn(env: GithubEnv, start: string, code = 'good', state?: string): Promise<Response> {
  const away = githubStart(new URL(start), env, note)
  expect(away.status).toBe(302)
  const cookie = away.headers.get('set-cookie')!.split(';')[0]!
  const sent = new URL(away.headers.get('location')!)
  const back = new URL(`https://app.test/v1/github/back?code=${code}&state=${state ?? sent.searchParams.get('state')!}`)
  return githubBack(new Request(back, { headers: { cookie } }), back, env, note)
}

describe('GitHub sign-in checks its state', () => {
  it('sends the browser to GitHub asking for no permissions, with a state only its cookie holds', () => {
    const { env } = anEnv({})
    const away = githubStart(new URL('https://app.test/v1/github/start'), env, note)
    const to = new URL(away.headers.get('location')!)
    expect(to.origin + to.pathname).toBe('https://github.test/login/oauth/authorize')
    expect(to.searchParams.get('client_id')).toBe('Iv1.test')
    expect(to.searchParams.get('redirect_uri')).toBe('https://app.test/v1/github/back')
    expect(to.searchParams.has('scope')).toBe(false)
    const state = to.searchParams.get('state')!
    expect(state).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(away.headers.get('set-cookie')).toBe(`__Host-locust-github=${state}; Path=/; Max-Age=600; HttpOnly; SameSite=Lax; Secure`)
  })

  it('finishes nothing it did not start, and asks GitHub nothing then', async () => {
    const { env } = anEnv({})
    const forged = await signIn(env, 'https://app.test/v1/github/start', 'good', randomBytes(32).toString('base64url'))
    expect((await read(forged)).title).toBe('GitHub sign-in did not finish')
    const back = new URL('https://app.test/v1/github/back?code=good&state=x')
    expect((await read(await githubBack(new Request(back), back, env, note))).title).toBe('GitHub sign-in did not finish')
    expect(asked).toEqual([])
  })

  it('from the PC, ties the room to the account and adds it to the account’s computers', async () => {
    const { env, calls, roomsOf } = anEnv({ [ROOM]: () => Response.json({ key: KEY, name: 'DESKTOP-1' }) })
    const page = await read(await signIn(env, `https://app.test/v1/github/start?room=${ROOM}&link=${LINK}`))
    expect(page.title).toBe('Signed in as octo')
    expect(calls).toEqual([{ room: ROOM, path: '/github/linked', body: { room: ROOM, link: LINK, user: 42, login: 'octo' } }])
    expect(await roomsOf(42)).toMatchObject([{ room: ROOM, key: KEY, name: 'DESKTOP-1' }])
    // A link that is not whole sends nobody to GitHub.
    expect((await read(githubStart(new URL(`https://app.test/v1/github/start?room=${ROOM}`), env, note))).title).toBe('That link is not whole')
  })

  it('from any browser, hands it a ticket into its one computer and goes on to pair', async () => {
    const { env, roomsOf } = anEnv({
      [ROOM]: (path) => path === '/github/linked' ? Response.json({ key: KEY, name: 'DESKTOP-1' }) : Response.json({ ticket: TICKET, key: KEY, name: 'DESKTOP-1' })
    })
    await signIn(env, `https://app.test/v1/github/start?room=${ROOM}&link=${LINK}`)
    expect(await roomsOf(42)).toHaveLength(1)
    const back = await signIn(env, 'https://app.test/v1/github/start')
    expect(back.status).toBe(302)
    expect(back.headers.get('location')).toBe(`/#g=${ROOM}.${KEY}.${TICKET}.${TOKEN}`)
    // The cookie is spent with it.
    expect(back.headers.get('set-cookie')).toContain('Max-Age=0')
  })

  it('says so when no computer is signed in, and drops a room that let the account go', async () => {
    const { env, roomsOf } = anEnv({
      [ROOM]: (path) => path === '/github/linked' ? Response.json({ key: KEY, name: 'DESKTOP-1' }) : new Response('{}', { status: 404 })
    })
    expect((await read(await signIn(env, 'https://app.test/v1/github/start'))).title).toBe('No computer is signed in yet')
    await signIn(env, `https://app.test/v1/github/start?room=${ROOM}&link=${LINK}`)
    expect((await read(await signIn(env, 'https://app.test/v1/github/start'))).title).toBe('No computer is signed in yet')
    expect(await roomsOf(42)).toEqual([])
  })

  it('lets the person choose between computers, and says when GitHub is not on here', async () => {
    const answers = (name: string) => (path: string) => path === '/github/linked' ? Response.json({ key: KEY, name }) : Response.json({ ticket: TICKET, key: KEY, name })
    const { env } = anEnv({ [ROOM]: answers('DESKTOP-1'), [OTHER_ROOM]: answers('Laptop') })
    await signIn(env, `https://app.test/v1/github/start?room=${ROOM}&link=${LINK}`)
    await signIn(env, `https://app.test/v1/github/start?room=${OTHER_ROOM}&link=${LINK}`)
    const page = await read(await signIn(env, 'https://app.test/v1/github/start'))
    expect(page.title).toBe('Which computer?')
    expect((page.links as { label: string }[]).map((link) => link.label)).toEqual(['DESKTOP-1', 'Laptop'])
    expect((await read(githubStart(new URL('https://app.test/v1/github/start'), anEnv({}, false).env, note))).title).toBe('GitHub sign-in is not on here')
  })

  it('says GitHub’s own reason when it gives no account, so the next try names what to fix', async () => {
    const { env } = anEnv({})
    const ran = await read(await signIn(env, 'https://app.test/v1/github/start', 'spent'))
    expect(ran).toMatchObject({ title: 'GitHub did not say who you are' })
    expect(String(ran.words)).toContain('code ran out or was used already')
    expect(String(ran.words)).toContain('(GitHub: bad_verification_code)')
    // A refused secret is the relay's to fix, not the person's.
    const refused = await read(await signIn({ ...env, GITHUB_CLIENT_SECRET: 'the-wrong-one' }, 'https://app.test/v1/github/start'))
    expect(String(refused.words)).toContain('refused this relay’s own sign-in key')
    expect(String(refused.words)).toContain('(GitHub: incorrect_client_credentials)')
    // Nothing GitHub sent is shown but its code.
    expect(JSON.stringify(refused)).not.toContain('the-wrong-one')
  })

  it('keeps each computer once, newest last, at most ten', () => {
    let rooms: readonly AccountRoom[] = []
    for (let i = 0; i < MAX_ACCOUNT_ROOMS + 2; i += 1) rooms = changeRooms(rooms, { add: { room: `r${String(i)}`, key: KEY, name: 'PC', at: i } })
    expect(rooms).toHaveLength(MAX_ACCOUNT_ROOMS)
    rooms = changeRooms(rooms, { add: { room: 'r5', key: KEY, name: 'PC again', at: 99 } })
    expect(rooms.at(-1)).toMatchObject({ room: 'r5', name: 'PC again' })
    expect(rooms.filter((one) => one.room === 'r5')).toHaveLength(1)
    expect(changeRooms(rooms, { remove: 'r5' }).some((one) => one.room === 'r5')).toBe(false)
  })
})
