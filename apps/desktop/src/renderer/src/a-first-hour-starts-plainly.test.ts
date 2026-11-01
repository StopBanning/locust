import { describe, expect, it } from 'vitest'

import { asReviewer } from '../../shared/route-at-start.js'
import APP from './App.tsx?raw'
import { connectorLabel } from './components/NewTeammateDialog.js'
import DIALOG from './components/NewTeammateDialog.tsx?raw'
import HOME from './components/FirstLaunch.tsx?raw'
import SCREENS from './components/Screens.tsx?raw'

/*
 * A first-hour pass on 0.512 (a fresh profile, an empty folder): the work went
 * well and getting started did not. These are its five (0.514).
 */
describe('a first hour that starts plainly', () => {
  it('Home\'s free sentence has a button that puts the chat box on the free model', () => {
    expect(HOME).toContain('Use a free model')
    expect(HOME).toContain("freeStart === 'yes' && onUseFree !== undefined")
    // Offered only when it changes something: not while the chat box is already on OpenCode.
    expect(APP).toContain("if (route.runtime === 'opencode') return {}")
  })

  it('a new teammate\'s model is chosen right under its name, before how it looks', () => {
    expect(DIALOG.indexOf('<div className="lc-teammatemodel">')).toBeGreaterThan(0)
    expect(DIALOG.indexOf('<div className="lc-teammatemodel">')).toBeLessThan(DIALOG.indexOf('CHOOSE A LOOK'))
    expect(DIALOG.indexOf('<div className="lc-teammatemodel">')).toBeLessThan(DIALOG.indexOf('<span className="lc-fieldlabel">Connectors</span>'))
  })

  it('a connector is named for a person, its id kept for hover', () => {
    expect(connectorLabel('plugin:small-business:zoho-projects')).toBe('Zoho Projects')
    expect(connectorLabel('plugin:small-business:airwallex-agentos')).toBe('Airwallex Agentos')
    expect(connectorLabel('claude.ai Gmail')).toBe('Gmail')
    expect(connectorLabel('robinhood')).toBe('robinhood')
  })

  it('a copy says it leaves the machine\'s agents alone, instead of a switch that would not', () => {
    expect(SCREENS).toContain('This copy is not the installed Locust, so it never updates the agents on this machine on its own.')
    expect(SCREENS).toContain('runtimeUpdates.heldHere !== true && onKeepAgentsCurrent !== undefined')
  })

  it('a review reads: Ask, for that run only', () => {
    const edit = { teammateId: 'tm_rev', route: { runtime: 'opencode', model: 'opencode/free' }, mode: 'accept-edits', effort: undefined } as const
    expect(asReviewer(edit)).toEqual({ ...edit, mode: 'ask', oneOff: true })
    // Already reading: unchanged. Antigravity's app cannot be held to Ask, so it is left alone.
    expect(asReviewer({ ...edit, mode: 'ask' })).toEqual({ ...edit, mode: 'ask' })
    expect(asReviewer({ ...edit, route: { runtime: 'antigravity', model: 'x' } })).toEqual({ ...edit, route: { runtime: 'antigravity', model: 'x' } })
    // Through its CLI it can (agy in Ask refused git status, 10/02).
    expect(asReviewer({ ...edit, route: { runtime: 'antigravity', model: 'x' } }, true)).toEqual({ ...edit, route: { runtime: 'antigravity', model: 'x' }, mode: 'ask', oneOff: true })
    expect(APP).toContain('{ as: asReviewer(startAs(reviewer, { route, mode, effort }, pickerRoutes), antigravityCli) }')
    expect(APP).toContain("...(as?.oneOff === true ? { keepSavedRoute: true } : {}),")
  })
})
