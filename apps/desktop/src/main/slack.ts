import { randomInt } from 'node:crypto'

import { pairingCodeIn } from '../shared/chat-apps.js'
import { SLACK_APP_TOKEN, SLACK_BOT_TOKEN, slackEscape, slackMessages, slackPlain } from '../shared/slack.js'
import {
  createChatLink, type ChatApp, type ChatBot, type ChatEvents, type ChatIncoming, type ChatLink, type ChatLinkOptions, type ChatTransport
} from './chat-link.js'

/*
 * YOUR TEAMMATES IN SLACK (0.832, shared/slack.ts). The person's own Slack app, their direct messages with it, and
 * Locust in between: one Socket Mode connection while Locust is open -- each envelope acknowledged at once, as Slack
 * asks, and the connection made again when Slack refreshes it -- and Slack's Web API to answer. Only the app's direct
 * messages are subscribed to, and only the paired person is heard (chat-link.ts).
 */
export const SLACK_FILE = 'slack.json'
export const SLACK_API = 'https://slack.com'
const TOKEN_REFUSED = 'Slack no longer takes the app’s tokens. Paste them again from the app’s settings: Remove, then set it up again.'
const REFUSED = new Set(['invalid_auth', 'not_authed', 'account_inactive', 'token_revoked', 'token_expired', 'not_allowed_token_type'])

export interface SlackOptions extends ChatLinkOptions {
  readonly fetcher?: typeof fetch
  /** Slack's address; a drive names a stand-in on this computer. */
  readonly api?: () => string
  /** The WebSocket Socket Mode is reached with; a test brings a stand-in. */
  readonly socket?: typeof WebSocket
}

/** What Slack said instead of an answer: its own error's name. */
export class SlackError extends Error {
  constructor(readonly error: string, readonly status: number) { super(error) }
}

interface SlackTokens { readonly bot: string; readonly app: string }
interface SlackEvent {
  readonly type?: string
  readonly subtype?: string
  readonly channel?: string
  readonly channel_type?: string
  readonly user?: string
  readonly bot_id?: string
  readonly text?: string
  readonly ts?: string
  readonly thread_ts?: string
  readonly files?: readonly unknown[]
}

const pause = (ms: number): Promise<void> => new Promise((settle) => setTimeout(settle, ms))
const tokensOf = (secret: string): SlackTokens | undefined => {
  try {
    const parsed = JSON.parse(secret) as Partial<SlackTokens>
    return typeof parsed.bot === 'string' && typeof parsed.app === 'string' && SLACK_BOT_TOKEN.test(parsed.bot) && SLACK_APP_TOKEN.test(parsed.app) ? { bot: parsed.bot, app: parsed.app } : undefined
  } catch { return undefined }
}

/** One Web API method, its arguments as a form, the token in the header -- never the form, and never said. */
function slackWeb(options: SlackOptions) {
  const fetcher = options.fetcher ?? fetch
  return async <T>(token: string, method: string, args: Record<string, unknown> = {}): Promise<T> => {
    const form = new URLSearchParams()
    for (const [key, value] of Object.entries(args)) if (value !== undefined) form.set(key, typeof value === 'string' ? value : JSON.stringify(value))
    for (let attempt = 0; ; attempt += 1) {
      let response: Response
      try {
        response = await fetcher(`${(options.api?.() ?? SLACK_API).replace(/\/+$/, '')}/api/${method}`, {
          method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/x-www-form-urlencoded' }, body: form.toString(), redirect: 'error'
        })
      } catch { throw new SlackError('unreachable', 0) }
      if (response.status === 429 && attempt === 0) { await pause(Math.min(30, Number(response.headers.get('retry-after')) || 2) * 1000); continue }
      const data = await response.json().catch(() => undefined) as ({ ok?: boolean; error?: string } & T) | undefined
      if (data?.ok === true) return data
      throw new SlackError(typeof data?.error === 'string' ? data.error.slice(0, 80) : `http_${String(response.status)}`, response.status)
    }
  }
}

function slackTransport(tokens: SlackTokens, bot: ChatBot, options: SlackOptions, events: ChatEvents): ChatTransport {
  const web = slackWeb(options)
  // A stand-in on this computer serves its own sockets; Slack's are only ever Slack's.
  const local = /^http:\/\/(?:127\.0\.0\.1|localhost):\d{1,5}$/.test(options.api?.() ?? '')
  const allowed = (url: unknown): url is string =>
    typeof url === 'string' && (local ? /^wss?:\/\/(?:127\.0\.0\.1|localhost):\d{1,5}\//.test(url) : /^wss:\/\/[a-z0-9.-]+\.slack\.com\//i.test(url))
  let stopped = false
  let socket: WebSocket | undefined
  let helloOn: WebSocket | undefined
  let retry: ReturnType<typeof setTimeout> | undefined
  let backoff = 1_000
  const later = (ms: number): void => {
    clearTimeout(retry)
    if (!stopped) retry = setTimeout(() => { void open() }, ms)
  }
  const refused = (error: unknown): boolean => error instanceof SlackError && REFUSED.has(error.error)

  const incomingOf = (event: SlackEvent): ChatIncoming => {
    const ts = event.ts ?? ''
    // In a thread: it answers the thread's first message, and its answer goes in the thread too.
    const thread = typeof event.thread_ts === 'string' && event.thread_ts !== ts ? event.thread_ts : undefined
    const file = event.subtype === 'file_share' || (Array.isArray(event.files) && event.files.length > 0)
    return {
      from: event.user ?? '',
      // The app's own messages, another bot's, and an edit or a deletion are never acted on.
      bot: event.bot_id !== undefined || event.user === String(bot.id) || (event.subtype !== undefined && event.subtype !== 'file_share' && event.subtype !== 'thread_broadcast'),
      place: event.channel ?? '',
      direct: event.channel_type === 'im',
      id: ts,
      ...(file || typeof event.text !== 'string' || event.text.trim().length === 0 ? {} : { text: slackPlain(event.text) }),
      sentAt: Math.floor(Number(ts) * 1000) || Date.now(),
      ...(thread === undefined ? {} : { replyTo: thread, answerTo: thread })
    }
  }

  const received = (ws: WebSocket, raw: unknown): void => {
    let envelope: { type?: string; envelope_id?: string; reason?: string; payload?: Record<string, unknown> }
    try { envelope = JSON.parse(typeof raw === 'string' ? raw : Buffer.from(raw as ArrayBuffer).toString('utf8')) as typeof envelope } catch { return }
    // Acknowledged at once, before anything is done with it, so Slack does not send it again.
    if (typeof envelope.envelope_id === 'string' && ws.readyState === 1) ws.send(JSON.stringify({ envelope_id: envelope.envelope_id }))
    if (envelope.type === 'hello') { helloOn = ws; backoff = 1_000; events.connected(); return }
    if (envelope.type === 'disconnect') {
      if (envelope.reason === 'link_disabled') { events.trouble('Socket Mode is off for the Slack app, so Locust cannot hear it. Turn it on in the app’s settings, under Socket Mode.', true); return }
      // A refresh: the new connection first, then this one goes.
      socket = undefined
      void open().finally(() => { try { ws.close(1000) } catch { /* closed already */ } })
      return
    }
    if (envelope.type === 'events_api') {
      const event = (envelope.payload?.event ?? {}) as SlackEvent
      if (event.type === 'message') void events.message(incomingOf(event))
      return
    }
    if (envelope.type === 'interactive' && envelope.payload?.type === 'block_actions') {
      const payload = envelope.payload as { user?: { id?: string }; channel?: { id?: string }; container?: { channel_id?: string; message_ts?: string }; message?: { ts?: string }; actions?: readonly { action_id?: string; value?: string }[] }
      const action = payload.actions?.[0]
      void events.press({
        from: payload.user?.id ?? '',
        place: payload.channel?.id ?? payload.container?.channel_id ?? '',
        message: payload.message?.ts ?? payload.container?.message_ts ?? '',
        data: action?.action_id ?? action?.value ?? '',
        answer: async () => undefined
      })
    }
  }

  const open = async (): Promise<void> => {
    if (stopped || socket !== undefined) return
    let url: string | undefined
    try {
      url = (await web<{ url?: string }>(tokens.app, 'apps.connections.open')).url
    } catch (error) {
      if (refused(error)) { events.trouble(TOKEN_REFUSED, true); return }
      events.trouble('Slack could not be reached. Locust keeps trying.')
      later(backoff)
      backoff = Math.min(backoff * 2, 60_000)
      return
    }
    if (stopped) return
    if (!allowed(url)) { events.trouble('Slack gave an address Locust does not trust, so it did not connect. Locust tries again.'); later(60_000); return }
    let ws: WebSocket
    try { ws = options.socket === undefined ? new WebSocket(url) : new options.socket(url) } catch { later(backoff); return }
    socket = ws
    ws.onmessage = (event: MessageEvent) => { received(ws, event.data) }
    ws.onclose = () => {
      if (socket !== ws) return
      socket = undefined
      if (stopped) return
      if (helloOn !== ws) {
        events.trouble('Slack could not be reached. Locust keeps trying.')
        later(backoff)
        backoff = Math.min(backoff * 2, 60_000)
        return
      }
      later(500)
    }
    // An error is followed by a close, which is where it is taken.
    ws.onerror = () => undefined
  }
  void open()

  const failed = (error: unknown, what: string): void => {
    if (refused(error)) events.trouble(TOKEN_REFUSED, true)
    else if (error instanceof SlackError && error.error === 'missing_scope') events.problem('The Slack app is missing a permission Locust needs. Make it again from Locust’s link, then paste its tokens.')
    events.note(`${what}: ${error instanceof Error ? error.message : String(error)}`)
  }

  return {
    async stop() {
      stopped = true
      clearTimeout(retry)
      const ws = socket
      socket = undefined
      if (ws !== undefined && ws.readyState <= 1) { try { ws.close(1000) } catch { /* closed already */ } }
    },
    async send(place, text, extra) {
      const buttons = extra.buttons?.flat().slice(0, 25) ?? []
      try {
        const sent = await web<{ ts: string }>(tokens.bot, 'chat.postMessage', {
          channel: place,
          text,
          unfurl_links: false,
          unfurl_media: false,
          // In a thread, every part of an answer goes in it.
          ...(extra.answerTo === undefined ? {} : { thread_ts: extra.answerTo }),
          ...(buttons.length === 0 ? {} : {
            blocks: [
              { type: 'section', text: { type: 'mrkdwn', text: text.slice(0, 3_000) } },
              { type: 'actions', elements: buttons.map((button) => ({ type: 'button', text: { type: 'plain_text', text: button.label.slice(0, 75), emoji: true }, action_id: button.data, value: button.data, ...(button.style === 'allow' ? { style: 'primary' } : button.style === 'deny' ? { style: 'danger' } : {}) })) }
            ]
          })
        })
        return sent.ts
      } catch (error) {
        failed(error, 'a message was not sent')
        return undefined
      }
    },
    async edit(place, message, text) {
      // Words and no blocks: the buttons go.
      await web(tokens.bot, 'chat.update', { channel: place, ts: message, text, blocks: [] }).catch((error: unknown) => { failed(error, 'a message was not changed') })
    },
    // Slack shows no bot typing: the person's message is marked while its turn works.
    working: async (place, message, on) => {
      await web(tokens.bot, on ? 'reactions.add' : 'reactions.remove', { channel: place, timestamp: message, name: 'eyes' }).catch(() => undefined)
    },
    whoIs: async (user) => {
      const found = await web<{ user?: { name?: string; real_name?: string; profile?: { display_name?: string; real_name?: string } } }>(tokens.bot, 'users.info', { user }).catch(() => undefined)
      const person = found?.user
      if (person === undefined) return undefined
      const name = person.profile?.display_name || person.profile?.real_name || person.real_name || person.name
      return { ...(name === undefined ? {} : { name }), ...(person.name === undefined ? {} : { username: person.name }) }
    }
  }
}

/** Slack as a chat link needs it: its markup, a teammate named as a person writes, and a code sent to the app to pair. */
export function slackApp(options: SlackOptions): ChatApp {
  const web = slackWeb(options)
  return {
    id: 'slack',
    file: SLACK_FILE,
    escape: slackEscape,
    bold: (text) => `*${slackEscape(text)}*`,
    italic: (text) => `_${slackEscape(text)}_`,
    answerParts: slackMessages,
    // A slash is Slack's own, so Locust's commands are words on their own: "who", "new", "waiting", "help".
    parse: (text) => {
      const own = /^(who|new|waiting|help)[.!?]?$/i.exec(text.trim())
      if (own !== null) return { command: own[1]!.toLowerCase(), rest: '' }
      const fresh = /^new\s*:\s*([\s\S]+)$/i.exec(text.trim())
      return fresh === null ? undefined : { command: 'new', rest: fresh[1]!.trim() }
    },
    codeIn: (text) => pairingCodeIn(text),
    freshCode: () => String(randomInt(0, 100_000_000)).padStart(8, '0'),
    pairing: (_bot, code, until) => ({ code: { code, until } }),
    shown: (bot: ChatBot) => ({
      username: bot.username,
      name: bot.name,
      ...(bot.extra?.team === undefined ? {} : { team: bot.extra.team }),
      ...(bot.extra?.app === undefined || bot.extra.teamId === undefined ? {} : { open: `https://slack.com/app_redirect?app=${encodeURIComponent(bot.extra.app)}&team=${encodeURIComponent(bot.extra.teamId)}` })
    }),
    shape: (given) => {
      const pasted = (typeof given === 'object' && given !== null ? given : {}) as { bot?: unknown; app?: unknown }
      const botToken = typeof pasted.bot === 'string' ? pasted.bot.trim() : ''
      const appToken = typeof pasted.app === 'string' ? pasted.app.trim() : ''
      if (SLACK_APP_TOKEN.test(botToken) && SLACK_BOT_TOKEN.test(appToken)) return { ok: false, message: 'The two tokens are in each other’s places: the bot’s begins xoxb-, the app’s begins xapp-.' }
      if (!SLACK_BOT_TOKEN.test(botToken)) return { ok: false, message: 'That is not the bot’s token. In your Slack app, open OAuth & Permissions, and copy the Bot User OAuth Token: it begins xoxb-.' }
      if (!SLACK_APP_TOKEN.test(appToken)) return { ok: false, message: 'That is not the app’s own token. In Basic Information, under App-Level Tokens, make one with the connections:write scope, and copy it: it begins xapp-.' }
      return { ok: true, secret: JSON.stringify({ bot: botToken, app: appToken }) }
    },
    verify: async (secret) => {
      const tokens = tokensOf(secret)
      if (tokens === undefined) return { ok: false, message: 'Those are not the app’s tokens.' }
      let who: { url?: string; team?: string; team_id?: string; user?: string; user_id?: string; bot_id?: string }
      try {
        who = await web(tokens.bot, 'auth.test')
      } catch (error) {
        return { ok: false, message: error instanceof SlackError && REFUSED.has(error.error) ? 'Slack does not know that bot token. Copy it again from the app’s OAuth & Permissions page: install the app to your workspace first.' : 'Slack could not be reached. Check the connection, then try again.' }
      }
      let app: string | undefined
      try {
        const opened = await web<{ url?: string }>(tokens.app, 'apps.connections.open')
        app = typeof opened.url === 'string' ? new URL(opened.url).searchParams.get('app_id') ?? undefined : undefined
      } catch (error) {
        return {
          ok: false,
          message: error instanceof SlackError && REFUSED.has(error.error)
            ? 'Slack does not take that app token. Make it under Basic Information > App-Level Tokens, with the connections:write scope.'
            : error instanceof SlackError && error.status !== 0 ? `Slack would not open a connection for the app (${error.error}). Check that Socket Mode is on in the app’s settings.` : 'Slack could not be reached. Check the connection, then try again.'
        }
      }
      if (who.user_id === undefined) return { ok: false, message: 'That token is not a bot’s. Copy the Bot User OAuth Token, which begins xoxb-.' }
      return {
        ok: true,
        bot: {
          id: who.user_id,
          username: who.user ?? 'locust',
          name: who.user ?? 'Locust',
          extra: { ...(who.team === undefined ? {} : { team: who.team }), ...(who.team_id === undefined ? {} : { teamId: who.team_id }), ...(app === undefined ? {} : { app }) }
        }
      }
    },
    keeps: (secret) => tokensOf(secret) !== undefined,
    connect: (secret, bot, events) => slackTransport(tokensOf(secret)!, bot, options, events),
    words: {
      address: (_command, name) => `“${name}:”`,
      own: (command) => `“${command}”`,
      whoLine: (_command, name, role) => `*${slackEscape(name)}*, ${slackEscape(role)}`,
      carryOn: 'Reply in an answer’s thread to carry on that conversation.',
      replyToCard: { orOwn: 'Or reply in this message’s thread with an answer of your own.', only: 'Reply in this message’s thread with your answer.' },
      wrongCode: 'That code does not pair this account. Make a new one in Locust: Settings > General > Chat apps.',
      noToken: 'Paste the app’s two tokens first.',
      unreadable: 'The Slack app’s tokens could not be read on this computer. Paste them again.'
    }
  }
}

export function createSlackLink(options: SlackOptions): ChatLink {
  return createChatLink(slackApp(options), options)
}
