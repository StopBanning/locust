import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import { createFileMissionLedger, MISSION_LEDGER_SCHEMA_VERSION } from '../src/index.js'

/**
 * ANYTHING A PERSON CAN PICK MUST BE SOMETHING THE LEDGER CAN WRITE DOWN.
 *
 * Colin, on 0.247.0, first run on the new runtime:
 *
 *   Stopped -- the mission ledger could not be written
 *   The mission could not be created in the durable local ledger. Locust
 *   stopped the run rather than continue without a durable record.
 *
 * Muse Code had been added to `MissionRuntimeId`, to the display names, to
 * the capability table, to discovery and to the command builder. It had a
 * measured event normalizer and a packaged build that offered it in the
 * picker. What it did not have was a line in the ledger's own
 * `MISSION_RUNTIMES`, which was `readonly string[]` and therefore tied to
 * nothing the compiler checks -- so every Muse mission was refused at the
 * last gate before launch, by the one component that must never be the
 * reason a run does not happen.
 *
 * The list is a `satisfies Record<MissionRuntimeId, true>` now, so the next
 * runtime is a compile error. This is the other half: the same claim proved
 * against real files on disk, because a type says what a value should be and
 * a file says what happened.
 */
const EVERY_RUNTIME = {
  codex: true,
  claude: true,
  cursor: true,
  gemini: true,
  opencode: true,
  copilot: true,
  antigravity: true,
  muse: true,
  kilo: true,
  acp: true
} as const satisfies Record<MissionRuntimeId, true>

const NOW = '2026-09-21T23:00:00.000Z'
const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

describe('every runtime a person can choose', () => {
  it('can be recorded, read back, and listed', async () => {
    const root = await mkdtemp(join(tmpdir(), 'teammate-runtimes-'))
    roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: root })
    const runtimes = Object.keys(EVERY_RUNTIME) as readonly MissionRuntimeId[]
    // The control: a walk that found nothing would pass every assertion below.
    expect(runtimes.length).toBeGreaterThanOrEqual(8)

    for (const runtime of runtimes) {
      await expect(
        ledger.createMission({
          missionId: `mission_${runtime}`,
          runId: `run_${runtime}`,
          prompt: 'Read this workspace and say what it is.',
          runtime,
          model: 'account-default',
          requestedRouteId: runtime,
          resolvedRouteId: `${runtime}-account:default`,
          cliVersion: '1.0.0',
          workspaceId: 'ws_test',
          sandbox: 'read-only',
          mode: 'ask',
          executionPolicyVersion: 1,
          createdAt: NOW
        }),
        `${runtime} cannot be written to the ledger, so no mission can run under it`
      ).resolves.not.toThrow()
    }

    // Written is not enough: a record the reader refuses is a history that
    // disappears. Read every one back through the real listing.
    const listed = await ledger.listMissions()
    expect(listed.missions.map((mission) => mission.metadata.runtime).sort()).toEqual([...runtimes].sort())
    for (const mission of listed.missions) {
      expect(mission.metadata.schemaVersion ?? MISSION_LEDGER_SCHEMA_VERSION).toBe(MISSION_LEDGER_SCHEMA_VERSION)
    }
  })
})
