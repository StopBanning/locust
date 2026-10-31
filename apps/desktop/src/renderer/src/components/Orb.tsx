import { useEffect, useRef, useState } from 'react'
import { MODE_DRAWS, ThinkingOrb, resolvePreset } from 'thinking-orbs'
import { DRAWING_BEAT } from '../frameBeat.js'
import { useDrawnTheme } from '../drawnTheme.js'

import type { OrbState } from 'thinking-orbs'
import type { ReactElement } from 'react'

/**
 * One orb, drawn from the asset that actually looks like the asset.
 *
 * THE LIBRARY SHIPS TWO DRAWINGS PER ORB, and its own types say why:
 * *"Exactly two tuned presets ship: 64 (chat-avatar scale) and 20
 * (inline-text scale). Each size carries its own dot count, dot size and
 * speed tuning — they are separate designs, not a scale factor."*
 *
 * The 20 preset is therefore a deliberate design, not a degraded one — but
 * it is a different picture, and how different is measurable from the
 * library's own preset table: at 20 the rubik keeps 25% of its points at
 * 1.8x the dot size, and the web keeps 19% at 1.5x. Colin, looking at the
 * shipped app beside the library's page: *"the one we have set for 'using a
 * tool' doesnt even look like any of the ones in that asset pack lol"*. He
 * was right, and nothing had been redrawn — all 23 files are byte-identical
 * to the published tarball. We were simply showing the other design.
 *
 * So this renders the 64 asset into a box smaller than 64. That is a
 * DOWNSCALE, which is sharp — the opposite of the first attempt, a CSS
 * transform on the 20px raster, which Colin correctly called cooking the
 * resolution.
 *
 * `box` is the space it gets, and it is a real dial rather than a constant
 * because some of these need more room than others to read at all. Colin,
 * setting the expectation before the work: *"its certainly possible once we
 * do this that some of these assets will either need to be given more
 * space/size to be legibile or we might have to scrap them altogether for
 * ones that are more visible to the naked eye"*.
 *
 * WHERE THE 20 PRESET IS STILL RIGHT: below about 18px there is not enough
 * room for the 64 asset's point count and it turns to mud. The sidebar's
 * conversation row is that case, and it keeps the inline design it was
 * designed for.
 */
/**
 * Which shapes the 64 asset actually improves, decided by rendering all nine
 * in the live line's own geometry and looking.
 *
 * THE RULE THE PICTURES MADE: the 64 asset wins wherever a shape's identity
 * is carried by POINT DENSITY -- the rubik's bands, the web's constellation,
 * the braid's strands, the globe's and wave's surfaces, the ribbon's sash.
 * At 20 those are a handful of fat dots and the shape is unrecognisable,
 * which is exactly what Colin saw: *"the one we have set for 'using a tool'
 * doesnt even look like any of the ones in that asset pack lol"*.
 *
 * It LOSES wherever the shape is an OUTLINE. `shaping` is a dotted square and
 * `breathing` a dotted ring: at 64 their outlines are hairline-thin and fade
 * into the panel, while the 20 preset deliberately fattens them, which is the
 * whole reason that design exists. Colin predicted this before the work --
 * *"some of these assets will either need to be given more space/size to be
 * legibile or we might have to scrap them altogether"* -- and the answer for
 * those two is neither: they already have a design tuned for this size.
 *
 * So the preset is chosen per shape. Mixing them is not an inconsistency: the
 * nine are different drawings anyway, and the thing that must be consistent
 * is that each one READS.
 */
const DENSE: ReadonlySet<OrbState> = new Set([
  'composing', 'listening', 'solving', 'searching', 'connecting', 'weaving'
])

export function Orb({
  state,
  box,
  label,
  preset
}: {
  readonly state: OrbState
  /** CSS pixels the drawing gets. 64 is painted down into this. */
  readonly box: number
  readonly label?: string
  /**
   * Force one of the library's two drawings, where the default is wrong for
   * this surface.
   *
   * There is exactly one caller: the plan's step. `listening` is a density
   * shape, so it takes the 64 asset on the live line -- but at marker size
   * the inline drawing is the one that reads, and it is also usefully
   * DIFFERENT from the same shape on the line below. Colin picked it out of
   * a contact sheet by pointing at that exact cell: *"the original small
   * .221 20 preset @ 20 pxl its honestly different enough from everything
   * else"*.
   */
  readonly preset?: 20 | 64
}): ReactElement {
  // The page's theme, not the computer's (0.789, drawnTheme.ts).
  const theme = useDrawnTheme()
  const dense = preset === undefined ? DENSE.has(state) : preset === 64
  if (dense) return <PaintedDown state={state} box={box} label={label} />
  return (
    <ThinkingOrb
      state={state}
      size={20}
      theme={theme}
      // An outline shape keeps its own 20px drawing at its own 20px size,
      // centred in the same box. Painting it into 26 would stretch a design
      // tuned for 20 and undo the point of keeping it.
      style={{ display: 'block' }}
      {...(label === undefined ? { 'aria-hidden': true } : { 'aria-label': label })}
    />
  )
}

/** How many times larger the 64 design is drawn before it is painted down. */
export const PAINT_DOWN_FACTOR = 4

/** How often a moving orb is drawn, at most, a second: the bots' rate (0.543). */
export const ORB_FRAMES_PER_SECOND = 30

/**
 * THE 64 DESIGN, PAINTED DOWN WELL.
 *
 * Colin, 2026-09-22: "make sure they are as sharp as they can be ... is there
 * anyway you would improve on them visually". The designs were never the
 * problem -- every file is the published tarball's -- but the LAST STEP was.
 * The library sizes its canvas by its preset, 64 CSS px, and this box is 26,
 * so the browser shrank a finished 64px bitmap by 2.46 on every frame. At a
 * ratio that is not a whole number, and more than two, the compositor's
 * filter samples some of a dot's pixels and skips others: dots flared,
 * vanished, and swapped as they moved.
 *
 * MEASURED against the best picture 26 device pixels can hold -- the same
 * frame drawn 8x larger and averaged down exactly -- at eight moments of all
 * six density shapes (scratchpad orbs/score.py, 2026-09-22):
 *
 *   the library's 64 canvas shrunk by CSS  mean error 10.4, swinging up to 1.3 frame to frame
 *   drawn by the engine straight at 26     17.7 (sub-pixel dots are painted fat)
 *   drawn at exactly 2x, shrunk by CSS      5.3
 *   drawn at 4x, shrunk with 'high'         0.9, steady to 0.08
 *
 * So: the library's own engine and presets (`resolvePreset`, `MODE_DRAWS`,
 * the 64 design, its speed), drawn four times larger than the box, and
 * shrunk by the canvas with high-quality smoothing -- the same "bigger asset
 * painted down" rule as before, done by a filter that looks at every pixel.
 * Everything else the library does is kept: a still frame for reduced
 * motion, and no drawing while the orb is off screen or the window hidden.
 */
function PaintedDown({
  state,
  box,
  label
}: {
  readonly state: OrbState
  readonly box: number
  readonly label: string | undefined
}): ReactElement {
  const ref = useRef<HTMLCanvasElement>(null)
  const [reduced, setReduced] = useState(
    () => typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true
  )

  useEffect(() => {
    const query = typeof window === 'undefined' ? undefined : window.matchMedia?.('(prefers-reduced-motion: reduce)')
    if (query === undefined) return undefined
    const change = (event: MediaQueryListEvent): void => setReduced(event.matches)
    query.addEventListener('change', change)
    return () => query.removeEventListener('change', change)
  }, [])

  useEffect(() => {
    const canvas = ref.current
    if (canvas === null) return undefined
    // The library's own rule for pixel density, capped at 2.
    const dpr = Math.min(2, window.devicePixelRatio || 1)
    const side = Math.round(box * dpr)
    canvas.width = side
    canvas.height = side
    /*
     * BOTH CANVASES DRAWN IN SOFTWARE (2026-10-30). Colin, on the Mac: "the thinking/working orbs appear to not look
     * right on mac or be as clear as they are as windows". On the graphics card, macOS's Chromium does not honour
     * 'high' for a 4:1 shrink: measured against the exact 8x average, at six moments of each density shape, the
     * painted-down orb was 3.2 to 7.0 out of 255 off (composing worst), where Windows measured 0.9. Kept in software
     * (`willReadFrequently`), the same shrink is 0.4 to 0.7 on the Mac, and the same code path on every system. Two
     * small canvases at 30 frames a second, and only while a turn is running.
     */
    const shown = canvas.getContext('2d', { willReadFrequently: true })
    const big = document.createElement('canvas')
    big.width = side * PAINT_DOWN_FACTOR
    big.height = side * PAINT_DOWN_FACTOR
    const drawn = big.getContext('2d', { willReadFrequently: true })
    if (shown === null || drawn === null) return undefined
    const { mode, speed, opts } = resolvePreset(state, 64)
    const paint = MODE_DRAWS[mode]
    const k = big.width / 64
    const frame = (t: number): void => {
      drawn.setTransform(k, 0, 0, k, 0, 0)
      drawn.clearRect(0, 0, 64, 64)
      paint(drawn, 64, t, true, opts)
      shown.clearRect(0, 0, side, side)
      shown.imageSmoothingEnabled = true
      shown.imageSmoothingQuality = 'high'
      shown.drawImage(big, 0, 0, side, side)
    }
    if (reduced) {
      // The library's own still: the same moment it shows.
      frame(0.6)
      return undefined
    }
    let handle = 0
    let running = false
    /*
     * At most 30 drawings a second, as the bots (BOT_FRAMES_PER_SECOND).
     * MEASURED 0.543, three teammates streaming: this one 26px orb, drawn at
     * every animation frame (60 a second, 144 on a gaming laptop's screen),
     * was the largest single cost in the window while a reply arrived --
     * more than React and the reply's text together. Its time is still the
     * clock's, so it moves at the same speed; it is only drawn less often.
     */
    // On the window's one beat with the bots (frameBeat.ts, 0.654): drawn in the same frames as they are.
    const loop = (now: number): void => {
      if (DRAWING_BEAT.due(now)) frame((performance.now() / 1000) * speed)
      if (running) handle = requestAnimationFrame(loop)
    }
    const start = (): void => {
      if (running) return
      running = true
      handle = requestAnimationFrame(loop)
    }
    const stop = (): void => {
      running = false
      cancelAnimationFrame(handle)
    }
    frame((performance.now() / 1000) * speed)
    let onScreen = true
    const watch =
      typeof IntersectionObserver === 'undefined'
        ? undefined
        : new IntersectionObserver(([entry]) => {
            onScreen = entry?.isIntersecting ?? true
            if (onScreen && document.visibilityState !== 'hidden') start()
            else stop()
          })
    watch?.observe(canvas)
    const onVisibility = (): void => {
      if (document.visibilityState === 'hidden') stop()
      else if (onScreen) start()
    }
    document.addEventListener('visibilitychange', onVisibility)
    if (watch === undefined) start()
    return () => {
      stop()
      watch?.disconnect()
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [state, box, reduced])

  return (
    <canvas
      ref={ref}
      role="img"
      style={{ width: box, height: box, display: 'block' }}
      {...(label === undefined ? { 'aria-hidden': true } : { 'aria-label': label })}
    />
  )
}

/**
 * The size the live line and the plan step give an orb.
 *
 * 26 rather than 20: the 64 asset needs the room, and at 20 its points are
 * too fine to separate. The row's box grows with it rather than the drawing
 * spilling out of a 20px box, so nothing overlaps the word beside it.
 */
export const ORB_BOX = 26
