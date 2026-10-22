import type { JsonValue } from '@teammate/runtime-adapters'

/**
 * NONE OF YOUR CODEX'S PROGRAMS IN ASK AND PLAN (0.824).
 *
 * MEASURED 2026-10-22 on Codex 0.163.0 under Locust's Ask -- sandbox read-only, approval never -- with an MCP server
 * brought in for one run by `-c` overrides, so nobody's config.toml was touched: a tool that writes a file was refused
 * ("MCP tool call requires approval, but approval policy is never"), but the same tool marked `readOnlyHint: true`
 * ran without asking and wrote it. Codex takes a server's word for which of its tools only read, and a server this
 * computer runs as a program is outside Codex's sandbox. The same day, told to run a command with its shell switched
 * off, Codex ran it through Colin's node_repl instead (`spawnSync('cmd.exe', ...)` from its `js` tool), outside the
 * sandbox and unasked. With `mcp_servers.<name>.enabled = false` for the run, the tool was not there.
 *
 * So a read-only thread starts none of them: each server in the person's own settings that runs as a program is
 * switched off by name, `features.plugins` off takes every server a plugin brings, whatever its name, and Codex's own
 * browser and computer use go too, which act outside the sandbox as well (the voice call's switches,
 * voiceThreadConfig). A server reached at an address -- a connector, a service somewhere else -- stays as their Codex
 * has it: Codex runs the tools it says only read, and refuses the rest. Read from `config/read`, which starts nothing,
 * as the voice call does.
 *
 * Refused, rather than started with something left on, when the settings do not read as settings or a name cannot be
 * written as a key.
 */
const PLAIN_NAME = /^[A-Za-z0-9@_-]{1,128}$/

const UNREAD =
  'Locust could not read your Codex settings, so it did not start this turn. In Ask and Plan it switches off the MCP servers your Codex runs as programs on this computer, and it could not tell which those are. Send it again.'

export type CodexReadOnlyConfig =
  | { readonly ok: true; readonly config: Readonly<Record<string, JsonValue>> }
  | { readonly ok: false; readonly message: string }

export function codexReadOnlyThreadConfig(settings: unknown, options: { readonly sealed?: boolean } = {}): CodexReadOnlyConfig {
  if (typeof settings !== 'object' || settings === null || Array.isArray(settings)) return { ok: false, message: UNREAD }
  const sealed = options.sealed === true
  const config: Record<string, JsonValue> = {
    'features.plugins': false,
    'features.browser_use': false,
    'features.browser_use_external': false,
    'features.in_app_browser': false,
    'features.computer_use': false,
    /*
     * SEALED (0.824, Scout's look): no tool at all. MEASURED 2026-10-22 on Codex 0.163.0, with these and the person's
     * node_repl off for one run, asked to read a file and search the web "by any means": "I could do neither: no
     * file-reading or web-search tool is available in this session." Without `features.apps` it had searched through
     * ChatGPT's apps (`search_service.web_run`), and without node_repl off it had run the command there.
     */
    ...(sealed ? { 'features.apps': false, 'features.shell_tool': false, web_search: 'disabled' } : {})
  }
  const servers = (settings as Record<string, unknown>).mcp_servers
  if (servers === undefined || servers === null) return { ok: true, config }
  if (typeof servers !== 'object' || Array.isArray(servers)) return { ok: false, message: UNREAD }
  for (const [name, entry] of Object.entries(servers as Record<string, unknown>)) {
    const fields = typeof entry === 'object' && entry !== null && !Array.isArray(entry) ? (entry as Record<string, unknown>) : {}
    // A sealed thread keeps no connector either.
    if (!sealed && typeof fields.url === 'string' && fields.command === undefined) continue
    if (!PLAIN_NAME.test(name)) {
      return { ok: false, message: `Locust could not switch off your Codex MCP server "${name.slice(0, 60)}" for this turn, so it did not start it. In Ask and Plan it starts none of the servers your Codex runs as programs on this computer.` }
    }
    config[`mcp_servers.${name}.enabled`] = false
  }
  return { ok: true, config }
}

/** A sealed run's thread (0.824, Scout's look): no tool at all, no server of the person's, no connector. */
export const codexSealedThreadConfig = (settings: unknown): CodexReadOnlyConfig => codexReadOnlyThreadConfig(settings, { sealed: true })
