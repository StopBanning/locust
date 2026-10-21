/**
 * YOUR OWN BENCHMARK (0.763): a comparison run again. The canvas "Your Own Benchmark" (56riCzZ3ZzR5AQ2sg6ssJm), after
 * Artificial Analysis's Optima, which grades models on your own tasks; Colin, of its calls: trust your judgement.
 *
 * One run can be luck. A comparison the judge has scored can be run again, once or three times more: each run is a new
 * comparison of the same models on the same question -- from the same commit, when it changes files -- scored by the
 * same judge on the same points. They run one after another, unattended, and none is a conversation of its own: the
 * comparison they run again gathers them, and shows each model's score, time and cost across all of them.
 *
 * Each run waits for every one of its models to finish before its judge reads them, and for its judge before the next
 * run begins. A run that never settles is stopped after `runTimeoutMs` and the batch goes on. Its copies of the project
 * are removed once it is over, as Keep removes a comparison's other copies.
 */

import { newestJudgeMissionIds, rerunRefusalOf, RERUN_TIMES, type CompareRoute, type PublicCompare, type RerunProgress } from '../shared/compare.js'
import type { CompareStore } from './compare-store.js'

type RerunStep = RerunProgress['step']

export interface CompareReruns {
  /** Runs it again `times` more (one of RERUN_TIMES); answers at once, while the runs go on. */
  start(originId: string, times: number, waitForChecks?: boolean): Promise<{ readonly ok: true; readonly compare: PublicCompare } | { readonly ok: false; readonly message: string }>
  /** Release only this measured run to its saved judge, never a new route. */
  checked(originId: string, compareId: string): boolean
  /** The runs stop: the one under way is stopped where it is, and no more begin. */
  stop(originId: string): void
  progress(): readonly RerunProgress[]
  /**
   * At a start (0.764): a batch is held in memory only, so a run Locust closed on was neither running nor over -- its
   * chip said "waiting" for good. Each run again not yet over, with nothing of it running here, is over now: its copies
   * removed and settled, so it reads as scored, or as not scored and why.
   */
  settleLeftover(): Promise<void>
}

export function createCompareReruns(options: {
  readonly compares: Pick<CompareStore, 'list' | 'get' | 'create' | 'addRerun' | 'settle'>
  /** Every column asked the question (index.ts askEveryColumn); a column that cannot start says so in its place. */
  readonly begin: (compare: PublicCompare) => Promise<void>
  /** The judge asked (index.ts judgeCompare), with the two beside it when three vote (0.765): undefined once it started, else why it did not. */
  readonly judge: (compare: PublicCompare, route: CompareRoute, criteria: string | undefined, panel: readonly CompareRoute[]) => Promise<string | undefined>
  /** Whether a mission's run is still going here. */
  readonly live: (missionId: string) => boolean
  /** How a mission ended, as its record says: `completed`, `failed`, `cancelled`. */
  readonly phaseOf: (missionId: string) => Promise<string | undefined>
  /** One column asked again, on the same model (index.ts startCompareColumn): undefined once it started. */
  readonly retry: (compare: PublicCompare, slot: PublicCompare['slots'][number]['slot']) => Promise<string | undefined>
  /** A mission still going, stopped. */
  readonly stopMission: (missionId: string) => void
  /** Its copies of the project, removed once it is over. */
  readonly tidy: (compare: PublicCompare) => Promise<void>
  /** The window is told: comparisons that changed, and where every batch is. */
  readonly tell: (update: { readonly compares: readonly PublicCompare[]; readonly progress: readonly RerunProgress[] }) => void
  readonly sleep?: (ms: number) => Promise<void>
  readonly pollMs?: number
  readonly runTimeoutMs?: number
  readonly now?: () => number
}): CompareReruns {
  const sleep = options.sleep ?? ((ms: number) => new Promise<void>((done) => setTimeout(done, ms)))
  const pollMs = options.pollMs ?? 3_000
  const runTimeoutMs = options.runTimeoutMs ?? 45 * 60_000
  const now = options.now ?? (() => Date.now())
  const store = options.compares
  type Batch = { progress: RerunProgress; stopped: boolean; waitForChecks: boolean; checked: boolean }
  const batches = new Map<string, Batch>()

  const progress = (): readonly RerunProgress[] => [...batches.values()].map((batch) => batch.progress)
  const tell = (compares: readonly PublicCompare[]): void => {
    try {
      options.tell({ compares, progress: progress() })
    } catch {
      // The window being gone stops nothing here.
    }
  }

  /** Until every mission named is no longer running, the batch is stopped, or the time is up: whether all settled. */
  const settled = async (ids: () => Promise<readonly string[]>, batch: { stopped: boolean }, until: number): Promise<boolean> => {
    for (;;) {
      const running = (await ids()).filter((id) => options.live(id))
      if (running.length === 0) return true
      if (batch.stopped || now() > until) {
        for (const id of running) options.stopMission(id)
        return false
      }
      await sleep(pollMs)
    }
  }

  const runOnce = async (origin: PublicCompare, run: number, times: number, batch: Batch): Promise<void> => {
    const rerun = await store.create({
      ...(origin.teammateId === undefined ? {} : { teammateId: origin.teammateId }),
      prompt: origin.prompt,
      routes: origin.slots.map((column) => column.route),
      ...(origin.changes === true ? { changes: true } : {}),
      ...(origin.changesIn === undefined ? {} : { changesIn: origin.changesIn }),
      ...(origin.base === undefined ? {} : { base: origin.base }),
      rerunOf: origin.compareId
    })
    const gathered = await store.addRerun(origin.compareId, rerun.compareId)
    batch.progress = { originId: origin.compareId, run, of: times, step: 'answering', compareId: rerun.compareId }
    tell([gathered, rerun])
    const until = now() + runTimeoutMs
    const current = async (): Promise<PublicCompare> => (await store.get(rerun.compareId)) ?? rerun
    try {
      await options.begin(rerun)
      tell([await current()])
      // Every column's answer, then the judge's: what it reads is what they finished with.
      const newest = async (): Promise<readonly string[]> => (await current()).slots.flatMap((column) => column.missionIds.slice(-1))
      let answered = await settled(newest, batch, until)
      /*
       * A model whose answer FAILED is asked once more, on the same model, before the judge reads (the canvas: "tried
       * once more on the same model, then counted as did not run, not as a failure of the model"). The first 0.763
       * drive: two OpenCode columns in one folder at once, and one died on OpenCode's own "Failed to execute
       * statement" -- its storage, not the model.
       */
      // A measured run is one attempt, not an unbudgeted retry hiding a failed turn.
      if (answered && !batch.stopped && !batch.waitForChecks) {
        const failed: PublicCompare['slots'][number]['slot'][] = []
        for (const column of (await current()).slots) {
          const id = column.missionIds.at(-1)
          if (id !== undefined && (await options.phaseOf(id).catch(() => undefined)) === 'failed') failed.push(column.slot)
        }
        for (const slot of failed) await options.retry(await current(), slot).catch(() => undefined)
        if (failed.length > 0) answered = await settled(newest, batch, until)
      }
      if (answered && !batch.stopped && batch.waitForChecks) {
        batch.checked = false
        batch.progress = { ...batch.progress, step: 'checking' }
        tell([await current()])
        while (!batch.checked && !batch.stopped && now() <= until) await sleep(pollMs)
        if (!batch.checked || batch.stopped || now() > until) answered = false
      }
      const judge = origin.judge
      if (answered && !batch.stopped && judge !== undefined) {
        batch.progress = { ...batch.progress, step: 'judging' }
        tell([await current()])
        const why = await options.judge(await current(), judge.route, judge.criteria, (judge.panel ?? []).map((member) => member.route))
        if (why === undefined) {
          tell([await current()])
          // Every judge of it: three voting are read only once all three are done.
          await settled(async () => newestJudgeMissionIds((await current()).judge), batch, until)
        }
      }
    } finally {
      const over = await current()
      await options.tidy(over).catch(() => undefined)
      tell([await store.settle(rerun.compareId).catch(() => over)])
    }
  }

  return {
    start: async (originId, times, waitForChecks = false) => {
      if (!(RERUN_TIMES as readonly number[]).includes(times)) return { ok: false, message: 'Run it again once, or three times.' }
      const origin = await store.get(originId).catch(() => undefined)
      if (origin === undefined) return { ok: false, message: 'That comparison is no longer here.' }
      const why = rerunRefusalOf(origin)
      if (why !== undefined) return { ok: false, message: why }
      if (batches.has(origin.compareId)) return { ok: false, message: 'It is running again already. Stop it, or wait for it to finish.' }
      const batch: Batch = { progress: { originId: origin.compareId, run: 1, of: times, step: 'answering' as RerunStep }, stopped: false, waitForChecks, checked: false }
      batches.set(origin.compareId, batch)
      tell([])
      void (async () => {
        for (let run = 1; run <= times && !batch.stopped; run += 1) {
          await runOnce(origin, run, times, batch).catch(() => undefined)
        }
      })().finally(() => {
        batches.delete(origin.compareId)
        tell([])
      })
      return { ok: true, compare: origin }
    },
    checked: (originId, compareId) => {
      const batch = batches.get(originId)
      if (batch === undefined || batch.stopped || batch.progress.step !== 'checking' || batch.progress.compareId !== compareId || batch.checked) return false
      batch.checked = true
      return true
    },
    stop: (originId) => {
      const batch = batches.get(originId)
      if (batch !== undefined) batch.stopped = true
    },
    progress,
    settleLeftover: async () => {
      const running = new Set([...batches.values()].flatMap((batch) => (batch.progress.compareId === undefined ? [] : [batch.progress.compareId])))
      for (const compare of await store.list().catch(() => [] as readonly PublicCompare[])) {
        if (compare.rerunOf === undefined || compare.settled === true || running.has(compare.compareId)) continue
        const ids = [...compare.slots.flatMap((column) => column.missionIds), ...(compare.judge?.missionIds ?? []), ...(compare.judge?.panelMissionIds ?? [])]
        // A turn that outlived Locust (a background run) is left to finish; the next start looks again.
        if (ids.some((id) => options.live(id))) continue
        await options.tidy(compare).catch(() => undefined)
        const over = await store.settle(compare.compareId).catch(() => undefined)
        if (over !== undefined) tell([over])
      }
    }
  }
}
