import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { joinMessageFragments } from './messageFragments.js'
import { blocksOutsideCode } from './protocolTags.js'

/**
 * KEYS A TEAMMATE NEEDS (0.833, the Rakazo scrub's third take). A teammate that is to charge a test card, post to a
 * channel or read a private API needs a key, and until now the only way to give it one was to paste it into the chat --
 * where it sat in the conversation, the ledger, every brief that quoted it, and a chat app's servers. So a teammate asks
 * for a key by name, in a `locust-key` block; Locust draws a card under that answer with a masked box; and the key is
 * kept as the operating system encrypts it, for that teammate alone. Its runs get it as an environment variable of
 * that name and nothing else does -- not another teammate, not a cloud turn, not a sealed one. The brief names the
 * variable, never its value, and anything a run says or does that holds the value has it taken out before Locust
 * records or shows it.
 */
export const KEY_TAG = 'locust-key'

export const TEAMMATE_KEYS_LIST_CHANNEL = 'teammate-keys:list'
export const TEAMMATE_KEYS_SET_CHANNEL = 'teammate-keys:set'
export const TEAMMATE_KEYS_REMOVE_CHANNEL = 'teammate-keys:remove'

export type TeammateKeysResult = { readonly ok: true; readonly names: readonly string[] } | { readonly ok: false; readonly message: string; readonly names: readonly string[] }

export interface TeammateKeysApi {
  /** The names of a teammate's keys. Their values never come back to the window. */
  list(teammateId: string): Promise<TeammateKeysResult>
  set(teammateId: string, name: string, value: string): Promise<TeammateKeysResult>
  remove(teammateId: string, name: string): Promise<TeammateKeysResult>
}

/** A key's name, as a variable is named: capitals, digits and underscores, starting with a letter. */
export const KEY_NAME = /^[A-Z][A-Z0-9_]{1,63}$/
export const MAX_KEY_LENGTH = 4_096
export const MAX_KEYS_PER_TEAMMATE = 20

/**
 * Names an agent reads for its own sign-in or start-up, and Locust's own. Given one, a teammate's agent would sign in
 * with the key instead of the person's plan -- paying twice -- or start differently. So no key may take one.
 */
const RESERVED = /^(?:ANTHROPIC_|CLAUDE_|OPENAI_|CODEX_|OPENCODE_|GEMINI_|GOOGLE_|CURSOR_|COPILOT_|GH_|AZURE_OPENAI_|LOCUST_|ELECTRON_|NODE_|NPM_)/
const RESERVED_NAMES: ReadonlySet<string> = new Set([
  'GITHUB_TOKEN', 'APPDATA', 'COMSPEC', 'HOME', 'HOMEDRIVE', 'HOMEPATH', 'HTTP_PROXY', 'HTTPS_PROXY', 'NO_PROXY', 'LANG', 'LC_ALL',
  'LOCALAPPDATA', 'LOGNAME', 'NO_COLOR', 'NODEFAULTCURRENTDIRECTORYINEXEPATH', 'PATH', 'PATHEXT', 'SHELL', 'SSL_CERT_DIR',
  'SSL_CERT_FILE', 'SYSTEMROOT', 'TEMP', 'TMP', 'TMPDIR', 'USER', 'USERNAME', 'USERPROFILE', 'WINDIR', 'XDG_CACHE_HOME',
  'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'PWD', 'OLDPWD', 'TERM', 'PROMPT', 'PS1'
])

/** Why a name will not do, in plain words; undefined when it will. */
export function keyNameRefusal(name: string): string | undefined {
  if (!KEY_NAME.test(name)) return 'A key’s name is in capitals, digits and underscores, starting with a letter, like STRIPE_KEY.'
  if (RESERVED.test(name) || RESERVED_NAMES.has(name)) return `${name} is a name an agent or this computer reads for itself, so a key can’t take it. Pick another, like ${name.replace(/^[A-Z]+_/, 'MY_')}.`
  return undefined
}

/** Why a value will not do; undefined when it will. */
export function keyValueRefusal(value: string): string | undefined {
  if (value.length === 0) return 'Paste the key first.'
  if (value.length > MAX_KEY_LENGTH) return `That is longer than any key Locust keeps (${String(MAX_KEY_LENGTH)} characters).`
  if (/[\r\n\0]/.test(value)) return 'A key is one line. Paste only the key itself.'
  return undefined
}

/**
 * The agents a key reaches: each MEASURED with a real run, a command of the teammate's printing the key's length and
 * the value kept out of the record (_tools/drive-a-teammates-key-reaches-its-runs.mjs, 2026-10-24: Claude Code on Haiku
 * and Cursor on the person's own model, both PRESENT at full length). An agent joins once its own run passes. Not yet:
 * OpenCode (its free models were out of usage that night), Gemini and Copilot (not ready on that computer). Codex
 * drops every variable named like a key from the commands it runs, and the ways past that would hand its commands the
 * person's own secrets too; Antigravity runs its commands in its own app, not in a process Locust starts.
 */
export const KEYS_REACH: ReadonlySet<string> = new Set(['claude', 'cursor'])
/** The agents a key reaches, as a person reads them. */
export const KEYS_REACH_SAID = 'Claude Code and Cursor'

export interface KeyRequest { readonly name: string; readonly why: string }

const REQUEST = new RegExp(`<${KEY_TAG}\\s+name\\s*=\\s*"([^"]{1,80})"\\s*>([\\s\\S]*?)</${KEY_TAG}>`, 'g')

/** The keys a message asks for, outside code, each once and at most three. */
export function keyRequestsIn(text: string): readonly KeyRequest[] {
  const found: KeyRequest[] = []
  for (const block of blocksOutsideCode(text, REQUEST)) {
    const name = block[1]!.trim()
    if (found.some((one) => one.name === name)) continue
    found.push({ name, why: block[2]!.replace(/\s+/g, ' ').trim().slice(0, 300) })
    if (found.length === 3) break
  }
  return found
}

/** The keys a finished turn asked for, read from its last whole message. */
export function keyRequestsInEvents(events: readonly NormalizedRuntimeEvent[]): readonly KeyRequest[] {
  const final = joinMessageFragments(events).findLast((event) => event.type === 'message.delta' && event.payload.final)
  return final?.type === 'message.delta' ? keyRequestsIn(final.payload.text) : []
}

/** What a teammate is told of its keys: their names, how to use them, and how to ask for another. */
export function keysSection(names: readonly string[], runtimeTakesThem: boolean): string {
  if (!runtimeTakesThem) {
    return names.length === 0 ? '' : `Keys: the person gave you ${names.join(', ')}, but Locust does not give keys to this agent yet, so you cannot use them here. Say so if a task needs one.`
  }
  const lines = ['Keys:']
  if (names.length > 0) {
    lines.push(`You have these keys as environment variables: ${names.join(', ')}. Use them from commands and code by name (for example $${names[0]!} or process.env.${names[0]!}). Never print, echo, log or write a key's value anywhere: Locust takes the value out of anything it records, and the key is the person's.`)
  }
  lines.push(`If a task needs a key you do not have, do not ask for it in the chat. Write <${KEY_TAG} name="NAME">what it is for</${KEY_TAG}> on a line of its own, with the variable's name in capitals, and stop: the person is shown a box to put it in, and you have it from your next turn.`)
  return lines.join('\n')
}

/** A teammate's key as a run is given it. */
export interface RunKey { readonly name: string; readonly value: string }

const SHORTEST_TAKEN = 4
const marker = (name: string): string => `[key ${name}]`

/** Text with every key's exact value replaced by its name in brackets. Longest first, so no value is left in part. */
export function withoutKeyValues(text: string, keys: readonly RunKey[]): string {
  let clean = text
  for (const key of [...keys].sort((a, b) => b.value.length - a.value.length)) {
    if (key.value.length < SHORTEST_TAKEN) continue
    if (clean.includes(key.value)) clean = clean.split(key.value).join(marker(key.name))
  }
  return clean
}

/** Any value -- an event, its payload, its evidence -- with each key's value taken out of every string in it. */
export function scrubKeyValues<T>(value: T, keys: readonly RunKey[]): T {
  if (keys.length === 0) return value
  const walk = (item: unknown): unknown => {
    if (typeof item === 'string') return withoutKeyValues(item, keys)
    if (Array.isArray(item)) return item.map(walk)
    if (item !== null && typeof item === 'object') {
      const out: Record<string, unknown> = {}
      for (const [key, inner] of Object.entries(item as Record<string, unknown>)) out[key] = walk(inner)
      return out
    }
    return item
  }
  return walk(value) as T
}
