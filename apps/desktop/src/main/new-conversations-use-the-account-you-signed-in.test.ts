import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import type { ExecutableLaunch, RuntimeCommandSpec, RuntimeDiscovery, RuntimeProcessRun } from '@teammate/runtime-adapters'
import type { MissionLedger } from '@teammate/mission-store'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAgentAccounts } from './agent-accounts.js'
import { createCodexMissionService } from './codex-mission.js'
import { accountFromRoute } from '../shared/agent-accounts.js'
import { carryOnOffer, parsedCarryOnRoute } from '../shared/carry-on.js'
import { createCarryOn } from './carry-on.js'
import type { CarryOnStart } from './carry-on.js'

/**
 * NEW CONVERSATIONS USE THE ACCOUNT YOU SIGNED IN (0.876).
 *
 * Colin, 2026-11-16, after adding his Max account in Settings and signing in to it, and every new conversation still
 * stopping on the other account's used-up 5-hour window: "A brand new conversation should just use whatever account is
 * logged in. That is 100% a bug." An added account was used only by a conversation that picked it in the model menu;
 * nothing said so, and This Mac's sign-in -- the older account -- took every conversation that did not. Now signing in
 * an added account puts new conversations on it (unless another added account already has them), Settings says which
 * account they use and moves them with one press, and a start that names no account runs on that one, its route
 * saying so, so the ledger, a resume and the usage meter read the account it really ran on.
 */

const cleanups: (() => Promise<unknown>)[] = []
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) await cleanup() })

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), 'locust-for-new-'))
  cleanups.push(() => rm(directory, { recursive: true, force: true }))
  let finish: (ok: boolean) => void = () => undefined
  const store = createAgentAccounts({ directory, login: async (_runtime, _home, _env, done) => { finish = done; return () => undefined } })
  const signIn = async (id: string, ok = true) => { await store.login(id); finish(ok); await store.list() }
  return { directory, store, signIn }
}

const launch: ExecutableLaunch = { commandName: 'claude', discoveredPath: '/tools/claude', executablePath: '/tools/claude', prefixArgs: [], kind: 'native', env: {} }

describe('new conversations use the account you signed in', () => {
  it('moves new conversations to an added account the moment its sign-in finishes', async () => {
    const { store, signIn } = await fixture()
    const max = await store.add('claude', 'max')
    expect(await store.forNew('claude')).toBeUndefined()
    await signIn(max.id)
    expect(await store.forNew('claude')).toBe(max.id)
    expect((await store.list())[0]).toMatchObject({ name: 'max', state: 'ready', forNew: true })
  })

  it('leaves them where they are when the sign-in fails, or when another added account already has them', async () => {
    const { store, signIn } = await fixture()
    const work = await store.add('claude', 'Work')
    await signIn(work.id, false)
    expect(await store.forNew('claude')).toBeUndefined()
    expect((await store.list()).some(a => a.forNew === true)).toBe(false)
    await signIn(work.id)
    const personal = await store.add('claude', 'Personal')
    await signIn(personal.id)
    expect(await store.forNew('claude')).toBe(work.id)
  })

  it('moves them to another signed-in account, or back to This Mac’s sign-in, with one press', async () => {
    const { store, signIn } = await fixture()
    const work = await store.add('claude', 'Work')
    const personal = await store.add('claude', 'Personal')
    await signIn(work.id)
    await signIn(personal.id)
    await store.useForNew('claude', personal.id)
    expect(await store.forNew('claude')).toBe(personal.id)
    expect((await store.list()).filter(a => a.forNew === true).map(a => a.name)).toEqual(['Personal'])
    await store.useForNew('claude', undefined)
    expect(await store.forNew('claude')).toBeUndefined()
    expect((await store.list()).some(a => a.forNew === true)).toBe(false)
  })

  it('refuses an account not signed in, or another agent’s, and changes nothing', async () => {
    const { store, signIn } = await fixture()
    const work = await store.add('claude', 'Work')
    await signIn(work.id)
    const fresh = await store.add('claude', 'Fresh')
    const codex = await store.add('codex', 'Codex work')
    await signIn(codex.id)
    await expect(store.useForNew('claude', fresh.id)).rejects.toThrow('Sign in to Fresh first. New conversations are unchanged.')
    await expect(store.useForNew('claude', codex.id)).rejects.toThrow('another agent')
    await expect(store.useForNew('cursor', undefined)).rejects.toThrow('does not support')
    expect(await store.forNew('claude')).toBe(work.id)
  })

  it('keeps each agent’s choice its own', async () => {
    const { store, signIn } = await fixture()
    const claude = await store.add('claude', 'Max')
    const codex = await store.add('codex', 'Plus')
    await signIn(claude.id)
    await signIn(codex.id)
    await store.useForNew('claude', undefined)
    expect(await store.forNew('claude')).toBeUndefined()
    expect(await store.forNew('codex')).toBe(codex.id)
  })

  it('does not use an account whose sign-in is being redone or failed', async () => {
    const { store, signIn } = await fixture()
    const max = await store.add('claude', 'max')
    await signIn(max.id)
    await store.login(max.id)
    expect(await store.forNew('claude')).toBeUndefined()
  })

  it('runs a start that names no account on that account, and its route says so; a named account and a cloud turn are left alone', async () => {
    const { store, signIn, directory } = await fixture()
    const max = await store.add('claude', 'max')
    const other = await store.add('claude', 'Other')
    await signIn(max.id)
    await signIn(other.id)
    const commands: RuntimeCommandSpec[] = []
    const ledger = { createMission: vi.fn(async () => undefined), appendEvents: vi.fn(async () => undefined), appendHostFailure: vi.fn(async () => undefined), flush: async () => undefined } as unknown as MissionLedger
    const service = createCodexMissionService({ workspacePath: directory, ledger, schedule: task => queueMicrotask(task),
      accountForNew: runtime => store.forNew(runtime),
      discover: async (runtime, account) => [{ id: 'claude', kind: 'agent-runtime', displayName: 'Claude Code', optional: false, availability: 'available', readiness: 'ready', executable: await store.launch(runtime!, account, launch), supportedFeatures: [], requiredFeatures: [], diagnostics: [] } as RuntimeDiscovery],
      runner: { start: command => { commands.push(command); return { records: { async *[Symbol.asyncIterator]() {}, drainAvailable: () => [] }, completion: Promise.resolve({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 0, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: '', finishedAt: '' }) } satisfies RuntimeProcessRun } }
    })
    const fresh = await service.start('Read this folder', 'claude', 'ask', {}, () => undefined, undefined, undefined, undefined, undefined, undefined, undefined, { key: 'a' })
    const named = await service.start('Read this folder', 'claude', 'ask', { account: other.id }, () => undefined, undefined, undefined, undefined, undefined, undefined, undefined, { key: 'b' })
    expect(fresh.ok && accountFromRoute(fresh.data.resolvedRouteId)).toBe(max.id)
    expect(commands[0]?.env?.CLAUDE_CONFIG_DIR).toBe(join(directory, 'agent-accounts', max.id))
    expect(named.ok && accountFromRoute(named.data.resolvedRouteId)).toBe(other.id)
    await store.useForNew('claude', undefined)
    const mac = await service.start('Read this folder', 'claude', 'ask', {}, () => undefined, undefined, undefined, undefined, undefined, undefined, undefined, { key: 'c' })
    expect(mac.ok && mac.data.resolvedRouteId.endsWith('-account:default')).toBe(true)
    expect(commands[2]?.env?.CLAUDE_CONFIG_DIR).toBeUndefined()
    await service.dispose()
  })

  it('offers the card’s carry-on on another signed-in account of the same agent before any other agent', () => {
    const max = `account_${'a'.repeat(32)}`
    const ready = new Set(['claude', 'codex'] as const)
    expect(carryOnOffer('claude', undefined, ready, max)).toEqual({ runtime: 'claude', account: max })
    expect(carryOnOffer('claude', undefined, ready)).toEqual({ runtime: 'codex' })
    // The person's own pick in Settings still comes first.
    expect(carryOnOffer('claude', { runtime: 'codex', automatic: false }, ready, max)).toEqual({ runtime: 'codex' })
    expect(parsedCarryOnRoute({ runtime: 'claude', account: max })).toEqual({ runtime: 'claude', account: max })
    expect(parsedCarryOnRoute({ runtime: 'claude', account: '../elsewhere' })).toBeUndefined()
  })

  it('carries a turn on with the same agent only on another account than the one whose plan ran out', async () => {
    const max = `account_${'a'.repeat(32)}`
    const pro = `account_${'b'.repeat(32)}`
    const started: CarryOnStart[] = []
    const ranOutOn = (resolvedRouteId: string) => createCarryOn({
      ledger: { getMission: async () => ({ phase: 'failed', events: [{ type: 'run.failed', sourceAdapter: 'claude', payload: { kind: 'quota-exhausted', message: 'limit' } }], metadata: { missionId: 'mission_1', runId: 'run_1', runtime: 'claude', mode: 'accept-edits', resolvedRouteId } }) as never },
      setting: async () => undefined,
      sentByPerson: () => true,
      note: () => undefined,
      start: async (asked) => { started.push(asked); return { ok: true, missionId: 'mission_2' } }
    })
    expect(await ranOutOn('claude-account:default').carry('mission_1', { runtime: 'claude', account: max })).toEqual({ ok: true, missionId: 'mission_2' })
    expect(started[0]).toMatchObject({ runtime: 'claude', account: max, followUpOf: 'mission_1', from: 'claude' })
    expect(await ranOutOn(`claude-account:${max}`).carry('mission_1', { runtime: 'claude', account: max })).toEqual({ ok: false, message: 'Claude Code is the agent whose plan ran out.' })
    expect(await ranOutOn(`claude-account:${pro}`).carry('mission_1', { runtime: 'claude' })).toEqual({ ok: false, message: 'Claude Code is the agent whose plan ran out.' })
    expect(started).toHaveLength(1)
  })
})
