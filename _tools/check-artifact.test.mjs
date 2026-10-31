import assert from 'node:assert/strict'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'vitest'
import { checkArtifact } from './check-artifact.mjs'

// A Windows build laid out as electron-builder lays it, in a folder of its own: the archive, the installer, its feed,
// and one file the builder config names beside the archive.
function aBuild({ asar = 'the app says Talk runs on a Mac', feed = 'version: 9.9.9', beside = 'icon' } = {}) {
  const desktop = mkdtempSync(join(tmpdir(), 'check-artifact-'))
  mkdirSync(join(desktop, 'release', 'win-unpacked', 'resources'), { recursive: true })
  mkdirSync(join(desktop, 'resources'), { recursive: true })
  writeFileSync(join(desktop, 'electron-builder.yml'), 'extraResources:\n  - from: resources/icon.png\n    to: icon.png\n')
  writeFileSync(join(desktop, 'resources', 'icon.png'), 'icon')
  writeFileSync(join(desktop, 'release', 'win-unpacked', 'resources', 'icon.png'), beside)
  writeFileSync(join(desktop, 'release', 'win-unpacked', 'resources', 'app.asar'), asar)
  writeFileSync(join(desktop, 'release', 'Locust-9.9.9-setup.exe'), 'installer')
  writeFileSync(join(desktop, 'release', 'latest.yml'), feed)
  return desktop
}

function check(desktop, markers) {
  const passed = []
  const failed = []
  try {
    checkArtifact({ desktop, version: '9.9.9', markers, ok: (what) => passed.push(what), bad: (what) => failed.push(what) })
  } finally {
    rmSync(desktop, { recursive: true, force: true })
  }
  return { passed, failed }
}

test('a whole build passes, with each claim found in the archive', () => {
  const { passed, failed } = check(aBuild(), ['Talk runs on a Mac'])
  assert.deepEqual(failed, [])
  assert.ok(passed.includes('shipped: "Talk runs on a Mac"'))
  assert.ok(passed.includes('latest.yml points at 9.9.9'))
  assert.ok(passed.includes('beside the asar: icon.png'))
})

test('a claim the archive lacks, a feed for another version and a changed file beside the archive each fail', () => {
  assert.deepEqual(check(aBuild(), ['start at login on a Mac']).failed, ['shipped: "start at login on a Mac"'])
  assert.deepEqual(check(aBuild({ feed: 'version: 9.9.8' }), []).failed, ['latest.yml points at 9.9.9'])
  assert.deepEqual(check(aBuild({ beside: 'not the icon' }), []).failed, ['beside the asar: icon.png'])
})
