import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { talkRunsOn } from '../shared/voice-call.js'
import { microphoneRunsOn, voicePermission } from './voice-permission.js'
import { speakHello, speakHelloCommand } from './voice-preview.js'

/**
 * TALK RUNS ON A MAC (the first Mac run, 2026-10-30). Colin: "That's definitely not how we had the voice working" --
 * Talk is ChatGPT's voice through his signed-in Codex, and on the Mac it was nowhere: the window drew no Talk, the
 * main side refused every call and preview as "on Windows for now", and the window was refused the microphone
 * outright, which also left Your OpenAI account's voice typing with nothing to record. On that Mac, Codex 0.166
 * signed in with ChatGPT took `thread/realtime/start` and passed it on to the voice service.
 */
describe('Talk on a Mac', () => {
  it('runs on Windows and a Mac, and not yet on Linux', () => {
    expect(talkRunsOn('win32')).toBe(true)
    expect(talkRunsOn('darwin')).toBe(true)
    expect(talkRunsOn('linux')).toBe(false)
    expect(talkRunsOn(undefined)).toBe(false)
  })

  it("gives the window the microphone on a Mac, and still only Locust's own audio", () => {
    expect(microphoneRunsOn('darwin')).toBe(true)
    expect(microphoneRunsOn('win32')).toBe(true)
    expect(microphoneRunsOn('linux')).toBe(false)
    expect(voicePermission(microphoneRunsOn('darwin'), 'media', true, ['audio'])).toBe(true)
    expect(voicePermission(microphoneRunsOn('darwin'), 'media', true, ['video'])).toBe(false)
    expect(voicePermission(microphoneRunsOn('darwin'), 'media', false, ['audio'])).toBe(false)
  })

  it("asks the Mac's own say for the preview's hello, and PowerShell on Windows", () => {
    expect(speakHello('darwin', '/tmp/a b/hello.wav')).toEqual({ command: '/usr/bin/say', args: ['-o', '/tmp/a b/hello.wav', '--file-format=WAVE', '--data-format=LEI16@24000', 'Hello?'] })
    expect(speakHello('win32', 'C:\\x\\hello.wav')).toEqual({ command: 'powershell.exe', args: speakHelloCommand('C:\\x\\hello.wav') })
  })

  it.runIf(process.platform === 'darwin')("gets a WAV from the Mac's say, as the preview checks it", () => {
    const dir = mkdtempSync(join(tmpdir(), 'locust-say-'))
    try {
      const wav = join(dir, 'hello.wav')
      const { command, args } = speakHello('darwin', wav)
      execFileSync(command, [...args], { timeout: 20_000 })
      const bytes = readFileSync(wav)
      expect(bytes.subarray(0, 4).toString('latin1')).toBe('RIFF')
      expect(bytes.subarray(8, 12).toString('latin1')).toBe('WAVE')
      // fmt: one channel, 24 kHz, 16 bits.
      expect(bytes.readUInt16LE(22)).toBe(1)
      expect(bytes.readUInt32LE(24)).toBe(24_000)
      expect(bytes.readUInt16LE(34)).toBe(16)
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })

  it("leaves no Windows-only gate on a call, a preview or the microphone; Talk's keys stay Windows' for now", () => {
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    // A handler's first two lines: its gate is the first thing it does.
    const handler = (channel: string): string => {
      const at = main.indexOf(`ipcMain.handle(${channel},`)
      expect(at).toBeGreaterThan(0)
      return main.slice(at).split('\n').slice(0, 2).join('\n')
    }
    for (const channel of ['VOICE_CALL_AVAILABLE', 'VOICE_CALL_START', 'VOICE_CALL_PREVIEW', 'VOICE_PREVIEW_TRIGGER']) {
      expect(handler(channel)).toContain('talkRunsOn(process.platform)')
      expect(handler(channel)).not.toContain("'win32'")
    }
    expect(main).toContain('speakHello(process.platform, wavPath)')
    expect(main).toContain('setPermissionRequestHandler((contents, permission, callback, details) => {\n      callback(voicePermission(microphoneRunsOn(process.platform) && ')
    expect(main).toContain('setPermissionCheckHandler((contents, permission, _origin, details) =>\n      voicePermission(microphoneRunsOn(process.platform) && ')
    expect(main).toContain("refusedHere: 'Talk from any app is on Windows for now.'")
  })
})
