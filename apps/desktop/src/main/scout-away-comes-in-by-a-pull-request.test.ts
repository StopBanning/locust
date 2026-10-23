import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { afterEach, describe, expect, it } from 'vitest'

import type { Letter } from '../../../relay/src/protocol.js'
import { openAwayLetter, sealAwayLetter, type AwayLetter } from '../shared/away-letter.js'
import { createScout, type ScoutDeps, type ScoutGithub } from './scout.js'
import { awayConfigOf } from './scout-away-look.js'
import { AWAY_SCRIPT_PATH, AWAY_WORKFLOW_PATH, awayWorkflow, createScoutAway, type ScoutAwayRelay } from './scout-away.js'

/**
 * SCOUT'S AWAY LOOK COMES IN BY A PULL REQUEST (0.829). Turned on, Locust lets the look into the relay, puts what it
 * needs in the repository's secret -- on gh's stdin, never its command line -- and opens a pull request with the
 * workflow and the one file it runs; it is on once merged. When the PC is back, the away looks' letters go to Scout
 * in order, and only theirs; off undoes all of it. And Scout puts what they found on the Board, told already.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })
const ROOM = 'R'.repeat(22)
const SCRIPT = '// the away look\nconsole.log(1)\n'

interface Call { readonly args: readonly string[]; readonly input?: string }

async function aDesk(options: { readonly permission?: string; readonly visibility?: string } = {}) {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-away-'))
  roots.push(root)
  const calls: Call[] = []
  let merged = false
  let prState = 'open'
  let secret: string | undefined
  const answer = (args: readonly string[], input?: string): string => {
    const line = args.join(' ')
    if (line.startsWith('repo view')) return JSON.stringify({ nameWithOwner: 'acme/app', visibility: options.visibility ?? 'PRIVATE', defaultBranchRef: { name: 'main' }, viewerPermission: options.permission ?? 'ADMIN' })
    if (line === 'api user --jq .login') return 'person\n'
    if (line.startsWith('secret set')) { secret = input; return '' }
    if (line.startsWith(`api repos/acme/app/contents/${AWAY_WORKFLOW_PATH}?ref=main`) || line.startsWith(`api repos/acme/app/contents/${AWAY_SCRIPT_PATH}?ref=main`)) {
      if (!merged) throw new Error('HTTP 404: Not Found')
      return line.includes('--jq') ? 'sha\n' : ''
    }
    if (line === 'api repos/acme/app/git/ref/heads/main --jq .object.sha') return 'base1\n'
    if (line === 'api repos/acme/app/git/commits/base1') return JSON.stringify({ sha: 'base1', tree: { sha: 'tree0' } })
    if (line.includes('git/trees')) return JSON.stringify({ sha: 'tree1' })
    if (line.includes('git/commits')) return JSON.stringify({ sha: 'commit1' })
    if (line.includes('git/refs')) return '{}'
    if (line.includes('repos/acme/app/pulls --input')) return JSON.stringify({ url: 'https://github.com/acme/app/pull/12', number: 12 })
    if (line.startsWith('api repos/acme/app/pulls/12')) return `${prState}\n`
    return ''
  }
  const gh = async (args: readonly string[]): Promise<string> => { calls.push({ args }); return answer(args) }
  const ghInput = async (args: readonly string[], _cwd: string, input: string): Promise<string> => { calls.push({ args, input }); return answer(args, input) }
  const opened: string[] = []
  const posted: { n: string; c: string }[] = []
  let letters: Letter[] = []
  let closed = 0
  let kept = 0
  let room: string | undefined = ROOM
  const relay: ScoutAwayRelay = {
    ready: async () => undefined,
    room: () => (room === undefined ? undefined : { relay: 'https://app.locust.lol', room }),
    connected: () => true,
    awayOpen: async (hash) => { opened.push(hash); return true },
    awayClose: async () => { closed += 1; return true },
    keepAwayAlerts: async () => { kept += 1; return true },
    post: async (box) => { posted.push(box); return true },
    letters: async (after) => {
      const rest = after === undefined ? letters : letters.filter((letter) => letter.id > after)
      return { items: rest.slice(0, 2), more: rest.length > 2 }
    }
  }
  let toldNow: { prs: never[]; told: string[]; briefDay?: string } = { prs: [], told: ['pr:a'] }
  const absorbed: AwayLetter[][] = []
  const away = createScoutAway({
    file: join(root, 'scout-away.json'),
    folder: () => 'C:/work/app',
    gh, ghInput,
    relay: () => relay,
    scout: { told: async () => toldNow, absorb: async (got) => { absorbed.push([...got]) }, hours: async () => ({ quiet: { from: 22, to: 7 }, briefAt: 7 }) },
    script: async () => SCRIPT,
    tz: () => 'America/New_York',
    now: () => new Date('2026-10-23T14:00:00Z')
  })
  return {
    away, calls, opened, posted, absorbed, root,
    secret: () => secret,
    merge: () => { merged = true },
    closePr: () => { prState = 'closed' },
    setLetters: (next: Letter[]) => { letters = next },
    setTold: (next: typeof toldNow) => { toldNow = next },
    loseRoom: () => { room = undefined },
    closed: () => closed,
    kept: () => kept
  }
}

const letterOf = async (key: string, id: string, letter: Partial<AwayLetter> = {}, from: 'away' | 'pc' = 'away'): Promise<Letter> => {
  const box = await sealAwayLetter(key, { v: 1, kind: from === 'pc' ? 'told' : 'look', at: `2026-10-23T1${id}:00:00Z`, repo: 'acme/app', read: true, found: [], prs: [], told: [], ...letter }, ROOM, from)
  return { id: `${String(1_800_000_000_000 + Number(id)).padStart(13, '0')}-aaaaaaaa`, at: 0, from, ...box }
}

describe('Scout\'s away look comes in by a pull request', () => {
  it('cannot be turned on without the phone’s way from anywhere', async () => {
    const desk = await aDesk()
    desk.loseRoom()
    const status = await desk.away.set(true)
    expect(status).toEqual({ state: 'off', blocked: 'Turn on “Your phone, from anywhere” first: Scout leaves what it finds there.', message: 'Turn on “Your phone, from anywhere” first: Scout leaves what it finds there.' })
    expect(desk.calls).toEqual([])
  })

  it('needs an owner or admin of the repository, who can add its secret', async () => {
    const desk = await aDesk({ permission: 'WRITE' })
    expect((await desk.away.set(true))).toMatchObject({ state: 'off', message: 'Only an owner or admin of acme/app can add the secret the away look needs.' })
    expect(desk.opened).toEqual([])
  })

  it('puts its settings in the secret on stdin only, and opens a pull request with the workflow and its file', async () => {
    const desk = await aDesk()
    const status = await desk.away.set(true)
    expect(status).toEqual({ state: 'waiting', repo: 'acme/app', pr: 'https://github.com/acme/app/pull/12' })

    const config = awayConfigOf(desk.secret())!
    expect(config).toMatchObject({ v: 1, relay: 'https://app.locust.lol', room: ROOM, login: 'person', tz: 'America/New_York', quiet: { from: 22, to: 7 }, briefAt: 7 })
    // The relay was given the token's hash, never the token.
    expect(desk.opened).toEqual([expect.not.stringContaining(config.token)])
    expect(desk.opened[0]).toMatch(/^[A-Za-z0-9_-]{43}$/)
    for (const call of desk.calls) for (const arg of call.args) { expect(arg).not.toContain(config.token); expect(arg).not.toContain(config.key) }

    const tree = JSON.parse(desk.calls.find((call) => call.args.some((arg) => arg.endsWith('git/trees')))!.input!) as { base_tree: string; tree: { path: string; content: string }[] }
    expect(tree.base_tree).toBe('tree0')
    expect(tree.tree).toEqual([
      expect.objectContaining({ path: AWAY_WORKFLOW_PATH, content: awayWorkflow(2) }),
      expect.objectContaining({ path: AWAY_SCRIPT_PATH, content: SCRIPT })
    ])
    // A private repository: every two hours, and its cost said.
    expect(awayWorkflow(2)).toContain("cron: '23 */2 * * *'")
    const pr = JSON.parse(desk.calls.find((call) => call.args.some((arg) => arg.endsWith('/pulls')))!.input!) as { head: string; base: string; body: string }
    expect(pr).toMatchObject({ head: 'locust/scout-away', base: 'main' })
    expect(pr.body).toContain('about 360 of the 2,000 free Actions minutes')
    // What Scout told already, for the first away look; and the phones' alerts kept.
    const told = await openAwayLetter(config.key, desk.posted[0]!, ROOM, 'pc')
    expect(told).toMatchObject({ kind: 'told', repo: 'acme/app', told: ['pr:a'] })
    expect(desk.kept()).toBe(1)
  })

  it('is on once the pull request is merged, and off when it is closed unmerged', async () => {
    const desk = await aDesk({ visibility: 'PUBLIC' })
    await desk.away.set(true)
    expect(await desk.away.status(true)).toMatchObject({ state: 'waiting' })
    desk.merge()
    expect(await desk.away.status(true)).toEqual({ state: 'on', repo: 'acme/app', every: 1, private: false })

    const other = await aDesk()
    await other.away.set(true)
    other.closePr()
    expect(await other.away.status(true)).toMatchObject({ state: 'off', message: 'The pull request was closed without merging, so Scout does not look while your computer is off.' })
    expect(other.calls.map((call) => call.args.slice(0, 2).join(' '))).toContain('secret delete')
    expect(other.closed()).toBe(1)
  })

  it('gives Scout the away looks’ letters in order, a page at a time, and only theirs', async () => {
    const desk = await aDesk()
    await desk.away.set(true)
    desk.merge()
    await desk.away.status(true)
    const key = awayConfigOf(desk.secret())!.key
    desk.setLetters([
      await letterOf(key, '1', { found: [{ key: 'issue:1', urgency: 'soon', said: 'There is a new issue.' }] }),
      await letterOf(key, '2', {}, 'pc'),
      await letterOf(key, '3', { repo: 'other/repo' }),
      await letterOf('k'.repeat(43), '4'),
      await letterOf(key, '5', { found: [{ key: 'pr:2', urgency: 'now', said: 'A check failed.' }] })
    ])
    await desk.away.catchUp()
    expect(desk.absorbed.map((batch) => batch.map((letter) => letter.at))).toEqual([['2026-10-23T11:00:00Z', '2026-10-23T15:00:00Z']])
    expect(await desk.away.status()).toMatchObject({ state: 'on', lastLook: '2026-10-23T15:00:00Z' })
    // Read once: the cursor moved past them.
    await desk.away.catchUp()
    expect(desk.absorbed).toHaveLength(1)
    const saved = JSON.parse(await readFile(join(desk.root, 'scout-away.json'), 'utf8')) as { cursor: string }
    expect(saved.cursor).toMatch(/^\d{13}-aaaaaaaa$/)
  })

  it('leaves what Scout told only when it changed', async () => {
    const desk = await aDesk()
    await desk.away.set(true)
    expect(desk.posted).toHaveLength(1)
    desk.away.told()
    await desk.away.status()
    expect(desk.posted).toHaveLength(1)
    desk.setTold({ prs: [], told: ['pr:a', 'issue:9'] })
    desk.away.told()
    await desk.away.status()
    expect(desk.posted).toHaveLength(2)
  })

  it('off: the relay lets it go, the secret is deleted, and the merged workflow is turned off', async () => {
    const desk = await aDesk()
    await desk.away.set(true)
    desk.merge()
    await desk.away.status(true)
    expect(await desk.away.forPhone()).toEqual({ key: awayConfigOf(desk.secret())!.key, repo: 'acme/app' })
    expect(await desk.away.set(false)).toEqual({ state: 'off' })
    const said = desk.calls.map((call) => call.args.slice(0, 2).join(' '))
    expect(said).toEqual(expect.arrayContaining(['secret delete', 'workflow disable']))
    expect(desk.closed()).toBe(1)
    expect(await desk.away.forPhone()).toBeUndefined()
  })

  it('stops, and says why, when the phones were turned off and their room is gone', async () => {
    const desk = await aDesk()
    await desk.away.set(true)
    desk.loseRoom()
    expect(await desk.away.status()).toMatchObject({ state: 'off', message: 'Your phones were turned off, so Scout stopped looking while your computer is off. Turn it on again here.' })
  })
})

async function scoutHarness() {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-absorb-'))
  roots.push(root)
  const teammates = new Set<string>()
  let raised = 0
  let notified = 0
  const github: ScoutGithub = { prs: [], reviews: [] }
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'),
    logFile: join(root, 'scout-looks.json'),
    now: () => new Date(2026, 9, 23, 18, 0, 0),
    github: async () => github,
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [],
    ask: async () => ({ refused: 'No model in this test.' }),
    stop: () => undefined,
    nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'It said nothing.' }),
    makeTeammate: async () => { teammates.add('tm_scout_1'); return 'tm_scout_1' },
    teammateExists: async (id) => teammates.has(id),
    changed: () => undefined,
    notify: () => { notified += 1 },
    raised: () => { raised += 1 },
    schedule: () => 'timer',
    clear: () => undefined,
    later: () => 'later',
    cancel: () => undefined
  }
  return { scout: createScout(deps), raised: () => raised, notified: () => notified }
}

describe('what the away looks found goes on the Board', () => {
  it('as cards told already, said nowhere else, each look in What Scout saw', async () => {
    const h = await scoutHarness()
    await h.scout.turnOn({ runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' })
    const pr = { number: 7, title: 'Fix', url: 'https://github.com/acme/app/pull/7', state: 'open' as const, checks: 'failing' as const, checkCount: 2, notPassed: 1 }
    const look = (at: string, found: AwayLetter['found'], extra: Partial<AwayLetter> = {}): AwayLetter => ({ v: 1, kind: 'look', at, repo: 'acme/app', read: true, found, prs: [pr], told: [], ...extra })
    const state = await h.scout.absorb([
      look('2026-10-23T15:00:00Z', [{ key: 'pr:7:x', urgency: 'now', said: 'Your pull request #7, Fix: 1 of 2 checks did not pass.', url: pr.url }]),
      look('2026-10-23T11:00:00Z', [{ key: 'brief:2026-10-23', urgency: 'later', said: 'Good morning. Here is where things stand.', lines: ['Your computer is off, so Scout looked at GitHub only.'] }], { briefDay: '2026-10-23' }),
      look('2026-10-23T16:00:00Z', [], { read: false })
    ])
    expect(state.cards.map((card) => card.key)).toEqual(expect.arrayContaining(['pr:7:x', 'brief:2026-10-23']))
    const card = state.cards.find((one) => one.key === 'pr:7:x')!
    expect(card.chips).toContainEqual({ text: 'while your PC was off', tone: 'quiet' })
    expect(card.open).toEqual({ kind: 'url', url: pr.url, label: 'Open on GitHub' })
    expect(h.raised()).toBe(0)
    expect(h.notified()).toBe(0)
    const { entries } = await h.scout.log()
    const away = entries.filter((entry) => entry.away === true)
    expect(away.map((entry) => entry.at)).toEqual(['2026-10-23T11:00:00.000Z', '2026-10-23T15:00:00.000Z', '2026-10-23T16:00:00.000Z'])
    expect(away.at(-1)?.githubUnread).toBe(true)

    // Told, so neither the same letters again nor Scout's next look here tell it twice.
    const again = await h.scout.absorb([look('2026-10-23T15:00:00Z', [{ key: 'pr:7:x', urgency: 'now', said: 'Again.' }])])
    expect(again.cards.filter((one) => one.key === 'pr:7:x')).toHaveLength(1)
    expect((await h.scout.told()).told).toEqual(expect.arrayContaining(['pr:7:x', 'brief:2026-10-23']))
    expect((await h.scout.told()).briefDay).toBe('2026-10-23')
    expect((await h.scout.told()).prs).toEqual([pr])
  })
})

describe('the away look file', () => {
  it('is what its source builds to', async () => {
    const script = pathToFileURL(join(__dirname, '..', '..', 'scripts', 'build-scout-away.mjs')).href
    const { builtScoutAway } = await import(/* @vite-ignore */ script) as { builtScoutAway: () => Promise<string> }
    const kept = (await readFile(join(__dirname, '..', '..', 'resources', 'scout-away.mjs'), 'utf8')).replace(/\r\n/g, '\n')
    expect(kept).toBe(await builtScoutAway())
  })
})
