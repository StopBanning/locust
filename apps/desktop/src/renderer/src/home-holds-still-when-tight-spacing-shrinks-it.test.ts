import { describe, expect, it } from 'vitest'

import { homeIsShort, sectionsAtNormalSpacing, type HomeSpacingSeen } from './components/HomeCover.js'

/**
 * HOME HOLDS STILL WHEN TIGHT SPACING SHRINKS IT (0.838). Colin, 2026-10-27: "twitchy home screen bug is back". Tight
 * spacing also shortens the sections (the accounts block, the cards, the starters), so a Home just short of room went
 * tight, measured shorter, went loose, measured longer, and flicked every frame: six teammates from 882px tall, three
 * at 766px (drive-home-holds-still --team). Measured from the drive: three teammates, loose 409px, tight 395px.
 */
const FOOTPRINT = 220
// A pane in the band: short at the normal spacing, not short as drawn tight.
const PANE = Array.from({ length: 800 }, (_, i) => 400 + i).find((pane) => homeIsShort(409, 4, FOOTPRINT, pane) && !homeIsShort(395, 4, FOOTPRINT, pane)) ?? 0

/** One measure as Home runs it: the class it has now, the sections as drawn at that class, the decision it makes. */
function step(seen: HomeSpacingSeen, tightNow: boolean): { readonly seen: HomeSpacingSeen; readonly tight: boolean } {
  const drawn = tightNow ? 395 : 409
  const normal = sectionsAtNormalSpacing(seen, 4, drawn, tightNow)
  return { seen: normal.seen, tight: homeIsShort(normal.sections, 4, FOOTPRINT, PANE) }
}

describe('Home holds still when tight spacing shrinks it', () => {
  it('the two heights argue each other: short at the normal spacing, not short as drawn tight', () => {
    expect(PANE).toBeGreaterThan(0)
    expect(homeIsShort(409, 4, FOOTPRINT, PANE)).toBe(true)
    expect(homeIsShort(395, 4, FOOTPRINT, PANE)).toBe(false)
  })

  it('goes tight once and stays tight', () => {
    let seen: HomeSpacingSeen = { count: -1 }
    let tight = false
    const classes: boolean[] = []
    for (let i = 0; i < 12; i += 1) {
      const next = step(seen, tight)
      seen = next.seen
      tight = next.tight
      classes.push(tight)
    }
    expect(classes).toEqual(Array.from({ length: 12 }, () => true))
  })

  it('adds back what tight took, and only while tight', () => {
    const loose = sectionsAtNormalSpacing({ count: -1 }, 4, 409, false)
    expect(loose.sections).toBe(409)
    const tight = sectionsAtNormalSpacing(loose.seen, 4, 395, true)
    expect(tight.sections).toBe(409)
    // Content that grows while tight still counts in full.
    expect(sectionsAtNormalSpacing(tight.seen, 4, 420, true).sections).toBe(434)
  })

  it('a new count of sections starts over rather than borrowing an old height', () => {
    const loose = sectionsAtNormalSpacing({ count: -1 }, 4, 409, false)
    const tight = sectionsAtNormalSpacing(loose.seen, 5, 395, true)
    expect(tight.sections).toBe(395)
    expect(tight.seen.took).toBeUndefined()
  })
})
