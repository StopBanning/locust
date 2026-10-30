import { useEffect, useState } from 'react'
import { isVoiceMode, type VoiceApi, type VoiceSettings as Settings, type VoiceSettingsChange } from '../../../shared/voice.js'
import { Select } from './Select.js'

export function VoiceSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.voice, platform = typeof window === 'undefined' ? undefined : window.desktop?.platform }: { readonly api?: VoiceApi; readonly platform?: string }) {
  // Fast and Accurate run whisper.cpp's Windows build; elsewhere they say so (0.843, the first Mac run).
  const windowsOnly = platform !== undefined && platform !== 'win32' ? ' (Windows only for now)' : ''
  // Where the saved key is locked: Windows' account, a Mac's keychain (the first Mac run, 2026-10-30).
  const lockedTo = platform === 'darwin' ? "this Mac's keychain" : 'your Windows account'
  const [settings, setSettings] = useState<Settings>()
  const [key, setKey] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => {
    let alive = true
    void api?.settings?.().then((value) => { if (alive) setSettings(value) }).catch(() => { if (alive) setMessage('Voice typing settings could not be read. Voice typing keeps its last choice.') })
    return () => { alive = false }
  }, [api])
  const save = async (change: VoiceSettingsChange): Promise<void> => {
    if (!api?.saveSettings || busy) return
    setBusy(true); setMessage('')
    try {
      const result = await api.saveSettings(change)
      if (result.ok) { setSettings(result.settings); setMessage(change.key === undefined ? '' : change.key ? 'API key saved on this computer.' : 'API key removed.') }
      else setMessage(result.message)
    } catch { setMessage('Voice typing settings could not be saved. The previous choice is still in use.') }
    finally { setKey(''); setBusy(false) }
  }
  return <div className="lc-voice-settings">
    <Select className="lc-control" aria-label="Voice typing model" disabled={!settings || busy} value={settings?.mode ?? 'fast'} onChange={(next) => { if (isVoiceMode(next)) void save({ mode: next }) }} options={[
      { value: 'fast', label: `Fast — 41 MB${windowsOnly}` },
      { value: 'accurate', label: `Accurate — 69 MB, slower${windowsOnly}` },
      { value: 'openai', label: 'Your OpenAI account' }
    ]} />
    {settings?.mode === 'openai' && <div>
      <p className="lc-settings__note">{settings.hasKey ? `API key saved. It is locked to ${lockedTo}.` : 'Add your OpenAI API key. A ChatGPT subscription does not include API usage.'}</p>
      <input className="lc-input" type="password" aria-label="OpenAI voice API key" autoComplete="off" spellCheck={false} value={key} disabled={busy} onChange={(event) => { setKey(event.target.value) }} placeholder={settings.hasKey ? 'Replace saved key' : 'API key'} />
      <button type="button" className="lc-control lc-control--boxed" disabled={busy || !key.trim()} onClick={() => { void save({ key }) }}>Save key</button>
      {settings.hasKey && <button type="button" className="lc-control" disabled={busy} onClick={() => { void save({ key: '' }) }}>Remove key</button>}
    </div>}
    {message && <p className="lc-settings__note" role="status">{message}</p>}
  </div>
}
