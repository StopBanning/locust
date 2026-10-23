/**
 * YOUR TEAMMATES IN TELEGRAM (0.831, the Rakazo scrub's first take; Slack and Discord follow).
 *
 * Every always-on rival lets a person write to their bots from a chat app. Locust's phone page (0.775, 0.826) shows
 * the conversations and the cards, but a chat app is where a person already is. So a person makes a bot of their own
 * with Telegram's @BotFather, pastes its token into Settings, and pairs their own chat with it by a link: `/ada` and a
 * message reaches Ada, the answer comes back in the chat, and a card a turn from there raises comes with Allow once and
 * Deny buttons -- never Always, which stays in the window, as on the phone.
 *
 * Nothing listens on this computer: Locust asks Telegram for the bot's new messages, one long request at a time, and
 * only while it is open. The token is kept as the operating system encrypts it. Telegram's servers can read what is
 * written there -- a bot is not end-to-end encrypted -- and Settings says so in those words.
 */
export const TELEGRAM_SETTINGS_CHANNEL = 'telegram:settings'
export const TELEGRAM_SET_TOKEN_CHANNEL = 'telegram:set-token'
export const TELEGRAM_NEW_LINK_CHANNEL = 'telegram:new-link'
export const TELEGRAM_REMOVE_CHANNEL = 'telegram:remove'
export const TELEGRAM_CHANGED_CHANNEL = 'telegram:changed'

export interface TelegramSettings {
  /** A token is kept, and Telegram named its bot. */
  readonly bot?: { readonly username: string; readonly name: string }
  /** The one chat it answers: the person's own, paired by the link. */
  readonly chat?: { readonly name: string; readonly username?: string; readonly pairedAt: string }
  /** The link that pairs a chat, while it can be used: once, until `until`. */
  readonly link?: { readonly url: string; readonly until: string }
  /** Whether Locust is reading the bot's messages now. */
  readonly connected: boolean
  /** Why the last change did not happen, or why it is not connected, in plain words. */
  readonly message?: string
}

export interface TelegramApi {
  settings(): Promise<TelegramSettings>
  /** The token is checked with Telegram before it is kept, encrypted. */
  setToken(token: string): Promise<TelegramSettings>
  /** A new pairing link: the one before it stops working. */
  newLink(): Promise<TelegramSettings>
  /** The token and the chat are forgotten, and Locust stops reading the bot. */
  remove(): Promise<TelegramSettings>
  onChanged(callback: (settings: TelegramSettings) => void): () => void
}

/** BotFather's tokens: the bot's number, a colon, and 35 characters (a margin either way). */
export const TELEGRAM_TOKEN = /^\d{5,15}:[A-Za-z0-9_-]{30,50}$/

/** What Telegram takes in one message is 4,096 characters; markup is counted here too, so this leaves room. */
export const TELEGRAM_MESSAGE_MAX = 4_000
/** An answer longer than this many messages ends with where the rest is. */
export const TELEGRAM_MAX_PARTS = 4
/** A message older than this, when Locust reads it, came while it was closed: it starts nothing. */
export const TELEGRAM_STALE_MS = 10 * 60_000
/** A pairing link works once, for this long. */
export const TELEGRAM_LINK_MS = 10 * 60_000

/** Locust's own commands, which no teammate's name may take. */
export const TELEGRAM_OWN_COMMANDS = ['start', 'help', 'who', 'new', 'waiting'] as const
const RESERVED: ReadonlySet<string> = new Set([...TELEGRAM_OWN_COMMANDS, 'settings', 'stop', 'cancel'])

/**
 * Each teammate's command, by id, in the team's order: the name as Telegram allows a command -- lowercase letters,
 * digits and underscores, 32 at most -- with a number after one that is taken or is one of Locust's own.
 */
export function teammateCommands(team: readonly { readonly teammateId: string; readonly name: string }[]): ReadonlyMap<string, string> {
  const commands = new Map<string, string>()
  const taken = new Set<string>()
  for (const mate of team) {
    const base = mate.name.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '_').slice(0, 28).replace(/^_+|_+$/g, '') || 'teammate'
    let command = base
    for (let next = 2; RESERVED.has(command) || taken.has(command); next += 1) command = `${base}${String(next)}`
    taken.add(command)
    commands.set(mate.teammateId, command)
  }
  return commands
}

/** A message's command, the way Telegram sends one: `/ada hello`, or `/ada@your_bot hello` in a menu's tap. */
export function parsedCommand(text: string, botUsername: string): { readonly command: string; readonly rest: string } | undefined {
  const match = /^\/([A-Za-z0-9_]{1,32})(?:@([A-Za-z0-9_]{3,64}))?(?:\s+([\s\S]*))?$/.exec(text.trim())
  if (match === null) return undefined
  // A command meant for another bot is not this one's.
  if (match[2] !== undefined && match[2].toLowerCase() !== botUsername.toLowerCase()) return undefined
  return { command: match[1]!.toLowerCase(), rest: (match[3] ?? '').trim() }
}

const escaped = (text: string): string => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
/** Text for Telegram's HTML: only these three are special outside a tag. */
export const telegramEscape = escaped

/** One line's inline Markdown in Telegram's HTML: code first, since nothing inside it is Markdown; then links and emphasis. */
function inline(text: string): string {
  return text.split(/(`[^`\n]+`)/).map((part, index) => {
    if (index % 2 === 1) return `<code>${escaped(part.slice(1, -1))}</code>`
    let html = escaped(part)
    html = html.replace(/\[([^\]\n]+)\]\((https?:\/\/[^\s)<>]+)\)/g, (_all, label: string, url: string) => `<a href="${url.replace(/"/g, '&quot;')}">${label}</a>`)
    html = html.replace(/\*\*(?=\S)([^*\n]*?\S)\*\*/g, '<b>$1</b>')
    html = html.replace(/(^|[\s(])\*(?=[^\s*])([^*\n]*?[^\s*])\*(?=$|[\s).,;:!?])/g, '$1<i>$2</i>')
    html = html.replace(/~~(?=\S)([^~\n]*?\S)~~/g, '<s>$1</s>')
    return html
  }).join('')
}

const preOf = (text: string, language: string | undefined): string =>
  language !== undefined && /^[A-Za-z0-9+#-]{1,20}$/.test(language) ? `<pre><code class="language-${language}">${escaped(text)}</code></pre>` : `<pre>${escaped(text)}</pre>`

/** A block of code too long for one message, as several, split between lines. */
function prePieces(text: string, language: string | undefined, max: number): string[] {
  const pieces: string[] = []
  let lines: string[] = []
  const flush = (): void => { if (lines.length > 0) pieces.push(preOf(lines.join('\n'), language)); lines = [] }
  for (const line of text.split('\n')) {
    // A line longer than a message on its own is cut; Locust on the computer has the whole of it.
    const cut = line.length > max - 60 ? `${line.slice(0, max - 61)}…` : line
    if (escaped([...lines, cut].join('\n')).length + 60 > max) flush()
    lines.push(cut)
  }
  flush()
  return pieces
}

/**
 * MARKDOWN AS TELEGRAM SHOWS IT. A teammate answers in Markdown; Telegram takes a small HTML instead -- bold, italic,
 * struck, code, blocks of code, links and quotes -- and refuses a whole message over one unbalanced tag. So each block
 * is made whole on its own: a heading in bold, a list with bullets, a table as a block of code (Telegram has none), a
 * quote as a quote. Anything it does not know is plain, escaped text.
 */
export function telegramBlocks(markdown: string, max: number = TELEGRAM_MESSAGE_MAX): string[] {
  const blocks: string[] = []
  const lines = markdown.replace(/\r\n?/g, '\n').split('\n')
  let paragraph: string[] = []
  const endParagraph = (): void => {
    if (paragraph.length === 0) return
    const html = paragraph.join('\n')
    paragraph = []
    if (html.length <= max) { blocks.push(html); return }
    // A paragraph longer than a message: split between words.
    let rest = html.replace(/<[^>]+>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&')
    while (rest.length > 0) {
      const room = max - 20
      const at = rest.length <= room ? rest.length : Math.max(rest.lastIndexOf(' ', room), Math.floor(room / 2))
      blocks.push(escaped(rest.slice(0, at)))
      rest = rest.slice(at).trimStart()
    }
  }
  for (let at = 0; at < lines.length; at += 1) {
    const line = lines[at]!
    const fence = /^\s*(```+|~~~+)\s*([^\s`]*)\s*$/.exec(line)
    if (fence !== null) {
      endParagraph()
      const body: string[] = []
      let end = at + 1
      while (end < lines.length && !lines[end]!.trim().startsWith(fence[1]!)) { body.push(lines[end]!); end += 1 }
      at = end
      blocks.push(...prePieces(body.join('\n'), fence[2] || undefined, max))
      continue
    }
    if (/^\s*\|.*\|\s*$/.test(line)) {
      endParagraph()
      const table: string[] = []
      while (at < lines.length && /^\s*\|.*\|\s*$/.test(lines[at]!)) {
        // The row of dashes under a table's head says nothing in a block of code.
        if (!/^\s*\|[\s:|-]+\|\s*$/.test(lines[at]!)) table.push(lines[at]!.trim())
        at += 1
      }
      at -= 1
      blocks.push(...prePieces(table.join('\n'), undefined, max))
      continue
    }
    if (/^\s*>/.test(line)) {
      endParagraph()
      const quoted: string[] = []
      while (at < lines.length && /^\s*>/.test(lines[at]!)) { quoted.push(inline(lines[at]!.replace(/^\s*>\s?/, ''))); at += 1 }
      at -= 1
      blocks.push(`<blockquote>${quoted.join('\n')}</blockquote>`)
      continue
    }
    if (line.trim().length === 0) { endParagraph(); continue }
    const heading = /^\s*#{1,6}\s+(.*?)\s*#*\s*$/.exec(line)
    if (heading !== null) { endParagraph(); blocks.push(`<b>${inline(heading[1]!)}</b>`); continue }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { endParagraph(); continue }
    const bullet = /^(\s*)[-*+]\s+(.*)$/.exec(line)
    paragraph.push(bullet === null ? inline(line) : `${'  '.repeat(Math.min(3, Math.floor(bullet[1]!.length / 2)))}• ${inline(bullet[2]!)}`)
  }
  endParagraph()
  return blocks
}

/** Blocks into messages, each under the limit, in order. */
export function packedMessages(blocks: readonly string[], max: number = TELEGRAM_MESSAGE_MAX): string[] {
  const messages: string[] = []
  let current = ''
  for (const block of blocks) {
    if (current.length > 0 && current.length + 2 + block.length > max) { messages.push(current); current = '' }
    current = current.length === 0 ? block : `${current}\n\n${block}`
  }
  if (current.length > 0) messages.push(current)
  return messages
}

export const TELEGRAM_REST_IN_LOCUST = '<i>The rest is in Locust on your computer.</i>'

/**
 * An answer as the messages that carry it: who answered, in bold, then the answer, at most TELEGRAM_MAX_PARTS
 * messages, the last of them saying where the rest is when there is more.
 */
export function answerMessages(name: string, answer: string, max: number = TELEGRAM_MESSAGE_MAX): string[] {
  const messages = packedMessages([`<b>${escaped(name)}</b>`, ...telegramBlocks(answer, max - 40)], max)
  if (messages.length <= TELEGRAM_MAX_PARTS) return messages
  const kept = messages.slice(0, TELEGRAM_MAX_PARTS)
  const last = kept[TELEGRAM_MAX_PARTS - 1]!
  kept[TELEGRAM_MAX_PARTS - 1] = last.length + 2 + TELEGRAM_REST_IN_LOCUST.length <= max ? `${last}\n\n${TELEGRAM_REST_IN_LOCUST}` : TELEGRAM_REST_IN_LOCUST
  return kept
}

/** The same words without the markup, for when Telegram refuses the markup. */
export function plainOf(html: string): string {
  return html.replace(/<[^>]+>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&')
}
