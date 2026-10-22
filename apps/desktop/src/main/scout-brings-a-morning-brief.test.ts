import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { briefFact, briefSpan, scoutSettingsFrom, DEFAULT_SCOUT_SETTINGS } from '../shared/scout.js'
import type { WaitingCard } from '../shared/scout.js'
import { createScout, type ScoutDeps, type ScoutGithub } from './scout.js'

/*
 * SCOUT BRINGS A MORNING BRIEF (0.816). Once a day, with the first look from its hour on and never in quiet hours: who
 * waits on the person, what ran since the last brief, the check, pull requests and the cards still waiting -- every
 * line counted by Locust. A new brief takes yesterday's place.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

async function harness(overrides: Partial<ScoutDeps> = {}) {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-brief-'))
  roots.push(root)
  let clock = new Date(2026, 9, 22, 6, 30, 0)
  let waiting: WaitingCard[] = []
  const since: { since: string; looks: readonly string[] }[] = []
  const asked: string[] = []
  const teammates = new Set<string>()
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'),
    now: () => clock,
    github: async () => ({ prs: [], reviews: [] }),
    waiting: async () => waiting,
    failedTurn: async () => undefined,
    team: async () => [{ teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }],
    ask: async (_teammateId, prompt) => { asked.push(prompt); return { missionId: `look_${String(asked.length)}`, runId: `run_${String(asked.length)}` } },
    stop: () => undefined,
    nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'It said nothing.' }),
    makeTeammate: async () => { teammates.add('tm_scout_1'); return 'tm_scout_1' },
    teammateExists: async (id) => teammates.has(id),
    changed: () => undefined,
    notify: () => undefined,
    schedule: () => 'timer',
    clear: () => undefined,
    later: () => 'later',
    cancel: () => undefined,
    briefFacts: async (from, looks) => { since.push({ since: from, looks }); return { finished: 4, failed: 1 } },
    ...overrides
  }
  return {
    scout: createScout(deps), asked, since,
    at: (day: number, hours: number, minutes = 0) => { clock = new Date(2026, 9, day, hours, minutes, 0) },
    setWaiting: (next: WaitingCard[]) => { waiting = next }
  }
}
const ROUTE = { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' }
const briefs = async (scout: Awaited<ReturnType<typeof harness>>['scout']) => (await scout.state()).cards.filter((card) => card.key.startsWith('brief:'))

describe('the morning brief', () => {
  it('says each count in Locust’s words, and one sentence when nothing happened', () => {
    const full = briefFact({
      waiting: [{ name: 'Ada', minutes: 190 }, { minutes: 20 }],
      finished: 4, failed: 1,
      check: { command: 'npm test', passed: false },
      prs: { open: 3, needsWork: 1 },
      cards: { total: 2, now: 1 }
    }, '2026-10-22')
    expect(full).toMatchObject({ key: 'brief:2026-10-22', urgency: 'soon', said: 'Good morning. Here is where things stand.' })
    expect(full.lines).toEqual([
      'Waiting on you: Ada, 3 h; a teammate, 20 min.',
      'Since yesterday: 4 turns finished, 1 failed.',
      'The check npm test fails.',
      'Your pull requests: 3 open, 1 needing work.',
      'Scout has 2 more cards for you, 1 needing you now.'
    ])
    const quiet = briefFact({ waiting: [], finished: 0, failed: 0, cards: { total: 0, now: 0 } }, '2026-10-22')
    expect(quiet).toMatchObject({ urgency: 'later', said: 'Good morning. Nothing waits on you, and nothing ran since yesterday.' })
    expect(quiet).not.toHaveProperty('lines')
  })

  it('is kept as an hour Settings offers, and turned off with null', () => {
    expect(scoutSettingsFrom({ briefAt: 7 }, DEFAULT_SCOUT_SETTINGS).briefAt).toBe(7)
    expect(scoutSettingsFrom({ briefAt: 3 }, DEFAULT_SCOUT_SETTINGS)).not.toHaveProperty('briefAt')
    expect(scoutSettingsFrom({ briefAt: null }, { ...DEFAULT_SCOUT_SETTINGS, briefAt: 7 })).not.toHaveProperty('briefAt')
  })

  it('comes with the first look from its hour on, once a day, counting since the last', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ briefAt: 7 })
    await h.scout.look()
    expect(await briefs(h.scout)).toEqual([])
    h.at(22, 8, 15)
    h.setWaiting([{ approvalId: 'a1', missionId: 'm1', name: 'Ada', summary: 'npm install', requestedAt: new Date(2026, 9, 22, 7, 45).toISOString() }])
    await h.scout.look()
    const first = await briefs(h.scout)
    expect(first).toHaveLength(1)
    expect(first[0]?.lines).toEqual(['Waiting on you: Ada, 30 min.', 'Since yesterday: 4 turns finished, 1 failed.'])
    // Scout's own looks are named, so they are never counted as turns that ran.
    expect(h.since.at(-1)?.looks).toEqual([])
    // Locust's own count: its model is asked about the turn waiting on you, never about the brief (0.818).
    expect(h.asked.at(-1)).toContain('Ada has waited 30 minutes for your OK.')
    expect(h.asked.at(-1)).not.toContain('Good morning.')
    expect(first[0]).not.toHaveProperty('reading')
    await h.scout.look()
    expect(await briefs(h.scout)).toHaveLength(1)
    // The next day: a new brief, in yesterday's place, counting from yesterday's.
    h.at(23, 9, 0)
    await h.scout.look()
    const second = await briefs(h.scout)
    expect(second.map((card) => card.key)).toEqual(['brief:2026-10-23'])
    expect(h.since.at(-1)?.since).toBe(new Date(2026, 9, 22, 8, 15).toISOString())
  })

  /*
   * 0.823, Grok's review: the span was always "Since yesterday", a capped count was silent, GitHub's line came from the
   * last list when GitHub could not be read, a check the folder moved past still said how it stands, and a check that
   * turned was counted as two cards.
   */
  it('names the span it counts, says "at least" past its cut, and says when a check is out of date', () => {
    expect(briefSpan(new Date(2026, 9, 22, 8, 15), new Date(2026, 9, 23, 9, 0))).toBe('Since yesterday')
    expect(briefSpan(new Date(2026, 9, 23, 6, 30), new Date(2026, 9, 26, 6, 30))).toBe('Since Friday')
    expect(briefSpan(new Date(2026, 9, 12, 6, 30), new Date(2026, 9, 26, 6, 30))).toBe('Since October 12')
    const brief = briefFact({ waiting: [], finished: 80, failed: 2, more: true, span: 'Since Friday', check: { command: 'npm test', passed: true, stale: true }, cards: { total: 0, now: 0 } }, '2026-10-26')
    expect(brief.lines).toEqual(['Since Friday: at least 80 turns finished, 2 failed.', 'The check npm test passed when it last ran; the folder has changed since.'])
    expect(briefFact({ waiting: [], finished: 0, failed: 0, span: 'Since Friday', cards: { total: 0, now: 0 } }, '2026-10-26').said).toBe('Good morning. Nothing waits on you, and nothing ran since Friday.')
  })

  it('after a weekend asleep, Monday’s brief says since Friday, and leaves out GitHub it could not read', async () => {
    let github: ScoutGithub | undefined = { prs: [{ number: 7, title: 'One', url: 'https://github.com/acme/app/pull/7', state: 'open', checks: 'passing', checkCount: 1, notPassed: 0 }], reviews: [] }
    const h = await harness({ github: async () => github })
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ briefAt: 7 })
    h.at(23, 8, 0)
    await h.scout.look()
    expect((await briefs(h.scout))[0]?.lines).toContain('Your pull requests: 1 open.')
    // Asleep from Friday morning; GitHub cannot be read on Monday.
    github = undefined
    h.at(26, 8, 0)
    await h.scout.look()
    const monday = await briefs(h.scout)
    expect(monday.map((card) => card.key)).toEqual(['brief:2026-10-26'])
    expect(monday[0]?.lines).toContain('Since Friday: 4 turns finished, 1 failed.')
    expect(monday[0]?.lines?.some((line) => line.startsWith('Your pull requests'))).toBe(false)
  })

  it('waits for quiet hours to end', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ briefAt: 6, quiet: { from: 22, to: 7 } })
    h.at(22, 6, 30)
    await h.scout.look()
    expect(await briefs(h.scout)).toEqual([])
    h.at(22, 7, 5)
    await h.scout.look()
    expect(await briefs(h.scout)).toHaveLength(1)
  })
})
