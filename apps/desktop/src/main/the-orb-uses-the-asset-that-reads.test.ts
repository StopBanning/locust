import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * Which drawing each orb uses, pinned — because it was decided by looking and
 * nothing else in the suite can see it.
 *
 * The library ships TWO drawings per orb and says so in its own types:
 * *"Exactly two tuned presets ship: 64 (chat-avatar scale) and 20
 * (inline-text scale). Each size carries its own dot count, dot size and
 * speed tuning — they are separate designs, not a scale factor."*
 *
 * Locust shipped the 20 design everywhere, and Colin held the app up next to
 * the library's page: *"the one we have set for 'using a tool' doesnt even
 * look like any of the ones in that asset pack lol"*. Nothing had been
 * redrawn — all 23 files are byte-identical to the published tarball. It was
 * the other design: at 20 the web keeps 19% of its points at 1.5x the size.
 *
 * The split is by what carries the shape. Density shapes take the 64 asset
 * painted down; OUTLINE shapes keep the 20 design, because at 64 their lines
 * are hairline-thin and vanish into the panel.
 */

const SRC = fileURLToPath(new URL('../renderer/src/', import.meta.url))
const orb = readFileSync(`${SRC}components/Orb.tsx`, 'utf8')
const css = readFileSync(`${SRC}shell.css`, 'utf8')

describe('the orb uses the asset that reads', () => {
  it('gives the density shapes the 64 asset', () => {
    const set = orb.slice(orb.indexOf('const DENSE'), orb.indexOf('export function Orb'))
    for (const state of ['composing', 'listening', 'solving', 'searching', 'connecting', 'weaving']) {
      expect(set).toContain(`'${state}'`)
    }
  })

  it('leaves the outline shapes on their own 20px design', () => {
    const set = orb.slice(orb.indexOf('const DENSE'), orb.indexOf('export function Orb'))
    // `shaping` is a dotted square and `breathing` a dotted ring. The 20
    // design fattens them on purpose; the 64 one is hairline.
    expect(set).not.toContain("'shaping'")
    expect(set).not.toContain("'breathing'")
    // The outline shapes keep the library's own 20px component; the density
    // shapes are the 64 design, painted down (see the next test).
    expect(orb).toContain('size={20}')
    expect(orb).toContain('resolvePreset(state, 64)')
  })

  it('keeps the sidebar row off the orbs: it wears the spark, a glyph, which no scale can smudge (0.392)', () => {
    /*
     * The row wore the dotted outline from 0.225 to 0.391 and could never be
     * shrunk -- at 0.75 and 0.85 of its 20px it was a grey smudge. Colin,
     * 2026-09-27: "getting outdated". It is Claude Code's spinner now
     * (WorkingSpark.tsx), and the rule that let the outline spill from the
     * dot's box is gone with it.
     */
    const sidebar = readFileSync(`${SRC}components/Sidebar.tsx`, 'utf8')
    expect(sidebar).not.toContain('ThinkingOrb')
    // Since 0.839 a running row shimmers its title instead (Colin, 2026-10-28: "too distracting" with many chats).
    expect(sidebar.split('<WorkingSpark />').length - 1).toBe(0)
    expect(sidebar.split('data-orb="shimmer"').length - 1).toBe(2)
    expect(css).not.toContain('.lc-row__orb')
  })

  it('paints the 64 design down with a filter that looks at every pixel', () => {
    // 2026-09-22: the CSS shrink of the 64 canvas (2.46x) sampled some of a
    // dot's pixels and skipped others -- error 10.4 against the ideal 26px
    // picture, flickering frame to frame. Drawn 4x and shrunk with 'high'
    // smoothing: 0.9, steady.
    expect(orb).toContain('export const PAINT_DOWN_FACTOR = 4')
    expect(orb).toContain("imageSmoothingQuality = 'high'")
    expect(orb).toContain('MODE_DRAWS[mode]')
  })

  it('never scales a raster to get a bigger orb', () => {
    // The first attempt, and Colin's verdict on it: "you just cooked the
    // resolution". A bigger orb is a bigger ASSET painted down, never a
    // transform on the small one.
    expect(css).not.toContain(".lc-livestep__orb[data-orb='")
    expect(orb).not.toContain('scale(')
  })

  it('gives the live line a box big enough for the asset', () => {
    const box = css.slice(css.indexOf('.lc-livestep__orb {'))
    expect(box.slice(0, 200)).toContain('width: 26px')
  })

  it('gives the plan card one big orb and one small one', () => {
    /*
     * THE SIZE IS THE WHOLE ARGUMENT, so both ends are pinned.
     *
     * The rubik's identity is scrambling bands, which an 11px row marker
     * cannot hold -- the same finding that cut the web and the braid, landing
     * on the orb we had just fought to keep. It moved to the card header,
     * where there is room and where "this plan is running" is a card-level
     * claim anyway. The step went back to a marker-sized mark.
     */
    const head = css.slice(css.indexOf('.lc-plancard__orb {'))
    expect(head.slice(0, 200)).toContain('width: 24px')
    const step = css.slice(css.indexOf('.lc-plan__orb {'))
    // 20 is the inline drawing's own size: the step asks for that preset by
    // name, so the box is the picture rather than a scale of it.
    expect(step.slice(0, 260)).toContain('width: 20px')
  })
})
