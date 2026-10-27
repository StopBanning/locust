import { afterEach, describe, expect, it, vi } from 'vitest'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { COMMENT_EVERY_MS, createScout, type ScoutDeps } from './scout.js'
import type { CommentWatch } from './scout-comments.js'
import { DEFAULT_SCOUT_SETTINGS, commentAskFact, type ScoutFact } from '../shared/scout.js'

/**
 * /LOCUST WITHIN A MINUTE (0.838). A comment asking Locust waited for Scout's next look, ten minutes to an hour away.
 * Your comments are read every minute on their own, with no model; a new ask starts a look at once.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })
const at = '2026-10-27T12:00:00.000Z'
const team = [{ teammateId: 'ada', name: 'Ada', role: 'Code' }]
const ask = { id: '12', repository: 'acme/app', number: 212, url: 'https://github.com/acme/app/pull/212#issuecomment-12', message: 'fix the flaky Mac test', namedTeammateId: 'ada' }
const asked = commentAskFact(ask, 'Talk starts from any app', 'fix the flaky Mac test')

async function harness(overrides: Partial<ScoutDeps> = {}) {
  const root = await mkdtemp(join(tmpdir(), 'locust-minute-'))
  roots.push(root)
  const timers: { task: () => void; ms: number }[] = []
  const github = vi.fn(async () => undefined)
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'), now: () => new Date(at), github,
    waiting: async () => [], failedTurn: async () => undefined, team: async () => team,
    ask: vi.fn(async () => ({ refused: 'No model' })), stop: vi.fn(), nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'No answer' }), makeTeammate: async () => 'scout', teammateExists: async () => true,
    changed: vi.fn(), notify: vi.fn(), schedule: (task, ms) => { timers.push({ task, ms }); return timers.length }, clear: vi.fn(), later: () => 1, cancel: vi.fn(),
    ...overrides
  }
  const scout = createScout(deps)
  /** Runs the minute's read the way its timer does, and waits for what it started. */
  const minute = async (): Promise<void> => {
    const tick = timers.filter((timer) => timer.ms === COMMENT_EVERY_MS).at(-1)
    if (tick === undefined) throw new Error('no minute timer')
    tick.task()
    await new Promise((done) => setTimeout(done, 50))
  }
  return { deps, scout, timers, github, minute }
}

describe('your /locust comments', () => {
  it('are read every minute while Scout is on, apart from its looks', async () => {
    const h = await harness({ comments: async (watch) => ({ watch, facts: [] }) })
    await h.scout.turnOn({ runtime: 'opencode', model: 'free' })
    expect(h.timers.map((timer) => timer.ms)).toContain(COMMENT_EVERY_MS)
    expect(h.timers.map((timer) => timer.ms)).toContain(DEFAULT_SCOUT_SETTINGS.everyMinutes * 60_000)
  })

  it('become a card within the minute, not at the next look', async () => {
    let facts: ScoutFact[] = []
    const h = await harness({ comments: async (watch) => ({ watch, facts }) })
    await h.scout.turnOn({ runtime: 'opencode', model: 'free' })
    expect((await h.scout.state()).cards).toHaveLength(0)
    facts = [asked]
    await h.minute()
    const cards = (await h.scout.state()).cards
    expect(cards).toHaveLength(1)
    expect(cards[0]?.draft).toEqual({ teammateId: 'ada', name: 'Ada', message: ask.message })
  })

  it('cost one read and no model in a minute with nothing new', async () => {
    const comments = vi.fn(async (watch: CommentWatch) => ({ watch: { ...watch, repositories: { 'acme/app': { since: at, page: 1, seen: [] } } }, facts: [] }))
    const h = await harness({ comments })
    await h.scout.turnOn({ runtime: 'opencode', model: 'free' })
    const looked = h.github.mock.calls.length
    const read = comments.mock.calls.length
    await h.minute()
    expect(comments.mock.calls.length).toBe(read + 1)
    expect(h.github.mock.calls.length).toBe(looked)
    expect(h.deps.ask).not.toHaveBeenCalled()
    // Where reading reached is kept, so the next minute reads on from there.
    expect(JSON.parse(await readFile(h.deps.file, 'utf8')).comments.repositories['acme/app'].since).toBe(at)
  })

  it('are raised once, by the minute or by a look, never both', async () => {
    let facts: ScoutFact[] = []
    const h = await harness({ comments: async (watch) => ({ watch, facts }) })
    await h.scout.turnOn({ runtime: 'opencode', model: 'free' })
    facts = [asked]
    await h.minute()
    await h.scout.look()
    await h.minute()
    expect((await h.scout.state()).cards).toHaveLength(1)
  })

  it('are not read with Scout off or its comment watch off', async () => {
    const comments = vi.fn(async (watch: CommentWatch) => ({ watch, facts: [] }))
    const h = await harness({ comments })
    await h.scout.turnOn({ runtime: 'opencode', model: 'free' })
    await h.scout.set({ watches: { ...DEFAULT_SCOUT_SETTINGS.watches, comments: false } })
    const read = comments.mock.calls.length
    await h.minute().catch(() => undefined)
    expect(comments.mock.calls.length).toBe(read)
    expect(h.deps.clear).toHaveBeenCalled()
  })
})
