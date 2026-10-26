import type { RuntimeDiscovery } from '@teammate/runtime-adapters'
import type {
  LocalRuntimeId,
  PublicRuntimeStatus,
  RuntimeAuthState,
  RuntimeDiscoveryResponse,
  RuntimeProbeStatus
} from '../shared/ipc.js'
export { RUNTIME_DISCOVERY_CHANNEL } from '../shared/ipc.js'
export type {
  LocalRuntimeId,
  PublicRuntimeStatus,
  RuntimeAuthState,
  RuntimeDiscoveryResponse,
  RuntimeProbeStatus
} from '../shared/ipc.js'

export interface RuntimeDiscoverySnapshot {
  readonly checkedAt: string
  readonly runtimes: readonly PublicRuntimeStatus[]
}

export interface RuntimeDiscoveryService {
  get(): Promise<RuntimeDiscoveryResponse>
  invalidate(): void
}

interface RuntimeDiscoveryServiceOptions {
  readonly probe: () => Promise<readonly RuntimeDiscovery[]>
  readonly now?: () => Date
  readonly cacheTtlMs?: number
  /**
   * Whether npm can be run. Absent means "assume it is there", which is what
   * every existing caller and test expects; the app wires it for real.
   */
  readonly npmPresent?: () => Promise<boolean>
  /**
   * Whether the npm that would run is this app's own copy. Absent means no,
   * which is what every build before 0.178.0 and every existing test expect.
   */
  readonly npmIsBundled?: () => Promise<boolean>
  /** npm is on this machine and did not answer in five seconds. */
  readonly npmDidNotAnswer?: () => Promise<boolean>
  /** What an ACP agent last said it can do (acp-capabilities.ts, W12). */
  readonly agentCapabilities?: (runtime: LocalRuntimeId) => PublicRuntimeStatus['agentCapabilities']
}

export function publicStatus(runtime: RuntimeDiscovery): PublicRuntimeStatus {
  const installed = runtime.availability === 'available'
  // "Ready" does not always mean "signed in". OpenCode's readiness probe is
  // `opencode models`, which answers happily with no account at all -- and on
  // the machine every drive in this repo runs on, `opencode auth list`
  // reports ZERO credentials while Settings said "Signed in on this machine,
  // using your own account" (QA, 2026-09-06, reproduced live). That is an
  // invented sign-in, and it contradicts the one thing the first-run screen
  // is built around: this runtime's free model needs no account.
  //
  // `not-applicable` is what the type already carries for a runtime whose
  // readiness is not a statement about an account.
  const auth: RuntimeAuthState = runtime.id === 'omniroute' || runtime.id === 'opencode' || runtime.id === 'kilo'
    ? 'not-applicable'
    : runtime.readiness === 'ready'
      ? 'authenticated'
      : runtime.readiness === 'authentication-required'
        ? 'unauthenticated'
        : 'unknown'

  // A version this Locust cannot run yet (0.805, OpenCode 2): said, rather than read as a check that did not answer.
  const refused = runtime.diagnostics?.find((note) => note.code === 'version-unsupported')
  let status: RuntimeProbeStatus
  if (!installed) status = 'not-installed'
  else if (refused !== undefined) status = 'unsupported'
  else if (runtime.readiness === 'ready') status = 'ready'
  else if (runtime.readiness === 'authentication-required') status = 'auth-required'
  else if (runtime.readiness === 'unhealthy') status = 'offline'
  else status = 'probe-failed'

  return {
    id: runtime.id,
    displayName: runtime.displayName,
    installed,
    version: runtime.version?.version ?? null,
    auth,
    ready: status === 'ready',
    status,
    ...(refused === undefined ? {} : { unsupportedNote: `${refused.message}${refused.resolution === undefined ? '' : ` ${refused.resolution}`}` }),
    // Antigravity through its own CLI is an ordinary runtime, not the app's unpublished interface (0.541).
    ...(runtime.id === 'antigravity' && runtime.executable?.commandName === 'agy' ? { throughCli: true as const } : {}),
    // The sweep went on without this agent's check (main/sweep-settle.ts).
    ...(installed && runtime.diagnostics?.some((note) => note.code === 'check-pending') === true ? { checking: true as const } : {})
  }
}

function discoveryFailed(): RuntimeDiscoveryResponse {
  return {
    ok: false,
    error: {
      code: 'DISCOVERY_FAILED',
      message: 'Local runtime discovery could not complete.'
    }
  }
}

export function createRuntimeDiscoveryService(
  options: RuntimeDiscoveryServiceOptions
): RuntimeDiscoveryService {
  const now = options.now ?? (() => new Date())
  const cacheTtlMs = options.cacheTtlMs ?? 10_000
  if (!Number.isFinite(cacheTtlMs) || cacheTtlMs < 0) {
    throw new Error('cacheTtlMs must be a non-negative finite number')
  }

  let cached: { readonly expiresAt: number; readonly response: RuntimeDiscoveryResponse } | undefined
  let inFlight: Promise<RuntimeDiscoveryResponse> | undefined
  /** Bumped by `invalidate()`, so a build that was already running knows it was. */
  let generation = 0

  /*
   * What an ACP agent said it can do is added to EVERY answer, the cached one
   * too (W12): it changes when a run starts, not when a CLI is probed, so it
   * must never wait for -- or force -- a re-probe of every CLI. Measured on
   * the packaged 0.566 drive: recorded, but the row was the launch's answer.
   */
  const told = (response: RuntimeDiscoveryResponse): RuntimeDiscoveryResponse => {
    if (!response.ok || options.agentCapabilities === undefined) return response
    return {
      ...response,
      data: {
        ...response.data,
        runtimes: response.data.runtimes.map((status) => {
          const said = options.agentCapabilities?.(status.id)
          return said === undefined ? status : { ...status, agentCapabilities: said }
        })
      }
    }
  }

  return {
    get(): Promise<RuntimeDiscoveryResponse> {
      const currentTime = now().getTime()
      if (cached && currentTime < cached.expiresAt) return Promise.resolve(told(cached.response))
      if (inFlight) return options.agentCapabilities === undefined ? inFlight : inFlight.then(told)

      /*
       * An invalidation DURING a build is not lost (2026-10-05). An agent the
       * sweep went on without answers in the middle of it -- the npm questions
       * below can take seconds -- and `invalidate()` was a no-op because
       * nothing was cached yet, so the build finished with the list it began
       * with and kept it for ten seconds: the agent that had answered still
       * read "checking". Now the build asks for the list again if it was
       * invalidated while it ran (cheap: the held answer, with the late one in
       * it), and what it builds is cached only if nothing invalidated it.
       */
      const generationAtStart = generation
      const running: Promise<RuntimeDiscoveryResponse> = (async (): Promise<RuntimeDiscoveryResponse> => {
        let runtimes = await options.probe()
        const npmPresent = options.npmPresent === undefined ? true : await options.npmPresent()
        const npmIsBundled = options.npmIsBundled === undefined ? false : await options.npmIsBundled()
        const npmDidNotAnswer = options.npmDidNotAnswer === undefined ? false : await options.npmDidNotAnswer()
        if (generation !== generationAtStart) runtimes = await options.probe()
        return {
          ok: true,
          data: {
            checkedAt: now().toISOString(),
            runtimes: runtimes.map(publicStatus),
            npmPresent,
            npmIsBundled,
            npmDidNotAnswer
          }
        }
      })()
        .catch(() => discoveryFailed())
        .then((response) => {
          if (generation === generationAtStart) cached = { expiresAt: now().getTime() + cacheTtlMs, response }
          return response
        })
        .finally(() => {
          if (inFlight === running) inFlight = undefined
        })
      inFlight = running

      return options.agentCapabilities === undefined ? inFlight : inFlight.then(told)
    },
    invalidate(): void {
      cached = undefined
      // A build in progress began before whatever changed: the next ask starts its own.
      inFlight = undefined
      generation += 1
    }
  }
}
