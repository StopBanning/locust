import { describe, expect, it } from 'vitest'

import { deskAccess } from '../shared/desk-calls.js'
import { WORKSPACE_SETTINGS_CHANGE_CHANNEL, WORKSPACE_SETTINGS_WRITE_CHANNEL } from '../shared/ipc.js'
import { settingsChangeKept } from './workspace-settings-change.js'

/**
 * A BROWSER CHANGES ONLY WHAT GRANTS NOTHING (0.849). Colin, in a browser, could not turn Plans off: the whole settings
 * write stays on the computer because it carries Auto, reach, swarm and relay. The narrow change works from a browser
 * and the computer keeps only the harmless keys of it.
 */
describe('a settings change from a browser', () => {
  it('is a call a browser may make, where the whole write is not', () => {
    expect(deskAccess(WORKSPACE_SETTINGS_CHANGE_CHANNEL, [{ keepATodoList: false }])).toBe('act')
    expect(deskAccess(WORKSPACE_SETTINGS_WRITE_CHANNEL, [{}])).toBeUndefined()
  })

  it('keeps Plans, the look and what teammates remember', () => {
    expect(settingsChangeKept({ keepATodoList: false, replySize: 'large', layout: 'compact', plush: true, aboutYou: 'Short answers.' }))
      .toEqual({ keepATodoList: false, replySize: 'large', layout: 'compact', plush: true, aboutYou: 'Short answers.' })
  })

  it('sets none of Auto, reach, swarm, relay, the check command or the model new chats start on', () => {
    expect(settingsChangeKept({ autoMode: true, askConnectors: true, swarm: true, relay: true, relayHopCap: 12, checkCommand: 'rm -rf /', newChatRoute: { runtime: 'claude', model: 'opus' }, keepATodoList: true }))
      .toEqual({ keepATodoList: true })
    expect(settingsChangeKept(null)).toEqual({})
    expect(settingsChangeKept(['keepATodoList'])).toEqual({})
  })
})
