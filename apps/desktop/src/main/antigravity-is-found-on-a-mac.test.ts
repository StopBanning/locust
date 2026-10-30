import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { createAntigravityHostProbe } from './antigravity-host.js'

/**
 * ANTIGRAVITY IS FOUND ON A MAC (the first Mac run, 2026-10-30).
 *
 * agy 1.4.11, installed by Antigravity's own install.sh into ~/.local/bin and signed in, read as "not installed" on
 * Home and in Settings: the probe looked only for %LOCALAPPDATA%\agy\bin\agy.exe, and only on Windows. install.sh
 * puts agy in ~/.local/bin, or wherever its `--dir` said, so a Mac or Linux looks there and then on PATH.
 */

const folders: string[] = []
afterEach(() => {
  for (const folder of folders.splice(0)) rmSync(folder, { recursive: true, force: true })
})

/** A folder with an `agy` at `where` inside it. */
function withFile(where: readonly string[]): string {
  const root = mkdtempSync(join(tmpdir(), 'locust-agy-mac-'))
  folders.push(root)
  mkdirSync(join(root, ...where.slice(0, -1)), { recursive: true })
  writeFileSync(join(root, ...where), '')
  return root
}

// localAppData is given so the LOCUST_HIDE_RUNTIMES seam stays the test's own, as in the other probe tests.
const probeOn = (platform: NodeJS.Platform, home: string, pathEnv = '') =>
  createAntigravityHostProbe({ platform, home, localAppData: home, pathEnv, run: async () => ({ stdout: '', stderr: '', code: 0 }) })

describe('Antigravity CLI on a Mac or Linux', () => {
  it.skipIf(process.platform === 'win32')('is found where install.sh puts it, ~/.local/bin/agy', () => {
    const home = withFile(['.local', 'bin', 'agy'])
    for (const platform of ['darwin', 'linux'] as const) {
      const probe = probeOn(platform, home)
      expect(probe.cliPath()).toBe(join(home, '.local', 'bin', 'agy'))
      expect(probe.pendingRecord().availability).toBe('available')
    }
  })

  it.skipIf(process.platform === 'win32')('is found on PATH when --dir put it elsewhere', () => {
    const home = withFile(['unused'])
    const tools = withFile(['bin', 'agy'])
    expect(probeOn('darwin', home, `/nowhere:${join(tools, 'bin')}`).cliPath()).toBe(join(tools, 'bin', 'agy'))
  })

  it('is not found where there is none, and a relative PATH entry is not searched (control)', () => {
    const home = withFile(['unused'])
    const probe = probeOn('darwin', home, 'bin:.')
    expect(probe.cliPath()).toBeUndefined()
    expect(probe.pendingRecord().availability).not.toBe('available')
  })

  it('on Windows, still looks only where install.ps1 puts it, not in ~/.local/bin (control)', () => {
    const home = withFile(['.local', 'bin', 'agy'])
    expect(probeOn('win32', home).cliPath()).toBeUndefined()
  })
})
