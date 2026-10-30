import { execFile } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, posix } from 'node:path'

import type { RuntimeDiscovery } from '@teammate/runtime-adapters'
import { parseAgyModelList, parseAntigravityProjects, projectIdForWorkspace } from '@teammate/runtime-adapters'

import { ANTIGRAVITY_TIER_NAMES } from '../shared/antigravity-models.js'

/**
 * Antigravity, EXPERIMENTAL.
 *
 * Google's Antigravity IDE runs its own agent inside a folder it has open. It
 * has no supported automation surface; what it has is `language_server.exe
 * agentapi`, a tiny CLI meant for its own sub-agents, which starts a
 * conversation and sends messages into one. Everything below was measured on
 * 2026-09-03 against Antigravity 2.11.0 and is reverse-engineered:
 *
 * - the server the CLI talks to is the running `language_server.exe` that
 *   Antigravity itself launched; its CSRF token is on that process's command
 *   line and its port is whichever of the process's listening ports answers
 *   an RPC (one of the two measured ports refuses with a connection error);
 * - `new-conversation` needs a PROJECT id for the workspace, which lives in a
 *   protobuf file with no schema (`agyhub_summaries_proto.pb`);
 * - the agent's work is then observable in a JSONL transcript per
 *   conversation, which the mission service polls.
 *
 * So this route only works while Antigravity is open, only for a folder it
 * has open, and it cannot be held read-only: Antigravity's agent runs its
 * own tools under its own policy. All of that is said in the UI, by name.
 */

export interface AntigravityHost {
  readonly executablePath: string
  readonly version: string | undefined
  readonly address: string
  readonly csrfToken: string
  readonly projects: ReadonlyMap<string, string>
  /**
   * Every port the server listens on. The CLI's is `address`; a question is
   * answered over the plain-HTTP one of the two (antigravity-cascade.ts).
   */
  readonly ports?: readonly number[]
}

export interface AntigravityProbeOptions {
  readonly localAppData?: string
  readonly home?: string
  readonly platform?: NodeJS.Platform
  /** Test seam: the PATH searched for `agy` off Windows (the app's own, which shellPath has built, by default). */
  readonly pathEnv?: string
  /**
   * Test seam: the pids of every `language_server.exe`, as `tasklist`
   * reports them in tens of milliseconds -- or `undefined` when it could not
   * say, which falls through to the PowerShell path below.
   */
  readonly listPids?: () => Promise<readonly number[] | undefined>
  /** Test seam: the process list as `Win32_Process` reports it. */
  readonly listProcesses?: () => Promise<readonly { readonly pid: number; readonly commandLine: string }[]>
  /** Test seam: listening TCP ports of a process. */
  readonly listeningPorts?: (pid: number) => Promise<readonly number[]>
  /** Test seam: the clock, so a test can step past the TTL. */
  readonly now?: () => number
  /** Test seam: run the CLI once. `timeoutMs` is how long it may take before it is killed. */
  readonly run?: (executable: string, args: readonly string[], env: Readonly<Record<string, string>>, timeoutMs?: number) => Promise<{ stdout: string; stderr: string; code: number | null; timedOut?: boolean }>
  /** Test seam: how long `agy` may take to answer the readiness check (default ten seconds, like every other probe). */
  readonly cliCheckCapMs?: number
}

// Named from the one table the window reads too (shared/antigravity-models.ts).
export const ANTIGRAVITY_MODELS = [
  { id: 'flash', displayName: ANTIGRAVITY_TIER_NAMES.flash!, description: 'Antigravity tier flash' },
  { id: 'pro', displayName: ANTIGRAVITY_TIER_NAMES.pro!, description: 'Antigravity tier pro' },
  { id: 'flash_lite', displayName: ANTIGRAVITY_TIER_NAMES.flash_lite!, description: 'Antigravity tier flash_lite' }
] as const

const PROBE_TTL_MS = 10_000

/*
 * How long `agy` may take to answer `--version` and `models` at a sweep.
 *
 * Every CLI probe in discovery has had ten seconds (PROBE_TIMEOUT_MS), and
 * this one ran with the sixty seconds the RUN of a command gets -- so an `agy`
 * that hung (a model list that waits on a network that is not there) held the
 * whole sweep, the first screen and every start behind it for a minute.
 */
const CLI_CHECK_CAP_MS = 10_000

export function antigravityExecutableCandidates(localAppData: string): readonly string[] {
  return [
    join(localAppData, 'Programs', 'antigravity', 'resources', 'bin', 'language_server.exe'),
    join(localAppData, 'Programs', 'Antigravity', 'resources', 'bin', 'language_server.exe')
  ]
}

export function antigravityDataDirectory(home: string): string {
  return join(home, '.gemini', 'antigravity')
}

export function transcriptPathFor(home: string, conversationId: string): string {
  return join(antigravityDataDirectory(home), 'brain', conversationId, '.system_generated', 'logs', 'transcript.jsonl')
}

/** The CSRF token and version Antigravity put on its own server's command line. */
export function parseServerCommandLine(commandLine: string): { readonly csrfToken: string; readonly version: string | undefined } | undefined {
  if (!/--override_ide_name antigravity\b/.test(commandLine)) return undefined
  const token = /--csrf_token\s+([0-9a-f-]{36})/i.exec(commandLine)?.[1]
  if (token === undefined) return undefined
  const version = /--override_ide_version\s+(\S+)/.exec(commandLine)?.[1]
  return { csrfToken: token, version }
}

function runCli(
  executable: string,
  args: readonly string[],
  env: Readonly<Record<string, string>>,
  timeoutMs = 60_000
): Promise<{ stdout: string; stderr: string; code: number | null; timedOut?: boolean }> {
  return new Promise((resolve) => {
    execFile(
      executable,
      [...args],
      { env: { ...process.env, ...env }, timeout: timeoutMs, maxBuffer: 4 * 1024 * 1024, windowsHide: true },
      (error, stdout, stderr) => {
        const code = error === null ? 0 : typeof (error as { code?: unknown }).code === 'number' ? ((error as { code: number }).code) : null
        // Killed by its own clock: not an answer of any kind, and not "signed out".
        const timedOut = error !== null && (error as { killed?: unknown }).killed === true
        resolve({ stdout: String(stdout), stderr: String(stderr), code, ...(timedOut ? { timedOut: true } : {}) })
      }
    )
  })
}

function powershell(script: string): Promise<string> {
  return new Promise((resolve) => {
    execFile(
      'powershell.exe',
      ['-NoProfile', '-NonInteractive', '-Command', script],
      { timeout: 20_000, maxBuffer: 4 * 1024 * 1024, windowsHide: true },
      (_error, stdout) => resolve(String(stdout))
    )
  })
}

async function listProcessesWindows(): Promise<readonly { readonly pid: number; readonly commandLine: string }[]> {
  // Filtered by WMI itself, not piped through Where-Object: measured 328 ms
  // against 392 ms for the same answer (2026-09-22).
  const out = await powershell(
    "Get-CimInstance Win32_Process -Filter \"Name LIKE 'language_server%'\" | ForEach-Object { \"$($_.ProcessId)`t$($_.CommandLine)\" }"
  )
  return out
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => {
      const tab = line.indexOf('\t')
      return { pid: Number(line.slice(0, tab)), commandLine: line.slice(tab + 1) }
    })
    .filter((entry) => Number.isFinite(entry.pid))
}

/**
 * The ports a process listens on, from `netstat -ano`.
 *
 * It was `Get-NetTCPConnection`, which loads a PowerShell module before it
 * answers: 724 ms measured on Colin's machine (2026-09-22), the largest part
 * of Antigravity's 1.7 s check on every launch. `netstat -ano -p TCP` says
 * the same in 48 ms.
 *
 * Read by SHAPE, not by word: netstat translates its state column ("LISTENING"
 * is "ABHÖREN" in German), so a listening socket is told by its empty remote
 * end -- `0.0.0.0:0`, or `[::]:0` -- which every locale prints the same.
 */
export function listeningPortsIn(netstat: string, pid: number): readonly number[] {
  const ports = new Set<number>()
  for (const line of netstat.split(/\r?\n/)) {
    const columns = line.trim().split(/\s+/)
    if (columns.length < 5 || !/^tcp/i.test(columns[0] ?? '')) continue
    if (Number(columns[columns.length - 1]) !== pid) continue
    const remote = columns[2] ?? ''
    if (!/^(0\.0\.0\.0|\[::\]):0$/.test(remote)) continue
    const local = columns[1] ?? ''
    const port = Number(local.slice(local.lastIndexOf(':') + 1))
    if (Number.isInteger(port) && port > 0) ports.add(port)
  }
  return [...ports]
}

function listeningPortsWindows(pid: number): Promise<readonly number[]> {
  return new Promise((resolve) => {
    execFile('netstat.exe', ['-ano', '-p', 'TCP'], { timeout: 10_000, maxBuffer: 4 * 1024 * 1024, windowsHide: true }, (_error, stdout) =>
      resolve(listeningPortsIn(String(stdout), pid))
    )
  })
}

/**
 * Every `language_server.exe` pid, from `tasklist` -- tens of milliseconds
 * where each PowerShell start is one to three seconds cold. `undefined` when
 * tasklist itself failed, so the caller falls back rather than concluding
 * "not open" from a tool that did not answer.
 */
function listLanguageServerPidsWindows(): Promise<readonly number[] | undefined> {
  return new Promise((resolve) => {
    execFile(
      'tasklist.exe',
      ['/FI', 'IMAGENAME eq language_server.exe', '/FO', 'CSV', '/NH'],
      { timeout: 10_000, maxBuffer: 1024 * 1024, windowsHide: true },
      (error, stdout) => {
        if (error !== null) {
          resolve(undefined)
          return
        }
        // `"language_server.exe","12345","Console","1","123,456 K"`; with no
        // match tasklist prints an INFO sentence instead of a CSV row.
        const pids = String(stdout)
          .split(/\r?\n/)
          .map((line) => /^"language_server\.exe","(\d+)"/i.exec(line.trim())?.[1])
          .filter((pid): pid is string => pid !== undefined)
          .map((pid) => Number(pid))
        resolve(pids)
      }
    )
  })
}

const samePids = (left: readonly number[], right: readonly number[]): boolean =>
  left.length === right.length && [...left].sort((a, b) => a - b).every((pid, index) => pid === [...right].sort((a, b) => a - b)[index])

/**
 * Ask one thing, and stop waiting at `capMs` whatever it does: a clock of this
 * function's own, because a fake or a wedged runner has no clock of its own.
 * No answer is `undefined`.
 */
async function askWithin<T>(capMs: number, ask: () => Promise<T>): Promise<T | undefined> {
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      ask().catch(() => undefined),
      new Promise<undefined>((resolve) => {
        timer = setTimeout(() => resolve(undefined), capMs)
      })
    ])
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}

export function createAntigravityHostProbe(options: AntigravityProbeOptions = {}): {
  probe(): Promise<AntigravityHost | undefined>
  /** The check itself. Asked twice at once, it is run once: the second caller gets the first one's answer. */
  discoveryRecord(): Promise<RuntimeDiscovery>
  /**
   * What to show for Antigravity while its check is still going: installed,
   * not answered. Never a claim about signing in, and never the last launch's
   * answer dressed as this one.
   */
  pendingRecord(): RuntimeDiscovery
  /** Antigravity CLI (`agy`), when installed (0.540): runs go through it instead of the app. */
  cliPath(): string | undefined
} {
  const platform = options.platform ?? process.platform
  const localAppData = options.localAppData ?? process.env.LOCALAPPDATA ?? ''
  const home = options.home ?? homedir()
  const listPids = options.listPids ?? listLanguageServerPidsWindows
  const listProcesses = options.listProcesses ?? listProcessesWindows
  const listeningPorts = options.listeningPorts ?? listeningPortsWindows
  const run = options.run ?? runCli
  const now = options.now ?? Date.now
  let cached: { readonly at: number; readonly value: AntigravityHost | undefined; readonly pids: readonly number[] | undefined } | undefined

  /*
   * `LOCUST_HIDE_RUNTIMES=1` HAS TO REACH THIS PROBE TOO.
   *
   * The seam exists so `_tools/drive-first-run.mjs` can see what a person
   * with an empty laptop sees, on a machine that has all six agents. It works
   * by handing discovery an executable locator that finds nothing -- and this
   * probe does not use that locator. It looks for Antigravity's own install
   * directly with `existsSync`, so it stayed READY through every "bare
   * machine" drive.
   *
   * MEASURED 2026-09-21, driving 0.244.0: `drive-first-run.mjs` exited on
   * *"the composer is on Antigravity / Account Default"*, which is neither a
   * bare machine nor a free route. So the one tool whose whole job is the
   * first-run screen has not been driving the first-run screen since
   * Antigravity shipped -- and that screen is where eight of the nine
   * findings in Sol's beta review were, found on a separate Linux box
   * because this seam could not show them here.
   *
   * Read from `options` first so the unit tests keep their own seam, and the
   * environment only decides when nothing else has.
   */
  // Read at ASK time, not at construction: a probe built during module load
  // would latch whatever the environment said before the drive set it.
  const hidden = (): boolean => options.localAppData === undefined && process.env.LOCUST_HIDE_RUNTIMES === '1'
  /**
   * Where the Antigravity CLI installer puts `agy`: %LOCALAPPDATA%\agy\bin\agy.exe on Windows (install.ps1,
   * 2026-10-02). On a Mac and Linux, install.sh puts it in ~/.local/bin, or wherever its `--dir` said, so that
   * folder first and then PATH. Until the first Mac run (2026-10-30) only the Windows place was looked at, and a
   * signed-in agy 1.4.11 read as "not installed".
   */
  const agyPath = (): string | undefined => {
    if (hidden()) return undefined
    if (platform !== 'win32') {
      const folders = [posix.join(home, '.local', 'bin'), ...(options.pathEnv ?? process.env.PATH ?? '').split(':')]
      return folders.filter((folder) => posix.isAbsolute(folder)).map((folder) => posix.join(folder, 'agy')).find((file) => existsSync(file))
    }
    if (localAppData.length === 0) return undefined
    const candidate = join(localAppData, 'agy', 'bin', 'agy.exe')
    return existsSync(candidate) ? candidate : undefined
  }

  const probe = async (): Promise<AntigravityHost | undefined> => {
    if (hidden()) return undefined
    if (cached !== undefined && now() - cached.at < PROBE_TTL_MS) return cached.value
    if (platform !== 'win32') return undefined
    const executablePath = antigravityExecutableCandidates(localAppData).find((candidate) => existsSync(candidate))
    if (executablePath === undefined) return undefined
    /*
     * The cheap question first: is a language server running at all, and is
     * it the same one as last time?
     *
     * Two PowerShell round trips per sweep -- `Get-CimInstance Win32_Process`
     * and `Get-NetTCPConnection` -- were 4.2-5.5 s of every sweep on Colin's
     * machine (2026-09-21), for an answer that only changes when Antigravity
     * opens or closes. `tasklist` says in tens of milliseconds whether any
     * `language_server.exe` exists; the same pids as the last full look mean
     * the same server, the same port and the same token, so the last answer
     * stands. PowerShell runs only when the set of processes has changed.
     * (Fable's probing review, #7.)
     */
    const pids = await listPids()
    if (pids !== undefined) {
      if (pids.length === 0) {
        cached = { at: now(), value: undefined, pids }
        return undefined
      }
      if (cached?.value !== undefined && cached.pids !== undefined && samePids(cached.pids, pids)) {
        cached = { ...cached, at: now() }
        return cached.value
      }
    }
    const value = await (async () => {
      const servers = (await listProcesses())
        .map((entry) => ({ pid: entry.pid, parsed: parseServerCommandLine(entry.commandLine) }))
        .filter((entry): entry is { pid: number; parsed: NonNullable<ReturnType<typeof parseServerCommandLine>> } => entry.parsed !== undefined)
      for (const server of servers) {
        const ports = await listeningPorts(server.pid)
        /*
         * Every port asked AT ONCE, and the first that answers in port order
         * wins -- the same answer the one-at-a-time loop gave. The language
         * server listens on two here, and the loop paid a CLI start for the
         * wrong one before it tried the right one (2026-09-22).
         */
        const answers = await Promise.all(
          ports.map((port) =>
            // A probe that cannot succeed: the one thing it proves is whether
            // this port speaks the RPC at all. The wrong port answers with a
            // connection error; the right one with a clean "not found".
            run(executablePath, ['agentapi', 'get-conversation-metadata', '00000000-0000-0000-0000-000000000000'], {
              ANTIGRAVITY_LS_ADDRESS: `localhost:${String(port)}`,
              ANTIGRAVITY_CSRF_TOKEN: server.parsed.csrfToken
            })
          )
        )
        for (const [index, port] of ports.entries()) {
          const address = `localhost:${String(port)}`
          const result = answers[index]
          if (result === undefined) continue
          const text = `${result.stdout}\n${result.stderr}`
          if (/connection error|wsarecv|is not set/i.test(text)) continue
          let projects: ReadonlyMap<string, string> = new Map()
          try {
            projects = parseAntigravityProjects(readFileSync(join(antigravityDataDirectory(home), 'agyhub_summaries_proto.pb')))
          } catch {
            projects = new Map()
          }
          return { executablePath, version: server.parsed.version, address, csrfToken: server.parsed.csrfToken, projects, ports }
        }
      }
      return undefined
    })()
    cached = { at: now(), value, pids }
    return value
  }

  /** The check, once. Only `discoveryRecord` starts it. */
  const check = async (): Promise<RuntimeDiscovery> => {
    /*
     * THE CLI FIRST (0.540). Google moved personal accounts from Gemini CLI
     * to Antigravity CLI on 2026-06-18, and `agy` runs headless with a
     * stream Locust can read whole -- every tool, the answer, the end of
     * the run -- where the app route reads its transcript files and has
     * missed all three. So when it is installed, it is Antigravity here,
     * with the models it lists, and the app need not be open.
     */
    const cli = agyPath()
    if (cli !== undefined) {
      /*
       * BOTH QUESTIONS AT ONCE, EACH ON A TEN-SECOND CLOCK.
       *
       * `--version` (0.3-1.3 s) was awaited and THEN `models` (1.8-3.6 s, it
       * fetches the list from Google's servers), so the check took their sum
       * -- 3.5-5.0 s of a sweep on Colin's machine, 2026-10-05, where the
       * slower of the two alone is 1.8-3.6 s. Neither needs the other. And
       * each is killed at CLI_CHECK_CAP_MS: a `models` that waits on a network
       * that is not there used to be left running for sixty seconds.
       */
      const cap = options.cliCheckCapMs ?? CLI_CHECK_CAP_MS
      const [version, listed] = await Promise.all([
        askWithin(cap, () => run(cli, ['--version'], {}, cap)),
        askWithin(cap, () => run(cli, ['models'], {}, cap))
      ])
      // Not answering is not being signed out: the person is told which.
      const silent = listed === undefined || listed.timedOut === true
      const models = silent ? undefined : parseAgyModelList(listed.stdout)
      const raw = version?.stdout.trim().split(/\r?\n/)[0]
      return {
        id: 'antigravity',
        kind: 'agent-runtime',
        displayName: 'Antigravity',
        optional: true,
        supportedFeatures: [],
        requiredFeatures: [],
        modelHints: models ?? { aliases: [], efforts: [], models: [] },
        availability: 'available',
        readiness: silent ? 'unknown' : models === undefined ? 'authentication-required' : 'ready',
        executable: { commandName: 'agy', discoveredPath: cli, executablePath: cli, prefixArgs: [], kind: 'native' },
        ...(raw === undefined ? {} : { version: parseVersion(raw) }),
        diagnostics: silent
          ? [{ code: 'readiness-unverifiable', severity: 'warning', message: 'Antigravity CLI did not answer in time.', resolution: 'Locust asks again shortly; if it keeps happening, run agy models in a terminal.' }]
          : models === undefined
            ? [{ code: 'authentication-required', severity: 'warning', message: 'Antigravity CLI is installed but could not list its models.', resolution: 'Run agy in a terminal once and sign in with Google.' }]
            : []
      } as RuntimeDiscovery
    }
    const installed = !hidden() && platform === 'win32' && antigravityExecutableCandidates(localAppData).some((candidate) => existsSync(candidate))
    const host = installed ? await probe() : undefined
    const base = {
      id: 'antigravity' as const,
      kind: 'agent-runtime' as const,
      displayName: 'Antigravity',
      optional: true,
      supportedFeatures: [],
      requiredFeatures: [],
      modelHints: { aliases: [], efforts: [], models: [...ANTIGRAVITY_MODELS] }
    }
    if (!installed) {
      return {
        ...base,
        availability: 'unavailable',
        readiness: 'unknown',
        diagnostics: [
          { code: 'executable-not-found', severity: 'info', message: 'Antigravity is not installed on this machine.' }
        ]
      }
    }
    if (host === undefined) {
      return {
        ...base,
        availability: 'available',
        readiness: 'unhealthy',
        diagnostics: [
          {
            code: 'readiness-unverifiable',
            severity: 'warning',
            message: 'Antigravity is installed but not open. This route drives the agent inside the running app.',
            resolution: 'Open Antigravity with the workspace folder, then retry discovery.'
          }
        ]
      }
    }
    return {
      ...base,
      availability: 'available',
      readiness: 'ready',
      executable: { commandName: 'language_server', discoveredPath: host.executablePath, executablePath: host.executablePath, prefixArgs: ['agentapi'], kind: 'native' },
      ...(host.version === undefined ? {} : { version: parseVersion(host.version) }),
      diagnostics: [
        {
          code: 'readiness-unverifiable',
          severity: 'info',
          message: `Experimental: runs Antigravity's own agent in a folder Antigravity has open (${String(host.projects.size)} known). It cannot be held read-only.`
        }
      ]
    }
  }

  /**
   * One check at a time. A sweep that gave up waiting for a slow `agy` leaves
   * it running, and the re-check that follows must pick that one up, not start
   * a second `agy models` beside it.
   */
  let checking: Promise<RuntimeDiscovery> | undefined
  const discoveryRecord = (): Promise<RuntimeDiscovery> => {
    if (checking !== undefined) return checking
    const running = check().finally(() => {
      checking = undefined
    })
    checking = running
    return running
  }

  const pendingRecord = (): RuntimeDiscovery => {
    const cli = agyPath()
    const installed = cli !== undefined || (!hidden() && platform === 'win32' && antigravityExecutableCandidates(localAppData).some((candidate) => existsSync(candidate)))
    return {
      id: 'antigravity',
      kind: 'agent-runtime',
      displayName: 'Antigravity',
      optional: true,
      supportedFeatures: [],
      requiredFeatures: [],
      modelHints: { aliases: [], efforts: [], models: [] },
      availability: installed ? 'available' : 'unavailable',
      readiness: 'unknown',
      ...(cli === undefined ? {} : { executable: { commandName: 'agy', discoveredPath: cli, executablePath: cli, prefixArgs: [], kind: 'native' } }),
      diagnostics: [{ code: 'check-pending', severity: 'info', message: 'Antigravity is still being checked.' }]
    } as RuntimeDiscovery
  }

  return {
    probe,
    cliPath(): string | undefined {
      return agyPath()
    },
    discoveryRecord,
    pendingRecord
  }
}

function parseVersion(raw: string): RuntimeDiscovery['version'] {
  const match = /^(\d+)\.(\d+)\.(\d+)/.exec(raw)
  if (match === null) return undefined
  return { raw, version: match[0], major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) }
}

export interface AgentApi {
  newConversation(input: { readonly projectId: string; readonly model: string; readonly title: string; readonly prompt: string }): Promise<string>
  sendMessage(input: { readonly projectId: string; readonly conversationId: string; readonly content: string }): Promise<void>
}

export function createAgentApi(host: AntigravityHost, run: AntigravityProbeOptions['run'] = runCli): AgentApi {
  const env = (projectId: string): Readonly<Record<string, string>> => ({
    ANTIGRAVITY_LS_ADDRESS: host.address,
    ANTIGRAVITY_CSRF_TOKEN: host.csrfToken,
    ANTIGRAVITY_PROJECT_ID: projectId
  })
  const parse = (result: { stdout: string; stderr: string }): { response?: Record<string, unknown>; error?: string } => {
    try {
      return JSON.parse(result.stdout) as { response?: Record<string, unknown>; error?: string }
    } catch {
      return { error: `Antigravity's agent API did not answer with JSON: ${result.stderr.slice(0, 200) || result.stdout.slice(0, 200)}` }
    }
  }
  return {
    async newConversation(input) {
      const result = await run(host.executablePath, ['agentapi', 'new-conversation', `--model=${input.model}`, `--title=${input.title}`, input.prompt], env(input.projectId))
      const parsed = parse(result)
      if (parsed.error !== undefined) throw new Error(parsed.error)
      const created = (parsed.response?.newConversation as { conversationId?: unknown } | undefined)?.conversationId
      if (typeof created !== 'string' || created.length === 0) throw new Error("Antigravity did not return a conversation id.")
      return created
    },
    async sendMessage(input) {
      const result = await run(host.executablePath, ['agentapi', 'send-message', input.conversationId, input.content], env(input.projectId))
      const parsed = parse(result)
      if (parsed.error !== undefined) throw new Error(parsed.error)
    }
  }
}

/** The project id Antigravity gave the mission workspace, or undefined when it has not opened that folder. */
export function projectIdFor(host: AntigravityHost, workspacePath: string): string | undefined {
  return projectIdForWorkspace(host.projects, workspacePath)
}
