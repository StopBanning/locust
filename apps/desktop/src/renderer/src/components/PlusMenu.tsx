import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'
import { Liquid } from 'liquid-gooey'

import { PlusGlyph } from './ChatGlyphs.js'
import { useDrawnTheme } from '../drawnTheme.js'

/**
 * THE GOOEY +, libraries.dev's plus menu.
 *
 * Colin, 2026-09-23, on the metal chatbox: it "allows us to use their gooey +
 * button as well". This is the page's Morph demo -- "the live PlusMenu,
 * verbatim" (sites/home/src/studio/gooey.tsx in Jakubantalik/Libraries.dev):
 * the + splits into satellites like droplets on a liquid surface, the + turns
 * to a cross, the icons hold back while a satellite is still merged and cross-
 * blur in as it pulls clear, and closing dips the whole surface toward the
 * returning satellites before it settles. Same goo (blur 6, contrast 18), the
 * same easings and timings, the same surface and shadow (tokens.css, "the
 * gooey +").
 *
 * The satellites are what the + does here: attach files, and choose the
 * folder -- which left the row with the metal composer, since the folder's
 * name is in the title bar. They rise up and to the right, two of the page's
 * three positions, so none leaves the box on its left.
 *
 * `liquid-gooey` 0.2.1: the goo runs on a silhouette layer, the buttons stay
 * real DOM (focus, labels, handlers), and it sleeps when nothing moves.
 *
 * Each satellite SAYS what it does (design pass, 2026-10-19): two bare icons
 * were a guess, and the words lived only in a hover. The words sit beside the
 * droplets, outside the liquid so the goo never melts them, and choose what
 * their droplet chooses.
 */

const EASE_BOUNCY = 'cubic-bezier(0.34, 1.56, 0.64, 1)'
const EASE_SNAPPY = 'cubic-bezier(0.22, 1, 0.36, 1)'
/** The page's defaults (MORPH_DEFAULTS). */
const OPEN = { duration: 550, stagger: 40 }
const CLOSE = { duration: 250, stagger: 0 }
/** The dip's own 5px over 700ms, and the icons' 180ms fade, are the stylesheet's (.lc-plusmenu). */
const ANTICIPATION = { duration: 700 }
const ICON = { delay: 120 }
const GOO = { blur: 6, contrast: 18, waviness: 0 }

interface Satellite {
  readonly key: 'attach' | 'folder'
  readonly label: string
  readonly x: number
  readonly y: number
  readonly icon: ReactElement
}

/** The page's "New file" and "New folder" icons, at its top and right positions. */
const SATELLITES: readonly Satellite[] = [
  {
    key: 'attach',
    label: 'Attach files',
    x: 0,
    y: -64,
    icon: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M9 1.5H4A1.5 1.5 0 0 0 2.5 3v10A1.5 1.5 0 0 0 4 14.5h8a1.5 1.5 0 0 0 1.5-1.5V6z" />
        <path d="M9 1.5V6h4.5" />
      </svg>
    )
  },
  {
    key: 'folder',
    label: 'Choose a folder',
    x: 54,
    y: -34,
    icon: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M14.5 12.5A1.5 1.5 0 0 1 13 14H3a1.5 1.5 0 0 1-1.5-1.5V3A1.5 1.5 0 0 1 3 1.5h3L7.5 4H13a1.5 1.5 0 0 1 1.5 1.5z" />
      </svg>
    )
  }
]

/**
 * The liquid's surface and shadow, read from the tokens: the palette has one definition. Read once per theme
 * (0.789): kept from the first read, a dark fill stayed on after the window turned light.
 */
let surface: { readonly theme: string; readonly fill: string; readonly shadow: string } | undefined
function liquidSurface(theme: string): { readonly fill: string; readonly shadow: string } {
  if (surface?.theme === theme) return surface
  if (typeof document === 'undefined') return { fill: 'currentColor', shadow: 'none' }
  const style = getComputedStyle(document.documentElement)
  surface = {
    theme,
    fill: style.getPropertyValue('--lc-plusmenu-fill').trim() || 'currentColor',
    shadow: style.getPropertyValue('--lc-plusmenu-shadow').trim() || 'none'
  }
  return surface
}

function reducedMotion(): boolean {
  return typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true
}

export function PlusMenu({
  disabled,
  folderMissing,
  onAttach,
  onChooseFolder,
  folderStays
}: {
  readonly disabled: boolean
  /** No folder yet: the folder satellite then says so. */
  readonly folderMissing: boolean
  readonly onAttach: () => void
  readonly onChooseFolder: () => void
  /** In a browser (0.850): the computer's folder picker would open there, so the satellite is held and says this. */
  readonly folderStays?: string
}): ReactElement {
  // Re-read when the window changes theme (drawnTheme.ts).
  const theme = useDrawnTheme()
  const [open, setOpen] = useState(false)
  const [anticipating, setAnticipating] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const seat = useRef<HTMLSpanElement>(null)
  useEffect(() => () => { if (timer.current !== null) clearTimeout(timer.current) }, [])

  const close = (): void => {
    // The page's anticipation: the surface, and the + riding it, dips toward
    // the returning satellites and settles. Side effects stay out of the
    // state updater (StrictMode runs updaters twice).
    if (!reducedMotion()) {
      if (timer.current !== null) clearTimeout(timer.current)
      setAnticipating(false)
      requestAnimationFrame(() => setAnticipating(true))
      timer.current = setTimeout(() => setAnticipating(false), ANTICIPATION.duration)
    }
    setOpen(false)
  }

  // Escape, a press anywhere else, or the run starting: the menu closes.
  useEffect(() => {
    if (!open) return undefined
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') close()
    }
    const onDown = (event: PointerEvent): void => {
      if (seat.current !== null && event.target instanceof Node && !seat.current.contains(event.target)) close()
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('pointerdown', onDown, true)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('pointerdown', onDown, true)
    }
  }, [open])
  useEffect(() => {
    if (disabled && open) setOpen(false)
  }, [disabled, open])

  const phase = open
    ? { duration: OPEN.duration, ease: EASE_BOUNCY, stagger: OPEN.stagger }
    : { duration: CLOSE.duration, ease: EASE_SNAPPY, stagger: CLOSE.stagger }
  const { fill, shadow } = liquidSurface(theme)
  const choose = (key: Satellite['key']): void => {
    close()
    if (key === 'attach') onAttach()
    else onChooseFolder()
  }

  return (
    <span className="lc-plusmenu__seat" ref={seat}>
      <Liquid
        blur={GOO.blur}
        contrast={GOO.contrast}
        fill={fill}
        shadow={shadow}
        waviness={GOO.waviness}
        className={`lc-plusmenu${open ? ' is-open' : ''}${anticipating ? ' is-anticipating' : ''}`}
      >
        {SATELLITES.map((satellite, index) => (
          <Liquid.Item
            key={satellite.key}
            className="lc-plusmenu__slot"
            x={open ? satellite.x : 0}
            y={open ? satellite.y : 0}
            transition={{ duration: reducedMotion() ? 0 : phase.duration, ease: phase.ease }}
            delay={index * phase.stagger}
          >
            <button
              type="button"
              className="lc-plusmenu__button lc-plusmenu__satellite"
              data-satellite={satellite.key}
              aria-label={satellite.key === 'folder' && folderStays !== undefined ? 'Choose a folder, on your computer' : satellite.key === 'folder' && folderMissing ? 'Choose a folder — none is chosen yet' : satellite.label}
              title={satellite.key === 'folder' ? folderStays ?? satellite.label : satellite.label}
              disabled={satellite.key === 'folder' && folderStays !== undefined}
              tabIndex={open ? 0 : -1}
              aria-hidden={!open}
              onClick={() => choose(satellite.key)}
            >
              <span
                className="lc-plusmenu__icon"
                style={{ transitionDelay: open ? `${String(ICON.delay + index * phase.stagger)}ms` : '0ms' }}
              >
                {satellite.icon}
              </span>
            </button>
          </Liquid.Item>
        ))}
        <Liquid.Item className="lc-plusmenu__slot">
          <button
            type="button"
            className="lc-plusmenu__button lc-plusmenu__main"
            aria-haspopup="menu"
            aria-expanded={open}
            aria-label={open ? 'Close the menu' : 'Attach files or choose a folder'}
            title={open ? undefined : 'Attach files, or choose the folder teammates work in'}
            disabled={disabled}
            onClick={() => (open ? close() : setOpen(true))}
          >
            <span className="lc-plusmenu__plus">
              <PlusGlyph />
            </span>
          </button>
        </Liquid.Item>
      </Liquid>
      {SATELLITES.map((satellite, index) => (
        <span
          key={satellite.key}
          className={`lc-plusmenu__label${open ? ' is-open' : ''}`}
          data-satellite={satellite.key}
          aria-hidden="true"
          style={{ transitionDelay: open ? `${String(ICON.delay + index * phase.stagger)}ms` : '0ms' }}
          onClick={() => choose(satellite.key)}
        >
          {satellite.label}
        </span>
      ))}
    </span>
  )
}
