import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createVoiceSettingsStore } from './voice-settings.js'
import { createVoiceHost, downloadVoiceFile, VOICE_ACCURATE_MODEL } from './voice-host.js'
import { transcribeOpenAI, VOICE_OPENAI_MODEL, VOICE_OPENAI_URL } from './voice-openai.js'
import { voiceWav } from '../shared/voice-pcm.js'
import { LEFT_OUT } from './profile-backup.js'

const folders: string[] = []
async function folder() { const path = await mkdtemp(join(tmpdir(), 'voice-model-unit-')); folders.push(path); return path }
afterEach(async () => { for (const path of folders.splice(0)) await rm(path, { recursive: true, force: true }) })
const key = 'fake-unit-key-not-a-credential'
const box = { available: () => true, encrypt: (text: string) => Buffer.from(text.split('').reverse().join('')), decrypt: (bytes: Buffer) => bytes.toString().split('').reverse().join('') }
const wav = voiceWav(new Float32Array(160))

describe('voice model settings and key boundary', () => {
  it('defaults existing profiles to Fast, without creating anything', async () => {
    const root = await folder()
    const store = createVoiceSettingsStore(root, box, 'win32')
    expect(await store.settings()).toEqual({ mode: 'fast', hasKey: false, openaiConsent: false })
    await expect(readFile(join(root, 'voice-settings.json'))).rejects.toMatchObject({ code: 'ENOENT' })
  })
  it('opens a Mac on your OpenAI account, where Fast and Accurate cannot run, and reads a stored local choice so', async () => {
    const root = await folder()
    expect((await createVoiceSettingsStore(root, box, 'darwin').settings()).mode).toBe('openai')
    expect((await createVoiceSettingsStore(root, box, 'win32').save({ mode: 'accurate' })).ok).toBe(true)
    expect((await createVoiceSettingsStore(root, box, 'darwin').settings()).mode).toBe('openai')
    expect((await createVoiceSettingsStore(root, box, 'win32').settings()).mode).toBe('accurate')
  })
  it('round trips all three choices through a reopened store', async () => {
    const root = await folder()
    for (const mode of ['accurate', 'openai', 'fast'] as const) {
      expect((await createVoiceSettingsStore(root, box, 'win32').save({ mode })).ok).toBe(true)
      expect((await createVoiceSettingsStore(root, box, 'win32').settings()).mode).toBe(mode)
    }
  })
  it('returns only saved status, never the key or ciphertext, even on consent/save', async () => {
    const root = await folder(); const store = createVoiceSettingsStore(root, box, 'win32')
    const results = [await store.save({ mode: 'openai', key }), await store.consent(), await store.settings()]
    const publicText = JSON.stringify(results)
    expect(publicText).not.toContain(key)
    expect(publicText).not.toContain('encryptedKey')
    expect(Object.keys(await store.settings()).sort()).toEqual(['hasKey', 'mode', 'openaiConsent'])
    expect(await readFile(join(root, 'voice-settings.json'), 'utf8')).not.toContain(key)
    expect(await store.key()).toBe(key)
    expect((await createVoiceSettingsStore(root, box, 'win32').settings()).openaiConsent).toBe(true)
    expect(LEFT_OUT).toHaveProperty('voice-settings.json')
  })
  it('refuses plaintext storage when OS encryption is unavailable', async () => {
    const store = createVoiceSettingsStore(await folder(), { ...box, available: () => false })
    expect(await store.save({ key })).toMatchObject({ ok: false, message: expect.stringContaining('not saved') })
    expect((await store.settings()).hasKey).toBe(false)
  })
  it('key removal clears consent and never echoes an invalid input', async () => {
    const store = createVoiceSettingsStore(await folder(), box, 'win32')
    await store.save({ mode: 'openai', key }); await store.consent()
    await store.save({ key: '' })
    expect(await store.settings()).toEqual({ mode: 'openai', hasKey: false, openaiConsent: false })
    expect(JSON.stringify(await store.save({ key: key + '\n' }))).not.toContain(key)
    expect((await store.save({ mode: 'https://evil.test' })).ok).toBe(false)
  })
  it('cannot consent without the selected OpenAI mode and saved key', async () => {
    const store = createVoiceSettingsStore(await folder(), box, 'win32')
    expect((await store.consent()).ok).toBe(false)
    await store.save({ key })
    expect((await store.consent()).ok).toBe(false)
  })
  it('Accurate uses the pinned base model and refuses a wrong cached/downloaded file', async () => {
    expect(VOICE_ACCURATE_MODEL.bytes).toBe(59_721_011)
    expect(VOICE_ACCURATE_MODEL.url).toContain('/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin')
    const path = join(await folder(), 'ggml-base.en-q5_1.bin')
    await writeFile(path, 'wrong cached model')
    const fetcher = vi.fn(async () => new Response(new Uint8Array(VOICE_ACCURATE_MODEL.bytes))) as unknown as typeof fetch
    await expect(downloadVoiceFile(VOICE_ACCURATE_MODEL, path, new AbortController().signal, () => undefined, fetcher)).rejects.toThrow('SHA-256')
    await expect(readFile(path)).rejects.toMatchObject({ code: 'ENOENT' })
    await expect(readFile(path + '.part')).rejects.toMatchObject({ code: 'ENOENT' })
  })
  it('main refuses upload before consent, and stale local capture cannot switch to cloud', async () => {
    const root = await folder(); const store = createVoiceSettingsStore(root, box, 'win32')
    await store.save({ mode: 'openai', key })
    const fetcher = vi.fn(async () => Response.json({ text: 'Allowed words.' }))
    const host = createVoiceHost(join(root, 'voice'), { store, openaiFetch: fetcher })
    expect((await host.transcribe(wav, 'openai')).ok).toBe(false)
    await store.consent()
    expect((await host.transcribe(wav, 'fast')).ok).toBe(false)
    expect(fetcher).not.toHaveBeenCalled()
    expect(await host.transcribe(wav, 'openai')).toEqual({ ok: true, text: 'Allowed words.' })
    expect(fetcher).toHaveBeenCalledOnce()
    await host.dispose()
  })
})

describe('OpenAI transcription uses only the fixed destination', () => {
  it('sends a multipart WAV and model, with the key only in Authorization', async () => {
    const fetcher = vi.fn(async (url: string | URL | Request, options?: RequestInit) => {
      expect(url).toBe(VOICE_OPENAI_URL)
      expect(new URL(String(url)).origin).toBe('https://api.openai.com')
      expect(options?.redirect).toBe('error')
      expect(options?.credentials).toBe('omit')
      expect(options?.headers).toEqual({ Authorization: `Bearer ${key}` })
      const body = options?.body as FormData
      const audio = body.get('file') as File
      expect(audio.name).toBe('speech.wav'); expect(audio.type).toBe('audio/wav')
      expect(new Uint8Array(await audio.arrayBuffer())).toEqual(wav)
      expect(body.get('model')).toBe(VOICE_OPENAI_MODEL)
      expect(body.get('response_format')).toBe('json')
      expect([...body.keys()]).toEqual(['file', 'model', 'response_format', 'language'])
      expect([...body.values()]).not.toContain(key)
      return Response.json({ text: 'Editable words.', extra: key })
    })
    expect(await transcribeOpenAI(wav, key, new AbortController().signal, fetcher)).toEqual({ ok: true, text: 'Editable words.' })
    expect(fetcher).toHaveBeenCalledOnce()
  })
  it('missing key makes no request', async () => {
    const fetcher = vi.fn()
    expect(await transcribeOpenAI(wav, undefined, new AbortController().signal, fetcher)).toMatchObject({ ok: false, message: expect.stringContaining('Add your OpenAI API key') })
    expect(fetcher).not.toHaveBeenCalled()
  })
  it.each([401, 403, 429, 500])('status %s returns no text and never leaks the raw body', async (status) => {
    const result = await transcribeOpenAI(wav, key, new AbortController().signal, vi.fn(async () => new Response(key, { status })))
    expect(result).toMatchObject({ ok: false, message: expect.stringContaining('Your message is unchanged') })
    expect(result).not.toHaveProperty('text'); expect(JSON.stringify(result)).not.toContain(key)
    if (status === 401 || status === 403) expect(result).toMatchObject({ message: expect.stringContaining('refused your API key') })
  })
  it('does not echo fetch exceptions or a key reflected as transcript', async () => {
    const failure = await transcribeOpenAI(wav, key, new AbortController().signal, vi.fn(async () => { throw new Error(key) }))
    const reflected = await transcribeOpenAI(wav, key, new AbortController().signal, vi.fn(async () => Response.json({ text: key })))
    for (const result of [failure, reflected]) { expect(result.ok).toBe(false); expect(JSON.stringify(result)).not.toContain(key) }
  })
  it('cancellation discards a late fake response', async () => {
    const controller = new AbortController()
    const result = await transcribeOpenAI(wav, key, controller.signal, vi.fn(async () => { controller.abort(); return Response.json({ text: 'Too late.' }) }))
    expect(result.ok).toBe(false); expect(result).not.toHaveProperty('text')
  })
})
