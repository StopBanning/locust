import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { createMemoryStore } from './memory-store.js'
import { createRoomStore } from './room-store.js'
import { createRoutineStore } from './routine-store.js'
import { createTeammateStore } from './teammate-store.js'

/**
 * A store that cannot save must not answer as though it did.
 *
 * This is fault injection against the real filesystem rather than a stubbed
 * function: the store is pointed at a root that CANNOT hold its file, because
 * a plain file already sits where its directory needs to be. Every write then
 * fails the way a full or read-only disk fails, through the same syscalls, at
 * the same place.
 *
 * It exists because of what today produced. Three defects, all the same shape,
 * none of them in the saving mechanism -- the stores write-and-rename, which
 * is atomic, and the ledger truncates at the first torn line, which is
 * crash-consistent. The bugs were all in what happens when those mechanisms
 * REPORT a failure:
 *
 *   - a live run's receipt failure was discarded, and the run carried on
 *     working with nothing recorded (0.51.0)
 *   - the same on the Antigravity path, which additionally advanced its
 *     transcript cursor past events it had failed to write (0.51.1)
 *   - a failed settings write answered with hardcoded defaults, so the
 *     renderer showed switches that contradicted the file on disk
 *
 * The mechanism was never the weak part. The reporting was. So this pins the
 * one invariant that failure reporting rests on: **a mutating store call that
 * cannot reach the disk REJECTS.** Whatever a caller then chooses to do --
 * stop the run, tell the person, retry -- it can only do it if the call said
 * so, and every one of those three defects began with a call that did not.
 */

/** A root directory that no store can write into: a file blocks its path. */
async function unwritableRoot(): Promise<string> {
  const base = await mkdtemp(join(tmpdir(), 'locust-faultinjection-'))
  const root = join(base, 'blocked')
  // Not a directory. Anything trying to create or open a file beneath it gets
  // a real ENOTDIR/ENOENT from the OS.
  await writeFile(root, 'this is a file where a directory is needed', 'utf8')
  return root
}

describe('a store that cannot reach the disk says so', () => {
  it('the fault is real: the root genuinely cannot be written into', async () => {
    // The control. If the root turned out to be writable, every expectation
    // below would be asserting against a store that never failed -- and they
    // would all pass, for the wrong reason.
    const root = await unwritableRoot()
    await expect(writeFile(join(root, 'probe.json'), 'x', 'utf8')).rejects.toThrow()
  })

  it('and the SAME calls all succeed on a root that works', async () => {
    /*
     * The control that makes the rest mean anything.
     *
     * Every expectation below asserts a rejection -- and a rejection is also
     * what a bad argument produces. Without this, a typo in a hue or a missing
     * field would make all five pass while proving nothing whatever about the
     * disk. Same arguments, writable root, all resolve.
     */
    const good = async (): Promise<string> => mkdtemp(join(tmpdir(), 'locust-writable-'))
    await expect(
      createTeammateStore({ rootDirectory: await good() }).create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })
    ).resolves.toBeDefined()
    await expect(
      createTeammateStore({ rootDirectory: await good() }).writeSettings({ relay: false })
    ).resolves.toBeDefined()
    await expect(
      createRoutineStore({ rootDirectory: await good() }).create({
        name: 'Nightly',
        teammateId: 'tm_wren',
        route: { runtime: 'codex', model: 'gpt-6-astra', mode: 'accept-edits' },
        steps: ['Check the build.'],
        learnedFrom: []
      })
    ).resolves.toBeDefined()
    await expect(
      createRoomStore({ rootDirectory: await good() }).create({ name: 'Standup', teammateIds: ['tm_wren'] })
    ).resolves.toBeDefined()
    await expect(
      createMemoryStore({ rootDirectory: await good() }).add({
        text: 'The build is green on Windows only.',
        scope: 'global',
        workspaceId: 'ws_1',
        workspaceName: 'scratch',
        by: { name: 'Colin' },
        status: 'kept'
      })
    ).resolves.toBeDefined()
  })

  it('the teammate roster rejects rather than pretending to save', async () => {
    const store = createTeammateStore({ rootDirectory: await unwritableRoot() })
    await expect(store.create({ name: 'Wren', hue: 'lime', role: 'Code & Migrations' })).rejects.toThrow()
  })

  it('workspace settings reject rather than pretending to save', async () => {
    // The one whose failure path was answering with invented defaults until
    // today. The store itself has to reject for the caller to have anything
    // to react to.
    const store = createTeammateStore({ rootDirectory: await unwritableRoot() })
    await expect(store.writeSettings({ relay: false })).rejects.toThrow()
  })

  it('a routine rejects rather than pretending to save', async () => {
    const store = createRoutineStore({ rootDirectory: await unwritableRoot() })
    await expect(
      store.create({
        name: 'Nightly',
        teammateId: 'tm_wren',
        route: { runtime: 'codex', model: 'gpt-6-astra', mode: 'accept-edits' },
        steps: ['Check the build.'],
        learnedFrom: []
      })
    ).rejects.toThrow()
  })

  it('a room rejects rather than pretending to save', async () => {
    const store = createRoomStore({ rootDirectory: await unwritableRoot() })
    await expect(store.create({ name: 'Standup', teammateIds: ['tm_wren'] })).rejects.toThrow()
  })

  it('a memory rejects rather than pretending to save', async () => {
    const store = createMemoryStore({ rootDirectory: await unwritableRoot() })
    await expect(store.add({
        text: 'The build is green on Windows only.',
        scope: 'global',
        workspaceId: 'ws_1',
        workspaceName: 'scratch',
        by: { name: 'Colin' },
        status: 'kept'
      })).rejects.toThrow()
  })
})
