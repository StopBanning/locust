import { describe, expect, it } from 'vitest'
import { codexReadOnlyThreadConfig } from './codex-read-only.js'

/*
 * 0.824. MEASURED 2026-10-22 on Codex 0.163.0 under Locust's Ask (read-only, approval never): a tool marked
 * `readOnlyHint: true` that wrote a file ran without asking and wrote it; with `mcp_servers.<name>.enabled = false` it
 * was not there. So Ask and Plan switch off, for the turn only, every server the person's Codex runs as a program.
 */
describe('Ask and Plan start none of the programs your Codex runs as MCP servers', () => {
  it('switches off each server that runs as a program, by name, and every plugin', () => {
    expect(codexReadOnlyThreadConfig({
      model: 'gpt-6-luna',
      mcp_servers: {
        node_repl: { command: 'node', args: ['repl.mjs'] },
        notes: { command: 'notes-server', enabled: true },
        docs: { url: 'https://docs.example.com/mcp' }
      }
    })).toEqual({
      ok: true,
      config: { 'features.plugins': false, 'mcp_servers.node_repl.enabled': false, 'mcp_servers.notes.enabled': false }
    })
  })

  it('leaves a server reached at an address as the person has it: Codex still refuses its tools that do more than read', () => {
    const made = codexReadOnlyThreadConfig({ mcp_servers: { gmail: { url: 'https://mail.example.com/mcp' } } })
    expect(made).toEqual({ ok: true, config: { 'features.plugins': false } })
  })

  it('switches off a server that names both a program and an address, and one it cannot read the shape of', () => {
    const made = codexReadOnlyThreadConfig({ mcp_servers: { both: { url: 'https://x.example.com', command: 'x' }, odd: 'not a table' } })
    expect(made).toEqual({ ok: true, config: { 'features.plugins': false, 'mcp_servers.both.enabled': false, 'mcp_servers.odd.enabled': false } })
  })

  it('with no servers of the person’s own, still takes every plugin’s', () => {
    expect(codexReadOnlyThreadConfig({})).toEqual({ ok: true, config: { 'features.plugins': false } })
    expect(codexReadOnlyThreadConfig({ mcp_servers: null })).toEqual({ ok: true, config: { 'features.plugins': false } })
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
