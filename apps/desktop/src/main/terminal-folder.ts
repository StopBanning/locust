import type { TerminalContext } from '../shared/terminal.js'
import { createWorktreeManager } from './worktrees.js'

/** Resolve identities to host-known folders. Never create a worktree just to open a shell. */
export async function terminalFolder(context: TerminalContext, options: {
  readonly current: () => string | undefined
  readonly mission: (id: string) => Promise<{ readonly workspaceId: string } | undefined>
  readonly folderOf: (id: string) => Promise<string | undefined>
  readonly ownerOf: (id: string) => Promise<string | undefined>
  readonly teammates: () => Promise<readonly { readonly teammateId: string; readonly folder?: string; readonly worktree?: boolean }[]>
  readonly trees?: (folder: string) => Promise<readonly { readonly teammateId: string; readonly path: string }[]>
}): Promise<string | undefined> {
  let folder = options.current()
  let owner = context.teammateId
  if (context.missionId !== undefined) {
    const mission = await options.mission(context.missionId)
    if (mission === undefined) return undefined
    folder = await options.folderOf(mission.workspaceId)
    owner = await options.ownerOf(context.missionId)
  }
  const teammate = owner === undefined ? undefined : (await options.teammates()).find(one => one.teammateId === owner)
  if (owner !== undefined && teammate === undefined) return undefined
  const root = teammate?.folder ?? folder
  if (root === undefined) return undefined
  if (teammate?.worktree !== true) return root
  const trees = await (options.trees ?? (path => createWorktreeManager({ workspacePath: path }).list()))(root)
  return trees.find(tree => tree.teammateId === teammate.teammateId)?.path
}
