/**
 * BACK UP AND RESTORE (0.614; the PRD's R21, its trust item T5).
 *
 * "The ledger is append-only, fsynced and integrity-checked. Everything else
 * is mutable JSON under userData ... There is no backup or restore and no
 * export of a profile. A new laptop or a wiped profile loses what the setup
 * sprint built." Item 6 as written: a folder with a manifest, keys excluded by
 * design, restore with a preview from the same reader, refuse a newer schema
 * and refuse while a run is live. No cloud and no migration.
 *
 * A BACKUP is a new folder, `Locust backup <date> <time>`, holding a copy of
 * the profile's own files under `profile/` and a manifest written LAST, so a
 * folder without one is a backup that did not finish. Every copied file is
 * named in the manifest with its size and SHA-256, read back from the copy.
 *
 * WHAT IT CARRIES is named, one file and one folder at a time, never
 * "everything but": a store added later is left out until it is classified
 * here (a-profile-backup-classifies-every-store guards that), so nothing new
 * is copied without a decision. Own-model keys are never carried: Windows
 * locks them to one account, and a key belongs in a keychain, not a folder.
 *
 * A RESTORE is read by the same reader as the preview, and refused when the
 * manifest is missing, a file does not match its hash, the backup came from a
 * newer Locust, or a run is going. It is not applied while the app holds its
 * stores in memory -- a store writing its old state back over the restored
 * file is the failure that would look like success. The request is written
 * down, the app restarts, and the restore is applied at the next start before
 * any store is read. What it replaces is moved aside into the profile, never
 * deleted, so a restore can be undone by hand.
 */
import { createHash } from 'node:crypto'
import { copyFile, mkdir, readdir, readFile, rename, rm, rmdir, stat, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

export const BACKUP_SCHEMA = 1
export const BACKUP_MANIFEST = 'locust-backup.json'
/** The restore asked for, written by the request, applied and removed at the next start. */
export const PENDING_RESTORE_FILE = 'pending-restore.json'
/** What the last restore did, kept until the window has said so once. */
export const LAST_RESTORE_FILE = 'last-restore.json'

/** The files a backup carries: the person's setup. */
export const BACKED_UP_FILES = [
  'teammates.json',
  'routines.json',
  'memories.json',
  'approval-rules.json',
  'groups.json',
  'rooms.json',
  'compares.json',
  'folders.json',
  'pets-removed.json',
  // What was said on calls with teammates (0.747): part of the conversations it is kept with.
  'calls.json',
  // Scout, on or off, and which teammate it is (0.756): without it a restored Scout would be met again as a stranger.
  'scout.json',
  // Which GitHub issue a conversation was started from: the header's #n, and Fixes #n on its pull request.
  'issue-origins.json',
  // The runs whose answer belongs to a panel (0.771): a commit message, a walkthrough. The ledger carries them, and
  // without this list each would come back as a conversation.
  'panel-runs.json',
  // Dark, Light or Match computer (0.789): a restored Locust looks the way its person chose.
  'appearance.json',
  // The folders like home or .claude the person said to work in anyway (0.790): a restore does not ask again.
  'folder-cautions.json',
  // What earlier tunes of a teammate showed (0.810): part of the teammate's tuning history.
  'tune-lessons.json'
] as const

/**
 * The folders a backup carries: the conversations, the rooms' messages, the pets taken in, the skills kept
 * from GitHub (0.710) -- chosen by the person, and a repository can move or vanish after -- and each teammate's own
 * skills, kept from its tunes (0.811).
 */
export const BACKED_UP_FOLDERS = ['mission-ledger', 'workroom', 'pets', 'skill-library', 'teammate-skills'] as const

/** Never copied out of a folder a backup carries: the ledger's write probe. */
const NEVER_COPIED = new Set(['.writable'])

/**
 * What a backup never carries, and why -- in the manifest, so a backup says
 * what it is not. Each belongs to this machine or this sign-in, or is a cache
 * that rebuilds itself.
 */
export const LEFT_OUT: Readonly<Record<string, string>> = {
  'claude-context-launches.json': 'machine-local flags for context inspection: recorded again by the next Claude turn',
  'conversation-references.json': 'reference folders belong to this machine: choose them again after a restore',
  'reference-repositories': 'shallow reference copies: downloaded again after choosing a repository',
  'reference-trash': 'recoverable old reference copies on this machine, never permanently deleted by reference cleanup',
  'agent-accounts': 'added agent homes and sign-ins: they stay on this machine and are never copied into a backup',
  'locust-mcp-connection.json': 'the local MCP server token and address: they belong to this Windows account and are minted again',
  'locust-mcp.json': 'whether other AI apps can use Locust on this machine: enable again after a restore',
  'voice-call-mcp': 'a voice call\'s own token for handing work to teammates: minted again for every call (0.739)',
  'voice-call.json': 'the voice Talk speaks in, and the keys that start it from any app: preferences, picked again in a moment after a restore (0.745, 0.755)',
  'voice-previews': 'the greeting each voice gave the first time it was played: made again by playing it (0.751)',
  'claude-app.json': 'when Locust last changed the Claude app\'s settings on this machine, to say whether Claude needs a restart (0.741)',
  'own-models.json': 'your own model keys: Windows locks them to this account, so add them again after a restore',
  'voice-settings.json': 'your voice API key and consent: Windows locks the key to this account, so choose voice typing again after a restore',
  'claude-cloud.json': 'cloud sessions, which belong to this machine and its sign-in',
  'cloud-tasks.json': 'cloud tasks, which belong to this machine and its sign-in',
  'cloud-turns.json': "turns sent to your cloud computer, which name this machine's folders: their changes stay on the codespace, on GitHub (0.761)",
  'brief-sessions.json': 'each agent\'s own session names, which do not carry to another machine',
  'runtime-commands.json': 'where each AI agent is installed on this machine',
  'runtime-facts.json': 'what was found out about the AI agents installed here',
  'runtime-updates.json': 'the agent updates this machine installed',
  'cursor-connector-holds.json': 'this machine\'s Cursor settings, held while Locust runs it',
  'cursor-default-model.json': 'this machine\'s Cursor default model, kept to put back',
  'terminal-imports.json': 'conversations already brought in from this machine\'s terminal',
  'usage-readings.json': 'usage read from this machine\'s accounts',
  'memory-recall-cache.json': 'a cache that rebuilds itself',
  voice: "voice typing's engine and model, downloaded again when it is next used",
  'claude-background.json': "Claude turns running in the background on this computer, which Claude Code keeps",
  'claude-skills': 'skills copied for runs in progress, made again for each run',
  'run-skills': 'skills copied for Codex and OpenCode runs in progress, made again for each run',
  checkpoints: 'copies of your folders from before each turn, for Undo: they belong to the folders on this machine, and can hold files like .env',
  'queued-messages.json': 'messages waiting to send from here: a restore never sends anything',
  'away.json': 'what happened while this machine was away',
  'seen-version.json': 'which version this machine last showed',
  'update-lane.json': 'this machine\'s update choice',
  'window.json': 'this machine\'s window size and place',
  'workspace.json': 'the folder this machine last had open',
  'background.json': 'whether Locust keeps running on this machine',
  'locust-errors.log': 'this machine\'s error log',
  'pets-cache': 'a cache that rebuilds itself',
  'mac-updates': 'a Mac update being downloaded',
  'dictionaries': 'the spelling dictionary this machine downloaded'
}

export interface BackupCounts {
  readonly teammates: number
  readonly routines: number
  readonly memories: number
  readonly rules: number
  readonly groups: number
  readonly rooms: number
  readonly compares: number
  readonly folders: number
  readonly conversations: number
}

export interface BackupFileEntry {
  /** Relative to the backup's `profile/`, with forward slashes. */
  readonly path: string
  readonly bytes: number
  readonly sha256: string
}

export interface BackupManifest {
  readonly schema: number
  readonly app: 'Locust'
  readonly appVersion: string
  readonly createdAt: string
  readonly counts: BackupCounts
  readonly files: readonly BackupFileEntry[]
  readonly leftOut: readonly string[]
}

export type BackupWritten =
  | { readonly ok: true; readonly folder: string; readonly counts: BackupCounts; readonly bytes: number; readonly files: number }
  | { readonly ok: false; readonly reason: string }

export type BackupReading =
  | { readonly ok: true; readonly folder: string; readonly manifest: BackupManifest; readonly bytes: number }
  | { readonly ok: false; readonly reason: string }

export interface RestoreOutcome {
  readonly ok: boolean
  readonly folder: string
  readonly at: string
  /** Where what the restore replaced was moved, inside the profile. */
  readonly aside?: string
  readonly counts?: BackupCounts
  readonly reason?: string
}

const sha256 = async (path: string): Promise<string> => createHash('sha256').update(await readFile(path)).digest('hex')
const exists = async (path: string): Promise<boolean> => (await stat(path).catch(() => undefined)) !== undefined

/** The length of a store's list, or 0 where the file is absent or unreadable. */
async function listLength(path: string, key: string): Promise<number> {
  try {
    const value = JSON.parse(await readFile(path, 'utf8')) as Record<string, unknown>
    const list = value[key]
    return Array.isArray(list) ? list.length : 0
  } catch {
    return 0
  }
}

/** What a profile holds, by the person's own nouns. */
export async function countProfile(root: string): Promise<BackupCounts> {
  const ledger = await readdir(join(root, 'mission-ledger'), { withFileTypes: true }).catch(() => [])
  return {
    teammates: await listLength(join(root, 'teammates.json'), 'teammates'),
    routines: await listLength(join(root, 'routines.json'), 'routines'),
    memories: await listLength(join(root, 'memories.json'), 'memories'),
    rules: await listLength(join(root, 'approval-rules.json'), 'rules'),
    groups: await listLength(join(root, 'groups.json'), 'groups'),
    rooms: await listLength(join(root, 'rooms.json'), 'rooms'),
    compares: await listLength(join(root, 'compares.json'), 'compares'),
    folders: await listLength(join(root, 'folders.json'), 'folders'),
    conversations: ledger.filter((entry) => entry.isFile() && entry.name.endsWith('.jsonl')).length
  }
}

/** Every file under `folder`, relative to it, with forward slashes; the never-copied left out. */
async function filesUnder(folder: string, base = ''): Promise<string[]> {
  const entries = await readdir(join(folder, base), { withFileTypes: true }).catch(() => [])
  const found: string[] = []
  for (const entry of entries) {
    if (NEVER_COPIED.has(entry.name)) continue
    const path = base === '' ? entry.name : `${base}/${entry.name}`
    if (entry.isDirectory()) found.push(...(await filesUnder(folder, path)))
    else if (entry.isFile()) found.push(path)
  }
  return found
}

/** The profile files a backup carries, relative to the profile, that exist. */
async function carriedFiles(profile: string): Promise<string[]> {
  const files: string[] = []
  for (const name of BACKED_UP_FILES) if (await exists(join(profile, name))) files.push(name)
  for (const name of BACKED_UP_FOLDERS) files.push(...(await filesUnder(profile, name)))
  return files
}

const two = (n: number): string => String(n).padStart(2, '0')
/** `2026-10-04 2015`, the person's own clock: a folder name they will read. */
export function backupFolderName(at: Date): string {
  return `Locust backup ${String(at.getFullYear())}-${two(at.getMonth() + 1)}-${two(at.getDate())} ${two(at.getHours())}${two(at.getMinutes())}`
}

/**
 * A NEW backup folder in `parent`, the copy and its manifest. Never writes into
 * an existing folder: a name already taken is refused, not overwritten.
 */
export async function writeBackup(profile: string, parent: string, options: { readonly appVersion: string; readonly now: () => Date }): Promise<BackupWritten> {
  if (!(await exists(parent))) return { ok: false, reason: 'That folder could not be found.' }
  const at = options.now()
  const folder = join(parent, backupFolderName(at))
  if (await exists(folder)) return { ok: false, reason: `A backup named "${backupFolderName(at)}" is already there. Try again in a minute.` }
  try {
    const entries: BackupFileEntry[] = []
    let bytes = 0
    for (const path of await carriedFiles(profile)) {
      const from = join(profile, ...path.split('/'))
      const to = join(folder, 'profile', ...path.split('/'))
      await mkdir(join(to, '..'), { recursive: true })
      await copyFile(from, to)
      // Read back from the COPY: the manifest vouches for what is in the backup, not what was meant to be.
      const size = (await stat(to)).size
      entries.push({ path, bytes: size, sha256: await sha256(to) })
      bytes += size
    }
    const counts = await countProfile(join(folder, 'profile'))
    const manifest: BackupManifest = {
      schema: BACKUP_SCHEMA,
      app: 'Locust',
      appVersion: options.appVersion,
      createdAt: at.toISOString(),
      counts,
      files: entries,
      leftOut: Object.keys(LEFT_OUT)
    }
    await mkdir(folder, { recursive: true })
    // Last: a folder with no manifest is a backup that did not finish, and the reader says so.
    await writeFile(join(folder, BACKUP_MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`, 'utf8')
    return { ok: true, folder, counts, bytes, files: entries.length }
  } catch (error) {
    return { ok: false, reason: `The backup could not be written: ${error instanceof Error ? error.message : String(error)}` }
  }
}

/** -1, 0 or 1 for two `major.minor.patch` versions; a part that is not a number counts as 0. */
export function compareVersions(left: string, right: string): number {
  const a = left.split('.').map((part) => Number.parseInt(part, 10) || 0)
  const b = right.split('.').map((part) => Number.parseInt(part, 10) || 0)
  for (let at = 0; at < Math.max(a.length, b.length); at += 1) {
    const difference = (a[at] ?? 0) - (b[at] ?? 0)
    if (difference !== 0) return difference < 0 ? -1 : 1
  }
  return 0
}

/**
 * THE READER, for the preview and the restore alike: the manifest, then every
 * file it names, by size and hash. The first thing wrong is the reason.
 */
export async function readBackup(folder: string, options: { readonly appVersion: string }): Promise<BackupReading> {
  let manifest: BackupManifest
  try {
    manifest = JSON.parse(await readFile(join(folder, BACKUP_MANIFEST), 'utf8')) as BackupManifest
  } catch {
    return { ok: false, reason: 'This folder is not a Locust backup, or the backup did not finish: it has no locust-backup.json.' }
  }
  if (manifest.app !== 'Locust' || !Array.isArray(manifest.files) || typeof manifest.appVersion !== 'string') {
    return { ok: false, reason: 'This folder\'s locust-backup.json is not one Locust wrote.' }
  }
  if (typeof manifest.schema !== 'number' || manifest.schema > BACKUP_SCHEMA) {
    return { ok: false, reason: `This backup was made by a newer Locust (${manifest.appVersion}). Update Locust, then restore it.` }
  }
  if (compareVersions(manifest.appVersion, options.appVersion) > 0) {
    return { ok: false, reason: `This backup was made by Locust ${manifest.appVersion}, newer than this one (${options.appVersion}). Update Locust, then restore it.` }
  }
  let bytes = 0
  for (const entry of manifest.files) {
    // Read from a file a person could have edited: nothing about it is taken on trust.
    const named: unknown = (entry as { readonly path?: unknown }).path
    if (typeof named !== 'string' || named.split('/').some((part: string) => part === '..' || part === '') || !carries(named)) {
      return { ok: false, reason: `The backup names a file it should not carry: ${String(named)}.` }
    }
    const path = join(folder, 'profile', ...entry.path.split('/'))
    const seen = await stat(path).catch(() => undefined)
    if (seen === undefined) return { ok: false, reason: `The backup is missing ${entry.path}.` }
    if (seen.size !== entry.bytes || (await sha256(path)) !== entry.sha256) {
      return { ok: false, reason: `${entry.path} has changed since the backup was made, so the backup cannot be trusted.` }
    }
    bytes += seen.size
  }
  return { ok: true, folder, manifest, bytes }
}

/** Whether a path in a manifest is one a backup carries: a named file, or under a named folder. */
function carries(path: string): boolean {
  const [first] = path.split('/')
  return (BACKED_UP_FILES as readonly string[]).includes(path) || (BACKED_UP_FOLDERS as readonly string[]).includes(first ?? '')
}

/**
 * The restore asked for: read, refused for a live run, and written down for
 * the next start. The caller restarts the app.
 */
export async function requestRestore(
  folder: string,
  profile: string,
  options: { readonly appVersion: string; readonly liveRuns: number; readonly now: () => Date }
): Promise<{ readonly ok: true; readonly counts: BackupCounts } | { readonly ok: false; readonly reason: string }> {
  if (options.liveRuns > 0) {
    return { ok: false, reason: `${options.liveRuns === 1 ? 'A run is' : `${String(options.liveRuns)} runs are`} going. Restore once ${options.liveRuns === 1 ? 'it has' : 'they have'} finished or been stopped.` }
  }
  const reading = await readBackup(folder, options)
  if (!reading.ok) return reading
  await writeFile(join(profile, PENDING_RESTORE_FILE), `${JSON.stringify({ folder, requestedAt: options.now().toISOString() })}\n`, 'utf8')
  return { ok: true, counts: reading.manifest.counts }
}

const stamp = (at: Date): string =>
  `${String(at.getFullYear())}${two(at.getMonth() + 1)}${two(at.getDate())}-${two(at.getHours())}${two(at.getMinutes())}${two(at.getSeconds())}`

/**
 * AT THE NEXT START, before any store is read: the restore written down, read
 * again (the folder may have changed since), what it replaces moved aside, the
 * backup's files copied in. Anything that fails puts back what was moved. The
 * outcome is kept for the window to say once. Undefined when none was asked.
 */
export async function applyPendingRestore(
  profile: string,
  options: {
    readonly appVersion: string
    readonly now: () => Date
    /** The copy, which a test makes fail part-way to see that everything is put back. */
    readonly copy?: (from: string, to: string) => Promise<void>
  }
): Promise<RestoreOutcome | undefined> {
  const copy = options.copy ?? copyFile
  let pending: { folder?: unknown }
  try {
    pending = JSON.parse(await readFile(join(profile, PENDING_RESTORE_FILE), 'utf8')) as { folder?: unknown }
  } catch {
    return undefined
  }
  // Taken off first: a restore that fails at every start must not be retried at every start.
  await rm(join(profile, PENDING_RESTORE_FILE), { force: true })
  const at = options.now()
  const folder = typeof pending.folder === 'string' ? pending.folder : ''
  const done = async (outcome: RestoreOutcome): Promise<RestoreOutcome> => {
    await writeFile(join(profile, LAST_RESTORE_FILE), `${JSON.stringify(outcome)}\n`, 'utf8').catch(() => undefined)
    return outcome
  }
  const reading = await readBackup(folder, options)
  if (!reading.ok) return done({ ok: false, folder, at: at.toISOString(), reason: reading.reason })
  const aside = join(profile, `before-restore-${stamp(at)}`)
  const moved: string[] = []
  let copying = false
  try {
    await mkdir(aside, { recursive: true })
    for (const name of [...BACKED_UP_FILES, ...BACKED_UP_FOLDERS]) {
      if (!(await exists(join(profile, name)))) continue
      await rename(join(profile, name), join(aside, name))
      moved.push(name)
    }
    copying = true
    for (const entry of reading.manifest.files) {
      const to = join(profile, ...entry.path.split('/'))
      await mkdir(join(to, '..'), { recursive: true })
      await copy(join(folder, 'profile', ...entry.path.split('/')), to)
    }
    return done({ ok: true, folder, at: at.toISOString(), aside, counts: reading.manifest.counts })
  } catch (error) {
    // Once copying began every name that was here had been moved aside, so a name not moved is one
    // the copy brought: it goes. Before then a name not moved is still the original, and is left.
    if (copying) {
      for (const name of [...BACKED_UP_FILES, ...BACKED_UP_FOLDERS]) {
        if (!moved.includes(name)) await rm(join(profile, name), { recursive: true, force: true }).catch(() => undefined)
      }
    }
    // Then what was moved comes back, over anything copied in its place.
    for (const name of moved) {
      await rm(join(profile, name), { recursive: true, force: true }).catch(() => undefined)
      await rename(join(aside, name), join(profile, name)).catch(() => undefined)
    }
    // Only if it is empty: a file that could not be put back is still in it, and stays.
    await rmdir(aside).catch(() => undefined)
    return done({ ok: false, folder, at: at.toISOString(), reason: `The restore could not be finished, and nothing was changed: ${error instanceof Error ? error.message : String(error)}` })
  }
}

/** The last restore's outcome, once: read and taken away. */
export async function takeLastRestore(profile: string): Promise<RestoreOutcome | undefined> {
  try {
    const outcome = JSON.parse(await readFile(join(profile, LAST_RESTORE_FILE), 'utf8')) as RestoreOutcome
    await rm(join(profile, LAST_RESTORE_FILE), { force: true })
    return outcome
  } catch {
    return undefined
  }
}
