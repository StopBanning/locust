import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { accessSync, constants as fsConstants, createWriteStream, existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, posix } from 'node:path'

import { MAC_RELEASES_API, newerMacRelease, type MacRelease } from './mac-release.js'
import { removeStagedCopy } from './staged-copy.js'
import type { UpdaterLike } from './updates.js'

/**
 * A MAC LOCUST UPDATES ITSELF, LIKE THE WINDOWS ONE (0.516).
 *
 * macOS's own updater (Squirrel.Mac, behind electron-updater) installs only an
 * app signed with an Apple Developer ID, and the Mac builds are not -- so a
 * Mac copy could not update at all. Colin, 2026-10-01, for a tester on a Mac:
 * "try and make it work for him and just like how ours is".
 *
 * So Locust does it itself, behind the same `UpdaterLike` the Windows build
 * uses, and everything above it -- the check, the quiet download, the banner,
 * "Restart and install", the refusal while a teammate is working -- is the
 * one service (updates.ts), unchanged:
 *
 *   1. The newest release with this chip's disk image (mac-release.ts).
 *   2. Downloaded by Locust itself, and checked against the SHA-256 the
 *      releases API states. Not a browser download, so macOS does not mark
 *      it quarantined, and the new copy opens without the "could not verify"
 *      prompt the first install met.
 *   3. Its Locust.app copied, beside the running one, as `.Locust-update.app`.
 *   4. On "Restart and install": a small shell script, started detached,
 *      waits for Locust to exit, moves the old app aside, moves the new one
 *      into its place -- putting the old one back if that fails -- and opens
 *      it. Nothing about the person's data is touched: it is not in the app.
 *
 * Only where it can work (`macSelfUpdateTarget`): a packaged Locust.app whose
 * folder this user can write -- /Applications for an administrator, as a
 * drag-install leaves it -- and not one run from the disk image or from
 * macOS's translocation copy of a download.
 */

/** The Locust.app this process runs from, when it can replace itself there. */
export function macSelfUpdateTarget(execPath: string, canWrite: (folder: string) => boolean = writable): string | undefined {
  // /Applications/Locust.app/Contents/MacOS/Locust
  const bundle = posix.dirname(posix.dirname(posix.dirname(execPath)))
  if (!bundle.endsWith('.app')) return undefined
  if (bundle.startsWith('/Volumes/') || bundle.includes('/AppTranslocation/')) return undefined
  return canWrite(posix.dirname(bundle)) ? bundle : undefined
}

function writable(folder: string): boolean {
  try {
    accessSync(folder, fsConstants.W_OK)
    return true
  } catch {
    return false
  }
}

/** What runs a command and says how it ended. Injected so the steps are testable off a Mac. */
export type RunCommand = (command: string, args: readonly string[]) => Promise<{ readonly code: number | null; readonly output: string }>

const runCommand: RunCommand = (command, args) =>
  new Promise((resolve) => {
    const child = spawn(command, [...args], { stdio: ['ignore', 'pipe', 'pipe'] })
    let output = ''
    child.stdout?.on('data', (chunk: Buffer) => { output += chunk.toString('utf8') })
    child.stderr?.on('data', (chunk: Buffer) => { output += chunk.toString('utf8') })
    child.on('error', (error) => resolve({ code: null, output: `${output}${error.message}` }))
    child.on('close', (code) => resolve({ code, output }))
  })

/**
 * The swap, as the shell runs it once Locust has gone. Arguments: the exiting
 * process, the app, the staged copy. Every path is quoted; nothing is read
 * from anywhere but its own arguments.
 */
export const SWAP_SCRIPT = [
  '#!/bin/sh',
  '# Locust, updating itself on a Mac (mac-self-update.ts). Waits for the old app to exit,',
  '# puts the new one in its place, puts the old one back if that fails, and opens it.',
  'PID="$1"; APP="$2"; NEW="$3"; OLD="$APP.replaced"',
  'i=0',
  'while kill -0 "$PID" 2>/dev/null && [ "$i" -lt 600 ]; do sleep 0.2; i=$((i+1)); done',
  'rm -rf "$OLD"',
  'if mv "$APP" "$OLD"; then',
  '  if mv "$NEW" "$APP"; then rm -rf "$OLD"; else mv "$OLD" "$APP"; fi',
  'fi',
  'open "$APP"',
  ''
].join('\n')

export interface MacUpdaterOptions {
  readonly currentVersion: string
  readonly arch: string
  /** The Locust.app to replace (`macSelfUpdateTarget`). */
  readonly bundle: string
  /** Where downloads wait; created as needed. */
  readonly cacheFolder: string
  /** Quit the app once the swap is handed over. */
  readonly quit: () => void
  readonly pid?: number
  /** Test seams. */
  readonly fetchReleases?: () => Promise<unknown>
  readonly download?: (url: string, to: string, onPercent: (percent: number) => void) => Promise<void>
  readonly run?: RunCommand
  readonly startSwap?: (script: string, args: readonly string[]) => void
  /**
   * The Mac CI's own check that the swap works (mac-update-smoke.mjs): a disk
   * image on this machine offered as `version`, with no download. Never set
   * by the app itself.
   */
  readonly testImage?: { readonly path: string; readonly version: string }
}

export function createMacUpdater(options: MacUpdaterOptions): UpdaterLike {
  const listeners = new Map<string, ((payload?: unknown) => void)[]>()
  const emit = (event: string, payload?: unknown): void => {
    for (const listener of listeners.get(event) ?? []) listener(payload)
  }
  const run = options.run ?? runCommand
  const staged = join(dirname(options.bundle), '.Locust-update.app')
  let found: MacRelease | undefined
  let ready: string | undefined
  let downloading: Promise<unknown> | undefined

  const fetchReleases = options.fetchReleases ?? (async () => {
    const response = await fetch(MAC_RELEASES_API, { signal: AbortSignal.timeout(15_000) })
    if (!response.ok) throw new Error(`The releases list answered ${String(response.status)}.`)
    return response.json() as Promise<unknown>
  })

  const download = options.download ?? (async (url: string, to: string, onPercent: (percent: number) => void) => {
    const response = await fetch(url, { signal: AbortSignal.timeout(30 * 60 * 1000) })
    if (!response.ok || response.body === null) throw new Error(`The download answered ${String(response.status)}.`)
    const total = Number(response.headers.get('content-length') ?? 0)
    const out = createWriteStream(to)
    let received = 0
    let said = -1
    const reader = response.body.getReader()
    try {
      for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        received += value.byteLength
        if (!out.write(value)) await new Promise<void>((resolve) => out.once('drain', () => resolve()))
        const percent = total > 0 ? Math.floor((received / total) * 100) : 0
        if (percent !== said) {
          said = percent
          onPercent(percent)
        }
      }
    } finally {
      await new Promise<void>((resolve) => out.end(() => resolve()))
    }
  })

  /** The image, checked, then its app copied beside the running one. */
  const prepare = async (release: MacRelease): Promise<void> => {
    let image: string
    if (options.testImage !== undefined) {
      image = options.testImage.path
    } else {
      await mkdir(options.cacheFolder, { recursive: true })
      image = join(options.cacheFolder, `Locust-${release.version}-mac-${options.arch}.dmg`)
      await download(release.url, image, (percent) => emit('download-progress', { percent }))
      const facts = await stat(image)
      if (release.size !== undefined && facts.size !== release.size) throw new Error('The download is not the size the release states.')
      // No digest, no update (0.543, the 0.536 review SEC-06): GitHub states one
      // for every asset (measured on 0.532's dmgs), so its absence is a release
      // that cannot be checked, never one to install unchecked.
      if (release.sha256 === undefined) throw new Error('The release does not say what the download should be, so it was not installed.')
      const digest = createHash('sha256').update(await readFile(image)).digest('hex')
      if (digest !== release.sha256) throw new Error('The download does not match the release it came from.')
    }
    const mount = await mkdtemp(join(tmpdir(), 'locust-update-mount-'))
    const attached = await run('hdiutil', ['attach', image, '-nobrowse', '-readonly', '-noautoopen', '-mountpoint', mount])
    if (attached.code !== 0) throw new Error(`The disk image would not open: ${attached.output.trim().slice(0, 200)}`)
    try {
      const source = join(mount, 'Locust.app')
      if (!existsSync(source)) throw new Error('The disk image holds no Locust.app.')
      // Not this file's `rm`: Electron's fs cannot remove a folder holding an app.asar (staged-copy.ts).
      await removeStagedCopy(staged)
      const copied = await run('ditto', [source, staged])
      if (copied.code !== 0) throw new Error(`The new Locust could not be copied: ${copied.output.trim().slice(0, 200)}`)
      // Locust's own download carries no quarantine; a copy that somehow does would ask again.
      await run('xattr', ['-dr', 'com.apple.quarantine', staged])
    } finally {
      await run('hdiutil', ['detach', mount, '-force'])
      await rm(mount, { recursive: true, force: true }).catch(() => undefined)
    }
    ready = release.version
    emit('update-downloaded', { version: release.version })
  }

  const updater: UpdaterLike = {
    autoDownload: false,
    allowPrerelease: false,
    // Installing happens on "Restart and install" only: the swap needs the
    // app to be gone, and a quit from anywhere else is not a request for it.
    autoInstallOnAppQuit: false,
    async checkForUpdates() {
      const release = options.testImage !== undefined
        ? { version: options.testImage.version, url: '' }
        : newerMacRelease(await fetchReleases(), options.currentVersion, options.arch)
      if (release === undefined) return { updateInfo: { version: options.currentVersion }, isUpdateAvailable: false }
      found = release
      if (ready === release.version) {
        emit('update-downloaded', { version: release.version })
        return { updateInfo: { version: release.version }, isUpdateAvailable: true }
      }
      const downloadPromise = updater.autoDownload ? updater.downloadUpdate() : null
      return { updateInfo: { version: release.version }, isUpdateAvailable: true, downloadPromise }
    },
    downloadUpdate() {
      if (found === undefined) return Promise.reject(new Error('There is nothing to download.'))
      const release = found
      downloading ??= prepare(release).catch((error: unknown) => {
        emit('error', error)
        throw error
      }).finally(() => {
        downloading = undefined
      })
      return downloading
    },
    quitAndInstall() {
      if (ready === undefined || !existsSync(staged)) return
      const script = join(tmpdir(), `locust-swap-${String(Date.now())}.sh`)
      void writeFile(script, SWAP_SCRIPT, { encoding: 'utf8', mode: 0o755 }).then(() => {
        const args = [String(options.pid ?? process.pid), options.bundle, staged]
        if (options.startSwap !== undefined) options.startSwap(script, args)
        else spawn('/bin/sh', [script, ...args], { detached: true, stdio: 'ignore' }).unref()
        options.quit()
      })
    },
    on(event, listener) {
      listeners.set(event, [...(listeners.get(event) ?? []), listener])
      return updater
    }
  }
  return updater
}
