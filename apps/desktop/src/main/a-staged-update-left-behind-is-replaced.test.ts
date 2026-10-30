import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

/**
 * A STAGED UPDATE LEFT BEHIND IS REPLACED (the first update on a real Mac, 2026-10-30).
 *
 * 0.841 found 0.842, downloaded it and checked it, and then said "The update could not be downloaded" at every check:
 * a staged /Applications/.Locust-update.app from an interrupted copy was there, and removing it before the new copy
 * failed. Electron's fs reads a path through app.asar as a folder in that archive, so a recursive rm of a folder
 * holding a real Contents/Resources/app.asar ends in ENOTEMPTY. Only Electron has that fs, so these run in Electron's
 * own Node (ELECTRON_RUN_AS_NODE), with an asar packed the way the app's is.
 */
const ELECTRON = createRequire(import.meta.url)('electron') as unknown as string

/** An asar archive holding one file, laid out as @electron/asar writes it: two pickles, then the file. */
function asarHolding(name: string, text: string): Buffer {
  const content = Buffer.from(text, 'utf8')
  const json = Buffer.from(JSON.stringify({ files: { [name]: { size: content.length, offset: '0' } } }), 'utf8')
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4)
  json.copy(padded)
  const header = Buffer.alloc(8 + padded.length)
  header.writeUInt32LE(4 + padded.length, 0)
  header.writeUInt32LE(json.length, 4)
  padded.copy(header, 8)
  const size = Buffer.alloc(8)
  size.writeUInt32LE(4, 0)
  size.writeUInt32LE(header.length, 4)
  return Buffer.concat([size, header, content])
}

const folders: string[] = []
afterEach(() => {
  for (const folder of folders.splice(0)) rmSync(folder, { recursive: true, force: true })
})

/** A staged Locust.app with a real app.asar in its Resources. */
function stagedApp(): string {
  const root = mkdtempSync(join(tmpdir(), 'locust-staged-'))
  folders.push(root)
  const resources = join(root, '.Locust-update.app', 'Contents', 'Resources')
  mkdirSync(resources, { recursive: true })
  writeFileSync(join(resources, 'app.asar'), asarHolding('package.json', '{"name":"staged"}'))
  return join(root, '.Locust-update.app')
}

/** Run a module script in Electron's Node, and say what it printed. */
function inElectron(script: string): string {
  return execFileSync(ELECTRON, ['--input-type=module', '-e', script], { env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, encoding: 'utf8', timeout: 30_000 }).trim()
}

describe('a staged Locust.app left behind', () => {
  it.skipIf(process.platform !== 'darwin')("is not removed by Electron's own fs (the failure, as it was)", () => {
    const staged = stagedApp()
    const said = inElectron(`import { rm } from 'node:fs/promises'; await rm(${JSON.stringify(staged)}, { recursive: true, force: true }).then(() => console.log('removed'), (error) => console.log(error.code))`)
    expect(said).toBe('ENOTEMPTY')
    expect(existsSync(staged)).toBe(true)
  })

  it.skipIf(process.platform !== 'darwin')('is removed by removeStagedCopy in Electron, app.asar and all', () => {
    const staged = stagedApp()
    const module = join(__dirname, 'staged-copy.ts')
    const said = inElectron(`const { removeStagedCopy } = await import(${JSON.stringify(module)}); await removeStagedCopy(${JSON.stringify(staged)}).then(() => console.log('removed'), (error) => console.log(error.code))`)
    expect(said).toBe('removed')
    expect(existsSync(staged)).toBe(false)
  })

  it('is what the update removes before it copies the new one', () => {
    const update = readFileSync(join(__dirname, 'mac-self-update.ts'), 'utf8')
    const prepare = update.slice(update.indexOf('const prepare = async'), update.indexOf("run('ditto'"))
    expect(prepare).toContain('await removeStagedCopy(staged)')
    expect(prepare).not.toMatch(/\brm\(staged/)
  })
})
