import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * The up arrow and Escape work on an ordinary composer, not only inside a menu.
 *
 * Ported from Claude Code on Colin's standing ask (2026-09-09): "if there's
 * any other quirks or cool features of Claude code that you can port over
 * that would be great."
 *
 * Both keys were already handled — but only in the branch that runs WHILE
 * THE SLASH MENU IS OPEN, where they move the menu selection and clear the
 * box. On an empty composer, with no menu, neither did anything: no recall
 * of the last message, no way to stop a run from the keyboard.
 *
 * That is the shape this guards. A future edit that moves either key back
 * inside the slash branch would restore exactly the old behaviour while
 * still looking, in a diff, like the key is handled.
 *
 * The behaviour itself is driven, not asserted here —
 * `_tools/probe-composer-keys.mjs` presses the real keys on the built app.
 * This session already shipped a feature whose two halves each had a passing
 * test and which was invisible on every runtime, so a source check gets to
 * guard a shape and nothing more.
 */

const COMPOSER = readFileSync(
  fileURLToPath(new URL('../renderer/src/components/Composer.tsx', import.meta.url)),
  'utf8'
)

/** One brace-matched block, starting at the first `{` on or after `from`. */
function blockAt(source: string, from: number): { readonly start: number; readonly end: number } {
  const start = source.indexOf('{', from)
  let depth = 0
  for (let i = start; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1
    else if (source[i] === '}') {
      depth -= 1
      if (depth === 0) return { start, end: i }
    }
  }
  return { start, end: source.length }
}

/** The handler with the slash-menu branch cut out of it. */
function outsideTheSlashMenu(source: string): string {
  /*
   * Bounded to the handler's OWN block.
   *
   * The first version sliced from `const keyDown =` to the end of the file,
   * so everything after the handler came with it -- including the slash
   * menu's own `setSlashAt` calls elsewhere in the component. The control
   * below caught it, which is the only reason this comment exists.
   */
  const at = source.indexOf('const keyDown =')
  const body = blockAt(source, at)
  const handler = source.slice(at, body.end + 1)
  const guard = handler.indexOf('if (slashChoices.length > 0) {')
  if (guard < 0) return handler
  // What is left is the rest of the handler: the part that runs with no
  // menu open, which is where both keys had to move.
  const branch = blockAt(handler, guard)
  return handler.slice(0, guard) + handler.slice(branch.end + 1)
}

describe('the composer’s keys', () => {
  it('can tell the slash-menu branch from the rest', () => {
    // The control. If the branch stops being found, everything below would
    // pass by searching the whole handler including the menu's own keys.
    expect(COMPOSER).toContain('if (slashChoices.length > 0) {')
    const rest = outsideTheSlashMenu(COMPOSER)
    expect(rest.length).toBeGreaterThan(200)
    expect(rest).not.toContain('setSlashAt')
  })

  it('recalls the last message from an empty box', () => {
    const rest = outsideTheSlashMenu(COMPOSER)
    expect(rest, 'ArrowUp is handled only inside the slash menu again').toContain("keyEvent.key === 'ArrowUp'")
    expect(rest).toContain("keyEvent.key === 'ArrowDown'")
  })

  it('stops a run on Escape, and only when nothing else would take it', () => {
    const rest = outsideTheSlashMenu(COMPOSER)
    expect(rest, 'Escape is handled only inside the slash menu again').toContain("keyEvent.key === 'Escape'")
    /*
     * The guard matters as much as the key. A panel's own Escape closes it,
     * and one keystroke that both dismissed a menu AND killed a mission
     * would be the worst kind of surprise -- one of those is free and the
     * other is not.
     */
    // The effort choice is the foot of the model picker, so an open picker is
    // the panel Escape closes before it may stop a run.
    expect(rest).toMatch(/Escape'\s*&&\s*running\s*&&\s*!modeOpen\s*&&\s*!pickerOpen/)
    expect(rest).not.toContain('effortOpen')
  })
})
