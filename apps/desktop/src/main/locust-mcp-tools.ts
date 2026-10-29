import type { MissionStarter } from '@teammate/mission-store'
import type { CodexMissionStartRequest, CodexMissionStartResponse, MissionMode, MissionReadResponse, PublicModel, PublicRecoveredMission, PublicRoutine, RoutineRunResponse, TeammateListResponse, TeammateRoute } from '../shared/ipc.js'
import type { PublicBackgroundRun } from '../shared/background.js'
import { joinMessageFragments } from '../shared/messageFragments.js'
import { unwrapProtocolTags } from '../shared/protocolTags.js'
import { mcpText, type McpToolResult } from './locust-mcp-host.js'

/** A routine step in flight, as the routine runner reports it. */
export interface McpRoutineStep {
  readonly routineId: string
  readonly teammateId: string
  readonly step: number
  readonly of: number
  readonly missionId: string
}

/** Modes that change nothing: a routine made only of these may run from outside with the switch off. */
const READ_ONLY: ReadonlySet<MissionMode> = new Set(['ask', 'plan'])
/** The words Locust's own mode control uses, for a sentence a person may read. */
const MODE_NAME: Readonly<Record<MissionMode, string>> = { ask: 'Ask', plan: 'Plan', 'accept-edits': 'Edit', 'approve-each': 'Approve each action', auto: 'Auto' }

/** Only adapters to the window's existing roster, start and ledger readers. */
/** The longest read_reply waits for an answer when asked to. */
export const MCP_WAIT_LIMIT_S = 20

export function createLocustMcpTools(options: {
  readonly teammates: () => Promise<TeammateListResponse>
  readonly busy: (teammateId: string) => boolean
  readonly start: (request: CodexMissionStartRequest, origin: MissionStarter) => Promise<CodexMissionStartResponse>
  readonly read: (missionId: string) => Promise<MissionReadResponse>
  readonly newest: (missionId: string) => Promise<string>
  readonly owner: (missionId: string) => Promise<string | undefined>
  readonly live: (missionId: string) => boolean
  readonly background: () => Promise<readonly PublicBackgroundRun[]>
  /** The person's switch in Settings: the teammate's own mode instead of Ask. Read on every turn. */
  readonly ownMode?: () => boolean
  /** Who a conversation started here is recorded as started by: another app, or a voice call's Codex (0.739). */
  readonly origin?: MissionStarter
  /** A card of this turn is waiting in Locust's window. */
  readonly waiting?: (missionId: string) => boolean
  /**
   * The one conversation a voice call with a teammate may continue (0.746): the person pressed Talk in it, so its
   * asks are theirs. Undefined outside a call, and for every other server: nothing else is continued from here.
   */
  readonly here?: () => string | undefined
  /*
   * ROUTINES (0.704, Colin: "would be sick if we could actually integrate
   * workflows from here"). The routine runner's own run path -- the one Run
   * on a routine card takes -- so every check it makes still holds.
   */
  readonly routines?: () => Promise<readonly PublicRoutine[]>
  readonly runRoutine?: (routineId: string, values: Readonly<Record<string, string>> | undefined) => Promise<RoutineRunResponse>
  readonly routineSteps?: () => readonly McpRoutineStep[]
  /** The newest step a routine started, kept as each one starts, so a step that ended between two reads is not missed. */
  readonly routineLastStep?: (routineId: string) => string | undefined
  /**
   * THE MODELS A TURN MAY ASK FOR (0.842). The window's own catalog, after the organization's policy: a model and
   * effort another app names are checked against what the teammate's AI agent offers here, and nothing else.
   * Undefined for a server that does not offer the choice (a voice call's).
   */
  readonly models?: () => Promise<readonly PublicModel[]>
  /**
   * THE FOLDER A TURN WORKS IN (0.842), by its name alone: a turn runs in the folder Locust has open, which the other
   * app could not see. Colin's Duet asked for a change to Locust's code, and the turn ran in another project.
   */
  readonly folder?: () => string | undefined
  /**
   * STOPPING A TURN IT STARTED (0.842), by the turn's run id: the window's own Stop. Colin's Duet started a turn in
   * Auto that went the wrong way, and only he could stop it. Undefined for a server that does not offer it.
   */
  readonly cancel?: (runId: string) => boolean
  /** How a read_reply that was asked to wait sleeps between looks; tests pass one that does not. */
  readonly pause?: (ms: number) => Promise<void>
}) {
  const pause = options.pause ?? ((ms: number) => new Promise<void>(done => setTimeout(done, ms)))
  const starting = new Set<string>()
  // The routines this server started. Their status is readable here; a person's own runs are not.
  const routinesStartedHere = new Set<string>()
  const text = (value: unknown, max = 200): value is string => typeof value === 'string' && value.trim().length > 0 && value.length <= max && !value.includes('\0')
  const lookup = async (id: string) => {
    const root = await options.read(id)
    if (!root.ok) return undefined
    if (root.data.mission.startedBy?.kind !== 'mcp' && (options.here?.() === undefined || options.here() !== id)) return undefined
    const newest = await options.newest(id)
    const read = newest === id ? root : await options.read(newest)
    return read.ok ? read.data.mission : undefined
  }
  /*
   * Ask unless the person turned on "each teammate's own mode" (0.703); then
   * the mode saved on the teammate in Locust -- never one the other app names.
   * Auto is still refused on the host's one path unless Settings allows it,
   * and approval cards still go only to Locust's window.
   */
  const modeOf = async (teammateId: string): Promise<MissionMode> => {
    if (options.ownMode?.() !== true) return 'ask'
    const list = await options.teammates()
    return (list.ok ? list.data.teammates.find(t => t.teammateId === teammateId)?.route?.mode : undefined) ?? 'ask'
  }
  /** A teammate's saved route, from the roster. */
  const routeOf = async (teammateId: string): Promise<TeammateRoute | undefined> => {
    const list = await options.teammates()
    return list.ok ? list.data.teammates.find(t => t.teammateId === teammateId)?.route : undefined
  }
  /** The models the teammate's AI agent offers here, with the efforts each takes. */
  const modelsFor = async (route: TeammateRoute): Promise<readonly PublicModel[]> =>
    (await options.models?.().catch(() => [] as readonly PublicModel[]) ?? []).filter(model => model.runtime === route.runtime && model.own !== true)
  const efforts = (model: PublicModel): readonly string[] => [...new Set([...model.supportedEfforts, ...Object.keys(model.variants ?? {})])]
  /**
   * A MODEL AND EFFORT FOR THIS TURN ALONE (0.842; Colin's Duet asked for it: "run this on model X at high effort").
   * Only on the teammate's own AI agent and account, only a model and effort that agent offers here, and never the
   * mode: that stays Ask, or the teammate's own when the person said so. The saved route is not changed.
   */
  const choiceOf = async (teammateId: string, args: Record<string, unknown>): Promise<Partial<CodexMissionStartRequest> | string | undefined> => {
    if (args.model === undefined && args.effort === undefined) return undefined
    if (options.models === undefined) return 'This server does not take a model or effort. Leave them out to use the teammate’s own.'
    if ((args.model !== undefined && !text(args.model)) || (args.effort !== undefined && !text(args.effort, 40))) return 'model and effort are short text: use the ids list_models gives.'
    const route = await routeOf(teammateId)
    if (route === undefined) return 'That teammate has no AI agent set up yet. Choose one for it in Locust first.'
    const offered = await modelsFor(route)
    const asked = typeof args.model === 'string' ? args.model.trim().toLowerCase() : route.model.toLowerCase()
    const model = offered.find(one => one.id.toLowerCase() === asked || one.displayName.toLowerCase() === asked || Object.values(one.variants ?? {}).some(id => id.toLowerCase() === asked))
    if (model === undefined) {
      if (args.model === undefined) return 'The teammate runs on its account’s default model, whose efforts Locust cannot name: give a model too, from list_models.'
      return `That model is not one ${route.runtime} offers in this Locust. Ask list_models for this teammate’s choices.`
    }
    const effort = typeof args.effort === 'string' ? args.effort.trim().toLowerCase() : undefined
    if (effort !== undefined && !efforts(model).includes(effort)) {
      return efforts(model).length === 0 ? `${model.displayName} takes no effort level here: leave effort out.` : `${model.displayName} takes these efforts here: ${efforts(model).join(', ')}.`
    }
    // The saved effort goes with a model asked for alone, when that model takes it.
    const kept = effort ?? (route.effort !== undefined && efforts(model).includes(route.effort) ? route.effort : undefined)
    return { runtime: route.runtime, model: model.id, ...(kept === undefined ? {} : { effort: kept }), ...(route.account === undefined ? {} : { account: route.account }), routeOverrideFor: teammateId }
  }
  const started = async (request: Pick<CodexMissionStartRequest, 'prompt' | 'teammateId' | 'followUpOf'>, conversation?: string, args: Record<string, unknown> = {}): Promise<McpToolResult> => {
    // This is the only request shape that can reach the start path. A tool
    // argument named mode/route/etc. never widens it or changes a saved route.
    const choice = await choiceOf(request.teammateId!, args)
    if (typeof choice === 'string') return mcpText(`${choice} Nothing was started.`, true)
    // READ ONLY, ASKED FOR (0.842): an app may step a turn DOWN to Ask, never up -- a teammate set to Auto can still be
    // asked a question that changes nothing. Any other mode it names is ignored, as before.
    const mode = args.read_only === true ? 'ask' : await modeOf(request.teammateId!)
    const result = await options.start({ ...request, ...choice, mode, keepSavedRoute: true } as CodexMissionStartRequest, options.origin ?? { kind: 'mcp' })
    const folder = options.folder?.()
    return result.ok
      ? mcpText(JSON.stringify({ conversation_id: conversation ?? result.data.missionId, mode, ...(choice?.model === undefined ? {} : { model: choice.model }), ...(choice?.effort === undefined ? {} : { effort: choice.effort }), ...(folder === undefined ? {} : { folder }) }))
      : mcpText(result.error.message, true)
  }
  /** A finished turn's answer, or Locust's own words for why there is none. */
  const answerOf = (mission: PublicRecoveredMission): McpToolResult => {
    const failed = mission.events.findLast(e => e.type === 'run.failed')
    if (mission.phase === 'failed') return mcpText(mission.hostFailureMessage ?? (failed?.type === 'run.failed' ? failed.payload.message : 'The AI agent ended without an answer.'), true)
    if (mission.phase !== 'completed') return mcpText('The run was interrupted or stopped before it finished.', true)
    const final = joinMessageFragments(mission.events).findLast(e => e.type === 'message.delta' && e.payload.final)
    return mcpText(final?.type === 'message.delta' ? unwrapProtocolTags(final.payload.text) : 'The AI agent finished without a text answer.')
  }
  /*
   * The mode each step runs in, as the runner decides it: the routine's saved
   * mode for its own teammate's steps, and a step handed to another teammate
   * runs on THAT teammate's route (routine-runner.ts, 0.435).
   */
  const stepModes = (routine: PublicRoutine, team: TeammateListResponse): readonly MissionMode[] =>
    routine.steps.map((_, index) => {
      const owner = routine.handOffs?.[index]?.teammateId ?? routine.teammateId
      if (owner === routine.teammateId) return routine.route.mode
      return (team.ok ? team.data.teammates.find(t => t.teammateId === owner)?.route?.mode : undefined) ?? 'ask'
    })
  const findRoutine = async (given: unknown): Promise<PublicRoutine | string> => {
    if (options.routines === undefined) return 'This Locust does not offer routines to other apps.'
    if (!text(given)) return 'Name a routine by its id or name from list_routines.'
    const all = await options.routines()
    const matches = all.filter(r => r.routineId === given || r.name.toLowerCase() === given.toLowerCase())
    return matches.length === 1 ? matches[0]! : 'That routine could not be identified. Use its id from list_routines.'
  }
  return async (name: string, arguments_: unknown): Promise<McpToolResult> => {
    const args = typeof arguments_ === 'object' && arguments_ !== null && !Array.isArray(arguments_) ? arguments_ as Record<string, unknown> : {}
    if (name === 'list_teammates') {
      const list = await options.teammates()
      if (!list.ok) return mcpText(list.error.message, true)
      const own = options.ownMode?.() === true
      const folder = options.folder?.()
      return mcpText(JSON.stringify(list.data.teammates.map(t => ({ id: t.teammateId, name: t.name, role: t.roleTitle ?? t.role, ...(folder === undefined ? {} : { folder }),
        runtime: t.route?.runtime ?? null, model: t.route?.model ?? null, effort: t.route?.effort ?? null, busy: options.busy(t.teammateId),
        // What a turn started from here would use, so the other app is never surprised by an edit.
        runs_in: own ? t.route?.mode ?? 'ask' : 'ask' }))))
    }
    if (name === 'list_background_runs') return mcpText(JSON.stringify(await options.background()))
    if (name === 'start_conversation') {
      if (!text(args.teammate) || !text(args.message, 8_000)) return mcpText('Choose a teammate and give it a message of up to 8,000 characters. Nothing was started.', true)
      const list = await options.teammates()
      if (!list.ok) return mcpText(list.error.message, true)
      const matches = list.data.teammates.filter(t => t.teammateId === args.teammate || t.name.toLowerCase() === (args.teammate as string).toLowerCase())
      if (matches.length !== 1) return mcpText('That teammate could not be identified. Use its id from list_teammates. Nothing was started.', true)
      return started({ prompt: args.message, teammateId: matches[0]!.teammateId }, undefined, args)
    }
    if (name === 'list_models') {
      if (options.models === undefined) return mcpText('This server does not offer a choice of model.', true)
      if (!text(args.teammate)) return mcpText('Name a teammate by its id or name from list_teammates.', true)
      const list = await options.teammates()
      if (!list.ok) return mcpText(list.error.message, true)
      const matches = list.data.teammates.filter(t => t.teammateId === args.teammate || t.name.toLowerCase() === (args.teammate as string).toLowerCase())
      if (matches.length !== 1) return mcpText('That teammate could not be identified. Use its id from list_teammates.', true)
      const route = matches[0]!.route
      if (route === undefined) return mcpText('That teammate has no AI agent set up yet. Choose one for it in Locust first.', true)
      const offered = await modelsFor(route)
      return mcpText(JSON.stringify({ teammate: matches[0]!.name, runtime: route.runtime, model: route.model, effort: route.effort ?? null,
        models: offered.filter(one => one.older !== true).map(one => ({ id: one.id, name: one.displayName, efforts: efforts(one), ...(one.defaultEffort === undefined ? {} : { default_effort: one.defaultEffort }) })) }))
    }
    if (name === 'list_routines') {
      if (options.routines === undefined) return mcpText('This Locust does not offer routines to other apps.', true)
      const [all, team] = await Promise.all([options.routines(), options.teammates()])
      const own = options.ownMode?.() === true
      const nameOf = (id: string) => (team.ok ? team.data.teammates.find(t => t.teammateId === id)?.name : undefined) ?? 'a teammate no longer here'
      return mcpText(JSON.stringify(all.map(routine => {
        const modes = stepModes(routine, team)
        const writes = modes.some(mode => !READ_ONLY.has(mode))
        const folder = (routine.inputs ?? []).some(input => input.kind === 'folder')
        return {
          id: routine.routineId, name: routine.name,
          steps: routine.steps.map((_, index) => ({ teammate: nameOf(routine.handOffs?.[index]?.teammateId ?? routine.teammateId), mode: modes[index], ...(routine.handOffs?.[index]?.check === true ? { checks_the_work: true } : {}), ...(routine.handOffs?.[index]?.askFirst === true ? { waits_for_your_ok: true } : {}) })),
          inputs: (routine.inputs ?? []).map(input => ({ key: input.key, label: input.label, kind: input.kind, required: input.required,
            ...(input.default === undefined ? {} : { default: input.default }), ...(input.choices === undefined ? {} : { choices: input.choices }) })),
          changes_files: writes,
          ...(routine.paused === true ? { paused: true } : {}),
          // Whether run_routine would start it now, and why not.
          runs_from_here: routine.paused !== true && !folder && (!writes || own),
          ...(routine.paused === true ? { why_not: 'It is paused in Locust.' } : folder ? { why_not: "It asks for a folder, which is chosen in Locust's own picker." } : writes && !own ? { why_not: "It changes files, and \"Use each teammate's own mode\" is off in Locust." } : {})
        }
      })))
    }
    if (name === 'run_routine') {
      const routine = await findRoutine(args.routine)
      if (typeof routine === 'string') return mcpText(`${routine} Nothing was started.`, true)
      if (options.runRoutine === undefined) return mcpText('This Locust does not offer routines to other apps. Nothing was started.', true)
      // Paused (0.705) is the person's word that it should not go on its own; another app's request is not theirs.
      if (routine.paused === true) return mcpText(`"${routine.name}" is paused in Locust. Resume it there, or run it from Locust. Nothing was started.`, true)
      if ((routine.inputs ?? []).some(input => input.kind === 'folder')) {
        return mcpText(`"${routine.name}" asks for a folder, which is chosen in Locust's own picker. Run it from Locust. Nothing was started.`, true)
      }
      const team = await options.teammates()
      const modes = stepModes(routine, team)
      const writing = modes.findIndex(mode => !READ_ONLY.has(mode))
      if (writing >= 0 && options.ownMode?.() !== true) {
        return mcpText(`"${routine.name}" changes files (step ${String(writing + 1)} runs in ${MODE_NAME[modes[writing]!]}). Turn on "Use each teammate's own mode" in Locust's Settings to let other apps run it. Nothing was started.`, true)
      }
      // Strings only; the runner's own reader checks each against the routine's inputs.
      let values: Record<string, string> | undefined
      if (args.values !== undefined) {
        if (typeof args.values !== 'object' || args.values === null || Array.isArray(args.values)) return mcpText('values must be an object of input keys to text. Nothing was started.', true)
        const entries = Object.entries(args.values as Record<string, unknown>)
        if (entries.length > 12 || entries.some(([key, value]) => !text(key, 80) || typeof value !== 'string' || value.length > 4_000)) return mcpText('values must be at most 12 input keys, each to text of up to 4,000 characters. Nothing was started.', true)
        values = Object.fromEntries(entries) as Record<string, string>
      }
      const result = await options.runRoutine(routine.routineId, values)
      if (!result.ok) return mcpText(result.error.message, true)
      routinesStartedHere.add(routine.routineId)
      return mcpText(JSON.stringify({ routine_id: routine.routineId, steps: routine.steps.length, first_conversation_id: result.data.missionId }))
    }
    if (name === 'routine_status') {
      const routine = await findRoutine(args.routine)
      if (typeof routine === 'string') return mcpText(routine, true)
      if (!routinesStartedHere.has(routine.routineId)) return mcpText('This server has not started that routine since Locust opened. Its runs are in Locust.', true)
      const step = options.routineSteps?.().find(entry => entry.routineId === routine.routineId)
      if (step !== undefined) {
        // This app cannot answer a card: there is no tool for it, by design.
        if (options.waiting?.(step.missionId) === true) return mcpText(`Waiting for the person to approve or deny an action in Locust's window (step ${String(step.step)} of ${String(step.of)}). This app cannot answer it; routine_status again later.`)
        return mcpText(`Still working: step ${String(step.step)} of ${String(step.of)}.`)
      }
      const execution = routine.execution
      if (execution !== undefined && execution.status === 'held') {
        return mcpText(`Stopped at step ${String(execution.step)} of ${String(execution.of)} and waiting for the person in Locust${execution.reason === undefined ? '.' : `: ${execution.reason}`}`, true)
      }
      if (execution !== undefined && execution.status !== 'abandoned') return mcpText(`Still working: step ${String(execution.step)} of ${String(execution.of)}.`)
      if (routine.lastFailed !== undefined) return mcpText(`The run ended with its check still failing: ${routine.lastFailed}`, true)
      const last = options.routineLastStep?.(routine.routineId)
      if (last === undefined) return mcpText('The run ended, and Locust has no answer from it to give.', true)
      const read = await options.read(await options.newest(last))
      if (!read.ok) return mcpText('The run ended, but its last answer could not be read from the local ledger.', true)
      const answer = answerOf(read.data.mission)
      if (answer.isError === true || routine.staged === undefined) return answer
      return mcpText(`Its changes are waiting in Locust under Routines for the person to Keep or Discard; nothing has reached the folder yet.\n\n${answer.content[0]!.text}`)
    }
    if (name === 'stop_turn' && options.cancel !== undefined) {
      if (!text(args.conversation_id)) return mcpText('Give a conversation id returned by start_conversation.', true)
      const mission = await lookup(args.conversation_id)
      if (mission === undefined) return mcpText('That conversation could not be read from the local ledger. Use an id returned by this server.', true)
      // Only a turn an app started: the person may have gone on in Locust's window, and that turn is theirs to stop.
      if (mission.startedBy?.kind !== 'mcp') return mcpText('The newest turn there was started by the person in Locust. Only they can stop it. Nothing was stopped.', true)
      if (!options.live(mission.missionId)) return mcpText('Nothing is running in that conversation now. Nothing was stopped.')
      return options.cancel(mission.runId)
        ? mcpText('Stopping it. What it already changed stays; the person can undo that turn in Locust.')
        : mcpText('Locust could not stop it from here. The person can stop it in Locust.', true)
    }
    if (name === 'send_message' || name === 'read_reply') {
      if (!text(args.conversation_id)) return mcpText('Give a conversation id returned by start_conversation.', true)
      const id = args.conversation_id
      if (starting.has(id)) return mcpText('Still working: starting the next turn.')
      // Claim before the first read: concurrent sends must not both resume
      // the same old tip, even if the first finishes while the second reads.
      if (name === 'send_message') starting.add(id)
      try {
        let mission = await lookup(id)
        if (mission === undefined) return mcpText('That conversation could not be read from the local ledger. Use an id returned by this server.', true)
        // WAITING FOR THE ANSWER (0.842): an app asks read_reply again and again while a turn works -- on Claude on the
        // web every call is one more Allow -- so it may wait here for the turn to end or a card to come up, up to
        // MCP_WAIT_LIMIT_S: under the relay's 30 seconds and the bridge's 60.
        if (name === 'read_reply' && typeof args.wait_seconds === 'number' && Number.isFinite(args.wait_seconds)) {
          const looking = mission.missionId
          for (let left = Math.min(Math.max(Math.floor(args.wait_seconds), 0), MCP_WAIT_LIMIT_S) * 2; left > 0 && options.live(looking) && options.waiting?.(looking) !== true; left--) await pause(500)
          mission = await lookup(id)
          if (mission === undefined) return mcpText('That conversation could not be read from the local ledger. Use an id returned by this server.', true)
        }
        if (options.live(mission.missionId)) {
          // This app cannot answer a card: there is no tool for it, by design.
          if (options.waiting?.(mission.missionId) === true) return mcpText("Waiting for the person to approve or deny an action in Locust's window. This app cannot answer it; read_reply again later.")
          const last = mission.events.findLast(e => e.type === 'tool.started' || e.type === 'step.started')
          const doing = last?.type === 'tool.started' ? last.payload.title ?? `Using ${last.payload.name}` : last?.type === 'step.started' ? last.payload.message : undefined
          return mcpText(`Still working${doing ? `: ${doing}` : ': waiting for the AI agent to answer.'}`)
        }
        if (name === 'send_message') {
          if (!text(args.message, 8_000)) return mcpText('Give a message of up to 8,000 characters. Nothing was sent.', true)
          const owner = await options.owner(mission.missionId)
          if (owner === undefined) return mcpText('That conversation no longer has a teammate. Nothing was sent.', true)
          // Keep the claim until startup settles, including the saved-mode lookup.
          return await started({ prompt: args.message, teammateId: owner, followUpOf: mission.missionId }, id, args)
        }
        return answerOf(mission)
      } finally { if (name === 'send_message') starting.delete(id) }
    }
    return mcpText('That tool is not offered by Locust.', true)
  }
}
