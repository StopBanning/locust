import { describe, expect, it } from 'vitest'

import { deskAccess, WINDOW_ONLY } from './desk-calls.js'
import {
  COMPARE_ADD_MODEL_CHANNEL, COMPARE_ASK_CHANNEL, COMPARE_DRAFT_POINTS_CHANNEL, COMPARE_JUDGE_CHANNEL, COMPARE_KEEP_CHANNEL,
  COMPARE_LIST_CHANNEL, COMPARE_RERUN_CHANNEL, COMPARE_RERUN_CHECKED_CHANNEL, COMPARE_RERUN_STOP_CHANNEL, COMPARE_RETRY_CHANNEL,
  COMPARE_START_CHANNEL, ROOM_CREATE_CHANNEL, ROOM_LIST_CHANNEL, ROOM_POST_CHANNEL, ROOM_REMOVE_CHANNEL, ROOM_RENAME_CHANNEL, ROOM_TASK_CHANNEL,
  ROUTINE_CREATE_CHANNEL, ROUTINE_EXPORT_CHANNEL, ROUTINE_FOLDER_CHANNEL, ROUTINE_IMPORT_CHANNEL, ROUTINE_IMPORT_PREVIEW_CHANNEL,
  ROUTINE_LIST_CHANNEL, ROUTINE_PAUSE_CHANNEL, ROUTINE_REMOVE_CHANNEL, ROUTINE_RUN_CHANNEL, ROUTINE_SETTLE_CHANNEL, ROUTINE_UPDATE_CHANNEL,
  TEAMMATE_CONNECTORS_CHANNEL, TEAMMATE_CREATE_CHANNEL, TEAMMATE_FOLDER_CHANNEL, TEAMMATE_REMOVE_CHANNEL, TEAMMATE_UPDATE_CHANNEL
} from './ipc.js'
import { ROUTINE_RECOVERY_CHANNEL } from './routine-recovery.js'
import { TEAMMATE_SKILL_USE_CHANNEL } from './tune.js'
import { ROUTINE_HOOK_READ, ROUTINE_HOOK_NEW } from './routine-hook.js'

/**
 * ROUTINES, TEAMMATES AND ROOMS FROM A BROWSER (0.834, desk-calls.ts). Locust in a browser works a routine, a
 * teammate and a room as the window does, all but what a teammate may spend; what would open something on the computer -- a folder picker, a file to import or export, the file
 * manager over a copy -- stays there, and says so.
 */
describe('routines and rooms from a browser', () => {
  it('reads a called link, and classifies making or replacing its password as an action', () => {
    expect(deskAccess(ROUTINE_HOOK_READ, ['rt_a'])).toBe('read')
    expect(deskAccess(ROUTINE_HOOK_NEW, ['rt_a'])).toBe('act')
    expect(deskAccess(ROUTINE_UPDATE_CHANNEL, [{ schedule: { kind: 'hook' } }])).toBe('act')
  })
  it('makes, changes, pauses, runs and removes a routine, and settles what it was asked to', () => {
    for (const channel of [ROUTINE_CREATE_CHANNEL, ROUTINE_UPDATE_CHANNEL, ROUTINE_PAUSE_CHANNEL, ROUTINE_RUN_CHANNEL, ROUTINE_REMOVE_CHANNEL, ROUTINE_RECOVERY_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBe('act')
      expect(WINDOW_ONLY, channel).not.toContain(channel)
    }
    expect(deskAccess(ROUTINE_LIST_CHANNEL, [])).toBe('read')
    expect(deskAccess(ROUTINE_SETTLE_CHANNEL, [{ routineId: 'r', decision: 'keep' }])).toBe('act')
    expect(deskAccess(ROUTINE_SETTLE_CHANNEL, [{ routineId: 'r', decision: 'discard' }])).toBe('act')
  })

  it('leaves on the computer what opens something there', () => {
    expect(deskAccess(ROUTINE_SETTLE_CHANNEL, [{ routineId: 'r', decision: 'open' }])).toBeUndefined()
    expect(deskAccess(ROUTINE_SETTLE_CHANNEL, [])).toBeUndefined()
    for (const channel of [ROUTINE_FOLDER_CHANNEL, ROUTINE_EXPORT_CHANNEL, ROUTINE_IMPORT_CHANNEL, ROUTINE_IMPORT_PREVIEW_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBeUndefined()
      expect(WINDOW_ONLY, channel).toContain(channel)
    }
  })

  it('makes, changes and removes a teammate, but never what it may spend, its folder, connectors or skills', () => {
    for (const channel of [TEAMMATE_CREATE_CHANNEL, TEAMMATE_REMOVE_CHANNEL]) expect(deskAccess(channel, [{}]), channel).toBe('act')
    expect(deskAccess(TEAMMATE_UPDATE_CHANNEL, [{ teammateId: 't', name: 'Ada', instructions: 'Be brief.' }])).toBe('act')
    expect(deskAccess(TEAMMATE_UPDATE_CHANNEL, [{ teammateId: 't', name: 'Ada', monthlyLimitUsd: 500 }])).toBeUndefined()
    expect(deskAccess(TEAMMATE_UPDATE_CHANNEL, [{ teammateId: 't', name: 'Ada', monthlyLimitUsd: null }])).toBeUndefined()
    expect(deskAccess(TEAMMATE_UPDATE_CHANNEL, [])).toBeUndefined()
    for (const channel of [TEAMMATE_FOLDER_CHANNEL, TEAMMATE_CONNECTORS_CHANNEL, TEAMMATE_SKILL_USE_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBeUndefined()
      expect(WINDOW_ONLY, channel).toContain(channel)
    }
  })

  it('starts, asks, judges, runs again and keeps a comparison (0.835)', () => {
    expect(deskAccess(COMPARE_LIST_CHANNEL, [])).toBe('read')
    for (const channel of [COMPARE_START_CHANNEL, COMPARE_ASK_CHANNEL, COMPARE_ADD_MODEL_CHANNEL, COMPARE_RETRY_CHANNEL, COMPARE_JUDGE_CHANNEL,
      COMPARE_DRAFT_POINTS_CHANNEL, COMPARE_KEEP_CHANNEL, COMPARE_RERUN_CHANNEL, COMPARE_RERUN_CHECKED_CHANNEL, COMPARE_RERUN_STOP_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBe('act')
      expect(WINDOW_ONLY, channel).not.toContain(channel)
    }
  })

  it('makes, renames, removes and posts to a room, and moves its board', () => {
    expect(deskAccess(ROOM_LIST_CHANNEL, [])).toBe('read')
    for (const channel of [ROOM_CREATE_CHANNEL, ROOM_RENAME_CHANNEL, ROOM_REMOVE_CHANNEL, ROOM_POST_CHANNEL, ROOM_TASK_CHANNEL]) {
      expect(deskAccess(channel, [{}]), channel).toBe('act')
      expect(WINDOW_ONLY, channel).not.toContain(channel)
    }
  })
})
