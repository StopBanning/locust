import { createHash } from 'node:crypto'
import { stat } from 'node:fs/promises'
import { join } from 'node:path'

import { gitText } from './github-issues.js'

/*
 * WHERE THE FOLDER STANDS, FOR SCOUT'S CHECK (0.815): its last commit, and each changed or new file's size and time.
 * Two looks that find the same mark have nothing new for the check to read, so it is not run; a commit, or a save to
 * any changed or new file, moves it. A folder that is not a git checkout has no mark, and Scout runs no check there.
 */
const MOST_FILES = 200

export async function folderMarkOf(folder: string, git: (args: readonly string[], cwd: string) => Promise<string> = gitText): Promise<string | undefined> {
  const head = await git(['rev-parse', 'HEAD'], folder).catch(() => undefined)
  if (head === undefined || head.trim().length === 0) return undefined
  const status = await git(['status', '--porcelain', '-z'], folder).catch(() => '')
  // A rename's second name follows it with no status of its own: only entries that carry one are files to look at.
  const entries = status.split('\0').filter((entry) => /^[ MADRCUT?!]{2} ./.test(entry)).slice(0, MOST_FILES)
  const touched = await Promise.all(entries.map(async (entry) => {
    const info = await stat(join(folder, entry.slice(3))).catch(() => undefined)
    return `${entry}\t${info === undefined ? '-' : `${String(info.size)}:${String(Math.round(info.mtimeMs))}`}`
  }))
  return createHash('sha256').update([head.trim(), ...touched].join('\n')).digest('hex').slice(0, 32)
}
