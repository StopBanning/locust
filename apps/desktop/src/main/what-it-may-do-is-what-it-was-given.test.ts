import {
  codexAppServerPolicy,
  createClaudePrintCommand,
  createCodexAppServerCommand,
  createCodexExecCommand,
  createCopilotPromptCommand,
  createCursorPrintCommand,
  createMuseExecCommand,
  createOpenCodeRunCommand,
  createOpenCodeServeCommand
} from '@teammate/runtime-adapters'
import type { ExecutableLaunch, MissionSandbox } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { whatItMayDo } from '../shared/may-do.js'
import { codexReadOnlyThreadConfig } from './codex-read-only.js'

/**
 * WHAT THE PANEL SAYS A RUN MAY DO IS WHAT THE RUN WAS GIVEN (0.359).
 *
 * The Activity panel said "deny: ... any network access beyond the model's
 * own" under an OpenCode run that had just searched the web twice (the
 * first-session drive, packaged 0.358). The sentence was written once for
 * every runtime and checked against none of them.
 *
 * Each claim below is read back out of the command the real builder makes
 * for that runtime and mode -- its config, its tool list, its flags -- so a
 * change to what a run is given fails here until the panel says it too.
 */
const launch = (commandName: string): ExecutableLaunch =>
  ({ commandName, discoveredPath: `C:\\tools\\${commandName}.exe`, executablePath: `C:\\tools\\${commandName}.exe`, prefixArgs: [], kind: 'native' }) as ExecutableLaunch
const options = (sandbox: MissionSandbox) => ({ workspacePath: 'C:\\work\\pebble', prompt: 'go', sandbox })
const said = (runtime: Parameters<typeof whatItMayDo>[0], sandbox: MissionSandbox, mode?: Parameters<typeof whatItMayDo>[2]) =>
  whatItMayDo(runtime, sandbox, mode).rows.map((row) => `${row.verdict} ${row.text}`)
const SANDBOXES = ['read-only', 'workspace-write', 'full-access'] as const

describe('what the panel says a run may do', () => {
  it('OpenCode: the web, commands and the folder, from the config the run is given', () => {
    for (const sandbox of SANDBOXES) {
      const spec = createOpenCodeRunCommand(launch('opencode'), options(sandbox))
      const permission = (JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? '{}') as { permission?: Record<string, unknown> }).permission ?? {}
      const rows = said('opencode', sandbox)
      if (sandbox === 'full-access') {
        expect(spec.args).toContain('--auto')
        expect(permission.external_directory).toBe('allow')
        expect(rows).toContain('allow run any command your account can run, and reach any network')
        continue
      }
      // The web is allowed: by OpenCode's own default in Edit, which names no
      // web rule, and by name after the catch-all in Ask (0.821). The panel
      // must say so either way.
      for (const web of [permission.webfetch, permission.websearch]) expect([undefined, 'allow'], sandbox).toContain(web)
      expect(rows).toContain('allow search the web and open web pages')
      // Ask's catch-all refuses a person's own MCP tools (0.821); Edit leaves them to OpenCode, which allows them.
      if (sandbox === 'read-only') {
        expect(Object.entries(permission)[0]).toEqual(['*', 'ask'])
        expect(rows).toContain('deny using tools from your own MCP servers')
      } else {
        expect(permission['*']).toBeUndefined()
        expect(rows.join('\n')).not.toMatch(/MCP/)
      }
      expect(rows.join('\n')).not.toMatch(/network access|deny.*network/)
      // `run` refuses every "ask"; nothing named means allowed.
      if (permission.bash === 'ask') expect(rows).toContain('deny running commands')
      if (permission.bash === undefined) expect(rows).toContain('allow run commands, which can reach anything your account can')
      expect(permission.external_directory).toBe('deny')
      expect(rows).toContain('deny opening files outside this folder, other than by running a command')
    }
  })

  it('OpenCode, Approve each: what its server asks for is what the panel says it asks for', () => {
    const spec = createOpenCodeServeCommand(launch('opencode'), { workspacePath: 'C:\\work\\pebble' })
    const permission = (JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? '{}') as { permission: Record<string, unknown> }).permission
    expect([permission.edit, permission.bash, permission.webfetch, permission.external_directory]).toEqual(['ask', 'ask', 'ask', 'ask'])
    // 0.821: everything not named asks, first -- a person's own MCP tools among it -- and the search is allowed by name.
    expect(Object.entries(permission)[0]).toEqual(['*', 'ask'])
    expect(permission.websearch).toBe('allow')
    const rows = said('opencode', 'workspace-write', 'approve-each')
    expect(rows).toContain('allow change files in this folder, once you approve each change')
    expect(rows).toContain('allow run commands and open web pages, once you approve each one')
    expect(rows).toContain('allow search the web')
    expect(rows).toContain('allow open files outside this folder, once you approve it')
    expect(rows).toContain('allow use tools from your own MCP servers, once you approve each one')
  })

  it('Claude Code: commands only when its tool list has Bash; the web in every mode, asked about outside Auto', () => {
    for (const sandbox of ['read-only', 'workspace-write'] as const) {
      const spec = createClaudePrintCommand(launch('claude'), options(sandbox))
      const tools = spec.args[spec.args.indexOf('--tools') + 1]!.split(',')
      const rows = said('claude', sandbox)
      // 0.711: given, and asked about -- no allow rule names them, so each goes to the bridge's card.
      expect(tools).toContain('WebFetch')
      expect(tools).toContain('WebSearch')
      expect(spec.args.some((arg) => /^Web(?:Search|Fetch)/.test(arg) && arg !== tools.join(','))).toBe(false)
      expect(rows).toContain('allow search the web and open web pages, once you approve each one')
      expect(rows).toContain(tools.includes('Bash') ? 'allow run commands, which can reach anything your account can' : 'deny running commands')
      // Skills (0.679): the tool, and the skills it is handed as plugins.
      expect(tools).toContain('Skill')
      expect(rows).toContain("allow use this folder's skills, yours when Settings lends them, and the ones you kept from GitHub, with only the tools listed here")
      const handed = createClaudePrintCommand(launch('claude'), { ...options(sandbox), skillPlugins: ['C:/data/claude-skills/run/project'] })
      expect(handed.args.slice(handed.args.indexOf('--plugin-dir'), handed.args.indexOf('--plugin-dir') + 4)).toEqual(['--plugin-dir', 'C:/data/claude-skills/run/project', '--add-dir', 'C:/data/claude-skills/run/project'])
    }
    // Auto runs as the person and finds the folder's skills and theirs itself; it is handed only the ones kept
    // from GitHub (0.710), and needs no --add-dir for them.
    const auto = createClaudePrintCommand(launch('claude'), { ...options('full-access'), skillPlugins: ['C:/data/x/library'] })
    expect(auto.args.slice(auto.args.indexOf('--plugin-dir'), auto.args.indexOf('--plugin-dir') + 2)).toEqual(['--plugin-dir', 'C:/data/x/library'])
    expect(auto.args).not.toContain('--add-dir')
    expect(auto.args[auto.args.indexOf('--tools') + 1]!.split(',')).toContain('Skill')
  })

  it('Copilot CLI: commands refused only where its deny list says so, and paths held unless Auto', () => {
    const readOnly = createCopilotPromptCommand(launch('copilot'), options('read-only'))
    expect(readOnly.args).toContain('--deny-tool=write,shell')
    expect(said('copilot', 'read-only')).toContain('deny running commands')
    // 0.824: and none of the MCP servers in the person's Copilot settings.
    expect(createCopilotPromptCommand(launch('copilot'), { ...options('read-only'), copilotMcpServersOff: ['notes'] }).args).toContain('--disable-mcp-server')
    expect(said('copilot', 'read-only')).toContain('deny starting the MCP servers in your Copilot settings')
    const edit = createCopilotPromptCommand(launch('copilot'), options('workspace-write'))
    expect(edit.args.join(' ')).not.toMatch(/--deny-tool|--allow-all-paths/)
    expect(said('copilot', 'workspace-write')).toEqual(expect.arrayContaining(['allow run commands, which can reach anything your account can', 'deny opening files outside this folder, other than by running a command']))
    expect(createCopilotPromptCommand(launch('copilot'), options('full-access')).args).toContain('--allow-all-paths')
  })

  it("Muse Code: its sandbox stays on, its web tools stay on, its shell is off when read-only", () => {
    const readOnly = createMuseExecCommand(launch('muse'), options('read-only'))
    expect(readOnly.args).toEqual(expect.arrayContaining(['--disable-write', '--disable-shell']))
    expect(said('muse', 'read-only')).toContain('deny running commands')
    const edit = createMuseExecCommand(launch('muse'), options('workspace-write'))
    expect(edit.args.join(' ')).not.toMatch(/--disable-sandbox|--yolo|--disable-web-tools/)
    expect(edit.args.join(' ')).toContain('--approval-mode never')
    expect(said('muse', 'workspace-write')).toEqual(expect.arrayContaining(["allow run commands inside Muse's sandbox", 'allow search the web and open web pages']))
    // Auto (0.773): outside Muse's sandbox, and the chip says so.
    const auto = createMuseExecCommand(launch('muse'), options('full-access'))
    expect(auto.args).toContain('--disable-sandbox')
    expect(auto.args).not.toContain('--yolo')
    expect(said('muse', 'full-access')).toContain('allow run any command your account can run, and reach any network')
  })

  it('Codex: its own sandbox in every mode, and Approve each asks', () => {
    expect(codexAppServerPolicy('read-only').sandbox).toBe('read-only')
    expect(said('codex', 'read-only')).toContain("allow run commands in Codex's sandbox, which lets them read and change nothing")
    expect(codexAppServerPolicy('workspace-write').sandbox).toBe('workspace-write')
    expect(said('codex', 'workspace-write')).toContain("allow run commands in Codex's sandbox, which keeps their changes in this folder")
    expect(codexAppServerPolicy('workspace-write', 'approve-each').approvalPolicy).toBe('untrusted')
    expect(said('codex', 'workspace-write', 'approve-each')).toContain("allow run commands in Codex's sandbox, asking you before any that does more than read")
    expect(codexAppServerPolicy('full-access').sandbox).toBe('danger-full-access')
    expect(said('codex', 'full-access')).toContain('allow run any command your account can run, and reach any network')
    // 0.824: read-only starts none of the person's Codex programs (codexReadOnlyThreadConfig, given to its run); no
    // other mode says anything about them.
    expect(codexReadOnlyThreadConfig({ mcp_servers: { notes: { command: 'notes' } } })).toMatchObject({ ok: true, config: { 'features.plugins': false, 'features.computer_use': false, 'mcp_servers.notes.enabled': false } })
    expect(said('codex', 'read-only')).toContain('deny starting the MCP servers and plugins your Codex runs as programs on this computer, or its own browser and computer use')
    for (const sandbox of ['workspace-write', 'full-access'] as const) expect(said('codex', sandbox).join('\n')).not.toMatch(/MCP/)
  })

  it('Codex: its own web search is left on in every mode, and Approve each says it does not ask', () => {
    const appServer = createCodexAppServerCommand(launch('codex'), { workspacePath: 'C:\\work\\pebble', cliVersion: '0.162.0' })
    expect(appServer.args.join(' ')).not.toMatch(/web_search/)
    expect(createCodexExecCommand(launch('codex'), options('workspace-write')).args.join(' ')).not.toMatch(/web_search/)
    for (const sandbox of SANDBOXES) {
      expect(said('codex', sandbox)).toContain('allow search the web and open web pages')
    }
    expect(said('codex', 'workspace-write', 'approve-each')).toContain('allow search the web and open web pages, without asking')
  })

  it('Cursor Agent: says nothing about commands it was not given a rule for', () => {
    expect(createCursorPrintCommand(launch('cursor'), options('workspace-write')).args).not.toContain('--force')
    expect(said('cursor', 'workspace-write')).toEqual([
      'allow read the files in this folder',
      'allow change files in this folder',
      'deny searching the web or opening web pages, which Cursor Agent allows here only in Auto'
    ])
    expect(createCursorPrintCommand(launch('cursor'), options('full-access')).args).toContain('--force')
    expect(said('cursor', 'full-access')).toEqual(expect.arrayContaining(['allow run any command your account can run, and reach any network', 'allow search the web and open web pages']))
    // 0.823: Ask and Plan leave out `--approve-mcps`, so a folder's own `.cursor/mcp.json` server is never started.
    expect(createCursorPrintCommand(launch('cursor'), options('read-only')).args).not.toContain('--approve-mcps')
    expect(said('cursor', 'read-only')).toContain('deny starting the MCP servers this folder names')
    expect(createCursorPrintCommand(launch('cursor'), options('workspace-write')).args).toContain('--approve-mcps')
    expect(said('cursor', 'workspace-write').join('\n')).not.toMatch(/MCP/)
  })

  it('the web, runtime by runtime, as measured on 2026-10-09 (probe-web-search-each-runtime)', () => {
    // Copilot's web_fetch is one of --allow-all-tools, and read-only denies only write and shell.
    expect(createCopilotPromptCommand(launch('copilot'), options('read-only')).args).toContain('--deny-tool=write,shell')
    for (const sandbox of SANDBOXES) expect(said('copilot', sandbox)).toContain('allow search the web and open web pages')
    // Cursor and Antigravity: their own web tools ask, so only Auto runs them.
    for (const runtime of ['cursor', 'antigravity'] as const) {
      expect(said(runtime, 'full-access')).toContain('allow search the web and open web pages')
      for (const sandbox of ['read-only', 'workspace-write'] as const) {
        expect(said(runtime, sandbox).some((row) => row.startsWith('deny searching the web'))).toBe(true)
      }
    }
  })

  it('leaves what it was not given to the runtime, by name', () => {
    expect(whatItMayDo('cursor', 'workspace-write').rest).toBe("Anything not listed is left to Cursor Agent's own settings.")
    expect(whatItMayDo('opencode', 'read-only').rest).toBe("Anything not listed is left to OpenCode's own settings.")
  })

  it('never denies anything in Auto, on any runtime that offers it', () => {
    for (const runtime of ['codex', 'claude', 'cursor', 'opencode', 'copilot'] as const) {
      expect(whatItMayDo(runtime, 'full-access').rows.every((row) => row.verdict === 'allow')).toBe(true)
    }
  })
})
