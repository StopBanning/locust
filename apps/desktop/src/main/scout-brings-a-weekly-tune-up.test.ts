import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import type { FolderPullRequest } from '../shared/pull-request.js'
import { daysBetween, emptyWeek, watchOfKey, weekFrom } from '../shared/scout.js'
import { createScout, type ScoutDeps } from './scout.js'

/*
 * SCOUT BRINGS A WEEKLY TUNE-UP (0.817; the always-on plan's last step). Locust counts a week of looks and what became
 * of each card -- sent on, changed first, dismissed, or left -- and the first look a week on brings one card with the
 * count and, by Locust's own rule, one change to Scout's settings that a press applies.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

const failing = (number: number): FolderPullRequest => ({ number, title: `Fix ${String(number)}`, url: `https://github.com/acme/app/pull/${String(number)}`, state: 'open', checks: 'failing', checkCount: 9, notPassed: 2 })
const ROUTE = { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' }
const DAY_ONE = new Date(2026, 9, 22, 14, 0, 0)
const WEEK_ON = new Date(2026, 9, 29, 9, 0, 0)

async function harness() {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-week-'))
  roots.push(root)
  let prs: FolderPullRequest[] = []
  let at = DAY_ONE
  const prompts: string[] = []
  const teammates = new Set<string>()
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'),
    now: () => at,
    github: async () => ({ prs, reviews: [] }),
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [{ teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }],
    ask: async (_teammateId, prompt) => { prompts.push(prompt); return { missionId: `look_${String(prompts.length)}`, runId: `run_${String(prompts.length)}` } },
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
    cancel: () => undefined
  }
  const scout = createScout(deps)
  const cardsOf = async (prefix: string) => (await scout.state()).cards.filter((card) => card.key.startsWith(prefix))
  const onDisk = async () => (JSON.parse(await readFile(deps.file, 'utf8')) as { week?: unknown }).week
  return { scout, deps, cardsOf, onDisk, prompts, setPrs: (next: FolderPullRequest[]) => { prs = next }, moveTo: (next: Date) => { at = next } }
}

describe('Scout’s weekly tune-up', () => {
  it('counts a week, and the first look seven days on brings it as one card; the next week starts with that look', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setPrs([failing(1), failing(2), failing(3)])
    await h.scout.look()
    const raised = await h.cardsOf('pr:')
    expect(raised).toHaveLength(3)
    await h.scout.dismiss(raised[0]!.cardId)
    await h.scout.dismiss(raised[1]!.cardId, 'sent')
    // Six days on is not a week.
    h.moveTo(new Date(2026, 9, 28, 23, 0, 0))
    await h.scout.look()
    expect(await h.cardsOf('week:')).toHaveLength(0)
    h.moveTo(WEEK_ON)
    await h.scout.look()
    const [week] = await h.cardsOf('week:')
    expect(week?.said).toBe('This week: 3 looks, 1 of them read by its model.')
    expect(week?.lines).toEqual(['It raised 3 cards. You sent 1 on, dismissed 1 and left 1.'])
    expect(week?.tune).toBeUndefined()
    expect(week?.urgency).toBe('later')
    // Locust's own card: not read by Scout's model, so it carries no take or draft, and that look asked none.
    expect(week?.reading).toBeUndefined()
    expect(week?.draft).toBeUndefined()
    expect(await h.onDisk()).toMatchObject({ from: '2026-10-29', looks: 1, asked: 0, sent: 0, dismissed: 0 })
    // Kept across a restart, and never said twice.
    const again = createScout(h.deps)
    await again.look()
    expect((await again.state()).cards.filter((card) => card.key.startsWith('week:'))).toHaveLength(1)
  })

  it('offers to stop watching GitHub when its cards were dismissed three times and never used, and the press does it', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setPrs([failing(1), failing(2), failing(3), failing(4)])
    await h.scout.look()
    for (const card of await h.cardsOf('pr:')) await h.scout.dismiss(card.cardId)
    h.moveTo(WEEK_ON)
    await h.scout.look()
    const [week] = await h.cardsOf('week:')
    expect(week?.lines).toEqual(['It raised 4 cards. You dismissed 4.', 'You dismissed all 4 GitHub cards and sent none on.'])
    expect(week?.tune).toEqual({ label: 'Stop watching GitHub', watchOff: 'github' })
    const after = await h.scout.tune(week!.cardId)
    // GitHub goes off; the rest stay as they were, Your /locust comments (0.823, on by default) among them.
    expect(after.settings.watches).toEqual({ github: false, teammates: true, check: false, comments: true })
    expect(after.cards.some((card) => card.key.startsWith('week:'))).toBe(false)
  })

  it('does not offer it when one card was used, sent on or changed first', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setPrs([failing(1), failing(2), failing(3), failing(4)])
    await h.scout.look()
    const [first, ...rest] = await h.cardsOf('pr:')
    await h.scout.dismiss(first!.cardId, 'changed')
    for (const card of rest) await h.scout.dismiss(card.cardId)
    h.moveTo(WEEK_ON)
    await h.scout.look()
    const [week] = await h.cardsOf('week:')
    expect(week?.lines).toEqual(['It raised 4 cards. You changed 1 first and dismissed 3.'])
    expect(week?.tune).toBeUndefined()
  })

  it('is never put to Scout’s model: beside other news it is left out of the look’s prompt', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.moveTo(WEEK_ON)
    h.setPrs([failing(7)])
    await h.scout.look()
    expect(h.prompts).toHaveLength(1)
    expect(h.prompts[0]).toContain('Your pull request #7')
    expect(h.prompts[0]).not.toContain('This week:')
    const [week] = await h.cardsOf('week:')
    const [pr] = await h.cardsOf('pr:')
    expect(week?.reading).toBeUndefined()
    expect(pr?.reading).toBe(true)
  })

  it('offers looking every 30 minutes after a week that needed nothing', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.moveTo(WEEK_ON)
    await h.scout.look()
    const [week] = await h.cardsOf('week:')
    expect(week?.said).toBe('This week: 1 look, none of them needing its model.')
    expect(week?.lines).toEqual(['Nothing needed you.', 'Looking every 30 minutes would have found the same, with fewer looks.'])
    expect((await h.scout.tune(week!.cardId)).settings.everyMinutes).toBe(30)
  })

  it('counts only the cards a week raised, and lets a tally two weeks old go without a card', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setPrs([failing(1)])
    await h.scout.look()
    h.moveTo(WEEK_ON)
    await h.scout.look()
    // Last week's card, dismissed this week, is not this week's.
    const [old] = await h.cardsOf('pr:')
    await h.scout.dismiss(old!.cardId)
    expect(await h.onDisk()).toMatchObject({ from: '2026-10-29', dismissed: 0 })
    h.moveTo(new Date(2026, 10, 13, 9, 0, 0))
    await h.scout.look()
    expect((await h.cardsOf('week:')).map((card) => card.key)).toEqual(['week:2026-10-22'])
    expect(await h.onDisk()).toMatchObject({ from: '2026-11-13', looks: 1 })
  })

  it('keeps its tally whole or not at all, and counts days by the calendar', () => {
    expect(weekFrom(emptyWeek('2026-10-22'))).toEqual(emptyWeek('2026-10-22'))
    expect(weekFrom({ ...emptyWeek('2026-10-22'), looks: -1 })).toBeUndefined()
    expect(weekFrom({ ...emptyWeek('2026-10-22'), watches: { github: { raised: 1, used: 0, dismissed: 0 } } })).toBeUndefined()
    expect(weekFrom('a week')).toBeUndefined()
    // A day the clocks go back is still one day.
    expect(daysBetween('2026-10-31', '2026-11-01')).toBe(1)
    expect(daysBetween('2026-10-22', '2026-10-29')).toBe(7)
    expect([watchOfKey('pr:https://x:failing::'), watchOfKey('review:1'), watchOfKey('waiting:a'), watchOfKey('check:m:fails'), watchOfKey('brief:2026-10-22'), watchOfKey('week:2026-10-22')])
      .toEqual(['github', 'github', 'teammates', 'check', undefined, undefined])
  })
})
