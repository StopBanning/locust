import { randomInt } from 'node:crypto'

import { pairingCodeIn } from '../shared/chat-apps.js'
import { DISCORD_TOKEN, discordEscape, discordInvite, discordMessages } from '../shared/discord.js'
import {
  createChatLink, type ChatApp, type ChatBot, type ChatButtons, type ChatEvents, type ChatLink, type ChatLinkOptions, type ChatTransport
} from './chat-link.js'

/*
 * YOUR TEAMMATES IN DISCORD (0.832, shared/discord.ts). The person's own bot, their own direct messages with it, and
 * Locust in between: one connection to Discord's gateway while Locust is open (heartbeats, and a resume when it drops,
 * as Discord asks), and its REST calls to answer. Only direct messages are asked for, so the bot reads nothing in any
 * server it is in, and only the paired account is heard (chat-link.ts).
 */
export const DISCORD_FILE = 'discord.json'
export const DISCORD_API = 'https://discord.com'
const GATEWAY = 'wss://gateway.discord.gg'
/** DIRECT_MESSAGES (1 << 12): a person's messages to the bot, whose words come without the privileged intent. */
const INTENTS = 1 << 12
const TOKEN_REFUSED = 'Discord no longer takes this bot’s token. On the bot’s page in the Developer Portal, press Reset Token and paste the new one.'

export interface DiscordOptions extends ChatLinkOptions {
  readonly fetcher?: typeof fetch
  /** Discord's address; a drive names a stand-in on this computer. */
  readonly api?: () => string
  /** The WebSocket the gateway is reached with; a test brings a stand-in. */
  readonly socket?: typeof WebSocket
  /** Locust's version, said in each request as Discord asks of a bot. */
  readonly version?: string
}

/** What Discord said instead of an answer. */
export class DiscordError extends Error {
  constructor(readonly status: number, readonly code: number, readonly detail: string) { super(detail) }
}

interface DiscordUser { readonly id: string; readonly username?: string; readonly global_name?: string | null; readonly discriminator?: string; readonly bot?: boolean }
interface DiscordMessage {
  readonly id: string
  readonly channel_id: string
  readonly guild_id?: string
  readonly author?: DiscordUser
  readonly webhook_id?: string
  readonly content?: string
  readonly timestamp?: string
  readonly type?: number
  readonly message_reference?: { readonly message_id?: string }
  readonly attachments?: readonly unknown[]
}
interface DiscordInteraction {
  readonly id: string
  readonly application_id: string
  readonly type: number
  readonly token: string
  readonly guild_id?: string
  readonly channel_id?: string
  readonly channel?: { readonly id: string }
  readonly context?: number
  readonly user?: DiscordUser
  readonly member?: { readonly user?: DiscordUser }
  readonly message?: { readonly id: string }
  readonly data?: { readonly name?: string; readonly custom_id?: string; readonly options?: readonly { readonly name: string; readonly value?: unknown }[] }
}

const pause = (ms: number): Promise<void> => new Promise((settle) => setTimeout(settle, ms))
/** When a Discord id was made: its first bits are milliseconds since 2015. */
const madeAt = (id: string): number => { try { return Number((BigInt(id) >> 22n) + 1_420_070_400_000n) } catch { return Date.now() } }

/** One REST call. The token goes in the header, never the address, and nothing that holds it is said or written. */
function discordRest(options: DiscordOptions) {
  const fetcher = options.fetcher ?? fetch
  const agent = `DiscordBot (https://locust.lol, ${options.version ?? '1'})`
  return async <T>(token: string | undefined, method: string, path: string, body?: unknown): Promise<T> => {
    for (let attempt = 0; ; attempt += 1) {
      let response: Response
      try {
        response = await fetcher(`${(options.api?.() ?? DISCORD_API).replace(/\/+$/, '')}/api/v10${path}`, {
          method,
          headers: { 'user-agent': agent, ...(token === undefined ? {} : { authorization: `Bot ${token}` }), ...(body === undefined ? {} : { 'content-type': 'application/json' }) },
          ...(body === undefined ? {} : { body: JSON.stringify(body) }),
          redirect: 'error'
        })
      } catch { throw new DiscordError(0, 0, 'Discord could not be reached.') }
      if (response.status === 204) return undefined as T
      const data = await response.json().catch(() => undefined) as Record<string, unknown> | undefined
      if (response.ok) return data as T
      if (response.status === 429 && attempt === 0) { await pause(Math.min(30, typeof data?.retry_after === 'number' ? data.retry_after : 2) * 1000); continue }
      throw new DiscordError(response.status, typeof data?.code === 'number' ? data.code : 0, typeof data?.message === 'string' ? data.message.slice(0, 300) : `Discord answered ${String(response.status)}.`)
    }
  }
}

/**
 * Discord's buttons: five to a row and five rows. A list of short ones -- the teammates to pick from -- sits side by
 * side, five to a row; longer ones, an answer's words, stay one to a row while they fit.
 */
function componentsOf(buttons: ChatButtons): unknown[] {
  const short = buttons.length > 1 && buttons.every((row) => row.length === 1 && row[0]!.label.length <= 20)
  const rows = !short && buttons.length <= 5 && buttons.every((row) => row.length <= 5) ? buttons : (() => {
    const flat = buttons.flat()
    const laid = []
    for (let at = 0; at < flat.length; at += 5) laid.push(flat.slice(at, at + 5))
    return laid
  })()
  return rows.slice(0, 5).map((row) => ({
    type: 1,
    components: row.map((button) => ({ type: 2, style: button.style === 'allow' ? 3 : button.style === 'deny' ? 4 : 2, label: button.label.slice(0, 80), custom_id: button.data.slice(0, 100) }))
  }))
}
/** A message's body: its words, no one pinged, no previews of its links, and its buttons. */
const bodyOf = (text: string, buttons?: ChatButtons): Record<string, unknown> => ({
  content: text,
  allowed_mentions: { parse: [] },
  flags: 1 << 2,
  ...(buttons === undefined || buttons.length === 0 ? {} : { components: componentsOf(buttons) })
})

function discordTransport(token: string, options: DiscordOptions, events: ChatEvents): ChatTransport {
  const rest = discordRest(options)
  // A stand-in on this computer serves its own gateway; Discord's is only ever Discord's.
  const local = /^http:\/\/(?:127\.0\.0\.1|localhost):\d{1,5}$/.test(options.api?.() ?? '')
  const allowed = (url: unknown): url is string =>
    typeof url === 'string' && (local ? /^wss?:\/\/(?:127\.0\.0\.1|localhost):\d{1,5}(?:\/|$)/.test(url) : /^wss:\/\/[a-z0-9.-]+\.discord\.gg(?:\/|$)/i.test(url))
  let stopped = false
  let socket: WebSocket | undefined
  let gateway: string | undefined
  let session: { readonly id: string; readonly resume: string } | undefined
  let seq: number | null = null
  let acked = true
  let firstBeat: ReturnType<typeof setTimeout> | undefined
  let beats: ReturnType<typeof setInterval> | undefined
  let retry: ReturnType<typeof setTimeout> | undefined
  let backoff = 1_000
  let quietFor: number | undefined
  /** The connection Discord said READY or RESUMED on: a close of it is a drop, not a failure to connect. */
  let readyOn: WebSocket | undefined

  const stopBeats = (): void => { clearTimeout(firstBeat); clearInterval(beats); firstBeat = undefined; beats = undefined }
  const later = (ms: number): void => {
    clearTimeout(retry)
    if (!stopped) retry = setTimeout(() => { void open() }, ms)
  }
  const sendPayload = (ws: WebSocket, payload: unknown): void => { if (ws.readyState === 1) ws.send(JSON.stringify(payload)) }

  /** An interaction answered, as Discord asks within three seconds. */
  const callback = async (interaction: DiscordInteraction, response: Record<string, unknown>, withResponse = false): Promise<{ interaction?: { response_message_id?: string } } | undefined> => {
    try {
      return await rest(undefined, 'POST', `/interactions/${interaction.id}/${interaction.token}/callback${withResponse ? '?with_response=true' : ''}`, response)
    } catch (error) {
      events.note(`a command was not answered in time: ${error instanceof Error ? error.message : String(error)}`)
      return undefined
    }
  }
  const original = (interaction: DiscordInteraction, method: 'GET' | 'PATCH', body?: Record<string, unknown>): Promise<{ id?: string } | undefined> =>
    rest<{ id?: string }>(undefined, method, `/webhooks/${interaction.application_id}/${interaction.token}/messages/@original`, body).catch(() => undefined)

  const interacted = async (interaction: DiscordInteraction): Promise<void> => {
    const user = interaction.user ?? interaction.member?.user
    const place = interaction.channel_id ?? interaction.channel?.id ?? ''
    const direct = interaction.guild_id === undefined && (interaction.context === undefined || interaction.context === 1)
    if (interaction.type === 3) {
      // A button: taken at once, without a change; the link says what became of it by editing the message.
      await callback(interaction, { type: 6 })
      await events.press({ from: user?.id ?? '', place: direct ? place : '', message: interaction.message?.id ?? '', data: interaction.data?.custom_id ?? '', answer: async () => undefined })
      return
    }
    if (interaction.type !== 2) return
    // A slash command: "Locust is thinking", then its answer in that same message.
    const deferred = await callback(interaction, { type: 5 }, true)
    const id = deferred?.interaction?.response_message_id ?? (await original(interaction, 'GET'))?.id
    let responded = false
    const words = interaction.data?.options?.find((one) => one.name === 'message')?.value
    const name = String(interaction.data?.name ?? '').toLowerCase()
    await events.message({
      from: user?.id ?? '',
      bot: user?.bot === true,
      place,
      direct,
      id: id ?? interaction.id,
      text: `/${name}${typeof words === 'string' && words.trim().length > 0 ? ` ${words.trim()}` : ''}`,
      sentAt: madeAt(interaction.id),
      command: { command: name, rest: typeof words === 'string' ? words.trim() : '' },
      ...(id === undefined ? {} : { answerTo: id }),
      ...(user?.global_name ?? user?.username) === undefined ? {} : { name: user?.global_name ?? user?.username },
      ...(user?.username === undefined ? {} : { username: user.username }),
      respond: async (text, buttons) => {
        responded = true
        return (await original(interaction, 'PATCH', bodyOf(text, buttons)))?.id ?? id
      }
    })
    // Anyone else's command, or one from a server, is answered with nothing of the person's.
    if (!responded) await original(interaction, 'PATCH', bodyOf('Locust answers only the person it is paired with.'))
  }

  const dispatched = (ws: WebSocket, type: string, data: Record<string, unknown>): void => {
    if (type === 'READY') {
      session = { id: String(data.session_id), resume: allowed(data.resume_gateway_url) ? data.resume_gateway_url : gateway ?? GATEWAY }
      readyOn = ws
      backoff = 1_000
      events.connected()
      return
    }
    if (type === 'RESUMED') { readyOn = ws; backoff = 1_000; events.connected(); return }
    if (type === 'MESSAGE_CREATE') {
      const message = data as unknown as DiscordMessage
      const text = typeof message.content === 'string' && message.content.length > 0 ? message.content : undefined
      void events.message({
        from: message.author?.id ?? '',
        // A webhook's, a system message, or a bot's -- this one's own included -- is never acted on.
        bot: message.author?.bot === true || message.webhook_id !== undefined || (message.type !== undefined && message.type !== 0 && message.type !== 19),
        place: message.channel_id,
        direct: message.guild_id === undefined,
        id: message.id,
        ...(text === undefined ? {} : { text }),
        sentAt: Date.parse(message.timestamp ?? '') || madeAt(message.id),
        ...(message.type === 19 && message.message_reference?.message_id !== undefined ? { replyTo: message.message_reference.message_id } : {}),
        answerTo: message.id,
        ...((message.author?.global_name ?? message.author?.username) === undefined ? {} : { name: message.author?.global_name ?? message.author?.username }),
        ...(message.author?.username === undefined ? {} : { username: message.author.username })
      })
      return
    }
    if (type === 'INTERACTION_CREATE') void interacted(data as unknown as DiscordInteraction).catch(() => undefined)
  }

  const received = (ws: WebSocket, raw: unknown): void => {
    let payload: { op?: number; d?: unknown; s?: number | null; t?: string | null }
    try { payload = JSON.parse(typeof raw === 'string' ? raw : Buffer.from(raw as ArrayBuffer).toString('utf8')) as typeof payload } catch { return }
    if (typeof payload.s === 'number') seq = payload.s
    switch (payload.op) {
      case 10: {
        // Hello: heartbeats from now on, the first after a random part of the interval, as Discord asks.
        const interval = Number((payload.d as { heartbeat_interval?: number } | undefined)?.heartbeat_interval) || 41_250
        stopBeats()
        acked = true
        const beat = (): void => {
          // No answer to the last one: the connection is a zombie. Closed, and resumed.
          if (!acked) { ws.close(4000, 'no heartbeat answer'); return }
          acked = false
          sendPayload(ws, { op: 1, d: seq })
        }
        firstBeat = setTimeout(() => { beat(); beats = setInterval(beat, interval) }, Math.floor(interval * Math.random()))
        if (session !== undefined) sendPayload(ws, { op: 6, d: { token, session_id: session.id, seq } })
        else sendPayload(ws, { op: 2, d: { token, intents: INTENTS, properties: { os: process.platform, browser: 'Locust', device: 'Locust' } } })
        return
      }
      case 11: acked = true; return
      case 1: sendPayload(ws, { op: 1, d: seq }); return
      // Reconnect: Discord asks for a new connection; the session resumes on it.
      case 7: ws.close(4000, 'reconnect asked'); return
      case 9: {
        // Invalid session: resumed when Discord says it can be, begun again otherwise -- after a pause of a few seconds.
        if (payload.d !== true) { session = undefined; seq = null }
        quietFor = 1_000 + Math.floor(Math.random() * 4_000)
        ws.close(4000, 'invalid session')
        return
      }
      case 0: if (typeof payload.t === 'string') dispatched(ws, payload.t, (payload.d ?? {}) as Record<string, unknown>); return
      default: return
    }
  }

  const closed = (code: number, wasReady: boolean): void => {
    stopBeats()
    socket = undefined
    if (stopped) return
    if (code === 4004) { events.trouble(TOKEN_REFUSED, true); return }
    if (code === 4013 || code === 4014) { events.trouble('Discord refused what Locust asks to read: only direct messages. Make a new application and paste its bot’s token.', true); return }
    if (code === 4010 || code === 4011 || code === 4012) { events.trouble('Discord closed the connection for good. Remove the bot here and add it again.', true); return }
    if (code === 4007 || code === 4009) { session = undefined; seq = null }
    const wait = quietFor ?? (wasReady ? 500 : backoff)
    quietFor = undefined
    if (!wasReady) {
      events.trouble('Discord could not be reached. Locust keeps trying.')
      backoff = Math.min(backoff * 2, 60_000)
    }
    later(wait)
  }

  const open = async (): Promise<void> => {
    if (stopped || socket !== undefined) return
    if (session === undefined && gateway === undefined) {
      try {
        const found = await rest<{ url?: string; session_start_limit?: { remaining?: number; reset_after?: number } }>(token, 'GET', '/gateway/bot')
        if (allowed(found?.url)) gateway = found.url
        if (found?.session_start_limit?.remaining === 0) {
          events.trouble('Discord asks the bot to wait before it connects again. Locust tries again later.')
          later(Math.min(Math.max(found.session_start_limit.reset_after ?? 60_000, 5_000), 3_600_000))
          return
        }
      } catch (error) {
        if (error instanceof DiscordError && error.status === 401) { events.trouble(TOKEN_REFUSED, true); return }
        events.trouble('Discord could not be reached. Locust keeps trying.')
        later(backoff)
        backoff = Math.min(backoff * 2, 60_000)
        return
      }
    }
    if (stopped) return
    const base = (session?.resume ?? gateway ?? GATEWAY).replace(/\/+$/, '')
    let ws: WebSocket
    const url = `${base}/?v=10&encoding=json`
    try { ws = options.socket === undefined ? new WebSocket(url) : new options.socket(url) } catch { later(backoff); return }
    socket = ws
    ws.onmessage = (event: MessageEvent) => { if (socket === ws) received(ws, event.data) }
    ws.onclose = (event: CloseEvent) => { if (socket === ws) closed(event.code, readyOn === ws) }
    // An error is followed by a close, which is where it is taken.
    ws.onerror = () => undefined
  }
  void open()

  return {
    async stop() {
      stopped = true
      clearTimeout(retry)
      stopBeats()
      const ws = socket
      socket = undefined
      // A close of 1000 ends the session: the bot shows offline at once.
      if (ws !== undefined && ws.readyState <= 1) { try { ws.close(1000, 'Locust closed') } catch { /* closed already */ } }
    },
    async send(place, text, extra) {
      const body = { ...bodyOf(text, extra.buttons), ...(extra.answerTo === undefined || extra.continued === true ? {} : { message_reference: { message_id: extra.answerTo, fail_if_not_exists: false } }) }
      try {
        return String((await rest<{ id: string }>(token, 'POST', `/channels/${place}/messages`, body)).id)
      } catch (error) {
        if (error instanceof DiscordError && error.code === 50007) events.problem('Discord did not deliver the bot’s message. In Discord, allow direct messages from members of the server the bot is in, then write to it again.')
        else if (error instanceof DiscordError && error.status === 401) events.trouble(TOKEN_REFUSED, true)
        events.note(`a message was not sent: ${error instanceof Error ? error.message : String(error)}`)
        return undefined
      }
    },
    async edit(place, message, text) {
      await rest(token, 'PATCH', `/channels/${place}/messages/${message}`, { content: text, components: [], allowed_mentions: { parse: [] } }).catch(() => undefined)
    },
    typing: async (place) => { await rest(token, 'POST', `/channels/${place}/typing`).catch(() => undefined) },
    // Discord shows the bot typing for ten seconds.
    typingMs: 8_000,
    commands: async (list) => {
      const application = (await rest<{ id: string }>(token, 'GET', '/applications/@me')).id
      await rest(token, 'PUT', `/applications/${application}/commands`, list.map((one) => ({
        name: one.command,
        type: 1,
        description: one.description.slice(0, 100),
        // Only in the person's direct messages with the bot, never in a server.
        contexts: [1],
        integration_types: [0],
        ...(one.words === undefined ? {} : { options: [{ type: 3, name: 'message', description: one.words.slice(0, 100), required: false, max_length: 4_000 }] })
      })))
    }
  }
}

/** Discord as a chat link needs it: its Markdown, its slash commands, and a code sent to the bot to pair. */
export function discordApp(options: DiscordOptions): ChatApp {
  const rest = discordRest(options)
  return {
    id: 'discord',
    file: DISCORD_FILE,
    escape: discordEscape,
    bold: (text) => `**${discordEscape(text)}**`,
    italic: (text) => `*${discordEscape(text)}*`,
    answerParts: discordMessages,
    parse: (text) => {
      const match = /^\/([A-Za-z0-9_]{1,32})(?:\s+([\s\S]*))?$/.exec(text.trim())
      return match === null ? undefined : { command: match[1]!.toLowerCase(), rest: (match[2] ?? '').trim() }
    },
    codeIn: (text) => pairingCodeIn(text),
    freshCode: () => String(randomInt(0, 100_000_000)).padStart(8, '0'),
    pairing: (_bot, code, until) => ({ code: { code, until } }),
    shown: (bot: ChatBot) => ({
      username: bot.username,
      name: bot.name,
      ...(bot.extra?.application === undefined ? {} : { invite: discordInvite(bot.extra.application) }),
      open: `https://discord.com/users/${String(bot.id)}`
    }),
    shape: (given) => {
      const candidate = typeof given === 'string' ? given.trim().replace(/^Bot\s+/i, '') : ''
      return DISCORD_TOKEN.test(candidate) ? { ok: true, secret: candidate } : { ok: false, message: 'That is not a bot’s token. On your application’s Bot page, press Reset Token, then copy all of the token it shows.' }
    },
    verify: async (token) => {
      try {
        const me = await rest<DiscordUser>(token, 'GET', '/users/@me')
        if (me.bot !== true) return { ok: false, message: 'That token is not a bot’s. Copy the one on your application’s Bot page.' }
        const application = await rest<{ id: string; name?: string }>(token, 'GET', '/applications/@me')
        const tag = me.discriminator !== undefined && me.discriminator !== '0' ? `${me.username ?? 'bot'}#${me.discriminator}` : me.username ?? 'bot'
        return { ok: true, bot: { id: me.id, username: tag, name: application.name ?? me.global_name ?? me.username ?? 'Your bot', extra: { application: application.id } } }
      } catch (error) {
        return { ok: false, message: error instanceof DiscordError && error.status === 401 ? 'Discord does not know that token. Copy it again from your application’s Bot page: Reset Token gives a new one.' : 'Discord could not be reached. Check the connection, then try again.' }
      }
    },
    keeps: (token) => DISCORD_TOKEN.test(token),
    connect: (token, _bot, events) => discordTransport(token, options, events),
    // The commands the bot offered go with it.
    forget: async (token, bot) => { await rest(token, 'PUT', `/applications/${bot.extra?.application ?? String(bot.id)}/commands`, []) },
    words: {
      address: (command) => `/${command}`,
      own: (command) => `/${command}`,
      whoLine: (command, name, role) => `/${command} · ${discordEscape(name)}, ${discordEscape(role)}`,
      carryOn: 'Reply to an answer to carry on that conversation.',
      replyToCard: { orOwn: 'Or reply to this message with an answer of your own.', only: 'Reply to this message with your answer.' },
      wrongCode: 'That code does not pair this account. Make a new one in Locust: Settings > General > Chat apps.',
      helpMore: 'Type / here to pick a teammate from Discord’s list.',
      noToken: 'Paste your bot’s token first.',
      unreadable: 'The bot’s token could not be read on this computer. Paste it again.'
    }
  }
}

export function createDiscordLink(options: DiscordOptions): ChatLink {
  return createChatLink(discordApp(options), options)
}
