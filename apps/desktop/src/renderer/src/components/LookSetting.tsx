import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { AppearanceState, LookChoice } from '../../../shared/appearance.js'

/**
 * Standard or Liquid glass (0.855, main/mac-glass.ts), beside the theme. Shown only in the window of a Mac that has
 * macOS's glass: anywhere else, a browser showing Locust included, there is nothing to choose. The window changes in
 * place.
 */
const CHOICES: readonly (readonly [LookChoice, string])[] = [
  ['standard', 'Standard'],
  ['liquid', 'Liquid glass']
]

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
    </section>
  )
}
