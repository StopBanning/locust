import { readFileSync } from 'node:fs'
import { mkdir as mkdirAsync, mkdtemp as mkdtempAsync, rm as rmAsync, writeFile as writeFileAsync } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { seedAvatar } from '../shared/avatar.js'
import type { PublicTeammate } from '../shared/ipc.js'
import { setupSections, type TeammateSetup } from '../shared/teammate-setup.js'
import { gatherTeammateSetup, lastTurnOf } from './teammate-setup.js'

/*
 * A TEAMMATE'S SETUP (0.811). `/setup` says what shapes a teammate's turns, read fresh from the same places its brief
 * and its run read them, in the order a turn is given them -- after Harness²'s census, and as Claude Code's `/status`
 * does for a session -- and what the conversation's last turn said it loaded. It writes nothing and starts nothing.
 */
const made: string[] = []
afterEach(async () => {
  for (const dir of made.splice(0)) await rmAsync(dir, { recursive: true, force: true })
})
const scratch = async (prefix: string): Promise<string> => {
  const dir = await mkdtempAsync(join(tmpdir(), prefix))
  made.push(dir)
  return dir
}
const skill = async (root: string, name: string): Promise<void> => {
  await mkdirAsync(join(root, name), { recursive: true })
  await writeFileAsync(join(root, name, 'SKILL.md'), `---\nname: ${name}\ndescription: test\n---\nSay hi.\n`)
}
const mate = (over: Partial<PublicTeammate> = {}): PublicTeammate => ({
  teammateId: 'tm_ada',
  name: 'Ada',
  role: 'Research & Briefs',
  hue: 'blue',
  avatar: seedAvatar('tm_ada'),
  createdAt: '2026-10-21T12:00:00.000Z',
  instructions: 'Separate what you observed from what you assumed.',
  route: { runtime: 'claude', model: 'haiku', mode: 'accept-edits' },
  ...over
})
const started = (raw: Record<string, unknown>): NormalizedRuntimeEvent =>
  ({ type: 'run.started', payload: { runtimeThreadId: 's', evidence: { raw, redacted: true } } }) as unknown as NormalizedRuntimeEvent

describe('what a run said it loaded', () => {
  it('is the counts Claude Code gave as it started, and nothing that is not a count', () => {
    expect(lastTurnOf([started({ claude_code_version: '2.1.310', toolCount: 21, skillCount: 80, mcpServerCount: 0, agentCount: 3, cwd: 'C:/x' })])).toEqual({ version: '2.1.310', tools: 21, skills: 80, mcpServers: 0, agents: 3 })
    expect(lastTurnOf([started({ toolCount: -1, skillCount: 1.5, claude_code_version: 'bad version!' })])).toBeUndefined()
    expect(lastTurnOf([])).toBeUndefined()
    expect(lastTurnOf(undefined)).toBeUndefined()
  })
})

describe('a Claude Code teammate’s setup', () => {
  it('reads each layer from its folder, its record and its stores', async () => {
    const folder = await scratch('locust-setup-ws-')
    const home = await scratch('locust-setup-home-')
    const library = await scratch('locust-setup-lib-')
    await writeFileAsync(join(folder, 'LOCUST.md'), 'Run the tests with npm test.\nKeep answers short.\n')
    await writeFileAsync(join(folder, 'CLAUDE.md'), 'one\ntwo\nthree\n')
    await skill(join(folder, '.claude', 'skills'), 'review')
    await skill(join(home, '.claude', 'skills'), 'mine')
    await skill(library, 'kept')
    const setup = await gatherTeammateSetup({
      teammate: mate({ connectors: ['Gmail'] }),
      projectPath: folder,
      group: { name: 'Release', instructions: 'Ship once a day.' },
      settings: { aboutYou: 'I am Colin.', memoryMode: 'project' },
      memoryCount: async () => 4,
      ownSkills: async () => ['evidence-first'],
      lessons: async () => 1,
      libraryFolder: library,
      homeDirectory: home,
      lastTurn: [started({ claude_code_version: '2.1.310', toolCount: 21, skillCount: 4 })]
    })
    expect(setup).toEqual({
      teammateId: 'tm_ada',
      folder: { name: basename(folder), kind: 'project' },
      locustMd: { lines: 2, truncated: false },
      runtimeFile: { name: 'CLAUDE.md', lines: 3 },
      group: { name: 'Release', characters: 16 },
      instructions: 49,
      aboutYou: true,
      memory: { on: true, count: 4 },
      skills: { own: ['evidence-first'], project: 1, personal: { count: 1, lent: false }, library: 1 },
      connectors: ['Gmail'],
      lessons: 1,
      lastTurn: { version: '2.1.310', tools: 21, skills: 4 }
    } satisfies TeammateSetup)
  })

  it('says what is missing as missing: no LOCUST.md, no CLAUDE.md, memory off, nothing lent, no last turn', async () => {
    const folder = await scratch('locust-setup-ws-')
    const setup = await gatherTeammateSetup({
      teammate: mate({ instructions: undefined, worktree: true, route: { runtime: 'claude', model: 'haiku', mode: 'auto' } }),
      projectPath: folder,
      settings: { memoryMode: 'off' },
      memoryCount: async () => 9,
      ownSkills: async () => [],
      lessons: async () => 0,
      libraryFolder: join(folder, 'no-library'),
      homeDirectory: join(folder, 'no-home')
    })
    expect(setup.locustMd).toBeUndefined()
    expect(setup.runtimeFile).toEqual({ name: 'CLAUDE.md' })
    expect(setup.folder.kind).toBe('worktree')
    expect(setup.memory).toEqual({ on: false, count: 0 })
    // Auto runs as the person: Claude Code finds their skills itself.
    expect(setup.skills?.personal).toEqual({ count: 0, lent: true })
    expect(setup.lastTurn).toBeUndefined()
    expect(setup.instructions).toBe(0)
  })
})

describe('another runtime’s setup', () => {
  it('names its own file where Locust knows it, and offers no skills or connectors', async () => {
    const folder = await scratch('locust-setup-ws-')
    await writeFileAsync(join(folder, 'AGENTS.md'), 'x\n')
    const setup = await gatherTeammateSetup({
      teammate: mate({ route: { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free', mode: 'accept-edits' } }),
      projectPath: folder,
      settings: { memoryMode: 'project' },
      memoryCount: async () => 0,
      ownSkills: async () => ['never-read'],
      lessons: async () => 0,
      libraryFolder: folder,
      homeDirectory: folder
    })
    expect(setup.runtimeFile).toEqual({ name: 'AGENTS.md', lines: 1 })
    expect(setup.skills).toBeUndefined()
    expect(setup.connectors).toBeUndefined()
    const cursor = await gatherTeammateSetup({ teammate: mate({ route: { runtime: 'cursor', model: 'account-default', mode: 'ask' } }), projectPath: folder, settings: { memoryMode: 'off' }, memoryCount: async () => 0, ownSkills: async () => [], lessons: async () => 0, libraryFolder: folder, homeDirectory: folder })
    expect(cursor.runtimeFile).toBeUndefined()
  })
})

describe('the card’s words', () => {
  const setup: TeammateSetup = {
    teammateId: 'tm_ada',
    folder: { name: 'locust', kind: 'project' },
    locustMd: { lines: 210, truncated: true },
    runtimeFile: { name: 'CLAUDE.md', lines: 1 },
    instructions: 49,
    aboutYou: false,
    memory: { on: true, count: 1 },
    skills: { own: [], project: 0, personal: { count: 3, lent: false }, library: 2 },
    connectors: [],
    lessons: 2,
    lastTurn: { version: '2.1.310', tools: 21, skills: 1, mcpServers: 0, agents: 3 }
  }

  it('go in the order a turn is given them, each layer said plainly', () => {
    const sections = setupSections(setup, 'Ada', 'Claude Code')
    expect(sections.map((section) => section.title)).toEqual(['Every turn', 'When the work matches', 'Only when you tune it', 'Its last turn here'])
    expect(sections[0]!.rows).toEqual([
      { label: 'Folder', value: 'The project folder, locust' },
      { label: 'LOCUST.md', value: '210 lines read; the rest is cut' },
      { label: 'CLAUDE.md', value: '1 line, read by Claude Code itself' },
      { label: 'Its instructions', value: '49 characters' },
      { label: 'About you', value: 'None' },
      { label: 'Team memory', value: 'On, 1 memory in this folder' }
    ])
    expect(sections[1]!.rows).toEqual([
      { label: 'Its own skills', value: 'None yet. A tune can give it one.' },
      { label: 'Project skills', value: 'None in .claude/skills' },
      { label: 'Your skills', value: 'Not lent; Settings can lend them' },
      { label: 'Kept from GitHub', value: '2 skills' },
      { label: 'Connectors', value: 'Off' }
    ])
    expect(sections[2]!.rows).toEqual([{ label: 'Tune lessons', value: '2 lessons' }])
    expect(sections[2]!.note).toBe('Read by the next /tune of this teammate, never by its everyday turns.')
    expect(sections[3]!.rows).toEqual([
      { label: 'Version', value: 'Claude Code 2.1.310' },
      { label: 'Loaded', value: '21 tools, 1 skill, 0 connector servers, 3 agents' }
    ])
  })

  it('leave out what a runtime does not have, and a last turn that said nothing', () => {
    const { skills: _skills, connectors: _connectors, lastTurn: _last, runtimeFile: _file, ...plain } = setup
    const sections = setupSections({ ...plain, group: { name: 'Release', characters: 16 } }, 'Ada', 'OpenCode')
    expect(sections.map((section) => section.title)).toEqual(['Every turn', 'Only when you tune it'])
    expect(sections[0]!.rows.map((row) => row.label)).toEqual(['Folder', 'LOCUST.md', 'Group', 'Its instructions', 'About you', 'Team memory'])
    expect(sections[0]!.rows[2]).toEqual({ label: 'Group', value: 'Release’s instructions, 16 characters' })
  })
})

describe('the setup handler', () => {
  it('answers only the window, reads the conversation only when it is the teammate’s, and writes nothing', () => {
    const source = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')
    const handler = source.slice(source.indexOf('ipcMain.handle(TEAMMATE_SETUP_CHANNEL'), source.indexOf('ipcMain.handle(TEAMMATE_SKILL_REMOVE_CHANNEL'))
    expect(handler).toContain('fromOwnWindow(event)')
    expect(handler).toContain('(await teammates.missionOwners())[missionId] === teammateId')
    expect(handler).not.toMatch(/\.(add|update|remove|write|set)\w*\(/)
  })
})
