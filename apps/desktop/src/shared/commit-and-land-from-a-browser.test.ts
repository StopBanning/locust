import { describe, expect, it } from 'vitest'

import { COMMIT_DRAFT_CHANNEL } from './commit-draft.js'
import { deskAccess, WINDOW_ONLY } from './desk-calls.js'
import { FOLDER_COMMIT_CHANNEL } from './folder-commit.js'
import { WORKTREE_LAND_CHANNEL, WORKTREE_REMOVE_CHANNEL, WORKTREE_RESOLVE_CHANNEL } from './ipc.js'

/**
 * COMMIT AND LAND FROM A BROWSER (0.835, desk-calls.ts). Locust in a browser drafts and makes a commit and lands a
 * teammate's branch as the window does; what puts the work on GitHub, or throws a branch away for good, stays on
 * the computer.
 */
describe('commit and land from a browser', () => {
  it('drafts and makes a commit, and lands a teammate’s branch or begins its merge', () => {
    expect(deskAccess(FOLDER_COMMIT_CHANNEL, ['Fix the header', 'commit', ['a.ts']])).toBe('act')
    for (const channel of [COMMIT_DRAFT_CHANNEL, WORKTREE_LAND_CHANNEL, WORKTREE_RESOLVE_CHANNEL]) {
      expect(deskAccess(channel, ['tm_one']), channel).toBe('act')
      expect(WINDOW_ONLY, channel).not.toContain(channel)
    }
  })

  it('never pushes or opens a pull request, and never throws a branch away', () => {
    expect(deskAccess(FOLDER_COMMIT_CHANNEL, ['Fix the header', 'push', ['a.ts']])).toBeUndefined()
    expect(deskAccess(FOLDER_COMMIT_CHANNEL, ['Fix the header', 'pull-request', ['a.ts']])).toBeUndefined()
    expect(deskAccess(FOLDER_COMMIT_CHANNEL, [])).toBeUndefined()
    expect(deskAccess(WORKTREE_REMOVE_CHANNEL, ['tm_one'])).toBeUndefined()
    expect(WINDOW_ONLY).toContain(WORKTREE_REMOVE_CHANNEL)
  })
})
