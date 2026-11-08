import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { checkAppFolder } from './app-folder.js'

/**
 * AN APP NAMES A FOLDER LOCUST KNOWS (0.869). The permission boundary of an app choosing where a turn works: a folder
 * Locust knows, one inside it, or a worktree of one of their repositories -- the case Colin hit, worktrees in
 * ~/work/locust-wt of the project open in Locust. Real folders and a real `git worktree add`.
 */
let root: string
let project: string
let worktree: string
let stranger: string
const git = (args: readonly string[], cwd: string): string => execFileSync('git', [...args], { cwd, encoding: 'utf8' }).trim()
const commonDir = async (folder: string): Promise<string | undefined> => {
  try { return git(['rev-parse', '--path-format=absolute', '--git-common-dir'], folder) } catch { return undefined }
}

beforeAll(async () => {
  root = await realpath(await mkdtemp(join(tmpdir(), 'locust-app-folder-')))
  project = join(root, 'now-coding')
  worktree = join(root, 'locust-wt', 'readme-pass')
  stranger = join(root, 'someone-else')
  await mkdir(join(project, 'src'), { recursive: true })
  await mkdir(stranger, { recursive: true })
  git(['init', '-q', '-b', 'main'], project)
  await writeFile(join(project, 'README.md'), 'hi\n')
  git(['-c', 'user.email=t@example.invalid', '-c', 'user.name=t', 'add', 'README.md'], project)
  git(['-c', 'user.email=t@example.invalid', '-c', 'user.name=t', 'commit', '-q', '-m', 'one'], project)
  await mkdir(join(root, 'locust-wt'), { recursive: true })
  git(['worktree', 'add', '-q', '-b', 'exec/readme-pass', worktree, 'HEAD'], project)
  git(['init', '-q', '-b', 'main'], stranger)
})
afterAll(async () => { await rm(root, { recursive: true, force: true }) })

const checks = () => ({ known: async () => [project], commonDir })

describe('a folder an app names', () => {
  it('is taken when it is the open folder, or inside it', async () => {
    expect(await checkAppFolder(project, checks())).toEqual({ path: project })
    expect(await checkAppFolder(join(project, 'src'), checks())).toEqual({ path: join(project, 'src') })
  })

  it("is taken when it is a worktree of the open folder's repository, wherever it was made", async () => {
    expect(await checkAppFolder(worktree, checks())).toEqual({ path: worktree })
  })

  it('is refused when Locust never knew it, even a repository of its own', async () => {
    const result = await checkAppFolder(stranger, checks())
    expect('refused' in result && result.refused).toMatch(/is not a folder Locust knows/)
  })

  it('is refused when it is the folder above a known one, a relative path, missing, or a file', async () => {
    expect('refused' in await checkAppFolder(root, checks())).toBe(true)
    expect(await checkAppFolder('now-coding', checks())).toEqual({ refused: 'folder is a full path, from the top of the disk.' })
    expect(await checkAppFolder(join(root, 'gone'), checks())).toEqual({ refused: `There is no folder at ${join(root, 'gone')}.` })
    expect(await checkAppFolder(join(project, 'README.md'), checks())).toEqual({ refused: `${join(project, 'README.md')} is a file, not a folder.` })
  })

  it('is refused when a path climbs out of a known folder', async () => {
    expect('refused' in await checkAppFolder(join(project, '..', 'someone-else'), checks())).toBe(true)
  })
})
