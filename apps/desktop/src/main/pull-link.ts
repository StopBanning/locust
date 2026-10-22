import { LINK_ISSUE_FIELDS, LINK_PULL_FIELDS, issueLinkRequest, linkedIssueOf, linkedPullRequestOf, pullLinkRequest } from '../shared/pull-request.js'
import type { LinkedIssue, LinkedPullRequest, PullLinkAddress } from '../shared/pull-request.js'

/**
 * Read a linked pull request or issue with the person's own gh.
 *
 * Many links to one pull request or issue share one read. A conversation
 * full of different ones runs a few gh processes at a time, not one each. A
 * missing gh and a repository gh cannot see are the same answer: nothing,
 * and nothing said. The error is not kept — gh can print a token into it.
 * A pull request and an issue with the same number are separate kinds and
 * never share a cache entry.
 */
const MINUTE_MS = 60_000
const AT_ONCE = 2

type AnyLinked = LinkedPullRequest | LinkedIssue

export function createPullLinkReader(options: {
  readonly gh: (args: readonly string[]) => Promise<string>
  readonly now?: () => number
  readonly cacheForMs?: number
  readonly atOnce?: number
}): { read(request: unknown): Promise<LinkedPullRequest | undefined>; readIssue(request: unknown): Promise<LinkedIssue | undefined> } {
  const now = options.now ?? (() => Date.now())
  const cacheForMs = options.cacheForMs ?? MINUTE_MS
  const atOnce = options.atOnce ?? AT_ONCE
  const cache = new Map<string, { at: number; value: AnyLinked | undefined }>()
  const flying = new Map<string, Promise<AnyLinked | undefined>>()
  let running = 0
  const waiting: (() => void)[] = []

  const take = (): Promise<void> => {
    if (running < atOnce) {
      running += 1
      return Promise.resolve()
    }
    return new Promise((resolve) => {
      waiting.push(resolve)
    })
  }
  // The next waiter takes this slot; running stays where it is.
  const release = (): void => {
    const next = waiting.shift()
    if (next !== undefined) next()
    else running -= 1
  }

  const keyOf = (kind: 'pull' | 'issue', address: PullLinkAddress): string => `${kind}:${address.owner}/${address.repo}#${String(address.number)}`

  const ask = async (kind: 'pull' | 'issue', address: PullLinkAddress): Promise<AnyLinked | undefined> => {
    await take()
    try {
      const json =
        kind === 'pull'
          ? await options.gh(['pr', 'view', String(address.number), '--repo', `${address.owner}/${address.repo}`, '--json', LINK_PULL_FIELDS])
          : await options.gh(['issue', 'view', String(address.number), '--repo', `${address.owner}/${address.repo}`, '--json', LINK_ISSUE_FIELDS])
      return kind === 'pull' ? linkedPullRequestOf(json) : linkedIssueOf(json)
    } catch {
      return undefined
    } finally {
      release()
    }
  }

  const readKind = (kind: 'pull' | 'issue', request: unknown): Promise<AnyLinked | undefined> => {
    const address = kind === 'pull' ? pullLinkRequest(request) : issueLinkRequest(request)
    if (address === undefined) return Promise.resolve(undefined)
    const key = keyOf(kind, address)
    const kept = cache.get(key)
    if (kept !== undefined && now() - kept.at < cacheForMs) return Promise.resolve(kept.value)
    const existing = flying.get(key)
    if (existing !== undefined) return existing
    const job = ask(kind, address).then((value) => {
      cache.set(key, { at: now(), value })
      return value
    })
    flying.set(key, job)
    void job.finally(() => {
      if (flying.get(key) === job) flying.delete(key)
    })
    return job
  }

  return {
    read: (request) => readKind('pull', request) as Promise<LinkedPullRequest | undefined>,
    readIssue: (request) => readKind('issue', request) as Promise<LinkedIssue | undefined>
  }
}
