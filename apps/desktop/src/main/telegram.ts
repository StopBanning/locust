import { randomBytes } from 'node:crypto'

import { answerMessages, parsedCommand, plainOf, TELEGRAM_TOKEN, telegramEscape } from '../shared/telegram.js'
import {
  chatAnswer, createChatLink, type ChatApp, type ChatEvents, type ChatIncoming, type ChatLink, type ChatLinkOptions, type ChatMission,
  type ChatPress, type ChatStart, type ChatTeammate, type ChatTransport
} from './chat-link.js'

/*
 * YOUR TEAMMATES IN TELEGRAM (0.831, shared/telegram.ts). The person's own bot, their own chat, and Locust in between:
 * it asks Telegram for the bot's new messages one long request at a time (getUpdates), so nothing on this computer
 * listens, and it answers through the same bot. Only the paired chat is heard; every other one is ignored (chat-link.ts,
 * which Discord and Slack share since 0.832).
 */
export const TELEGRAM_FILE = 'telegram.json'
export const TELEGRAM_API = 'https://api.telegram.org'

export type TelegramTeammate = ChatTeammate
export type TelegramMission = ChatMission
export type TelegramStart = ChatStart
export type TelegramLink = ChatLink

export interface TelegramOptions extends ChatLinkOptions {
  readonly fetcher?: typeof fetch
  /** Telegram's address; a drive names a stand-in on this computer. */
  readonly api?: () => string
  /** How long one request for new messages waits at Telegram, in seconds. */
  readonly pollSeconds?: number
}

interface TelegramUser { readonly id: number; readonly is_bot?: boolean; readonly first_name?: string; readonly last_name?: string; readonly username?: string }
interface TelegramMessage {
  readonly message_id: number
  readonly from?: TelegramUser
  readonly chat: { readonly id: number; readonly type: string }
  readonly date: number
  readonly text?: string
  readonly reply_to_message?: { readonly message_id: number }
}
interface TelegramUpdate {
  readonly update_id: number
  readonly message?: TelegramMessage
  readonly callback_query?: { readonly id: string; readonly from: TelegramUser; readonly message?: { readonly message_id: number; readonly chat: { readonly id: number } }; readonly data?: string }
}

/** What Telegram said instead of an answer. The address is never in it: the token is part of the address. */
export class TelegramError extends Error {
  constructor(readonly code: number, readonly description: string, readonly retryAfter?: number) { super(description) }
}

const nameOf = (user: TelegramUser | undefined): string => [user?.first_name, user?.last_name].filter((part) => part !== undefined && part.length > 0).join(' ') || 'Your chat'

/** The answer a turn gave, as the chat is told it (chat-link.ts). */
export const telegramAnswer = chatAnswer

/** One Bot API method. Nothing that holds the address -- and so the token -- is ever said or written. */
function telegramCall(options: TelegramOptions) {
  const fetcher = options.fetcher ?? fetch
  return async <T>(token: string, method: string, body: Record<string, unknown>, signal?: AbortSignal): Promise<T> => {
    let response: Response
    try {
      response = await fetcher(`${(options.api?.() ?? TELEGRAM_API).replace(/\/+$/, '')}/bot${token}/${method}`, {
        method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body), redirect: 'error', ...(signal === undefined ? {} : { signal })
      })
    } catch (error) {
      if (signal?.aborted === true) throw error
      throw new TelegramError(0, 'Telegram could not be reached.')
    }
    const data = await response.json().catch(() => undefined) as { ok?: boolean; result?: T; error_code?: number; description?: string; parameters?: { retry_after?: number } } | undefined
    if (data?.ok === true) return data.result as T
    throw new TelegramError(data?.error_code ?? response.status, typeof data?.description === 'string' ? data.description.slice(0, 300) : `Telegram answered ${String(response.status)}.`, data?.parameters?.retry_after)
  }
}

const incomingOf = (message: TelegramMessage): ChatIncoming => ({
  from: message.from === undefined ? '' : String(message.from.id),
  bot: message.from?.is_bot === true,
  place: String(message.chat.id),
  direct: message.chat.type === 'private',
  id: String(message.message_id),
  ...(message.text === undefined ? {} : { text: message.text }),
  sentAt: message.date * 1000,
  ...(message.reply_to_message === undefined ? {} : { replyTo: String(message.reply_to_message.message_id) }),
  answerTo: String(message.message_id),
  name: nameOf(message.from),
  ...(message.from?.username === undefined ? {} : { username: message.from.username })
})

function telegramTransport(token: string, options: TelegramOptions, events: ChatEvents): ChatTransport {
  const call = telegramCall(options)
  const pollSeconds = options.pollSeconds ?? 25
  const stop = new AbortController()

  const press = (query: NonNullable<TelegramUpdate['callback_query']>): ChatPress => ({
    from: String(query.from.id),
    place: query.message === undefined ? '' : String(query.message.chat.id),
    message: query.message === undefined ? '' : String(query.message.message_id),
    data: query.data ?? '',
    answer: async (text) => { await call(token, 'answerCallbackQuery', { callback_query_id: query.id, ...(text === undefined ? {} : { text }) }).catch(() => undefined) }
  })

  const loop = (async () => {
    let backoff = 2_000
    let first = true
    while (!stop.signal.aborted) {
      const timeout = new AbortController()
      const timer = setTimeout(() => timeout.abort(), (pollSeconds + 20) * 1000)
      const both = new AbortController()
      const abortBoth = (): void => both.abort()
      stop.signal.addEventListener('abort', abortBoth)
      timeout.signal.addEventListener('abort', abortBoth)
      try {
        // The first asks without waiting, so Settings says it is connected at once; the rest wait at Telegram.
        const offset = events.offset()
        const updates = await call<TelegramUpdate[]>(token, 'getUpdates', { timeout: first ? 0 : pollSeconds, allowed_updates: ['message', 'callback_query'], ...(offset === undefined ? {} : { offset }) }, both.signal)
        first = false
        events.connected()
        backoff = 2_000
        if (updates.length > 0) {
          // Kept before any is acted on: a message Locust crashed on is not started twice when it opens again.
          await events.setOffset(Math.max(...updates.map((one) => one.update_id)) + 1)
          for (const one of updates) {
            if (stop.signal.aborted) return
            if (one.message !== undefined) await events.message(incomingOf(one.message))
            else if (one.callback_query !== undefined) await events.press(press(one.callback_query))
          }
        }
      } catch (error) {
        if (stop.signal.aborted) return
        const code = error instanceof TelegramError ? error.code : 0
        if (code === 401 || code === 404) {
          events.trouble('Telegram no longer takes this bot’s token. Paste a new one from @BotFather.', true)
          return
        }
        events.trouble(code === 409
          ? 'Another program is reading this bot’s messages. Give Locust a bot of its own, made with @BotFather.'
          : 'Telegram could not be reached. Locust keeps trying.')
        await new Promise((settle) => { const wait = setTimeout(settle, code === 409 ? 30_000 : backoff); stop.signal.addEventListener('abort', () => { clearTimeout(wait); settle(undefined) }) })
        backoff = Math.min(backoff * 2, 60_000)
      } finally {
        clearTimeout(timer)
        stop.signal.removeEventListener('abort', abortBoth)
      }
    }
  })()

  return {
    async stop() {
      stop.abort()
      await loop.catch(() => undefined)
    },
    /** A message to the chat, in Telegram's HTML; as plain text if Telegram refuses the markup. */
    async send(place, html, extra) {
      const body = {
        chat_id: Number(place), link_preview_options: { is_disabled: true },
        ...(extra.answerTo === undefined || extra.continued === true ? {} : { reply_parameters: { message_id: Number(extra.answerTo), allow_sending_without_reply: true } }),
        ...(extra.buttons === undefined || extra.buttons.length === 0 ? {} : { reply_markup: { inline_keyboard: extra.buttons.map((row) => row.map((button) => ({ text: button.label, callback_data: button.data }))) } })
      }
      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          return String((await call<{ message_id: number }>(token, 'sendMessage', { ...body, text: html, parse_mode: 'HTML' })).message_id)
        } catch (error) {
          if (!(error instanceof TelegramError)) return undefined
          if (error.code === 400 && /entit|tag|parse/i.test(error.description)) {
            try { return String((await call<{ message_id: number }>(token, 'sendMessage', { ...body, text: plainOf(html) })).message_id) } catch { return undefined }
          }
          if (error.code === 429 && attempt === 0) { await new Promise((settle) => setTimeout(settle, Math.min(30, error.retryAfter ?? 3) * 1000)); continue }
          if (error.code === 403) events.problem('The chat blocked the bot, so Locust’s messages do not reach it. Unblock it in Telegram.')
          events.note(`a message was not sent: ${error.description}`)
          return undefined
        }
      }
      return undefined
    },
    /** A message of the bot's, said again; its buttons go, since an edit names the ones it keeps and this keeps none. */
    async edit(place, messageId, html) {
      const body = { chat_id: Number(place), message_id: Number(messageId), link_preview_options: { is_disabled: true }, reply_markup: { inline_keyboard: [] } }
      try { await call(token, 'editMessageText', { ...body, text: html, parse_mode: 'HTML' }) } catch (error) {
        if (error instanceof TelegramError && error.code === 400 && /entit|tag|parse/i.test(error.description)) await call(token, 'editMessageText', { ...body, text: plainOf(html) }).catch(() => undefined)
      }
    },
    typing: async (place) => { await call(token, 'sendChatAction', { chat_id: Number(place), action: 'typing' }).catch(() => undefined) },
    typingMs: 4_500,
    commands: async (list) => { await call(token, 'setMyCommands', { commands: list.map(({ command, description }) => ({ command, description })) }) }
  }
}

/** Telegram as a chat link needs it: its HTML, its /commands, and its pairing link. */
export function telegramApp(options: TelegramOptions): ChatApp {
  const call = telegramCall(options)
  return {
    id: 'telegram',
    file: TELEGRAM_FILE,
    escape: telegramEscape,
    bold: (text) => `<b>${telegramEscape(text)}</b>`,
    italic: (text) => `<i>${telegramEscape(text)}</i>`,
    answerParts: answerMessages,
    parse: (text, bot) => parsedCommand(text, bot.username),
    // Pairing: the link's code, which Telegram sends as "/start <code>" when the person presses Start.
    codeIn: (text, bot) => {
      const command = parsedCommand(text, bot.username)
      return command?.command === 'start' && command.rest.length > 0 ? command.rest : undefined
    },
    freshCode: () => randomBytes(12).toString('base64url'),
    pairing: (bot, code, until) => ({ link: { url: `https://t.me/${bot.username}?start=${code}`, until } }),
    shown: (bot) => ({ username: bot.username, name: bot.name }),
    shape: (given) => {
      const candidate = typeof given === 'string' ? given.trim() : ''
      return TELEGRAM_TOKEN.test(candidate) ? { ok: true, secret: candidate } : { ok: false, message: 'That is not a bot’s token. @BotFather gives one like 123456789:AAH…, on a line of its own: copy all of it.' }
    },
    verify: async (token) => {
      try {
        const bot = await call<{ id: number; username: string; first_name: string }>(token, 'getMe', {})
        const hook = await call<{ url?: string }>(token, 'getWebhookInfo', {})
        if (typeof hook.url === 'string' && hook.url.length > 0) return { ok: false, message: 'This bot sends its messages to another program already. Make a new bot for Locust with @BotFather.' }
        return { ok: true, bot: { id: bot.id, username: bot.username, name: bot.first_name } }
      } catch (error) {
        const code = error instanceof TelegramError ? error.code : 0
        return { ok: false, message: code === 401 || code === 404 ? 'Telegram does not know that token. Copy it again from @BotFather.' : 'Telegram could not be reached. Check the connection, then try again.' }
      }
    },
    keeps: (token) => TELEGRAM_TOKEN.test(token),
    connect: (token, _bot, events) => telegramTransport(token, options, events),
    words: {
      address: (command) => `/${command}`,
      own: (command) => `/${command}`,
      whoLine: (command, name, role) => `/${command} ${telegramEscape(name)}, ${telegramEscape(role)}`,
      carryOn: 'Reply to an answer to carry on that conversation.',
      replyToCard: { orOwn: 'Or reply to this message with an answer of your own.', only: 'Reply to this message with your answer.' },
      wrongCode: 'That link does not pair this chat any more. Make a new one in Locust: Settings > General > Chat apps.',
      noToken: 'Paste a bot’s token first.',
      unreadable: 'The bot’s token could not be read on this computer. Paste it again.'
    }
  }
}

export function createTelegramLink(options: TelegramOptions): TelegramLink {
  return createChatLink(telegramApp(options), options)
}
