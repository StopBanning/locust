import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { AWAY_ASK_KEY, newAwayKey, openAwayLetter, sealAwayLetter, type AwayLetter } from '../shared/away-letter.js'
import { createScout, type ScoutDeps } from './scout.js'
import { askOf, awayAsk, awayCommentOf, type AwayComment, type AwayConfig } from './scout-away-look.js'
import { AWAY_SCRIPT_PATH, AWAY_WORKFLOW_PATH, awayWorkflow, createScoutAway, type ScoutAwayRelay } from './scout-away.js'

/**
 * YOUR /LOCUST ASK WHILE YOUR COMPUTER IS OFF (0.838). The away look's workflow also runs when a comment is written;
 * the person's own /locust ask goes to their phone at once, sealed, and the PC -- once it is back -- makes the card a
 * teammate takes it from, never a second one. A repository on the older away look is offered the newer one by a pull
 * request.
 */
const ROOM = 'R'.repeat(22)
const REPO = 'acme/app'
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

const configOf = (key: string, change: Partial<AwayConfig> = {}): AwayConfig => ({ v: 1, relay: 'https://app.locust.lol', room: ROOM, token: 'T'.repeat(43), key, login: 'Person', tz: 'America/New_York', ...change })
const yours = (change: Partial<AwayComment> = {}): AwayComment => ({ id: 12, login: 'person', body: '/locust ada fix the flaky Mac test\nand say what broke', url: `https://github.com/${REPO}/pull/212#issuecomment-12`, number: 212, title: 'Talk starts from any app', ...change })

/** The relay's away door: what it holds, and what it was given. */
function aRelay(options: { readonly pc?: boolean; readonly latest?: { n: string; c: string; from: string } } = {}) {
  const posted: { n: string; c: string; alert?: string }[] = []
  const fetch = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
    if ((init?.method ?? 'GET') === 'GET') return new Response(JSON.stringify({ pc: options.pc === true, ...(options.latest === undefined ? {} : { latest: options.latest }) }), { status: 200 })
    posted.push(JSON.parse(String(init?.body)) as { n: string; c: string; alert?: string })
    return new Response(JSON.stringify({ rung: 1 }), { status: 200 })
  }) as unknown as typeof globalThis.fetch & { mock: { calls: unknown[] } }
  return { fetch, posted }
}
const gh = async (): Promise<string> => { throw new Error('An ask reads nothing from GitHub.') }

describe('your own /locust comment, with your computer off', () => {
  it('goes to your phone at once, sealed, ringing it', async () => {
    const key = newAwayKey()
    const before = await sealAwayLetter(key, { v: 1, kind: 'look', at: '2026-10-27T10:00:00Z', repo: REPO, read: true, found: [], prs: [], told: ['pr:a'], briefDay: '2026-10-27' }, ROOM, 'away')
    const relay = aRelay({ latest: { ...before, from: 'away' } })
    const result = await awayAsk({ config: configOf(key), repo: REPO, gh, fetch: relay.fetch, now: () => new Date('2026-10-27T15:00:00Z'), comment: yours() })
    expect(result).toEqual({ said: 'Scout passed your ask on. It rang 1 phone.', ok: true })
    expect(relay.posted).toHaveLength(1)
    expect(relay.posted[0]?.alert).toBe('found')
    const letter = await openAwayLetter(key, relay.posted[0]!, ROOM, 'away') as AwayLetter
    expect(letter.found).toEqual([expect.objectContaining({ key: `${AWAY_ASK_KEY}${REPO}:12`, urgency: 'now', said: 'You asked on #212 Talk starts from any app: ada fix the flaky Mac test', detail: 'ada fix the flaky Mac test\nand say what broke', url: yours().url })])
    // What was told before is carried on, so the next away look tells none of it again.
    expect(letter.told).toEqual(['pr:a', `${AWAY_ASK_KEY}${REPO}:12`])
    expect(letter.briefDay).toBe('2026-10-27')
    // Nothing the run prints names the issue or the ask.
    expect(result.said).not.toContain('Talk starts')
  })

  it('does nothing for anyone else’s comment, or one that asks Locust nothing', async () => {
    for (const comment of [yours({ login: 'stranger' }), yours({ body: 'Please /locust ada fix it' }), yours({ body: '/locust   ' }), yours({ url: 'https://github.com/other/app/pull/212#issuecomment-12' })]) {
      const relay = aRelay()
      const result = await awayAsk({ config: configOf(newAwayKey()), repo: REPO, gh, fetch: relay.fetch, comment })
      expect(result.ok).toBe(true)
      expect(relay.fetch).not.toHaveBeenCalled()
    }
  })

  it('leaves it to Locust when your computer is on', async () => {
    const relay = aRelay({ pc: true })
    expect(await awayAsk({ config: configOf(newAwayKey()), repo: REPO, gh, fetch: relay.fetch, comment: yours() })).toEqual({ said: 'Locust is open on your computer, so it reads your ask from there.', ok: true })
    expect(relay.posted).toEqual([])
  })

  it('is passed on once, and rings no phone in quiet hours', async () => {
    const key = newAwayKey()
    const told = await sealAwayLetter(key, { v: 1, kind: 'look', at: '2026-10-27T10:00:00Z', repo: REPO, read: true, found: [], prs: [], told: [`${AWAY_ASK_KEY}${REPO}:12`] }, ROOM, 'away')
    const again = aRelay({ latest: { ...told, from: 'away' } })
    expect((await awayAsk({ config: configOf(key), repo: REPO, gh, fetch: again.fetch, comment: yours() })).said).toBe('Your phone has this ask already.')
    expect(again.posted).toEqual([])
    const night = aRelay()
    // 03:00 in New York.
    await awayAsk({ config: configOf(key, { quiet: { from: 22, to: 7 } }), repo: REPO, gh, fetch: night.fetch, now: () => new Date('2026-10-27T07:00:00Z'), comment: yours() })
    expect(night.posted).toHaveLength(1)
    expect(night.posted[0]?.alert).toBeUndefined()
  })

  it('is read from the event GitHub gives the run, only when the comment was written', () => {
    const event = { action: 'created', comment: { id: 12, user: { login: 'person' }, body: '/locust go', html_url: yours().url }, issue: { number: 212, title: 'T' } }
    expect(awayCommentOf(event)).toEqual({ id: 12, login: 'person', body: '/locust go', url: yours().url, number: 212, title: 'T' })
    expect(awayCommentOf({ ...event, action: 'edited' })).toBeUndefined()
    expect(askOf('Some preface\n/locust fix it\nKeep the test.')).toBe('fix it\nKeep the test.')
  })

  it('runs the workflow for a comment, one run each, and ends anyone else’s at the job', () => {
    const workflow = awayWorkflow(1)
    expect(workflow).toContain('  issue_comment:\n    types: [created]\n')
    expect(workflow).toContain("if: github.event_name != 'issue_comment' || contains(github.event.comment.body, '/locust')")
    expect(workflow).toContain("group: locust-scout${{ github.event_name == 'issue_comment' && format('-ask-{0}', github.event.comment.id) || '' }}")
    // Still the workflow's own read-only token.
    expect(workflow).not.toMatch(/: write/)
  })
})

describe('when your computer is back', () => {
  it('makes no card from the phone’s line: the comment itself becomes the ask', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-away-ask-'))
    roots.push(root)
    const deps: ScoutDeps = {
      file: join(root, 'scout.json'), now: () => new Date('2026-10-27T16:00:00Z'), github: async () => undefined,
      waiting: async () => [], failedTurn: async () => undefined, team: async () => [],
      ask: vi.fn(async () => ({ refused: 'No model' })), stop: vi.fn(), nextFreeModel: async () => undefined,
      answerOf: async () => ({ failed: 'No answer' }), makeTeammate: async () => 'scout', teammateExists: async () => true,
      changed: vi.fn(), notify: vi.fn(), schedule: () => 1, clear: vi.fn(), later: () => 1, cancel: vi.fn()
    }
    const scout = createScout(deps)
    await scout.absorb([{
      v: 1, kind: 'look', at: '2026-10-27T15:00:00Z', repo: REPO, read: true, prs: [], told: [],
      found: [
        { key: `${AWAY_ASK_KEY}${REPO}:12`, urgency: 'now', said: 'You asked on #212: fix it' },
        { key: 'issue:acme/app:9', urgency: 'soon', said: 'A new issue: #9' }
      ]
    }])
    expect((await scout.state()).cards.map((card) => card.key)).toEqual(['issue:acme/app:9'])
    expect((await scout.state()).cards.some((card) => card.key.startsWith(AWAY_ASK_KEY))).toBe(false)
  })
})

describe('a repository on the older away look', () => {
  async function aDesk() {
    const root = await mkdtemp(join(tmpdir(), 'locust-away-update-'))
    roots.push(root)
    let script = '// the older away look\n'
    let onMain: { workflow: string; script: string } | undefined
    let prState = 'open'
    const trees: { path: string; content: string }[][] = []
    const titles: string[] = []
    const answer = (args: readonly string[], input?: string): string => {
      const line = args.join(' ')
      if (line.startsWith('repo view')) return JSON.stringify({ nameWithOwner: REPO, visibility: 'PUBLIC', defaultBranchRef: { name: 'main' }, viewerPermission: 'ADMIN' })
      if (line === 'api user --jq .login') return 'person\n'
      if (line.startsWith(`api repos/${REPO}/contents/${AWAY_WORKFLOW_PATH}?ref=main`)) { if (onMain === undefined) throw new Error('HTTP 404'); return line.includes('--jq') ? 'sha\n' : onMain.workflow }
      if (line.startsWith(`api repos/${REPO}/contents/${AWAY_SCRIPT_PATH}?ref=main`)) { if (onMain === undefined) throw new Error('HTTP 404'); return onMain.script }
      if (line === `api repos/${REPO}/git/ref/heads/main --jq .object.sha`) return 'base1\n'
      if (line === `api repos/${REPO}/git/commits/base1`) return JSON.stringify({ sha: 'base1', tree: { sha: 'tree0' } })
      if (line.includes('git/trees')) { trees.push((JSON.parse(input!) as { tree: { path: string; content: string }[] }).tree); return JSON.stringify({ sha: 'tree1' }) }
      if (line.includes('git/commits')) return JSON.stringify({ sha: 'commit1' })
      if (line.includes('git/refs')) return '{}'
      if (line.includes(`repos/${REPO}/pulls --input`)) { titles.push((JSON.parse(input!) as { title: string }).title); return JSON.stringify({ url: `https://github.com/${REPO}/pull/${String(11 + titles.length)}`, number: 11 + titles.length }) }
      if (line.startsWith(`api repos/${REPO}/pulls/`)) return `${prState}\n`
      return ''
    }
    const relay: ScoutAwayRelay = {
      ready: async () => undefined, room: () => ({ relay: 'https://app.locust.lol', room: ROOM }), connected: () => true,
      awayOpen: async () => true, awayClose: async () => true, keepAwayAlerts: async () => true, post: async () => true,
      letters: async () => ({ items: [], more: false })
    }
    const make = () => createScoutAway({
      file: join(root, 'scout-away.json'), folder: () => 'C:/work/app',
      gh: async (args) => answer(args), ghInput: async (args, _cwd, input) => answer(args, input),
      relay: () => relay,
      scout: { told: async () => ({ prs: [], told: [] }), absorb: async () => undefined, hours: async () => ({}) },
      script: async () => script, tz: () => 'America/New_York', now: () => new Date('2026-10-27T14:00:00Z')
    })
    return {
      make, trees, titles,
      merge: () => { onMain = { workflow: awayWorkflow(1), script } },
      newerLocust: () => { script = '// the newer away look, that passes on /locust asks\n' },
      closePr: () => { prState = 'closed' }
    }
  }

  it('is offered the newer one by a pull request, and is on it once that is merged', async () => {
    const desk = await aDesk()
    await desk.make().set(true)
    desk.merge()
    expect(await desk.make().status(true)).toMatchObject({ state: 'on', repo: REPO })
    expect(await desk.make().status()).not.toHaveProperty('outdated')

    desk.newerLocust()
    const away = desk.make()
    expect(await away.status()).toMatchObject({ state: 'on', outdated: true })
    const opened = await away.set('update')
    expect(opened).toMatchObject({ state: 'on', update: `https://github.com/${REPO}/pull/13` })
    expect(opened).not.toHaveProperty('outdated')
    expect(desk.titles.at(-1)).toBe('Scout’s away look, updated from Locust')
    expect(desk.trees.at(-1)).toEqual([
      expect.objectContaining({ path: AWAY_WORKFLOW_PATH, content: awayWorkflow(1) }),
      expect.objectContaining({ path: AWAY_SCRIPT_PATH, content: '// the newer away look, that passes on /locust asks\n' })
    ])
    // Not merged yet: it waits, and asks again.
    expect(await away.status(true)).toMatchObject({ state: 'on', update: `https://github.com/${REPO}/pull/13` })
    desk.merge()
    const after = await away.status(true)
    expect(after).toMatchObject({ state: 'on', repo: REPO })
    expect(after).not.toHaveProperty('update')
    expect(after).not.toHaveProperty('outdated')
  })

  it('goes on as it was when the update is closed without merging, and can be offered again', async () => {
    const desk = await aDesk()
    await desk.make().set(true)
    desk.merge()
    await desk.make().status(true)
    desk.newerLocust()
    const away = desk.make()
    await away.set('update')
    desk.closePr()
    expect(await away.status(true)).toMatchObject({ state: 'on', outdated: true, message: 'The update’s pull request was closed without merging, so the away look goes on as it was.' })
  })
})
