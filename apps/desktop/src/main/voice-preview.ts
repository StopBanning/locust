import { mkdir, readFile, stat, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { VOICE_PREVIEW_MAX_BYTES, isChatGptVoice, isVoicePreviewAudio } from '../shared/voice-call.js'
import { replaceFile } from './replace-file.js'

/*
 * WHAT A VOICE SOUNDED LIKE, KEPT (0.751). The first time a voice is played it makes a short call and greets the
 * person (shared/voice-call.ts, voicePreviewRules); the window records the greeting and it is kept here, one file a
 * voice, so the next play is that recording, at once and on nobody's plan. A voice that is not one of ChatGPT's nine,
 * or audio that is not a short WebM recording, is refused. The spoken "Hello?" the voice answers is made once by
 * the computer's own speech -- Windows', or a Mac's `say` -- and kept beside them.
 */
export interface VoicePreviews {
  saved(voice: unknown): Promise<Uint8Array | undefined>
  save(voice: unknown, audio: unknown): Promise<boolean>
  trigger(): Promise<Uint8Array | undefined>
}

/** A WAV is "RIFF....WAVE": the speech made for the trigger, read back before it is handed on. */
const isWave = (bytes: Uint8Array): boolean =>
  bytes.byteLength > 44 && bytes.byteLength <= VOICE_PREVIEW_MAX_BYTES
  && String.fromCharCode(...bytes.subarray(0, 4)) === 'RIFF' && String.fromCharCode(...bytes.subarray(8, 12)) === 'WAVE'

export function createVoicePreviews(deps: {
  readonly root: string
  /** Writes a short spoken "Hello?" as a WAV at this path (the computer's own speech, in the app). */
  readonly speak: (wavPath: string) => Promise<void>
}): VoicePreviews {
  const fileOf = (voice: string): string => join(deps.root, `${voice}.webm`)
  const triggerFile = join(deps.root, 'hello.wav')
  let making: Promise<Uint8Array | undefined> | undefined

  const read = async (path: string): Promise<Uint8Array | undefined> => {
    const size = await stat(path).then((entry) => entry.size).catch(() => undefined)
    if (size === undefined || size > VOICE_PREVIEW_MAX_BYTES) return undefined
    return new Uint8Array(await readFile(path).catch(() => Buffer.alloc(0)))
  }

  return {
    async saved(voice) {
      if (!isChatGptVoice(voice)) return undefined
      const audio = await read(fileOf(voice))
      return audio !== undefined && isVoicePreviewAudio(audio) ? audio : undefined
    },

    async save(voice, audio) {
      if (!isChatGptVoice(voice) || !isVoicePreviewAudio(audio)) return false
      try {
        await mkdir(deps.root, { recursive: true })
        // Whole or not at all: a play never meets half a recording.
        const temporary = `${fileOf(voice)}.${String(process.pid)}.tmp`
        await writeFile(temporary, audio)
        await replaceFile(temporary, fileOf(voice))
        return true
      } catch {
        return false
      }
    },

    trigger() {
      // Made once, by one caller at a time; a failed making is tried again on the next play.
      making ??= (async () => {
        const kept = await read(triggerFile)
        if (kept !== undefined && isWave(kept)) return kept
        await mkdir(deps.root, { recursive: true })
        await deps.speak(triggerFile)
        const made = await read(triggerFile)
        return made !== undefined && isWave(made) ? made : undefined
      })().catch(() => undefined).then((made) => {
        if (made === undefined) making = undefined
        return made
      })
      return making
    }
  }
}

/**
 * The PowerShell that speaks "Hello?" into a WAV, passed as -EncodedCommand so no path or quote in it is ever
 * parsed twice. 24 kHz mono 16-bit, as the calls' own drives record their questions.
 */
export function speakHelloCommand(wavPath: string): readonly string[] {
  const quoted = `'${wavPath.replace(/'/g, "''")}'`
  const script = [
    'Add-Type -AssemblyName System.Speech',
    '$s = New-Object System.Speech.Synthesis.SpeechSynthesizer',
    '$f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(24000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)',
    `$s.SetOutputToWaveFile(${quoted}, $f)`,
    "$s.Speak('Hello?')",
    '$s.Dispose()'
  ].join('; ')
  return ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')]
}

/**
 * The program that speaks "Hello?" into a WAV here: PowerShell on Windows, and a Mac's own `say` (the first Mac
 * run, 2026-10-30), in the same 24 kHz mono 16-bit -- measured there: "1 ch, 24000 Hz, Int16". The path is one
 * argument of its own, never parsed by a shell.
 */
export function speakHello(platform: string, wavPath: string): { readonly command: string; readonly args: readonly string[] } {
  return platform === 'darwin'
    ? { command: '/usr/bin/say', args: ['-o', wavPath, '--file-format=WAVE', '--data-format=LEI16@24000', 'Hello?'] }
    : { command: 'powershell.exe', args: speakHelloCommand(wavPath) }
}
