import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import type { FolderPullRequest } from '../shared/pull-request.js'
import { DEFAULT_SCOUT_SETTINGS, SCOUT_WAIT_MOST_MS, checkFacts, inQuietHours, scoutPrompt, scoutSettingsFrom, type FolderCheckResult, type ScoutState } from '../shared/scout.js'
import { createScout, type ScoutDeps, type ScoutGithub } from './scout.js'

/*
 * SCOUT KEEPS ITS GOAL, ITS HOURS AND ITS WORD (0.815, the first slice of the always-on plan). What it keeps watch for
 * is the person's own words, read into every look. Its own timer keeps quiet hours and waits for the person's own
 * turn, at most half an hour; a look the person asks for runs at once. The folder's check runs only when the folder
 * moved and nothing is working in it, and is news only when it turns. Off stops a look already under way, and today's
 * looks are counted, with how many needed its model.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })

const URL = 'https://github.com/acme/app/pull/212'
const failing = (change: Partial<FolderPullRequest> = {}): FolderPullRequest => ({ number: 212, title: 'Talk starts from any app', url: URL, state: 'open', checks: 'failing', checkCount: 9, notPassed: 2, ...change })
const red: FolderCheckResult = { command: 'npm test', passed: false, lines: ['FAIL cart.test.js', 'expected 6, got 0'] }
const green: FolderCheckResult = { command: 'npm test', passed: true, lines: [] }

async function harness(overrides: Partial<ScoutDeps> = {}) {
  const root = await mkdtemp(join(tmpdir(), 'locust-scout-slice-'))
  roots.push(root)
  let github: ScoutGithub | undefined = { prs: [], reviews: [] }
  let clock = new Date(2026, 9, 22, 14, 0, 0)
  const asked: string[] = []
  const stopped: string[] = []
  const notified: string[] = []
  let ticked: (() => void) | undefined
  const waits: (() => void)[] = []
  let person = false
  let mark = 'mark_1'
  let check: FolderCheckResult | undefined = red
  let checks = 0
  let busyFolder = false
  const teammates = new Set<string>()
  const deps: ScoutDeps = {
    file: join(root, 'scout.json'),
    now: () => clock,
    github: async () => github,
    waiting: async () => [],
    failedTurn: async () => undefined,
    team: async () => [{ teammateId: 'tm_scout_1', name: 'Scout', role: 'Keeps watch' }, { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }],
    ask: async (_teammateId, prompt) => { asked.push(prompt); return { missionId: `look_${String(asked.length)}`, runId: `run_${String(asked.length)}` } },
    stop: (runId) => { stopped.push(runId) },
    nextFreeModel: async () => undefined,
    answerOf: async () => ({ failed: 'It said nothing.' }),
    makeTeammate: async () => { teammates.add('tm_scout_1'); return 'tm_scout_1' },
    teammateExists: async (id) => teammates.has(id),
    changed: (_state: ScoutState) => undefined,
    notify: (card) => { notified.push(card.said) },
    schedule: (task) => { ticked = task; return 'timer' },
    clear: () => undefined,
    // A look waiting for the person's turn asks again on `later`; a look's own answer wait is never reached here.
    later: (task, ms) => { if (ms <= 60_000) waits.push(task); return 'later' },
    cancel: () => undefined,
    folderMark: async () => mark,
    runCheck: async () => { checks += 1; return check },
    folderBusy: () => busyFolder,
    personBusy: () => person,
    ...overrides
  }
  const scout = createScout(deps)
  return {
    scout, asked, stopped, notified,
    setGithub: (next: ScoutGithub | undefined) => { github = next },
    at: (hours: number, minutes = 0) => { clock = new Date(2026, 9, 22, hours, minutes, 0) },
    tick: (ms: number) => { clock = new Date(clock.getTime() + ms) },
    tickTimer: () => { ticked?.() },
    runWaits: () => { const now = waits.splice(0); for (const task of now) task() },
    waiting: () => waits.length,
    setPerson: (busy: boolean) => { person = busy },
    setMark: (next: string) => { mark = next },
    setCheck: (next: FolderCheckResult | undefined) => { check = next },
    setFolderBusy: (busy: boolean) => { busyFolder = busy },
    checks: () => checks
  }
}
const settled = async (): Promise<void> => { for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve)) }
/** A look started by its timer is not awaited by anyone: wait for what it does, writes to disk included. */
const until = async (ok: () => boolean): Promise<void> => { for (let i = 0; i < 300 && !ok(); i += 1) await new Promise((resolve) => setTimeout(resolve, 10)) }
const ROUTE = { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' }

describe('what to keep watch for', () => {
  it('is kept in the person’s words, trimmed, at most 600 characters, and cleared by an empty one', () => {
    const kept = scoutSettingsFrom({ goal: '  Keep main’s tests\n passing.  ' }, DEFAULT_SCOUT_SETTINGS)
    expect(kept.goal).toBe('Keep main’s tests passing.')
    expect(scoutSettingsFrom({ goal: 'x'.repeat(700) }, kept).goal).toHaveLength(600)
    expect(scoutSettingsFrom({ on: true }, kept).goal).toBe('Keep main’s tests passing.')
    expect(scoutSettingsFrom({ goal: '   ' }, kept)).not.toHaveProperty('goal')
  })

  it('is read into a look’s prompt, and only when there is one', async () => {
    const facts = [{ key: 'k', urgency: 'now' as const, said: 'A fact.' }]
    expect(scoutPrompt({ facts, team: [], goal: 'Keep main green.' })).toContain('The person asked you to keep watch for this: Keep main green.')
    expect(scoutPrompt({ facts, team: [] })).not.toContain('keep watch for this')
    const h = await harness()
    await h.scout.set({ goal: 'Keep main green.' })
    h.setGithub({ prs: [failing()], reviews: [] })
    await h.scout.turnOn(ROUTE)
    expect(h.asked[0]).toContain('The person asked you to keep watch for this: Keep main green.')
  })
})

describe('quiet hours', () => {
  it('cross midnight, and are none when from and to are the same hour', () => {
    const quiet = { from: 22, to: 7 }
    expect(inQuietHours(quiet, new Date(2026, 9, 22, 23, 30))).toBe(true)
    expect(inQuietHours(quiet, new Date(2026, 9, 22, 6, 59))).toBe(true)
    expect(inQuietHours(quiet, new Date(2026, 9, 22, 7, 0))).toBe(false)
    expect(inQuietHours({ from: 0, to: 8 }, new Date(2026, 9, 22, 12, 0))).toBe(false)
    expect(scoutSettingsFrom({ quiet: { from: 9, to: 9 } }, DEFAULT_SCOUT_SETTINGS)).not.toHaveProperty('quiet')
    expect(scoutSettingsFrom({ quiet: null }, { ...DEFAULT_SCOUT_SETTINGS, quiet })).not.toHaveProperty('quiet')
  })

  it('keep its own timer from looking, while Look now still looks, and nothing goes to Windows then', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ quiet: { from: 22, to: 7 } })
    h.at(23)
    h.setGithub({ prs: [failing()], reviews: [] })
    h.tickTimer()
    await settled()
    expect(h.asked).toHaveLength(0)
    // Asked for, it looks -- and the card waits on Home: nothing in Windows in quiet hours.
    await h.scout.look()
    expect(h.asked).toHaveLength(1)
    expect(h.notified).toEqual([])
  })
})

describe('waiting while the person works', () => {
  it('holds a due look while their own turn runs, and looks once it has ended', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setGithub({ prs: [failing()], reviews: [] })
    h.setPerson(true)
    h.tickTimer()
    await settled()
    expect(h.asked).toHaveLength(0)
    expect(h.waiting()).toBe(1)
    h.setPerson(false)
    h.tick(60_000)
    h.runWaits()
    await until(() => h.asked.length > 0)
    expect(h.asked).toHaveLength(1)
  })

  it('looks anyway after half an hour, so a long run never silences it', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    h.setGithub({ prs: [failing()], reviews: [] })
    h.setPerson(true)
    h.tickTimer()
    for (let minute = 0; minute < 31 && h.waiting() > 0; minute += 1) {
      expect(h.asked).toHaveLength(0)
      h.tick(60_000)
      h.runWaits()
      await settled()
    }
    await until(() => h.asked.length > 0)
    expect(h.asked).toHaveLength(1)
    expect(SCOUT_WAIT_MOST_MS).toBe(30 * 60_000)
  })

  it('does not wait when told not to', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ waitWhileWorking: false })
    h.setGithub({ prs: [failing()], reviews: [] })
    h.setPerson(true)
    h.tickTimer()
    await until(() => h.asked.length > 0)
    expect(h.asked).toHaveLength(1)
    expect(h.waiting()).toBe(0)
  })
})

describe('the folder’s check', () => {
  it('is told when it turns, and only then', () => {
    expect(checkFacts(undefined, green, 'a')).toEqual([])
    expect(checkFacts(undefined, red, 'a')[0]).toMatchObject({ urgency: 'now', said: 'The check npm test fails: FAIL cart.test.js' })
    expect(checkFacts({ passed: true }, red, 'b')[0]).toMatchObject({ urgency: 'now', said: 'The check npm test fails since the folder changed: FAIL cart.test.js' })
    expect(checkFacts({ passed: false }, red, 'c')).toEqual([])
    expect(checkFacts({ passed: false }, green, 'd')[0]).toMatchObject({ urgency: 'later', said: 'The check npm test passes again.' })
  })

  it('runs only when chosen, only when the folder moved, and never while a turn works there', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    expect(h.checks()).toBe(0)
    await h.scout.set({ watches: { github: true, teammates: true, check: true } })
    await h.scout.look()
    expect(h.checks()).toBe(1)
    expect((await h.scout.state()).cards[0]).toMatchObject({ said: 'The check npm test fails: FAIL cart.test.js', urgency: 'now' })
    // The same folder: not run again.
    await h.scout.look()
    expect(h.checks()).toBe(1)
    // Moved, but a turn is working there: not yet.
    h.setMark('mark_2')
    h.setFolderBusy(true)
    await h.scout.look()
    expect(h.checks()).toBe(1)
    // Quiet again: run, and it passes -- news.
    h.setFolderBusy(false)
    h.setCheck(green)
    await h.scout.look()
    expect(h.checks()).toBe(2)
    // How it stands now, and only that: the card saying it failed has gone.
    const cards = (await h.scout.state()).cards.filter((card) => card.key.startsWith('check:'))
    expect(cards.map((card) => card.said)).toEqual(['The check npm test passes again.'])
    expect(cards[0]?.urgency).toBe('later')
    // Good news, with nothing for a teammate to do: not put to Scout's model (0.818).
    expect(cards[0]).not.toHaveProperty('reading')
  })

  // 0.823, Grok's review: a turn that began while the check ran made its half-done work a failure needing the person.
  it('drops a result from a check a turn began beside, and runs it again when the folder is quiet', async () => {
    let busyDuringCheck = true
    const h = await harness({ runCheck: async () => { if (busyDuringCheck) h.setFolderBusy(true); return red } })
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ watches: { github: true, teammates: true, check: true } })
    await h.scout.look()
    expect((await h.scout.state()).cards.filter((card) => card.key.startsWith('check:'))).toEqual([])
    // The mark stayed: once the folder is quiet, the next look runs the check and keeps what it found.
    busyDuringCheck = false
    h.setFolderBusy(false)
    await h.scout.look()
    expect((await h.scout.state()).cards.filter((card) => card.key.startsWith('check:')).map((card) => card.said)).toEqual(['The check npm test fails: FAIL cart.test.js'])
  })
})

describe('off means off', () => {
  it('stops a look already under way, and its cards stay in Locust’s words', async () => {
    const h = await harness()
    h.setGithub({ prs: [failing()], reviews: [] })
    await h.scout.turnOn(ROUTE)
    expect((await h.scout.state()).cards[0]?.reading).toBe(true)
    const state = await h.scout.set({ on: false })
    expect(h.stopped).toEqual(['run_1'])
    expect(state.cards[0]).not.toHaveProperty('reading')
  })

  /*
   * 0.823, Grok's review: Off, and quiet hours, were read once as a look began. A look that then waited minutes on the
   * folder's check raised its cards, told Windows and asked the model after Scout was turned off or quiet hours began.
   */
  it('keeps nothing from a look turned off while it waited on the folder’s check', async () => {
    let during: () => Promise<void> = async () => undefined
    const h = await harness({ runCheck: async () => { await during(); return red } })
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ watches: { github: true, teammates: true, check: false } })
    h.setGithub({ prs: [], reviews: [] })
    await h.scout.set({ watches: { github: true, teammates: true, check: true } })
    h.setMark('mark_2')
    const asked = h.asked.length
    during = async () => { await h.scout.set({ on: false }) }
    await h.scout.look()
    expect(h.asked).toHaveLength(asked)
    expect(h.notified).toEqual([])
    expect((await h.scout.state()).cards.filter((card) => card.key.startsWith('check:'))).toEqual([])
  })

  it('keeps nothing from a timer look whose quiet hours began while it waited, and tells Windows nothing', async () => {
    let during: () => void = () => undefined
    const h = await harness({ runCheck: async () => { during(); return red } })
    await h.scout.turnOn(ROUTE)
    await h.scout.set({ quiet: { from: 22, to: 7 }, watches: { github: true, teammates: true, check: true } })
    h.at(21, 59)
    h.setMark('mark_9')
    const asked = h.asked.length
    during = () => { h.at(22, 4) }
    h.tickTimer()
    await settled()
    expect(h.asked).toHaveLength(asked)
    expect(h.notified).toEqual([])
    expect((await h.scout.state()).cards.filter((card) => card.key.startsWith('check:'))).toEqual([])
  })

  it('stops a look’s turn that was starting as Scout was turned off', async () => {
    let during: () => Promise<void> = async () => undefined
    let n = 0
    const h = await harness({ ask: async () => { n += 1; await during(); return { missionId: `look_${String(n)}`, runId: `run_${String(n)}` } } })
    h.setGithub({ prs: [failing()], reviews: [] })
    during = async () => { await h.scout.set({ on: false }) }
    await h.scout.turnOn(ROUTE)
    // Off found nothing waiting while the turn was starting: it is stopped the moment it is known.
    expect(h.stopped).toEqual(['run_1'])
    expect((await h.scout.state()).cards.some((card) => card.reading === true)).toBe(false)
  })
})

describe('what today used', () => {
  it('counts the looks and how many asked its model, and starts again the next day', async () => {
    const h = await harness()
    await h.scout.turnOn(ROUTE)
    await h.scout.look()
    expect((await h.scout.state()).today).toMatchObject({ looks: 2, asked: 0 })
    h.setGithub({ prs: [failing()], reviews: [] })
    await h.scout.look()
    expect((await h.scout.state()).today).toMatchObject({ looks: 3, asked: 1 })
    h.tick(24 * 60 * 60_000)
    expect((await h.scout.state()).today).toBeUndefined()
    await h.scout.look()
    expect((await h.scout.state()).today).toMatchObject({ looks: 1, asked: 0 })
  })
})
