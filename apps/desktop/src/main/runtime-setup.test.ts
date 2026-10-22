import { describe, expect, it } from 'vitest'

import { copilotMcpServerNames, readRuntimeSetup } from './runtime-setup.js'

/** A fake disk: paths that exist map to text; anything else is ENOENT. */
function disk(files: Readonly<Record<string, string>>) {
  const normal = (p: string) => p.replace(/\\/g, '/').toLowerCase()
  const held = new Map(Object.entries(files).map(([path, text]) => [normal(path), text]))
  return async (path: string): Promise<string> => {
    const text = held.get(normal(path))
    if (text === undefined) {
      const error = new Error('not found') as NodeJS.ErrnoException
      error.code = 'ENOENT'
      throw error
    }
    if (text === '<<EACCES>>') {
      const error = new Error('denied') as NodeJS.ErrnoException
      error.code = 'EACCES'
      throw error
    }
    return text
  }
}

const HOME = 'C:/Users/colin'
const WS = 'C:/Users/colin/shop'

describe("what each runtime has set up for itself", () => {
  it('reads Claude Code hooks from user and project settings, and MCP servers from three places, names only', async () => {
    const setup = await readRuntimeSetup({
      workspacePath: WS,
      homeDirectory: HOME,
      read: disk({
        'C:/Users/colin/.claude/settings.json': JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: 'command', command: 'secret.sh' }] }], PreToolUse: [{}, {}] } }),
        'C:/Users/colin/shop/.claude/settings.json': JSON.stringify({ hooks: { PostToolUse: [{}] } }),
        'C:/Users/colin/.claude.json': JSON.stringify({
          mcpServers: { github: { command: 'npx', args: ['--token', 'SECRET'] } },
          projects: { 'C:\\Users\\colin\\shop': { mcpServers: { robinhood: {} } }, 'C:/elsewhere': { mcpServers: { other: {} } } }
        }),
        'C:/Users/colin/shop/.mcp.json': JSON.stringify({ mcpServers: { playwright: {} } })
      })
    })
    expect(setup.claude?.hooks).toEqual(['Stop (1)', 'PreToolUse (2)', 'PostToolUse (1)'])
    expect(setup.claude?.mcpServers).toEqual(['github', 'robinhood', 'playwright'])
    expect(setup.claude?.unreadable).toEqual([])
    expect(JSON.stringify(setup)).not.toContain('SECRET')
    expect(JSON.stringify(setup)).not.toContain('secret.sh')
  })

  it('reads Codex MCP servers from TOML table headers and its notify hook, without a TOML parser', async () => {
    const setup = await readRuntimeSetup({
      workspacePath: undefined,
      homeDirectory: HOME,
      read: disk({
        'C:/Users/colin/.codex/config.toml': [
          'model = "gpt-5"',
          'notify = ["node", "C:\\\\scripts\\\\notify.mjs"]',
          '',
          '[mcp_servers.node_repl]',
          'command = "node"',
          '',
          '[mcp_servers.node_repl.env]',
          'TOKEN = "SECRET"',
          '',
          '[mcp_servers.robinhood]',
          'url = "https://example"'
        ].join('\n')
      })
    })
    expect(setup.codex?.mcpServers).toEqual(['node_repl', 'robinhood'])
    expect(setup.codex?.hooks).toEqual(['notify (1)'])
    expect(JSON.stringify(setup)).not.toContain('SECRET')
  })

  it('reads Cursor, OpenCode and Copilot from their own files, in the folder and at home', async () => {
    const setup = await readRuntimeSetup({
      workspacePath: WS,
      homeDirectory: HOME,
      read: disk({
        'C:/Users/colin/.cursor/mcp.json': JSON.stringify({ mcpServers: { figma: {} } }),
        'C:/Users/colin/shop/.cursor/hooks.json': JSON.stringify({ hooks: { afterFileEdit: [{ command: 'fmt' }] } }),
        'C:/Users/colin/shop/opencode.json': JSON.stringify({ mcp: { context7: {} } }),
        'C:/Users/colin/.copilot/mcp-config.json': JSON.stringify({ mcpServers: { github: {} } })
      })
    })
    expect(setup.cursor).toMatchObject({ mcpServers: ['figma'], hooks: ['afterFileEdit (1)'] })
    expect(setup.opencode?.mcpServers).toEqual(['context7'])
    expect(setup.copilot?.mcpServers).toEqual(['github'])
  })

  it('an absent file is nothing; a file that cannot be read or parsed is named, never guessed', async () => {
    const setup = await readRuntimeSetup({
      workspacePath: WS,
      homeDirectory: HOME,
      read: disk({
        'C:/Users/colin/.claude/settings.json': '{not json',
        'C:/Users/colin/.cursor/mcp.json': '<<EACCES>>'
      })
    })
    expect(setup.claude?.hooks).toEqual([])
    expect(setup.claude?.unreadable.map((p) => p.replace(/\\/g, '/'))).toEqual(['C:/Users/colin/.claude/settings.json'])
    expect(setup.cursor?.unreadable.map((p) => p.replace(/\\/g, '/'))).toEqual(['C:/Users/colin/.cursor/mcp.json'])
    expect(setup.codex).toEqual({ mcpServers: [], hooks: [], skills: [], agents: [], sources: [], unreadable: [] })
  })

  // 0.824: the names an Ask or Plan run on Copilot leaves off, from the same file.
  it('names the MCP servers in the person’s own Copilot settings, and none when there are none to read', async () => {
    const read = disk({ 'C:/Users/colin/.copilot/mcp-config.json': JSON.stringify({ mcpServers: { notes: { command: 'n' }, 'robinhood-trading': { url: 'https://x' } } }) })
    expect(await copilotMcpServerNames({ homeDirectory: HOME, read })).toEqual(['notes', 'robinhood-trading'])
    expect(await copilotMcpServerNames({ homeDirectory: HOME, read: disk({}) })).toEqual([])
    expect(await copilotMcpServerNames({ homeDirectory: HOME, read: disk({ 'C:/Users/colin/.copilot/mcp-config.json': '{not json' }) })).toEqual([])
  })
})

/*
 * A4.1: skills from every runtime's own folders, as each finds them -- Codex
 * measured through its app-server's skills/list, OpenCode read from its
 * source. Cursor and Copilot are not guessed at.
 */
describe('the skills each runtime finds', () => {
  const folders = (tree: Readonly<Record<string, readonly string[]>>) => {
    const normal = (p: string) => p.replace(/\\/g, '/').toLowerCase()
    const held = new Map(Object.entries(tree).map(([path, entries]) => [normal(path), entries]))
    return async (directory: string): Promise<readonly string[]> => {
      const entries = held.get(normal(directory))
      if (entries === undefined) {
        const error = new Error('not found') as NodeJS.ErrnoException
        error.code = 'ENOENT'
        throw error
      }
      return entries
    }
  }
  const tree = {
    'C:/Users/colin/.claude/skills': ['reskin/', 'new-lead/'],
    'C:/Users/colin/.agents/skills': ['shared-one/'],
    'C:/Users/colin/.codex/skills': ['.system/', 'codex-own/'],
    'C:/Users/colin/shop/.agents/skills': ['project-agents/'],
    'C:/Users/colin/shop/.opencode/skills': ['opencode-own/'],
    'C:/Users/colin/.copilot/skills': ['copilot-own/'],
    'C:/Users/colin/shop/.github/skills': ['github-project/'],
    'C:/Users/colin/.cursor/skills': ['cursor-own/'],
    'C:/Users/colin/.cursor/skills-cursor': ['canvas/']
  }

  it('lists Codex skills from ~/.codex/skills and the .agents folders, never the Claude folder or its built-ins', async () => {
    const setup = await readRuntimeSetup({ workspacePath: WS, homeDirectory: HOME, read: disk({}), list: folders(tree) })
    expect(setup.codex?.skills).toEqual(['codex-own', 'shared-one', 'project-agents'])
  })

  it('lists OpenCode skills from the Claude and .agents folders and its own', async () => {
    const setup = await readRuntimeSetup({ workspacePath: WS, homeDirectory: HOME, read: disk({}), list: folders(tree) })
    expect(setup.opencode?.skills).toEqual(['reskin', 'new-lead', 'shared-one', 'project-agents', 'opencode-own'])
    // Claude Code's own list is unchanged: its two folders.
    expect(setup.claude?.skills).toEqual(['reskin', 'new-lead'])
  })

  it('lists Cursor skills from its own two roots, never its built-ins or the third-party folders', async () => {
    const setup = await readRuntimeSetup({ workspacePath: WS, homeDirectory: HOME, read: disk({}), list: folders(tree) })
    expect(setup.cursor?.skills).toEqual(['cursor-own', 'shared-one', 'project-agents'])
  })

  it('lists Copilot skills from where `copilot skill --help` says it looks, the Claude folder only in the project', async () => {
    const setup = await readRuntimeSetup({ workspacePath: WS, homeDirectory: HOME, read: disk({}), list: folders(tree) })
    expect(setup.copilot?.skills).toEqual(['copilot-own', 'shared-one', 'github-project', 'project-agents'])
  })
})
