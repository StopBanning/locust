import { execFileSync } from 'node:child_process'
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

/**
 * A MAC ASKS ONLY FOR WHAT LOCUST USES (the first Mac run, 2026-10-30).
 *
 * The installed 0.841 carried Electron's own reasons, "This app needs access to the camera", "...to Bluetooth", "...to
 * the microphone", "...to audio capture": a prompt Locust could not explain. Locust uses the microphone (voice typing,
 * voice calls) and nothing else of these, so its reason is its own and the rest are taken out before signing.
 */
const DESKTOP = join(__dirname, '..', '..')
const { UNUSED, trimUnusedPermissions } = createRequire(import.meta.url)(join(DESKTOP, 'scripts', 'mac-permissions.cjs')) as {
  readonly UNUSED: readonly string[]
  readonly trimUnusedPermissions: (plist: string) => void
}

const folders: string[] = []
afterEach(() => {
  for (const folder of folders.splice(0)) rmSync(folder, { recursive: true, force: true })
})

/** Every .ts and .tsx file under a folder, tests left out. */
function sources(folder: string): string[] {
  return readdirSync(folder).flatMap((name) => {
    const path = join(folder, name)
    if (statSync(path).isDirectory()) return sources(path)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

describe('what a Mac may be asked for', () => {
  it.skipIf(process.platform !== 'darwin')('takes the camera, Bluetooth and system audio reasons out of Info.plist, and keeps the microphone', () => {
    const folder = mkdtempSync(join(tmpdir(), 'locust-plist-'))
    folders.push(folder)
    const plist = join(folder, 'Info.plist')
    execFileSync('plutil', ['-create', 'xml1', plist])
    for (const key of [...UNUSED, 'NSMicrophoneUsageDescription']) execFileSync('plutil', ['-insert', key, '-string', 'This app needs access', plist])
    trimUnusedPermissions(plist)
    const kept = JSON.parse(execFileSync('plutil', ['-convert', 'json', '-o', '-', plist], { encoding: 'utf8' })) as Record<string, string>
    expect(Object.keys(kept)).toEqual(['NSMicrophoneUsageDescription'])
    // Run again on a plist that no longer has them: nothing to take out, and no error.
    expect(() => trimUnusedPermissions(plist)).not.toThrow()
  })

  it('says why Locust wants the microphone, in its own words, and trims before it signs', () => {
    const builder = readFileSync(join(DESKTOP, 'electron-builder.yml'), 'utf8')
    expect(builder).toContain('NSMicrophoneUsageDescription: Locust uses the microphone only while you use voice typing or a voice call.')
    const sign = readFileSync(join(DESKTOP, 'scripts', 'adhoc-sign.cjs'), 'utf8')
    expect(sign.indexOf('trimUnusedPermissions(')).toBeGreaterThan(-1)
    expect(sign.indexOf('trimUnusedPermissions(')).toBeLessThan(sign.indexOf("execFileSync('codesign'"))
  })

  it('is right that nothing in Locust uses the camera or Bluetooth (control)', () => {
    const asks = sources(join(DESKTOP, 'src')).filter((file) => /video\s*:\s*(true|\{)|navigator\.bluetooth|desktopCapturer/.test(readFileSync(file, 'utf8')))
    expect(asks).toEqual([])
  })
})
