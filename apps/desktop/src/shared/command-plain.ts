/**
 * WHAT A COMMAND DOES, IN PLAIN WORDS (0.840).
 *
 * An approval card for a command said "Run a command" and showed the command:
 * `git push --force origin main` is exact, and to someone who does not use Git
 * it says nothing about the commits on GitHub it throws away. The phone said
 * less -- "Run a command" and Allow once. This reads the command the way
 * command-reach.ts does and says, in a sentence, what the familiar ones are
 * for: a push, a commit, a delete, an install, a script downloaded and run.
 *
 * Worked out from the words of the command, never asked of a model: the card
 * shows only what the request itself says (ApprovalCard.tsx), and a model
 * reading a command can be talked into calling it safe by the command.
 *
 * It speaks only when it knows EVERY part. A chain with one part it cannot
 * read -- `npm test && ./deploy.sh` -- gets no plain words at all, since a
 * sentence about the test alone would read as the whole of it. So does a part
 * that runs something inside `$( )` or `( )`, hands a filter a `{ }` block of
 * code, or is longer than four steps -- twelve when every one only looks, three
 * of them said and the rest counted. Silence is the honest answer for those,
 * and the card's exact row still says what will run.
 *
 * Measured against the 9,834 different commands the agents ran here by
 * 2026-10-28: Codex's long chains of reads, `Get-Content -LiteralPath` and a
 * script's output saved to a log were most of what it could not read.
 *
 * Pure and dependency-free, as command-reach.ts is: the main process (the
 * phone, the chat apps) and the renderer both use it.
 */
import { commandWords, innerCommands, programName, splitChain, tokens, wrapped } from './command-reach.js'

export interface PlainCommand {
  /** A sentence a step, for a person. */
  readonly said: string
  /** Worth a second look: a delete, a forced push, changes thrown away, a script downloaded and run unseen. */
  readonly careful: boolean
  /** Every part only looks: shows, lists or searches. */
  readonly onlyLooks: boolean
}

interface Step {
  readonly said: string
  readonly looks?: true
  readonly careful?: true
}

const MAX_STEPS = 4
/** A chain that only looks may be longer: Codex reads five files and a diff in one command. */
const MAX_LOOKING_STEPS = 12
/** Of a longer chain, the steps said before the rest are counted. */
const SAID_OF_A_LONG_ONE = 3
const ONLY_LOOKS = ' It only looks.'
const NO_BIN = ' Nothing goes to the Recycle Bin or Trash.'

export function plainCommand(command: string | undefined): PlainCommand | undefined {
  if (typeof command !== 'string' || command.trim().length === 0 || command.length > 2000) return undefined
  // An escaped quote -- `\"` to bash and to a Windows command line, `` `" `` to PowerShell -- ends a quote here, so
  // the rest of a chain could read as a search's words: `rg 'a=\"b' src; pnpm build` said only "Searches".
  if (/\\"|`["']/.test(command) || !closed(command)) {
    // Still said when it is a download run at once, whatever the quotes: that one most needs saying.
    const url = /\b(?:curl|wget|irm|iwr|invoke-restmethod|invoke-webrequest)\b[^|;&]*?(https?:\/\/[^\s'"\\)]+)[^|;&]*\|\s*(?:sh|bash|zsh|iex|invoke-expression)\b/i.exec(command)?.[1]
      ?? (/\b(?:iex|invoke-expression)\b/i.test(command) ? runsDownloaded('iex', command) : undefined)
    if (url === undefined) return undefined
    const said = downloadAndRun(url).said
    return { said, careful: true, onlyLooks: false }
  }
  const all = stepsOf(command, 0)
  if (all === undefined || all.length > MAX_LOOKING_STEPS) return undefined
  // A step that says nothing worth a word (`exit 0`) is not one.
  const steps = once(all.filter((step) => step.said.length > 0))
  if (steps.length === 0) return undefined
  const onlyLooks = steps.every((step) => step.looks === true)
  // Only a chain that changes nothing is cut short: a step left unsaid could be the one that deletes.
  if (steps.length > MAX_STEPS && !onlyLooks) return undefined
  const shown = steps.length > MAX_STEPS
    ? [...steps.slice(0, SAID_OF_A_LONG_ONE).map((step) => step.said), `And ${String(steps.length - SAID_OF_A_LONG_ONE)} more step${steps.length - SAID_OF_A_LONG_ONE === 1 ? '' : 's'}.`]
    : steps.map((step) => step.said)
  return {
    said: `${shown.join(' ')}${onlyLooks ? ONLY_LOOKS : ''}`,
    careful: steps.some((step) => step.careful === true),
    onlyLooks
  }
}

/** Every quote that opens closes: one left open takes the rest of the chain into its words. */
function closed(command: string): boolean {
  let quote: string | undefined
  for (const ch of command) {
    if (quote !== undefined) {
      if (ch === quote) quote = undefined
    } else if (ch === '"' || ch === "'") quote = ch
  }
  return quote === undefined
}

/** The same step run again and again -- one script driven with different words -- said once, with how often. */
function once(steps: readonly Step[]): Step[] {
  const kept: { step: Step; times: number }[] = []
  for (const step of steps) {
    const last = kept[kept.length - 1]
    // Only a one-sentence step: a count after a second sentence would read as belonging to it.
    if (last !== undefined && last.step.said === step.said && !/\.\s/.test(step.said)) {
      last.times += 1
      // Amber if either was: said once, it still stands for both.
      if (step.careful === true) last.step = { ...last.step, careful: true }
      if (step.looks !== true) last.step = { said: last.step.said, ...(last.step.careful === true ? { careful: true } : {}) }
    } else kept.push({ step, times: 1 })
  }
  return kept.map(({ step, times }) => (times === 1 ? step : { ...step, said: `${step.said.replace(/\.$/, '')}, ${times === 2 ? 'twice' : `${String(times)} times`}.` }))
}

function stepsOf(command: string, depth: number): Step[] | undefined {
  if (depth > 3) return undefined
  const parts = splitChain(command)
  const steps: Step[] = []
  for (let i = 0; i < parts.length; i += 1) {
    const set = setting(parts[i]!.text)
    if (set !== undefined && parts[i + 1]?.after !== '|') {
      steps.push(set)
      continue
    }
    let end = i
    while (end + 1 < parts.length && parts[end + 1]!.after === '|') end += 1
    const found = pipeline(parts.slice(i, end + 1).map((part) => part.text), depth)
    if (found === undefined) return undefined
    // `a || b`: b runs only when a fails.
    steps.push(...(parts[i]!.after === '||' ? found.map((step, at) => (at === 0 ? { ...step, said: `If that fails, ${lowerFirst(step.said)}` } : step)) : found))
    i = end
  }
  return steps
}

/**
 * A PowerShell line that only sets something for the lines after it. The exit code kept for later and how errors
 * are taken say nothing worth a word; an environment variable is named, since it can change what the next command
 * does, and never its value.
 */
function setting(text: string): Step | undefined {
  if (/^\s*\$\w+\s*=\s*\$(LASTEXITCODE|\?)\s*$/i.test(text)) return { said: '', looks: true }
  if (/^\s*\$ErrorActionPreference\s*=\s*['"]?\w+['"]?\s*$/i.test(text)) return { said: '', looks: true }
  const env = /^\s*\$env:([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:'[^']*'|"[^"$`]*"|[^\s;|&()$`'"]+)\s*$/i.exec(text)
  return env === null ? undefined : { said: `Sets ${env[1]!} for the commands after it.` }
}

// --- one pipeline: a command, or commands piped into each other -----------------

function pipeline(texts: readonly string[], depth: number): Step[] | undefined {
  // `NODE_ENV=production node build.mjs`: the setting is named, as a PowerShell one is.
  const sets = texts.length === 1 ? leadingSettings(tokens(texts[0]!)) : []
  const words = texts.map((text) => withoutRedirects(commandWords(tokens(text))))
  if (words.some((each) => each.length === 0)) return undefined
  // `command -v git` asks where git is; read as `git` it would be explained as git.
  if (texts.some((text) => /^\s*(command|builtin)\s/i.test(text))) return undefined
  // A download run at once is said before anything else is weighed: `iex (irm …)` is all brackets.
  const ran = texts.length === 1 ? runsDownloaded(programName(words[0]![0]!), texts[0]!) : undefined
  if (ran !== undefined) return [downloadAndRun(ran)]
  // `( )` runs what is inside it -- `Remove-Item (Join-Path $env:TEMP x)`, a subshell -- and a `{ }` handed to a
  // filter is code: `gci | ? { Remove-Item $_ }` lists nothing.
  if (texts.some(computes)) return undefined
  // What it prints, saved in a file: said after it, and from the last command only.
  if (texts.slice(0, -1).some((text) => redirects(text)?.length !== 0)) return undefined
  const saved = redirects(texts[texts.length - 1]!)
  if (saved === undefined || saved.length > 1) return undefined
  const found = commands(texts, words, depth)
  if (found === undefined) return undefined
  return [...sets.map((name) => ({ said: `Sets ${name} for this command.` })), ...found, ...saved.map(savedIn)]
}

function commands(texts: readonly string[], words: readonly (readonly string[])[], depth: number): Step[] | undefined {
  const last = words[words.length - 1]!
  // A script downloaded and run at once: `curl … | sh`, `irm … | iex`, `bash -c "$(curl …)"`, `iex (irm …)`.
  if (texts.length > 1 && RUNNERS.has(programName(last[0]!))) {
    const url = downloaded(words[0]!)
    if (url !== undefined) return [downloadAndRun(url)]
    return undefined
  }
  if (texts.length === 1) {
    const verb = programName(words[0]![0]!)
    if (innerCommands(texts[0]!).length > 0) return undefined
    const inner = wrapped(words[0]!)
    if (inner !== undefined) return stepsOf(inner, depth + 1)
    // `bash _tools/gate.sh`, `powershell -File setup.ps1`: a script file, as `node x.mjs` is.
    const file = ['bash', 'sh', 'zsh'].includes(verb) ? words[0]![1] : ['powershell', 'pwsh'].includes(verb) ? valueOf(words[0]!.slice(1), ['-file']) : undefined
    if (file !== undefined && !file.startsWith('-')) return [{ said: `Runs the script ${file}.` }]
    if (['powershell', 'pwsh', 'cmd', 'bash', 'sh', 'zsh'].includes(verb)) return undefined
    const step = stepOf(words[0]!)
    if (step === undefined) return undefined
    return [/^\s*(sudo|doas)\s/i.test(texts[0]!) ? { ...step, said: `${step.said} It runs with administrator rights.` } : step]
  }
  // A command whose output is only narrowed: `git log | head -20`, `npm test 2>&1 | tail -40`, `gci | Select-Object Name`.
  if (texts.some((text) => innerCommands(text).length > 0)) return undefined
  const first = stepOf(words[0]!)
  if (first === undefined) return undefined
  const rest = words.slice(1)
  if (!rest.every((each) => FILTERS.has(programName(each[0]!)) && !writesFromAFilter(each))) return undefined
  // `Get-Content app.js | Select-Object -Skip 760 -First 40` reads forty lines of it, not all of it.
  const narrowed = rest.some((each) => ['head', 'tail'].includes(programName(each[0]!)) || ['select-object', 'select'].includes(programName(each[0]!)) && each.some((word) => /^-(first|last|skip|index)$/i.test(word)))
  return [narrowed && first.said.startsWith('Shows what is in ') ? { ...first, said: `Shows part of ${first.said.slice('Shows what is in '.length)}` } : first]
}

/** Names set in front of a command for it alone: `CI=1 NODE_ENV=production node build.mjs`. Never their values. */
function leadingSettings(words: readonly string[]): string[] {
  const names: string[] = []
  for (const word of words) {
    const set = /^([A-Za-z_][A-Za-z0-9_]*)=/.exec(word)
    if (set === null) break
    names.push(set[1]!)
  }
  return names
}

/** A `(` or a `{` outside quotes: something worked out or run before the command is. `HEAD@{1}` is a commit. */
function computes(text: string): boolean {
  const bare = text.replace(/\w@\{[\w.:-]+\}/g, '')
  let quote: string | undefined
  for (const ch of bare) {
    if (quote !== undefined) {
      if (ch === quote) quote = undefined
      continue
    }
    if (ch === '"' || ch === "'") quote = ch
    else if (ch === '(' || ch === '{') return true
  }
  return false
}

interface Saved {
  readonly file: string
  readonly adds: boolean
  readonly errors: boolean
}

function savedIn(saved: Saved): Step {
  const what = saved.errors ? 'Its error messages' : 'What it prints'
  if (saved.adds) return { said: `${what} ${saved.errors ? 'are' : 'is'} added to the end of ${saved.file}.` }
  // Replacing a log is what a log is for; replacing any other file loses what was in it.
  if (/\.(log|tap|out)$/i.test(saved.file)) return { said: `${what} ${saved.errors ? 'are' : 'is'} saved in ${saved.file}.` }
  return { said: `${what} ${saved.errors ? 'replace' : 'replaces'} what was in ${saved.file}.`, careful: true }
}

/** A filter told to write a file: `sed -i`, `sort -o out.txt`. */
function writesFromAFilter(words: readonly string[]): boolean {
  const name = programName(words[0]!)
  if (name === 'sed') return words.some((word) => /^-[a-z]*i/.test(word) || word.startsWith('--in-place'))
  if (name === 'sort') return words.some((word) => word === '-o' || word.startsWith('--output') || /^-[a-z]*o/.test(word))
  return false
}

const RUNNERS = new Set(['sh', 'bash', 'zsh', 'dash', 'iex', 'invoke-expression', 'python', 'python3', 'node', 'powershell', 'pwsh'])
const FILTERS = new Set([
  'head', 'tail', 'grep', 'egrep', 'rg', 'findstr', 'select-string', 'sls', 'sort', 'sort-object', 'uniq', 'wc', 'cut', 'tr', 'less', 'more',
  'select-object', 'select', 'where-object', '?', 'measure-object', 'measure', 'format-table', 'ft', 'format-list', 'fl', 'out-string', 'out-host',
  'out-null', 'jq', 'column', 'cat', 'sed', 'group-object', 'convertfrom-json', 'convertto-json', 'nl', 'where'
])

/** The URL a download command fetches, when the words are one: curl, wget, irm, iwr. */
function downloaded(words: readonly string[]): string | undefined {
  if (!DOWNLOADERS.has(programName(words[0]!))) return undefined
  return words.slice(1).find((word) => /^https?:\/\//i.test(word))
}

const DOWNLOADERS = new Set(['curl', 'wget', 'irm', 'iwr', 'invoke-restmethod', 'invoke-webrequest'])

/**
 * A shell or `iex` handed a download to run in one part: `bash -c "$(curl -fsSL …)"`, `bash <(curl …)`,
 * `iex (irm …)`. A shell only when the download is inside `$( )` or `<( )`: `bash -c "curl -o x …"` only downloads.
 */
function runsDownloaded(verb: string, text: string): string | undefined {
  if (['sh', 'bash', 'zsh', 'dash'].includes(verb)) {
    return /(?:\$\(|<\()\s*(?:curl|wget)\b[^)]*?(https?:\/\/[^\s'")]+)/i.exec(text)?.[1]
  }
  if (verb !== 'iex' && verb !== 'invoke-expression') return undefined
  // Whatever iex is given runs: `iex (irm …)`, `iex ((New-Object Net.WebClient).DownloadString('https://…'))`.
  const fetched = /\b(?:curl|wget|irm|iwr|invoke-restmethod|invoke-webrequest)\b[^)]*?(https?:\/\/[^\s'")]+)/i.exec(text)
  return fetched?.[1] ?? /downloadstring\(\s*['"](https?:\/\/[^'"]+)['"]/i.exec(text)?.[1]
}

function downloadAndRun(url: string): Step {
  return { said: `Downloads a script from ${hostOf(url)} and runs it straight away, without showing it to you first.`, careful: true }
}

// --- one command -------------------------------------------------------------

function stepOf(words: readonly string[]): Step | undefined {
  const verb = programName(words[0]!)
  const args = words.slice(1)
  const version = versionOf(verb, args)
  if (version !== undefined) return version
  switch (verb) {
    case 'git':
      return git(args)
    case 'gh':
      return gh(args)
    case 'npm':
    case 'pnpm':
    case 'yarn':
    case 'bun':
      return packages(verb, args)
    case 'npx':
    case 'bunx':
      return runPackage(args)
    case 'pip':
    case 'pip3':
      return pip(args)
    case 'uv':
      return args[0] === 'pip' ? pip(args.slice(1)) : undefined
    case 'python':
    case 'python3':
    case 'py':
      return python(args)
    case 'node':
    case 'deno':
      return script(args)
    case 'pytest':
      return { said: 'Runs this project’s Python tests.' }
    case 'vitest':
    case 'jest':
      return { said: 'Runs this project’s tests.' }
    case 'cargo':
      return cargo(args)
    case 'go':
      return goTool(args)
    case 'tsc':
      return args.includes('--noEmit') ? { said: 'Checks this project’s TypeScript for mistakes.', looks: true } : undefined
    case 'rm':
    case 'del':
    case 'erase':
    case 'rd':
    case 'rmdir':
    case 'ri':
    case 'remove-item':
    case 'unlink':
      return remove(verb, args)
    case 'mv':
    case 'move':
    case 'mi':
    case 'move-item':
      return move(verb, args)
    case 'ren':
    case 'rename':
    case 'rni':
    case 'rename-item':
      return rename(args)
    case 'cp':
    case 'copy':
    case 'cpi':
    case 'copy-item':
    case 'xcopy':
      return copy(verb, args)
    case 'robocopy':
      return robocopy(args)
    case 'mkdir':
    case 'md':
      return named(plainOf(args, verb), (folders) => `Makes the folder${plural(folders)} ${join(folders)}.`)
    case 'new-item':
    case 'ni':
      return newItem(args)
    case 'touch':
      return named(plainOf(args, verb), (files) => `Makes ${join(files)} if ${files.length === 1 ? 'it is' : 'they are'} not there yet.`)
    case 'type':
      // `type git` in a POSIX shell says what git is; Windows' `type` shows a file.
      return plainOf(args, verb).every((word) => /[./\\]/.test(word)) ? looks(named(plainOf(args, verb), (files) => `Shows what is in ${join(files)}.`)) : undefined
    case 'cat':
    case 'gc':
    case 'get-content':
    case 'less':
    case 'more': {
      // `Get-Content -LiteralPath x -Tail 20`: the file is a parameter's value, and a part of it may be all that is read.
      const part = valueOf(args, ['-tail', '-last']) !== undefined ? 'the end of' : valueOf(args, ['-totalcount', '-head', '-first']) !== undefined ? 'the start of' : 'what is in'
      return looks(named([...valuesOf(args, ['-path', '-literalpath']), ...plainOf(args, verb)].flatMap(commaList), (files) => `Shows ${part} ${join(files)}.`))
    }
    case 'test-path':
      return looks(named([...valuesOf(args, ['-path', '-literalpath']), ...plainOf(args, verb)].flatMap(commaList), (files) => `Checks whether ${join(files)} ${files.length === 1 ? 'is' : 'are'} there.`))
    case 'get-date':
    case 'date':
      // GNU `date -s` sets the clock.
      return args.some((arg) => arg === '-s' || arg.startsWith('--set')) ? undefined : { said: 'Shows the date and time.', looks: true }
    case 'sleep':
    case 'start-sleep':
      return waits(args)
    case 'get-process':
    case 'gps': {
      // `Get-Process node`, `Get-Process -Id 4120`: the ones named, not every program.
      const which = [...valuesOf(args, ['-id', '-name']), ...plainOf(args, verb, ['-id'])].flatMap(commaList)
      return { said: which.length === 0 ? 'Lists the programs running on this computer.' : `Looks up the running program${plural(which)} ${join(which)}.`, looks: true }
    }
    case 'ps':
    case 'tasklist':
      return { said: 'Lists the programs running on this computer.', looks: true }
    case 'get-ciminstance':
    case 'gcim':
    case 'get-wmiobject':
    case 'gwmi': {
      const kind = (valueOf(args, ['-classname', '-class']) ?? plainOf(args, verb)[0] ?? '').toLowerCase()
      if (kind === 'win32_process') return { said: 'Lists the programs running on this computer.', looks: true }
      if (kind === 'win32_logicaldisk') return { said: 'Shows how much room is left on each drive.', looks: true }
      return undefined
    }
    case 'get-psdrive':
    case 'gdr':
    case 'df':
      return { said: 'Shows how much room is left on each drive.', looks: true }
    case 'head':
    case 'tail':
      return looks(named(plainOf(args, verb, ['-n', '-c']), (files) => `Shows the ${verb === 'head' ? 'start' : 'end'} of ${join(files)}.`))
    case 'ls':
    case 'dir':
    case 'gci':
    case 'get-childitem':
    case 'tree':
      return listing(verb, args)
    case 'find':
      return args.some((arg) => /^-(delete|exec|execdir|ok|okdir|fprint\w*|fls)$/.test(arg)) ? undefined : { said: `Looks for files in ${place(plainOf(args, verb)[0])}.`, looks: true }
    case 'grep':
    case 'egrep':
    case 'rg':
    case 'ag':
    case 'findstr':
    case 'sls':
    case 'select-string':
      return search(verb, args)
    case 'pwd':
    case 'gl':
    case 'get-location':
      return { said: 'Shows which folder this is.', looks: true }
    case 'echo':
    case 'printf':
    case 'write-output':
    case 'write-host':
      return { said: 'Prints a line of text.', looks: true }
    case 'which':
    case 'where':
    case 'gcm':
    case 'get-command':
      return looks(named(plainOf(args, verb), (programs) => `Finds where ${join(programs)} ${programs.length === 1 ? 'is' : 'are'} installed.`))
    case 'cd':
    case 'chdir':
    case 'pushd':
    case 'sl':
    case 'set-location': {
      const to = plainOf(args, verb)[0]
      return to === undefined ? undefined : { said: to === '..' ? 'Goes up one folder.' : `Goes into ${to}.`, looks: true }
    }
    case 'exit':
      return args.length <= 1 ? { said: '', looks: true } : undefined
    case 'start':
    case 'open':
    case 'xdg-open':
    case 'start-process':
    case 'saps':
      return opens(args)
    case 'curl':
    case 'wget':
    case 'irm':
    case 'iwr':
    case 'invoke-restmethod':
    case 'invoke-webrequest':
      return fetches(verb, args)
    default:
      return undefined
  }
}

const VERSIONED = new Set(['node', 'npm', 'pnpm', 'yarn', 'bun', 'deno', 'python', 'python3', 'py', 'pip', 'pip3', 'git', 'gh', 'cargo', 'rustc', 'go', 'java', 'tsc', 'uv', 'docker', 'code', 'claude', 'codex', 'opencode', 'cursor-agent', 'gemini'])

function versionOf(verb: string, args: readonly string[]): Step | undefined {
  if (!VERSIONED.has(verb) || args.length !== 1 || !['--version', '-v', '-V', 'version', '-version'].includes(args[0]!)) return undefined
  return { said: `Shows which version of ${verb} is installed.`, looks: true }
}

// --- git ---------------------------------------------------------------------

function git(all: readonly string[]): Step | undefined {
  // Options before the subcommand: `git -C path`, `git -c key=value`, `git --no-pager`.
  let at = 0
  while (at < all.length && all[at]!.startsWith('-')) at += all[at] === '-C' || all[at] === '-c' ? 2 : 1
  const sub = all[at]
  const args = all.slice(at + 1)
  const plain = plainOf(args, 'git', ['-m', '--message', '-b', '-B', '-c', '-C', '-u', '--set-upstream-to', '--author', '--format', '--pretty', '-n', '--depth', '-o', '--origin'])
  const has = (...flags: readonly string[]): boolean => args.some((arg) => flags.includes(arg) || flags.some((flag) => flag.startsWith('--') && arg.startsWith(`${flag}=`)))
  switch (sub) {
    case 'status':
      return { said: 'Shows which files have changed.', looks: true }
    case 'diff':
      if (has('--check')) return { said: 'Checks the changes for stray spaces and other whitespace mistakes.', looks: true }
      return { said: has('--staged', '--cached') ? 'Shows the changes marked for the next commit.' : 'Shows what has changed in the files.', looks: true }
    case 'grep':
      return search('git grep', args)
    case 'worktree':
      return args[0] === 'list' ? { said: 'Lists the folders this repository is checked out in.', looks: true } : undefined
    case 'log':
    case 'shortlog':
    case 'reflog':
      return args[0] === 'expire' || args[0] === 'delete' ? undefined : { said: 'Lists recent commits.', looks: true }
    case 'show': {
      // `git show main:src/cart.ts` is a file as it was, not a commit.
      const at = plain.length === 1 ? /^([^:\s]{2,}):(.+)$/.exec(plain[0]!) : null
      return { said: at === null ? 'Shows a commit and what it changed.' : `Shows ${at[2]!} as it was at ${at[1]!}.`, looks: true }
    }
    case 'blame':
      return { said: `Shows who last changed each line of ${plain[0] ?? 'a file'}.`, looks: true }
    case 'ls-files':
    case 'rev-parse':
    case 'describe':
      return { said: 'Reads what Git knows about this folder.', looks: true }
    case 'remote':
      return args.length === 0 || has('-v', '--verbose') && args.length === 1 ? { said: 'Lists where this repository sends and gets commits.', looks: true } : undefined
    case 'add':
      return plain.length === 0 && !has('-A', '--all', '-u', '--update')
        ? undefined
        : { said: has('-A', '--all') || plain.includes('.') ? 'Marks every change in this folder to go in the next commit.' : has('-u', '--update') ? 'Marks every change to files Git already tracks to go in the next commit.' : `Marks ${join(plain)} to go in the next commit.` }
    case 'commit': {
      const message = valueOf(args, ['-m', '--message', '-am'])
      const withMessage = message === undefined ? '' : ` with the message “${cut(message.split('\n')[0]!, 72)}”`
      if (has('--amend')) return { said: `Rewrites the last commit${withMessage}, on this computer only.` }
      return { said: `Saves ${has('-a', '--all') || args.some((arg) => /^-[a-zA-Z]*a[a-zA-Z]*$/.test(arg)) ? 'every change to tracked files' : 'the marked changes'} as a commit${withMessage}, on this computer only.` }
    }
    case 'push':
      return push(args)
    case 'pull':
      return { said: `Gets new commits from ${plain[0] ?? 'the repository this branch is linked to'} and ${has('--rebase', '-r') ? 'puts this branch’s own commits on top of them' : 'merges them into this branch'}.` }
    case 'fetch':
      return { said: `Gets new commits from ${has('--all') ? 'every linked repository' : plain[0] ?? 'the linked repository'} without changing your files.` }
    case 'clone': {
      const from = plain[0]
      return from === undefined ? undefined : { said: `Downloads a copy of the repository ${repositoryOf(from)} into ${plain[1] === undefined ? 'a new folder here' : `the folder ${plain[1]}`}.` }
    }
    case 'switch': {
      const made = valueOf(args, ['-c', '-C', '--create'])
      if (made !== undefined) return { said: `Makes a new branch, ${made}, and switches to it.` }
      return plain[0] === undefined ? undefined : { said: `Switches to the branch ${plain[0]}.` }
    }
    case 'checkout':
      return checkout(args, plain)
    case 'restore':
      if (plain.length === 0) return undefined
      return has('--staged', '-S') && !has('--worktree', '-W')
        ? { said: `Takes ${join(plain)} out of the next commit; the changes stay in the files.` }
        : { said: `Throws away the changes to ${everything(plain)} that are not committed. They cannot be brought back.`, careful: true }
    case 'reset': {
      if (has('--hard')) return { said: `Throws away every change not committed, in every file Git tracks${plain[0] === undefined ? '' : `, and moves this branch back to ${plain[0]}`}. It cannot be undone.`, careful: true }
      const files = args.includes('--') ? args.slice(args.indexOf('--') + 1) : plain.filter((one) => one !== 'HEAD' && looksLikeAFile(one))
      if (files.length > 0) return { said: `Takes ${join(files)} out of the next commit; the changes stay in the files.` }
      if (plain.length === 0 || plain.length === 1 && plain[0] === 'HEAD') return { said: 'Takes everything out of the next commit; the changes stay in the files.' }
      if (plain.length !== 1) return undefined
      return { said: `Moves this branch back to ${plain[0]!}; the changes since stay in the files${has('--soft') ? ', marked for the next commit' : ''}.` }
    }
    case 'clean':
      if (has('-n', '--dry-run') || args.some((arg) => /^-[a-z]*n[a-z]*$/.test(arg))) return { said: 'Lists the files Git does not track that it would delete.', looks: true }
      if (!args.some((arg) => /^-[a-z]*f[a-z]*$/.test(arg) || arg === '--force')) return undefined
      return { said: `Deletes every file Git does not track${args.some((arg) => /^-[a-z]*d[a-z]*$/.test(arg)) ? ', and folders' : ''}${args.some((arg) => /^-[a-z]*x[a-z]*$/i.test(arg)) ? ', ignored ones too' : ''}: new files never committed are gone for good.`, careful: true }
    case 'stash':
      switch (plain[0] ?? 'push') {
        case 'push':
        case 'save':
          return { said: 'Puts the changes not committed aside, to bring back later.' }
        case 'pop':
        case 'apply':
          return { said: 'Brings back changes that were put aside.' }
        case 'list':
        case 'show':
          return { said: 'Lists changes that were put aside.', looks: true }
        case 'drop':
        case 'clear':
          return { said: `Throws away ${plain[0] === 'clear' ? 'every set of changes' : 'a set of changes'} that was put aside.`, careful: true }
        default:
          return undefined
      }
    case 'branch':
      return branch(args)
    case 'merge':
      return has('--abort') ? { said: 'Stops the merge under way and puts things back as they were before it.' } : plain[0] === undefined ? undefined : { said: `Merges the branch ${plain[0]} into this one.` }
    case 'rebase':
      return has('--abort') ? { said: 'Stops the rebase under way and puts the branch back as it was.' } : has('--continue') ? { said: 'Carries on with the rebase under way.' } : plain[0] === undefined ? undefined : { said: `Replays this branch’s commits on top of ${plain[0]}, rewriting them.` }
    case 'tag':
      return plain.length === 0 || has('-l', '--list') ? { said: 'Lists the tags.', looks: true } : has('-d', '--delete') ? { said: `Deletes the tag ${plain[0]!} on this computer.` } : { said: `Marks this commit with the tag ${plain[0]!}.` }
    case 'rm':
      if (plain.length === 0) return undefined
      return has('--cached')
        ? { said: `Stops Git tracking ${join(plain)}; the file${plural(plain)} stay${plain.length === 1 ? 's' : ''} on this computer.` }
        : { said: `Deletes ${join(plain)} and marks the deletion for the next commit.` }
    case 'mv':
      return plain.length === 2 ? { said: `Moves ${plain[0]!} to ${plain[1]!}, and tells Git.` } : undefined
    case 'init':
      return { said: 'Starts Git in this folder, so changes to it can be tracked.' }
    default:
      return undefined
  }
}

function push(args: readonly string[]): Step | undefined {
  const plain = plainOf(args, 'git', ['-o', '--push-option', '--repo'])
  const remote = plain[0]
  const refs = plain.slice(1)
  const where = remote ?? 'the repository this branch is linked to'
  const branchOf = (ref: string): string => ref.replace(/^\+/, '').split(':').pop()!.replace(/^refs\/heads\//, '')
  const branchName = refs[0] === undefined ? undefined : branchOf(refs[0])
  const which = branchName === undefined || branchName === 'HEAD' ? 'this branch' : branchName
  if (args.includes('--delete') || args.includes('-d') || refs.some((ref) => ref.startsWith(':'))) {
    return { said: `Deletes the branch ${which} on ${where}.`, careful: true }
  }
  if (args.some((arg) => arg.startsWith('--force-with-lease'))) {
    return { said: `Replaces ${which} on ${where} with this computer’s version, unless someone else changed it there since you last fetched. Commits on ${where} that are not on this computer are lost.`, careful: true }
  }
  if (args.some((arg) => arg === '--force' || arg === '-f' || /^-[a-zA-Z]*f[a-zA-Z]*$/.test(arg) && !arg.startsWith('--')) || refs.some((ref) => ref.startsWith('+'))) {
    return { said: `Replaces ${which} on ${where} with this computer’s version: commits there that are not on this computer are lost.`, careful: true }
  }
  if (args.includes('--tags')) return { said: `Sends every tag to ${where}.` }
  return { said: branchName === undefined || branchName === 'HEAD' ? `Sends this branch’s new commits to ${where}.` : `Sends the commits on ${branchName} to ${where}.` }
}

function checkout(args: readonly string[], plain: readonly string[]): Step | undefined {
  const made = valueOf(args, ['-b', '-B'])
  if (made !== undefined) return { said: `Makes a new branch, ${made}, and switches to it.` }
  const dashes = args.indexOf('--')
  const files = dashes >= 0 ? args.slice(dashes + 1) : plain.length === 1 && looksLikeAFile(plain[0]!) ? plain : []
  if (files.length > 0) return { said: `Throws away the changes to ${everything(files)} that are not committed. They cannot be brought back.`, careful: true }
  if (plain.length !== 1) return undefined
  // A commit rather than a branch: `HEAD~3`, `abc1234`.
  if (/^HEAD|[~^]|^[0-9a-f]{7,40}$/.test(plain[0]!)) return { said: `Puts the files as they were at ${plain[0]!}, outside any branch.` }
  return { said: `Switches to ${plain[0]!}. If ${plain[0]!} is a file rather than a branch, its changes not committed are thrown away.` }
}

function branch(args: readonly string[]): Step | undefined {
  const plain = args.filter((arg) => !arg.startsWith('-'))
  if (args.includes('--show-current')) return { said: 'Shows which branch this is.', looks: true }
  if (args.includes('-D') || args.includes('--delete') && args.includes('--force')) {
    return plain.length === 0 ? undefined : { said: `Deletes the branch${plural(plain)} ${join(plain)}, even if ${plain.length === 1 ? 'its' : 'their'} commits are on no other branch: those commits can be lost.`, careful: true }
  }
  if (args.includes('-d') || args.includes('--delete')) return plain.length === 0 ? undefined : { said: `Deletes the branch${plural(plain)} ${join(plain)}, only if ${plain.length === 1 ? 'its' : 'their'} commits are merged.` }
  if (args.includes('-m') || args.includes('-M')) return plain.length === 0 ? undefined : { said: `Renames the branch ${plain.length === 2 ? `${plain[0]!} to ${plain[1]!}` : `to ${plain[0]!}`}.` }
  if (plain.length === 0) return { said: 'Lists the branches.', looks: true }
  return plain.length === 1 ? { said: `Makes a new branch, ${plain[0]!}, without switching to it.` } : undefined
}

function gh(args: readonly string[]): Step | undefined {
  const [area, act] = args
  const number = args.slice(2).find((arg) => /^#?\d+$/.test(arg))
  const it = number === undefined ? '' : ` #${number.replace('#', '')}`
  const read = ['view', 'list', 'status', 'checks', 'diff', 'watch']
  if ((area === 'pr' || area === 'issue' || area === 'run' || area === 'release' || area === 'repo') && act !== undefined && read.includes(act)) {
    const one = number !== undefined && (area === 'pr' || area === 'issue')
    return { said: `Reads ${one ? `the ${area === 'pr' ? 'pull request' : 'issue'}${it}` : area === 'pr' ? 'pull requests' : area === 'issue' ? 'issues' : area === 'run' ? 'check runs' : area === 'release' ? 'releases' : 'the repository'} on GitHub.`, looks: true }
  }
  if (area === 'pr') {
    if (act === 'create') return { said: `Opens a pull request on GitHub${args.includes('--draft') ? ', as a draft' : ''}.` }
    if (act === 'merge') return { said: `Merges the pull request${it} on GitHub.` }
    if (act === 'close') return { said: `Closes the pull request${it} on GitHub.` }
    if (act === 'comment' || act === 'review') return { said: `Posts a ${act} on the pull request${it} on GitHub.` }
    if (act === 'checkout') return { said: `Gets the pull request${it} onto this computer and switches to it.` }
  }
  if (area === 'issue') {
    if (act === 'create') return { said: 'Opens an issue on GitHub.' }
    if (act === 'comment') return { said: `Posts a comment on the issue${it} on GitHub.` }
    if (act === 'close') return { said: `Closes the issue${it} on GitHub.` }
  }
  if (area === 'release' && act === 'create') return { said: 'Publishes a release on GitHub.' }
  if (area === 'repo' && act === 'create') return { said: 'Makes a new repository on GitHub.' }
  if (area === 'repo' && act === 'clone' && args[2] !== undefined) return { said: `Downloads a copy of the repository ${args[2]} from GitHub into a new folder here.` }
  if (area === 'run' && act === 'rerun') return { said: 'Runs the checks on GitHub again.' }
  if (area === 'auth' && act === 'status') return { said: 'Shows which GitHub account is signed in.', looks: true }
  return undefined
}

// --- packages and scripts ----------------------------------------------------

const SETUP_SCRIPTS = ' A package can run its own setup code as it installs.'
const BARE_SCRIPTS = new Set(['build', 'dev', 'lint', 'typecheck', 'format', 'preview', 'check', 'serve', 'watch'])

function packages(manager: string, args: readonly string[]): Step | undefined {
  const takesValue = manager === 'npm' ? ['--prefix', '-w', '--workspace'] : manager === 'pnpm' ? ['-C', '--dir', '--filter', '-F'] : ['--cwd']
  const plain = plainOf(args, manager, takesValue)
  const sub = plain[0]
  const rest = plain.slice(1)
  const global = args.includes('-g') || args.includes('--global') || manager === 'yarn' && sub === 'global'
  switch (sub) {
    case undefined:
      // Only a bare `yarn` installs; a bare npm, pnpm or bun prints its help.
      return manager === 'yarn' ? { said: `Installs the packages this project lists, downloading them from the internet.${SETUP_SCRIPTS}` } : undefined
    case 'install':
    case 'i':
    case 'add':
    case 'ci': {
      const names = sub === 'ci' ? [] : rest
      if (names.length === 0) return { said: `Installs the packages this project lists, downloading them from the internet.${SETUP_SCRIPTS}` }
      return { said: `Downloads ${join(names)} from the npm registry and installs ${names.length === 1 ? 'it' : 'them'} ${global ? 'for every project on this computer' : 'in this project'}.${SETUP_SCRIPTS}` }
    }
    case 'uninstall':
    case 'remove':
    case 'rm':
    case 'un':
      return rest.length === 0 ? undefined : { said: `Removes ${join(rest)} from ${global ? 'this computer' : 'this project'}.` }
    case 'test':
    case 't':
    case 'start':
      // `bun test` is Bun's own test runner, not a script.
      return manager === 'bun' && sub !== 'start' ? { said: 'Runs this project’s tests.' } : { said: projectScript(sub === 't' ? 'test' : sub) }
    case 'run':
    case 'run-script':
      return rest[0] === undefined ? { said: 'Lists this project’s scripts.', looks: true } : { said: projectScript(rest[0]) }
    case 'exec':
      return manager === 'npm' || rest[0] === undefined ? undefined : { said: `Runs ${rest[0]}, a program this project installed.` }
    case 'dlx':
      return rest[0] === undefined ? undefined : { said: `Downloads the package ${rest[0]} and runs it.` }
    case 'list':
    case 'ls':
      return { said: 'Lists the packages installed in this project.', looks: true }
    case 'outdated':
      return { said: 'Lists the packages that have newer versions.', looks: true }
    default:
      return manager !== 'npm' && BARE_SCRIPTS.has(sub) ? { said: projectScript(sub) } : undefined
  }
}

function projectScript(name: string): string {
  return `Runs this project’s “${name}” script: whatever its package.json says that is.`
}

function runPackage(args: readonly string[]): Step | undefined {
  const name = plainOf(args, 'npx', ['-p', '--package'])[0]
  return name === undefined ? undefined : { said: `Runs ${name}, downloading it first if this project does not have it.` }
}

function pip(args: readonly string[]): Step | undefined {
  const sub = args[0]
  const plain = plainOf(args.slice(1), 'pip', ['-r', '--requirement', '-e', '--editable', '-i', '--index-url', '-t', '--target'])
  switch (sub) {
    case 'install': {
      const listed = valueOf(args, ['-r', '--requirement'])
      if (listed !== undefined) return { said: `Downloads the Python packages listed in ${listed} and installs them.${SETUP_SCRIPTS}` }
      const editable = valueOf(args, ['-e', '--editable'])
      if (editable !== undefined || plain.length === 1 && plain[0] === '.') return { said: `Installs the Python project in ${place(editable)}.${SETUP_SCRIPTS}` }
      return plain.length === 0 ? undefined : { said: `Downloads ${join(plain)} from PyPI and installs ${plain.length === 1 ? 'it' : 'them'}.${SETUP_SCRIPTS}` }
    }
    case 'uninstall':
      return plain.length === 0 ? undefined : { said: `Removes the Python package${plural(plain)} ${join(plain)}.` }
    case 'list':
    case 'freeze':
    case 'show':
      return { said: 'Lists the Python packages installed.', looks: true }
    default:
      return undefined
  }
}

function python(args: readonly string[]): Step | undefined {
  if (args.includes('-c')) return undefined
  const module = valueOf(args, ['-m'])
  if (module !== undefined) {
    const after = args.slice(args.indexOf('-m') + 2)
    if (module === 'pip') return pip(after)
    if (module === 'pytest') return { said: 'Runs this project’s Python tests.' }
    const folder = after.find((arg) => !arg.startsWith('-'))
    if (module === 'venv') return folder === undefined ? undefined : { said: `Makes a Python environment in the folder ${folder}.` }
    return { said: `Runs the Python module ${module}.` }
  }
  return script(args)
}

function script(args: readonly string[]): Step | undefined {
  if (args.some((arg) => arg === '-e' || arg === '--eval' || arg === '-p' || arg === '--print')) return undefined
  const file = args.find((arg) => !arg.startsWith('-'))
  if (file === undefined) return undefined
  // `node --check app.js` reads the file and runs none of it.
  if (args.includes('--check') || args.includes('-c')) return { said: `Checks ${file} for mistakes in how it is written, without running it.`, looks: true }
  if (args.includes('--test')) return { said: `Runs the tests in ${join(args.filter((arg) => !arg.startsWith('-')))}.` }
  return { said: `Runs the script ${file}.` }
}

function cargo(args: readonly string[]): Step | undefined {
  switch (args[0]) {
    case 'test': return { said: 'Runs this Rust project’s tests.' }
    case 'build': return { said: 'Builds this Rust project.' }
    case 'run': return { said: 'Builds this Rust project and runs it.' }
    case 'check':
    case 'clippy': return { said: 'Checks this Rust project for mistakes.', looks: true }
    default: return undefined
  }
}

function goTool(args: readonly string[]): Step | undefined {
  switch (args[0]) {
    case 'test': return { said: 'Runs this Go project’s tests.' }
    case 'build': return { said: 'Builds this Go project.' }
    case 'vet': return { said: 'Checks this Go project for mistakes.', looks: true }
    case 'run': return args[1] === undefined ? undefined : { said: `Builds ${args[1]} and runs it.` }
    default: return undefined
  }
}

// --- files -------------------------------------------------------------------

/** PowerShell parameters that take a value: their value is not a file to act on. */
const PS_VALUES = ['-erroraction', '-ea', '-include', '-exclude', '-filter', '-itemtype', '-type', '-value', '-encoding', '-depth', '-first', '-last', '-tail', '-totalcount', '-head', '-newname', '-destination', '-path', '-literalpath', '-name', '-pattern', '-pathtype', '-classname', '-class', '-seconds', '-milliseconds', '-readcount', '-delimiter']

function remove(verb: string, args: readonly string[]): Step | undefined {
  const targets = [...(valuesOf(args, ['-path', '-literalpath'])), ...plainOf(args, verb)].flatMap(commaList)
  if (targets.length === 0) return undefined
  // `Remove-Item Env:LOCUST_X` takes a setting away; no file is touched.
  if (targets.every((target) => /^env:[A-Za-z_][A-Za-z0-9_]*$/i.test(target))) return { said: `Clears ${join(targets.map((target) => target.slice(4)))} for the commands after it.` }
  // `-r`, `-rf`, `-Re` (PowerShell takes the start of -Recurse); never `-Force`, which has an r in it.
  const deep = args.some((arg) => /^-recurse$/i.test(arg) || arg === '--recursive' || /^-[a-zA-Z]{1,3}$/.test(arg) && /r/i.test(arg) || /^\/s$/i.test(arg))
  // `rmdir build` without /s or -r takes a folder only when it is already empty.
  if ((verb === 'rmdir' || verb === 'rd') && !deep) return { said: `Deletes the folder${plural(targets)} ${join(targets)}, only if ${targets.length === 1 ? 'it is' : 'they are'} empty.` }
  if ((verb === 'del' || verb === 'erase') && deep) return { said: `Deletes ${join(targets)} in this folder and every folder inside it.${NO_BIN}`, careful: true }
  return { said: `Deletes ${join(targets)}${deep ? ` and everything inside ${targets.length === 1 ? 'it' : 'them'}` : ''}.${NO_BIN}`, careful: true }
}

function move(verb: string, args: readonly string[]): Step | undefined {
  const given = valuesOf(args, ['-path', '-literalpath']).flatMap(commaList)
  const to = valueOf(args, ['-destination'])
  const plain = plainOf(args, verb, ['-t', '--target-directory'])
  const dest = to ?? (given.length > 0 ? plain[0] : plain.length > 1 ? plain[plain.length - 1] : undefined)
  const from = [...given, ...(to === undefined && given.length === 0 ? plain.slice(0, -1) : to === undefined ? [] : plain)]
  if (from.length === 0 || dest === undefined) return undefined
  return { said: `Moves ${join(from)} to ${dest}.` }
}

function rename(args: readonly string[]): Step | undefined {
  const from = valueOf(args, ['-path', '-literalpath'])
  const to = valueOf(args, ['-newname'])
  const plain = plainOf(args, 'rename')
  const one = from ?? plain[0]
  const two = to ?? (from === undefined ? plain[1] : plain[0])
  return one === undefined || two === undefined ? undefined : { said: `Renames ${one} to ${two}.` }
}

function copy(verb: string, args: readonly string[]): Step | undefined {
  const given = valuesOf(args, ['-path', '-literalpath']).flatMap(commaList)
  const to = valueOf(args, ['-destination'])
  const plain = plainOf(args, verb, ['-t', '--target-directory'])
  const dest = to ?? (given.length > 0 ? plain[0] : plain.length > 1 ? plain[plain.length - 1] : undefined)
  const from = [...given, ...(to === undefined && given.length === 0 ? plain.slice(0, -1) : to === undefined ? [] : plain)]
  if (from.length === 0 || dest === undefined) return undefined
  return { said: `Copies ${join(from)} to ${dest}.` }
}

function robocopy(args: readonly string[]): Step | undefined {
  const plain = args.filter((arg) => !arg.startsWith('/'))
  const [from, to] = plain
  if (from === undefined || to === undefined) return undefined
  if (args.some((arg) => /^\/(mir|purge)$/i.test(arg))) return { said: `Makes ${to} match ${from} exactly: files in ${to} that ${from} does not have are deleted.`, careful: true }
  if (args.some((arg) => /^\/mov(e)?$/i.test(arg))) return { said: `Moves the files in ${from} to ${to}.` }
  return { said: `Copies the files in ${from} to ${to}.` }
}

function newItem(args: readonly string[]): Step | undefined {
  const type = valueOf(args, ['-itemtype', '-type'])?.toLowerCase()
  const path = [...valuesOf(args, ['-path', '-name']), ...plainOf(args, 'new-item')].flatMap(commaList)
  if (path.length === 0) return undefined
  if (type === 'directory') return { said: `Makes the folder${plural(path)} ${join(path)}.` }
  if (type !== undefined && type !== 'file') return undefined
  const force = args.some((arg) => /^-force$/i.test(arg))
  return force
    ? { said: `Makes the file${plural(path)} ${join(path)}. A file already there is replaced, and what was in it is lost.`, careful: true }
    : { said: `Makes the file${plural(path)} ${join(path)}.` }
}

function listing(verb: string, args: readonly string[]): Step | undefined {
  const where = [...valuesOf(args, ['-path', '-literalpath']), ...plainOf(args, verb)][0]
  const deep = args.some((arg) => /^-recurse$/i.test(arg) || arg === '-R' || /^\/s$/i.test(arg)) || verb === 'tree'
  return { said: verb === 'tree' ? `Shows the folders and files in ${place(where)} as a tree.` : `Lists the files in ${place(where)}${deep ? ' and every folder inside it' : ''}.`, looks: true }
}

function search(verb: string, args: readonly string[]): Step | undefined {
  const takesValue = ['-e', '-f', '-A', '-B', '-C', '-m', '-g', '-t', '-T', '--glob', '--type', '--max-count', '-pattern', '-path', '-include', '-exclude', '-context']
  // `rg --files -g AGENTS.md`: lists files, searching none of them.
  if (verb === 'rg' && args.includes('--files')) {
    const where = plainOf(args, verb, takesValue)
    const like = valuesOf(args, ['-g', '--glob', '--iglob'])
    const named = like.length > 0 && like.every((glob) => !glob.startsWith('!')) ? ` named like ${like.length === 1 ? like[0]! : `${like.slice(0, -1).map((one) => cut(one, 40)).join(', ')} or ${cut(like[like.length - 1]!, 40)}`}` : ''
    return { said: `Lists the files in ${where.length === 0 ? 'this folder' : join(where.map(place))}${named}.`, looks: true }
  }
  const pattern = valueOf(args, ['-e', '--regexp', '-pattern'])
  const plain = plainOf(args, verb, takesValue)
  const what = pattern ?? plain[0]
  if (what === undefined) return undefined
  const where = [...valuesOf(args, ['-path', '-literalpath']), ...(pattern === undefined ? plain.slice(1) : plain)]
  return { said: `Searches ${where.length === 0 ? 'this folder' : join(where.map(place))} for “${cut(what, 60)}”.`, looks: true }
}

function opens(args: readonly string[]): Step | undefined {
  const target = valueOf(args, ['-filepath']) ?? plainOf(args, 'start', ['-argumentlist', '-a']).find((arg) => arg.length > 0)
  if (target === undefined) return undefined
  if (/^https?:\/\//i.test(target)) return { said: `Opens ${hostOf(target)} in your web browser.` }
  return undefined
}

function waits(args: readonly string[]): Step | undefined {
  const ms = valueOf(args, ['-milliseconds', '-m'])
  if (ms !== undefined) return /^\d+$/.test(ms) ? { said: 'Waits a moment.', looks: true } : undefined
  const given = valueOf(args, ['-seconds', '-s']) ?? args.find((arg) => !arg.startsWith('-'))
  const time = given === undefined ? null : /^(\d+(?:\.\d+)?)([smhd]?)$/.exec(given)
  if (time === null) return undefined
  const seconds = Number(time[1]) * ({ '': 1, s: 1, m: 60, h: 3600, d: 86400 } as Record<string, number>)[time[2]!]!
  if (seconds < 120) return { said: `Waits ${String(Math.round(seconds))} second${Math.round(seconds) === 1 ? '' : 's'}.`, looks: true }
  if (seconds < 7200) return { said: `Waits about ${String(Math.round(seconds / 60))} minutes.`, looks: true }
  return { said: `Waits about ${String(Math.round(seconds / 3600))} hours.`, looks: true }
}

function fetches(verb: string, args: readonly string[]): Step | undefined {
  const url = valueOf(args, ['-uri']) ?? args.find((arg) => /^https?:\/\//i.test(arg))
  if (url === undefined || !/^https?:\/\//i.test(url)) return undefined
  const host = hostOf(url)
  const method = (valueOf(args, ['-X', '--request', '-method']) ?? '').toUpperCase()
  // Case matters for curl (`-f` fails quietly, `-F` sends a form); PowerShell's names take any case.
  const sends = method === 'POST' || method === 'PUT' || method === 'PATCH' || args.some((arg) => /^(-d|--data.*|-F|--form|-T|--upload-file|--json)$/.test(arg) || /^-(body|infile)$/i.test(arg))
  if (method === 'DELETE') return { said: `Asks ${host} to delete something.`, careful: true }
  if (sends) return { said: `Sends data to ${host}.` }
  const out = valueOf(args, ['-o', '--output', '-outfile', ...(verb === 'wget' ? ['-O', '--output-document'] : [])])
  if (out !== undefined && out !== '-') return { said: `Downloads ${shortUrl(url)} into ${out}.` }
  if (args.includes('-O') && verb === 'curl' || args.includes('--remote-name')) return { said: `Downloads ${shortUrl(url)} into this folder.` }
  if (verb === 'wget' && out === undefined) return { said: `Downloads ${shortUrl(url)} into this folder.` }
  return { said: `Fetches ${shortUrl(url)} and shows what comes back.` }
}

// --- reading the words --------------------------------------------------------

/**
 * The files a part's output goes into, outside quotes: `> out.txt`, `>> log`, `*> drive.log`. Not `2>&1`,
 * `> /dev/null` or `> $null`. Undefined when a target is worked out -- `> $env:TEMP\x.log` -- since where it goes
 * is then not in the words.
 */
function redirects(text: string): Saved[] | undefined {
  const found: Saved[] = []
  let quote: string | undefined
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i]!
    if (quote !== undefined) {
      if (ch === quote) quote = undefined
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      continue
    }
    if (ch !== '>') continue
    const errors = text[i - 1] === '2'
    const adds = text[i + 1] === '>'
    let at = i + 1
    while (text[at] === '>' || text[at] === ' ') at += 1
    const opener = text[at]
    const target = opener === '"' || opener === "'"
      ? text.slice(at, text.indexOf(opener, at + 1) + 1 || text.length)
      : /^(?:&\d?|[^\s;|&]*)/.exec(text.slice(at))![0]
    i = at + target.length - 1
    const file = target.replace(/^(["'])(.*)\1$/, '$2')
    if (/^(&\d?|\/dev\/null|\$null|nul)$/i.test(file)) continue
    if (file.length === 0 || /[$`(){}@*?]/.test(file)) return undefined
    found.push({ file, adds, errors })
  }
  return found
}

/** The words with redirections and what they point at taken out: `2>&1`, `> /dev/null`. */
function withoutRedirects(words: readonly string[]): string[] {
  const kept: string[] = []
  for (let i = 0; i < words.length; i += 1) {
    const word = words[i]!
    if (/^(\d|\*|&)?>>?$/.test(word)) {
      i += 1
      continue
    }
    if (/^(\d|\*|&)?>/.test(word)) continue
    kept.push(word)
  }
  return kept
}

/**
 * The arguments that are neither flags nor a flag's value. A Windows flag is `/x`; a PowerShell parameter that
 * takes a value (`-ErrorAction Stop`) takes the word after it with it.
 */
function plainOf(args: readonly string[], verb: string, takesValue: readonly string[] = []): string[] {
  const windows = ['del', 'erase', 'rd', 'rmdir', 'copy', 'move', 'xcopy', 'type', 'dir', 'findstr', 'md', 'mkdir', 'tree', 'more', 'ren', 'rename', 'start', 'where', 'cd', 'chdir', 'pushd'].includes(verb)
  const found: string[] = []
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i]!
    if (arg === '--') {
      found.push(...args.slice(i + 1))
      break
    }
    if (takesValue.includes(arg) || PS_VALUES.includes(arg.toLowerCase())) {
      i += 1
      continue
    }
    if (arg.startsWith('-') && arg.length > 1) continue
    if (windows && /^\/[a-z?][a-z:]{0,3}$/i.test(arg)) continue
    found.push(arg)
  }
  return found
}

function valueOf(args: readonly string[], names: readonly string[]): string | undefined {
  return valuesOf(args, names)[0]
}

function valuesOf(args: readonly string[], names: readonly string[]): string[] {
  const found: string[] = []
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i]!
    const lower = arg.toLowerCase()
    const name = names.find((one) => one === arg || one.toLowerCase() === lower && one.startsWith('-') && !one.startsWith('--') && one.length > 2)
    if (name !== undefined && args[i + 1] !== undefined) {
      found.push(args[i + 1]!)
      i += 1
      continue
    }
    const long = names.find((one) => one.startsWith('--') && arg.startsWith(`${one}=`))
    if (long !== undefined) found.push(arg.slice(long.length + 1))
  }
  return found
}

function commaList(value: string): string[] {
  return value.split(',').map((one) => one.trim()).filter((one) => one.length > 0)
}

function named(list: readonly string[], say: (list: readonly string[]) => string): Step | undefined {
  return list.length === 0 ? undefined : { said: say(list) }
}

function looks(step: Step | undefined): Step | undefined {
  return step === undefined ? undefined : { ...step, looks: true }
}

function looksLikeAFile(word: string): boolean {
  return word === '.' || /\.[a-z0-9]{1,6}$/i.test(word) && !/^\.\.?$/.test(word)
}

/** `.` and `./` as words. */
function place(path: string | undefined): string {
  return path === undefined || path === '.' || path === './' || path === '.\\' ? 'this folder' : path
}

function everything(files: readonly string[]): string {
  return files.length === 1 && files[0] === '.' ? 'every file in this folder' : join(files)
}

function join(list: readonly string[]): string {
  const shown = list.map((one) => cut(one, 80))
  if (shown.length <= 1) return shown[0] ?? ''
  if (shown.length <= 3) return `${shown.slice(0, -1).join(', ')} and ${shown[shown.length - 1]!}`
  return `${shown.slice(0, 2).join(', ')} and ${String(shown.length - 2)} more`
}

function plural(list: readonly string[]): string {
  return list.length === 1 ? '' : 's'
}

function cut(text: string, most: number): string {
  return text.length > most ? `${text.slice(0, most - 1).trimEnd()}…` : text
}

function lowerFirst(text: string): string {
  return text.length === 0 ? text : `${text[0]!.toLowerCase()}${text.slice(1)}`
}

function hostOf(url: string): string {
  try {
    return new URL(url).host
  } catch {
    return cut(url, 60)
  }
}

function shortUrl(url: string): string {
  try {
    const parsed = new URL(url)
    return cut(`${parsed.host}${parsed.pathname === '/' ? '' : parsed.pathname}`, 70)
  } catch {
    return cut(url, 70)
  }
}

function repositoryOf(from: string): string {
  const github = /github\.com[/:]([^/\s]+\/[^/\s]+?)(?:\.git)?\/?$/i.exec(from)
  if (github !== null) return `${github[1]!} from GitHub`
  return /^https?:\/\//i.test(from) ? `at ${shortUrl(from)}` : from
}
