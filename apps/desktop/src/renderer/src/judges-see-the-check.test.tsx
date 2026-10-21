import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicCompare } from '../../shared/compare.js'
import type { Scorecard } from '../../shared/compare-score.js'
import { benchmarkRows, type BenchmarkAnswer } from './benchmark.js'
import { CompareBenchmark, ScoreChart, stepWords, type BenchmarkView } from './components/CompareBenchmark.js'
import { CompareView, checkFailedEverywhere, checkWord } from './components/CompareView.js'
import type { CompareColumnView } from './components/CompareView.js'
import type { EditCheckShown } from './components/EditCheckCard.js'
import { shownRouteLabel } from './routeName.js'

/*
 * JUDGES SEE THE CHECK (0.811). Each column of a comparison that edits ran your check in its copy after its turn
 * (A3.3); what it said is shown in the column and in the judges' table, and every judge reads the same. Beside it, the
 * benchmark fixes from Colin's screenshot of 10/21: the batch's count, names that were ids, and loose chart labels.
 */
const passed: EditCheckShown = { command: 'npm test', outcome: 'passed', newLines: [], unchanged: false, first: true }
const failed: EditCheckShown = { command: 'npm test', outcome: 'failed', newLines: ['FAIL totals.test.mjs', 'TypeError: summarize is not a function'], unchanged: false, first: true }

const compare: PublicCompare = {
  compareId: 'cmp_1',
  prompt: 'Split total into parseRow and summarize; keep the tests passing.',
  createdAt: '2026-10-21T12:00:00.000Z',
  changes: true,
  base: 'b'.repeat(40),
  slots: [
    { slot: 'a', route: { runtime: 'opencode', model: 'own-0a1b2c3d/claude-haiku-5-5', label: 'claude-haiku-5-5' }, missionIds: ['m_a'] },
    { slot: 'b', route: { runtime: 'opencode', model: 'own-0a1b2c3d/claude-sonnet-5-5', label: 'Sonnet at work' }, missionIds: ['m_b'] }
  ]
}
const column = (slot: 'a' | 'b', check?: EditCheckShown): CompareColumnView => ({
  slot,
  name: slot === 'a' ? 'Claude Haiku 5.5' : 'Sonnet at work',
  runtime: 'opencode',
  runtimeName: 'OpenCode',
  turns: [{ missionId: `m_${slot}`, items: [], running: false }],
  running: false,
  keepable: true,
  retryable: false,
  answer: 'Done.',
  state: 'done',
  ...(check === undefined ? {} : { check })
})
const scorecard: Scorecard = {
  points: ['Splits total into parseRow and summarize'],
  byJudge: false,
  verdicts: { a: [{ met: true, why: 'Split as asked.' }], b: [{ met: true, why: 'Split as asked.' }] }
}
const render = (columns: readonly CompareColumnView[], extra: { readonly checkCommand?: string; readonly judged?: boolean } = {}): string => renderToStaticMarkup(
  <CompareView
    compare={compare}
    changeLines={{}}
    prompts={[compare.prompt]}
    columns={columns}
    owner={undefined}
    workspacePath={undefined}
    keeping={false}
    retrying={undefined}
    onKeep={() => undefined}
    onRetry={() => undefined}
    onBack={undefined}
    judgeChoices={[{ key: 'opencode:opencode/deepseek-v4.1-flash-free', label: 'Deepseek V4.1 Flash Free' }]}
    onJudge={() => undefined}
    {...(extra.checkCommand === undefined ? {} : { checkCommand: extra.checkCommand })}
    {...(extra.judged === true ? { judge: { name: 'Deepseek V4.1 Flash Free', running: false, answer: 'I would keep Answer A.', items: [], pick: 'a' as const, scorecard } } : {})}
  />
)

describe('your check, in a comparison that edits', () => {
  it('shows what it said in each copy, under that column\'s answer', () => {
    const html = render([column('a', passed), column('b', failed)], { checkCommand: 'npm test' })
    expect(html).toContain('passed after this turn.')
    expect(html).toContain('failed after this turn')
    expect(html).toContain('TypeError: summarize is not a function')
    // A column is not a conversation: nothing is sent back from here.
    expect(html).not.toContain('Send to')
  })

  it('tells the person the judges read it, or where to set one', () => {
    expect(render([column('a', passed), column('b', failed)], { checkCommand: 'npm test' })).toContain('Each judge also reads what your check, npm test, said in each copy.')
    expect(render([column('a'), column('b')])).toContain('Set a check command in Settings &gt; Project folder, and each judge also reads whether every copy passes it.')
    // Set, but no copy has run it yet: nothing to promise and nothing to ask.
    const quiet = render([column('a'), column('b')], { checkCommand: 'npm test' })
    expect(quiet).not.toContain('Each judge also reads')
    expect(quiet).not.toContain('Set a check command')
  })

  it('puts it beside the judges\' score', () => {
    const html = render([column('a', passed), column('b', failed)], { checkCommand: 'npm test', judged: true })
    expect(html).toMatch(/<th scope="row">Your check<\/th><td class="lc-compare__recordnum lc-tone-green">passed<\/td><td class="lc-compare__recordnum lc-tone-amber">failed<\/td>/)
    expect(render([column('a'), column('b')], { judged: true })).not.toContain('Your check')
  })

  it('says once when it failed the same way in every copy, and what to do about it', () => {
    const missing: EditCheckShown = { ...failed, newLines: ['sh: vitest: command not found'] }
    const columns = [column('a', missing), column('b', missing)]
    expect(checkFailedEverywhere(columns)).toBe(true)
    expect(render(columns, { checkCommand: 'npm test' })).toContain('Your check failed the same way in every copy, so it tells them apart on nothing.')
    expect(checkFailedEverywhere([column('a', passed), column('b', failed)])).toBe(false)
    expect(render([column('a', passed), column('b', failed)], { checkCommand: 'npm test' })).not.toContain('failed the same way in every copy')
  })

  it('reads each outcome in a word', () => {
    expect(checkWord(passed)).toBe('passed')
    expect(checkWord({ ...passed, outcome: 'timed-out' })).toBe('timed out')
    expect(checkWord({ ...passed, outcome: 'could-not-run' })).toBe('could not run')
    expect(checkWord(undefined)).toBe('—')
  })
})

describe('your check, in your benchmark', () => {
  const answer = (key: string, extra: Partial<BenchmarkAnswer>): BenchmarkAnswer => ({ key, runtime: 'opencode', model: key, name: key, answered: true, met: 1, of: 1, ...extra })
  const rows = benchmarkRows([
    { compareId: 'cmp_1', answers: [answer('haiku', { check: 'passed' }), answer('sonnet', { check: 'failed' })] },
    { compareId: 'cmp_2', answers: [answer('haiku', { check: 'passed' }), answer('sonnet', { check: 'passed' })] }
  ])
  const view: BenchmarkView = {
    rows,
    runs: [{ compareId: 'cmp_1', state: 'scored', done: true }, { compareId: 'cmp_2', state: 'scored', done: true }],
    busy: false,
    onRun: () => undefined,
    onStop: () => undefined,
    onOpenRun: () => undefined
  }

  it('counts the runs each model\'s copy passed it', () => {
    expect(rows.find((row) => row.key === 'haiku')).toMatchObject({ checked: 2, checkPassed: 2 })
    expect(rows.find((row) => row.key === 'sonnet')).toMatchObject({ checked: 2, checkPassed: 1 })
    const html = renderToStaticMarkup(<CompareBenchmark view={view} />)
    expect(html).toContain('<th scope="col">Check passed</th>')
    expect(html).toContain('<td class="lc-compare__recordnum lc-tone-green">2 of 2</td>')
    expect(html).toContain('<td class="lc-compare__recordnum lc-tone-amber">1 of 2</td>')
    expect(html).toContain('Check passed counts the runs whose copy passed your check.')
  })

  it('has no such column where no run was checked', () => {
    const unchecked = benchmarkRows([{ compareId: 'cmp_1', answers: [answer('haiku', {}), answer('sonnet', {})] }])
    expect(renderToStaticMarkup(<CompareBenchmark view={{ ...view, rows: unchecked }} />)).not.toContain('Check passed')
  })
})

describe('the benchmark, as Colin\'s screenshot of 10/21 found it', () => {
  it('says a run again by the batch\'s own count, and three judges as three', () => {
    expect(stepWords({ originId: 'cmp_1', run: 1, of: 1, step: 'judging' }, true)).toBe('Running it once more: the judges are reading the answers')
    expect(stepWords({ originId: 'cmp_1', run: 2, of: 3, step: 'answering' })).toBe('Running it again, 2 of 3: the models are answering')
    expect(stepWords({ originId: 'cmp_1', run: 1, of: 1, step: 'judging' })).toBe('Running it once more: the judge is reading the answers')
  })

  it('joins a label moved off its point\'s line to its point', () => {
    const html = renderToStaticMarkup(<ScoreChart chart={{
      axis: 'time',
      because: 'unpriced',
      points: [
        { key: 'haiku', name: 'Claude Haiku 5.5', score: 100, x: 16_000, free: false },
        { key: 'sonnet', name: 'Claude Sonnet 5.5', score: 100, x: 17_000, free: false },
        { key: 'opus', name: 'Claude Opus 5.5', score: 100, x: 21_000, free: false }
      ],
      left: []
    }} />)
    // Three at 100: the first sits on its point's line, the two moved below it each keep a line to theirs.
    expect(html.match(/class="lc-benchmark__leader"/g)?.length).toBe(2)
  })

  it('reads a model added under its own id by its name, and leaves a name the person wrote alone', () => {
    expect(shownRouteLabel({ model: 'own-0a1b2c3d/claude-haiku-5-5', label: 'claude-haiku-5-5' })).toBe('Claude Haiku 5.5')
    expect(shownRouteLabel({ model: 'own-0a1b2c3d/claude-sonnet-5-5', label: 'Sonnet at work' })).toBe('Sonnet at work')
    expect(shownRouteLabel({ model: 'opencode/deepseek-v4.1-flash-free', label: 'opencode/deepseek-v4.1-flash-free' })).toBe('opencode/deepseek-v4.1-flash-free')
    expect(shownRouteLabel({ model: 'gpt-6-sol' })).toBeUndefined()
  })
})
