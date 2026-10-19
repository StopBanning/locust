import { randomUUID } from 'node:crypto'
import { existsSync } from 'node:fs'
import { stat } from 'node:fs/promises'
import { delimiter, join, basename, isAbsolute } from 'node:path'
import { execFile } from 'node:child_process'
import type { IPty } from '@lydell/node-pty'
import type { TerminalContext, TerminalEvent, TerminalShell, TerminalTab } from '../shared/terminal.js'

interface Shell extends TerminalShell { readonly file: string; readonly args: string[] }
const onPath = (name: string, env: NodeJS.ProcessEnv): string | undefined =>
  (env.PATH ?? env.Path ?? '').split(delimiter).map(folder => join(folder, name)).find(file => isAbsolute(file) && existsSync(file))

/** Fixed executables discovered by the host; the renderer chooses only an id. No probing processes at rest. */
export function terminalShells(platform = process.platform, env = process.env): readonly Shell[] {
  if (platform !== 'win32') {
    const file = env.SHELL && isAbsolute(env.SHELL) && existsSync(env.SHELL) ? env.SHELL : '/bin/sh'
    return [{ id: 'login', name: basename(file), file, args: ['-l'] }]
  }
  const windows = existsSync('C:\\Windows\\System32') ? 'C:\\Windows' : env.SystemRoot ?? 'C:\\Windows'
  const pwsh = onPath('pwsh.exe', env) ?? [join(env.ProgramFiles ?? 'C:\\Program Files', 'PowerShell', '7', 'pwsh.exe')].find(existsSync)
  const git = onPath('git.exe', env)
  const bash = [join(env.ProgramFiles ?? 'C:\\Program Files', 'Git', 'bin', 'bash.exe'), ...(git === undefined ? [] : [join(git, '..', '..', 'bin', 'bash.exe')])].find(existsSync)
  return [
    ...(pwsh === undefined ? [] : [{ id: 'pwsh', name: 'PowerShell 7', file: pwsh, args: ['-NoLogo'] }]),
    { id: 'powershell', name: 'Windows PowerShell', file: join(windows, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'), args: ['-NoLogo'] },
    { id: 'cmd', name: 'Command Prompt', file: join(windows, 'System32', 'cmd.exe'), args: [] },
    ...(bash === undefined ? [] : [{ id: 'git-bash', name: 'Git Bash', file: bash, args: ['--login', '-i'] }]),
    { id: 'wsl', name: 'WSL', file: join(windows, 'System32', 'wsl.exe'), args: [] }
  ].filter(shell => existsSync(shell.file))
}

/** Jobs can have their own process groups. Snapshot parentage before ending the shell, not after it is reparented. */
function descendantsOf(snapshot: string, root: number): readonly number[] {
  const children = new Map<number, number[]>()
  for (const line of snapshot.split('\n')) {
    const [pid, parent] = line.trim().split(/\s+/).map(Number)
    if (!Number.isSafeInteger(pid) || !Number.isSafeInteger(parent) || pid! <= 1 || parent! < 1) continue
    children.set(parent!, [...(children.get(parent!) ?? []), pid!])
  }
  const owned = [root]
  const seen = new Set(owned)
  for (let i = 0; i < owned.length; i++) for (const pid of children.get(owned[i]!) ?? []) {
    if (!seen.has(pid)) { seen.add(pid); owned.push(pid) }
  }
  return owned.slice(1).reverse()
}

/** Kill the owned tree while the shell is still alive; never kill by image name. */
export async function endTerminalTree(pty: Pick<IPty, 'pid' | 'kill'>, options: {
  readonly platform?: NodeJS.Platform
  readonly processes?: () => Promise<string>
  readonly kill?: (pid: number, signal: NodeJS.Signals) => void
} = {}): Promise<void> {
  if ((options.platform ?? process.platform) === 'win32') {
    await new Promise<void>(resolve => execFile(join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'taskkill.exe'),
      ['/PID', String(pty.pid), '/T', '/F'], { windowsHide: true, timeout: 3_000 }, () => resolve()))
  } else {
    if (pty.pid <= 1) return
    const read = options.processes ?? (() => new Promise<string>(resolve => execFile('/bin/ps', ['-axo', 'pid=,ppid='],
      { encoding: 'utf8', timeout: 1_000, maxBuffer: 4 * 1024 * 1024 }, (error, stdout) => resolve(error === null ? stdout : ''))))
    const snapshot = await read().catch(() => '')
    const kill = options.kill ?? ((pid, signal) => { process.kill(pid, signal) })
    for (const pid of descendantsOf(snapshot, pty.pid)) { try { kill(pid, 'SIGKILL') } catch { /* already ended */ } }
    try { kill(-pty.pid, 'SIGKILL') } catch { try { pty.kill('SIGKILL') } catch { /* already ended */ } }
    return
  }
  try { pty.kill() } catch { /* already ended */ }
}

type Session = { readonly pty: IPty; readonly tab: TerminalTab; pending: string; timer?: ReturnType<typeof setTimeout>; ended: boolean }

export function createTerminalHost(options: {
  readonly folder: (context: TerminalContext) => Promise<string | undefined>
  readonly refusal: () => string | undefined
  readonly emit: (event: TerminalEvent) => void
  readonly shells?: () => readonly Shell[]
  readonly spawn?: (file: string, args: string[], options: { cwd: string; env: NodeJS.ProcessEnv; cols: number; rows: number; name: string }) => Promise<IPty>
  readonly end?: (pty: IPty) => Promise<void>
}) {
  const sessions = new Map<string, Session>()
  let disposed = false
  let opening = 0
  const pendingCreates = new Set<Promise<unknown>>()
  const pendingCloses = new Set<Promise<void>>()
  const shells = options.shells ?? terminalShells
  const flush = (session: Session): void => {
    if (session.timer !== undefined) clearTimeout(session.timer)
    session.timer = undefined
    if (session.pending.length === 0) return
    const data = session.pending
    session.pending = ''
    options.emit({ id: session.tab.id, kind: 'data', data })
    if (!session.ended) session.pty.resume()
  }
  const close = async (id: string): Promise<void> => {
    const session = sessions.get(id)
    if (session === undefined) return
    sessions.delete(id)
    const wasEnded = session.ended
    session.ended = true
    flush(session)
    if (!wasEnded) await (options.end ?? endTerminalTree)(session.pty)
  }
  const create = async (context: TerminalContext, shellId?: string): Promise<TerminalTab> => {
    const refused = options.refusal()
    if (refused !== undefined) throw new Error(refused)
    if (disposed || sessions.size + opening >= 8) throw new Error('Close a terminal tab before opening another.')
    opening++
    try {
      const cwd = await options.folder(context)
      if (cwd === undefined || !isAbsolute(cwd) || !(await stat(cwd)).isDirectory()) throw new Error('Choose a project folder first. Your terminals are unchanged.')
      if (disposed) throw new Error('Locust is closing. No terminal was started.')
      const choices = shells()
      const shell = shellId === undefined ? choices[0] : choices.find(one => one.id === shellId)
      if (shell === undefined) throw new Error('That shell is unavailable. Your terminals are unchanged.')
      const spawn = options.spawn ?? (async (file, args, config) => (await import('@lydell/node-pty')).spawn(file, args, config))
      const pty = await spawn(shell.file, shell.args, { cwd, env: { ...process.env, TERM: 'xterm-256color' }, cols: 80, rows: 24, name: 'xterm-256color' })
      if (disposed) { await (options.end ?? endTerminalTree)(pty); throw new Error('Locust is closing. The terminal was ended.') }
      const tab = { id: randomUUID(), shell: shell.name, folder: cwd }
      const session: Session = { pty, tab, pending: '', ended: false }
      sessions.set(tab.id, session)
      pty.onData(data => {
        if (session.ended) return
        session.pending += data
        if (session.pending.length >= 256 * 1024) pty.pause()
        if (session.timer === undefined) session.timer = setTimeout(() => flush(session), 16)
      })
      pty.onExit(({ exitCode }) => {
        session.ended = true
        flush(session)
        options.emit({ id: tab.id, kind: 'exit', exitCode })
      })
      return tab
    } finally { opening-- }
  }
  return {
    shells: () => { const refused = options.refusal(); if (refused !== undefined) throw new Error(refused); return shells().map(({ id, name }) => ({ id, name })) },
    create: (context: TerminalContext, shellId?: string) => {
      const pending = create(context, shellId)
      pendingCreates.add(pending)
      void pending.finally(() => pendingCreates.delete(pending)).catch(() => undefined)
      return pending
    },
    input: (id: string, data: string) => { const session = sessions.get(id); if (!session?.ended) session?.pty.write(data) },
    resize: (id: string, cols: number, rows: number) => { const session = sessions.get(id); if (!session?.ended) session?.pty.resize(cols, rows) },
    close: (id: string) => {
      const pending = close(id)
      pendingCloses.add(pending)
      void pending.finally(() => pendingCloses.delete(pending)).catch(() => undefined)
      return pending
    },
    dispose: async () => { disposed = true; await Promise.all([...sessions.keys()].map(close)); await Promise.allSettled([...pendingCreates, ...pendingCloses]) }
  }
}
