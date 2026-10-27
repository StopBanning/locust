import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { afterEach, describe, expect, it } from 'vitest'

import { fakeTelegram, htmlProblem, type FakeTelegram } from '../../../../_tools/fake-telegram.mjs'
import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import type { TelegramSettings } from '../shared/telegram.js'
import { createTelegramLink, TELEGRAM_FILE, type TelegramLink, type TelegramMission, type TelegramTeammate } from './telegram.js'

/**
 * TEAMMATES ANSWER THEIR PERSON IN TELEGRAM (0.831, telegram.ts). Locust's link to the person's own bot, against a
 * stand-in for Telegram's Bot API as strict as Telegram on markup (_tools/fake-telegram.mjs): a token is checked and
 * kept encrypted, one chat pairs by the link, anyone else is ignored, `/ada` reaches Ada and the answer comes back,
 * and a card from a turn started there is answered with Allow once or Deny -- by the window's own path, once.
 */
const ADA: TelegramTeammate = { teammateId: 'mate_ada', name: 'Ada', role: 'Research & Briefs' }
const WREN: TelegramTeammate = { teammateId: 'mate_wren', name: 'Wren', role: 'Code & Migrations' }
const STRANGER = { id: 9_999, is_bot: false, first_name: 'Mallory' }

const said = (text: string): readonly NormalizedRuntimeEvent[] =>
  [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text, final: true } } as unknown as NormalizedRuntimeEvent]

interface Harness {
  readonly telegram: FakeTelegram
  readonly link: TelegramLink
  readonly directory: string
  readonly started: { teammateId: string; text: string; followUpOf?: string }[]
  readonly decided: MissionApprovalAnswer[]
  readonly missions: Map<string, TelegramMission>
  readonly live: Set<string>
  readonly changes: TelegramSettings[]
  team: readonly TelegramTeammate[]
  waiting: { request: MissionApprovalRequest; teammateId?: string }[]
  refusal?: string
  clock: number
  /** The pairing link's code, once a chat paired by it. */
  code?: string
  /** Ends a turn: its answer goes in the record, and the run-end step is told. */
  end(missionId: string, answer: string, phase?: string): Promise<void>
}

const harnesses: Harness[] = []
afterEach(async () => {
  for (const harness of harnesses.splice(0)) {
    await harness.link.dispose()
    await rm(harness.directory, { recursive: true, force: true })
  }
})

async function harness(options: { readonly team?: readonly TelegramTeammate[]; readonly webhook?: string } = {}): Promise<Harness> {
  const directory = await mkdtemp(join(tmpdir(), 'locust-telegram-'))
  const telegram = fakeTelegram({ ...(options.webhook === undefined ? {} : { webhook: options.webhook }), maxWaitSeconds: 1 })
  let next = 0
  const self: Harness = {
    telegram,
    directory,
    started: [],
    decided: [],
    missions: new Map(),
    live: new Set(),
    changes: [],
    team: options.team ?? [ADA, WREN],
    waiting: [],
    clock: Date.now(),
    link: undefined as unknown as TelegramLink,
    async end(missionId, answer, phase = 'completed') {
      self.live.delete(missionId)
      self.missions.set(missionId, { missionId, phase, events: said(answer) })
      await self.link.onRunEnded({ missionId })
    }
  }
  ;(self as { link: TelegramLink }).link = createTelegramLink({
    directory,
    // A stand-in for the operating system's encryption: reversible, and never the token in the clear.
    secrets: { available: () => true, encrypt: (text) => Buffer.from([...Buffer.from(text, 'utf8')].map((byte) => byte ^ 0x5a)), decrypt: (cipher) => Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8') },
    refusal: () => self.refusal,
    computer: () => 'DESK-PC',
    team: async () => self.team,
    start: async (input) => {
      self.started.push({ ...input })
      next += 1
      const missionId = `mission_${String(next)}`
      self.live.add(missionId)
      return { ok: true, missionId }
    },
    read: async (missionId) => self.missions.get(missionId),
    live: (missionId) => self.live.has(missionId),
    waiting: () => self.waiting,
    decide: async (answer) => {
      self.decided.push(answer)
      const waited = self.waiting.some((entry) => entry.request.approvalId === answer.approvalId)
      self.waiting = self.waiting.filter((entry) => entry.request.approvalId !== answer.approvalId)
      // The window's one path tells the link a card was answered, as answerApproval does.
      if (waited) self.link.cardSettled(answer.approvalId, 'answers' in answer ? 'answered' : answer.decision === 'deny' ? 'denied' : 'allowed')
      return waited
    },
    changed: (settings) => { self.changes.push(settings) },
    fetcher: telegram.fetch,
    api: () => 'http://127.0.0.1:1',
    now: () => self.clock,
    protectFile: async () => undefined,
    pollSeconds: 1,
    typingMs: 30
  })
  harnesses.push(self)
  return self
}

/** A token kept, a link made, and the person's chat paired by it. */
async function paired(options: Parameters<typeof harness>[0] = {}): Promise<Harness> {
  const self = await harness(options)
  const settings = await self.link.setToken(self.telegram.token)
  const code = new URL(settings.link!.url).searchParams.get('start')!
  self.code = code
  self.telegram.say(`/start ${code}`)
  await self.telegram.until((state) => state.sent.some((one) => one.text.startsWith('Paired with Locust on DESK-PC')), 5_000, 'the pairing message')
  return self
}

const card = (approvalId: string, missionId: string, more: Partial<MissionApprovalRequest> = {}): MissionApprovalRequest => ({
  approvalId, runId: `run_${missionId}`, missionId, kind: 'command', summary: 'Run npm test in tides', detail: 'npm test', cwd: 'C:/work/tides', requestedAt: new Date().toISOString(), ...more
})

describe('teammates answer their person in Telegram', () => {
  it('pairs nothing with a link past its ten minutes', async () => {
    const self = await harness()
    const settings = await self.link.setToken(self.telegram.token)
    const code = new URL(settings.link!.url).searchParams.get('start')!
    self.clock += 11 * 60_000
    expect((await self.link.settings()).link).toBeUndefined()
    self.telegram.say(`/start ${code}`)
    await self.telegram.until((state) => state.sent.some((one) => one.text.includes('does not pair this chat')), 5_000, 'the old link refused')
    expect((await self.link.settings()).chat).toBeUndefined()
  })

  it('checks a token with Telegram, keeps it encrypted, and refuses one it does not know or another program reads', async () => {
    const self = await harness()
    expect((await self.link.setToken('not a token')).message).toContain('not a bot’s token')
    expect((await self.link.setToken('123456789:AAHwrongwrongwrongwrongwrongwrongwro')).message).toContain('Telegram does not know that token')
    const kept = await self.link.setToken(`  ${self.telegram.token} `)
    expect(kept.bot).toEqual({ username: 'locust_test_bot', name: 'Locust test bot' })
    expect(kept.link?.url).toMatch(/^https:\/\/t\.me\/locust_test_bot\?start=[A-Za-z0-9_-]{16}$/)
    expect(kept.message).toBeUndefined()
    const file = await readFile(join(self.directory, TELEGRAM_FILE), 'utf8')
    expect(file).not.toContain(self.telegram.token)
    expect(file).not.toContain(self.telegram.token.split(':')[1])
    await expect.poll(async () => (await self.link.settings()).connected, { timeout: 2_000 }).toBe(true)

    const hooked = await harness({ webhook: 'https://example.com/hook' })
    expect((await hooked.link.setToken(hooked.telegram.token)).message).toContain('sends its messages to another program')
    expect((await hooked.link.settings()).bot).toBeUndefined()
  })

  it('pairs the one chat that opens the link, once, and ignores everyone else', async () => {
    const self = await paired()
    const settings = await self.link.settings()
    expect(settings.chat).toMatchObject({ name: 'Sam', username: 'sam_here' })
    expect(settings.link).toBeUndefined()
    // The bot's menu names each teammate, then Locust's own.
    const menu = await self.telegram.until((state) => state.commands.at(-1), 5_000, 'the command menu')
    expect(menu.map((one) => one.command)).toEqual(['ada', 'wren', 'who', 'new', 'waiting', 'help'])

    // A stranger who finds the bot, even with the code already used: it pairs nothing, and their messages start nothing.
    const before = self.telegram.state.sent.length
    self.telegram.say(`/start ${self.code!}`, { from: STRANGER })
    self.telegram.say('/ada delete everything', { from: STRANGER })
    self.telegram.say('/ada and this', { from: STRANGER, chatId: self.telegram.person.id })
    await self.telegram.until((state) => state.sent.length > before, 5_000, 'the wrong link said')
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.started).toEqual([])
    expect(self.telegram.state.sent.slice(before).map((one) => one.chat.id)).toEqual([STRANGER.id])
    expect(self.telegram.state.sent.at(-1)!.text).toContain('does not pair this chat')
    expect((await self.link.settings()).chat).toMatchObject({ name: 'Sam' })
    // A group the bot is added to is not the person's chat either.
    self.telegram.say('/ada in a group', { chatType: 'group', chatId: -100 })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.started).toEqual([])
  })

  it('sends /ada and a message to Ada, answers in the chat, and carries the conversation on', async () => {
    const self = await paired()
    const asked = self.telegram.say('/ada what changed in tides today?')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    expect(self.started[0]).toEqual({ teammateId: 'mate_ada', text: 'what changed in tides today?' })
    // While it works, the chat shows the bot typing.
    await self.telegram.until((state) => state.actions.length > 0, 5_000, 'typing')
    await self.end('mission_1', '**Three** commits:\n\n- `fix: tides`\n- docs\n\n```ts\nconst a = 1 < 2\n```')
    const answer = self.telegram.state.sent.at(-1)!
    expect(answer.reply_to).toBe(asked)
    expect(answer.html).toContain('<b>Ada</b>')
    expect(answer.html).toContain('<b>Three</b> commits')
    expect(answer.html).toContain('<pre><code class="language-ts">const a = 1 &lt; 2</code></pre>')
    expect(self.telegram.state.refused).toEqual([])

    // No command: the same teammate, the same conversation.
    self.telegram.say('and yesterday?')
    await self.telegram.until(() => self.started.length === 2, 5_000, 'the next turn')
    expect(self.started[1]).toEqual({ teammateId: 'mate_ada', text: 'and yesterday?', followUpOf: 'mission_1' })
    await self.end('mission_2', 'Two more.')
    // /new: the next message starts a conversation of its own.
    self.telegram.say('/new')
    await self.telegram.until((state) => state.sent.at(-1)?.text.includes('starts a new conversation with Ada'), 5_000, '/new said')
    self.telegram.say('start over')
    await self.telegram.until(() => self.started.length === 3, 5_000, 'a fresh turn')
    expect(self.started[2]).toEqual({ teammateId: 'mate_ada', text: 'start over' })
    await self.end('mission_3', 'Fresh.')
    // A reply to an older answer carries THAT conversation on.
    self.telegram.say('more on this', { replyTo: answer.message_id })
    await self.telegram.until(() => self.started.length === 4, 5_000, 'the reply turn')
    expect(self.started[3]).toEqual({ teammateId: 'mate_ada', text: 'more on this', followUpOf: 'mission_1' })
    // /wren on its own switches who is written to.
    self.telegram.say('/wren')
    await self.telegram.until((state) => state.sent.at(-1)?.text.includes('Writing to Wren'), 5_000, 'the switch')
    self.telegram.say('run the tests')
    await self.telegram.until(() => self.started.length === 5, 5_000, 'Wren’s turn')
    expect(self.started[4]).toEqual({ teammateId: 'mate_wren', text: 'run the tests' })
  })

  it('takes anything shaped like a key out of an answer before Telegram has it', async () => {
    const self = await paired()
    self.telegram.say('/ada the token?')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.end('mission_1', 'It is ghp_abcdefghijklmnopqrstuvwxyz0123456789 in .env.') // public-guard: allow (a made-up token in a test)
    const sent = self.telegram.state.sent.at(-1)!.text
    expect(sent).not.toContain('ghp_')
    expect(sent).toContain('[secret-shaped text removed]')
  })

  it('asks who a message is for, once, when nobody is being written to', async () => {
    const self = await paired()
    self.telegram.say('can someone look at the build?')
    const asked = await self.telegram.until((state) => state.sent.find((one) => one.text === 'Who is this for?'), 5_000, 'the question')
    expect(asked.reply_markup?.inline_keyboard.map((row) => row[0]!.text)).toEqual(['Ada', 'Wren'])
    self.telegram.press(asked.message_id, asked.reply_markup!.inline_keyboard[1]![0]!.callback_data)
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the held message sent')
    expect(self.started[0]).toEqual({ teammateId: 'mate_wren', text: 'can someone look at the build?' })
    expect(self.telegram.state.edits.at(-1)!.html).toBe('To <b>Wren</b>.')
    // With one teammate there is nobody to ask about.
    const alone = await paired({ team: [ADA] })
    alone.telegram.say('hello')
    await alone.telegram.until(() => alone.started.length === 1, 5_000, 'the only teammate')
    expect(alone.started[0]!.teammateId).toBe('mate_ada')
  })

  it('brings a card from its own turn with Allow once and Deny, and answers it by the window’s path', async () => {
    const self = await paired()
    self.telegram.say('/wren run the tests')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    const request = card('appr_1', 'mission_1', { summary: 'Run npm test --token=ghp_abcdefghijklmnopqrstuvwxyz0123456789 in tides' }) // public-guard: allow (a made-up token in a test)
    self.waiting = [{ request, teammateId: 'mate_wren' }]
    self.link.cardRaised(request, 'Wren')
    const posted = await self.telegram.until((state) => state.sent.find((one) => one.text.startsWith('Wren needs your approval')), 5_000, 'the card')
    expect(posted.reply_markup?.inline_keyboard[0]!.map((button) => button.text)).toEqual(['Allow once', 'Deny'])
    const [allow, deny] = posted.reply_markup!.inline_keyboard[0]!.map((button) => button.callback_data)
    // Nothing shaped like a key is sent to Telegram's servers.
    expect(posted.text).not.toContain('ghp_')
    expect(posted.text).toContain('[secret-shaped text removed]')
    // A card of a turn started in the window stays in the window.
    self.link.cardRaised(card('appr_window', 'mission_window'), 'Ada')
    await new Promise((settle) => setTimeout(settle, 100))
    expect(self.telegram.state.sent.filter((one) => one.text.includes('needs your approval'))).toHaveLength(1)

    // A stranger's press does nothing.
    self.telegram.press(posted.message_id, allow!, { from: STRANGER, chatId: STRANGER.id })
    await self.telegram.until((state) => state.answers.length === 1, 5_000, 'the stranger answered nothing')
    expect(self.decided).toEqual([])
    // The person's press: Allow once, never Always, and the card says so and loses its buttons.
    self.telegram.press(posted.message_id, allow!)
    await self.telegram.until(() => self.decided.length === 1, 5_000, 'the answer')
    expect(self.decided[0]).toEqual({ approvalId: 'appr_1', decision: 'approve-once' })
    const edited = await self.telegram.until((state) => state.edits.find((one) => one.message_id === posted.message_id), 5_000, 'the card settled')
    expect(edited.html).toContain('<b>Allowed once.</b>')
    expect(edited.reply_markup).toEqual({ inline_keyboard: [] })
    // Pressed again: answered already, nothing decided twice.
    self.telegram.press(posted.message_id, deny!)
    await self.telegram.until((state) => state.answers.length === 3, 5_000, 'the second press answered')
    expect(self.decided).toHaveLength(1)
    expect(self.telegram.state.answers.at(-1)!.text).toContain('already answered')
  })

  it('tells the person here what needs them from any run, once they turn it on (0.837)', async () => {
    const self = await paired()
    const request = card('appr_away', 'mission_window')
    self.waiting = [{ request }]
    const alert = { cards: [{ request, name: 'Ada' }], lines: [{ title: 'Ivy hit the Codex limit', body: 'Pick where the work carries on.' }] }
    self.link.alert(alert)
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.telegram.state.sent.some((one) => one.text.includes('needs your approval') || one.text.includes('Codex limit'))).toBe(false)
    expect((await self.link.setAlerts(true)).alerts).toBe(true)
    self.link.alert(alert)
    const posted = await self.telegram.until((state) => state.sent.find((one) => one.text.startsWith('Ada needs your approval')), 5_000, 'the card')
    expect(posted.reply_markup?.inline_keyboard[0]!.map((button) => button.text)).toEqual(['Allow once', 'Deny'])
    await self.telegram.until((state) => state.sent.find((one) => one.text === 'Ivy hit the Codex limit\nPick where the work carries on.'), 5_000, 'the limit line')
  })

  it('says in the chat what became of a card answered in the window, or whose turn ended', async () => {
    const self = await paired()
    self.telegram.say('/ada look')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    const first = card('appr_a', 'mission_1')
    const second = card('appr_b', 'mission_1', { summary: 'Edit README.md' })
    self.waiting = [{ request: first }, { request: second }]
    self.link.cardRaised(first, 'Ada')
    self.link.cardRaised(second, 'Ada')
    await self.telegram.until((state) => state.sent.filter((one) => one.text.includes('needs your approval')).length === 2, 5_000, 'both cards')
    self.link.cardSettled('appr_a', 'denied')
    await self.telegram.until((state) => state.edits.some((one) => one.html.includes('Denied in Locust.')), 5_000, 'the window’s answer said')
    await self.end('mission_1', 'Stopped there.', 'interrupted')
    await self.telegram.until((state) => state.edits.some((one) => one.html.includes('The turn ended before this was answered.')), 5_000, 'the ended turn said')
    expect(self.telegram.state.sent.at(-1)!.text).toContain('Ada’s turn stopped before it finished.')
  })

  it('answers a question with its options, or a reply in words, but never a secret one', async () => {
    const self = await paired()
    self.telegram.say('/ada plan it')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    const choose = card('appr_q', 'mission_1', {
      kind: 'question', summary: 'Which branch?',
      questions: [{ id: 'q1', header: 'Branch', question: 'Which branch should I use?', options: [{ label: 'main', description: null }, { label: 'release', description: null }], isOther: true, isSecret: false }]
    })
    const secret = card('appr_s', 'mission_1', { kind: 'question', summary: 'A password', questions: [{ id: 'q2', header: null, question: 'The database password?', options: [], isOther: true, isSecret: true }] })
    self.waiting = [{ request: choose }, { request: secret }]
    self.link.cardRaised(choose, 'Ada')
    self.link.cardRaised(secret, 'Ada')
    const asked = await self.telegram.until((state) => state.sent.find((one) => one.text.startsWith('Ada is asking you') && one.text.includes('Which branch')), 5_000, 'the question')
    const hidden = await self.telegram.until((state) => state.sent.find((one) => one.text.includes('The database password?')), 5_000, 'the secret question')
    expect(hidden.reply_markup).toBeUndefined()
    expect(hidden.text).toContain('Answer it in Locust on your computer.')
    expect(asked.reply_markup?.inline_keyboard.map((row) => row[0]!.text)).toEqual(['main', 'release'])
    expect(asked.text).toContain('Or reply to this message with an answer of your own.')
    // A reply in words to the secret one answers nothing.
    self.telegram.say('hunter2hunter2', { replyTo: hidden.message_id })
    await self.telegram.until(() => self.started.length === 2, 5_000, 'the reply taken as a message')
    expect(self.decided).toEqual([])
    self.telegram.press(asked.message_id, asked.reply_markup!.inline_keyboard[1]![0]!.callback_data)
    await self.telegram.until(() => self.decided.length === 1, 5_000, 'the option chosen')
    expect(self.decided[0]).toEqual({ approvalId: 'appr_q', answers: { q1: ['release'] } })
  })

  it('starts nothing from a message that waited while Locust was closed, and acts on each message once', async () => {
    const self = await paired()
    self.telegram.say('/ada deploy now', { date: Math.floor((self.clock - 20 * 60_000) / 1000) })
    await self.telegram.until((state) => state.sent.at(-1)?.text.includes('came while Locust was closed'), 5_000, 'the stale message said')
    expect(self.started).toEqual([])
    // The place in the bot's messages is kept before any is acted on, so a Locust opened again starts none twice.
    self.telegram.say('/ada once')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.link.dispose()
    const saved = JSON.parse(await readFile(join(self.directory, TELEGRAM_FILE), 'utf8')) as { offset: number }
    const again = createTelegramLink({
      directory: self.directory,
      secrets: { available: () => true, encrypt: (text) => Buffer.from([...Buffer.from(text, 'utf8')].map((byte) => byte ^ 0x5a)), decrypt: (cipher) => Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8') },
      refusal: () => undefined, computer: () => 'DESK-PC', team: async () => [ADA],
      start: async (input) => { self.started.push({ ...input }); return { ok: true, missionId: 'mission_again' } },
      read: async () => undefined, live: () => false, waiting: () => [], decide: async () => false,
      fetcher: self.telegram.fetch, api: () => 'http://127.0.0.1:1', now: () => self.clock, protectFile: async () => undefined, pollSeconds: 1
    })
    await again.load()
    await self.telegram.until((state) => state.polls > 3, 5_000, 'polling again')
    await new Promise((settle) => setTimeout(settle, 150))
    expect(saved.offset).toBeGreaterThan(0)
    expect(self.started).toHaveLength(1)
    // The turn that never ended is said to have stopped, once Locust is open again.
    expect(self.telegram.read().at(-1)).toContain('could not be read')
    await again.dispose()
  })

  it('stops at the organization’s word, and forgets the token and the chat on Remove', async () => {
    const self = await paired()
    self.refusal = 'Teammates in chat apps are off. Set by your organization.'
    self.telegram.say('/ada anything')
    await self.telegram.until((state) => state.sent.at(-1)?.text.includes('Teammates in chat apps are off'), 5_000, 'the refusal')
    expect(self.started).toEqual([])
    expect((await self.link.setToken(self.telegram.token)).message).toContain('Teammates in chat apps are off')
    self.refusal = undefined
    const removed = await self.link.remove()
    expect(removed).toEqual({ connected: false })
    await expect(readFile(join(self.directory, TELEGRAM_FILE), 'utf8')).rejects.toThrow()
  })

  it('sends only markup Telegram takes, and the plain words when it refuses some', async () => {
    const self = await paired()
    self.telegram.say('/ada show me')
    await self.telegram.until(() => self.started.length === 1, 5_000, 'the turn')
    const before = self.telegram.state.sent.length
    await self.end('mission_1', `${'A long line of findings. '.repeat(400)}\n\n| a | b |\n|---|---|\n| 1 | 2 |`)
    const parts = self.telegram.state.sent.slice(before)
    expect(parts.length).toBeGreaterThan(1)
    expect(parts[0]!.html.startsWith('<b>Ada</b>')).toBe(true)
    expect(parts.at(-1)!.html).toContain('<pre>| a | b |\n| 1 | 2 |</pre>')
    for (const part of parts) expect(htmlProblem(part.html)).toBeUndefined()
    expect(self.telegram.state.refused).toEqual([])
  })
})
