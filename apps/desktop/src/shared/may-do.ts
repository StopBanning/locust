import type { MissionRuntimeId, MissionSandbox } from '@teammate/runtime-adapters'

import type { MissionMode } from './ipc.js'
import { runtimeDisplayName } from './runtimes.js'

/**
 * WHAT A RUN MAY DO, runtime by runtime (0.359).
 *
 * The Activity panel said the same three rows for every runtime, and the
 * third was false for the one a new person meets first. A Research & money
 * teammate on OpenCode's free model searched the web twice, and the panel
 * listing those two searches said, under them, "deny: ... any network access
 * beyond the model's own" (the first-session drive, packaged 0.358). OpenCode
 * is given no rule about the web at all, so its own search and fetch tools
 * run. The sentence had been "any network the runtime does not make itself"
 * until 0.355 shortened it into something untrue.
 *
 * So every row here is a thing Locust hands THAT runtime in THAT mode, read
 * off the command it builds (packages/runtime-adapters/src/commands.ts), and
 * nothing else. What Locust does not set is not guessed at: the last line
 * says it is left to the runtime's own settings. The rows are checked
 * against the real command builders in what-it-may-do-is-what-it-was-given.
 */
export interface MayDoRow {
  readonly verdict: 'allow' | 'deny'
  /** It reaches past this folder: amber rather than green. */
  readonly wide?: true
  readonly text: string
}

export interface MayDo {
  readonly rows: readonly MayDoRow[]
  /** What decides everything the rows do not name. */
  readonly rest: string
}

const READ: MayDoRow = { verdict: 'allow', text: 'read the files in this folder' }
const NO_CHANGES: MayDoRow = { verdict: 'deny', text: 'changing any file' }
const CHANGES_HERE: MayDoRow = { verdict: 'allow', text: 'change files in this folder' }
const CHANGES_ANYWHERE: MayDoRow = { verdict: 'allow', wide: true, text: 'change files anywhere your account can reach, in this folder and outside it' }
const ANY_COMMAND: MayDoRow = { verdict: 'allow', wide: true, text: 'run any command your account can run, and reach any network' }
const NO_COMMANDS: MayDoRow = { verdict: 'deny', text: 'running commands' }
/** A command with nothing around it but the person's own account. */
const OPEN_COMMANDS: MayDoRow = { verdict: 'allow', wide: true, text: 'run commands, which can reach anything your account can' }
const THE_WEB: MayDoRow = { verdict: 'allow', wide: true, text: 'search the web and open web pages' }
const CLAUDE_SKILLS: MayDoRow = { verdict: 'allow', text: "use this folder's skills, yours when Settings lends them, and the ones you kept from GitHub, with only the tools listed here" }
const NOT_OUTSIDE: MayDoRow = { verdict: 'deny', text: 'opening files outside this folder, other than by running a command' }
/** A runtime whose own web tools ask, and so are refused in a run nobody can answer. */
const webOnlyInAuto = (runtime: MissionRuntimeId): MayDoRow => ({
  verdict: 'deny',
  text: `searching the web or opening web pages, which ${runtimeDisplayName(runtime)} allows here only in Auto`
})

/**
 * The rows for one run. `sandbox` is what the run was started with, read
 * from its start receipt -- never the mode the composer shows now -- and
 * `mode` says whether it was Approve-each, which shares Edit's sandbox and
 * stops before each act instead.
 */
export function whatItMayDo(runtime: MissionRuntimeId, sandbox: MissionSandbox, mode?: MissionMode): MayDo {
  const rest = `Anything not listed is left to ${runtimeDisplayName(runtime)}'s own settings.`
  const files = sandbox === 'read-only' ? NO_CHANGES : sandbox === 'workspace-write' ? CHANGES_HERE : CHANGES_ANYWHERE
  const rows = (...own: readonly MayDoRow[]): MayDo => ({ rows: [READ, files, ...own], rest })
  const asks = mode === 'approve-each' && sandbox === 'workspace-write'
  switch (runtime) {
    case 'codex':
      // The sandbox is Codex's own, named in every thread/start: read-only,
      // workspace-write, or danger-full-access for Auto. Approve-each is
      // workspace-write with approval "untrusted", which runs a plain read
      // unasked and asks before anything else (codexAppServerPolicy).
      // Its own web_search tool is left at Codex's default, which is on, and
      // never asks: MEASURED 2026-10-09 on 0.162.0
      // (_tools/probe-codex-web-search.mjs), all four modes opened
      // releases.electronjs.org and no approval request came.
      if (sandbox === 'full-access') return rows(ANY_COMMAND, THE_WEB)
      if (asks) {
        return {
          rows: [
            READ,
            { verdict: 'allow', text: 'change files in this folder, once you approve each change' },
            { verdict: 'allow', text: "run commands in Codex's sandbox, asking you before any that does more than read" },
            { verdict: 'allow', wide: true, text: 'search the web and open web pages, without asking' }
          ],
          rest
        }
      }
      // Read-only also starts none of the MCP servers or plugins the person's Codex runs as programs (0.824,
      // codexReadOnlyThreadConfig): Codex runs a server's tool here when the server says it only reads.
      return rows(
        {
          verdict: 'allow',
          text: sandbox === 'read-only' ? "run commands in Codex's sandbox, which lets them read and change nothing" : "run commands in Codex's sandbox, which keeps their changes in this folder"
        },
        THE_WEB,
        ...(sandbox === 'read-only' ? [{ verdict: 'deny' as const, text: 'starting the MCP servers and plugins your Codex runs as programs on this computer' }] : [])
      )
    case 'claude':
      // `--tools` is the whole list: Bash only when the mode may edit;
      // WebSearch and WebFetch in every mode (0.711), asked about through the
      // card outside Auto (createClaudePrintCommand, permission-host.ts).
      // Skill, with the folder's skills (and the person's own, when Settings
      // lends them) handed over as plugins (0.679, claude-skills.ts). A skill
      // is instructions: it uses only the tools named above.
      if (sandbox === 'full-access') return rows(ANY_COMMAND, THE_WEB)
      return rows(
        sandbox === 'read-only' ? NO_COMMANDS : OPEN_COMMANDS,
        { verdict: 'allow', wide: true, text: 'search the web and open web pages, once you approve each one' },
        CLAUDE_SKILLS
      )
    case 'opencode':
      // Read-only: edit, write and patch denied, bash "ask" -- which `run`
      // answers with a refusal every time -- and since 0.821 every tool not
      // named asks too, first, so a person's own MCP tools are refused; the
      // reading tools and the web's are allowed by name after it. Edit:
      // external_directory "deny" and nothing else, so bash, websearch,
      // webfetch and MCP tools run as OpenCode's own defaults have them.
      // Auto: `--auto` and external_directory "allow".
      if (sandbox === 'full-access') return rows(ANY_COMMAND)
      if (asks) {
        // `opencode serve` (createOpenCodeServeCommand): everything not named
        // "ask" (0.821), so edit, bash, webfetch, external_directory and a
        // person's own MCP tools ask; websearch is allowed by name.
        return {
          rows: [
            READ,
            { verdict: 'allow', text: 'change files in this folder, once you approve each change' },
            { verdict: 'allow', wide: true, text: 'run commands and open web pages, once you approve each one' },
            { verdict: 'allow', wide: true, text: 'search the web' },
            { verdict: 'allow', wide: true, text: 'open files outside this folder, once you approve it' },
            { verdict: 'allow', wide: true, text: 'use tools from your own MCP servers, once you approve each one' }
          ],
          rest
        }
      }
      if (sandbox === 'read-only') return rows(NO_COMMANDS, THE_WEB, NOT_OUTSIDE, { verdict: 'deny', text: 'using tools from your own MCP servers' })
      return rows(OPEN_COMMANDS, THE_WEB, NOT_OUTSIDE)
    case 'copilot':
      // `--allow-all-tools` in every mode; read-only adds
      // `--deny-tool=write,shell`, and only Auto adds `--allow-all-paths`,
      // so file paths outside the folder are refused otherwise. Its web_fetch
      // is one of "all tools": MEASURED 2026-10-09
      // (_tools/probe-web-search-each-runtime.mjs), every mode fetched
      // releases.electronjs.org.
      if (sandbox === 'full-access') return rows(ANY_COMMAND, THE_WEB)
      // Read-only also leaves off every MCP server in the person's own Copilot settings (0.824, --disable-mcp-server).
      if (sandbox === 'read-only') return rows(NO_COMMANDS, THE_WEB, { verdict: 'deny', text: 'starting the MCP servers in your Copilot settings' })
      return rows(OPEN_COMMANDS, THE_WEB, NOT_OUTSIDE)
    case 'acp':
      // An agent the person added (0.777) runs only in Approve each, and Locust offers it no files and no terminal of
      // its own: it acts with its own tools and asks over the protocol first. WHICH acts it asks about is the agent's
      // choice -- the protocol does not make it ask before reading -- so that is said rather than promised.
      return {
        rows: [READ, { verdict: 'allow', text: "change files and run commands when it asks and you approve; which actions it asks about is the agent's own choice" }],
        rest
      }
    case 'muse':
      // Read-only: `--disable-write --disable-shell`. Edit: approval `never`
      // with Muse's sandbox left on ("shell filesystem/network sandboxing",
      // `muse exec --help`). Auto adds `--disable-sandbox` (0.773). Web tools
      // are on unless `--disable-web-tools`, which is never passed; only a
      // mode that never asks is sure to use them.
      if (sandbox === 'read-only') return rows(NO_COMMANDS)
      if (sandbox === 'full-access') return rows(ANY_COMMAND, THE_WEB)
      return { rows: [READ, CHANGES_HERE, { verdict: 'allow', text: "run commands inside Muse's sandbox" }, THE_WEB], rest }
    case 'cursor':
      // Only Auto says anything about commands (`--force`). Read-only is
      // Cursor's own ask mode, with its sandbox where one runs (macOS, Linux).
      // Its webSearch and webFetch ask, and a print run has nobody to answer,
      // so outside Auto both are refused. MEASURED 2026-10-09
      // (probe-web-search-each-runtime): a `WebFetch(*)` rule in the folder's
      // `.cursor/cli.json` did not change that; only `--force` or the
      // person's own global `autoAcceptWebSearch` would.
      // Read-only also leaves out `--approve-mcps` (0.823), so a server the
      // folder's own `.cursor/mcp.json` names is never started.
      if (sandbox === 'read-only') return rows(webOnlyInAuto(runtime), { verdict: 'deny', text: 'starting the MCP servers this folder names' })
      return sandbox === 'full-access' ? rows(ANY_COMMAND, THE_WEB) : rows(webOnlyInAuto(runtime))
    case 'antigravity':
      // Through its CLI (0.540, measured): read-only refuses writes and
      // commands, Edit allows file edits and still refuses commands, and only
      // Auto lets it run them. Its read_url is refused the same way outside
      // Auto (MEASURED 2026-10-09): an allow rule would have to go in the
      // person's own global settings, and no folder file stands in for it.
      return sandbox === 'full-access' ? rows(ANY_COMMAND, THE_WEB) : rows(webOnlyInAuto(runtime))
    case 'gemini':
      // Gemini CLI is refused before anything runs. Nothing to add to the mode itself.
      return rows()
  }
}
