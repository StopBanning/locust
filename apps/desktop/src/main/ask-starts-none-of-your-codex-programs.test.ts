import { describe, expect, it } from 'vitest'
import { codexReadOnlyThreadConfig, codexSealedThreadConfig } from './codex-read-only.js'

/*
 * 0.824. MEASURED 2026-10-22 on Codex 0.163.0 under Locust's Ask (read-only, approval never): a tool marked
 * `readOnlyHint: true` that wrote a file ran without asking and wrote it, and with its shell off Codex ran a command
 * through the person's node_repl; with `mcp_servers.<name>.enabled = false` neither tool was there. So Ask and Plan
 * switch off, for the turn only, every server the person's Codex runs as a program, its plugins, and its own browser
 * and computer use.
 */
const OFF = {
  'features.plugins': false,
  'features.browser_use': false,
  'features.browser_use_external': false,
  'features.in_app_browser': false,
  'features.computer_use': false
}

describe('Ask and Plan start none of the programs your Codex runs as MCP servers', () => {
  it('switches off each server that runs as a program, by name, every plugin, and its own browser and computer use', () => {
    expect(codexReadOnlyThreadConfig({
      model: 'gpt-6-luna',
      mcp_servers: {
        node_repl: { command: 'node', args: ['repl.mjs'] },
        notes: { command: 'notes-server', enabled: true },
        docs: { url: 'https://docs.example.com/mcp' }
      }
    })).toEqual({ ok: true, config: { ...OFF, 'mcp_servers.node_repl.enabled': false, 'mcp_servers.notes.enabled': false } })
  })

  it('leaves a server reached at an address as the person has it: Codex still refuses its tools that do more than read', () => {
    expect(codexReadOnlyThreadConfig({ mcp_servers: { gmail: { url: 'https://mail.example.com/mcp' } } })).toEqual({ ok: true, config: OFF })
  })

  it('switches off a server that names both a program and an address, and one it cannot read the shape of', () => {
    const made = codexReadOnlyThreadConfig({ mcp_servers: { both: { url: 'https://x.example.com', command: 'x' }, odd: 'not a table' } })
    expect(made).toEqual({ ok: true, config: { ...OFF, 'mcp_servers.both.enabled': false, 'mcp_servers.odd.enabled': false } })
  })

  it('with no servers of the person’s own, still takes every plugin’s, and the browser and computer use', () => {
    expect(codexReadOnlyThreadConfig({})).toEqual({ ok: true, config: OFF })
    expect(codexReadOnlyThreadConfig({ mcp_servers: null })).toEqual({ ok: true, config: OFF })
  })

  it('starts nothing, and says why, rather than start with something left on', () => {
    for (const settings of [undefined, null, 'settings', [], { mcp_servers: 'x' }, { mcp_servers: [] }]) {
      const made = codexReadOnlyThreadConfig(settings)
      expect(made.ok).toBe(false)
      if (!made.ok) expect(made.message).toMatch(/could not read your Codex settings, so it did not start this turn/)
    }
    // A name Codex's own settings path cannot hold as one key.
    const dotted = codexReadOnlyThreadConfig({ mcp_servers: { 'a.b': { command: 'x' } } })
    expect(dotted).toMatchObject({ ok: false, message: expect.stringContaining('"a.b"') })
  })
})

/*
 * A SEALED LOOK (0.824, Scout; Grok's review #4): no tool at all. MEASURED 2026-10-22 with these switches, asked to read
 * a file and search the web "by any means": "I could do neither". Without `features.apps` it had searched through
 * ChatGPT's apps.
 */
describe('a sealed run is started with no tool', () => {
  it('adds no shell, no web search and no ChatGPT apps, and switches off every server, connectors too', () => {
    expect(codexSealedThreadConfig({ mcp_servers: { node_repl: { command: 'node' }, gmail: { url: 'https://mail.example.com/mcp' } } })).toEqual({
      ok: true,
      config: {
        ...OFF,
        'features.apps': false,
        'features.shell_tool': false,
        web_search: 'disabled',
        'mcp_servers.node_repl.enabled': false,
        'mcp_servers.gmail.enabled': false
      }
    })
  })

  it('refuses as Ask does when it cannot read the settings', () => {
    expect(codexSealedThreadConfig(undefined).ok).toBe(false)
  })
})
