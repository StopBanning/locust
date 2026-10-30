import type { AppServerRunProcess as AppServerProcess } from '@teammate/runtime-adapters'
import { describe, expect, it, vi } from 'vitest'

import { voicePreviewRules, type VoiceAddOn, type VoiceCallEvent } from '../shared/voice-call.js'
import { createVoiceCalls, type VoiceCallDeps } from './voice-call.js'

// The window's folder, as the machine running the test spells it: the call names it by this machine's basename.
const ACME = process.platform === 'win32' ? 'C:\\work\\acme' : '/work/acme'

/*
 * A VOICE CALL, PLACED BY CODEX (0.737; 0.738 after Sol's review). Against a stand-in for `codex app-server`
 * that answers the way Codex 0.162.1 did for Colin's Plus sign-in: the call is asked for on a read-only,
 * ephemeral thread with every add-on off, over WebRTC on realtime v3, and the voice service's answer is
 * handed back; the lines said come through; an approval is refused; End stops it and the server. And the
 * review's lifecycle findings: one start at a time, an End during a start reaches it, an answer for a call
 * already ended is not a call, and End does not wait on a Codex that never answers.
 */
interface Asked { readonly method: string; readonly params: Record<string, unknown> }
interface FakeAnswers {
  readonly account?: unknown
  readonly realtime?: 'answer' | 'error' | 'answer-then-closed'
  readonly config?: 'ok' | 'fail'
  readonly answersStop?: boolean
}

function fakeCodex(answers: FakeAnswers = {}) {
  const asked: Asked[] = []
  let onData: (chunk: string) => void = () => undefined
  let onExit: () => void = () => undefined
  let killed = false
  const say = (message: unknown): void => { setTimeout(() => onData(`${JSON.stringify({ jsonrpc: '2.0', ...(message as object) })}\n`), 0) }
  const result = (method: string): unknown => {
    switch (method) {
      case 'initialize': return { userAgent: 'codex' }
      case 'account/read': return answers.account ?? { account: { type: 'chatgpt', planType: 'plus' }, requiresOpenaiAuth: true }
      case 'config/read': return answers.config === 'fail' ? undefined : { config: { model: 'gpt-6-sol', mcp_servers: { robinhood: {} } } }
      case 'thread/start': return { thread: { id: 'thread_1' } }
      case 'thread/realtime/start':
        if (answers.realtime === 'error') say({ method: 'thread/realtime/error', params: { threadId: 'thread_1', message: 'realtime conversation requires API key auth' } })
        else {
          say({ method: 'thread/realtime/started', params: { threadId: 'thread_1', version: 'v3' } })
          say({ method: 'thread/realtime/sdp', params: { threadId: 'thread_1', sdp: 'v=0\r\nanswer' } })
          if (answers.realtime === 'answer-then-closed') say({ method: 'thread/realtime/closed', params: { threadId: 'thread_1', reason: 'transport' } })
        }
        return {}
      default: return {}
    }
  }
  const child: AppServerProcess = {
    write: (line) => {
      const message = JSON.parse(line) as { id?: number; method?: string; params?: Record<string, unknown>; result?: unknown }
      if (message.method === undefined) { asked.push({ method: `answer:${String(message.id)}`, params: message.result as Record<string, unknown> }); return }
      asked.push({ method: message.method, params: message.params ?? {} })
      if (message.id === undefined) return
      if (message.method === 'config/read' && answers.config === 'fail') { say({ id: message.id, error: { code: -32000, message: 'no config' } }); return }
      if (message.method === 'thread/realtime/stop' && answers.answersStop === false) return
      say({ id: message.id, result: result(message.method) })
    },
    kill: () => { killed = true; setTimeout(() => onExit(), 0) },
    onData: (listener) => { onData = listener },
    onExit: (listener) => { onExit = listener }
  }
  return { child, asked, killed: () => killed, say }
}

const NO_TOOLS = { 'features.shell_tool': false, 'features.browser_use': false, 'features.browser_use_external': false, 'features.in_app_browser': false, 'features.computer_use': false } as const
const CODEX = { commandName: 'codex', discoveredPath: 'codex', executablePath: 'codex.exe', prefixArgs: [] as string[], kind: 'native' as const }

function calls(fakes: () => ReturnType<typeof fakeCodex>, options: {
  readonly folder?: string | null
  readonly codex?: () => Promise<typeof CODEX | undefined>
  readonly addOn?: { readonly open: () => Promise<VoiceAddOn | undefined>; readonly close: () => Promise<void> }
  readonly connectors?: boolean
  readonly askWaitMs?: number
  readonly voice?: () => Promise<string | undefined>
  readonly teammate?: VoiceCallDeps['teammate']
  readonly onTarget?: VoiceCallDeps['onTarget']
} = {}) {
  const sent: VoiceCallEvent[] = []
  const spawned: ReturnType<typeof fakeCodex>[] = []
  const voice = createVoiceCalls({
    codex: options.codex ?? (async () => CODEX),
    spawn: () => { const fake = fakes(); spawned.push(fake); return fake.child },
    folder: () => (options.folder === null ? undefined : options.folder ?? ACME),
    send: (event) => sent.push(event),
    version: '0.738.0',
    answerTimeoutMs: 2_000,
    stopWaitMs: 50,
    ...(options.addOn === undefined ? {} : { addOn: options.addOn }),
    ...(options.connectors === undefined ? {} : { connectors: options.connectors }),
    ...(options.askWaitMs === undefined ? {} : { askWaitMs: options.askWaitMs }),
    ...(options.voice === undefined ? {} : { voice: options.voice }),
    ...(options.teammate === undefined ? {} : { teammate: options.teammate }),
    ...(options.onTarget === undefined ? {} : { onTarget: options.onTarget })
  })
  return { voice, sent, spawned }
}
const one = (answers: FakeAnswers = {}) => {
  const fake = fakeCodex(answers)
  return { fake, ...calls(() => fake) }
}
const by = (fake: ReturnType<typeof fakeCodex>, method: string): Record<string, unknown> => fake.asked.find((entry) => entry.method === method)!.params
const settle = (ms = 20) => new Promise((resolve) => setTimeout(resolve, ms))

describe('a voice call', () => {
  it('is asked of Codex on a read-only thread with every add-on off, and the answer comes back', async () => {
    const { fake, voice, spawned } = one()
    expect(await voice.start('v=0\r\noffer')).toEqual({ ok: true, answer: 'v=0\r\nanswer' })
    expect(spawned).toHaveLength(1)
    expect(by(fake, 'initialize')).toMatchObject({ capabilities: { experimentalApi: true } })
    expect(by(fake, 'thread/start')).toEqual({
      cwd: ACME,
      sandbox: 'read-only',
      approvalPolicy: 'untrusted',
      ephemeral: true,
      config: { 'features.apps': false, 'features.plugins': false, 'features.hooks': false, notify: [], ...NO_TOOLS, 'mcp_servers.robinhood.enabled': false }
    })
    expect(by(fake, 'thread/realtime/start')).toMatchObject({ threadId: 'thread_1', outputModality: 'audio', transport: { type: 'webrtc', sdp: 'v=0\r\noffer' }, version: 'v3' })
    expect(String(by(fake, 'thread/realtime/start').realtimeStartInstructions)).toContain('"acme"')
    // The account is checked and the settings read before any thread exists -- and the add-on list, which
    // starts every server to list its tools, is never asked for (Sol's #1).
    const order = fake.asked.map((entry) => entry.method).filter((method) => !method.startsWith('answer:'))
    expect(order.slice(0, 5)).toEqual(['initialize', 'initialized', 'account/read', 'config/read', 'thread/start'])
    expect(order).not.toContain('mcpServerStatus/list')
  })

  it('says what is said, refuses whatever Codex asks to do, and ends with End', async () => {
    const { fake, voice, sent } = one()
    await voice.start('v=0\r\noffer')
    fake.say({ method: 'thread/realtime/transcript/delta', params: { threadId: 'thread_1', role: 'user', delta: 'What is two' } })
    fake.say({ method: 'thread/realtime/transcript/done', params: { threadId: 'thread_1', role: 'user', text: 'What is two plus two' } })
    fake.say({ method: 'thread/realtime/transcript/done', params: { threadId: 'thread_1', role: 'assistant', text: 'That’s four.' } })
    fake.say({ method: 'turn/started', params: { threadId: 'thread_1' } })
    fake.say({ id: 99, method: 'item/commandExecution/requestApproval', params: { threadId: 'thread_1', command: 'rm -rf build' } })
    await settle()
    expect(sent).toEqual([
      { kind: 'partial', role: 'you', delta: 'What is two' },
      { kind: 'said', role: 'you', text: 'What is two plus two' },
      { kind: 'said', role: 'voice', text: 'That’s four.' },
      { kind: 'working', working: true }
    ])
    expect(fake.asked).toContainEqual({ method: 'answer:99', params: { decision: 'decline' } })
    await voice.stop()
    expect(fake.asked.map((entry) => entry.method)).toContain('thread/realtime/stop')
    expect(fake.killed()).toBe(true)
    expect(sent.at(-1)).toEqual({ kind: 'ended' })
  })

  it('is not placed on an API key, and says how to fix it', async () => {
    const { fake, voice } = one({ account: { account: { type: 'apiKey' }, requiresOpenaiAuth: true } })
    expect(await voice.start('v=0\r\noffer')).toEqual({ ok: false, message: 'Codex is signed in with an API key. Voice calls use your ChatGPT sign-in: sign Codex in with ChatGPT and try again.' })
    expect(fake.asked.map((entry) => entry.method)).not.toContain('thread/start')
    expect(fake.killed()).toBe(true)
  })

  it('is not placed when the settings cannot be read: nothing is left on by accident (Sol’s #2)', async () => {
    const { fake, voice } = one({ config: 'fail' })
    const started = await voice.start('v=0\r\noffer')
    expect(started).toEqual({ ok: false, message: 'Locust could not read your Codex settings, so it did not place the call. Try again.' })
    expect(fake.asked.map((entry) => entry.method)).not.toContain('thread/start')
    expect(fake.killed()).toBe(true)
  })

  it('says the voice service’s refusal in words, and stops the server', async () => {
    const { fake, voice } = one({ realtime: 'error' })
    const started = await voice.start('v=0\r\noffer')
    expect(started.ok === false && started.message).toMatch(/sign Codex in with ChatGPT/)
    expect(fake.killed()).toBe(true)
  })

  it('needs a folder, and Codex', async () => {
    expect(await one().voice.available()).toEqual({ available: true })
    expect(await calls(() => fakeCodex(), { folder: null }).voice.available()).toEqual({ available: false, why: 'Pick a folder first.' })
    const none = calls(() => fakeCodex(), { codex: async () => undefined })
    expect((await none.voice.available()).available).toBe(false)
    expect((await none.voice.start('v=0\r\n')).ok).toBe(false)
    expect(none.spawned).toHaveLength(0)
  })
})

describe('a voice call’s starts and ends (Sol’s review of 0.737)', () => {
  it('runs one start at a time: a second press ends the first before it spawns anything (#6)', async () => {
    let release: (value: typeof CODEX) => void = () => undefined
    const slow = new Promise<typeof CODEX>((resolve) => { release = resolve })
    const { voice, spawned } = calls(() => fakeCodex(), { codex: () => slow })
    const first = voice.start('v=0\r\nfirst')
    const second = voice.start('v=0\r\nsecond')
    release(CODEX)
    const [a, b] = await Promise.all([first, second])
    expect(a).toEqual({ ok: false, message: 'The call was ended before it began.' })
    expect(b).toEqual({ ok: true, answer: 'v=0\r\nanswer' })
    expect(spawned).toHaveLength(1)
  })

  it('reaches a start still finding Codex: End then spawns nothing (#6)', async () => {
    let release: (value: typeof CODEX) => void = () => undefined
    const slow = new Promise<typeof CODEX>((resolve) => { release = resolve })
    const { voice, spawned } = calls(() => fakeCodex(), { codex: () => slow })
    const starting = voice.start('v=0\r\noffer')
    await voice.stop()
    release(CODEX)
    expect(await starting).toEqual({ ok: false, message: 'The call was ended before it began.' })
    expect(spawned).toHaveLength(0)
  })

  it('reaches a start already talking to Codex: End stops it there and the server goes (#6)', async () => {
    const { fake, voice } = one()
    const starting = voice.start('v=0\r\noffer')
    await settle(1)
    await voice.stop()
    expect((await starting).ok).toBe(false)
    expect(fake.killed()).toBe(true)
  })

  it('hands back no answer for a call that closed as it came (#11)', async () => {
    const { fake, voice } = one({ realtime: 'answer-then-closed' })
    const started = await voice.start('v=0\r\noffer')
    expect(started.ok).toBe(false)
    expect(fake.killed()).toBe(true)
  })

  it('does not wait on a Codex that never says it stopped (#12)', async () => {
    const { fake, voice } = one({ answersStop: false })
    await voice.start('v=0\r\noffer')
    const at = Date.now()
    await voice.stop()
    expect(Date.now() - at).toBeLessThan(1_000)
    expect(fake.killed()).toBe(true)
  })

  it('ends at once when the app quits', async () => {
    const { fake, voice, sent } = one({ answersStop: false })
    await voice.start('v=0\r\noffer')
    voice.dispose()
    expect(fake.killed()).toBe(true)
    expect(sent.at(-1)).toEqual({ kind: 'ended' })
  })
})

/*
 * HANDING WORK TO TEAMMATES (0.739). The call's Codex gets one add-on, Locust's own (a private copy of its
 * add-on server, only the hand-off tools), and Codex's question before using it -- `mcpServer/elicitation/
 * request`, `codex_approval_kind: mcp_tool_call`, measured 2026-10-11 -- is accepted for that server alone.
 */
describe('a voice call’s hand-offs', () => {
  const ADD_ON: VoiceAddOn = { command: 'Locust.exe', args: ['locust-mcp-bridge.mjs', 'voice-call-mcp/locust-mcp-connection.json'], env: { ELECTRON_RUN_AS_NODE: '1' } }
  const withAddOn = (open: () => Promise<VoiceAddOn | undefined> = async () => ADD_ON) => {
    const fake = fakeCodex()
    const closed: number[] = []
    const made = calls(() => fake, { addOn: { open, close: async () => { closed.push(1) } } })
    return { fake, closed, ...made }
  }
  const asks = (fake: ReturnType<typeof fakeCodex>, id: number, serverName: string, kind = 'mcp_tool_call'): void =>
    fake.say({ id, method: 'mcpServer/elicitation/request', params: { threadId: 'thread_1', serverName, mode: 'form', _meta: { codex_approval_kind: kind }, message: `Allow the ${serverName} MCP server to run tool "start_conversation"?` } })

  it('gives the call’s thread Locust’s own add-on, under the call’s own name, and tells Codex how to hand over', async () => {
    const { fake, voice } = withAddOn()
    expect((await voice.start('v=0\r\noffer')).ok).toBe(true)
    expect(by(fake, 'thread/start').config).toMatchObject({
      'features.plugins': false,
      'mcp_servers.locust_call.command': 'Locust.exe',
      'mcp_servers.locust_call.args': ['locust-mcp-bridge.mjs', 'voice-call-mcp/locust-mcp-connection.json'],
      'mcp_servers.locust_call.env': { ELECTRON_RUN_AS_NODE: '1' },
      'mcp_servers.locust_call.enabled': true
    })
    const told = String(by(fake, 'thread/realtime/start').realtimeStartInstructions)
    expect(told).toContain('locust_call tools')
    expect(told).toContain('wait for them to say yes')
  })

  it('lets Codex use that add-on and nothing else, and says a hand-off in the strip', async () => {
    const { fake, voice, sent } = withAddOn()
    await voice.start('v=0\r\noffer')
    asks(fake, 71, 'locust_call')
    asks(fake, 72, 'robinhood')
    asks(fake, 73, 'locust_call', 'something_else')
    fake.say({ method: 'item/started', params: { threadId: 'thread_1', item: { type: 'mcpToolCall', server: 'locust_call', tool: 'start_conversation', arguments: { teammate: 'Wren', message: 'Fix the login redirect loop' } } } })
    fake.say({ method: 'item/started', params: { threadId: 'thread_1', item: { type: 'mcpToolCall', server: 'locust_call', tool: 'list_teammates', arguments: {} } } })
    await settle()
    expect(fake.asked).toContainEqual({ method: 'answer:71', params: { action: 'accept', content: {} } })
    expect(fake.asked).toContainEqual({ method: 'answer:72', params: { action: 'decline' } })
    expect(fake.asked).toContainEqual({ method: 'answer:73', params: { action: 'decline' } })
    expect(sent.filter((event) => event.kind === 'handed')).toEqual([{ kind: 'handed', to: 'Wren', task: 'Fix the login redirect loop' }])
  })

  it('closes the add-on with the call', async () => {
    const { voice, closed } = withAddOn()
    await voice.start('v=0\r\noffer')
    expect(closed).toHaveLength(0)
    await voice.stop()
    await settle()
    expect(closed).toHaveLength(1)
  })

  it('still talks without it, and then lets Codex use nothing', async () => {
    const { fake, voice, closed } = withAddOn(async () => undefined)
    expect((await voice.start('v=0\r\noffer')).ok).toBe(true)
    expect(Object.keys(by(fake, 'thread/start').config as object)).not.toContain('mcp_servers.locust_call.enabled')
    expect(String(by(fake, 'thread/realtime/start').realtimeStartInstructions)).not.toContain('locust_call')
    asks(fake, 81, 'locust_call')
    await settle()
    expect(fake.asked).toContainEqual({ method: 'answer:81', params: { action: 'decline' } })
    await voice.stop()
    await settle()
    expect(closed).toHaveLength(0)
  })
})

/*
 * YOUR CONNECTORS IN A CALL (0.742). Colin, 2026-10-11: "want to be able to do simple things like check gmail by
 * asking". A connector that only reads runs without a question (Codex's rule, measured); one that would change
 * something asks, and the question is the person's, in the strip, never answered for them.
 */
describe('a voice call with your connectors', () => {
  // The question Codex asked of a throwaway server's send_note, measured 2026-10-11 (Codex 0.162.1).
  const ASKED = {
    threadId: 'thread_1', turnId: 'turn_1', serverName: 'robinhood', mode: 'form',
    _meta: { codex_approval_kind: 'mcp_tool_call', persist: ['session', 'always'], tool_description: 'Places an order.', tool_params: { symbol: 'AAPL', quantity: 1 },
      tool_params_display: [{ name: 'symbol', value: 'AAPL', display_name: 'symbol' }, { name: 'quantity', value: 1, display_name: 'quantity' }] },
    message: 'Allow the robinhood MCP server to run tool "place_equity_order"?', requestedSchema: { type: 'object', properties: {} }
  }
  const connected = (askWaitMs?: number) => {
    const fake = fakeCodex()
    return { fake, ...calls(() => fake, { connectors: true, ...(askWaitMs === undefined ? {} : { askWaitMs }) }) }
  }
  const asksOf = (sent: readonly VoiceCallEvent[]) => sent.filter((event): event is Extract<VoiceCallEvent, { kind: 'asks' }> => event.kind === 'asks')

  it('keeps the person’s connectors on for the call’s thread, and its thinking quick', async () => {
    const { fake, voice } = connected()
    await voice.start('v=0\r\noffer')
    // The fake's robinhood names no address, so it is not known to be a service elsewhere: it stays off.
    expect(by(fake, 'thread/start').config).toEqual({ 'features.plugins': false, 'features.hooks': false, notify: [], ...NO_TOOLS, model_reasoning_effort: 'low', 'mcp_servers.robinhood.enabled': false })
    const told = String(by(fake, 'thread/realtime/start').realtimeStartInstructions)
    expect(told).toContain('Allow and Deny in the call’s strip')
    await voice.stop()
  })

  it('asks the person before a connector changes anything, and does it only on Allow', async () => {
    const { fake, voice, sent } = connected()
    await voice.start('v=0\r\noffer')
    fake.say({ id: 50, method: 'mcpServer/elicitation/request', params: ASKED })
    await settle()
    const [asked] = asksOf(sent)
    expect(asked).toMatchObject({ what: 'robinhood: place equity order', detail: 'symbol: AAPL · quantity: 1' })
    // And each value on its own, for the card in a teammate's conversation (0.749).
    expect(asked?.fields).toEqual([{ name: 'symbol', value: 'AAPL' }, { name: 'quantity', value: '1' }])
    expect(fake.asked.some((entry) => entry.method === 'answer:50')).toBe(false)
    voice.answer(asked!.id, true)
    await settle()
    expect(fake.asked).toContainEqual({ method: 'answer:50', params: { action: 'accept', content: {} } })
    expect(sent).toContainEqual({ kind: 'asked', id: asked!.id, allowed: true })
    // An answer twice, or for an id nobody asked, changes nothing.
    voice.answer(asked!.id, false)
    voice.answer('nobody', true)
    await settle()
    expect(fake.asked.filter((entry) => entry.method === 'answer:50')).toHaveLength(1)
    await voice.stop()
  })

  it('is a Deny on Deny, when nobody answers in time, and when the call ends', async () => {
    const { fake, voice, sent } = connected(60)
    await voice.start('v=0\r\noffer')
    fake.say({ id: 51, method: 'mcpServer/elicitation/request', params: ASKED })
    await settle()
    voice.answer(asksOf(sent)[0]!.id, false)
    fake.say({ id: 52, method: 'mcpServer/elicitation/request', params: ASKED })
    await settle(120)
    fake.say({ id: 53, method: 'mcpServer/elicitation/request', params: { ...ASKED } })
    await settle()
    await voice.stop()
    await settle()
    for (const id of [51, 52]) expect(fake.asked).toContainEqual({ method: `answer:${String(id)}`, params: { action: 'decline' } })
    // The third ended with the call: Codex went with it, and the strip was told it is over.
    const over = sent.filter((event) => event.kind === 'asked')
    expect(over).toHaveLength(3)
    expect(over.every((event) => event.kind === 'asked' && !event.allowed)).toBe(true)
  })

  it('declines unasked what is not a connector’s tool call, and every connector when they are off', async () => {
    const { fake, voice, sent } = connected()
    await voice.start('v=0\r\noffer')
    fake.say({ id: 60, method: 'mcpServer/elicitation/request', params: { ...ASKED, mode: 'url', url: 'https://example.com' } })
    fake.say({ id: 61, method: 'item/fileChange/requestApproval', params: { threadId: 'thread_1' } })
    await settle()
    expect(asksOf(sent)).toEqual([])
    expect(fake.asked).toContainEqual({ method: 'answer:60', params: { action: 'decline' } })
    expect(fake.asked).toContainEqual({ method: 'answer:61', params: { decision: 'decline' } })
    await voice.stop()
    const off = one()
    await off.voice.start('v=0\r\noffer')
    off.fake.say({ id: 62, method: 'mcpServer/elicitation/request', params: ASKED })
    await settle()
    expect(asksOf(off.sent)).toEqual([])
    expect(off.fake.asked).toContainEqual({ method: 'answer:62', params: { action: 'decline' } })
    await off.voice.stop()
  })
})

/*
 * TALK'S VOICE (0.745): the one chosen in Settings goes to Codex as the call starts. Measured 2026-10-11: Codex
 * 0.162.1 took `voice: 'sol'` on a realtime v3 call and the call went on as before.
 */
describe('a voice call’s voice', () => {
  it('is the one chosen, and Codex’s own when none is or it cannot be read', async () => {
    const chosen = fakeCodex()
    const sol = calls(() => chosen, { voice: async () => 'sol' })
    await sol.voice.start('v=0\r\noffer')
    expect(by(chosen, 'thread/realtime/start')).toMatchObject({ voice: 'sol', version: 'v3' })
    await sol.voice.stop()
    for (const voice of [undefined, async () => undefined, async () => { throw new Error('unreadable') }]) {
      const fake = fakeCodex()
      const call = calls(() => fake, voice === undefined ? {} : { voice })
      expect((await call.voice.start('v=0\r\noffer')).ok).toBe(true)
      expect(by(fake, 'thread/realtime/start')).not.toHaveProperty('voice')
      await call.voice.stop()
    }
  })
})

/*
 * A CALL WITH A TEAMMATE (0.746): the voice speaks as them, in their voice, and the add-on may continue their
 * conversation while the call is on -- and not a moment after.
 */
describe('a voice call with a teammate', () => {
  const ADD_ON: VoiceAddOn = { command: 'locust.exe', args: ['bridge.mjs', 'conn.json', 'call'], env: { ELECTRON_RUN_AS_NODE: '1' } }
  it('speaks as the teammate, in its voice, into its conversation, and lets go of it when the call ends', async () => {
    const fake = fakeCodex()
    const targets: unknown[] = []
    const { voice, sent } = calls(() => fake, {
      addOn: { open: async () => ADD_ON, close: async () => undefined },
      voice: async () => 'cove',
      teammate: async (target) => ({ teammateId: target.teammateId, name: 'Wren', role: 'Code & Migrations', runtime: 'Claude Code', conversationId: 'mission_7', voice: 'sol' }),
      onTarget: (target) => { targets.push(target) }
    })
    expect((await voice.start('v=0\r\noffer', { teammateId: 'tm_wren', conversationId: 'mission_7' })).ok).toBe(true)
    const started = by(fake, 'thread/realtime/start')
    expect(started.voice).toBe('sol')
    expect(String(started.realtimeStartInstructions)).toContain('You are Wren\'s voice')
    // And the voice itself is told who it is and never to guess (0.748): the instructions above reach the Codex behind it.
    expect(started.initialItems).toEqual([{ role: 'developer', text: expect.stringContaining('You are the voice of Wren') }])
    expect(String((started.initialItems as { text: string }[])[0]!.text)).toContain('Never guess')
    expect(targets).toEqual([{ teammateId: 'tm_wren', conversationId: 'mission_7' }])
    // Its ask into that conversation is said in the strip as one, under the teammate's name.
    fake.say({ method: 'item/started', params: { threadId: 'thread_1', item: { type: 'mcpToolCall', server: 'locust_call', tool: 'send_message', arguments: { conversation_id: 'mission_7', message: 'Fix it and add a test.' } } } })
    await settle()
    expect(sent).toContainEqual({ kind: 'handed', to: 'Wren', task: 'Fix it and add a test.' })
    await voice.stop()
    expect(targets.at(-1)).toBeUndefined()
  })

  it('is an ordinary call when the teammate cannot be found', async () => {
    const fake = fakeCodex()
    const targets: unknown[] = []
    const { voice } = calls(() => fake, { teammate: async () => undefined, onTarget: (target) => { targets.push(target) } })
    expect((await voice.start('v=0\r\noffer', { teammateId: 'tm_gone' })).ok).toBe(true)
    expect(String(by(fake, 'thread/realtime/start').realtimeStartInstructions)).not.toContain('You are')
    await voice.stop()
    expect(targets.filter((target) => target !== undefined)).toEqual([])
  })
})

/*
 * HEARING A VOICE BEFORE CHOOSING IT (0.751): a preview is a call with that voice alone -- no add-on, no connector,
 * no teammate and nothing of Codex's own start -- told only to greet. It never ends a call that is on.
 */
describe('a voice heard before it is chosen', () => {
  it('is a call with that voice alone, told only to greet', async () => {
    const fake = fakeCodex()
    const open = vi.fn(async (): Promise<VoiceAddOn | undefined> => ({ command: 'locust.exe', args: ['bridge.mjs'], env: {} }))
    const onTarget = vi.fn()
    const teammate = vi.fn(async () => ({ teammateId: 'tm_ada', name: 'Ada', runtime: 'Claude Code', voice: 'ember' }))
    const { voice } = calls(() => fake, { connectors: true, addOn: { open, close: async () => undefined }, onTarget, teammate })
    expect(await voice.start('v=0\r\noffer', { teammateId: 'tm_ada' }, 'sol')).toEqual({ ok: true, answer: 'v=0\r\nanswer' })
    expect(open).not.toHaveBeenCalled()
    expect(teammate).not.toHaveBeenCalled()
    expect(onTarget).not.toHaveBeenCalled()
    expect(by(fake, 'thread/start').config).toEqual({ 'features.apps': false, 'features.plugins': false, 'features.hooks': false, notify: [], ...NO_TOOLS, 'mcp_servers.robinhood.enabled': false })
    const started = by(fake, 'thread/realtime/start')
    expect(started).toMatchObject({ outputModality: 'audio', version: 'v3', voice: 'sol', includeStartupContext: false })
    expect(started).not.toHaveProperty('realtimeStartInstructions')
    expect(started.initialItems).toEqual([{ role: 'developer', text: voicePreviewRules('sol') }])
    expect(voicePreviewRules('sol')).toContain('Your voice is called Sol')
    await voice.stop()
  })

  it('never ends a call that is on, and one preview may take the place of another', async () => {
    const { voice, spawned } = calls(() => fakeCodex())
    expect((await voice.start('v=0\r\noffer')).ok).toBe(true)
    expect(await voice.start('v=0\r\noffer', undefined, 'sol')).toEqual({ ok: false, message: 'A call is on. End it to hear a voice.' })
    expect(spawned).toHaveLength(1)
    await voice.stop()
    expect((await voice.start('v=0\r\noffer', undefined, 'sol')).ok).toBe(true)
    expect((await voice.start('v=0\r\noffer', undefined, 'cove')).ok).toBe(true)
    expect(spawned).toHaveLength(3)
    expect(spawned[1]!.killed()).toBe(true)
    await voice.stop()
  })
})

/*
 * THE CALL'S OWN SERVER CLOSES WITH IT (0.753, bug hunt 2026-10-12, TK3). A call ended while Locust's add-on for it
 * was still opening had nothing to close at that moment; the open then finished, and the server stayed up -- its
 * token in its own file -- until the next call ended or Locust quit.
 */
describe('a call ended while its add-on opens', () => {
  it('closes the add-on once it has opened', async () => {
    let opened: (value: VoiceAddOn) => void = () => undefined
    const open = vi.fn(() => new Promise<VoiceAddOn>((resolve) => { opened = resolve }))
    const close = vi.fn(async () => undefined)
    const { voice } = calls(() => fakeCodex(), { addOn: { open, close } })
    const starting = voice.start('v=0\r\noffer')
    await settle(60)
    expect(open).toHaveBeenCalledTimes(1)
    await voice.stop()
    expect(close).not.toHaveBeenCalled()
    opened({ command: 'locust.exe', args: ['bridge.mjs'], env: {} })
    expect((await starting).ok).toBe(false)
    await settle()
    expect(close).toHaveBeenCalledTimes(1)
  })
})
