import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { LocalRuntimeId, MissionMode, PublicModel } from './ipc.js'
import { parsedAcpRouteModel } from './acp-agents.js'
import { runtimeDisplayName } from './runtimes.js'

export type PolicyRuntimeId = LocalRuntimeId | MissionRuntimeId
export const POLICY_RUNTIME_IDS = ['codex', 'claude', 'cursor', 'gemini', 'opencode', 'copilot', 'antigravity', 'muse', 'omniroute', 'acp'] as const satisfies readonly PolicyRuntimeId[]

/** Increasing authority: read, plan, ask before actions, edit without asking, unrestricted. */
export const POLICY_MODE_ORDER: readonly MissionMode[] = ['ask', 'plan', 'approve-each', 'accept-edits', 'auto']
const MODE_NAMES: Readonly<Record<MissionMode, string>> = { ask: 'Ask', plan: 'Plan', 'approve-each': 'Approve each', 'accept-edits': 'Accept edits', auto: 'Auto' }

export interface OrganizationPolicy {
  readonly version?: 1
  readonly allowedAgents?: readonly PolicyRuntimeId[]
  readonly allowedModels?: Readonly<Partial<Record<PolicyRuntimeId, readonly string[]>>>
  readonly highestMode?: MissionMode
  readonly monthlyLimitUsd?: number
  readonly otherApps?: boolean
  readonly phone?: boolean
  readonly cloud?: boolean
  readonly terminal?: boolean
}

/** A launch-time snapshot. An error is a closed gate, never an empty policy. */
export interface OrganizationPolicyState {
  readonly path: string
  readonly present: boolean
  readonly policy: OrganizationPolicy
  readonly error?: string
}

export interface PolicyTurn {
  readonly runtime: PolicyRuntimeId
  readonly model?: string
  readonly mode: MissionMode
  readonly cloud?: boolean
}

export function policyRefusal(state: OrganizationPolicyState, setting: keyof OrganizationPolicy, why: string): string {
  return `${why} Set by your organization (${setting} in ${state.path}). Nothing was started.`
}

export function policyFeatureRefusal(state: OrganizationPolicyState | undefined, setting: 'otherApps' | 'phone' | 'cloud' | 'terminal'): string | undefined {
  if (state?.error !== undefined) return state.error
  if (state?.policy[setting] !== false) return undefined
  const name = { otherApps: 'Access from other AI apps, the locust command and editors is off.', phone: 'Phone pairing is off.', cloud: 'Cloud computers and cloud tasks are off.', terminal: 'The terminal panel is off.' }[setting]
  return policyRefusal(state, setting, name)
}

export function policyModeAllowed(state: OrganizationPolicyState | undefined, mode: MissionMode): boolean {
  if (state?.error !== undefined) return false
  const highest = state?.policy.highestMode
  return highest === undefined || POLICY_MODE_ORDER.indexOf(mode) <= POLICY_MODE_ORDER.indexOf(highest)
}

/** Only * has special meaning; model ids are otherwise literal, with no regex metacharacters. */
export function policyModelMatches(pattern: string, id: string): boolean {
  let p = 0; let i = 0; let star = -1; let from = 0
  while (i < id.length) {
    if (pattern[p] === '*') { star = p++; from = i }
    else if (p < pattern.length && pattern[p] === id[i]) { p++; i++ }
    else if (star !== -1) { p = star + 1; i = ++from }
    else return false
  }
  while (pattern[p] === '*') p++
  return p === pattern.length
}

export function policyAgentAllowed(state: OrganizationPolicyState | undefined, runtime: PolicyRuntimeId): boolean {
  return state?.error === undefined && (state?.policy.allowedAgents === undefined || state.policy.allowedAgents.includes(runtime))
}

export function policyModelAllowed(state: OrganizationPolicyState | undefined, runtime: PolicyRuntimeId, model: string | undefined): boolean {
  if (!policyAgentAllowed(state, runtime)) return false
  const patterns = state?.policy.allowedModels?.[runtime]
  if (patterns === undefined || patterns.includes('*')) return true
  const id = runtime === 'acp' ? parsedAcpRouteModel(model)?.model : model
  // A runtime's default is unknown until it runs. A finite allowlist needs an explicit model.
  return id !== undefined && id !== 'account-default' && patterns.some((pattern) => policyModelMatches(pattern, id))
}

export function policyTurnRefusal(state: OrganizationPolicyState | undefined, turn: PolicyTurn): string | undefined {
  if (state === undefined) return undefined
  if (state.error !== undefined) return state.error
  if (turn.cloud === true) {
    const refused = policyFeatureRefusal(state, 'cloud')
    if (refused !== undefined) return refused
  }
  const agent = turn.runtime === 'omniroute' ? 'OmniRoute' : runtimeDisplayName(turn.runtime)
  if (!policyAgentAllowed(state, turn.runtime)) return policyRefusal(state, 'allowedAgents', `${agent} is not an allowed AI agent.`)
  if (!policyModelAllowed(state, turn.runtime, turn.model)) return policyRefusal(state, 'allowedModels', `The ${turn.runtime} model ${turn.model ?? 'account default'} is not allowed. Pick an allowed, explicit model.`)
  if (!policyModeAllowed(state, turn.mode)) return policyRefusal(state, 'highestMode', `${MODE_NAMES[turn.mode]} exceeds the highest allowed mode, ${MODE_NAMES[state.policy.highestMode!]}.`)
  return undefined
}

const agentWords = (runtime: PolicyRuntimeId): string => runtime === 'omniroute' ? 'OmniRoute' : runtime === 'acp' ? 'Agents you added' : runtimeDisplayName(runtime)

/**
 * A policy setting as Settings says it (0.792): agents and modes by name, a limit in dollars a month. It was the
 * file's own JSON -- `["codex"]`, `{"codex":["gpt-*"]}`, `accept-edits` -- which an administrator wrote and the
 * person reading Settings did not.
 */
export function policySettingWords(key: string, value: unknown): string {
  switch (key) {
    case 'allowedAgents':
      return Array.isArray(value) && value.length > 0 ? (value as PolicyRuntimeId[]).map(agentWords).join(', ') : 'None: no AI agent can run'
    case 'allowedModels': {
      const each = Object.entries((value ?? {}) as Partial<Record<PolicyRuntimeId, readonly string[]>>)
      return each.length === 0 ? 'Any' : each.map(([runtime, patterns]) => `${agentWords(runtime as PolicyRuntimeId)}: ${patterns === undefined || patterns.length === 0 ? 'none' : patterns.join(', ')}`).join('; ')
    }
    case 'highestMode':
      return MODE_NAMES[value as MissionMode] ?? String(value)
    case 'monthlyLimitUsd':
      return value === 0 ? '$0, so nothing that spends can start' : `$${Number.isInteger(value) ? String(value) : Number(value).toFixed(2)} a month`
    case 'otherApps':
    case 'phone':
    case 'cloud':
    case 'terminal':
      return value === false ? 'Off' : 'Allowed'
    default:
      return String(value)
  }
}

export function policyMonthlyLimit(state: OrganizationPolicyState | undefined, personal: number | undefined): number | undefined {
  const cap = state?.policy.monthlyLimitUsd
  return cap === undefined ? personal : personal === undefined ? cap : Math.min(cap, personal)
}

/** Interactive terminals and legacy cloud sessions do not return spending receipts to Locust. */
export function policyUnmeteredRefusal(state: OrganizationPolicyState | undefined, turn: PolicyTurn): string | undefined {
  return policyTurnRefusal(state, turn) ?? (state?.policy.monthlyLimitUsd === undefined ? undefined
    : policyRefusal(state, 'monthlyLimitUsd', 'This launch cannot report spending to Locust, so its monthly limit cannot be enforced.'))
}

/** A plain cloud shell cannot constrain which installed agent a person starts there. */
export function policyCloudTerminalRefusal(state: OrganizationPolicyState | undefined): string | undefined {
  const refused = policyFeatureRefusal(state, 'cloud')
  if (refused !== undefined) return refused
  if (state?.policy.allowedAgents !== undefined) return policyRefusal(state, 'allowedAgents', 'A cloud terminal cannot enforce the allowed AI agents. Use a conversation in Locust.')
  if (state?.policy.allowedModels !== undefined) return policyRefusal(state, 'allowedModels', 'A cloud terminal cannot enforce the allowed models. Use a conversation in Locust.')
  return policyUnmeteredRefusal(state, { runtime: 'opencode', mode: 'auto' })
}

/** A person's unrestricted shell cannot enforce an agent, mode, model or spending restriction. */
export function policyTerminalRefusal(state: OrganizationPolicyState | undefined): string | undefined {
  const refused = policyFeatureRefusal(state, 'terminal')
  if (refused !== undefined || state === undefined) return refused
  for (const key of ['allowedAgents', 'allowedModels', 'monthlyLimitUsd'] as const) {
    if (state.policy[key] !== undefined) return policyRefusal(state, key, 'An interactive shell cannot enforce this organization restriction. Use a conversation in Locust.')
  }
  if (state.policy.highestMode !== undefined && state.policy.highestMode !== 'auto') return policyRefusal(state, 'highestMode', 'An interactive shell cannot enforce the highest permission mode. Use a conversation in Locust.')
  return undefined
}

/** Keep only selectable ids, including each effort variant; no synthetic default escapes the allowlist. */
export function policyModels(state: OrganizationPolicyState | undefined, models: readonly PublicModel[]): readonly PublicModel[] {
  return models.flatMap((model) => {
    if (model.variants === undefined) return policyModelAllowed(state, model.runtime, model.id) ? [model] : []
    const variants = Object.fromEntries(Object.entries(model.variants).filter(([, id]) => policyModelAllowed(state, model.runtime, id)))
    if (policyModelAllowed(state, model.runtime, model.id) && Object.keys(variants).length === Object.keys(model.variants).length) return [model]
    const id = policyModelAllowed(state, model.runtime, model.id) ? model.id : Object.values(variants)[0]
    if (id === undefined) return []
    const supportedEfforts = model.supportedEfforts.filter((effort) => variants[effort] !== undefined)
    return [{ ...model, id, variants, supportedEfforts, defaultEffort: model.defaultEffort !== undefined && variants[model.defaultEffort] !== undefined ? model.defaultEffort : supportedEfforts[0] }]
  })
}
