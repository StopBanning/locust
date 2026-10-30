/** PCM and settings cross this bridge; no URL, command, or path comes from the window. */
export const VOICE_SETTINGS_READ = 'voice:settings-read'
export const VOICE_SETTINGS_SAVE = 'voice:settings-save'
export const VOICE_OPENAI_CONSENT = 'voice:openai-consent'
export type VoiceMode = 'fast' | 'accurate' | 'openai'
export const isVoiceMode = (value: unknown): value is VoiceMode => value === 'fast' || value === 'accurate' || value === 'openai'
export interface VoiceSettings { readonly mode: VoiceMode; readonly hasKey: boolean; readonly openaiConsent: boolean }
export interface VoiceSettingsChange { readonly mode?: VoiceMode; readonly key?: string }
export type VoiceSettingsResult = { readonly ok: true; readonly settings: VoiceSettings } | { readonly ok: false; readonly message: string }
export const VOICE_READY = 'voice:ready'
export const VOICE_DOWNLOAD = 'voice:download'
export const VOICE_TRANSCRIBE = 'voice:transcribe'
export const VOICE_CANCEL = 'voice:cancel'
export const VOICE_PROGRESS = 'voice:progress'
export const VOICE_RATE = 16_000
export const VOICE_MAX_SECONDS = 60
export const VOICE_MAX_BYTES = 44 + VOICE_RATE * 2 * VOICE_MAX_SECONDS
export type VoiceResult = { readonly ok: true; readonly text?: string } | { readonly ok: false; readonly message: string }
/**
 * FAST AND ACCURATE ARE WINDOWS' (the first Mac run, 2026-10-30). They run whisper.cpp's Windows x64 build; the
 * OpenAI route is a web request and works anywhere. Elsewhere, a local mode says so, and a Mac names its own
 * dictation, which is free and stays on the Mac.
 */
export const voiceLocalOnly = (platform: string | undefined): string =>
  platform === 'darwin'
    ? 'Fast and Accurate voice typing run on Windows for now. On a Mac, use its own dictation here (the microphone key, or Fn twice), or choose Your OpenAI account under Voice typing in Settings. Your message is unchanged.'
    : 'Fast and Accurate voice typing run on Windows for now. Choose Your OpenAI account under Voice typing in Settings. Your message is unchanged.'

export interface VoiceApi {
  settings?(): Promise<VoiceSettings>
  saveSettings?(change: VoiceSettingsChange): Promise<VoiceSettingsResult>
  allowOpenAI?(): Promise<VoiceSettingsResult>
  ready(mode?: VoiceMode): Promise<boolean>
  download(mode?: VoiceMode): Promise<VoiceResult>
  transcribe(wav: Uint8Array, mode?: VoiceMode): Promise<VoiceResult>
  cancel(): Promise<void>
  onProgress(listener: (percent: number) => void): () => void
}
