import { readFile } from 'node:fs/promises'
import { basename, join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { PublicTeammate } from '../shared/ipc.js'
import { SKILL_FOLDERS, takesSkills } from '../shared/skill-runtimes.js'
import { RUNTIME_FILE, type TeammateSetup } from '../shared/teammate-setup.js'
import { skillFoldersIn } from './claude-skills.js'
import { readWorkspaceBrief } from './workspace-brief.js'

/**
 * A TEAMMATE'S SETUP (0.811, shared/teammate-setup.ts), gathered from the same sources its brief and its run read:
 * the folder's LOCUST.md as this runtime is given it, the runtime's own file, the group, the teammate's instructions,
 * About you, memory, the skills a Claude Code run is handed or finds, its connectors, its tune lessons, and what the
 * conversation's last turn said it loaded. Nothing here is written, and nothing is started.
 */
export interface SetupSources {
  readonly teammate: PublicTeammate
  /** The folder its turns stand in when it has none of its own: the window's. */
  readonly projectPath: string
  readonly group?: { readonly name: string; readonly instructions: string }
  readonly settings: { readonly aboutYou?: string; readonly memoryMode: string; readonly claudeOwnSkills?: boolean }
  readonly memoryCount: (folder: string) => Promise<number>
  /** The names in the teammate's own skill record (teammate-skills.ts). */
  readonly ownSkills: () => Promise<readonly string[]>
  readonly lessons: () => Promise<number>
  readonly libraryFolder: string
  readonly homeDirectory: string
  /** The conversation's last turn, when one is open. */
  readonly lastTurn?: readonly NormalizedRuntimeEvent[]
  readonly read?: (path: string) => Promise<string>
}

const lineCount = (text: string): number => {
  const lines = text.replace(/\r\n?/g, '\n').split('\n')
  while (lines.length > 0 && lines[lines.length - 1] === '') lines.pop()
  return lines.length
}

/** What a run said it loaded as it started: the counts claude-events.ts keeps in place of the names. */
export function lastTurnOf(events: readonly NormalizedRuntimeEvent[] | undefined): TeammateSetup['lastTurn'] {
  const started = events?.find((event) => event.type === 'run.started')
  const raw = (started?.payload as { readonly evidence?: { readonly raw?: unknown } } | undefined)?.evidence?.raw
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return undefined
  const row = raw as Record<string, unknown>
  const number = (key: string): number | undefined => (typeof row[key] === 'number' && Number.isInteger(row[key]) && (row[key] as number) >= 0 ? (row[key] as number) : undefined)
  const version = typeof row.claude_code_version === 'string' && /^[\w.+-]{1,40}$/.test(row.claude_code_version) ? row.claude_code_version : undefined
  const last = {
    ...(version === undefined ? {} : { version }),
    ...(number('toolCount') === undefined ? {} : { tools: number('toolCount')! }),
    ...(number('skillCount') === undefined ? {} : { skills: number('skillCount')! }),
    ...(number('mcpServerCount') === undefined ? {} : { mcpServers: number('mcpServerCount')! }),
    ...(number('agentCount') === undefined ? {} : { agents: number('agentCount')! })
  }
  return Object.keys(last).length === 0 ? undefined : last
}

export async function gatherTeammateSetup(sources: SetupSources): Promise<TeammateSetup> {
  const { teammate } = sources
  const read = sources.read ?? ((path: string) => readFile(path, 'utf8'))
  const runtime = teammate.route?.runtime
  const folderPath = teammate.folder ?? sources.projectPath
  const folder = {
    name: basename(folderPath) || folderPath,
    kind: teammate.worktree === true ? 'worktree' as const : teammate.folder !== undefined ? 'own' as const : 'project' as const
  }
  const brief = await readWorkspaceBrief(folderPath, read, runtime).catch(() => undefined)
  const fileName = runtime === undefined ? undefined : RUNTIME_FILE[runtime]
  const fileText = fileName === undefined ? undefined : await read(join(folderPath, fileName)).catch(() => undefined)
  const memoryOn = sources.settings.memoryMode !== 'off'
  const claude = runtime === 'claude'
  // The folders this runtime reads skills from, the folder's and the person's (0.813, shared/skill-runtimes.ts).
  const folders = runtime !== undefined && takesSkills(runtime) ? SKILL_FOLDERS[runtime as keyof typeof SKILL_FOLDERS] : undefined
  const namesIn = async (roots: readonly string[]): Promise<number> =>
    new Set((await Promise.all(roots.map((root) => skillFoldersIn(root).catch(() => [] as readonly string[])))).flat()).size
  const personal = folders === undefined ? 0 : await namesIn(folders.personal.map((path) => join(sources.homeDirectory, ...path.split('/'))))
  // Auto runs as the person, and Claude Code finds their own skills itself (claude-skills.ts); Codex and OpenCode always do.
  const lent = !claude || sources.settings.claudeOwnSkills === true || teammate.route?.mode === 'auto'
  const lastTurn = lastTurnOf(sources.lastTurn)
  return {
    teammateId: teammate.teammateId,
    folder,
    ...(brief === undefined ? {} : { locustMd: { lines: brief.lines, truncated: brief.truncated } }),
    ...(fileName === undefined ? {} : { runtimeFile: { name: fileName, ...(fileText === undefined ? {} : { lines: lineCount(fileText) }) } }),
    ...(sources.group === undefined || sources.group.instructions.length === 0 ? {} : { group: { name: sources.group.name, characters: sources.group.instructions.length } }),
    instructions: (teammate.instructions ?? '').length,
    aboutYou: sources.settings.aboutYou !== undefined && sources.settings.aboutYou.trim().length > 0,
    memory: { on: memoryOn, count: memoryOn ? await sources.memoryCount(folderPath).catch(() => 0) : 0 },
    ...(folders === undefined
      ? {}
      : {
          skills: {
            own: await sources.ownSkills().catch(() => []),
            project: await namesIn(folders.project.map((path) => join(folderPath, ...path.split('/')))),
            projectFolders: folders.project,
            personal: { count: personal, lent, ...(claude ? {} : { foundItself: true as const }) },
            library: (await skillFoldersIn(sources.libraryFolder)).length
          }
        }),
    // Connectors are Claude Code's alone here: the ones it may use without asking.
    ...(claude ? { connectors: teammate.connectors ?? [] } : {}),
    lessons: await sources.lessons().catch(() => 0),
    ...(lastTurn === undefined ? {} : { lastTurn })
  }
}
