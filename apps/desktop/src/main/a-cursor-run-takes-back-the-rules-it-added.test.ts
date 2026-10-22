import { describe, expect, it } from 'vitest'

import { type AllowIo, createCursorConnectorKeeper } from './cursor-connector-allow.js'

/**
 * A Cursor run outside Auto is given `Mcp(<server>:*)` rules in the folder's
 * `.cursor/cli.json`. They used to stay for good: a later Ask run in the same
 * folder inherited connector access it was never given, and the folder kept a
 * file the person did not write. Now the run gives back exactly what it added
 * when it ends.
 */

const FILE = 'C:/work/.cursor/cli.json'
const DIR = 'C:/work/.cursor'

/** The keeper joins paths the platform's way; the fake reads them one way. */
const at = (path: string): string => path.replaceAll('\\', '/')

/** A folder in memory: files by path, directories by path. */
function folder(files: Record<string, string> = {}, dirs: readonly string[] = []) {
  const held = new Map(Object.entries(files))
  const made = new Set(dirs)
  const io: AllowIo = {
    readFile: async (path) => {
      const text = held.get(at(path))
      if (text === undefined) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' })
      return text
    },
    writeFile: async (path, text) => {
      held.set(at(path), text)
    },
    mkdir: async (path) => {
      made.add(at(path))
    },
    exists: async (path) => made.has(at(path)) || held.has(at(path)),
    removeFile: async (path) => {
      held.delete(at(path))
    },
    removeDir: async (path) => {
      if ([...held.keys()].some((file) => file.startsWith(`${at(path)}/`))) throw new Error('ENOTEMPTY')
      made.delete(at(path))
    }
  }
  const read = (): Record<string, unknown> | undefined => {
    const text = held.get(FILE)
    return text === undefined ? undefined : (JSON.parse(text) as Record<string, unknown>)
  }
  const write = (value: unknown): void => {
    held.set(FILE, `${JSON.stringify(value, null, 2)}\n`)
  }
  return { io, held, made, read, write }
}

const keeper = (io: AllowIo) => createCursorConnectorKeeper(io)

describe('a Cursor run takes back the rules it added', () => {
  it('leaves the rules that were already in the file, and its own other settings', async () => {
    const before = {
      version: 1,
      permissions: { allow: ['Shell(ls)', 'Mcp(robinhood-trading:get_accounts)', 'Mcp(a:*)'], deny: ['Shell(rm)'] }
    }
    const fs = folder({ [FILE]: `${JSON.stringify(before, null, 2)}\n` }, [DIR])
    const grant = await keeper(fs.io).allow('C:/work', ['a', 'robinhood-trading'])
    // `a` was the person's own rule: only the other one is this run's.
    expect(grant.added).toEqual(['Mcp(robinhood-trading:*)'])
    expect((fs.read()!.permissions as { allow: string[] }).allow).toContain('Mcp(robinhood-trading:*)')
    await grant.release()
    expect(fs.read()).toEqual(before)
  })

  it('leaves a rule the person added while the run was going', async () => {
    const fs = folder({}, [])
    const grant = await keeper(fs.io).allow('C:/work', ['a'])
    const during = fs.read() as { permissions: { allow: string[]; deny: string[] } }
    fs.write({ ...during, permissions: { allow: [...during.permissions.allow, 'Shell(git status)'], deny: ['Shell(rm)'] } })
    await grant.release()
    expect(fs.read()).toEqual({ permissions: { allow: ['Shell(git status)'], deny: ['Shell(rm)'] } })
  })

  it('never touches deny', async () => {
    const fs = folder({ [FILE]: JSON.stringify({ permissions: { allow: [], deny: ['Mcp(a:*)'] } }) }, [DIR])
    const grant = await keeper(fs.io).allow('C:/work', ['b'])
    await grant.release()
    expect((fs.read()!.permissions as { deny: string[] }).deny).toEqual(['Mcp(a:*)'])
  })

  it('keeps a rule until the last of two overlapping runs ends', async () => {
    const fs = folder({}, [])
    const both = keeper(fs.io)
    const first = await both.allow('C:/work', ['a'])
    const second = await both.allow('C:/work', ['a', 'b'])
    // The second run found `a` already there (the first run's), and added `b`.
    expect(second.added).toEqual(['Mcp(b:*)'])
    await first.release()
    // The second run is still going and still needs `a`.
    expect((fs.read()!.permissions as { allow: string[] }).allow).toEqual(['Mcp(a:*)', 'Mcp(b:*)'])
    await second.release()
    expect(fs.read()).toBeUndefined()
  })

  // 0.825: a Cursor turn in Ask or Plan is not started in a folder whose rules a live run holds.
  it('says whether a live run holds rules in a folder, until the last one ends', async () => {
    const fs = folder({}, [])
    const both = keeper(fs.io)
    expect(both.holds('C:/work')).toBe(false)
    const first = await both.allow('C:/work', ['a'])
    const second = await both.allow('C:/work', ['a'])
    expect(both.holds('C:/work')).toBe(true)
    expect(both.holds('C:/elsewhere')).toBe(false)
    await first.release()
    expect(both.holds('C:/work')).toBe(true)
    await second.release()
    expect(both.holds('C:/work')).toBe(false)
    // A rule the person wrote themselves is theirs: no run holds it.
    fs.write({ permissions: { allow: ['Mcp(a:*)'] } })
    const theirs = await both.allow('C:/work', ['a'])
    expect(theirs.added).toEqual([])
    expect(both.holds('C:/work')).toBe(false)
  })

  it('keeps the rule when the run that added it ends first, and the other is still going', async () => {
    const fs = folder({}, [])
    const both = keeper(fs.io)
    const first = await both.allow('C:/work', ['a'])
    const second = await both.allow('C:/work', ['a'])
    await first.release()
    expect((fs.read()!.permissions as { allow: string[] }).allow).toEqual(['Mcp(a:*)'])
    await second.release()
    expect(fs.read()).toBeUndefined()
  })

  it('leaves a file that has stopped parsing exactly as it is', async () => {
    const fs = folder({}, [])
    const grant = await keeper(fs.io).allow('C:/work', ['a'])
    fs.held.set(FILE, '{ the person is half way through typing')
    await grant.release()
    expect(fs.held.get(FILE)).toBe('{ the person is half way through typing')
  })

  it('leaves no file, and no folder, when the run created them', async () => {
    const fs = folder({}, [])
    const grant = await keeper(fs.io).allow('C:/work', ['a', 'b'])
    expect(fs.read()).toBeDefined()
    expect(fs.made.has(DIR)).toBe(true)
    await grant.release()
    expect(fs.held.has(FILE)).toBe(false)
    expect(fs.made.has(DIR)).toBe(false)
  })

  it('keeps the folder, but not the file, when the folder was already there', async () => {
    const fs = folder({}, [DIR])
    const grant = await keeper(fs.io).allow('C:/work', ['a'])
    await grant.release()
    expect(fs.held.has(FILE)).toBe(false)
    expect(fs.made.has(DIR)).toBe(true)
  })

  it('keeps a file the run created if the person put something else in it meanwhile', async () => {
    const fs = folder({}, [])
    const grant = await keeper(fs.io).allow('C:/work', ['a'])
    fs.write({ editor: { vimMode: true }, permissions: { allow: ['Mcp(a:*)'], deny: [] } })
    await grant.release()
    expect(fs.read()).toEqual({ editor: { vimMode: true } })
  })

  it('does not take back a connector rule the person had already allowed, though the run asked for it too', async () => {
    const fs = folder({ [FILE]: JSON.stringify({ permissions: { allow: ['Mcp(a:*)'], deny: [] } }) }, [DIR])
    const grant = await keeper(fs.io).allow('C:/work', ['a', 'b'])
    expect(grant.added).toEqual(['Mcp(b:*)'])
    await grant.release()
    expect((fs.read()!.permissions as { allow: string[] }).allow).toEqual(['Mcp(a:*)'])
  })

  it('does nothing twice when the run is released twice', async () => {
    const fs = folder({}, [])
    const both = keeper(fs.io)
    const first = await both.allow('C:/work', ['a'])
    const second = await both.allow('C:/work', ['a'])
    await first.release()
    await first.release()
    // The second run still holds it: a second release by the first must not drop it.
    expect(fs.read()).toBeDefined()
    await second.release()
    expect(fs.read()).toBeUndefined()
  })

  it('does not mix up two folders', async () => {
    const fs = folder({}, [])
    const both = keeper(fs.io)
    const here = await both.allow('C:/work', ['a'])
    const there = await both.allow('C:/other', ['a'])
    await here.release()
    expect(fs.held.has(FILE)).toBe(false)
    expect(fs.held.has('C:/other/.cursor/cli.json')).toBe(true)
    await there.release()
    expect(fs.held.has('C:/other/.cursor/cli.json')).toBe(false)
  })
})

describe('what a later read-only run finds in the folder', () => {
  it('no connector rule, after an Accept-edits run in the same folder has ended', async () => {
    const fs = folder({ [FILE]: JSON.stringify({ permissions: { allow: ['Shell(ls)'], deny: [] } }) }, [DIR])
    const grant = await keeper(fs.io).allow('C:/work', ['robinhood-trading', 'robinhood-local'])
    expect(fs.held.get(FILE)).toContain('Mcp(')
    await grant.release()
    // An Ask run adds no rule of its own (0.550); what it inherits is this file.
    expect(fs.held.get(FILE)).not.toContain('Mcp(')
  })
})
