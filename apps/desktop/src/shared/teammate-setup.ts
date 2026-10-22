/**
 * A TEAMMATE'S SETUP (0.811), after Harness²'s census of a harness: every piece that shapes a teammate's turns, read
 * fresh from the same places its brief and its run read them -- never a copy that could drift. Claude Code's own
 * `/status` says what a session runs with; `/setup` in a teammate's conversation says the same for the teammate, in
 * the order a turn is given them, and what its last turn here actually loaded.
 */
export const TEAMMATE_SETUP_CHANNEL = 'teammate:setup'

export interface TeammateSetup {
  readonly teammateId: string
  /** Where its turns stand: the project folder, its own folder, or a worktree of its own. */
  readonly folder: { readonly name: string; readonly kind: 'project' | 'own' | 'worktree' }
  /** The folder's LOCUST.md, as this runtime's teammates are given it. */
  readonly locustMd?: { readonly lines: number; readonly truncated: boolean }
  /** The runtime's own instruction file in that folder, which the runtime reads itself. */
  readonly runtimeFile?: { readonly name: string; readonly lines?: number }
  /** The group this conversation is in, when it has instructions. */
  readonly group?: { readonly name: string; readonly characters: number }
  readonly instructions: number
  readonly aboutYou: boolean
  readonly memory: { readonly on: boolean; readonly count: number }
  /** Claude Code, Codex and OpenCode (0.813, shared/skill-runtimes.ts): the skills a run is handed or finds. */
  readonly skills?: {
    readonly own: readonly string[]
    readonly project: number
    /** The folder's own skill folders this runtime reads, such as `.claude/skills`. */
    readonly projectFolders?: readonly string[]
    /**
     * The person's own: lent to Claude Code by Settings or Auto; found by Codex and OpenCode themselves, Settings or
     * not, as in the person's own sessions.
     */
    readonly personal: { readonly count: number; readonly lent: boolean; readonly foundItself?: boolean }
    readonly library: number
  }
  /** Claude Code only: the connectors it may use without asking; none means it works without them. */
  readonly connectors?: readonly string[]
  readonly lessons: number
  /** What the conversation's last turn loaded, as its runtime said when it started (claude-events.ts summarizeInit). */
  readonly lastTurn?: {
    readonly version?: string
    readonly tools?: number
    readonly skills?: number
    readonly mcpServers?: number
    readonly agents?: number
  }
}

export type TeammateSetupResponse =
  | { readonly ok: true; readonly data: TeammateSetup }
  | { readonly ok: false; readonly message: string }

export interface SetupRow { readonly label: string; readonly value: string }
export interface SetupSection { readonly title: string; readonly note: string; readonly rows: readonly SetupRow[] }

/** The runtime's own file, by the name each reads in a folder's root. Absent: Locust does not claim one. */
export const RUNTIME_FILE: Readonly<Record<string, string>> = { claude: 'CLAUDE.md', codex: 'AGENTS.md', opencode: 'AGENTS.md', gemini: 'GEMINI.md' }

const count = (n: number, one: string, many = `${one}s`): string => `${String(n)} ${n === 1 ? one : many}`

/** Folders in a sentence, joined both ways: "a, b and c" and "a, b or c". */
const listed = (folders: readonly string[]): { readonly and: string; readonly or: string } => {
  const join = (word: string): string => folders.length < 2 ? (folders[0] ?? '') : `${folders.slice(0, -1).join(', ')} ${word} ${folders.at(-1) ?? ''}`
  return { and: join('and'), or: join('or') }
}

/** The card's words, in the order a turn is given them. `runtime` names the runtime the way the window does. */
export function setupSections(setup: TeammateSetup, name: string, runtime: string): readonly SetupSection[] {
  const folder = setup.folder.kind === 'worktree'
    ? `A worktree of its own, from ${setup.folder.name}`
    : setup.folder.kind === 'own' ? `Its own folder, ${setup.folder.name}` : `The project folder, ${setup.folder.name}`
  const every: SetupRow[] = [
    { label: 'Folder', value: folder },
    {
      label: 'LOCUST.md',
      value: setup.locustMd === undefined
        ? 'None in this folder'
        : setup.locustMd.truncated ? `${count(setup.locustMd.lines, 'line')} read; the rest is cut` : count(setup.locustMd.lines, 'line')
    },
    ...(setup.runtimeFile === undefined ? [] : [{
      label: setup.runtimeFile.name,
      value: setup.runtimeFile.lines === undefined ? 'None in this folder' : `${count(setup.runtimeFile.lines, 'line')}, read by ${runtime} itself`
    }]),
    ...(setup.group === undefined ? [] : [{ label: 'Group', value: `${setup.group.name}’s instructions, ${count(setup.group.characters, 'character')}` }]),
    { label: 'Its instructions', value: setup.instructions === 0 ? 'None' : count(setup.instructions, 'character') },
    { label: 'About you', value: setup.aboutYou ? 'Your note, given to every teammate' : 'None' },
    { label: 'Team memory', value: setup.memory.on ? `On, ${count(setup.memory.count, 'memory', 'memories')} in this folder` : 'Off' }
  ]
  const sections: SetupSection[] = [{ title: 'Every turn', note: `What ${name} is given on every turn, in this order.`, rows: every }]
  if (setup.skills !== undefined || setup.connectors !== undefined) {
    const matched: SetupRow[] = []
    if (setup.skills !== undefined) {
      matched.push(
        { label: 'Its own skills', value: setup.skills.own.length === 0 ? 'None yet. A tune can give it one.' : setup.skills.own.join(', ') },
        (() => {
          const folders = listed(setup.skills.projectFolders ?? ['.claude/skills'])
          return { label: 'Project skills', value: setup.skills.project === 0 ? `None in ${folders.or}` : `${count(setup.skills.project, 'skill')} in ${folders.and}` }
        })(),
        {
          label: 'Your skills',
          value: setup.skills.personal.foundItself === true
            ? `${count(setup.skills.personal.count, 'skill')}, found by ${runtime} itself`
            : setup.skills.personal.lent ? count(setup.skills.personal.count, 'skill') : 'Not lent; Settings can lend them'
        },
        { label: 'Kept from GitHub', value: setup.skills.library === 0 ? 'None' : count(setup.skills.library, 'skill') }
      )
    }
    if (setup.connectors !== undefined) {
      matched.push({ label: 'Connectors', value: setup.connectors.length === 0 ? 'Off' : `On: ${setup.connectors.join(', ')}` })
    }
    sections.push({ title: 'When the work matches', note: `${runtime} loads a skill only when the work matches when to use it.`, rows: matched })
  }
  sections.push({
    title: 'Only when you tune it',
    note: 'Read by the next /tune of this teammate, never by its everyday turns.',
    rows: [{ label: 'Tune lessons', value: setup.lessons === 0 ? 'None' : count(setup.lessons, 'lesson') }]
  })
  const last = setup.lastTurn
  if (last !== undefined) {
    const loaded = [
      last.tools === undefined ? undefined : count(last.tools, 'tool'),
      last.skills === undefined ? undefined : count(last.skills, 'skill'),
      last.mcpServers === undefined ? undefined : count(last.mcpServers, 'connector server'),
      last.agents === undefined ? undefined : count(last.agents, 'agent')
    ].filter((part): part is string => part !== undefined)
    const rows: SetupRow[] = [
      ...(last.version === undefined ? [] : [{ label: 'Version', value: `${runtime} ${last.version}` }]),
      ...(loaded.length === 0 ? [] : [{ label: 'Loaded', value: loaded.join(', ') }])
    ]
    // Its count, not ours: the skills above and the runtime's own built-in ones together.
    if (rows.length > 0) sections.push({ title: 'Its last turn here', note: `As ${runtime} counted them when the turn started, its own built-in ones included.`, rows })
  }
  return sections
}
