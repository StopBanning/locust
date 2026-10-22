import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { briefFact, scoutSettingsFrom, DEFAULT_SCOUT_SETTINGS } from '../shared/scout.js'
import type { WaitingCard } from '../shared/scout.js'
import { createScout, type ScoutDeps } from './scout.js'

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
    // Its model reads it like any news, against the goal when there is one.
    expect(h.asked.at(-1)).toContain('Good morning. Here is where things stand.')
    await h.scout.look()
    expect(await briefs(h.scout)).toHaveLength(1)
    // The next day: a new brief, in yesterday's place, counting from yesterday's.
    h.at(23, 9, 0)
    await h.scout.look()
    const second = await briefs(h.scout)
    expect(second.map((card) => card.key)).toEqual(['brief:2026-10-23'])
    expect(h.since.at(-1)?.since).toBe(new Date(2026, 9, 22, 8, 15).toISOString())
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
