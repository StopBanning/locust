import { describe, expect, it } from 'vitest'
import { aboutYouSection } from './about-you.js'
import { memorySection } from './memory.js'

/**
 * THE ABOUT-YOU LINE IS TAUGHT ONCE (0.810). With the person's note in the brief, its section teaches `about you ::`
 * with the example that fixed Haiku (0.424), and the memory block listed the same line again on every turn. The block
 * lists it only when there is no note to teach it.
 */
const base = { workspaceName: 'pebble', memories: [], askFirst: false } as const
const taught = (text: string): number => text.split('\n').filter((line) => line.startsWith('about you ::')).length

describe('the about-you line, in a brief with memory on', () => {
  it('is taught by the section of the note alone, when there is a note', () => {
    const block = memorySection({ ...base, aboutYouGiven: true })
    expect(taught(block)).toBe(0)
    const brief = [aboutYouSection('Keep answers short.'), block].join('\n\n')
    expect(taught(brief)).toBe(1)
    expect(brief).toContain('about you :: Prefers a one-line summary before any detail.')
  })

  it('is still taught by the memory block when there is no note yet, so a first line can be suggested', () => {
    expect(taught(memorySection(base))).toBe(1)
  })
})
