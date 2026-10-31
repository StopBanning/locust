import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * AN ORB IS SHRUNK IN SOFTWARE (2026-10-30). Colin, on the Mac: "the thinking/working orbs appear to not look right on
 * mac or be as clear as they are as windows". The painted-down orb draws the 64 design four times larger and shrinks
 * it with 'high' smoothing; on the graphics card, macOS's Chromium shrank it 3.2 to 7.0 out of 255 away from the
 * exact average (Windows: 0.9). Both canvases are kept in software, where the same shrink is 0.4 to 0.7 on the Mac and
 * the same code path everywhere. _tools/look-orbs.mjs photographs the live orb on either system.
 */
describe('the painted-down orb', () => {
  const orb = readFileSync(fileURLToPath(new URL('./components/Orb.tsx', import.meta.url)), 'utf8')

  it('draws and shrinks in software, both canvases', () => {
    expect(orb).toContain("const shown = canvas.getContext('2d', { willReadFrequently: true })")
    expect(orb).toContain("const drawn = big.getContext('2d', { willReadFrequently: true })")
  })

  it('still shrinks a drawing four times larger, with high-quality smoothing', () => {
    expect(orb).toContain('export const PAINT_DOWN_FACTOR = 4')
    expect(orb).toContain("shown.imageSmoothingQuality = 'high'")
  })
})
