// Locust's command guard: the check Claude Code makes before every command a
// teammate runs (0.717; by id since 0.718).
//
// Arena round 4, 2026-10-09: two of five models on Auto tidied up a test
// browser with `taskkill //F //IM msedge.exe` and `taskkill //F //IM
// chrome.exe`. That ends EVERY window of that browser on the computer, the
// person's own included. Auto runs as the person without asking, and that is
// the person's choice; ending what the person has open is part of no job a
// teammate is given.
//
// So a command that ends programs BY NAME is refused, in every mode, when the
// name is one the person or another teammate is running -- a browser, the web
// view other apps draw their windows in, Node, Locust itself, the Windows
// desktop, an editor, an agent's CLI.
//
// 2026-10-10, the next night: refused by name, Fable read the ids off
// `tasklist` and ended the person's Chrome BY ID, 13 seconds later -- and the
// refusal had told it to end things by id. So ids are checked too: an id is
// looked up in the process table, and ending it is refused when it is one of
// those programs and no teammate started it (it does not descend from Locust,
// LOCUST_GUARD_PARENT, nor is it an orphan younger than Locust), when it is
// another run's agent, or when it is Locust or this run's own agent. Locust
// starts the agent anew for every message, so "a teammate started it" is
// Locust's, not one message's (the drive found that). Ending what a teammate
// started -- a test browser, a server -- still works, a headless browser has
// no windows to lose, and starting anything is never refused. The words the
// model reads say to leave the person's program running, and give a browser
// of its own a profile of its own: a headless Chrome on the person's profile
// is what hung, and why it wanted theirs gone.
//
// Claude Code runs this as a PreToolUse hook (`--settings`, command-guard.ts),
// under its own shell -- Git Bash on Windows -- with Electron's binary as
// node. MEASURED 2026-10-10 on Claude Code 2.1.296, Haiku, `bypassPermissions`:
// the deny below stopped the command before the shell saw it, the model read
// the reason word for word, and a capitalised TASKKILL was stopped too. About
// 90 ms a command; a command that ends something by id also reads the process
// table (well under a second).
//
// Deliberately small and dependency-free, like the permission bridge beside
// it. What it cannot read it lets through, saying nothing: it is a guard
// against one mistake, not a sandbox.
//
// Ships beside app.asar via extraResources (electron-builder.yml).

import { spawnSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

/** What ending each of these would end besides the teammate's own. */
const ENDS = [
  { names: ['google chrome', 'chrome', 'chromium'], what: 'Chrome', harm: 'windows', browser: true },
  { names: ['msedgewebview2'], what: 'Edge web view', harm: 'apps' },
  { names: ['microsoft edge', 'msedge'], what: 'Edge', harm: 'windows', browser: true },
  { names: ['firefox'], what: 'Firefox', harm: 'windows', browser: true },
  { names: ['brave browser', 'brave'], what: 'Brave', harm: 'windows', browser: true },
  { names: ['opera'], what: 'Opera', harm: 'windows', browser: true },
  { names: ['vivaldi'], what: 'Vivaldi', harm: 'windows', browser: true },
  { names: ['iexplore'], what: 'Internet Explorer', harm: 'windows', browser: true },
  { names: ['safari'], what: 'Safari', harm: 'windows', browser: true },
  { names: ['node'], what: 'Node', harm: 'work' },
  { names: ['electron'], what: 'Electron', harm: 'work' },
  { names: ['locust'], what: 'Locust', harm: 'work' },
  { names: ['explorer'], what: 'File Explorer', harm: 'desktop' },
  { names: ['code'], what: 'VS Code', harm: 'windows' },
  { names: ['cursor-agent', 'cursor'], what: 'Cursor', harm: 'work', agent: true },
  { names: ['claude'], what: 'Claude Code', harm: 'work', agent: true },
  { names: ['codex'], what: 'Codex', harm: 'work', agent: true },
  { names: ['gemini'], what: 'Gemini CLI', harm: 'work', agent: true },
  { names: ['copilot'], what: 'Copilot CLI', harm: 'work', agent: true },
  { names: ['opencode'], what: 'OpenCode', harm: 'work', agent: true },
  { names: ['agy', 'antigravity'], what: 'Antigravity', harm: 'work', agent: true }
]

const HARM = {
  windows: "the person's own windows",
  apps: "other apps' windows, which draw in it",
  work: "the person's own programs and other teammates' work",
  desktop: "the person's desktop and taskbar"
}

/** Whose a running one is, when this run did not start it. */
const OWNER = {
  windows: "the person's own",
  apps: "the person's own",
  work: "the person's or another teammate's",
  desktop: "the person's own"
}

const BROWSER_TIP = 'A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs.'
const tipFor = (entry) => (entry.browser === true ? BROWSER_TIP : 'End only the processes this run started; Locust checks.')

const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
// Longest first, so "google chrome" is read before "chrome".
const ALL = ENDS.flatMap((entry) => entry.names.map((name) => ({ name, entry }))).sort((a, b) => b.name.length - a.name.length)
/** A protected name as a word of its own, with or without `.exe`: never "codex" for "code", never "locust-scratch". */
const NAME = `(?<![\\w-])(${ALL.map(({ name }) => escape(name).replace(/ /g, '\\s+')).join('|')})(?:\\.exe|\\.app)?(?![\\w-])`
/** Within one command: up to the next `;`, `&`, `|` or line. */
const SAME = '[^\\n;&|]*?'
/** Within one pipeline: up to the next `;`, `&` or line. */
const PIPE = '[^\\n;&]*?'
/** Within one line. */
const LINE = '[^\\n]*?'
const KILLS = '(?:taskkill|tskill|stop-process|spps|pkill|killall|\\bkill\\b|\\.kill\\s*\\()'

// Each, a way to end processes by NAME (or to find the person's by name and end what was found).
const BY_NAME = [
  // taskkill / tskill with an image-name selector: /IM, -IM, //IM (Git Bash), /FI "IMAGENAME eq ..."
  `\\b(?:taskkill|tskill)(?:\\.exe)?\\b(?=${SAME}(?:[\\/-]{1,2}(?:im|fi)\\b))${SAME}${NAME}`,
  `\\btskill(?:\\.exe)?\\s+['"]?${NAME}`,
  // PowerShell: Stop-Process -Name / -ProcessName (or its aliases)
  `\\b(?:stop-process|spps|kill)\\b${SAME}-(?:processname|name|n)\\b${SAME}${NAME}`,
  // Get-Process <name> piped to Stop-Process, or .Kill() on what it found
  `\\b(?:get-process|gps|ps)\\b${SAME}${NAME}${PIPE}(?:\\|\\s*(?:stop-process|spps|kill)\\b|\\.kill\\s*\\(|foreach-object\\s+kill\\b)`,
  // pkill / killall <name>
  `\\b(?:pkill|killall)\\b${SAME}${NAME}`,
  // pgrep / pidof / grep <name> ... | xargs kill
  `\\b(?:pgrep|pidof|grep)\\b${SAME}${NAME}${PIPE}\\|\\s*(?:xargs\\s+(?:-\\S+\\s+)*)?(?:kill|taskkill)\\b`,
  // kill $(pgrep <name>) / kill \`pidof <name>\`
  `\\bkill\\b${SAME}(?:\\$\\(|\`)\\s*(?:pgrep|pidof)\\b[^)\`]*?${NAME}`,
  // wmic process where name='<name>' delete / call terminate
  `\\bwmic\\b${SAME}${NAME}${SAME}\\b(?:delete|terminate)\\b`,
  // Win32_Process ... <name> ... Terminate / Remove-CimInstance
  `\\bwin32_process\\b${PIPE}${NAME}${PIPE}\\b(?:terminate|remove-ciminstance)\\b`,
  // macOS: quit an app by name
  `\\bosascript\\b${SAME}(?:quit\\s+app(?:lication)?\\s+['"\\\\]*${NAME}|tell\\s+app(?:lication)?\\s+['"\\\\]*${NAME}['"\\\\]*\\s+to\\s+quit)`,
  // The person's found by name and ended in the same line, through a variable or the pipeline:
  // `for p in $(tasklist ... chrome ...); do taskkill //PID $p`, `foreach ($p in Get-Process chrome) { $p.Kill() }`.
  // An id written out is not this: it is looked up below.
  `\\b(?:tasklist|get-process|gps|pgrep|pidof|wmic|win32_process|get-ciminstance)\\b${LINE}${NAME}${LINE}(?:\\b(?:taskkill|tskill)\\b${SAME}[\\/-]{1,2}pid\\s+["']?[$%]|\\b(?:stop-process|spps)\\b|(?<![\\w.-])kill\\s+(?:-\\S+\\s+)*["']?[$%]|\\.kill\\s*\\(|\\bxargs\\s+(?:-\\S+\\s+)*(?:kill|taskkill)\\b)`,
  `${KILLS}${LINE}(?:\\$\\(|\`)[^)\`]*\\b(?:tasklist|get-process|gps|pgrep|pidof|wmic)\\b[^)\`]*?${NAME}`
].map((pattern) => new RegExp(pattern, 'i'))

/** The process ids a command ends BY ID, as written in it. Variables and substitutions are not ids. */
export function endedIds(command) {
  if (typeof command !== 'string') return []
  const ids = new Set()
  const add = (text) => {
    for (const one of String(text).split(/[\s,]+/)) if (/^\d{1,9}$/.test(one)) ids.add(Number(one))
  }
  const each = (pattern, group = 1) => {
    for (const m of command.matchAll(pattern)) add(m[group])
  }
  // taskkill /PID n (/PID m ...), in Windows' spelling or Git Bash's //PID
  for (const segment of command.split(/[\n;&|]+/)) {
    if (!/\b(?:taskkill|tskill)\b/i.test(segment)) continue
    for (const m of segment.matchAll(/[/-]{1,2}pid\s+['"]?(\d+)/gi)) add(m[1])
    for (const m of segment.matchAll(/\btskill(?:\.exe)?\s+(\d+)\b/gi)) add(m[1])
  }
  // PowerShell: Stop-Process -Id n[,m] / Stop-Process n / Get-Process -Id n | Stop-Process / .Kill()
  each(/\b(?:stop-process|spps)\b[^\n;&|]*?-id\s+([\d,\s]+)/gi)
  each(/\b(?:stop-process|spps)\s+(\d[\d,\s]*)/gi)
  each(/\bkill\s+-id\s+([\d,\s]+)/gi)
  each(/\bget-process\s+-id\s+([\d,\s]+)[^\n;&]*?(?:\|\s*(?:stop-process|spps|kill)\b|\.kill\s*\()/gi)
  each(/getprocessbyid\(\s*(\d+)\s*\)\s*\.kill\s*\(/gi)
  // kill [-9|-KILL|-s SIG] n [m ...]  (not %1, a job of the shell's own)
  each(/(?<![\w.-])kill((?:\s+-(?:s\s+\S+|\w+))*)((?:\s+\d+)+)(?=\s|$|[;&|)])/gi, 2)
  // node -e "process.kill(n)"
  each(/process\.kill\(\s*(\d+)/gi)
  // wmic process where processid=n delete / wmic process n delete
  each(/\bwmic\b[^\n;&|]*?processid\s*=\s*['"]?(\d+)[^\n;&|]*?\b(?:delete|terminate)\b/gi)
  each(/\bwmic\s+process\s+(\d+)\s+(?:delete|call\s+terminate)\b/gi)
  // Win32_Process -Filter "ProcessId = n" | Invoke-CimMethod -MethodName Terminate
  each(/\bwin32_process\b[^\n;&]*?processid\s*=\s*['"]?(\d+)[^\n;&]*?\b(?:terminate|remove-ciminstance)\b/gi)
  // Ids written out and ended through a variable (arena round B, 2026-10-10: Sonnet's
  // `for p in 28700 30432 ...; do taskkill //F //PID $p; done` was never looked up).
  const killsVariable = (body, name) =>
    new RegExp(
      `(?:\\b(?:taskkill|tskill)\\b[^\\n;&|]*?[/-]{1,2}pid\\s+["']?|(?<![\\w.-])kill\\s+(?:-\\S+\\s+)*["']?|\\b(?:stop-process|spps)\\b[^\\n;&|]*?(?:-id\\s+)?|process\\.kill\\(\\s*)\\$\\{?${name}\\b`,
      'i'
    ).test(body)
  for (const m of command.matchAll(/\bfor\s+(\w+)\s+in\s+([\d\s]+?)\s*;\s*do\b([\s\S]*?)\bdone\b/gi)) if (killsVariable(m[3], m[1])) add(m[2])
  for (const m of command.matchAll(/\bforeach\s*\(\s*\$(\w+)\s+in\s+@?\(?\s*([\d,\s]+?)\s*\)?\s*\)\s*\{([^}]*)\}/gi)) if (killsVariable(m[3], m[1])) add(m[2])
  for (const m of command.matchAll(/(?:^|[\s;("'])@?\(?\s*(\d+(?:\s*,\s*\d+)*)\s*\)?\s*\|\s*(?:%|foreach-object|foreach)\s*\{([^}]*)\}/gi)) if (killsVariable(m[2], '_')) add(m[1])
  each(/\b(?:echo|printf)\s+['"]?([\d\s]+?)['"]?\s*\|\s*xargs\s+(?:-\S+\s+)*(?:kill|taskkill)\b/gi)
  return [...ids]
}

const normalName = (name) => String(name ?? '').replace(/\\/g, '/').split('/').at(-1).toLowerCase().replace(/\.(?:exe|app)$/, '').trim()
/** An agent's CLI running on Node: its own file is named in its command line. */
const AGENT_ON_NODE = /claude-code|@anthropic-ai[\\/]claude|gemini-cli|@github[\\/]copilot|[\\/]opencode|@openai[\\/]codex|cursor-agent/i

/** Which protected program a running process is, or undefined. */
function protectedAs(process) {
  const name = normalName(process.name)
  const hit = ALL.find(({ name: one }) => name === one || name.startsWith(`${one} `))?.entry
  if (hit === undefined) return undefined
  // Node by id is a person's server as often as an agent: only an agent's CLI is kept.
  if (hit.what === 'Node') return AGENT_ON_NODE.test(String(process.line ?? '')) ? { ...hit, what: 'an AI agent on Node', agent: true } : undefined
  return hit
}

/** What starts a person's session: a process whose own parent is gone is theirs if it is one of these. */
const LAUNCHERS = /^(?:explorer|svchost|services|wininit|winlogon|userinit|sihost|runtimebroker|dwm|csrss|smss|system|launchd|systemd|init|loginwindow)$/

/**
 * Whether `pid` was started under `root`: it descends from it -- or its own
 * parent is gone (or that id now belongs to a later process) and it started
 * after `root` did. Claude Code runs each command in a shell of its own, and
 * Locust starts the agent anew for every message, so a test browser started
 * in the background is an orphan by the next command. The person's programs
 * have living, older parents, back to what started their session (LAUNCHERS).
 */
function startedUnder(table, pid, root) {
  const rootStart = table.get(root)?.created ?? Number.POSITIVE_INFINITY
  let at = pid
  for (let step = 0; step < 64; step += 1) {
    if (at === root) return true
    const one = table.get(at)
    if (one === undefined) return false
    const parent = one.ppid > 0 && one.ppid !== at ? table.get(one.ppid) : undefined
    if (parent === undefined || (parent.created ?? 0) > (one.created ?? 0)) return !LAUNCHERS.test(normalName(one.name)) && (one.created ?? 0) >= rootStart
    at = one.ppid
  }
  return false
}

/** A browser with no windows: started headless, or a helper of one that was. */
function windowless(table, pid) {
  let at = pid
  for (let step = 0; step < 16; step += 1) {
    const one = table.get(at)
    if (one === undefined) return false
    if (/--headless\b/i.test(String(one.line ?? ''))) return true
    if (!/--type=/i.test(String(one.line ?? ''))) return false
    at = one.ppid
  }
  return false
}

/** Every match of each, not only the first: one narrowed find must not hide a plain one later in the line. */
const BY_NAME_ALL = BY_NAME.map((pattern) => new RegExp(pattern.source, 'gi'))

/**
 * The command-line filter a find-by-name was narrowed with, as a RegExp, or
 * undefined: PowerShell's `$_.CommandLine -like '*profile*'` and WQL's
 * `CommandLine LIKE '%profile%'`. Only what reads exactly the same here as
 * there: a `[`-range or an `-or` is not narrowing, and neither is `-match`,
 * whose .NET regex is not JavaScript's.
 */
function narrowedTo(text) {
  if (/\s-or\s|\sor\s/i.test(text)) return undefined
  const filters = [
    ...[...text.matchAll(/commandline\s+-[ci]?like\s+\\?(['"])(.*?)\\?\1/gi)].map((m) => ({ pattern: m[2], any: '*', one: '?' })),
    ...[...text.matchAll(/commandline\s+like\s+\\?(['"])(.*?)\\?\1/gi)].map((m) => ({ pattern: m[2], any: '%', one: '_' }))
  ]
  if (filters.length === 0 || filters.some(({ pattern }) => pattern.length === 0 || pattern.includes('['))) return undefined
  const sources = filters.map(({ pattern, any, one }) =>
    [...pattern].map((ch) => (ch === any ? '.*' : ch === one ? '.' : escape(ch))).join('')
  )
  // Every filter must hold (they are joined by -and, or are one WQL condition each).
  return { test: (line) => sources.every((source) => new RegExp(`^${source}$`, 'is').test(line)) }
}

/** The first process of `entry` the filter matches that this run did not start, or undefined when there is none. */
function notTheRuns(seen, entry, filter) {
  const agent = seen.agent ?? seen.root
  for (const [pid, one] of seen.table) {
    if (!entry.names.includes(normalName(one.name))) continue
    if (!filter.test(String(one.line ?? ''))) continue
    // Locust, and the agent this run is, are never a teammate's to end, however they are found.
    if (pid === seen.root || pid === agent || seen.above.includes(pid)) return pid
    const ours = entry.agent === true ? startedUnder(seen.table, pid, agent) : startedUnder(seen.table, pid, seen.root)
    if (ours || (entry.browser === true && windowless(seen.table, pid))) continue
    return pid
  }
  return undefined
}

// A FIX'S COPY PUBLISHES NOTHING (0.828, scout-fix.ts). Scout's fixes are
// prepared in a copy of the person's folder, in a folder named
// rt_scoutfix<12 hex digits>. There, `git push`, `git remote` (and git's other
// ways out: credential, send-email, request-pull) and `gh` are refused in every
// mode: the fix reaches the person only through Apply, and a pull request is
// theirs to open. Known by the folder the command runs in -- the hook's own
// `cwd`, as each runtime hands it, the folder OpenCode's plugin was loaded for,
// and this process's own -- never by what the command says about itself.
const FIX_FOLDER = /(?:^|[\\/])rt_scoutfix[a-f0-9]{12}(?:[\\/]|$)/i
const GH_COMMAND = /(?:^|[\s"'`(;&|=\\/])gh(?:\.exe)?(?=$|[\s"'`);&|])/i
const GIT_OUT = /(?:^|[\s"'`(;&|\\/])git(?:\.exe)?["']?\s+(?:(?:-C\s+(?:"[^"]*"|'[^']*'|\S+)|-c\s+\S+|--?[\w-]+(?:=\S+)?)\s+)*(?:push|remote|credential|send-email|request-pull)(?=$|[\s"'`);&|])/i
const FIX_REFUSAL = 'Refused: this is a copy Scout made to prepare a fix, and from here Locust does not let anything push, add a remote or use gh. The person brings the fix into their folder with Apply and opens any pull request themselves. Make the fix in these files, then say what you changed.'

/** Whether any of these folders is inside a fix's copy. Exported for the tests. */
export function inFixCopy(folders) {
  return Array.isArray(folders) && folders.some((folder) => typeof folder === 'string' && FIX_FOLDER.test(folder))
}

/** Whether a command would reach past a fix's copy: a push, a remote, gh. Exported for the tests. */
export function publishesFromCopy(command) {
  return typeof command === 'string' && (GH_COMMAND.test(command) || GIT_OUT.test(command))
}

/**
 * Why a command is refused, in words the model reads, or undefined when it
 * may run. `look` reads the process table when the command ends something by
 * id (the tests hand in their own); exported for them. `folders` are where it
 * runs (0.828): a fix's copy refuses what would publish from it. The hook
 * below is the only other caller.
 */
export function stoppedBecause(command, look = lookAtProcesses, folders = []) {
  if (typeof command !== 'string' || command.length === 0) return undefined
  if (inFixCopy(folders) && publishesFromCopy(command)) return FIX_REFUSAL
  // The process table, read once and only when something needs it; null when it cannot be read.
  let seen
  const looked = () => {
    if (seen === undefined) {
      try {
        seen = look() ?? null
      } catch {
        seen = null
      }
    }
    return seen
  }
  for (const pattern of BY_NAME_ALL) {
    for (const found of command.matchAll(pattern)) {
      const named = found.slice(1).find((group) => group !== undefined)?.toLowerCase().replace(/\s+/g, ' ')
      const entry = ALL.find(({ name }) => name === named)?.entry
      if (entry === undefined) continue
      // Found by name AND narrowed by its command line (arena round B, 2026-10-10: two models ended
      // only the Edge they had started, matched by its own profile folder): each process the filter
      // matches is looked up, and it may run when every one is this run's.
      const filter = narrowedTo(found[0])
      const table = filter === undefined ? null : looked()
      if (filter !== undefined && table !== null) {
        const theirs = notTheRuns(table, entry, filter)
        if (theirs === undefined) continue
        return (
          `Locust stopped this command before it ran: its command-line filter also matches process ${String(theirs)}, ` +
          `${entry.what} that no teammate started, so it is ${OWNER[entry.harm]}. Leave it running. ${tipFor(entry)}`
        )
      }
      return (
        `Locust stopped this command before it ran: it ends ${entry.what} by name, which ends every ${entry.what} ` +
        `on this computer, ${HARM[entry.harm]} too. Leave the person's ${entry.what} running. ${tipFor(entry)}`
      )
    }
  }
  const ids = endedIds(command)
  if (ids.length === 0) return undefined
  if (looked() === null) return undefined
  for (const pid of ids) {
    const one = seen.table.get(pid)
    if (one === undefined) continue
    // The agent this run is, or Locust above it: never a teammate's to end.
    const agent = seen.agent ?? seen.root
    if (pid === agent || (seen.above.includes(pid) && /^(?:locust|electron)$/.test(normalName(one.name)))) {
      return `Locust stopped this command before it ran: process ${String(pid)} is ${pid === agent ? 'the agent this run is' : 'Locust itself'}, so it is not this run's to end. Leave it running.`
    }
    const entry = protectedAs(one)
    if (entry === undefined) continue
    // Whose it is. An agent's CLI is this run's only if this run's agent started it (a helper of its own); anything
    // else a teammate started -- under Locust, in this message or an earlier one -- a teammate may end.
    if (entry.agent === true ? startedUnder(seen.table, pid, agent) : startedUnder(seen.table, pid, seen.root)) continue
    if (entry.browser === true && windowless(seen.table, pid)) continue
    return entry.agent === true
      ? `Locust stopped this command before it ran: process ${String(pid)} is ${entry.what}, which this run did not start, so it is another teammate's or the person's. Leave it running.`
      : `Locust stopped this command before it ran: process ${String(pid)} is ${entry.what}, which no teammate started, so it is ${OWNER[entry.harm]}. Leave it running.${entry.browser === true ? ` ${BROWSER_TIP}` : ''}`
  }
  return undefined
}

/**
 * The process table, and where this run sits in it: `root` is Locust
 * (LOCUST_GUARD_PARENT), under which every teammate's work runs; `agent` is
 * the agent Locust started for this run (the ancestor of this hook whose
 * parent is Locust); `above` is the hook's ancestors, from its shell up.
 * Undefined when it cannot be read.
 */
export function lookAtProcesses() {
  const table = new Map()
  if (process.platform === 'win32') {
    const script =
      "Get-CimInstance Win32_Process | ForEach-Object { [string]$_.ProcessId + [char]9 + [string]$_.ParentProcessId + [char]9 + [string]$(if ($_.CreationDate) { ([DateTimeOffset]$_.CreationDate).ToUnixTimeMilliseconds() } else { 0 }) + [char]9 + $_.Name + [char]9 + (([string]$_.CommandLine) -replace '\\s+', ' ') }"
    const read = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { encoding: 'utf8', timeout: 8000, windowsHide: true, maxBuffer: 64 * 1024 * 1024 })
    if (read.status !== 0 || typeof read.stdout !== 'string') return undefined
    for (const line of read.stdout.split(/\r?\n/)) {
      const [pid, ppid, created, name, ...rest] = line.split('\t')
      if (!/^\d+$/.test(pid ?? '') || !/^\d+$/.test(ppid ?? '')) continue
      table.set(Number(pid), { ppid: Number(ppid), created: Number(created) || 0, name, line: rest.join('\t') })
    }
  } else {
    // etime is how long ago it started: [[dd-]hh:]mm:ss.
    const names = spawnSync('ps', ['-A', '-o', 'pid=', '-o', 'ppid=', '-o', 'etime=', '-o', 'comm='], { encoding: 'utf8', timeout: 8000, maxBuffer: 64 * 1024 * 1024 })
    const lines = spawnSync('ps', ['-A', '-o', 'pid=', '-o', 'command='], { encoding: 'utf8', timeout: 8000, maxBuffer: 64 * 1024 * 1024 })
    if (names.status !== 0 || typeof names.stdout !== 'string') return undefined
    const commandOf = new Map()
    for (const line of String(lines.stdout ?? '').split('\n')) {
      const m = /^\s*(\d+)\s+(.*)$/.exec(line)
      if (m !== null) commandOf.set(Number(m[1]), m[2])
    }
    const now = Date.now()
    for (const line of names.stdout.split('\n')) {
      const m = /^\s*(\d+)\s+(\d+)\s+(?:(\d+)-)?(?:(\d+):)?(\d+):(\d+)\s+(.*)$/.exec(line)
      if (m === null) continue
      const ago = ((Number(m[3] ?? 0) * 24 + Number(m[4] ?? 0)) * 60 + Number(m[5])) * 60 + Number(m[6])
      table.set(Number(m[1]), { ppid: Number(m[2]), created: now - ago * 1000, name: m[7], line: commandOf.get(Number(m[1])) ?? '' })
    }
  }
  if (table.size === 0) return undefined
  const above = []
  let at = table.get(process.pid)?.ppid
  for (let step = 0; step < 64 && at !== undefined && at > 0 && !above.includes(at); step += 1) {
    above.push(at)
    at = table.get(at)?.ppid
  }
  // Locust, whose teammates' work is all under it; and this run's agent, the ancestor Locust started.
  const locust = Number(process.env.LOCUST_GUARD_PARENT)
  const known = Number.isInteger(locust) && locust > 0 && table.has(locust) && above.includes(locust)
  const agent = known ? above.find((pid) => table.get(pid)?.ppid === locust) : above[1]
  // Without Locust's id (a hook run some other way), the shell's parent: the agent that ran the hook.
  const root = known ? locust : (above[1] ?? above[0])
  if (root === undefined) return undefined
  return { table, root, agent: agent ?? root, above }
}

/** The hook: one PreToolUse event on stdin; a deny on stdout, or nothing. */
async function main() {
  let input = ''
  process.stdin.setEncoding('utf8')
  for await (const chunk of process.stdin) input += chunk
  let event
  try {
    event = JSON.parse(input)
  } catch {
    return
  }
  // Where it runs: the runtime's own word for it, and this hook's folder, which each runtime starts it in.
  const reason = stoppedBecause(event?.tool_input?.command, lookAtProcesses, [event?.cwd, event?.tool_input?.workdir, process.cwd()])
  // For a drive only (0.720): a line per question, so a live run can show the
  // guard was asked at all. Codex reports its hooks in no record Locust keeps.
  if (typeof process.env.LOCUST_GUARD_LOG === 'string' && process.env.LOCUST_GUARD_LOG.length > 0) {
    try {
      appendFileSync(process.env.LOCUST_GUARD_LOG, `${JSON.stringify({ tool: event?.tool_name, command: String(event?.tool_input?.command ?? '').slice(0, 200), refused: reason !== undefined })}\n`)
    } catch {}
  }
  if (reason === undefined) return
  process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny', permissionDecisionReason: reason } }))
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().then(
    () => process.exit(0),
    () => process.exit(0)
  )
}
