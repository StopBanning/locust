import { readFile, readdir } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import type { PublicRuntimeSetup } from '../shared/ipc.js'

/**
 * What each runtime has configured for itself: MCP servers and hooks, read
 * from the runtime's own files and shown, never changed.
 *
 * Colin, 2026-09-05, on hooks: "make sure the user actually knows what's
 * going on". A hook fires shell commands at points in a run; an MCP server
 * gives the runtime tools. Both are the runtime's own configuration, which
 * is why Locust adds none of its own (a Locust-level hook would fire beside
 * the runtime's, and a second MCP config would be one nothing reads). What
 * Locust owes is to SAY what is there, so a formatter that runs mid-mission
 * or a tool a teammate reaches for is not a surprise.
 *
 * Files are read with a byte bound and parsed leniently: a file that cannot
 * be read is named as unreadable, never guessed at. Only names are kept --
 * a server's command line and a hook's script can hold secrets, and the
 * screen needs neither.
 */

const MAX_FILE_BYTES = 2 * 1024 * 1024
const MAX_NAMES = 40

export interface RuntimeSetupOptions {
  readonly workspacePath: string | undefined
  readonly homeDirectory?: string
  /** Test seam. */
  readonly read?: (path: string) => Promise<string>
  /** Test seam: the entries of a directory, names only. */
  readonly list?: (directory: string) => Promise<readonly string[]>
}

interface Found {
  readonly mcpServers: string[]
  readonly hooks: string[]
  readonly skills: string[]
  readonly agents: string[]
  readonly sources: string[]
  readonly unreadable: string[]
}

const fresh = (): Found => ({ mcpServers: [], hooks: [], skills: [], agents: [], sources: [], unreadable: [] })

/**
 * The names in a directory a runtime keeps its skills or agents in: one
 * folder per skill (with a SKILL.md), one .md per agent. Absent is nothing;
 * unreadable is said. Claude Code's own init record lists the same names
 * (MEASURED 2026-09-05), which is how this list is known to be the one it
 * would show.
 */
async function names(directory: string, options: RuntimeSetupOptions, found: Found, kind: 'folders' | 'markdown'): Promise<readonly string[]> {
  try {
    const entries = await (options.list ?? (async (dir: string) => (await readdir(dir, { withFileTypes: true })).map((entry) => (entry.isDirectory() ? `${entry.name}/` : entry.name))))(directory)
    found.sources.push(directory)
    return entries
      .filter((entry) => (kind === 'folders' ? entry.endsWith('/') : /\.md$/i.test(entry)))
      .map((entry) => entry.replace(/\/$/, '').replace(/\.md$/i, ''))
      .filter((entry) => !entry.startsWith('.'))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') found.unreadable.push(directory)
    return []
  }
}

function addNames(into: string[], names: readonly string[]): void {
  for (const name of names) {
    if (typeof name !== 'string' || name.trim().length === 0) continue
    if (into.includes(name)) continue
    if (into.length >= MAX_NAMES) return
    into.push(name.slice(0, 80))
  }
}

async function readJson(path: string, options: RuntimeSetupOptions, found: Found): Promise<Record<string, unknown> | undefined> {
  let text: string
  try {
    text = await (options.read ?? ((p: string) => readFile(p, 'utf8')))(path)
  } catch (error) {
    // Absent is the common case and not a finding; anything else is.
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') found.unreadable.push(path)
    return undefined
  }
  if (Buffer.byteLength(text, 'utf8') > MAX_FILE_BYTES) {
    found.unreadable.push(path)
    return undefined
  }
  try {
    const value = JSON.parse(text) as unknown
    if (typeof value !== 'object' || value === null) {
      found.unreadable.push(path)
      return undefined
    }
    found.sources.push(path)
    return value as Record<string, unknown>
  } catch {
    found.unreadable.push(path)
    return undefined
  }
}

async function readText(path: string, options: RuntimeSetupOptions, found: Found): Promise<string | undefined> {
  try {
    const text = await (options.read ?? ((p: string) => readFile(p, 'utf8')))(path)
    if (Buffer.byteLength(text, 'utf8') > MAX_FILE_BYTES) {
      found.unreadable.push(path)
      return undefined
    }
    found.sources.push(path)
    return text
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') found.unreadable.push(path)
    return undefined
  }
}

const keysOf = (value: unknown): string[] => (typeof value === 'object' && value !== null ? Object.keys(value as object) : [])

/** Hook events with how many entries each: "Stop (1)". */
function hookLabels(value: unknown): string[] {
  if (typeof value !== 'object' || value === null) return []
  return Object.entries(value as Record<string, unknown>).map(([event, entries]) => {
    const count = Array.isArray(entries) ? entries.length : typeof entries === 'object' && entries !== null ? Object.keys(entries).length : 1
    return `${event} (${String(count)})`
  })
}

/** The keys of Claude Code's per-project record, for either slash style the file uses. */
function claudeProjectRecord(projects: unknown, workspacePath: string): Record<string, unknown> | undefined {
  if (typeof projects !== 'object' || projects === null) return undefined
  const wanted = new Set([workspacePath, workspacePath.replace(/\\/g, '/'), workspacePath.replace(/\//g, '\\')].map((p) => p.toLowerCase()))
  for (const [key, value] of Object.entries(projects as Record<string, unknown>)) {
    if (wanted.has(key.toLowerCase()) && typeof value === 'object' && value !== null) return value as Record<string, unknown>
  }
  return undefined
}

async function claude(options: RuntimeSetupOptions, home: string, ws: string | undefined): Promise<Found> {
  const found = fresh()
  const user = await readJson(join(home, '.claude', 'settings.json'), options, found)
  addNames(found.hooks, hookLabels(user?.hooks))
  if (ws !== undefined) {
    for (const name of ['settings.json', 'settings.local.json']) {
      const project = await readJson(join(ws, '.claude', name), options, found)
      addNames(found.hooks, hookLabels(project?.hooks))
    }
  }
  const global = await readJson(join(home, '.claude.json'), options, found)
  addNames(found.mcpServers, keysOf(global?.mcpServers))
  if (ws !== undefined) {
    addNames(found.mcpServers, keysOf(claudeProjectRecord(global?.projects, ws)?.mcpServers))
    const projectMcp = await readJson(join(ws, '.mcp.json'), options, found)
    addNames(found.mcpServers, keysOf(projectMcp?.mcpServers))
  }
  // Skills and agents: what a teammate on Claude Code can reach for, by
  // name, from the user's own folders and the project's.
  addNames(found.skills, await names(join(home, '.claude', 'skills'), options, found, 'folders'))
  addNames(found.agents, await names(join(home, '.claude', 'agents'), options, found, 'markdown'))
  if (ws !== undefined) {
    addNames(found.skills, await names(join(ws, '.claude', 'skills'), options, found, 'folders'))
    addNames(found.agents, await names(join(ws, '.claude', 'agents'), options, found, 'markdown'))
  }
  return found
}

async function codex(options: RuntimeSetupOptions, home: string, ws: string | undefined): Promise<Found> {
  const found = fresh()
  for (const path of [join(home, '.codex', 'config.toml'), ...(ws === undefined ? [] : [join(ws, '.codex', 'config.toml')])]) {
    const text = await readText(path, options, found)
    if (text === undefined) continue
    // Table headers only: [mcp_servers.<name>] and nothing beneath them.
    addNames(found.mcpServers, [...text.matchAll(/^\s*\[mcp_servers\.([^\]\s.]+)\]/gm)].map((match) => match[1] ?? ''))
    if (/^\s*notify\s*=/m.test(text)) addNames(found.hooks, ['notify (1)'])
  }
  /*
   * A4.1: Codex's skills, from where Codex itself finds them. MEASURED
   * 2026-09-24 with its app-server's `skills/list` (starts no turn): the
   * user's ~/.agents/skills, and its own ~/.codex/skills -- whose `.system`
   * folder of built-ins is left out, as every dot-name is. The project's
   * `.agents/skills` is named beside them in the 0.156.1 binary. Skills a
   * Codex PLUGIN brings (~/.codex/plugins/cache) are the plugin's, not listed.
   * Not ~/.claude/skills: Codex does not read it.
   */
  addNames(found.skills, await names(join(home, '.codex', 'skills'), options, found, 'folders'))
  addNames(found.skills, await names(join(home, '.agents', 'skills'), options, found, 'folders'))
  if (ws !== undefined) addNames(found.skills, await names(join(ws, '.agents', 'skills'), options, found, 'folders'))
  return found
}

async function cursor(options: RuntimeSetupOptions, home: string, ws: string | undefined): Promise<Found> {
  const found = fresh()
  for (const dir of [join(home, '.cursor'), ...(ws === undefined ? [] : [join(ws, '.cursor')])]) {
    const mcp = await readJson(join(dir, 'mcp.json'), options, found)
    addNames(found.mcpServers, keysOf(mcp?.mcpServers))
    const hooks = await readJson(join(dir, 'hooks.json'), options, found)
    addNames(found.hooks, hookLabels(hooks?.hooks))
  }
  /*
   * A4.1: Cursor's skills, READ FROM ITS CODE (cursor-agent
   * 2026.09.23-86fc751, index.js): its skill roots are `.cursor/skills` and
   * `.agents/skills`, and it also knows `.claude`, `.codex` and `.grok` skill
   * folders but marks them `thirdParty` -- read behind a setting this cannot
   * see, so they are not listed rather than guessed at. Its own
   * `skills-cursor` folder is its built-ins, left out as Codex's are.
   */
  for (const root of [home, ...(ws === undefined ? [] : [ws])]) {
    addNames(found.skills, await names(join(root, '.cursor', 'skills'), options, found, 'folders'))
    addNames(found.skills, await names(join(root, '.agents', 'skills'), options, found, 'folders'))
  }
  return found
}

async function opencode(options: RuntimeSetupOptions, home: string, ws: string | undefined): Promise<Found> {
  const found = fresh()
  const paths = [join(home, '.config', 'opencode', 'opencode.json'), ...(ws === undefined ? [] : [join(ws, 'opencode.json'), join(ws, '.opencode', 'opencode.json')])]
  for (const path of paths) {
    const config = await readJson(path, options, found)
    addNames(found.mcpServers, keysOf(config?.mcp))
  }
  /*
   * A4.1: OpenCode's skills, READ FROM ITS SOURCE (skill/index.ts,
   * 2026-09-24): Claude Code's and the shared `.agents` folders at home and
   * in the project -- so an OpenCode teammate carries the person's Claude
   * skills too, which the research found (18 of them) and nothing showed --
   * and its own `skill` or `skills` folder beside its config.
   */
  const skillFolders = [
    join(home, '.claude', 'skills'),
    join(home, '.agents', 'skills'),
    join(home, '.config', 'opencode', 'skill'),
    join(home, '.config', 'opencode', 'skills'),
    ...(ws === undefined
      ? []
      : [join(ws, '.claude', 'skills'), join(ws, '.agents', 'skills'), join(ws, '.opencode', 'skill'), join(ws, '.opencode', 'skills')])
  ]
  for (const folder of skillFolders) addNames(found.skills, await names(folder, options, found, 'folders'))
  return found
}

async function copilot(options: RuntimeSetupOptions, home: string, ws: string | undefined): Promise<Found> {
  const found = fresh()
  const config = await readJson(join(home, '.copilot', 'mcp-config.json'), options, found)
  addNames(found.mcpServers, keysOf(config?.mcpServers))
  /*
   * A4.1: Copilot's skills, from where Copilot itself says it looks.
   * MEASURED 2026-09-24 off `copilot skill --help` (1.0.88): personal
   * ~/.copilot/skills or ~/.agents/skills; in the project .github/skills,
   * .agents/skills or .claude/skills. `copilot skill list` on this machine
   * named the two in ~/.agents/skills as its own. Plugin and `skill add`
   * directories are not read here.
   */
  const skillFolders = [
    join(home, '.copilot', 'skills'),
    join(home, '.agents', 'skills'),
    ...(ws === undefined ? [] : [join(ws, '.github', 'skills'), join(ws, '.agents', 'skills'), join(ws, '.claude', 'skills')])
  ]
  for (const folder of skillFolders) addNames(found.skills, await names(folder, options, found, 'folders'))
  return found
}

/**
 * The MCP servers in the person's own Copilot settings, by name: the ones an Ask or Plan run on Copilot starts none
 * of (0.824, `--disable-mcp-server`). None when the file is absent or does not read as settings.
 */
export async function copilotMcpServerNames(options: Pick<RuntimeSetupOptions, 'homeDirectory' | 'read'> = {}): Promise<readonly string[]> {
  const config = await readJson(join(options.homeDirectory ?? homedir(), '.copilot', 'mcp-config.json'), { workspacePath: undefined, ...options }, fresh())
  return keysOf(config?.mcpServers)
}

/** Every runtime's own MCP servers and hooks, by runtime id. */
export async function readRuntimeSetup(options: RuntimeSetupOptions): Promise<Readonly<Record<string, PublicRuntimeSetup>>> {
  const home = options.homeDirectory ?? homedir()
  const ws = options.workspacePath === undefined || options.workspacePath.length === 0 ? undefined : resolve(options.workspacePath)
  const entries: [string, Found][] = [
    ['claude', await claude(options, home, ws)],
    ['codex', await codex(options, home, ws)],
    ['cursor', await cursor(options, home, ws)],
    ['opencode', await opencode(options, home, ws)],
    ['copilot', await copilot(options, home, ws)]
  ]
  return Object.fromEntries(
    entries.map(([id, found]) => [id, { mcpServers: found.mcpServers, hooks: found.hooks, skills: found.skills, agents: found.agents, sources: found.sources, unreadable: found.unreadable }])
  )
}
