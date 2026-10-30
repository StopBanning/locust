import { isVoicePreviewAudio, voiceName, type ChatGptVoice, type VoiceCallApi } from '../../shared/voice-call.js'
import { gathered } from './voiceCall.js'

/**
 * A VOICE, HEARD BEFORE IT IS CHOSEN (0.751). See shared/voice-call.ts (voicePreviewRules) for why it is a call.
 *
 * The greeting a voice gave before plays at once. The first time, the window makes a call with no microphone -- its
 * one track is silence it controls -- plays a spoken "Hello?" into it, which never reaches the speakers, and plays
 * the voice's answer out loud as it comes. What the voice said is recorded from the moment the hello ends and kept,
 * so the next play is the recording. Every way out closes the call and tells Codex to stop.
 */
export class VoicePreviewFailed extends Error {}

export interface VoicePreviewTimings {
  readonly gatherMs: number
  readonly connectMs: number
  /** How long the voice has to answer the hello. */
  readonly answerMs: number
  /** The longest a greeting may go on before it is kept as it stands. */
  readonly speakMs: number
}

const TIMINGS: VoicePreviewTimings = { gatherMs: 3_000, connectMs: 15_000, answerMs: 15_000, speakMs: 10_000 }

const pause = (ms: number, signal?: AbortSignal): Promise<void> => new Promise((resolve) => {
  const timer = setTimeout(resolve, ms)
  signal?.addEventListener('abort', () => { clearTimeout(timer); resolve() }, { once: true })
})

/** Plays a kept greeting to its end, or until the play is stopped. */
/**
 * Plays a kept greeting to its end, or until the play is stopped. Decoded and played by Web Audio, not from a blob:
 * address: the window's own policy plays media from itself only (media-src falls to default-src 'self'), and a
 * greeting is not a reason to widen it -- the first build tried an <audio> on a blob: URL and was refused.
 */
async function playKept(audio: Uint8Array, signal: AbortSignal | undefined, onPlaying: () => void): Promise<void> {
  const context = new AudioContext()
  try {
    const decoded = await context.decodeAudioData(audio.slice().buffer).catch(() => { throw new VoicePreviewFailed('The kept greeting could not play.') })
    await context.resume().catch(() => undefined)
    const source = context.createBufferSource()
    source.buffer = decoded
    source.connect(context.destination)
    await new Promise<void>((resolve) => {
      source.onended = () => { resolve() }
      signal?.addEventListener('abort', () => { try { source.stop() } catch { /* not started */ } resolve() }, { once: true })
      source.start()
      onPlaying()
    })
  } finally {
    void context.close().catch(() => undefined)
  }
}

export async function playVoicePreview(
  api: VoiceCallApi,
  voice: ChatGptVoice,
  { signal, onPlaying = () => undefined, timings = TIMINGS }: { readonly signal?: AbortSignal; readonly onPlaying?: () => void; readonly timings?: VoicePreviewTimings } = {}
): Promise<'kept' | 'called'> {
  const name = voiceName(voice)
  /** Stopped by the person: a function, since the signal changes under every await. */
  const stopped = (): boolean => signal?.aborted === true
  const kept = await api.previewSaved?.(voice).catch(() => undefined)
  if (kept !== undefined && isVoicePreviewAudio(kept)) {
    await playKept(kept, signal, onPlaying)
    return 'kept'
  }
  if (api.preview === undefined || api.previewTrigger === undefined) throw new VoicePreviewFailed(`${name} cannot be played here.`)
  const hello = await api.previewTrigger().catch(() => undefined)
  if (hello === undefined) throw new VoicePreviewFailed(`Locust could not make the hello ${name} answers. The computer’s own speech is needed for it.`)
  if (stopped()) return 'called'

  const context = new AudioContext()
  const silence = context.createMediaStreamDestination()
  const connection = new RTCPeerConnection()
  const played = new Audio()
  played.autoplay = true
  let remote: MediaStream | undefined
  let analyser: AnalyserNode | undefined
  let asked = false
  let closed = false
  const close = (): void => {
    if (closed) return
    closed = true
    connection.close()
    played.srcObject = null
    void context.close().catch(() => undefined)
    if (asked) void api.stop().catch(() => undefined)
  }
  /*
   * THE VOICE'S ANSWER: begun when its words or its sound arrive, over when it has gone quiet after speaking. Codex's
   * v3 sends the voice's finished line late or not at all (measured 2026-10-12: the greeting played, its "said" never
   * came in 15 s), so the sound says when it is over. What happens to another call before this one is asked for --
   * the end of the preview it replaced -- is not this one's.
   */
  let begun = false
  let saidLine = false
  let ours = false
  let failure: string | undefined
  const off = api.onEvent((event) => {
    if (event.kind === 'partial' && event.role === 'voice' && ours && !begun) { begun = true; onPlaying() }
    else if (event.kind === 'said' && event.role === 'voice' && ours) saidLine = true
    else if (event.kind === 'error' && ours) failure ??= event.message
    else if (event.kind === 'ended' && ours) failure ??= event.why ?? `The call with ${name} ended before it answered.`
  })
  try {
    const track = silence.stream.getAudioTracks()[0]
    if (track === undefined) throw new VoicePreviewFailed(`${name} could not be called from this window.`)
    connection.addTrack(track, silence.stream)
    connection.createDataChannel('oai-events')
    connection.ontrack = (event) => {
      remote = event.streams[0] ?? new MediaStream([event.track])
      played.srcObject = remote
      void played.play().catch(() => undefined)
      analyser = context.createAnalyser()
      analyser.fftSize = 1024
      context.createMediaStreamSource(remote).connect(analyser)
    }
    await connection.setLocalDescription(await connection.createOffer())
    await gathered(connection, timings.gatherMs)
    const offer = connection.localDescription?.sdp
    if (offer === undefined || offer.length === 0 || stopped()) return 'called'
    asked = true
    const started = await api.preview(offer, voice)
    if (!started.ok) { asked = false; throw new VoicePreviewFailed(started.message) }
    ours = true
    if (stopped()) return 'called'
    await connection.setRemoteDescription({ type: 'answer', sdp: started.answer })
    await new Promise<void>((resolve, reject) => {
      if (connection.connectionState === 'connected') return resolve()
      const timer = setTimeout(() => { reject(new VoicePreviewFailed(`${name} did not connect. Check your internet connection and try again.`)) }, timings.connectMs)
      connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'connected') { clearTimeout(timer); resolve() }
        else if (connection.connectionState === 'failed' || connection.connectionState === 'closed') { clearTimeout(timer); reject(new VoicePreviewFailed(`${name} did not connect. Try again.`)) }
      }
    })
    await pause(600, signal)
    if (stopped()) return 'called'
    // The hello, into the call only.
    const spoken = await context.decodeAudioData(hello.slice().buffer)
    const source = context.createBufferSource()
    source.buffer = spoken
    source.connect(silence)
    await new Promise<void>((resolve) => { source.onended = () => { resolve() }; source.start() })
    // What comes back from here is the greeting.
    const chunks: Blob[] = []
    const recorder = remote === undefined ? undefined : new MediaRecorder(remote, { mimeType: 'audio/webm;codecs=opus' })
    if (recorder !== undefined) {
      recorder.ondataavailable = (event) => { if (event.data.size > 0) chunks.push(event.data) }
      recorder.start(250)
    }
    const buffer = new Float32Array(1024)
    const loudNow = (): boolean => {
      if (analyser === undefined) return false
      analyser.getFloatTimeDomainData(buffer)
      let sum = 0
      for (const sample of buffer) sum += sample * sample
      return Math.sqrt(sum / buffer.length) >= 0.01
    }
    const waitFrom = Date.now()
    let heardAt: number | undefined
    let quietFor = 0
    for (;;) {
      await pause(100, signal)
      if (stopped()) return 'called'
      if (failure !== undefined) throw new VoicePreviewFailed(failure)
      const loud = loudNow()
      if (loud && heardAt === undefined) {
        heardAt = Date.now()
        if (!begun) { begun = true; onPlaying() }
      }
      if (heardAt === undefined) {
        if (Date.now() - waitFrom > timings.answerMs) throw new VoicePreviewFailed(`${name} did not answer. Try again.`)
        continue
      }
      quietFor = loud ? 0 : quietFor + 100
      // Over: quiet a moment after its line was said, quiet long enough after speaking, or too long for a hello.
      if ((saidLine && quietFor >= 400) || quietFor >= 900 || Date.now() - heardAt > timings.speakMs) break
    }
    if (recorder !== undefined && recorder.state === 'recording') {
      await new Promise<void>((resolve) => { recorder.onstop = () => { resolve() }; recorder.stop() })
      const audio = new Uint8Array(await new Blob(chunks, { type: 'audio/webm' }).arrayBuffer())
      if (isVoicePreviewAudio(audio)) await api.previewSave?.(voice, audio).catch(() => false)
    }
    return 'called'
  } finally {
    off()
    close()
  }
}
