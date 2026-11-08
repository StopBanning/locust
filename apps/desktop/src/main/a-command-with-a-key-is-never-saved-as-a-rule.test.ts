import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { ruleCandidateOf } from '../shared/approval-rules.js'
import { createApprovalRuleStore } from './approval-rule-store.js'

/**
 * A COMMAND WITH A KEY IS NEVER SAVED AS A RULE (0.870).
 *
 * A saved rule is the command as written, kept for good in approval-rules.json and in the sealed backup. On
 * 2026-11-08 HN's front page carried "Claude Code's 'Don't Ask Again' exposes passwords in 1,122 public GitHub
 * files": allow rules saved with the secret inside the command. Locust keeps its rules in its own data folder, never
 * the project, but a key in a rule is still a key kept. So the card offers no rule for such a command, and the store
 * refuses one however it arrives.
 */
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })
// Built from pieces so this file holds no key-shaped literal.
const KEY = ['sk', 'ant', 'api03', 'Zq9xR2mT7vLp4WcN8bYd3KfH6sJ1gQe5'].join('-')
const context = { teammateId: 'tm_codec', folder: '/work/app' }

describe('a command that carries a key', () => {
  it('is offered no "always allow" rule on its card', () => {
    expect(ruleCandidateOf({ kind: 'command', command: `curl -H "x-api-key: ${KEY}" https://api.example.com` }, context)).toBeUndefined()
    expect(ruleCandidateOf({ kind: 'command', command: `ANTHROPIC_API_KEY=${KEY} npm test` }, context)).toBeUndefined()
  })

  it('still offers one for an ordinary command (the control)', () => {
    expect(ruleCandidateOf({ kind: 'command', command: 'npm test' }, context)).toMatchObject({ effect: 'allow', kind: 'command', pattern: 'npm test' })
  })

  it('is refused by the store, in words, and nothing is written', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-key-rule-'))
    roots.push(root)
    const store = createApprovalRuleStore({ rootDirectory: root })
    await expect(store.add({ effect: 'allow', kind: 'command', pattern: `curl -H "Authorization: Bearer ${KEY}" https://x.test` }))
      .rejects.toThrow(/holds .*so it isn't saved as a rule.*Allow it once instead\./)
    expect(await store.list()).toEqual([])
    await store.add({ effect: 'allow', kind: 'command', pattern: 'npm test' })
    const saved = await readFile(join(root, 'approval-rules.json'), 'utf8')
    expect(saved).toContain('npm test')
    expect(saved).not.toContain(KEY)
  })
})
