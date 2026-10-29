import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { AGENT_ACCOUNTS_CHANNEL } from './agent-accounts.js'
import { DESK_TAKE_BACK_CHANNEL, DESK_WHERE_CHANNEL, PHONE_DESK_CHANNEL, deskDecode, deskEncode, deskJoiner, deskParts, DESK_PART_CHARS } from './desk.js'
import { DESK_EVENTS, deskAccess, WINDOW_ONLY, WINDOW_ONLY_EVENTS, WINDOW_ONLY_SAYS, WINDOW_ONLY_SENDS } from './desk-calls.js'
import { REMOTE_CONTROL_SET_CHANNEL } from './claude-remote-control.js'
import { CONNECTOR_ADD_CHANNEL } from './connector-add.js'
import {
  APPROVAL_RULE_FROM_CARD_CHANNEL, APP_UPDATE_INSTALL_CHANNEL, CODEX_MISSION_START_CHANNEL, MISSION_APPROVAL_CHANNEL, MISSION_APPROVAL_DECIDE_CHANNEL,
  MISSION_HISTORY_CHANNEL, OPEN_LINK_CHANNEL, OWN_MODEL_ADD_CHANNEL, PROFILE_RESTORE_CHANNEL, RUNTIME_INSTALL_CHANNEL, RUNTIME_SIGN_IN_CHANNEL,
  WORKSPACE_CHOOSE_CHANNEL
} from './ipc.js'
import { GITHUB_SIGN_IN_CHANNEL, GITHUB_SIGN_IN_CODE_CHANNEL } from './github-account.js'
import { LOCUST_MCP_SET } from './locust-mcp.js'
import { REMOTE_MCP_DECIDE, REMOTE_MCP_SET } from './remote-mcp.js'
import { PHONE_ANYWHERE_CHANGED_CHANNEL, PHONE_NEW_CODE_CHANNEL } from './phone.js'
import { TERMINAL_CREATE, TERMINAL_INPUT } from './terminal.js'

/**
 * THE BROWSER ASKS ONLY WHAT IT MAY (0.830, Locust in a browser). The window in a browser makes the window's own
 * calls, and desk-calls.ts says which a computer answers from one. Every call the window can make is named there --
 * one the window gains later fails this until someone decides -- and what grants permissions, signs agents in,
 * pairs more devices, spends, opens the terminal or a dialog on the computer, is never answered from a browser.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const api = read('./desktop-api.ts')
const calls = read('./desk-calls.ts')
const named = (pattern: RegExp): string[] => [...new Set([...api.matchAll(pattern)].map((match) => match[1]!))]

describe('the browser asks only what it may', () => {
  it('names every call, message and update the window makes, each decided one way or the other', () => {
    const undecided = [
      ...named(/ipcRenderer\.invoke\(([A-Z_][A-Z0-9_]*|'[^']+')/g),
      ...named(/ipcRenderer\.on\(([A-Z_][A-Z0-9_]*|'[^']+')/g),
      ...named(/ipcRenderer\.send\(([A-Z_][A-Z0-9_]*|'[^']+')/g)
    ].filter((name) => !calls.includes(name))
    expect(undecided).toEqual([])
    expect(named(/ipcRenderer\.invoke\(([A-Z_][A-Z0-9_]*|'[^']+')/g).length).toBeGreaterThan(300)
  })

  it('never answers what grants, signs in, pairs, spends, installs, or opens something on the computer', () => {
    for (const channel of [
      APPROVAL_RULE_FROM_CARD_CHANNEL, RUNTIME_SIGN_IN_CHANNEL, GITHUB_SIGN_IN_CHANNEL, PHONE_NEW_CODE_CHANNEL, PHONE_DESK_CHANNEL, OWN_MODEL_ADD_CHANNEL,
      CONNECTOR_ADD_CHANNEL, REMOTE_CONTROL_SET_CHANNEL, LOCUST_MCP_SET, APP_UPDATE_INSTALL_CHANNEL, PROFILE_RESTORE_CHANNEL, RUNTIME_INSTALL_CHANNEL,
      WORKSPACE_CHOOSE_CHANNEL, OPEN_LINK_CHANNEL, TERMINAL_CREATE, TERMINAL_INPUT, DESK_TAKE_BACK_CHANNEL, DESK_WHERE_CHANNEL,
      // An app from anywhere is let in only on the computer (0.841): a browser window can neither allow one nor turn it on.
      REMOTE_MCP_DECIDE, REMOTE_MCP_SET
    ]) {
      expect(deskAccess(channel, []), channel).toBeUndefined()
      expect(WINDOW_ONLY, channel).toContain(channel)
    }
    expect(WINDOW_ONLY_SAYS).toBe('This works in Locust on your computer.')
  })

  it('reads and acts as the window does for the rest: the history, a new turn', () => {
    expect(deskAccess(MISSION_HISTORY_CHANNEL, [])).toBe('read')
    expect(deskAccess(CODEX_MISSION_START_CHANNEL, [{}])).toBe('act')
  })

  it('answers Allow once and Deny from a browser, never Always; reads the accounts, never adds one', () => {
    expect(deskAccess(MISSION_APPROVAL_DECIDE_CHANNEL, [{ approvalId: 'a', decision: 'approve-once' }])).toBe('act')
    expect(deskAccess(MISSION_APPROVAL_DECIDE_CHANNEL, [{ approvalId: 'a', decision: 'deny', reason: 'no' }])).toBe('act')
    expect(deskAccess(MISSION_APPROVAL_DECIDE_CHANNEL, [{ approvalId: 'a', answers: {} }])).toBe('act')
    expect(deskAccess(MISSION_APPROVAL_DECIDE_CHANNEL, [{ approvalId: 'a', decision: 'approve-always' }])).toBeUndefined()
    expect(deskAccess(AGENT_ACCOUNTS_CHANNEL, [{ kind: 'list' }])).toBe('read')
    for (const action of [{ kind: 'add', runtime: 'claude', name: 'Work' }, { kind: 'login', id: 'account_x' }, { kind: 'remove', id: 'account_x' }]) expect(deskAccess(AGENT_ACCOUNTS_CHANNEL, [action])).toBeUndefined()
  })

  it('tells a browser what the window is told, but never a sign-in code or a pairing code', () => {
    expect(DESK_EVENTS.has(MISSION_APPROVAL_CHANNEL)).toBe(true)
    for (const quiet of [GITHUB_SIGN_IN_CODE_CHANNEL, PHONE_ANYWHERE_CHANGED_CHANNEL]) {
      expect(DESK_EVENTS.has(quiet)).toBe(false)
      expect(WINDOW_ONLY_EVENTS).toContain(quiet)
    }
    expect(WINDOW_ONLY_SENDS.length).toBeGreaterThan(0)
  })
})

describe('the browser’s line keeps what the window’s IPC keeps', () => {
  it('keeps bytes, an undefined answer and an undefined argument, and leaves an object’s undefined out', () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255])
    expect(deskDecode(deskEncode([bytes, undefined, 'x']))).toEqual([bytes, undefined, 'x'])
    expect(deskDecode(deskEncode(undefined))).toBeUndefined()
    expect(deskDecode(deskEncode(null))).toBeNull()
    expect(deskDecode(deskEncode({ a: undefined, b: 1 }))).toEqual({ b: 1 })
    expect(deskDecode(deskEncode({ image: { bytes } }))).toEqual({ image: { bytes } })
  })

  it('carries a frame too long for one box in parts, and puts it back together once, whole', () => {
    const text = 'x'.repeat(DESK_PART_CHARS * 2 + 17)
    const parts = deskParts('f1', text)
    expect(parts.map((part) => part.n)).toEqual([0, 1, 2])
    const join = deskJoiner()
    expect(join(parts[2]!)).toBeUndefined()
    expect(join(parts[0]!)).toBeUndefined()
    expect(join(parts[0]!)).toBeUndefined()
    expect(join(parts[1]!)).toBe(text)
    // Too much for one frame: let go.
    const small = deskJoiner({ mostChars: DESK_PART_CHARS })
    expect(small(parts[0]!)).toBeUndefined()
    expect(small(parts[1]!)).toBeUndefined()
    expect(small(parts[2]!)).toBeUndefined()
  })
})
