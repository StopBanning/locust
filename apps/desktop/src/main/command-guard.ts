import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

/**
 * NO TEAMMATE ENDS YOUR BROWSER (0.717): the settings every Claude Code run
 * is launched with (`--settings`), naming Locust's command guard as a
 * PreToolUse hook on the shell tools. resources/locust-command-guard.mjs says
 * what it refuses, and why.
 *
 * Claude Code runs a hook's command in its own shell -- Git Bash on Windows,
 * MEASURED 2026-10-10 on 2.1.296 -- so the command is written for a POSIX
 * shell: Electron's binary, run as node by ELECTRON_RUN_AS_NODE (guaranteed
 * present, as the permission bridge's is), on the script beside app.asar.
 * Forward slashes: Windows takes them, and a backslash means something to a
 * shell. `--settings` applies under `--restricted` too (Claude Code's help), so
 * every mode gets it, Auto included.
 *
 * LOCUST_GUARD_PARENT (0.718) is this Locust's own process id: a process the
 * run started descends from the agent Locust started for it, whose parent is
 * Locust, and the guard ends only those by id.
 */
export function commandGuardSettings(options: { readonly node: string; readonly guardPath: string; readonly parent: number }): string {
  const quoted = (path: string): string => `"${path.replace(/\\/g, '/').replace(/(["$`])/g, '\\$1')}"`
  const parent = Number.isInteger(options.parent) && options.parent > 0 ? options.parent : 0
  return JSON.stringify(
    {
      hooks: {
        PreToolUse: [
          {
            matcher: 'Bash|PowerShell',
            hooks: [{ type: 'command', command: `ELECTRON_RUN_AS_NODE=1 LOCUST_GUARD_PARENT=${String(parent)} ${quoted(options.node)} ${quoted(options.guardPath)}`, timeout: 15 }]
          }
        ]
      }
    },
    null,
    2
  )
}

/**
 * THE SAME GUARD FOR CODEX (0.720): the config a Codex thread is started,
 * resumed or forked with (`thread/start`'s `config`), naming the same script
 * as a PreToolUse hook on its shell tool.
 *
 * MEASURED 2026-10-10 on Codex 0.162.1, gpt-6-luna, app-server:
 * - Codex hands the hook Claude Code's own event (`tool_name: "Bash"`,
 *   `tool_input.command`) and honours its `permissionDecision: "deny"`: the
 *   command never ran and the model read the reason.
 * - On Windows it runs the hook as `powershell.exe -NoProfile -Command`, so
 *   the POSIX `NAME=value cmd` form and a bare quoted path both did nothing
 *   there; `$env:NAME='value'; & 'program' 'script'` ran. Elsewhere the
 *   command is the same as Claude Code's (not measured on a Mac).
 * - Codex runs only hooks the person has reviewed, and that review is kept in
 *   their config.toml, which Locust never writes. `bypass_hook_trust` runs
 *   this thread's hooks without it; without it the hook never ran. It skips
 *   the review for EVERY hook the thread loads, so it is only ever sent when
 *   Locust's is the only one there is: see `otherCodexHooks`.
 */
export function codexCommandGuardConfig(options: {
  readonly node: string
  readonly guardPath: string
  readonly parent: number
  readonly platform?: NodeJS.Platform
}): { readonly bypass_hook_trust: true; readonly hooks: { readonly PreToolUse: readonly unknown[] } } {
  return {
    bypass_hook_trust: true,
    hooks: { PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: guardCommand(options), timeout: 15 }] }] }
  }
}

/**
 * The guard's command line: PowerShell's form on Windows, a POSIX shell's
 * elsewhere. `log` is a drive's LOCUST_GUARD_LOG, named in the command because
 * a runtime the process runner starts inherits only its allowlist.
 */
function guardCommand(options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly platform?: NodeJS.Platform; readonly log?: string }): string {
  const parent = Number.isInteger(options.parent) && options.parent > 0 ? options.parent : 0
  const forward = (path: string): string => path.replace(/\\/g, '/')
  const log = options.log === undefined || options.log.length === 0 ? undefined : forward(options.log)
  return (options.platform ?? process.platform) === 'win32'
    ? `$env:ELECTRON_RUN_AS_NODE='1'; $env:LOCUST_GUARD_PARENT='${String(parent)}'; ${log === undefined ? '' : `$env:LOCUST_GUARD_LOG='${log.replace(/'/g, "''")}'; `}& ${[options.node, options.guardPath].map((path) => `'${forward(path).replace(/'/g, "''")}'`).join(' ')}`
    : `ELECTRON_RUN_AS_NODE=1 LOCUST_GUARD_PARENT=${String(parent)} ${log === undefined ? '' : `LOCUST_GUARD_LOG="${log.replace(/(["$`])/g, '\\$1')}" `}${[options.node, options.guardPath].map((path) => `"${forward(path).replace(/(["$`])/g, '\\$1')}"`).join(' ')}`
}

/**
 * THE SAME GUARD FOR COPILOT (0.721): a plugin folder every Copilot run is
 * given (`--plugin-dir`), in Claude Code's plugin format, naming the same
 * script as a PreToolUse hook.
 *
 * MEASURED 2026-10-10 on Copilot CLI 1.0.95 (Auto, which ran gpt-6-luna):
 * - A `.claude-plugin/plugin.json` with `hooks/hooks.json` beside it loads
 *   from `--plugin-dir` in a folder never trusted, in `-p` and in `--acp`
 *   alike. Copilot hands the hook Claude Code's own event (`tool_name:
 *   "Bash"` for its PowerShell tool, `tool_input.command`), and the matcher
 *   `Bash|PowerShell` takes it. A `permissionDecision: "deny"` stops the
 *   command ("Denied by preToolUse hook: <reason>"); over ACP it is refused
 *   before anyone is asked to approve it.
 * - On Windows it runs the hook as `powershell.exe -nop -nol -c`, as Codex
 *   does, so the command is Codex's.
 * - UNLIKE Claude Code and Codex, Copilot refuses the command when a hook
 *   fails: a hook that exited 1, or whose program was not there, denied
 *   `echo` ("hook errored"). A guard Locust could not start would stop every
 *   command a Copilot teammate runs. So the command cannot fail: PowerShell's
 *   `try {} catch {}; exit 0`, measured letting `echo` run with the program
 *   missing; `|| true` elsewhere (not measured on a Mac). The guard itself
 *   always exits 0, and refuses only by what it prints.
 * - Copilot keeps an empty folder per plugin path in ~/.copilot/plugin-data;
 *   nothing else of the person's is written.
 */
export function copilotCommandGuardHooks(options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly platform?: NodeJS.Platform; readonly log?: string }): string {
  const command = guardCommand(options)
  return JSON.stringify(
    {
      hooks: {
        PreToolUse: [
          {
            matcher: 'Bash|PowerShell',
            hooks: [{ type: 'command', command: (options.platform ?? process.platform) === 'win32' ? `try { ${command} } catch {}; exit 0` : `${command} || true`, timeout: 15 }]
          }
        ]
      }
    },
    null,
    2
  )
}

/**
 * THE SAME GUARD FOR OPENCODE (0.722): a plugin file every OpenCode run is
 * told to load (`plugin` in OPENCODE_CONFIG_CONTENT, as a file URL), whose
 * `tool.execute.before` asks the same script about each `bash` command.
 *
 * MEASURED 2026-10-10 on OpenCode 1.18.27, nemotron-3-ultra-free, with
 * Locust's own edit-mode config:
 * - The plugin is handed `{ tool: "bash" }` and `{ args: { command } }`
 *   before the command runs; an Error it throws stops the command, and the
 *   model reads the Error's message as the tool's result.
 * - A plugin file that is missing is skipped, and one that throws as it loads
 *   is logged ("failed to load plugin") and skipped: the run goes on either
 *   way. So this one may fail open too, and does: anything but a refusal
 *   from the guard lets the command run.
 * - A `plugin` list in OPENCODE_CONFIG_CONTENT is added to the person's own,
 *   not put in its place: a plugin named in another config ran beside it.
 *
 * It runs inside OpenCode's process, so the guard runs as its own process,
 * as every runtime's hook does, and without blocking: `opencode serve` holds
 * other sessions on the same event loop.
 */
export function openCodeCommandGuardPlugin(options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly log?: string }): string {
  const parent = Number.isInteger(options.parent) && options.parent > 0 ? options.parent : 0
  const env = { ELECTRON_RUN_AS_NODE: '1', LOCUST_GUARD_PARENT: String(parent), ...(options.log === undefined || options.log.length === 0 ? {} : { LOCUST_GUARD_LOG: options.log }) }
  return [
    '// Written by Locust (command-guard.ts): asks Locust\'s command guard before each command a teammate runs.',
    "import { spawn } from 'node:child_process'",
    '',
    `const NODE = ${JSON.stringify(options.node)}`,
    `const GUARD = ${JSON.stringify(options.guardPath)}`,
    `const ENV = ${JSON.stringify(env)}`,
    '',
    '/** The guard\'s reason for refusing this command, or undefined: anything but a refusal lets it run. */',
    'const refusal = (command, cwd, workdir) =>',
    '  new Promise((resolve) => {',
    '    let out = \'\'',
    '    let child',
    '    try {',
    "      child = spawn(NODE, [GUARD], { stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true, env: { ...process.env, ...ENV } })",
    '    } catch {',
    '      return resolve(undefined)',
    '    }',
    '    const timer = setTimeout(() => {',
    '      child.kill()',
    '      resolve(undefined)',
    '    }, 15000)',
    "    child.on('error', () => resolve(undefined))",
    "    child.stdout.setEncoding('utf8')",
    "    child.stdout.on('data', (chunk) => (out += chunk))",
    "    child.on('close', () => {",
    '      clearTimeout(timer)',
    '      try {',
    '        const answer = JSON.parse(out)?.hookSpecificOutput',
    "        resolve(answer?.permissionDecision === 'deny' && typeof answer.permissionDecisionReason === 'string' ? answer.permissionDecisionReason : undefined)",
    '      } catch {',
    '        resolve(undefined)',
    '      }',
    '    })',
    "    child.stdin.on('error', () => undefined)",
    "    child.stdin.end(JSON.stringify({ hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command, workdir }, cwd }))",
    '  })',
    '',
    '// The folder this plugin was loaded for, and the one the command names: a fix\'s copy refuses what would publish (0.828).',
    'export const LocustCommandGuard = async (context) => ({',
    "  'tool.execute.before': async (input, output) => {",
    "    const command = input?.tool === 'bash' ? output?.args?.command : undefined",
    "    if (typeof command !== 'string') return",
    '    const reason = await refusal(command, context?.directory, output?.args?.workdir)',
    '    if (reason !== undefined) throw new Error(reason)',
    '  }',
    '})',
    ''
  ].join('\n')
}

/** OpenCode's plugin file, written once at start in the profile, as the file URL a config names; undefined if it could not be written. */
export async function writeOpenCodeCommandGuard(folder: string, options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly log?: string }): Promise<string | undefined> {
  try {
    await mkdir(folder, { recursive: true })
    const path = join(folder, 'opencode-command-guard.js')
    await writeFile(path, openCodeCommandGuardPlugin(options), 'utf8')
    return pathToFileURL(path).href
  } catch {
    return undefined
  }
}

/**
 * OpenCode 2 loads a default plugin definition and registers hooks in setup;
 * the V1 hook object is not accepted by that API. Keep the entrypoints separate
 * so writing a V2 plugin never replaces a V1 run's plugin. Both hooks ask the
 * existing guard about shell commands: tool execution and shells created by
 * plugins. As on V1, only an explicit refusal denies a command; this is not a
 * sandbox. A broken guard is bounded and lets the command proceed.
 */
export function openCode2CommandGuardPlugin(options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly log?: string }): string {
  const parent = Number.isInteger(options.parent) && options.parent > 0 ? options.parent : 0
  const env = { ELECTRON_RUN_AS_NODE: '1', LOCUST_GUARD_PARENT: String(parent), ...(options.log === undefined || options.log.length === 0 ? {} : { LOCUST_GUARD_LOG: options.log }) }
  return [
    '// Written by Locust (command-guard.ts): OpenCode 2 asks the same command guard as the other agents.',
    "import { spawn } from 'node:child_process'",
    '',
    `const NODE = ${JSON.stringify(options.node)}`,
    `const GUARD = ${JSON.stringify(options.guardPath)}`,
    `const ENV = ${JSON.stringify(env)}`,
    '',
    'const refusal = (command, cwd, workdir) =>',
    '  new Promise((resolve) => {',
    "    let out = ''",
    '    let bytes = 0',
    '    let child',
    '    let timer',
    '    let settled = false',
    '    const finish = (reason, stop = false) => {',
    '      if (settled) return',
    '      settled = true',
    '      clearTimeout(timer)',
    '      if (stop) {',
    '        try { child?.kill() } catch {}',
    '      }',
    '      resolve(reason)',
    '    }',
    '    try {',
    "      child = spawn(NODE, [GUARD], { stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true, env: { ...process.env, ...ENV } })",
    '    } catch {',
    '      return finish(undefined)',
    '    }',
    '    timer = setTimeout(() => finish(undefined, true), 15000)',
    "    child.once('error', () => finish(undefined, true))",
    "    child.stdout.once('error', () => finish(undefined, true))",
    "    child.stdout.setEncoding('utf8')",
    "    child.stdout.on('data', (chunk) => {",
    '      if (settled) return',
    '      bytes += Buffer.byteLength(chunk)',
    '      if (bytes > 64 * 1024) return finish(undefined, true)',
    '      out += chunk',
    '    })',
    "    child.once('close', () => {",
    '      if (settled) return',
    '      try {',
    '        const answer = JSON.parse(out)?.hookSpecificOutput',
    "        finish(answer?.permissionDecision === 'deny' && typeof answer.permissionDecisionReason === 'string' ? answer.permissionDecisionReason : undefined)",
    '      } catch {',
    '        finish(undefined)',
    '      }',
    '    })',
    "    child.stdin.on('error', () => finish(undefined, true))",
    '    try {',
    "      child.stdin.end(JSON.stringify({ hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command, workdir }, cwd }))",
    '    } catch {',
    '      finish(undefined, true)',
    '    }',
    '  })',
    '',
    'const check = async (command, cwd, workdir) => {',
    "  if (typeof command !== 'string') return",
    '  const reason = await refusal(command, cwd, workdir)',
    '  if (reason !== undefined) throw new Error(reason)',
    '}',
    '',
    'export default {',
    "  id: 'locust.command-guard',",
    '  async setup(ctx) {',
    '    // The folder it runs for: a fix\'s copy refuses what would publish (0.828).',
    '    const folder = ctx?.directory ?? ctx?.project?.directory ?? ctx?.worktree',
    "    await ctx.tool.hook('execute.before', (event) => check(event?.tool === 'shell' || event?.tool === 'bash' ? event.input?.command : undefined, folder, event?.input?.workdir))",
    "    await ctx.shell.hook('create.before', (event) => check(event?.command, folder, event?.cwd))",
    '  }',
    '}',
    ''
  ].join('\n')
}

/** V2 configured plugins must be directories; its loader resolves this package's index. */
export async function writeOpenCode2CommandGuard(folder: string, options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly log?: string }): Promise<string | undefined> {
  try {
    const plugin = join(folder, 'opencode-2-command-guard')
    await mkdir(plugin, { recursive: true })
    await mkdir(join(plugin, 'config'), { recursive: true })
    await writeFile(join(plugin, 'index.js'), openCode2CommandGuardPlugin(options), 'utf8')
    await writeFile(join(plugin, 'package.json'), `${JSON.stringify({ name: 'locust-command-guard', private: true, type: 'module', main: './index.js', exports: './index.js' }, null, 2)}\n`, 'utf8')
    return pathToFileURL(plugin).href
  } catch {
    return undefined
  }
}

/** Copilot's plugin folder, written once at start in the profile; undefined if it could not be, and runs go without it. */
export async function writeCopilotCommandGuard(folder: string, options: { readonly node: string; readonly guardPath: string; readonly parent: number; readonly log?: string }): Promise<string | undefined> {
  try {
    const plugin = join(folder, 'copilot-command-guard')
    await mkdir(join(plugin, '.claude-plugin'), { recursive: true })
    await mkdir(join(plugin, 'hooks'), { recursive: true })
    await writeFile(join(plugin, '.claude-plugin', 'plugin.json'), `${JSON.stringify({ name: 'locust-command-guard', version: '1.0.0', description: 'Locust refuses a command that would end a program the person runs.' }, null, 2)}\n`, 'utf8')
    await writeFile(join(plugin, 'hooks', 'hooks.json'), copilotCommandGuardHooks(options), 'utf8')
    return plugin
  } catch {
    return undefined
  }
}

/**
 * Every hook Codex might load for a run besides Locust's own, by where Codex
 * keeps them: the person's config (`hooks` in any `.toml` in CODEX_HOME, which
 * is also where a managed hooks folder is named), a `hooks.json` in CODEX_HOME
 * or its `hooks/`, an installed plugin that brings hooks, and a project's
 * `.codex/` in the folder or any folder above it. Any of them, or any that
 * cannot be read, and the run goes without the guard rather than run
 * someone's unreviewed hook. Erring toward finding one costs only the guard.
 *
 * Except the plugins Codex ships itself (`plugins/cache/openai-bundled`):
 * MEASURED 2026-10-10, its browser, Chrome and computer-use plugins each
 * declare hooks (an `mcp_tool` telling its own REPL a turn ended) and are on
 * by default, so counting them would leave nearly every Codex without the
 * guard. Only installed plugins are read: dot-folders under `plugins/` are
 * marketplace copies and half-finished installs, and the rest of CODEX_HOME
 * (sessions, worktrees: 59,000 folders here, 30 s) holds no hooks.
 */
export async function otherCodexHooks(options: { readonly codexHome: string; readonly folder: string }): Promise<readonly string[]> {
  const found: string[] = []
  const names = async (folder: string): Promise<readonly import('node:fs').Dirent[] | undefined> => {
    try {
      return await readdir(folder, { withFileTypes: true })
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') found.push(`${folder} (could not be read)`)
      return undefined
    }
  }
  const namesHooks = async (path: string): Promise<boolean> => {
    try {
      return /^\s*\[\s*"?hooks\b|^\s*"?hooks"?\s*[.=]/m.test(await readFile(path, 'utf8'))
    } catch {
      found.push(`${path} (could not be read)`)
      return false
    }
  }
  for (const entry of (await names(options.codexHome)) ?? []) {
    if (entry.isFile() && entry.name.endsWith('.toml') && (await namesHooks(join(options.codexHome, entry.name)))) found.push(join(options.codexHome, entry.name))
  }
  for (const folder of [options.codexHome, join(options.codexHome, 'hooks')]) {
    for (const entry of (await names(folder)) ?? []) if (entry.isFile() && entry.name === 'hooks.json') found.push(join(folder, entry.name))
  }
  const plugins = join(options.codexHome, 'plugins')
  const bundled = join(plugins, 'cache', 'openai-bundled')
  const walk = async (folder: string, depth: number): Promise<void> => {
    if (depth > 8 || resolve(folder) === resolve(bundled)) return
    for (const entry of (await names(folder)) ?? []) {
      const path = join(folder, entry.name)
      if (entry.isFile() && entry.name === 'hooks.json') found.push(path)
      else if (entry.isFile() && entry.name === 'plugin.json') {
        try {
          const declared = (JSON.parse(await readFile(path, 'utf8')) as { hooks?: unknown }).hooks
          if (declared !== undefined && declared !== null && !(typeof declared === 'object' && Object.keys(declared).length === 0)) found.push(path)
        } catch {
          found.push(`${path} (could not be read)`)
        }
      } else if (entry.isDirectory() && (!entry.name.startsWith('.') || entry.name === '.codex-plugin' || entry.name === '.claude-plugin')) await walk(path, depth + 1)
    }
  }
  await walk(plugins, 0)
  for (let folder = resolve(options.folder), step = 0; step < 64; step += 1) {
    const project = join(folder, '.codex')
    if (resolve(project) !== resolve(options.codexHome)) {
      for (const entry of (await names(project)) ?? []) {
        if (entry.isFile() && entry.name === 'hooks.json') found.push(join(project, entry.name))
        if (entry.isFile() && entry.name.endsWith('.toml') && (await namesHooks(join(project, entry.name)))) found.push(join(project, entry.name))
      }
    }
    const up = dirname(folder)
    if (up === folder) break
    folder = up
  }
  return found
}

/** Written once at start, in the profile; undefined if it could not be, and runs go without it. */
export async function writeCommandGuard(folder: string, options: { readonly node: string; readonly guardPath: string; readonly parent: number }): Promise<string | undefined> {
  try {
    await mkdir(folder, { recursive: true })
    const path = join(folder, 'command-guard.json')
    await writeFile(path, commandGuardSettings(options), 'utf8')
    return path
  } catch {
    return undefined
  }
}
