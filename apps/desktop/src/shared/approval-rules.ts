/**
 * SAVED APPROVAL RULES (0.521), the evaluator. Product ideas (Bloks note,
 * item 1): an approval card's "Always" becomes a rule a person can see as a
 * sentence and remove, scoped to a teammate and a folder. Copied from Claude
 * Code's own permission rules where they fit: `Bash(npm run test:*)` is a
 * command prefix, `Edit(src/**)` a path pattern, and deny always wins.
 *
 * This file only DECIDES; nothing here runs, writes or answers a card. It is
 * a security surface, so its rules are narrow on purpose:
 *
 *   - Deny is checked first and wins over any allow.
 *   - An allow on a command covers ONE simple command: never one carrying
 *     `;` `&&` `||` `|` `&` `$(` a backtick, a redirect or a newline. Such a
 *     command still asks, whatever the rules say. A deny, though, matches
 *     any simple part of a compound command -- `ls; rm -rf x` is denied by
 *     a rule denying `rm`.
 *   - A path rule matches only paths inside the folder the rule is scoped
 *     to, relative to it; `..`, another drive or an absolute path elsewhere
 *     is never allowed by a rule.
 *   - A question to the person is never governed: it always asks.
 *   - No rule, or anything this cannot read: ask.
 */

import { commandReach } from './command-reach.js'
import { secretIn } from './secrets.js'
import type { CommandReach } from './command-reach.js'

export type RuleEffect = 'allow' | 'deny'
/** What a rule is about: a shell command, a file edited, a file read, a connector's tool. */
export type RuleKind = 'command' | 'edit' | 'read' | 'connector'

export interface ApprovalRule {
  readonly ruleId: string
  readonly effect: RuleEffect
  readonly kind: RuleKind
  /**
   * command: the exact command, or a prefix ending in `:*` ("npm run test:*").
   * edit/read: a path pattern relative to the folder; `*` is any name, `**`
   * any depth ("src/**", "docs/*.md").
   * connector: "server" for any of its tools, or "server/tool".
   */
  readonly pattern: string
  /** Only for this teammate; absent, any teammate. */
  readonly teammateId?: string
  /** Only in this folder (an absolute path); absent, any folder. */
  readonly folder?: string
  readonly createdAt: string
}

/** One thing a runtime asked to do, as the evaluator reads it. */
export type RuledAction =
  | { readonly kind: 'command'; readonly command: string }
  | { readonly kind: 'edit' | 'read'; readonly paths: readonly string[] }
  | { readonly kind: 'connector'; readonly server: string; readonly tool: string }
  /** A question to the person, or anything else: never governed. */
  | { readonly kind: 'other' }

export interface RuleContext {
  readonly teammateId?: string
  /** The folder the run works in, absolute. */
  readonly folder?: string
}

export type RuleVerdict =
  | { readonly decision: 'allow' | 'deny'; readonly rule: ApprovalRule }
  | { readonly decision: 'ask'; readonly why?: string }

/** Characters that make a command more than one simple command. */
const COMPOUND = /;|&&|\|\||\||&|\$\(|`|>|<|\r|\n/

export const COMPOUND_REFUSAL = 'It is more than one simple command, so no saved rule allows it.'

const flat = (command: string): string => command.trim().replace(/\s+/g, ' ')

/** A command's simple parts, for deny: split on the operators, each part trimmed. */
function simpleParts(command: string): readonly string[] {
  return command
    .split(/;|&&|\|\||\||&|\r|\n|\$\(|`|\)/)
    .map((part) => flat(part.replace(/[<>].*$/, '')))
    .filter((part) => part.length > 0)
}

function commandMatches(pattern: string, command: string): boolean {
  const want = flat(pattern)
  const have = flat(command)
  if (want.length === 0 || have.length === 0) return false
  if (want.endsWith(':*')) {
    const prefix = want.slice(0, -2).trimEnd()
    // A prefix is whole words: "npm run test:*" covers "npm run test -- x", never "npm run tests".
    return prefix.length > 0 && (have === prefix || have.startsWith(`${prefix} `))
  }
  return have === want
}

/** Normalised, forward slashes, no trailing slash, case kept. */
const slashes = (path: string): string => path.replace(/\\/g, '/').replace(/\/+$/, '')

/**
 * A path relative to the folder, or undefined when it is not inside it.
 * Windows paths compare without case, as the file system does.
 */
export function insideFolder(path: string, folder: string): string | undefined {
  const base = slashes(folder)
  const raw = slashes(path)
  const absolute = /^[A-Za-z]:\//.test(raw) || raw.startsWith('/')
  const joined = absolute ? raw : `${base}/${raw}`
  // Resolve . and .. by hand; a path that climbs above the folder is not inside it.
  const parts: string[] = []
  for (const part of joined.split('/')) {
    if (part === '' && parts.length > 0) continue
    if (part === '.') continue
    if (part === '..') {
      if (parts.length <= 1) return undefined
      parts.pop()
      continue
    }
    parts.push(part)
  }
  const resolved = parts.join('/')
  const windows = /^[A-Za-z]:/.test(base)
  const same = (left: string, right: string): boolean => (windows ? left.toLowerCase() === right.toLowerCase() : left === right)
  if (!same(resolved.slice(0, base.length), base)) return undefined
  const rest = resolved.slice(base.length)
  if (rest.length === 0) return ''
  if (!rest.startsWith('/')) return undefined
  return rest.slice(1)
}

function globToRegExp(pattern: string, caseBlind = true): RegExp {
  let out = ''
  const text = slashes(pattern).replace(/^\.\//, '')
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index] as string
    if (character === '*') {
      if (text[index + 1] === '*') {
        // "**/" is any depth, including none; a bare "**" is anything.
        if (text[index + 2] === '/') {
          out += '(?:.*/)?'
          index += 2
        } else {
          out += '.*'
          index += 1
        }
      } else {
        out += '[^/]*'
      }
    } else if (character === '?') {
      out += '[^/]'
    } else {
      out += character.replace(/[.+^${}()|[\]\\]/g, '\\$&')
    }
  }
  return new RegExp(`^${out}$`, caseBlind ? 'i' : '')
}

function pathMatches(pattern: string, relative: string, caseBlind = true): boolean {
  return globToRegExp(pattern, caseBlind).test(relative)
}

function scoped(rule: ApprovalRule, context: RuleContext): boolean {
  if (rule.teammateId !== undefined && rule.teammateId !== context.teammateId) return false
  if (rule.folder !== undefined) {
    if (context.folder === undefined) return false
    if (insideFolder(context.folder, rule.folder) !== '') return false
  }
  return true
}

/** Whether a rule speaks to this action, deny reading compound commands part by part. */
function matches(rule: ApprovalRule, action: RuledAction, context: RuleContext): boolean {
  if (rule.kind !== action.kind) return false
  if (action.kind === 'command') {
    if (rule.effect === 'deny') return [flat(action.command), ...simpleParts(action.command)].some((part) => commandMatches(rule.pattern, part))
    return !COMPOUND.test(action.command) && commandMatches(rule.pattern, action.command)
  }
  if (action.kind === 'edit' || action.kind === 'read') {
    const folder = rule.folder ?? context.folder
    if (folder === undefined || action.paths.length === 0) return false
    const relative = action.paths.map((path) => insideFolder(path, folder))
    // Deny: any path it names. Allow: every path inside the folder, and every one matching.
    if (rule.effect === 'deny') return relative.some((path) => path !== undefined && pathMatches(rule.pattern, path)) || action.paths.some((path) => pathMatches(rule.pattern, slashes(path)))
    // An ALLOW matches case-blind only where the folder is (Windows): off it SRC is not src (2026-10-10 sweep).
    // A deny stays case-blind: the safe direction.
    const caseBlind = /^[A-Za-z]:/.test(folder)
    return relative.every((path) => path !== undefined && path.length > 0 && pathMatches(rule.pattern, path, caseBlind))
  }
  if (action.kind === 'connector') {
    const [server, tool] = rule.pattern.split('/')
    return server === action.server && (tool === undefined || tool === action.tool)
  }
  return false
}

export function decideByRules(action: RuledAction, rules: readonly ApprovalRule[], context: RuleContext): RuleVerdict {
  if (action.kind === 'other') return { decision: 'ask' }
  const applicable = rules.filter((rule) => scoped(rule, context))
  const denied = applicable.find((rule) => rule.effect === 'deny' && matches(rule, action, context))
  if (denied !== undefined) return { decision: 'deny', rule: denied }
  const allowed = applicable.find((rule) => rule.effect === 'allow' && matches(rule, action, context))
  if (allowed !== undefined) return { decision: 'allow', rule: allowed }
  if (action.kind === 'command' && COMPOUND.test(action.command) && applicable.some((rule) => rule.effect === 'allow' && rule.kind === 'command' && commandMatches(rule.pattern, simpleParts(action.command)[0] ?? ''))) {
    return { decision: 'ask', why: COMPOUND_REFUSAL }
  }
  return { decision: 'ask' }
}

/** A rule as a person reads it, on the card and in Settings. */
export function ruleSentence(rule: ApprovalRule, teammateName?: string): string {
  const who = rule.teammateId === undefined ? 'Any teammate' : teammateName ?? 'This teammate'
  const verb = rule.effect === 'allow' ? 'may' : 'may not'
  const what =
    rule.kind === 'command'
      ? rule.pattern.endsWith(':*')
        ? `run commands starting "${rule.pattern.slice(0, -2).trim()}"`
        : `run "${rule.pattern}"`
      : rule.kind === 'edit'
        ? `change files matching ${rule.pattern}`
        : rule.kind === 'read'
          ? `read files matching ${rule.pattern}`
          : rule.pattern.includes('/')
            ? `use ${rule.pattern.split('/')[1] ?? ''} on ${rule.pattern.split('/')[0] ?? ''}`
            : `use the ${rule.pattern} connector`
  const where = rule.folder === undefined ? '' : ` in ${slashes(rule.folder).split('/').pop() ?? rule.folder}`
  // An allow is about not being asked; a deny is a plain no.
  return `${who} ${verb} ${what}${where}${rule.effect === 'allow' ? ' without asking' : ''}.`
}

/**
 * What a card's request is, for the rules (0.521) -- or 'other' when it
 * cannot be read exactly, which always asks. Each runtime words a request
 * its own way (main/approval-channel.ts, permission-host.ts):
 *
 *   - a command is "Run a command" with the command as its detail; any other
 *     "Use <tool>" a runtime asks about is not a shell command and is not
 *     governed;
 *   - a file change names its files in the diff it carries, else Claude Code
 *     names one file and Copilot a comma-separated list; Codex's summary
 *     sentence names none, so without a diff it is not governed;
 *   - a connector is "Use <tool> on <server>".
 */
/**
 * The command inside a runtime's shell wrapper (0.521). Codex on Windows asks
 * about `"C:\...\powershell.exe" -Command 'git status --short'`, and a rule
 * saved from that read as the wrapper, matched only that exact path, and said
 * so in its sentence. The wrapper is taken off only when the WHOLE text is
 * exactly one wrapper around one single-quoted command -- PowerShell's `''`
 * read as one quote -- so what a rule matches is still everything that runs.
 * Anything else is left as it is.
 */
export function unwrappedCommand(command: string): string {
  const text = command.trim()
  const powershell = /^(?:"[^"]*(?:powershell|pwsh)(?:\.exe)?"|\S*(?:powershell|pwsh)(?:\.exe)?)\s+(?:-(?:NoProfile|NoLogo|NonInteractive)\s+)*-Command\s+'((?:[^']|'')*)'$/i.exec(text)
  if (powershell !== null) return (powershell[1] as string).replace(/''/g, "'")
  const posix = /^(?:\/(?:usr\/)?bin\/)?(?:bash|sh|zsh)\s+-l?c\s+'([^']*)'$/.exec(text)
  if (posix !== null) return posix[1] as string
  return text
}

export function ruledActionOf(request: {
  readonly kind: string
  readonly summary: string
  readonly detail: string
  readonly runtime?: string
  readonly patch?: { readonly text: string }
}): RuledAction {
  if (request.kind === 'command') {
    return /^Run a command$/.test(request.summary) && request.detail.trim().length > 0 ? { kind: 'command', command: unwrappedCommand(request.detail) } : { kind: 'other' }
  }
  if (request.kind === 'file-change') {
    const fromDiff = request.patch === undefined ? [] : diffPaths(request.patch.text)
    if (fromDiff.length > 0) return { kind: 'edit', paths: fromDiff }
    const detail = request.detail.trim()
    if (detail.length === 0) return { kind: 'other' }
    if (request.runtime === 'claude' && /^Change 1 file/.test(request.summary)) return { kind: 'edit', paths: [detail] }
    if (request.runtime === 'copilot') return { kind: 'edit', paths: detail.split(', ').filter((path) => path.length > 0) }
    return { kind: 'other' }
  }
  if (request.kind === 'connector') {
    const named = /^Use (\S+) on (.+)$/.exec(request.summary)
    return named === null ? { kind: 'other' } : { kind: 'connector', tool: named[1] as string, server: named[2] as string }
  }
  return { kind: 'other' }
}

/** The files a unified diff names, from its headers, /dev/null left out. */
export function diffPaths(diff: string): readonly string[] {
  const paths = new Set<string>()
  for (const line of diff.split(/\r?\n/)) {
    const header = /^(?:\+\+\+|---) (?:[ab]\/)?(.+?)\s*$/.exec(line)
    if (header !== null && header[1] !== '/dev/null') paths.add(header[1] as string)
  }
  return [...paths]
}

/**
 * The rule "Yes, and don't ask again" would save for this request (0.521),
 * or undefined when it would not be exact: a compound command, a file not
 * inside the folder, anything 'other'. Narrow on purpose -- the exact
 * command, the exact files, the one connector tool -- as Claude Code
 * proposes the command it was shown.
 */
export function ruleCandidateOf(
  action: RuledAction,
  context: RuleContext
): Omit<ApprovalRule, 'ruleId' | 'createdAt'> | undefined {
  const scope = { ...(context.teammateId === undefined ? {} : { teammateId: context.teammateId }), ...(context.folder === undefined ? {} : { folder: context.folder }) }
  if (action.kind === 'command') {
    if (COMPOUND.test(action.command) || flat(action.command).length === 0 || flat(action.command).length > 400) return undefined
    // No rule is offered for a command carrying a key (0.870): the rule would keep the command, key and all, for good.
    // Claude Code's "Don't ask again" was found saving such rules into public repositories (HN, 2026-11-08).
    if (secretIn(action.command) !== undefined) return undefined
    return { effect: 'allow', kind: 'command', pattern: flat(action.command), ...scope }
  }
  if (action.kind === 'edit' || action.kind === 'read') {
    if (context.folder === undefined || action.paths.length !== 1) return undefined
    const relative = insideFolder(action.paths[0] as string, context.folder)
    if (relative === undefined || relative.length === 0 || /[*?]/.test(relative)) return undefined
    return { effect: 'allow', kind: action.kind, pattern: relative, ...scope }
  }
  if (action.kind === 'connector') return { effect: 'allow', kind: 'connector', pattern: `${action.server}/${action.tool}`, ...scope }
  return undefined
}

/**
 * THE HOST'S OWN GUARD ON "ALWAYS" (0.598). Since 0.579 the card hides
 * "Always allow this session" and the rule offer when the command reaches
 * other programs (`Stop-Process -Name node`, `taskkill /IM python.exe`,
 * `shutdown`). The card was the only guard: the main process took any
 * `approve-always` the window sent and remembered it for every later command
 * of the run, and saved a rule for the same command without looking. One
 * stale window, one devtools call, one renderer regression was the whole
 * distance. Now the host decides the same thing from the same classifier.
 */
export function reachOfRequest(request: Parameters<typeof ruledActionOf>[0]): CommandReach | undefined {
  const action = ruledActionOf(request)
  return action.kind === 'command' ? commandReach(action.command) : undefined
}

/**
 * The answer the host applies. An "Always" for a command that reaches other
 * programs becomes "this once", with a note the record keeps; every other
 * answer -- an ordinary command's Always, a denial, a question's answers --
 * is applied as it came.
 */
export function enforcedAnswer<T extends { readonly approvalId: string; readonly decision?: string }>(
  request: Parameters<typeof ruledActionOf>[0],
  answer: T
): { readonly answer: T; readonly note?: string } {
  if (answer.decision !== 'approve-always') return { answer }
  const reach = reachOfRequest(request)
  if (reach === undefined) return { answer }
  return { answer: { ...answer, decision: 'approve-once' } as T, note: `asked each time: ${reach.short}` }
}

/** Why a rule is not saved for this request, in the card's own words; undefined when it may be. */
export function ruleRefusalFor(request: Parameters<typeof ruledActionOf>[0]): string | undefined {
  const reach = reachOfRequest(request)
  return reach === undefined ? undefined : `Not saved as a rule: this command ${reach.short}. ${reach.said} Locust asks each time.`
}

/**
 * WHAT WOULD HAPPEN IF (0.874; the 13 Nov rival scan: CopilotKit's what-if page, where a refusal names its rule).
 * A command a person types in Settings, tried against the saved rules, once for every place they speak to: any
 * teammate anywhere, and each teammate and folder a rule is scoped to. It runs nothing; it is `decideByRules` asked
 * the same question in each place, so what it says is what a card would do.
 */
export interface WhatIfLine {
  readonly teammateId?: string
  readonly folder?: string
  readonly verdict: RuleVerdict
}

export function whatIf(command: string, rules: readonly ApprovalRule[]): readonly WhatIfLine[] {
  const text = command.trim()
  if (text.length === 0) return []
  const places = new Map<string, RuleContext>([['', {}]])
  for (const rule of rules) {
    if (rule.kind !== 'command') continue
    // A rule for any teammate anywhere speaks to the first place, already there.
    if (rule.teammateId === undefined && rule.folder === undefined) continue
    places.set(`${rule.teammateId ?? ''}\u0000${rule.folder ?? ''}`, {
      ...(rule.teammateId === undefined ? {} : { teammateId: rule.teammateId }),
      ...(rule.folder === undefined ? {} : { folder: rule.folder })
    })
  }
  return [...places.values()].map((place) => ({
    ...(place.teammateId === undefined ? {} : { teammateId: place.teammateId }),
    ...(place.folder === undefined ? {} : { folder: place.folder }),
    verdict: decideByRules({ kind: 'command', command: text }, rules, place)
  }))
}

/** One place's answer, as a person reads it: who and where, what would happen, and the rule that decides. */
export function whatIfSentence(line: WhatIfLine, teammateName?: string): string {
  const who = line.teammateId === undefined ? 'Any teammate' : teammateName ?? 'This teammate'
  const where = line.folder === undefined ? (line.teammateId === undefined ? ', anywhere' : '') : ` in ${slashes(line.folder).split('/').pop() ?? line.folder}`
  const verdict = line.verdict
  if (verdict.decision !== 'ask') {
    const what = verdict.decision === 'allow' ? 'it runs without asking' : 'it is refused'
    return `${who}${where}: ${what}, because of the rule “${ruleSentence(verdict.rule, teammateName)}”`
  }
  return `${who}${where}: it asks you first. ${verdict.why ?? 'No saved rule speaks to it.'}`
}
