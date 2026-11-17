import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFileMissionLedger } from '@teammate/mission-store'
import type { RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRunner } from '@teammate/runtime-adapters'

import { whatItMayDo } from '../shared/may-do.js'
import { runtimeDisplayName } from '../shared/runtimes.js'
import { appleHelperPath, createAppleModelHost } from './apple-model.js'
import { createCodexMissionService } from './codex-mission.js'
import { appleModelsFrom, withAccountDefaults } from './model-catalog.js'

/*
 * APPLE INTELLIGENCE IN THE ELECTRON APP (0.878): a teammate on Apple's on-device model, as the Rust app runs one
 * (apps/native/core/src/runtime/apple_tests.rs); its modes and mark are held in
 * renderer/src/apple-intelligence-runs-in-ask-only.test.ts. The runner is a stand-in that answers the way the helper does, so the
 * test sees exactly what the helper is sent.
 */

const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })

const HELPER = { commandName: 'locust-apple-model', discoveredPath: '/app/out/native/locust-apple-model', executablePath: '/app/out/native/locust-apple-model', prefixArgs: [], kind: 'native' as const }
const READY: RuntimeDiscovery = {
  id: 'apple', kind: 'agent-runtime', displayName: 'Apple Intelligence', optional: true, availability: 'available', readiness: 'ready',
  executable: HELPER, supportedFeatures: [], requiredFeatures: [], diagnostics: []
}
const completion: RuntimeProcessCompletion = { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: '2026-11-17T18:00:00.000Z', finishedAt: '2026-11-17T18:00:01.000Z' }

/** Answers as the helper does: "You said: " and the end of what it was sent. */
const echoRunner = (): RuntimeProcessRunner => ({
  start: vi.fn((_command, prompt: string) => {
    const answer = `You said: ${prompt.slice(-40)}`
    return {
      records: {
        async *[Symbol.asyncIterator]() {
          yield { sequence: 1, raw: JSON.stringify({ type: 'delta', text: answer }) }
          yield { sequence: 2, raw: JSON.stringify({ type: 'done', text: answer }) }
        },
        drainAvailable: () => []
      },
      completion: Promise.resolve(completion)
    }
  })
})

const PIPPIN = { self: { teammateId: 'a1', name: 'Pippin', role: 'Test' }, others: [] }

describe('a teammate on Apple Intelligence', () => {
  it('answers, and a follow-up is told what was said, in its own short brief', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-apple-turn-')); roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: join(root, 'ledger') })
    const runner = echoRunner()
    const scheduled: (() => void)[] = []
    let id = 0
    const service = createCodexMissionService({
      workspacePath: root, discover: async () => [READY], ledger, runner, createId: () => String(++id),
      schedule: (task) => scheduled.push(task),
      teammateInstructions: async (teammateId) => (teammateId === 'a1' ? 'Answer in one sentence.' : undefined)
    })
    const send = async (prompt: string, prior?: string) => {
      const started = await service.start(prompt, 'apple', 'ask', { model: 'apple-on-device' }, () => undefined, undefined, PIPPIN, prior)
      if (!started.ok) throw new Error(started.error.message)
      scheduled.splice(0).forEach((task) => task())
      await vi.waitFor(() => expect(service.hasMission(started.data.missionId)).toBe(false))
      return started.data
    }
    try {
      const first = await send('Name a primary colour')
      const recorded = await ledger.getMission(first.missionId)
      expect(recorded?.metadata.runtime).toBe('apple')
      expect(recorded?.events.some((event) => event.type === 'run.completed')).toBe(true)
      await send('And another', first.missionId)
      const calls = vi.mocked(runner.start).mock.calls
      expect(calls).toHaveLength(2)
      expect(calls[0]![0]).toMatchObject({ runtime: 'apple', stdin: 'apple-json', sandbox: 'read-only' })
      const prompts = calls.map((call) => call[1])
      expect(prompts[0]).toContain('Name a primary colour')
      // No session to resume: the follow-up quotes the first turn, question and answer.
      expect(prompts[1]).toContain('could not be resumed')
      expect(prompts[1]).toContain('Name a primary colour')
      expect(prompts[1]).toContain('You said:')
      expect(prompts[1]).toContain('And another')
      // Its own short brief: who it is, what it cannot do, its instructions; never the tag rules a coding agent is taught.
      for (const prompt of prompts) {
        expect(prompt.startsWith('You are Pippin, a teammate in Locust (Test).')).toBe(true)
        expect(prompt).toContain('You can only talk')
        expect(prompt).toContain('Your instructions from the person:\nAnswer in one sentence.')
        expect(prompt).not.toContain('<locust-')
      }
    } finally { await service.dispose() }
  })

  it('refuses a mode with tools before anything is sent or written', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-apple-refused-')); roots.push(root)
    const ledger = createFileMissionLedger({ rootDirectory: join(root, 'ledger') })
    const runner = echoRunner()
    const service = createCodexMissionService({ workspacePath: root, discover: async () => [READY], ledger, runner, createId: () => 'x', schedule: (task) => task(), autoModeAllowed: async () => true })
    try {
      for (const mode of ['accept-edits', 'auto', 'approve-each'] as const) {
        const started = await service.start('Fix the bug', 'apple', mode, {}, () => undefined)
        expect(started.ok, mode).toBe(false)
        if (!started.ok) expect(started.error.message, mode).toContain('Ask mode only')
      }
      expect(runner.start).not.toHaveBeenCalled()
      expect((await ledger.listMissions()).missions).toEqual([])
    } finally { await service.dispose() }
  })

  it('says it may not touch files, commands or the web', () => {
    const may = whatItMayDo('apple', 'read-only', 'ask')
    expect(may.rows.every((row) => row.verdict === 'deny')).toBe(true)
    expect(may.rows.map((row) => row.text)).toEqual(['reading or changing your files', 'running commands', 'searching the web or opening web pages'])
    expect(may.rest).toContain('nothing leaves it')
    expect(runtimeDisplayName('apple')).toBe('Apple Intelligence')
  })
})

describe("Apple Intelligence's model", () => {
  it('is offered as one model, with no account-default row, only where it answers', () => {
    expect(appleModelsFrom([])).toEqual([])
    expect(appleModelsFrom([{ ...READY, readiness: 'unsupported' }])).toEqual([])
    const listed = withAccountDefaults(appleModelsFrom([READY]))
    expect(listed.map((model) => [model.runtime, model.id, model.displayName])).toEqual([['apple', 'apple-on-device', 'On-device model']])
  })
})

describe("Apple Intelligence's helper", () => {
  it('is found beside the main bundle, outside the archive in an installed app, and only on a Mac', () => {
    const there = (): boolean => true
    expect(appleHelperPath('darwin', '/Applications/Locust.app/Contents/Resources/app.asar/out/main', there))
      .toBe('/Applications/Locust.app/Contents/Resources/app.asar.unpacked/out/native/locust-apple-model')
    expect(appleHelperPath('darwin', '/work/apps/desktop/out/main', there)).toBe('/work/apps/desktop/out/native/locust-apple-model')
    expect(appleHelperPath('linux', '/work/apps/desktop/out/main', there)).toBeUndefined()
    expect(appleHelperPath('darwin', '/work/apps/desktop/out/main', () => false)).toBeUndefined()
  })

  it('is asked once a launch, and says why the model is not there in its row', async () => {
    const run = vi.fn(async () => '{"available":false,"reason":"appleIntelligenceNotEnabled"}\n')
    const host = createAppleModelHost({ helperPath: '/x/locust-apple-model', run })
    const check = host.check()
    expect(check.pending()).toMatchObject({ id: 'apple', readiness: 'unknown' })
    const row = await check.result
    expect(row?.readiness).toBe('unsupported')
    expect(row?.diagnostics[0]?.message).toContain('turned off on this Mac')
    await host.record()
    expect(run).toHaveBeenCalledTimes(1)
  })

  it('has no row where there is no helper, or it cannot say', async () => {
    expect(await createAppleModelHost({ helperPath: undefined }).record()).toBeUndefined()
    expect(await createAppleModelHost({ helperPath: '/x', run: async () => 'dyld: Library not loaded' }).record()).toBeUndefined()
    expect(await createAppleModelHost({ helperPath: '/x', run: async () => '{"available":true,"contextSize":8192}' }).record()).toMatchObject({ readiness: 'ready' })
  })
})
