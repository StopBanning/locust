import type { MissionLedger } from '@teammate/mission-store'

import type { PublicTeammate } from '../shared/ipc.js'
import { parseDecision } from '../shared/decision.js'
import { createTranscriptTracker } from './peer-exchange.js'

/**
 * When a run ends, notice the two ways it can end WAITING FOR THE PERSON
 * that no notification covered: a decision card (the teammate asked a
 * question and stopped) and an account limit (the run stopped at a
 * checkpoint until a person picks where it continues). Approvals already
 * toast; rooms already toast; these were the parity map's remaining gap
 * (row 62) and needed no new screen -- the same attention path, two more
 * reasons.
 *
 * Decided from the record: the reply is rebuilt from the ledger and read
 * for an ask block; the limit is the event the adapter recorded. Nothing
 * is said for a run that ended any other way -- a person who stepped away
 * is told what needs them, not everything that happened.
 */
export interface AttentionReaderOptions {
  readonly ledger: Pick<MissionLedger, 'getMission'>
  readonly teammates: {
    list(): Promise<readonly PublicTeammate[]>
    missionOwners(): Promise<Readonly<Record<string, string>>>
  }
  readonly conversationTitle?: (missionId: string) => Promise<string>
  readonly attention: {
    decisionAsked(teammateName: string | undefined, question: string, missionId?: string, conversationTitle?: string): boolean
    limitHit(teammateName: string | undefined, runtime: string, message: string | undefined): boolean
  }
}

export interface AttentionReader {
  onRunEnded(mission: { readonly missionId: string }): Promise<void>
}

export function createAttentionReader(options: AttentionReaderOptions): AttentionReader {
  return {
    async onRunEnded(mission) {
      let recovered
      try {
        recovered = await options.ledger.getMission(mission.missionId)
      } catch {
        return
      }
      if (recovered === undefined) return

      let name: string | undefined
      try {
        const owner = (await options.teammates.missionOwners())[mission.missionId]
        if (owner !== undefined) name = (await options.teammates.list()).find((entry) => entry.teammateId === owner)?.name
      } catch {
        // Unnamed is said as "a teammate", never guessed.
      }

      // A limit: the newest such event, when the run did not go on to complete after it.
      let limit: { readonly runtime: string; readonly message: string | undefined } | undefined
      for (const event of recovered.events) {
        if (event.type === 'route.limit_detected' && event.payload.kind === 'quota-exhausted') {
          limit = { runtime: event.sourceAdapter, message: event.payload.message }
        } else if (event.type === 'run.completed') {
          limit = undefined
        }
      }
      if (limit !== undefined) {
        options.attention.limitHit(name, limit.runtime, limit.message)
        return
      }

      if (recovered.phase !== 'completed') return
      const tracker = createTranscriptTracker()
      tracker.track(recovered.events)
      const text = tracker.latestFinal
      if (text === undefined) return
      const decision = parseDecision(text)
      if (decision === undefined) return
      options.attention.decisionAsked(name, decision.question, mission.missionId, await options.conversationTitle?.(mission.missionId) ?? recovered.metadata?.prompt?.replace(/\s+/g, ' ').trim().slice(0, 100) ?? 'Conversation')
    }
  }
}
