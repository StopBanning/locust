import { spawn } from 'node:child_process'
import { realpathSync } from 'node:fs'
import { dirname, sep } from 'node:path'

import { releaseProcessTree } from '@teammate/runtime-adapters'
import type { RuntimeDiscovery } from '@teammate/runtime-adapters'

import type { RuntimeUpdateStatus, RuntimeUpdateView, RuntimeUpdatesState } from '../shared/ipc.js'
import { DOWNLOADS } from '../shared/locust-home.js'
import { runtimeInstallFacts } from '../shared/runtime-install.js'

/**
 * KEEPING THE CODING AGENTS CURRENT.
 *
 * Colin, 2026-09-23: "new gpt-6 models released we need those added", and
 * then "is there a way to make it so the models will automatically update
 * without messing up load times or interfering with the app". The model lists
 * are already read live from each agent (model-catalog.ts); what goes stale is
 * the agent. Codex CLI 0.153.0 listed GPT-6-Astra; 0.156.1 adds GPT-6-Sol and
 * GPT-6-Luna. Claude Code, OpenCode and Cursor Agent keep themselves current;
 * Codex and Copilot, installed with npm, never do -- and Locust runs them
 * headless, so the notice Codex prints about a newer version is seen by no one.
 *
 * So, for those two:
 *
 * - LOOK, never on the way up. The first look waits until the app has been up
 *   a while (FIRST_LOOK_AFTER_MS), and again at most every CHECK_EVERY_MS. A
 *   look is one `npm view` per agent, and what it saw is kept on disk, so a
 *   launch inside that window goes to the network not at all.
 * - UPDATE ONLY WHAT NOTHING IS USING. No process may be running from the
 *   agent's own folder -- every mission Locust runs on it, and anything else
 *   on this machine that runs that copy. (The Codex app keeps a copy of its
 *   own under AppData\Local\OpenAI, and running it does not hold this one.)
 *   And only a release out for RELEASE_AGE_MS: one pulled within hours of
 *   going out never lands here.
 * - ONLY WHAT NPM PUT THERE. An agent found somewhere npm did not install it
 *   is left alone: `npm install -g` would put a second copy beside it.
 * - Then the machine is asked again and the models re-read, so the new ones
 *   are in the picker without a restart.
 *
 * A switch in Settings turns it off; off, a newer version is still reported.
 *
 * AND IT ASKS FIRST (0.303). A beta tester, the evening 0.302 went out: "when
 * I sent the prompt everything on the computer disconnected from internet ...
 * I stopped it and closed locust and everything went back to normal", in the
 * middle of a Google Meet, with Locust saying "reconnecting". Codex CLI's
 * update is one npm package of 159 MB (0.156.1's win32 tarball, measured), and
 * 0.302 started it by itself 45 s after launch: on an ordinary home line that
 * is a minute or two of the whole connection. Nothing on this machine can
 * know that someone is on a call, or on a slow line. So the LOOK stays
 * automatic -- a few kilobytes -- and the DOWNLOAD is the person's: the row
 * says a newer version is out and offers Update. Updating on its own is a
 * switch they turn on, and it starts OFF -- a new saved key, so every 0.302
 * install that had it on by default starts off again.
 *
 * ON AGAIN BY DEFAULT (0.304). Colin: "automatic updating for the models
 * should be fine, i dont think thats what knocked out his internet" -- and
 * the tester's connection went again, and again only on Codex runs, which
 * one download cannot explain. So it updates on its own unless the person
 * turned that off; the Update button stays for anyone who does. Only a
 * choice the person made is kept (`chosen`): 0.303 saved OFF for everyone
 * who never touched the switch, and that is not a choice.
 */

/**
 * Whether this launch may change the machine's agents at all. A script drives
 * the app with `--remote-debugging-port` -- more than seventy drives do, each
 * on a throwaway profile -- and a drive that updated the Codex every other
 * session on this machine runs would be a test with a side effect no one
 * asked for. `LOCUST_UPDATE_AGENTS=1` lets a drive that is about updating do
 * it (drive-agents-kept-current points npm at a folder of its own first).
 */
export function mayUpdateAgents(
  argv: readonly string[],
  environment: Readonly<Record<string, string | undefined>>,
  /**
   * False for a packaged copy that is not the installed Locust (0.514): a
   * tester's copy in a temp folder changes nothing on the machine, the rule
   * 0.507 made for Locust's own updates. A first-hour pass on 0.512 found the
   * switch on in a fresh copy's Settings.
   */
  installed = true
): boolean {
  if (environment.LOCUST_UPDATE_AGENTS === '1') return true
  return installed && !argv.some((argument) => argument.startsWith('--remote-debugging-port'))
}

/** What a saved file holds, checked field by field: anything else is nothing saved. */
export function savedUpdatesFrom(value: unknown): SavedUpdates | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  const latest: Record<string, Release> = {}
  if (typeof record.latest === 'object' && record.latest !== null) {
    for (const [name, release] of Object.entries(record.latest as Record<string, unknown>)) {
      if (typeof release !== 'object' || release === null) continue
      const { version, publishedAt } = release as Record<string, unknown>
      if (typeof version === 'string') latest[name] = { version, publishedAt: typeof publishedAt === 'string' ? publishedAt : undefined }
    }
  }
  const last: Record<string, RuntimeUpdateStatus> = {}
  if (typeof record.last === 'object' && record.last !== null) {
    for (const [runtime, status] of Object.entries(record.last as Record<string, unknown>)) {
      if (typeof status !== 'object' || status === null) continue
      const entry = status as Record<string, unknown>
      if (entry.kind === 'updated' && typeof entry.from === 'string' && typeof entry.to === 'string' && typeof entry.at === 'string') {
        last[runtime] = { kind: 'updated', from: entry.from, to: entry.to, at: entry.at }
      } else if (entry.kind === 'failed' && typeof entry.version === 'string' && typeof entry.what === 'string' && typeof entry.at === 'string') {
        last[runtime] = { kind: 'failed', version: entry.version, what: entry.what, at: entry.at }
      }
    }
  }
  // What the person chose, when they chose; the default otherwise -- 0.303
  // wrote `automatic: false` for everyone, and 0.302 its own `enabled`.
  const chosen = record.chosen === true && typeof record.automatic === 'boolean'
  const held: Record<string, string[]> = {}
  if (typeof record.held === 'object' && record.held !== null) {
    for (const [name, versions] of Object.entries(record.held as Record<string, unknown>)) {
      if (Array.isArray(versions)) held[name] = versions.filter((version): version is string => typeof version === 'string')
    }
  }
  return {
    ...(Object.keys(held).length === 0 ? {} : { held }),
    automatic: chosen ? record.automatic === true : NOTHING_SAVED.automatic,
    chosen,
    checkedAt: typeof record.checkedAt === 'number' && Number.isFinite(record.checkedAt) ? record.checkedAt : undefined,
    latest,
    last
  }
}

export const KEPT_CURRENT = ['codex', 'copilot'] as const
export type KeptCurrent = (typeof KEPT_CURRENT)[number]

export const FIRST_LOOK_AFTER_MS = 45_000
export const CHECK_EVERY_MS = 6 * 60 * 60 * 1000
export const RELEASE_AGE_MS = 12 * 60 * 60 * 1000

export interface Release {
  readonly version: string
  /** When npm says it went out; undefined when it did not say. */
  readonly publishedAt: string | undefined
}

/** What is kept on disk between launches. */
export interface SavedUpdates {
  /** Updates without being asked. On unless the person turned it off. */
  readonly automatic: boolean
  /** Whether `automatic` is the person's own choice rather than the default. */
  readonly chosen: boolean
  readonly checkedAt: number | undefined
  /** The newest release seen, by package. */
  readonly latest: Readonly<Record<string, Release>>
  /** What last happened to each agent that is worth saying again: an update, or a failed one. */
  readonly last: Readonly<Record<string, RuntimeUpdateStatus>>
  /**
   * Versions held back, by package (0.501): the ones Locust's release check
   * (_tools/runtime-canary.mjs) ran through a real turn and could not read.
   * Read with each look; absent when the check could not be reached.
   */
  readonly held?: Readonly<Record<string, readonly string[]>>
}

export const NOTHING_SAVED: SavedUpdates = { automatic: true, chosen: false, checkedAt: undefined, latest: {}, last: {} }

/**
 * Newer, older or the same: dotted numbers compared as numbers, and a
 * prerelease below its release (`0.158.0-alpha.4` < `0.158.0`).
 */
export function compareVersions(left: string, right: string): number {
  const split = (version: string): { readonly parts: readonly number[]; readonly pre: string | undefined } => {
    const [core = '', ...rest] = version.trim().replace(/^v/i, '').split('-')
    return { parts: core.split('.').map((part) => Number.parseInt(part, 10) || 0), pre: rest.length === 0 ? undefined : rest.join('-') }
  }
  const a = split(left)
  const b = split(right)
  for (let index = 0; index < Math.max(a.parts.length, b.parts.length); index += 1) {
    const difference = (a.parts[index] ?? 0) - (b.parts[index] ?? 0)
    if (difference !== 0) return Math.sign(difference)
  }
  if (a.pre === b.pre) return 0
  if (a.pre === undefined) return 1
  if (b.pre === undefined) return -1
  // Prerelease identifiers one by one, numbers as numbers: alpha.10 is after alpha.9 (2026-10-10 sweep).
  const ap = a.pre.split('.')
  const bp = b.pre.split('.')
  for (let index = 0; index < Math.max(ap.length, bp.length); index += 1) {
    const x = ap[index]
    const y = bp[index]
    if (x === undefined) return -1
    if (y === undefined) return 1
    const xn = /^\d+$/.test(x)
    const yn = /^\d+$/.test(y)
    if (xn && yn && Number(x) !== Number(y)) return Math.sign(Number(x) - Number(y))
    if (xn !== yn) return xn ? -1 : 1
    if (!xn && x !== y) return x < y ? -1 : 1
  }
  return 0
}

/**
 * What to do about one agent, from what is known -- the whole rule, alone,
 * so it is tested alone.
 */
export function decide(input: {
  readonly installed: string | undefined
  readonly latest: Release | undefined
  readonly automatic: boolean
  readonly inUse: boolean
  readonly now: number
  /** Locust's release check could not read this version (0.501). */
  readonly held?: boolean
}): RuntimeUpdateStatus | { readonly kind: 'update'; readonly version: string } {
  const { installed, latest } = input
  if (installed === undefined || latest === undefined || compareVersions(latest.version, installed) <= 0) return { kind: 'current' }
  // Not automatic: it waits for the person to press Update.
  if (!input.automatic) return { kind: 'waiting', version: latest.version, why: 'ask' }
  // A release Locust could not read is never installed on its own, however old.
  if (input.held === true) return { kind: 'waiting', version: latest.version, why: 'held' }
  const published = latest.publishedAt === undefined ? Number.NaN : Date.parse(latest.publishedAt)
  // A release npm gives no date for is treated as brand new: it waits.
  if (!Number.isFinite(published) || input.now - published < RELEASE_AGE_MS) return { kind: 'waiting', version: latest.version, why: 'too new' }
  if (input.inUse) return { kind: 'waiting', version: latest.version, why: 'in use' }
  return { kind: 'update', version: latest.version }
}

/** Where a link really points, or nothing when it cannot be read. */
const realPathOf = (path: string): string | undefined => {
  try {
    return realpathSync(path)
  } catch {
    return undefined
  }
}

/**
 * Where npm keeps an agent it installed, when it did: the package's own folder.
 *
 * AND ON A MAC OR LINUX (the first Mac run, 2026-10-30): npm's shim there is a link in `<prefix>/bin` to the
 * package's own script, with `npm root -g` at `<prefix>/lib/node_modules`, so the Windows test -- a shim beside
 * node_modules -- never matched, and codex from npm was never updated. The link is followed instead. Homebrew's
 * own codex sits in the same bin, but its link leads into Homebrew's Cellar, not npm's folder, so it is left alone.
 */
export function npmPackageDir(discovery: RuntimeDiscovery, npmRoot: string, realPath: (path: string) => string | undefined = realPathOf): string | undefined {
  const facts = runtimeInstallFacts(discovery.id)
  if (facts === undefined || facts.install.kind !== 'npm' || discovery.executable === undefined) return undefined
  // Joined as npmRoot is spelled, so a Mac's root read on any machine stays a Mac's.
  const folder = [npmRoot, ...facts.install.packageName.split('/')].join(npmRoot.includes('/') && !npmRoot.includes('\\') ? '/' : sep)
  const prefix = dirname(npmRoot)
  const executable = discovery.executable
  const same = (a: string, b: string): boolean => a.toLowerCase().replace(/[\\/]+/g, '/') === b.toLowerCase().replace(/[\\/]+/g, '/')
  const inside = (path: string, dir: string): boolean => path.toLowerCase().replace(/[\\/]+/g, '/').startsWith(`${dir.toLowerCase().replace(/[\\/]+/g, '/')}/`)
  // npm's own shim in its own prefix, a path inside the package itself, or a link that leads into it.
  const npmPut = same(dirname(executable.discoveredPath), prefix)
    || [executable.executablePath, ...executable.prefixArgs].some((path) => inside(path, folder))
    || inside(realPath(executable.discoveredPath) ?? '', folder)
  return npmPut ? folder : undefined
}

export interface RuntimeUpdatesOptions {
  readonly discover: () => Promise<readonly RuntimeDiscovery[]>
  /** npm's global folder (`npm root -g`); undefined when there is no npm to ask. */
  readonly npmRoot: () => Promise<string | undefined>
  readonly latest: (packageName: string) => Promise<Release | undefined>
  /**
   * The versions Locust's release check could not read, by package (0.501);
   * undefined when it could not be asked, which holds nothing back.
   */
  readonly heldVersions?: () => Promise<Readonly<Record<string, readonly string[]>> | undefined>
  /** Whether any process on this machine is running from inside `dir`. */
  readonly inUse: (dir: string) => Promise<boolean>
  readonly install: (runtime: string, version: string) => Promise<{ readonly ok: true } | { readonly ok: false; readonly what: string }>
  readonly load: () => Promise<SavedUpdates | undefined>
  readonly save: (saved: SavedUpdates) => Promise<void>
  /** An agent was updated: ask the machine again, re-read the models. */
  readonly updated: (runtime: string) => void
  /** Anything a window shows changed. */
  readonly changed: () => void
  readonly now?: () => number
}

export interface RuntimeUpdates {
  state(): Promise<RuntimeUpdatesState>
  setAutomatic(automatic: boolean): Promise<RuntimeUpdatesState>
  /** The person pressed Update: this agent, now, unless something is using it. */
  updateNow(runtime: string): Promise<RuntimeUpdatesState>
  /** Look, if a look is due; then update whatever is behind and not in use. */
  tick(): Promise<void>
  /** The agent being updated right now, if one is: a mission must not start on it. */
  updating(): string | undefined
}

export function createRuntimeUpdates(options: RuntimeUpdatesOptions): RuntimeUpdates {
  const now = options.now ?? Date.now
  let saved: SavedUpdates | undefined
  let current: string | undefined
  let ticking: Promise<void> | undefined
  let views: readonly RuntimeUpdateView[] = []

  const held = async (): Promise<SavedUpdates> => {
    if (saved === undefined) saved = (await options.load().catch(() => undefined)) ?? NOTHING_SAVED
    return saved
  }
  const keep = async (next: SavedUpdates): Promise<void> => {
    saved = next
    await options.save(next).catch(() => undefined)
  }

  interface Agent {
    readonly id: string
    readonly installed: string
    readonly dir: string
    readonly packageName: string
  }
  const agentsNow = async (): Promise<readonly Agent[] | undefined> => {
    const runtimes = await options.discover()
    const root = await options.npmRoot().catch(() => undefined)
    if (root === undefined) return undefined
    return KEPT_CURRENT.flatMap((id) => {
      const discovery = runtimes.find((entry) => entry.id === id)
      const dir = discovery === undefined ? undefined : npmPackageDir(discovery, root)
      const packageName = runtimeInstallFacts(id)?.install
      return discovery?.version !== undefined && dir !== undefined && packageName?.kind === 'npm'
        ? [{ id, installed: discovery.version.version, dir, packageName: packageName.packageName }]
        : []
    })
  }
  /** One update, reported as it starts and as it ends; the view it leaves. */
  const updateOne = async (agent: Agent, version: string, others: readonly RuntimeUpdateView[]): Promise<RuntimeUpdateView> => {
    current = agent.id
    views = [...others, { runtime: agent.id, installed: agent.installed, latest: version, status: { kind: 'updating', version } }]
    options.changed()
    const outcome = await options.install(agent.id, version).catch((error: unknown) => ({ ok: false as const, what: error instanceof Error ? error.message : String(error) }))
    current = undefined
    const at = new Date(now()).toISOString()
    const status: RuntimeUpdateStatus = outcome.ok
      ? { kind: 'updated', from: agent.installed, to: version, at }
      : { kind: 'failed', version, what: outcome.what, at }
    const state = await held()
    await keep({ ...state, last: { ...state.last, [agent.id]: status } })
    if (outcome.ok) options.updated(agent.id)
    return { runtime: agent.id, installed: outcome.ok ? version : agent.installed, latest: version, status }
  }

  const run = async (): Promise<void> => {
    let state = await held()
    const agents = await agentsNow()
    if (agents === undefined) return
    // Look, if due.
    if (state.checkedAt === undefined || now() - state.checkedAt >= CHECK_EVERY_MS) {
      const latest: Record<string, Release> = { ...state.latest }
      for (const agent of agents) {
        const release = await options.latest(agent.packageName).catch(() => undefined)
        if (release !== undefined) latest[agent.packageName] = release
      }
      const held = await options.heldVersions?.().catch(() => undefined)
      state = { ...state, checkedAt: now(), latest, ...(held === undefined ? {} : { held }) }
      await keep(state)
    }
    const next: RuntimeUpdateView[] = []
    for (const agent of agents) {
      const release = state.latest[agent.packageName]
      const ask = (inUse: boolean): ReturnType<typeof decide> =>
        decide({ installed: agent.installed, latest: release, automatic: state.automatic, inUse, now: now(), held: release !== undefined && (state.held?.[agent.packageName] ?? []).includes(release.version) })
      // The machine's process table is read only when it is the last question:
      // an update would go ahead if nothing were using the agent.
      const provisional = ask(false)
      const verdict = provisional.kind === 'update' ? ask(await options.inUse(agent.dir).catch(() => true)) : provisional
      if (verdict.kind !== 'update') {
        // A past update is still worth saying until something newer is out.
        const last = state.last[agent.id]
        next.push({ runtime: agent.id, installed: agent.installed, latest: release?.version, status: verdict.kind === 'current' && last !== undefined ? last : verdict })
        continue
      }
      next.push(await updateOne(agent, verdict.version, next))
      state = await held()
    }
    views = next
    options.changed()
  }

  const service: RuntimeUpdates = {
    async state() {
      const state = await held()
      return { automatic: state.automatic, checkedAt: state.checkedAt === undefined ? undefined : new Date(state.checkedAt).toISOString(), agents: views }
    },
    async setAutomatic(automatic) {
      await keep({ ...(await held()), automatic, chosen: true })
      // Turned on, whatever was waiting on it goes now, not at the next look.
      if (automatic) void this.tick()
      return this.state()
    },
    async updateNow(runtime) {
      // One lock for both (L12): a tick that came while this installed ran the
      // same install beside it. This waits for a tick, then holds the lock
      // itself, so a tick meanwhile waits for it.
      while (ticking !== undefined) await ticking.catch(() => undefined)
      let unlock!: () => void
      ticking = new Promise<void>((resolve) => {
        unlock = resolve
      })
      try {
        return await pressUpdate(runtime)
      } finally {
        ticking = undefined
        unlock()
      }
    },
    tick() {
      if (ticking !== undefined) return ticking
      ticking = run().finally(() => {
        ticking = undefined
      })
      return ticking
    },
    updating: () => current
  }

  async function pressUpdate(runtime: string): Promise<RuntimeUpdatesState> {
    const agent = (await agentsNow())?.find((entry) => entry.id === runtime)
    const state = await held()
    const release = agent === undefined ? undefined : state.latest[agent.packageName]
    if (agent === undefined || release === undefined || compareVersions(release.version, agent.installed) <= 0 || current !== undefined) return service.state()
    const others = views.filter((view) => view.runtime !== runtime)
    if (await options.inUse(agent.dir).catch(() => true)) {
      views = [...others, { runtime, installed: agent.installed, latest: release.version, status: { kind: 'waiting', version: release.version, why: 'in use' } }]
      options.changed()
      return service.state()
    }
    const view = await updateOne(agent, release.version, others)
    views = [...others, view]
    options.changed()
    return service.state()
  }
  return service
}

/** One command, no shell, its output, and a time limit. */
function capture(command: string, args: readonly string[], timeoutMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, [...args], { windowsHide: true, shell: false })
    let out = ''
    const timer = setTimeout(() => {
      // The whole tree on Windows: npm is a .cmd, so the child is cmd.exe,
      // and killing it alone left a hung `npm view` running (a B4 lead).
      // Root last: once it is gone, taskkill cannot find the tree by it.
      if (process.platform === 'win32' && child.pid !== undefined) {
        void releaseProcessTree(child.pid).catch(() => false).finally(() => child.kill())
      } else {
        child.kill()
      }
      reject(new Error(`${command} took longer than ${String(timeoutMs / 1000)} s`))
    }, timeoutMs)
    child.stdout?.on('data', (chunk: Buffer) => {
      if (out.length < 4_000_000) out += chunk.toString('utf8')
    })
    child.on('error', (error) => {
      clearTimeout(timer)
      reject(error)
    })
    child.on('close', (code) => {
      clearTimeout(timer)
      if (code === 0) resolve(out)
      else reject(new Error(`${command} exited ${String(code)}`))
    })
  })
}

/**
 * What npm says the newest release of a package is, and when it went out.
 * `npm` on Windows is a .cmd, which only a shell runs: it goes through
 * cmd.exe with fixed, quoted arguments -- a package name from the facts table,
 * never anything typed.
 */
/**
 * Where Locust's release check publishes what it found (0.501), beside the
 * installers on Locust's own downloads since 0.783 (shared/locust-home.ts):
 * `{ "packages": { "<npm package>": { "<version>": { "ok": boolean } } } }`.
 */
export const CANARY_VERDICTS_URL = `${DOWNLOADS}runtime-canary.json`

/** The versions the check ran and could not read, by package: a few kilobytes, asked with each look. */
export function heldFrom(value: unknown): Readonly<Record<string, readonly string[]>> {
  const packages = typeof value === 'object' && value !== null ? (value as { packages?: unknown }).packages : undefined
  const held: Record<string, string[]> = {}
  if (typeof packages !== 'object' || packages === null) return held
  for (const [name, versions] of Object.entries(packages as Record<string, unknown>)) {
    if (typeof versions !== 'object' || versions === null) continue
    const failed = Object.entries(versions as Record<string, unknown>)
      .filter(([, verdict]) => typeof verdict === 'object' && verdict !== null && (verdict as { ok?: unknown }).ok === false)
      .map(([version]) => version)
    if (failed.length > 0) held[name] = failed
  }
  return held
}

export async function canaryHeldVersions(): Promise<Readonly<Record<string, readonly string[]>> | undefined> {
  try {
    const answer = await fetch(CANARY_VERDICTS_URL, { signal: AbortSignal.timeout(10_000) })
    return answer.ok ? heldFrom(await answer.json()) : undefined
  } catch {
    return undefined
  }
}

export async function npmRelease(packageName: string): Promise<Release | undefined> {
  const args = ['view', packageName, 'dist-tags.latest', 'time', '--json']
  const text = process.platform === 'win32'
    ? await capture(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', `npm ${args.join(' ')}`], 60_000)
    : await capture('npm', args, 60_000)
  const parsed: unknown = JSON.parse(text)
  if (typeof parsed !== 'object' || parsed === null) return undefined
  const record = parsed as Record<string, unknown>
  const version = record['dist-tags.latest']
  if (typeof version !== 'string' || version.length === 0) return undefined
  const times = record.time
  const publishedAt = typeof times === 'object' && times !== null ? (times as Record<string, unknown>)[version] : undefined
  return { version, publishedAt: typeof publishedAt === 'string' ? publishedAt : undefined }
}

/** npm's global folder, where `npm install -g` puts packages. */
export async function npmGlobalRoot(): Promise<string | undefined> {
  const text = process.platform === 'win32'
    ? await capture(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', 'npm root -g'], 30_000)
    : await capture('npm', ['root', '-g'], 30_000)
  const root = text.trim().split(/\r?\n/).pop()?.trim()
  return root === undefined || root.length === 0 ? undefined : root
}

/**
 * Whether any process on this machine is running from inside `dir`: its
 * executable there, or -- for an agent that is a node script -- that script
 * on its command line. Read from Windows' own process table.
 */
export async function processesUsing(dir: string): Promise<boolean> {
  const needle = dir.toLowerCase().replace(/[\\/]+/g, '\\')
  if (process.platform !== 'win32') {
    const text = await capture('ps', ['-eo', 'command'], 20_000)
    return text.toLowerCase().includes(dir.toLowerCase())
  }
  const text = await capture(
    'powershell.exe',
    // UTF-8 out: in the console's OEM code page a folder with any non-ASCII
    // letter never matched, so an agent in use read as free (L13).
    ['-NoProfile', '-NonInteractive', '-Command', '[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-CimInstance Win32_Process | ForEach-Object { "$($_.ExecutablePath)|$($_.CommandLine)" }'],
    30_000
  )
  return text.toLowerCase().replace(/[\\/]+/g, '\\').includes(needle)
}
