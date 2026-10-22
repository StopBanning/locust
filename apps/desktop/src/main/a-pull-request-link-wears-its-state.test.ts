import { describe, expect, it } from 'vitest'

import { createPullLinkReader } from './pull-link.js'
import {
  LINK_PULL_FIELDS,
  conversationPieces,
  githubLinkOf,
  linkedPullRequestOf,
  pullLinkHover,
  pullLinkRequest
} from '../shared/pull-request.js'

/*
 * A PULL REQUEST LINK IN A CONVERSATION.
 *
 * The header already reads the folder's own pull request. A link in what was
 * said is a different one: any owner, any repo, that number. The window may
 * ask for those three and nothing else. gh's answer becomes Open, Draft,
 * Merged or Closed, and one mark for the checks.
 */

const run = (conclusion: string, status = 'COMPLETED') => ({ __typename: 'CheckRun', conclusion, status, name: 'CI' })
const body = (over: Record<string, unknown>) =>
  JSON.stringify({ state: 'OPEN', isDraft: false, mergedAt: null, title: 'Fix the cart', statusCheckRollup: [], ...over })
const address = { owner: 'acme', repo: 'app', number: 42 }

describe('which links wear a chip', () => {
  it('is a pull request, and an issue is named beside it', () => {
    expect(githubLinkOf('https://github.com/acme/app/pull/42')).toEqual({ kind: 'pull', owner: 'acme', repo: 'app', number: 42 })
    expect(githubLinkOf('https://github.com/acme/app/pull/42/')).toEqual({ kind: 'pull', owner: 'acme', repo: 'app', number: 42 })
    expect(githubLinkOf('https://www.github.com/acme/app/pull/42?w=1#discussion')).toEqual({ kind: 'pull', owner: 'acme', repo: 'app', number: 42 })
    expect(githubLinkOf('https://github.com/acme/app/issues/3')).toEqual({ kind: 'issue', owner: 'acme', repo: 'app', number: 3 })
  })

  it('leaves a commit, a file, and anything that is not that address', () => {
    expect(githubLinkOf('https://github.com/acme/app/commit/abcdef1')).toBeUndefined()
    expect(githubLinkOf('https://github.com/acme/app/blob/main/README.md')).toBeUndefined()
    expect(githubLinkOf('https://github.com/acme/app/pull/42/files')).toBeUndefined()
    expect(githubLinkOf('http://github.com/acme/app/pull/42')).toBeUndefined()
    expect(githubLinkOf('https://github.com.evil/acme/app/pull/42')).toBeUndefined()
    expect(githubLinkOf('https://evil.example/acme/app/pull/42')).toBeUndefined()
    expect(githubLinkOf('https://user:token@github.com/acme/app/pull/42')).toBeUndefined()
    expect(githubLinkOf('https://github.com/-acme/app/pull/1')).toBeUndefined()
    expect(githubLinkOf('https://github.com/acme/app/pull/0')).toBeUndefined()
    expect(githubLinkOf('https://github.com/acme/../pull/1')).toBeUndefined()
  })

  it('finds a bare pull request in a sentence, and the issue beside it as a link of its own kind', () => {
    const pieces = conversationPieces('Opened https://github.com/acme/app/pull/42 today. See https://github.com/acme/app/issues/3.')
    expect(pieces).toEqual([
      { kind: 'text', text: 'Opened ' },
      { kind: 'link', text: 'https://github.com/acme/app/pull/42', href: 'https://github.com/acme/app/pull/42', pull: { owner: 'acme', repo: 'app', number: 42 } },
      { kind: 'text', text: ' today. See ' },
      { kind: 'link', text: 'https://github.com/acme/app/issues/3', href: 'https://github.com/acme/app/issues/3', issue: { owner: 'acme', repo: 'app', number: 3 } },
      { kind: 'text', text: '.' }
    ])
    expect(conversationPieces('See [the issue](https://github.com/acme/app/issues/3) and [the file](https://github.com/acme/app/blob/main/README.md).')).toEqual([
      { kind: 'text', text: 'See ' },
      { kind: 'link', text: 'the issue', href: 'https://github.com/acme/app/issues/3', issue: { owner: 'acme', repo: 'app', number: 3 } },
      { kind: 'text', text: ' and ' },
      { kind: 'link', text: 'the file', href: 'https://github.com/acme/app/blob/main/README.md' },
      { kind: 'text', text: '.' }
    ])
    expect(conversationPieces('Run `https://github.com/acme/app/pull/42` first.')).toEqual([
      { kind: 'text', text: 'Run `https://github.com/acme/app/pull/42` first.' }
    ])
  })
})

describe('the request the window may make', () => {
  it('refuses anything that is not an owner, a repo and a number', () => {
    expect(pullLinkRequest(address)).toEqual(address)
    expect(pullLinkRequest({ ...address, cwd: 'C:/work' })).toBeUndefined()
    expect(pullLinkRequest({ ...address, args: ['auth', 'token'] })).toBeUndefined()
    expect(pullLinkRequest({ owner: 'acme', repo: 'app', number: '42' })).toBeUndefined()
    expect(pullLinkRequest({ owner: '-acme', repo: 'app', number: 1 })).toBeUndefined()
    expect(pullLinkRequest({ owner: 'acme', repo: '..', number: 1 })).toBeUndefined()
    expect(pullLinkRequest({ owner: 'acme', repo: 'app', number: 0 })).toBeUndefined()
    expect(pullLinkRequest('gh pr view 42 --repo acme/app')).toBeUndefined()
    expect(pullLinkRequest(['acme', 'app', 42])).toBeUndefined()
  })
})

describe('what gh said', () => {
  it('reads open, draft, merged and closed', () => {
    expect(linkedPullRequestOf(body({}))).toMatchObject({ state: 'open', checks: 'none', total: 0 })
    expect(linkedPullRequestOf(body({ isDraft: true }))).toMatchObject({ state: 'draft', checks: 'none' })
    expect(linkedPullRequestOf(body({ state: 'MERGED', mergedAt: '2026-10-18T12:00:00Z', statusCheckRollup: [run('SUCCESS'), run('SUCCESS')] }))).toMatchObject({
      state: 'merged', checks: 'none', total: 2
    })
    expect(linkedPullRequestOf(body({ state: 'CLOSED', mergedAt: '2026-10-01T00:00:00Z' }))).toMatchObject({ state: 'merged', checks: 'none' })
    expect(linkedPullRequestOf(body({ state: 'CLOSED', mergedAt: null, statusCheckRollup: [run('FAILURE')] }))).toMatchObject({ state: 'closed', checks: 'none', failed: 1 })
    expect(linkedPullRequestOf(body({ state: 'CLOSED', mergedAt: '  ' }))).toMatchObject({ state: 'closed' })
    expect(linkedPullRequestOf('no pull request')).toBeUndefined()
    expect(linkedPullRequestOf('[]')).toBeUndefined()
  })

  it('reads checks as passing, failing, running or none, and says the hover', () => {
    const running = linkedPullRequestOf(body({ statusCheckRollup: [run('SUCCESS'), run('SUCCESS'), run('SUCCESS'), run('', 'IN_PROGRESS')] }))!
    expect(running).toMatchObject({ state: 'open', checks: 'running', passed: 3, failed: 0, running: 1, total: 4 })
    expect(pullLinkHover(running)).toBe('Open · 3 of 4 checks passed · 1 running')

    const failing = linkedPullRequestOf(body({ statusCheckRollup: [run('FAILURE'), run('SUCCESS')] }))!
    expect(failing).toMatchObject({ checks: 'failing', passed: 1, failed: 1, running: 0, total: 2 })
    expect(pullLinkHover(failing)).toBe('Open · 1 of 2 checks passed · 1 failing')

    const both = linkedPullRequestOf(body({ statusCheckRollup: [run('FAILURE'), run('', 'QUEUED')] }))!
    expect(both.checks).toBe('failing')
    expect(pullLinkHover(both)).toBe('Open · 0 of 2 checks passed · 1 failing · 1 running')

    const passing = linkedPullRequestOf(body({ statusCheckRollup: [run('SUCCESS')] }))!
    expect(passing.checks).toBe('passing')
    expect(pullLinkHover(passing)).toBe('Open · 1 of 1 check passed')
    expect(pullLinkHover(linkedPullRequestOf(body({}))!)).toBe('Open')
    expect(pullLinkHover(linkedPullRequestOf(body({ state: 'MERGED' }))!)).toBe('Merged')
    expect(pullLinkHover(linkedPullRequestOf(body({ isDraft: true, statusCheckRollup: [run('FAILURE')] }))!)).toBe('Draft · 0 of 1 check passed · 1 failing')
  })
})

const flush = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await new Promise((resolve) => setTimeout(resolve, 0))
}

describe('reading it', () => {
  it('asks gh for that pull request and nothing else', async () => {
    const seen: string[][] = []
    const reader = createPullLinkReader({
      gh: (args) => {
        seen.push([...args])
        return Promise.resolve(body({}))
      }
    })
    expect(await reader.read(address)).toMatchObject({ state: 'open', checks: 'none' })
    expect(seen).toEqual([['pr', 'view', '42', '--repo', 'acme/app', '--json', LINK_PULL_FIELDS]])
  })

  it('reads once when many links ask together, and keeps the answer for a minute', async () => {
    let calls = 0
    let time = 10_000
    let release: (value: string) => void = () => undefined
    const held = new Promise<string>((resolve) => {
      release = resolve
    })
    const reader = createPullLinkReader({
      now: () => time,
      gh: () => {
        calls += 1
        return held
      }
    })
    const all = Promise.all([reader.read(address), reader.read(address), reader.read(address)])
    await flush()
    expect(calls).toBe(1)
    release(body({ statusCheckRollup: [run('SUCCESS')] }))
    const answers = await all
    expect(answers[0]).toMatchObject({ checks: 'passing' })
    expect(answers[1]).toBe(answers[0])
    expect(await reader.read(address)).toBe(answers[0])
    expect(calls).toBe(1)
    time += 59_000
    expect(await reader.read(address)).toMatchObject({ checks: 'passing' })
    expect(calls).toBe(1)
    time += 2_000
    expect(await reader.read(address)).toMatchObject({ checks: 'passing' })
    expect(calls).toBe(2)
  })

  it('does not ask fifty times at once', async () => {
    let current = 0
    let max = 0
    const reader = createPullLinkReader({
      gh: async () => {
        current += 1
        max = Math.max(max, current)
        await new Promise((resolve) => setTimeout(resolve, 40))
        current -= 1
        return body({})
      }
    })
    await Promise.all(Array.from({ length: 12 }, (_, index) => reader.read({ owner: 'acme', repo: 'app', number: index + 1 })))
    expect(max).toBe(2)
  })

  it('reads nothing into the chip when gh cannot, and does not ask again for a minute', async () => {
    let calls = 0
    const reader = createPullLinkReader({
      now: () => 5_000,
      gh: async () => {
        calls += 1
        throw new Error('spawn gh ENOENT')
      }
    })
    await expect(reader.read(address)).resolves.toBeUndefined()
    await expect(reader.read(address)).resolves.toBeUndefined()
    await expect(reader.read({ ...address, token: 'secret' })).resolves.toBeUndefined()
    expect(calls).toBe(1)
  })
})
