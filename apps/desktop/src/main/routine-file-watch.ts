import { readdir, stat } from 'node:fs/promises'
import { basename, dirname, join, resolve, sep } from 'node:path'

/**
 * FOLDER WATCHERS (0.522), for routines that run "on a new file"
 * (shared/routine-schedule.ts). Product ideas, Bloks note item 2: "when a
 * file lands in this folder, ask this teammate". The rules, each one a way
 * this could go wrong:
 *
 *   - The first look is a BASELINE: what is in the folder then starts
 *     nothing. A watcher made over a folder of 400 files does not run 400
 *     times.
 *   - A new file counts once it has STOPPED CHANGING for `SETTLE_MS`: a
 *     download or a copy in progress is not yet the file.
 *   - A file a teammate wrote is not an arrival (0.840; Macro's routines
 *     take no trigger from what agents and bots change). "When a file lands
 *     in inbox/, write its summary there" started again on its own summary,
 *     six times an hour, every hour. While any run is live, a file that
 *     settles is HELD; each turn's record names what it wrote (Undo's own
 *     checkpoints), and those are let go. What is left once runs have been
 *     quiet for `RELEASE_MS` came from someone else, and starts the routine.
 *     A turn with no record -- Undo could not keep that folder -- is not told
 *     apart, so the cap below still stands behind it.
 *   - At most `MAX_FIRES_PER_HOUR` runs an hour, per routine: a folder a
 *     teammate itself writes into cannot set off a loop.
 *   - Files arriving together go to ONE run, named together.
 *   - Top level only, no hidden or temporary names (".x", "~$x", "x.tmp",
 *     "x.crdownload", "x.part"), and only inside the project folder.
 *
 * Polled, not `fs.watch`: a poll every `POLL_MS` is the same on every
 * platform and every file system, including a synced or network folder
 * where watch events are not delivered.
 */
export const POLL_MS = 10_000
export const SETTLE_MS = 20_000
export const MAX_FIRES_PER_HOUR = 6
/** How long runs stay quiet before held files are taken as someone else's: a turn's record is written as it ends. */
export const RELEASE_MS = 15_000
const MAX_ENTRIES = 2_000
const MAX_PENDING = 20

const PASSING = /^(\.|~\$)|\.(tmp|temp|crdownload|part|partial|download)$/i

interface Watched {
  readonly folder: string
  /** Names seen: the baseline, and every file since that settled. */
  readonly seen: Set<string>
  /** New names still changing: their size and time when last looked at, and since when they held. */
  readonly settling: Map<string, { size: number; mtimeMs: number; since: number }>
  /** Settled new files, waiting for a run. */
  pending: string[]
  /** Settled while a run was live: perhaps that run's own, until its turn says what it wrote. */
  held: string[]
  /** When each run this hour started. */
  fires: number[]
  baselined: boolean
}

export interface FileArrivals {
  /**
   * Look at every watched folder; the routines to watch are given each time, and whether a teammate's run is live
   * now -- a file that settles then is held until that turn says what it wrote.
   */
  poll(watching: readonly { readonly routineId: string; readonly folder: string }[], now?: number, writing?: boolean): Promise<void>
  /** A turn ended having changed these paths, relative to `workspace` with `/`: none of them is an arrival. */
  wrote(workspace: string, paths: readonly string[]): void
  /** The settled new files a routine may run for now, or none (none waiting, or its hour is full). */
  ready(routineId: string, now?: number): readonly string[]
  /** A run started for these files. */
  fired(routineId: string, files: readonly string[], now?: number): void
}

/** The watched folder, absolute, when it is inside the project; undefined otherwise. */
export function watchedPath(projectFolder: string, folder: string): string | undefined {
  const root = resolve(projectFolder)
  const path = resolve(root, folder)
  return path.startsWith(root + sep) ? path : undefined
}

/** One path, the same however it was spelled: Windows does not tell `C:\Work` from `c:\work`. */
const samePath = (one: string, other: string): boolean => (process.platform === 'win32' ? one.toLowerCase() === other.toLowerCase() : one === other)

export function createFileArrivals(options: { readonly projectFolder: string }): FileArrivals {
  const byRoutine = new Map<string, Watched>()
  let writingAt: number | undefined
  return {
    async poll(watching, now = Date.now(), writing = false) {
      if (writing) writingAt = now
      const quiet = writingAt === undefined || now - writingAt >= RELEASE_MS
      // A routine no longer watching -- removed, or its schedule changed -- is forgotten.
      for (const routineId of [...byRoutine.keys()]) {
        const still = watching.find((entry) => entry.routineId === routineId)
        if (still === undefined || still.folder !== byRoutine.get(routineId)?.folder) byRoutine.delete(routineId)
      }
      for (const { routineId, folder } of watching) {
        const path = watchedPath(options.projectFolder, folder)
        if (path === undefined) continue
        const watched = byRoutine.get(routineId) ?? { folder, seen: new Set(), settling: new Map(), pending: [], held: [], fires: [], baselined: false }
        byRoutine.set(routineId, watched)
        let names: string[]
        try {
          names = (await readdir(path, { withFileTypes: true }))
            .filter((entry) => entry.isFile() && !PASSING.test(entry.name))
            .map((entry) => entry.name)
            .slice(0, MAX_ENTRIES)
        } catch {
          // Not there yet: a folder made later is watched from the moment it appears.
          if (!watched.baselined) continue
          names = []
        }
        if (!watched.baselined) {
          for (const name of names) watched.seen.add(name)
          watched.baselined = true
          continue
        }
        for (const name of names) {
          if (watched.seen.has(name)) continue
          const measured = await stat(join(path, name)).catch(() => undefined)
          if (measured === undefined) continue
          const held = watched.settling.get(name)
          if (held === undefined || held.size !== measured.size || held.mtimeMs !== measured.mtimeMs) {
            watched.settling.set(name, { size: measured.size, mtimeMs: measured.mtimeMs, since: now })
            continue
          }
          if (now - held.since < SETTLE_MS) continue
          watched.settling.delete(name)
          watched.seen.add(name)
          if (watched.held.length + watched.pending.length < MAX_PENDING) watched.held.push(name)
        }
        // A name that went away while settling is let go.
        for (const name of [...watched.settling.keys()]) if (!names.includes(name)) watched.settling.delete(name)
        // Runs quiet long enough for every turn to have said what it wrote: what is held came from someone else.
        if (quiet && watched.held.length > 0) {
          watched.pending.push(...watched.held.filter((name) => !watched.pending.includes(name)))
          watched.held = []
        }
      }
    },
    wrote(workspace, paths) {
      for (const watched of byRoutine.values()) {
        const folder = watchedPath(options.projectFolder, watched.folder)
        if (folder === undefined) continue
        for (const path of paths) {
          const full = resolve(workspace, path)
          if (!samePath(dirname(full), folder)) continue
          const name = basename(full)
          // Seen, so it is never new; and out of whatever it was waiting in.
          watched.seen.add(name)
          watched.settling.delete(name)
          watched.held = watched.held.filter((one) => one !== name)
          watched.pending = watched.pending.filter((one) => one !== name)
        }
      }
    },
    ready(routineId, now = Date.now()) {
      const watched = byRoutine.get(routineId)
      if (watched === undefined || watched.pending.length === 0) return []
      watched.fires = watched.fires.filter((at) => now - at < 3_600_000)
      return watched.fires.length >= MAX_FIRES_PER_HOUR ? [] : [...watched.pending]
    },
    fired(routineId, files, now = Date.now()) {
      const watched = byRoutine.get(routineId)
      if (watched === undefined) return
      watched.pending = watched.pending.filter((name) => !files.includes(name))
      watched.fires.push(now)
    }
  }
}

/** What step 1 is told, before its own words: the files, by name, inside the folder. */
export function arrivalNote(folder: string, files: readonly string[]): string {
  const where = folder.replace(/\\/g, '/')
  const named = files.map((name) => `${where}/${name}`).join(', ')
  return files.length === 1
    ? `A new file just arrived: ${named}. This run is for it.`
    : `${String(files.length)} new files just arrived: ${named}. This run is for them.`
}
