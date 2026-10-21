import { describe, expect, it } from 'vitest'
import { tuneLessonPrompt } from './tune-lessons.js'

/**
 * A KEPT SETUP IS NOT PROOF ITS EDIT HELPED (0.810), after Harness²'s own warning: adoption says which output read
 * best, not which edit worked, and "an invented attribution poisons the library for every later task". The first
 * prompt had a kept setup's edit called "helped" and a lesson written from any such row, so nearly every tune would
 * have left one.
 */
const prompt = tuneLessonPrompt({
  ask: 'Explain the cause.',
  setups: [
    { setup: 'Current', instructions: 'Be concise.', answer: 'A.' },
    { setup: 'How it works', instructions: 'Check your work.', why: 'It stopped before checking.', answer: 'B.' },
    { setup: 'What it knows', instructions: 'Name your assumptions.', why: 'It guessed.', answer: 'C.' }
  ],
  kept: 'How it works',
  usedInstructions: true
})

describe('noting what a tune showed', () => {
  it('judges each edit from the runs, never from which setup the person kept', () => {
    expect(prompt).toContain('says which answer they preferred, not that its edit caused the difference')
    expect(prompt).toContain('point to the moment in a run')
    expect(prompt).not.toMatch(/use "helped" when its run followed/)
  })

  it('writes a lesson only when it would change work on a different ask, and no lesson is a normal outcome', () => {
    expect(prompt).toContain('would change how this teammate should work on a different ask')
    expect(prompt).toContain('which is a normal outcome')
    expect(prompt).not.toMatch(/When any edit row is "helped" or "hurt", write one lesson/)
  })
})
