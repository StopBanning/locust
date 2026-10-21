import { describe, expect, it } from 'vitest'
import { MEMORY_HEADING, MEMORY_RULES, MEMORY_TAG, MEMORY_WRITE, memorySection } from '../shared/memory.js'
import { composeSoloPrompt, MAX_RUNTIME_PROMPT_LENGTH } from './workroom-briefing.js'

/**
 * A NEW PROJECT IS TOLD HOW TO WRITE ITS FIRST MEMORY (0.810), and not how to read memories it does not have.
 * MEASURED 2026-10-21: the memory section was 2,687 characters with nothing remembered -- about 1,100 of them on how
 * to weigh notes that did not exist -- on the first turn of every new project.
 */
const empty = memorySection({ selfName: 'Wren', workspaceName: 'shop', memories: [], askFirst: false })
const one = memorySection({ selfName: 'Wren', workspaceName: 'shop', memories: [{ text: 'Port is 3001.', scope: 'workspace', by: 'you', where: undefined }], askFirst: false })

describe('a brief with nothing remembered', () => {
  it('says nothing is remembered and how to write the first memory, block and all', () => {
    expect(empty).toContain(`${MEMORY_HEADING} What is remembered for the folder "shop" and everywhere:\nNothing is remembered yet.`)
    expect(empty).toContain(MEMORY_WRITE)
    expect(empty).toContain(`<${MEMORY_TAG}>`)
    expect(empty).toContain('kept at once')
  })

  it('leaves out how to read memories until there is one, and brings it back with the first', () => {
    for (const reading of [MEMORY_RULES, 'colleague\'s notes', 'the file wins']) {
      expect(empty).not.toContain(reading)
      expect(one).toContain(reading)
    }
    expect(empty.length).toBeLessThan(1_800)
    expect(one.length - empty.length).toBeGreaterThan(1_000)
  })

  it('still gives the memory paragraphs up first when a run that belongs to nobody is over the cap (A5.3)', () => {
    const over = composeSoloPrompt({ prompt: 'x'.repeat(MAX_RUNTIME_PROMPT_LENGTH), memory: empty, keepATodoList: false }).prompt
    expect(over).not.toContain(MEMORY_HEADING)
    expect(over).not.toContain(MEMORY_WRITE)
    // The reply formats are not the part to lose.
    expect(over).toContain('To give the person a file')
  })
})
