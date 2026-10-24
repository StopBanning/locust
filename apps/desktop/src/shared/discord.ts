/**
 * YOUR TEAMMATES IN DISCORD (0.832, shared/chat-apps.ts). Colin, 2026-10-23, of 0.831's Telegram: "id be way more
 * intrigued for discord". A person makes an application of their own in Discord's Developer Portal, pastes its bot's
 * token into Settings, adds the bot to a server of theirs, and sends it the code Settings shows in a direct message:
 * that pairs their account. Then `/ada` -- picked from Discord's own list of commands -- reaches Ada, the answer comes
 * back as a reply, and a card a turn from there raises comes with Allow once and Deny buttons, never Always.
 *
 * Locust keeps one connection open to Discord's gateway while it is open, as every bot does, and reads only direct
 * messages: their words come without Discord's privileged Message Content intent, so the person turns nothing on.
 * Discord's servers can read what is written there, and Settings says so.
 */
import { CHAT_MAX_PARTS, CHAT_REST_IN_LOCUST } from './chat-apps.js'

export const DISCORD_SETTINGS_CHANNEL = 'discord:settings'
export const DISCORD_SET_TOKEN_CHANNEL = 'discord:set-token'
export const DISCORD_NEW_LINK_CHANNEL = 'discord:new-link'
export const DISCORD_REMOVE_CHANNEL = 'discord:remove'
export const DISCORD_CHANGED_CHANNEL = 'discord:changed'

export const DISCORD_DEVELOPER_PORTAL = 'https://discord.com/developers/applications'

/** A bot's token: the bot's id in base64, a dot, a time, a dot, and its signature. A margin either way. */
export const DISCORD_TOKEN = /^[A-Za-z0-9_-]{20,40}\.[A-Za-z0-9_-]{4,10}\.[A-Za-z0-9_-]{20,80}$/

/** Discord takes 2,000 characters in one message; this leaves room for the header and a closing fence. */
export const DISCORD_MESSAGE_MAX = 1_900

/** The link that adds the bot to a server of the person's: no permissions asked, since it only answers them directly. */
export const discordInvite = (applicationId: string): string =>
  `https://discord.com/oauth2/authorize?client_id=${encodeURIComponent(applicationId)}&scope=bot%20applications.commands&permissions=0`

/**
 * Plain text as Discord shows it: every mark Discord's Markdown reads is escaped, and a line's opening that would make
 * a heading, a quote or a list is too. A backslash before any mark shows the mark.
 */
export function discordEscape(text: string): string {
  return text
    .replace(/[\\*_~`|[\]<@]/g, '\\$&')
    .replace(/^(\s*)([#>+-])/gm, '$1\\$2')
    .replace(/^(\s*\d+)\./gm, '$1\\.')
}

const fenceOf = (language: string | undefined): string => (language !== undefined && /^[A-Za-z0-9+#-]{1,20}$/.test(language) ? `\`\`\`${language}` : '```')

/** A block of code too long for one message, as several, split between lines, each fenced again. */
function codePieces(text: string, language: string | undefined, max: number): string[] {
  const open = fenceOf(language)
  const pieces: string[] = []
  let lines: string[] = []
  const flush = (): void => { if (lines.length > 0) pieces.push(`${open}\n${lines.join('\n')}\n\`\`\``); lines = [] }
  for (const line of text.replace(/```/g, '`​``').split('\n')) {
    // A line longer than a message on its own is cut; Locust on the computer has the whole of it.
    const cut = line.length > max - 40 ? `${line.slice(0, max - 41)}…` : line
    if ([...lines, cut].join('\n').length + open.length + 8 > max) flush()
    lines.push(cut)
  }
  flush()
  return pieces
}

/**
 * MARKDOWN AS DISCORD SHOWS IT. Discord reads most of what a teammate writes -- bold, italics, code, lists, quotes,
 * links and three sizes of heading -- so an answer goes nearly as written. What it does not read is made so it does:
 * a table becomes a block of code, a smaller heading bold, a picture a link, a rule nothing. A block too long for one
 * message is split where a reader would split it, and a block of code is fenced again in each piece.
 */
export function discordBlocks(markdown: string, max: number = DISCORD_MESSAGE_MAX): string[] {
  const blocks: string[] = []
  const lines = markdown.replace(/\r\n?/g, '\n').split('\n')
  let paragraph: string[] = []
  const endParagraph = (): void => {
    if (paragraph.length === 0) return
    let rest = paragraph.join('\n')
    paragraph = []
    // A paragraph longer than a message: split between lines, then between words.
    while (rest.length > max) {
      const room = max - 20
      const lineEnd = rest.lastIndexOf('\n', room)
      const at = lineEnd > room / 2 ? lineEnd : Math.max(rest.lastIndexOf(' ', room), Math.floor(room / 2))
      blocks.push(rest.slice(0, at).trimEnd())
      rest = rest.slice(at).trimStart()
    }
    if (rest.length > 0) blocks.push(rest)
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
      blocks.push(...codePieces(body.join('\n'), fence[2] || undefined, max))
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
      blocks.push(...codePieces(table.join('\n'), undefined, max))
      continue
    }
    if (line.trim().length === 0) { endParagraph(); continue }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { endParagraph(); continue }
    const heading = /^\s*(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line)
    if (heading !== null) {
      endParagraph()
      blocks.push(heading[1]!.length <= 3 ? `${heading[1]!} ${heading[2]!}` : `**${heading[2]!}**`)
      continue
    }
    // A picture is a link in Discord: its words, to where it is.
    paragraph.push(line.replace(/!\[([^\]\n]*)\]\((https?:\/\/[^\s)]+)\)/g, (_all, label: string, url: string) => `[${label.length > 0 ? label : 'picture'}](${url})`))
  }
  endParagraph()
  return blocks
}

/** Blocks into messages, each under the limit, in order. */
export function discordPacked(blocks: readonly string[], max: number = DISCORD_MESSAGE_MAX): string[] {
  const messages: string[] = []
  let current = ''
  for (const block of blocks) {
    if (current.length > 0 && current.length + 2 + block.length > max) { messages.push(current); current = '' }
    current = current.length === 0 ? block : `${current}\n\n${block}`
  }
  if (current.length > 0) messages.push(current)
  return messages
}

export const DISCORD_REST_IN_LOCUST = `*${CHAT_REST_IN_LOCUST}*`

/** An answer as the messages that carry it: who answered, in bold, then the answer, at most four messages. */
export function discordMessages(name: string, answer: string, max: number = DISCORD_MESSAGE_MAX): string[] {
  const messages = discordPacked([`**${discordEscape(name)}**`, ...discordBlocks(answer, max - 40)], max)
  if (messages.length <= CHAT_MAX_PARTS) return messages
  const kept = messages.slice(0, CHAT_MAX_PARTS)
  const last = kept[CHAT_MAX_PARTS - 1]!
  kept[CHAT_MAX_PARTS - 1] = last.length + 2 + DISCORD_REST_IN_LOCUST.length <= max ? `${last}\n\n${DISCORD_REST_IN_LOCUST}` : DISCORD_REST_IN_LOCUST
  return kept
}
