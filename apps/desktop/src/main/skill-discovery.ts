import { lstat, readFile, readdir, realpath, stat } from 'node:fs/promises'
import { basename, dirname, join, resolve } from 'node:path'
import { parseDocument } from 'yaml'

/** Use the declared name for collisions even when the directory's spelling differs. */
export function skillNameIn(text: string): string | undefined {
  const front = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(text)?.[1]
  if (front === undefined) return undefined
  const document = parseDocument(front)
  if (document.errors.length > 0) return undefined
  const name: unknown = document.get('name')
  return typeof name === 'string' && name.trim().length > 0 ? name.trim() : undefined
}

export const skillNameKey = (name: string, platform: NodeJS.Platform = process.platform): string =>
  platform === 'win32' ? name.toLowerCase() : name

/** CWD first, stopping at its git worktree. A .git file counts too. Nothing in .git is read. */
export async function skillProjectFolders(folder: string): Promise<readonly string[]> {
  const folders: string[] = []
  for (let at = resolve(folder); ; at = dirname(at)) {
    folders.push(at)
    if (await lstat(join(at, '.git')).then(() => true, () => false) || dirname(at) === at) break
  }
  return folders
}

/** A census, not a copy: native discovery follows linked skill folders and nested groups. */
export async function countedSkillNames(roots: readonly string[], platform: NodeJS.Platform = process.platform): Promise<ReadonlySet<string>> {
  const names = new Set<string>()
  const seen = new Set<string>()
  const pending = roots.map((path) => ({ path, depth: 0 }))
  while (pending.length > 0) {
    const next = pending.pop()!
    try {
      const path = await realpath(next.path)
      if (seen.has(path)) continue
      seen.add(path)
      const text = await readFile(join(path, 'SKILL.md'), 'utf8').catch(() => undefined)
      if (text !== undefined) {
        const name = skillNameIn(text) ?? basename(next.path)
        names.add(skillNameKey(name, platform))
        continue
      }
      if (next.depth >= 6) continue
      for (const entry of await readdir(path, { withFileTypes: true })) {
        if (entry.name.startsWith('.') || entry.name === 'node_modules') continue
        const child = join(path, entry.name)
        if (entry.isDirectory() || (entry.isSymbolicLink() && await stat(child).then((info) => info.isDirectory(), () => false))) {
          pending.push({ path: child, depth: next.depth + 1 })
        }
      }
    } catch {
      // Missing, broken or unreadable links do not stand for a skill. Cycles stop at realpath's seen set.
    }
  }
  return names
}
