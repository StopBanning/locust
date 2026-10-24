import { describe, expect, it } from 'vitest'

import { APPEARANCE_SET_CHANNEL } from './appearance.js'
import { deskAccess, WINDOW_ONLY } from './desk-calls.js'
import {
  GROUP_CREATE_CHANNEL, GROUP_INSTRUCTIONS_CHANNEL, GROUP_REMOVE_CHANNEL, GROUP_RENAME_CHANNEL, GROUP_ROUTE_CHANNEL,
  MEMORY_ADD_CHANNEL, MEMORY_CLEAR_CHANNEL, MEMORY_REMOVE_CHANNEL, MEMORY_RESTORE_CHANNEL, MEMORY_UPDATE_CHANNEL,
  WORKSPACE_SETTINGS_WRITE_CHANNEL
} from './ipc.js'
import { SCOUT_NOTES, SCOUT_SET, SCOUT_TUNE } from './scout.js'

/**
 * SETTINGS FROM A BROWSER (0.835, desk-calls.ts). Locust in a browser changes the look, groups, what teammates
 * remember and Scout as the window does -- but not what grants or runs on its own: forgetting everything at once,
 * Scout preparing fixes, and the folder's settings, which hold Auto and what teammates may reach.
 */
describe('settings from a browser', () => {
  it('changes the look, groups, memories, and Scout’s notes and tune-up', () => {
    for (const channel of [APPEARANCE_SET_CHANNEL, GROUP_CREATE_CHANNEL, GROUP_RENAME_CHANNEL, GROUP_REMOVE_CHANNEL, GROUP_INSTRUCTIONS_CHANNEL,
      GROUP_ROUTE_CHANNEL, MEMORY_ADD_CHANNEL, MEMORY_UPDATE_CHANNEL, MEMORY_REMOVE_CHANNEL, MEMORY_RESTORE_CHANNEL, SCOUT_NOTES, SCOUT_TUNE]) {
      expect(deskAccess(channel, [{}]), channel).toBe('act')
      expect(WINDOW_ONLY, channel).not.toContain(channel)
    }
  })

  it('changes Scout’s settings, but never turns on its preparing fixes', () => {
    expect(deskAccess(SCOUT_SET, [{ everyMinutes: 30, quiet: null }])).toBe('act')
    expect(deskAccess(SCOUT_SET, [{ fixes: true }])).toBeUndefined()
    expect(deskAccess(SCOUT_SET, [{ fixes: false, on: true }])).toBeUndefined()
    expect(deskAccess(SCOUT_SET, [])).toBeUndefined()
  })

  it('leaves forgetting everything, and the folder’s own settings, on the computer', () => {
    for (const channel of [MEMORY_CLEAR_CHANNEL, WORKSPACE_SETTINGS_WRITE_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBeUndefined()
      expect(WINDOW_ONLY, channel).toContain(channel)
    }
  })
})
