import type { MissionLedger } from '@teammate/mission-store'
import { describe, expect, it, vi } from 'vitest'

import type { AgentApi, AntigravityHost } from './antigravity-host.js'
import { antigravityExecutableCandidates, createAntigravityHostProbe, parseServerCommandLine, transcriptPathFor } from './antigravity-host.js'
import { ANTIGRAVITY_POLL_MS, AntigravityStartError, MAX_LIVE_ANTIGRAVITY_MISSIONS, antigravityStartRefusal, antigravityTier, createAntigravityMissionService } from './antigravity-mission.js'
import type { AntigravityMissionOptions, AntigravityMissionService } from './antigravity-mission.js'
import type { MissionPeerContext } from './workroom-briefing.js'

const NOW = '2026-09-03T09:00:00.000Z'
const HOME = 'C:\\Users\\dev'
const WORKSPACE = 'C:\\work\\pebble'
const CONVERSATION = '03a2fcb4-8fd9-468e-a683-6bf3a5acd077'

const SERVER_CMD =
  'C:\\Users\\dev\\AppData\\Local\\Programs\\antigravity\\resources\\bin\\language_server.exe --standalone --override_ide_name antigravity --subclient_type hub --override_ide_version 2.11.0 --https_server_port 0 --csrf_token 60843f52-6d41-4d97-9b31-53157a780b5e --app_data_dir antigravity'

/** The transcript Antigravity wrote for a write-then-answer conversation, as measured. */
const WRITE_LINES = [
  JSON.stringify({ step_index: 0, source: 'USER_EXPLICIT', type: 'USER_INPUT', status: 'DONE', created_at: NOW, content: '<USER_REQUEST>make hello.txt</USER_REQUEST>' }),
  JSON.stringify({ step_index: 1, source: 'SYSTEM', type: 'CHECKPOINT', status: 'DONE', created_at: NOW, content: '{{ CHECKPOINT 0 }} the whole conversation so far' }),
  JSON.stringify({
    step_index: 2,
    source: 'MODEL',
    type: 'PLANNER_RESPONSE',
    status: 'DONE',
    created_at: NOW,
    thinking: 'private reasoning',
    tool_calls: [{ name: 'write_to_file', args: { TargetFile: '"c:/work/pebble/hello.txt"', CodeContent: '"hello\\n"', Overwrite: 'true' } }]
  }),
  JSON.stringify({ step_index: 3, source: 'MODEL', type: 'GENERIC', status: 'DONE', created_at: NOW, content: 'Created file file:///c:/work/pebble/hello.txt with requested content.' }),
  JSON.stringify({ step_index: 4, source: 'MODEL', type: 'PLANNER_RESPONSE', status: 'DONE', created_at: NOW, content: 'DONE' })
]

function fakeLedger(refuseAppend = false): { ledger: MissionLedger; created: unknown[]; appended: { missionId: string; events: readonly { type: string; payload: Record<string, unknown> }[] }[] } {
  const created: unknown[] = []
  const appended: { missionId: string; events: readonly { type: string; payload: Record<string, unknown> }[] }[] = []
  const ledger = {
    createMission: async (input: unknown) => {
      created.push(input)
    },
    appendEvents: async (missionId: string, events: readonly { type: string; payload: Record<string, unknown> }[]) => {
      if (refuseAppend) throw new Error('no space left on device')
      appended.push({ missionId, events })
    },
    appendHostFailure: async () => undefined,
    getMission: async () => undefined,
    appendPeerLinks: async () => undefined, appendEditCheck: async () => undefined, appendApproval: async () => undefined
  } as unknown as MissionLedger
  return { ledger, created, appended }
}

function host(): AntigravityHost {
  return {
    executablePath: 'C:\\lang\\language_server.exe',
    version: '2.11.0',
    address: 'localhost:49839',
    csrfToken: '60843f52-6d41-4d97-9b31-53157a780b5e',
    projects: new Map([['c:/work/pebble', 'daf0f8ec-bb8e-49e8-a445-954eb0a62d0f']])
  }
}

interface Harness {
  readonly service: AntigravityMissionService
  readonly api: { calls: { kind: string; input: unknown }[] }
  readonly transcript: { lines: string[] }
  readonly emitted: { type: string; payload: Record<string, unknown> }[]
  readonly created: unknown[]
  readonly appended: ReturnType<typeof fakeLedger>['appended']
  readonly updates: { kind: string }[]
  readonly notices: { message: string }[]
  /** Every path handed to readTranscript (0.608): how often the file was read. */
  readonly reads: string[]
}

function harness(options: { host?: AntigravityHost | undefined; lines?: string[]; statTranscript?: (path: string) => Promise<{ size: number; mtimeMs: number } | undefined>; idleTimeoutMs?: number; askingNoticeMs?: number; notStartedNoticeMs?: number; refuseAppend?: boolean; ended?: string[]; spendRefusal?: (teammateId: string) => Promise<string | undefined>; observeDisk?: AntigravityMissionOptions['observeDisk']; observePatches?: AntigravityMissionOptions['observePatches'] } = {}): Harness {
  const { ledger, created, appended } = fakeLedger(options.refuseAppend === true)
  const api = { calls: [] as { kind: string; input: unknown }[] }
  const transcript = { lines: options.lines ?? [] }
  const emitted: { type: string; payload: Record<string, unknown> }[] = []
  const updates: { kind: string }[] = []
  const notices: { message: string }[] = []
  const reads: string[] = []
  let ids = 0
  const service = createAntigravityMissionService({
    workspacePath: WORKSPACE,
    ledger,
    probe: async () => (options.host === undefined && 'host' in options ? undefined : (options.host ?? host())),
    emitEvent: (_runId, _missionId, event) => emitted.push(event as { type: string; payload: Record<string, unknown> }),
    emitUpdate: (update) => updates.push(update as unknown as { kind: string }),
    agentApi: (): AgentApi => ({
      newConversation: async (input) => {
        api.calls.push({ kind: 'new', input })
        return CONVERSATION
      },
      sendMessage: async (input) => {
        api.calls.push({ kind: 'send', input })
      }
    }),
    readTranscript: async (path) => {
      reads.push(path)
      return path === transcriptPathFor(HOME, CONVERSATION) ? transcript.lines.join('\n') : undefined
    },
    ...(options.statTranscript === undefined ? {} : { statTranscript: options.statTranscript }),
    home: HOME,
    createId: () => String(++ids),
    now: () => new Date(NOW),
    pollMs: 5,
    notify: ({ message }) => notices.push({ message }),
    ...(options.ended === undefined ? {} : { onRunEnded: async ({ missionId }) => { options.ended!.push(missionId) } }),
    ...(options.idleTimeoutMs === undefined ? {} : { idleTimeoutMs: options.idleTimeoutMs }),
    ...(options.askingNoticeMs === undefined ? {} : { askingNoticeMs: options.askingNoticeMs }),
    ...(options.notStartedNoticeMs === undefined ? {} : { notStartedNoticeMs: options.notStartedNoticeMs }),
    ...(options.spendRefusal === undefined ? {} : { spendRefusal: options.spendRefusal }),
    ...(options.observeDisk === undefined ? {} : { observeDisk: options.observeDisk }),
    ...(options.observePatches === undefined ? {} : { observePatches: options.observePatches })
  })
  return { service, api, transcript, emitted, created, appended, updates, notices, reads }
}

const settle = async (ticks = 12): Promise<void> => {
  for (let i = 0; i < ticks; i += 1) await new Promise((resolve) => setTimeout(resolve, 10))
}

/*
 * L3 (the code review): the conversation is opened in Antigravity before the
 * mission is recorded -- deliberately, so a refusal leaves nothing behind --
 * but when the RECORD then failed, the person was told only that the start
 * failed, while Antigravity's agent was already working. A retry started it
 * twice.
 */
describe('a mission Antigravity started and Locust could not record', () => {
  it('says the agent may already be working, and not to send it again', async () => {
    const failing = { createMission: async () => { throw new Error('no space left on device') } }
    const service = createAntigravityMissionService({
      workspacePath: WORKSPACE,
      ledger: { ...fakeLedger().ledger, ...failing } as unknown as MissionLedger,
      probe: async () => host(),
      emitEvent: () => undefined,
      agentApi: (): AgentApi => ({ newConversation: async () => CONVERSATION, sendMessage: async () => undefined }),
      readTranscript: async () => undefined,
      home: HOME,
      createId: () => 'x',
      now: () => new Date(NOW),
      pollMs: 5
    })
    await expect(service.start('Fix the build.', undefined, {})).rejects.toThrow(/already started on this.*do not send it again/i)
  })
})

describe('finding the running Antigravity', () => {
  it("reads the token and version off the server's own command line, and only Antigravity's", () => {
    expect(parseServerCommandLine(SERVER_CMD)).toEqual({ csrfToken: '60843f52-6d41-4d97-9b31-53157a780b5e', version: '2.11.0' })
    expect(parseServerCommandLine(SERVER_CMD.replace('--override_ide_name antigravity', '--override_ide_name windsurf'))).toBeUndefined()
    expect(parseServerCommandLine('language_server.exe --override_ide_name antigravity')).toBeUndefined()
  })

  // Antigravity's app route is looked for on a Windows host only (%LOCALAPPDATA%\Programs), which joins its path this way.
  it.skipIf(process.platform !== 'win32')('looks under both spellings of the install folder', () => {
    expect(antigravityExecutableCandidates('C:\\lad')).toEqual([
      'C:\\lad\\Programs\\antigravity\\resources\\bin\\language_server.exe',
      'C:\\lad\\Programs\\Antigravity\\resources\\bin\\language_server.exe'
    ])
  })

  it('picks the listening port that speaks the RPC, not the one that drops the connection', async () => {
    const asked: string[] = []
    const probe = createAntigravityHostProbe({
      platform: 'win32',
      localAppData: 'C:\\lad',
      home: HOME,
      listProcesses: async () => [{ pid: 5344, commandLine: SERVER_CMD }],
      listeningPorts: async () => [49838, 49839],
      run: async (_exe, _args, env) => {
        asked.push(env.ANTIGRAVITY_LS_ADDRESS ?? '')
        return env.ANTIGRAVITY_LS_ADDRESS === 'localhost:49838'
          ? { stdout: '{"error":"rpc error: connection error: wsarecv: An established connection was aborted"}', stderr: '', code: 0 }
          : { stdout: '{"error":"conversation not found"}', stderr: '', code: 0 }
      }
    })
    // The executable check is a real filesystem look; nothing is installed at C:\lad.
    const record = await probe.discoveryRecord()
    expect(record.id).toBe('antigravity')
    expect(record.availability).toBe('unavailable')
    expect(asked).toEqual([])
  })
})

describe('a mission through Antigravity', () => {
  it('maps any model to a tier, defaulting to flash', () => {
    expect(antigravityTier('pro')).toBe('pro')
    expect(antigravityTier('flash_lite')).toBe('flash_lite')
    expect(antigravityTier('account-default')).toBe('flash')
    expect(antigravityTier(undefined)).toBe('flash')
  })

  it('refuses, recording nothing, when Antigravity is not open', async () => {
    const h = harness({ host: undefined })
    await expect(h.service.start('hi', undefined, {})).rejects.toThrow(/not open/)
    expect(h.created).toHaveLength(0)
    expect(h.api.calls).toHaveLength(0)
  })

  it('refuses, naming the folder, when Antigravity has not opened the workspace', async () => {
    const h = harness({ host: { ...host(), projects: new Map() } })
    await expect(h.service.start('hi', undefined, {})).rejects.toThrow(/has not opened C:\\work\\pebble/)
    expect(h.created).toHaveLength(0)
  })

  it('opens the conversation with the project id, records the mission, and follows the transcript to the final answer', async () => {
    const h = harness({ lines: [] })
    const mission = await h.service.start('make hello.txt', undefined, { model: 'flash' })
    expect(mission.conversationId).toBe(CONVERSATION)
    expect(h.api.calls[0]).toMatchObject({ kind: 'new', input: { projectId: 'daf0f8ec-bb8e-49e8-a445-954eb0a62d0f', model: 'flash' } })
    expect(h.created[0]).toMatchObject({ runtime: 'antigravity', model: 'flash', sandbox: 'workspace-write', resolvedRouteId: 'antigravity:hub' })
    expect(h.service.has(mission.runId)).toBe(true)

    // The agent works; lines arrive over time.
    h.transcript.lines = WRITE_LINES.slice(0, 4)
    await vi.waitFor(() => {
      expect(h.emitted.some((event) => event.type === 'tool.started' && event.payload.name === 'write_to_file')).toBe(true)
    })
    expect(h.service.has(mission.runId)).toBe(true)

    h.transcript.lines = WRITE_LINES
    await vi.waitFor(() => {
      const types = h.emitted.map((event) => event.type)
      expect(types).toContain('message.delta')
      expect(types.at(-1)).toBe('run.completed')
    })
    expect(h.emitted.at(-1)?.payload.runtimeThreadId ?? (h.emitted.at(-1) as { runtimeThreadId?: string }).runtimeThreadId).toBeDefined()
    expect(h.service.has(mission.runId)).toBe(false)
    // Persisted before emitted, and nothing private in what was persisted.
    const persisted = JSON.stringify(h.appended)
    expect(persisted).not.toContain('private reasoning')
    expect(persisted).not.toContain('the whole conversation so far')
  })

  it('a follow-up sends into the same conversation and reads only the lines after the earlier turns', async () => {
    const { ledger, created, appended } = fakeLedger()
    ;(ledger as unknown as { getMission: unknown }).getMission = async () => ({
      metadata: { missionId: 'mission_prior', runtime: 'antigravity' },
      events: [{ type: 'run.completed', payload: { runtimeThreadId: CONVERSATION } }]
    })
    const api = { calls: [] as { kind: string; input: unknown }[] }
    const transcript = { lines: [...WRITE_LINES] }
    const emitted: { type: string }[] = []
    let ids = 0
    const service = createAntigravityMissionService({
      workspacePath: WORKSPACE,
      ledger,
      probe: async () => host(),
      emitEvent: (_r, _m, event) => emitted.push(event as { type: string }),
      agentApi: (): AgentApi => ({
        newConversation: async () => {
          throw new Error('must not open a new conversation for a follow-up')
        },
        sendMessage: async (input) => {
          api.calls.push({ kind: 'send', input })
        }
      }),
      readTranscript: async () => transcript.lines.join('\n'),
      home: HOME,
      createId: () => String(++ids),
      now: () => new Date(NOW),
      pollMs: 5
    })
    const mission = await service.start('what did you reply?', undefined, { followUpOf: 'mission_prior' })
    expect(mission.followsUp).toEqual({ missionId: 'mission_prior', runtimeThreadId: CONVERSATION })
    expect(api.calls[0]).toMatchObject({ kind: 'send', input: { conversationId: CONVERSATION } })
    expect(created[0]).toMatchObject({ continuesFrom: { missionId: 'mission_prior', reason: 'follow-up', runtimeThreadId: CONVERSATION } })
    await settle()
    // Nothing from the earlier turns was replayed as this run's work.
    expect(emitted.some((event) => event.type === 'tool.started')).toBe(false)
    transcript.lines = [
      ...WRITE_LINES,
      JSON.stringify({ step_index: 5, source: 'SYSTEM', type: 'SYSTEM_MESSAGE', status: 'DONE', created_at: NOW, content: '<SYSTEM_MESSAGE>what did you reply?</SYSTEM_MESSAGE>' }),
      JSON.stringify({ step_index: 6, source: 'MODEL', type: 'PLANNER_RESPONSE', status: 'DONE', created_at: NOW, content: 'DONE' })
    ]
    await vi.waitFor(() => {
      expect(emitted.map((event) => event.type).at(-1)).toBe('run.completed')
    })
    expect(appended.length).toBeGreaterThan(0)
  })

  /**
   * Colin, 2026-09-21, on a run that sat for forty minutes: *"this antigrav
   * question was not appearing in locust ui, didnt know it was hung up."*
   *
   * Antigravity had stopped on its own "Allow reading this URL?" prompt --
   * read from his transcript afterwards: a `search_web` call at 12:09:53
   * with nothing after it. The sentence that explains this existed only in
   * the ten-minute timeout, which ENDS the mission, and his app restarted
   * for an update before it fired. So the one thing he needed to know was
   * ten minutes away and then never came at all.
   */
  it('says the agent may be asking, long before the timeout and without ending the run', async () => {
    // A tool is open (step 2 calls `write_to_file`) and nothing follows it.
    const h = harness({ lines: WRITE_LINES.slice(0, 3), askingNoticeMs: 15, idleTimeoutMs: 100_000 })
    const mission = await h.service.start('hi', undefined, {})
    await vi.waitFor(() => {
      const said = h.notices.map((notice) => notice.message).join(' | ')
      expect(said).toMatch(/write_to_file/)
      expect(said).toMatch(/its own window/i)
    })
    // NOT an ending: the run is still being watched.
    expect(h.service.has(mission.runId)).toBe(true)
    expect(h.emitted.map((event) => event.type)).not.toContain('run.failed')
  })

  /*
   * NOT STARTED (0.800): Antigravity took the turn -- it answered "new conversation" -- and wrote nothing for it.
   * 2026-10-19, after its window crashed: four turns sat "running" 150s each, and the only word was the ending.
   */
  it('says a turn Antigravity took has not started, once, without ending the run', async () => {
    const h = harness({ lines: [], notStartedNoticeMs: 15, idleTimeoutMs: 100_000 })
    const mission = await h.service.start('hi', undefined, {})
    await vi.waitFor(() => {
      expect(h.notices.map((notice) => notice.message).join(' | ')).toMatch(/has not started it.*open it again/i)
    })
    await settle(5)
    expect(h.notices).toHaveLength(1)
    expect(h.service.has(mission.runId)).toBe(true)
    expect(h.emitted.map((event) => event.type)).not.toContain('run.failed')
  })

  it('says nothing of the kind once the turn has written a line', async () => {
    const h = harness({ lines: WRITE_LINES.slice(0, 1), notStartedNoticeMs: 15, askingNoticeMs: 100_000, idleTimeoutMs: 400_000 })
    await h.service.start('hi', undefined, {})
    await settle(10)
    expect(h.notices.map((notice) => notice.message).join(' | ')).not.toMatch(/has not started/i)
  })

  it('says it once, not on every tick', async () => {
    const h = harness({ lines: WRITE_LINES.slice(0, 3), askingNoticeMs: 15, idleTimeoutMs: 100_000 })
    await h.service.start('hi', undefined, {})
    await vi.waitFor(() => {
      expect(h.notices).toHaveLength(1)
    })
    await settle(5)
    expect(h.notices).toHaveLength(1)
  })

  it('never lets the notice sit at or past the ending, where it would be dead code', async () => {
    // A caller that shortens the ending must still get one notice. Before
    // this, the 90 s default against a 20 s ending meant the notice could
    // never fire and nothing said so -- Builder.io's §6.4 in miniature.
    // A 1 s ending, waited for up to 5 s. At 200 ms the notice's window (half
    // the ending to the ending) was 100 ms wide, and under the whole suite's
    // load a tick stepped over it once (the 0.709 ship gate, 2026-10-08).
    const h = harness({ lines: WRITE_LINES.slice(0, 3), idleTimeoutMs: 1_000 })
    await h.service.start('hi', undefined, {})
    await vi.waitFor(() => {
      expect(h.notices.length).toBeGreaterThan(0)
      expect(h.notices[0]?.message).toMatch(/write_to_file/)
    }, { timeout: 5_000 })
  })

  it('says nothing while the agent is still writing', async () => {
    const h = harness({ lines: WRITE_LINES, askingNoticeMs: 15, idleTimeoutMs: 100_000 })
    await h.service.start('hi', undefined, {})
    await settle(6)
    // The transcript reaches a final answer, so there is no open tool and
    // no silence worth naming.
    expect(h.notices).toHaveLength(0)
  })

  it('gives up when the agent writes nothing for too long, as a failure and not a completion', async () => {
    const h = harness({ lines: [], idleTimeoutMs: 20 })
    const mission = await h.service.start('hi', undefined, {})
    await vi.waitFor(() => {
      expect(h.service.has(mission.runId)).toBe(false)
      expect(h.emitted.map((event) => event.type).at(-1)).toBe('run.failed')
    })
  })

  it('stopping stops the watch and says the agent may still be working', async () => {
    const h = harness({ lines: WRITE_LINES.slice(0, 3) })
    const mission = await h.service.start('hi', undefined, {})
    expect(h.service.cancel(mission.runId)).toBe(true)
    await vi.waitFor(() => {
      expect(h.service.has(mission.runId)).toBe(false)
      expect(h.emitted.map((event) => event.type).at(-1)).toBe('run.cancelled')
    })
    expect(h.service.cancel(mission.runId)).toBe(false)
  })

  it('one live mission per teammate', async () => {
    const h = harness({ lines: [] })
    const peer = { self: { teammateId: 'tm_w', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    await h.service.start('one', peer, {})
    await expect(h.service.start('two', peer, {})).rejects.toThrow(/already has a mission running/)
  })

  it('says a busy teammate is busy, so a relayed reply is held and not dropped (A2.19)', async () => {
    const h = harness({ lines: [] })
    const peer = { self: { teammateId: 'tm_w', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    await h.service.start('one', peer, {})
    const refused = await h.service.start('two', peer, {}).then(() => undefined, (error: unknown) => error)
    expect(antigravityStartRefusal(refused)).toEqual({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE', message: 'Wren already has a mission running. Wait for it to finish or stop it first.' }
    })
    // Anything else is a start that failed.
    expect(antigravityStartRefusal(new AntigravityStartError('Antigravity is not open.')).error.code).toBe('RUNTIME_START_FAILED')
  })

  it('says every slot being taken is busy too, and which kind', async () => {
    const h = harness({ lines: [] })
    for (let index = 0; index < MAX_LIVE_ANTIGRAVITY_MISSIONS; index += 1) {
      await h.service.start(`run ${String(index)}`, { self: { teammateId: `tm_${String(index)}`, name: `Mate${String(index)}`, role: 'Code & Migrations' }, others: [] }, {})
    }
    const extra = { self: { teammateId: 'tm_extra', name: 'Extra', role: 'Code & Migrations' }, others: [] }
    const refused = await h.service.start('one more', extra, {}).then(() => undefined, (error: unknown) => error)
    expect(antigravityStartRefusal(refused).error).toMatchObject({ code: 'RUN_ALREADY_ACTIVE', busy: 'pool' })
  })

  it("refuses a teammate at their monthly limit before Antigravity is even asked, and it is not 'busy'", async () => {
    const said = "Wren has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Wren, or it starts again on October 1."
    const h = harness({ lines: [], spendRefusal: async (teammateId) => (teammateId === 'tm_w' ? said : undefined) })
    const peer = { self: { teammateId: 'tm_w', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    const refused = await h.service.start('one', peer, {}).then(() => undefined, (error: unknown) => error)
    // Its own code: waiting for a run to end would start nothing, so a relay must not hold it.
    expect(antigravityStartRefusal(refused)).toEqual({ ok: false, error: { code: 'SPEND_LIMIT_REACHED', message: said } })
    expect(h.api.calls).toEqual([])
    expect(h.created).toHaveLength(0)
  })
})

describe('when the ledger refuses a receipt', () => {
  /*
   * The same defect the app-server path carried until 0.51.0, found by
   * sweeping for the shape rather than waiting for it to bite. This one was
   * worse in one specific way, which the second test pins.
   */
  it('ends the run and says so, instead of polling on with nobody recording', async () => {
    const test = harness({ lines: WRITE_LINES, refuseAppend: true })
    await test.service.start('Write hello.txt', undefined, {})
    await vi.waitFor(() => {
      expect(test.updates.some((update) => update.kind === 'persistence-error')).toBe(true)
    })
  })

  // A B4 lead from the code review, settled: a run ended this way never said
  // it had ended, so a relay reply held behind it, a room drain and memory
  // reading all waited on a run that was over.
  it('says the run ended, as every other end does', async () => {
    const ended: string[] = []
    const test = harness({ lines: WRITE_LINES, refuseAppend: true, ended })
    const mission = await test.service.start('Write hello.txt', undefined, {})
    await vi.waitFor(() => {
      expect(ended).toEqual([mission.missionId])
    })
  })

  it('does NOT advance past events it failed to write', async () => {
    /*
     * The cursor used to move BEFORE the write, so a ledger failure took those
     * events with it -- never persisted, never emitted, and never retried,
     * because the next tick started after them. Re-reading the transcript from
     * the same place is what makes a transient failure genuinely retryable.
     */
    const test = harness({ lines: WRITE_LINES, refuseAppend: true })
    await test.service.start('Write hello.txt', undefined, {})
    await vi.waitFor(() => {
      expect(test.updates.some((update) => update.kind === 'persistence-error')).toBe(true)
    })
    expect(test.appended).toHaveLength(0)
    expect(test.emitted).toHaveLength(0)
  })

  it('leaves a healthy run completely alone', async () => {
    // The control: without it both tests above would pass against a service
    // that ended every run it started.
    const test = harness({ lines: WRITE_LINES })
    await test.service.start('Write hello.txt', undefined, {})
    await vi.waitFor(() => {
      expect(test.appended.length).toBeGreaterThan(0)
    })
    expect(test.updates.some((update) => update.kind === 'persistence-error')).toBe(false)
  })
})

/*
 * 0.596. Colin's ledger (10/04): every Antigravity edit row read "Antigravity
 * did not report the change" -- the transcript names the file and never the
 * change, and this service, unlike the process transports, never looked at
 * the disk. Now it looks before the agent is told to begin and after the run.
 */
describe('what an Antigravity run changed on disk', () => {
  const PATCH = { text: '--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n', added: 1, removed: 0, truncated: false }
  /** Empty on the first look, hello.txt new on every look after. */
  const looks = (): { observeDisk: NonNullable<AntigravityMissionOptions['observeDisk']>; looked: string[] } => {
    const looked: string[] = []
    return {
      looked,
      observeDisk: async (folder) => {
        looked.push(folder)
        return looked.length === 1 ? new Map() : new Map([['hello.txt', '??']])
      }
    }
  }
  type Sequenced = { type: string; sequence: number; payload: Record<string, unknown> }

  it('reads the change behind a file the agent named and puts it after its own events, in sequence', async () => {
    const { observeDisk, looked } = looks()
    const h = harness({ lines: [], observeDisk, observePatches: async () => new Map([['hello.txt', PATCH]]) })
    await h.service.start('make hello.txt', undefined, { model: 'flash' })
    // Looked before the agent was told to begin.
    expect(looked).toEqual([WORKSPACE])
    expect(h.api.calls[0]?.kind).toBe('new')
    h.transcript.lines = WRITE_LINES
    await vi.waitFor(() => {
      expect(looked).toEqual([WORKSPACE, WORKSPACE])
      const all = h.appended.flatMap((entry) => entry.events) as unknown as Sequenced[]
      const completed = all.findIndex((event) => event.type === 'run.completed')
      expect(completed).toBeGreaterThan(0)
    })
    const all = h.appended.flatMap((entry) => entry.events) as unknown as Sequenced[]
    const completed = all.findIndex((event) => event.type === 'run.completed')
    const observed = all.slice(completed + 1)
    expect(observed.map((event) => event.type)).toEqual(['tool.started', 'tool.completed'])
    // write_to_file named hello.txt, so the observation is that row's patch, not a second row.
    expect(observed[0]!.payload).toMatchObject({ toolKind: 'observed_edit', command: 'hello.txt', status: 'reported by the runtime, read from disk' })
    expect(observed[1]!.payload).toMatchObject({ toolKind: 'observed_edit', phase: 'completed', patch: PATCH })
    expect(observed[0]!.sequence).toBe(all[completed]!.sequence + 1)
    expect(observed[1]!.sequence).toBe(observed[0]!.sequence + 1)
    // Persisted first, then emitted the same way.
    expect(h.emitted.slice(-2).map((event) => event.type)).toEqual(['tool.started', 'tool.completed'])
  })

  it('says nothing when nothing changed, and nothing when it could not look first', async () => {
    let looksMade = 0
    const quiet = harness({ lines: [], observeDisk: async () => { looksMade += 1; return new Map() }, observePatches: async () => { throw new Error('nothing to read') } })
    await quiet.service.start('make hello.txt', undefined, { model: 'flash' })
    quiet.transcript.lines = WRITE_LINES
    await vi.waitFor(() => {
      expect(looksMade).toBe(2)
      expect(quiet.appended.flatMap((entry) => entry.events).at(-1)?.type).toBe('run.completed')
    })

    let blindLooks = 0
    const blind = harness({ lines: [], observeDisk: async () => { blindLooks += 1; return undefined } })
    await blind.service.start('make hello.txt', undefined, { model: 'flash' })
    blind.transcript.lines = WRITE_LINES
    await vi.waitFor(() => {
      expect(blindLooks).toBe(1)
      expect(blind.appended.flatMap((entry) => entry.events).at(-1)?.type).toBe('run.completed')
    })
  })

  it('from a folder two Antigravity runs shared, says the reading names nobody rather than counting it', async () => {
    const { observeDisk } = looks()
    const h = harness({ lines: [], observeDisk, observePatches: async () => new Map([['hello.txt', PATCH]]) })
    const wren: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code' }, others: [] }
    const booty: MissionPeerContext = { self: { teammateId: 'tm_booty', name: 'Booty', role: 'Code' }, others: [] }
    await h.service.start('make hello.txt', wren, { model: 'flash' })
    await h.service.start('make hello.txt', booty, { model: 'flash' })
    h.transcript.lines = WRITE_LINES
    await vi.waitFor(() => {
      const all = h.appended.flatMap((entry) => entry.events)
      expect(all.some((event) => event.type === 'adapter.diagnostic' && event.payload.code === 'host.shared_workspace')).toBe(true)
    })
    const all = h.appended.flatMap((entry) => entry.events)
    expect(all.some((event) => event.payload.toolKind === 'observed_edit')).toBe(false)
  })
})

/*
 * A LIVE RUN IS READ FOUR TIMES A SECOND (0.608; the review's item 7), and a
 * transcript that has not changed is not read at all: the poll stats the
 * file first and reads only when its size or time has moved. Four stats a
 * second cost nothing; four whole-file reads a second of a transcript that
 * grows for an hour would.
 */
describe('how often a live Antigravity run is read', () => {
  it('polls four times a second by default', () => {
    expect(ANTIGRAVITY_POLL_MS).toBe(250)
  })

  it('reads the transcript only when its size or time has moved', async () => {
    const stats: { size: number; mtimeMs: number }[] = [{ size: 10, mtimeMs: 1 }]
    const h = harness({ lines: [], statTranscript: async () => stats[0] })
    await h.service.start('Fix the build.', undefined, {})
    await vi.waitFor(() => {
      expect(h.reads.length).toBeGreaterThan(0)
    })
    const afterFirst = h.reads.length
    // Twenty-odd polls at 5 ms, the file unmoved: no further read.
    await settle(10)
    expect(h.reads.length).toBe(afterFirst)
    // The file grew: one read, then quiet again.
    stats[0] = { size: 12, mtimeMs: 2 }
    await vi.waitFor(() => {
      expect(h.reads.length).toBe(afterFirst + 1)
    })
    await settle(6)
    expect(h.reads.length).toBe(afterFirst + 1)
  })

  it('reads every time when the file cannot be stated, as before', async () => {
    const h = harness({ lines: [], statTranscript: async () => undefined })
    await h.service.start('Fix the build.', undefined, {})
    await vi.waitFor(() => {
      expect(h.reads.length).toBeGreaterThan(3)
    })
  })
})
