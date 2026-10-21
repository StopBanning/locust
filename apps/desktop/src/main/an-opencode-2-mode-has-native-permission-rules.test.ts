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

  it('denies edits and subagents in Ask while preserving reading commands and refusing unsafe git variants', () => {
    const rules = config('read-only').agents.locust.permissions
    expect(rules).toContainEqual({ action: 'edit', resource: '*', effect: 'deny' })
    expect(rules).toContainEqual({ action: 'subagent', resource: '*', effect: 'deny' })
    expect(rules).toContainEqual({ action: 'external_directory', resource: '*', effect: 'deny' })
    const read = rules.findIndex((rule: { resource: string }) => rule.resource === 'git diff*')
    const unsafe = rules.findIndex((rule: { resource: string }) => rule.resource === 'git *--output*')
    expect(rules[read]).toEqual({ action: 'shell', resource: 'git diff*', effect: 'allow' })
    expect(rules[unsafe]).toEqual({ action: 'shell', resource: 'git *--output*', effect: 'ask' })
    expect(unsafe).toBeGreaterThan(read)
    expect(rules).not.toContainEqual(expect.objectContaining({ action: 'bash' }))
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
