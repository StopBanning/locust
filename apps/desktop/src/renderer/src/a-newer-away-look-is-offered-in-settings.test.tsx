import { createElement, type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import type { ScoutAwayStatus } from '../../shared/scout-away.js'
import { AwayLine } from './components/ScoutSetting.js'

/**
 * A NEWER AWAY LOOK IS OFFERED IN SETTINGS (0.838). A repository on the older away look is told what the newer one
 * does -- your /locust comments reach your phone as you write them -- and Update it opens the pull request; while
 * that waits to be merged, the line says so and opens it.
 */
const api = { status: vi.fn(async () => ({ state: 'off' }) as ScoutAwayStatus), set: vi.fn(), onChanged: vi.fn(() => () => undefined) }
const switchOf = (label: string, checked: boolean): ReactNode => createElement('button', { 'aria-label': label, 'aria-checked': String(checked) })
const draw = (initial: ScoutAwayStatus): string =>
  renderToStaticMarkup(createElement(AwayLine, { api, switchOf, lastLookWords: () => '5 minutes ago', initial }))
const on: ScoutAwayStatus = { state: 'on', repo: 'acme/app', every: 1, private: false, lastLook: '2026-10-27T14:00:00Z' }

describe('Look while my computer is off', () => {
  it('offers the newer away look, and says what it does', () => {
    const html = draw({ ...on, outdated: true })
    expect(html).toContain('A newer away look is ready: your own /locust comments reach your phone as you write them. It comes in by a pull request you merge.')
    expect(html).toContain('>Update it</button>')
  })

  it('says the update waits to be merged, and opens its pull request', () => {
    const html = draw({ ...on, update: 'https://github.com/acme/app/pull/13' })
    expect(html).toContain('Merge the update in acme/app. The away look goes on as it was until then.')
    expect(html).toContain('>Open the pull request</button>')
    expect(html).not.toContain('>Update it</button>')
  })

  it('offers nothing on the newest look', () => {
    const html = draw(on)
    expect(html).not.toContain('Update it')
    expect(html).not.toContain('Merge the update')
    expect(html).toContain('Last away look: 5 minutes ago.')
  })
})
