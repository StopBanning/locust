import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { allowCursorConnectors, cursorAllowRule, mergedCursorConfig } from './cursor-connector-allow.js'

/**
 * A Cursor run outside Auto can call the connectors the person configured.
 *
 * MEASURED 2026-09-15/16, Colin's ledger: 17, 5, 3, 2 `user rejected MCP` per
 * Accept-edits run, 0 on Auto -- each call asked and nobody could answer.
 * The cure is Cursor's own project permissions file, not `--force`.
 */

describe('the rule written for a server', () => {
  it('allows every tool of that server and nothing else', () => {
    expect(cursorAllowRule('robinhood-local')).toBe('Mcp(robinhood-local:*)')
  })
})

describe('merging allow rules into .cursor/cli.json', () => {
  it('creates the file when there is none', () => {
    const merged = mergedCursorConfig(undefined, ['robinhood-local'])
    expect(merged?.added).toEqual(['Mcp(robinhood-local:*)'])
    expect(JSON.parse(merged!.text!)).toEqual({ permissions: { allow: ['Mcp(robinhood-local:*)'], deny: [] } })
  })

  it('keeps everything already in the file, and never touches deny', () => {
    const before = {
      version: 1,
      editor: { vimMode: true },
      permissions: { allow: ['Shell(ls)', 'Mcp(robinhood-trading:get_accounts)'], deny: ['Shell(rm)'] }
    }
    const merged = mergedCursorConfig(JSON.stringify(before), ['robinhood-trading', 'robinhood-local'])
    const after = JSON.parse(merged!.text!) as typeof before
    expect(after.version).toBe(1)
    expect(after.editor).toEqual({ vimMode: true })
    expect(after.permissions.deny).toEqual(['Shell(rm)'])
    expect(after.permissions.allow).toEqual([
      'Shell(ls)',
      'Mcp(robinhood-trading:get_accounts)',
      'Mcp(robinhood-trading:*)',
      'Mcp(robinhood-local:*)'
    ])
  })

  it('writes nothing when every rule is already there', () => {
    const text = JSON.stringify({ permissions: { allow: ['Mcp(a:*)'], deny: [] } })
    expect(mergedCursorConfig(text, ['a'])).toEqual({ text: undefined, added: [] })
    expect(mergedCursorConfig(JSON.stringify({ permissions: { allow: ['Mcp(*:*)'] } }), ['a'])?.added).toEqual([])
  })

  it('leaves a file it cannot read as the config alone, rather than writing over it', () => {
    // The same ruling as every local store: unreadable is never empty.
    expect(mergedCursorConfig('{ not json', ['a'])).toBeUndefined()
    expect(mergedCursorConfig('[]', ['a'])).toBeUndefined()
    expect(mergedCursorConfig(JSON.stringify({ permissions: 'yes' }), ['a'])).toBeUndefined()
    expect(mergedCursorConfig(JSON.stringify({ permissions: { allow: 'all' } }), ['a'])).toBeUndefined()
  })
})

describe('writing the workspace file', () => {
  const io = (files: Record<string, string>) => {
    const writes: Record<string, string> = {}
    const dirs: string[] = []
    return {
      writes,
      dirs,
      io: {
        readFile: async (path: string) => {
          const held = files[path]
          if (held === undefined) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' })
          return held
        },
        writeFile: async (path: string, text: string) => {
          writes[path] = text
        },
        mkdir: async (path: string) => {
          dirs.push(path)
        },
        exists: async () => false,
        removeFile: async () => undefined,
        removeDir: async () => undefined
      }
    }
  }

  it('adds the rules under the workspace and says which were added', async () => {
    const fake = io({})
    const { added } = await allowCursorConnectors('C:\\work', ['robinhood-local'], fake.io)
    expect(added).toEqual(['Mcp(robinhood-local:*)'])
    const [path, text] = Object.entries(fake.writes)[0]!
    expect(path.replace(/\\/g, '/')).toBe('C:/work/.cursor/cli.json')
    expect(JSON.parse(text).permissions.allow).toEqual(['Mcp(robinhood-local:*)'])
    expect(fake.dirs[0]!.replace(/\\/g, '/')).toBe('C:/work/.cursor')
  })

  it('does nothing with no servers, and does not write when nothing is new', async () => {
    const fake = io({})
    expect((await allowCursorConnectors('C:\\work', [], fake.io)).added).toEqual([])
    expect(fake.writes).toEqual({})
  })

  it('does not write over a file it could not read, and does not throw', async () => {
    // Joined as the code joins it, by this machine's rules: C:\work\.cursor\cli.json on Windows.
    const fake = io({ [join('C:\\work', '.cursor', 'cli.json')]: '{ broken' })
    expect((await allowCursorConnectors('C:\\work', ['a'], fake.io)).added).toEqual([])
    expect(fake.writes).toEqual({})
    const failing = { ...fake.io, readFile: async () => { throw Object.assign(new Error('EACCES'), { code: 'EACCES' }) } }
    expect((await allowCursorConnectors('C:\\work', ['a'], failing)).added).toEqual([])
  })
})
