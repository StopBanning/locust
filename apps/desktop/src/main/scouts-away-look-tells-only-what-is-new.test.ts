import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import { roomCore, type Attachment, type Peer } from '../../../relay/src/room-core.js'
import type { Letter, RelayFrame } from '../../../relay/src/protocol.js'
import { awayLetterFrom, newAwayKey, openAwayLetter, sealAwayLetter, type AwayLetter } from '../shared/away-letter.js'
import { awayConfigOf, awayLook, localTime, type AwayConfig } from './scout-away-look.js'

/**
 * SCOUT'S AWAY LOOK TELLS ONLY WHAT IS NEW (0.829). The look a workflow runs while the PC is off, against the relay's
 * own rules in memory and a stand-in for `gh`: with the PC on it does nothing; the first time it only notes what is
 * open; after that it tells what changed since the last letter -- the PC's or its own -- rings the phones for what
 * can't wait, brings the morning brief once a day, and keeps quiet hours. Its log names nothing from GitHub.
 */
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const REPO = 'acme/app'
const PR_URL = `https://github.com/${REPO}/pull/7`
const TITLE = 'Secret launch plan'

class FakePeer implements Peer {
  attachment: Attachment = { as: 'new', at: 0 }
  readonly heard: RelayFrame[] = []
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { this.heard.push(frame) }
  close(): void { /* not needed here */ }
}

interface World {
  checks: 'SUCCESS' | 'FAILURE'
  issues: { number: number; title: string; url: string; createdAt: string; author: { login: string } }[]
  github: boolean
}

function aRelay() {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  const rung: string[] = []
  let now = Date.parse('2026-10-23T14:00:00Z')
  const core = roomCore({
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers],
    now: () => now,
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => hash(text),
    push: async (endpoint, body) => { rung.push(body); return 201 }
  })
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, JSON.stringify(frame))
  /** fetch, answered by the room's own away handler. */
  const fetcher = (async (url: string | URL | Request, init?: RequestInit) => {
    const bearer = /^Bearer (.+)$/.exec(String((init?.headers as Record<string, string> | undefined)?.authorization ?? ''))?.[1]
    expect(String(url)).toMatch(/\/v1\/room\/[A-Za-z0-9_-]{22}\/away$/)
    const answer = await core.away({ method: init?.method ?? 'GET', ...(bearer === undefined ? {} : { token: bearer }), ...(typeof init?.body === 'string' ? { body: init.body } : {}) })
    return new Response(JSON.stringify(answer.body), { status: answer.status })
  }) as typeof fetch
  return { core, peers, say, fetcher, rung, setNow: (at: string) => { now = Date.parse(at) } }
}

async function aWorld(options: { readonly quiet?: { from: number; to: number }; readonly briefAt?: number } = {}) {
  const relay = aRelay()
  const pc = new FakePeer()
  relay.peers.push(pc)
  await relay.say(pc, { t: 'auth', as: 'pc', token: randomBytes(32).toString('base64url') })
  const token = randomBytes(32).toString('base64url')
  await relay.say(pc, { t: 'away-open', hash: hash(token), id: 'w' })
  // A phone that is not open, with its alerts kept at the relay.
  const device = randomBytes(16).toString('base64url')
  await relay.say(pc, { t: 'allow', device, hash: hash(randomBytes(32).toString('base64url')), id: 'a' })
  await relay.say(pc, { t: 'away-alerts', alerts: [{ device, endpoint: 'https://fcm.googleapis.com/fcm/send/x', found: 'F'.repeat(200), brief: 'B'.repeat(200) }], id: 'k' })
  const room = randomBytes(16).toString('base64url')
  const config: AwayConfig = { v: 1, relay: 'https://app.locust.lol', room, token, key: newAwayKey(), login: 'person', tz: 'America/New_York', ...options }
  const world: World = { checks: 'SUCCESS', issues: [], github: true }
  const asked: string[][] = []
  const gh = async (args: readonly string[]): Promise<string> => {
    asked.push([...args])
    if (!world.github) throw new Error('gh: could not resolve to a repository')
    if (args[0] === 'pr' && args.includes('--author')) {
      expect(args[args.indexOf('--author') + 1]).toBe('person')
      return JSON.stringify([{ number: 7, title: TITLE, url: PR_URL, state: 'OPEN', isDraft: false, mergeable: 'MERGEABLE', reviewDecision: '', updatedAt: '2026-10-23T12:00:00Z', statusCheckRollup: [{ __typename: 'CheckRun', status: 'COMPLETED', conclusion: world.checks }] }])
    }
    if (args[0] === 'pr') return '[]'
    if (args[0] === 'repo') return 'main\n'
    if (args[0] === 'run') return JSON.stringify([{ databaseId: 1, conclusion: 'success', status: 'completed', workflowName: 'Locust Scout', url: `https://github.com/${REPO}/actions/runs/1` }])
    if (args[0] === 'issue') return JSON.stringify(world.issues)
    return ''
  }
  let nowIso = '2026-10-23T14:00:00Z'
  const look = () => awayLook({ config, repo: REPO, gh, fetch: relay.fetcher, now: () => new Date(nowIso) })
  const at = (iso: string): void => { nowIso = iso; relay.setNow(iso) }
  /** The newest letter, opened as the PC would. */
  const newest = async (): Promise<AwayLetter | undefined> => {
    const got = (await relay.core.away({ method: 'GET', token })).body.latest as Letter | undefined
    return got === undefined ? undefined : openAwayLetter(config.key, got, room, got.from)
  }
  const leaveTold = async (letter: Omit<AwayLetter, 'v' | 'kind' | 'repo' | 'read' | 'found'>): Promise<void> => {
    const box = await sealAwayLetter(config.key, { v: 1, kind: 'told', repo: REPO, read: true, found: [], ...letter }, room, 'pc')
    await relay.say(pc, { t: 'post', ...box, id: 'p' })
  }
  const pcLeaves = async (): Promise<void> => { await relay.core.closed(pc); relay.peers.splice(relay.peers.indexOf(pc), 1) }
  return { relay, world, look, at, newest, leaveTold, pcLeaves, asked, config }
}

describe('Scout\'s away look tells only what is new', () => {
  it('does nothing more while Locust is open on the PC', async () => {
    const w = await aWorld()
    expect(await w.look()).toEqual({ said: 'Locust is open on your computer, so Scout looks from there.', ok: true })
    expect(w.asked).toEqual([])
  })

  it('only notes what is open the first time, then tells and rings for a check that starts failing', async () => {
    const w = await aWorld()
    await w.pcLeaves()
    w.world.checks = 'FAILURE'
    const first = await w.look()
    expect(first.said).toMatch(/^Scout’s first away look: it noted what is open now/)
    expect((await w.newest())?.found).toEqual([])
    expect(w.relay.rung).toEqual([])

    // Fixed, then failing again: news.
    w.world.checks = 'SUCCESS'
    w.at('2026-10-23T15:00:00Z')
    await w.look()
    w.world.checks = 'FAILURE'
    w.at('2026-10-23T16:00:00Z')
    const told = await w.look()
    expect(told).toEqual({ said: 'Scout looked: 1 new thing. It rang 1 phone.', ok: true })
    const letter = await w.newest()
    expect(letter?.found.map((one) => one.urgency)).toEqual(['now'])
    expect(letter?.found[0]?.url).toBe(PR_URL)
    expect(w.relay.rung).toEqual(['F'.repeat(200)])
    // Nothing from GitHub in what the workflow prints.
    expect(`${first.said} ${told.said}`).not.toContain(TITLE)

    // The same again: nothing new, nothing rung.
    w.at('2026-10-23T17:00:00Z')
    expect((await w.look()).said).toBe('Scout looked: nothing new.')
    expect(w.relay.rung).toHaveLength(1)
  })

  it('does not tell again what the PC told before it went off', async () => {
    const w = await aWorld()
    w.world.checks = 'FAILURE'
    // What Scout on the PC saw and told, left as it closed.
    await w.leaveTold({ at: '2026-10-23T13:00:00Z', prs: [{ number: 7, title: TITLE, url: PR_URL, state: 'open', checks: 'failing', checkCount: 1, notPassed: 1 }], told: [`pr:${PR_URL}:failing::`] })
    await w.pcLeaves()
    expect((await w.look()).said).toBe('Scout looked: nothing new.')
    w.world.issues = [{ number: 9, title: 'Crash on start', url: `https://github.com/${REPO}/issues/9`, createdAt: '2026-10-23T13:30:00Z', author: { login: 'someone' } }]
    w.at('2026-10-23T15:00:00Z')
    expect((await w.look()).said).toBe('Scout looked: 1 new thing.')
    expect((await w.newest())?.found.map((one) => one.key)).toEqual([expect.stringMatching(/^issue:/)])
    // An issue can wait: no phone rung.
    expect(w.relay.rung).toEqual([])
  })

  it('keeps quiet hours, in the person’s own time zone', async () => {
    // 10 PM to 7 AM in New York; 03:00 UTC is 11 PM there.
    const w = await aWorld({ quiet: { from: 22, to: 7 } })
    await w.leaveTold({ at: '2026-10-23T13:00:00Z', prs: [{ number: 7, title: TITLE, url: PR_URL, state: 'open', checks: 'passing', checkCount: 1, notPassed: 0 }], told: [] })
    await w.pcLeaves()
    w.world.checks = 'FAILURE'
    w.at('2026-10-24T03:00:00Z')
    expect((await w.look()).said).toBe('Scout looked: 1 new thing. Quiet hours: no phone was rung.')
    expect(w.relay.rung).toEqual([])
    expect(localTime(new Date('2026-10-24T03:00:00Z'), 'America/New_York')).toEqual({ day: '2026-10-23', hour: 23 })
  })

  it('brings the morning brief once a day from its hour on, and rings for it', async () => {
    const w = await aWorld({ briefAt: 7 })
    await w.leaveTold({ at: '2026-10-23T02:00:00Z', prs: [], told: [], briefDay: '2026-10-22' })
    await w.pcLeaves()
    // 6 AM in New York: not yet.
    w.at('2026-10-23T10:00:00Z')
    expect((await w.look()).said).toBe('Scout looked: nothing new.')
    w.at('2026-10-23T11:30:00Z')
    expect((await w.look()).said).toBe('Scout looked: nothing new. It brought the morning brief. It rang 1 phone.')
    const brief = (await w.newest())?.found.find((one) => one.key === 'brief:2026-10-23')
    expect(brief?.lines).toContain('Your computer is off, so Scout looked at GitHub only.')
    expect(w.relay.rung).toEqual(['B'.repeat(200)])
    w.at('2026-10-23T12:30:00Z')
    expect((await w.look()).said).toBe('Scout looked: nothing new.')
  })

  it('says GitHub could not be read, rings nothing, and keeps what was known for the next look', async () => {
    const w = await aWorld()
    await w.leaveTold({ at: '2026-10-23T13:00:00Z', prs: [{ number: 7, title: TITLE, url: PR_URL, state: 'open', checks: 'passing', checkCount: 1, notPassed: 0 }], told: ['x'] })
    await w.pcLeaves()
    w.world.github = false
    expect((await w.look()).said).toBe('Scout could not read GitHub this time.')
    const letter = await w.newest()
    expect(letter?.read).toBe(false)
    expect(letter?.prs.map((pr) => pr.number)).toEqual([7])
    expect(letter?.told).toEqual(['x'])
  })

  it('stops quietly once the away look is off in Locust', async () => {
    const w = await aWorld()
    const pc = w.relay.peers[0]!
    await w.relay.say(pc as FakePeer, { t: 'away-close', id: 'x' })
    expect((await w.look()).said).toMatch(/^Scout’s away look is off in Locust\./)
  })

  it('takes a letter’s links only to its own repository on GitHub, and a look only from the away look', () => {
    const letter = (url: string) => ({ v: 1, kind: 'look', at: '2026-10-23T14:00:00Z', repo: REPO, read: true, prs: [], told: [], found: [{ key: 'k', urgency: 'now', said: 'Something.', url }] })
    expect(awayLetterFrom(letter(`https://github.com/${REPO}/issues/1`), 'away')?.found[0]?.url).toBe(`https://github.com/${REPO}/issues/1`)
    for (const url of ['https://github.com/other/repo/issues/1', 'https://evil.example/acme/app', `https://github.com/${REPO}.evil/x`, `javascript:alert(1)//https://github.com/${REPO}/`]) {
      expect(awayLetterFrom(letter(url), 'away')?.found[0]?.url).toBeUndefined()
    }
    // A look is never the PC's, and what the PC told is never a look's.
    expect(awayLetterFrom(letter(''), 'pc')).toBeUndefined()
    expect(awayLetterFrom({ ...letter(''), kind: 'told' }, 'away')).toBeUndefined()
  })

  it('reads its settings only as Locust wrote them', () => {
    const good = { v: 1, relay: 'https://app.locust.lol', room: 'r'.repeat(22), token: 't'.repeat(43), key: 'k'.repeat(43), login: 'person', tz: 'UTC', quiet: { from: 22, to: 7 }, briefAt: 7 }
    expect(awayConfigOf(JSON.stringify(good))).toEqual(good)
    for (const bad of [undefined, '', 'not json', JSON.stringify({ ...good, v: 2 }), JSON.stringify({ ...good, relay: 'https://evil.example/path' }), JSON.stringify({ ...good, login: 'a b' })]) expect(awayConfigOf(bad)).toBeUndefined()
  })
})
