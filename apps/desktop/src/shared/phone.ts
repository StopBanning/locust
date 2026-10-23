import type { MissionRuntimeId, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type { PublicRecoveredMission } from './ipc.js'
import { joinMessageFragments } from './messageFragments.js'
import { unwrapProtocolTags } from './protocolTags.js'
import { runtimeDisplayName } from './runtimes.js'

/**
 * YOUR PHONE (0.775).
 *
 * Every rival and both first-party agent apps let a person look in from their phone; Locust could not. A person away
 * from the desk with a run waiting on a card had to walk back to answer it. So Locust can serve one small page on the
 * home network: scan the QR code in Settings, and the phone shows the conversations, the cards waiting on you
 * (Allow once / Deny), and a box to send the next message -- the same turn the desktop composer would start.
 *
 * Off until the person turns it on. The address carries a key in its #fragment (never sent over the network by the
 * browser, never in a log); every request after that sends the key, and turning it off or Forget this phone makes a
 * new one. The page itself holds nothing until the key is checked.
 */
export const PHONE_SETTINGS_CHANNEL = 'phone:settings'
export const PHONE_SET_ENABLED_CHANNEL = 'phone:set-enabled'
export const PHONE_FORGET_CHANNEL = 'phone:forget'

export interface PhoneSettings {
  readonly enabled: boolean
  /** The address the QR code holds, key included. Present only while it is on. */
  readonly url?: string
  /** Every address this computer has on a home network, for a phone the QR code's one does not reach. */
  readonly addresses: readonly string[]
  /** Why the last change did not happen, in plain words. */
  readonly message?: string
}

/*
 * FROM ANYWHERE (0.826, main/phone-anywhere.ts): the same page, reached through Locust's relay instead of the home
 * network, every message sealed between this computer and the phone.
 */
export const PHONE_ANYWHERE_CHANNEL = 'phone:anywhere'
export const PHONE_SET_MODE_CHANNEL = 'phone:set-mode'
export const PHONE_NEW_CODE_CHANNEL = 'phone:new-code'
export const PHONE_REMOVE_CHANNEL = 'phone:remove'
export const PHONE_ANYWHERE_CHANGED_CHANNEL = 'phone:anywhere-changed'
export const PHONE_NIGHT_CHANNEL = 'phone:night-approvals'

/** How a phone reaches Locust: not at all, from anywhere through the relay, or on this Wi-Fi only. */
export type PhoneMode = 'off' | 'anywhere' | 'wifi'
export const PHONE_MODES: readonly PhoneMode[] = ['off', 'anywhere', 'wifi']

export interface PhoneAnywhereDevice {
  readonly id: string
  /** As the phone named itself: iPhone, iPad, Android phone, Browser. */
  readonly name: string
  readonly added: string
  readonly seen?: string
  readonly online: boolean
  /** Whether this phone turned alerts on (0.827). */
  readonly alerts?: boolean
}

export interface PhoneAnywhereSettings {
  readonly enabled: boolean
  /** Whether this computer is connected to the relay now. */
  readonly connected: boolean
  /** The code's address while one can be scanned: once, until `until`. */
  readonly code?: { readonly url: string; readonly until: string }
  readonly devices: readonly PhoneAnywhereDevice[]
  /** Why the last change did not happen, or why it is not connected, in plain words. */
  readonly message?: string
  /** Whether approvals and questions buzz a phone in quiet hours (0.827); off unless the person turns it on. */
  readonly nightApprovals?: boolean
}

export interface PhoneModeState {
  readonly wifi: PhoneSettings
  readonly anywhere: PhoneAnywhereSettings
}

export interface PhoneApi {
  settings(): Promise<PhoneSettings>
  setEnabled(enabled: boolean): Promise<PhoneSettings>
  /** A new code: every paired phone is unpaired. */
  forget(): Promise<PhoneSettings>
  anywhere(): Promise<PhoneAnywhereSettings>
  /** The way phones reach Locust. Turning one way on turns the other off, and off unpairs every phone of that way. */
  setMode(mode: PhoneMode): Promise<PhoneModeState>
  /** A code for the next phone, from anywhere: the one before it stops working. */
  newCode(): Promise<PhoneAnywhereSettings>
  remove(device: string): Promise<PhoneAnywhereSettings>
  /** Whether approvals and questions buzz a phone in quiet hours. */
  setNightApprovals(on: boolean): Promise<PhoneAnywhereSettings>
  /** Called when a phone pairs, comes and goes, or the connection to the relay changes. */
  onAnywhereChanged(callback: (settings: PhoneAnywhereSettings) => void): () => void
}

export interface PhoneWaiting {
  readonly approvalId: string
  readonly missionId: string
  readonly summary: string
  readonly requestedAt: string
  readonly name?: string
}

export interface PhoneConversation {
  /** The newest turn: a message sent from the phone continues this one. */
  readonly missionId: string
  readonly title: string
  readonly agent: string
  readonly live: boolean
  readonly phase: PublicRecoveredMission['phase'] | 'running'
  readonly at: string
  /** The start of the newest answer, for the list. */
  readonly snippet?: string
}

export interface PhoneTurn {
  readonly missionId: string
  /** What the person asked; absent on a turn nobody typed (a relay, a carry-on, a routine). */
  readonly asked?: string
  readonly answer?: string
  readonly agent: string
  readonly at: string
  readonly phase: PhoneConversation['phase']
}

export interface PhoneState {
  readonly waiting: readonly PhoneWaiting[]
  readonly conversations: readonly PhoneConversation[]
}

export const PHONE_MAX_CONVERSATIONS = 20
export const PHONE_MAX_TURNS = 8
export const PHONE_MAX_ANSWER = 8_000
const SNIPPET = 160

/** The answer as Locust's own MCP tools read it (the last whole message), or the one still arriving on a live turn. */
export function answerOf(events: readonly NormalizedRuntimeEvent[]): string | undefined {
  const messages = joinMessageFragments(events).filter((event) => event.type === 'message.delta')
  const chosen = messages.findLast((event) => event.type === 'message.delta' && event.payload.final) ?? messages.at(-1)
  const answer = chosen?.type === 'message.delta' ? unwrapProtocolTags(chosen.payload.text).trim() : undefined
  if (answer === undefined || answer.length === 0) return undefined
  return answer.length > PHONE_MAX_ANSWER ? `${answer.slice(0, PHONE_MAX_ANSWER)}…` : answer
}

/** The person's own words for a turn, or nothing when the host wrote them. */
function askedOf(mission: PublicRecoveredMission): string | undefined {
  // A route switch's prompt is the machine-written brief; a follow-up's is what the person typed next.
  if (mission.startedBy !== undefined && mission.startedBy.kind !== 'terminal') return undefined
  if (mission.continuesFrom?.reason === 'route-switch') return undefined
  return mission.prompt
}

/** Who answered: the teammate's name, as the window says it; the agent's name for a turn of nobody's. */
export type PhoneNames = (missionId: string) => string | undefined
const whoOf = (mission: PublicRecoveredMission, names: PhoneNames | undefined): string =>
  names?.(mission.missionId) ?? runtimeDisplayName(mission.runtime as MissionRuntimeId)

/**
 * The conversations a phone lists: chains of turns, newest first, each named by its first turn's words. A side
 * question is not a turn of any conversation, so it is never listed (as the sidebar does not list it).
 */
export function phoneConversations(
  missions: readonly PublicRecoveredMission[],
  liveMissionIds: readonly string[],
  names?: PhoneNames
): { readonly list: readonly PhoneConversation[]; readonly chains: ReadonlyMap<string, readonly PublicRecoveredMission[]> } {
  const listed = missions.filter((mission) => mission.startedBy?.kind !== 'side')
  const byId = new Map(listed.map((mission) => [mission.missionId, mission]))
  const rootOf = (mission: PublicRecoveredMission): PublicRecoveredMission => {
    const seen = new Set<string>()
    let at = mission
    for (;;) {
      seen.add(at.missionId)
      const before = at.continuesFrom === undefined ? undefined : byId.get(at.continuesFrom.missionId)
      if (before === undefined || seen.has(before.missionId)) return at
      at = before
    }
  }
  const chains = new Map<string, PublicRecoveredMission[]>()
  for (const mission of listed) {
    const root = rootOf(mission).missionId
    chains.set(root, [...(chains.get(root) ?? []), mission])
  }
  const live = new Set(liveMissionIds)
  const list: PhoneConversation[] = []
  for (const [rootId, turns] of chains) {
    turns.sort((a, b) => a.createdAt.localeCompare(b.createdAt))
    const root = byId.get(rootId)!
    const newest = turns.at(-1)!
    const answer = answerOf(newest.events)
    const title = askedOf(root) ?? turns.map(askedOf).find((asked) => asked !== undefined) ?? 'A conversation Locust started'
    list.push({
      missionId: newest.missionId,
      title: oneLine(title, 120),
      agent: whoOf(newest, names),
      live: live.has(newest.missionId),
      phase: live.has(newest.missionId) ? 'running' : newest.phase,
      at: newest.lastUpdatedAt,
      ...(answer === undefined ? {} : { snippet: oneLine(answer, SNIPPET) })
    })
  }
  list.sort((a, b) => b.at.localeCompare(a.at))
  return { list: list.slice(0, PHONE_MAX_CONVERSATIONS), chains }
}

/** One conversation's newest turns, oldest first, from any of its turns. */
export function phoneTurns(
  chains: ReadonlyMap<string, readonly PublicRecoveredMission[]>,
  missionId: string,
  liveMissionIds: readonly string[],
  names?: PhoneNames
): readonly PhoneTurn[] | undefined {
  const chain = [...chains.values()].find((turns) => turns.some((turn) => turn.missionId === missionId))
  if (chain === undefined) return undefined
  const live = new Set(liveMissionIds)
  return chain.slice(-PHONE_MAX_TURNS).map((turn) => {
    const asked = askedOf(turn)
    const answer = answerOf(turn.events)
    return {
      missionId: turn.missionId,
      ...(asked === undefined ? {} : { asked }),
      ...(answer === undefined ? {} : { answer }),
      agent: whoOf(turn, names),
      at: turn.createdAt,
      phase: live.has(turn.missionId) ? 'running' : turn.phase
    }
  })
}

function oneLine(text: string, max: number): string {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length > max ? `${flat.slice(0, max - 1)}…` : flat
}
