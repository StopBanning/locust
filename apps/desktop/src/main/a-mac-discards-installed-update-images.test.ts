import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'

import { createMacUpdater } from './mac-self-update.js'

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})
const setup = () => {
  const root = mkdtempSync(join(tmpdir(), 'locust-image-cleanup-'))
  roots.push(root)
  const cacheFolder = join(root, 'cache')
  mkdirSync(cacheFolder)
  return { root, cacheFolder }
}

it('removes installed images at startup and preserves newer images and everything outside its exact file names', async () => {
  const { root, cacheFolder } = setup()
  const removed = ['Locust-0.842.0-mac-arm64.dmg', 'Locust-0.843.0-mac-arm64.dmg', 'Locust-0.9.0-mac-x64.dmg']
  const kept = ['Locust-0.844.0-mac-arm64.dmg', 'Locust-1.0.0-mac-x64.dmg', 'Other-0.842.0-mac-arm64.dmg', 'Locust-0.842.0-mac-arm64.dmg.part', 'Locust-0.842.0-beta-mac-arm64.dmg', 'Locust-0.842.0-mac-unknown.dmg']
  for (const name of [...removed, ...kept]) writeFileSync(join(cacheFolder, name), 'image')
  const directory = 'Locust-0.840.0-mac-arm64.dmg'
  mkdirSync(join(cacheFolder, directory))
  const outside = join(root, removed[0]!)
  writeFileSync(outside, 'outside')
  const link = 'Locust-0.841.0-mac-arm64.dmg'
  let linked = false
  try {
    symlinkSync(outside, join(cacheFolder, link))
    linked = true
  } catch {
    // Windows without developer-mode symlink privilege refuses a file symlink; the link's row is then left out.
  }
  const run = vi.fn(async () => ({ code: 0, output: '' }))
  const updater = createMacUpdater({ currentVersion: '0.843.0', arch: 'arm64', bundle: join(root, 'Locust.app'), cacheFolder, quit: vi.fn(), run, fetchReleases: async () => [] })
  expect(readdirSync(cacheFolder).sort()).toEqual([...kept, directory, ...(linked ? [link] : [])].sort())
  expect(existsSync(outside)).toBe(true)
  // Cleanup is finished before the updater is exposed. Checks and install
  // requests must never run it again, even if another old image appears.
  writeFileSync(join(cacheFolder, removed[0]!), 'later')
  await updater.checkForUpdates()
  updater.quitAndInstall()
  expect(existsSync(join(cacheFolder, removed[0]!))).toBe(true)
  expect(run).not.toHaveBeenCalled()
})

it('leaves ambiguous running versions alone and tolerates a cache that cannot be listed', () => {
  const { root, cacheFolder } = setup()
  const image = join(cacheFolder, 'Locust-0.843.0-mac-arm64.dmg')
  writeFileSync(image, 'image')
  const options = { arch: 'arm64', bundle: join(root, 'Locust.app'), cacheFolder, quit: vi.fn() }
  for (const currentVersion of ['0.843.0-beta', 'invalid']) {
    createMacUpdater({ ...options, currentVersion })
    expect(existsSync(image)).toBe(true)
  }
  expect(() => createMacUpdater({ ...options, currentVersion: '0.843.0', cacheFolder: image })).not.toThrow()
  expect(() => createMacUpdater({ ...options, currentVersion: '0.843.0', cacheFolder: join(root, 'missing') })).not.toThrow()
})
