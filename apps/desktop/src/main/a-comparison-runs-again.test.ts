import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { rerunMissionIds, rerunRefusalOf, type CompareRoute, type PublicCompare, type RerunProgress } from '../shared/compare.js'
import { createCompareReruns } from './compare-reruns.js'
import { createCompareStore, withColumnFolders } from './compare-store.js'
import { worktreeAddArgs } from './worktrees.js'

/*
 * YOUR OWN BENCHMARK (0.763): a comparison the judge has scored, run again once or three times more. One run can be
 * luck. Each run again is a comparison of the same models on the same question, from the same commit when it changes
 * files, scored by the same judge on the same points -- one after another, unattended, none a conversation of its own.
 */
const BASE = 'c'.repeat(40)
const ROUTES: readonly CompareRoute[] = [
  { runtime: 'opencode', model: 'opencode/space-bunny-free', label: 'Space Bunny' },
  { runtime: 'opencode', model: 'opencode/ling-3.1-flash-free', label: 'Ling 3.1' }
]
const JUDGE: CompareRoute = { runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', label: 'Muse Spark' }

// Real files, written and synced several times a run: ~200 ms a test here, 5 s and more on a machine busy with
// another session's video render (2026-10-12) -- the time limit is for a stall, not for that.
describe('a comparison run again', { timeout: 20_000 }, () => {
  let folder: string
  beforeEach(async () => {
    folder = await mkdtemp(join(tmpdir(), 'locust-reruns-'))
  })
  afterEach(async () => {
    await rm(folder, { recursive: true, force: true })
  })

  /** A scored comparison in a fresh store, and the runner around it, with every outside step a recorded fake. */
  const setUp = async (options: { readonly changes?: boolean; readonly refuseJudge?: string; readonly neverSettles?: boolean; readonly failFirst?: boolean; readonly panel?: readonly CompareRoute[]; readonly slowPanel?: boolean } = {}) => {
    let id = 0
    const compares = createCompareStore({ rootDirectory: folder, createId: () => `cmp_${String(++id).padStart(4, '0')}` })
    let origin = await compares.create({ teammateId: 'tm_wren', prompt: 'Explain the date bug.', routes: ROUTES, ...(options.changes === true ? { changes: true, base: BASE } : {}) })
    origin = await compares.addTurn(origin.compareId, 'a', 'mission_a1')
    origin = await compares.addTurn(origin.compareId, 'b', 'mission_b1')
    origin = await compares.judge(origin.compareId, JUDGE, 'mission_j1', 'Names the cause\nFixes the test', (options.panel ?? []).map((route, index) => ({ route, missionId: `mission_p1_${String(index)}` })))
    const calls: string[] = []
    const live = new Set<string>()
    const looks = new Map<string, number>()
    const told: { compares: readonly PublicCompare[]; progress: readonly RerunProgress[] }[] = []
    let missions = 0
    // The first answer of the first run fails, as OpenCode's own storage made one fail on the first 0.763 drive.
    const failing = 'mission_1'
    const reruns = createCompareReruns({
      compares,
      begin: async (compare) => {
        calls.push(`begin:${compare.compareId}`)
        for (const column of compare.slots) {
          const missionId = `mission_${String(++missions)}`
          await compares.addTurn(compare.compareId, column.slot, missionId)
          if (options.neverSettles === true) live.add(missionId)
        }
      },
      judge: async (compare, route, criteria, panel) => {
        calls.push(`judge:${compare.compareId}:${route.model}:${criteria ?? ''}`)
        if (panel.length > 0) calls.push(`panel:${compare.compareId}:${panel.map((one) => one.model).join(',')}`)
        if (options.refuseJudge !== undefined) return options.refuseJudge
        const members = panel.map((one) => ({ route: one, missionId: `mission_panel_${String(++missions)}` }))
        // A slow panel judge: still reading after the first is done, until the runner has looked twice.
        if (options.slowPanel === true) for (const member of members) live.add(member.missionId)
        await compares.judge(compare.compareId, route, `mission_judge_${String(++missions)}`, criteria, members)
        return undefined
      },
      live: (missionId) => {
        if (options.slowPanel === true && missionId.startsWith('mission_panel_')) {
          const looked = (looks.get(missionId) ?? 0) + 1
          looks.set(missionId, looked)
          if (looked === 1) calls.push(`waited:${missionId}`)
          if (looked > 2) live.delete(missionId)
        }
        return live.has(missionId)
      },
      phaseOf: async (missionId) => (options.failFirst === true && missionId === failing ? 'failed' : 'completed'),
      retry: async (compare, slot) => {
        calls.push(`retry:${compare.compareId}:${slot}`)
        await compares.retry(compare.compareId, slot, `mission_retried_${String(++missions)}`)
        return undefined
      },
      stopMission: (missionId) => {
        calls.push(`stop:${missionId}`)
        live.delete(missionId)
      },
      tidy: async (compare) => {
        calls.push(`tidy:${compare.compareId}`)
      },
      tell: (update) => told.push(update),
      sleep: async () => { await new Promise((done) => setTimeout(done, 1)) },
      pollMs: 1,
      runTimeoutMs: 60_000
    })
    const finished = async (): Promise<void> => {
      // Until every batch is over, however busy the machine is: a test about order must not race its own runner.
      for (let tries = 0; tries < 2_000 && reruns.progress().length > 0; tries += 1) await new Promise((done) => setTimeout(done, 5))
    }
    return { compares, origin, reruns, calls, live, told, finished }
  }

  it('holds a measured runs copies before judging, accepts only that run once, and skips automatic retries', async () => {
    const { origin, reruns, calls, finished } = await setUp({ failFirst: true })
    expect(reruns.checked(origin.compareId, 'cmp_wrong')).toBe(false)
    await reruns.start(origin.compareId, 1, true)
    for (let tries = 0; tries < 2_000 && reruns.progress()[0]?.step !== 'checking'; tries += 1) await new Promise((done) => setTimeout(done, 5))
    const waiting = reruns.progress()[0]!
    expect(waiting.step).toBe('checking')
    expect(calls.some((call) => /^(judge|tidy|retry):/.test(call))).toBe(false)
    expect(reruns.checked('cmp_wrong', waiting.compareId!)).toBe(false)
    expect(reruns.checked(origin.compareId, 'cmp_wrong')).toBe(false)
    expect(reruns.checked(origin.compareId, waiting.compareId!)).toBe(true)
    expect(reruns.checked(origin.compareId, waiting.compareId!)).toBe(false)
    await finished()
    expect(calls.some((call) => call.startsWith('judge:'))).toBe(true)
    expect(calls.some((call) => call.startsWith('tidy:'))).toBe(true)
  })

  it('stops a measured run waiting for checks without starting its judge or the next run', async () => {
    const { origin, reruns, calls, finished } = await setUp()
    await reruns.start(origin.compareId, 3, true)
    for (let tries = 0; tries < 2_000 && reruns.progress()[0]?.step !== 'checking'; tries += 1) await new Promise((done) => setTimeout(done, 5))
    expect(reruns.progress()[0]?.step).toBe('checking')
    const id = reruns.progress()[0]!.compareId!
    reruns.stop(origin.compareId)
    expect(reruns.checked(origin.compareId, id)).toBe(false)
    await finished()
    expect(calls.filter((call) => call.startsWith('begin:'))).toHaveLength(1)
    expect(calls.some((call) => call.startsWith('judge:'))).toBe(false)
    expect(calls).toContain(`tidy:${id}`)
  })

  it('runs it again as new comparisons of the same models and question, each judged by the same judge on the same points', async () => {
    const { compares, origin, reruns, calls, finished } = await setUp()
    const started = await reruns.start(origin.compareId, 3)
    expect(started.ok).toBe(true)
    await finished()
    const gathered = await compares.get(origin.compareId)
    expect(gathered?.reruns).toHaveLength(3)
    for (const rerunId of gathered?.reruns ?? []) {
      const rerun = await compares.get(rerunId)
      expect(rerun).toMatchObject({ rerunOf: origin.compareId, prompt: 'Explain the date bug.', teammateId: 'tm_wren', settled: true })
      expect(rerun?.slots.map((column) => column.route.model)).toEqual(ROUTES.map((route) => route.model))
      expect(calls).toContain(`judge:${rerunId}:${JUDGE.model}:Names the cause\nFixes the test`)
      expect(calls).toContain(`tidy:${rerunId}`)
    }
    // One after another: each run begins only once the last one's judge was asked.
    const order = calls.filter((call) => call.startsWith('begin:') || call.startsWith('judge:')).map((call) => call.split(':')[0])
    expect(order).toEqual(['begin', 'judge', 'begin', 'judge', 'begin', 'judge'])
  })

  it('judges each run again by the same three judges when three voted, and waits for all three before the next run', async () => {
    const LING: CompareRoute = { runtime: 'opencode', model: 'opencode/ling-3.1-flash-free', label: 'Ling 3.1' }
    const NEMO: CompareRoute = { runtime: 'opencode', model: 'opencode/nemotron-3-ultra-free', label: 'Nemotron' }
    const { compares, origin, reruns, calls, finished } = await setUp({ panel: [LING, NEMO], slowPanel: true })
    await reruns.start(origin.compareId, 3)
    await finished()
    const ids = (await compares.get(origin.compareId))?.reruns ?? []
    expect(ids).toHaveLength(3)
    for (const rerunId of ids) {
      expect(calls).toContain(`panel:${rerunId}:${LING.model},${NEMO.model}`)
      expect((await compares.get(rerunId))?.judge?.panel?.map((member) => member.route.model)).toEqual([LING.model, NEMO.model])
    }
    // The second run begins only after the first run's slow panel judges were waited for.
    const begins = calls.flatMap((call, index) => (call.startsWith('begin:') ? [index] : []))
    const firstWait = calls.findIndex((call) => call.startsWith('waited:'))
    expect(firstWait).toBeGreaterThan(-1)
    expect(firstWait).toBeLessThan(begins[1] ?? -1)
  })

  it('starts a run again of a comparison that changes files from the commit the first one started on', async () => {
    const { compares, origin, reruns, finished } = await setUp({ changes: true })
    await reruns.start(origin.compareId, 1)
    await finished()
    const rerunId = (await compares.get(origin.compareId))?.reruns?.[0]
    expect((await compares.get(rerunId))).toMatchObject({ changes: true, base: BASE })
    expect(worktreeAddArgs('/tree', 'locust/compare-x', false, 'linux', BASE)).toEqual(['worktree', 'add', '-b', 'locust/compare-x', '/tree', BASE])
    expect(() => worktreeAddArgs('/tree', 'locust/compare-x', false, 'linux', 'HEAD~1; rm -rf /')).toThrow()
  })

  it('says where it is while it runs, and nothing once it is done', async () => {
    const { origin, reruns, told, finished } = await setUp()
    await reruns.start(origin.compareId, 1)
    await finished()
    expect(told.some((update) => update.progress.some((one) => one.originId === origin.compareId && one.step === 'judging'))).toBe(true)
    expect(told.at(-1)?.progress).toEqual([])
  })

  it('stops the run under way where it is, and begins no more', async () => {
    const { compares, origin, reruns, calls, finished } = await setUp({ neverSettles: true })
    await reruns.start(origin.compareId, 3)
    for (let tries = 0; tries < 100 && !calls.some((call) => call.startsWith('begin:')); tries += 1) await new Promise((done) => setTimeout(done, 5))
    reruns.stop(origin.compareId)
    await finished()
    expect(calls.filter((call) => call.startsWith('begin:'))).toHaveLength(1)
    expect(calls.filter((call) => call.startsWith('stop:'))).toHaveLength(2)
    expect(calls.some((call) => call.startsWith('judge:'))).toBe(false)
    // Stopped, it is still over: its copies tidied, and it says it is settled.
    const rerunId = (await compares.get(origin.compareId))?.reruns?.[0]
    expect(calls).toContain(`tidy:${rerunId}`)
    expect((await compares.get(rerunId))?.settled).toBe(true)
  })

  it('asks a model whose answer failed once more, on the same model, before the judge reads', async () => {
    const { compares, origin, reruns, calls, finished } = await setUp({ failFirst: true })
    await reruns.start(origin.compareId, 1)
    await finished()
    const rerunId = (await compares.get(origin.compareId))?.reruns?.[0]
    expect(calls.filter((call) => call.startsWith('retry:'))).toEqual([`retry:${String(rerunId)}:a`])
    // Asked again before the judge, and the judge reads the answer that replaced it.
    expect(calls.indexOf(`retry:${String(rerunId)}:a`)).toBeLessThan(calls.findIndex((call) => call.startsWith('judge:')))
    expect((await compares.get(rerunId))?.slots[0]?.missionIds.at(-1)).toMatch(/^mission_retried_/)
  })

  it('goes on when a run cannot be judged, and leaves that run unscored', async () => {
    const { compares, origin, reruns, calls, finished } = await setUp({ refuseJudge: 'A judge needs at least two finished answers to read.' })
    await reruns.start(origin.compareId, 3)
    await finished()
    expect(calls.filter((call) => call.startsWith('begin:'))).toHaveLength(3)
    for (const rerunId of (await compares.get(origin.compareId))?.reruns ?? []) expect((await compares.get(rerunId))?.judge).toBeUndefined()
  })

  it('refuses in words what cannot be run again as it is', async () => {
    const { compares, origin, reruns, finished } = await setUp()
    expect(await reruns.start(origin.compareId, 2)).toMatchObject({ ok: false })
    expect(await reruns.start('cmp_gone', 1)).toMatchObject({ ok: false, message: 'That comparison is no longer here.' })
    const started = await reruns.start(origin.compareId, 3)
    expect(started.ok).toBe(true)
    expect(await reruns.start(origin.compareId, 1)).toMatchObject({ ok: false })
    reruns.stop(origin.compareId)
    await finished()
    const unjudged = await compares.create({ prompt: 'Which is faster?', routes: ROUTES })
    expect(rerunRefusalOf(unjudged)).toMatch(/Ask a judge first/)
    expect(rerunRefusalOf({ ...origin, blind: true })).toMatch(/Keep one first/)
    expect(rerunRefusalOf({ ...origin, changes: true })).toMatch(/only in a git project/)
    expect(rerunRefusalOf({ ...origin, slots: origin.slots.map((column) => ({ ...column, missionIds: ['m1', 'm2'] })) })).toMatch(/went on past its first question/)
    expect(rerunRefusalOf({ ...origin, rerunOf: 'cmp_other' })).toMatch(/a run of another comparison/)
    expect(rerunRefusalOf(origin)).toBeUndefined()
  })

  it('settles at a start a run Locust closed on, but not one whose turn is still going', async () => {
    const { compares, origin, reruns, calls, live } = await setUp()
    // As a batch left them when Locust closed: two runs again begun, neither over.
    const left = await compares.create({ teammateId: 'tm_wren', prompt: origin.prompt, routes: ROUTES, rerunOf: origin.compareId })
    const going = await compares.create({ teammateId: 'tm_wren', prompt: origin.prompt, routes: ROUTES, rerunOf: origin.compareId })
    await compares.addTurn(going.compareId, 'a', 'mission_still_going')
    live.add('mission_still_going')
    await reruns.settleLeftover()
    expect((await compares.get(left.compareId))?.settled).toBe(true)
    expect(calls).toContain(`tidy:${left.compareId}`)
    expect((await compares.get(going.compareId))?.settled).toBeUndefined()
    expect((await compares.get(origin.compareId))?.settled).toBeUndefined()
  })

  it('keeps its runs again out of the conversation list, and a settled one claims no copies', async () => {
    const { compares, origin, reruns, finished } = await setUp({ changes: true })
    await reruns.start(origin.compareId, 1)
    await finished()
    const all = await compares.list()
    const rerun = all.find((one) => one.rerunOf === origin.compareId)!
    const hidden = rerunMissionIds(all)
    for (const id of [...rerun.slots.flatMap((column) => column.missionIds), ...(rerun.judge?.missionIds ?? [])]) expect(hidden.has(id)).toBe(true)
    expect(hidden.has('mission_a1')).toBe(false)
    expect(withColumnFolders(rerun, '/copies').slots.every((column) => column.folder === undefined)).toBe(true)
  })
})
