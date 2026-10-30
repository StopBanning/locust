import { rm } from 'node:fs/promises'
import { createRequire } from 'node:module'

/**
 * REMOVING A STAGED Locust.app (the first update on a real Mac, 2026-10-30).
 *
 * Electron's own fs treats any path through an `app.asar` file as a folder
 * inside that archive. A staged copy holds a real Contents/Resources/app.asar,
 * so `rm(staged, { recursive: true })` in the app failed with ENOTEMPTY, and a
 * copy left behind -- an interrupted staging, or one a newer release overtook
 * -- stopped every later update: "The update could not be downloaded", at
 * every check, for good. Electron's unpatched fs, `original-fs`, removes it.
 *
 * No imports of Locust's own here: a test runs this file in Electron's Node,
 * where the patched fs lives, to prove it.
 */
export async function removeStagedCopy(path: string): Promise<void> {
  if (process.versions.electron === undefined) {
    await rm(path, { recursive: true, force: true })
    return
  }
  const unpatched = createRequire(import.meta.url)('original-fs') as typeof import('node:fs')
  await unpatched.promises.rm(path, { recursive: true, force: true })
}
