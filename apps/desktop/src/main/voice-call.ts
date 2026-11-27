import { randomUUID } from 'node:crypto'
import { basename } from 'node:path'

import { codexRefusalFor, createAppServerClient } from '@teammate/runtime-adapters'
import type { AppServerClient, AppServerRunProcess as AppServerProcess, ExecutableLaunch, JsonValue } from '@teammate/runtime-adapters'

import {
  VOICE_CALL_ADD_ON,
  VOICE_CALL_ASK_WAIT_MS,
  voiceCallAsk,
  voiceCallInstructions,
  voiceCallVoiceRules,
  voicePreviewRules,
  voiceCallRole,
  voiceCallSaid,
  voiceThreadConfig,
  type ChatGptVoice,
  type VoiceAddOn,
  type VoiceCallAsked,
  type VoiceCallAvailability,
  type VoiceCallTarget,
  type VoiceCallTeammate,
  type VoiceCallEvent,
  type VoiceCallStartResult
} from '../shared/voice-call.js'

/**
 * ONE VOICE CALL AT A TIME, placed by Codex (0.737). See shared/voice-call.ts for what was measured.
 *
 * The window makes the WebRTC offer and plays what comes back; this side starts Codex's app server,
 * checks it is signed in with ChatGPT, opens a read-only thread of its own in the person's folder with
 * every add-on off, asks for the call on realtime v3, and hands the window the voice service's answer.
 * What is said comes back as Codex's transcript notifications and goes to the window as lines.
 *
 * A START IS A CALL FROM ITS FIRST MOMENT (0.738, Sol's review of 0.737, #6 and #11). It was only tracked
 * once Codex had been found and spawned, so two quick presses spawned two servers and stopped one, and an
 * End or a closed window during the start returned before a server was spawned behind it. Now a start holds
 * a ticket from the first line; Stop cancels it at whatever step it has reached, each step looks at it
 * before going on, and an answer that arrives for a call already ended is not handed back as a call.
 */
export interface VoiceCallDeps {
  readonly policyRefusal?: (target?: VoiceCallTarget, preview?: boolean) => Promise<string | undefined>
  /** Codex as discovery found it, when it is ready to start. */
  readonly codex: () => Promise<ExecutableLaunch | undefined>
  readonly spawn: (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>) => AppServerProcess
  /** The folder the teammates work in; a call is refused without one. */
  readonly folder: () => string | undefined
  readonly send: (event: VoiceCallEvent) => void
  readonly version: string
  /** How long the voice service has to answer the offer. */
  readonly answerTimeoutMs?: number
  /** How long End waits for Codex to say it stopped before the server is ended anyway. */
  readonly stopWaitMs?: number
  /**
   * Locust's own add-on server, a private copy for the call (0.739): opened as the call starts, closed with it.
   * Without it a call can look and talk but hand nothing over.
   */
  readonly addOn?: {
    readonly open: () => Promise<VoiceAddOn | undefined>
    readonly close: () => Promise<void>
  }
  /** The person's own connectors stay on for the call, each that would change something asking first (0.742). */
  readonly connectors?: boolean
  /** How long a connector's question waits for Allow. */
  readonly askWaitMs?: number
  /** A drive's throwaway server for a development build's call, so a question can be asked and denied (0.742). */
  readonly testServer?: VoiceAddOn
  /** The voice the person chose in Settings (0.745); Codex's own when there is none. */
  readonly voice?: () => Promise<string | undefined>
  /** The teammate a call is with (0.746): as the voice is told it, with its own voice when it has one. */
  readonly teammate?: (target: VoiceCallTarget) => Promise<(VoiceCallTeammate & { readonly voice?: string }) | undefined>
  /** The conversation the call's add-on may continue while the call is on; undefined once it ends. */
  readonly onTarget?: (target: VoiceCallTarget | undefined) => void
}

export interface VoiceCalls {
  available(): Promise<VoiceCallAvailability>
  /** With `preview`, a short call with only that voice, to hear it before choosing it (0.751): nothing else is on. */
  start(offer: string, target?: VoiceCallTarget, preview?: ChatGptVoice): Promise<VoiceCallStartResult>
  stop(): Promise<void>
  /** The person's Allow or Deny for one question in the strip; an id no call is waiting on changes nothing. */
  answer(id: unknown, allowed: unknown): void
  dispose(): void
}

interface Live {
  readonly client: AppServerClient
  readonly child: AppServerProcess
  threadId?: string
  answer?: (sdp: string) => void
  failed?: (message: string) => void
  ended: boolean
  /** Locust's add-on was opened for this call, and closes with it. */
  addOn?: boolean
  /** Questions waiting for the person's Allow, by the id the strip answers with. */
  readonly asks: Map<string, (allowed: boolean) => void>
  /** The teammate the call is with (0.746), so an ask sent to its conversation is said as one. */
  teammateName?: string
}

/** One start, from its first line: cancelled by Stop at whatever step it has reached. */
interface Ticket {
  cancelled: boolean
  call?: Live
  /** A voice heard before it is chosen (0.751): one preview may take another's place, never a call's. */
  preview?: boolean
}

const ANSWER_TIMEOUT_MS = 20_000
const REQUEST_TIMEOUT_MS = 30_000
const STOP_WAIT_MS = 1_500
const ENDED = 'The call was ended before it began.'

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** A failure already in a person's words: passed on as it is, never said twice. */
class Said extends Error {}

export function createVoiceCalls(deps: VoiceCallDeps): VoiceCalls {
  let current: Ticket | undefined

  const end = (call: Live, why?: string): void => {
    if (call.ended) return
    call.ended = true
    if (current?.call === call) current = undefined
    // A start still waiting on the answer is told why it will not come.
    call.failed?.(why ?? 'The call ended.')
    call.failed = undefined
    // Anything still waiting for Allow did not happen.
    for (const settle of [...call.asks.values()]) settle(false)
    call.client.dispose('The call ended.')
    call.child.kill()
    if (call.addOn === true) void deps.addOn?.close().catch(() => undefined)
    // The add-on may continue the teammate's conversation only while this call is on.
    deps.onTarget?.(undefined)
    deps.send(why === undefined ? { kind: 'ended' } : { kind: 'ended', why })
  }

  const notified = (call: Live, method: string, params: JsonValue | undefined): void => {
    const p = isRecord(params) ? params : {}
    // Another thread's news is not this call's (there is only one, but say so).
    if (call.threadId !== undefined && typeof p.threadId === 'string' && p.threadId !== call.threadId) return
    switch (method) {
      case 'thread/realtime/sdp':
        if (typeof p.sdp === 'string') call.answer?.(p.sdp)
        return
      case 'thread/realtime/transcript/delta': {
        const role = voiceCallRole(p.role)
        if (role !== undefined && typeof p.delta === 'string' && p.delta.length > 0) deps.send({ kind: 'partial', role, delta: p.delta })
        return
      }
      case 'thread/realtime/transcript/done': {
        const role = voiceCallRole(p.role)
        if (role !== undefined && typeof p.text === 'string') deps.send({ kind: 'said', role, text: p.text })
        return
      }
      case 'thread/realtime/error': {
        const message = voiceCallSaid(typeof p.message === 'string' ? p.message : 'The voice service reported an error.')
        if (call.failed !== undefined) {
          // Still starting: the start answers with it.
          call.failed(message)
          return
        }
        deps.send({ kind: 'error', message })
        return
      }
      case 'thread/realtime/closed':
        end(call, typeof p.reason === 'string' && p.reason !== 'requested' ? `The voice service closed the call (${p.reason}).` : undefined)
        return
      // A hand-off to a teammate, said in the strip as it starts (0.739).
      case 'item/started': {
        const item = isRecord(p.item) ? p.item : undefined
        const args = item !== undefined && isRecord(item.arguments) ? item.arguments : undefined
        if (item?.type === 'mcpToolCall' && item.server === VOICE_CALL_ADD_ON && item.tool === 'start_conversation' && args !== undefined) {
          deps.send({ kind: 'handed', to: String(args.teammate ?? '').slice(0, 80), task: String(args.message ?? '').slice(0, 400) })
        }
        // An ask sent into the teammate's own conversation (0.746): said as one, under the teammate's name.
        if (item?.type === 'mcpToolCall' && item.server === VOICE_CALL_ADD_ON && item.tool === 'send_message' && args !== undefined && call.teammateName !== undefined) {
          deps.send({ kind: 'handed', to: call.teammateName, task: String(args.message ?? '').slice(0, 400) })
        }
        return
      }
      // Codex working behind the voice: something it was handed to look at.
      case 'turn/started':
        deps.send({ kind: 'working', working: true })
        return
      case 'turn/completed':
        deps.send({ kind: 'working', working: false })
        return
      default:
    }
  }

  /** Ends one start or call, at whatever step it has reached. */
  const cancel = async (ticket: Ticket): Promise<void> => {
    ticket.cancelled = true
    if (current === ticket) current = undefined
    const call = ticket.call
    if (call === undefined || call.ended) return
    if (call.threadId !== undefined) {
      // Asked to stop, but not waited on for long: a Codex that never answers is ended anyway (#12).
      await Promise.race([
        call.client.request('thread/realtime/stop', { threadId: call.threadId }).catch(() => undefined),
        new Promise((resolve) => setTimeout(resolve, deps.stopWaitMs ?? STOP_WAIT_MS))
      ])
    }
    end(call)
  }

  /** Puts one question in the strip and waits for Allow, Deny, the time running out or the call ending. */
  const ask = (call: Live, asked: VoiceCallAsked): Promise<boolean> =>
    new Promise((resolve) => {
      const id = randomUUID()
      const timer = setTimeout(() => { settle(false) }, deps.askWaitMs ?? VOICE_CALL_ASK_WAIT_MS)
      function settle(allowed: boolean): void {
        if (!call.asks.delete(id)) return
        clearTimeout(timer)
        deps.send({ kind: 'asked', id, allowed })
        resolve(allowed)
      }
      call.asks.set(id, settle)
      deps.send({ kind: 'asks', id, what: asked.what, detail: asked.detail, fields: asked.fields })
    })

  const stop = async (): Promise<void> => {
    const ticket = current
    if (ticket !== undefined) await cancel(ticket)
  }

  return {
    async available() {
      const managed = await deps.policyRefusal?.()
      if (managed !== undefined) return { available: false, why: managed }
      if (deps.folder() === undefined) return { available: false, why: 'Pick a folder first.' }
      const codex = await deps.codex().catch(() => undefined)
      return codex === undefined
        ? { available: false, why: 'Voice calls use ChatGPT’s voice through Codex CLI, which is not ready on this computer.' }
        : { available: true }
    },

    async start(offer, target, preview) {
      // A voice heard before it is chosen never ends a call that is on (0.751).
      if (preview !== undefined && current !== undefined && current.preview !== true) return { ok: false, message: 'A call is on. End it to hear a voice.' }
      // The ticket is taken before the first await, so a second press or an End can always see this start.
      const previous = current
      const ticket: Ticket = { cancelled: false, ...(preview === undefined ? {} : { preview: true }) }
      current = ticket
      if (previous !== undefined) await cancel(previous)
      if (ticket.cancelled) return { ok: false, message: ENDED }
      if (deps.policyRefusal !== undefined) {
        const managed = await deps.policyRefusal(target, preview !== undefined)
        if (ticket.cancelled) return { ok: false, message: ENDED }
        if (managed !== undefined) { if (current === ticket) current = undefined; return { ok: false, message: managed } }
      }
      const folder = deps.folder()
      if (folder === undefined) { if (current === ticket) current = undefined; return { ok: false, message: 'Pick a folder first.' } }
      // The teammate the call is with (0.746); one that cannot be found makes an ordinary call, never a refused one.
      const teammate = target === undefined || preview !== undefined ? undefined : await deps.teammate?.(target).catch(() => undefined)
      if (ticket.cancelled) return { ok: false, message: ENDED }
      const codex = await deps.codex().catch(() => undefined)
      if (ticket.cancelled) return { ok: false, message: ENDED }
      if (codex === undefined) { if (current === ticket) current = undefined; return { ok: false, message: 'Voice calls use ChatGPT’s voice through Codex CLI, which is not ready on this computer.' } }
      const child = deps.spawn(codex.executablePath, [...codex.prefixArgs, 'app-server'], codex.env)
      let call: Live
      const client = createAppServerClient({
        transport: { send: (line) => child.write(line), close: () => child.kill() },
        onNotification: ({ method, params }) => notified(call, method, params),
        /*
         * Nothing in a call acts by itself. Codex asking to use Locust's own add-on is let through: it only hands
         * work to teammates and reads the folder (measured 2026-10-11: an MCP tool call on this thread first asks
         * `mcpServer/elicitation/request`, `codex_approval_kind: mcp_tool_call`; declined, the tool never runs).
         * One of the person's connectors that would change something asks THEM, in the strip (0.742); a command,
         * a file change or anything else is refused, since a call has nowhere to show it.
         */
        onRequest: async (request): Promise<JsonValue> => {
          const p = isRecord(request.params) ? request.params : {}
          const meta = isRecord(p._meta) ? p._meta : {}
          if (request.method === 'mcpServer/elicitation/request') {
            if (p.serverName === VOICE_CALL_ADD_ON) {
              return call.addOn === true && meta.codex_approval_kind === 'mcp_tool_call' ? { action: 'accept', content: {} } : { action: 'decline' }
            }
            const asked = deps.connectors === true && preview === undefined ? voiceCallAsk(p) : undefined
            if (asked === undefined || call.ended) return { action: 'decline' }
            return (await ask(call, asked)) ? { action: 'accept', content: {} } : { action: 'decline' }
          }
          return codexRefusalFor(request.method)
        },
        requestTimeoutMs: REQUEST_TIMEOUT_MS
      })
      call = { client, child, ended: false, asks: new Map() }
      ticket.call = call
      child.onData((chunk) => client.accept(chunk))
      // Nothing if the call already ended; otherwise Codex went away under it.
      child.onExit(() => end(call, 'Codex stopped, so the call ended.'))
      const refuse = (message: string): VoiceCallStartResult => {
        end(call, message)
        return { ok: false, message }
      }
      /** Each step looks at the ticket before going on: an End during the start ends it there. */
      const going = (): boolean => !ticket.cancelled && !call.ended
      try {
        await client.request('initialize', { clientInfo: { name: 'locust', title: 'Locust', version: deps.version }, capabilities: { experimentalApi: true, requestAttestation: false } })
        if (!going()) return refuse(ENDED)
        client.notify('initialized')
        const account = await client.request('account/read', {})
        if (!going()) return refuse(ENDED)
        const kind = isRecord(account) && isRecord(account.account) ? account.account.type : undefined
        if (kind !== 'chatgpt') {
          return refuse(kind === undefined
            ? 'Codex is not signed in. Voice calls use your ChatGPT sign-in: sign Codex in with ChatGPT and try again.'
            : 'Codex is signed in with an API key. Voice calls use your ChatGPT sign-in: sign Codex in with ChatGPT and try again.')
        }
        // The settings alone, which start nothing (#1); unreadable settings place no call (#2).
        const read = await client.request('config/read', { cwd: folder }).catch(() => undefined)
        if (!going()) return refuse(ENDED)
        // Locust's own add-on for the hand-offs; a call without it still talks. A preview has nothing to hand over.
        const addOn = deps.addOn === undefined || preview !== undefined ? undefined : await deps.addOn.open().catch(() => undefined)
        if (addOn !== undefined) {
          // A call that ended while its add-on was opening had nothing to close then, and the add-on stayed up until
          // the next call ended or Locust quit (0.753, bug hunt 2026-10-12, TK3): closed here instead.
          if (call.ended) { void deps.addOn?.close().catch(() => undefined); return refuse(ENDED) }
          call.addOn = true
        }
        if (!going()) return refuse(ENDED)
        // A preview is the voice alone: no connectors, no add-on of the person's, nothing to run (0.751).
        const threadConfig = voiceThreadConfig(isRecord(read) ? read.config : undefined, addOn, preview !== undefined ? {} : { connectors: deps.connectors === true, ...(deps.testServer === undefined ? {} : { testServer: deps.testServer }) })
        if (!threadConfig.ok) return refuse(threadConfig.message)
        const thread = await client.request('thread/start', {
          cwd: folder,
          sandbox: 'read-only',
          approvalPolicy: 'untrusted',
          ephemeral: true,
          config: threadConfig.config as Readonly<Record<string, JsonValue>>
        })
        if (!going()) return refuse(ENDED)
        const threadId = isRecord(thread) && isRecord(thread.thread) && typeof thread.thread.id === 'string' ? thread.thread.id : undefined
        if (threadId === undefined) return refuse('Codex did not start the call’s conversation.')
        call.threadId = threadId
        // The voice chosen in Settings; one that cannot be read is Codex's own, never a refused call.
        const voice = preview ?? teammate?.voice ?? await deps.voice?.().catch(() => undefined)
        if (!going()) return refuse(ENDED)
        // With a teammate and Locust's add-on, the add-on may continue that teammate's conversation, this call only.
        if (teammate !== undefined && addOn !== undefined) {
          call.teammateName = teammate.name
          deps.onTarget?.({ teammateId: teammate.teammateId, ...(teammate.conversationId === undefined ? {} : { conversationId: teammate.conversationId }) })
        }
        const answered = new Promise<string>((resolve, reject) => {
          call.answer = resolve
          call.failed = (message) => reject(new Said(message))
          setTimeout(() => reject(new Said('The voice service did not answer in time. Try again.')), deps.answerTimeoutMs ?? ANSWER_TIMEOUT_MS)
        })
        // Settled whichever way the start goes, so a failure before the answer is never unhandled.
        answered.catch(() => undefined)
        await client.request('thread/realtime/start', preview !== undefined
          ? {
              // The voice alone, told only to greet: nothing of the folder or of Codex's own start goes with it.
              threadId, outputModality: 'audio', transport: { type: 'webrtc', sdp: offer }, version: 'v3', includeStartupContext: false,
              initialItems: [{ role: 'developer', text: voicePreviewRules(preview) }], voice: preview
            }
          : {
              threadId,
              outputModality: 'audio',
              transport: { type: 'webrtc', sdp: offer },
              version: 'v3',
              realtimeStartInstructions: voiceCallInstructions(basename(folder) || folder, addOn !== undefined, deps.connectors === true, teammate),
              // The voice's own rules (0.748): the instructions above reach the Codex model behind it, not the voice.
              initialItems: [{ role: 'developer', text: voiceCallVoiceRules(teammate) }],
              ...(voice === undefined ? {} : { voice })
            })
        const answer = await answered
        call.answer = undefined
        call.failed = undefined
        // An answer for a call that ended meanwhile is not a call (#11).
        if (!going()) return refuse(ENDED)
        return { ok: true, answer }
      } catch (error) {
        const message = ticket.cancelled ? ENDED : error instanceof Said ? error.message : voiceCallSaid(error instanceof Error ? error.message : String(error))
        return call.ended ? { ok: false, message } : refuse(message)
      }
    },

    stop,

    answer(id, allowed) {
      if (typeof id !== 'string' || typeof allowed !== 'boolean') return
      current?.call?.asks.get(id)?.(allowed)
    },

    // Quitting: ended now, not after Codex has had its say.
    dispose() {
      const ticket = current
      if (ticket === undefined) return
      ticket.cancelled = true
      current = undefined
      if (ticket.call !== undefined) end(ticket.call)
    }
  }
}
