import { randomBytes } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { addressedTeammate, CHAT_LINK_MS, CHAT_STALE_MS, teammateCommands, type ChatAppId, type ChatAppSettings } from '../shared/chat-apps.js'
import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import { joinMessageFragments } from '../shared/messageFragments.js'
import { unwrapProtocolTags } from '../shared/protocolTags.js'
import { scrubSecrets } from '../shared/secrets.js'
import { privateMcpFile } from './locust-mcp-host.js'
import type { SecretBox } from './own-models.js'
import { replaceFile } from './replace-file.js'

/*
 * YOUR TEAMMATES IN CHAT APPS (0.831 Telegram; 0.832 Discord and Slack; shared/chat-apps.ts). What the three share:
 * the one chat that is heard, each message to a teammate as the person's next one, the answer back in the chat, and a
 * card from a turn started there with Allow once and Deny. Each app brings its own connection, words and markup
 * (telegram.ts, discord.ts, slack.ts); this keeps who is heard, and what becomes of each message, the same in all.
 */
/** The longest answer Locust reads for the chat; each app's own limits cut it further. */
const MAX_ANSWER = 15_000
const MAX_ANSWERS_KEPT = 200
const MAX_CARDS_KEPT = 50
const MAX_TURNS_KEPT = 20
/** A turn answered here, whose carry-on or check-fix still counts as from here, for this long. */
const SUCCESSOR_MS = 30 * 60_000
const HELD_MS = 10 * 60_000
/** After this many wrong codes sent to the bot, the code shown in Settings stops working and a new one is made. */
const WRONG_CODES = 10

export interface ChatTeammate { readonly teammateId: string; readonly name: string; readonly role: string }

/** What a chat needs of a turn in the record: how it ended and what it said. */
export interface ChatMission {
  readonly missionId: string
  readonly phase: string
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly startedBy?: { readonly kind: string }
  readonly continuesFrom?: { readonly missionId: string }
  readonly hostFailureMessage?: string
}

export type ChatStart = { readonly ok: true; readonly missionId: string } | { readonly ok: false; readonly message: string }

export interface ChatLinkOptions {
  readonly directory: string
  readonly secrets: SecretBox
  /** The organization's word, read at every change and every message. */
  readonly refusal: () => string | undefined
  /** This computer's name, said when a chat pairs. */
  readonly computer: () => string
  readonly team: () => Promise<readonly ChatTeammate[]>
  /** The person's next message to a teammate: a new conversation, or the next turn of the one it continues. */
  readonly start: (input: { readonly teammateId: string; readonly text: string; readonly followUpOf?: string }) => Promise<ChatStart>
  readonly read: (missionId: string) => Promise<ChatMission | undefined>
  readonly live: (missionId: string) => boolean
  /** A turn that ran out of its agent's plan and was carried on with another (carry-on.ts). */
  readonly carried?: (missionId: string) => boolean
  readonly waiting: () => readonly { readonly request: MissionApprovalRequest; readonly teammateId?: string }[]
  /** The window's own path for a card's answer: Allow once, Deny, or a question's answer -- never Always. */
  readonly decide: (answer: MissionApprovalAnswer) => Promise<boolean>
  readonly changed?: (settings: ChatAppSettings) => void
  readonly note?: (label: string, detail: string) => void
  readonly now?: () => number
  readonly protectFile?: (path: string) => Promise<void>
  /** How often the chat is shown the bot typing, while a turn works. */
  readonly typingMs?: number
}

export interface ChatLink {
  load(): Promise<void>
  settings(): Promise<ChatAppSettings>
  setToken(token: unknown): Promise<ChatAppSettings>
  newLink(): Promise<ChatAppSettings>
  remove(): Promise<ChatAppSettings>
  /** A card the window shows: sent to the chat too when its turn came from there. */
  cardRaised(request: MissionApprovalRequest, name: string | undefined): void
  /** A card answered anywhere, or taken down: its message in the chat says so, and its buttons go. */
  cardSettled(approvalId: string, how: 'allowed' | 'denied' | 'answered' | 'gone'): void
  onRunEnded(mission: { readonly missionId: string }): Promise<void>
  dispose(): Promise<void>
}

/** The bot, as its app named it when the token was checked. */
export interface ChatBot {
  /** Its id in the app: Telegram's number, Discord's or Slack's user id. */
  readonly id: string | number
  readonly username: string
  readonly name: string
  /** What else the app said of it: Discord's application, Slack's workspace and app. */
  readonly extra?: Readonly<Record<string, string>>
}

/** One message to the bot, as its app said it. */
export interface ChatIncoming {
  /** Who wrote it: their id in the app. */
  readonly from: string
  /** Written by a bot -- this one's own messages, or another's: never acted on. */
  readonly bot?: boolean
  /** Where it was written: a chat's id. */
  readonly place: string
  /** A private chat between the person and the bot: the only kind of place a link hears. */
  readonly direct: boolean
  readonly id: string
  /** Its words; none for a picture, a file or a sticker. */
  readonly text?: string
  /** When it was written, in milliseconds. */
  readonly sentAt: number
  /** The message it answers: a reply's (Telegram, Discord), or its thread's first (Slack). */
  readonly replyTo?: string
  /** Where an answer to it goes: a reply to it (Telegram, Discord), or its thread (Slack); none for the chat itself. */
  readonly answerTo?: string
  /** The writer's name and handle, kept once their chat pairs. */
  readonly name?: string
  readonly username?: string
  /** A command the app parsed itself (Discord's slash commands), in place of reading one from the words. */
  readonly command?: { readonly command: string; readonly rest: string }
  /** The app's own reply to this one message, used once, in place of a new message (a slash command's answer). */
  readonly respond?: (text: string, buttons?: ChatButtons) => Promise<string | undefined>
}

/** A button pressed under one of the bot's messages. */
export interface ChatPress {
  readonly from: string
  readonly place: string
  /** The bot's message the button is under. */
  readonly message: string
  readonly data: string
  /** Says the press was taken, with a few words where the app shows them for a moment (Telegram). */
  readonly answer: (text?: string) => Promise<void>
}

export interface ChatButton { readonly label: string; readonly data: string; readonly style?: 'allow' | 'deny' }
export type ChatButtons = readonly (readonly ChatButton[])[]
export interface ChatSendExtra {
  /** As a reply to this message, or in its thread. */
  readonly answerTo?: string
  /** A later part of one answer: a reply only where the app keeps a thread (Slack). */
  readonly continued?: true
  readonly buttons?: ChatButtons
}
export interface ChatCommand {
  readonly command: string
  readonly description: string
  /** The words it takes, said in the app's own menu (Discord's option). */
  readonly words?: string
}

/** What a link is told by its app's connection as it goes. */
export interface ChatEvents {
  /** A message, acted on in turn; resolves once it has been. */
  message(incoming: ChatIncoming): Promise<void>
  press(press: ChatPress): Promise<void>
  /** The app is being read now. */
  connected(): void
  /** It is not: why, in plain words. `stop` when it will not come back without the person (a token refused). */
  trouble(message: string, stop?: boolean): void
  /** The chat is read, but a message did not reach it, and why (blocked, for one). */
  problem(message: string): void
  note(detail: string): void
  /** Telegram's place in its updates, kept before any is acted on. */
  offset(): number | undefined
  setOffset(offset: number): Promise<void>
}

/** A connection to the app, open until stopped. */
export interface ChatTransport {
  stop(): Promise<void>
  /** A message to a chat; its id, or undefined when it was not sent. */
  send(place: string, text: string, extra: ChatSendExtra): Promise<string | undefined>
  /** A message of the bot's, said again; its buttons go. */
  edit(place: string, message: string, text: string): Promise<void>
  /** Shows the bot working in the chat for a few seconds (Telegram, Discord). */
  readonly typing?: (place: string) => Promise<void>
  readonly typingMs?: number
  /** Marks the person's message while its turn works, and unmarks it once answered (Slack's reaction). */
  readonly working?: (place: string, message: string, on: boolean) => Promise<void>
  /** The app's menu of commands: each teammate, then Locust's own. */
  readonly commands?: (list: readonly ChatCommand[]) => Promise<void>
  /** The person's name and handle, for an app whose messages do not say them (Slack). */
  readonly whoIs?: (user: string) => Promise<{ readonly name?: string; readonly username?: string } | undefined>
}

/** The words that differ from app to app. Each is plain text: the link marks it up for the app. */
export interface ChatWords {
  /** How a teammate is written to: '/ada' (Telegram, Discord) or 'Ada:' (Slack). */
  readonly address: (command: string, name: string) => string
  /** How one of Locust's own commands is written: '/who' or 'who'. */
  readonly own: (command: 'who' | 'new' | 'waiting' | 'help') => string
  /** A teammate's line in the list, marked up: '/ada Ada, Research & Briefs'. */
  readonly whoLine: (command: string, name: string, role: string) => string
  /** How an older conversation is carried on. */
  readonly carryOn: string
  /** How a question card is answered in words: with options shown, and with none. */
  readonly replyToCard: { readonly orOwn: string; readonly only: string }
  /** Said, at most once a minute, to a chat that sent a code or link that pairs nothing. */
  readonly wrongCode: string
  /** More of the help, the app's own. */
  readonly helpMore?: string
  /** Asked for a pairing code before anything is kept. */
  readonly noToken: string
  /** The kept token could not be read on this computer. */
  readonly unreadable: string
}

/** A chat app as a link needs it: its words, its markup, how it pairs, and how it is reached with what was pasted. */
export interface ChatApp {
  readonly id: ChatAppId
  /** The file the link keeps in Locust's own folder. */
  readonly file: string
  /** Plain text made safe in the app's markup; bold and italic take plain text too. */
  readonly escape: (text: string) => string
  readonly bold: (text: string) => string
  readonly italic: (text: string) => string
  /** An answer as the messages that carry it: who answered, then the answer, each under the app's limit. */
  readonly answerParts: (name: string, markdown: string) => string[]
  /** A command written the app's own way ('/ada hello'); undefined when the words are not one. */
  readonly parse: (text: string, bot: ChatBot) => { readonly command: string; readonly rest: string } | undefined
  /** The pairing code a message carries, when it carries one. */
  readonly codeIn: (text: string, bot: ChatBot) => string | undefined
  /** A new pairing code: Telegram's goes in a link; Discord's and Slack's is typed. */
  readonly freshCode: () => string
  /** The code as Settings shows it: in Telegram's link, or itself, to type. */
  readonly pairing: (bot: ChatBot, code: string, until: string) => Pick<ChatAppSettings, 'link' | 'code'>
  /** The bot as Settings names it. */
  readonly shown: (bot: ChatBot) => NonNullable<ChatAppSettings['bot']>
  /** Whether what was pasted is the shape of this app's token: the secret to keep, or why not, at once. */
  readonly shape: (given: unknown) => { readonly ok: true; readonly secret: string } | { readonly ok: false; readonly message: string }
  /** The app's own word on the secret: the bot it belongs to, or why it will not do. */
  readonly verify: (secret: string) => Promise<{ readonly ok: true; readonly bot: ChatBot } | { readonly ok: false; readonly message: string }>
  /** Whether a secret read back is still this app's shape. */
  readonly keeps: (secret: string) => boolean
  readonly connect: (secret: string, bot: ChatBot, events: ChatEvents) => ChatTransport
  /** What the app keeps of the link, cleared on Remove (Discord's commands). */
  readonly forget?: (secret: string, bot: ChatBot) => Promise<void>
  readonly words: ChatWords
}

interface SavedChat { readonly place: string; readonly user: string; readonly name: string; readonly username?: string; readonly pairedAt: string }
interface SavedTurn {
  readonly missionId: string
  readonly teammateId: string
  /** Where its answer goes (ChatIncoming.answerTo). */
  readonly answerTo?: string
  /** The person's message that started it. */
  readonly asked?: string
  readonly at: number
  readonly carried?: true
}
interface SavedCard {
  readonly key: string
  readonly approvalId: string
  readonly missionId: string
  readonly messageId: string
  readonly text: string
  /** A question answered here: its id, its options' labels, and whether a reply in words answers it. */
  readonly question?: { readonly id: string; readonly options: readonly string[]; readonly words: boolean }
}
interface Saved {
  readonly schemaVersion: 1
  /** The token, or tokens, as the operating system encrypted them, base64. */
  readonly token: string
  readonly bot: ChatBot
  readonly chat?: SavedChat
  /** Telegram's place in its updates. */
  readonly offset?: number
  /** Who a message with no command goes to. */
  readonly current?: string
  /** Teammates whose next message starts a new conversation. */
  readonly fresh?: readonly string[]
  /** Each teammate's conversation from the chat, by the turn last sent there. */
  readonly conversations?: Readonly<Record<string, string>>
  /** The chat's messages that carry a conversation -- answers, and the person's own that started a turn. */
  readonly answers?: readonly { readonly messageId: string; readonly missionId: string; readonly teammateId?: string }[]
  readonly turns?: readonly SavedTurn[]
  readonly cards?: readonly SavedCard[]
  /** Turns answered here lately, whose carry-on or check-fix is answered here too. */
  readonly answered?: readonly { readonly missionId: string; readonly at: number }[]
}

/** 0.831's Telegram file, with numbers for its ids, read into this shape. */
function migrated(raw: Record<string, unknown>): Saved {
  const id = (value: unknown): string | undefined => (typeof value === 'number' || typeof value === 'string' ? String(value) : undefined)
  const chat = raw.chat as Record<string, unknown> | undefined
  const turns = Array.isArray(raw.turns) ? raw.turns as Record<string, unknown>[] : undefined
  const answers = Array.isArray(raw.answers) ? raw.answers as Record<string, unknown>[] : undefined
  const cards = Array.isArray(raw.cards) ? raw.cards as Record<string, unknown>[] : undefined
  return {
    ...(raw as unknown as Saved),
    ...(chat === undefined ? {} : {
      chat: {
        place: id(chat.place ?? chat.id) ?? '', user: id(chat.user ?? chat.userId) ?? '', name: String(chat.name ?? 'Your chat'),
        ...(typeof chat.username === 'string' ? { username: chat.username } : {}), pairedAt: String(chat.pairedAt ?? new Date(0).toISOString())
      }
    }),
    ...(turns === undefined ? {} : {
      turns: turns.map((turn) => {
        const { replyTo, answerTo, asked, ...rest } = turn
        const to = id(answerTo ?? replyTo)
        const by = id(asked ?? replyTo)
        return { ...(rest as unknown as SavedTurn), ...(to === undefined ? {} : { answerTo: to }), ...(by === undefined ? {} : { asked: by }) }
      })
    }),
    ...(answers === undefined ? {} : { answers: answers.map((one) => ({ ...(one as unknown as { missionId: string; teammateId?: string }), messageId: id(one.messageId) ?? '' })) }),
    ...(cards === undefined ? {} : {
      cards: cards.map((one) => {
        const { html, ...rest } = one
        return { ...(rest as unknown as SavedCard), messageId: id(one.messageId) ?? '', text: String(one.text ?? html ?? '') }
      })
    })
  }
}

/** The answer a turn gave, as the chat is told it: its last whole message, the reason it failed, or that it stopped. */
export function chatAnswer(mission: ChatMission, name: string): string {
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

export function createChatLink(app: ChatApp, options: ChatLinkOptions): ChatLink {
  const file = join(options.directory, app.file)
  const now = options.now ?? Date.now
  let saved: Saved | undefined
  let secret: string | undefined
  let link: { readonly code: string; readonly until: number } | undefined
  let wrongCodes = 0
  let connected = false
  let message: string | undefined
  let transport: ChatTransport | undefined
  let ticker: ReturnType<typeof setInterval> | undefined
  let typing: ReturnType<typeof setInterval> | undefined
  let commandsSet = ''
  let commandsAt = 0
  let held: { readonly text: string; readonly origin: { readonly id: string; readonly answerTo?: string }; readonly at: number; readonly choices: readonly string[] } | undefined
  const answeringHere = new Set<string>()
  const wrongCodeAt = new Map<string, number>()
  const seen: string[] = []
  let writing: Promise<void> = Promise.resolve()
  let queue: Promise<void> = Promise.resolve()

  const note = (detail: string): void => { options.note?.(app.id, detail) }
  const { escape, bold, italic, words } = app

  const settingsNow = (): ChatAppSettings => ({
    ...(saved === undefined ? {} : { bot: app.shown(saved.bot) }),
    ...(saved?.chat === undefined ? {} : { chat: { name: saved.chat.name, ...(saved.chat.username === undefined ? {} : { username: saved.chat.username }), pairedAt: saved.chat.pairedAt } }),
    ...(saved === undefined || link === undefined || link.until <= now() ? {} : app.pairing(saved.bot, link.code, new Date(link.until).toISOString())),
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

  /** A message to the paired chat, in the app's markup. */
  const send = async (text: string, extra: ChatSendExtra = {}): Promise<string | undefined> => {
    const chat = saved?.chat
    if (chat === undefined || transport === undefined) return undefined
    return transport.send(chat.place, text, extra).catch(() => undefined)
  }
  const edit = async (messageId: string, text: string): Promise<void> => {
    const chat = saved?.chat
    if (chat === undefined || transport === undefined) return
    await transport.edit(chat.place, messageId, text).catch(() => undefined)
  }

  const ours = (team: readonly ChatTeammate[]): ReadonlyMap<string, string> => teammateCommands(team)
  const teammateNamed = (team: readonly ChatTeammate[], id: string | undefined): ChatTeammate | undefined => team.find((mate) => mate.teammateId === id)

  /** The app's menu of commands: each teammate, then Locust's own. Set again only when the team changed. */
  const refreshCommands = async (force = false): Promise<void> => {
    if (saved?.chat === undefined || transport?.commands === undefined) return
    if (!force && now() - commandsAt < 60_000) return
    commandsAt = now()
    const team = await options.team().catch(() => [])
    const commands = ours(team)
    const list: ChatCommand[] = [
      ...team.slice(0, 90).map((mate) => ({ command: commands.get(mate.teammateId)!, description: `Write to ${mate.name} (${mate.role})`.slice(0, 256), words: `Your message to ${mate.name}` })),
      { command: 'who', description: 'Your teammates, and who you are writing to' },
      { command: 'new', description: 'Start a new conversation with them', words: 'The first message of the new conversation' },
      { command: 'waiting', description: 'The cards waiting on you' },
      { command: 'help', description: 'What you can do here' }
    ]
    const said = JSON.stringify(list)
    if (said === commandsSet) return
    try { await transport.commands(list); commandsSet = said } catch { /* The menu is a help, not the way in: commands work without it. */ }
  }

  const helpText = (team: readonly ChatTeammate[]): string => {
    const commands = ours(team)
    const first = team[0]
    const example = first === undefined ? words.address('name', 'Name') : words.address(commands.get(first.teammateId)!, first.name)
    return [
      bold(`Locust on ${options.computer()}`),
      `${escape(example)} and a message writes to ${first === undefined ? 'that teammate' : escape(first.name)}. After that, just write: it goes to the same teammate until you pick another.`,
      escape(words.carryOn),
      `${escape(words.own('new'))} starts a new conversation with whoever you are writing to.`,
      `${escape(words.own('who'))} lists your teammates. ${escape(words.own('waiting'))} shows the cards waiting on you.`,
      ...(words.helpMore === undefined ? [] : [escape(words.helpMore)]),
      'Locust answers while it is open on your computer. Approval cards here offer Allow once and Deny; Always stays in Locust.'
    ].join('\n')
  }

  const pairedFrom = (from: string | undefined, place: string): boolean => saved?.chat !== undefined && saved.chat.place === place && from === saved.chat.user

  const pair = async (incoming: ChatIncoming): Promise<void> => {
    const known = incoming.name === undefined ? await transport?.whoIs?.(incoming.from).catch(() => undefined) : undefined
    const name = incoming.name ?? known?.name ?? 'Your chat'
    const username = incoming.username ?? known?.username
    const chat: SavedChat = { place: incoming.place, user: incoming.from, name, ...(username === undefined ? {} : { username }), pairedAt: new Date(now()).toISOString() }
    // A new chat starts clean: what the last one was writing, and to whom, was that chat's.
    update((was) => ({ schemaVersion: 1, token: was.token, bot: was.bot, chat, ...(was.offset === undefined ? {} : { offset: was.offset }) }))
    link = undefined
    wrongCodes = 0
    message = undefined
    held = undefined
    await persist().catch(() => undefined)
    changed()
    const team = await options.team().catch(() => [])
    const welcome = `Paired with Locust on ${bold(options.computer())}.\n\n${helpText(team)}`
    if (incoming.respond !== undefined) await incoming.respond(welcome).catch(() => undefined)
    else await send(welcome)
    await refreshCommands(true)
  }

  /** Says once a minute at most, to a chat, that its code or link did not pair it. Too many wrong ones end the code. */
  const wrongCode = async (incoming: ChatIncoming): Promise<void> => {
    wrongCodes += 1
    if (wrongCodes >= WRONG_CODES && link !== undefined) {
      link = { code: app.freshCode(), until: now() + CHAT_LINK_MS }
      wrongCodes = 0
      message = 'Too many wrong codes were sent to the bot, so Locust made a new one.'
      changed()
    }
    const last = wrongCodeAt.get(incoming.place)
    if (last !== undefined && now() - last < 60_000) return
    wrongCodeAt.set(incoming.place, now())
    const said = escape(words.wrongCode)
    try {
      if (incoming.respond !== undefined) await incoming.respond(said)
      else await transport?.send(incoming.place, said, {})
    } catch { /* A stranger's chat is owed nothing more. */ }
  }

  const typingOn = (): void => {
    if (typing !== undefined || transport?.typing === undefined) return
    const beat = async (): Promise<void> => {
      // A turn that never said it ended (Locust was killed) is let go after half a day.
      if ((saved?.turns ?? []).some((turn) => now() - turn.at > 12 * 60 * 60_000 && !options.live(turn.missionId))) {
        update((was) => ({ ...was, turns: (was.turns ?? []).filter((turn) => now() - turn.at <= 12 * 60 * 60_000 || options.live(turn.missionId)) }))
      }
      const turns = saved?.turns ?? []
      const waitingOn = new Set(options.waiting().map((entry) => entry.request.missionId))
      const working = turns.filter((turn) => options.live(turn.missionId) && !waitingOn.has(turn.missionId))
      if (turns.length === 0) { clearInterval(typing); typing = undefined; return }
      if (working.length > 0 && saved?.chat !== undefined) await transport?.typing?.(saved.chat.place).catch(() => undefined)
    }
    typing = setInterval(() => { void beat() }, options.typingMs ?? transport.typingMs ?? 4_500)
    void beat()
  }

  /** How a message is answered: its app's own reply once (a slash command's), then messages to the chat. */
  const replier = (incoming: ChatIncoming) => {
    let responded = false
    const say = async (text: string, extra: { readonly reply?: boolean; readonly buttons?: ChatButtons } = {}): Promise<string | undefined> => {
      if (incoming.respond !== undefined && !responded) { responded = true; return incoming.respond(text, extra.buttons).catch(() => undefined) }
      return send(text, { ...(extra.reply === true && incoming.answerTo !== undefined ? { answerTo: incoming.answerTo } : {}), ...(extra.buttons === undefined ? {} : { buttons: extra.buttons }) })
    }
    return { say, pending: () => incoming.respond !== undefined && !responded }
  }
  type Replier = ReturnType<typeof replier>

  /** The person's message to a teammate: the next turn of their conversation from here, or a new one. */
  const sendTo = async (teammate: ChatTeammate, text: string, origin: { readonly id: string; readonly answerTo?: string }, reply?: Replier, continues?: string): Promise<void> => {
    const fresh = saved?.fresh?.includes(teammate.teammateId) === true
    const kept = fresh ? undefined : saved?.conversations?.[teammate.teammateId]
    // A conversation deleted in Locust since is not carried on: the message starts a new one.
    const keptStands = kept !== undefined && (options.live(kept) || (await options.read(kept).catch(() => undefined)) !== undefined)
    const followUpOf = continues ?? (keptStands ? kept : undefined)
    const started = await options.start({ teammateId: teammate.teammateId, text, ...(followUpOf === undefined ? {} : { followUpOf }) })
    if (!started.ok) {
      if (reply !== undefined) await reply.say(escape(started.message), { reply: true })
      else await send(escape(started.message), origin.answerTo === undefined ? {} : { answerTo: origin.answerTo })
      return
    }
    update((was) => ({
      ...was,
      current: teammate.teammateId,
      fresh: (was.fresh ?? []).filter((id) => id !== teammate.teammateId),
      conversations: { ...(was.conversations ?? {}), [teammate.teammateId]: started.missionId },
      turns: [...(was.turns ?? []), { missionId: started.missionId, teammateId: teammate.teammateId, ...(origin.answerTo === undefined ? {} : { answerTo: origin.answerTo }), asked: origin.id, at: now() }].slice(-MAX_TURNS_KEPT),
      // A reply to the person's own message carries its conversation on too (a thread under it, in Slack).
      answers: [...(was.answers ?? []), { messageId: origin.id, missionId: started.missionId, teammateId: teammate.teammateId }].slice(-MAX_ANSWERS_KEPT)
    }))
    await persist().catch(() => undefined)
    // An app that answers each command in place (Discord) is told who it went to.
    if (reply?.pending() === true) await reply.say(`${bold(`To ${teammate.name}`)}: ${escape(text)}`)
    if (saved?.chat !== undefined) void transport?.working?.(saved.chat.place, origin.id, true).catch(() => undefined)
    typingOn()
  }

  const cardKey = (): string => randomBytes(6).toString('base64url')

  /** A card in the chat: words, and the buttons that answer it here when it can be answered here. */
  const postCard = async (request: MissionApprovalRequest, name: string | undefined): Promise<void> => {
    if (saved?.chat === undefined || saved.cards?.some((card) => card.approvalId === request.approvalId) === true) return
    const who = name ?? 'A teammate'
    const key = cardKey()
    let text: string
    let buttons: ChatButton[][] = []
    let question: SavedCard['question']
    if (request.kind === 'question') {
      const asked = request.questions ?? []
      const only = asked.length === 1 ? asked[0] : undefined
      const said = asked.map((one) => escape(scrubSecrets([one.header, one.question].filter((part) => part !== null && part.length > 0).join(': ')).text)).join('\n')
      text = `${bold(`${who} is asking you`)}\n${said.length > 0 ? said : escape(scrubSecrets(request.summary).text)}`
      if (request.answerIn !== undefined || only === undefined || only.isSecret) {
        text += `\n\n${italic(`Answer it in ${request.answerIn ?? 'Locust on your computer'}.`)}`
      } else {
        const labels = only.options.slice(0, 8).map((option) => option.label)
        const free = only.options.length === 0 || only.isOther
        buttons = labels.map((label, index) => [{ label: label.slice(0, 60), data: `o:${key}:${String(index)}` }])
        if (free) text += `\n\n${italic(labels.length > 0 ? words.replyToCard.orOwn : words.replyToCard.only)}`
        question = { id: only.id, options: labels, words: free }
      }
    } else {
      text = `${bold(`${who} needs your approval`)}\n${escape(scrubSecrets(request.summary).text)}\n\n${italic('The whole of it is in Locust on your computer.')}`
      buttons = [[{ label: 'Allow once', data: `y:${key}`, style: 'allow' }, { label: 'Deny', data: `n:${key}`, style: 'deny' }]]
    }
    const messageId = await send(text, buttons.length === 0 ? {} : { buttons })
    if (messageId === undefined) return
    update((was) => ({ ...was, cards: [...(was.cards ?? []), { key, approvalId: request.approvalId, missionId: request.missionId, messageId, text, ...(question === undefined ? {} : { question }) }].slice(-MAX_CARDS_KEPT) }))
    await persist().catch(() => undefined)
  }

  /** The card's message says what became of it, and loses its buttons. Once: a card settled is forgotten here. */
  const settle = async (approvalId: string, said: string): Promise<void> => {
    const card = saved?.cards?.find((one) => one.approvalId === approvalId)
    if (card === undefined) return
    update((was) => ({ ...was, cards: (was.cards ?? []).filter((one) => one.approvalId !== approvalId) }))
    await persist().catch(() => undefined)
    await edit(card.messageId, `${card.text}\n\n${bold(said)}`)
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

  const answerTurn = async (missionId: string, teammateId: string | undefined, origin: SavedTurn | undefined): Promise<void> => {
    const mission = await options.read(missionId).catch(() => undefined)
    const team = await options.team().catch(() => [])
    const name = teammateNamed(team, teammateId)?.name ?? 'Your teammate'
    const said = mission === undefined ? `${name}’s answer could not be read. It is in Locust.` : chatAnswer(mission, name)
    // Nothing shaped like a key goes to the app's servers, whoever wrote it (shared/secrets.ts).
    const parts = app.answerParts(name, scrubSecrets(said).text)
    const sent: { messageId: string; missionId: string; teammateId?: string }[] = []
    for (const [index, part] of parts.entries()) {
      const messageId = await send(part, { ...(origin?.answerTo === undefined ? {} : { answerTo: origin.answerTo }), ...(index > 0 ? { continued: true as const } : {}) })
      if (messageId !== undefined) sent.push({ messageId, missionId, ...(teammateId === undefined ? {} : { teammateId }) })
    }
    if (origin?.asked !== undefined && saved?.chat !== undefined) void transport?.working?.(saved.chat.place, origin.asked, false).catch(() => undefined)
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
    const teammateId = origin?.teammateId ?? (here.successorOf === undefined ? undefined : teammateOfAnswered(here.successorOf))
    update((was) => ({ ...was, turns: (was.turns ?? []).filter((one) => one.missionId !== missionId && one.missionId !== here.successorOf) }))
    await answerTurn(missionId, teammateId, origin)
  }
  const teammateOfAnswered = (missionId: string): string | undefined =>
    Object.entries(saved?.conversations ?? {}).find(([, id]) => id === missionId)?.[0]

  const handleMessage = async (incoming: ChatIncoming): Promise<void> => {
    if (!incoming.direct || incoming.bot === true || incoming.from.length === 0 || saved === undefined) return
    // An app that sends a message twice (a retried event, a resumed connection) has it acted on once. By chat and id:
    // Telegram numbers each chat's messages on their own.
    const key = `${incoming.place}:${incoming.id}`
    if (seen.includes(key)) return
    seen.push(key)
    if (seen.length > 500) seen.splice(0, seen.length - 500)
    const reply = replier(incoming)
    const { say } = reply
    const text = (incoming.text ?? '').trim()
    // Pairing: the code, once, before it runs out. A new chat that has it takes the place of the old one.
    const code = incoming.command === undefined && text.length > 0 ? app.codeIn(text, saved.bot) : undefined
    if (code !== undefined) {
      if (link !== undefined && link.until > now() && code === link.code) { await pair(incoming); return }
      if (!pairedFrom(incoming.from, incoming.place)) { await wrongCode(incoming); return }
    }
    // Every other chat is ignored: a bot can be found by anyone, and it answers only its person.
    if (!pairedFrom(incoming.from, incoming.place)) return
    const refused = options.refusal()
    if (refused !== undefined) { await say(escape(refused)); return }
    const team = await options.team().catch(() => [])
    const commands = ours(team)
    if (incoming.text === undefined && incoming.command === undefined) {
      await say('Locust reads text here for now. Pictures and files go in through Locust on your computer.', { reply: true })
      return
    }
    const command = incoming.command ?? app.parse(text, saved.bot)
    if (command !== undefined && ['start', 'help'].includes(command.command)) { await say(helpText(team)); return }
    if (command?.command === 'who') {
      const current = teammateNamed(team, saved.current)
      const lines = team.map((mate) => words.whoLine(commands.get(mate.teammateId)!, mate.name, mate.role))
      await say(team.length === 0 ? 'You have no teammates yet. Make one in Locust.' : `${bold('Your teammates')}\n${lines.join('\n')}${current === undefined ? '' : `\n\nWriting to ${escape(current.name)}.`}`)
      return
    }
    if (command?.command === 'waiting') {
      const waiting = options.waiting()
      if (waiting.length === 0) { await say('Nothing is waiting on you.'); return }
      if (reply.pending()) await say(`${String(waiting.length)} waiting on you:`)
      for (const entry of waiting.slice(0, 5)) await postCard(entry.request, teammateNamed(team, entry.teammateId)?.name)
      if (waiting.length > 5) await say(`${String(waiting.length - 5)} more are waiting in Locust.`)
      return
    }
    const stale = now() - incoming.sentAt > CHAT_STALE_MS
    const staleSaid = (): Promise<string | undefined> => say(`This came while Locust was closed (sent ${new Date(incoming.sentAt).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}), so nothing was started. Send it again to start it now.`, { reply: true })
    const origin = { id: incoming.id, ...(incoming.answerTo === undefined ? {} : { answerTo: incoming.answerTo }) }
    if (command?.command === 'new') {
      const current = teammateNamed(team, saved.current)
      if (current === undefined) { await say(`Pick who to write to first: ${escape(words.own('who'))} lists your teammates.`); return }
      update((was) => ({ ...was, fresh: [...new Set([...(was.fresh ?? []), current.teammateId])] }))
      await persist().catch(() => undefined)
      if (command.rest.length === 0) { await say(`Your next message starts a new conversation with ${escape(current.name)}.`); return }
      if (stale) { await staleSaid(); return }
      await sendTo(current, command.rest, origin, reply)
      return
    }
    // A teammate's command ('/ada hello'), or its name at the start ('Ada: hello').
    const named = command === undefined ? addressedTeammate(text, team) : undefined
    const commandMate = command === undefined ? undefined : team.find((one) => commands.get(one.teammateId) === command.command)
    if (command !== undefined && commandMate === undefined) {
      await say(`There is no ${escape(words.address(command.command, command.command))}. ${escape(words.own('who'))} lists your teammates.`, { reply: true })
      return
    }
    const mate = commandMate ?? teammateNamed(team, named?.teammateId)
    const rest = command?.rest ?? named?.rest
    if (mate !== undefined && rest !== undefined) {
      if (rest.length === 0) {
        update((was) => ({ ...was, current: mate.teammateId }))
        await persist().catch(() => undefined)
        await say(`Writing to ${escape(mate.name)}. Send your message.`)
        return
      }
      if (stale) { await staleSaid(); return }
      await sendTo(mate, rest, origin, reply)
      return
    }
    if (stale) { await staleSaid(); return }
    // A reply to a card's message answers its question; a reply to an answer carries that conversation on.
    const repliedTo = incoming.replyTo
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
      const answeredMate = teammateNamed(team, answered?.teammateId)
      if (answered !== undefined && answeredMate !== undefined) { await sendTo(answeredMate, text, origin, reply, answered.missionId); return }
    }
    const current = teammateNamed(team, saved.current) ?? (team.length === 1 ? team[0] : undefined)
    if (current !== undefined) { await sendTo(current, text, origin, reply); return }
    if (team.length === 0) { await say('You have no teammates yet. Make one in Locust, then write here.'); return }
    // Who it is for, asked once: the message waits for the answer, for a while.
    const choices = team.slice(0, 8)
    held = { text, origin, at: now(), choices: choices.map((one) => one.teammateId) }
    await say('Who is this for?', { reply: true, buttons: choices.map((one, index) => [{ label: one.name.slice(0, 60), data: `t:${String(index)}` }]) })
  }

  const handlePress = async (press: ChatPress): Promise<void> => {
    if (saved === undefined || !pairedFrom(press.from, press.place)) { await press.answer(); return }
    const refused = options.refusal()
    if (refused !== undefined) { await press.answer(refused.slice(0, 190)); return }
    const data = press.data
    const chosen = /^t:(\d)$/.exec(data)
    if (chosen !== null) {
      const team = await options.team().catch(() => [])
      const mate = teammateNamed(team, held?.choices[Number(chosen[1])])
      const waited = held
      held = undefined
      if (mate === undefined || waited === undefined || now() - waited.at > HELD_MS) {
        await press.answer('That message waited too long. Send it again.')
        await edit(press.message, `Who is this for? ${italic('It waited too long: send it again.')}`)
        return
      }
      await press.answer(`To ${mate.name}`)
      await edit(press.message, `To ${bold(mate.name)}.`)
      await sendTo(mate, waited.text, waited.origin)
      return
    }
    const decided = /^([yno]):([A-Za-z0-9_-]{8})(?::(\d))?$/.exec(data)
    const card = decided === null ? undefined : saved.cards?.find((one) => one.key === decided[2])
    if (decided === null || card === undefined) { await press.answer('This card was already answered, or its turn ended.'); return }
    const answer: MissionApprovalAnswer | undefined = decided[1] === 'o'
      ? card.question?.options[Number(decided[3])] === undefined ? undefined : { approvalId: card.approvalId, answers: { [card.question.id]: [card.question.options[Number(decided[3])]!] } }
      : { approvalId: card.approvalId, decision: decided[1] === 'y' ? 'approve-once' : 'deny' }
    if (answer === undefined) { await press.answer(); return }
    answeringHere.add(card.approvalId)
    const answered = await options.decide(answer).catch(() => false).finally(() => answeringHere.delete(card.approvalId))
    const said = !answered ? 'This was already answered, or its turn ended.'
      : decided[1] === 'y' ? 'Allowed once.' : decided[1] === 'n' ? 'Denied.' : `Answered: ${card.question!.options[Number(decided[3])]!}`
    await press.answer(said.slice(0, 190))
    await settle(card.approvalId, said)
  }

  /** One message or press at a time, in the order the app gave them. */
  const inTurn = (work: () => Promise<void>): Promise<void> => {
    const next = queue.then(work).catch((error: unknown) => { note(`a message could not be handled: ${error instanceof Error ? error.message : String(error)}`) })
    queue = next
    return next
  }

  const events: ChatEvents = {
    message: (incoming) => inTurn(() => handleMessage(incoming)),
    press: (press) => inTurn(() => handlePress(press)),
    connected: () => { if (!connected || message !== undefined) { connected = true; message = undefined; changed() } },
    trouble: (said, stop) => {
      connected = false
      message = said
      changed()
      if (stop === true) void stopTransport()
    },
    problem: (said) => { message = said; changed() },
    note,
    offset: () => saved?.offset,
    setOffset: async (offset) => {
      update((was) => ({ ...was, offset }))
      await persist().catch(() => undefined)
    }
  }

  const stopTransport = async (): Promise<void> => {
    clearInterval(ticker)
    ticker = undefined
    const was = transport
    transport = undefined
    connected = false
    await was?.stop().catch(() => undefined)
  }

  const startTransport = (): void => {
    if (transport !== undefined || saved === undefined || secret === undefined) return
    transport = app.connect(secret, saved.bot, events)
    // Each minute: the menu follows the team, and the organization's word, once given, ends the connection.
    ticker = setInterval(() => {
      const refused = options.refusal()
      if (refused !== undefined) { message = refused; void stopTransport().then(changed); return }
      void refreshCommands()
    }, 60_000)
    ticker.unref?.()
  }

  const freshLink = (): void => {
    link = { code: app.freshCode(), until: now() + CHAT_LINK_MS }
    wrongCodes = 0
  }

  return {
    async load() {
      const text = await readFile(file, 'utf8').catch(() => undefined)
      if (text === undefined) return
      try {
        const parsed = JSON.parse(text) as Record<string, unknown>
        const bot = parsed.bot as Record<string, unknown> | undefined
        if (parsed.schemaVersion !== 1 || typeof parsed.token !== 'string' || typeof bot?.username !== 'string') return
        saved = migrated(parsed)
      } catch { return }
      try {
        secret = options.secrets.available() ? options.secrets.decrypt(Buffer.from(saved.token, 'base64')) : undefined
      } catch { secret = undefined }
      if (secret === undefined || !app.keeps(secret)) {
        secret = undefined
        message = words.unreadable
        return
      }
      if (options.refusal() !== undefined) return
      startTransport()
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
      const shaped = app.shape(given)
      if (!shaped.ok) return { ...settingsNow(), message: shaped.message }
      if (!options.secrets.available()) return { ...settingsNow(), message: 'This computer cannot encrypt the token, so it was not kept.' }
      const verified = await app.verify(shaped.secret)
      if (!verified.ok) return { ...settingsNow(), message: verified.message }
      await stopTransport()
      secret = shaped.secret
      // A new bot is paired anew: the chat of another bot is not this one's. The same bot's token again keeps its chat.
      const same = saved !== undefined && String(saved.bot.id) === String(verified.bot.id) ? saved : undefined
      saved = { ...(same ?? {}), schemaVersion: 1, token: options.secrets.encrypt(shaped.secret).toString('base64'), bot: verified.bot }
      commandsSet = ''
      message = undefined
      if (saved.chat === undefined) freshLink()
      try { await persist() } catch {
        saved = undefined; secret = undefined; link = undefined
        return { ...settingsNow(), message: 'The token could not be kept on this computer. Nothing changed.' }
      }
      startTransport()
      changed()
      return settingsNow()
    },
    async newLink() {
      if (saved === undefined) return { ...settingsNow(), message: words.noToken }
      freshLink()
      changed()
      return settingsNow()
    },
    async remove() {
      const kept = saved
      const keptSecret = secret
      await stopTransport()
      clearInterval(typing)
      typing = undefined
      saved = undefined
      secret = undefined
      link = undefined
      message = undefined
      held = undefined
      await persist().catch(() => undefined)
      if (kept !== undefined && keptSecret !== undefined) await app.forget?.(keptSecret, kept.bot).catch(() => undefined)
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
      // An answer given here says itself where it was given (handlePress).
      if (answeringHere.has(approvalId)) return
      const said = how === 'allowed' ? 'Allowed in Locust.' : how === 'denied' ? 'Denied in Locust.' : how === 'answered' ? 'Answered in Locust.' : 'No longer asked.'
      void settle(approvalId, said).catch(() => undefined)
    },
    async onRunEnded(mission) {
      await runEnded(mission.missionId)
    },
    async dispose() {
      clearInterval(typing)
      typing = undefined
      await stopTransport()
      await writing
    }
  }
}
