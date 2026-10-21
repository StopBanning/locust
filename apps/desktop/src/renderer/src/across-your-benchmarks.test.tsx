import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { acrossBenchmarks, type BenchmarkRow } from './benchmark.js'
import { BenchmarksScreen, type BenchmarkEntry } from './components/BenchmarksScreen.js'

/*
 * ACROSS YOUR BENCHMARKS (0.811). The API benchmark of 10/21 ran three Claude models on five tasks, and each task was
 * a benchmark of its own: five tables to read for one answer. The Benchmarks screen now reads them once, a row a model.
 */
const row = (model: string, extra: Partial<BenchmarkRow> = {}): BenchmarkRow => ({
  key: `opencode:own-0a1b2c3d/${model}`,
  runtime: 'opencode',
  model: `own-0a1b2c3d/${model}`,
  name: model,
  runs: 3,
  scored: 3,
  everyPoint: 3,
  checked: 0,
  checkPassed: 0,
  ...extra
})
const entry = (compareId: string, rows: readonly BenchmarkRow[]): BenchmarkEntry => ({
  compareId,
  question: `Question ${compareId}`,
  models: rows.map((one) => one.name),
  runs: 3,
  judgeName: 'Claude Opus 5.5',
  lastAt: '2026-10-21T16:00:00.000Z',
  rows,
  changes: true
})

describe('your benchmarks, read across', () => {
  it('averages each model over the benchmarks it ran in, each counted once, and counts its runs over all of them', () => {
    const across = acrossBenchmarks([
      { rows: [row('Haiku', { score: 100, spanMs: 24_000, cost: { usd: 0.06 } }), row('Opus', { score: 100, everyPoint: 2, spanMs: 32_000, cost: { usd: 0.46 } })] },
      // Run fifteen times, it still counts once.
      { rows: [row('Haiku', { score: 80, runs: 15, scored: 15, everyPoint: 9, spanMs: 42_000, cost: { usd: 0.1 } }), row('Opus', { score: 70, spanMs: 55_000, cost: { usd: 0.7 } }), row('Sonnet', { score: 100 })] }
    ])
    expect(across.map((one) => one.name)).toEqual(['Haiku', 'Opus'])
    expect(across[0]).toMatchObject({ benchmarks: 2, score: 90, runs: 18, scored: 18, everyPoint: 12, spanMs: 33_000 })
    expect(across[0]?.cost?.usd).toBeCloseTo(0.08, 10)
    expect(across[1]).toMatchObject({ benchmarks: 2, score: 85, everyPoint: 5 })
  })

  it('reads nothing across until two models have each run in two benchmarks', () => {
    expect(acrossBenchmarks([{ rows: [row('Haiku', { score: 100 }), row('Opus', { score: 90 })] }])).toEqual([])
    expect(acrossBenchmarks([{ rows: [row('Haiku', { score: 100 }), row('Opus', { score: 90 })] }, { rows: [row('Haiku', { score: 100 }), row('Sonnet', { score: 90 })] }])).toEqual([])
  })

  it('stands over the list on the Benchmarks screen, in a benchmark table\'s own columns, and not for one benchmark', () => {
    const two = renderToStaticMarkup(<BenchmarksScreen entries={[
      entry('cmp_bug', [row('Claude Haiku 5.5', { score: 100, cost: { usd: 0.065 } }), row('Claude Opus 5.5', { score: 100, cost: { usd: 0.461 } })]),
      entry('cmp_html', [row('Claude Haiku 5.5', { score: 100, cost: { usd: 0.095 } }), row('Claude Opus 5.5', { score: 80, everyPoint: 1, cost: { usd: 0.711 } })])
    ]} onOpen={() => undefined} />)
    expect(two).toContain('aria-label="Across your benchmarks"')
    expect(two).toContain('ACROSS YOUR BENCHMARKS')
    expect(two).toMatch(/<th scope="row" class="lc-benchmark__model">Claude Haiku 5\.5<\/th><td class="lc-compare__recordnum">2<\/td><td class="lc-compare__recordnum lc-benchmark__score is-best">100<\/td><td class="lc-compare__recordnum">6 of 6<\/td>/)
    expect(two).toContain('<td class="lc-compare__recordnum lc-benchmark__score">90</td>')
    expect(two).toContain('each benchmark counted once')
    // The table stands before the list of benchmarks.
    expect(two.indexOf('Across your benchmarks')).toBeLessThan(two.indexOf('lc-benchmarks__list'))
    const one = renderToStaticMarkup(<BenchmarksScreen entries={[entry('cmp_bug', [row('Claude Haiku 5.5', { score: 100 }), row('Claude Opus 5.5', { score: 100 })])]} onOpen={() => undefined} />)
    expect(one).not.toContain('Across your benchmarks')
  })
})
