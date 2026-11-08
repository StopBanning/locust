import { realpath, stat } from 'node:fs/promises'
import { isAbsolute, relative, resolve } from 'node:path'

/**
 * A FOLDER AN APP NAMES FOR A TURN (0.869).
 *
 * Colin, 2026-11-07: "mcp should allow main agent to choose perms honestly". An app had handed a Codex teammate
 * worktrees in ~/work/locust-wt, and every turn stood in the folder Locust had open, so its writes there were refused
 * ("Operation not permitted") and nothing changed.
 *
 * An app may name a folder Locust already knows -- the one it has open, one it has worked in, a teammate's own -- or
 * one inside it, or a worktree of the same repository as one of them (a `git worktree add` lands anywhere). Nothing
 * else: the app is a guest, and a folder the person never pointed Locust at is not its to open.
 */
export interface AppFolderChecks {
  /** The folders Locust knows, read fresh. */
  readonly known: () => Promise<readonly string[]>
  /** The repository a folder belongs to, as git's common directory, absolute; undefined outside one. */
  readonly commonDir: (folder: string) => Promise<string | undefined>
}

const inside = (folder: string, base: string): boolean => {
  const way = relative(base, folder)
  return way === '' || (!way.startsWith('..') && !isAbsolute(way))
}

export async function checkAppFolder(requested: string, checks: AppFolderChecks): Promise<{ readonly path: string } | { readonly refused: string }> {
  if (!isAbsolute(requested)) return { refused: 'folder is a full path, from the top of the disk.' }
  let path: string
  try {
    path = await realpath(resolve(requested))
    if (!(await stat(path)).isDirectory()) return { refused: `${requested} is a file, not a folder.` }
  } catch {
    return { refused: `There is no folder at ${requested}.` }
  }
  const known = (await Promise.all((await checks.known().catch(() => [] as readonly string[])).map(async (folder) => realpath(folder).catch(() => undefined))))
    .filter((folder): folder is string => folder !== undefined)
  if (known.some((base) => inside(path, base))) return { path }
  const own = await checks.commonDir(path).catch(() => undefined)
  if (own !== undefined) {
    for (const base of known) {
      const theirs = await checks.commonDir(base).catch(() => undefined)
      if (theirs !== undefined && resolve(theirs) === resolve(own)) return { path }
    }
  }
  return { refused: `${path} is not a folder Locust knows: only the folder it has open, one it worked in, a teammate's own, a folder inside one, or a worktree of one of their repositories.` }
}
