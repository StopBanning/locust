import { join, sep } from 'node:path'

import { describe, expect, it } from 'vitest'

import { createCursorConnectorKeeper } from './cursor-connector-allow.js'
import type { AllowIo, HoldStore } from './cursor-connector-allow.js'

/*
 * CURSOR RULES LEFT BY A QUIT ARE TAKEN BACK (0.588).
 *
 * 0.584 gave a Cursor run's connector rules back when its process ended. A
 * run that was live when Locust quit or crashed never reached that, so its
 * `Mcp(server:*)` rules stayed in the folder's .cursor/cli.json and the next
 * run there -- in Ask, even -- had connector access it was never given; the
 * next Locust saw rules it did not add and left them. Found in the 0.587
 * code review. The keeper now writes what it holds to a store in the profile
 * and, at start, takes back what a store from an earlier Locust still names.
 */

// Joined as the keeper joins them, by this machine's rules: C:\work\.cursor\cli.json on Windows.
const DIR = join('C:\\work', '.cursor')
const FILE = join(DIR, 'cli.json')

/** A folder on disk, as the keeper sees it, and the profile's store. */
function disk(files: Record<string, string> = {}, dirs: string[] = [], storeText?: string) {
  const held: Record<string, string> = { ...files }
  const present = new Set(dirs)
  let store = storeText
  const io: AllowIo = {
    readFile: async (path) => {
      const text = held[path]
      if (text === undefined) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' })
      return text
    },
    writeFile: async (path, text) => {
      held[path] = text
    },
    mkdir: async (path) => {
      present.add(path)
    },
    exists: async (path) => present.has(path) || held[path] !== undefined,
    removeFile: async (path) => {
      delete held[path]
    },
    removeDir: async (path) => {
      if (Object.keys(held).some((file) => file.startsWith(`${path}${sep}`))) throw new Error('ENOTEMPTY')
      present.delete(path)
    }
  }
  const holds: HoldStore = {
    read: async () => store,
    write: async (text) => {
      store = text
    }
  }
  return { io, holds, files: held, dirs: present, store: () => store }
}

describe('what the keeper writes down', () => {
  it('names the file, how it looked before, and the rules this Locust added; given back, nothing', async () => {
    const fake = disk()
    const keeper = createCursorConnectorKeeper(fake.io, fake.holds)
    const grant = await keeper.allow('C:\\work', ['robinhood-local'])
    expect(grant.added).toEqual(['Mcp(robinhood-local:*)'])
    expect(JSON.parse(fake.store()!)).toEqual({
      version: 1,
      folders: { [FILE]: { original: { createdFile: true, createdDir: true, hadPermissions: false, hadAllow: false, hadDeny: false }, rules: ['Mcp(robinhood-local:*)'] } }
    })
    await grant.release()
    expect(JSON.parse(fake.store()!)).toEqual({ version: 1, folders: {} })
    expect(fake.files[FILE]).toBeUndefined()
  })

  it('writes nothing down for rules it did not add (control)', async () => {
    const fake = disk({ [FILE]: JSON.stringify({ permissions: { allow: ['Mcp(robinhood-local:*)'] } }) }, [DIR])
    const keeper = createCursorConnectorKeeper(fake.io, fake.holds)
    const grant = await keeper.allow('C:\\work', ['robinhood-local'])
    expect(grant.added).toEqual([])
    expect(fake.store()).toBeUndefined()
  })
})

describe('the next Locust, started after a quit mid-run', () => {
  it('takes back the rules the store names, and the file and folder the run made', async () => {
    // The first Locust: a run takes rules, then the process is gone before release.
    const first = disk()
    const earlier = createCursorConnectorKeeper(first.io, first.holds)
    await earlier.allow('C:\\work', ['robinhood-local'])
    expect(first.files[FILE]).toBeDefined()
    // The next Locust: same disk, same store, a new keeper that holds nothing.
    const next = createCursorConnectorKeeper(first.io, first.holds)
    expect(await next.recover()).toEqual([FILE])
    expect(first.files[FILE]).toBeUndefined()
    expect(first.dirs.has(DIR)).toBe(false)
    expect(JSON.parse(first.store()!)).toEqual({ version: 1, folders: {} })
    // Recovering again finds nothing to do.
    expect(await next.recover()).toEqual([])
  })

  it('leaves what the person had in the file, and what they added meanwhile', async () => {
    const before = { version: 1, permissions: { allow: ['Shell(ls)'], deny: ['Shell(rm)'] } }
    const first = disk({ [FILE]: JSON.stringify(before) }, [DIR])
    const earlier = createCursorConnectorKeeper(first.io, first.holds)
    await earlier.allow('C:\\work', ['robinhood-local'])
    // The person edits the file while the run is live.
    const during = JSON.parse(first.files[FILE]!) as { permissions: { allow: string[] } }
    during.permissions.allow.push('Shell(git status)')
    first.files[FILE] = JSON.stringify(during)
    const next = createCursorConnectorKeeper(first.io, first.holds)
    expect(await next.recover()).toEqual([FILE])
    expect(JSON.parse(first.files[FILE]!)).toEqual({ version: 1, permissions: { allow: ['Shell(ls)', 'Shell(git status)'], deny: ['Shell(rm)'] } })
  })

  it('does nothing with no store, an empty store, a store it cannot read, or a file already gone (controls)', async () => {
    expect(await createCursorConnectorKeeper(disk().io).recover()).toEqual([])
    expect(await createCursorConnectorKeeper(disk().io, disk().holds).recover()).toEqual([])
    const broken = disk({ [FILE]: JSON.stringify({ permissions: { allow: ['Mcp(a:*)'] } }) }, [DIR], '{ not json')
    expect(await createCursorConnectorKeeper(broken.io, broken.holds).recover()).toEqual([])
    expect(JSON.parse(broken.files[FILE]!).permissions.allow).toEqual(['Mcp(a:*)'])
    expect(broken.store()).toBe('{ not json')
    const gone = disk({}, [], JSON.stringify({ version: 1, folders: { [FILE]: { original: { createdFile: true }, rules: ['Mcp(a:*)'] } } }))
    expect(await createCursorConnectorKeeper(gone.io, gone.holds).recover()).toEqual([])
    expect(JSON.parse(gone.store()!)).toEqual({ version: 1, folders: {} })
  })

  it('never writes over a file that no longer reads as the config (control)', async () => {
    const odd = disk({ [FILE]: '{ broken' }, [DIR], JSON.stringify({ version: 1, folders: { [FILE]: { original: { createdFile: false }, rules: ['Mcp(a:*)'] } } }))
    expect(await createCursorConnectorKeeper(odd.io, odd.holds).recover()).toEqual([])
    expect(odd.files[FILE]).toBe('{ broken')
  })
})
