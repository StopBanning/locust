import type { MissionRuntimeId } from '@teammate/runtime-adapters'

/**
 * Runtime identity, in one place. Every runtime discovery knows how to find is
 * a mission runtime; which of them the host can actually RUN a mission under is
 * a narrower fact, decided by whether its event stream has been measured.
 */

const DISPLAY_NAMES: Readonly<Record<MissionRuntimeId, string>> = {
  codex: 'Codex CLI',
  claude: 'Claude Code',
  cursor: 'Cursor Agent',
  gemini: 'Gemini CLI',
  opencode: 'OpenCode',
  copilot: 'Copilot CLI',
  antigravity: 'Antigravity',
  muse: 'Muse Code',
  kilo: 'Kilo',
  // An agent the person added (0.777): its own name, from Settings, wherever one is known (shared/acp-agents.ts).
  acp: 'Your own agent'
}

export function isMissionRuntime(value: unknown): value is MissionRuntimeId {
  return typeof value === 'string' && Object.hasOwn(DISPLAY_NAMES, value)
}

export function runtimeDisplayName(runtime: MissionRuntimeId): string {
  return DISPLAY_NAMES[runtime]
}

function buildKnownAgentNames(): readonly { readonly name: string; readonly key: string }[] {
  const entries: { readonly name: string; readonly key: string }[] = []
  const seen = new Set<string>()

  const add = (candidate: string): void => {
    const key = candidate.trim().toLowerCase().replace(/\s+/g, '')
    if (key.length > 0 && !seen.has(key)) {
      seen.add(key)
      entries.push({ name: candidate, key })
    }
  }

  // Full display names first (e.g. "Claude Code", "Cursor Agent") so compound names match before single words
  for (const [, displayName] of Object.entries(DISPLAY_NAMES)) {
    add(displayName)
  }
  // Base agent names derived from the first word of each display name (e.g. "Codex", "Claude", "Cursor", "Gemini", "OpenCode", "Copilot", "Antigravity", "Muse")
  for (const [, displayName] of Object.entries(DISPLAY_NAMES)) {
    const base = displayName.split(/\s+/)[0]
    if (base !== undefined && base.length > 0) {
      add(base)
    }
  }
  return entries
}

const KNOWN_AGENT_NAMES = buildKnownAgentNames()

/**
 * When a name matches an AI agent's name (ignoring case and spacing),
 * returns the canonical agent display name; otherwise undefined.
 */
export function agentNamed(name: string | undefined): string | undefined {
  if (typeof name !== 'string') return undefined
  const cleaned = name.trim().toLowerCase().replace(/\s+/g, '')
  if (cleaned.length === 0) return undefined
  for (const candidate of KNOWN_AGENT_NAMES) {
    if (candidate.key === cleaned) {
      return candidate.name
    }
  }
  return undefined
}

/**
 * The runtimes whose event streams the host has a normalizer for, each built
 * from fixtures measured off the real CLI. Gemini CLI is found and signed
 * into like the others, but its stream has never been captured (Google now
 * refuses the CLI to consumer accounts), so a mission under it would be a
 * process whose output nobody can read -- and it is refused, by name, before
 * anything is recorded.
 *
 * Muse Code is here on a narrower measurement than the rest: its stream was
 * captured off `muse exec --provider echo`, which is free and needs no
 * account, so every envelope, the session id and the terminal record are
 * real -- but no tool call under a paying provider has ever been seen. See
 * the note at the top of `muse-events.ts` for what that does and does not
 * establish.
 */
export function hostReadsEventsOf(runtime: MissionRuntimeId): runtime is 'codex' | 'claude' | 'cursor' | 'opencode' | 'copilot' | 'muse' | 'antigravity' | 'acp' | 'kilo' {
  return (
    // Kilo's ACP stream is the same records OpenCode's ACP runner writes.
    runtime === 'kilo'
    // An agent the person added, over the Agent Client Protocol, read as Copilot's is (0.777).
    || runtime === 'acp'
    // Antigravity through its CLI, `agy` (0.540); the runner refuses the app's own server.
    || runtime === 'antigravity'
    || runtime === 'codex'
    || runtime === 'claude'
    || runtime === 'cursor'
    || runtime === 'opencode'
    || runtime === 'copilot'
    || runtime === 'muse'
  )
}

/**
 * Whether the host can own a mission under this runtime at all. Six stream
 * their events through a process the host reads; Antigravity is driven
 * through its own running app and watched through a transcript file, which
 * is a different transport with the same receipts -- so it can run a mission
 * without being one the process runner reads.
 */
export function hostCanRunMission(runtime: MissionRuntimeId): boolean {
  return hostReadsEventsOf(runtime)
}
