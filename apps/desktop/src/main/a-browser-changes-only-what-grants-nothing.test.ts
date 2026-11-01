import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

import { deskAccess } from '../shared/desk-calls.js'
import { SETTINGS_A_BROWSER_MAY_CHANGE, WORKSPACE_SETTINGS_CHANGE_CHANNEL, WORKSPACE_SETTINGS_WRITE_CHANNEL } from '../shared/ipc.js'
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

  it('sets none of Auto, reach, swarm, relay or the check command', () => {
    expect(settingsChangeKept({ autoMode: true, askConnectors: true, swarm: true, relay: true, relayHopCap: 12, checkCommand: 'rm -rf /', planRunsOut: { runtime: 'claude', automatic: true }, keepATodoList: true }))
      .toEqual({ keepATodoList: true })
    // The model a new chat starts on is kept (0.850): a browser already starts a run on any model.
    expect(settingsChangeKept({ newChatRoute: { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' } })).toEqual({ newChatRoute: { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free' } })
    expect(settingsChangeKept(null)).toEqual({})
    expect(settingsChangeKept(['keepATodoList'])).toEqual({})
  })

  it("is not undone by the window: each of the window's writes sends only the key it changes", () => {
    // The PC chat's review of 0.849: every window write carried its own copy of the five keys a browser now changes,
    // so Plans turned off in Safari came back on the next time the window picked a new chat's model.
    const app = readFileSync(fileURLToPath(new URL('../renderer/src/App.tsx', import.meta.url)), 'utf8')
    const writes = [...app.matchAll(/writeWorkspaceSettings\(\x7b([^\x7d]*)\x7d\)/g)].map((match) => match[1] ?? '')
    expect(writes.length).toBeGreaterThan(10)
    for (const body of writes) {
      const keys = body.split(',').map((entry) => entry.trim().split(':')[0]?.trim() ?? '').filter((key) => key.length > 0)
      expect(keys, body).toHaveLength(1)
      expect(SETTINGS_A_BROWSER_MAY_CHANGE as readonly string[], body).not.toContain(keys[0])
    }
  })

  it('is shown by the window the next time it is looked at', () => {
    const app = readFileSync(fileURLToPath(new URL('../renderer/src/App.tsx', import.meta.url)), 'utf8')
    expect(app).toContain("window.addEventListener('focus', rereadSettings)")
    expect(app).toMatch(/const rereadSettings = \(\): void => \{\n\s+void window\.desktop\?\.readWorkspaceSettings\(\)\.then\(\(settings\) => \{\n\s+setKeepATodoList\(settings\.keepATodoList === true\)/)
  })

  it("is laid over what is stored inside the store's own lock, not over a copy read before it", () => {
    const main = readFileSync(fileURLToPath(new URL('./index.ts', import.meta.url)), 'utf8')
    expect(main).toContain('return forThisFolder(await teammates.writeSettings(settingsChangeKept(change)))')
  })
})
