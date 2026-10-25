import { PULL_REQUEST_FIELDS, pullRequestsOf } from '../shared/pull-request.js'
import { branchRunsOf, issuesOf, reviewsAskedOf } from '../shared/scout.js'
import type { ScoutGithub } from './scout.js'

/** The workflow Scout's away look runs as, in the watched repository (0.829). */
export const AWAY_WORKFLOW_NAME = 'Locust Scout'

/**
 * WHAT SCOUT READS FROM GITHUB (0.756), with the person's own GitHub CLI in the folder -- the one Commit and the
 * pull request chip already use, so it reads only what that sign-in can: their open pull requests and how their
 * checks stand, the pull requests that ask them for a review, and how the default branch's newest checks ended.
 * A folder that is not on GitHub, or a CLI that is missing or signed out, is no news rather than an error.
 *
 * `who` is the person: their own sign-in, or (Scout's away look, 0.829) their login, read with a workflow's token.
 */
/*
 * WHY GITHUB WAS NOT READ (0.835). Every look in Colin's log said "GitHub could not be read", for days: his folder was
 * `.claude`, which is no repository at all, while his GitHub CLI was signed in and well. A folder that is not on
 * GitHub has nothing there to watch, which is not a failure; a signed-out or missing CLI is one, with a way to fix it.
 */
export type ScoutGithubUnread = 'not-on-github' | 'signed-out' | 'no-cli' | 'unreadable'

/** What the GitHub CLI's failure says: its own words come back in the error, after the command. */
export function githubUnreadWhy(error: unknown): ScoutGithubUnread {
  const code = typeof error === 'object' && error !== null ? (error as { code?: unknown }).code : undefined
  if (code === 'ENOENT') return 'no-cli'
  const said = error instanceof Error ? error.message : String(error)
  if (/not a git repository|no git remotes|none of the git remotes|not a github repository|could not determine (?:the )?(?:base )?repo/i.test(said)) return 'not-on-github'
  if (/gh auth login|not logged in|authentication (?:required|failed)|HTTP 401|bad credentials/i.test(said)) return 'signed-out'
  if (/is not recognized as an internal or external command|command not found/i.test(said)) return 'no-cli'
  return 'unreadable'
}

export async function readScoutGithub(folder: string, gh: (args: readonly string[], cwd: string) => Promise<string>, who = '@me', unread?: (why: ScoutGithubUnread) => void): Promise<ScoutGithub | undefined> {
  // And when each last moved, for "ready and waiting" (0.758): the chip's own fields stay as they are.
  const mine = await gh(['pr', 'list', '--author', who, '--state', 'open', '--limit', '30', '--json', `${PULL_REQUEST_FIELDS},updatedAt`], folder).catch((error: unknown) => { unread?.(githubUnreadWhy(error)); return undefined })
  // The first answer decides: no answer, and the rest are not asked.
  if (mine === undefined) return undefined
  const reviews = await gh(['pr', 'list', '--search', `review-requested:${who}`, '--state', 'open', '--limit', '20', '--json', 'number,title,url,author'], folder).catch(() => '[]')
  const branch = (await gh(['repo', 'view', '--json', 'defaultBranchRef', '--jq', '.defaultBranchRef.name'], folder).catch(() => '')).trim()
  // Thirty, not ten (0.829): Scout's own away look runs every hour or two on the branch, and is not a check of it.
  const runs = /^[\w./-]{1,200}$/.test(branch)
    ? await gh(['run', 'list', '--branch', branch, '--limit', '30', '--json', 'databaseId,conclusion,status,workflowName,url'], folder).catch(() => '[]')
    : undefined
  // The issues people open on it (0.758): for a project with users, the first word of a bug.
  const issues = await gh(['issue', 'list', '--state', 'open', '--limit', '20', '--json', 'number,title,url,createdAt,author'], folder).catch(() => '[]')
  return {
    prs: pullRequestsOf(mine),
    reviews: reviewsAskedOf(reviews),
    ...(runs === undefined ? {} : { branch: { name: branch, runs: branchRunsOf(runs).filter((run) => run.workflow !== AWAY_WORKFLOW_NAME) } }),
    issues: issuesOf(issues)
  }
}
