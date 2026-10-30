import { useEffect, useLayoutEffect, useRef, useState } from 'react'

import { talkRunsOn, voiceCallText, type VoiceCallApi, type VoiceCallAvailability, type VoiceCallRole } from '../../../shared/voice-call.js'
import type { AvatarSpec } from '../../../shared/avatar.js'
import type { TeammateHue } from '../../../shared/ipc.js'
import { TeammateBot } from './TeammateBot.js'
import { VoiceOrb } from './VoiceOrb.js'
import { MAX_CALL_LINE } from '../../../shared/calls.js'
import { keptCall, liveCall, markCallSaved, setLiveCall, setLiveCallAnswer, startCallHandOffs, updateLiveCall, useCallHandOffs } from '../liveCall.js'
import { todoCount } from '../callTodos.js'
import { clearTalkPress, setTalkPress } from '../talkPress.js'
import { CallTodoList } from './CallWords.js'
import type { LiveVoiceCall } from '../voiceCall.js'
import { setVoiceLevel, setVoiceState } from '../voiceLevel.js'
import { Icon } from './Icon.js'

/**
 * TALK (0.737): a voice call with ChatGPT's own voice, placed by Codex -- the waveform beside the
 * microphone, as Claude's and ChatGPT's apps draw it (Colin's screenshot: "Use voice mode" next to the
 * dictation mic). Voice typing types what you say; this talks back.
 *
 * The strip above the chat box carries the call: who is speaking, the lines as they are said, Mute and
 * End. When it ends the lines stay, and one press puts them in the chat box for any teammate.
 */
type Phase = 'idle' | 'calling' | 'live' | 'ended'
interface Line { readonly role: VoiceCallRole; readonly text: string }

export const VOICE_CALL_TITLE = 'Talk with ChatGPT’s voice. It can read this folder and change nothing.'

/**
 * The teammate a call is with (0.746): Talk pressed in their conversation is a call WITH them -- their face in the
 * strip, their name on the lines, and what you ask goes into that conversation, where they do it.
 */
export interface VoiceCallTalkTo {
  readonly teammateId: string
  readonly name: string
  readonly hue: TeammateHue
  readonly avatar: AvatarSpec
  /** The conversation on screen, when it is theirs. */
  readonly conversationId?: string
  /** The call is drawn in the conversation on screen, so a connector's question is a card there, not a row here (0.749). */
  readonly asksInThread?: boolean
}

export function VoiceCallButton({ platform, onText, api = typeof window === 'undefined' ? undefined : window.desktop?.voiceCall, availability: given, talkTo }: {
  readonly platform: string
  readonly onText: (text: string) => void
  /** The teammate whose conversation this is (0.746); absent, Talk is ChatGPT's own voice. */
  readonly talkTo?: VoiceCallTalkTo
  readonly api?: VoiceCallApi
  /** For a test: what `available()` would say. */
  readonly availability?: VoiceCallAvailability
}) {
  const [availability, setAvailability] = useState<VoiceCallAvailability | undefined>(given)
  const [phase, setPhase] = useState<Phase>('idle')
  const [lines, setLines] = useState<readonly Line[]>([])
  /** What the voice handed to teammates (0.739), as it said so: the words "Put it in the chat box" carries. */
  const [handed, setHanded] = useState<readonly { readonly to: string; readonly task: string }[]>([])
  /** The turns those hand-offs started (0.755): the call's to-dos, each with how it is going. */
  const todos = useCallHandOffs()
  /** A connector waiting for Allow (0.742): what it would do, until it is answered. */
  const [asks, setAsks] = useState<readonly { readonly id: string; readonly what: string; readonly detail: string }[]>([])
  const [partial, setPartial] = useState<Readonly<Record<VoiceCallRole, string>>>({ you: '', voice: '' })
  const [message, setMessage] = useState('')
  const [muted, setMuted] = useState(false)
  const [working, setWorking] = useState(false)
  const [levels, setLevels] = useState({ you: 0, voice: 0 })
  const [seconds, setSeconds] = useState(0)
  const call = useRef<LiveVoiceCall | undefined>(undefined)
  /** The start under way, so End and leaving the conversation can reach it (0.738, Sol's review #5). */
  const starting = useRef<AbortController | undefined>(undefined)
  const generation = useRef(0)
  const anchor = useRef<HTMLSpanElement>(null)
  const stripBox = useRef<HTMLDivElement>(null)
  /** When each one began the line they are saying (0.748): a line goes in the thread where it began, not where it ended. */
  const lineFrom = useRef<Partial<Record<VoiceCallRole, number>>>({})
  /** The call's words could not be saved with the conversation (0.753): said, and left on screen, never claimed as kept. */
  const [notKept, setNotKept] = useState(false)
  const [lift, setLift] = useState(0)
  const current = useRef<Phase>('idle')
  /*
   * A CALL WITH A TEAMMATE IS KEPT WITH THEIR CONVERSATION (0.747): its lines go to the live call the thread reads,
   * and when it ends it is saved, so the thread can put the words back among the turns it started.
   */
  const keep = (): void => {
    const call = liveCall()
    if (call === undefined) return
    const done = keptCall(call, lineFrom.current)
    lineFrom.current = {}
    setLiveCallAnswer(undefined)
    setLiveCall({ ...call, ...done, phase: 'ended', partial: { you: '', voice: '' }, asks: [] })
    if (done.lines.length === 0 && done.turns.length === 0) { setLiveCall(undefined); return }
    void (window.desktop?.calls?.save(done) ?? Promise.resolve(false)).catch(() => false).then((saved) => {
      // Saved: the thread reads it back from the log. Not saved: its words stay on screen from the live call, and the
      // bar says it was not kept -- it said "kept in this conversation" either way (bug hunt 2026-10-12, with TK1).
      if (saved && liveCall()?.callId === done.callId) setLiveCall(undefined)
      if (!saved) setNotKept(true)
      markCallSaved()
    })
  }
  const transition = (next: Phase): void => {
    current.current = next
    setPhase(next)
    if (talkTo === undefined) return
    // Counted from when the call connected, as the strip's clock is (0.748).
    if (next === 'live') updateLiveCall((call) => ({ ...call, phase: 'live', startedAt: Date.now() }))
    if (next === 'ended') keep()
  }

  useEffect(() => {
    if (given !== undefined || api === undefined || !talkRunsOn(platform)) return
    let alive = true
    void api.available().then((answer) => { if (alive) setAvailability(answer) }).catch(() => undefined)
    return () => { alive = false }
  }, [api, given, platform])

  // What is said, as Codex hears and speaks it.
  useEffect(() => {
    if (api === undefined || (phase !== 'calling' && phase !== 'live')) return
    return api.onEvent((update) => {
      if (update.kind === 'partial') {
        lineFrom.current[update.role] ??= Date.now()
        setPartial((was) => ({ ...was, [update.role]: was[update.role] + update.delta }))
        if (talkTo !== undefined) updateLiveCall((call) => ({ ...call, partial: { ...call.partial, [update.role]: call.partial[update.role] + update.delta } }))
      } else if (update.kind === 'said') {
        setPartial((was) => ({ ...was, [update.role]: '' }))
        const text = update.text.trim()
        const began = lineFrom.current[update.role] ?? Date.now()
        lineFrom.current[update.role] = undefined
        if (text.length > 0) setLines((was) => [...was, { role: update.role, text }])
        if (talkTo !== undefined) {
          updateLiveCall((call) => ({
            ...call,
            partial: { ...call.partial, [update.role]: '' },
            lines: text.length === 0 ? call.lines : [...call.lines, { role: update.role, text: text.slice(0, MAX_CALL_LINE), at: began }]
          }))
        }
      } else if (update.kind === 'working') setWorking(update.working)
      else if (update.kind === 'handed') setHanded((was) => [...was, { to: update.to, task: update.task }])
      else if (update.kind === 'asks') {
        setAsks((was) => [...was, { id: update.id, what: update.what, detail: update.detail }])
        if (talkTo !== undefined) updateLiveCall((call) => ({ ...call, asks: [...(call.asks ?? []), { id: update.id, what: update.what, detail: update.detail, fields: update.fields ?? [] }] }))
      } else if (update.kind === 'asked') {
        setAsks((was) => was.filter((asked) => asked.id !== update.id))
        if (talkTo !== undefined) updateLiveCall((call) => ({ ...call, asks: (call.asks ?? []).filter((asked) => asked.id !== update.id) }))
      }
      else if (update.kind === 'error') setMessage(update.message)
      else if (update.kind === 'ended' && current.current === 'live') {
        setAsks([])
        void call.current?.hangUp()
        call.current = undefined
        if (update.why !== undefined) setMessage(update.why)
        transition('ended')
      } else if (update.kind === 'ended' && current.current === 'calling') {
        // Ended on the other side while it was starting (the window closed to the tray): the start's own
        // answer says why, if it has anything to say.
        starting.current?.abort()
      }
    })
  }, [api, phase])

  // The strip's bars and the chat box's glow follow whoever is talking; nothing runs between calls.
  useEffect(() => {
    if (phase !== 'live') return
    setVoiceState('listening')
    const started = performance.now()
    const timer = setInterval(() => {
      const now = call.current?.levels() ?? { you: 0, voice: 0 }
      setLevels(now)
      setVoiceLevel(Math.max(now.you, now.voice))
      setSeconds(Math.floor((performance.now() - started) / 1000))
    }, 100)
    return () => {
      clearInterval(timer)
      setVoiceLevel(0)
      setVoiceState('off')
    }
  }, [phase])

  /*
   * AND THE THREAD ENDS ABOVE IT (0.748). The strip floats over the bottom of the conversation, and with a call's lines
   * in the thread the newest one -- the words still being said -- sat under it. While it is up, the chat box keeps
   * the strip's height clear above itself (--lc-call-room on its form), so the thread's last line stays in view.
   */
  useLayoutEffect(() => {
    const form = anchor.current?.closest('form')
    const strip = stripBox.current
    if (phase === 'idle' || form === null || form === undefined || strip === null) return
    const room = (): void => { form.style.setProperty('--lc-call-room', `${String(Math.ceil(strip.getBoundingClientRect().height) + 12)}px`) }
    room()
    const observer = new ResizeObserver(room)
    observer.observe(strip)
    return () => { observer.disconnect(); form.style.removeProperty('--lc-call-room') }
  }, [phase])

  // The strip stands above the whole chat box, not on its text: lifted from this button to the box's top
  // edge, and kept there if the box grows (the drive's first frame had it over "Message Wren...").
  useLayoutEffect(() => {
    if (phase === 'idle') return
    const span = anchor.current
    const form = span?.closest('form')
    if (span === null || span === undefined || form === null || form === undefined) return
    const measure = (): void => { setLift(Math.max(0, span.getBoundingClientRect().top - form.getBoundingClientRect().top)) }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(form)
    return () => { observer.disconnect() }
  }, [phase])

  // Leaving the conversation ends its call: no voice goes on talking from a box nobody can see.
  useEffect(() => () => {
    generation.current += 1
    starting.current?.abort()
    void call.current?.hangUp()
    call.current = undefined
    // A call with a teammate that ended this way is kept like any other.
    if (liveCall() !== undefined && liveCall()?.phase !== 'ended') keep()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const start = async (): Promise<void> => {
    if (api === undefined || current.current === 'calling' || current.current === 'live') return
    setLines([]); setHanded([]); setPartial({ you: '', voice: '' }); setMessage(''); setMuted(false); setWorking(false); setSeconds(0); setNotKept(false)
    lineFrom.current = {}
    startCallHandOffs()
    if (talkTo !== undefined) {
      setLiveCall({
        callId: `call_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`,
        teammateId: talkTo.teammateId, name: talkTo.name,
        ...(talkTo.conversationId === undefined ? {} : { afterMissionId: talkTo.conversationId }),
        startedAt: Date.now(), lines: [], turns: [], phase: 'calling', partial: { you: '', voice: '' }
      })
      setLiveCallAnswer((id, allowed) => api.answer(id, allowed))
    }
    transition('calling')
    const token = ++generation.current
    const controller = new AbortController()
    starting.current = controller
    try {
      const { openVoiceCall } = await import('../voiceCall.js')
      // With a teammate, the call is placed WITH them, for the conversation on screen as Talk was pressed.
      const target = talkTo === undefined ? undefined : { teammateId: talkTo.teammateId, ...(talkTo.conversationId === undefined ? {} : { conversationId: talkTo.conversationId }) }
      const callApi: VoiceCallApi = target === undefined ? api : { ...api, start: (offer) => api.start(offer, target) }
      const opened = await openVoiceCall(callApi, (why) => {
        if (generation.current !== token) return
        call.current = undefined
        setMessage(why)
        transition('ended')
      }, controller.signal)
      // A start that was ended, left, or overtaken by a newer one is hung up, never adopted (#5).
      if (generation.current !== token || controller.signal.aborted || (current.current as Phase) !== 'calling') { await opened.hangUp(); return }
      call.current = opened
      transition('live')
    } catch (error) {
      if (generation.current !== token) return
      const { VoiceCallAborted, VoiceCallRefused } = await import('../voiceCall.js')
      if (error instanceof VoiceCallAborted) {
        if ((current.current as Phase) === 'calling') transition('ended')
        return
      }
      setMessage(error instanceof VoiceCallRefused ? error.message : 'The call could not start. Try again.')
      transition('ended')
    } finally {
      if (starting.current === controller) starting.current = undefined
    }
  }
  const end = async (): Promise<void> => {
    const live = call.current
    call.current = undefined
    // A call still starting is reached through its controller: the microphone closes and Codex is told now.
    starting.current?.abort()
    transition('ended')
    await live?.hangUp()
  }
  const close = (): void => { transition('idle'); setMessage('') }

  /*
   * PRESSED BY THE KEYS TOO (0.755, talkPress.ts): while this button is on screen and Talk is ready, the shortcut
   * presses it from any app -- a call starts, or the one that is on ends.
   */
  const pressed = useRef<() => void>(() => undefined)
  pressed.current = () => { void (current.current === 'calling' || current.current === 'live' ? end() : start()) }
  const ready = talkRunsOn(platform) && api !== undefined && availability?.available === true
  useEffect(() => {
    if (!ready) return
    const mine = (): void => { pressed.current() }
    setTalkPress(mine)
    return () => { clearTalkPress(mine) }
  }, [ready])

  if (!ready) return null
  const on = phase === 'calling' || phase === 'live'
  const speaking = levels.voice > 0.08
  // Who speaks on the lines and in the status: the teammate the call is with (0.746), else ChatGPT.
  const voiceName = talkTo?.name ?? 'ChatGPT'
  const status = phase === 'calling'
    ? talkTo === undefined ? 'Calling…' : `Calling ${talkTo.name}…`
    : asks.length > 0 ? 'Waiting for you to allow it' : working ? talkTo === undefined ? 'Working on it…' : `${talkTo.name} is on it…` : speaking ? `${voiceName} is speaking` : muted ? 'Muted' : 'Listening'
  // The teammate's face does what the call does: speaks, works, waits on you; still while it listens.
  const faceActivity = asks.length > 0 ? 'waiting' : working ? 'working' : speaking ? 'responding' : 'idle'
  const clock = `${String(Math.floor(seconds / 60))}:${String(seconds % 60).padStart(2, '0')}`
  const shown = phase === 'ended' ? lines : lines.slice(-3)
  const loud = Math.max(levels.you, levels.voice)
  // On a call with a teammate whose conversation is on screen, a connector's question is a card there (0.749).
  const asksInThread = talkTo?.asksInThread === true
  return <span className="lc-call" ref={anchor}>
    <button
      type="button"
      className={`lc-control lc-call__button${on ? ' is-on' : ''}`}
      onMouseDown={(event) => { event.preventDefault() }}
      onClick={() => { void (on ? end() : start()) }}
      aria-label={on ? 'End the voice call' : 'Talk'}
      aria-pressed={on}
      title={on ? 'End the call' : talkTo === undefined ? VOICE_CALL_TITLE : `Talk to ${talkTo.name}. What you ask goes to ${talkTo.name} in this conversation; the voice is ChatGPT’s.`}
    >
      <Icon name="waveform" size={17} />
    </button>
    {phase !== 'idle' && <div ref={stripBox} className="lc-call__strip" role="region" aria-label="Voice call" style={{ bottom: `calc(100% + ${String(lift + 10)}px)` }}>
      <div className="lc-call__head">
        {phase === 'ended'
          ? <span className="lc-call__status">{talkTo === undefined ? 'Call ended' : `Call with ${talkTo.name} ended`}{seconds > 0 ? ` · ${clock}` : ''}{talkTo === undefined && todos.length > 0 ? ` · ${todoCount(todos.length)}` : ''}{talkTo !== undefined && lines.length > 0 ? (notKept ? ' · not kept: it could not be saved' : ' · kept in this conversation') : ''}</span>
          : <>
            {talkTo !== undefined && <TeammateBot hue={talkTo.hue} avatar={talkTo.avatar} size={28} teammateId={talkTo.teammateId} motion="full" hopsWhenDone={false}
              activity={faceActivity} presence={asks.length > 0 ? 'approval' : working ? 'working' : 'none'} />}
            {/* ChatGPT's own voice has a face too (0.752): the orb, where a teammate's call wears the teammate's. */}
            {talkTo === undefined
              ? <VoiceOrb size={28} level={phase === 'live' ? (speaking ? levels.voice : levels.you * 0.6) : 0} speaking={speaking} active={phase === 'live'} />
              : <span className={`lc-call__bars${speaking ? ' is-voice' : ''}`} aria-hidden="true">
                {[0.55, 0.85, 1, 0.85, 0.55].map((weight, index) => (
                  <span key={index} className="lc-call__bar" style={{ transform: `scaleY(${String(0.18 + 0.82 * Math.sqrt(Math.min(1, loud * weight * (phase === 'live' ? 1 : 0))))})` }} />
                ))}
              </span>}
            <span className="lc-call__status" role="status">{status}{phase === 'live' ? ` · ${clock}` : ''}</span>
          </>}
        <span className="lc-call__acts">
          {phase === 'live' && <button type="button" className="lc-control" aria-pressed={muted} onClick={() => { const next = !muted; setMuted(next); call.current?.setMuted(next) }}>
            <Icon name={muted ? 'mic-off' : 'mic'} size={15} />{muted ? 'Unmute' : 'Mute'}
          </button>}
          {on && asksInThread && asks.length > 0 && <button type="button" className="lc-control lc-control--boxed" onClick={() => { document.querySelector('[data-call-ask]')?.scrollIntoView({ block: 'center' }) }}>Show</button>}
          {on && <button type="button" className="lc-control lc-call__end" onClick={() => { void end() }}>End</button>}
          {phase === 'ended' && talkTo === undefined && voiceCallText(lines, handed).length > 0 && <button type="button" className="lc-control lc-control--boxed" onClick={() => { onText(voiceCallText(lines, handed)); close() }}>Put it in the chat box</button>}
          {phase === 'ended' && <button type="button" className="lc-control" onClick={close}>Close</button>}
        </span>
      </div>
      {/*
        * WHAT THE CALL HANDED OUT (0.755): each to-do as it starts, with how it is going and one press to open it --
        * and, when the call ends, first, as its summary. A call with a teammate sums them up in their conversation.
        */}
      {/* Opened once the call is over: opening a conversation leaves this one, and leaving it ends the call. */}
      {talkTo === undefined && todos.length > 0 && <CallTodoList handed={todos} label={phase === 'ended' ? 'What the call handed out' : 'Handed out on this call'} openable={phase === 'ended'} />}
      {/* A call with a teammate says its lines in their conversation (0.747); the strip keeps the call's controls. */}
      {talkTo === undefined && (shown.length > 0 || partial.you.length > 0 || partial.voice.length > 0) && <div className="lc-call__lines">
        {shown.map((line, index) => <p key={index} className={`lc-call__line is-${line.role}`}><span className="lc-call__who">{line.role === 'you' ? 'You' : voiceName}</span>{line.text}</p>)}
        {on && (['you', 'voice'] as const).map((role) => partial[role].trim().length > 0 && <p key={`partial-${role}`} className={`lc-call__line is-${role} is-partial`}><span className="lc-call__who">{role === 'you' ? 'You' : voiceName}</span>{partial[role].trim()}</p>)}
      </div>}
      {/* A connector that would change something, asked here: nothing happens until Allow (0.742). With a teammate
          whose conversation is on screen, it is asked on a card there, and the strip's Show points at it (0.749). */}
      {on && !asksInThread && asks.map((asked) => <div key={asked.id} className="lc-call__ask" role="group" aria-label={`Allow ${asked.what}?`}>
        <p className="lc-call__asktext"><span className="lc-call__who">Allow?</span>{asked.what}{asked.detail.length > 0 && <span className="lc-call__askdetail">{asked.detail}</span>}</p>
        <span className="lc-call__askacts">
          <button type="button" className="lc-control lc-control--boxed" onClick={() => { void api.answer(asked.id, true) }}>Allow</button>
          <button type="button" className="lc-control" onClick={() => { void api.answer(asked.id, false) }}>Deny</button>
        </span>
      </div>)}
      {message && <p className="lc-call__message">{message}</p>}
    </div>}
  </span>
}
