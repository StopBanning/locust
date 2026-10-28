import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { SettingsScreen } from './components/Screens.js'

/**
 * A PLUGGED-IN COMPUTER STAYS AWAKE, IN SETTINGS (0.838; main/a-plugged-in-computer-stays-awake-for-what-is-scheduled
 * .test.ts holds the rest). The switch sits under Startup and says what it leaves alone.
 */
describe('staying awake in Settings', () => {
  it('sits in Settings under Startup and says what it leaves alone', () => {
    const draw = (stayAwake: boolean): string => renderToStaticMarkup(createElement(SettingsScreen as never, { page: 'app', stayAwake } as never))
    const off = draw(false)
    expect(off).toContain('aria-checked="false" aria-label="Keep this computer awake while it is plugged in, so routines and Scout run on time."')
    expect(off).not.toContain('On battery it sleeps as usual.')
    const on = draw(true)
    expect(on).toContain('aria-checked="true" aria-label="Keep this computer awake while it is plugged in')
    expect(on).toContain('The screen still turns off, and Sleep and closing the lid work as they always do. On battery it sleeps as usual.')
  })
})
