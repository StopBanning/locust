import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { afterAll, describe, expect, it } from 'vitest'

import { checkoutOf, pathsNamedIn, projectNamedIn, repositoryOf } from './named-folder.js'

/*
 * A MESSAGE THAT NAMES ITS PROJECT STARTS THERE (0.814, named-folder.ts). A handoff pasted into a conversation that
 * stands in .claude names a brief inside a worktree; the conversation starts in that worktree, and a message that
 * only mentions some other repository from inside a project stays where it is.
 */
const root = mkdtempSync(join(tmpdir(), 'locust-named-folder-'))
afterAll(() => rmSync(root, { recursive: true, force: true }))

const made = (path: string, body = ''): string => {
  mkdirSync(join(path, '..'), { recursive: true })
  writeFileSync(path, body)
  return path
}
const repo = join(root, 'locust')
mkdirSync(join(repo, '.git', 'worktrees', 'grok'), { recursive: true })
made(join(repo, 'docs', 'PLAN.md'))
const worktree = join(root, 'locust-wt', 'grok')
made(join(worktree, '.git'), `gitdir: ${join(repo, '.git', 'worktrees', 'grok')}\n`)
const brief = made(join(worktree, 'docs', 'HANDOFF-grok.md'), '# A brief')
const other = join(root, 'elsewhere')
mkdirSync(join(other, '.git'), { recursive: true })
const otherFile = made(join(other, 'README.md'))
const settings = join(root, '.claude')
mkdirSync(settings, { recursive: true })
const loose = made(join(root, 'loose', 'notes.md'))

const same = (one: string, other: string): boolean => resolve(one).toLowerCase() === resolve(other).toLowerCase()
const isNoProject = (folder: string): boolean => same(folder, settings)
const from = (current: string, text: string): string | undefined => projectNamedIn(text, { current, isNoProject, same })

describe('a message that names its project starts there', () => {
  it('reads the paths a message names, without quotes or end punctuation, and not a web address', () => {
    expect(pathsNamedIn('Read C:\\work\\locust-wt\\grok\\docs\\HANDOFF.md and do what it says.', 'win32')).toEqual(['C:\\work\\locust-wt\\grok\\docs\\HANDOFF.md'])
    expect(pathsNamedIn('See `D:\\My Projects\\app\\notes.md`, then C:/work/locust.', 'win32')).toEqual(['D:\\My Projects\\app\\notes.md', 'C:/work/locust'])
    expect(pathsNamedIn('Open https://github.com/example/app/pull/1 please', 'win32')).toEqual([])
    expect(pathsNamedIn('Read /home/me/work/app/BRIEF.md, then reply.', 'linux')).toEqual(['/home/me/work/app/BRIEF.md'])
    expect(pathsNamedIn('Open https://example.com/a/b and ~/x', 'linux')).toEqual([])
  })

  it('finds the checkout a path is in, and the repository a worktree shares', () => {
    expect(checkoutOf(brief)).toBe(resolve(worktree))
    expect(checkoutOf(join(repo, 'docs'))).toBe(resolve(repo))
    expect(checkoutOf(loose)).toBeUndefined()
    expect(checkoutOf(join(worktree, 'docs', 'not-there.md'))).toBeUndefined()
    expect(repositoryOf(worktree)).toBe(resolve(repo, '.git'))
    expect(repositoryOf(repo)).toBe(resolve(repo, '.git'))
  })

  it('from a folder like .claude, a brief inside a worktree starts the conversation in that worktree', () => {
    expect(from(settings, brief)).toBe(resolve(worktree))
    expect(from(settings, `Read ${brief} and do what it says.`)).toBe(resolve(worktree))
  })

  it('from one checkout, a brief inside another worktree of the same repository starts there', () => {
    expect(from(repo, `Read ${brief} and do what it says.`)).toBe(resolve(worktree))
    expect(from(worktree, `Compare with ${join(repo, 'docs', 'PLAN.md')}`)).toBe(resolve(repo))
  })

  it('stays where it is for another repository, its own checkout, a loose file or a path that is not there', () => {
    expect(from(repo, `How does ${otherFile} do it?`)).toBeUndefined()
    expect(from(worktree, `Read ${brief} again.`)).toBeUndefined()
    expect(from(join(worktree, 'docs'), `Read ${brief} again.`)).toBeUndefined()
    expect(from(settings, `Read ${loose}.`)).toBeUndefined()
    expect(from(settings, `Read ${join(worktree, 'gone.md')}.`)).toBeUndefined()
    expect(from(settings, 'Just a question, no path.')).toBeUndefined()
  })

  it('never moves into a folder that is itself no place for a project', () => {
    const inside = made(join(settings, 'plugins', 'x', 'SKILL.md'))
    mkdirSync(join(settings, '.git'), { recursive: true })
    try {
      expect(from(settings, `Read ${inside}`)).toBeUndefined()
      expect(from(repo, `Read ${inside}`)).toBeUndefined()
    } finally {
      rmSync(join(settings, '.git'), { recursive: true, force: true })
    }
  })
})
