import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { afterEach, describe, expect, it } from 'vitest'

import { fakeDiscord, type FakeDiscord } from '../../../../_tools/fake-discord.mjs'
import type { ChatAppSettings } from '../shared/chat-apps.js'
import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import type { ChatLink, ChatMission, ChatTeammate } from './chat-link.js'
import { createDiscordLink, DISCORD_FILE } from './discord.js'

/**
 * TEAMMATES ANSWER THEIR PERSON IN DISCORD (0.832, discord.ts). Locust's link to the person's own bot, against a
 * stand-in for Discord's REST and gateway as strict as Discord on lengths, buttons and commands
 * (_tools/fake-discord.mjs): a token is checked and kept encrypted, one account pairs by the code sent to the bot,
 * anyone else is ignored, `/ada` from Discord's list reaches Ada and the answer comes back as a reply, and a card from
 * a turn started there is answered with Allow once or Deny -- by the window's own path, once.
 */
const ADA: ChatTeammate = { teammateId: 'mate_ada', name: 'Ada', role: 'Research & Briefs' }
const WREN: ChatTeammate = { teammateId: 'mate_wren', name: 'Wren', role: 'Code & Migrations' }
const STRANGER = { id: '1200000000000000999', username: 'mallory', global_name: 'Mallory' }
const STRANGER_CHANNEL = '1300000000000000999'

/** What Discord shows of escaped text: each mark without the backslash before it. */
const shown = (text: string): string => text.replace(/\\(.)/g, '$1')
const said = (text: string): readonly NormalizedRuntimeEvent[] =>
  [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text, final: true } } as unknown as NormalizedRuntimeEvent]

interface Harness {
  readonly discord: FakeDiscord
  readonly link: ChatLink
  readonly directory: string
  readonly started: { teammateId: string; text: string; followUpOf?: string }[]
  readonly decided: MissionApprovalAnswer[]
  readonly missions: Map<string, ChatMission>
  readonly live: Set<string>
  readonly changes: ChatAppSettings[]
  team: readonly ChatTeammate[]
  waiting: { request: MissionApprovalRequest; teammateId?: string }[]
  refusal?: string
  end(missionId: string, answer: string, phase?: string): Promise<void>
}

const harnesses: Harness[] = []
afterEach(async () => {
  for (const harness of harnesses.splice(0)) {
    await harness.link.dispose()
    await rm(harness.directory, { recursive: true, force: true })
  }
})

async function harness(options: { readonly team?: readonly ChatTeammate[]; readonly heartbeatMs?: number } = {}): Promise<Harness> {
  const directory = await mkdtemp(join(tmpdir(), 'locust-discord-'))
  const discord = fakeDiscord({ ...(options.heartbeatMs === undefined ? {} : { heartbeatMs: options.heartbeatMs }), channels: [STRANGER_CHANNEL] })
  let next = 0
  const self: Harness = {
    discord,
    directory,
    started: [],
    decided: [],
    missions: new Map(),
    live: new Set(),
    changes: [],
    team: options.team ?? [ADA, WREN],
    waiting: [],
    link: undefined as unknown as ChatLink,
    async end(missionId, answer, phase = 'completed') {
      self.live.delete(missionId)
      self.missions.set(missionId, { missionId, phase, events: said(answer) })
      await self.link.onRunEnded({ missionId })
    }
  }
  ;(self as { link: ChatLink }).link = createDiscordLink({
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
      if (waited) self.link.cardSettled(answer.approvalId, 'answers' in answer ? 'answered' : answer.decision === 'deny' ? 'denied' : 'allowed')
      return waited
    },
    changed: (settings) => { self.changes.push(settings) },
    fetcher: discord.fetch,
    socket: discord.WebSocket,
    api: () => 'http://127.0.0.1:1',
    protectFile: async () => undefined,
    typingMs: 30
  })
  harnesses.push(self)
  return self
}

/** A token kept, and the person's account paired by sending the code Settings shows. */
async function paired(options: Parameters<typeof harness>[0] = {}): Promise<Harness> {
  const self = await harness(options)
  const settings = await self.link.setToken(self.discord.token)
  await self.discord.until((state) => state.identified > 0, 5_000, 'the gateway identify')
  self.discord.say(settings.code!.code.replace(/^(\d{4})/, '$1 '))
  await self.discord.until(() => self.discord.read().some((text) => text.startsWith('Paired with Locust on **DESK-PC**')), 5_000, 'the pairing message')
  return self
}

const card = (approvalId: string, missionId: string, more: Partial<MissionApprovalRequest> = {}): MissionApprovalRequest => ({
  approvalId, runId: `run_${missionId}`, missionId, kind: 'command', summary: 'Run npm test in tides', detail: 'npm test', cwd: 'C:/work/tides', requestedAt: new Date().toISOString(), ...more
})

describe('teammates answer their person in Discord', () => {
  it('checks a token with Discord, keeps it encrypted, and reads only direct messages', async () => {
    const self = await harness()
    expect((await self.link.setToken('not a token')).message).toContain('not a bot’s token')
    expect((await self.link.setToken('MTEwMDAwMDAwMDAwMDAwMDAwMQ.Gwrong.wrong_wrong_wrong_wrong_wrong_wrong_wro')).message).toContain('Discord does not know that token')
    const kept = await self.link.setToken(`  Bot ${self.discord.token} `)
    expect(kept.bot).toMatchObject({ username: 'Locust test#4242', name: 'Locust test' })
    expect(kept.bot?.invite).toBe('https://discord.com/oauth2/authorize?client_id=1100000000000000001&scope=bot%20applications.commands&permissions=0')
    expect(kept.code?.code).toMatch(/^\d{8}$/)
    expect(kept.link).toBeUndefined()
    const file = await readFile(join(self.directory, DISCORD_FILE), 'utf8')
    expect(file).not.toContain(self.discord.token)
    expect(file).not.toContain(self.discord.token.split('.')[2])
    await expect.poll(async () => (await self.link.settings()).connected, { timeout: 3_000 }).toBe(true)
    // Direct messages only: no privileged intent, so the person turns nothing on in the Developer Portal.
    expect(self.discord.state.intents).toEqual([1 << 12])
  })

  it('pairs the one account that sends the code, once, and ignores everyone else', async () => {
    const self = await paired()
    expect((await self.link.settings()).chat).toMatchObject({ name: 'Sam', username: 'sam_here' })
    expect((await self.link.settings()).code).toBeUndefined()
    // Discord's list of commands: each teammate, then Locust's own -- in direct messages only, each taking a message.
    const commands = await self.discord.until((state) => state.commands.at(-1), 5_000, 'the commands')
    expect(commands.map((one) => one.name)).toEqual(['ada', 'wren', 'who', 'new', 'waiting', 'help'])
    expect(commands.every((one) => JSON.stringify(one.contexts) === '[1]')).toBe(true)
    expect(commands[0]!.options?.[0]).toMatchObject({ name: 'message', type: 3 })
    expect(commands.find((one) => one.name === 'who')!.options).toBeUndefined()

    // A stranger who shares a server with the bot: a wrong code is said once, and nothing they write starts anything.
    self.discord.say('1234 5678', { from: STRANGER, channelId: STRANGER_CHANNEL })
    self.discord.say('/ada delete everything', { from: STRANGER, channelId: STRANGER_CHANNEL })
    self.discord.say('Ada: and this', { from: STRANGER, channelId: self.discord.personChannel })
    await self.discord.until(() => self.discord.read(STRANGER_CHANNEL).length === 1, 5_000, 'the wrong code said')
    expect(self.discord.read(STRANGER_CHANNEL)[0]).toContain('That code does not pair this account')
    // A message in a server is not the person's direct message, even from the person.
    self.discord.say('/ada in a server', { guildId: '1400000000000000001', channelId: '1500000000000000001' })
    // Whatever channel it names: a message Discord says came from a server is never taken for a direct one.
    self.discord.say('Ada: from a server', { guildId: '1400000000000000001', channelId: self.discord.personChannel })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.started).toEqual([])
    expect((await self.link.settings()).chat).toMatchObject({ name: 'Sam' })
  })

  it('takes /ada from Discord’s list to Ada, answers as a reply, and carries the conversation on', async () => {
    const self = await paired()
    const asked = self.discord.command('ada', 'what changed in tides today?')
    await self.discord.until(() => self.started.length === 1, 5_000, 'the turn')
    expect(self.started[0]).toEqual({ teammateId: 'mate_ada', text: 'what changed in tides today?' })
    // Answered at once ("Locust is thinking"), then said who it went to, in that same message.
    expect(self.discord.state.callbacks).toContainEqual({ id: asked.id, kind: 2, type: 5 })
    const echo = await self.discord.until((state) => state.responses.find((one) => one.interaction === asked.id), 5_000, 'the command’s answer')
    expect(echo.content).toBe('**To Ada**: what changed in tides today?')
    // While Ada works, the bot shows typing.
    await self.discord.until((state) => state.typing.length > 0, 5_000, 'typing')
    await self.end('mission_1', '**Three** commits:\n\n- `fix: tides`\n- docs\n\n```ts\nconst a = 1 < 2\n```')
    const answer = self.discord.state.sent.at(-1)!
    expect(answer.reply_to).toBe(echo.id)
    expect(answer.content.startsWith('**Ada**\n\n**Three** commits:')).toBe(true)
    expect(answer.content).toContain('```ts\nconst a = 1 < 2\n```')
    expect(answer.allowed_mentions).toEqual({ parse: [] })
    expect(self.discord.state.refused).toEqual([])

    // A plain message: the same teammate, the same conversation.
    self.discord.say('and yesterday?')
    await self.discord.until(() => self.started.length === 2, 5_000, 'the next turn')
    expect(self.started[1]).toEqual({ teammateId: 'mate_ada', text: 'and yesterday?', followUpOf: 'mission_1' })
    await self.end('mission_2', 'Two more.')
    // A reply to the older answer carries THAT conversation on.
    self.discord.say('more on this', { replyTo: answer.id })
    await self.discord.until(() => self.started.length === 3, 5_000, 'the reply turn')
    expect(self.started[2]).toEqual({ teammateId: 'mate_ada', text: 'more on this', followUpOf: 'mission_1' })
    // A teammate's name and a colon writes to them, as /wren does.
    self.discord.say('Wren: run the tests')
    await self.discord.until(() => self.started.length === 4, 5_000, 'Wren’s turn')
    expect(self.started[3]).toEqual({ teammateId: 'mate_wren', text: 'run the tests' })
    // /new from the list: the next message starts a conversation of its own.
    const fresh = self.discord.command('new')
    await self.discord.until((state) => state.responses.find((one) => one.interaction === fresh.id), 5_000, '/new answered')
    expect(self.discord.state.responses.at(-1)!.content).toBe('Your next message starts a new conversation with Wren.')
  })

  it('makes a new code once too many wrong ones were sent to the bot, and the old one pairs nothing', async () => {
    const self = await harness()
    const settings = await self.link.setToken(self.discord.token)
    await self.discord.until((state) => state.identified > 0, 5_000, 'the gateway identify')
    const first = settings.code!.code
    const wrong = first === '00000000' ? '11111111' : '00000000'
    for (let tries = 0; tries < 10; tries += 1) self.discord.say(wrong, { from: STRANGER, channelId: STRANGER_CHANNEL })
    await expect.poll(async () => (await self.link.settings()).code?.code, { timeout: 3_000 }).not.toBe(first)
    expect((await self.link.settings()).message).toContain('Too many wrong codes were sent to the bot')
    // Said to the stranger once, not ten times.
    expect(self.discord.read(STRANGER_CHANNEL)).toHaveLength(1)
    self.discord.say(first)
    await new Promise((settle) => setTimeout(settle, 150))
    expect((await self.link.settings()).chat).toBeUndefined()
  })

  it('answers someone else’s command with nothing of the person’s', async () => {
    const self = await paired()
    const theirs = self.discord.command('ada', 'show me the files', { from: STRANGER, channelId: STRANGER_CHANNEL })
    const answered = await self.discord.until((state) => state.responses.find((one) => one.interaction === theirs.id), 5_000, 'the stranger answered')
    expect(answered.content).toBe('Locust answers only the person it is paired with.')
    expect(self.started).toEqual([])
  })

  it('asks who a message is for with buttons, when nobody is being written to', async () => {
    const self = await paired()
    self.discord.say('can someone look at the build?')
    const asked = await self.discord.until((state) => state.sent.find((one) => one.content === 'Who is this for?'), 5_000, 'the question')
    expect(asked.components[0]!.components.map((button) => button.label)).toEqual(['Ada', 'Wren'])
    self.discord.press(asked.id, asked.components[0]!.components[1]!.custom_id)
    await self.discord.until(() => self.started.length === 1, 5_000, 'the held message sent')
    expect(self.started[0]).toEqual({ teammateId: 'mate_wren', text: 'can someone look at the build?' })
    expect(self.discord.state.callbacks.at(-1)).toMatchObject({ kind: 3, type: 6 })
    expect(self.discord.state.edits.at(-1)).toMatchObject({ id: asked.id, content: 'To **Wren**.', components: [] })
  })

  it('brings a card from its own turn with Allow once and Deny, and answers it by the window’s path', async () => {
    const self = await paired()
    self.discord.command('wren', 'run the tests')
    await self.discord.until(() => self.started.length === 1, 5_000, 'the turn')
    const request = card('appr_1', 'mission_1', { summary: 'Run npm test --token=MTEwMDAwMDAwMDAwMDAwMDAwMQ.GabcDE.abcdefghijklmnopqrstuvwxyz0123456789ab in tides' })
    self.waiting = [{ request, teammateId: 'mate_wren' }]
    self.link.cardRaised(request, 'Wren')
    const posted = await self.discord.until((state) => state.sent.find((one) => one.content.startsWith('**Wren needs your approval**')), 5_000, 'the card')
    const buttons = posted.components[0]!.components
    expect(buttons.map((button) => [button.label, button.style])).toEqual([['Allow once', 3], ['Deny', 4]])
    // Nothing shaped like a key is sent to Discord's servers.
    expect(posted.content).not.toContain('MTEwMDAw')
    expect(shown(posted.content)).toContain('Run npm test --token=[secret-shaped text removed] in tides')
    // A card of a turn started in the window stays in the window.
    self.link.cardRaised(card('appr_window', 'mission_window'), 'Ada')
    await new Promise((settle) => setTimeout(settle, 100))
    expect(self.discord.state.sent.filter((one) => one.content.includes('needs your approval'))).toHaveLength(1)

    // A stranger's press does nothing.
    self.discord.press(posted.id, buttons[0]!.custom_id, { from: STRANGER, channelId: STRANGER_CHANNEL })
    await self.discord.until((state) => state.callbacks.filter((one) => one.kind === 3).length === 1, 5_000, 'the stranger’s press taken')
    await new Promise((settle) => setTimeout(settle, 100))
    expect(self.decided).toEqual([])
    // The person's press: Allow once, never Always, and the card says so and loses its buttons.
    self.discord.press(posted.id, buttons[0]!.custom_id)
    await self.discord.until(() => self.decided.length === 1, 5_000, 'the answer')
    expect(self.decided[0]).toEqual({ approvalId: 'appr_1', decision: 'approve-once' })
    const edited = await self.discord.until((state) => state.edits.find((one) => one.id === posted.id), 5_000, 'the card settled')
    expect(edited.content).toContain('**Allowed once.**')
    expect(edited.components).toEqual([])
    // Pressed again: answered already, nothing decided twice.
    self.discord.press(posted.id, buttons[1]!.custom_id)
    await self.discord.until((state) => state.callbacks.filter((one) => one.kind === 3).length === 3, 5_000, 'the second press taken')
    await new Promise((settle) => setTimeout(settle, 100))
    expect(self.decided).toHaveLength(1)
  })

  it('says which file or command a card allows, and posts a card once when its turn and the alert rules both bring it (0.837)', async () => {
    const self = await paired()
    self.discord.command('wren', 'make the notes')
    await self.discord.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.link.setAlerts(true)
    // A Claude Code card: its summary is the kind of action, its detail the exact file.
    const request = card('appr_file', 'mission_1', { kind: 'file-change', summary: 'Change 1 file', detail: 'notes/plan.md' })
    self.waiting = [{ request, teammateId: 'mate_wren' }]
    // From its own turn, and from the alert rules, at the same moment.
    self.link.cardRaised(request, 'Wren')
    self.link.alert({ cards: [{ request, name: 'Wren' }], lines: [] })
    const posted = await self.discord.until((state) => state.sent.find((one) => one.content.startsWith('**Wren needs your approval**')), 5_000, 'the card')
    expect(shown(posted.content)).toContain('Change 1 file\nnotes/plan.md')
    await new Promise((settle) => setTimeout(settle, 200))
    expect(self.discord.state.sent.filter((one) => one.content.startsWith('**Wren needs your approval**'))).toHaveLength(1)
    expect(JSON.parse(await readFile(join(self.directory, DISCORD_FILE), 'utf8')).cards.filter((one: { approvalId: string }) => one.approvalId === 'appr_file')).toHaveLength(1)
  })

  /*
   * WHAT NEEDS YOU, HERE (0.837). Off until the person turns it on. On, what the alert rules let through reaches the
   * chat: a card from a turn started in the window, with its buttons, once; and Scout's or a limit's line, who and what
   * kind only. Off again, nothing more.
   */
  it('tells the person here what needs them, from any run, only once they turn it on', async () => {
    const self = await paired()
    expect((await self.link.settings()).alerts).toBeUndefined()
    const request = card('appr_away', 'mission_window')
    self.waiting = [{ request }]
    const alert = { cards: [{ request, name: 'Ada' }], lines: [{ title: 'Scout', body: 'A check failed on a branch.' }] }
    self.link.alert(alert)
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.discord.state.sent.filter((one) => one.content.includes('needs your approval') || one.content.includes('Scout'))).toEqual([])

    const on = await self.link.setAlerts(true)
    expect(on.alerts).toBe(true)
    expect(JSON.parse(await readFile(join(self.directory, DISCORD_FILE), 'utf8')).alerts).toBe(true)
    self.link.alert(alert)
    const posted = await self.discord.until((state) => state.sent.find((one) => one.content.startsWith('**Ada needs your approval**')), 5_000, 'the card')
    expect(posted.components[0]!.components.map((button) => button.label)).toEqual(['Allow once', 'Deny'])
    const line = await self.discord.until((state) => state.sent.find((one) => one.content === '**Scout**\nA check failed on a branch.'), 5_000, 'Scout’s line')
    expect(line.components ?? []).toEqual([])
    // The same card told again is not sent twice.
    self.link.alert({ cards: [{ request, name: 'Ada' }], lines: [] })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.discord.state.sent.filter((one) => one.content.includes('needs your approval'))).toHaveLength(1)
    // Answered here by the window's own path, Allow once only.
    self.discord.press(posted.id, posted.components[0]!.components[0]!.custom_id)
    await self.discord.until(() => self.decided.length === 1, 5_000, 'the answer')
    expect(self.decided[0]).toEqual({ approvalId: 'appr_away', decision: 'approve-once' })

    expect((await self.link.setAlerts(false)).alerts).toBeUndefined()
    const next = card('appr_off', 'mission_window_2')
    self.waiting = [{ request: next }]
    self.link.alert({ cards: [{ request: next, name: 'Ada' }], lines: [{ title: 'Scout', body: 'There is a new issue.' }] })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.discord.state.sent.filter((one) => one.content.includes('needs your approval'))).toHaveLength(1)
    expect(self.discord.state.sent.some((one) => one.content.includes('There is a new issue.'))).toBe(false)
  })

  it('takes anything shaped like a key out of an answer before Discord has it', async () => {
    const self = await paired()
    self.discord.command('ada', 'the token?')
    await self.discord.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.end('mission_1', 'The bot token is MTEwMDAwMDAwMDAwMDAwMDAwMQ.GabcDE.abcdefghijklmnopqrstuvwxyz0123456789ab and the GitHub one is ghp_abcdefghijklmnopqrstuvwxyz0123456789.') // public-guard: allow (made-up tokens in a test)
    const sent = self.discord.state.sent.at(-1)!.content
    expect(sent).not.toContain('ghp_')
    expect(sent).not.toContain('MTEwMDAw')
    expect(sent).toContain('[secret-shaped text removed]')
  })

  it('splits a long answer into messages Discord takes, and a table into a block of code', async () => {
    const self = await paired()
    self.discord.command('ada', 'show me')
    await self.discord.until(() => self.started.length === 1, 5_000, 'the turn')
    const before = self.discord.state.sent.length
    await self.end('mission_1', `${'A long line of findings. '.repeat(200)}\n\n| a | b |\n|---|---|\n| 1 | 2 |`)
    const parts = self.discord.state.sent.slice(before)
    expect(parts.length).toBeGreaterThan(1)
    expect(parts[0]!.content.startsWith('**Ada**')).toBe(true)
    expect(parts.at(-1)!.content).toContain('```\n| a | b |\n| 1 | 2 |\n```')
    for (const part of parts) expect(part.content.length).toBeLessThanOrEqual(2000)
    // Only the first part is a reply; the rest follow it.
    expect(parts.slice(1).every((part) => part.reply_to === undefined)).toBe(true)
    expect(self.discord.state.refused).toEqual([])
  })

  it('beats as Discord asks, and resumes the same session when the connection drops', async () => {
    const self = await paired({ heartbeatMs: 120 })
    await self.discord.until((state) => state.heartbeats >= 2, 5_000, 'heartbeats')
    self.discord.drop()
    await self.discord.until((state) => state.resumed === 1, 5_000, 'the resume')
    expect(self.discord.state.identified).toBe(1)
    self.discord.say('Ada: still there?')
    await self.discord.until(() => self.started.length === 1, 5_000, 'a turn after the resume')
    // Discord asks for a new connection: it resumes again, without a second identify.
    self.discord.reconnect()
    await self.discord.until((state) => state.resumed === 2, 5_000, 'the second resume')
    expect(self.discord.state.identified).toBe(1)
  })

  it('stops when Discord refuses the token, and forgets the token and the commands on Remove', async () => {
    const self = await paired()
    self.discord.drop(4004)
    await expect.poll(async () => (await self.link.settings()).message ?? '', { timeout: 3_000 }).toContain('Discord no longer takes this bot’s token')
    expect((await self.link.settings()).connected).toBe(false)
    const removed = await self.link.remove()
    expect(removed).toEqual({ connected: false })
    await expect(readFile(join(self.directory, DISCORD_FILE), 'utf8')).rejects.toThrow()
    // The bot's commands go with it.
    await self.discord.until((state) => state.commands.at(-1)?.length === 0, 5_000, 'the commands cleared')
  })

  it('stops at the organization’s word', async () => {
    const self = await paired()
    self.refusal = 'Teammates in chat apps are off. Set by your organization.'
    self.discord.say('Ada: anything')
    await self.discord.until(() => self.discord.read().at(-1)?.includes('Teammates in chat apps are off'), 5_000, 'the refusal')
    expect(self.started).toEqual([])
    expect((await self.link.setToken(self.discord.token)).message).toContain('Teammates in chat apps are off')
  })
})
