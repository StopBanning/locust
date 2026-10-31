import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createWorktreeManager, defaultRunGit, worktreeAddArgs } from './worktrees.js'

const roots: string[] = []
const git = defaultRunGit
afterEach(async () => { for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }) })
async function repository(hook: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'locust-checkout-hook-'))
  roots.push(root)
  await git(['init', '-q', '-b', 'main'], root)
  await git(['config', 'user.name', 'Locust test'], root)
  await git(['config', 'user.email', 'test@locust.test'], root)
  await git(['config', 'core.autocrlf', 'false'], root)
  await writeFile(join(root, 'file.txt'), 'checked out\n')
  await git(['add', 'file.txt'], root)
  await git(['-c', 'commit.gpgsign=false', 'commit', '-q', '-m', 'fixture'], root)
  await mkdir(join(root, '.git', 'hooks'), { recursive: true })
  await writeFile(join(root, '.git', 'hooks', 'post-checkout'), `#!/bin/sh\n${hook}\n`, { mode: 0o755 })
  return root
}
const teammate = { teammateId: 'tm_hook', name: 'Hook' }

describe('only a finished worktree survives a checkout-hook failure', () => {
  it('accepts and uses the real worktree, keeping full hook stderr in the diagnostic callback', { timeout: 30_000 }, async () => {
    const stderr = 'hook diagnostic: ' + 'x'.repeat(400)
    const root = await repository(`printf '%s\\n' '${stderr}' >&2\nexit 1`)
    const note = vi.fn()
    const manager = createWorktreeManager({ workspacePath: root, note })
    const path = await manager.ensure(teammate)
    expect(await readFile(join(path, 'file.txt'), 'utf8')).toBe('checked out\n')
    expect((await git(['symbolic-ref', '--short', 'HEAD'], path)).trim()).toBe('locust/hook')
    expect((await git(['rev-parse', '--verify', 'HEAD'], path)).trim()).toMatch(/^[0-9a-f]{40}$/)
    expect(note).toHaveBeenCalledOnce()
    expect(note.mock.calls[0]?.[0]).toContain(stderr)
    await writeFile(join(path, 'file.txt'), 'usable after the hook\n')
    expect(await git(['status', '--porcelain'], path)).toContain(' M file.txt')
  })

  it('refuses a hook failure that removes the worktree', { timeout: 30_000 }, async () => {
    // Release the hook's current-directory handle before removing its own
    // checkout on Windows. Only this fixture's linked worktree is removed.
    const root = await repository('tree="$PWD"\ncommon=$(git rev-parse --git-common-dir)\ncd "$common/.." || exit 2\nunset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE\ngit worktree remove --force "$tree" || exit 2\necho removed-worktree >&2\nexit 1')
    const note = vi.fn()
    await expect(createWorktreeManager({ workspacePath: root, note }).ensure(teammate)).rejects.toThrow('removed-worktree')
    const path = join(root, '.locust', 'worktrees', teammate.teammateId)
    expect(await stat(path).then(() => true, () => false)).toBe(false)
    expect(await git(['worktree', 'list', '--porcelain'], root)).not.toContain('refs/heads/locust/hook')
    expect(note).not.toHaveBeenCalled()
  })

  it('refuses a hook that leaves HEAD on the wrong branch', { timeout: 30_000 }, async () => {
    const root = await repository('git symbolic-ref HEAD refs/heads/main\necho wrong-branch >&2\nexit 1')
    const note = vi.fn()
    await expect(createWorktreeManager({ workspacePath: root, note }).ensure(teammate)).rejects.toThrow('wrong-branch')
    expect(note).not.toHaveBeenCalled()
  })

  it('requires HEAD verification to succeed before accepting a nonzero add', { timeout: 30_000 }, async () => {
    const root = await repository('echo failed-hook >&2\nexit 1')
    const note = vi.fn()
    const runGit = vi.fn(async (args: readonly string[], cwd: string, timeout?: number) => {
      if (args.join(' ') === 'rev-parse --verify HEAD') throw new Error('unreadable HEAD')
      return git(args, cwd, timeout)
    })
    await expect(createWorktreeManager({ workspacePath: root, note, runGit }).ensure(teammate)).rejects.toThrow('failed-hook')
    expect(runGit.mock.calls.some(([args, cwd]) => args.join(' ') === 'rev-parse --verify HEAD' && cwd === join(root, '.locust', 'worktrees', teammate.teammateId))).toBe(true)
    expect(note).not.toHaveBeenCalled()
  })

  it('does not treat a timeout as a finished hook failure', { timeout: 30_000 }, async () => {
    const root = await repository('exit 0')
    const note = vi.fn()
    const timeout = new Error('checkout timed out')
    const runGit = async (args: readonly string[], cwd: string, milliseconds?: number) => {
      const result = await git(args, cwd, milliseconds)
      if (args.includes('add')) throw timeout
      return result
    }
    await expect(createWorktreeManager({ workspacePath: root, note, runGit }).ensure(teammate)).rejects.toBe(timeout)
    expect(note).not.toHaveBeenCalled()
  })

  it('keeps an initializing checkout refused even if its recorded branch and HEAD exist', { timeout: 30_000 }, async () => {
    const root = await repository('admin=$(git rev-parse --git-dir)\nprintf "initializing\\n" > "$admin/locked"\necho unfinished-hook >&2\nexit 1')
    const note = vi.fn()
    await expect(createWorktreeManager({ workspacePath: root, note }).ensure(teammate)).rejects.toThrow('unfinished-hook')
    expect(note).not.toHaveBeenCalled()
  })

  it('does not fail an otherwise usable tree if the diagnostics callback fails', { timeout: 30_000 }, async () => {
    const root = await repository('echo hook-failed >&2\nexit 1')
    const note = vi.fn(() => { throw new Error('log unavailable') })
    const path = await createWorktreeManager({ workspacePath: root, note }).ensure(teammate)
    expect(await readFile(join(path, 'file.txt'), 'utf8')).toBe('checked out\n')
    expect(note).toHaveBeenCalledOnce()
  })

  it('uses longpaths only on add commands without changing core.longpaths config', { timeout: 30_000 }, async () => {
    const root = await repository('exit 0')
    await git(['config', 'core.longpaths', 'false'], root)
    const calls: string[][] = []
    const manager = createWorktreeManager({ workspacePath: root, runGit: (args, cwd, timeout) => { calls.push([...args]); return git(args, cwd, timeout) } })
    const path = await manager.ensure(teammate)
    expect((await git(['config', '--local', '--get', 'core.longpaths'], root)).trim()).toBe('false')
    await manager.remove(teammate.teammateId)
    expect(await manager.ensure(teammate)).toBe(path)
    const adds = calls.filter(args => args.includes('add'))
    expect(adds).toEqual([worktreeAddArgs(path, 'locust/hook', false), worktreeAddArgs(path, 'locust/hook', true)])
    expect(calls.filter(args => args.includes('core.longpaths=true'))).toEqual(process.platform === 'win32' ? adds : [])
    expect((await git(['config', '--local', '--get', 'core.longpaths'], root)).trim()).toBe('false')
  })

  it.each(['win32', 'darwin', 'linux'] as const)('adds the longpaths flag only for Windows (%s), on both branch forms', platform => {
    const prefix = platform === 'win32' ? ['-c', 'core.longpaths=true'] : []
    expect(worktreeAddArgs('/tree', 'locust/test', false, platform)).toEqual([...prefix, 'worktree', 'add', '-b', 'locust/test', '/tree', 'HEAD'])
    expect(worktreeAddArgs('/tree', 'locust/test', true, platform)).toEqual([...prefix, 'worktree', 'add', '/tree', 'locust/test'])
  })
})
