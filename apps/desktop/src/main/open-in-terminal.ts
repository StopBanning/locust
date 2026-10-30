import { spawn } from 'node:child_process'
import { win32 } from 'node:path'

import type { ExecutableLaunch, MissionRuntimeId } from '@teammate/runtime-adapters'

import type { OpenInTerminalResponse } from '../shared/ipc.js'
import { runtimeDisplayName } from '../shared/runtimes.js'
import { consoleCommandLine } from './runtime-sign-in.js'
import { openInScriptTerminal, opensAScriptTerminal } from './script-terminal.js'
export { macCommandScript } from './mac-terminal.js'

/**
 * OPEN A CONVERSATION IN THE RUNTIME'S OWN TERMINAL (0.387).
 *
 * Colin, 2026-09-26, looking at Orca, which runs every agent in a terminal
 * pane: "do you think we should maybe give an option in the chat to give the
 * user a terminal option for that model or no?" -- then, on the answer, "we
 * can either ship it in the three dot dropdown or have it more visible, or do
 * like a coding terminal button". Not a terminal INSIDE the conversation:
 * that is Orca's product, and a raw terminal skips Locust's approval cards,
 * spend limits and record. An escape hatch instead: the same session, in the
 * runtime's own interface, in the teammate's folder -- and honest that Locust
 * does not see what happens there.
 *
 * The session is the conversation's own (the host reads it from the ledger;
 * the window only names a mission), resumed with each CLI's own flag, as its
 * `--help` gave it on 2026-09-26:
 *
 *   codex 0.157.1      codex resume <SESSION_ID>
 *   claude 2.1.283     claude --resume <session-id>
 *   copilot 1.0.88     copilot --resume=<session-id>   (its own example's form)
 *   cursor-agent       cursor-agent --resume=<chatId>  (an optional value)
 *   opencode 1.18.27   opencode --session <id>
 *   muse 1.4.0         muse resume <session-ref>
 *   agy 1.2.14         agy --conversation <id>   (Antigravity CLI, 0.543)
 *
 * Antigravity is an app with a window of its own, and Gemini CLI does not run
 * here at all: neither is offered.
 *
 * Started the way the sign-in window is (runtime-sign-in.ts) -- no prompt,
 * no model text, only the program discovery found and a session id checked to
 * be a plain token -- in Windows Terminal when it is there, since these TUIs
 * are drawn for it, else in a console window of its own.
 */

/** A session id as the runtimes write them: UUIDs, `ses_...`, names. Never an option, never a shell word. */
export function isResumableSessionId(id: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9._:-]{0,199}$/.test(id)
}

/** The words that resume `sessionId` in `runtime`'s own interface, or undefined where there are none. */
export function resumeArgsFor(runtime: string, sessionId: string): readonly string[] | undefined {
  if (!isResumableSessionId(sessionId)) return undefined
  switch (runtime) {
    case 'codex':
      return ['resume', sessionId]
    case 'claude':
      return ['--resume', sessionId]
    case 'copilot':
      return [`--resume=${sessionId}`]
    case 'cursor':
      return [`--resume=${sessionId}`]
    case 'opencode':
      return ['--session', sessionId]
    case 'muse':
      return ['resume', sessionId]
    case 'antigravity':
      return ['--conversation', sessionId]
    default:
      return undefined
  }
}

/**
 * The program a terminal should start, and its words.
 *
 * A `.cmd` launcher the locator runs as `cmd.exe /d /s /c <launcher>` is
 * started as the launcher itself: a terminal runs a batch file through cmd on
 * its own, and wrapping it again only adds a layer of quoting to get wrong.
 */
export function terminalProgram(launch: ExecutableLaunch, args: readonly string[]): { readonly file: string; readonly args: readonly string[] } {
  const launcher = launch.prefixArgs[3]
  const throughCmd =
    /(^|[\\/])cmd\.exe$/i.test(launch.executablePath)
    && launch.prefixArgs[0] === '/d'
    && launch.prefixArgs[1] === '/s'
    && launch.prefixArgs[2] === '/c'
    && launcher !== undefined
  return throughCmd ? { file: launcher, args: [...launch.prefixArgs.slice(4), ...args] } : { file: launch.executablePath, args: [...launch.prefixArgs, ...args] }
}

/**
 * Windows Terminal's command line, or undefined when it cannot carry this one.
 *
 * MEASURED 2026-09-26 (wt 1.24): a program under a path with a space, a
 * script argument with spaces and quotes, and `--resume=<id>` to a `.cmd` in
 * a folder with a space all arrived intact, in the folder `-d` named. The one
 * thing wt reads for itself is `;`, which separates its own commands -- so a
 * line with one goes to the console window instead.
 */
export function windowsTerminalArgs(program: { readonly file: string; readonly args: readonly string[] }, cwd: string, title: string): readonly string[] | undefined {
  if ([program.file, ...program.args, cwd, title].some((part) => part.includes(';'))) return undefined
  return ['-w', 'new', 'new-tab', '--title', title, '-d', cwd, program.file, ...program.args]
}

/** The console window's cmd.exe line (runtime-sign-in's shape): every word quoted but the checked session words. */
export function consoleLine(program: { readonly file: string; readonly args: readonly string[] }, resumeArgs: readonly string[]): string {
  const leading = program.args.slice(0, program.args.length - resumeArgs.length).map((part) => `"${part}"`)
  return consoleCommandLine(program.file, [...leading, ...resumeArgs])
}

export interface TerminalRequest {
  readonly account?: string
  readonly runtime: string
  readonly sessionId: string
  readonly cwd: string
  readonly launch: ExecutableLaunch
  /** The window's title: whose conversation, on what. */
  readonly title: string
  /** W5: teleport a cloud copy, rather than resume a local conversation. */
  readonly cloudSession?: true
  /** W10: `claude attach <id>`, a background session, where a waiting one is answered. */
  readonly attach?: true
  /** W10: Claude Code itself in the folder, for the person to answer its trust question once. */
  readonly setup?: true
}

export function terminalArgsFor(request: TerminalRequest): readonly string[] | undefined {
  if (request.setup === true) return request.runtime === 'claude' ? [] : undefined
  if (request.attach === true) {
    // Hex, as `claude --bg` prints it: never an option, never a shell word.
    return request.runtime === 'claude' && /^[0-9a-f]{6,64}$/i.test(request.sessionId) ? ['attach', request.sessionId] : undefined
  }
  if (request.cloudSession === true) {
    return request.runtime === 'claude' && isResumableSessionId(request.sessionId) ? ['--teleport', request.sessionId] : undefined
  }
  return resumeArgsFor(request.runtime, request.sessionId)
}

/** What the host knows, handed in so the decision can be tested without a ledger. */
export interface TerminalFacts {
  readonly liveMissionIds: () => readonly string[]
  readonly getMission: (missionId: string) => Promise<{ readonly runtime: MissionRuntimeId; readonly model: string; readonly session: string | undefined; readonly account?: string } | undefined>
  /** The teammate whose conversation this is, if anyone's. */
  readonly ownerOf: (missionId: string) => Promise<{ readonly teammateId: string; readonly name: string } | undefined>
  /** Where that teammate's runs happen (peerContextFor's cwd), or undefined for the folder. */
  readonly cwdFor: (teammateId: string) => Promise<string | undefined>
  readonly workspacePath: string
  readonly launchFor: (runtime: MissionRuntimeId, account?: string) => Promise<ExecutableLaunch | undefined>
}

/**
 * The request for one conversation, or why there is none. Never while its run
 * is going: Locust's run and a terminal would both write to one session, each
 * unaware of the other.
 */
export async function terminalRequestFor(missionId: string, facts: TerminalFacts): Promise<TerminalRequest | { readonly refused: string }> {
  if (facts.liveMissionIds().includes(missionId)) {
    return { refused: 'This conversation is still running. Open it in a terminal once the run finishes: two programs on one session at once would talk over each other.' }
  }
  const mission = await facts.getMission(missionId).catch(() => undefined)
  if (mission === undefined) return { refused: 'That conversation is not in the ledger.' }
  const name = runtimeDisplayName(mission.runtime)
  // A model of the person's own runs on settings Locust hands OpenCode per run.
  if (/^own-[a-f0-9]{8}\//.test(mission.model)) {
    return { refused: 'A model of your own runs on settings only Locust gives OpenCode, so a terminal could not run it. Keep this conversation here.' }
  }
  if (mission.session === undefined) return { refused: `This conversation left no ${name} session to open. Send a message here first.` }
  if (resumeArgsFor(mission.runtime, mission.session) === undefined) return { refused: `${name} cannot resume this conversation in a terminal.` }
  const owner = await facts.ownerOf(missionId).catch(() => undefined)
  const cwd = (owner === undefined ? undefined : await facts.cwdFor(owner.teammateId).catch(() => undefined)) ?? facts.workspacePath
  const launch = await facts.launchFor(mission.runtime, ...(mission.account === undefined ? [] : [mission.account])).catch(() => undefined)
  // Antigravity resumes only through its CLI; its app's conversations live in its own window.
  if (mission.runtime === 'antigravity' && launch?.commandName !== 'agy') return { refused: "An Antigravity conversation lives in Antigravity's own window. Open it there, or install Antigravity CLI to open it in a terminal." }
  if (launch === undefined) return { refused: `Locust could not find ${name} on this machine.` }
  return { runtime: mission.runtime, ...(mission.account === undefined ? {} : { account: mission.account }), sessionId: mission.session, cwd, launch, title: `${owner?.name ?? 'Locust'} · ${name}` }
}

export interface OpenInTerminalOptions {
  readonly platform?: NodeJS.Platform
  /** Test seam, and the drive's: `console` skips Windows Terminal. */
  readonly prefer?: 'windows-terminal' | 'console'
  readonly spawn?: typeof spawn
  readonly localAppData?: string
}

/** Resolves once the process has started, or with the reason it did not. */
function started(child: ReturnType<typeof spawn>): Promise<true | Error> {
  return new Promise((resolve) => {
    child.once('spawn', () => resolve(true))
    child.once('error', (error) => resolve(error))
  })
}

export async function openInTerminal(request: TerminalRequest, options: OpenInTerminalOptions = {}): Promise<OpenInTerminalResponse> {
  const platform = options.platform ?? process.platform
  if (opensAScriptTerminal(platform)) {
    const resumeHere = terminalArgsFor(request)
    if (resumeHere === undefined) return { ok: false, message: 'This conversation has no session its runtime can resume in a terminal.' }
    return openInScriptTerminal(platform, request.cwd, terminalProgram(request.launch, resumeHere), request.launch.env, options.spawn ?? spawn)
  }
  if (platform !== 'win32') {
    return { ok: false, message: 'Opening a conversation in a terminal works on Windows, macOS and Linux.' }
  }
  const resume = terminalArgsFor(request)
  if (resume === undefined) return { ok: false, message: 'This conversation has no session its runtime can resume in a terminal.' }
  const program = terminalProgram(request.launch, resume)
  const run = options.spawn ?? spawn

  /*
   * Windows Terminal, unless the launch needs Locust's own environment: a CLI
   * run under the app's own Node (no Node on the machine) carries
   * ELECTRON_RUN_AS_NODE, and a wt that is already running would start the
   * tab in ITS environment, not this one. Tried by starting it, never by
   * looking for the file: `wt.exe` is an app execution alias, and
   * `existsSync` says it is not there when it is (measured 2026-09-26).
   */
  const wtArgs = windowsTerminalArgs(program, request.cwd, request.title)
  if (options.prefer !== 'console' && request.launch.env === undefined && wtArgs !== undefined) {
    // A Windows path, joined by Windows' rules whatever machine builds it.
    const wt = options.localAppData === undefined ? 'wt.exe' : win32.join(options.localAppData, 'Microsoft', 'WindowsApps', 'wt.exe')
    try {
      const child = run(wt, [...wtArgs], { detached: true, stdio: 'ignore', windowsHide: false })
      if ((await started(child)) === true) {
        child.unref()
        return { ok: true, where: 'Windows Terminal' }
      }
    } catch {
      // Not installed, or refused: the console window below.
    }
  }

  try {
    const child = run('cmd.exe', [consoleLine(program, resume)], {
      cwd: request.cwd,
      // Detached on Windows is a console of its own: the visible window.
      detached: true,
      stdio: 'ignore',
      windowsHide: false,
      windowsVerbatimArguments: true,
      ...(request.launch.env === undefined ? {} : { env: { ...process.env, ...request.launch.env } })
    })
    const result = await started(child)
    if (result !== true) return { ok: false, message: `The terminal could not be opened (${result.message}). The conversation is here, as it was.` }
    child.unref()
    return { ok: true, where: 'a console window' }
  } catch (error) {
    return { ok: false, message: `The terminal could not be opened (${error instanceof Error ? error.message : String(error)}). The conversation is here, as it was.` }
  }
}

/**
 * A program Locust chose, in a terminal window of its own (0.761): `gh codespace ssh` onto the person's cloud
 * computer, or `gh auth refresh` for GitHub's codespace permission, which the person answers there themselves.
 * Windows Terminal when it starts, else a console window; Terminal on a Mac.
 */
export async function openProgramInTerminal(
  program: { readonly file: string; readonly args: readonly string[] },
  cwd: string,
  title: string,
  options: OpenInTerminalOptions = {}
): Promise<OpenInTerminalResponse> {
  const platform = options.platform ?? process.platform
  const run = options.spawn ?? spawn
  if (opensAScriptTerminal(platform)) return openInScriptTerminal(platform, cwd, program, undefined, run)
  if (platform !== 'win32') return { ok: false, message: 'Opening a terminal works on Windows, macOS and Linux.' }
  const wtArgs = windowsTerminalArgs(program, cwd, title)
  if (options.prefer !== 'console' && wtArgs !== undefined) {
    const wt = options.localAppData === undefined ? 'wt.exe' : win32.join(options.localAppData, 'Microsoft', 'WindowsApps', 'wt.exe')
    try {
      const child = run(wt, [...wtArgs], { detached: true, stdio: 'ignore', windowsHide: false })
      if ((await started(child)) === true) {
        child.unref()
        return { ok: true, where: 'Windows Terminal' }
      }
    } catch {
      // Not installed, or refused: the console window below.
    }
  }
  try {
    const child = run('cmd.exe', [consoleLine(program, [])], { cwd, detached: true, stdio: 'ignore', windowsHide: false, windowsVerbatimArguments: true })
    const result = await started(child)
    if (result !== true) return { ok: false, message: `The terminal could not be opened (${result.message}).` }
    child.unref()
    return { ok: true, where: 'a console window' }
  } catch (error) {
    return { ok: false, message: `The terminal could not be opened (${error instanceof Error ? error.message : String(error)}).` }
  }
}
