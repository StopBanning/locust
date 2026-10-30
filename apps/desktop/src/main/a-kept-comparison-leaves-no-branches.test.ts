import { execFile } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { createWorktreeManager, recordedBranchOf } from './worktrees.js'

/*
 * A KEPT COMPARISON LEAVES NO BRANCHES (0.676).
 *
 * Keep removed the comparison's folders first and asked git to remove the worktrees after: by then a tree's
 * folder could not say which branch it was on, and every comparison kept in a git project left two
 * `locust/compare-...` branches in the person's repository (drive-compare-changes, 2026-10-06). Keep now goes
 * through git first; and a tree whose folder is gone is read from git's own record, which outlives it.
 */
const roots: string[] = []
afterEach(async () => {
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true })
})
const git = (args: readonly string[], cwd: string): Promise<string> =>
  new Promise((resolve, reject) => {
    execFile('git', [...args], { cwd, windowsHide: true }, (error, stdout) => (error ? reject(error) : resolve(stdout)))
  })
async function repository(): Promise<string> {
  const root = await mkdtemp(join(process.env.LOCUST_GATE_TMP ?? tmpdir(), 'locust-kept-compare-'))
  roots.push(root)
  await git(['init', '-q', '-b', 'main'], root)
  await git(['config', 'user.email', 'smoke@locust.test'], root)
  await git(['config', 'user.name', 'Locust smoke'], root)
  await writeFile(join(root, 'NOTES.md'), '# notes\n', 'utf8')
  await git(['add', 'NOTES.md'], root)
  await git(['commit', '-q', '-m', 'first'], root)
  return root
}
const branches = async (root: string): Promise<string[]> =>
  (await git(['branch', '--format=%(refname:short)'], root)).split(/\r?\n/).filter((name) => name.length > 0)

describe("a comparison's worktree", () => {
  it('is discarded with its branch, as before', { timeout: 60_000 }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    await manager.ensure({ teammateId: 'compare-cmp_x-a', name: 'compare cmp_x a' })
    expect((await branches(root)).some((name) => name.startsWith('locust/compare'))).toBe(true)
    await manager.discard('compare-cmp_x-a')
    expect(await branches(root)).toEqual(['main'])
  })

  it('takes its branch with it even when its folder was removed first', { timeout: 60_000 }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    const path = await manager.ensure({ teammateId: 'compare-cmp_y-b', name: 'compare cmp_y b' })
    await rm(path, { recursive: true, force: true })
    await manager.discard('compare-cmp_y-b')
    expect(await branches(root)).toEqual(['main'])
  })
})

describe('recordedBranchOf', () => {
  it("reads a tree's branch from `git worktree list --porcelain`, by path", () => {
    // git's list and the tree's path as the machine running the test gives them: on Windows git writes C:/r for C:\r.
    const windows = process.platform === 'win32'
    const root = windows ? 'C:/r' : '/r'
    const porcelain = `worktree ${root}\nHEAD abc\nbranch refs/heads/main\n\nworktree ${root}/.locust/worktrees/t\nHEAD def\nbranch refs/heads/locust/compare-t\n`
    expect(recordedBranchOf(porcelain, windows ? 'C:\\r\\.locust\\worktrees\\t' : '/r/.locust/worktrees/t')).toBe('locust/compare-t')
    expect(recordedBranchOf(porcelain, windows ? 'C:/elsewhere' : '/elsewhere')).toBe('')
  })
})
