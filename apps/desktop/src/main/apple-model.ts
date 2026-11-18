import { execFile } from 'node:child_process'
import { existsSync } from 'node:fs'
import { posix } from 'node:path'

import {
  APPLE_DISPLAY_NAME,
  APPLE_HELPER_NAME,
  APPLE_RUNTIME,
  appleDiscoveryRecord,
  readAppleProbe,
  type AppleProbe,
  type ExecutableLaunch,
  type RuntimeDiscovery
} from '@teammate/runtime-adapters'

import type { SweepCheck } from './sweep-settle.js'

/**
 * APPLE INTELLIGENCE, ON THIS MAC (0.878): where the helper is, and whether the model answers.
 *
 * The helper is built beside the main bundle (scripts/build-mac-apple-model.cjs, out/native), and in an installed app
 * it is outside the archive (electron-builder's asarUnpack of out/native): a process cannot be started from inside
 * app.asar, so the path is the unpacked one. Only on a Mac, and only where it was built.
 *
 * Whether the model answers is asked once a launch (`--probe`), as the Rust app asks it: turning Apple Intelligence on
 * is noticed at the next launch, and a probe costs a process start.
 */

/** The helper's path, from the main bundle's folder; undefined off a Mac or where this build carries none. */
export function appleHelperPath(
  platform: NodeJS.Platform,
  mainDir: string,
  exists: (path: string) => boolean = existsSync
): string | undefined {
  if (platform !== 'darwin') return undefined
  // A Mac's path, built the Mac's way on any computer (the tests run on Windows too).
  const path = posix.join(mainDir, '..', 'native', APPLE_HELPER_NAME).replace(/([\\/])app\.asar([\\/])/, '$1app.asar.unpacked$2')
  return exists(path) ? path : undefined
}

/** The helper as discovery hands a runtime's program to a run. */
export function appleLaunch(path: string): ExecutableLaunch {
  return { commandName: APPLE_HELPER_NAME, discoveredPath: path, executablePath: path, prefixArgs: [], kind: 'native' }
}

/** Runs `--probe` and gives back what it printed; never throws. Ten seconds at most. */
export type AppleProbeRunner = (path: string) => Promise<string>

export const runAppleProbe: AppleProbeRunner = (path) =>
  new Promise((resolve) => {
    execFile(path, ['--probe'], { timeout: 10_000, maxBuffer: 64 * 1024 }, (_error, stdout) => resolve(typeof stdout === 'string' ? stdout : ''))
  })

export interface AppleModelHost {
  /** What the probe said; asked once, and the same answer after. Unknown where there is no helper. */
  probe(): Promise<AppleProbe>
  /** The runtime as discovery reports it, or nothing (no row is better than a broken one). */
  record(): Promise<RuntimeDiscovery | undefined>
  /** Apple Intelligence as a check in the sweep beside the other agents (main/index.ts askRuntimes). */
  check(): SweepCheck
}

export function createAppleModelHost(options: { readonly helperPath: string | undefined; readonly run?: AppleProbeRunner }): AppleModelHost {
  const run = options.run ?? runAppleProbe
  let asked: Promise<AppleProbe> | undefined
  const probe = (): Promise<AppleProbe> => {
    if (options.helperPath === undefined) return Promise.resolve({ available: false, reason: 'unknown' })
    const path = options.helperPath
    asked ??= run(path).then(readAppleProbe, () => ({ available: false, reason: 'unknown' }) as const)
    return asked
  }
  const launch = options.helperPath === undefined ? undefined : appleLaunch(options.helperPath)
  const record = async (): Promise<RuntimeDiscovery | undefined> => appleDiscoveryRecord(await probe(), launch)
  return {
    probe,
    record,
    check: () => ({
      id: APPLE_RUNTIME,
      result: record(),
      located: Promise.resolve(),
      pending: () => ({
        id: APPLE_RUNTIME,
        kind: 'agent-runtime',
        displayName: APPLE_DISPLAY_NAME,
        optional: true,
        availability: launch === undefined ? 'unavailable' : 'available',
        readiness: 'unknown',
        ...(launch === undefined ? {} : { executable: launch }),
        supportedFeatures: [],
        requiredFeatures: [],
        diagnostics: [{ code: 'check-pending', severity: 'info', message: `${APPLE_DISPLAY_NAME} is still being checked.` }]
      })
    })
  }
}
