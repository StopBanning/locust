import { readFileSync, statSync } from 'node:fs'
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises'
import { dirname, isAbsolute, resolve, win32, posix } from 'node:path'

import { folderRemote } from './restored-folders.js'
import { workspaceIdFor } from './workspace.js'
import { replaceFile } from './replace-file.js'

/**
 * EVERY FOLDER LOCUST HAS WORKED IN, BY ID AND PATH (0.458).
 *
 * Colin, 2026-09-29: "Just make it work exactly like Claude code ... Claude
 * allows you to switch work folder but also has projects grouped." A
 * conversation belongs to a folder, and the sidebar lists every folder's
 * conversations under its name (docs/PLAN-2026-09-29-FOLDERS-LIKE-CLAUDE-CODE.md).
 *
 * A mission records its folder only as `workspaceId` -- a hash of the path --
 * so the path has to be kept somewhere to continue a conversation there. This
 * is that somewhere: `folders.json`, written whenever a folder is worked in.
 * A folder from before it existed is recovered from the conversation's own
 * record (`recoverFolderPath`).
 */

/** A folder by path, with the id and name memory and the brief know it by. */
export interface FolderContext {
  readonly path: string
  readonly id: string
  readonly name: string
}

export interface KnownFolder {
  readonly id: string
  readonly path: string
  /** The folder's own name, as the sidebar heads its group. */
  readonly name: string
  /** Original path before an explicit move to another computer. */
  readonly previousPath?: string
  /** Origin URL observed while the folder was available on this computer. */
  readonly remote?: string
  /** When a conversation was last started or opened there; absent for one only recovered. */
  readonly lastUsedAt?: string
}

interface FoldersFile {
  readonly schemaVersion: 1
  readonly folders: readonly KnownFolder[]
}

export function folderName(path: string): string {
  return win32.basename(path) || posix.basename(path) || path
}

/**
 * A folder's path, from any absolute paths a conversation's record holds.
 *
 * The id is a hash of the folder's path, so whichever of those paths -- or a
 * folder above one of them -- hashes to it IS the folder. A run's tool calls
 * name files by absolute path often enough that this found 5 of the 6 folders
 * in Colin's history (2026-09-29). Nothing is guessed: no match, no path.
 */
export function recoverFolderPath(id: string, text: string): string | undefined {
  const tried = new Set<string>()
  for (const found of text.matchAll(/[A-Za-z]:(?:\\\\|\\|\/)[^"\s<>|*?\r\n]+|\/(?:Users|home|Volumes|mnt|opt|srv|tmp|var)\/[^"\s<>|*?\r\n]+/g)) {
    let path = found[0].replace(/\\\\/g, '\\')
    if (/^[A-Za-z]:/.test(path)) path = path.replace(/\//g, '\\')
    for (;;) {
      if (!tried.has(path)) {
        tried.add(path)
        if (workspaceIdFor(path) === id) return path
      }
      const parent = /^[A-Za-z]:/.test(path) ? windowsParent(path) : dirname(path)
      if (parent === undefined || parent === path) break
      path = parent
    }
  }
  return undefined
}

function windowsParent(path: string): string | undefined {
  const cut = path.replace(/\\+$/, '').lastIndexOf('\\')
  if (cut <= 2) return undefined
  return path.slice(0, cut)
}

export interface FolderRegistry {
  move(id: string, path: string): Promise<void>
  list(): Promise<readonly KnownFolder[]>
  pathOf(id: string): Promise<string | undefined>
  /** A folder worked in now: kept, and marked as just used. */
  use(path: string): Promise<KnownFolder>
  /** Folders found, not used: kept without touching when each was last used. */
  learn(found: readonly { readonly id: string; readonly path: string }[]): Promise<void>
  /** The known folder this path is on disk, as it was first known; the path itself when none is. */
  sameAs(path: string): Promise<string>
}

/**
 * `sameAs` before the registry is open, for the folder the app starts in:
 * the start runs before anything is awaited. Reads the registry file itself.
 */
export function sameFolderAtStart(file: string, path: string): string {
  const full = resolve(path)
  let known: readonly string[] = []
  try {
    const body = JSON.parse(readFileSync(file, 'utf8')) as Partial<FoldersFile>
    known = (Array.isArray(body.folders) ? body.folders : []).flatMap((entry) =>
      typeof entry === 'object' && entry !== null && typeof entry.path === 'string' && (isAbsolute(entry.path) || win32.isAbsolute(entry.path) || posix.isAbsolute(entry.path)) ? [entry.path] : []
    )
  } catch {
    return full
  }
  if (known.includes(full)) return full
  const identity = identityAtStart(full)
  if (identity === undefined) return full
  return known.find((entry) => identityAtStart(entry) === identity) ?? full
}

function identityAtStart(path: string): string | undefined {
  try {
    const found = statSync(path, { bigint: true })
    return found.isDirectory() && found.ino !== 0n ? `${String(found.dev)}:${String(found.ino)}` : undefined
  } catch {
    return undefined
  }
}

/** A folder on disk, whatever it was called: its volume and file number. */
async function identityOf(path: string): Promise<string | undefined> {
  try {
    const found = await stat(path, { bigint: true })
    return found.isDirectory() && found.ino !== 0n ? `${String(found.dev)}:${String(found.ino)}` : undefined
  } catch {
    return undefined
  }
}

export function createFolderRegistry(options: { readonly file: string; readonly now?: () => Date }): FolderRegistry {
  const now = options.now ?? (() => new Date())
  let held: KnownFolder[] | undefined
  const read = async (): Promise<KnownFolder[]> => {
    if (held !== undefined) return held
    try {
      const parsed = JSON.parse(await readFile(options.file, 'utf8')) as Partial<FoldersFile>
      held = (Array.isArray(parsed.folders) ? parsed.folders : []).filter(
        (entry): entry is KnownFolder =>
          typeof entry === 'object' && entry !== null && typeof entry.id === 'string' && typeof entry.path === 'string' && (isAbsolute(entry.path) || win32.isAbsolute(entry.path) || posix.isAbsolute(entry.path))
      ).map((entry) => ({ ...entry, name: folderName(entry.path) }))
    } catch {
      held = []
    }
    return held
  }
  // One write at a time, whole or not at all.
  let writing = Promise.resolve()
  const save = (folders: KnownFolder[], strict = false): Promise<void> => {
    held = folders
    writing = writing.then(async () => {
      await mkdir(dirname(options.file), { recursive: true })
      const body: FoldersFile = { schemaVersion: 1, folders }
      await writeFile(`${options.file}.tmp`, `${JSON.stringify(body, null, 2)}\n`, 'utf8')
      await replaceFile(`${options.file}.tmp`, options.file)
    })
    const result = writing
    writing = result.catch(() => undefined)
    return strict ? result.catch(error => { held = undefined; throw error }) : writing
  }
  return {
    async move(id, path) {
      const current = await read()
      const old = current.find(entry => entry.id === id)
      if (old === undefined) throw new Error('Unknown folder')
      const full = resolve(path)
      if (await identityOf(full) === undefined) throw new Error('Choose an existing folder')
      await save(current.map(entry => entry.id === id ? { ...entry, previousPath: entry.previousPath ?? entry.path, path: full, name: folderName(full) } : entry), true)
    },
    list: async () => [...(await read())].sort((a, b) => (b.lastUsedAt ?? '').localeCompare(a.lastUsedAt ?? '')),
    pathOf: async (id) => (await read()).find((entry) => entry.id === id)?.path,
    async use(path) {
      const full = resolve(path)
      const remote = await folderRemote(full)
      const entry: KnownFolder = { ...(remote === undefined ? {} : { remote }), id: workspaceIdFor(full), path: full, name: folderName(full), lastUsedAt: now().toISOString() }
      await save([...(await read()).filter((one) => one.id !== entry.id), entry])
      return entry
    },
    /*
     * ONE FOLDER, HOWEVER IT IS SPELLED (QA-2026-09-29 round 2, N13). The id
     * hashes the path as written, so `c:\work` and `C:\work`, or a link to
     * a project, were a second folder: its own sidebar group, none of the
     * conversations, the history looking gone. A path that is a known folder
     * on disk -- the same volume and file number -- is that folder, spelled
     * the way it was first known. Ids never change: every record keeps its
     * folder.
     */
    async sameAs(path) {
      const known = await read()
      const moved = known.find(entry => entry.previousPath === path)
      if (moved !== undefined) return moved.path
      const full = resolve(path)
      if (known.some((entry) => entry.path === full)) return full
      const identity = await identityOf(full)
      if (identity === undefined) return full
      for (const entry of known) {
        if ((await identityOf(entry.path)) === identity) return entry.path
      }
      return full
    },
    async learn(found) {
      const current = await read()
      const fresh = found.filter((one) => !current.some((entry) => entry.id === one.id))
      if (fresh.length === 0) return
      await save([...current, ...fresh.map((one) => ({ id: one.id, path: one.path, name: folderName(one.path) }))])
    }
  }
}
