import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { afterEach, describe, expect, it } from 'vitest'

import { fakeSlack, type FakeSlack } from '../../../../_tools/fake-slack.mjs'
import type { ChatAppSettings } from '../shared/chat-apps.js'
import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import type { ChatLink, ChatMission, ChatTeammate } from './chat-link.js'
import { createSlackLink, SLACK_FILE } from './slack.js'

/**
 * TEAMMATES ANSWER THEIR PERSON IN SLACK (0.832, slack.ts). Locust's link to the person's own Slack app, against a
 * stand-in for Slack's Web API and Socket Mode as strict as Slack on tokens, limits and threads
 * (_tools/fake-slack.mjs): both tokens are checked and kept encrypted, one account pairs by the code sent to the app,
 * anyone else is ignored, "Ada:" reaches Ada and the answer comes back -- in the thread when asked in one -- and a card
 * from a turn started there is answered with Allow once or Deny, by the window's own path, once.
 */
const ADA: ChatTeammate = { teammateId: 'mate_ada', name: 'Ada', role: 'Research & Briefs' }
const WREN: ChatTeammate = { teammateId: 'mate_wren', name: 'Wren', role: 'Code & Migrations' }
const STRANGER = { id: 'U0MAL00001', name: 'mallory', real_name: 'Mallory', profile: { display_name: 'Mallory' } }

/** A made-up Slack token, put together here so no file holds one whole for a secret scanner to take for real. */
const slackShaped = (...parts: string[]): string => parts.join('-')
const said = (text: string): readonly NormalizedRuntimeEvent[] =>
  [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text, final: true } } as unknown as NormalizedRuntimeEvent]

interface Harness {
  readonly slack: FakeSlack
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
  tokens(): { bot: string; app: string }
  end(missionId: string, answer: string, phase?: string): Promise<void>
}

const harnesses: Harness[] = []
afterEach(async () => {
  for (const harness of harnesses.splice(0)) {
    await harness.link.dispose()
    await rm(harness.directory, { recursive: true, force: true })
  }
})

async function harness(options: { readonly team?: readonly ChatTeammate[] } = {}): Promise<Harness> {
  const directory = await mkdtemp(join(tmpdir(), 'locust-slack-'))
  const slack = fakeSlack()
  let next = 0
  const self: Harness = {
    slack,
    directory,
    started: [],
    decided: [],
    missions: new Map(),
    live: new Set(),
    changes: [],
    team: options.team ?? [ADA, WREN],
    waiting: [],
    link: undefined as unknown as ChatLink,
    tokens: () => ({ bot: slack.botToken, app: slack.appToken }),
    async end(missionId, answer, phase = 'completed') {
      self.live.delete(missionId)
      self.missions.set(missionId, { missionId, phase, events: said(answer) })
      await self.link.onRunEnded({ missionId })
    }
  }
  ;(self as { link: ChatLink }).link = createSlackLink({
    directory,
    // A stand-in for the operating system's encryption: reversible, and never the tokens in the clear.
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
    fetcher: slack.fetch,
    socket: slack.WebSocket,
    api: () => 'http://127.0.0.1:1',
    protectFile: async () => undefined
  })
  harnesses.push(self)
  return self
}

/** Both tokens kept, and the person's account paired by sending the app the code Settings shows. */
async function paired(options: Parameters<typeof harness>[0] = {}): Promise<Harness> {
  const self = await harness(options)
  const settings = await self.link.setToken(self.tokens())
  await self.slack.until((state) => state.hellos > 0, 5_000, 'the connection')
  self.slack.say(settings.code!.code)
  await self.slack.until(() => self.slack.read().some((text) => text.startsWith('Paired with Locust on *DESK-PC*')), 5_000, 'the pairing message')
  return self
}

const card = (approvalId: string, missionId: string, more: Partial<MissionApprovalRequest> = {}): MissionApprovalRequest => ({
  approvalId, runId: `run_${missionId}`, missionId, kind: 'command', summary: 'Run npm test in tides', detail: 'npm test', cwd: 'C:/work/tides', requestedAt: new Date().toISOString(), ...more
})

describe('teammates answer their person in Slack', () => {
  it('checks both tokens with Slack, keeps them encrypted, and says which one is wrong', async () => {
    const self = await harness()
    const { bot, app } = self.tokens()
    expect((await self.link.setToken({ bot: 'not a token', app })).message).toContain('not the bot’s token')
    expect((await self.link.setToken({ bot: app, app: bot })).message).toContain('in each other’s places')
    expect((await self.link.setToken({ bot, app: 'xapp-nope' })).message).toContain('not the app’s own token')
    expect((await self.link.setToken({ bot: `${bot}0`, app })).message).toContain('Slack does not know that bot token')
    expect((await self.link.setToken({ bot, app: `${app}0` })).message).toContain('Slack does not take that app token')
    const kept = await self.link.setToken({ bot: ` ${bot} `, app: `${app}\n` })
    expect(kept.bot).toEqual({ username: 'locust', name: 'locust', team: 'Locust Test', open: 'https://slack.com/app_redirect?app=A0LOCUST01&team=T0LOCUST01' })
    expect(kept.code?.code).toMatch(/^\d{8}$/)
    const file = await readFile(join(self.directory, SLACK_FILE), 'utf8')
    expect(file).not.toContain(bot)
    expect(file).not.toContain(app)
    expect(file).not.toContain('xox')
    await expect.poll(async () => (await self.link.settings()).connected, { timeout: 3_000 }).toBe(true)
  })

  it('pairs the one account that sends the code, names it, and ignores everyone else', async () => {
    const self = await paired()
    expect((await self.link.settings()).chat).toMatchObject({ name: 'Sam', username: 'sam' })
    expect(self.slack.read()[0]).toContain('*Locust on DESK-PC*')
    expect(self.slack.read()[0]).toContain('“Ada:” and a message writes to Ada.')
    // A stranger in the workspace: a wrong code is said once, and nothing they write starts anything.
    self.slack.say('1234 5678', { from: STRANGER })
    self.slack.say('Ada: delete everything', { from: STRANGER })
    self.slack.say('Ada: and this', { from: STRANGER, channel: self.slack.personChannel })
    await self.slack.until(() => self.slack.read('D0MAL00001').length === 1, 5_000, 'the wrong code said')
    expect(self.slack.read('D0MAL00001')[0]).toContain('That code does not pair this account')
    // A message in a channel, an app's own message and an edit are not the person's direct message.
    self.slack.say('Ada: in a channel', { channel: 'C0GENERAL1', channelType: 'channel' })
    self.slack.say('Ada: from a bot', { botId: 'B0OTHER001' })
    self.slack.say('Ada: an edit', { subtype: 'message_changed' })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.started).toEqual([])
  })

  it('takes “Ada:” to Ada, answers in the chat, and carries a conversation on in its thread', async () => {
    const self = await paired()
    const asked = self.slack.say('Ada: what changed in tides today?')
    await self.slack.until(() => self.started.length === 1, 5_000, 'the turn')
    expect(self.started[0]).toEqual({ teammateId: 'mate_ada', text: 'what changed in tides today?' })
    // While Ada works, her person's message is marked.
    await self.slack.until((state) => state.reactions.some((one) => one.ts === asked && one.name === 'eyes' && one.on), 5_000, 'the mark')
    await self.end('mission_1', '**Three** commits:\n\n- `fix: tides`\n- docs <b>\n\n```ts\nconst a = 1 < 2\n```')
    const answer = self.slack.state.sent.at(-1)!
    expect(answer.thread_ts).toBeUndefined()
    expect(answer.text.startsWith('*Ada*\n\n*Three* commits:\n\n• `fix: tides`\n• docs &lt;b&gt;')).toBe(true)
    expect(answer.text).toContain('```\nconst a = 1 &lt; 2\n```')
    expect(self.slack.state.reactions.find((one) => one.ts === asked)?.on).toBe(false)
    expect(self.slack.state.refused).toEqual([])

    // A message with no name: the same teammate, the same conversation.
    self.slack.say('and yesterday?')
    await self.slack.until(() => self.started.length === 2, 5_000, 'the next turn')
    expect(self.started[1]).toEqual({ teammateId: 'mate_ada', text: 'and yesterday?', followUpOf: 'mission_1' })
    await self.end('mission_2', 'Two more.')
    // A reply in the first answer's thread carries THAT conversation on, and is answered in the thread.
    self.slack.say('more on this', { thread: answer.ts })
    await self.slack.until(() => self.started.length === 3, 5_000, 'the thread turn')
    expect(self.started[2]).toEqual({ teammateId: 'mate_ada', text: 'more on this', followUpOf: 'mission_1' })
    await self.end('mission_3', 'More, here.')
    expect(self.slack.state.sent.at(-1)).toMatchObject({ thread_ts: answer.ts })
    // "new" on its own: the next message starts a conversation of its own.
    self.slack.say('new')
    await self.slack.until(() => self.slack.read().at(-1)?.includes('starts a new conversation with Ada'), 5_000, 'new said')
    self.slack.say('start over')
    await self.slack.until(() => self.started.length === 4, 5_000, 'a fresh turn')
    expect(self.started[3]).toEqual({ teammateId: 'mate_ada', text: 'start over' })
    // "who" lists the team.
    self.slack.say('who')
    await self.slack.until(() => self.slack.read().at(-1)?.startsWith('*Your teammates*'), 5_000, 'who said')
    expect(self.slack.read().at(-1)).toContain('*Wren*, Code &amp; Migrations')
  })

  it('acknowledges every envelope at once, and acts once on a message Slack sends twice', async () => {
    const self = await paired()
    self.slack.say('Ada: once')
    await self.slack.until(() => self.started.length === 1, 5_000, 'the turn')
    self.slack.again(self.slack.state.envelopes.at(-1)!)
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.started).toHaveLength(1)
    expect(self.slack.state.envelopes.every((one) => self.slack.state.acked.has(one.envelope_id))).toBe(true)
  })

  it('asks who a message is for with buttons, when nobody is being written to', async () => {
    const self = await paired()
    self.slack.say('can someone look at the build?')
    const asked = await self.slack.until((state) => state.sent.find((one) => one.text === 'Who is this for?'), 5_000, 'the question')
    const buttons = asked.blocks.find((block) => block.type === 'actions')!.elements!
    expect(buttons.map((button) => button.text.text)).toEqual(['Ada', 'Wren'])
    self.slack.press(asked.ts, buttons[1]!.action_id)
    await self.slack.until(() => self.started.length === 1, 5_000, 'the held message sent')
    expect(self.started[0]).toEqual({ teammateId: 'mate_wren', text: 'can someone look at the build?' })
    expect(self.slack.state.edits.at(-1)).toMatchObject({ ts: asked.ts, text: 'To *Wren*.', blocks: [] })
  })

  it('brings a card from its own turn with Allow once and Deny, and answers it by the window’s path', async () => {
    const self = await paired()
    self.slack.say('Wren: run the tests')
    await self.slack.until(() => self.started.length === 1, 5_000, 'the turn')
    const request = card('appr_1', 'mission_1', { summary: `Run npm test --token=${slackShaped('xoxb', '1000000000001', '2000000000002', 'abcdef'.repeat(3))} in tides` })
    self.waiting = [{ request, teammateId: 'mate_wren' }]
    self.link.cardRaised(request, 'Wren')
    const posted = await self.slack.until((state) => state.sent.find((one) => one.text.startsWith('*Wren needs your approval*')), 5_000, 'the card')
    const buttons = posted.blocks.find((block) => block.type === 'actions')!.elements!
    expect(buttons.map((button) => [button.text.text, button.style])).toEqual([['Allow once', 'primary'], ['Deny', 'danger']])
    expect(posted.text).not.toContain('xoxb-')
    expect(posted.text).toContain('[secret-shaped text removed]')
    // A stranger's press does nothing.
    self.slack.press(posted.ts, buttons[0]!.action_id, { from: STRANGER, channel: 'D0MAL00001' })
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.decided).toEqual([])
    // The person's press: Allow once, never Always, and the card says so and loses its buttons.
    self.slack.press(posted.ts, buttons[0]!.action_id)
    await self.slack.until(() => self.decided.length === 1, 5_000, 'the answer')
    expect(self.decided[0]).toEqual({ approvalId: 'appr_1', decision: 'approve-once' })
    const edited = await self.slack.until((state) => state.edits.find((one) => one.ts === posted.ts), 5_000, 'the card settled')
    expect(edited.text).toContain('*Allowed once.*')
    expect(edited.blocks).toEqual([])
    self.slack.press(posted.ts, buttons[1]!.action_id)
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.decided).toHaveLength(1)
  })

  it('tells the person here what needs them from any run, once they turn it on (0.837)', async () => {
    const self = await paired()
    const request = card('appr_away', 'mission_window')
    self.waiting = [{ request }]
    const alert = { cards: [{ request, name: 'Ada' }], lines: [{ title: 'Scout', body: 'One of your pull requests needs you.' }] }
    self.link.alert(alert)
    await new Promise((settle) => setTimeout(settle, 150))
    expect(self.slack.state.sent.some((one) => one.text.includes('needs your approval') || one.text.includes('pull requests'))).toBe(false)
    expect((await self.link.setAlerts(true)).alerts).toBe(true)
    self.link.alert(alert)
    const posted = await self.slack.until((state) => state.sent.find((one) => one.text.startsWith('*Ada needs your approval*')), 5_000, 'the card')
    expect(posted.blocks.find((block) => block.type === 'actions')!.elements!.map((button) => button.text.text)).toEqual(['Allow once', 'Deny'])
    await self.slack.until((state) => state.sent.find((one) => one.text === '*Scout*\nOne of your pull requests needs you.'), 5_000, 'Scout’s line')
  })

  it('takes anything shaped like a key out of an answer before Slack has it', async () => {
    const self = await paired()
    self.slack.say('Ada: the tokens?')
    await self.slack.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.end('mission_1', `The app token is ${slackShaped('xapp', '1', 'A0LOCUST01', '3000000000003', 'abcdef'.repeat(3))} and the bot one is ${slackShaped('xoxb', '1000000000001', '2000000000002', 'abcdef'.repeat(3))}.`)
    const sent = self.slack.state.sent.at(-1)!.text
    expect(sent).not.toContain('xapp-')
    expect(sent).not.toContain('xoxb-')
    expect(sent).toContain('[secret-shaped text removed]')
  })

  it('splits a long answer into messages Slack takes, all in the thread it was asked in', async () => {
    const self = await paired()
    const root = self.slack.say('Ada: start')
    await self.slack.until(() => self.started.length === 1, 5_000, 'the turn')
    await self.end('mission_1', 'Started.')
    self.slack.say('show me everything', { thread: root })
    await self.slack.until(() => self.started.length === 2, 5_000, 'the thread turn')
    const before = self.slack.state.sent.length
    await self.end('mission_2', `${'A long line of findings. '.repeat(400)}\n\n| a | b |\n|---|---|\n| 1 | 2 |`)
    const parts = self.slack.state.sent.slice(before)
    expect(parts.length).toBeGreaterThan(1)
    expect(parts[0]!.text.startsWith('*Ada*')).toBe(true)
    expect(parts.at(-1)!.text).toContain('```\n| a | b |\n| 1 | 2 |\n```')
    for (const part of parts) {
      expect(part.text.length).toBeLessThanOrEqual(4_000)
      expect(part.thread_ts).toBe(root)
    }
    expect(self.slack.state.refused).toEqual([])
  })

  it('moves to a new connection when Slack refreshes it, and stops when Socket Mode is turned off', async () => {
    const self = await paired()
    self.slack.disconnect('refresh_requested')
    await self.slack.until((state) => state.hellos === 2, 5_000, 'the new connection')
    self.slack.say('Ada: still there?')
    await self.slack.until(() => self.started.length === 1, 5_000, 'a turn on the new connection')
    self.slack.disconnect('link_disabled')
    await expect.poll(async () => (await self.link.settings()).message ?? '', { timeout: 3_000 }).toContain('Socket Mode is off')
    expect((await self.link.settings()).connected).toBe(false)
  })

  it('stops at the organization’s word, and forgets the tokens and the chat on Remove', async () => {
    const self = await paired()
    self.refusal = 'Teammates in chat apps are off. Set by your organization.'
    self.slack.say('Ada: anything')
    await self.slack.until(() => self.slack.read().at(-1)?.includes('Teammates in chat apps are off'), 5_000, 'the refusal')
    expect(self.started).toEqual([])
    self.refusal = undefined
    expect(await self.link.remove()).toEqual({ connected: false })
    await expect(readFile(join(self.directory, SLACK_FILE), 'utf8')).rejects.toThrow()
  })
})
