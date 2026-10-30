import { describe, expect, it } from 'vitest'

import { bundledNpmBinDirectory, bundledNpmPaths, bundledNpmPrefix, findBundledNpm } from './bundled-npm.js'

// The app's own folders are on the machine running the test, joined by its rules: as written on Windows, with / elsewhere.
const here = (windowsPath: string): string =>
  process.platform === 'win32' ? windowsPath : windowsPath.replace(/^C:/, '').replace(/\\/g, '/')

/**
 * Where it installs to, and where the locator then looks.
 *
 * Grok's pass 10, the day after this shipped: Install ran, npm said done,
 * Locust said "installed, but Locust still cannot find the command". npm
 * had chosen a prefix from the binary's location and nobody searched it.
 * The two halves below are the same folder, said twice, so they cannot
 * drift: what npm is told and what the locator is told come from one call.
 */
describe('where the bundled npm installs', () => {
  it('is a folder in the profile that Locust owns, not npm\'s guess from the binary', () => {
    expect(bundledNpmPrefix(here('C:\\Users\\ian\\AppData\\Roaming\\Locust'))).toBe(
      here('C:\\Users\\ian\\AppData\\Roaming\\Locust\\npm')
    )
  })

  it('has its shims at the prefix itself on Windows, which is where npm puts them', () => {
    expect(bundledNpmBinDirectory('C:\\Users\\ian\\AppData\\Roaming\\Locust\\npm', 'win32')).toBe(
      'C:\\Users\\ian\\AppData\\Roaming\\Locust\\npm'
    )
  })

  it('and under bin everywhere else', () => {
    expect(bundledNpmBinDirectory('/home/ian/.config/Locust/npm', 'linux')).toMatch(/npm[\\/]bin$/)
  })
})

/**
 * The app carries an npm, so a machine with no Node can still install a CLI.
 *
 * Ian's machine is the case: Locust's whole first screen is "install one of
 * these", and without Node every button on it was disabled.
 */

const PACKAGED = here('C:\\Program Files\\Locust\\resources')
const APP = here('C:\\Program Files\\Locust\\resources\\app.asar')
const EXEC = here('C:\\Program Files\\Locust\\Locust.exe')

describe('the npm this app carries', () => {
  it('is looked for beside the asar, the one route that carries the whole tree', () => {
    // Measured 2026-09-18 on a real build: through the asar all 111 bundled
    // packages and all 8 nested node_modules arrive, and that npm installed
    // a package with no Node on PATH. The same staged directory copied as a
    // plain resource arrived with 0 packages -- the packager filters
    // `node_modules` out of a directory copy -- and died on its first run
    // with "Cannot find module 'graceful-fs'".
    const [first] = bundledNpmPaths(PACKAGED, APP)
    expect(first).toBe(
      here('C:\\Program Files\\Locust\\resources\\app.asar.unpacked\\resources\\npm\\bin\\npm-cli.js')
    )
  })

  it('is found, and comes with the variable that makes the app behave as Node', () => {
    const found = findBundledNpm({
      resourcesPath: PACKAGED,
      appPath: APP,
      execPath: EXEC,
      exists: (path) => path.includes('resources')
    })
    expect(found?.nodePath).toBe(EXEC)
    expect(found?.cliPath).toContain('npm-cli.js')
    // Without it the same path opens a second copy of the app instead.
    expect(found?.env).toEqual({ ELECTRON_RUN_AS_NODE: '1' })
  })

  it('falls back to the workspace copy in development', () => {
    const found = findBundledNpm({
      resourcesPath: here('C:\\nope'),
      appPath: here('C:\\repo\\apps\\desktop'),
      execPath: EXEC,
      exists: (path) => path === here('C:\\repo\\apps\\desktop\\node_modules\\npm\\bin\\npm-cli.js')
    })
    expect(found?.cliPath).toBe(here('C:\\repo\\apps\\desktop\\node_modules\\npm\\bin\\npm-cli.js'))
  })

  it('is nothing at all when this build shipped none, rather than a path that is not there', () => {
    // A guessed path would spawn and fail with a message about a missing
    // file, which reads as the install breaking rather than as this build
    // not carrying npm.
    expect(findBundledNpm({ resourcesPath: PACKAGED, appPath: APP, execPath: EXEC, exists: () => false })).toBeUndefined()
  })
})
