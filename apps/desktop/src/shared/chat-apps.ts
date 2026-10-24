/**
 * YOUR TEAMMATES IN CHAT APPS (0.831 Telegram; 0.832 Discord and Slack, the Rakazo scrub's first take).
 *
 * A person writes to a teammate from the chat app they already have open, the answer comes back there, and a card a
 * turn from there raises comes with Allow once and Deny -- never Always, which stays in the window, as on the phone.
 * Each app is the person's own: their bot (Telegram, Discord) or their app (Slack), made in a few steps Settings walks
 * them through, with its token kept as the operating system encrypts it. One chat is paired, by a link or a code, and
 * every other chat is ignored. Nothing listens on this computer: Locust keeps one connection out to the app while it
 * is open (main/chat-link.ts is what the three share; telegram.ts, discord.ts and slack.ts are each app's own).
 */
export type ChatAppId = 'discord' | 'slack' | 'telegram'
/** The order Settings shows them in: the ones most people already have, first. */
export const CHAT_APPS: readonly ChatAppId[] = ['discord', 'slack', 'telegram']
export const CHAT_APP_NAMES: Readonly<Record<ChatAppId, string>> = { discord: 'Discord', slack: 'Slack', telegram: 'Telegram' }

export interface ChatAppSettings {
  /** A token is kept, and the app named its bot. Discord's says where to add it; Slack's names its workspace. */
  readonly bot?: {
    readonly username: string
    readonly name: string
    /** Discord: the link that adds the bot to a server of the person's. */
    readonly invite?: string
    /** Slack: the workspace the app is in. */
    readonly team?: string
    /** Where the bot's own chat opens in the app, once Locust knows. */
    readonly open?: string
  }
  /** The one chat it answers: the person's own, paired by the link or the code. */
  readonly chat?: { readonly name: string; readonly username?: string; readonly pairedAt: string }
  /** Telegram's pairing link, while it can be used: once, until `until`. */
  readonly link?: { readonly url: string; readonly until: string }
  /** The code that pairs a chat when the person sends it to the bot (Discord, Slack), while it can be used. */
  readonly code?: { readonly code: string; readonly until: string }
  /** Whether Locust is reading the bot's messages now. */
  readonly connected: boolean
  /** Why the last change did not happen, or why it is not connected, in plain words. */
  readonly message?: string
}

/** Slack takes two tokens: its bot's, which writes, and the app's own, which opens the connection Locust keeps. */
export interface SlackTokens { readonly bot: string; readonly app: string }

export interface ChatAppApi {
  settings(): Promise<ChatAppSettings>
  /** What was pasted is checked with the app before it is kept, encrypted. */
  setToken(token: string | SlackTokens): Promise<ChatAppSettings>
  /** A new pairing link or code: the one before it stops working. */
  newLink(): Promise<ChatAppSettings>
  /** The token and the chat are forgotten, and Locust stops reading the app. */
  remove(): Promise<ChatAppSettings>
  onChanged(callback: (settings: ChatAppSettings) => void): () => void
}

/** A message older than this, when Locust reads it, came while it was closed: it starts nothing. */
export const CHAT_STALE_MS = 10 * 60_000
/** A pairing link or code works once, for this long. */
export const CHAT_LINK_MS = 10 * 60_000
/** An answer longer than this many messages ends with where the rest is. */
export const CHAT_MAX_PARTS = 4
export const CHAT_REST_IN_LOCUST = 'The rest is in Locust on your computer.'

/** Locust's own commands, which no teammate's name may take. */
export const CHAT_OWN_COMMANDS = ['start', 'help', 'who', 'new', 'waiting'] as const
const RESERVED: ReadonlySet<string> = new Set([...CHAT_OWN_COMMANDS, 'settings', 'stop', 'cancel', 'pair'])

/**
 * Each teammate's command, by id, in the team's order: the name as Telegram and Discord allow a command -- lowercase
 * letters, digits and underscores, 32 at most -- with a number after one that is taken or is one of Locust's own.
 */
export function teammateCommands(team: readonly { readonly teammateId: string; readonly name: string }[]): ReadonlyMap<string, string> {
  const commands = new Map<string, string>()
  const taken = new Set<string>()
  for (const mate of team) {
    const base = mate.name.normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '_').slice(0, 28).replace(/^_+|_+$/g, '') || 'teammate'
    let command = base
    for (let next = 2; RESERVED.has(command) || taken.has(command); next += 1) command = `${base}${String(next)}`
    taken.add(command)
    commands.set(mate.teammateId, command)
  }
  return commands
}

const folded = (char: string): string => char.normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLowerCase()
/** How much of `text` spells `name` from its start, accents and case aside; undefined when it does not. */
function spelled(text: string, name: string): number | undefined {
  const wanted = [...name].map(folded).join('')
  if (wanted.length === 0) return undefined
  const chars = [...text]
  let built = ''
  let at = 0
  while (built.length < wanted.length && at < chars.length) { built += folded(chars[at]!); at += 1 }
  return built === wanted ? chars.slice(0, at).join('').length : undefined
}

/**
 * A teammate named at the start of a message, the way a person writes to someone: "Ada: check the build", "Ada,
 * check the build" or "@ada check the build" -- by the teammate's name or its command, in any case. The longest name
 * wins, so "Ada Lovelace:" is not Ada's. With nothing after the name, `rest` is empty: who the next messages go to.
 */
export function addressedTeammate(text: string, team: readonly { readonly teammateId: string; readonly name: string }[]): { readonly teammateId: string; readonly rest: string } | undefined {
  const trimmed = text.trimStart()
  const commands = teammateCommands(team)
  let best: { readonly teammateId: string; readonly rest: string; readonly length: number } | undefined
  for (const mate of team) {
    for (const said of new Set([mate.name.trim(), commands.get(mate.teammateId)!])) {
      for (const at of [true, false]) {
        const head = at ? (trimmed.startsWith('@') ? 1 : undefined) : 0
        if (head === undefined) continue
        const length = spelled(trimmed.slice(head), said)
        if (length === undefined) continue
        const after = trimmed.slice(head + length)
        // "@ada hello", "@ada: hello"; without the @, a colon or a comma must follow the name ("Ada: hello").
        const separated = at ? /^(?:\s*[:,]|\s|$)/.exec(after) : /^\s*[:,]/.exec(after)
        if (separated === null) continue
        const total = head + length
        if (best !== undefined && best.length >= total) continue
        best = { teammateId: mate.teammateId, rest: after.slice(separated[0].length).trim(), length: total }
      }
    }
  }
  return best === undefined ? undefined : { teammateId: best.teammateId, rest: best.rest }
}

/** A typed pairing code: eight digits, however they were spaced. */
export const PAIRING_CODE = /^\d{8}$/
/** The code a message is, once spaces and dashes are out of it; undefined when it is something else. */
export function pairingCodeIn(text: string): string | undefined {
  const digits = text.trim().replace(/[\s-]+/g, '')
  return PAIRING_CODE.test(digits) ? digits : undefined
}
/** A code as a person reads it out: two groups of four. */
export const spacedCode = (code: string): string => (code.length === 8 ? `${code.slice(0, 4)} ${code.slice(4)}` : code)
