import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { MissionLedger, Workroom } from '@teammate/mission-store'
import type { RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRecordStream, RuntimeProcessRun } from '@teammate/runtime-adapters'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { createBriefSessions } from './brief-sessions.js'
import { createCodexMissionService } from './codex-mission.js'
import { workspaceIdFor } from './workspace.js'

/**
 * A CONVERSATION RUNS IN ITS OWN FOLDER (0.458, docs/PLAN-2026-09-29-FOLDERS-LIKE-CLAUDE-CODE.md).
 *
 * The window can be in another folder now without a restart, like Claude
 * Code. A turn that continues a conversation runs where that conversation
 * ran -- never where the window happens to be -- and one whose folder is not
 * known, or is gone, is refused rather than run somewhere it never was.
 */
const NOW = '2026-09-29T15:00:00.000Z'

describe('a turn that continues a conversation', () => {
  let here: string
  let there: string
  let scratch: string
  beforeEach(async () => {
    here = await mkdtemp(join(tmpdir(), 'locust-folder-here-'))
    there = await mkdtemp(join(tmpdir(), 'locust-folder-there-'))
    scratch = await mkdtemp(join(tmpdir(), 'locust-folder-briefs-'))
  })
  afterEach(async () => {
    for (const folder of [here, there, scratch]) await rm(folder, { recursive: true, force: true })
  })

  const runtime: RuntimeDiscovery = {
    id: 'codex',
    kind: 'agent-runtime',
    displayName: 'Codex CLI',
    optional: false,
    availability: 'available',
    readiness: 'ready',
    executable: {
      commandName: 'codex',
      discoveredPath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      executablePath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      prefixArgs: [],
      kind: 'native'
    },
    version: { raw: 'codex-cli 0.151.0', version: '0.151.0', major: 0, minor: 151, patch: 0 },
    supportedFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
    requiredFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
    diagnostics: []
  }
  const open = (): RuntimeProcessRun => ({
    records: {
      async *[Symbol.asyncIterator]() {
        yield { sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread-1' }) }
      },
      drainAvailable: () => []
    } as RuntimeProcessRecordStream,
    completion: new Promise<RuntimeProcessCompletion>(() => undefined)
  })
  const workroom: Workroom = {
    post: async () => {
      throw new Error('not used')
    },
    unread: async () => ({ messages: [], remaining: 0 }),
    markDelivered: async () => undefined,
    read: async () => ({ messages: [], deliveries: [], issues: [] }),
    flush: async () => undefined
  }

  /** The window in `here`; one earlier conversation, `mission_earlier`, that ran in `folderOfEarlier`. */
  const service = (folderOfEarlier: string, known: Readonly<Record<string, string>>) => {
    const spawned: string[] = []
    const recorded: string[] = []
    let nextId = 0
    const missions = createCodexMissionService({
      workspacePath: here,
      currentFolder: () => here,
      folderOf: async (id) => known[id],
      discover: async () => [runtime],
      runner: {
        start: (spec: { readonly cwd: string }) => {
          spawned.push(spec.cwd)
          return open()
        }
      },
      ledger: {
        createMission: async (input: { readonly workspaceId: string }) => {
          recorded.push(input.workspaceId)
        },
        appendEvents: async () => undefined,
        appendHostFailure: async () => undefined,
        appendPeerLinks: async () => undefined,
        appendEditCheck: async () => undefined, appendApproval: async () => undefined,
        getMission: async (missionId: string) =>
          missionId === 'mission_earlier'
            ? {
                metadata: { missionId, runId: 'run_earlier', runtime: 'codex', workspaceId: workspaceIdFor(folderOfEarlier), mode: 'ask', createdAt: NOW },
                phase: 'completed',
                events: []
              }
            : undefined,
        listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }),
        flush: async () => undefined
      } as unknown as MissionLedger,
      workroom,
      briefSessions: createBriefSessions({ rootDirectory: scratch }),
      createId: () => String(++nextId),
      now: () => new Date(NOW),
      schedule: () => undefined
    })
    const followUp = () => missions.start('And the tests?', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_earlier')
    return { missions, spawned, recorded, followUp }
  }

  it('runs in its conversation\'s folder, not the window\'s, and is recorded there', async () => {
    const { spawned, recorded, followUp } = service(there, { [workspaceIdFor(there)]: there })
    const started = await followUp()
    expect(started.ok).toBe(true)
    expect(spawned).toEqual([there])
    expect(recorded).toEqual([workspaceIdFor(there)])
  })

  it('continues a restored Windows conversation in the chosen Mac folder', async () => {
    const old = 'C:\\work\\demo'
    const { spawned, recorded, followUp } = service(old, { [workspaceIdFor(old)]: there })
    expect((await followUp()).ok).toBe(true)
    expect(spawned).toEqual([there])
    expect(recorded).toEqual([workspaceIdFor(there)])
  })

  it('a new conversation still starts in the window\'s folder', async () => {
    const { missions, spawned, recorded } = service(there, { [workspaceIdFor(there)]: there })
    expect((await missions.start('Start something', 'codex', 'ask', {}, () => undefined)).ok).toBe(true)
    expect(spawned).toEqual([here])
    expect(recorded).toEqual([workspaceIdFor(here)])
  })

  it('is refused, not run here, when its folder is not known', async () => {
    const { spawned, followUp } = service(there, {})
    const started = await followUp()
    expect(started.ok).toBe(false)
    if (!started.ok) expect(started.error.message).toMatch(/does not know which folder this conversation ran in/)
    expect(spawned).toEqual([])
  })

  it('is refused, not run here, when its folder is gone', async () => {
    const gone = join(there, 'deleted-project')
    const { spawned, followUp } = service(gone, { [workspaceIdFor(gone)]: gone })
    const started = await followUp()
    expect(started.ok).toBe(false)
    if (!started.ok) expect(started.error.message).toContain('that folder is not there any more')
    expect(spawned).toEqual([])
  })
})
