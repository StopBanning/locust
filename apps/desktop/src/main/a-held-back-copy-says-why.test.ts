import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { createUpdateService, type UpdaterLike } from './updates.js'

/**
 * A HELD-BACK COPY SAYS WHY (the first Mac run, 2026-10-30). The window guessed from its user agent and told every
 * Mac it could not update itself. The service now says why this copy does not -- a development build, a launch a
 * script drives, or a copy that is not the installed Locust -- and only with the `unsupported` phase.
 */
const updater: UpdaterLike = {
  autoDownload: false,
  allowPrerelease: false,
  autoInstallOnAppQuit: false,
  checkForUpdates: async () => null,
  downloadUpdate: async () => undefined,
  quitAndInstall: () => undefined,
  on() { return updater }
}
const service = (supported: boolean) =>
  createUpdateService({ updater, currentVersion: '0.843.0', supported, heldBack: 'driven', liveMissionCount: () => 0, requestQuit: () => undefined })

describe('a copy held back from updating', () => {
  it('says why, from its first state and at every check', async () => {
    const held = service(false)
    expect(held.state()).toMatchObject({ phase: 'unsupported', heldBack: 'driven' })
    const checked = await held.check()
    expect(checked.ok && checked.data).toMatchObject({ phase: 'unsupported', heldBack: 'driven' })
  })

  it('says nothing of it when it does update itself (control)', () => {
    expect(service(true).state()).not.toHaveProperty('heldBack')
  })

  it('is told by main which of the three it is', () => {
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    expect(main).toContain("heldBack: !app.isPackaged ? 'development' : !updatesItself ? 'driven' : 'copy',")
  })
})
