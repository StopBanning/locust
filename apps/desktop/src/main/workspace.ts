import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { isAbsolute, posix, win32 } from 'node:path'

/**
 * The id a mission records for the folder it ran in.
 *
 * Derived from the path, so the same folder always produces the same id and a
 * mission can be matched back to where it happened. Codex missions were
 * already doing this inline; the app-server and Antigravity paths were minting
 * a RANDOM id instead, which looks the same in a receipt and means the
 * opposite -- their missions could never be matched to any folder at all.
 * MEASURED 2026-09-03 while making the app open the right conversation for the
 * folder it was launched in.
 *
 * Hashed rather than stored plainly: the ledger is a durable local record and
 * a path can name a person, a client, or an unreleased project.
 */
export function workspaceIdFor(workspacePath: string): string {
  return `ws_${createHash('sha256').update(workspacePath, 'utf8').digest('hex').slice(0, 32)}`
}

/**
 * Which folder the teammates work in, and how the app came to know it.
 *
 * Until 0.21.5 the answer was the process's working directory, full stop --
 * which is right when the app is launched from a terminal inside a project
 * and WRONG for the way a person actually opens an installed app: the Start
 * menu shortcut runs it from its own install folder. Every teammate on
 * Colin's machine was working inside `AppData\Local\Programs\Locust`, and the
 * first route that checks its folder (Antigravity) said so out loud
 * (2026-09-05: "Antigravity has not opened C:\...\Programs\Locust").
 *
 * So the install folder is never a workspace. Launched from there, the app
 * uses the folder the person chose last time -- and with none chosen, a
 * folder Locust makes for it (Documents\Locust), the way a terminal always
 * has a working directory. Colin, 2026-09-05: "every similar program lets
 * you do it, so maybe it just writes a project folder if you don't have
 * one". The 0.21.5-0.31.1 answer was to open with no workspace and refuse
 * the first message, which read as a bug in the first minute of use.
 * The install folder is still never the workspace.
 */
export interface WorkspaceResolution {
  readonly path: string | undefined
  readonly source: 'argument' | 'launch-folder' | 'remembered' | 'default' | 'none'
}

export const WORKSPACE_ARGUMENT = '--workspace='

export function resolveWorkspacePath(options: {
  readonly argv: readonly string[]
  readonly cwd: string
  /** Where the installed app lives; undefined for a development build, which can be launched from anywhere. */
  readonly installDirectory: string | undefined
  readonly remembered: string | undefined
  /** The folder to make and use when nothing else names one; undefined keeps the old "no workspace" answer. */
  readonly defaultWorkspace?: string | undefined
  readonly platform: NodeJS.Platform
  /** The person's home folder; a launch from it, or from an agent's settings folder in it, is not a workspace. */
  readonly homeDirectory?: string | undefined
  /** Windows' own folder (`%SystemRoot%`); a launch from inside it is not a workspace. */
  readonly systemDirectory?: string | undefined
}): WorkspaceResolution {
  const path = options.platform === 'win32' ? win32 : posix
  // An explicit argument wins: it is how the app reopens itself in a folder
  // the person just chose.
  const argument = options.argv.find((entry) => entry.startsWith(WORKSPACE_ARGUMENT))?.slice(WORKSPACE_ARGUMENT.length)
  if (argument !== undefined && argument.length > 0 && path.isAbsolute(argument)) {
    return { path: path.resolve(argument), source: 'argument' }
  }
  // Launched from a real folder -- a terminal, a shortcut with a start-in --
  // that folder is the workspace, as it always was. Not the home folder, an
  // agent's own settings folder, or Windows' own (0.572; see notATeammateFolder).
  if (
    !isInsideDirectory(options.cwd, options.installDirectory, options.platform)
    && notATeammateFolder(options.cwd, options.homeDirectory, options.platform, options.systemDirectory) === undefined
  ) {
    return { path: path.resolve(options.cwd), source: 'launch-folder' }
  }
  if (
    options.remembered !== undefined
    && path.isAbsolute(options.remembered)
    && !isInsideDirectory(options.remembered, options.installDirectory, options.platform)
  ) {
    return { path: path.resolve(options.remembered), source: 'remembered' }
  }
  if (
    options.defaultWorkspace !== undefined
    && path.isAbsolute(options.defaultWorkspace)
    && !isInsideDirectory(options.defaultWorkspace, options.installDirectory, options.platform)
  ) {
    return { path: path.resolve(options.defaultWorkspace), source: 'default' }
  }
  return { path: undefined, source: 'none' }
}

/**
 * The folders under home where the agents keep their own settings, sign-ins
 * and transcripts. A teammate working in one reads them as its project.
 */
// `.copilot` (0.587): Copilot CLI's own folder, which runtime-setup.ts already reads.
export const AGENT_SETTINGS_FOLDERS = ['.claude', '.codex', '.cursor', '.gemini', '.copilot', '.config'] as const

/**
 * Why a folder is no place for teammates to work, or undefined when it is fine.
 *
 * Colin's ledger, 2026-10-03: Bro (Chief of Staff) worked in
 * `C:\Users\<name>\.claude`, Claude Code's own folder; in Auto its first
 * command listed the credential files there, and its report was written into
 * it. A launch folder like that was taken as the workspace without a word.
 */
export function notATeammateFolder(
  candidate: string,
  homeDirectory: string | undefined,
  platform: NodeJS.Platform,
  systemDirectory?: string
): string | undefined {
  const path = platform === 'win32' ? win32 : posix
  if (!path.isAbsolute(candidate)) return undefined
  if (systemDirectory !== undefined && isInsideDirectory(candidate, systemDirectory, platform)) return "Windows' own folder"
  if (homeDirectory === undefined) return undefined
  if (isInsideDirectory(homeDirectory, candidate, platform)) return 'your home folder'
  for (const name of AGENT_SETTINGS_FOLDERS) {
    if (isInsideDirectory(candidate, path.resolve(homeDirectory, name), platform)) return `${name}, where the agents keep their own settings`
  }
  return undefined
}


/**
 * Whether `candidate` is `directory` or somewhere under it. Case-blind on Windows, where the filesystem is.
 * Read with the platform's own path rules, not the host's: a Windows folder is judged as Windows would judge it.
 */
export function isInsideDirectory(
  candidate: string,
  directory: string | undefined,
  platform: NodeJS.Platform
): boolean {
  if (directory === undefined) return false
  const path = platform === 'win32' ? win32 : posix
  const fold = (value: string): string => {
    const full = path.resolve(value).replace(/[\\/]+$/, '')
    return platform === 'win32' ? full.toLowerCase() : full
  }
  const inner = fold(candidate)
  const outer = fold(directory)
  return inner === outer || inner.startsWith(outer + path.sep)
}

interface RememberedWorkspaceFile {
  readonly schemaVersion: 1
  readonly path: string
}

/** The folder chosen last time, or undefined when there is none or the file is unreadable. */
export function readRememberedWorkspace(file: string): string | undefined {
  try {
    const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'))
    if (typeof parsed !== 'object' || parsed === null) return undefined
    const record = parsed as Partial<RememberedWorkspaceFile>
    return typeof record.path === 'string' && record.path.length > 0 ? record.path : undefined
  } catch {
    return undefined
  }
}

export async function writeRememberedWorkspace(file: string, path: string): Promise<void> {
  const record: RememberedWorkspaceFile = { schemaVersion: 1, path }
  await writeFile(file, `${JSON.stringify(record, null, 2)}\n`, 'utf8')
}

/*
 * FOLDERS LIKE THESE, SAID YES TO (0.790). The picker asks when such a folder is chosen, but a folder can be the
 * window's without that question -- picked again from the folder menu, or chosen before 0.582 -- and on 2026-10-18
 * three handoffs started in `.claude` with nobody noticing. So the first conversation in one is asked about too, in
 * the window (Composer.tsx), once per folder, as an editor asks once whether to trust one: the answers are kept here.
 */
interface FolderCautionsFile {
  readonly schemaVersion: 1
  /** The folders like these that the person said to work in anyway, each asked about once. */
  readonly confirmed: readonly string[]
}

/** The folders the person said to work in anyway, or none when the file is missing or unreadable. */
export function readConfirmedFolders(file: string): readonly string[] {
  try {
    const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'))
    if (typeof parsed !== 'object' || parsed === null) return []
    const confirmed = (parsed as Partial<FolderCautionsFile>).confirmed
    return Array.isArray(confirmed) ? confirmed.filter((folder): folder is string => typeof folder === 'string' && isAbsolute(folder)) : []
  } catch {
    return []
  }
}

/** The same folder, not one inside it: confirming `.claude` says nothing about `.claude\skills`. */
export function sameFolder(left: string, right: string, platform: NodeJS.Platform): boolean {
  return isInsideDirectory(left, right, platform) && isInsideDirectory(right, left, platform)
}

export async function writeConfirmedFolders(file: string, confirmed: readonly string[]): Promise<void> {
  const record: FolderCautionsFile = { schemaVersion: 1, confirmed }
  await writeFile(file, `${JSON.stringify(record, null, 2)}\n`, 'utf8')
}
