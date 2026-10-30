import { describe, expect, it } from 'vitest'
import type { RuntimeDiscovery } from '@teammate/runtime-adapters'

import {
  CHECK_EVERY_MS,
  NOTHING_SAVED,
  RELEASE_AGE_MS,
  compareVersions,
  createRuntimeUpdates,
  decide,
  heldFrom,
  mayUpdateAgents,
  npmPackageDir,
  savedUpdatesFrom
} from './runtime-updates.js'
import type { Release, SavedUpdates } from './runtime-updates.js'

/**
 * KEEPING THE CODING AGENTS CURRENT.
 *
 * Colin, 2026-09-23: "new gpt-6 models released we need those added", then
 * "is there a way to make it so the models will automatically update without
 * messing up load times or interfering with the app". Locust reads each
 * agent's models live; Codex CLI 0.153.0 listed GPT-6-Astra and 0.156.1 adds
 * GPT-6-Sol and GPT-6-Luna -- and Codex, from npm, never updates itself.
 *
 * 0.303 made it ask first, suspecting the 159 MB download for a beta
 * tester's dropped connection; the drops went on, only on Codex runs, and
 * Colin: "automatic updating for the models should be fine". So 0.304 updates
 * on its own again by default, and asks only when the person turned that off.
 */

const ROOT = 'C:\\Users\\me\\AppData\\Roaming\\npm\\node_modules'
const HOUR = 60 * 60 * 1000
const NOW = Date.parse('2026-09-24T12:00:00.000Z')

const codex = (overrides: Partial<RuntimeDiscovery> = {}): RuntimeDiscovery =>
  ({
    id: 'codex',
    kind: 'cli',
    displayName: 'Codex CLI',
    optional: false,
    availability: 'available',
    readiness: 'ready',
    executable: {
      commandName: 'codex',
      discoveredPath: 'C:\\Users\\me\\AppData\\Roaming\\npm\\codex.cmd',
      executablePath: 'C:\\Program Files\\nodejs\\node.exe',
      prefixArgs: [`${ROOT}\\@openai\\codex\\bin\\codex.js`],
      kind: 'node-shim'
    },
    version: { raw: 'codex-cli 0.153.0', version: '0.153.0', major: 0, minor: 153, patch: 0 },
    supportedFeatures: [],
    requiredFeatures: [],
    diagnostics: [],
    ...overrides
  }) as unknown as RuntimeDiscovery

const released = (version: string, hoursAgo: number): Release => ({ version, publishedAt: new Date(NOW - hoursAgo * HOUR).toISOString() })

describe('versions', () => {
  it('compare as numbers, a prerelease below its release', () => {
    expect(compareVersions('0.156.1', '0.153.0')).toBe(1)
    expect(compareVersions('0.153.0', '0.156.1')).toBe(-1)
    expect(compareVersions('0.10.0', '0.9.9')).toBe(1)
    // Prereleases by their numbers (2026-10-10 sweep).
    expect(compareVersions('0.158.0-alpha.10', '0.158.0-alpha.9')).toBe(1)
    expect(compareVersions('0.158.0-alpha.9', '0.158.0-beta.1')).toBe(-1)
    expect(compareVersions('0.158.0-alpha', '0.158.0-alpha.1')).toBe(-1)
    expect(compareVersions('0.158.0-alpha.4', '0.158.0')).toBe(-1)
    expect(compareVersions('1.0.88', '1.0.88')).toBe(0)
  })
})

describe('what to do about one agent', () => {
  const base = { installed: '0.153.0', automatic: true, inUse: false, now: NOW }

  it('asks first: a newer release waits for the person to press Update, however old and however idle', () => {
    expect(decide({ ...base, automatic: false, latest: released('0.156.1', 400) })).toEqual({ kind: 'waiting', version: '0.156.1', why: 'ask' })
  })

  it('updates to a newer release that has been out long enough, when nothing is using it', () => {
    expect(decide({ ...base, latest: released('0.156.1', 14) })).toEqual({ kind: 'update', version: '0.156.1' })
  })

  it('waits while something is using it -- a mission, or another session running that copy', () => {
    expect(decide({ ...base, inUse: true, latest: released('0.156.1', 14) })).toEqual({ kind: 'waiting', version: '0.156.1', why: 'in use' })
  })

  it('waits on a release out for less than twelve hours, or one npm gives no date for', () => {
    expect(decide({ ...base, latest: released('0.156.1', 2) })).toEqual({ kind: 'waiting', version: '0.156.1', why: 'too new' })
    expect(decide({ ...base, latest: { version: '0.156.1', publishedAt: undefined } })).toMatchObject({ why: 'too new' })
    expect(RELEASE_AGE_MS).toBe(12 * HOUR)
  })


  it('does nothing when it is current, or ahead of what npm calls newest', () => {
    expect(decide({ ...base, latest: released('0.153.0', 40) })).toEqual({ kind: 'current' })
    expect(decide({ ...base, installed: '0.157.0', latest: released('0.156.1', 40) })).toEqual({ kind: 'current' })
  })
})

describe('only what npm put there', () => {
  // npm's Windows layout (codex.cmd beside node_modules, run by node.exe), which only a Windows host joins this way.
  it.skipIf(process.platform !== 'win32')("is npm's: its shim in npm's own folder, or its script inside the package", () => {
    expect(npmPackageDir(codex(), ROOT)).toBe(`${ROOT}\\@openai\\codex`)
  })

  it.skipIf(process.platform !== 'win32')('is not an agent found somewhere else: npm would put a second copy beside it', () => {
    const elsewhere = codex({
      executable: { commandName: 'codex', discoveredPath: 'C:\\tools\\codex\\codex.exe', executablePath: 'C:\\tools\\codex\\codex.exe', prefixArgs: [] }
    } as unknown as Partial<RuntimeDiscovery>)
    expect(npmPackageDir(elsewhere, ROOT)).toBeUndefined()
  })

  it('never names an agent that updates itself', () => {
    expect(npmPackageDir(codex({ id: 'cursor' } as Partial<RuntimeDiscovery>), ROOT)).toBeUndefined()
  })
})

describe('a scripted launch', () => {
  it('never changes the machine: drives run on throwaway profiles, and the agents are everyone’s', () => {
    expect(mayUpdateAgents(['locust.exe'], {})).toBe(true)
    expect(mayUpdateAgents(['locust.exe', '--remote-debugging-port=9443'], {})).toBe(false)
    expect(mayUpdateAgents(['locust.exe', '--remote-debugging-port=9443'], { LOCUST_UPDATE_AGENTS: '1' })).toBe(true)
    // A copy that is not the installed Locust leaves the machine alone (0.514); the drive about updating still may.
    expect(mayUpdateAgents(['locust.exe'], {}, false)).toBe(false)
    expect(mayUpdateAgents(['locust.exe'], { LOCUST_UPDATE_AGENTS: '1' }, false)).toBe(true)
  })
})

describe('what is kept on disk', () => {
  it('reads back what was written, and nothing else', () => {
    const saved: SavedUpdates = {
      automatic: false,
      chosen: true,
      checkedAt: NOW,
      latest: { '@openai/codex': released('0.156.1', 14) },
      last: { codex: { kind: 'updated', from: '0.153.0', to: '0.156.1', at: '2026-09-24T11:00:00.000Z' } }
    }
    expect(savedUpdatesFrom(JSON.parse(JSON.stringify(saved)))).toEqual(saved)
    expect(savedUpdatesFrom({ automatic: 'yes', latest: { x: 3 }, last: { codex: { kind: 'updating' } } })).toEqual({ ...NOTHING_SAVED })
    expect(savedUpdatesFrom('nonsense')).toBeUndefined()
  })

  it('updates on its own unless the person turned that off -- a default 0.303 saved is not a choice', () => {
    expect(NOTHING_SAVED.automatic).toBe(true)
    expect(savedUpdatesFrom({ automatic: false, checkedAt: NOW })?.automatic).toBe(true)
    expect(savedUpdatesFrom({ enabled: false, checkedAt: NOW })?.automatic).toBe(true)
    expect(savedUpdatesFrom({ automatic: false, chosen: true, checkedAt: NOW })?.automatic).toBe(false)
  })
})

describe('keeping current, end to end on fakes', () => {
  const AUTOMATIC: SavedUpdates = NOTHING_SAVED
  const ASKS: SavedUpdates = { ...NOTHING_SAVED, automatic: false, chosen: true }
  const run = (setup: { saved?: SavedUpdates; inUse?: boolean; latest?: Release; installOk?: boolean; discovered?: RuntimeDiscovery; held?: Readonly<Record<string, readonly string[]>> }) => {
    const calls = { latest: 0, installs: [] as string[], updated: 0, inUse: 0 }
    let saved = setup.saved
    const updates = createRuntimeUpdates({
      discover: async () => [setup.discovered ?? codex()],
      npmRoot: async () => ROOT,
      latest: async () => {
        calls.latest += 1
        return setup.latest ?? released('0.156.1', 14)
      },
      ...(setup.held === undefined ? {} : { heldVersions: async () => setup.held }),
      inUse: async () => {
        calls.inUse += 1
        return setup.inUse === true
      },
      install: async (runtime, version) => {
        calls.installs.push(`${runtime}@${version}`)
        return setup.installOk === false ? { ok: false, what: 'npm could not replace the files.' } : { ok: true }
      },
      load: async () => saved,
      save: async (next) => {
        saved = next
      },
      updated: () => {
        calls.updated += 1
      },
      changed: () => undefined,
      now: () => NOW
    })
    return { updates, calls, saved: () => saved }
  }

  it('holds back a version Locust\'s release check could not read, and installs it only when asked (0.501)', async () => {
    const { updates, calls } = run({ held: { '@openai/codex': ['0.156.1'] } })
    await updates.tick()
    expect(calls.installs).toEqual([])
    expect((await updates.state()).agents[0]?.status).toEqual({ kind: 'waiting', version: '0.156.1', why: 'held' })
    // Update is the person's choice, and still installs it.
    await updates.updateNow('codex')
    expect(calls.installs).toEqual(['codex@0.156.1'])
  })

  it('holds nothing back that the check passed, or when it cannot be reached (0.501)', async () => {
    const passed = run({ held: { '@openai/codex': ['0.155.0'] } })
    await passed.updates.tick()
    expect(passed.calls.installs).toEqual(['codex@0.156.1'])
    const unreachable = run({})
    await unreachable.updates.tick()
    expect(unreachable.calls.installs).toEqual(['codex@0.156.1'])
  })

  it('turned off, only looks: 0.156.1 is out, and nothing is downloaded until the person asks', async () => {
    const { updates, calls } = run({ saved: ASKS })
    await updates.tick()
    expect(calls.latest).toBe(1)
    expect(calls.inUse).toBe(0)
    expect(calls.installs).toEqual([])
    expect((await updates.state()).agents[0]?.status).toEqual({ kind: 'waiting', version: '0.156.1', why: 'ask' })
  })

  it('updates when the person presses Update, and asks for the models again', async () => {
    const { updates, calls } = run({ saved: ASKS })
    await updates.tick()
    await updates.updateNow('codex')
    expect(calls.installs).toEqual(['codex@0.156.1'])
    expect(calls.updated).toBe(1)
    expect((await updates.state()).agents[0]?.status).toMatchObject({ kind: 'updated', from: '0.153.0', to: '0.156.1' })
  })

  it('does not update on the press while something is using it, and says why', async () => {
    const { updates, calls } = run({ saved: ASKS, inUse: true })
    await updates.tick()
    await updates.updateNow('codex')
    expect(calls.installs).toEqual([])
    expect((await updates.state()).agents[0]?.status).toEqual({ kind: 'waiting', version: '0.156.1', why: 'in use' })
  })

  it('by default looks, finds 0.156.1, updates Codex, and asks for the models again', async () => {
    const { updates, calls, saved } = run({})
    await updates.tick()
    expect(calls.installs).toEqual(['codex@0.156.1'])
    expect(calls.updated).toBe(1)
    const state = await updates.state()
    expect(state.agents[0]?.status).toMatchObject({ kind: 'updated', from: '0.153.0', to: '0.156.1' })
    expect(saved()?.last.codex).toMatchObject({ kind: 'updated' })
  })

  it('turned on, leaves it be while something is using it, and says so', async () => {
    const { updates, calls } = run({ saved: AUTOMATIC, inUse: true })
    await updates.tick()
    expect(calls.installs).toEqual([])
    expect((await updates.state()).agents[0]?.status).toEqual({ kind: 'waiting', version: '0.156.1', why: 'in use' })
  })

  it('does not go to the network when the last look is recent, and reads no processes for an agent that is current', async () => {
    const recent: SavedUpdates = { ...NOTHING_SAVED, checkedAt: NOW - (CHECK_EVERY_MS - HOUR), latest: { '@openai/codex': released('0.153.0', 40) } }
    const { updates, calls } = run({ saved: recent })
    await updates.tick()
    expect(calls.latest).toBe(0)
    expect(calls.inUse).toBe(0)
    expect(calls.installs).toEqual([])
  })

  it('says when an update failed, and keeps saying it', async () => {
    const { updates, saved } = run({ saved: AUTOMATIC, installOk: false })
    await updates.tick()
    expect((await updates.state()).agents[0]?.status).toMatchObject({ kind: 'failed', version: '0.156.1', what: 'npm could not replace the files.' })
    expect(saved()?.last.codex).toMatchObject({ kind: 'failed' })
  })


})

/*
 * L12 (the code review): a tick that came while a manual Update was
 * installing ran the same install again beside it -- npm fighting itself --
 * recorded the refused second one as a failed update, and cleared
 * "updating" while the first was still running.
 */
describe('a tick during a manual Update', () => {
  it('waits for it, and installs once', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const installs: string[] = []
    const updates = createRuntimeUpdates({
      discover: async () => [codex()],
      npmRoot: async () => ROOT,
      latest: async () => released('0.156.1', 14),
      inUse: async () => false,
      install: async (runtime, version) => {
        installs.push(`${runtime}@${version}`)
        await gate
        return { ok: true }
      },
      load: async () => ({ ...NOTHING_SAVED, checkedAt: NOW, latest: { '@openai/codex': released('0.156.1', 14) } }),
      save: async () => undefined,
      updated: () => undefined,
      changed: () => undefined,
      now: () => NOW
    })
    const pressed = updates.updateNow('codex')
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(updates.updating()).toBe('codex')
    const ticked = updates.tick()
    await new Promise((resolve) => setTimeout(resolve, 20))
    // Still the one install, and still updating.
    expect(installs).toEqual(['codex@0.156.1'])
    expect(updates.updating()).toBe('codex')
    release()
    await pressed
    await ticked
    expect(installs).toEqual(['codex@0.156.1'])
  })
})

describe("the release check's verdicts (0.501)", () => {
  it('holds only the versions it ran and could not read', () => {
    expect(heldFrom({ packages: { '@openai/codex': { '0.159.2': { ok: true }, '0.160.0': { ok: false } }, '@github/copilot': { '1.0.0': { ok: true } } } }))
      .toEqual({ '@openai/codex': ['0.160.0'] })
  })

  it('reads anything else as nothing held', () => {
    expect(heldFrom(undefined)).toEqual({})
    expect(heldFrom({ packages: 'no' })).toEqual({})
    expect(heldFrom({ packages: { '@openai/codex': { '0.160.0': { ok: 'false' } } } })).toEqual({})
  })

  it('keeps what it held across a restart', () => {
    expect(savedUpdatesFrom({ latest: {}, last: {}, held: { '@openai/codex': ['0.160.0', 7] } })?.held).toEqual({ '@openai/codex': ['0.160.0'] })
    expect(savedUpdatesFrom({ latest: {}, last: {} })?.held).toBeUndefined()
  })
})
