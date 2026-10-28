import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { filesHeading } from './components/Thread.js'

/**
 * AN UNDONE TURN SAYS SO ON ITS CARD (0.840). The clean-machine drive of 2026-10-28: after Undo put laptop-setup.md
 * back, the turn's card still read "Edited 1 file" with the file "new +27", and only a line under it said otherwise.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('an undone turn says so on its card', () => {
  it('adds undone to the heading, and only once Undo has put files back', () => {
    expect(filesHeading(1, 0)).toBe('Edited 1 file')
    expect(filesHeading(1, 0, true)).toBe('Edited 1 file · undone')
    expect(filesHeading(2, 1, true)).toBe('Edited 2 files · 1 more changed in the folder · undone')
    expect(filesHeading(0, 3, true)).toBe('3 files changed in the folder while it ran · undone')
  })

  it('is told by the Undo below the card when files were put back, not when nothing was', () => {
    const undo = read('./components/TurnUndo.tsx')
    expect(undo).toContain("const putBack = state?.kind === 'undone' && state.restored.length > 0")
    expect(undo).toContain('if (putBack) onUndone?.()')
    const thread = read('./components/Thread.tsx')
    expect(thread).toContain('<TurnUndo runId={item.runId} onUndone={markUndone} />')
    expect(thread).toContain('const heading = filesHeading(edited, seen, undone)')
  })
})
