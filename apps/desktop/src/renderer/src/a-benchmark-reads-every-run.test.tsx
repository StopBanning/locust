import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { averageCost, benchmarkChart, benchmarkRows, median, type BenchmarkAnswer, type BenchmarkRunInput } from './benchmark.js'
import { CompareBenchmark, ScoreChart, type BenchmarkView } from './components/CompareBenchmark.js'

/*
 * YOUR OWN BENCHMARK, ON SCREEN (0.763): every run of one comparison read as one table, a row a model -- the judge's
 * points met out of 100 averaged over the runs it scored, the runs that met every point, the middle run's time, a run's
 * cost in its runtime's own unit -- and score against cost on a chart, or against time when too few were priced.
 */
const answer = (key: string, extra: Partial<BenchmarkAnswer> = {}): BenchmarkAnswer => ({
  key,
  runtime: key.split(':')[0] as BenchmarkAnswer['runtime'],
  model: key.split(':')[1] ?? key,
  name: key.split(':')[1] ?? key,
  answered: true,
  ...extra
})
const run = (compareId: string, answers: readonly BenchmarkAnswer[]): BenchmarkRunInput => ({ compareId, answers })

const SONNET = 'claude:claude-sonnet-5-5'
const BUNNY = 'opencode:opencode/space-bunny-free'
const SOL = 'codex:gpt-6-sol'

describe('a benchmark read from every run', () => {
  it('averages the points met over the runs the judge scored, and counts the runs that met every one', () => {
    const rows = benchmarkRows([
      run('cmp_1', [answer(SONNET, { met: 2, of: 2, spanMs: 60_000, cost: { usd: 0.02 } }), answer(BUNNY, { met: 1, of: 2, spanMs: 90_000, cost: { usd: 0 } })]),
      run('cmp_2', [answer(SONNET, { met: 2, of: 2, spanMs: 80_000, cost: { usd: 0.04 } }), answer(BUNNY, { met: 2, of: 2, spanMs: 100_000, cost: { usd: 0 } })]),
      // Not scored: its judge could not read it. It still counts as a run each model answered.
      run('cmp_3', [answer(SONNET, { spanMs: 70_000, cost: { usd: 0.03 } }), answer(BUNNY, { answered: false })])
    ])
    expect(rows.map((row) => row.key)).toEqual([SONNET, BUNNY])
    expect(rows[0]).toMatchObject({ score: 100, scored: 2, everyPoint: 2, runs: 3, spanMs: 70_000 })
    expect(rows[0]?.cost?.usd).toBeCloseTo(0.03)
    expect(rows[1]).toMatchObject({ score: 75, scored: 2, everyPoint: 1, runs: 2, spanMs: 95_000 })
  })

  it('keeps each runtime in its own unit: dollars, premium requests, tokens, or in your plan', () => {
    expect(averageCost([{ usd: 0.01 }, { usd: 0.03 }, undefined])).toEqual({ usd: 0.02 })
    expect(averageCost([{ premiumRequests: 1 }, { premiumRequests: 2 }])).toEqual({ premiumRequests: 1.5 })
    expect(averageCost([{ plan: true, inputTokens: 1000, outputTokens: 100 }, { plan: true, inputTokens: 3000, outputTokens: 300 }])).toEqual({ plan: true, inputTokens: 2000, outputTokens: 200 })
    expect(averageCost([undefined])).toBeUndefined()
    expect(median([3, 1, 2])).toBe(2)
    expect(median([4, 1, 3, 2])).toBe(2.5)
    expect(median([])).toBeUndefined()
  })

  it('charts score against cost when at least two models were priced in dollars, a free one at $0', () => {
    const rows = benchmarkRows([run('cmp_1', [
      answer(SONNET, { met: 2, of: 2, cost: { usd: 0.02 }, spanMs: 1_000 }),
      answer(BUNNY, { met: 1, of: 2, cost: { usd: 0 }, spanMs: 2_000 }),
      answer(SOL, { met: 2, of: 2, cost: { plan: true }, spanMs: 1_500 })
    ])])
    const chart = benchmarkChart(rows)
    expect(chart?.axis).toBe('cost')
    expect(chart?.points.map((point) => point.key).sort()).toEqual([BUNNY, SONNET].sort())
    expect(chart?.points.find((point) => point.key === BUNNY)?.free).toBe(true)
    expect(chart?.left).toEqual(['gpt-6-sol'])
  })

  it('charts score against time when too few were priced, and draws no chart for fewer than two scored', () => {
    const timed = benchmarkChart(benchmarkRows([run('cmp_1', [answer(SONNET, { met: 2, of: 2, cost: { plan: true }, spanMs: 1_000 }), answer(SOL, { met: 1, of: 2, cost: { plan: true }, spanMs: 2_000 })])]))
    expect(timed).toMatchObject({ axis: 'time', because: 'unpriced' })
    expect(benchmarkChart(benchmarkRows([run('cmp_1', [answer(SONNET, { met: 2, of: 2 }), answer(SOL)])]))).toBeUndefined()
  })

  it('charts two free models against time: on cost they would stand on one line', () => {
    const free = benchmarkChart(benchmarkRows([run('cmp_1', [
      answer(BUNNY, { met: 2, of: 2, cost: { usd: 0 }, spanMs: 41_000 }),
      answer('opencode:opencode/muse-spark-1.3-contributor-free', { met: 0, of: 2, cost: { usd: 0 }, spanMs: 33_000 })
    ])]))
    expect(free).toMatchObject({ axis: 'time', because: 'same-price' })
    expect(free?.points.map((point) => point.x).sort()).toEqual([33_000, 41_000])
    const html = renderToStaticMarkup(<ScoreChart chart={free!} />)
    expect(html).toContain('every one cost the same here, so the chart is against time')
  })
})

describe('your benchmark, in the comparison', () => {
  const base: BenchmarkView = {
    rows: [],
    runs: [{ compareId: 'cmp_1', state: 'scored', done: true }],
    busy: false,
    onRun: () => undefined,
    onStop: () => undefined,
    onOpenRun: () => undefined
  }

  it('asks to run it again before it has been, saying it starts from the same commit', () => {
    const html = renderToStaticMarkup(<CompareBenchmark view={{ ...base, base: 'c'.repeat(40) }} />)
    expect(html).toContain('YOUR BENCHMARK')
    expect(html).toContain('One run can be luck.')
    expect(html).toContain('from commit ccccccc')
    expect(html).toContain('>Run 3 more times</button>')
    expect(html).toContain('>Run once more</button>')
  })

  it('says why, in place of the buttons, when it cannot be run again', () => {
    const html = renderToStaticMarkup(<CompareBenchmark view={{ ...base, refusal: 'Keep one first: the names stay hidden until you do, and a benchmark shows them.' }} />)
    expect(html).toContain('Keep one first')
    expect(html).not.toContain('Run 3 more times')
  })

  it('reads every run as one table, the best score marked, and sets a teammate on a row', () => {
    const rows = benchmarkRows([
      run('cmp_1', [answer(SONNET, { name: 'Sonnet 5.5', met: 2, of: 2, spanMs: 72_000, cost: { usd: 0.024 } }), answer(BUNNY, { name: 'Space Bunny', met: 1, of: 2, spanMs: 107_000, cost: { usd: 0 } })]),
      run('cmp_2', [answer(SONNET, { name: 'Sonnet 5.5', met: 2, of: 2, spanMs: 70_000, cost: { usd: 0.024 } }), answer(BUNNY, { name: 'Space Bunny', met: 2, of: 2, spanMs: 100_000, cost: { usd: 0 } })])
    ])
    const html = renderToStaticMarkup(<CompareBenchmark view={{
      ...base,
      rows,
      ...(benchmarkChart(rows) === undefined ? {} : { chart: benchmarkChart(rows)! }),
      runs: [{ compareId: 'cmp_1', state: 'scored', done: true }, { compareId: 'cmp_2', state: 'scored', done: true }],
      teammateName: 'Wren',
      teammateKey: BUNNY,
      onUse: () => undefined
    }} />)
    expect(html).toContain('YOUR BENCHMARK · 2 RUNS')
    expect(html).toContain('Sonnet 5.5')
    expect(html).toMatch(/lc-benchmark__score is-best">100</)
    expect(html).toContain('>75<')
    expect(html).toContain('2 of 2')
    expect(html).toContain('free')
    expect(html).toContain('>Use for Wren</button>')
    expect(html).toContain("Wren&#x27;s model")
    expect(html).toContain('Run 1 · this one')
    expect(html).toContain('Score against cost a task: ')
    expect(html).toContain('Strong and cheap')
  })

  it('says how many runs a model answered, when its answer failed in one', () => {
    const rows = benchmarkRows([
      run('cmp_1', [answer(SONNET, { name: 'Sonnet 5.5', met: 2, of: 2 }), answer(BUNNY, { name: 'Space Bunny', met: 1, of: 2 })]),
      run('cmp_2', [answer(SONNET, { name: 'Sonnet 5.5', met: 2, of: 2 }), answer(BUNNY, { name: 'Space Bunny', answered: false })])
    ])
    const html = renderToStaticMarkup(<CompareBenchmark view={{
      ...base,
      rows,
      runs: [{ compareId: 'cmp_1', state: 'scored', done: true }, { compareId: 'cmp_2', state: 'not scored · 1 of 2 answered', done: true }]
    }} />)
    expect(html).toContain('answered 1 of 2 runs')
    expect(html.split('answered 1 of 2 runs').length).toBe(2)
    expect(html).toContain('not scored · 1 of 2 answered')
  })

  it('says which run it is on, and what that run is doing, with a way to stop', () => {
    const html = renderToStaticMarkup(<CompareBenchmark view={{ ...base, runs: [...base.runs, { compareId: 'cmp_2', state: 'answering…', done: false }], progress: { originId: 'cmp_1', run: 1, of: 3, step: 'answering', compareId: 'cmp_2' } }} />)
    // The batch's count, said as the batch's (0.810): "Run 1 of 1" read as the benchmark's beside "2 RUNS".
    expect(html).toContain('Running it again, 1 of 3: the models are answering')
    expect(html).toContain('>Stop</button>')
    expect(html).toContain('answering…')
    const checking = renderToStaticMarkup(<CompareBenchmark view={{ ...base, progress: { originId: 'cmp_1', run: 1, of: 1, step: 'checking', compareId: 'cmp_2' } }} />)
    expect(checking).toContain('waiting for checks before judging')
    expect(checking).toContain('>Stop</button>')
  })
})
