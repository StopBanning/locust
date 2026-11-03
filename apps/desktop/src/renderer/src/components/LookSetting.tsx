import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { AppearanceState, GlassGround, LookChoice } from '../../../shared/appearance.js'

/**
 * Standard or Liquid glass (0.855, main/mac-glass.ts), beside the theme. Shown only in the window of a Mac that has
 * macOS's glass: anywhere else, a browser showing Locust included, there is nothing to choose. The window changes in
 * place.
 */
const CHOICES: readonly (readonly [LookChoice, string])[] = [
  ['standard', 'Standard'],
  ['liquid', 'Liquid glass']
]

const GROUNDS: readonly (readonly [GlassGround, string])[] = [
  ['desktop', 'Your desktop'],
  ['rainbow', 'Rainbow']
]

/** Behind the glass (0.859): what the glass shows through it. */
export function groundLede(ground: GlassGround): string {
  return ground === 'rainbow'
    ? 'Locust’s own rainbow, under the glass inside the window, so it glows the same over any desktop.'
    : 'Whatever is behind the window: brightest over a colourful desktop.'
}

export function lookLede(look: LookChoice): string {
  return look === 'liquid'
    ? 'The window on macOS’s glass, the sidebar on a floating panel. It shows your desktop through it, so it is brightest over a colourful one.'
    : 'Locust as it is drawn everywhere, with the sidebar on the Mac’s glass.'
}

export function LookSetting(): ReactElement | null {
  const [state, setState] = useState<AppearanceState>()
  useEffect(() => {
    let live = true
    void window.desktop?.readAppearance().then((next) => {
      if (live) setState(next)
    })
    const stop = window.desktop?.onAppearance((next) => setState(next))
    return () => {
      live = false
      stop?.()
    }
  }, [])
  // The Mac window's own look: a browser showing Locust draws none of it (main.tsx marks only the Mac window).
  if (state?.liquidOffered !== true || window.desktop?.remote !== undefined) return null
  const look = state.look
  return (
    <section className="lc-settings__section lc-settings__section--line">
      <div className="lc-settingline">
        <div className="lc-settingline__text">
          <h2 className="lc-settings__heading">Look</h2>
          <p className="lc-settings__lede">{lookLede(look)}</p>
        </div>
        <div className="lc-segmented" role="radiogroup" aria-label="How the window is drawn">
          {CHOICES.map(([option, label]) => (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={look === option}
              className={`lc-button${look === option ? ' is-active' : ''}`}
              onClick={() => {
                void window.desktop?.setLook(option).then(setState)
              }}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      {look === 'liquid' && (
        <div className="lc-settingline lc-settingline--sub">
          <div className="lc-settingline__text">
            <h3 className="lc-settings__heading">Behind the glass</h3>
            <p className="lc-settings__lede">{groundLede(state.glassGround)}</p>
          </div>
          <div className="lc-segmented" role="radiogroup" aria-label="What the glass shows through it">
            {GROUNDS.map(([option, label]) => (
              <button
                key={option}
                type="button"
                role="radio"
                aria-checked={state.glassGround === option}
                className={`lc-button${state.glassGround === option ? ' is-active' : ''}`}
                onClick={() => {
                  void window.desktop?.setGlassGround(option).then(setState)
                }}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
      )}
    </section>
  )
}
