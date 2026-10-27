import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { codexAppServerPolicy, createClaudePrintCommand, createOpenCodeServeCommand } from '@teammate/runtime-adapters'
import type { ExecutableLaunch, MissionRuntimeId, MissionSandbox } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { WHAT_LOCUST_CAN_STOP, whatLocustCanStopDocument } from '../shared/what-locust-can-stop.js'
import { acpPermissionRequest, openCodePermissionRequest } from './approval-channel.js'

/**
 * WHAT LOCUST CAN STOP IS WHAT EACH AGENT ASKS (0.617; the PRD's R9).
 *
 * The table in Settings > AI agents and docs/WHAT-LOCUST-CAN-STOP.md come
 * from one list. Each row's reach is held here to the code that builds the
 * run and the paths that raise a card, so a change to either fails until the
 * table says it too.
 */
const launch = (commandName: string): ExecutableLaunch =>
  ({ commandName, discoveredPath: `C:\\tools\\${commandName}.exe`, executablePath: `C:\\tools\\${commandName}.exe`, prefixArgs: [], kind: 'native' }) as ExecutableLaunch
const row = (runtime: MissionRuntimeId) => WHAT_LOCUST_CAN_STOP.find((entry) => entry.runtime === runtime)!
const source = (file: string): string => readFileSync(fileURLToPath(new URL(file, import.meta.url)), 'utf8')
/*
 * The modes each runtime offers, read from RUNTIME_CAPABILITIES as written
 * (renderer/src/status.ts): the main process's typecheck does not reach the
 * renderer, so this reads the table the way the other main tests read
 * renderer source.
 */
const RUNTIMES: readonly MissionRuntimeId[] = ['codex', 'claude', 'cursor', 'gemini', 'opencode', 'copilot', 'antigravity', 'muse', 'kilo', 'acp']
const capabilities = source('../renderer/src/status.ts')
const table = capabilities.slice(capabilities.indexOf('export const RUNTIME_CAPABILITIES'))
const modesOf = (runtime: MissionRuntimeId): string => {
  const found = new RegExp(`\\n  ${runtime}: \\{ modes: \\[([^\\]]*)\\]`).exec(table)
  if (found === null) throw new Error(`no RUNTIME_CAPABILITIES row for ${runtime}`)
  return found[1]!.trim()
}

describe('one row for every AI agent a run can start on', () => {
  it('lists each runtime with a mode, once, and none without one', () => {
    const runnable = RUNTIMES.filter((runtime) => modesOf(runtime).length > 0)
    expect(runnable).not.toContain('gemini')
    expect(WHAT_LOCUST_CAN_STOP.map((entry) => entry.runtime).sort()).toEqual([...runnable].sort())
  })

  it('says "each action" exactly where Approve each action is offered and Locust sets what is asked', () => {
    for (const entry of WHAT_LOCUST_CAN_STOP) {
      const offered = modesOf(entry.runtime).includes("'approve-each'")
      // An added ACP agent (0.777) decides for itself which actions it asks about.
      expect(entry.reach === 'each-action', entry.runtime).toBe(offered && entry.runtime !== 'acp')
      expect(/Approve each action/.test(entry.asks), entry.runtime).toBe(offered)
    }
    expect(RUNTIMES.filter((runtime) => modesOf(runtime).includes("'approve-each'")).sort()).toEqual(['acp', 'claude', 'codex', 'copilot', 'opencode'])
    expect(row('acp').reach).toBe('some-actions')
    expect(row('acp').asks).toMatch(/agent’s own setting/)
  })
})

describe('each row is what the run is given', () => {
  it('Codex: asks in Approve each action, and never in the other modes', () => {
    expect(codexAppServerPolicy('workspace-write', 'approve-each').approvalPolicy).toBe('untrusted')
    for (const sandbox of ['read-only', 'workspace-write', 'full-access'] as const) expect(codexAppServerPolicy(sandbox).approvalPolicy, sandbox).toBe('never')
    expect(row('codex').always).toMatch(/^Locust keeps an Always/)
  })

  it('Claude Code: asks in every mode but Auto, and has a shell only in Edit', () => {
    const bridge = { configPath: 'C:\\locust\\mcp.json', toolName: 'mcp__locust__approve' }
    const args = (sandbox: MissionSandbox) => createClaudePrintCommand(launch('claude'), { workspacePath: 'C:\\work', sandbox, permissionBridge: bridge }).args
    for (const sandbox of ['read-only', 'workspace-write'] as const) expect(args(sandbox), sandbox).toContain('--permission-prompt-tool')
    expect(args('full-access')).not.toContain('--permission-prompt-tool')
    const tools = (sandbox: MissionSandbox) => args(sandbox)[args(sandbox).indexOf('--tools') + 1]!.split(',')
    expect(tools('read-only')).not.toContain('Bash')
    expect(tools('workspace-write')).toContain('Bash')
    // Approve each (0.837): Claude Code's own default mode, so every edit and command asks the bridge.
    const each = createClaudePrintCommand(launch('claude'), { workspacePath: 'C:\\work', sandbox: 'workspace-write', askEach: true, permissionBridge: bridge }).args
    expect(each[each.indexOf('--permission-mode') + 1]).toBe('default')
    expect(row('claude').reach).toBe('each-action')
    expect(source('./codex-mission.ts')).toContain("...(mode === 'approve-each' ? { askEach: true } : {})")
    // Every mode but Auto registers a run with the permission host.
    expect(source('./codex-mission.ts')).toContain("runtime === 'claude' && effectiveSandbox !== 'full-access' && options.permissionHost !== undefined")
  })

  it('OpenCode: in Approve each action its server asks about commands, changes, web pages, outside paths and the person\'s own MCP tools, not searches', () => {
    const permission = (JSON.parse(createOpenCodeServeCommand(launch('opencode'), { workspacePath: 'C:\\work' }).env?.OPENCODE_CONFIG_CONTENT ?? '{}') as { permission: Record<string, unknown> }).permission
    expect([permission.bash, permission.edit, permission.webfetch, permission.external_directory]).toEqual(['ask', 'ask', 'ask', 'ask'])
    // 0.821: every tool it does not name asks -- a person's own MCP tool among them -- and searches are allowed by name.
    expect(Object.entries(permission)[0]).toEqual(['*', 'ask'])
    expect([permission.websearch, permission.codesearch]).toEqual(['allow', 'allow'])
    expect(row('opencode').asks).toMatch(/your own MCP servers/)
    expect(row('opencode').without).toMatch(/^Web searches, in every mode\./)
    // And in Ask and Plan the same catch-all is a refusal: none of the person's own MCP tools run there.
    const ask = (JSON.parse(createOpenCodeServeCommand(launch('opencode'), { workspacePath: 'C:\\work', sandbox: 'read-only' }).env?.OPENCODE_CONFIG_CONTENT ?? '{}') as { permission: Record<string, unknown> }).permission
    expect(Object.entries(ask)[0]).toEqual(['*', 'ask'])
    expect(row('opencode').without).toMatch(/In Ask and Plan, no file changes, no commands and none of your own MCP tools/)
  })

  it('Antigravity: only its questions reach a card', () => {
    const antigravity = source('./antigravity-mission.ts')
    const raised = [...antigravity.matchAll(/options\.emitApproval\((\w+)/g)].map((match) => match[1])
    expect(raised).toEqual(['questionCard'])
    expect(row('antigravity').reach).toBe('questions-only')
  })

  it('Cursor and Muse: no path raises a card for them', () => {
    // Each call's arguments, up to its first closing parenthesis (no braces here: the
    // assertion guard reads a test's body by counting them).
    const handlers = source('./codex-mission.ts').split('requestHandlerFor(').slice(1).map((after) => after.slice(0, after.indexOf(')')))
    expect(handlers.length).toBeGreaterThan(2)
    expect(handlers.some((handler) => /cursor|muse/.test(handler))).toBe(false)
    for (const runtime of ['cursor', 'muse'] as const) expect(row(runtime).reach, runtime).toBe('nothing')
  })

  it('an Always is kept by Locust exactly where the request carries a key for it', () => {
    expect(source('./permission-host.ts')).toContain('alwaysKey: `claude:${alwaysKeyOf(toolName)}`')
    expect((openCodePermissionRequest({ permission: 'bash', patterns: ['echo hi'], always: ['echo *'], metadata: {} }, 'C:/work').params as Record<string, unknown>).locustAlwaysKey).toBeDefined()
    expect((acpPermissionRequest({ toolCallId: 't', title: 'x', kind: 'execute', command: 'npm test', paths: [], diff: undefined, options: [] }, 'C:/work').params as Record<string, unknown>).locustAlwaysKey).toBeDefined()
    for (const runtime of ['codex', 'claude', 'opencode', 'copilot', 'acp'] as const) expect(row(runtime).always, runtime).toMatch(/^Locust keeps an Always/)
    for (const runtime of ['cursor', 'antigravity', 'muse'] as const) expect(row(runtime).always, runtime).toBeUndefined()
  })
})

describe('the document is the list', () => {
  it('docs/WHAT-LOCUST-CAN-STOP.md is written from it (rewrite with -u)', async () => {
    await expect(whatLocustCanStopDocument()).toMatchFileSnapshot('../../../../docs/WHAT-LOCUST-CAN-STOP.md')
  })
})
