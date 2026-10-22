import { describe, expect, it } from 'vitest'
import type { ExecutableLaunch } from '@teammate/runtime-adapters'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { openCode2Launch, OPENCODE_2_MODE } from './opencode-2-launch.js'

const executable: ExecutableLaunch = { commandName: 'opencode', discoveredPath: '/bin/opencode', executablePath: '/bin/opencode', prefixArgs: [], kind: 'native' }
const guardPlugin = pathToFileURL(join(tmpdir(), 'locust-guard')).href
const config = (sandbox: 'read-only' | 'workspace-write' | 'full-access', approveEach = false, more = {}) => JSON.parse(openCode2Launch(executable, { workspacePath: '/project', sandbox, approveEach, guardPlugin, ...more }).env.OPENCODE_CONFIG_CONTENT!)

describe('an OpenCode 2 mode has native permission rules', () => {
  it('uses an owned ACP mode and a directory plugin, without unsupported V1 flags', () => {
    const found = config('read-only')
    expect(found.default_agent).toBe(OPENCODE_2_MODE)
    expect(found.agents[OPENCODE_2_MODE].mode).toBe('primary')
    expect(found.plugins).toEqual([guardPlugin])
    expect(found.permission).toBeUndefined()
    expect(found.plugin).toBeUndefined()
    expect(found.experimental).toBeUndefined()
    expect(found.policies).toEqual([
      { action: 'integration.use', resource: 'plugin:*', effect: 'deny' },
      { action: 'integration.use', resource: `plugin:${fileURLToPath(guardPlugin)}`, effect: 'allow' }
    ])
    const readOnly = openCode2Launch(executable, { workspacePath: '/project', sandbox: 'read-only', approveEach: false, guardPlugin })
    expect(readOnly.env).toMatchObject({ OPENCODE_CONFIG_PROJECT_DISABLE: '1', OPENCODE_DISABLE_CLAUDE_CODE: '1', OPENCODE_CONFIG_DIR: join(fileURLToPath(guardPlugin), 'config') })
    expect(openCode2Launch(executable, { workspacePath: '/project', sandbox: 'workspace-write', approveEach: false, guardPlugin }).env.OPENCODE_CONFIG_PROJECT_DISABLE).toBeUndefined()
  })

  it('denies edits and subagents in Ask, and lets no shell command through', () => {
    const rules = config('read-only').agents.locust.permissions
    expect(rules).toContainEqual({ action: 'edit', resource: '*', effect: 'deny' })
    expect(rules).toContainEqual({ action: 'subagent', resource: '*', effect: 'deny' })
    expect(rules).toContainEqual({ action: 'external_directory', resource: '*', effect: 'deny' })
    // 0.818: V1 matched an allowed `git status*` with its redirect taken off, so `git status > x.txt` wrote the file.
    // V1's read-only shell asks for everything again, and V2 is told the same: one rule, no allows after it.
    expect(rules.filter((rule: { action: string }) => rule.action === 'shell')).toEqual([{ action: 'shell', resource: '*', effect: 'ask' }])
    expect(rules).not.toContainEqual(expect.objectContaining({ action: 'bash' }))
  })

  it('keeps its own catch-all, never V1\'s (0.821)', () => {
    // V1 now asks about every tool its rules do not name and allows its reading tools after, by V1's names. V2's names
    // for those were never measured, so V1's catch-all is not carried over: one catch-all, V2's own, first.
    for (const [sandbox, approveEach] of [['read-only', false], ['workspace-write', true], ['workspace-write', false], ['full-access', false]] as const) {
      const rules = config(sandbox, approveEach).agents.locust.permissions as Array<{ action: string; resource: string; effect: string }>
      expect(rules.filter((rule) => rule.action === '*' && rule.resource === '*'), `${sandbox}${approveEach ? ', approve each' : ''}`).toHaveLength(1)
      expect(rules[0]?.action).toBe('*')
    }
    expect(config('read-only').agents.locust.permissions[0]).toEqual({ action: '*', resource: '*', effect: 'allow' })
  })

  it('keeps Approve each asking and makes Auto explicit without silently widening Accept edits', () => {
    const asking = config('workspace-write', true).agents.locust.permissions
    expect(asking[0]).toEqual({ action: '*', resource: '*', effect: 'ask' })
    expect(asking).toContainEqual({ action: 'edit', resource: '*', effect: 'ask' })
    expect(asking).toContainEqual({ action: 'shell', resource: '*', effect: 'ask' })
    expect(config('workspace-write').agents.locust.permissions).toContainEqual({ action: 'external_directory', resource: '*', effect: 'deny' })
    expect(config('full-access').agents.locust.permissions).toContainEqual({ action: 'external_directory', resource: '*', effect: 'allow' })
    expect(config('full-access').agents.locust.permissions[0]).toEqual({ action: '*', resource: '*', effect: 'allow' })
    expect(config('full-access').policies).toBeUndefined()
  })

  it('grants references and worktree git metadata after the default denial, without granting the shared root', () => {
    const rules = config('read-only', false, { repositoryRoot: '/shared/repo', referenceFolders: ['/reference'] }).agents.locust.permissions as Array<{ action: string; resource: string; effect: string }>
    const paths = rules.filter((rule) => rule.action === 'external_directory')
    expect(paths[0]).toEqual({ action: 'external_directory', resource: '*', effect: 'deny' })
    expect(paths).toContainEqual({ action: 'external_directory', resource: '/shared/repo/.git/*', effect: 'allow' })
    expect(paths).toContainEqual({ action: 'external_directory', resource: '/reference/**', effect: 'allow' })
    expect(paths).not.toContainEqual({ action: 'external_directory', resource: '/shared/repo/*', effect: 'allow' })
  })

  it('does not enable a chat-only own model to call tools and keeps its provider declaration per-run', () => {
    const found = config('full-access', false, { providers: { 'own-local': { name: 'Local', baseUrl: 'http://localhost:1234/v1', models: ['model'], toolCalls: false } } })
    expect(found.provider['own-local']).toBeDefined()
    expect(found.agents.locust.permissions.at(-1)).toEqual({ action: '*', resource: '*', effect: 'deny' })
  })
})
