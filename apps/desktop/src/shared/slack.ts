/**
 * YOUR TEAMMATES IN SLACK (0.832, shared/chat-apps.ts). Colin, 2026-10-23: "id download slack". A person makes a Slack
 * app of their own in one step -- Settings' link opens Slack's own "create an app" with everything already filled in
 * (Slack's app manifest) -- installs it to their workspace, and pastes its two tokens: the bot's, which writes, and the
 * app's own, which opens the one connection Locust keeps to Slack while it is open (Socket Mode: nothing listens on
 * this computer). Then they send the app the code Settings shows, and "Ada: check the build" reaches Ada. Slack has no
 * per-teammate commands, so a teammate is named the way a person writes to anyone; the answer comes back in the chat,
 * or in the thread it was asked in, and a card comes with Allow once and Deny buttons, never Always.
 */
import { CHAT_MAX_PARTS, CHAT_REST_IN_LOCUST } from './chat-apps.js'

export const SLACK_SETTINGS_CHANNEL = 'slack:settings'
export const SLACK_SET_TOKEN_CHANNEL = 'slack:set-token'
export const SLACK_NEW_LINK_CHANNEL = 'slack:new-link'
export const SLACK_REMOVE_CHANNEL = 'slack:remove'
export const SLACK_CHANGED_CHANNEL = 'slack:changed'

/** The bot's token, from the app's OAuth & Permissions page once it is installed. */
export const SLACK_BOT_TOKEN = /^xoxb-[A-Za-z0-9-]{20,}$/
/** The app's own token, made under Basic Information > App-Level Tokens with the connections:write scope. */
export const SLACK_APP_TOKEN = /^xapp-\d-[A-Za-z0-9-]{20,}$/

/**
 * The app, as Slack's manifest says it: a bot in the person's direct messages (its Messages tab, open for writing),
 * reading only those, reacting to show it is working, and reached over Socket Mode -- no address of Locust's anywhere.
 * users:read names the person in Settings once they pair.
 */
export const SLACK_MANIFEST = {
  _metadata: { major_version: 1, minor_version: 1 },
  display_information: {
    name: 'Locust',
    description: 'Your Locust teammates, in Slack. It answers while Locust is open on your computer.',
    background_color: '#1b1d22'
  },
  features: {
    app_home: { home_tab_enabled: false, messages_tab_enabled: true, messages_tab_read_only_enabled: false },
    bot_user: { display_name: 'Locust', always_online: true }
  },
  oauth_config: { scopes: { bot: ['chat:write', 'im:history', 'reactions:write', 'users:read'] } },
  settings: {
    event_subscriptions: { bot_events: ['message.im'] },
    interactivity: { is_enabled: true },
    org_deploy_enabled: false,
    socket_mode_enabled: true,
    token_rotation_enabled: false
  }
} as const

/** Slack's own "create an app", filled in with Locust's manifest. */
export const SLACK_MANIFEST_LINK = `https://api.slack.com/apps?new_app=1&manifest_json=${encodeURIComponent(JSON.stringify(SLACK_MANIFEST))}`
/** Where the person's apps are, to find the tokens of the one they made. */
export const SLACK_APPS_PAGE = 'https://api.slack.com/apps'

/** Slack takes this much in one message's words; this leaves room for the header and a closing fence. */
export const SLACK_MESSAGE_MAX = 3_800

/** Text made safe in Slack's markup: only these three are its own. */
export const slackEscape = (text: string): string => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/** A person's words as Slack sent them, back to plain: its links, mentions and the three escaped marks. */
export function slackPlain(text: string): string {
  return text
    .replace(/<((?:https?|mailto):[^|>]+)\|([^>]+)>/g, (_all, url: string, label: string) => (url.replace(/^mailto:/, '') === label || url === label ? label : `${label} (${url})`))
    .replace(/<((?:https?|mailto):[^>]+)>/g, (_all, url: string) => url.replace(/^mailto:/, ''))
    .replace(/<#[A-Z0-9]+\|([^>]+)>/g, '#$1')
    .replace(/<@[A-Z0-9]+>/g, '@someone')
    .replace(/<!(here|channel|everyone)>/g, '@$1')
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&')
}

/** One line's Markdown in Slack's markup: code first, since nothing inside it is markup; then links and emphasis. */
function inline(text: string): string {
  return text.split(/(`[^`\n]+`)/).map((part, index) => {
    if (index % 2 === 1) return `\`${slackEscape(part.slice(1, -1))}\``
    let out = slackEscape(part)
    out = out.replace(/!?\[([^\]\n]+)\]\((https?:\/\/[^\s)<>]+)\)/g, (_all, label: string, url: string) => `<${url}|${label.replace(/[|<>]/g, ' ')}>`)
    // Bold is held aside while italics are read, since Slack's bold is Markdown's italic mark.
    out = out.replace(/\*\*(?=\S)([^*\n]*?\S)\*\*/g, '\u0001$1\u0001').replace(/__(?=\S)([^_\n]*?\S)__/g, '\u0001$1\u0001')
    out = out.replace(/(^|[\s(])\*(?=[^\s*])([^*\n]*?[^\s*])\*(?=$|[\s).,;:!?])/g, '$1_$2_')
    out = out.replace(/~~(?=\S)([^~\n]*?\S)~~/g, '~$1~')
    return out.replace(/\u0001/g, '*')
  }).join('')
}

/** A block of code too long for one message, as several, split between lines, each fenced again. */
function codePieces(text: string, max: number): string[] {
  const pieces: string[] = []
  let lines: string[] = []
  const flush = (): void => { if (lines.length > 0) pieces.push(`\`\`\`\n${lines.join('\n')}\n\`\`\``); lines = [] }
  for (const line of slackEscape(text).replace(/```/g, '`​``').split('\n')) {
    // A line longer than a message on its own is cut; Locust on the computer has the whole of it.
    const cut = line.length > max - 40 ? `${line.slice(0, max - 41)}…` : line
    if ([...lines, cut].join('\n').length + 10 > max) flush()
    lines.push(cut)
  }
  flush()
  return pieces
}

/**
 * MARKDOWN AS SLACK SHOWS IT. Slack's own markup is Markdown's cousin: one star for bold, an underscore for italics,
 * links in angle brackets, and no headings, lists or tables. So each block is made into what Slack reads: a heading in
 * bold, a list with bullets, a table as a block of code, a quote as a quote, a block of code without its language.
 */
export function slackBlocks(markdown: string, max: number = SLACK_MESSAGE_MAX): string[] {
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
      blocks.push(...codePieces(body.join('\n'), max))
      continue
    }
    if (/^\s*\|.*\|\s*$/.test(line)) {
      endParagraph()
      const table: string[] = []
      while (at < lines.length && /^\s*\|.*\|\s*$/.test(lines[at]!)) {
        if (!/^\s*\|[\s:|-]+\|\s*$/.test(lines[at]!)) table.push(lines[at]!.trim())
        at += 1
      }
      at -= 1
      blocks.push(...codePieces(table.join('\n'), max))
      continue
    }
    if (/^\s*>/.test(line)) {
      endParagraph()
      const quoted: string[] = []
      while (at < lines.length && /^\s*>/.test(lines[at]!)) { quoted.push(`> ${inline(lines[at]!.replace(/^\s*>\s?/, ''))}`); at += 1 }
      at -= 1
      blocks.push(quoted.join('\n'))
      continue
    }
    if (line.trim().length === 0) { endParagraph(); continue }
    const heading = /^\s*#{1,6}\s+(.*?)\s*#*\s*$/.exec(line)
    if (heading !== null) { endParagraph(); blocks.push(`*${inline(heading[1]!).replace(/\*/g, '')}*`); continue }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { endParagraph(); continue }
    const bullet = /^(\s*)[-*+]\s+(.*)$/.exec(line)
    paragraph.push(bullet === null ? inline(line) : `${'    '.repeat(Math.min(3, Math.floor(bullet[1]!.length / 2)))}• ${inline(bullet[2]!)}`)
  }
  endParagraph()
  return blocks
}

/** Blocks into messages, each under the limit, in order. */
export function slackPacked(blocks: readonly string[], max: number = SLACK_MESSAGE_MAX): string[] {
  const messages: string[] = []
  let current = ''
  for (const block of blocks) {
    if (current.length > 0 && current.length + 2 + block.length > max) { messages.push(current); current = '' }
    current = current.length === 0 ? block : `${current}\n\n${block}`
  }
  if (current.length > 0) messages.push(current)
  return messages
}

export const SLACK_REST_IN_LOCUST = `_${CHAT_REST_IN_LOCUST}_`

/** An answer as the messages that carry it: who answered, in bold, then the answer, at most four messages. */
export function slackMessages(name: string, answer: string, max: number = SLACK_MESSAGE_MAX): string[] {
  const messages = slackPacked([`*${slackEscape(name)}*`, ...slackBlocks(answer, max - 40)], max)
  if (messages.length <= CHAT_MAX_PARTS) return messages
  const kept = messages.slice(0, CHAT_MAX_PARTS)
  const last = kept[CHAT_MAX_PARTS - 1]!
  kept[CHAT_MAX_PARTS - 1] = last.length + 2 + SLACK_REST_IN_LOCUST.length <= max ? `${last}\n\n${SLACK_REST_IN_LOCUST}` : SLACK_REST_IN_LOCUST
  return kept
}
