import { randomBytes } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import { joinMessageFragments } from '../shared/messageFragments.js'
import { unwrapProtocolTags } from '../shared/protocolTags.js'
import { scrubSecrets } from '../shared/secrets.js'
import {
  answerMessages, parsedCommand, plainOf, teammateCommands, TELEGRAM_LINK_MS, TELEGRAM_STALE_MS, TELEGRAM_TOKEN,
  telegramEscape, type TelegramSettings
} from '../shared/telegram.js'
import { privateMcpFile } from './locust-mcp-host.js'
import type { SecretBox } from './own-models.js'
import { replaceFile } from './replace-file.js'

/*
 * YOUR TEAMMATES IN TELEGRAM (0.831, shared/telegram.ts). The person's own bot, their own chat, and Locust in between:
 * it asks Telegram for the bot's new messages one long request at a time (getUpdates), so nothing on this computer
 * listens, and it answers through the same bot. Only the paired chat is heard; every other one is ignored.
 */
export const TELEGRAM_FILE = 'telegram.json'
export const TELEGRAM_API = 'https://api.telegram.org'
/** The longest answer Locust reads for the chat; Telegram's own limits cut it further (shared/telegram.ts). */
const MAX_ANSWER = 15_000
const MAX_ANSWERS_KEPT = 200
const MAX_CARDS_KEPT = 50
const MAX_TURNS_KEPT = 20
/** A turn answered here, whose carry-on or check-fix still counts as from here, for this long. */
const SUCCESSOR_MS = 30 * 60_000
const HELD_MS = 10 * 60_000

export interface TelegramTeammate { readonly teammateId: string; readonly name: string; readonly role: string }

/** What Telegram needs of a turn in the record: how it ended and what it said. */
export interface TelegramMission {
  readonly missionId: string
  readonly phase: string
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly startedBy?: { readonly kind: string }
  readonly continuesFrom?: { readonly missionId: string }
  readonly hostFailureMessage?: string
}

export type TelegramStart = { readonly ok: true; readonly missionId: string } | { readonly ok: false; readonly message: string }

export interface TelegramOptions {
  readonly directory: string
  readonly secrets: SecretBox
  /** The organization's word, read at every change and every message. */
  readonly refusal: () => string | undefined
  /** This computer's name, said when a chat pairs. */
  readonly computer: () => string
  readonly team: () => Promise<readonly TelegramTeammate[]>
  /** The person's next message to a teammate: a new conversation, or the next turn of the one it continues. */
  readonly start: (input: { readonly teammateId: string; readonly text: string; readonly followUpOf?: string }) => Promise<TelegramStart>
  readonly read: (missionId: string) => Promise<TelegramMission | undefined>
  readonly live: (missionId: string) => boolean
  /** A turn that ran out of its agent's plan and was carried on with another (carry-on.ts). */
  readonly carried?: (missionId: string) => boolean
  readonly waiting: () => readonly { readonly request: MissionApprovalRequest; readonly teammateId?: string }[]
  /** The window's own path for a card's answer: Allow once, Deny, or a question's answer -- never Always. */
  readonly decide: (answer: MissionApprovalAnswer) => Promise<boolean>
  readonly changed?: (settings: TelegramSettings) => void
  readonly note?: (label: string, detail: string) => void
  readonly fetcher?: typeof fetch
  /** Telegram's address; a drive names a stand-in on this computer. */
  readonly api?: () => string
  readonly now?: () => number
  readonly protectFile?: (path: string) => Promise<void>
  /** How long one request for new messages waits at Telegram, in seconds. */
  readonly pollSeconds?: number
  readonly typingMs?: number
}

export interface TelegramLink {
  load(): Promise<void>
  settings(): Promise<TelegramSettings>
  setToken(token: unknown): Promise<TelegramSettings>
  newLink(): Promise<TelegramSettings>
  remove(): Promise<TelegramSettings>
  /** A card the window shows: sent to the chat too when its turn came from there. */
  cardRaised(request: MissionApprovalRequest, name: string | undefined): void
  /** A card answered anywhere, or taken down: its message in the chat says so, and its buttons go. */
  cardSettled(approvalId: string, how: 'allowed' | 'denied' | 'answered' | 'gone'): void
  onRunEnded(mission: { readonly missionId: string }): Promise<void>
  dispose(): Promise<void>
}

interface SavedChat { readonly id: number; readonly userId: number; readonly name: string; readonly username?: string; readonly pairedAt: string }
interface SavedTurn { readonly missionId: string; readonly teammateId: string; readonly replyTo?: number; readonly at: number; readonly carried?: true }
interface SavedCard {
  readonly key: string
  readonly approvalId: string
  readonly missionId: string
  readonly messageId: number
  readonly html: string
  /** A question answered here: its id, its options' labels, and whether a reply in words answers it. */
  readonly question?: { readonly id: string; readonly options: readonly string[]; readonly words: boolean }
}
interface Saved {
  readonly schemaVersion: 1
  /** The token as the operating system encrypted it, base64. */
  readonly token: string
  readonly bot: { readonly id: number; readonly username: string; readonly name: string }
  readonly chat?: SavedChat
  readonly offset?: number
  /** Who a message with no command goes to. */
  readonly current?: string
  /** Teammates whose next message starts a new conversation (/new). */
  readonly fresh?: readonly string[]
  /** Each teammate's conversation from the chat, by the turn last sent there. */
  readonly conversations?: Readonly<Record<string, string>>
  /** The chat's messages that carry an answer, by the turn they answer and whose: a reply to one carries that conversation on. */
  readonly answers?: readonly { readonly messageId: number; readonly missionId: string; readonly teammateId?: string }[]
  readonly turns?: readonly SavedTurn[]
  readonly cards?: readonly SavedCard[]
  /** Turns answered here lately, whose carry-on or check-fix is answered here too. */
  readonly answered?: readonly { readonly missionId: string; readonly at: number }[]
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

/** The answer a turn gave, as the chat is told it: its last whole message, the reason it failed, or that it stopped. */
export function telegramAnswer(mission: TelegramMission, name: string): string {
  if (mission.phase === 'failed') {
    const failed = mission.events.findLast((event) => event.type === 'run.failed')
    const why = mission.hostFailureMessage ?? (failed?.type === 'run.failed' ? failed.payload.message : undefined)
    return `${name}’s turn failed${why === undefined ? '.' : `: ${why}`}`
  }
  if (mission.phase !== 'completed') return `${name}’s turn stopped before it finished. What it did is in Locust.`
  const final = joinMessageFragments(mission.events).findLast((event) => event.type === 'message.delta' && event.payload.final)
  const text = final?.type === 'message.delta' ? unwrapProtocolTags(final.payload.text).trim() : ''
  if (text.length === 0) return `${name} finished without a written answer. What it did is in Locust.`
  return text.length > MAX_ANSWER ? `${text.slice(0, MAX_ANSWER)}…` : text
}

export function createTelegramLink(options: TelegramOptions): TelegramLink {
  const file = join(options.directory, TELEGRAM_FILE)
  const fetcher = options.fetcher ?? fetch
  const now = options.now ?? Date.now
  const pollSeconds = options.pollSeconds ?? 25
  let saved: Saved | undefined
  let token: string | undefined
  let link: { readonly code: string; readonly until: number } | undefined
  let connected = false
  let message: string | undefined
  let polling: AbortController | undefined
  let loop: Promise<void> | undefined
  let typing: ReturnType<typeof setInterval> | undefined
  let commandsSet = ''
  let commandsAt = 0
  let held: { readonly text: string; readonly messageId: number; readonly at: number; readonly choices: readonly string[] } | undefined
  const answeringHere = new Set<string>()
  const wrongLinkAt = new Map<number, number>()
  let writing: Promise<void> = Promise.resolve()

  const note = (detail: string): void => { options.note?.('telegram', detail) }

  const settingsNow = (): TelegramSettings => ({
    ...(saved === undefined ? {} : { bot: { username: saved.bot.username, name: saved.bot.name } }),
    ...(saved?.chat === undefined ? {} : { chat: { name: saved.chat.name, ...(saved.chat.username === undefined ? {} : { username: saved.chat.username }), pairedAt: saved.chat.pairedAt } }),
    ...(saved === undefined || link === undefined || link.until <= now() ? {} : { link: { url: `https://t.me/${saved.bot.username}?start=${link.code}`, until: new Date(link.until).toISOString() } }),
    connected: saved !== undefined && connected,
    ...(message === undefined ? {} : { message })
  })
  const changed = (): void => { options.changed?.(settingsNow()) }

  const writeNow = async (): Promise<void> => {
    const held = saved
    if (held === undefined) { await rm(file, { force: true }); return }
    await mkdir(options.directory, { recursive: true })
    const temp = `${file}.tmp`
    await rm(temp, { force: true })
    await writeFile(temp, '', { mode: 0o600, flag: 'wx' })
    // The chat and the encrypted token are written only once the file is the account's alone.
    await (options.protectFile ?? privateMcpFile)(temp)
    await writeFile(temp, JSON.stringify(held), { mode: 0o600 })
    await replaceFile(temp, file)
  }
  const persist = (): Promise<void> => {
    const next = writing.then(writeNow)
    writing = next.catch(() => undefined)
    return next
  }
  const update = (change: (was: Saved) => Saved): void => { if (saved !== undefined) saved = change(saved) }

  /** One Bot API method. Nothing that holds the address -- and so the token -- is ever said or written. */
  const call = async <T>(method: string, body: Record<string, unknown>, signal?: AbortSignal, withToken: string | undefined = token): Promise<T> => {
    if (withToken === undefined) throw new TelegramError(401, 'No token is kept.')
    let response: Response
    try {
      response = await fetcher(`${(options.api?.() ?? TELEGRAM_API).replace(/\/+$/, '')}/bot${withToken}/${method}`, {
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

  /** A message to the paired chat, in Telegram's HTML; as plain text if Telegram refuses the markup. */
  const send = async (html: string, extra: Record<string, unknown> = {}): Promise<number | undefined> => {
    const chat = saved?.chat
    if (chat === undefined) return undefined
    const body = { chat_id: chat.id, link_preview_options: { is_disabled: true }, ...extra }
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        return (await call<{ message_id: number }>('sendMessage', { ...body, text: html, parse_mode: 'HTML' })).message_id
      } catch (error) {
        if (!(error instanceof TelegramError)) return undefined
        if (error.code === 400 && /entit|tag|parse/i.test(error.description)) {
          try { return (await call<{ message_id: number }>('sendMessage', { ...body, text: plainOf(html) })).message_id } catch { return undefined }
        }
        if (error.code === 429 && attempt === 0) { await new Promise((settle) => setTimeout(settle, Math.min(30, error.retryAfter ?? 3) * 1000)); continue }
        if (error.code === 403) { message = 'The chat blocked the bot, so Locust’s messages do not reach it. Unblock it in Telegram.'; changed() }
        note(`a message was not sent: ${error.description}`)
        return undefined
      }
    }
    return undefined
  }
  /** A message of the bot's, said again; its buttons go, since an edit names the ones it keeps and this keeps none. */
  const edit = async (messageId: number, html: string): Promise<void> => {
    const chat = saved?.chat
    if (chat === undefined) return
    const body = { chat_id: chat.id, message_id: messageId, link_preview_options: { is_disabled: true }, reply_markup: { inline_keyboard: [] } }
    try { await call('editMessageText', { ...body, text: html, parse_mode: 'HTML' }) } catch (error) {
      if (error instanceof TelegramError && error.code === 400 && /entit|tag|parse/i.test(error.description)) await call('editMessageText', { ...body, text: plainOf(html) }).catch(() => undefined)
    }
  }

  const ours = (team: readonly TelegramTeammate[]): ReadonlyMap<string, string> => teammateCommands(team)
  const teammateNamed = (team: readonly TelegramTeammate[], id: string | undefined): TelegramTeammate | undefined => team.find((mate) => mate.teammateId === id)

  /** The bot's menu of commands: each teammate, then Locust's own. Set again only when the team changed. */
  const refreshCommands = async (force = false): Promise<void> => {
    if (saved?.chat === undefined) return
    if (!force && now() - commandsAt < 60_000) return
    commandsAt = now()
    const team = await options.team().catch(() => [])
    const commands = ours(team)
    const list = [
      ...team.slice(0, 90).map((mate) => ({ command: commands.get(mate.teammateId)!, description: `Write to ${mate.name} (${mate.role})`.slice(0, 256) })),
      { command: 'who', description: 'Your teammates, and who you are writing to' },
      { command: 'new', description: 'Start a new conversation with them' },
      { command: 'waiting', description: 'The cards waiting on you' },
      { command: 'help', description: 'What you can do here' }
    ]
    const said = JSON.stringify(list)
    if (said === commandsSet) return
    try { await call('setMyCommands', { commands: list }); commandsSet = said } catch { /* The menu is a help, not the way in: commands work without it. */ }
  }

  const helpText = (team: readonly TelegramTeammate[]): string => {
    const commands = ours(team)
    const first = team[0]
    const example = first === undefined ? '/name' : `/${commands.get(first.teammateId)!}`
    return [
      `<b>Locust on ${telegramEscape(options.computer())}</b>`,
      `${example} and a message writes to ${first === undefined ? 'that teammate' : telegramEscape(first.name)}. After that, just write: it goes to the same teammate until you pick another.`,
      'Reply to an answer to carry on that conversation.',
      '/new starts a new conversation with whoever you are writing to.',
      '/who lists your teammates. /waiting shows the cards waiting on you.',
      'Locust answers while it is open on your computer. Approval cards here offer Allow once and Deny; Always stays in Locust.'
    ].join('\n')
  }

  const pairedFrom = (from: TelegramUser | undefined, chatId: number): boolean => saved?.chat !== undefined && saved.chat.id === chatId && from?.id === saved.chat.userId

  const pair = async (incoming: TelegramMessage): Promise<void> => {
    const chat: SavedChat = {
      id: incoming.chat.id, userId: incoming.from!.id, name: nameOf(incoming.from),
      ...(incoming.from?.username === undefined ? {} : { username: incoming.from.username }), pairedAt: new Date(now()).toISOString()
    }
    // A new chat starts clean: what the last one was writing, and to whom, was that chat's.
    update((was) => ({ schemaVersion: 1, token: was.token, bot: was.bot, chat, ...(was.offset === undefined ? {} : { offset: was.offset }) }))
    link = undefined
    message = undefined
    held = undefined
    await persist().catch(() => undefined)
    changed()
    const team = await options.team().catch(() => [])
    await send(`Paired with Locust on <b>${telegramEscape(options.computer())}</b>.\n\n${helpText(team)}`)
    await refreshCommands(true)
  }

  /** Says once a minute at most, to a chat, that its pairing link did not work. */
  const badLink = async (chatId: number): Promise<void> => {
    const last = wrongLinkAt.get(chatId)
    if (last !== undefined && now() - last < 60_000) return
    wrongLinkAt.set(chatId, now())
    try {
      await call('sendMessage', { chat_id: chatId, text: 'That link does not pair this chat any more. Make a new one in Locust: Settings > General > Telegram.' })
    } catch { /* A stranger's chat is owed nothing more. */ }
  }

  const typingOn = (): void => {
    if (typing !== undefined) return
    const beat = async (): Promise<void> => {
      // A turn that never said it ended (Locust was killed) is let go after half a day.
      if ((saved?.turns ?? []).some((turn) => now() - turn.at > 12 * 60 * 60_000 && !options.live(turn.missionId))) {
        update((was) => ({ ...was, turns: (was.turns ?? []).filter((turn) => now() - turn.at <= 12 * 60 * 60_000 || options.live(turn.missionId)) }))
      }
      const turns = saved?.turns ?? []
      const waitingOn = new Set(options.waiting().map((entry) => entry.request.missionId))
      const working = turns.filter((turn) => options.live(turn.missionId) && !waitingOn.has(turn.missionId))
      if (turns.length === 0) { clearInterval(typing); typing = undefined; return }
      if (working.length > 0 && saved?.chat !== undefined) await call('sendChatAction', { chat_id: saved.chat.id, action: 'typing' }).catch(() => undefined)
    }
    typing = setInterval(() => { void beat() }, options.typingMs ?? 4_500)
    void beat()
  }

  /** The person's message to a teammate: the next turn of their conversation from here, or a new one. */
  const sendTo = async (teammate: TelegramTeammate, text: string, replyTo: number, continues?: string): Promise<void> => {
    const fresh = saved?.fresh?.includes(teammate.teammateId) === true
    const kept = fresh ? undefined : saved?.conversations?.[teammate.teammateId]
    // A conversation deleted in Locust since is not carried on: the message starts a new one.
    const keptStands = kept !== undefined && (options.live(kept) || (await options.read(kept).catch(() => undefined)) !== undefined)
    const followUpOf = continues ?? (keptStands ? kept : undefined)
    const started = await options.start({ teammateId: teammate.teammateId, text, ...(followUpOf === undefined ? {} : { followUpOf }) })
    if (!started.ok) {
      await send(telegramEscape(started.message), { reply_parameters: { message_id: replyTo, allow_sending_without_reply: true } })
      return
    }
    update((was) => ({
      ...was,
      current: teammate.teammateId,
      fresh: (was.fresh ?? []).filter((id) => id !== teammate.teammateId),
      conversations: { ...(was.conversations ?? {}), [teammate.teammateId]: started.missionId },
      turns: [...(was.turns ?? []), { missionId: started.missionId, teammateId: teammate.teammateId, replyTo, at: now() }].slice(-MAX_TURNS_KEPT)
    }))
    await persist().catch(() => undefined)
    typingOn()
  }

  const cardKey = (): string => randomBytes(6).toString('base64url')

  /** A card in the chat: words, and the buttons that answer it here when it can be answered here. */
  const postCard = async (request: MissionApprovalRequest, name: string | undefined): Promise<void> => {
    if (saved?.chat === undefined || saved.cards?.some((card) => card.approvalId === request.approvalId) === true) return
    const who = telegramEscape(name ?? 'A teammate')
    const key = cardKey()
    let html: string
    let keyboard: { text: string; callback_data: string }[][] = []
    let question: SavedCard['question']
    if (request.kind === 'question') {
      const asked = request.questions ?? []
      const only = asked.length === 1 ? asked[0] : undefined
      const words = asked.map((one) => telegramEscape(scrubSecrets([one.header, one.question].filter((part) => part !== null && part.length > 0).join(': ')).text)).join('\n')
      html = `<b>${who} is asking you</b>\n${words.length > 0 ? words : telegramEscape(scrubSecrets(request.summary).text)}`
      if (request.answerIn !== undefined || only === undefined || only.isSecret) {
        html += `\n\n<i>Answer it in ${telegramEscape(request.answerIn ?? 'Locust on your computer')}.</i>`
      } else {
        const options_ = only.options.slice(0, 8).map((option) => option.label)
        const free = only.options.length === 0 || only.isOther
        keyboard = options_.map((label, index) => [{ text: label.slice(0, 60), callback_data: `o:${key}:${String(index)}` }])
        if (free) html += `\n\n<i>${options_.length > 0 ? 'Or reply to this message with an answer of your own.' : 'Reply to this message with your answer.'}</i>`
        question = { id: only.id, options: options_, words: free }
      }
    } else {
      html = `<b>${who} needs your approval</b>\n${telegramEscape(scrubSecrets(request.summary).text)}\n\n<i>The whole of it is in Locust on your computer.</i>`
      keyboard = [[{ text: 'Allow once', callback_data: `y:${key}` }, { text: 'Deny', callback_data: `n:${key}` }]]
    }
    const messageId = await send(html, keyboard.length === 0 ? {} : { reply_markup: { inline_keyboard: keyboard } })
    if (messageId === undefined) return
    update((was) => ({ ...was, cards: [...(was.cards ?? []), { key, approvalId: request.approvalId, missionId: request.missionId, messageId, html, ...(question === undefined ? {} : { question }) }].slice(-MAX_CARDS_KEPT) }))
    await persist().catch(() => undefined)
  }

  /** The card's message says what became of it, and loses its buttons. Once: a card settled is forgotten here. */
  const settle = async (approvalId: string, words: string): Promise<void> => {
    const card = saved?.cards?.find((one) => one.approvalId === approvalId)
    if (card === undefined) return
    update((was) => ({ ...was, cards: (was.cards ?? []).filter((one) => one.approvalId !== approvalId) }))
    await persist().catch(() => undefined)
    await edit(card.messageId, `${card.html}\n\n<b>${telegramEscape(words)}</b>`)
  }

  /** A turn of the chat's, or one Locust carried on from a turn of the chat's. */
  const fromHere = async (missionId: string): Promise<{ readonly turn?: SavedTurn; readonly successorOf?: string } | undefined> => {
    const turn = saved?.turns?.find((one) => one.missionId === missionId)
    if (turn !== undefined) return { turn }
    const carried = (saved?.turns ?? []).filter((one) => one.carried === true).map((one) => one.missionId)
    const recent = (saved?.answered ?? []).filter((one) => now() - one.at < SUCCESSOR_MS).map((one) => one.missionId)
    if (carried.length === 0 && recent.length === 0) return undefined
    const mission = await options.read(missionId).catch(() => undefined)
    const before = mission?.continuesFrom?.missionId
    if (before === undefined || (mission?.startedBy?.kind !== 'carry-on' && mission?.startedBy?.kind !== 'check-fix')) return undefined
    return carried.includes(before) || recent.includes(before) ? { successorOf: before } : undefined
  }

  const answerTurn = async (missionId: string, teammateId: string | undefined, replyTo: number | undefined): Promise<void> => {
    const mission = await options.read(missionId).catch(() => undefined)
    const team = await options.team().catch(() => [])
    const name = teammateNamed(team, teammateId)?.name ?? 'Your teammate'
    const said = mission === undefined ? `${name}’s answer could not be read. It is in Locust.` : telegramAnswer(mission, name)
    // Nothing shaped like a key goes to Telegram's servers, whoever wrote it (shared/secrets.ts).
    const parts = answerMessages(name, scrubSecrets(said).text)
    let first = true
    const sent: { messageId: number; missionId: string; teammateId?: string }[] = []
    for (const part of parts) {
      const messageId = await send(part, first && replyTo !== undefined ? { reply_parameters: { message_id: replyTo, allow_sending_without_reply: true } } : {})
      first = false
      if (messageId !== undefined) sent.push({ messageId, missionId, ...(teammateId === undefined ? {} : { teammateId }) })
    }
    update((was) => ({
      ...was,
      answers: [...(was.answers ?? []), ...sent].slice(-MAX_ANSWERS_KEPT),
      answered: [...(was.answered ?? []).filter((one) => now() - one.at < SUCCESSOR_MS), { missionId, at: now() }].slice(-MAX_TURNS_KEPT),
      ...(teammateId === undefined ? {} : { conversations: { ...(was.conversations ?? {}), [teammateId]: missionId } })
    }))
    await persist().catch(() => undefined)
  }

  const runEnded = async (missionId: string): Promise<void> => {
    if (saved?.chat === undefined) return
    // A card of any turn that ended unanswered loses its buttons, whoever started the turn.
    for (const card of (saved.cards ?? []).filter((one) => one.missionId === missionId)) await settle(card.approvalId, 'The turn ended before this was answered.')
    const here = await fromHere(missionId)
    if (here === undefined) return
    if (here.turn !== undefined && options.carried?.(missionId) === true) {
      // It ran out of its agent's plan and carries on with another: the answer is the next turn's.
      update((was) => ({ ...was, turns: (was.turns ?? []).map((one) => one.missionId === missionId ? { ...one, carried: true as const } : one) }))
      await persist().catch(() => undefined)
      return
    }
    const origin = here.turn ?? saved.turns?.find((one) => one.missionId === here.successorOf)
    const teammateId = origin?.teammateId ?? (here.successorOf === undefined ? undefined : await teammateOfAnswered(here.successorOf))
    update((was) => ({ ...was, turns: (was.turns ?? []).filter((one) => one.missionId !== missionId && one.missionId !== here.successorOf) }))
    await answerTurn(missionId, teammateId, here.turn?.replyTo ?? origin?.replyTo)
  }
  const teammateOfAnswered = async (missionId: string): Promise<string | undefined> =>
    Object.entries(saved?.conversations ?? {}).find(([, id]) => id === missionId)?.[0]

  const handleMessage = async (incoming: TelegramMessage): Promise<void> => {
    if (incoming.chat.type !== 'private' || incoming.from === undefined || incoming.from.is_bot === true || saved === undefined) return
    const text = (incoming.text ?? '').trim()
    const command = parsedCommand(text, saved.bot.username)
    // Pairing: the link's code, once, before it runs out. A new chat that has it takes the place of the old one.
    if (command?.command === 'start' && command.rest.length > 0) {
      if (link !== undefined && link.until > now() && command.rest === link.code) { await pair(incoming); return }
      if (!pairedFrom(incoming.from, incoming.chat.id)) { await badLink(incoming.chat.id); return }
    }
    // Every other chat is ignored: a bot can be found by anyone, and it answers only its person.
    if (!pairedFrom(incoming.from, incoming.chat.id)) return
    const refused = options.refusal()
    if (refused !== undefined) { await send(telegramEscape(refused)); return }
    const team = await options.team().catch(() => [])
    const commands = ours(team)
    const replyTo = { reply_parameters: { message_id: incoming.message_id, allow_sending_without_reply: true } }
    if (incoming.text === undefined) {
      await send('Locust reads text here for now. Pictures and files go in through Locust on your computer.', replyTo)
      return
    }
    if (command !== undefined && ['start', 'help'].includes(command.command)) { await send(helpText(team)); return }
    if (command?.command === 'who') {
      const current = teammateNamed(team, saved.current)
      const lines = team.map((mate) => `/${commands.get(mate.teammateId)!} ${telegramEscape(mate.name)}, ${telegramEscape(mate.role)}`)
      await send(team.length === 0 ? 'You have no teammates yet. Make one in Locust.' : `<b>Your teammates</b>\n${lines.join('\n')}${current === undefined ? '' : `\n\nWriting to ${telegramEscape(current.name)}.`}`)
      return
    }
    if (command?.command === 'waiting') {
      const waiting = options.waiting()
      if (waiting.length === 0) { await send('Nothing is waiting on you.'); return }
      for (const entry of waiting.slice(0, 5)) await postCard(entry.request, teammateNamed(team, entry.teammateId)?.name)
      if (waiting.length > 5) await send(`${String(waiting.length - 5)} more are waiting in Locust.`)
      return
    }
    const stale = now() - incoming.date * 1000 > TELEGRAM_STALE_MS
    const staleSaid = (): Promise<number | undefined> => send(`This came while Locust was closed (sent ${new Date(incoming.date * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}), so nothing was started. Send it again to start it now.`, replyTo)
    if (command?.command === 'new') {
      const current = teammateNamed(team, saved.current)
      if (current === undefined) { await send('Pick who to write to first: /who lists your teammates.'); return }
      update((was) => ({ ...was, fresh: [...new Set([...(was.fresh ?? []), current.teammateId])] }))
      await persist().catch(() => undefined)
      if (command.rest.length === 0) { await send(`Your next message starts a new conversation with ${telegramEscape(current.name)}.`); return }
      if (stale) { await staleSaid(); return }
      await sendTo(current, command.rest, incoming.message_id)
      return
    }
    if (command !== undefined) {
      const mate = team.find((one) => commands.get(one.teammateId) === command.command)
      if (mate === undefined) { await send(`There is no /${telegramEscape(command.command)}. /who lists your teammates.`, replyTo); return }
      if (command.rest.length === 0) {
        update((was) => ({ ...was, current: mate.teammateId }))
        await persist().catch(() => undefined)
        await send(`Writing to ${telegramEscape(mate.name)}. Send your message.`)
        return
      }
      if (stale) { await staleSaid(); return }
      await sendTo(mate, command.rest, incoming.message_id)
      return
    }
    if (stale) { await staleSaid(); return }
    // A reply to a card's message answers its question; a reply to an answer carries that conversation on.
    const repliedTo = incoming.reply_to_message?.message_id
    if (repliedTo !== undefined) {
      const card = saved.cards?.find((one) => one.messageId === repliedTo)
      if (card?.question?.words === true) {
        answeringHere.add(card.approvalId)
        const answered = await options.decide({ approvalId: card.approvalId, answers: { [card.question.id]: [text] } }).finally(() => answeringHere.delete(card.approvalId))
        if (answered) await settle(card.approvalId, 'Answered.')
        else await settle(card.approvalId, 'This was already answered, or its turn ended.')
        return
      }
      const answered = saved.answers?.findLast((one) => one.messageId === repliedTo)
      const mate = teammateNamed(team, answered?.teammateId)
      if (answered !== undefined && mate !== undefined) { await sendTo(mate, text, incoming.message_id, answered.missionId); return }
    }
    const current = teammateNamed(team, saved.current) ?? (team.length === 1 ? team[0] : undefined)
    if (current !== undefined) { await sendTo(current, text, incoming.message_id); return }
    if (team.length === 0) { await send('You have no teammates yet. Make one in Locust, then write here.'); return }
    // Who it is for, asked once: the message waits for the answer, for a while.
    const choices = team.slice(0, 8)
    held = { text, messageId: incoming.message_id, at: now(), choices: choices.map((mate) => mate.teammateId) }
    await send('Who is this for?', { ...replyTo, reply_markup: { inline_keyboard: choices.map((mate, index) => [{ text: mate.name.slice(0, 60), callback_data: `t:${String(index)}` }]) } })
  }

  const handleButton = async (query: NonNullable<TelegramUpdate['callback_query']>): Promise<void> => {
    const answer = (text?: string): Promise<unknown> => call('answerCallbackQuery', { callback_query_id: query.id, ...(text === undefined ? {} : { text }) }).catch(() => undefined)
    if (saved === undefined || query.message === undefined || !pairedFrom(query.from, query.message.chat.id)) { await answer(); return }
    const refused = options.refusal()
    if (refused !== undefined) { await answer(refused.slice(0, 190)); return }
    const data = query.data ?? ''
    const chosen = /^t:(\d)$/.exec(data)
    if (chosen !== null) {
      const team = await options.team().catch(() => [])
      const mate = teammateNamed(team, held?.choices[Number(chosen[1])])
      const message = held
      held = undefined
      if (mate === undefined || message === undefined || now() - message.at > HELD_MS) {
        await answer('That message waited too long. Send it again.')
        await edit(query.message.message_id, 'Who is this for? <i>It waited too long: send it again.</i>')
        return
      }
      await answer(`To ${mate.name}`)
      await edit(query.message.message_id, `To <b>${telegramEscape(mate.name)}</b>.`)
      await sendTo(mate, message.text, message.messageId)
      return
    }
    const decided = /^([yno]):([A-Za-z0-9_-]{8})(?::(\d))?$/.exec(data)
    const card = decided === null ? undefined : saved.cards?.find((one) => one.key === decided[2])
    if (decided === null || card === undefined) { await answer('This card was already answered, or its turn ended.'); return }
    const reply: MissionApprovalAnswer | undefined = decided[1] === 'o'
      ? card.question?.options[Number(decided[3])] === undefined ? undefined : { approvalId: card.approvalId, answers: { [card.question.id]: [card.question.options[Number(decided[3])]!] } }
      : { approvalId: card.approvalId, decision: decided[1] === 'y' ? 'approve-once' : 'deny' }
    if (reply === undefined) { await answer(); return }
    answeringHere.add(card.approvalId)
    const answered = await options.decide(reply).catch(() => false).finally(() => answeringHere.delete(card.approvalId))
    const words = !answered ? 'This was already answered, or its turn ended.'
      : decided[1] === 'y' ? 'Allowed once.' : decided[1] === 'n' ? 'Denied.' : `Answered: ${card.question!.options[Number(decided[3])]!}`
    await answer(words.slice(0, 190))
    await settle(card.approvalId, words)
  }

  const handle = async (incoming: TelegramUpdate): Promise<void> => {
    try {
      if (incoming.message !== undefined) await handleMessage(incoming.message)
      else if (incoming.callback_query !== undefined) await handleButton(incoming.callback_query)
    } catch (error) {
      note(`a message could not be handled: ${error instanceof Error ? error.message : String(error)}`)
    }
  }

  const stopPolling = async (): Promise<void> => {
    polling?.abort()
    polling = undefined
    await loop?.catch(() => undefined)
    loop = undefined
    connected = false
  }

  const startPolling = (): void => {
    if (polling !== undefined || saved === undefined || token === undefined) return
    const stop = new AbortController()
    polling = stop
    loop = (async () => {
      let backoff = 2_000
      let first = true
      while (!stop.signal.aborted) {
        if (options.refusal() !== undefined) { message = options.refusal(); connected = false; changed(); return }
        const timeout = new AbortController()
        const timer = setTimeout(() => timeout.abort(), (pollSeconds + 20) * 1000)
        const both = new AbortController()
        const abortBoth = (): void => both.abort()
        stop.signal.addEventListener('abort', abortBoth)
        timeout.signal.addEventListener('abort', abortBoth)
        try {
          // The first asks without waiting, so Settings says it is connected at once; the rest wait at Telegram.
          const updates = await call<TelegramUpdate[]>('getUpdates', { timeout: first ? 0 : pollSeconds, allowed_updates: ['message', 'callback_query'], ...(saved?.offset === undefined ? {} : { offset: saved.offset }) }, both.signal)
          first = false
          if (!connected || message !== undefined) { connected = true; message = undefined; changed() }
          backoff = 2_000
          if (updates.length > 0) {
            // Kept before any is acted on: a message Locust crashed on is not started twice when it opens again.
            const next = Math.max(...updates.map((one) => one.update_id)) + 1
            update((was) => ({ ...was, offset: next }))
            await persist().catch(() => undefined)
            for (const one of updates) await handle(one)
          }
          void refreshCommands()
        } catch (error) {
          if (stop.signal.aborted) return
          const code = error instanceof TelegramError ? error.code : 0
          connected = false
          if (code === 401 || code === 404) {
            message = 'Telegram no longer takes this bot’s token. Paste a new one from @BotFather.'
            changed()
            return
          }
          message = code === 409
            ? 'Another program is reading this bot’s messages. Give Locust a bot of its own, made with @BotFather.'
            : 'Telegram could not be reached. Locust keeps trying.'
          changed()
          await new Promise((settle) => { const wait = setTimeout(settle, code === 409 ? 30_000 : backoff); stop.signal.addEventListener('abort', () => { clearTimeout(wait); settle(undefined) }) })
          backoff = Math.min(backoff * 2, 60_000)
        } finally {
          clearTimeout(timer)
          stop.signal.removeEventListener('abort', abortBoth)
        }
      }
    })()
  }

  const freshLink = (): void => { link = { code: randomBytes(12).toString('base64url'), until: now() + TELEGRAM_LINK_MS } }

  return {
    async load() {
      const text = await readFile(file, 'utf8').catch(() => undefined)
      if (text === undefined) return
      try {
        const parsed = JSON.parse(text) as Saved
        if (parsed.schemaVersion !== 1 || typeof parsed.token !== 'string' || typeof parsed.bot?.username !== 'string') return
        saved = parsed
      } catch { return }
      try {
        token = options.secrets.available() ? options.secrets.decrypt(Buffer.from(saved.token, 'base64')) : undefined
      } catch { token = undefined }
      if (token === undefined || !TELEGRAM_TOKEN.test(token)) {
        token = undefined
        message = 'The bot’s token could not be read on this computer. Paste it again.'
        return
      }
      if (options.refusal() !== undefined) return
      startPolling()
      // A turn that ended while Locust was closing is answered now; one that did not finish is said so.
      for (const turn of [...(saved.turns ?? [])]) {
        if (options.live(turn.missionId) || turn.carried === true) continue
        await runEnded(turn.missionId).catch(() => undefined)
      }
      if ((saved.turns ?? []).some((turn) => options.live(turn.missionId))) typingOn()
    },
    async settings() {
      return settingsNow()
    },
    async setToken(given) {
      const refused = options.refusal()
      if (refused !== undefined) return { ...settingsNow(), message: refused }
      const candidate = typeof given === 'string' ? given.trim() : ''
      if (!TELEGRAM_TOKEN.test(candidate)) return { ...settingsNow(), message: 'That is not a bot’s token. @BotFather gives one like 123456789:AAH…, on a line of its own: copy all of it.' }
      if (!options.secrets.available()) return { ...settingsNow(), message: 'This computer cannot encrypt the token, so it was not kept.' }
      let bot: { id: number; username: string; first_name: string }
      try {
        bot = await call('getMe', {}, undefined, candidate)
        const hook = await call<{ url?: string }>('getWebhookInfo', {}, undefined, candidate)
        if (typeof hook.url === 'string' && hook.url.length > 0) return { ...settingsNow(), message: 'This bot sends its messages to another program already. Make a new bot for Locust with @BotFather.' }
      } catch (error) {
        const code = error instanceof TelegramError ? error.code : 0
        return { ...settingsNow(), message: code === 401 || code === 404 ? 'Telegram does not know that token. Copy it again from @BotFather.' : 'Telegram could not be reached. Check the connection, then try again.' }
      }
      await stopPolling()
      token = candidate
      // A new bot is paired anew: the chat of another bot is not this one's. The same bot's token again keeps its chat.
      const same = saved !== undefined && saved.bot.id === bot.id ? saved : undefined
      saved = { ...(same ?? {}), schemaVersion: 1, token: options.secrets.encrypt(candidate).toString('base64'), bot: { id: bot.id, username: bot.username, name: bot.first_name } }
      commandsSet = ''
      message = undefined
      if (saved.chat === undefined) freshLink()
      try { await persist() } catch {
        saved = undefined; token = undefined; link = undefined
        return { ...settingsNow(), message: 'The token could not be kept on this computer. Nothing changed.' }
      }
      startPolling()
      changed()
      return settingsNow()
    },
    async newLink() {
      if (saved === undefined) return { ...settingsNow(), message: 'Paste a bot’s token first.' }
      freshLink()
      changed()
      return settingsNow()
    },
    async remove() {
      await stopPolling()
      clearInterval(typing)
      typing = undefined
      saved = undefined
      token = undefined
      link = undefined
      message = undefined
      held = undefined
      await persist().catch(() => undefined)
      changed()
      return settingsNow()
    },
    cardRaised(request, name) {
      if (saved?.chat === undefined) return
      void (async () => {
        if ((await fromHere(request.missionId)) === undefined) return
        await postCard(request, name)
      })().catch(() => undefined)
    },
    cardSettled(approvalId, how) {
      if (saved?.cards?.some((card) => card.approvalId === approvalId) !== true) return
      // An answer given here says itself where it was given (handleButton).
      if (answeringHere.has(approvalId)) return
      const words = how === 'allowed' ? 'Allowed in Locust.' : how === 'denied' ? 'Denied in Locust.' : how === 'answered' ? 'Answered in Locust.' : 'No longer asked.'
      void settle(approvalId, words).catch(() => undefined)
    },
    async onRunEnded(mission) {
      await runEnded(mission.missionId)
    },
    async dispose() {
      clearInterval(typing)
      typing = undefined
      await stopPolling()
      await writing
    }
  }
}
