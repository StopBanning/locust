import type { WorkroomMessage } from '@teammate/mission-store'
import { describe, expect, it } from 'vitest'

import type { CodexMissionStartResponse, CodexMissionUpdate } from '../shared/ipc.js'
import { MAX_RELAY_HOPS, budgetSentence, createRelay, decideRelay, meetingPrompt, relayPrompt } from './relay.js'
import type { RelayOptions, SharingMission } from './relay.js'
import type { MissionPeerContext } from './workroom-briefing.js'

const WREN = { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }
const BOOTY = { teammateId: 'tm_booty', name: 'Booty', role: 'Custom' }

const BOOTY_ROUTE = { runtime: 'claude' as const, model: 'sonnet', mode: 'ask' as const }
const wrenPeer: MissionPeerContext = { self: WREN, others: [BOOTY] }
const bootyPeer: MissionPeerContext = { self: { ...BOOTY, route: BOOTY_ROUTE }, others: [WREN] }
/** Booty before anyone has run them: no route of their own. */
const newBootyPeer: MissionPeerContext = { self: BOOTY, others: [WREN] }

function message(to: { teammateId: string; name: string }, text = 'What is the build command?'): WorkroomMessage {
  return {
    messageId: `msg_${to.teammateId}`,
    from: { teammateId: WREN.teammateId, name: WREN.name, missionId: 'mission_wren1' },
    to,
    text,
    postedAt: '2026-09-03T10:00:00.000Z'
  } as WorkroomMessage
}

function sharing(overrides: Partial<SharingMission> = {}): SharingMission {
  return {
    runId: 'run_wren1',
    missionId: 'mission_wren1',
    runtime: 'cursor',
    sandbox: 'workspace-write',
    model: 'composer-2.5',
    peer: wrenPeer,
    relay: undefined,
    ...overrides
  }
}

const BUSY: CodexMissionStartResponse = {
  ok: false,
  error: { code: 'RUN_ALREADY_ACTIVE', message: 'Booty already has a mission running.' }
}

function harness(options: {
  enabled?: boolean
  startResult?: CodexMissionStartResponse
  /** Answers by attempt, so a recipient can be busy on the first and free on the next. */
  startResults?: readonly (CodexMissionStartResponse | undefined)[]
  booty?: MissionPeerContext
  peerContextFor?: RelayOptions['peerContextFor']
  stillWaiting?: (teammateId: string) => Promise<boolean>
  /** The start path failing outright, not answering with an error. */
  throwOnStart?: boolean
  /** Whether a teammate may stop another's run to be heard now. */
  mayInterrupt?: boolean
  /** Whether there was anything to stop. */
  stopWorks?: boolean
  /** Whether the recipient's running turn takes a message at its next step (A2.10); absent: no steering at all. */
  steers?: boolean
  /** Whether Cursor can be held read-only here; absent reads as yes, the way the relay reads it. */
  cursorHoldsReadOnly?: boolean
  /** Why the recipient's plan cannot take a run (0.862); absent: the relay asks nothing. */
  outOfPlan?: RelayOptions['outOfPlan']
} = {}) {
  const starts: Parameters<RelayOptions['start']>[0][] = []
  const owners: [string, string][] = []
  const notices: CodexMissionUpdate[] = []
  const asked: string[] = []
  const stopped: string[] = []
  const steered: { teammateId: string; text: string }[] = []
  const kept: { missionId: string; message: string }[] = []
  let enabled = options.enabled ?? true
  const relay = createRelay({
    ...(options.cursorHoldsReadOnly === undefined ? {} : { cursorHoldsReadOnly: () => options.cursorHoldsReadOnly === true }),
    enabled: async () => enabled,
    ...(options.outOfPlan === undefined ? {} : { outOfPlan: options.outOfPlan }),
    note: async (input: { missionId: string; message: string }) => {
      kept.push(input)
    },
    mayInterrupt: async () => options.mayInterrupt === true,
    stopWorkOf: async (teammateId: string) => {
      stopped.push(teammateId)
      return options.stopWorks !== false
    },
    ...(options.steers === undefined
      ? {}
      : {
          steerWorkOf: async (teammateId: string, text: string) => {
            steered.push({ teammateId, text })
            return options.steers === true
          }
        }),
    ...(options.stillWaiting === undefined
      ? {}
      : {
          stillWaiting: async (teammateId: string) => {
            asked.push(teammateId)
            return options.stillWaiting!(teammateId)
          }
        }),
    peerContextFor: options.peerContextFor ?? (async (id) =>
      id === BOOTY.teammateId ? (options.booty ?? bootyPeer) : id === WREN.teammateId ? wrenPeer : undefined),
    start: async (input) => {
      starts.push(input)
      if (options.throwOnStart === true) throw new Error('the runtime is not there')
      const byAttempt = options.startResults?.[starts.length - 1]
      return (
        byAttempt ?? options.startResult ?? {
          ok: true,
          data: {
            runId: `run_${String(starts.length)}`,
            missionId: `mission_${String(starts.length)}`,
            runtime: input.runtime,
            model: input.model ?? 'account-default',
            resolvedRouteId: 'cursor-account:default',
            cliVersion: null,
            sandbox: input.mode === 'accept-edits' ? 'workspace-write' : 'read-only',
            peerMessages: [],
            peerDeliveryFailed: false
          }
        }
      )
    },
    assignOwner: async (teammateId, missionId) => {
      owners.push([teammateId, missionId])
    },
    notify: (update) => notices.push(update)
  })
  return {
    relay,
    starts,
    owners,
    notices,
    asked,
    stopped,
    steered,
    kept,
    switchOff: () => {
      enabled = false
    }
  }
}

/** A run ending, as the service reports it. */
function ended(missionId: string, peer: MissionPeerContext | undefined = bootyPeer) {
  return { missionId, peer, relay: undefined }
}

/** What the thread that shared was told, in order. */
function said(notices: readonly CodexMissionUpdate[]): string[] {
  return notices.flatMap((update) => (update.kind === 'relay-notice' ? [update.message] : []))
}

describe('deciding whether a teammate replies on their own', () => {
  it('when switched off, says the message waits', () => {
    const decision = decideRelay({ enabled: false, hop: 0, recipientName: 'Booty' })
    expect(decision.start).toBe(false)
    expect(decision.start ? '' : decision.reason).toMatch(/switched off in Settings/)
  })

  it('starts the first hop when switched on', () => {
    expect(decideRelay({ enabled: true, hop: 0, recipientName: 'Booty' })).toEqual({ start: true, hop: 1 })
  })

  it('stops at the cap, naming who will see the message next', () => {
    const decision = decideRelay({ enabled: true, hop: MAX_RELAY_HOPS, recipientName: 'Booty' })
    expect(decision.start).toBe(false)
    expect(decision.start ? '' : decision.reason).toContain('Booty')
    expect(decision.start ? '' : decision.reason).toContain(String(MAX_RELAY_HOPS))
  })

  it('the cap is a backstop, not a conversation length', () => {
    // It had stopped being one. Six was firing as the ordinary way an
    // exchange ended -- five runs of a one-word question went 6, 6, 3, 6, 7
    // (MEASURED 2026-09-11) -- and a limit that fires in the normal case is a
    // timer. Raised only after the brief was taught to name what a reply
    // costs, which is what made exchanges end on their own: 2, 5, 3.
    expect(MAX_RELAY_HOPS).toBe(12)
  })

  it('takes the person\u2019s own budget over the constant, and says the number it stopped at', () => {
    // 0.21.2 QA, rec. 6: "six hops is a useful backstop, but it is not a
    // user-controlled time/spend budget".
    expect(decideRelay({ enabled: true, hop: 2, recipientName: 'Booty', cap: 2 })).toMatchObject({ start: false })
    const stopped = decideRelay({ enabled: true, hop: 2, recipientName: 'Booty', cap: 2 })
    expect(stopped.start ? '' : stopped.reason).toContain('Stopped after 2 automatic replies')
    expect(decideRelay({ enabled: true, hop: 1, recipientName: 'Booty', cap: 2 })).toEqual({ start: true, hop: 2 })
    const one = decideRelay({ enabled: true, hop: 1, recipientName: 'Booty', cap: 1 })
    expect(one.start ? '' : one.reason).toContain('after 1 automatic reply.')
  })
})

describe('the brief a relayed run is started with', () => {
  it('names the sender, shows how to write back, and forbids unrelated work', () => {
    const prompt = relayPrompt({ sender: WREN, recipient: BOOTY, hop: 1 })
    expect(prompt).toContain('Wren (Code & Migrations) sent you a message')
    expect(prompt).toContain('<locust-share to="Wren">')
    expect(prompt).toContain('Stay on what was asked')
  })

  /*
   * 0.637: a relay run in Locust itself, two teammates on a free model. Wren
   * asked Booty to reply with exactly the word TANGERINE; Booty declined,
   * citing the brief's "in complete sentences". The person's own brief
   * already lets the shape they ask for win; a teammate's now does too.
   */
  it("lets the shape the sender asked for win over complete sentences", () => {
    const prompt = relayPrompt({ sender: WREN, recipient: BOOTY, hop: 1 })
    const ordinary = prompt.indexOf('in complete sentences')
    const shape = prompt.indexOf('If Wren said what shape the reply should take')
    expect(ordinary).toBeGreaterThan(0)
    expect(shape).toBeGreaterThan(ordinary)
    expect(prompt).toMatch(/one word, a number, a list -- give it in that shape inside the share block; that shape wins/)
  })

  it('every hop is told that silence is how an exchange finishes', () => {
    for (const hop of [1, 2, 5]) {
      const prompt = relayPrompt({ sender: BOOTY, recipient: WREN, hop })
      expect(prompt).toContain('When a reply is needed, end with one')
      expect(prompt).toContain('end with no share block')
    }
    expect(relayPrompt({ sender: BOOTY, recipient: WREN, hop: 2 })).toContain('Booty (Custom) replied to you')
  })

  it('tells a relayed run to ask the teammate, not a person', () => {
    // MEASURED 2026-09-05, three runs of relay-smoke: the recipient declined a
    // request as a possible prompt injection and asked for context with a
    // <locust-ask> block -- which reaches only a person, and a relayed run
    // has none. The question sat in a thread nobody was watching and the
    // exchange ended in silence. The teammate who asked is the right
    // recipient, and the share block is how to reach them.
    const prompt = relayPrompt({ sender: WREN, recipient: BOOTY, hop: 1 })
    expect(prompt).toContain('The person is not in this exchange')
    expect(prompt).toContain('ask Wren inside that share block')
    expect(prompt).toContain('A <locust-ask> block reaches only a person')
  })

  it('tells the reply that lands in the person\'s own conversation to say what it means for them', () => {
    // MEASURED 2026-09-16, Colin's ledger: "ask wembley to research aadx ...
    // and get back to you". Wembley's brief came back; Jimothy, told nobody
    // was watching, ended with a memory block and nothing for Colin.
    const prompt = relayPrompt({ sender: BOOTY, recipient: WREN, hop: 2, readByPerson: true })
    expect(prompt).toContain('the person who started this conversation reads it')
    expect(prompt).toContain('say in a line or two what Booty\'s reply means')
    expect(prompt).not.toContain('reach nobody and cost a run each')
    expect(prompt).not.toContain('nobody is watching this run')
    expect(prompt).toContain('a <locust-ask> block reaches them here')
    // The peer's side of the same exchange is still nobody's conversation.
    const peer = relayPrompt({ sender: WREN, recipient: BOOTY, hop: 3, readByPerson: false })
    expect(peer).toContain('reach nobody and cost a run each')
    // And a first message is never "read by the person", whoever it is for.
    expect(relayPrompt({ sender: WREN, recipient: BOOTY, hop: 1, readByPerson: true })).not.toContain('reads it')
  })

  it('says the same after a meeting, where several teammates could be asked', () => {
    const prompt = meetingPrompt({ repliers: ['Atlas', 'Juno'], silent: [] })
    expect(prompt).toContain('There is no person in this exchange')
    expect(prompt).toContain('never in a <locust-ask> block')
  })
})

describe('relaying a share', () => {
  it.each([undefined, BUSY])('keeps MCP shares visible without starts, steering, interruptions or deferred handoffs (%j)', async startResult => {
    const h = harness({ booty: { ...bootyPeer, self: { ...bootyPeer.self, route: { ...BOOTY_ROUTE, mode: 'auto' } } },
      mayInterrupt: true, steers: true, ...(startResult === undefined ? {} : { startResult }) })
    const posted = [{ ...message(BOOTY), urgent: true, wantsAnswer: true }]
    const before = JSON.stringify(posted)
    await h.relay.onShared(sharing({ sandbox: 'read-only', startedBy: { kind: 'mcp' } }), posted)
    await h.relay.onRunEnded(ended('mission_wren1', wrenPeer))
    await h.relay.onRunEnded(ended('mission_booty'))
    expect(h.starts).toHaveLength(0)
    expect(h.owners).toHaveLength(0)
    expect(h.stopped).toHaveLength(0)
    expect(h.steered).toHaveLength(0)
    expect(JSON.stringify(posted)).toBe(before)
    const messageText = 'Shares from another app are visible in Locust. No teammate was started automatically.'
    expect(h.kept).toEqual([{ missionId: 'mission_wren1', message: messageText }])
    expect(h.notices).toEqual([expect.objectContaining({ kind: 'relay-notice', message: messageText })])
  })

  it('starts nothing when off, and says so in the thread', async () => {
    const { relay, starts, notices } = harness({ enabled: false })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(0)
    expect(notices).toHaveLength(1)
    expect((notices[0] as { message?: string }).message).toMatch(/switched off in Settings/)
  })

  it('turns a failure between the decision and the start into a notice, never silence', async () => {
    const { relay, starts, notices } = harness({ peerContextFor: async () => { throw new Error('the roster could not be read.') } })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(0)
    expect(notices).toHaveLength(1)
    expect((notices[0] as { message?: string }).message).toMatch(/Booty could not reply on their own: the roster could not be read/)
  })

  it('starts no reply on a plan that is used up, and says so where the share was made (0.862)', async () => {
    // Colin's 2026-11-03: Codec handed a job to Argon, whose Antigravity plan was out until 9 November. Argon was
    // started anyway and failed twelve seconds in. Now nothing starts, and the sender's conversation says why.
    const asked: [string, string | undefined][] = []
    const h = harness({
      outOfPlan: async (runtime, account) => {
        asked.push([runtime, account])
        return 'their Claude Code plan is used up until Mon, Nov 9, 6:52 PM.'
      }
    })
    await h.relay.onShared(sharing(), [message(BOOTY)])
    expect(h.starts).toHaveLength(0)
    expect(h.owners).toHaveLength(0)
    expect(asked).toEqual([['claude', undefined]])
    const told = said(h.notices)
    expect(told).toEqual([
      'Booty did not reply on their own: their Claude Code plan is used up until Mon, Nov 9, 6:52 PM. The message waits for their next run. To have it answered sooner, give Booty another model on the Team screen.'
    ])
    // Kept in the sender's own record too, so reopening the thread later still says why.
    expect(h.kept.map((entry) => entry.message)).toEqual(told)
    expect(h.notices.some((update) => update.kind === 'relay-starting')).toBe(false)
  })

  it('a plan that can take the run is started as before, and a check that fails never blocks it', async () => {
    const fine = harness({ outOfPlan: async () => undefined })
    await fine.relay.onShared(sharing(), [message(BOOTY)])
    expect(fine.starts).toHaveLength(1)
    const broken = harness({ outOfPlan: async () => { throw new Error('the ledger could not be read') } })
    await broken.relay.onShared(sharing(), [message(BOOTY)])
    expect(broken.starts).toHaveLength(1)
  })

  it("starts the recipient's run on the recipient's OWN route, owned by the recipient", async () => {
    // Wren is Cursor / composer-2.5 with edits; Booty is Claude Code / sonnet,
    // read-only. Booty answers as Booty.
    const { relay, starts, owners, notices } = harness()
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(1)
    expect(starts[0]).toMatchObject({
      runtime: 'claude',
      mode: 'ask',
      model: 'sonnet',
      peer: bootyPeer,
      followUpOf: undefined,
      relay: { hop: 1, lastMissionOf: { tm_wren: 'mission_wren1' } }
    })
    expect(notices.some((update) => update.kind === 'relay-notice')).toBe(false)
    expect(starts[0]?.prompt).toContain('Wren (Code & Migrations) sent you a message')
    expect(owners).toEqual([[BOOTY.teammateId, 'mission_1']])
    const started = notices.find((update) => update.kind === 'mission-started')
    expect(started).toMatchObject({ kind: 'mission-started', teammateId: BOOTY.teammateId, startedBy: { kind: 'relay', hop: 1 } })
  })

  it('says so when a one-to-one reply never comes back, and where it went', async () => {
    // MEASURED 2026-09-05, Colin: Booty asked Wren whether Alphabet was
    // overvalued. Wren answered -- in prose, in its own conversation, with no
    // reply block -- so nothing came back and Booty's thread showed nothing
    // at all. A meeting says who stayed silent; an ordinary exchange said
    // nothing, which reads as the message never arriving.
    const { relay, starts, notices } = harness()
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(1)

    await relay.onRunEnded({ missionId: 'mission_1', peer: bootyPeer, relay: { hop: 1, lastMissionOf: {} } })
    const said = notices.filter((update) => update.kind === 'relay-notice').at(-1)
    expect(said?.kind === 'relay-notice' ? said.message : '').toBe(
      'Booty finished without writing back. Anything they said is in their own conversation.'
    )
    // Addressed to the thread that ASKED, not to the run that stayed quiet.
    expect(said?.kind === 'relay-notice' ? said.missionId : '').toBe('mission_wren1')
  })

  it('stays quiet when the reply DID come back', async () => {
    const { relay, notices } = harness()
    await relay.onShared(sharing(), [message(BOOTY)])
    // Booty's run answers Wren, which is the whole point; nothing to report.
    await relay.onShared(
      {
        runId: 'run_booty',
        missionId: 'mission_1',
        peer: bootyPeer,
        relay: { hop: 1, lastMissionOf: {} },
        runtime: 'claude',
        sandbox: 'read-only',
        model: 'sonnet'
      },
      [message(WREN)]
    )
    await relay.onRunEnded({ missionId: 'mission_1', peer: bootyPeer, relay: { hop: 1, lastMissionOf: {} } })
    expect(
      notices.filter((update) => update.kind === 'relay-notice').map((update) => (update.kind === 'relay-notice' ? update.message : ''))
    ).not.toContain('Booty finished without writing back. Anything they said is in their own conversation.')
  })

  it('a teammate who has never run borrows the sender\'s route, and the thread says so', async () => {
    const { relay, starts, notices } = harness({ booty: newBootyPeer })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts[0]).toMatchObject({ runtime: 'cursor', model: 'composer-2.5', mode: 'accept-edits' })
    const said = notices.find((update) => update.kind === 'relay-notice')
    expect(said?.kind === 'relay-notice' ? said.message : '').toContain('has no model of their own yet')
    // The runtime by name, never the raw model id, and as information (0.366).
    expect(said?.kind === 'relay-notice' ? said.message : '').toContain('(Cursor Agent)')
    expect(said?.kind === 'relay-notice' ? said.message : '').not.toContain('composer-2.5')
    expect(said?.kind === 'relay-notice' ? said.level : undefined).toBe('info')
    // And what it may DO on that route. A sender on Auto lends `ask`, never
    // the run of the whole machine, and a person whose reply came back
    // read-only had no way to find out why (QA, 2026-09-06).
    expect(said?.kind === 'relay-notice' ? said.message : '').toContain('may edit this folder')
  })

  it("says read-only when that is what the sender's route lends", async () => {
    // The case the QA named: a sender on full-access lends `ask`, so the
    // reply cannot write at all, and the notice has to say so.
    const { relay, starts, notices } = harness({ booty: newBootyPeer })
    await relay.onShared(sharing({ sandbox: 'full-access' }), [message(BOOTY)])
    expect(starts[0]).toMatchObject({ mode: 'ask' })
    const said = notices.find((update) => update.kind === 'relay-notice')
    expect(said?.kind === 'relay-notice' ? said.message : '').toContain('read-only')
  })

  it('tells a read-only reply that a command is refused and the message is quoted (0.592), and says nothing of it to one that may edit', async () => {
    // Handoff review finding (a), 2026-10-03: a recipient in Ask tried a
    // command, was refused, and wrote nothing -- the answer was in the quote.
    const ro = harness({ booty: bootyPeer })
    await ro.relay.onShared(sharing(), [message(BOOTY)])
    expect(ro.starts[0]).toMatchObject({ mode: 'ask' })
    expect(ro.starts[0]?.prompt).toContain('You are replying in a read-only mode: a shell command is refused here and ends this run with nothing written.')
    expect(ro.starts[0]?.prompt).toContain('answer from it and from your read tools')
    const rw = harness({ booty: { self: { ...BOOTY, route: { ...BOOTY_ROUTE, mode: 'accept-edits' } }, others: [WREN] } })
    await rw.relay.onShared(sharing(), [message(BOOTY)])
    expect(rw.starts[0]).toMatchObject({ mode: 'accept-edits' })
    expect(rw.starts[0]?.prompt).not.toContain('read-only mode')
  })

  it('lends Accept edits to a Cursor recipient where read-only cannot be held, and says so', async () => {
    // MEASURED 2026-09-17 on Windows: a relayed start in `ask` on Cursor is
    // refused by the mission service ("cannot be held read-only on this
    // system"), so the reply never ran and the exchange died at hop 1. A
    // sender on Auto lends `ask`; on this platform that is a reply that
    // cannot start. Accept edits is the least it can run as.
    const { relay, starts, notices } = harness({ booty: newBootyPeer, cursorHoldsReadOnly: false })
    await relay.onShared(sharing({ sandbox: 'full-access' }), [message(BOOTY)])
    expect(starts[0]).toMatchObject({ runtime: 'cursor', mode: 'accept-edits' })
    const said = notices.filter((update) => update.kind === 'relay-notice').map((update) => (update.kind === 'relay-notice' ? update.message : ''))
    expect(said.some((line) => line.includes('replies in Edit mode rather than read-only: Cursor Agent cannot be held read-only'))).toBe(true)
    // A remembered Cursor route in ask is widened the same way; a Codex one is not.
    const remembered = harness({ booty: { ...newBootyPeer, self: { ...newBootyPeer.self, route: { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' } } }, cursorHoldsReadOnly: false })
    await remembered.relay.onShared(sharing(), [message(BOOTY)])
    expect(remembered.starts[0]).toMatchObject({ runtime: 'cursor', mode: 'accept-edits' })
    const codex = harness({ booty: { ...newBootyPeer, self: { ...newBootyPeer.self, route: { runtime: 'codex', model: 'account-default', mode: 'ask' } } }, cursorHoldsReadOnly: false })
    await codex.relay.onShared(sharing(), [message(BOOTY)])
    expect(codex.starts[0]).toMatchObject({ runtime: 'codex', mode: 'ask' })
  })

  it('a read-only sender gets a read-only reply when the recipient has no route', async () => {
    const { relay, starts } = harness({ booty: newBootyPeer })
    await relay.onShared(sharing({ sandbox: 'read-only' }), [message(BOOTY)])
    expect(starts[0]?.mode).toBe('ask')
  })

  it("an account-default route is sent as no model, the way the composer sends it", async () => {
    const { relay, starts } = harness({ booty: { self: { ...BOOTY, route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }, others: [WREN] } })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts[0]).toMatchObject({ runtime: 'codex', model: undefined })
  })

  // M10 (the code review): a relayed reply ran at the runtime's default
  // effort, whatever the teammate was saved at -- the model was passed and
  // the effort was not.
  it("a reply runs at the recipient's own saved effort", async () => {
    const { relay, starts } = harness({ booty: { self: { ...BOOTY, route: { runtime: 'codex', model: 'gpt-5.5', mode: 'ask', effort: 'high' } }, others: [WREN] } })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts[0]).toMatchObject({ runtime: 'codex', model: 'gpt-5.5', effort: 'high' })
  })

  it("a borrowed route borrows no effort", async () => {
    const { relay, starts } = harness({ booty: { self: { ...BOOTY }, others: [WREN] } })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts[0]?.effort).toBeUndefined()
  })

  it("the reply back follows up the mission that asked, so it lands in that thread", async () => {
    const { relay, starts } = harness()
    const bootyRun = sharing({
      runId: 'run_booty',
      missionId: 'mission_booty',
      peer: bootyPeer,
      relay: { hop: 1, lastMissionOf: { tm_wren: 'mission_wren1' } }
    })
    await relay.onShared(bootyRun, [{ ...message(WREN), from: { ...BOOTY, missionId: 'mission_booty' } } as WorkroomMessage])
    expect(starts).toHaveLength(1)
    expect(starts[0]).toMatchObject({
      peer: wrenPeer,
      followUpOf: 'mission_wren1',
      relay: { hop: 2, lastMissionOf: { tm_wren: 'mission_wren1', tm_booty: 'mission_booty' } }
    })
  })

  it("a third hop continues the recipient's OWN earlier turn, so their side reads as one thread too", async () => {
    const { relay, starts } = harness()
    const wrenAgain = sharing({
      runId: 'run_wren2',
      missionId: 'mission_wren2',
      relay: { hop: 2, lastMissionOf: { tm_wren: 'mission_wren1', tm_booty: 'mission_booty' } }
    })
    await relay.onShared(wrenAgain, [message(BOOTY, 'One more thing.')])
    expect(starts[0]).toMatchObject({
      peer: bootyPeer,
      followUpOf: 'mission_booty',
      relay: { hop: 3, lastMissionOf: { tm_wren: 'mission_wren2', tm_booty: 'mission_booty' } }
    })
  })

  it('stops after the cap and says so in the thread that shared', async () => {
    const { relay, starts, notices } = harness()
    const capped = sharing({ runId: 'run_wren2', missionId: 'mission_wren2', relay: { hop: MAX_RELAY_HOPS, lastMissionOf: { tm_wren: 'mission_wren1' } } })
    await relay.onShared(capped, [message(BOOTY)])
    expect(starts).toHaveLength(0)
    expect(notices).toHaveLength(1)
    expect(notices[0]).toMatchObject({ kind: 'relay-notice', runId: 'run_wren2', missionId: 'mission_wren2' })
    expect(notices[0]?.kind === 'relay-notice' ? notices[0].message : '').toContain(`Stopped after ${String(MAX_RELAY_HOPS)}`)
  })

  it("says why when the recipient's run could not start, and lets the message wait", async () => {
    const { relay, notices, owners } = harness({
      startResult: { ok: false, error: { code: 'RUNTIME_START_FAILED', message: 'Claude is not installed.' } }
    })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(owners).toHaveLength(0)
    expect(said(notices)[0]).toContain('Claude is not installed')
    expect(said(notices)[0]).toContain('waits for their next run')
  })

  /*
   * Colin, 2026-09-11, watching two teammates asked to argue: the room drew
   * one opening card each and nothing after. MEASURED with a trace compiled
   * into the renderer -- two missions started, two peer messages sent, zero
   * hops. Both answered at once, so each wrote to the other while the other
   * was still running, and a teammate takes one mission at a time. The relay
   * called that a failed start and gave up, and nothing ever started the
   * "next run" its own notice promised. The argument died at hop zero.
   */
  /*
   * EVERY SLOT TAKEN, THE RECIPIENT IDLE (harness review, 2026-09-24). The
   * pool cap answers with the same code as a busy teammate, so the reply was
   * held until the RECIPIENT's run ended -- which, for an idle one, never
   * came -- and the thread said they were "part-way through another mission".
   */
  describe('a reply held because every run slot was taken', () => {
    const POOL_FULL: CodexMissionStartResponse = {
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE', message: 'Up to 6 missions can run at once. Wait for one to finish or stop it first.', busy: 'pool' }
    }

    it('says so, not that the recipient is mid-run', async () => {
      const { relay, notices } = harness({ startResults: [POOL_FULL] })
      await relay.onShared(sharing(), [message(BOOTY)])
      expect(said(notices)[0]).toContain('Every run slot is in use')
      expect(said(notices)[0]).not.toContain('part-way')
    })

    it('starts when ANY run ends, not only the recipient’s own', async () => {
      const { relay, starts, owners } = harness({ startResults: [POOL_FULL] })
      await relay.onShared(sharing(), [message(BOOTY)])
      // Someone else's run ends -- Juno's, a stranger to this exchange -- and a slot frees.
      await relay.onRunEnded(ended('mission_juno', { self: { teammateId: 'tm_juno', name: 'Juno', role: 'Custom' }, others: [] }))
      expect(starts).toHaveLength(2)
      expect(owners).toEqual([['tm_booty', 'mission_2']])
    })

    it('is held again, quietly, while every slot is still taken', async () => {
      const { relay, starts, notices } = harness({ startResults: [POOL_FULL, POOL_FULL] })
      await relay.onShared(sharing(), [message(BOOTY)])
      await relay.onRunEnded(ended('mission_juno', { self: { teammateId: 'tm_juno', name: 'Juno', role: 'Custom' }, others: [] }))
      expect(starts).toHaveLength(2)
      expect(said(notices).filter((line) => line.includes('Every run slot'))).toHaveLength(1)
    })
  })

  describe('a reply whose recipient is mid-run', () => {
    it('is held rather than dropped, and the thread that shared is told', async () => {
      const { relay, owners, notices } = harness({ startResults: [BUSY] })
      await relay.onShared(sharing(), [message(BOOTY)])
      expect(owners).toHaveLength(0)
      expect(said(notices)[0]).toContain('part-way through another mission')
      expect(said(notices)[0]).toContain('starts when it ends')
    })

    it('starts the moment that run ends', async () => {
      const { relay, starts, owners } = harness({ startResults: [BUSY] })
      await relay.onShared(sharing(), [message(BOOTY)])
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(2)
      // The same brief, on Booty's own route, as the first attempt.
      expect(starts[1]).toMatchObject({ runtime: 'claude', model: 'sonnet', mode: 'ask', relay: { hop: 1 } })
      expect(owners).toEqual([['tm_booty', 'mission_2']])
    })

    it('is held once however many messages pile up for one teammate', async () => {
      // Everything unread is folded into one brief, so a second held reply
      // would start a second run that had already been told everything.
      const { relay, starts, notices } = harness({ startResults: [BUSY, BUSY] })
      await relay.onShared(sharing(), [message(BOOTY, 'first')])
      await relay.onShared(sharing({ runId: 'run_wren2', missionId: 'mission_wren2' }), [message(BOOTY, 'second')])
      expect(said(notices).filter((line) => line.includes('part-way'))).toHaveLength(1)
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(3)
    })

    it('tells every thread that waited on the held reply how it ended (2026-10-10 sweep)', async () => {
      const { relay, starts, notices } = harness({ startResults: [BUSY, BUSY] })
      await relay.onShared(sharing(), [message(BOOTY, 'first')])
      await relay.onShared(sharing({ runId: 'run_wren2', missionId: 'mission_wren2' }), [message(BOOTY, 'second')])
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(3)
      // The held reply ran, as mission_3, and wrote to nobody.
      await relay.onRunEnded(ended('mission_3'))
      const told = notices.flatMap((update) => (update.kind === 'relay-notice' && update.message.includes('finished without writing back') ? [update.missionId] : []))
      expect(told).toContain('mission_wren2')
      expect(told).toHaveLength(2)
    })

    it('waits again when the teammate was taken by something else in between', async () => {
      const { relay, starts } = harness({ startResults: [BUSY, BUSY] })
      await relay.onShared(sharing(), [message(BOOTY)])
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(2)
      // Still held: the next ending is another chance, not a lost message.
      await relay.onRunEnded(ended('mission_booty2'))
      expect(starts).toHaveLength(3)
    })

    it('does not start when a later run already carried the message', async () => {
      // A person messaged Booty while the reply was held. Whatever a teammate
      // has not read rides along on the next run whoever started it, so the
      // message has arrived and a second run would brief an empty inbox.
      const { relay, starts, asked } = harness({ startResults: [BUSY], stillWaiting: async () => false })
      await relay.onShared(sharing(), [message(BOOTY)])
      await relay.onRunEnded(ended('mission_booty'))
      expect(asked).toEqual(['tm_booty'])
      expect(starts).toHaveLength(1)
    })

    it('does not start once replies have been switched off', async () => {
      const { relay, starts, notices, switchOff } = harness({ startResults: [BUSY] })
      await relay.onShared(sharing(), [message(BOOTY)])
      switchOff()
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(1)
      expect(said(notices).at(-1)).toContain('switched off in Settings')
    })

    it('still tells the asker when the held reply finishes without writing back', async () => {
      const { relay, notices } = harness({ startResults: [BUSY] })
      await relay.onShared(sharing(), [message(BOOTY)])
      await relay.onRunEnded(ended('mission_booty'))
      // mission_2 is the run the held reply started.
      await relay.onRunEnded(ended('mission_2'))
      expect(said(notices).at(-1)).toContain('Booty finished without writing back')
      expect(notices.at(-1)).toMatchObject({ runId: 'run_wren1', missionId: 'mission_wren1' })
    })
  })

  /*
   * `mission-started` needs a run id, so it cannot go out until the runtime
   * is up -- and a cold Cursor Agent takes long enough that every row read
   * idle and a room in the middle of an argument looked finished (MEASURED
   * 2026-09-11). This pair is what the window has in the meantime.
   */
  describe('saying a hop is coming before it exists', () => {
    it('names the teammate and what they are answering, before the start', async () => {
      const { relay, notices } = harness()
      await relay.onShared(sharing(), [message(BOOTY)])
      const said = notices.find((update) => update.kind === 'relay-starting')
      expect(said).toMatchObject({
        kind: 'relay-starting',
        teammateId: 'tm_booty',
        name: 'Booty',
        answering: 'mission_wren1',
        hop: 1
      })
      // Before, so the window is never behind the host.
      expect(notices.indexOf(said!)).toBeLessThan(
        notices.findIndex((update) => update.kind === 'mission-started')
      )
    })

    it('clears it however the start ended', async () => {
      for (const result of [undefined, BUSY, { ok: false, error: { code: 'RUNTIME_START_FAILED', message: 'no' } } as const]) {
        const { relay, notices } = harness(result === undefined ? {} : { startResult: result })
        await relay.onShared(sharing(), [message(BOOTY)])
        expect(notices.filter((update) => update.kind === 'relay-starting')).toHaveLength(1)
        expect(notices.filter((update) => update.kind === 'relay-start-settled')).toHaveLength(1)
      }
    })

    it('clears it when the start throws, so a row is never stuck working', async () => {
      const { relay, notices } = harness({ throwOnStart: true })
      await relay.onShared(sharing(), [message(BOOTY)])
      expect(notices.filter((update) => update.kind === 'relay-start-settled')).toHaveLength(1)
    })
  })

  /*
   * A sender asking to be taken NOW -- `when="now"`.
   *
   * A teammate does one thing at a time, so a message that lands mid-run
   * waits. Usually right, and sometimes far too late: the message worth
   * interrupting for is "stop, I am editing that file", and delivering it
   * once the conflicting work is done delivers it after the damage.
   *
   * Stopping is the WHOLE action. The reply is started by the ordinary
   * held-message path when the run ends, so an interruption can only shorten
   * a wait -- never become a second way to start a mission, and never outrun
   * the hop cap or the relay switch, because it does not go near either.
   */
  describe('a message that asks to be taken now', () => {
    const urgent = (to: { teammateId: string; name: string }) => ({ ...message(to), urgent: true })

    it('stops the recipient when the person has allowed it', async () => {
      const { relay, stopped, notices } = harness({ startResults: [BUSY], mayInterrupt: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(stopped).toEqual(['tm_booty'])
      expect(said(notices).at(-1)).toContain('stopped part-way')
    })

    it('stops nobody when it is switched off, and says so rather than going quiet', async () => {
      const { relay, stopped, notices } = harness({ startResults: [BUSY] })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(stopped).toEqual([])
      expect(said(notices).at(-1)).toContain('switched off in Settings')
    })

    it('stops nobody when the recipient was free anyway', async () => {
      const { relay, stopped, starts } = harness({ mayInterrupt: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(starts).toHaveLength(1)
      expect(stopped).toEqual([])
    })

    it('leaves an ordinary message waiting, however busy the recipient is', async () => {
      const { relay, stopped } = harness({ startResults: [BUSY], mayInterrupt: true })
      await relay.onShared(sharing(), [message(BOOTY)])
      expect(stopped).toEqual([])
    })

    it('still only holds the message once: stopping is not a second way to start', async () => {
      // The reply goes through the same path every waiting message takes.
      const { relay, starts, stopped } = harness({ startResults: [BUSY], mayInterrupt: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(starts).toHaveLength(1)
      expect(stopped).toEqual(['tm_booty'])
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(2)
    })

    it('says nothing about stopping when there was nothing to stop', async () => {
      // The run ended between the refusal and the stop. Claiming otherwise
      // would put a sentence about discarded work in a thread where none was.
      const { relay, notices } = harness({ startResults: [BUSY], mayInterrupt: true, stopWorks: false })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(said(notices).some((line) => line.includes('stopped part-way'))).toBe(false)
    })

    it('cannot outrun the budget: a capped exchange never reaches the stop', async () => {
      const { relay, stopped } = harness({ mayInterrupt: true })
      const capped = sharing({ relay: { hop: MAX_RELAY_HOPS, lastMissionOf: { tm_wren: 'mission_wren1' } } })
      await relay.onShared(capped, [urgent(BOOTY)])
      expect(stopped).toEqual([])
    })

    it('cannot outrun the relay switch either', async () => {
      const { relay, stopped } = harness({ enabled: false, mayInterrupt: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(stopped).toEqual([])
    })

    /*
     * A2.10: SAFE-POINT DELIVERY. Where the recipient's runtime can take a
     * message into the running turn (Codex's app-server, `turn/steer`), that
     * comes first: nothing is discarded, so it needs no switch, and nobody is
     * stopped. It is a heads-up only -- the message is still their next turn.
     */
    it('is shown to a running turn that can take it, at its next step, and nobody is stopped', async () => {
      const { relay, steered, stopped, notices } = harness({ startResults: [BUSY], steers: true, mayInterrupt: true })
      await relay.onShared(sharing(), [{ ...urgent(BOOTY), text: 'Stop editing app.ts, I am changing it.' }])
      expect(steered).toHaveLength(1)
      expect(steered[0]!.teammateId).toBe('tm_booty')
      expect(steered[0]!.text).toContain('Wren sent you this while you were working')
      expect(steered[0]!.text).toContain('Stop editing app.ts, I am changing it.')
      expect(stopped).toEqual([])
      expect(said(notices).at(-1)).toContain('without stopping it')
    })

    it('needs no switch to be shown: it loses nothing', async () => {
      const { relay, steered, notices } = harness({ startResults: [BUSY], steers: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(steered).toHaveLength(1)
      expect(said(notices).some((line) => line.includes('switched off in Settings'))).toBe(false)
    })

    it('is still their next turn once the run ends: a heads-up, not a second answer', async () => {
      const { relay, starts } = harness({ startResults: [BUSY], steers: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(starts).toHaveLength(1)
      await relay.onRunEnded(ended('mission_booty'))
      expect(starts).toHaveLength(2)
    })

    it('falls back to the stop, under the switch, when the running turn cannot take it', async () => {
      const { relay, steered, stopped } = harness({ startResults: [BUSY], steers: false, mayInterrupt: true })
      await relay.onShared(sharing(), [urgent(BOOTY)])
      expect(steered).toHaveLength(1)
      expect(stopped).toEqual(['tm_booty'])
    })

    it('shows the running turn no protocol blocks from the sender', async () => {
      const { relay, steered } = harness({ startResults: [BUSY], steers: true })
      await relay.onShared(sharing(), [{ ...urgent(BOOTY), text: `Heads up. <locust-task>
claim :: Ship it
</locust-task>` }])
      expect(steered[0]!.text).not.toContain('<locust-task>')
    })

    it('never steers an ordinary message', async () => {
      const { relay, steered } = harness({ startResults: [BUSY], steers: true })
      await relay.onShared(sharing(), [message(BOOTY)])
      expect(steered).toEqual([])
    })
  })

  it('starts one run per recipient however many messages went to them', async () => {
    const { relay, starts } = harness()
    await relay.onShared(sharing(), [message(BOOTY, 'first'), message(BOOTY, 'second')])
    expect(starts).toHaveLength(1)
  })

  it('does not start anything for a recipient no longer on the roster', async () => {
    const { relay, starts, notices } = harness()
    await relay.onShared(sharing(), [message({ teammateId: 'tm_gone', name: 'Gone' })])
    expect(starts).toHaveLength(0)
    expect(notices[0]?.kind === 'relay-notice' ? notices[0].message : '').toContain('no longer on the roster')
  })

  it('treats an unreadable setting as off', async () => {
    const starts: unknown[] = []
    const relay = createRelay({
      enabled: async () => {
        throw new Error('disk')
      },
      peerContextFor: async () => bootyPeer,
      start: async (input) => {
        starts.push(input)
        throw new Error('should not start')
      },
      assignOwner: async () => undefined,
      notify: () => undefined
    })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(0)
  })
})

describe('a meeting: one asks several, and the next turn waits for all of them', () => {
  const ATLAS = { teammateId: 'tm_atlas', name: 'Atlas', role: 'Research & Briefs' }
  const ATLAS_ROUTE = { runtime: 'codex' as const, model: 'account-default', mode: 'ask' as const }
  const three = (self: { teammateId: string; name: string; role: string; route?: unknown }): MissionPeerContext => ({
    self: self as MissionPeerContext['self'],
    others: [WREN, BOOTY, ATLAS].filter((entry) => entry.teammateId !== self.teammateId)
  })
  function meetingHarness() {
    const starts: Parameters<RelayOptions['start']>[0][] = []
    const notices: string[] = []
    let count = 0
    const relay = createRelay({
      enabled: async () => true,
      peerContextFor: async (id) =>
        id === WREN.teammateId ? three(WREN) : id === BOOTY.teammateId ? three({ ...BOOTY, route: BOOTY_ROUTE }) : id === ATLAS.teammateId ? three({ ...ATLAS, route: ATLAS_ROUTE }) : undefined,
      start: async (input) => {
        starts.push(input)
        count += 1
        return {
          ok: true,
          data: {
            runId: `run_${String(count)}`,
            missionId: `mission_${input.peer.self.name.toLowerCase()}_${String(count)}`,
            runtime: input.runtime,
            model: input.model ?? 'account-default',
            resolvedRouteId: 'x',
            cliVersion: null,
            sandbox: 'read-only',
            peerMessages: [],
            peerDeliveryFailed: false
          }
        }
      },
      assignOwner: async () => undefined,
      notify: (update) => {
        if (update.kind === 'relay-notice') notices.push(update.message)
      }
    })
    return { relay, starts, notices }
  }
  const wrenAsks = (): SharingMission => ({ ...sharing(), peer: three(WREN) })
  const toBoth = (): WorkroomMessage[] => [message(BOOTY, 'Booty, check the tests.'), message(ATLAS, 'Atlas, check the docs.')]

  it('starts a run for each, and says the next turn is waiting on them', async () => {
    const { relay, starts, notices } = meetingHarness()
    await relay.onShared(wrenAsks(), toBoth())
    expect(starts.map((s) => s.peer.self.name)).toEqual(['Booty', 'Atlas'])
    expect(notices.at(-1)).toBe('Waiting on Booty, Atlas to reply before your next turn.')
  })

  it("holds the first reply, and starts the asker's turn once after the last, briefed with everyone", async () => {
    const { relay, starts, notices } = meetingHarness()
    await relay.onShared(wrenAsks(), toBoth())
    const bootyRun: SharingMission = { ...sharing(), runId: 'run_1', missionId: 'mission_booty_1', peer: three({ ...BOOTY, route: BOOTY_ROUTE }), relay: starts[0]!.relay }
    await relay.onShared(bootyRun, [{ ...message(WREN, 'Tests pass.'), from: { ...BOOTY, missionId: 'mission_booty_1' } } as WorkroomMessage])
    expect(starts).toHaveLength(2)
    expect(notices.at(-1)).toBe('Booty replied. Still waiting on Atlas.')
    const atlasRun: SharingMission = { ...sharing(), runId: 'run_2', missionId: 'mission_atlas_2', peer: three({ ...ATLAS, route: ATLAS_ROUTE }), relay: starts[1]!.relay }
    await relay.onShared(atlasRun, [{ ...message(WREN, 'Docs are stale.'), from: { ...ATLAS, missionId: 'mission_atlas_2' } } as WorkroomMessage])
    expect(starts).toHaveLength(3)
    expect(starts[2]).toMatchObject({ peer: three(WREN), followUpOf: 'mission_wren1' })
    expect(starts[2]?.prompt).toContain('Booty and Atlas replied to your message')
  })

  it('counts a teammate who finishes without replying, and still convenes the rest', async () => {
    const { relay, starts, notices } = meetingHarness()
    await relay.onShared(wrenAsks(), toBoth())
    await relay.onRunEnded({ missionId: 'mission_booty_1', peer: three(BOOTY), relay: starts[0]!.relay })
    expect(notices.at(-1)).toBe('Booty finished without replying. Still waiting on Atlas.')
    const atlasRun: SharingMission = { ...sharing(), runId: 'run_2', missionId: 'mission_atlas_2', peer: three({ ...ATLAS, route: ATLAS_ROUTE }), relay: starts[1]!.relay }
    await relay.onShared(atlasRun, [{ ...message(WREN, 'Docs are stale.'), from: { ...ATLAS, missionId: 'mission_atlas_2' } } as WorkroomMessage])
    expect(starts).toHaveLength(3)
    expect(starts[2]?.prompt).toContain('Atlas replied to your message')
    expect(starts[2]?.prompt).toContain('Booty finished without replying')
  })

  it('says so when nobody replied, and starts nothing', async () => {
    const { relay, starts, notices } = meetingHarness()
    await relay.onShared(wrenAsks(), toBoth())
    await relay.onRunEnded({ missionId: 'mission_booty_1', peer: three(BOOTY), relay: starts[0]!.relay })
    await relay.onRunEnded({ missionId: 'mission_atlas_2', peer: three(ATLAS), relay: starts[1]!.relay })
    expect(starts).toHaveLength(2)
    expect(notices.at(-1)).toBe('Nobody replied. Your message waits with them for their next run.')
  })

  it('one recipient is an ordinary exchange, not a meeting', async () => {
    const { relay, notices } = meetingHarness()
    await relay.onShared(wrenAsks(), [message(BOOTY)])
    expect(notices.some((n) => n.startsWith('Waiting on'))).toBe(false)
  })

  it('names everyone in the minutes', () => {
    expect(meetingPrompt({ repliers: ['Booty', 'Atlas', 'Juno'], silent: [] })).toContain('Booty, Atlas and Juno replied')
    expect(meetingPrompt({ repliers: ['Booty'], silent: ['Atlas'] })).toContain('Atlas finished without replying')
  })
})

/*
 * MEASURED 2026-09-11, `relay-smoke`: Wren asks Booty for one word, Booty says
 * it -- and the pair acknowledged each other until the cap fired. Six relayed
 * runs for a question with a one-word answer.
 *
 * The brief already said "if nothing more is needed, end with no share block",
 * and every hop got that same sentence whether it was the first or the fifth.
 * The host knows how much budget is left; the teammate did not. Same shape as
 * everything else fixed today: the app holding a fact the reader needed.
 */
/*
 * THE LEAK. MEASURED 2026-09-11, `relay-smoke`: seven automatic runs against a
 * cap of six.
 *
 * `hop` counts the depth of ONE chain, and an exchange is not one chain. A
 * reply held for a busy teammate (0.69.0) carries the hop decided when the
 * message was POSTED, so by the time it starts the exchange may have gone
 * deeper elsewhere -- every decision inside the cap, the total outside it.
 *
 * The budget is counted per exchange now, against the mission a person began,
 * and never below what the chain already proves.
 */
/*
 * A relay notice was a LIVE UPDATE ONLY. "Stopped after 12 automatic replies"
 * reached the window and nowhere else, so a person watching a different
 * conversation when an exchange ended never learned it had -- and reopening
 * the thread later showed nothing at all, so it simply appeared to stop for
 * no reason (MEASURED 2026-09-11, `relay-smoke`: no thread carried it).
 *
 * Endings are written into the mission's own record now. Only endings: "still
 * waiting on Booty" is true for a moment and false after it, and a record of
 * it would be a record of something no longer so.
 */
describe('an ending is written down, not only announced', () => {
  it('keeps the reason an exchange stopped at its budget', async () => {
    const { relay, kept } = harness()
    const capped = sharing({ relay: { hop: MAX_RELAY_HOPS, lastMissionOf: { tm_wren: 'mission_wren1' } } })
    await relay.onShared(capped, [message(BOOTY)])
    expect(kept).toHaveLength(1)
    expect(kept[0]?.message).toContain('Stopped after')
    // Against the mission a person would reopen: the one that shared.
    expect(kept[0]?.missionId).toBe('mission_wren1')
  })

  it('keeps that a teammate finished without writing back', async () => {
    const { relay, kept } = harness()
    await relay.onShared(sharing(), [message(BOOTY)])
    await relay.onRunEnded(ended('mission_1'))
    expect(kept.some((note) => note.message.includes('finished without writing back'))).toBe(true)
  })

  it('keeps nothing for a reply that is merely being waited on', async () => {
    // "Booty is part-way through another mission; their reply starts when it
    // ends" is true for a moment and false after it. A record of it would be
    // a record of something no longer so.
    const { relay, kept, notices } = harness({ startResults: [BUSY] })
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(said(notices).some((line) => line.includes('part-way through'))).toBe(true)
    expect(kept).toHaveLength(0)
  })

  it('says it as well as keeping it: the live update is unchanged', async () => {
    const { relay, kept, notices } = harness()
    const capped = sharing({ relay: { hop: MAX_RELAY_HOPS, lastMissionOf: { tm_wren: 'mission_wren1' } } })
    await relay.onShared(capped, [message(BOOTY)])
    expect(said(notices).some((line) => line.includes('Stopped after'))).toBe(true)
    expect(kept).toHaveLength(1)
  })
})

describe('a budget spent per exchange, not per chain', () => {
  it('refuses once the exchange has spent its budget, whatever this chain says', () => {
    // The leak in one assertion: a hop of 1 -- a fresh-looking chain -- in an
    // exchange that has already started six runs.
    expect(decideRelay({ enabled: true, hop: 1, recipientName: 'Booty', cap: 6, spent: 6 })).toEqual({
      start: false,
      reason: 'Stopped after 6 automatic replies. Booty will see this on their next run.'
    })
  })

  it('allows a deep chain that has not spent the budget', () => {
    // The other direction, and why `hop` alone was never the right number.
    expect(decideRelay({ enabled: true, hop: 2, recipientName: 'Booty', cap: 6, spent: 2 })).toEqual({
      start: true,
      hop: 3
    })
  })

  it('falls back to the chain depth when nothing counted', () => {
    // A record written before the exchange had a root, or a count lost to a
    // restart: `hop` survives in the mission record and is a floor.
    expect(decideRelay({ enabled: true, hop: 6, recipientName: 'Booty', cap: 6 }).start).toBe(false)
    expect(decideRelay({ enabled: true, hop: 0, recipientName: 'Booty', cap: 6 }).start).toBe(true)
  })

  it('counts every relayed run against one exchange, across separate chains', async () => {
    // Two teammates, each answering the same post: two chains, one budget.
    const { relay, starts } = harness()
    const first = sharing({ runId: 'run_a', missionId: 'mission_root' })
    await relay.onShared(first, [message(BOOTY)])
    // A second share from the SAME root, as a held reply or another branch
    // would be: hop 1 again, but the exchange has already spent one.
    await relay.onShared(
      sharing({ runId: 'run_b', missionId: 'mission_b', relay: { hop: 1, rootMissionId: 'mission_root', lastMissionOf: {} } }),
      [message(BOOTY)]
    )
    expect(starts).toHaveLength(2)
    // Everything started under this exchange carries its root forward, so the
    // count cannot be reset by a new branch.
    for (const start of starts) expect(start.relay.rootMissionId).toBe('mission_root')
  })

  it('gives a person-started mission its own exchange', async () => {
    const { relay, starts } = harness()
    await relay.onShared(sharing({ missionId: 'mission_wren1' }), [message(BOOTY)])
    expect(starts[0]?.relay.rootMissionId).toBe('mission_wren1')
  })
})

describe('telling a reply what a reply costs', () => {
  const brief = (hop: number): string =>
    relayPrompt({
      sender: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' },
      recipient: { teammateId: 'tm_booty', name: 'Booty', role: 'Custom' },
      hop,
      cap: 6
    })

  it('says nothing about it on the FIRST hop, where there is work to do', () => {
    // The first message is the one that has a job. Telling it not to reply
    // would be telling it not to do the thing it was started for.
    expect(brief(1)).not.toContain('costs money')
  })

  it('tells a reply that the other end is a model and a reply costs a run', () => {
    // MEASURED, `relay-smoke`, five runs: 6, 6, 3, 6, 7 hops for a question
    // whose answer is one word. "Write back only if that helps finish the
    // work" reads as permission; what it never said is what a reply COSTS.
    expect(brief(2)).toContain('is a MODEL, not a person')
    expect(brief(2)).toContain('costs money')
  })

  it('names the acknowledgements by name, because those are the ones that happen', () => {
    const said = brief(3)
    expect(said).toContain('END HERE with no share block')
    expect(said).toContain('Thanks, receipts and recaps reach nobody')
  })

  it('says what a reply IS, before what it is not', () => {
    // Colin, 2026-09-17: "it feels a little dumbed down when agents speak to
    // each other". The brief was eleven "do not"s across two files; Grok
    // Build's is "what a good message is", with one anti-pattern list at the
    // end. Same facts, and the affirmative sentence comes first.
    const said = brief(2)
    const worth = said.indexOf('A reply is worth that only when it moves the work')
    const nots = said.indexOf('Thanks, receipts and recaps')
    expect(worth).toBeGreaterThan(-1)
    expect(nots).toBeGreaterThan(worth)
    expect(said).toContain('written for a capable colleague who has not seen your turn')
    expect(said).toContain('lead with the answer, said as what is true rather than as a negation, then the evidence by name')
    expect(said).not.toContain('nobody is watching')
    expect(said).not.toContain('Do not thank')
  })
})

describe('telling a teammate where in the budget it is', () => {
  it('names the reply and the budget, plainly, in the middle of an exchange', () => {
    expect(budgetSentence(1, 6)).toBe('This is automatic reply 1 of 6.')
    expect(budgetSentence(3, 6)).toBe('This is automatic reply 3 of 6.')
  })

  it('warns on the one before the last, where the warning can still change something', () => {
    expect(budgetSentence(5, 6)).toContain('One more would be the last')
    expect(budgetSentence(5, 6)).toContain('5 of 6')
  })

  it('says what is actually true on the last: a reply written here reaches nobody', () => {
    // `decideRelay` refuses at the cap, so a share written on the last hop
    // starts nothing -- it waits for a person who may not look for hours, and
    // the teammate has no way to know that unless it is told.
    const last = budgetSentence(6, 6)
    expect(last).toContain('LAST automatic reply')
    expect(last).toContain('waits for a person')
  })

  it('treats anything past the cap as the last, never as a negative count', () => {
    expect(budgetSentence(9, 6)).toContain('LAST automatic reply')
  })

  it('falls back to the default budget when none was given', () => {
    expect(budgetSentence(1, undefined)).toBe(`This is automatic reply 1 of ${String(MAX_RELAY_HOPS)}.`)
  })

  it('handles a budget of one, where the first reply is also the last', () => {
    expect(budgetSentence(1, 1)).toContain('LAST automatic reply')
  })

  it('is in the brief a relayed run is actually started with', () => {
    const brief = relayPrompt({
      sender: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' },
      recipient: { teammateId: 'tm_booty', name: 'Booty', role: 'Custom' },
      hop: 5,
      cap: 6
    })
    expect(brief).toContain('One more would be the last')
  })
})

describe('the reply that lands back in the person\'s conversation', () => {
  it('is briefed as read by the person, and the hop after it is not', async () => {
    // Colin, 2026-09-16: "ask wembley to research aadx ... and get back to
    // you". The whole chain ran; the reply into his conversation was told
    // nobody was watching and said nothing to him. The person-started
    // mission is the first entry in `lastMissionOf`, and that teammate's
    // conversation is the one the person is reading.
    const { relay, starts } = harness()
    await relay.onShared(sharing(), [message(BOOTY)])
    expect(starts).toHaveLength(1)
    const bootyOrigin = starts[0]?.relay
    expect(bootyOrigin?.lastMissionOf).toEqual({ tm_wren: 'mission_wren1' })
    // Booty answers Wren: hop 2, into the conversation Colin started.
    await relay.onShared(
      { runId: 'run_1', missionId: 'mission_1', peer: bootyPeer, relay: bootyOrigin!, runtime: 'claude', sandbox: 'read-only', model: 'sonnet' },
      [message(WREN)]
    )
    expect(starts).toHaveLength(2)
    expect(starts[1]?.prompt).toContain('the person who started this conversation reads it')
    expect(starts[1]?.prompt).not.toContain('nobody is watching this run')
    // Wren writes back to Booty: hop 3, Booty's own conversation, nobody's.
    await relay.onShared(
      { runId: 'run_2', missionId: 'mission_2', peer: wrenPeer, relay: starts[1]!.relay!, runtime: 'cursor', sandbox: 'workspace-write', model: 'composer-2.5' },
      [message(BOOTY)]
    )
    expect(starts).toHaveLength(3)
    expect(starts[2]?.prompt).toContain('reach nobody and cost a run each')
    expect(starts[2]?.prompt).not.toContain('reads it')
  })
})
