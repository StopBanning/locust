import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, utimes, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterAll, describe, expect, it } from 'vitest'

import { folderMarkOf } from './scout-folder.js'

/*
 * SCOUT RUNS THE FOLDER'S CHECK ONLY WHEN THE FOLDER MOVED (0.815, scout-folder.ts). A check can take minutes: two
 * looks at a folder nobody touched must not run it twice, and a save to a file already changed must count as a move.
 */
const roots: string[] = []
afterAll(async () => { await Promise.all(roots.map((root) => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }))) })
const git = (cwd: string, ...args: string[]): void => { execFileSync('git', ['-c', 'user.name=Locust test', '-c', 'user.email=locust-test', ...args], { cwd, stdio: 'pipe' }) }

describe('where a folder stands, for Scout’s check', () => {
  it('stays put while nothing changes, and moves on a commit, a save to a changed file, and a new file', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-scout-folder-'))
    roots.push(folder)
    git(folder, 'init', '-q')
    await writeFile(join(folder, 'app.js'), 'export const total = 1\n')
    git(folder, 'add', '-A')
    git(folder, 'commit', '-q', '-m', 'first')
    const clean = await folderMarkOf(folder)
    expect(clean).toMatch(/^[0-9a-f]{32}$/)
    expect(await folderMarkOf(folder)).toBe(clean)

    await writeFile(join(folder, 'app.js'), 'export const total = 2\n')
    const edited = await folderMarkOf(folder)
    expect(edited).not.toBe(clean)
    expect(await folderMarkOf(folder)).toBe(edited)
    // Saved again, already changed: git's own status reads the same, the file does not.
    await writeFile(join(folder, 'app.js'), 'export const total = 3\n')
    await utimes(join(folder, 'app.js'), new Date(), new Date(Date.now() + 5_000))
    const savedAgain = await folderMarkOf(folder)
    expect(savedAgain).not.toBe(edited)

    await writeFile(join(folder, 'notes.md'), 'new\n')
    const added = await folderMarkOf(folder)
    expect(added).not.toBe(savedAgain)

    git(folder, 'add', '-A')
    git(folder, 'commit', '-q', '-m', 'second')
    const committed = await folderMarkOf(folder)
    expect(committed).not.toBe(added)
    expect(committed).not.toBe(clean)
  })

  it('has no mark outside a git checkout, so Scout runs no check there', async () => {
    const loose = await mkdtemp(join(tmpdir(), 'locust-scout-loose-'))
    roots.push(loose)
    await writeFile(join(loose, 'a.txt'), 'x')
    expect(await folderMarkOf(loose, async () => { throw new Error('fatal: not a git repository') })).toBeUndefined()
  })
})
