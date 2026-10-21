import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicCompare } from '../../shared/compare.js'
import { tuneRoutes } from '../../shared/tune.js'
import { benchmarkRows, type BenchmarkAnswer } from './benchmark.js'
import { CompareBenchmark, type BenchmarkView } from './components/CompareBenchmark.js'
import { CompareView, type CompareColumnView } from './components/CompareView.js'

/*
 * A TUNE, MEASURED, ON SCREEN (0.811). A tune's three columns are one model under three sets of instructions: its
 * benchmark is a row a setup, by its column, so the three never fold into the one model they share, and Setup A, B and
 * C stay blind until one is kept. Ask a judge says it reads setups, and that one run of each can be luck.
 */
const setupAnswer = (slot: 'a' | 'b' | 'c', met: number): BenchmarkAnswer => ({
  key: `opencode:opencode/example-free#${slot}`,
  runtime: 'opencode',
  model: 'opencode/example-free',
  slot,
  name: `Setup ${slot.toUpperCase()}`,
  answered: true,
  met,
  of: 2
})

describe('a tune\'s benchmark', () => {
  const rows = benchmarkRows([
    { compareId: 'cmp_tune', answers: [setupAnswer('a', 1), setupAnswer('b', 2), setupAnswer('c', 1)] },
    { compareId: 'cmp_tune_2', answers: [setupAnswer('a', 1), setupAnswer('b', 2), setupAnswer('c', 2)] }
  ])

  it('keeps a row a setup, by its column, though all three are one model', () => {
    expect(rows.map((row) => [row.name, row.slot, row.score])).toEqual([['Setup B', 'b', 100], ['Setup C', 'c', 75], ['Setup A', 'a', 50]])
  })

  it('reads as setups, with nothing to use for the teammate but Keep', () => {
    const view: BenchmarkView = {
      rows,
      setups: true,
      runs: [{ compareId: 'cmp_tune', state: 'scored', done: true }, { compareId: 'cmp_tune_2', state: 'scored', done: true }],
      busy: false,
      onRun: () => undefined,
      onStop: () => undefined,
      onOpenRun: () => undefined
    }
    const html = renderToStaticMarkup(<CompareBenchmark view={view} />)
    expect(html).toContain('<th scope="col">Setup</th>')
    expect(html).toContain('Each run asks the same model the same question under each setup’s instructions')
    expect(html).not.toContain('Use for')
    expect(html).not.toContain('opencode/example-free')
    // Before any run again: the invitation is the tune's own.
    const once = renderToStaticMarkup(<CompareBenchmark view={{ ...view, runs: view.runs.slice(0, 1) }} />)
    expect(once).toContain('One run can be luck. Run the three setups again and keep the one whose scores hold')
  })
})

describe('Ask a judge, in a tune not yet kept', () => {
  const tune: PublicCompare = {
    compareId: 'cmp_tune',
    teammateId: 'tm_wren',
    prompt: 'Explain the cause.',
    createdAt: '2026-10-21T12:00:00.000Z',
    blind: true,
    slots: tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free', label: 'Example Free' }, 'Be concise.', {
      how: { instructions: 'Check your work.', why: 'It stopped before checking.' },
      knows: { instructions: 'Name your assumptions.', why: 'It treated a guess as a fact.' }
    }).map((route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${String(index)}`] }))
  }
  const column = (slot: 'a' | 'b' | 'c'): CompareColumnView => ({
    slot,
    name: `Setup ${slot.toUpperCase()}`,
    runtime: 'opencode',
    runtimeName: 'OpenCode',
    turns: [{ missionId: `m_${slot}`, items: [], running: false }],
    running: false,
    keepable: true,
    retryable: false,
    answer: 'Done.',
    state: 'done'
  })
  const render = (compare: PublicCompare): string => renderToStaticMarkup(
    <CompareView
      compare={compare}
      changeLines={{}}
      prompts={[compare.prompt]}
      columns={[column('a'), column('b'), column('c')]}
      owner={undefined}
      workspacePath={undefined}
      keeping={false}
      retrying={undefined}
      onKeep={() => undefined}
      onRetry={() => undefined}
      onBack={undefined}
      judgeChoices={[{ key: 'opencode:opencode/judge-free', label: 'Judge Free' }]}
      onJudge={() => undefined}
    />
  )

  it('says it reads the setups without their names, and that one run of each can be luck', () => {
    const html = render(tune)
    expect(html).toContain('It reads the answers without the setups’ names')
    expect(html).toContain('One run of each setup can be luck. Once the judge has scored them, run the three again, and keep the setup whose scores hold up.')
  })

  it('says neither of a comparison of models', () => {
    const models: PublicCompare = { ...tune, slots: tune.slots.map((slot) => ({ ...slot, route: { runtime: slot.route.runtime, model: `${slot.route.model}-${slot.slot}` } })) }
    const html = render(models)
    expect(html).toContain('It reads the answers without the models’ names')
    expect(html).not.toContain('One run of each setup can be luck')
  })
})
