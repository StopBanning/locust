/**
 * THE FOLDER'S PULL REQUEST, IN THE CONVERSATION'S HEADER (0.720).
 *
 * t3code shows a thread's pull request -- open, merged, its checks -- where
 * the work is. Locust could open one (Commit and open a pull request) and
 * then said nothing more about it. Now, while the folder's branch has a pull
 * request on GitHub, the header says which, whether it is open, draft,
 * merged or closed, and how its checks stand; pressing it opens it.
 *
 * Read with the person's own `gh pr view --json` for the branch the folder is
 * on, the same GitHub CLI Commit uses. The shapes are gh 2.96.0's, MEASURED
 * 2026-10-10 on a public repository's pull requests: `statusCheckRollup` holds
 * CheckRuns (status, conclusion) and StatusContexts (state).
 */

export const FOLDER_PULL_REQUEST_CHANNEL = 'folder:pull-request'

/** The fields asked of gh, in its own names. */
export const PULL_REQUEST_FIELDS = 'number,title,url,state,isDraft,statusCheckRollup,reviewDecision,mergeable'

export type PullRequestState = 'open' | 'draft' | 'merged' | 'closed'
export type PullRequestChecks = 'passing' | 'failing' | 'pending' | 'none'

export interface FolderPullRequest {
  readonly number: number
  readonly title: string
  readonly url: string
  readonly state: PullRequestState
  readonly checks: PullRequestChecks
  /** How many checks there are, and how many have not passed (failed, or not yet finished). */
  readonly checkCount: number
  readonly notPassed: number
  readonly review?: 'approved' | 'changes-requested' | 'review-required'
  /** GitHub says it cannot merge into its base as it stands (`mergeable: CONFLICTING`, 0.731). */
  readonly conflicts?: true
  /** When it last moved, when it was asked for (`updatedAt`): Scout's "ready and waiting" (0.758). */
  readonly updatedAt?: string
}

const FAILED = new Set(['FAILURE', 'TIMED_OUT', 'CANCELLED', 'ACTION_REQUIRED', 'STARTUP_FAILURE', 'ERROR'])
const PASSED = new Set(['SUCCESS', 'NEUTRAL', 'SKIPPED'])

/** One check, as passed, failed or still going. */
function checkOf(check: Record<string, unknown>): 'passed' | 'failed' | 'pending' {
  if (check.__typename === 'StatusContext') {
    const state = String(check.state ?? '')
    return PASSED.has(state) ? 'passed' : FAILED.has(state) ? 'failed' : 'pending'
  }
  if (check.status !== 'COMPLETED') return 'pending'
  const conclusion = String(check.conclusion ?? '')
  return PASSED.has(conclusion) ? 'passed' : FAILED.has(conclusion) ? 'failed' : 'pending'
}

/** `gh pr view --json <PULL_REQUEST_FIELDS>` -> the pull request, or undefined when it is not one. */
export function pullRequestOf(json: string): FolderPullRequest | undefined {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return undefined
  }
  return pullRequestFrom(parsed)
}

/** `gh pr list --json <PULL_REQUEST_FIELDS>` -> each pull request in it that reads as one (0.756, for Scout). */
export function pullRequestsOf(json: string): readonly FolderPullRequest[] {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return []
  }
  return Array.isArray(parsed) ? parsed.map(pullRequestFrom).filter((one): one is FolderPullRequest => one !== undefined) : []
}

function pullRequestFrom(parsed: unknown): FolderPullRequest | undefined {
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined
  const pr = parsed as Record<string, unknown>
  const number = typeof pr.number === 'number' && Number.isInteger(pr.number) && pr.number > 0 ? pr.number : undefined
  const url = typeof pr.url === 'string' && /^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+$/.test(pr.url) ? pr.url : undefined
  if (number === undefined || url === undefined) return undefined
  const raw = String(pr.state ?? '').toUpperCase()
  const state: PullRequestState = raw === 'MERGED' ? 'merged' : raw === 'CLOSED' ? 'closed' : pr.isDraft === true ? 'draft' : 'open'
  const checks = (Array.isArray(pr.statusCheckRollup) ? pr.statusCheckRollup : []).filter(
    (one): one is Record<string, unknown> => typeof one === 'object' && one !== null
  )
  const read = checks.map(checkOf)
  const failed = read.filter((one) => one === 'failed').length
  const pending = read.filter((one) => one === 'pending').length
  const review =
    pr.reviewDecision === 'APPROVED' ? 'approved' : pr.reviewDecision === 'CHANGES_REQUESTED' ? 'changes-requested' : pr.reviewDecision === 'REVIEW_REQUIRED' ? 'review-required' : undefined
  return {
    number,
    title: typeof pr.title === 'string' ? pr.title.replace(/\s+/g, ' ').trim().slice(0, 200) : '',
    url,
    state,
    checks: read.length === 0 ? 'none' : failed > 0 ? 'failing' : pending > 0 ? 'pending' : 'passing',
    checkCount: read.length,
    notPassed: failed + pending,
    ...(review === undefined ? {} : { review }),
    ...(pr.mergeable === 'CONFLICTING' ? { conflicts: true as const } : {}),
    ...(typeof pr.updatedAt === 'string' && Number.isFinite(Date.parse(pr.updatedAt)) ? { updatedAt: pr.updatedAt } : {})
  }
}

const STATE_WORD: Record<PullRequestState, string> = { open: 'Open', draft: 'Draft', merged: 'Merged', closed: 'Closed' }

/** "Open · checks passing", "Merged", "Draft · 2 of 9 checks failing". */
export function pullRequestLine(pr: FolderPullRequest): string {
  const parts = [STATE_WORD[pr.state]]
  // A merged or closed one's checks are history.
  if (pr.state === 'open' || pr.state === 'draft') {
    if (pr.checks === 'passing') parts.push(pr.checkCount === 1 ? 'its check passed' : `all ${String(pr.checkCount)} checks passed`)
    else if (pr.checks === 'failing') parts.push(`${String(pr.notPassed)} of ${String(pr.checkCount)} checks not passed`)
    else if (pr.checks === 'pending') parts.push('checks running')
    if (pr.review === 'approved') parts.push('approved')
    else if (pr.review === 'changes-requested') parts.push('changes requested')
  }
  return parts.join(' · ')
}

/**
 * A WATCHED PULL REQUEST, AND WHAT IS NEWS ABOUT IT (0.731). The plan's "a teammate that watches a PR": it checks
 * the PR every few minutes and wakes when a check fails, someone reviews, or the branch conflicts. Locust watches
 * the folder's pull request while the person has asked it to, and says what changed -- in the conversation, with
 * a press that hands it to the teammate, and as a notification while the window is elsewhere. It never starts a
 * run on its own: what a fix costs stays the person's call.
 */
export const PULL_REQUEST_WATCH_CHANNEL = 'folder:pull-request-watch'
/** main -> renderer: news about the watched pull request. */
export const PULL_REQUEST_NEWS_CHANNEL = 'folder:pull-request-news'

export interface PullRequestNews {
  readonly number: number
  readonly url: string
  /** "2 of 9 checks failed", "changes were requested", joined. */
  readonly said: string
  /** Something a teammate could fix: a failed check, requested changes, a conflict. */
  readonly needsWork: boolean
}

/** What changed between two looks at one pull request that a person would want to hear; undefined for nothing. */
export function pullRequestNews(before: FolderPullRequest, now: FolderPullRequest): PullRequestNews | undefined {
  if (before.number !== now.number) return undefined
  const said: string[] = []
  let needsWork = false
  if (before.state !== now.state && (now.state === 'merged' || now.state === 'closed')) {
    said.push(now.state === 'merged' ? 'it was merged' : 'it was closed')
  } else {
    if (now.checks === 'failing' && before.checks !== 'failing') {
      said.push(`${String(now.notPassed)} of ${String(now.checkCount)} checks did not pass`)
      needsWork = true
    } else if (now.checks === 'passing' && before.checks !== 'passing' && before.checks !== 'none') {
      said.push(now.checkCount === 1 ? 'its check passed' : `all ${String(now.checkCount)} checks passed`)
    }
    if (now.review !== before.review && now.review === 'changes-requested') {
      said.push('changes were requested')
      needsWork = true
    } else if (now.review !== before.review && now.review === 'approved') {
      said.push('it was approved')
    }
    if (now.conflicts === true && before.conflicts !== true) {
      said.push('it conflicts with its base branch')
      needsWork = true
    }
  }
  if (said.length === 0) return undefined
  const sentence = said.join(', and ')
  return { number: now.number, url: now.url, said: sentence.charAt(0).toUpperCase() + sentence.slice(1), needsWork }
}

/**
 * A PULL REQUEST LINKED IN A CONVERSATION.
 *
 * The header reads the folder's own pull request. A link in what was said is
 * any owner and repo. The window asks for those three parts and nothing else;
 * the host runs `gh pr view` and answers with the state. An issue link sits
 * beside it the same way, read with `gh issue view` on its own channel, so a
 * pull request and an issue with the same number never share a read.
 */
export const GITHUB_PULL_LINK_CHANNEL = 'github:pull-link'
export const GITHUB_ISSUE_LINK_CHANNEL = 'github:issue-link'

/** What `gh pr view <n> --repo <owner>/<repo> --json` is asked for. */
export const LINK_PULL_FIELDS = 'state,isDraft,mergedAt,statusCheckRollup,title'

/** What `gh issue view <n> --repo <owner>/<repo> --json` is asked for. */
export const LINK_ISSUE_FIELDS = 'state,stateReason,title'

const OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/
const REPO = /^(?!\.\.?$)[A-Za-z0-9._-]{1,100}$/
const NUMBER = /^[1-9]\d{0,9}$/
const MAX_PULL = 1_000_000_000
const REQUEST_KEYS = new Set(['owner', 'repo', 'number'])

export interface PullLinkAddress {
  readonly owner: string
  readonly repo: string
  readonly number: number
}

/** A GitHub link a conversation can wear: a pull request or an issue. */
export type GitHubConversationLink =
  | ({ readonly kind: 'pull' } & PullLinkAddress)
  | ({ readonly kind: 'issue' } & PullLinkAddress)

export type PullLinkChecks = 'passing' | 'failing' | 'running' | 'none'

export interface LinkedPullRequest {
  readonly state: PullRequestState
  readonly checks: PullLinkChecks
  readonly passed: number
  readonly failed: number
  readonly running: number
  readonly total: number
}

/** An issue's standing: open, or closed and why (`stateReason`). */
export type IssueLinkState = 'open' | 'closed-completed' | 'closed-not-planned'

export interface LinkedIssue {
  readonly state: IssueLinkState
  readonly title: string
}

export type ConversationPiece =
  | { readonly kind: 'text'; readonly text: string }
  | { readonly kind: 'link'; readonly text: string; readonly href: string; readonly pull?: PullLinkAddress; readonly issue?: PullLinkAddress }

function addressOf(owner: string, repo: string, raw: string): PullLinkAddress | undefined {
  if (!OWNER.test(owner) || !REPO.test(repo) || !NUMBER.test(raw)) return undefined
  const number = Number(raw)
  if (!Number.isInteger(number) || number < 1 || number > MAX_PULL) return undefined
  return { owner, repo, number }
}

/** `https://github.com/<owner>/<repo>/pull/<n>`, or the same shape for an issue. Anything else is not one. */
export function githubLinkOf(url: string): GitHubConversationLink | undefined {
  if (url.length > 2048) return undefined
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    return undefined
  }
  if (parsed.protocol !== 'https:' || parsed.username !== '' || parsed.password !== '') return undefined
  if (parsed.hostname.toLowerCase().replace(/^www\./, '') !== 'github.com') return undefined
  const parts = parsed.pathname.split('/').filter((part) => part.length > 0)
  if (parts.length !== 4) return undefined
  const owner = parts[0] ?? ''
  const repo = parts[1] ?? ''
  const kind = parts[2]
  const address = addressOf(owner, repo, parts[3] ?? '')
  if (address === undefined) return undefined
  if (kind === 'pull') return { kind: 'pull', ...address }
  if (kind === 'issues') return { kind: 'issue', ...address }
  return undefined
}

/** The only request the window may make: owner, repo, and a positive number. Anything else is refused. */
export function pullLinkRequest(value: unknown): PullLinkAddress | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  for (const key of Object.keys(record)) if (!REQUEST_KEYS.has(key)) return undefined
  if (typeof record.owner !== 'string' || typeof record.repo !== 'string' || typeof record.number !== 'number') return undefined
  if (!Number.isInteger(record.number) || record.number < 1 || record.number > MAX_PULL) return undefined
  if (!OWNER.test(record.owner) || !REPO.test(record.repo)) return undefined
  return { owner: record.owner, repo: record.repo, number: record.number }
}

/** The only request the window may make for an issue: the same three parts, on its own channel. */
export function issueLinkRequest(value: unknown): PullLinkAddress | undefined {
  return pullLinkRequest(value)
}

function countChecks(rollup: unknown): { readonly passed: number; readonly failed: number; readonly running: number } {
  const checks = (Array.isArray(rollup) ? rollup : []).filter(
    (one): one is Record<string, unknown> => typeof one === 'object' && one !== null && !Array.isArray(one)
  )
  let passed = 0
  let failed = 0
  let running = 0
  for (const check of checks) {
    const standing = checkOf(check)
    if (standing === 'passed') passed += 1
    else if (standing === 'failed') failed += 1
    else running += 1
  }
  return { passed, failed, running }
}

/**
 * `gh pr view --json state,isDraft,mergedAt,statusCheckRollup,title`.
 * The number and the address came from the link, not from this JSON.
 * A closed pull request with `mergedAt` set is merged, which is GitHub's own meaning.
 * Checks that have not finished are "running" here; the header's word for the same fact is "pending".
 */
export function linkedPullRequestOf(json: string): LinkedPullRequest | undefined {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return undefined
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined
  const pr = parsed as Record<string, unknown>
  const raw = typeof pr.state === 'string' ? pr.state.toUpperCase() : ''
  const merged = typeof pr.mergedAt === 'string' && pr.mergedAt.trim().length > 0
  const state: PullRequestState | undefined =
    raw === 'MERGED' ? 'merged' : raw === 'CLOSED' ? (merged ? 'merged' : 'closed') : raw === 'OPEN' ? (pr.isDraft === true ? 'draft' : 'open') : undefined
  if (state === undefined) return undefined
  const tally = countChecks(pr.statusCheckRollup)
  const total = tally.passed + tally.failed + tally.running
  const settled = state === 'merged' || state === 'closed'
  const checks: PullLinkChecks = settled || total === 0 ? 'none' : tally.failed > 0 ? 'failing' : tally.running > 0 ? 'running' : 'passing'
  return { state, checks, ...tally, total }
}

/**
 * `gh issue view --json state,stateReason,title` (measured 2026-10-22 on a
 * public repository: an open issue says `stateReason: ""`).
 * A closed issue is completed unless GitHub says it was not planned.
 */
export function linkedIssueOf(json: string): LinkedIssue | undefined {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return undefined
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined
  const issue = parsed as Record<string, unknown>
  const raw = typeof issue.state === 'string' ? issue.state.toUpperCase() : ''
  const reason = typeof issue.stateReason === 'string' ? issue.stateReason.toUpperCase() : ''
  const state: IssueLinkState | undefined =
    raw === 'OPEN' ? 'open' : raw === 'CLOSED' ? (reason === 'NOT_PLANNED' ? 'closed-not-planned' : 'closed-completed') : undefined
  if (state === undefined) return undefined
  const title = typeof issue.title === 'string' ? issue.title.replace(/\s+/g, ' ').trim().slice(0, 200) : ''
  return { state, title }
}

/** GitHub's own words: its badge says Closed for a completed issue, and its reason tells a not-planned one apart. */
const ISSUE_WORD: Record<IssueLinkState, string> = { open: 'Open', 'closed-completed': 'Closed', 'closed-not-planned': 'Not planned' }
const ISSUE_STANDING: Record<IssueLinkState, string> = { open: 'Open', 'closed-completed': 'Closed as completed', 'closed-not-planned': 'Closed as not planned' }

export function issueLinkWord(state: IssueLinkState): string {
  return ISSUE_WORD[state]
}

/** "Closed as not planned · The cart forgets": its standing first, as the pull request chip's hover, then its title. */
export function issueLinkHover(link: LinkedIssue): string {
  return link.title.length > 0 ? `${ISSUE_STANDING[link.state]} · ${link.title}` : ISSUE_STANDING[link.state]
}

export function pullLinkWord(state: PullRequestState): string {
  return STATE_WORD[state]
}

/** "Open · 3 of 4 checks passed · 1 running". A merged or closed one's checks are history. */
export function pullLinkHover(link: LinkedPullRequest): string {
  const word = pullLinkWord(link.state)
  if (link.state === 'merged' || link.state === 'closed' || link.total === 0) return word
  const name = link.total === 1 ? 'check' : 'checks'
  const parts = [word, `${String(link.passed)} of ${String(link.total)} ${name} passed`]
  if (link.failed > 0) parts.push(`${String(link.failed)} failing`)
  if (link.running > 0) parts.push(`${String(link.running)} running`)
  return parts.join(' · ')
}

function webHref(url: string): boolean {
  if (url.length > 2048) return false
  try {
    const parsed = new URL(url)
    return (parsed.protocol === 'https:' || parsed.protocol === 'http:') && parsed.hostname.length > 0 && parsed.username === ''
  } catch {
    return false
  }
}

/**
 * Plain text split into words and web links.
 * A bare `https://github.com/.../pull/n` or `.../issues/n` becomes a link and
 * wears its chip. A commit or a file does not, unless it was already written
 * as a Markdown link, and then it stays a link with no chip on it. A URL
 * inside `code` stays code.
 */
export function conversationPieces(text: string): readonly ConversationPiece[] {
  const scan = text.replace(/`[^`\n]*`/g, (span) => ' '.repeat(span.length))
  const hits: { at: number; length: number; piece: ConversationPiece }[] = []
  for (const match of scan.matchAll(/\[([^\]\n]*)\]\((https?:\/\/[^)\s]+)\)/g)) {
    const href = match[2] ?? ''
    if (!webHref(href)) continue
    const found = githubLinkOf(href)
    const label = match[1] ?? ''
    hits.push({
      at: match.index ?? 0,
      length: match[0].length,
      piece: {
        kind: 'link',
        text: label.length > 0 ? label : href,
        href,
        ...(found?.kind === 'pull' ? { pull: { owner: found.owner, repo: found.repo, number: found.number } } : {}),
        ...(found?.kind === 'issue' ? { issue: { owner: found.owner, repo: found.repo, number: found.number } } : {})
      }
    })
  }
  for (const match of scan.matchAll(/https:\/\/github\.com\/[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?\/[A-Za-z0-9._-]{1,100}\/(?:pull|issues)\/[1-9]\d{0,9}\/?(?![A-Za-z0-9/])/g)) {
    const at = match.index ?? 0
    if (hits.some((hit) => at >= hit.at && at < hit.at + hit.length)) continue
    const href = match[0]
    const found = githubLinkOf(href)
    if (found === undefined) continue
    hits.push({
      at,
      length: href.length,
      piece:
        found.kind === 'pull'
          ? { kind: 'link', text: href, href, pull: { owner: found.owner, repo: found.repo, number: found.number } }
          : { kind: 'link', text: href, href, issue: { owner: found.owner, repo: found.repo, number: found.number } }
    })
  }
  hits.sort((a, b) => a.at - b.at)
  if (hits.length === 0) return [{ kind: 'text', text }]
  const pieces: ConversationPiece[] = []
  let cursor = 0
  for (const hit of hits) {
    if (hit.at < cursor) continue
    if (hit.at > cursor) pieces.push({ kind: 'text', text: text.slice(cursor, hit.at) })
    pieces.push(hit.piece)
    cursor = hit.at + hit.length
  }
  if (cursor < text.length) pieces.push({ kind: 'text', text: text.slice(cursor) })
  return pieces
}
