import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { GitReadsSpec } from '@teammate/runtime-adapters'

import { OPENCODE_GIT_TOOL_RULE } from './workroom-briefing.js'

/**
 * Locust's git tool for an OpenCode run with no shell (0.818, commands.ts withOpenCodeGitReads): its server,
 * resources/locust-git-reads.mjs, run by Electron's own binary as node, and the instructions file that tells every
 * agent of the run to use it, written once at start in the profile. Undefined if that file could not be written:
 * such a run goes without the tool and is told only that its shell is off.
 */
export async function writeOpenCodeGitReads(folder: string, options: { readonly node: string; readonly scriptPath: string }): Promise<GitReadsSpec | undefined> {
  try {
    await mkdir(folder, { recursive: true })
    const instructions = join(folder, 'opencode-git-tool.md')
    await writeFile(instructions, `${OPENCODE_GIT_TOOL_RULE}\n`, 'utf8')
    return { command: options.node, args: [options.scriptPath], env: { ELECTRON_RUN_AS_NODE: '1' }, instructions }
  } catch {
    return undefined
  }
}
