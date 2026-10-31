import { readdir, readFile, stat } from 'node:fs/promises'
import { join } from 'node:path'
import { folderName, type KnownFolder } from './folders.js'

export interface RestoredFolderMatch {
  readonly id: string
  readonly path: string
  readonly candidate?: string
}

export async function directoryExists(path: string): Promise<boolean> {
  return stat(path).then(entry => entry.isDirectory(), () => false)
}

/** Read configuration only: never run git or follow repository hooks during discovery. */
export async function folderRemote(path: string): Promise<string | undefined> {
  const text = await readFile(join(path, '.git', 'config'), 'utf8').catch(() => '')
  return text.match(/\[remote "origin"\][^[]*?\burl\s*=\s*([^\r\n]+)/)?.[1]?.trim()
}

function remoteKey(remote: string): string {
  return remote.trim().replace(/^git@([^:]+):/, 'https://$1/').replace(/^ssh:\/\/git@/, 'https://').replace(/\.git\/?$/, '').replace(/\/$/, '').toLowerCase()
}

/** A small, bounded scan. Links and hidden directories are never traversed. Ambiguity is left to the picker. */
export async function findRestoredFolders(folders: readonly KnownFolder[], home: string): Promise<readonly RestoredFolderMatch[]> {
  const missing: KnownFolder[] = []
  for (const folder of folders) if (!await directoryExists(folder.path)) missing.push(folder)
  if (missing.length === 0) return []
  const candidates: { path: string; remote?: string }[] = []
  const queue = ['work', 'Documents', 'Projects', 'code', 'src', 'Developer'].map(name => ({ path: join(home, name), depth: 0 }))
  let visited = 0
  while (queue.length > 0 && visited < 2000) {
    const next = queue.shift()!
    visited++
    const entries = await readdir(next.path, { withFileTypes: true }).catch(() => [])
    if (missing.some(folder => folderName(folder.path).toLowerCase() === folderName(next.path).toLowerCase()) || entries.some(entry => entry.name === '.git')) {
      const remote = await folderRemote(next.path)
      candidates.push({ path: next.path, ...(remote === undefined ? {} : { remote }) })
    }
    if (next.depth >= 3) continue
    for (const entry of entries) {
      if (queue.length + visited >= 2000) break
      if (entry.isDirectory() && !entry.name.startsWith('.') && !['node_modules', 'vendor', 'Library'].includes(entry.name)) queue.push({ path: join(next.path, entry.name), depth: next.depth + 1 })
    }
  }
  return missing.map(folder => {
    const byRemote = typeof folder.remote !== 'string' ? [] : candidates.filter(candidate => candidate.remote !== undefined && remoteKey(candidate.remote) === remoteKey(folder.remote!))
    const matches = typeof folder.remote === 'string' ? byRemote : candidates.filter(candidate => folderName(candidate.path).toLowerCase() === folderName(folder.path).toLowerCase())
    return { id: folder.id, path: folder.path, ...(matches.length === 1 ? { candidate: matches[0]!.path } : {}) }
  })
}
