import { useEffect, useRef, useState } from 'react'
import { voiceLocalOnly, type VoiceApi, type VoiceMode } from '../../../shared/voice.js'
import type { VoiceCapture } from '../voiceCapture.js'
import { Icon } from './Icon.js'
import { setVoiceState } from '../voiceLevel.js'

type Phase = 'idle' | 'asking' | 'checking' | 'downloading' | 'opening' | 'recording' | 'transcribing'
export const VOICE_DOWNLOAD_WORDS = 'Voice typing needs a one-time 41 MB download. It runs on this computer; nothing you say leaves it.'
export const VOICE_ACCURATE_WORDS = 'Accurate voice typing needs a one-time 69 MB download. It is slower than Fast, and it runs on this computer; nothing you say leaves it.'
export const VOICE_OPENAI_WORDS = 'Voice typing will send your recordings to OpenAI using your API key. OpenAI API charges apply. Allow this for future recordings?'
export function VoiceButton({ platform, onText, disabled = false, api = typeof window === 'undefined' ? undefined : window.desktop?.voice }: {
  readonly platform: string; readonly onText: (text: string) => void; readonly disabled?: boolean; readonly api?: VoiceApi
}) {
  const [phase, setPhase] = useState<Phase>('idle')
  const [seconds, setSeconds] = useState(0)
  const [percent, setPercent] = useState(0)
  const [message, setMessage] = useState('')
  const [mode, setMode] = useState<VoiceMode>('fast')
  const selectedMode = useRef<VoiceMode>('fast')
  const current = useRef<Phase>('idle')
  const capture = useRef<VoiceCapture | undefined>(undefined)
  const controller = useRef<AbortController | undefined>(undefined)
  const generation = useRef(0)
  const transition = (next: Phase): void => { current.current = next; setPhase(next) }
  // The chat box glows while it listens, and while its words are typed (voiceState, 0.686); never left on.
  useEffect(() => {
    setVoiceState(phase === 'recording' ? 'listening' : phase === 'transcribing' ? 'processing' : 'off')
  }, [phase])
  useEffect(() => () => setVoiceState('off'), [])
  const cancel = (): void => {
    generation.current++
    controller.current?.abort()
    void capture.current?.cancel()
    capture.current = undefined
    void api?.cancel()
    transition('idle')
    setMessage('')
  }
  useEffect(() => {
    const escape = (event: globalThis.KeyboardEvent): void => {
      if (event.key === 'Escape' && current.current !== 'idle') {
        event.preventDefault(); event.stopImmediatePropagation(); cancel()
      }
    }
    window.addEventListener('keydown', escape, true)
    return () => {
      window.removeEventListener('keydown', escape, true)
      generation.current++; controller.current?.abort()
      void capture.current?.cancel(); void api?.cancel()
    }
  }, [api])
  useEffect(() => {
    if (phase !== 'recording') return
    const started = performance.now()
    setSeconds(0)
    const timer = setInterval(() => { setSeconds(Math.floor((performance.now() - started) / 1000)) }, 250)
    return () => { clearInterval(timer) }
  }, [phase])
  const fail = (error: unknown, token: number): void => {
    if (generation.current !== token) return
    transition('idle')
    setMessage(error instanceof Error && error.message.includes('Your message') ? error.message : 'Voice typing could not finish. Your message is unchanged; check microphone access and try again.')
  }
  const stop = async (): Promise<void> => {
    if (current.current !== 'recording' || capture.current === undefined || api === undefined) return
    const token = generation.current
    const recording = capture.current
    capture.current = undefined
    transition('transcribing')
    try {
      const wav = await recording.stop()
      if (generation.current !== token) return
      const result = await api.transcribe(wav, selectedMode.current)
      if (generation.current !== token) return
      if (!result.ok) { transition('idle'); setMessage(result.message); return }
      transition('idle')
      if (result.text?.trim()) onText(result.text)
      else setMessage('No words were heard. Your message is unchanged; try again.')
    } catch (error) { fail(error, token) }
  }
  const record = async (token: number): Promise<void> => {
    if (generation.current !== token) return
    transition('opening')
    const abort = new AbortController()
    controller.current = abort
    const { startVoiceCapture } = await import('../voiceCapture.js')
    if (generation.current !== token) return
    const recording = await startVoiceCapture(abort.signal, () => { void stop() })
    if (generation.current !== token) { await recording.cancel(); return }
    capture.current = recording
    transition('recording')
  }
  const press = async (): Promise<void> => {
    if (current.current === 'recording') { await stop(); return }
    if (current.current !== 'idle' || api === undefined) return
    const token = ++generation.current
    setMessage(''); transition('checking')
    try {
      const settings = await api.settings?.()
      if (generation.current !== token) return
      selectedMode.current = settings?.mode ?? 'fast'
      setMode(selectedMode.current)
      // Off Windows, only the OpenAI route types (0.843): a local mode says so, instead of offering a download.
      if (platform !== 'win32' && settings?.mode !== 'openai') { transition('idle'); setMessage(voiceLocalOnly(platform)); return }
      if (settings?.mode === 'openai') {
        if (!settings.hasKey) { transition('idle'); setMessage('Add your OpenAI API key in Settings > General. Your message is unchanged.'); return }
        if (!settings.openaiConsent) { transition('asking'); return }
        await record(token)
        return
      }
      const ready = await api.ready(selectedMode.current)
      if (generation.current !== token) return
      if (!ready) { transition('asking'); return }
      await record(token)
    } catch (error) { fail(error, token) }
  }
  const download = async (): Promise<void> => {
    if (api === undefined || current.current !== 'asking') return
    const token = generation.current
    transition('downloading'); setPercent(0)
    const unsubscribe = api.onProgress((progress) => { if (generation.current === token) setPercent(Math.max(0, Math.min(100, progress))) })
    try {
      const result = selectedMode.current === 'openai'
        ? await api.allowOpenAI?.()
        : await api.download(selectedMode.current)
      if (generation.current !== token) return
      if (result === undefined) { transition('idle'); setMessage('Voice typing could not start. Your message is unchanged.'); return }
      if (!result.ok) { transition('idle'); setMessage(result.message); return }
      await record(token)
    } catch (error) { fail(error, token) } finally { unsubscribe() }
  }
  if (platform !== 'win32' && platform !== 'darwin') return null
  const recording = phase === 'recording'
  const busy = phase !== 'idle' && phase !== 'recording'
  const clock = `${String(Math.floor(seconds / 60))}:${String(seconds % 60).padStart(2, '0')}`
  return <span className="lc-voice">
    <button type="button" className={`lc-control lc-voice__button${recording ? ' is-recording' : ''}`} onMouseDown={(event) => { event.preventDefault() }} onClick={() => { void press() }} disabled={disabled || busy || api === undefined} aria-label={recording ? 'Stop voice typing' : 'Voice typing'} aria-pressed={recording} title={recording ? 'Stop and insert the words. Esc cancels.' : 'Type with your microphone'}>
      <Icon name="mic" size={17} />
      {recording ? <span>{clock}</span> : phase === 'downloading' ? <span>{percent}%</span> : busy ? <span>{phase === 'transcribing' ? 'Typing…' : phase === 'asking' ? '' : 'Starting…'}</span> : null}
    </button>
    {phase === 'asking' && <div className="lc-voice__ask" role="dialog" aria-label={mode === 'openai' ? 'Allow OpenAI voice typing' : 'Download voice typing'}>
      <p>{mode === 'openai' ? VOICE_OPENAI_WORDS : mode === 'accurate' ? VOICE_ACCURATE_WORDS : VOICE_DOWNLOAD_WORDS}</p>
      <button type="button" className="lc-control lc-control--boxed" onClick={() => { void download() }}>{mode === 'openai' ? 'Allow' : 'Download'}</button>
      <button type="button" className="lc-control" onClick={cancel}>Not now</button>
    </div>}
    {message && <span className="lc-voice__message" role="status">{message}</span>}
  </span>
}
