import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * A PET CANVAS DRAWN SHRUNK IS IN SOFTWARE (2026-10-30). Measured on a Mac with the pet sheet Locust carries, at 26, 40
 * and 56 px: a frame shrunk from the sheet as a picture or an ImageBitmap is 0.3 to 1.5 out of 255 off the exact
 * picture, whether drawn on the graphics card or not; the same pixels shrunk from a canvas kept on the graphics card
 * are 2.6 to 5.1 off, wherever they are drawn, and from a software canvas 0.3 to 1.5 again. So every canvas Codex
 * Buddy's rig cuts from his sheet and draws shrunk onto him -- his head and his face's glass -- is made in software,
 * as the orbs' are (an-orb-is-shrunk-in-software.test.ts). The sheets themselves are ImageBitmaps, and stay so.
 */
describe('the canvases a pet is drawn from', () => {
  const buddy = readFileSync(fileURLToPath(new URL('./buddyBody.ts', import.meta.url)), 'utf8')
  const pets = readFileSync(fileURLToPath(new URL('./pets.ts', import.meta.url)), 'utf8')

  it("are all in software in Codex Buddy's rig, his face's glass included", () => {
    const contexts = [...buddy.matchAll(/getContext\('2d'[^)]*\)/g)].map((match) => match[0])
    expect(contexts.length).toBeGreaterThanOrEqual(3)
    expect(contexts.every((call) => call.includes('willReadFrequently: true'))).toBe(true)
    expect(buddy).toContain("const context = canvas.getContext('2d', { willReadFrequently: true })\n  if (context === null) return false\n  const { box } = face")
  })

  it('come from a sheet read as an ImageBitmap, which shrinks well on every system', () => {
    expect(pets).toContain("createImageBitmap(new Blob([new Uint8Array(bytes)], { type: 'image/webp' }))")
  })
})
