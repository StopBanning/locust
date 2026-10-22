import { describe, expect, it } from 'vitest'

import { createPullLinkReader } from './pull-link.js'
import {
  LINK_ISSUE_FIELDS,
  conversationPieces,
  githubLinkOf,
  issueLinkHover,
  issueLinkRequest,
  issueLinkWord,
  linkedIssueOf
} from '../shared/pull-request.js'

/*
 * AN ISSUE LINK IN A CONVERSATION.
 *
 * A pull request link already wears its state. An issue link now wears its
 * own the same way: Open, Closed completed, or Closed not planned, read with
 * `gh issue view` through the same reader. A pull request and an issue with
 * the same number are different things and never share a read.
 */

const body = (over: Record<string, unknown>) => JSON.stringify({ state: 'OPEN', stateReason: '', title: 'Fix the cart', ...over })
const address = { owner: 'acme', repo: 'app', number: 42 }
const issueBody = (over: Record<string, unknown>) =>
  JSON.stringify({ state: 'OPEN', stateReason: '', title: 'The cart forgets', statusCheckRollup: [], isDraft: false, mergedAt: null, ...over })

describe('which links wear an issue chip', () => {
  it('is an issue, beside the pull request with the same address shape', () => {
    expect(githubLinkOf('https://github.com/acme/app/issues/3')).toEqual({ kind: 'issue', owner: 'acme', repo: 'app', number: 3 })
    expect(githubLinkOf('https://github.com/acme/app/pull/42')).toEqual({ kind: 'pull', owner: 'acme', repo: 'app', number: 42 })
  })

  it('finds a bare issue in a sentence, and a written one, beside the pull request', () => {
    expect(conversationPieces('See https://github.com/acme/app/issues/3 today. Opened https://github.com/acme/app/pull/42.')).toEqual([
      { kind: 'text', text: 'See ' },
      { kind: 'link', text: 'https://github.com/acme/app/issues/3', href: 'https://github.com/acme/app/issues/3', issue: { owner: 'acme', repo: 'app', number: 3 } },
      { kind: 'text', text: ' today. Opened ' },
      { kind: 'link', text: 'https://github.com/acme/app/pull/42', href: 'https://github.com/acme/app/pull/42', pull: { owner: 'acme', repo: 'app', number: 42 } },
      { kind: 'text', text: '.' }
    ])
    expect(conversationPieces('See [the issue](https://github.com/acme/app/issues/3).')).toEqual([
      { kind: 'text', text: 'See ' },
      { kind: 'link', text: 'the issue', href: 'https://github.com/acme/app/issues/3', issue: { owner: 'acme', repo: 'app', number: 3 } },
      { kind: 'text', text: '.' }
    ])
    expect(conversationPieces('Run `https://github.com/acme/app/issues/3` first.')).toEqual([
      { kind: 'text', text: 'Run `https://github.com/acme/app/issues/3` first.' }
    ])
  })
})

describe('the request the window may make for an issue', () => {
  it('refuses anything that is not an owner, a repo and a number', () => {
    expect(issueLinkRequest(address)).toEqual(address)
    expect(issueLinkRequest({ ...address, cwd: 'C:/work' })).toBeUndefined()
    expect(issueLinkRequest({ ...address, args: ['auth', 'token'] })).toBeUndefined()
    expect(issueLinkRequest({ owner: 'acme', repo: 'app', number: '42' })).toBeUndefined()
    expect(issueLinkRequest({ owner: '-acme', repo: 'app', number: 1 })).toBeUndefined()
    expect(issueLinkRequest({ owner: 'acme', repo: 'app', number: 0 })).toBeUndefined()
  })
})

describe('what gh said about an issue', () => {
  it('reads open, closed completed and closed not planned, and says the words and the hover', () => {
    const open = linkedIssueOf(body({}))!
    expect(open).toEqual({ state: 'open', title: 'Fix the cart' })
    expect(issueLinkWord(open.state)).toBe('Open')
    expect(issueLinkHover(open)).toBe('Open · Fix the cart')

    const completed = linkedIssueOf(body({ state: 'CLOSED', stateReason: 'COMPLETED' }))!
    expect(completed).toEqual({ state: 'closed-completed', title: 'Fix the cart' })
    expect(issueLinkWord(completed.state)).toBe('Closed')
    expect(issueLinkHover(completed)).toBe('Closed as completed · Fix the cart')

    const notPlanned = linkedIssueOf(body({ state: 'CLOSED', stateReason: 'NOT_PLANNED' }))!
    expect(notPlanned).toEqual({ state: 'closed-not-planned', title: 'Fix the cart' })
    expect(issueLinkWord(notPlanned.state)).toBe('Not planned')
    expect(issueLinkHover(notPlanned)).toBe('Closed as not planned · Fix the cart')

    expect(linkedIssueOf(body({ state: 'CLOSED', stateReason: '' }))).toMatchObject({ state: 'closed-completed' })
    expect(issueLinkHover(linkedIssueOf(body({ title: '  ' }))!)).toBe('Open')
    expect(linkedIssueOf('no issue')).toBeUndefined()
    expect(linkedIssueOf('[]')).toBeUndefined()
    expect(linkedIssueOf(body({ state: 'MERGED' }))).toBeUndefined()
  })
})

const flush = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await new Promise((resolve) => setTimeout(resolve, 0))
}

describe('reading an issue', () => {
  it('asks gh for that issue and nothing else', async () => {
    const seen: string[][] = []
    const reader = createPullLinkReader({
      gh: (args) => {
        seen.push([...args])
        return Promise.resolve(body({}))
      }
    })
    expect(await reader.readIssue(address)).toEqual({ state: 'open', title: 'Fix the cart' })
    expect(seen).toEqual([['issue', 'view', '42', '--repo', 'acme/app', '--json', LINK_ISSUE_FIELDS]])
  })

  it('keeps a pull request and an issue with one number apart', async () => {
    const seen: string[][] = []
    const reader = createPullLinkReader({
      gh: (args) => {
        seen.push([...args])
        return Promise.resolve(args[0] === 'pr' ? issueBody({}) : body({ state: 'CLOSED', stateReason: 'NOT_PLANNED', title: 'The cart forgets' }))
      }
    })
    expect(await reader.read(address)).toMatchObject({ state: 'open' })
    expect(await reader.readIssue(address)).toEqual({ state: 'closed-not-planned', title: 'The cart forgets' })
    // Each is kept on its own: asking again asks gh no more.
    expect(await reader.read(address)).toMatchObject({ state: 'open' })
    expect(await reader.readIssue(address)).toEqual({ state: 'closed-not-planned', title: 'The cart forgets' })
    expect(seen).toEqual([
      ['pr', 'view', '42', '--repo', 'acme/app', '--json', 'state,isDraft,mergedAt,statusCheckRollup,title'],
      ['issue', 'view', '42', '--repo', 'acme/app', '--json', LINK_ISSUE_FIELDS]
    ])
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
    const all = Promise.all([reader.readIssue(address), reader.readIssue(address), reader.readIssue(address)])
    await flush()
    expect(calls).toBe(1)
    release(body({ state: 'CLOSED', stateReason: 'COMPLETED' }))
    const answers = await all
    expect(answers[0]).toEqual({ state: 'closed-completed', title: 'Fix the cart' })
    expect(answers[1]).toBe(answers[0])
    expect(await reader.readIssue(address)).toBe(answers[0])
    expect(calls).toBe(1)
    time += 61_000
    expect(await reader.readIssue(address)).toEqual({ state: 'closed-completed', title: 'Fix the cart' })
    expect(calls).toBe(2)
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
    await expect(reader.readIssue(address)).resolves.toBeUndefined()
    await expect(reader.readIssue(address)).resolves.toBeUndefined()
    await expect(reader.readIssue({ ...address, token: 'secret' })).resolves.toBeUndefined()
    expect(calls).toBe(1)
  })
})
