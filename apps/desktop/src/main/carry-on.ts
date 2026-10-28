import type { MissionLedger, RecoveredMission } from '@teammate/mission-store'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import { carryOnPrompt, carryOnReason, carryOnRefusal } from '../shared/carry-on.js'
import type { MissionMode } from '../shared/ipc.js'
import type { CarryOnReason, CarryOnResponse, CarryOnRoute, PlanRunsOut } from '../shared/carry-on.js'
import { runtimeDisplayName } from '../shared/runtimes.js'

/**
 * CARRYING THE WORK ON WHEN A PLAN RUNS OUT (0.774, shared/carry-on.ts).
 *
 * Two ways in, one way through. The stopped run's card presses `carry` with the agent it offered; a run's end calls
 * `onRunEnded`, which carries on by itself only when the person picked an agent in Settings AND turned on Without
 * asking, and only for a turn they sent from the chat box -- never a comparison's column, a routine's step, a reply
 * between teammates or a turn Locust carried on itself, so one empty plan cannot start a chain of them.
 *
 * Either way the new turn is a reply on another agent, which the mission service already knows how to do: it
 * reconciles the stopped turn, briefs the new agent with what was done and how it ended, and records the seam
 * (codex-mission.ts, the route switch). This only decides whether to, and says so when it does not.
 *
 * AND A PROVIDER THAT FAILED FIRST (0.839). A turn that stopped on a sign-in, provider or network error before it ran
 * any tool is carried on the same two ways, to the same agent, since the person's pick is who takes over when an
 * agent cannot go on (shared/carry-on.ts, providerFailedFirst).
 */

export interface CarryOnStart {
  readonly prompt: string
  readonly runtime: MissionRuntimeId
  readonly model: string | undefined
  readonly mode: MissionMode
  /** The turn that ran out: the new one continues its conversation. */
  readonly followUpOf: string
  /** The agent whose plan ran out. */
  readonly from: MissionRuntimeId
}

export interface CarryOnOptions {
  readonly ledger: Pick<MissionLedger, 'getMission'>
  /** The person's pick in Settings, read when a run ends. */
  readonly setting: () => Promise<PlanRunsOut | undefined>
  /** Whether a person sent this turn from the chat box: the only turns carried on without a press. */
  readonly sentByPerson: (missionId: string) => boolean
  readonly start: (input: CarryOnStart) => Promise<CarryOnResponse>
  /** A line in the stopped turn's thread, when it was to be carried on without a press and could not be. */
  readonly note: (missionId: string, message: string) => void
}

export interface CarryOn {
  onRunEnded(mission: { readonly missionId: string }): Promise<void>
  /** The card's press: carry the turn that ran out on with `route`. */
  carry(missionId: string, route: CarryOnRoute): Promise<CarryOnResponse>
  /** Whether this turn was carried on, so nothing else says it is stopped and waiting. */
  carried(missionId: string): boolean
}

/**
 * Why a run that stopped can be carried on, if it can: its plan ran out, or its provider failed before it did
 * anything. A run that finished or was stopped by the person is never carried on.
 */
export function stoppedFor(mission: Pick<RecoveredMission, 'phase' | 'events'>): CarryOnReason | undefined {
  if (mission.phase === 'completed' || mission.phase === 'cancelled') return undefined
  return carryOnReason(mission.events)
}

/** Whether a run ended on its account's usage limit: stopped, not finished, and its events say the plan ran out. */
export function endedOnPlanLimit(mission: Pick<RecoveredMission, 'phase' | 'events'>): boolean {
  return stoppedFor(mission) === 'plan'
}

/** The mode the carried-on turn runs in: the stopped turn's own. A turn recorded before modes were is read by its sandbox. */
function modeOf(mission: RecoveredMission): MissionMode {
  return mission.metadata.mode ?? (mission.metadata.sandbox === 'read-only' ? 'ask' : 'accept-edits')
}

/** Turns remembered as carried on; the oldest go first. */
const REMEMBERED = 500

export function createCarryOn(options: CarryOnOptions): CarryOn {
  const carriedIds = new Set<string>()
  const remember = (missionId: string): void => {
    carriedIds.add(missionId)
    while (carriedIds.size > REMEMBERED) {
      const oldest = carriedIds.values().next().value
      if (oldest === undefined) break
      carriedIds.delete(oldest)
    }
  }

  const carry = async (missionId: string, route: CarryOnRoute): Promise<CarryOnResponse> => {
    if (carriedIds.has(missionId)) return { ok: false, message: 'This turn was already carried on.' }
    let recovered: RecoveredMission | undefined
    try {
      recovered = await options.ledger.getMission(missionId)
    } catch {
      recovered = undefined
    }
    if (recovered === undefined) return { ok: false, message: 'This conversation is not in the record any more, so it cannot be carried on.' }
    const reason = stoppedFor(recovered)
    if (reason === undefined) return { ok: false, message: 'This turn did not stop on its plan running out or its provider failing, so there is nothing to carry on.' }
    const from = recovered.metadata.runtime
    if (route.runtime === from) return { ok: false, message: reason === 'plan' ? `${runtimeDisplayName(from)} is the agent whose plan ran out.` : `${runtimeDisplayName(from)} is the agent that just failed.` }
    const mode = modeOf(recovered)
    const refusal = carryOnRefusal(mode, route.runtime)
    if (refusal !== undefined) return { ok: false, message: refusal }
    // Taken before the start, so a second press while the first is starting is refused rather than run twice.
    remember(missionId)
    let response: CarryOnResponse
    try {
      response = await options.start({ prompt: carryOnPrompt(from, reason), runtime: route.runtime, model: route.model, mode, followUpOf: missionId, from })
    } catch {
      response = { ok: false, message: 'The turn could not be started. The work stays where it stopped.' }
    }
    if (!response.ok) carriedIds.delete(missionId)
    return response
  }

  return {
    carry,
    carried: (missionId) => carriedIds.has(missionId),
    async onRunEnded(mission) {
      if (!options.sentByPerson(mission.missionId)) return
      let chosen: PlanRunsOut | undefined
      try {
        chosen = await options.setting()
      } catch {
        chosen = undefined
      }
      if (chosen === undefined || !chosen.automatic) return
      let recovered: RecoveredMission | undefined
      try {
        recovered = await options.ledger.getMission(mission.missionId)
      } catch {
        return
      }
      const reason = recovered === undefined ? undefined : stoppedFor(recovered)
      if (recovered === undefined || reason === undefined) return
      const name = runtimeDisplayName(chosen.runtime)
      if (chosen.runtime === recovered.metadata.runtime) {
        options.note(
          mission.missionId,
          reason === 'plan'
            ? `${name} is the agent picked to carry on when a plan runs out, and its own plan ran out, so nothing was carried on.`
            : `${name} is the agent picked to carry on, and it is the one whose provider failed, so nothing was carried on.`
        )
        return
      }
      const result = await carry(mission.missionId, chosen)
      if (!result.ok) options.note(mission.missionId, `Locust could not carry this on with ${name}: ${result.message}`)
    }
  }
}
