/**
 * Whether a shell command acts on programs it did not start (0.578).
 *
 * In the arena run of 2026-10-03, Opus ran `taskkill //F //IM python.exe`.
 * That stops EVERY Python program on the computer, not only the one the run
 * started, and nothing in the thread said so. This names that reach, for the
 * approval card (before it runs) and the command row (after it ran, where no
 * card was asked).
 *
 * It reads the command the way the runtimes really send it: inside
 * `powershell -Command "..."` (and `-EncodedCommand`), `cmd /c "..."` and
 * `bash -lc '...'`, chained with `&&`, `||`, `;`, `&` and `|`. Only the
 * command word of each part counts, so a reach that is only quoted or echoed
 * (`echo "taskkill /IM x"`) is not one. Acting on one program by its number
 * (`taskkill /PID 1234`, `kill 1234`, `Stop-Process -Id 1234`) is not a
 * reach: that number came from somewhere this run could see.
 *
 * Pure and dependency-free: the main process and the renderer both use it.
 */

export type CommandReachKind = 'every-process-named' | 'every-process-of-user' | 'every-process-on-port' | 'whole-machine'

export interface CommandReach {
  readonly kind: CommandReachKind
  /** The name, user or port as written, lower-cased (`.exe` kept if given). */
  readonly target: string
  /** A few words for a badge: "stops every python.exe". */
  readonly short: string
  /** One sentence for a person. */
  readonly said: string
}

const NOT_ONLY = 'not only the ones this run started'

export function commandReach(command: string | undefined): CommandReach | undefined {
  // A row whose runtime never sent the command has nothing to read.
  if (typeof command !== 'string') return undefined
  return reachIn(command, 0)
}

function reachIn(command: string, depth: number): CommandReach | undefined {
  if (depth > 4) return undefined
  const parts = splitChain(command)
  for (let i = 0; i < parts.length; i += 1) {
    // A group, a substitution or a backtick runs the commands inside it: read those too (2026-10-10 sweep).
    for (const inner of innerCommands(parts[i]!.text)) {
      const found = reachIn(inner, depth + 1)
      if (found !== undefined) return found
    }
    const words = commandWords(tokens(parts[i]!.text))
    if (words.length === 0) continue
    const inner = wrapped(words)
    if (inner !== undefined) {
      const found = reachIn(inner, depth + 1)
      if (found !== undefined) return found
      continue
    }
    // The parts piped into this one, nearest first.
    const piped: string[][] = []
    for (let j = i; j > 0 && parts[j]!.after === '|'; j -= 1) piped.push(commandWords(tokens(parts[j - 1]!.text)))
    const found = reachOf(words, piped)
    if (found !== undefined) return found
  }
  return undefined
}

// --- the commands ---------------------------------------------------------

function reachOf(words: readonly string[], piped: readonly (readonly string[])[]): CommandReach | undefined {
  const verb = programName(words[0]!)
  const args = words.slice(1)
  switch (verb) {
    case 'taskkill':
      return taskkill(args)
    case 'stop-process':
    case 'spps':
      return stopProcess(args, piped)
    case 'kill':
      // PowerShell's `kill` is Stop-Process; a POSIX `kill` takes numbers.
      return args.some((arg) => /^-(name|processname|id)$/i.test(arg)) ? stopProcess(args, piped) : posixKill(args, piped)
    case 'pkill':
      return pkill(args)
    case 'killall':
      return killall(args)
    case 'wmic':
      return wmic(args)
    case 'xargs':
      return xargsKill(args, piped)
    case 'fuser':
      return args.some((arg) => /^-[a-z]*k/i.test(arg)) ? onPort(args.find((arg) => /^\d+(\/(tcp|udp))?$/i.test(arg))?.replace(/\/.*/, '')) : undefined
    case 'kill-port':
      return onPort(args.find((arg) => /^\d+$/.test(arg)))
    case 'npx':
      return args[0] !== undefined && programName(args[0]) === 'kill-port' ? reachOf(args, piped) : undefined
    case 'shutdown':
      if (args.some((arg) => /^[/-]a$/i.test(arg) || arg === '-c')) return undefined
      // Windows' `/l` signs the person out, closing everything they run; `/h`
      // hibernates and closes nothing. POSIX `-h` is halt, told apart by the slash.
      if (args.some((arg) => /^\/l$/i.test(arg))) {
        return { kind: 'every-process-of-user', target: 'you', short: 'signs you out', said: 'Signs you out of Windows, closing every program you are running.' }
      }
      if (args.some((arg) => /^\/h$/i.test(arg))) return undefined
      return machine(args.some((arg) => /^([/-]r|--reboot|-g)$/i.test(arg)))
    case 'tskill': {
      // Windows' older kill: a process id, or every process of that name.
      const target = args.find((arg) => !arg.startsWith('/') && !arg.startsWith('-'))
      if (target === undefined || /^\d+$/.test(target)) return undefined
      return named(target)
    }
    case 'invoke-cimmethod':
      // `Get-CimInstance Win32_Process … | Invoke-CimMethod -MethodName Terminate`
      return args.some((arg, i) => /^-methodname$/i.test(arg) && /^terminate$/i.test(args[i + 1] ?? '')) ? fromPipeline(piped) : undefined
    case 'foreach-object':
    case '%':
      // `Get-WmiObject Win32_Process … | % { $_.Terminate() }`
      return /\.terminate\(/i.test(args.join(' ')) ? fromPipeline(piped) : undefined
    case 'restart-computer':
    case 'reboot':
      return machine(true)
    case 'stop-computer':
    case 'poweroff':
    case 'halt':
      return machine(false)
    case 'systemctl':
      if (args.includes('reboot')) return machine(true)
      if (args.includes('poweroff') || args.includes('halt')) return machine(false)
      return undefined
    default:
      return undefined
  }
}

function taskkill(args: readonly string[]): CommandReach | undefined {
  for (let i = 0; i < args.length; i += 1) {
    const flag = args[i]!.toLowerCase()
    if (/^(\/\/?|-)im$/.test(flag) && args[i + 1] !== undefined) return named(args[i + 1]!)
    if (/^(\/\/?|-)fi$/.test(flag) && args[i + 1] !== undefined) {
      const image = /imagename\s+eq\s+(\S+)/i.exec(args[i + 1]!)
      if (image !== null) return named(image[1]!)
      const user = /username\s+eq\s+(\S+)/i.exec(args[i + 1]!)
      if (user !== null) return ofUser(user[1]!)
    }
  }
  return undefined
}

function stopProcess(args: readonly string[], piped: readonly (readonly string[])[]): CommandReach | undefined {
  for (let i = 0; i < args.length; i += 1) {
    const flag = args[i]!.toLowerCase()
    if ((flag === '-name' || flag === '-processname') && args[i + 1] !== undefined) return named(args[i + 1]!.split(',')[0]!)
    if ((flag === '-id' || flag === '-pid') && args[i + 1] !== undefined) {
      // A number is one program; an expression that looks programs up is not.
      return lookedUp(args.slice(i + 1).join(' '))
    }
  }
  if (args.some((arg) => /^\d+$/.test(arg))) return undefined
  // `... | Stop-Process`: whatever the pipeline found.
  return fromPipeline(piped)
}

function posixKill(args: readonly string[], piped: readonly (readonly string[])[]): CommandReach | undefined {
  // `kill -1`, `kill -9 -1`, `kill -- -1`: pid -1 is every program the user
  // may signal. As the first argument alone it is a signal number only when
  // something follows it.
  const minusOne = args.indexOf('-1')
  if (minusOne === args.length - 1 && minusOne >= 0) return ofUser('you')
  const joined = args.join(' ')
  if (/\$\(|`/.test(joined)) return lookedUp(joined)
  if (args.length === 0) return fromPipeline(piped)
  return undefined
}

function pkill(args: readonly string[]): CommandReach | undefined {
  const user = optionValue(args, ['-u', '-U', '--euid', '--uid'])
  const full = args.some((arg) => arg === '-f' || arg === '--full' || /^-[a-z]*f[a-z]*$/i.test(arg) && !/^-\d/.test(arg))
  const pattern = lastPlain(args, ['-u', '-U', '--euid', '--uid', '-s', '--signal', '-g', '-P', '-t'])
  if (pattern !== undefined) return full ? matching(pattern) : named(pattern)
  if (user !== undefined) return ofUser(user)
  return undefined
}

function killall(args: readonly string[]): CommandReach | undefined {
  const user = optionValue(args, ['-u', '--user'])
  const name = lastPlain(args, ['-u', '--user', '-s', '--signal'])
  if (name !== undefined) return named(name)
  if (user !== undefined) return ofUser(user)
  return undefined
}

function wmic(args: readonly string[]): CommandReach | undefined {
  const text = args.join(' ')
  if (!/^process\b/i.test(text) || !/\b(delete|terminate)\b/i.test(text)) return undefined
  const name = /name\s*=\s*['"]?([^'"\s)]+)/i.exec(text)
  if (name !== null) return named(name[1]!)
  if (/processid\s*=/i.test(text)) return undefined
  return named('*')
}

function xargsKill(args: readonly string[], piped: readonly (readonly string[])[]): CommandReach | undefined {
  const at = args.findIndex((arg) => {
    const name = programName(arg)
    return name === 'kill' || name === 'taskkill'
  })
  if (at < 0) return undefined
  return fromPipeline(piped)
}

// --- what a pipeline or an expression looked up ----------------------------

/** lsof's `-i:3000`, `-ti tcp:5173`, `-t -i :8080`. */
const PORT_FLAG = /-[a-z]*i\s*(?:tcp|udp)?\s*:(\d+)/i

function fromPipeline(piped: readonly (readonly string[])[]): CommandReach | undefined {
  for (const words of piped) {
    if (words.length === 0) continue
    const verb = programName(words[0]!)
    const args = words.slice(1)
    if (verb === 'get-process' || verb === 'gps' || (verb === 'ps' && args.some((arg) => /^-(name|processname)$/i.test(arg) || !arg.startsWith('-')) && !args.some((arg) => /^(aux|-ef|-e|-a)$/i.test(arg)))) {
      const name = optionValue(args, ['-Name', '-name', '-ProcessName', '-processname']) ?? args.find((arg) => !arg.startsWith('-'))
      if (name !== undefined) return named(name.split(',')[0]!)
      continue
    }
    if (verb === 'pgrep' || verb === 'pidof') {
      const full = verb === 'pgrep' && args.some((arg) => arg === '-f' || arg === '--full')
      const pattern = lastPlain(args, ['-u', '-U', '-g', '-P', '-t'])
      if (pattern !== undefined) return full ? matching(pattern) : named(pattern)
    }
    if (verb === 'grep' || verb === 'findstr' || verb === 'select-string') {
      // An inverted match (`grep -v grep`, `findstr /v`, `-NotMatch`) names what is
      // left OUT, not what is stopped (0.589): the part before it says that.
      const inverted = args.some((arg) => arg === '--invert-match' || /^-[a-su-z]*v[a-z]*$/.test(arg) || /^\/v$/i.test(arg) || /^-notmatch$/i.test(arg))
      if (inverted) continue
      const pattern = args.find((arg) => !arg.startsWith('-') && !arg.startsWith('/'))
      if (pattern !== undefined && pattern !== 'grep') return matching(pattern)
    }
    if (verb === 'where-object' || verb === 'where' || verb === '?') {
      const name = /\.(?:process)?name\s+-(?:eq|like|match)\s+['"]?([^'"\s}]+)/i.exec(args.join(' '))
      if (name !== null) return named(name[1]!)
    }
    // CIM and WMI: `Get-CimInstance Win32_Process -Filter "name='x'"`; a process id names one program.
    if ((verb === 'get-ciminstance' || verb === 'gcim' || verb === 'get-wmiobject' || verb === 'gwmi') && args.some((arg) => /^win32_process$/i.test(arg))) {
      const filter = args.join(' ')
      if (/processid\s*=/i.test(filter)) continue
      const name = /\bname\s*=\s*['"]?([^'"\s]+)/i.exec(filter)
      if (name !== null) return named(name[1]!)
      continue
    }
    if (verb === 'lsof') {
      const port = PORT_FLAG.exec(args.join(' '))
      if (port !== null) return onPort(port[1])
    }
    if (verb === 'get-nettcpconnection') return onPort(optionValue(args, ['-LocalPort', '-localport']))
  }
  return undefined
}

function lookedUp(expression: string): CommandReach | undefined {
  const port = PORT_FLAG.exec(expression) ??/get-nettcpconnection[^)]*-localport\s+(\d+)/i.exec(expression)
  if (port !== null) return onPort(port[1])
  const process = /get-process\s+(?:-(?:process)?name\s+)?['"]?([^'"\s).|]+)/i.exec(expression)
  if (process !== null && !process[1]!.startsWith('-')) return named(process[1]!)
  const pgrep = /\b(pgrep|pidof)\s+((?:-\S+\s+)*)['"]?([^'"\s)`]+)/i.exec(expression)
  if (pgrep !== null) return /(^|\s)-f\b/.test(pgrep[2]!) ? matching(pgrep[3]!) : named(pgrep[3]!)
  return undefined
}

// --- the sentences --------------------------------------------------------

function named(raw: string): CommandReach {
  const target = unquote(raw).toLowerCase()
  if (target === '*' || target.includes('*')) {
    return { kind: 'every-process-named', target, short: `stops every ${target}`, said: `Stops every program named like ${target} on this computer, ${NOT_ONLY}.` }
  }
  return { kind: 'every-process-named', target, short: `stops every ${target}`, said: `Stops every ${target} on this computer, ${NOT_ONLY}.` }
}

function matching(raw: string): CommandReach {
  const target = unquote(raw).toLowerCase()
  return {
    kind: 'every-process-named',
    target,
    short: `stops every match of ${target}`,
    said: `Stops every program on this computer whose command line matches "${target}", ${NOT_ONLY}.`
  }
}

function ofUser(raw: string): CommandReach {
  const target = unquote(raw).toLowerCase()
  return target === 'you'
    ? { kind: 'every-process-of-user', target, short: 'stops all your programs', said: `Stops every program you are running on this computer, ${NOT_ONLY}.` }
    : { kind: 'every-process-of-user', target, short: `stops all of ${target}'s programs`, said: `Stops every program ${target} is running on this computer, ${NOT_ONLY}.` }
}

function onPort(port: string | undefined): CommandReach | undefined {
  if (port === undefined) return undefined
  return {
    kind: 'every-process-on-port',
    target: port,
    short: `stops whatever uses port ${port}`,
    said: `Stops whatever program is using port ${port}, which may be one this run did not start.`
  }
}

function machine(restart: boolean): CommandReach {
  return restart
    ? { kind: 'whole-machine', target: 'restart', short: 'restarts the computer', said: 'Restarts this computer, closing every program on it.' }
    : { kind: 'whole-machine', target: 'shut down', short: 'shuts the computer down', said: 'Shuts this computer down, closing every program on it.' }
}

// --- reading a command line -----------------------------------------------

interface Part {
  readonly text: string
  /** The operator before this part (`|` for a pipe), or '' for the first. */
  readonly after: string
}

/** Splits on `&&`, `||`, `;`, `&`, `|` and newlines, outside quotes. */
export function splitChain(command: string): Part[] {
  const parts: Part[] = []
  let current = ''
  let after = ''
  let quote: string | undefined
  let depth = 0
  for (let i = 0; i < command.length; i += 1) {
    const ch = command[i]!
    if (quote !== undefined) {
      current += ch
      if (ch === quote) quote = undefined
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      current += ch
      continue
    }
    // `$( ... )` and `( ... )` stay whole: Stop-Process -Id (Get-Process x).Id
    if (ch === '(') depth += 1
    if (ch === ')' && depth > 0) depth -= 1
    if (depth > 0) {
      current += ch
      continue
    }
    const two = command.slice(i, i + 2)
    let op: string | undefined
    if (two === '&&' || two === '||') op = two
    else if (ch === ';' || ch === '\n' || ch === '\r') op = ';'
    else if (ch === '|') op = '|'
    // `2>&1`, `&>` and `>&` are redirections, not separators.
    else if (ch === '&' && command[i - 1] !== '>' && command[i - 1] !== '<' && command[i + 1] !== '>') op = '&'
    if (op === undefined) {
      current += ch
      continue
    }
    if (current.trim().length > 0) parts.push({ text: current, after })
    current = ''
    after = op
    i += op.length - 1
  }
  if (current.trim().length > 0) parts.push({ text: current, after })
  return parts
}

/** Words, with quotes removed. A backslash is a path separator here, not an escape. */
export function tokens(text: string): string[] {
  const words: string[] = []
  let current = ''
  let started = false
  let quote: string | undefined
  for (const ch of text) {
    if (quote !== undefined) {
      if (ch === quote) quote = undefined
      else current += ch
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      started = true
      continue
    }
    if (/\s/.test(ch)) {
      if (started) words.push(current)
      current = ''
      started = false
      continue
    }
    current += ch
    started = true
  }
  if (started) words.push(current)
  return words
}

/** The text inside `( ... )` wrapping a whole part, each `$( ... )`, and each backtick span: commands that run. */
export function innerCommands(text: string): string[] {
  const found: string[] = []
  const trimmed = text.trim()
  const close = (from: number): number => {
    let depth = 0
    for (let at = from; at < trimmed.length; at += 1) {
      if (trimmed[at] === '(') depth += 1
      else if (trimmed[at] === ')') {
        depth -= 1
        if (depth === 0) return at
      }
    }
    return -1
  }
  if (trimmed.startsWith('(') && close(0) === trimmed.length - 1) found.push(trimmed.slice(1, -1))
  for (let at = trimmed.indexOf('$('); at >= 0; at = trimmed.indexOf('$(', at + 2)) {
    const end = close(at + 1)
    if (end > at) found.push(trimmed.slice(at + 2, end))
  }
  for (const match of trimmed.matchAll(/`([^`]+)`/g)) found.push(match[1]!)
  return found
}

/** Drops what runs the command rather than being it: `sudo`, `&`, `env X=1`, `timeout 10`, `nice -n 5`, `do`. */
export function commandWords(words: readonly string[]): string[] {
  let at = 0
  while (at < words.length) {
    const word = words[at]!
    const name = programName(word)
    // A loop's or a branch's body is a command like any other (2026-10-10 sweep).
    if (['do', 'then', 'else', 'elif', '{', '!'].includes(word)) {
      at += 1
      continue
    }
    if (word === '&' || word === '.' || ['sudo', 'doas', 'nohup', 'time', 'exec', 'command', 'env', 'nice', 'ionice', 'stdbuf'].includes(name)) {
      at += 1
      // `sudo -u x`, `env -i`, `nice -n 5`
      while (at < words.length && words[at]!.startsWith('-')) at += words[at] === '-u' || words[at] === '-n' ? 2 : 1
      continue
    }
    // `timeout [-s SIGNAL] 10 <command>`: its flags and its duration.
    if (name === 'timeout') {
      at += 1
      while (at < words.length && words[at]!.startsWith('-')) at += words[at] === '-s' || words[at] === '-k' ? 2 : 1
      if (at < words.length && /^\d+(?:\.\d+)?[smhd]?$/.test(words[at]!)) at += 1
      continue
    }
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(word)) {
      at += 1
      continue
    }
    break
  }
  return words.slice(at)
}

/** The command inside `powershell -Command`, `cmd /c` or `bash -c`, if this is one. */
export function wrapped(words: readonly string[]): string | undefined {
  const name = programName(words[0]!)
  const args = words.slice(1)
  if (name === 'powershell' || name === 'pwsh') {
    for (let i = 0; i < args.length; i += 1) {
      const flag = args[i]!.toLowerCase()
      if (/^-(e|ec|enc|encodedcommand)$/.test(flag) && args[i + 1] !== undefined) return decodeUtf16Base64(args[i + 1]!)
      if (/^-(c|command)$/.test(flag)) return args.slice(i + 1).join(' ')
      if (flag === '-file' || flag === '-f') return undefined
    }
    // `powershell Stop-Process -Name x`: the rest is the command.
    const first = args.findIndex((arg) => !arg.startsWith('-'))
    return first < 0 ? undefined : args.slice(first).join(' ')
  }
  if (name === 'cmd') {
    const at = args.findIndex((arg) => /^\/[ck]$/i.test(arg))
    return at < 0 ? undefined : args.slice(at + 1).join(' ')
  }
  if (['bash', 'sh', 'zsh', 'dash', 'ksh', 'fish'].includes(name)) {
    const at = args.findIndex((arg) => /^-[a-z]*c[a-z]*$/i.test(arg))
    return at < 0 ? undefined : args[at + 1]
  }
  return undefined
}

function decodeUtf16Base64(text: string): string | undefined {
  try {
    const bytes = atob(text)
    let out = ''
    for (let i = 0; i + 1 < bytes.length; i += 2) out += String.fromCharCode(bytes.charCodeAt(i) | (bytes.charCodeAt(i + 1) << 8))
    return out
  } catch {
    return undefined
  }
}

/** `C:\Windows\System32\taskkill.exe` and `/usr/bin/pkill` as `taskkill` and `pkill`. */
export function programName(word: string): string {
  const base = word.split(/[\\/]/).pop() ?? word
  return base.toLowerCase().replace(/\.(exe|cmd|bat|ps1)$/, '')
}

function unquote(word: string): string {
  return word.replace(/^['"]|['"]$/g, '')
}

function optionValue(args: readonly string[], names: readonly string[]): string | undefined {
  for (let i = 0; i < args.length - 1; i += 1) if (names.includes(args[i]!)) return args[i + 1]
  return undefined
}

/** The last argument that is neither a flag nor a flag's value. */
function lastPlain(args: readonly string[], takesValue: readonly string[]): string | undefined {
  let found: string | undefined
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i]!
    if (takesValue.includes(arg)) {
      i += 1
      continue
    }
    if (arg.startsWith('-')) continue
    found = arg
  }
  return found
}
