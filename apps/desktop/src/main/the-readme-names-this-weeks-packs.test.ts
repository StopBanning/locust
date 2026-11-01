import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

import { TRENDING_SKILLS } from '../shared/popular-skills.js'

/**
 * THE README NAMES THIS WEEK'S PACKS (0.849). The shelf moved to Unslop, tstack, pixelfx and Dimensional Drawing in
 * 0.847, and the README's New this week line still listed the week before's four (the PC chat caught it, 2026-11-01).
 * A shelf refresh now has to bring the README with it.
 */
describe("the README's New this week line", () => {
  it('names the packs the app shows, in its order', () => {
    const readme = readFileSync(fileURLToPath(new URL('../../../../README.md', import.meta.url)), 'utf8').replace(/\s+/g, ' ')
    const line = /a \*\*New this week\*\* row of the packs climbing fastest right now \(([^)]*)\)/.exec(readme)?.[1]
    expect(line).toBe(TRENDING_SKILLS.map((pack) => pack.name).join(', '))
  })
})
