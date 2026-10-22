import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { AppUpdatePhase } from '../shared/ipc.js'
import { UPDATE_CHECK_EVERY_MS, UPDATE_RETRY_AFTER_RUN_MS, createUpdateService, periodicCheckDue } from './updates.js'
import type { UpdaterLike } from './updates.js'

/**
 * AN OPEN LOCUST KEEPS LOOKING FOR UPDATES (C2, 0.367).
 *
 * It looked once, eight seconds after launch (known issue 5), so a person
 * who keeps the app open for days never heard of a release until they
 * restarted it. Then it looked every six hours -- and stopped for good once
 * one version was downloaded: a copy open since 12:53 AM still offered 0.817
 * at 9:38 AM with 0.818 and 0.819 out (2026-10-22). Now it looks every hour,
 * keeps looking while a downloaded version waits, and takes the newer one --
 * never while a teammate is working, and without ever taking the Restart and
 * install line away just to look.
 */
function updater(checks: { count: number }): UpdaterLike {
  return {
    autoDownload: false,
    allowPrerelease: false,
    autoInstallOnAppQuit: true,
    checkForUpdates: async () => {
      checks.count += 1
      return null
    },
    downloadUpdate: async () => undefined,
    quitAndInstall: () => undefined,
    on() {
      return this
    }
  }
}

/** How long the fake feed's download takes: a check answers before its download is down, as a real one does. */
const DOWNLOAD_MS = 1_000

/** A feed that offers `latest.version`, and downloads it the way electron-updater says so: 'update-downloaded'. */
function feed(latest: { version: string }): UpdaterLike {
  const listeners = new Map<string, ((payload?: unknown) => void)[]>()
  return {
    autoDownload: false,
    allowPrerelease: false,
    autoInstallOnAppQuit: true,
    checkForUpdates: async () => {
      const version = latest.version
      const downloadPromise = new Promise<void>((done) => setTimeout(done, DOWNLOAD_MS)).then(() => {
        for (const listener of listeners.get('update-downloaded') ?? []) listener({ version })
      })
      return { updateInfo: { version }, isUpdateAvailable: true, downloadPromise }
    },
    downloadUpdate: async () => undefined,
    quitAndInstall: () => undefined,
    on(event, listener) {
      listeners.set(event, [...(listeners.get(event) ?? []), listener])
      return this
    }
  }
}

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

describe('a Locust left open', () => {
  it('looks again every hour', async () => {
    expect(UPDATE_CHECK_EVERY_MS).toBe(60 * 60 * 1000)
    const checks = { count: 0 }
    const service = createUpdateService({ updater: updater(checks), currentVersion: '0.366.0', supported: true, liveMissionCount: () => 0, requestQuit: () => undefined })
    const stop = service.startPeriodicChecks()
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS - 1)
    expect(checks.count).toBe(0)
    await vi.advanceTimersByTimeAsync(1)
    expect(checks.count).toBe(1)
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS)
    expect(checks.count).toBe(2)
    stop()
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS * 3)
    expect(checks.count).toBe(2)
  })

  it('waits while a teammate is working, and tries again soon rather than an hour later', async () => {
    const checks = { count: 0 }
    let live = 1
    const service = createUpdateService({ updater: updater(checks), currentVersion: '0.366.0', supported: true, liveMissionCount: () => live, requestQuit: () => undefined })
    service.startPeriodicChecks()
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS)
    expect(checks.count).toBe(0)
    live = 0
    await vi.advanceTimersByTimeAsync(UPDATE_RETRY_AFTER_RUN_MS)
    expect(checks.count).toBe(1)
  })

  it('never looks in a build that cannot update itself', async () => {
    const checks = { count: 0 }
    const service = createUpdateService({ updater: updater(checks), currentVersion: '0.366.0', supported: false, liveMissionCount: () => 0, requestQuit: () => undefined })
    service.startPeriodicChecks()
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS * 2)
    expect(checks.count).toBe(0)
  })

  it('keeps looking while a downloaded version waits, and a newer one takes its place', async () => {
    const latest = { version: '0.817.0' }
    const said: { phase: AppUpdatePhase; version?: string }[] = []
    const service = createUpdateService({
      updater: feed(latest), currentVersion: '0.814.0', supported: true, liveMissionCount: () => 0, requestQuit: () => undefined,
      onStateChange: (state) => said.push({ phase: state.phase, ...(state.availableVersion === undefined ? {} : { version: state.availableVersion }) })
    })
    await service.check()
    await vi.advanceTimersByTimeAsync(DOWNLOAD_MS)
    expect(service.state()).toMatchObject({ phase: 'ready', availableVersion: '0.817.0' })

    service.startPeriodicChecks()
    said.length = 0
    // An hour on, nothing newer: the waiting version stays offered all through the look, which never said Checking.
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS)
    expect(service.state()).toMatchObject({ phase: 'ready', availableVersion: '0.817.0' })
    await vi.advanceTimersByTimeAsync(DOWNLOAD_MS)
    expect(service.state()).toMatchObject({ phase: 'ready', availableVersion: '0.817.0' })
    expect(said.every((entry) => entry.phase === 'ready' && entry.version === '0.817.0')).toBe(true)

    // Two releases later: the newer one is fetched, and offered in its place once it is down. While it comes
    // down, Restart and install is not offered for the one it replaces -- electron-updater clears that one.
    latest.version = '0.819.0'
    said.length = 0
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_EVERY_MS - DOWNLOAD_MS)
    expect(service.state()).toMatchObject({ phase: 'available', availableVersion: '0.819.0' })
    await vi.advanceTimersByTimeAsync(DOWNLOAD_MS)
    expect(service.state()).toMatchObject({ phase: 'ready', availableVersion: '0.819.0' })
    expect(said).toEqual([{ phase: 'available', version: '0.819.0' }, { phase: 'ready', version: '0.819.0' }])
  })

  it('still says Checking when the person presses Check now', async () => {
    const said: AppUpdatePhase[] = []
    const service = createUpdateService({
      updater: feed({ version: '0.817.0' }), currentVersion: '0.814.0', supported: true, liveMissionCount: () => 0, requestQuit: () => undefined,
      onStateChange: (state) => said.push(state.phase)
    })
    await service.check()
    expect(said[0]).toBe('checking')
  })
})

describe('whether a periodic look is due', () => {
  it('is, when nothing is found yet, the last look could not complete, or a downloaded version waits', () => {
    for (const phase of ['idle', 'current', 'failed', 'ready'] as const) expect(periodicCheckDue({ phase, currentVersion: '0.366.0' }, 0)).toBe(true)
  })

  it('is not while a check runs or a version is found or coming down -- or while a teammate works', () => {
    for (const phase of ['checking', 'available', 'downloading', 'unsupported'] as const) {
      expect(periodicCheckDue({ phase, currentVersion: '0.366.0' }, 0)).toBe(false)
    }
    expect(periodicCheckDue({ phase: 'current', currentVersion: '0.366.0' }, 2)).toBe(false)
    expect(periodicCheckDue({ phase: 'ready', currentVersion: '0.366.0' }, 1)).toBe(false)
  })
})
