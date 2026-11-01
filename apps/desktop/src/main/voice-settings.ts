import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { isVoiceMode, type VoiceSettings, type VoiceSettingsResult } from '../shared/voice.js'
import type { SecretBox } from './own-models.js'
import { replaceFile } from './replace-file.js'

/** Separate from public workspace settings: an OS-account-encrypted key never leaves main. */
export function createVoiceSettingsStore(directory: string, secrets: SecretBox, platform: string = process.platform) {
  const path = join(directory, 'voice-settings.json')
  /*
   * WHERE LOCAL VOICE TYPING RUNS (0.850): Fast and Accurate are whisper.cpp's Windows build. Elsewhere the default, and
   * a stored local choice, read as your OpenAI account: a Mac opened on Fast, which Settings now draws unavailable.
   */
  const usable = (mode: VoiceSettings['mode']): VoiceSettings['mode'] => (platform === 'win32' || mode === 'openai' ? mode : 'openai')
  let queue: Promise<unknown> = Promise.resolve()
  type Stored = { mode: VoiceSettings['mode']; encryptedKey?: string; openaiConsent: boolean }
  const read = async (): Promise<Stored> => {
    try {
      const value: unknown = JSON.parse(await readFile(path, 'utf8'))
      if (typeof value === 'object' && value !== null) {
        const row = value as Record<string, unknown>
        return { mode: usable(isVoiceMode(row.mode) ? row.mode : 'fast'), openaiConsent: row.openaiConsent === true,
          ...(typeof row.encryptedKey === 'string' ? { encryptedKey: row.encryptedKey } : {}) }
      }
    } catch { /* A new or unreadable profile stays local by default. */ }
    return { mode: usable('fast'), openaiConsent: false }
  }
  const publicOf = (row: Stored): VoiceSettings => ({ mode: row.mode, hasKey: Boolean(row.encryptedKey), openaiConsent: row.openaiConsent })
  const mutate = (change: unknown, consent = false): Promise<VoiceSettingsResult> => {
    const task = queue.then(async (): Promise<VoiceSettingsResult> => {
      try {
        const row = await read()
        if (consent) {
          if (row.mode !== 'openai' || !row.encryptedKey) return { ok: false, message: 'Save your OpenAI API key in Settings > General first. Your message is unchanged.' }
          row.openaiConsent = true
        } else {
          if (typeof change !== 'object' || change === null) throw new Error('invalid')
          const input = change as Record<string, unknown>
          if (input.mode !== undefined) {
            if (!isVoiceMode(input.mode)) throw new Error('invalid')
            row.mode = input.mode
          }
          if (input.key !== undefined) {
            if (typeof input.key !== 'string' || input.key.length > 4096 || /[\r\n\x00]/.test(input.key)) throw new Error('invalid')
            const key = input.key.trim()
            if (!key) { delete row.encryptedKey; row.openaiConsent = false }
            else {
              if (!secrets.available()) return { ok: false, message: 'Windows could not lock your API key to this account. The key was not saved.' }
              row.encryptedKey = secrets.encrypt(key).toString('base64')
            }
          }
        }
        await mkdir(directory, { recursive: true })
        const temp = path + '.tmp'
        await writeFile(temp, JSON.stringify(row), 'utf8')
        await replaceFile(temp, path)
        return { ok: true, settings: publicOf(row) }
      } catch { return { ok: false, message: 'Voice typing settings could not be saved. Your message is unchanged.' } }
    })
    queue = task
    return task
  }
  return {
    settings: async (): Promise<VoiceSettings> => { await queue; return publicOf(await read()) },
    save: (change: unknown): Promise<VoiceSettingsResult> => mutate(change),
    consent: (): Promise<VoiceSettingsResult> => mutate({}, true),
    key: async (): Promise<string | undefined> => {
      await queue
      const row = await read()
      if (!row.encryptedKey || !secrets.available()) return undefined
      try { return secrets.decrypt(Buffer.from(row.encryptedKey, 'base64')) } catch { return undefined }
    }
  }
}
