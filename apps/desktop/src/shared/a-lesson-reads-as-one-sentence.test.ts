import { describe, expect, it } from 'vitest'
import { tuneLessonLine } from './tune.js'

/**
 * A LESSON READS AS ONE SENTENCE (0.810). The model writes "when" and "do" as sentences of their own, and the 0.810
 * drive showed "When A teammate's standing instructions …, do Add a line …".
 */
describe('a lesson, as the strip and Edit teammate say it', () => {
  it('joins the two fields as one sentence, lower-casing a capitalised first word', () => {
    expect(tuneLessonLine({ when: 'A teammate gives a short answer', do: 'Add a line capping its length.' })).toBe('When a teammate gives a short answer, add a line capping its length.')
    expect(tuneLessonLine({ when: 'When explaining a cause.', do: 'Do: check assumptions before concluding.' })).toBe('When explaining a cause, check assumptions before concluding.')
  })

  it('keeps an acronym or a name as written', () => {
    expect(tuneLessonLine({ when: 'API errors repeat', do: 'UI text names the step that failed' })).toBe('When API errors repeat, UI text names the step that failed.')
  })
})
