import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { holdsNothingYet } from './workspace.js'

/**
 * A FOLDER WITH ONLY DOT FILES HOLDS NOTHING YET (2026-10-30). The folder Locust makes for a new person gets a
 * `.locust` folder at the first turn, and is still empty as far as the person is concerned: a teammate there offers to
 * make something. The first real file ends that, and a folder that cannot be read is never called empty.
 */
describe('a folder that holds nothing yet', () => {
  it('is one with nothing, or nothing but dot files, in it', () => {
    const folder = mkdtempSync(join(tmpdir(), 'locust-empty-'))
    try {
      expect(holdsNothingYet(folder)).toBe(true)
      mkdirSync(join(folder, '.locust'))
      writeFileSync(join(folder, '.locust', 'memory.md'), 'nothing yet')
      writeFileSync(join(folder, '.DS_Store'), '')
      expect(holdsNothingYet(folder)).toBe(true)
      writeFileSync(join(folder, 'index.html'), '<h1>Crumb</h1>')
      expect(holdsNothingYet(folder)).toBe(false)
    } finally {
      rmSync(folder, { recursive: true, force: true })
    }
  })

  it('is never a folder that cannot be read', () => {
    expect(holdsNothingYet(join(tmpdir(), 'locust-no-such-folder-30-10'))).toBe(false)
  })
})
