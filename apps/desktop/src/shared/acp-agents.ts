/**
 * YOUR OWN AGENTS (0.777).
 *
 * Locust ran the agents it was built for -- Codex, Claude Code, Cursor, Copilot, OpenCode, Antigravity, Muse -- and
 * nothing else. The Agent Client Protocol (agentclientprotocol.com) is how dozens more speak to an app: Goose, Qwen
 * Code, Kimi CLI, Mistral Vibe, Factory Droid, Junie and the rest, each started by a command line. Locust already
 * speaks it to Copilot (acp-run.ts). So a person adds an agent by its command line; Test opens a session with it,
 * sends no prompt, and keeps what it said: its name, its version and the models it offers. It then runs like any
 * teammate's agent, in Approve each, where every action it asks to take is a card.
 *
 * A route on one names it by its id in the route's model, with the model the person picked after a `::`.
 */
export const ACP_AGENTS_LIST_CHANNEL = 'acp-agents:list'
export const ACP_AGENTS_TEST_CHANNEL = 'acp-agents:test'
export const ACP_AGENTS_SAVE_CHANNEL = 'acp-agents:save'
export const ACP_AGENTS_REMOVE_CHANNEL = 'acp-agents:remove'

export const MAX_ACP_AGENTS = 12
const MAX_COMMAND_LINE = 1_000
const MAX_NAME = 40

export interface AcpAgentModel {
  readonly value: string
  readonly name: string
}

/** What the agent said when Test opened a session with it. */
export interface AcpAgentAnswer {
  readonly agentName?: string
  readonly version?: string
  /** The config option it sets its model by (`model` on OpenCode), and the values it offered. */
  readonly modelOption?: { readonly id: string; readonly current?: string; readonly models: readonly AcpAgentModel[] }
  /** How it says to sign in, when it does: its own words, for a person to do in a terminal. */
  readonly signIn?: readonly string[]
}

export interface AcpAgent {
  /** `acp_` and the name's letters: stable, and what a route names it by. */
  readonly id: string
  readonly name: string
  readonly commandLine: string
  readonly answer?: AcpAgentAnswer
  readonly testedAt?: string
}

export type AcpAgentsResponse = { readonly ok: true; readonly agents: readonly AcpAgent[] } | { readonly ok: false; readonly message: string }
export type AcpAgentTest = { readonly ok: true; readonly answer: AcpAgentAnswer } | { readonly ok: false; readonly message: string }

export interface AcpAgentsApi {
  list(): Promise<AcpAgentsResponse>
  /** Open a session with this command line and close it again. Sends no prompt: nothing is spent. */
  test(commandLine: string): Promise<AcpAgentTest>
  /** Tests first, and keeps it only when it answered. */
  save(name: string, commandLine: string): Promise<AcpAgentsResponse>
  remove(id: string): Promise<AcpAgentsResponse>
}

/**
 * A command line as words: spaces separate, double quotes keep a space inside one ("C:\Program Files\x\x.exe").
 * Undefined for an empty line, an unclosed quote, a line too long, or a first word that is a shell's, not a program's.
 */
export function commandWords(line: string): readonly string[] | undefined {
  if (typeof line !== 'string' || line.length > MAX_COMMAND_LINE) return undefined
  const words: string[] = []
  let word = ''
  let quoted = false
  let started = false
  for (const character of line.trim()) {
    if (character === '"') { quoted = !quoted; started = true; continue }
    if (!quoted && /\s/.test(character)) {
      if (started) words.push(word)
      word = ''
      started = false
      continue
    }
    word += character
    started = true
  }
  if (quoted) return undefined
  if (started) words.push(word)
  if (words.length === 0) return undefined
  // Run directly, never through a shell: a line is a program and its arguments, not a script.
  if (/[|&;<>`$]/.test(line.replace(/"[^"]*"/g, ''))) return undefined
  if (/^(?:cmd|cmd\.exe|powershell|powershell\.exe|pwsh|pwsh\.exe|bash|sh|zsh)$/i.test(words[0]!)) return undefined
  return words
}

export function acpAgentId(name: string, taken: readonly string[] = []): string {
  const base = `acp_${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 24) || 'agent'}`
  if (!taken.includes(base)) return base
  for (let n = 2; ; n += 1) if (!taken.includes(`${base}-${String(n)}`)) return `${base}-${String(n)}`
}

export function cleanAgentName(name: unknown): string | undefined {
  if (typeof name !== 'string') return undefined
  const clean = name.replace(/\s+/g, ' ').trim()
  return clean.length === 0 || clean.length > MAX_NAME ? undefined : clean
}

/** The route's model for an agent, and the model picked on it. */
export function acpRouteModel(agentId: string, model?: string): string {
  return model === undefined || model.length === 0 ? agentId : `${agentId}::${model}`
}

export function parsedAcpRouteModel(model: string | undefined): { readonly agentId: string; readonly model?: string } | undefined {
  if (typeof model !== 'string' || !model.startsWith('acp_')) return undefined
  const at = model.indexOf('::')
  if (at < 0) return { agentId: model }
  const agentId = model.slice(0, at)
  const picked = model.slice(at + 2)
  return picked.length === 0 ? { agentId } : { agentId, model: picked }
}

/** A saved file's agents, keeping only well-formed ones (a hand-edited file never reaches the runner unchecked). */
export function parsedAcpAgents(value: unknown): readonly AcpAgent[] {
  const list = typeof value === 'object' && value !== null && Array.isArray((value as { agents?: unknown }).agents) ? (value as { agents: unknown[] }).agents : []
  const agents: AcpAgent[] = []
  for (const raw of list) {
    if (typeof raw !== 'object' || raw === null) continue
    const entry = raw as Record<string, unknown>
    const name = cleanAgentName(entry.name)
    if (typeof entry.id !== 'string' || !/^acp_[a-z0-9-]{1,30}$/.test(entry.id) || name === undefined) continue
    if (typeof entry.commandLine !== 'string' || commandWords(entry.commandLine) === undefined) continue
    if (agents.some((agent) => agent.id === entry.id)) continue
    agents.push({
      id: entry.id,
      name,
      commandLine: entry.commandLine,
      ...(typeof entry.answer === 'object' && entry.answer !== null ? { answer: parsedAnswer(entry.answer) } : {}),
      ...(typeof entry.testedAt === 'string' ? { testedAt: entry.testedAt } : {})
    })
    if (agents.length === MAX_ACP_AGENTS) break
  }
  return agents
}

const text = (value: unknown, max = 200): string | undefined => (typeof value === 'string' && value.length > 0 ? value.slice(0, max) : undefined)

/*
 * EVERY MODEL AN AGENT OFFERS (0.835). This kept the first 200, and Kilo's agent offers 336 (Sol's measurement,
 * 2026-10-25): seven of its free models were among the 136 dropped, with nothing said. A bound stays, as on every
 * list read from another program, high enough that no agent known today meets it.
 */
export const MAX_AGENT_MODELS = 1_000

export function parsedAnswer(value: unknown): AcpAgentAnswer {
  const answer = (typeof value === 'object' && value !== null ? value : {}) as Record<string, unknown>
  const option = (typeof answer.modelOption === 'object' && answer.modelOption !== null ? answer.modelOption : undefined) as Record<string, unknown> | undefined
  const models = Array.isArray(option?.models)
    ? (option.models as unknown[]).flatMap((model) => {
        const entry = (typeof model === 'object' && model !== null ? model : {}) as Record<string, unknown>
        const value = text(entry.value)
        return value === undefined ? [] : [{ value, name: text(entry.name) ?? value }]
      }).slice(0, MAX_AGENT_MODELS)
    : []
  const optionId = text(option?.id, 100)
  const current = text(option?.current)
  const signIn = Array.isArray(answer.signIn) ? (answer.signIn as unknown[]).flatMap((line) => (text(line, 300) === undefined ? [] : [text(line, 300)!])).slice(0, 5) : []
  const agentName = text(answer.agentName, 80)
  const version = text(answer.version, 40)
  return {
    ...(agentName === undefined ? {} : { agentName }),
    ...(version === undefined ? {} : { version }),
    ...(optionId === undefined ? {} : { modelOption: { id: optionId, ...(current === undefined ? {} : { current }), models } }),
    ...(signIn.length === 0 ? {} : { signIn })
  }
}

/**
 * What an agent's `session/new` offered as its model: a config option of category `model` (OpenCode 1.18), whose
 * options may be grouped, and which the runner sets as it sets Copilot's (acp-run's `requiredConfig`). An agent that
 * offers no such option runs on its own default model, and the picker says so.
 */
export function modelOptionOf(session: unknown): AcpAgentAnswer['modelOption'] {
  const result = (typeof session === 'object' && session !== null ? session : {}) as Record<string, unknown>
  const options = Array.isArray(result.configOptions) ? (result.configOptions as unknown[]) : []
  for (const raw of options) {
    const option = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>
    if (option.category !== 'model' && option.id !== 'model') continue
    const id = text(option.id, 100)
    if (id === undefined) continue
    const flat = (Array.isArray(option.options) ? (option.options as unknown[]) : []).flatMap((entry) => {
      const group = (typeof entry === 'object' && entry !== null ? entry : {}) as Record<string, unknown>
      return Array.isArray(group.options) ? (group.options as unknown[]) : [entry]
    })
    const models = flat.flatMap((entry) => {
      const model = (typeof entry === 'object' && entry !== null ? entry : {}) as Record<string, unknown>
      const value = text(model.value) ?? text(model.id)
      return value === undefined ? [] : [{ value, name: text(model.name) ?? value }]
    })
    const current = text(option.currentValue)
    return { id, ...(current === undefined ? {} : { current }), models: models.slice(0, MAX_AGENT_MODELS) }
  }
  return undefined
}

/**
 * What the model picker lists for the agents the person added: each model its Test offered, or, when it offered
 * none, one row that is its own default -- which is exactly what the run is started with.
 */
export function acpCatalog(agents: readonly AcpAgent[]): readonly import('./ipc.js').PublicModel[] {
  return agents.flatMap((agent) => {
    const about = [agent.answer?.agentName, agent.answer?.version].filter((part): part is string => part !== undefined).join(' ')
    const description = `${about.length > 0 ? `${about} · ` : ''}Approve each: every action it asks to take is a card`
    const models = agent.answer?.modelOption?.models ?? []
    if (models.length === 0) {
      return [{ id: acpRouteModel(agent.id), runtime: 'acp' as const, displayName: 'Its own default model', description, supportedEfforts: [], agentName: agent.name }]
    }
    return models.map((model) => ({ id: acpRouteModel(agent.id, model.value), runtime: 'acp' as const, displayName: model.name, description, supportedEfforts: [], agentName: agent.name }))
  })
}
