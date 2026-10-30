import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createVoiceSettingsStore } from './voice-settings.js'
import { createVoiceHost } from './voice-host.js'
import { voiceWav } from '../shared/voice-pcm.js'

/**
 * VOICE TYPING ON A MAC GOES THROUGH OPENAI (0.843). The first Mac run (2026-10-30) found every voice route refused
 * off Windows x64, the OpenAI one too, though it is only a web request. Fast and Accurate run whisper.cpp's Windows
 * build, so on a Mac they say so and name the Mac's own dictation; Your OpenAI account works as it does on Windows.
 */
const folders: string[] = []
async function folder() { const path = await mkdtemp(join(tmpdir(), 'voice-mac-unit-')); folders.push(path); return path }
afterEach(async () => { for (const path of folders.splice(0)) await rm(path, { recursive: true, force: true }) })
const key = 'fake-unit-key-not-a-credential'
const box = { available: () => true, encrypt: (text: string) => Buffer.from(text.split('').reverse().join('')), decrypt: (bytes: Buffer) => bytes.toString().split('').reverse().join('') }
const wav = voiceWav(new Float32Array(160))

describe('voice typing on a Mac', () => {
  it('types through the OpenAI route once allowed, as on Windows', async () => {
    const root = await folder(); const store = createVoiceSettingsStore(root, box)
    await store.save({ mode: 'openai', key })
    await store.consent()
    const fetcher = vi.fn(async () => Response.json({ text: 'Typed on a Mac.' }))
    const host = createVoiceHost(join(root, 'voice'), { store, openaiFetch: fetcher, platform: 'darwin', arch: 'arm64' })
    expect(await host.ready('openai')).toBe(true)
    expect(await host.transcribe(wav, 'openai')).toEqual({ ok: true, text: 'Typed on a Mac.' })
    await host.dispose()
  })

  it('says Fast and Accurate run on Windows, and names the Mac’s own dictation, downloading nothing', async () => {
    const root = await folder(); const store = createVoiceSettingsStore(root, box)
    const host = createVoiceHost(join(root, 'voice'), { store, platform: 'darwin', arch: 'arm64' })
    expect(await host.ready('fast')).toBe(false)
    for (const result of [await host.download(() => undefined, 'fast'), await host.transcribe(wav, 'fast')]) {
      expect(result.ok).toBe(false)
      expect(result.ok ? '' : result.message).toMatch(/^Fast and Accurate voice typing run on Windows for now\. On a Mac, use its own dictation here/)
    }
    await host.dispose()
  })
})
