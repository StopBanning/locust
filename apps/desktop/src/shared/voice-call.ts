/**
 * A VOICE CALL (0.737): ChatGPT's own voice, through the Codex CLI Locust already drives.
 *
 * Colin, 2026-10-10: "locust interacts with these clis and models all day, can you just mirror or have a
 * command activate chatgpt and activate the voice model, the same way we interface with every other thing".
 * Codex 0.162 carries it: `codex features list` has `realtime_conversation` on, and its app server takes
 * `thread/realtime/start`. MEASURED on Colin's Plus sign-in (C:\work\scratch\voice-probe): over a websocket
 * Codex says "realtime conversation requires API key auth"; over WebRTC with Codex's default version the
 * voice service asks for a header Codex does not send; `version: 'v2'` is refused ("AVAS realtime calls
 * require realtime v1 or v3"); `version: 'v3'` connects -- "What is two plus two" came back as "That's four."
 * in about a second. Nothing of the sign-in passes through Locust: Codex places the call as itself.
 *
 * The call stands in the person's folder on a Codex thread of its own -- ephemeral, read-only, running no
 * command. 0.737 switched every add-on off for that thread (Colin's has a trading connector); since 0.742 the
 * person's connectors are the call's, and one that would change something asks them in the strip first.
 */

export const VOICE_CALL_AVAILABLE = 'voice-call:available'
export const VOICE_CALL_START = 'voice-call:start'
export const VOICE_CALL_STOP = 'voice-call:stop'
export const VOICE_CALL_EVENT = 'voice-call:event'
/** The person's Allow or Deny for something a call asked to do (0.742). */
export const VOICE_CALL_ANSWER = 'voice-call:answer'
/** Which voice a call speaks in (0.745): read, and set. */
export const VOICE_CALL_VOICES = 'voice-call:voices'
export const VOICE_CALL_SET_VOICE = 'voice-call:set-voice'
/** Hearing a voice before choosing it (0.751): a short call of its own, and the greeting it gave, kept. */
export const VOICE_CALL_PREVIEW = 'voice-call:preview'
export const VOICE_PREVIEW_SAVED = 'voice-preview:saved'
export const VOICE_PREVIEW_SAVE = 'voice-preview:save'
export const VOICE_PREVIEW_TRIGGER = 'voice-preview:trigger'

/*
 * CHATGPT'S OWN VOICES (0.745). Colin, 2026-10-11, holding ChatGPT's "Choose your voice" (Sol, "Savvy and
 * relaxed", the eighth of nine): "see if we can set this up". `thread/realtime/listVoices` on Codex 0.162.1 answers
 * two lists: v1, the nine ChatGPT's app offers, in its order -- juniper, maple, spruce, ember, vale, breeze, arbor,
 * sol, cove (default cove) -- and v2, the API's (alloy ... marin, default marin). The words beside each are the
 * ones ChatGPT's app shows.
 */
export const CHATGPT_VOICES = [
  { id: 'juniper', about: 'Open and upbeat' },
  { id: 'maple', about: 'Cheerful and candid' },
  { id: 'spruce', about: 'Calm and affirming' },
  { id: 'ember', about: 'Confident and optimistic' },
  { id: 'vale', about: 'Bright and inquisitive' },
  { id: 'breeze', about: 'Animated and earnest' },
  { id: 'arbor', about: 'Easygoing and versatile' },
  { id: 'sol', about: 'Savvy and relaxed' },
  { id: 'cove', about: 'Composed and direct' }
] as const
export type ChatGptVoice = (typeof CHATGPT_VOICES)[number]['id']
/** Codex's own choice when the person has not made one. */
export const DEFAULT_CHATGPT_VOICE: ChatGptVoice = 'cove'
export function isChatGptVoice(value: unknown): value is ChatGptVoice {
  return typeof value === 'string' && CHATGPT_VOICES.some((voice) => voice.id === value)
}
export const voiceName = (id: string): string => id.charAt(0).toUpperCase() + id.slice(1)

/**
 * WHERE TALK RUNS: Windows, and a Mac (the first Mac run, 2026-10-30). Nothing in a call is Windows' own -- Codex's
 * app server places it, and the window holds the microphone and the WebRTC connection -- and on that Mac Codex,
 * signed in with ChatGPT, took `thread/realtime/start` and passed it to the voice service. Linux waits for a run.
 */
export const talkRunsOn = (platform: string | undefined): boolean => platform === 'win32' || platform === 'darwin'

export type VoiceCallAvailability = { readonly available: true } | { readonly available: false; readonly why: string }
export type VoiceCallStartResult = { readonly ok: true; readonly answer: string } | { readonly ok: false; readonly message: string }

/** Who said a line: the person, or the voice. */
export type VoiceCallRole = 'you' | 'voice'

export type VoiceCallEvent =
  /** Words as they are heard or spoken, before the line is finished. */
  | { readonly kind: 'partial'; readonly role: VoiceCallRole; readonly delta: string }
  /** A finished line. */
  | { readonly kind: 'said'; readonly role: VoiceCallRole; readonly text: string }
  /** Codex is working behind the voice (it was handed something to look at). */
  | { readonly kind: 'working'; readonly working: boolean }
  /** The voice handed something to a teammate (0.739): a real conversation, in the teammate's own mode. */
  | { readonly kind: 'handed'; readonly to: string; readonly task: string }
  /**
   * One of the person's connectors would change something: it waits for Allow in the strip (0.742), or, on a call
   * with a teammate, on a card in their conversation, one row for each value it would send (0.749).
   */
  | { readonly kind: 'asks'; readonly id: string; readonly what: string; readonly detail: string; readonly fields?: readonly VoiceCallAskField[] }
  /** That wait is over: answered, timed out, or the call ended. */
  | { readonly kind: 'asked'; readonly id: string; readonly allowed: boolean }
  | { readonly kind: 'error'; readonly message: string }
  | { readonly kind: 'ended'; readonly why?: string }

/*
 * TALKING TO A TEAMMATE (0.746). Colin, 2026-10-11, on giving the call a face of its own: "we have the ability to
 * assign the teammate a voice or just talk to the teammate directly with the voice? cause otherwise it may be
 * clunky". So Talk pressed in a teammate's conversation is a call WITH that teammate: the voice speaks as them, in
 * their voice, and what the person asks goes into that same conversation, where the teammate does it on its own
 * runtime, in its own mode, with its cards in the thread. The voice is still ChatGPT's; the work is the teammate's.
 */
export interface VoiceCallTarget {
  readonly teammateId: string
  /** The conversation Talk was pressed in, when it has one: the call continues it, and only it. */
  readonly conversationId?: string
}

const CALL_ID = /^[A-Za-z0-9_-]{1,128}$/
export function isVoiceCallTarget(value: unknown): value is VoiceCallTarget {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const { teammateId, conversationId } = value as Record<string, unknown>
  return typeof teammateId === 'string' && CALL_ID.test(teammateId) && (conversationId === undefined || (typeof conversationId === 'string' && CALL_ID.test(conversationId)))
}

/** The teammate a call speaks as, in the words the voice is told. */
export interface VoiceCallTeammate {
  readonly teammateId: string
  readonly name: string
  /** Its role as the person reads it ("Code & Migrations"), when it has one. */
  readonly role?: string
  /** The AI agent its work runs on, by its own name ("Claude Code"). */
  readonly runtime: string
  readonly conversationId?: string
}

export interface VoiceCallApi {
  available(): Promise<VoiceCallAvailability>
  /** The window's own WebRTC offer in, and the teammate it is with (0.746); the voice service's answer back. */
  start(offer: string, target?: VoiceCallTarget): Promise<VoiceCallStartResult>
  stop(): Promise<void>
  /** Allow or Deny one `asks`. */
  answer(id: string, allowed: boolean): Promise<void>
  /** The voice a call speaks in, and setting it; the next call uses it. */
  voice(): Promise<ChatGptVoice>
  setVoice(voice: ChatGptVoice): Promise<ChatGptVoice>
  onEvent(listener: (event: VoiceCallEvent) => void): () => void
  /** A short call with only this voice, to hear it greet you (0.751); ended with `stop`. */
  preview?(offer: string, voice: ChatGptVoice): Promise<VoiceCallStartResult>
  /** The greeting a voice gave before, kept so it plays at once; undefined when there is none yet. */
  previewSaved?(voice: ChatGptVoice): Promise<Uint8Array | undefined>
  previewSave?(voice: ChatGptVoice, audio: Uint8Array): Promise<boolean>
  /** A spoken "Hello?" made by Windows' own speech, for the voice to answer: v3 speaks only after a spoken turn. */
  previewTrigger?(): Promise<Uint8Array | undefined>
}

/*
 * HEARING A VOICE BEFORE CHOOSING IT (0.751). Canvas board 4, "Each teammate has a voice": "Press a voice's play
 * button to hear it." ChatGPT's own picker plays a sample; Codex has none to give, and its v3 voice speaks only after
 * a SPOKEN turn -- measured 2026-10-12 on Codex 0.162.1: `appendText`, `appendSpeech` (with client-managed handoffs
 * too) and the start's `prompt` only append context, and `response.create` on the data channel is refused ("requires
 * a session with Responses delegation"). A spoken "Hello?" played into a silent call works: Sol answered "Hello, I'm
 * Sol, it's lovely to meet you" about three seconds after it. So the first play is a short call on the person's
 * plan, and the greeting is kept; every play after that is the kept one.
 */
export function voicePreviewRules(voice: ChatGptVoice): string {
  const name = voiceName(voice)
  return `This is a short preview of how your voice sounds, for someone choosing a voice in Locust. Your voice is called ${name}. Whatever you hear, answer with one warm sentence of greeting that says your name is ${name}, and nothing else. Do not ask a question.`
}

/** A kept greeting is a WebM recording of a few seconds, from this window's own recorder. */
export const VOICE_PREVIEW_MAX_BYTES = 1_500_000
export function isVoicePreviewAudio(value: unknown): value is Uint8Array {
  return value instanceof Uint8Array && value.byteLength >= 1_000 && value.byteLength <= VOICE_PREVIEW_MAX_BYTES
    && value[0] === 0x1a && value[1] === 0x45 && value[2] === 0xdf && value[3] === 0xa3
}

/** How long a connector's question waits for the person before it is a Deny. */
export const VOICE_CALL_ASK_WAIT_MS = 120_000

/** The name a person knows a server by: ChatGPT's own connectors come through Codex as `codex_apps`. */
export function voiceCallServerName(server: string): string {
  return server === 'codex_apps' ? 'ChatGPT' : server.replace(/[_-]+/g, ' ').trim() || 'A connector'
}

/**
 * WHAT A CONNECTOR WOULD DO, IN THE STRIP (0.742). Measured 2026-10-11 against Codex 0.162.1 with a throwaway
 * server: a tool marked read only runs with no question; one that is not first asks `mcpServer/elicitation/
 * request`, mode `form`, its message `Allow the probe MCP server to run tool "send_note"?` and its `_meta`
 * carrying `codex_approval_kind: mcp_tool_call`, `tool_description` and `tool_params_display` ([{display_name,
 * value}]). Undefined for anything that is not that question: it is declined unasked.
 */
export function voiceCallAsk(params: unknown): VoiceCallAsked | undefined {
  if (typeof params !== 'object' || params === null) return undefined
  const p = params as Record<string, unknown>
  const meta = typeof p._meta === 'object' && p._meta !== null ? p._meta as Record<string, unknown> : {}
  if (meta.codex_approval_kind !== 'mcp_tool_call' || p.mode !== 'form' || typeof p.serverName !== 'string') return undefined
  const tool = typeof p.message === 'string' ? /run tool "([^"]{1,120})"/.exec(p.message)?.[1] : undefined
  const words = (value: string): string => value.replace(/[_-]+/g, ' ').trim()
  // ChatGPT's own apps name their tools by app (0.750, read from Colin's account: `gmail.send_email`, 197 of them):
  // the app is what a person knows, so "Gmail: send email" rather than "ChatGPT: gmail.send email".
  const app = p.serverName === 'codex_apps' && tool !== undefined ? /^([a-z0-9_-]{1,40})\.(.+)$/i.exec(tool) : null
  const what = app !== null
    ? `${words(app[1]!).charAt(0).toUpperCase()}${words(app[1]!).slice(1)}: ${words(app[2]!)}`
    : `${voiceCallServerName(p.serverName)}: ${tool === undefined ? 'a tool' : words(tool)}`
  const shown = Array.isArray(meta.tool_params_display) ? meta.tool_params_display : []
  const fields = shown
    .map((entry): VoiceCallAskField | undefined => {
      if (typeof entry !== 'object' || entry === null) return undefined
      const { display_name: name, value } = entry as Record<string, unknown>
      if (typeof name !== 'string' || name.trim().length === 0) return undefined
      const said = typeof value === 'string' ? value : (JSON.stringify(value) as string | undefined) ?? ''
      return { name: name.replace(/\s+/g, ' ').trim().slice(0, 60), value: said.replace(/\s+/g, ' ').trim().slice(0, 600) }
    })
    .filter((field): field is VoiceCallAskField => field !== undefined)
    .slice(0, 8)
  const detail = fields.map((field) => `${field.name}: ${field.value.slice(0, 160)}`).join(' · ').slice(0, 400)
  return { what: what.slice(0, 140), detail, fields }
}

/** One value a connector would send, as Codex names it: a name such as "subject", and what it would send. */
export interface VoiceCallAskField {
  readonly name: string
  readonly value: string
}

/** A connector's question, for the strip (what, detail) and for the card in a teammate's conversation (fields). */
export interface VoiceCallAsked {
  readonly what: string
  readonly detail: string
  readonly fields: readonly VoiceCallAskField[]
}

/** An offer is SDP from this window's own RTCPeerConnection: bounded text that starts as SDP does. */
export const VOICE_CALL_MAX_OFFER = 64 * 1024
export function isVoiceCallOffer(value: unknown): value is string {
  return typeof value === 'string' && value.startsWith('v=0') && value.length <= VOICE_CALL_MAX_OFFER
}

/** Codex's transcript roles, in Locust's words. */
export function voiceCallRole(role: unknown): VoiceCallRole | undefined {
  if (role === 'user') return 'you'
  if (role === 'assistant') return 'voice'
  return undefined
}

/**
 * LOCUST ITSELF, THE ONE ADD-ON A CALL'S CODEX HAS (0.739). A private copy of Locust's own add-on server
 * ("Let your other AI apps use Locust", which can stay off), started for the call and stopped with it, carrying
 * only the hand-off tools. Its name is the call's own, so it never meets one of the person's servers.
 */
export const VOICE_CALL_ADD_ON = 'locust_call'
export interface VoiceAddOn {
  readonly command: string
  readonly args: readonly string[]
  readonly env: Readonly<Record<string, string>>
}
/** The tools the call's copy of Locust's server answers; anything else is refused there. */
export const VOICE_CALL_TOOLS: ReadonlySet<string> = new Set(['list_teammates', 'start_conversation', 'send_message', 'read_reply', 'list_background_runs', 'find_files', 'read_file', 'search_files'])

/** A name that can sit inside a dotted config key without being read as two. */
const PLAIN_NAME = /^[A-Za-z0-9@_-]{1,128}$/

/** What the call's Codex thread is started with, or why the call is not placed. */
export type VoiceThreadConfig =
  | { readonly ok: true; readonly config: Readonly<Record<string, boolean | string | readonly string[] | Readonly<Record<string, string>>>> }
  | { readonly ok: false; readonly message: string }

/** The hosts a call's own traffic may be pointed at: OpenAI's. */
function openAiHost(value: string): boolean {
  try {
    const host = new URL(value).hostname.toLowerCase()
    return host === 'chatgpt.com' || host.endsWith('.chatgpt.com') || host === 'openai.com' || host.endsWith('.openai.com')
  } catch {
    return false
  }
}

/**
 * NOTHING THE PERSON'S CODEX WOULD START, FOR THIS ONE THREAD (measured 2026-10-11 against Codex 0.162.1,
 * thread/start alone: Colin's machine starts cua_repl, node_repl, codex_apps and robinhood without this, none
 * with it).
 *
 * Read from `config/read` alone, which connects to nothing. 0.737 also asked `mcpServerStatus/list` for the
 * names, and that request STARTS every server to list its tools -- it tried Robinhood's OAuth refresh before
 * the call's thread existed (Sol's review of 0.737, #1). So: `features.apps` off (ChatGPT's connectors),
 * `features.plugins` off (every server a plugin brings, whatever its name), `features.hooks` off and `notify`
 * empty (commands Codex runs on the host, outside the sandbox: #3), and each server in the person's own
 * config switched off by name. `mcp_servers: {}` would change nothing (it merges), and naming a server that is
 * not in their config makes a half entry Codex refuses.
 *
 * Refused, rather than placed with something left on (#2), when the settings did not read as settings or a
 * name cannot be written as a key; and when Codex has been pointed at a voice or ChatGPT address that is not
 * OpenAI's, since the call's audio and control would follow it (#4).
 */
/*
 * AND THE PERSON'S CONNECTORS, WHEN THEY ARE THE CALL'S (0.742). Colin, 2026-10-11: "want to be able to do simple
 * things like check gmail by asking but then i also want to be able to hand off larger tasks". With `connectors`,
 * ChatGPT's connectors and the person's own servers stay as their Codex has them: one that only reads answers at
 * once, and one that would change something waits for Allow in the call's strip (voiceCallAsk). Plugins, hooks
 * and notify stay off either way: none of them is a connector the person asked for by name. The thinking behind
 * the voice is set low, because a call waits on it out loud.
 */
/** The test server a drive gives a development build's call (never a packaged one): `locust_probe`. */
export const VOICE_CALL_TEST_SERVER = 'locust_probe'

export function voiceThreadConfig(config: unknown, addOn?: VoiceAddOn, options: { readonly connectors?: boolean; readonly testServer?: VoiceAddOn } = {}): VoiceThreadConfig {
  if (typeof config !== 'object' || config === null || Array.isArray(config)) {
    return { ok: false, message: 'Locust could not read your Codex settings, so it did not place the call. Try again.' }
  }
  const settings = config as Record<string, unknown>
  for (const [key, value] of Object.entries(settings)) {
    // A realtime setting counts when it is an address; a base URL counts whatever it holds.
    const pointed = typeof value === 'string' && value.length > 0 && !openAiHost(value)
      && (key === 'chatgpt_base_url' || key === 'openai_base_url' || (/realtime/i.test(key) && /:\/\//.test(value)))
    const nested = /realtime/i.test(key) && typeof value === 'object' && value !== null && Object.values(value).some((inner) => typeof inner === 'string' && /:\/\//.test(inner) && !openAiHost(inner))
    if (pointed || nested) {
      return { ok: false, message: `Your Codex settings send ${/realtime/i.test(key) ? 'voice calls' : 'ChatGPT requests'} to an address that is not OpenAI’s (${key}), so Locust did not place the call. Remove that setting to use Talk.` }
    }
  }
  const connectors = options.connectors === true
  const out: Record<string, boolean | string | readonly string[] | Readonly<Record<string, string>>> = {
    ...(connectors ? {} : { 'features.apps': false }),
    'features.plugins': false,
    'features.hooks': false,
    notify: [],
    /*
     * NO SHELL, BROWSER OR COMPUTER for the call's own Codex (0.747). Asked to read a file, it ran a command first;
     * the call refused it, and the voice said "I need your approval in the app to continue" when there was nothing to
     * approve -- the confusion Colin hit on 10/11. The call reads with Locust's folder tools and works through
     * teammates, so these go: measured 2026-10-11 on Codex 0.162.1, with `features.shell_tool` off it ran nothing
     * and said "NO SHELL".
     */
    'features.shell_tool': false,
    'features.browser_use': false,
    'features.browser_use_external': false,
    'features.in_app_browser': false,
    'features.computer_use': false,
    ...(connectors ? { model_reasoning_effort: 'low' } : {})
  }
  const servers = settings.mcp_servers
  if (servers !== undefined && servers !== null) {
    if (typeof servers !== 'object' || Array.isArray(servers)) return { ok: false, message: 'Locust could not read your Codex add-ons, so it did not place the call. Try again.' }
    for (const name of Object.keys(servers)) {
      // The call's own name is the call's: one of the person's by that name cannot share it.
      if (name === VOICE_CALL_ADD_ON && addOn !== undefined) return { ok: false, message: `Your Codex has an add-on named "${VOICE_CALL_ADD_ON}", the name a call keeps for Locust, so Locust did not place the call.` }
      /*
       * A connector is a service somewhere else, reached at an address: those stay. A server the person's machine
       * runs as a program stays off: measured 2026-10-11, a call's Codex ran Colin's node_repl -- JavaScript on
       * this computer, outside the sandbox -- with no question, because Codex counts it as one that needs none.
       */
      const entry: unknown = (servers as Record<string, unknown>)[name]
      const remote = typeof entry === 'object' && entry !== null && typeof (entry as Record<string, unknown>).url === 'string' && (entry as Record<string, unknown>).command === undefined
      if (connectors && remote) continue
      if (!PLAIN_NAME.test(name)) return { ok: false, message: `Locust could not switch off your Codex add-on "${name.slice(0, 60)}" for the call, so it did not place it.` }
      out[`mcp_servers.${name}.enabled`] = false
    }
  }
  if (options.testServer !== undefined) {
    out[`mcp_servers.${VOICE_CALL_TEST_SERVER}.command`] = options.testServer.command
    out[`mcp_servers.${VOICE_CALL_TEST_SERVER}.args`] = options.testServer.args
    out[`mcp_servers.${VOICE_CALL_TEST_SERVER}.env`] = options.testServer.env
    out[`mcp_servers.${VOICE_CALL_TEST_SERVER}.enabled`] = true
  }
  if (addOn !== undefined) {
    out[`mcp_servers.${VOICE_CALL_ADD_ON}.command`] = addOn.command
    out[`mcp_servers.${VOICE_CALL_ADD_ON}.args`] = addOn.args
    out[`mcp_servers.${VOICE_CALL_ADD_ON}.env`] = addOn.env
    out[`mcp_servers.${VOICE_CALL_ADD_ON}.enabled`] = true
    /*
     * Its tools are used without Codex asking (0.747). Locust already said yes to each of them (voice-call.ts,
     * onRequest), but Codex's question still reached the voice, which asked the person "quick approval check: should
     * I run the locust call send message tool?" -- a question with no answer to give. Measured 2026-10-11 on Codex
     * 0.162.1: with `default_tools_approval_mode` "approve", a tool that asked before ran without asking. The person's
     * own connectors still ask, in the strip.
     */
    out[`mcp_servers.${VOICE_CALL_ADD_ON}.default_tools_approval_mode`] = 'approve'
  }
  return { ok: true, config: out }
}

/** What the call's Codex is told when it starts: where it is, and what it may not do. */
/**
 * THE VOICE'S OWN RULES (0.748). `realtimeStartInstructions` reach the Codex model BEHIND the voice -- Codex's own
 * words for it -- not the voice that speaks. Twice on 2026-10-11 the voice answered "the secret ingredient is
 * friendship" before the README had been read, then corrected itself. These go to the voice itself, as a developer
 * item at the start of its session (`initialItems`, realtime v3).
 */
export function voiceCallVoiceRules(teammate?: Pick<VoiceCallTeammate, 'name'>): string {
  const who = teammate === undefined
    ? 'You are the voice of a call the person started in Locust.'
    : `You are the voice of ${spoken(teammate.name, 60)}, a Locust teammate: speak as ${spoken(teammate.name, 60)}, in the first person, warmly and briefly.`
  return `${who} You do not know what any file, conversation, email, calendar or result says until the work behind you reports it. Never guess and never answer from memory: say you are checking, and answer only once you have the result. If the work cannot get it, say so plainly. Anything that would send, change or buy something waits for the person's Allow in Locust's window; never say it is done before it is.`
}

export function voiceCallInstructions(folderName: string, handsOff = false, connectors = false, teammate?: VoiceCallTeammate): string {
  if (teammate !== undefined && handsOff) return teammateCallInstructions(folderName, connectors, teammate)
  /*
   * Said as what works, because the voice believed what it was not told (2026-10-11): asked to read the folder,
   * it ran commands Locust refused, and said "I just sent the request in the app" when there was nothing to answer.
   */
  // "Never guess" (0.747): once, asked what a README said, the voice answered "friendship" without reading it.
  const folder = handsOff
    ? `This is a voice call the person started in Locust, about the folder "${folderName}". To look in that folder, use the ${VOICE_CALL_ADD_ON} tools find_files, read_file and search_files; never run shell commands, which this call cannot run. You change no file yourself. Never guess what a file or a connector says: look first, and if you cannot, say so.`
    : `This is a voice call the person started in Locust, about the folder "${folderName}". You may read files in that folder to answer, but you change nothing yourself during this call: no edits, no commands that change anything.`
  const tools = connectors
    ? ' For quick things, like checking their email or calendar, use their connectors directly. A connector that only reads answers at once. One that would send, change or buy something first shows Allow and Deny in the call’s strip in Locust: say what it will do and ask them to press Allow, and only say it is done once the tool says so. If they press Deny, or it is not answered, it did not happen.'
    : ''
  if (!handsOff) return `${folder}${tools} If they ask for a change, say that one of their teammates can make it, and that they can hand it over after the call.`
  const hands = ` Bigger work goes to their teammates: list_teammates says who there is and what each runs on. Before you hand anything over, say back in one short sentence who will do what, and wait for them to say yes; never start one they did not agree to. Then start_conversation with that teammate and the task in plain words, as the person would type it. The teammate works in its own mode, and anything it needs approved shows in Locust's window. Tell them it has started. When they ask how it went, read_reply with the conversation's id and say what came back, briefly.`
  return folder + tools + hands
}

/** A name fit to sit inside the voice's instructions: one line, no quotes to break out of. */
const spoken = (text: string, cap = 80): string => text.replace(/[\s"“”]+/g, ' ').trim().slice(0, cap)

/**
 * A call WITH a teammate (0.746): the voice is that teammate's, and their work goes into their own conversation.
 * Each ask goes in as the person's words, so the teammate's run is the one that does it -- on its own runtime, in its
 * own mode, with its approval cards in Locust's window -- and the voice says what came back, as the teammate.
 */
function teammateCallInstructions(folderName: string, connectors: boolean, teammate: VoiceCallTeammate): string {
  const name = spoken(teammate.name, 60)
  const role = teammate.role === undefined ? '' : ` (${spoken(teammate.role)})`
  const runtime = spoken(teammate.runtime, 40)
  const where = teammate.conversationId === undefined
    ? `The first time they ask ${name} to do something, start_conversation with teammate "${teammate.teammateId}" and their words; it returns a conversation id, and every later ask goes to that conversation with send_message.`
    : `${name}'s conversation is "${teammate.conversationId}": every ask goes there with send_message, conversation_id "${teammate.conversationId}".`
  const tools = connectors
    ? ' For a quick look at their email or calendar you may use their connectors yourself. One that would send, change or buy something first shows Allow and Deny in Locust: say what it will do, ask them to press Allow, and only say it is done once the tool says so.'
    : ''
  return `This is a voice call the person started in Locust, in their conversation with ${name}${role}, a Locust teammate whose work runs on ${runtime}, in the folder "${spoken(folderName)}". You are ${name}'s voice: speak as ${name}, in the first person, warmly and briefly, and never say you are someone else. `
    + `Anything they want looked into or done goes to ${name}'s own run, which does the work: ${where} Send their ask in plain words, as they would type it, and say it is underway. Then read_reply with that conversation id until it has finished, and tell them what came back in a sentence or two, as ${name}. If the reply is still working, say so and offer to check again; never invent a result. Anything ${name}'s run needs approved shows as a card in Locust's window: say so when read_reply mentions one. `
    + `To glance at a file yourself, use find_files, read_file and search_files; never run shell commands, which this call cannot run, and you change no file yourself.${tools} If they want a different teammate, list_teammates says who there is: say who you will ask, wait for a yes, then start_conversation with them.`
}

/** A failure from Codex or the voice service, in words a person can act on. */
export function voiceCallSaid(message: string): string {
  const first = message.replace(/\s+/g, ' ').trim()
  // A limit first: "AVAS quota exceeded" is a limit, not a version Locust does not know (Sol's review, #13).
  if (/rate.?limit|usage limit|quota|too many/i.test(first)) {
    return 'ChatGPT says your plan’s voice time is used up for now. Try again later.'
  }
  if (/did not answer .* in time|timed out|timeout/i.test(first)) {
    return 'Codex did not answer in time, so the call did not start. Try again; if it keeps happening, restart Locust.'
  }
  if (/requires API key auth|apiKey/i.test(first)) {
    return 'Codex could not place the call with how it is signed in. Voice calls use your ChatGPT sign-in; sign Codex in with ChatGPT and try again.'
  }
  if (/quicksilver|AVAS|realtime v1 or v3/i.test(first)) {
    return `This version of Codex answered the call in a way Locust does not know yet. Update Codex in Settings > AI agents and try again. (Codex said: ${first.slice(0, 160)})`
  }
  return `The call could not go on. Codex said: ${first.slice(0, 200)}`
}

/** A finished call as text for the chat box, said line by line, and what was handed over (0.739). */
export function voiceCallText(
  lines: readonly { readonly role: VoiceCallRole; readonly text: string }[],
  handed: readonly { readonly to: string; readonly task: string }[] = []
): string {
  const said = lines.map((line) => ({ ...line, text: line.text.replace(/\s+/g, ' ').trim() })).filter((line) => line.text.length > 0)
  if (said.length === 0 && handed.length === 0) return ''
  const over = handed.map((hand) => `- ${hand.to}: ${hand.task.replace(/\s+/g, ' ').trim()}`)
  return [
    'From a voice call:',
    '',
    ...said.map((line) => `${line.role === 'you' ? 'You' : 'ChatGPT'}: ${line.text}`),
    ...(over.length === 0 ? [] : ['', 'Handed over:', ...over])
  ].join('\n')
}
