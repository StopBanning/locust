import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ReactElement } from 'react'

import { WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'
import type { VoiceApi } from '../../shared/voice.js'
import { voiceWav } from '../../shared/voice-pcm.js'
import { VOICE_ALLOW_THERE, VOICE_DOWNLOAD_THERE, VOICE_KEY_THERE, VoiceButton } from './components/VoiceButton.js'

/**
 * VOICE TYPING IN A BROWSER (0.850). It records in the browser and the computer types the words: transcribing is a
 * call a browser makes. Allowing OpenAI and the one-time download are the computer's, so a browser says where they
 * are done, in place of an ask whose button would fail -- and a refusal says so, not "check microphone access".
 */
const mocks = vi.hoisted(() => ({ cleanups: [] as (() => void)[], phases: [] as unknown[], said: [] as unknown[], start: vi.fn(), stop: vi.fn(), cancel: vi.fn() }))
// The same harness as voice-typing-is-editable-and-cancellable: the real handlers, no DOM. The setters are told
// apart by what they start as: the phase starts idle, the words empty.
vi.mock('react', async (original) => ({ ...await original<typeof import('react')>(),
  useRef: (value: unknown) => ({ current: value }),
  useState: (value: unknown) => [value, (next: unknown) => { if (value === 'idle') mocks.phases.push(next); else if (value === '') mocks.said.push(next) }],
  useEffect: (effect: () => undefined | (() => void)) => { const cleanup = effect(); if (cleanup) mocks.cleanups.push(cleanup) }
}))
vi.mock('./voiceCapture.js', () => ({ startVoiceCapture: mocks.start }))

const place = (where: 'browser' | 'computer'): void => {
  vi.stubGlobal('window', Object.assign(new EventTarget(), where === 'browser' ? { desktop: { remote: { computer: 'your computer' } } } : { desktop: {} }))
}
beforeEach(() => {
  mocks.start.mockResolvedValue({ stop: mocks.stop, cancel: mocks.cancel })
  mocks.stop.mockResolvedValue(voiceWav(new Float32Array(16)))
  mocks.cancel.mockResolvedValue(undefined)
})
afterEach(() => {
  for (const cleanup of mocks.cleanups.splice(0)) cleanup()
  mocks.phases.length = 0
  mocks.said.length = 0
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})
const api = (settings: Awaited<ReturnType<NonNullable<VoiceApi['settings']>>>, ready = false): VoiceApi => ({
  ready: vi.fn(async () => ready), download: vi.fn(async () => ({ ok: true as const })),
  transcribe: vi.fn(async () => ({ ok: true as const, text: 'Words.' })), cancel: vi.fn(async () => undefined), onProgress: () => () => undefined,
  settings: vi.fn(async () => settings)
})
const press = (element: ReactElement): void => {
  const button = (element.props as { children: ReactElement[] }).children[0]
  ;(button!.props as { onClick(): void }).onClick()
}
const settle = async (bridge: VoiceApi): Promise<void> => {
  await vi.waitFor(() => { expect(bridge.settings).toHaveBeenCalled() })
  for (let turn = 0; turn < 4; turn++) await Promise.resolve()
}

describe('voice typing in a browser', () => {
  it('says allowing OpenAI is done on the computer, instead of asking here', async () => {
    place('browser')
    const bridge = api({ mode: 'openai', hasKey: true, openaiConsent: false })
    press(VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!)
    await settle(bridge)
    expect(mocks.phases).not.toContain('asking')
    expect(mocks.said).toContain(VOICE_ALLOW_THERE)
    expect(mocks.start).not.toHaveBeenCalled()
  })

  it("says the download is made on the computer, and asks the computer's settings, not this system", async () => {
    place('browser')
    // A Windows computer on Fast, reached from a Mac's browser: not "Fast runs on Windows for now".
    const bridge = api({ mode: 'fast', hasKey: false, openaiConsent: false })
    press(VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!)
    await settle(bridge)
    expect(bridge.ready).toHaveBeenCalledWith('fast')
    expect(mocks.phases).not.toContain('asking')
    expect(mocks.said).toEqual([''].concat(VOICE_DOWNLOAD_THERE))
  })

  it('says the key is added on the computer', async () => {
    place('browser')
    const bridge = api({ mode: 'openai', hasKey: false, openaiConsent: false })
    press(VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!)
    await settle(bridge)
    expect(mocks.said).toContain(VOICE_KEY_THERE)
  })

  it('records here and types through the computer once it is allowed there', async () => {
    place('browser')
    const bridge = api({ mode: 'openai', hasKey: true, openaiConsent: true })
    const element = VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!
    press(element)
    await vi.waitFor(() => { expect(mocks.start).toHaveBeenCalled() })
    press(element)
    await vi.waitFor(() => { expect(bridge.transcribe).toHaveBeenCalledWith(expect.any(Uint8Array), 'openai') })
  })

  it('says a refusal for what it is, not a microphone to check', async () => {
    place('browser')
    const bridge = api({ mode: 'openai', hasKey: true, openaiConsent: true })
    bridge.settings = vi.fn(async () => { throw new Error(WINDOW_ONLY_SAYS) })
    press(VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!)
    await settle(bridge)
    expect(mocks.said).toContain(WINDOW_ONLY_SAYS)
  })
})

describe('voice typing on the computer', () => {
  it('asks to allow OpenAI there, as ever', async () => {
    place('computer')
    const bridge = api({ mode: 'openai', hasKey: true, openaiConsent: false })
    press(VoiceButton({ platform: 'darwin', api: bridge, onText: vi.fn() })!)
    await settle(bridge)
    expect(mocks.phases).toContain('asking')
    expect(mocks.said).not.toContain(VOICE_ALLOW_THERE)
  })
})
