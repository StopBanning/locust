import type { WorkroomMessage } from '@teammate/mission-store'
import { describe, expect, it } from 'vitest'

import type { CodexMissionStartResponse } from '../shared/ipc.js'
import { createRelay } from './relay.js'
import type { RelayOptions, RelayedMessage, SharingMission, TurnRoute } from './relay.js'
import type { MissionPeerContext } from './workroom-briefing.js'

/*
 * AN AUTOMATIC REPLY STAYS ON THE MODEL PICKED IN ITS CONVERSATION (0.854).
 *
 * The first-session drive, 2026-11-01: Sable's free model ran out, the person
 * pressed "Stop and switch to Nemotron 3.5 Lightning Free", and Sable's turn
 * on Nemotron asked Penny. Penny's answer came back and started Sable on the
 * route saved on Sable's profile -- the DeepSeek that had just run out -- so
 * the conversation stalled again, offering the same switch. A conversation
 * keeps the model picked in it (0.552); a run that continues a teammate's own
 * turn in an exchange now starts on the route that turn ran on.
 */

const SABLE = { teammateId: 'tm_sable', name: 'Sable', role: 'Research & Briefs' }
const PENNY = { teammateId: 'tm_penny', name: 'Penny', role: 'Money' }
const ROOK = { teammateId: 'tm_rook', name: 'Rook', role: 'Chief of Staff' }
const DEEPSEEK = 'opencode/deepseek-v4.1-flash-free'
const NEMOTRON = 'opencode/nemotron-3.5-lightning-free'
const sablePeer: MissionPeerContext = { self: { ...SABLE, route: { runtime: 'opencode', model: DEEPSEEK, mode: 'accept-edits', effort: 'high' } }, others: [PENNY, ROOK] }
const pennyPeer: MissionPeerContext = { self: { ...PENNY, route: { runtime: 'claude', model: 'sonnet', mode: 'ask' } }, others: [SABLE, ROOK] }
const rookPeer: MissionPeerContext = { self: { ...ROOK, route: { runtime: 'codex', model: 'gpt-5.5', mode: 'ask' } }, others: [SABLE, PENNY] }

/** Sable's turn after the switch: on Nemotron, which the profile never saved. */
const SWITCHED: TurnRoute = { runtime: 'opencode', model: NEMOTRON, mode: 'accept-edits' }

const sableTurn: SharingMission = {
  runId: 'run_sable2',
  missionId: 'mission_sable2',
  runtime: 'opencode',
  sandbox: 'workspace-write',
  model: NEMOTRON,
  peer: sablePeer,
  relay: undefined
}
const message = (from: { teammateId: string; name: string; missionId: string }, to: { teammateId: string; name: string }, text: string, extra: Partial<RelayedMessage> = {}): RelayedMessage => ({
  messageId: `msg_${from.teammateId}`,
  sequence: 1,
  from,
  to,
  text,
  postedAt: '2026-11-01T17:40:00.000Z',
  ...extra
}) as WorkroomMessage & RelayedMessage
const toPenny = message({ teammateId: SABLE.teammateId, name: SABLE.name, missionId: 'mission_sable2' }, PENNY, 'What do the savings rates look like this month?', { wantsAnswer: true })

function harness(routeOf?: RelayOptions['routeOf']) {
  const starts: Parameters<RelayOptions['start']>[0][] = []
  const relay = createRelay({
    enabled: async () => true,
    peerContextFor: async (id) => [sablePeer, pennyPeer, rookPeer].find((peer) => peer.self.teammateId === id),
    finalReplyOf: async () => 'A high-yield savings account pays about 4.4% this month.',
    post: async () => ({ messageId: 'wm_returned' }),
    ...(routeOf === undefined ? {} : { routeOf }),
    start: async (request) => {
      starts.push(request)
      return {
        ok: true,
        data: { runId: `run_${String(starts.length)}`, missionId: `mission_${String(starts.length)}`, runtime: request.runtime, model: request.model ?? 'account-default', resolvedRouteId: 'x', cliVersion: null, sandbox: 'read-only', peerMessages: [], peerDeliveryFailed: false }
      } as CodexMissionStartResponse
    },
    assignOwner: async () => undefined,
    notify: () => undefined
  })
  return { relay, starts }
}

const known = (routes: Readonly<Record<string, TurnRoute>>): RelayOptions['routeOf'] => async (missionId) => routes[missionId]
const routeOfStart = (start: Parameters<RelayOptions['start']>[0] | undefined) =>
  start === undefined ? undefined : { runtime: start.runtime, model: start.model, mode: start.mode, effort: start.effort, account: start.account }

describe('an automatic run that continues a teammate in their own conversation', () => {
  it('the answer brought back starts the asker on the model picked there, not the one on their profile', async () => {
    const h = harness(known({ mission_sable2: SWITCHED }))
    await h.relay.onShared(sableTurn, [toPenny])
    await h.relay.onRunEnded({ missionId: 'mission_1', peer: pennyPeer, relay: h.starts[0]!.relay })
    expect(h.starts.map((start) => start.peer.self.name)).toEqual(['Penny', 'Sable'])
    expect(h.starts[1]?.followUpOf).toBe('mission_sable2')
    // DeepSeek's level stays with DeepSeek: Nemotron runs at its own.
    expect(routeOfStart(h.starts[1])).toEqual({ runtime: 'opencode', model: NEMOTRON, mode: 'accept-edits', effort: undefined, account: undefined })
  })

  it('so does a reply written back', async () => {
    const h = harness(known({ mission_sable2: SWITCHED }))
    await h.relay.onShared(sableTurn, [toPenny])
    const pennyTurn: SharingMission = { runId: 'run_1', missionId: 'mission_1', runtime: 'claude', sandbox: 'read-only', model: 'sonnet', peer: pennyPeer, relay: h.starts[0]!.relay }
    await h.relay.onShared(pennyTurn, [message({ teammateId: PENNY.teammateId, name: PENNY.name, missionId: 'mission_1' }, SABLE, 'About 4.4% in a high-yield savings account.')])
    expect(h.starts.map((start) => start.peer.self.name)).toEqual(['Penny', 'Sable'])
    expect(h.starts[1]?.followUpOf).toBe('mission_sable2')
    expect(routeOfStart(h.starts[1])?.model).toBe(NEMOTRON)
  })

  it('and so does the turn after a meeting, when several were asked at once', async () => {
    const h = harness(known({ mission_sable2: SWITCHED }))
    const toRook = { ...message({ teammateId: SABLE.teammateId, name: SABLE.name, missionId: 'mission_sable2' }, ROOK, 'What should the person ask their bank?'), messageId: 'msg_to_rook' }
    await h.relay.onShared(sableTurn, [toPenny, toRook])
    expect(h.starts.map((start) => start.peer.self.name)).toEqual(['Penny', 'Rook'])
    for (const [index, peer] of [pennyPeer, rookPeer].entries()) {
      const turn: SharingMission = { runId: `run_${String(index + 1)}`, missionId: `mission_${String(index + 1)}`, runtime: 'claude', sandbox: 'read-only', model: 'sonnet', peer, relay: h.starts[index]!.relay }
      await h.relay.onShared(turn, [message({ teammateId: peer.self.teammateId, name: peer.self.name, missionId: turn.missionId }, SABLE, 'Here is what I found.')])
    }
    expect(h.starts.map((start) => start.peer.self.name)).toEqual(['Penny', 'Rook', 'Sable'])
    expect(h.starts[2]?.prompt).toContain('Penny and Rook replied to your message')
    expect(h.starts[2]?.followUpOf).toBe('mission_sable2')
    expect(routeOfStart(h.starts[2])?.model).toBe(NEMOTRON)
  })

  it('its level and account go with it', async () => {
    const h = harness(known({ mission_sable2: { runtime: 'claude', model: 'opus', effort: 'low', account: 'work', mode: 'ask' } }))
    await h.relay.onShared(sableTurn, [toPenny])
    await h.relay.onRunEnded({ missionId: 'mission_1', peer: pennyPeer, relay: h.starts[0]!.relay })
    expect(routeOfStart(h.starts[1])).toEqual({ runtime: 'claude', model: 'opus', mode: 'ask', effort: 'low', account: 'work' })
  })

  it('a turn on the account default starts on the account default', async () => {
    const h = harness(known({ mission_sable2: { runtime: 'codex', mode: 'accept-edits' } }))
    await h.relay.onShared(sableTurn, [toPenny])
    await h.relay.onRunEnded({ missionId: 'mission_1', peer: pennyPeer, relay: h.starts[0]!.relay })
    expect(routeOfStart(h.starts[1])).toEqual({ runtime: 'codex', model: undefined, mode: 'accept-edits', effort: undefined, account: undefined })
  })
})

describe('everywhere else, as before', () => {
  it('a teammate with no turn in the exchange answers on their own saved model', async () => {
    const asked: string[] = []
    const h = harness(async (missionId) => {
      asked.push(missionId)
      return SWITCHED
    })
    await h.relay.onShared(sableTurn, [toPenny])
    expect(routeOfStart(h.starts[0])).toEqual({ runtime: 'claude', model: 'sonnet', mode: 'ask', effort: undefined, account: undefined })
    expect(asked).toEqual([])
  })

  it('a turn whose route is not known, or cannot be read, continues on the saved one', async () => {
    for (const routeOf of [known({}), async () => { throw new Error('the ledger is not readable') }, undefined] as const) {
      const h = harness(routeOf)
      await h.relay.onShared(sableTurn, [toPenny])
      await h.relay.onRunEnded({ missionId: 'mission_1', peer: pennyPeer, relay: h.starts[0]!.relay })
      expect(routeOfStart(h.starts[1])).toEqual({ runtime: 'opencode', model: DEEPSEEK, mode: 'accept-edits', effort: 'high', account: undefined })
    }
  })
})
