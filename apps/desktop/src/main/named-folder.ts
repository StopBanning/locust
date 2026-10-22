import { existsSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'

/*
 * A MESSAGE THAT NAMES ITS PROJECT STARTS THERE (0.814). Colin, 2026-10-22, on handing work to his teammates: "its
 * honestly a pain to have to switch it to some random fucking folder everytime so i just ignore you and send them the
 * handoff". So the handoffs ran in .claude, the window's folder, while the work they named sat in a project.
 *
 * A new conversation whose message names a file or folder inside a project -- a folder holding `.git`, a checkout --
 * starts in that project, as the folder menu would have switched to it, when the window stands where no project is
 * (home, .claude and the like: workspace.ts notATeammateFolder), or in another checkout of the same repository (a
 * worktree beside it). A message in a project that names some other repository is read where it is, as before: that
 * is a reference, not a move.
 */

/** At most this many paths are looked at in one message, and this many folders up from each. */
const MOST_PATHS = 12
const MOST_LEVELS = 64

export interface NamedFolderFiles {
  readonly exists: (path: string) => boolean
  readonly isDirectory: (path: string) => boolean
  readonly read: (path: string) => string
}

const realFiles: NamedFolderFiles = {
  exists: (path) => existsSync(path),
  isDirectory: (path) => {
    try {
      return statSync(path).isDirectory()
    } catch {
      return false
    }
  },
  read: (path) => readFileSync(path, 'utf8')
}

/**
 * The absolute paths a message names, without the quotes or end punctuation around them: first those in quotes or
 * backticks (which may hold spaces), then the bare ones. A drive letter counts only at the start of a word, so the
 * `s://` of a web address is not one.
 */
export function pathsNamedIn(text: string, platform: NodeJS.Platform = process.platform): string[] {
  const shapes = platform === 'win32'
    ? [/["'`]([A-Za-z]:[\\/][^"'`\r\n]*)["'`]/g, /(?<![\w"'`])([A-Za-z]:[\\/][^\s"'`<>|*?]*)/g]
    : [/["'`](\/[^"'`\r\n]+)["'`]/g, /(?<![\w:./~"'`])(\/[^\s"'`<>|*?]+)/g]
  const found: string[] = []
  for (const shape of shapes) {
    for (const match of text.matchAll(shape)) {
      const path = (match[1] ?? '').replace(/[.,;:!?)\]}]+$/, '')
      if (path.length > 3 && !found.includes(path)) found.push(path)
      if (found.length === MOST_PATHS) return found
    }
  }
  return found
}

/** The checkout a path is inside: the nearest folder at or above it that holds `.git`. */
export function checkoutOf(path: string, files: NamedFolderFiles = realFiles): string | undefined {
  if (!files.exists(path)) return undefined
  let folder = files.isDirectory(path) ? resolve(path) : dirname(resolve(path))
  for (let level = 0; level < MOST_LEVELS; level += 1) {
    if (files.exists(join(folder, '.git'))) return folder
    const up = dirname(folder)
    if (up === folder) return undefined
    folder = up
  }
  return undefined
}

/**
 * The repository a checkout belongs to: its `.git` folder, or for a worktree (whose `.git` is a file naming
 * `<repository>/.git/worktrees/<name>`) the `.git` folder it shares with the main checkout.
 */
export function repositoryOf(checkout: string, files: NamedFolderFiles = realFiles): string | undefined {
  const dotGit = join(checkout, '.git')
  if (files.isDirectory(dotGit)) return resolve(dotGit)
  try {
    const named = /^gitdir:\s*(.+)$/m.exec(files.read(dotGit))?.[1]?.trim()
    if (named === undefined || named.length === 0) return undefined
    const gitDir = resolve(checkout, named)
    return /[\\/]worktrees[\\/][^\\/]+[\\/]?$/.test(gitDir) ? resolve(gitDir, '..', '..') : gitDir
  } catch {
    return undefined
  }
}

export interface NamedFolderContext {
  /** The window's folder now. */
  readonly current: string
  /** A folder no project is in -- home, .claude, Windows' own, Locust's install -- never moved to. */
  readonly isNoProject: (folder: string) => boolean
  readonly same: (one: string, other: string) => boolean
  readonly platform?: NodeJS.Platform
  readonly files?: NamedFolderFiles
}

/** The project a new conversation's message names, to start it in; undefined to start where the window is. */
export function projectNamedIn(text: string, context: NamedFolderContext): string | undefined {
  const files = context.files ?? realFiles
  const nowhere = context.isNoProject(context.current)
  const here = nowhere ? undefined : checkoutOf(context.current, files)
  if (!nowhere && here === undefined) return undefined
  const ours = here === undefined ? undefined : repositoryOf(here, files)
  for (const path of pathsNamedIn(text, context.platform)) {
    const project = checkoutOf(path, files)
    if (project === undefined || context.isNoProject(project)) continue
    if (context.same(project, context.current) || (here !== undefined && context.same(project, here))) continue
    if (nowhere) return project
    const theirs = repositoryOf(project, files)
    if (ours !== undefined && theirs !== undefined && context.same(ours, theirs)) return project
  }
  return undefined
}
