import { createHash } from 'node:crypto'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, open, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { VOICE_MAX_BYTES, VOICE_RATE, isVoiceMode, voiceLocalOnly, type VoiceMode, type VoiceResult } from '../shared/voice.js'
import { VOICE_LICENSE } from '../shared/voice-license.js'
import type { createVoiceSettingsStore } from './voice-settings.js'
import { transcribeOpenAI } from './voice-openai.js'

export const VOICE_ARCHIVE = {
  url: 'https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip',
  sha256: '6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb',
  bytes: 8_928_640
} as const
export const VOICE_MODEL = {
  url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.en-q5_1.bin',
  sha256: 'c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b',
  bytes: 32_166_155
} as const
export const VOICE_ACCURATE_MODEL = {
  url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin',
  sha256: '4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f',
  bytes: 59_721_011
} as const
export const VOICE_NATIVE = {
  'whisper-cli.exe': '331dba46d6427105d2b802cdbc7eae916ea1c5abf9b0d3b5cfe460d8db8e4366',
  'whisper.dll': '034396d75bb5720c1851674ff64a96192b58f6927b68d334ebd254ce6cad9674',
  'ggml.dll': '31fe18616d872af9b6134b47a6dff8072ae4ecbe5df0273f4f8dc36766c9db8e',
  'ggml-base.dll': 'bede0d6b2d387236ecd2b5dd676d51c8984b5b6e0ae461a12174e9c01345c5f0',
  'ggml-cpu-x64.dll': '4b4b05caba40d4309b5d710ef74c59ba190516b8489f27bdb1ba11ae33c6e9d3',
  'ggml-cpu-haswell.dll': '1306b1014c8118d27234d8ddc0e66ced1f8c8476bf85071595fd182ea9b7263b'
} as const
const MODEL_NAME = 'ggml-tiny.en-q5_1.bin'
const modelFor = (mode: VoiceMode) => mode === 'accurate' ? VOICE_ACCURATE_MODEL : VOICE_MODEL
const modelNameFor = (mode: VoiceMode): string => mode === 'accurate' ? 'ggml-base.en-q5_1.bin' : MODEL_NAME
const digest = (bytes: Uint8Array): string => createHash('sha256').update(bytes).digest('hex')

async function verified(path: string, hash: string): Promise<boolean> {
  try {
    if (digest(await readFile(path)) === hash) return true
    await rm(path, { force: true })
  } catch { /* absent, unreadable, or a failed deletion: never execute it */ }
  return false
}

/** Interrupted files are restarted; only a checked, complete file is promoted. */
export async function downloadVoiceFile(
  asset: { readonly url: string; readonly sha256: string; readonly bytes: number },
  path: string, signal: AbortSignal, progress: (bytes: number) => void,
  fetcher: typeof fetch = fetch
): Promise<void> {
  if (await verified(path, asset.sha256)) { progress(asset.bytes); return }
  const partial = path + '.part'
  await rm(partial, { force: true })
  let file: Awaited<ReturnType<typeof open>> | undefined
  try {
    signal.throwIfAborted()
    const response = await fetcher(asset.url, { signal, credentials: 'omit' })
    if (!response.ok || response.body === null) throw new Error('Voice typing could not download its files. Nothing was recorded; try Download again.')
    file = await open(partial, 'w')
    const hash = createHash('sha256')
    let size = 0
    for await (const chunk of response.body) {
      signal.throwIfAborted()
      size += chunk.byteLength
      if (size > asset.bytes) throw new Error('The voice typing download was larger than expected. The file was deleted; try Download again.')
      hash.update(chunk)
      await file.writeFile(chunk)
      progress(size)
    }
    await file.close()
    file = undefined
    signal.throwIfAborted()
    if (size !== asset.bytes || hash.digest('hex') !== asset.sha256) {
      throw new Error('The voice typing file did not pass its SHA-256 check. The file was deleted; try Download again.')
    }
    await rename(partial, path)
  } catch (error) {
    await file?.close()
    await rm(partial, { force: true })
    throw error
  }
}

/** Await execFile's completion, including timeout/abort: there is no warm sidecar. */
export function runVoiceCli(executable: string, wav: string, model: string, options: {
  readonly signal: AbortSignal; readonly timeoutMs?: number; readonly prefix?: readonly string[]
}): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(executable, [...(options.prefix ?? []), '-m', model, '-f', wav, '-t', '2', '-l', 'en', '-ng', '-nt', '-np'], {
      windowsHide: true, timeout: options.timeoutMs ?? 120_000, signal: options.signal,
      killSignal: 'SIGKILL', maxBuffer: 256_000, encoding: 'utf8'
    }, (error, stdout) => {
      if (error !== null) {
        reject(new Error(options.signal.aborted
          ? 'Voice typing was cancelled. Nothing was inserted.'
          : error.killed ? 'Voice typing took too long and was stopped. Your message is unchanged; try a shorter recording.'
            : 'Voice typing could not read the recording. Your message is unchanged; try again.'))
      } else resolve(stdout.trim())
    })
  })
}

export function validVoiceWav(input: unknown): input is Uint8Array {
  if (!(input instanceof Uint8Array) || input.byteLength < 46 || input.byteLength > VOICE_MAX_BYTES) return false
  const bytes = Buffer.from(input.buffer, input.byteOffset, input.byteLength)
  return bytes.toString('ascii', 0, 4) === 'RIFF' && bytes.readUInt32LE(4) === bytes.length - 8
    && bytes.toString('ascii', 8, 16) === 'WAVEfmt ' && bytes.readUInt32LE(16) === 16
    && bytes.readUInt16LE(20) === 1 && bytes.readUInt16LE(22) === 1
    && bytes.readUInt32LE(24) === VOICE_RATE && bytes.readUInt32LE(28) === VOICE_RATE * 2
    && bytes.readUInt16LE(32) === 2 && bytes.readUInt16LE(34) === 16
    && bytes.toString('ascii', 36, 40) === 'data' && bytes.readUInt32LE(40) === bytes.length - 44
    && (bytes.length - 44) % 2 === 0
}

export function createVoiceHost(directory: string, options: { readonly store?: ReturnType<typeof createVoiceSettingsStore>; readonly openaiFetch?: typeof fetch; readonly platform?: string; readonly arch?: string } = {}) {
  let active: { controller: AbortController; done: Promise<VoiceResult> } | undefined
  let disposed = false
  // whisper.cpp's Windows x64 build runs Fast and Accurate; the OpenAI route runs anywhere (0.843, the first Mac run).
  const platform = options.platform ?? process.platform
  const local = platform === 'win32' && (options.arch ?? process.arch) === 'x64'
  const selected = async (requested: unknown): Promise<VoiceMode> => {
    const mode = (await options.store?.settings())?.mode ?? 'fast'
    if (requested !== undefined && (!isVoiceMode(requested) || requested !== mode)) throw new Error('Voice typing changed in Settings. Your message is unchanged; press the microphone again.')
    return mode
  }
  const ready = async (requested?: unknown): Promise<boolean> => {
    if (disposed) return false
    let mode: VoiceMode
    try { mode = await selected(requested) } catch { return false }
    if (mode === 'openai') {
      const settings = await options.store?.settings()
      return settings?.openaiConsent === true && Boolean(await options.store?.key())
    }
    if (!local) return false
    for (const [name, hash] of Object.entries(VOICE_NATIVE)) if (!(await verified(join(directory, name), hash))) return false
    return verified(join(directory, modelNameFor(mode)), modelFor(mode).sha256)
  }
  const begin = (work: (signal: AbortSignal) => Promise<VoiceResult>): Promise<VoiceResult> => {
    if (disposed) return Promise.resolve({ ok: false, message: 'Voice typing has closed. Your message is unchanged.' })
    if (active !== undefined) return Promise.resolve({ ok: false, message: 'Voice typing is already working. Your message is unchanged.' })
    const controller = new AbortController()
    const done = work(controller.signal).catch((error: unknown): VoiceResult => ({ ok: false, message: controller.signal.aborted ? 'Voice typing was cancelled. Nothing was inserted.' : error instanceof Error ? error.message : 'Voice typing could not finish. Your message is unchanged.' })).finally(() => { active = undefined })
    active = { controller, done }
    return done
  }
  const cancel = async (): Promise<void> => {
    const task = active
    task?.controller.abort()
    await task?.done
  }
  return {
    ready,
    download: (progress: (percent: number) => void, requested?: unknown): Promise<VoiceResult> => begin(async (signal) => {
      const mode = await selected(requested)
      if (mode === 'openai') return { ok: false, message: 'OpenAI voice typing does not need a download. Your message is unchanged.' }
      if (!local) return { ok: false, message: voiceLocalOnly(platform) }
      const model = modelFor(mode)
      await mkdir(directory, { recursive: true })
      const timed = AbortSignal.any([signal, AbortSignal.timeout(300_000)])
      const archive = join(directory, 'whisper.zip')
      const extracted = join(directory, 'unpack')
      try {
        const total = VOICE_ARCHIVE.bytes + model.bytes
        await downloadVoiceFile(VOICE_ARCHIVE, archive, timed, (bytes) => progress(Math.floor(bytes / total * 99)))
        await downloadVoiceFile(model, join(directory, modelNameFor(mode)), timed, (bytes) => progress(Math.floor((VOICE_ARCHIVE.bytes + bytes) / total * 99)))
        await rm(extracted, { recursive: true, force: true })
        const quote = (value: string): string => "'" + value.replaceAll("'", "''") + "'"
        const command = `Expand-Archive -LiteralPath ${quote(archive)} -DestinationPath ${quote(extracted)} -Force -ErrorAction Stop`
        await new Promise<void>((resolve, reject) => execFile(join(process.env.SystemRoot ?? 'C:\\Windows', 'System32/WindowsPowerShell/v1.0/powershell.exe'), ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(command, 'utf16le').toString('base64')], { windowsHide: true, timeout: 60_000, signal: timed }, (error) => error === null ? resolve() : reject(new Error('Voice typing could not unpack its files. Nothing was recorded; try Download again.'))))
        for (const [name, hash] of Object.entries(VOICE_NATIVE)) {
          timed.throwIfAborted()
          const source = join(extracted, 'Release', name)
          if (!(await verified(source, hash))) throw new Error('A voice typing program did not pass its SHA-256 check. The file was deleted; try Download again.')
          await rm(join(directory, name), { force: true })
          await rename(source, join(directory, name))
        }
        timed.throwIfAborted()
        await writeFile(join(directory, 'LICENSE.txt'), VOICE_LICENSE, 'utf8')
        progress(100)
        return { ok: true }
      } finally {
        await rm(extracted, { recursive: true, force: true })
        await rm(archive, { force: true })
      }
    }),
    transcribe: (input: unknown, requested?: unknown): Promise<VoiceResult> => begin(async (signal) => {
      if (!validVoiceWav(input)) return { ok: false, message: 'Voice typing needs a recording of at most one minute. Your message is unchanged.' }
      const mode = await selected(requested)
      if (mode === 'openai') {
        if ((await options.store?.settings())?.openaiConsent !== true) return { ok: false, message: 'Allow sending audio to OpenAI at the microphone first. Your message is unchanged.' }
        signal.throwIfAborted()
        return transcribeOpenAI(input, await options.store?.key(), signal, options.openaiFetch)
      }
      if (!local) return { ok: false, message: voiceLocalOnly(platform) }
      if (!(await ready(mode))) return { ok: false, message: 'Voice typing needs its checked files. Your message is unchanged; press the microphone to download them again.' }
      signal.throwIfAborted()
      const temp = await mkdtemp(join(directory, 'recording-'))
      try {
        const wav = join(temp, 'speech.wav')
        const file = await open(wav, 'w')
        try { await file.writeFile(input) } finally { await file.close() }
        signal.throwIfAborted()
        const text = await runVoiceCli(join(directory, 'whisper-cli.exe'), wav, join(directory, modelNameFor(mode)), { signal })
        signal.throwIfAborted()
        return { ok: true, text }
      } finally { await rm(temp, { recursive: true, force: true }) }
    }),
    cancel,
    dispose: async (): Promise<void> => { disposed = true; await cancel() }
  }
}
