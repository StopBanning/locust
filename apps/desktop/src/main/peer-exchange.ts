import type { MissionLedger, Workroom, WorkroomMessage } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { splitAttachments } from '../shared/attachments.js'
import type { CodexMissionUpdate, PublicPeerMessage } from '../shared/ipc.js'
import { boundedShareText, MAX_SHARES_PER_MISSION, parseShareBlocks } from '../shared/peer-share.js'
import { composeRuntimePrompt, composeSoloPrompt, MAX_INBOUND_MESSAGES, runtimeKeepsATodoList, runtimeNarratesWhenAsked } from './workroom-briefing.js'
import type { MissionPeerContext } from './workroom-briefing.js'

/**
 * The two moments a mission touches the workroom, kept apart from the mission
 * transports so every transport does them the same way.
 *
 * BEFORE the run: the teammate's waiting messages are quoted into the prompt
 * and the mission's ledger records which ones, by id. The channel marks them
 * delivered only once the run is actually live, so a start that fails never
 * consumes a message.
 *
 * AFTER a run that completed on its own terms: the final transcript is read
 * for share blocks, each is checked against the roster and bounded, and each
 * becomes one attributed workroom message plus one ledger link. A cancelled
 * or failed run shares nothing -- a half-finished claim is exactly the kind
 * of thing that misleads a colleague.
 */

const MAX_TRACKED_TEXT = 32_000

export interface PreparedPeerPrompt {
  /** What the runtime is sent. */
  readonly runtimePrompt: string
  /** What the prompt quotes, oldest first. */
  readonly delivered: readonly WorkroomMessage[]
  /** True when the channel could not be read; the run proceeds without it. */
  readonly failed: boolean
  /** What the session has been told once this is sent, with `alreadyGiven` (A2.5). */
  readonly given: readonly string[]
}

/** A run with no teammate's brief, and what its session has been told (A2.5). */
export interface PreparedSoloPrompt {
  readonly runtimePrompt: string
  readonly given: readonly string[]
}

export interface TranscriptTracker {
  track(events: readonly NormalizedRuntimeEvent[]): void
  /** The last assistant message the runtime marked final, rebuilt from deltas. */
  readonly latestFinal: string | undefined
  /** True once a `run.completed` has passed through. */
  readonly completed: boolean
}

export interface PeerExchange {
  /**
   * `runtime` decides whether the todo-list request is included: the setting
   * is the person's answer, but the runtime is what makes it answerable. See
   * `RUNTIMES_THAT_KEEP_A_TODO_LIST`.
   */
  prepare(prompt: string, peer: MissionPeerContext, runtime?: string, conversation?: ConversationHint): Promise<PreparedPeerPrompt>
  /**
   * The briefing for a run with no teammate: the folder and the project's
   * memory, and none of what needs a roster. Never throws -- a memory that
   * cannot be read is left out, the same as everywhere else.
   */
  briefSolo(prompt: string, runtime?: string, conversation?: ConversationHint): Promise<PreparedSoloPrompt>
  /** Throws when the ledger refuses: a mission must not run on messages it cannot record. */
  recordReceived(missionId: string, delivered: readonly WorkroomMessage[], occurredAt: string): Promise<void>
  /** Never throws: a delivery that cannot be marked is shown again next time, which is the safe direction. */
  markDelivered(missionId: string, delivered: readonly WorkroomMessage[]): Promise<void>
  /** Resolves to the messages actually posted, so a relay can act on them. */
  share(input: {
    readonly runId: string
    readonly missionId: string
    readonly peer: MissionPeerContext
    readonly text: string
    /** What the host saw this run change (A2.17), attached to every message it sends. */
    readonly observed?: readonly string[]
  }, report: (update: CodexMissionUpdate) => void): Promise<readonly PostedShare[]>
}

/**
 * A message as it was just posted, with what the SENDER asked for.
 *
 * `urgent` is deliberately not in the durable record. It is a fact about this
 * moment -- "take this before you finish what you are doing" -- and a message
 * still waiting after a restart is one whose moment has passed; reviving the
 * interruption then would stop work nobody is waiting on.
 */
export type PostedShare = WorkroomMessage & { readonly urgent: boolean }

export function publicPeerMessage(message: WorkroomMessage, direction: 'received' | 'posted'): PublicPeerMessage {
  return {
    messageId: message.messageId,
    direction,
    from: { teammateId: message.from.teammateId, name: message.from.name, missionId: message.from.missionId },
    to: { teammateId: message.to.teammateId, name: message.to.name },
    text: message.text,
    at: message.postedAt
  }
}

/**
 * Rebuild assistant text the way the transcript did -- deltas carry an append
 * or replace against a per-item buffer -- keeping the TAIL of an oversized
 * message, because the share form puts its blocks at the end.
 */
export function createTranscriptTracker(): TranscriptTracker {
  const buffers = new Map<string, string>()
  let latestFinal: string | undefined
  let completed = false
  return {
    track(events) {
      for (const event of events) {
        if (event.type === 'message.delta') {
          const { itemId, operation, text, final } = event.payload
          let next = operation === 'replace' ? text : `${buffers.get(itemId) ?? ''}${text}`
          if (next.length > MAX_TRACKED_TEXT) next = next.slice(-MAX_TRACKED_TEXT)
          buffers.set(itemId, next)
          if (final) latestFinal = next
        } else if (event.type === 'run.completed') {
          completed = true
        }
      }
    },
    get latestFinal() {
      return latestFinal
    },
    get completed() {
      return completed
    }
  }
}

/**
 * Which conversation a turn belongs to, as the host knows it at start: the
 * turn it continues from. Absent for the first turn of a new conversation.
 * The briefing slot walks back from here to find the group, if any.
 */
export interface ConversationHint {
  /** Compare's per-run replacement, never saved on the teammate. Empty withdraws their standing words. */
  readonly instructions?: string
  readonly previousMissionId?: string
  /** The folder the run stands in: its conversation's, which may not be the window's (0.458). */
  readonly folder?: string
  /**
   * The run's OWN folder, when it stands apart from the project: a comparison
   * column's copy (0.638). Told so, as a worktree run is -- not the project
   * folder's name, which a model in a copy took as a folder to make (an arena
   * round's OpenCode columns wrote `<copy>/<project name>/index.html`).
   */
  readonly ownFolder?: string
  /**
   * The paragraphs of the brief the resumed CLI session already holds, by
   * key (A2.5). Absent for a turn that must be briefed in full.
   */
  readonly alreadyGiven?: ReadonlySet<string>
  /**
   * The messages this run was started to answer (A2.12): shown first, and
   * the last to give way when the prompt is long.
   */
  readonly startedFor?: readonly string[]
  /** A2.9: the overlap note for this teammate in this folder, when there is one. */
  readonly overlap?: string
}

/** What the team remembers, worded for a runtime; undefined when memory is off. */
export interface MemoryBriefing {
  /**
   * `peer` is absent for a run that belongs to nobody; the folder and the
   * memory are still the project's. `prompt` is what was asked (`askedIn`),
   * so the memories that bear on it are the ones pasted. `runtime` is the
   * run's, so LOCUST.md's sections for other runtimes are left out (A4.2).
   */
  section(peer: MissionPeerContext | undefined, conversation?: ConversationHint, prompt?: string, runtime?: string): Promise<string | undefined>
}

/**
 * WHAT WAS ASKED, for the memory search -- not the host's words around it.
 *
 * The team's memory pastes the notes that share words with the prompt
 * (shared/memory.ts). A relayed run's prompt is nothing but the relay's
 * rules -- what a reply costs, when to write back -- with the message it
 * answers quoted beside it, and on Colin's store (2026-09-26) eight of the
 * nine relayed turns from Yurt pasted the same six notes, chosen by the
 * rules' own words. It is searched by the messages it was started to answer.
 * A message with files opens with the host's line naming them, which is left
 * out.
 *
 * A handoff's brief is searched whole, measured: the previous agent's account
 * of the work is ABOUT the work. Reading only the person's words out of it
 * found one of the six notes that bore on Colin's handed-over orb test, and
 * the whole brief found five.
 */
export function askedIn(prompt: string, answering?: readonly WorkroomMessage[]): string {
  if (answering !== undefined) return answering.map((message) => message.text).join('\n\n')
  return splitAttachments(prompt).text
}

/**
 * Who a share block meant, read the way a person would read it.
 *
 * A model writes `to=` from the roster the briefing printed, and the roster
 * prints `Name (Role)`. It has taken the role half twice now, so the role is
 * a way in -- but only when exactly one teammate answers to it. Several
 * teammates can be `Custom`, and a message delivered to whichever of them
 * happened to be first would be worse than one refused out loud.
 *
 * `'self'` is its own answer rather than `undefined`: addressing yourself is
 * not a roster you should go and fix.
 */
export function recipientOf(
  to: string,
  peer: MissionPeerContext
): MissionPeerContext['others'][number] | 'self' | undefined {
  const wanted = to.trim().toLowerCase()
  if (wanted.length === 0) return undefined
  const byName = peer.others.filter((entry) => entry.name.toLowerCase() === wanted)
  if (byName.length === 1) return byName[0]
  if (peer.self.name.toLowerCase() === wanted) return 'self'
  // Ambiguous by name is not resolved by role: two teammates really do share
  // that name, and the app cannot pick one for the person.
  if (byName.length > 1) return undefined
  const byRole = peer.others.filter((entry) => entry.role.toLowerCase() === wanted)
  if (byRole.length === 1) return byRole[0]
  if (byRole.length === 0 && peer.self.role.toLowerCase() === wanted) return 'self'
  return undefined
}

export function createPeerExchange(options: {
  readonly workroom: Workroom
  readonly ledger: MissionLedger
  readonly memory?: MemoryBriefing
  /** The person's opt-in, read when a mission starts rather than cached. */
  readonly keepATodoList?: () => Promise<boolean>
  /**
   * The connectors this machine's Cursor CLI can actually call, by name.
   *
   * Cursor only: the others either have no connector story here or read their
   * own. Absent, or throwing, means the brief simply does not mention any.
   */
  readonly readyConnectors?: () => Promise<string | undefined>
}): PeerExchange {
  return {
    async prepare(prompt, peer, runtime, conversation) {
      // What is waiting, read before memory: for a run the host started to
      // answer a teammate, those messages are what was asked (`askedIn`).
      // Reading marks nothing delivered, so the order changes nothing else.
      const unread = await options.workroom
        .unread(peer.self.teammateId, MAX_INBOUND_MESSAGES, conversation?.startedFor)
        .catch(() => undefined)
      const startedFor = conversation?.startedFor
      const asked =
        startedFor === undefined
          ? askedIn(prompt)
          : askedIn(prompt, (unread?.messages ?? []).filter((message) => startedFor.includes(message.messageId)))
      // Memory that cannot be read is left out, never a refusal to run.
      const memory = options.memory === undefined ? undefined : await options.memory.section(peer, conversation, asked, runtime).catch(() => undefined)
      /*
       * Both halves must be true, and the runtime half is not negotiable.
       * A setting that is on does not make Claude Code able to keep a list;
       * it has no such tool, and asking would produce an empty board with no
       * explanation. A setting that cannot be read reads as off.
       */
      const todos =
        runtime !== undefined &&
        runtimeKeepsATodoList(runtime) &&
        (await options.keepATodoList?.().catch(() => false)) === true
      /*
       * Which connectors it can call, by NAME.
       *
       * Colin had to tell a teammate to "try rh local" -- because the obvious
       * name on his machine was a broken entry, so the teammate looked at
       * that one, read `needsAuth, 0 tools`, and reported the connector dead
       * while a working one sat beside it. A person should not have to know
       * what a server is called in a config file.
       */
      const connectors =
        runtime === 'cursor' ? await options.readyConnectors?.().catch(() => undefined) : undefined
      try {
        if (unread === undefined) throw new Error('The workroom could not be read.')
        const composed = composeRuntimePrompt({
          prompt,
          peer,
          inbound: unread.messages,
          remaining: unread.remaining,
          ...(memory === undefined ? {} : { memory }),
          ...(connectors === undefined ? {} : { connectors }),
          keepATodoList: todos,
          narrate: runtime !== undefined && runtimeNarratesWhenAsked(runtime),
          ...(conversation?.alreadyGiven === undefined ? {} : { alreadyGiven: conversation.alreadyGiven }),
          ...(conversation?.overlap === undefined ? {} : { overlap: conversation.overlap }),
          now: new Date()
        })
        return { runtimePrompt: composed.prompt, delivered: composed.delivered, failed: false, given: composed.given }
      } catch {
        // The roster trailer still goes: the run can share even when it could
        // not be shown what was waiting.
        const composed = composeRuntimePrompt({
          prompt,
          peer,
          inbound: [],
          remaining: 0,
          ...(memory === undefined ? {} : { memory }),
          ...(connectors === undefined ? {} : { connectors }),
          keepATodoList: todos,
          narrate: runtime !== undefined && runtimeNarratesWhenAsked(runtime),
          ...(conversation?.alreadyGiven === undefined ? {} : { alreadyGiven: conversation.alreadyGiven }),
          ...(conversation?.overlap === undefined ? {} : { overlap: conversation.overlap })
        })
        return { runtimePrompt: composed.prompt, delivered: [], failed: true, given: composed.given }
      }
    },

    async briefSolo(prompt, runtime, conversation) {
      const memory = options.memory === undefined ? undefined : await options.memory.section(undefined, conversation, askedIn(prompt), runtime).catch(() => undefined)
      const todos =
        runtime !== undefined &&
        runtimeKeepsATodoList(runtime) &&
        (await options.keepATodoList?.().catch(() => false)) === true
      const connectors =
        runtime === 'cursor' ? await options.readyConnectors?.().catch(() => undefined) : undefined
      const composed = composeSoloPrompt({
        prompt,
        ...(memory === undefined ? {} : { memory }),
        ...(connectors === undefined ? {} : { connectors }),
        keepATodoList: todos,
        narrate: runtime !== undefined && runtimeNarratesWhenAsked(runtime),
        ...(conversation?.alreadyGiven === undefined ? {} : { alreadyGiven: conversation.alreadyGiven })
      })
      return { runtimePrompt: composed.prompt, given: composed.given }
    },

    async recordReceived(missionId, delivered, occurredAt) {
      if (delivered.length === 0) return
      await options.ledger.appendPeerLinks(
        missionId,
        delivered.map((message) => ({
          direction: 'received' as const,
          messageId: message.messageId,
          peerTeammateId: message.from.teammateId,
          occurredAt
        }))
      )
    },

    async markDelivered(missionId, delivered) {
      if (delivered.length === 0) return
      try {
        await options.workroom.markDelivered(delivered.map((message) => message.messageId), missionId)
      } catch {
        // Shown again next time. A repeated claim is recoverable; a lost one is not.
      }
    },

    async share(input, report) {
      const { peer } = input
      const failed = (message: string): void => {
        report({ kind: 'peer-share-failed', runId: input.runId, missionId: input.missionId, message })
      }
      const posted: PostedShare[] = []
      const blocks = parseShareBlocks(input.text)
      if (blocks.length > MAX_SHARES_PER_MISSION) {
        failed(
          `${peer.self.name} tried to share ${blocks.length} messages; only the first ${MAX_SHARES_PER_MISSION} were sent.`
        )
      }
      for (const block of blocks.slice(0, MAX_SHARES_PER_MISSION)) {
        const target = recipientOf(block.to, peer)
        if (target === 'self') {
          /*
           * It addressed ITSELF, and by its own role.
           *
           * The briefing prints the roster as `Jimothy (Finance Bro)`, and
           * on 2026-09-09 Jimothy wrote `to="Finance Bro"` -- his own role
           * label, not another teammate. The old matcher looked at names
           * only, found nothing, and reported "who is not on the roster",
           * which reads as a roster problem the person could go and fix.
           * There was nothing to fix: the message had nowhere to go because
           * it was addressed home.
           *
           * Third variation of one defect. The first two -- single quotes,
           * and the printed `(role)` suffix -- are handled in the parser and
           * documented there. All three are the app failing to read
           * something unambiguous.
           */
          /*
           * Said to nobody, because nobody has to do anything about it.
           *
           * This drew an amber line -- `Jimothy addressed a message to
           * "Finance Bro", which is Jimothy's own role rather than another
           * teammate. Nothing was sent.` -- and Colin, 2026-09-14: "these two
           * yellow texts are both unneccessary".
           *
           * Amber in this app means A PERSON MAY NEED TO ACT, and this fails
           * that test on its own terms: a teammate addressed ITSELF, so no
           * message was lost, nobody is waiting on one, and there is nothing
           * a person could usefully do. It is a model talking to itself, which
           * is a curiosity rather than an event.
           *
           * The roster miss below is NOT this. There, a teammate meant to
           * reach someone and the name was invented or stale -- something a
           * person thought was delivered was not, and to a real intended
           * recipient. That one still speaks.
           */
          continue
        }
        if (target === undefined) {
          // Said out loud: a runtime that addressed someone who is not here
          // was probably working from a stale or invented name, and the
          // person should know that rather than wonder why nothing arrived.
          failed(`${peer.self.name} addressed a message to "${block.to}", who is not on the roster. Nothing was sent.`)
          continue
        }
        const text = boundedShareText(block.text)
        if (text.length === 0) continue
        let message: WorkroomMessage
        try {
          message = await options.workroom.post({
            from: { teammateId: peer.self.teammateId, name: peer.self.name, missionId: input.missionId },
            to: { teammateId: target.teammateId, name: target.name },
            text,
            ...(input.observed === undefined ? {} : { observed: input.observed })
          })
        } catch {
          failed(`A message from ${peer.self.name} to ${target.name} could not be written to the workroom. Nothing was sent.`)
          continue
        }
        // The same block twice in one reply is one message (A2.2): the
        // workroom answers the repeat with the first, and it is recorded,
        // shown and relayed once.
        if (posted.some((held) => held.messageId === message.messageId)) continue
        try {
          await options.ledger.appendPeerLinks(input.missionId, [{
            direction: 'posted',
            messageId: message.messageId,
            peerTeammateId: target.teammateId,
            occurredAt: message.postedAt
          }])
        } catch {
          // The message exists and will reach its recipient; only this
          // mission's own record of having sent it is missing. Both facts
          // are reported: the message below, the gap here.
          failed(`${target.name} will receive ${peer.self.name}'s message, but this mission's ledger could not record that it was sent.`)
        }
        report({
          kind: 'peer-message',
          runId: input.runId,
          missionId: input.missionId,
          message: publicPeerMessage(message, 'posted')
        })
        posted.push({
          ...message,
          urgent: block.urgent,
          ...(block.defer === true ? { defer: true } : {}),
          ...(block.wantsAnswer === true ? { wantsAnswer: true } : {})
        })
      }
      return posted
    }
  }
}

/**
 * A mission that could not record the messages it was shown. Raised before
 * anything runs on them: a mission must not run on messages its own record
 * cannot name.
 */
export class PeerRecordError extends Error {}
