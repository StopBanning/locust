import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { COVER_CAST, COVER_MACHINE, COVER_WIDTH, HomeCover, coverScale } from './components/HomeCover.js'
import { THINKER, THINKER_OFFSET, WORKER, WORKER_OFFSET, beatAt, loopSeconds } from './coverScript.js'

/**
 * THE HOME SCREEN IS THE DESIGN SYSTEM'S COVER.
 *
 * Colin, 2026-09-22, of 0.270's home screen, which had the cover's lockup and
 * nothing else: "it can be a 1:1 once you fix the text description under
 * locust tbh ... cause right now it just looks like the locust logo and crt
 * shipped". So the card carries the whole cover -- lockup, the claim under
 * it, and the three teammates -- drawn to the column's width.
 *
 * AS A MACHINE (A2, 0.295). Colin: "the teammates could have a structure to
 * be on top of, the border, crt effect, would almost make it look like a
 * machine", then of the sample: "make the screen bigger and put the text in
 * it as well". The boot screen's bezel and glass, centred; the lockup and the
 * claim on the glass; the three standing on the bezel.
 */

const botAttr = (html: string, name: string): string[] =>
  [...html.matchAll(new RegExp(`class="lc-bot[^"]*"[^>]*${name}="([a-z-]+)"`, 'g'))].map((match) => match[1] ?? '')

describe('the cover, drawn to the column', () => {
  it('is the cover at the column width: 960 is its own size, 760 is 0.79 of it', () => {
    expect(coverScale(COVER_WIDTH)).toBe(1)
    expect(coverScale(760)).toBe(0.792)
    expect(coverScale(980)).toBe(1.021)
  })

  it('starts at the narrowest column before it has been measured', () => {
    expect(coverScale(0)).toBe(0.792)
    expect(coverScale(Number.NaN)).toBe(0.792)
  })
})

describe('what the home screen draws', () => {
  const before = renderToStaticMarkup(<HomeCover ready={false} tube="full" />)
  const after = renderToStaticMarkup(<HomeCover ready tube="full" />)

  it('has the lockup and, under it, the claim -- both on the machine\'s glass', () => {
    expect(after).toContain('aria-label="Locust"')
    expect(after).toContain('<p class="lc-cover__claim">Autonomous teammates on your own machine</p>')
    // Inside the glass, the lockup first and the claim after it.
    const glass = after.slice(after.indexOf('class="lc-cover__glass"'))
    expect(glass.indexOf('lc-lockup')).toBeGreaterThan(0)
    expect(glass.indexOf('lc-lockup')).toBeLessThan(glass.indexOf('lc-cover__claim'))
    // Under the tube's scanlines, like the boot screen's.
    expect(glass.indexOf('lc-cover__scan')).toBeLessThan(glass.indexOf('lc-lockup'))
  })

  it('has the three teammates on the machine, in the cover order: a ghost, Prompt, a Locust', () => {
    expect(after).toContain('class="lc-cover__machine"')
    expect(after).not.toContain('lc-cover__plate')
    // They stand on its top edge: every bot's box ends near the bezel's top (Prompt's lowest: its
    // body ends at 86 of its box's 100, so its box sinks 7 to stand rather than hover).
    for (const mate of COVER_CAST) expect(Math.abs(mate.y + 96 - COVER_MACHINE.y)).toBeLessThanOrEqual(18)
    expect(COVER_CAST.map((mate) => mate.key)).toEqual(['wren', 'atlas', 'sable'])
    // Colin: "lets definitely include ghost in there"; 0.562: "maybe use ghost, the codex looking one and our locust".
    expect(botAttr(after, 'data-bot')).toEqual(['ghost', 'prompt', 'critter'])
    expect(botAttr(after, 'data-state')).toEqual(['default', 'default', 'sleeping'])
    // Each in its own part from the first frame: thinking, typing, asleep.
    expect(botAttr(after, 'data-beat')).toEqual(['thinking', 'typing', 'asleep'])
  })

  it("has a white ghost that floats, and Claw'd asleep (0.583: the Hopper was jagged in Plush)", () => {
    // Colin: "maybe make the ghost white and the locust green lol", and of
    // its hops: "a little loud for a title screen, especially for a ghost".
    const ghost = COVER_CAST.find((mate) => mate.type === 'ghost')
    const locust = COVER_CAST.find((mate) => mate.part === 'sleeper')
    expect(ghost?.hue).toBeUndefined()
    expect(ghost?.floats).toBe(true)
    expect(locust).toMatchObject({ type: 'critter', hue: 'clay' })
    expect(after).toContain('class="lc-bot is-floating" data-bot="ghost"')
    // Nothing floats before the runtimes have answered.
    expect(before).not.toContain('is-floating')
  })

  it('wakes the teammates with the lockup: still and unmarked until the runtimes answer', () => {
    expect(botAttr(before, 'data-state')).toEqual(['still', 'still', 'still'])
    expect(before).not.toContain('lc-presence')
    expect(before).not.toContain('lc-bot__ring')
    expect(before).not.toContain('lc-cover__zzz')
    // Then the thinker and the worker play, and Sable sleeps, its z's drifting up -- all three unmarked
    // (0.610): a presence dot on a character said a state nobody was in ("two wear the live dot while
    // your team is idle", Colin's mockup, 2026-10-04). Who is working is said on the glass.
    expect(after).not.toContain('lc-presence')
    // The worker waits on you only in its waiting beat (coverScript), not from the first frame.
    expect(after).not.toContain('lc-bot__ring')
    expect(after).toContain('class="lc-cover__zzz"')
  })

  it('places each bot on the cover grid at the starting scale, in whole pixels', () => {
    // Wren is 96 across at (245, -10) on the cover; at 0.792 that was 76 at (194, -8). Home draws the machine a size
    // smaller since the 0.863 polish (COVER_HOME_SCALE 0.9): 0.713, so 68 at (213, -7), still centred on the card.
    expect(after).toMatch(/<span class="lc-cover__face" style="left:213px;top:-7px">/)
    expect(after).toMatch(/class="lc-bot is-floating" data-bot="ghost" data-state="default" data-beat="thinking" style="width:68px;height:68px"/)
  })

  it('is decorative apart from its words: no bot answers to a name', () => {
    const canvases = after.match(/<canvas[^>]*>/g) ?? []
    expect(canvases).toHaveLength(3)
    expect(canvases.every((canvas) => canvas.includes('aria-hidden="true"'))).toBe(true)
  })
})

/**
 * THE THREE PLAY THEIR PARTS (0.562). Colin, 2026-10-03: "make it so some of
 * them have different terminal eyes its just all the same sloppy set". Prompt
 * works, the ghost thinks, on loops of different lengths from different
 * starts, so the two awake ones are almost never wearing the same face.
 */
describe('the three play their parts', () => {
  it('runs the worker on a 17 s loop and the thinker on a 13 s one', () => {
    expect(loopSeconds(WORKER)).toBeCloseTo(17)
    expect(loopSeconds(THINKER)).toBeCloseTo(13)
  })

  it('keeps the worker and the thinker in different faces nearly all of the time', () => {
    let same = 0
    let samples = 0
    for (let t = 0; t < 221; t += 0.25) {
      const worker = beatAt(WORKER, t + WORKER_OFFSET).beat
      const thinker = beatAt(THINKER, t + THINKER_OFFSET).beat
      samples += 1
      if ((worker.eyes?.join('') ?? 'rest') + worker.phosphor === (thinker.eyes?.join('') ?? 'rest') + thinker.phosphor) same += 1
    }
    expect(same / samples).toBeLessThan(0.12)
  })

  it('shows the states a teammate really has, in their real colours', () => {
    const done = WORKER.find((beat) => beat.name === 'done')
    expect(done?.eyes?.join('')).toBe('^^')
    expect(done?.phosphor).toBe('green')
    expect(done?.hop).toBe(true)
    const waiting = WORKER.find((beat) => beat.waiting === true)
    expect(waiting?.phosphor).toBe('amber')
    // The ghost floats: it never hops.
    expect(THINKER.some((beat) => beat.hop === true)).toBe(false)
  })

  it('says how long until the next beat, so the cover renders only at a change', () => {
    expect(beatAt(WORKER, 0).beat.name).toBe('typing')
    expect(beatAt(WORKER, 6.5).beat.name).toBe('done')
    expect(beatAt(WORKER, 6).left).toBeCloseTo(0.5)
    expect(beatAt(WORKER, 17 + 6.5).beat.name).toBe('done')
  })
})

describe('the cover\'s glass while work runs (0.610)', () => {
  it('says who is working in place of the claim, and the claim when nothing runs', () => {
    const working = renderToStaticMarkup(<HomeCover ready tube="full" status="Casper waiting on you · Codex working" />)
    expect(working).toContain('<p class="lc-cover__claim is-status">Casper waiting on you · Codex working</p>')
    expect(working).not.toContain('Autonomous teammates on your own machine')
    const idle = renderToStaticMarkup(<HomeCover ready tube="full" />)
    expect(idle).toContain('Autonomous teammates on your own machine')
  })
})
