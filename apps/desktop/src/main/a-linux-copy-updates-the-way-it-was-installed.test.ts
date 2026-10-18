import { describe, expect, it } from 'vitest'

import { linuxInstallOf, linuxPackageUrl, linuxUpdateTestOf } from './linux-install.js'
import { createUpdateService } from './updates.js'
import type { UpdaterLike } from './updates.js'

/**
 * A LINUX COPY UPDATES THE WAY IT WAS INSTALLED (0.789). An AppImage replaces
 * its own file, as the Windows installer does its folder. A .deb or an .rpm
 * belongs to the package manager, whose update takes the person's password:
 * Locust says a new version is out and hands over the new package to open.
 * The unpacked folder a test runs is not installed, and changes nothing.
 */
describe('how a Linux copy was installed', () => {
  const none = (): string => {
    throw new Error('ENOENT')
  }

  it('is an AppImage when the AppImage runtime names its file', () => {
    expect(linuxInstallOf({ APPIMAGE: '/home/ian/Apps/Locust.AppImage' }, '/tmp/.mount_Locust/resources', none)).toBe('appimage')
  })

  it("is a .deb or an .rpm by the package-type file electron-builder leaves beside the app's archive", () => {
    expect(linuxInstallOf({}, '/opt/Locust/resources', (file) => (file.replace(/\\/g, '/') === '/opt/Locust/resources/package-type' ? 'deb\n' : none()))).toBe('deb')
    expect(linuxInstallOf({}, '/opt/Locust/resources', () => 'rpm')).toBe('rpm')
  })

  it('is not installed at all when neither says so, or the file names something else', () => {
    expect(linuxInstallOf({}, '/work/release/linux-unpacked/resources', none)).toBeUndefined()
    expect(linuxInstallOf({ APPIMAGE: '' }, '/r', () => 'pacman')).toBeUndefined()
  })

  it("hands a .deb copy the feed's .deb, and an .rpm copy its .rpm, as full addresses", () => {
    const files = [{ url: 'Locust-0.790.0-linux-x86_64.AppImage' }, { url: 'Locust-0.790.0-linux-amd64.deb' }, { url: 'Locust-0.790.0-linux-x86_64.rpm' }]
    expect(linuxPackageUrl('deb', files, 'https://dl.locust.lol/')).toBe('https://dl.locust.lol/Locust-0.790.0-linux-amd64.deb')
    expect(linuxPackageUrl('rpm', files, 'https://dl.locust.lol/')).toBe('https://dl.locust.lol/Locust-0.790.0-linux-x86_64.rpm')
    expect(linuxPackageUrl('deb', [{ url: 7 }], 'https://dl.locust.lol/')).toBeUndefined()
    expect(linuxPackageUrl('deb', undefined, 'https://dl.locust.lol/')).toBeUndefined()
  })

  it('leaves the Linux update test seam off by default, and activates it only on an AppImage copy', () => {
    expect(linuxUpdateTestOf('appimage', {})).toBeUndefined()
    expect(linuxUpdateTestOf('appimage', { LOCUST_LINUX_UPDATE_TEST_FEED: '', LOCUST_LINUX_UPDATE_TEST_VERSION: '' })).toBeUndefined()
    expect(linuxUpdateTestOf('appimage', { LOCUST_LINUX_UPDATE_TEST_VERSION: '9.9.9' })).toBeUndefined()
    const env = {
      LOCUST_LINUX_UPDATE_TEST_FEED: 'http://127.0.0.1:41999',
      LOCUST_LINUX_UPDATE_TEST_VERSION: '9.9.9',
      APPIMAGE_SILENT_INSTALL: 'true'
    }
    expect(linuxUpdateTestOf(undefined, env)).toBeUndefined()
    expect(linuxUpdateTestOf('deb', env)).toBeUndefined()
    expect(linuxUpdateTestOf('rpm', env)).toBeUndefined()
    expect(linuxUpdateTestOf('appimage', { LOCUST_LINUX_UPDATE_TEST_FEED: 'http://127.0.0.1:41999', LOCUST_LINUX_UPDATE_TEST_VERSION: '9.9.9' })).toEqual({
      feedUrl: 'http://127.0.0.1:41999'
    })
    expect(linuxUpdateTestOf('appimage', env)).toEqual({
      feedUrl: 'http://127.0.0.1:41999',
      version: '9.9.9'
    })
  })

  it('reads a test feed only from this machine, never from anywhere else', () => {
    for (const feed of ['https://dl.example.test/', 'http://192.168.1.20:8080/', 'http://user:pass@127.0.0.1:41999/', 'file:///tmp/feed', 'not a url']) {
      expect(linuxUpdateTestOf('appimage', { LOCUST_LINUX_UPDATE_TEST_FEED: feed }), feed).toBeUndefined()
    }
    expect(linuxUpdateTestOf('appimage', { LOCUST_LINUX_UPDATE_TEST_FEED: 'http://localhost:41999/' })).toEqual({ feedUrl: 'http://localhost:41999/' })
  })
})

describe('a copy the package manager owns', () => {
  const feed = (version: string): UpdaterLike => ({
    autoDownload: true,
    allowPrerelease: false,
    autoInstallOnAppQuit: true,
    checkForUpdates: async () => ({ updateInfo: { version, files: [{ url: `Locust-${version}-linux-amd64.deb` }] } }),
    downloadUpdate: async () => {
      throw new Error('a package copy never downloads its own update')
    },
    quitAndInstall: () => {
      throw new Error('a package copy never installs its own update')
    },
    on() {
      return this
    }
  })

  it('never downloads or installs, and says the new version is out with its package', async () => {
    const updater = feed('0.790.0')
    const service = createUpdateService({
      updater,
      currentVersion: '0.789.0',
      supported: true,
      liveMissionCount: () => 0,
      requestQuit: () => undefined,
      packageUrl: (files) => linuxPackageUrl('deb', files, 'https://dl.locust.lol/') ?? 'https://locust.lol/'
    })
    expect(updater.autoDownload).toBe(false)
    expect(updater.autoInstallOnAppQuit).toBe(false)
    const checked = await service.check()
    expect(checked).toEqual({ ok: true, data: { phase: 'available', currentVersion: '0.789.0', availableVersion: '0.790.0', downloadUrl: 'https://dl.locust.lol/Locust-0.790.0-linux-amd64.deb', everyBuild: false } })
    expect(service.install()).toMatchObject({ ok: false, error: { code: 'UPDATE_NOT_READY' } })
  })

  it('says nothing to download when it is up to date', async () => {
    const service = createUpdateService({ updater: feed('0.789.0'), currentVersion: '0.789.0', supported: true, liveMissionCount: () => 0, requestQuit: () => undefined, packageUrl: () => 'https://locust.lol/' })
    expect(await service.check()).toEqual({ ok: true, data: { phase: 'current', currentVersion: '0.789.0', everyBuild: false } })
  })
})
