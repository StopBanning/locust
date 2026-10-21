import { useEffect, useRef } from 'react'
import type { ReactElement } from 'react'

import type { RerunProgress } from '../../../shared/compare.js'
import type { BenchmarkChart, BenchmarkRow } from '../benchmark.js'
import { costCell } from '../cost.js'
import { durationText } from '../missionView.js'
import { WorkingSpark } from './WorkingSpark.js'

/**
 * YOUR OWN BENCHMARK, IN THE COMPARISON (0.763, main/compare-reruns.ts). The canvas "Your Own Benchmark", after
 * Artificial Analysis's Optima: a comparison the judge has scored can be run again, and every run of it is read as one
 * table -- score, the runs that met every point, time a task and cost a task -- with score against cost on a chart and
 * a teammate's model set from a row in one press. Before any run again it is one invitation: one run can be luck.
 */
export interface BenchmarkView {
  /** Why it cannot be run again as it is, said in place of the buttons. */
  readonly refusal?: string
  /** Three judges vote on it (0.765): each run again is scored by the same three. */
  readonly threeJudges?: boolean
  readonly rows: readonly BenchmarkRow[]
  readonly chart?: BenchmarkChart
  /** Every run, this comparison first; `done` once its models and its judge have finished. */
  readonly runs: readonly { readonly compareId: string; readonly state: string; readonly done: boolean }[]
  /** Where its runs again are, while they run. */
  readonly progress?: RerunProgress
  /** It changes files: the commit every run starts from. */
  readonly base?: string
  /** The teammate whose comparison it is, whose model a row can set. */
  readonly teammateName?: string
  /** That teammate's model now, `runtime:model`: its row says so rather than offering it. */
  readonly teammateKey?: string
  /** A run again being asked for, or a model being set: no second press. */
  readonly busy: boolean
  readonly using?: string
  readonly problem?: string
  readonly onRun: (times: number) => void
  readonly onStop: () => void
  readonly onOpenRun: (compareId: string) => void
  readonly onUse?: (row: BenchmarkRow) => void
  /** Opened for it -- from Benchmarks, or back from one of its runs (0.764): it is scrolled to, not left below the columns. */
  readonly reveal?: boolean
}

/**
 * Where a run again is (0.810). It said "Run 1 of 1" beside "YOUR BENCHMARK · 2 RUNS" (Colin's screenshot, 10/21): the
 * batch's own count read as the benchmark's. Three judges read as three.
 */
export const stepWords = (progress: RerunProgress, threeJudges = false): string => {
  const step = progress.step === 'checking' ? 'waiting for checks before judging'
    : progress.step === 'judging' ? (threeJudges ? 'the judges are reading the answers' : 'the judge is reading the answers')
    : 'the models are answering'
  return progress.of === 1 ? `Running it once more: ${step}` : `Running it again, ${String(progress.run)} of ${String(progress.of)}: ${step}`
}

const sameStart = (base: string | undefined): string => (base === undefined ? '' : `, from commit ${base.slice(0, 7)}`)

export function CompareBenchmark({ view }: { readonly view: BenchmarkView }): ReactElement {
  const place = useRef<HTMLDivElement>(null)
  // Once, as it opens: a person scrolling away from it afterwards is not pulled back.
  useEffect(() => {
    if (view.reveal === true) place.current?.scrollIntoView({ block: 'start' })
  }, [])
  const running = view.progress !== undefined
  const reran = view.runs.length > 1
  const actions = view.refusal !== undefined ? (
    <p className="lc-compare__judgenote">{view.refusal}</p>
  ) : running ? (
    <div className="lc-benchmark__actions">
      <span className="lc-benchmark__progress" role="status">
        <WorkingSpark />
        {stepWords(view.progress!, view.threeJudges === true)}
      </span>
      <button type="button" className="lc-button" onClick={view.onStop}>Stop</button>
    </div>
  ) : (
    <div className="lc-benchmark__actions">
      <button type="button" className={reran ? 'lc-button' : 'lc-primarybutton'} disabled={view.busy} onClick={() => view.onRun(3)}>Run 3 more times</button>
      <button type="button" className="lc-button" disabled={view.busy} onClick={() => view.onRun(1)}>Run once more</button>
    </div>
  )

  if (!reran) {
    return (
      <div className="lc-benchmark" aria-label="Your benchmark" ref={place}>
        <span className="lc-compare__recordlabel lc-mono">YOUR BENCHMARK</span>
        <p className="lc-benchmark__lead">
          {`One run can be luck. Run this comparison again and see whether the scores hold: the same models, the same question${sameStart(view.base)}, scored by ${view.threeJudges === true ? 'the same three judges' : 'the same judge'} on the same points.`}
        </p>
        {actions}
        {view.problem !== undefined && <p className="lc-compare__problem" role="status">{view.problem}</p>}
        <p className="lc-compare__judgenote">The runs go one after another and keep going with the window in the tray. They belong to this comparison, not to the conversation list.</p>
      </div>
    )
  }

  const scores = view.rows.flatMap((row) => (row.score === undefined ? [] : [row.score]))
  const best = Math.max(...scores)
  const alone = scores.filter((score) => score === best).length === 1
  const scoredRuns = view.runs.filter((run) => run.state === 'scored').length
  const doneRuns = view.runs.filter((run) => run.done).length
  // Your check in each copy (0.810): a column only once a run of it was checked.
  const checked = view.rows.some((row) => row.checked > 0)
  return (
    <div className="lc-benchmark" aria-label="Your benchmark" ref={place}>
      <div className="lc-benchmark__head">
        <span className="lc-compare__recordlabel lc-mono">{`YOUR BENCHMARK · ${String(view.runs.length)} RUNS`}</span>
        {actions}
      </div>
      {view.problem !== undefined && <p className="lc-compare__problem" role="status">{view.problem}</p>}
      <div className="lc-tablewrap">
        <table className="lc-table lc-benchmark__table">
          <thead>
            <tr>
              <th scope="col">Model</th>
              <th scope="col">Score</th>
              <th scope="col">Met every point</th>
              {checked && <th scope="col">Check passed</th>}
              <th scope="col">Time a task</th>
              <th scope="col">Cost a task</th>
              {view.teammateName !== undefined && view.onUse !== undefined && <th scope="col"><span className="lc-sr">{`Use for ${view.teammateName}`}</span></th>}
            </tr>
          </thead>
          <tbody>
            {view.rows.map((row) => (
              <tr key={row.key}>
                <th scope="row" className="lc-benchmark__model">
                  {row.name}
                  {/* A model whose answer failed in a run says so: its numbers are over the runs it answered. */}
                  {row.runs < doneRuns && <span className="lc-benchmark__missed">{`answered ${String(row.runs)} of ${String(doneRuns)} runs`}</span>}
                </th>
                <td className={`lc-compare__recordnum lc-benchmark__score${alone && row.score === best ? ' is-best' : ''}`}>
                  {row.score === undefined ? 'not scored' : String(row.score)}
                </td>
                <td className="lc-compare__recordnum">{row.scored === 0 ? '—' : `${String(row.everyPoint)} of ${String(row.scored)}`}</td>
                {checked && (
                  <td className={`lc-compare__recordnum${row.checked === 0 ? '' : row.checkPassed === row.checked ? ' lc-tone-green' : ' lc-tone-amber'}`}>
                    {row.checked === 0 ? '—' : `${String(row.checkPassed)} of ${String(row.checked)}`}
                  </td>
                )}
                <td className="lc-compare__recordnum">{row.spanMs === undefined ? '—' : durationText(row.spanMs)}</td>
                <td className="lc-compare__recordnum">{/-free$/i.test(row.model) && (row.cost?.usd ?? 0) === 0 ? 'free' : costCell(row.cost, row.model)}</td>
                {view.teammateName !== undefined && view.onUse !== undefined && (
                  <td className="lc-benchmark__use">
                    {row.key === view.teammateKey ? (
                      <span className="lc-benchmark__current">{`${view.teammateName}'s model`}</span>
                    ) : (
                      <button type="button" className="lc-button" disabled={view.using !== undefined} onClick={() => view.onUse?.(row)}>
                        {view.using === row.key ? 'Setting…' : `Use for ${view.teammateName}`}
                      </button>
                    )}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="lc-compare__judgenote">
        {`A score is the judge's points met, out of 100, averaged over the ${scoredRuns === 1 ? 'run' : `${String(scoredRuns)} runs`} it scored. Met every point counts the runs that met all of them.${checked ? ' Check passed counts the runs whose copy passed your check.' : ''} Time and cost are each run's own, as its agent reported them.`}
      </p>
      {view.chart !== undefined && <ScoreChart chart={view.chart} />}
      <div className="lc-benchmark__runs" aria-label="Its runs">
        {view.runs.map((run, index) => (
          <button key={run.compareId} type="button" className="lc-benchmark__run" onClick={() => view.onOpenRun(run.compareId)}>
            <span>{index === 0 ? 'Run 1 · this one' : `Run ${String(index + 1)}`}</span>
            <span className="lc-benchmark__runstate lc-mono">{run.state}</span>
          </button>
        ))}
      </div>
      <p className="lc-compare__judgenote">{`Each run asks the same models the same question${sameStart(view.base)}, and ${view.threeJudges === true ? 'the same three judges score' : 'the same judge scores'} it on the same points.`}</p>
    </div>
  )
}

/** Dollars as the chart's axis says them: enough places for a cheap task. */
function dollarTick(value: number): string {
  if (value === 0) return '$0'
  return value < 0.1 ? `$${value.toFixed(3)}` : `$${value.toFixed(2)}`
}

const WIDTH = 640
const HEIGHT = 244
const LEFT = 40
const RIGHT = 20
const TOP = 16
const BOTTOM = 44

/**
 * SCORE AGAINST COST A TASK (0.763), Optima's chart: up is better, left is cheaper -- or, where too few runs were
 * priced in dollars, left is faster. The corner that is both is named; nothing is joined, ranked or extrapolated.
 */
export function ScoreChart({ chart }: { readonly chart: BenchmarkChart }): ReactElement {
  const maxX = Math.max(...chart.points.map((point) => point.x))
  const span = maxX > 0 ? maxX * 1.15 : 1
  const plotWidth = WIDTH - LEFT - RIGHT
  const plotHeight = HEIGHT - TOP - BOTTOM
  const xAt = (value: number): number => LEFT + (value / span) * plotWidth
  const yAt = (score: number): number => TOP + (1 - score / 100) * plotHeight
  const cost = chart.axis === 'cost'
  const tick = (value: number): string => (cost ? dollarTick(value) : durationText(value))
  // Labels beside their points, turned to the left near the right edge, and a line apart when two would touch.
  const placed: { x: number; y: number }[] = []
  const labels = chart.points.map((point) => {
    const x = xAt(point.x)
    let y = yAt(point.score)
    const right = x < LEFT + plotWidth * 0.62
    while (placed.some((other) => Math.abs(other.y - y) < 14 && Math.abs(other.x - x) < 180)) y += 14
    placed.push({ x, y })
    return { point, x, y, right }
  })
  const summary = chart.points.map((point) => `${point.name} ${String(point.score)} at ${point.free ? 'free' : tick(point.x)}`).join('; ')
  // The corner's name gives way to a point's: a free model stands at the left edge, and a strong one at the top.
  const cornerClear = !labels.some(({ x, y }) => x < LEFT + 150 && y < TOP + 26)
  return (
    <figure className="lc-benchmark__chart">
      <svg viewBox={`0 0 ${String(WIDTH)} ${String(HEIGHT)}`} role="img" aria-label={`Score against ${cost ? 'cost' : 'time'} a task: ${summary}`}>
        {cornerClear && <text className="lc-benchmark__corner" x={LEFT + 8} y={TOP + 12}>{cost ? 'Strong and cheap' : 'Strong and fast'}</text>}
        {[0, 50, 100].map((score) => (
          <g key={`grid:${String(score)}`}>
            <line className="lc-benchmark__grid" x1={LEFT} x2={WIDTH - RIGHT} y1={yAt(score)} y2={yAt(score)} />
            <text className="lc-benchmark__tick" x={LEFT - 8} y={yAt(score) + 4} textAnchor="end">{String(score)}</text>
          </g>
        ))}
        <line className="lc-benchmark__axis" x1={LEFT} x2={LEFT} y1={TOP} y2={TOP + plotHeight} />
        <text className="lc-benchmark__tick" x={LEFT} y={TOP + plotHeight + 18} textAnchor="start">{tick(0)}</text>
        {/* Every model free: one tick, not "$0" twice at the same place. */}
        {maxX > 0 && <text className="lc-benchmark__tick" x={xAt(maxX)} y={TOP + plotHeight + 18} textAnchor="middle">{tick(maxX)}</text>}
        {/* The axis's name a line under its ticks: beside them, the far tick ran into it ("34stime a task"). */}
        <text className="lc-benchmark__tick" x={WIDTH - RIGHT} y={HEIGHT - 4} textAnchor="end">{cost ? 'cost a task' : 'time a task'}</text>
        {labels.map(({ point, x, y, right }) => (
          <g key={point.key}>
            {/* Moved down to clear another label, it is joined to its point: three at 100 read as three loose names. */}
            {y !== yAt(point.score) && <line className="lc-benchmark__leader" x1={right ? x + 8 : x - 8} y1={y} x2={x} y2={yAt(point.score)} />}
            <circle className="lc-benchmark__point" cx={x} cy={yAt(point.score)} r={5} />
            <text className="lc-benchmark__label" x={right ? x + 10 : x - 10} y={y + 4} textAnchor={right ? 'start' : 'end'}>
              {`${point.name} · ${String(point.score)}${point.free ? ' · free' : ''}`}
            </text>
          </g>
        ))}
      </svg>
      <figcaption className="lc-compare__judgenote">
        {cost
          ? 'Up is better, left is cheaper. Time a task is in the table; it does not move the chart.'
          : chart.because === 'same-price'
            ? 'Up is better, left is faster: every one cost the same here, so the chart is against time.'
            : 'Up is better, left is faster: too few of these runs were priced in dollars to chart cost.'}
        {chart.left.length > 0 ? ` Not on the chart: ${chart.left.join(', ')}${cost ? ', whose runs carry no price in dollars' : ', whose runs have no time'}.` : ''}
      </figcaption>
    </figure>
  )
}
