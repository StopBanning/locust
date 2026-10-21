import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { benchmarkLeader, type BenchmarkRow } from './benchmark.js'
import { benchmarkModelsLine, BenchmarksScreen, leaderWords, orderedBenchmarks, type BenchmarkEntry } from './components/BenchmarksScreen.js'

/*
 * YOUR BENCHMARKS, IN ONE PLACE (0.764): every comparison run again, a row each -- its question, its models, how many
 * runs, who leads and by how much, when it last ran -- the running ones first, then the newest, and the row opening it.
 */
const row = (name: string, score?: number): BenchmarkRow => ({
  key: `opencode:${name}`,
  runtime: 'opencode',
  model: name,
  name,
  runs: 2,
  scored: score === undefined ? 0 : 2,
  everyPoint: 0,
  checked: 0,
  checkPassed: 0,
  ...(score === undefined ? {} : { score })
})
const entry = (compareId: string, lastAt: string, extra: Partial<BenchmarkEntry> = {}): BenchmarkEntry => ({
  compareId,
  question: `Question ${compareId}`,
  models: ['Space Bunny', 'Muse Spark'],
  runs: 4,
  judgeName: 'Codex',
  lastAt,
  rows: [row('Space Bunny', 83), row('Muse Spark', 50)],
  changes: false,
  ...extra
})

describe('every benchmark, listed in one place', () => {
  it('names the leader, and says a tie as a tie', () => {
    expect(benchmarkLeader([row('Space Bunny', 83), row('Muse Spark', 50)])).toEqual({ names: ['Space Bunny'], score: 83 })
    expect(benchmarkLeader([row('A', 80), row('B', 80), row('C')])).toEqual({ names: ['A', 'B'], score: 80 })
    expect(benchmarkLeader([row('A'), row('B')])).toBeUndefined()
    expect(leaderWords({ names: ['Space Bunny'] })).toBe('Space Bunny')
    expect(leaderWords({ names: ['A', 'B'] })).toBe('A and B tie')
    expect(leaderWords({ names: ['A', 'B', 'C'] })).toBe('3 models tie')
    expect(benchmarkModelsLine(['A', 'B', 'C'])).toBe('A vs B vs C')
    expect(benchmarkModelsLine(['A', 'B', 'C', 'D'])).toBe('4 models')
  })

  it('puts the running ones first, then the newest', () => {
    const order = orderedBenchmarks([
      entry('cmp_old', '2026-10-10T10:00:00.000Z'),
      entry('cmp_new', '2026-10-12T10:00:00.000Z'),
      entry('cmp_going', '2026-10-01T10:00:00.000Z', { progress: { originId: 'cmp_going', run: 2, of: 3, step: 'answering' } })
    ])
    expect(order.map((one) => one.compareId)).toEqual(['cmp_going', 'cmp_new', 'cmp_old'])
  })

  it('draws a row a benchmark, with what it is, who leads it, and where it is while it runs', () => {
    const html = renderToStaticMarkup(
      <BenchmarksScreen
        entries={[
          entry('cmp_a', '2026-10-12T10:00:00.000Z', { changes: true }),
          entry('cmp_b', '2026-10-12T09:00:00.000Z', { rows: [row('A'), row('B')] }),
          entry('cmp_c', '2026-10-12T08:00:00.000Z', { progress: { originId: 'cmp_c', run: 2, of: 3, step: 'judging' } })
        ]}
        onOpen={() => undefined}
      />
    )
    expect(html).toContain('3 benchmarks')
    expect(html).toContain('12 runs')
    expect(html).toContain('Space Bunny vs Muse Spark · 4 runs · judged by Codex · changes files')
    expect(html).toContain('lc-benchmarks__leader">Space Bunny<')
    expect(html).toContain('width:83%')
    expect(html).toContain('not scored yet')
    expect(html).toContain('running again, 2 of 3 · judging')
    // One button a row: the whole row opens it.
    expect(html.match(/class="lc-benchmarks__row"/g)).toHaveLength(3)
  })

  it('says what a benchmark is, and how to make one, when there is none', () => {
    const html = renderToStaticMarkup(<BenchmarksScreen entries={[]} onOpen={() => undefined} />)
    expect(html).toContain('none yet')
    expect(html).toContain('Run 3 more times')
  })
})
