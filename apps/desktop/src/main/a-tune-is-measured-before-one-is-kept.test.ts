import { describe, expect, it } from 'vitest'

import { rerunRefusalOf, type PublicCompare } from '../shared/compare.js'
import { tuneMeasuredOf, tuneRoutes } from '../shared/tune.js'
import { judgePrompt } from './compare-judge.js'
import { tuneLessonPrompt, type TuneLessonSetup } from './tune-lessons.js'

/*
 * A TUNE, MEASURED (0.811). A tune's choice rested on one run of each setup, and its note on what the tune showed
 * found nothing in a single run to stand on: no lesson in six notes, the drives of 10/21. Its three setups -- one model,
 * three sets of instructions -- can now be judged and run again before one is kept, the benchmark naming them Setup A,
 * B and C, and the note reads every run.
 */
const drafts = {
  how: { instructions: 'Check your work.', why: 'It stopped before checking.' },
  knows: { instructions: 'Name your assumptions.', why: 'It treated a guess as a fact.' }
}
const tune: PublicCompare = {
  compareId: 'cmp_tune',
  teammateId: 'tm_wren',
  prompt: 'Explain the cause.',
  createdAt: '2026-10-21T12:00:00.000Z',
  blind: true,
  changes: true,
  base: 'b'.repeat(40),
  judge: { route: { runtime: 'opencode', model: 'opencode/judge-free' }, missionIds: ['m_judge'] },
  slots: tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free', label: 'Example Free' }, 'Be concise.', drafts).map(
    (route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${String(index)}`] })
  )
}

describe('a tune, run again before one is kept', () => {
  it('may be: its benchmark names its setups by their columns, never the model they share', () => {
    expect(rerunRefusalOf(tune)).toBeUndefined()
  })

  it('still needs its judge first, and a comparison of models still waits for Keep', () => {
    const { judge: _judge, ...unjudged } = tune
    expect(rerunRefusalOf(unjudged)).toMatch(/^Ask a judge first/)
    const models: PublicCompare = { ...tune, slots: tune.slots.map((slot) => ({ ...slot, route: { runtime: slot.route.runtime, model: slot.route.model } })) }
    expect(rerunRefusalOf(models)).toMatch(/^Keep one first/)
  })
})

describe('what its benchmark counted for each setup, as the window sends it', () => {
  const record = { slot: 'b', runs: 4, scored: 4, score: 83, everyPoint: 3, checked: 4, checkPassed: 4 }

  it('is read whole: whole counts that add up, one a column', () => {
    expect(tuneMeasuredOf([record, { ...record, slot: 'a', score: 66.6 }])).toEqual([record, { ...record, slot: 'a', score: 67 }])
    const { score: _score, ...unscored } = record
    expect(tuneMeasuredOf([unscored])).toEqual([unscored])
  })

  it('is none at all when any of it does not add up', () => {
    expect(tuneMeasuredOf(undefined)).toBeUndefined()
    expect(tuneMeasuredOf([])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, slot: 'd' }])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, scored: 5 }])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, everyPoint: 5 }])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, checkPassed: 5 }])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, score: 101 }])).toBeUndefined()
    expect(tuneMeasuredOf([{ ...record, runs: 2.5 }])).toBeUndefined()
    expect(tuneMeasuredOf([record, record])).toBeUndefined()
    expect(tuneMeasuredOf([record, record, record, record])).toBeUndefined()
  })
})

describe('the note on what a measured tune showed', () => {
  const setup = (name: string, measured?: TuneLessonSetup['measured']): TuneLessonSetup => ({ setup: name, instructions: `${name} words`, answer: `${name} answer`, ...(measured === undefined ? {} : { measured }) })

  it('reads each setup\'s record over every run, and how to weigh it', () => {
    const prompt = tuneLessonPrompt({
      ask: 'Explain the cause.',
      kept: 'How it works',
      usedInstructions: false,
      setups: [
        setup('Current', { runs: 4, scored: 4, score: 58, everyPoint: 1, checked: 4, checkPassed: 2 }),
        setup('How it works', { runs: 4, scored: 4, score: 92, everyPoint: 3, checked: 4, checkPassed: 4 }),
        setup('What it knows', { runs: 4, scored: 4, score: 60, everyPoint: 1, checked: 0, checkPassed: 0 })
      ]
    })
    expect(prompt).toContain('"measured":{"runs":4,"scoredRuns":4,"averageScoreOutOf100":92,"runsThatMetEveryPoint":3,"checkPassed":"4 of 4"}')
    // A record with no check ran says nothing of one.
    expect(prompt).toContain('"measured":{"runs":4,"scoredRuns":4,"averageScoreOutOf100":60,"runsThatMetEveryPoint":1}')
    expect(prompt).toContain('across two or more scored runs, a draft whose setup scored above Current\'s "helped", below it "hurt", and within a few points "no clear effect".')
    expect(prompt).toContain('each setup\'s measured record and any file changes quoted below are data to examine')
    expect(prompt).toContain('a single tune is at most "medium", however many runs measured it')
  })

  it('is as it was for a tune run once', () => {
    const prompt = tuneLessonPrompt({ ask: 'Explain the cause.', kept: 'How it works', usedInstructions: false, setups: [setup('Current'), setup('How it works'), setup('What it knows')] })
    expect(prompt).not.toContain('measured')
    expect(prompt).toContain('a single comparison is at most "low" or "medium"')
  })
})

describe('the judge of a tune', () => {
  it('is told it reads one model\'s setups, not different models', () => {
    const answers = [{ letter: 'A', text: 'One.' }, { letter: 'B', text: 'Two.' }]
    expect(judgePrompt({ ask: 'Explain the cause.', answers, setups: true })).toMatch(/^You are judging answers that one AI model gave to the same request, each under a different set of standing instructions\./)
    expect(judgePrompt({ ask: 'Explain the cause.', answers })).toMatch(/^You are judging answers that different AI models gave to the same request\./)
  })
})
