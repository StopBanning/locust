import { execFile } from 'node:child_process'
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { branchNameFor, changedPathsOf, createWorktreeManager, gitVersionSupportsWorktrees, parseGitVersion, parseWorktreeList, WorktreeHasChangesError } from './worktrees.js'

/*
 * These tests drive REAL git against REAL directories, and that is the point
 * of them -- the bugs they exist to catch are git's behaviour, not ours. What
 * it costs is that they are the only tests here whose runtime depends on the
 * machine, and twice on 2026-09-13 and once more on 2026-09-14 the
 * two-teammate case timed out under parallel load and then passed alone.
 *
 * Neither symptom was our code. The first is arithmetic: that case runs twelve
 * git invocations end to end, and twelve process starts on Windows -- with a
 * scanner in front of each -- does not reliably fit in vitest's 5s default
 * while the rest of the suite is using the cores. It gets a budget that
 * matches what it actually does.
 *
 * The second is Windows: `git worktree` leaves handles on the directory for a
 * moment after the process exits, so an immediate recursive delete raises
 * EBUSY. `rm` will wait if asked, and asking is the whole fix.
 */
const CLEANUP = { recursive: true, force: true, maxRetries: 5, retryDelay: 100 } as const

/** Twelve git process starts, measured, not guessed. */
const REAL_GIT_TIMEOUT_MS = 30_000

const roots: string[] = []
afterEach(async () => {
  await Promise.all(roots.splice(0).map((directory) => rm(directory, CLEANUP)))
})

const git = (args: readonly string[], cwd: string): Promise<string> =>
  new Promise((resolve, reject) => {
    execFile('git', [...args], { cwd, windowsHide: true }, (error, stdout) => (error ? reject(error) : resolve(stdout)))
  })

/** A real repository with one commit, so worktree commands have a HEAD to branch from. */
async function repository(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'locust-worktrees-'))
  roots.push(root)
  await git(['init', '-q', '-b', 'main'], root)
  await git(['config', 'user.email', 'smoke@locust.test'], root)
  await git(['config', 'user.name', 'Locust smoke'], root)
  await writeFile(join(root, 'NOTES.md'), '# notes\n', 'utf8')
  await git(['add', 'NOTES.md'], root)
  await git(['commit', '-q', '-m', 'first'], root)
  return root
}

describe('the pure parts', () => {
  it('names a branch from the teammate, folded to what git accepts', () => {
    expect(branchNameFor('Wren')).toBe('locust/wren')
    expect(branchNameFor('  Booty Call! ')).toBe('locust/booty-call')
    expect(branchNameFor('...')).toBe('locust/teammate')
    // M17: a name that folds to nothing is named by the teammate instead, so
    // two such teammates never share a branch.
    expect(branchNameFor('小明', 'tm_1a2b3c4d')).toBe('locust/tm_1a2b3c4d')
    expect(branchNameFor('小红', 'tm_9f8e7d6c')).toBe('locust/tm_9f8e7d6c')
    expect(branchNameFor('Wren', 'tm_1a2b3c4d')).toBe('locust/wren')
    expect(branchNameFor('x'.repeat(80)).length).toBeLessThanOrEqual('locust/'.length + 40)
  })

  it('reads the git version and knows which ones have worktrees', () => {
    expect(parseGitVersion('git version 2.45.1.windows.1\n')).toBe('2.45.1')
    expect(gitVersionSupportsWorktrees('2.45.1')).toBe(true)
    expect(gitVersionSupportsWorktrees('2.4.9')).toBe(false)
    expect(gitVersionSupportsWorktrees('3.0')).toBe(true)
  })

  it('keeps only the trees Locust made under the folder out of a porcelain list', () => {
    // git's list and the folder as the machine running the test gives them: on Windows git writes C:/w/shop for C:\w\shop.
    const windows = process.platform === 'win32'
    const shop = windows ? 'C:/w/shop' : '/w/shop'
    const output = [
      `worktree ${shop}`,
      'HEAD abc',
      'branch refs/heads/main',
      '',
      `worktree ${shop}/.locust/worktrees/tm_wren`,
      'HEAD def',
      'branch refs/heads/locust/wren',
      '',
      `worktree ${windows ? 'C:/elsewhere/other' : '/elsewhere/other'}`,
      'HEAD 123',
      'branch refs/heads/feature',
      ''
    ].join('\n')
    expect(parseWorktreeList(output, windows ? 'C:\\w\\shop' : shop)).toEqual([{ teammateId: 'tm_wren', path: `${shop}/.locust/worktrees/tm_wren`, branch: 'locust/wren' }])
  })
})

describe('a worktree per teammate, on a real repository', () => {
  it('refuses a folder that is not a repository, with the reason', async () => {
    const plain = await mkdtemp(join(tmpdir(), 'locust-notrepo-'))
    roots.push(plain)
    const manager = createWorktreeManager({ workspacePath: plain })
    const probe = await manager.probe()
    expect(probe.repository).toBe(false)
    expect(probe.reason).toMatch(/not a git repository/)
    await expect(manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })).rejects.toThrow(/not a git repository/)
  })

  it('makes the tree under .locust/worktrees on its own branch, excludes .locust, and is idempotent', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    expect((await manager.probe()).repository).toBe(true)
    const path = await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })
    expect(path.replace(/\\/g, '/')).toBe(`${root.replace(/\\/g, '/')}/.locust/worktrees/tm_wren`)
    expect((await stat(join(path, 'NOTES.md'))).isFile()).toBe(true)
    expect((await git(['rev-parse', '--abbrev-ref', 'HEAD'], path)).trim()).toBe('locust/wren')
    expect(await readFile(join(root, '.git', 'info', 'exclude'), 'utf8')).toContain('.locust/')
    // The main checkout does not see the new directory as untracked.
    expect((await git(['status', '--porcelain'], root)).trim()).toBe('')
    expect(await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })).toBe(path)
    expect((await manager.list()).map((entry) => [entry.teammateId, entry.branch])).toEqual([['tm_wren', 'locust/wren']])
  })

  /*
   * M17 (the code review): two names that fold to the same branch -- "Dev 1"
   * and "Dev-1", "Wren" and "wren", or any two non-Latin names -- and git
   * refuses the second tree: the branch is already checked out. That
   * teammate's start was refused with raw git text.
   */
  it('two teammates whose names fold to one branch each get a tree', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    await manager.ensure({ teammateId: 'tm_dev_one_a1', name: 'Dev 1' })
    const second = await manager.ensure({ teammateId: 'tm_dev_one_b2', name: 'Dev-1' })
    expect((await git(['rev-parse', '--abbrev-ref', 'HEAD'], second)).trim()).toBe('locust/dev-1-dev_one_b2')
    await manager.ensure({ teammateId: 'tm_ming_c3', name: '小明' })
    await manager.ensure({ teammateId: 'tm_hong_d4', name: '小红' })
    expect((await manager.list()).map((entry) => entry.branch).sort()).toEqual(['locust/dev-1', 'locust/dev-1-dev_one_b2', 'locust/tm_hong_d4', 'locust/tm_ming_c3'])
  })

  it('two teammates get two trees that do not see each other, and the main checkout stays untouched', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    const wren = await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })
    const booty = await manager.ensure({ teammateId: 'tm_booty', name: 'Booty' })
    await writeFile(join(wren, 'NOTES.md'), '# notes\nwren was here\n', 'utf8')
    await writeFile(join(booty, 'NOTES.md'), '# notes\nbooty was here\n', 'utf8')
    expect(await readFile(join(root, 'NOTES.md'), 'utf8')).toBe('# notes\n')
    expect(await readFile(join(wren, 'NOTES.md'), 'utf8')).toContain('wren')
    expect(await readFile(join(booty, 'NOTES.md'), 'utf8')).not.toContain('wren')
    expect((await manager.list()).map((entry) => entry.branch).sort()).toEqual(['locust/booty', 'locust/wren'])
    // Booty's edit is uncommitted: removing the tree deletes it, so it goes only when agreed to (C1).
    await manager.remove('tm_booty', { discard: ['NOTES.md'] })
    expect((await manager.list()).map((entry) => entry.teammateId)).toEqual(['tm_wren'])
    // The branch outlives the tree: the work is the person's to merge or drop.
    expect((await git(['branch', '--list', 'locust/booty'], root)).trim()).toContain('locust/booty')
    // A tree removed and asked for again comes back on the same branch.
    const again = await manager.ensure({ teammateId: 'tm_booty', name: 'Booty' })
    expect((await git(['rev-parse', '--abbrev-ref', 'HEAD'], again)).trim()).toBe('locust/booty')
  })

  /*
   * C1, the code review's one critical: Remove ran `git worktree remove
   * --force`, deleting a teammate's uncommitted and untracked work on one
   * click, under "Removing one keeps its branch" -- and nothing in Locust
   * commits, so that was all of it. Real git, a real tree.
   */
  it('C1: refuses to remove a copy with uncommitted work, naming it, and keeps every file', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    const wren = await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })
    await writeFile(join(wren, 'NOTES.md'), '# notes\nwren was here\n', 'utf8')
    await writeFile(join(wren, 'REPORT.md'), 'the findings\n', 'utf8')
    const refused = await manager.remove('tm_wren').then(() => undefined, (error: unknown) => error)
    expect(refused).toBeInstanceOf(WorktreeHasChangesError)
    expect([...(refused as WorktreeHasChangesError).changes].sort()).toEqual(['NOTES.md', 'REPORT.md'])
    expect(await readFile(join(wren, 'REPORT.md'), 'utf8')).toBe('the findings\n')
    expect((await manager.list()).map((entry) => entry.teammateId)).toEqual(['tm_wren'])
  })

  it('C1: removes it only for the exact changes agreed to -- one made since is refused again', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    const wren = await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })
    await writeFile(join(wren, 'REPORT.md'), 'the findings\n', 'utf8')
    // Shown REPORT.md; a second file appeared before the person agreed.
    await writeFile(join(wren, 'LATER.md'), 'written after\n', 'utf8')
    await expect(manager.remove('tm_wren', { discard: ['REPORT.md'] })).rejects.toBeInstanceOf(WorktreeHasChangesError)
    expect(await readFile(join(wren, 'LATER.md'), 'utf8')).toBe('written after\n')
    await manager.remove('tm_wren', { discard: ['LATER.md', 'REPORT.md'] })
    expect(await manager.list()).toEqual([])
    expect((await git(['branch', '--list', 'locust/wren'], root)).trim()).toContain('locust/wren')
  })

  it('C1: removes a clean copy at once, with git’s own unforced remove', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const calls: string[][] = []
    const manager = createWorktreeManager({
      workspacePath: root,
      runGit: (args, cwd) => {
        calls.push([...args])
        return git(args, cwd)
      }
    })
    await manager.ensure({ teammateId: 'tm_wren', name: 'Wren' })
    await manager.remove('tm_wren')
    expect(await manager.list()).toEqual([])
    const removal = calls.find((args) => args[0] === 'worktree' && args[1] === 'remove')
    expect(removal).not.toContain('--force')
  })

  it('names changes the way git status does, a quoted path unquoted', () => {
    expect(changedPathsOf(' M NOTES.md\n?? REPORT.md\r\n?? "with space.md"\n')).toEqual(['NOTES.md', 'REPORT.md', 'with space.md'])
    expect(changedPathsOf('')).toEqual([])
  })

  it('refuses an id that could name a path outside the folder', { timeout: REAL_GIT_TIMEOUT_MS }, async () => {
    const root = await repository()
    const manager = createWorktreeManager({ workspacePath: root })
    await expect(manager.ensure({ teammateId: '../escape', name: 'x' })).rejects.toThrow(/cannot name a worktree/)
    await expect(manager.remove('..')).rejects.toThrow(/cannot name a worktree/)
  })
})
