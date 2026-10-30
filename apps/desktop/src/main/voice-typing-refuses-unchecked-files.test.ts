import { createHash } from 'node:crypto'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { downloadVoiceFile, runVoiceCli, validVoiceWav, createVoiceHost } from './voice-host.js'
import { voicePermission } from './voice-permission.js'
import { voiceWav } from '../shared/voice-pcm.js'

const folders: string[] = []
async function folder(): Promise<string> { const path = await mkdtemp(join(tmpdir(), 'voice-unit-')); folders.push(path); return path }
afterEach(async () => { for (const path of folders.splice(0)) await rm(path, { recursive: true, force: true }) })
const hash = (data: string): string => createHash('sha256').update(data).digest('hex')
const asset = { url: 'https://example.test/model', bytes: 4, sha256: hash('good') }
const standIn = (data: string): typeof fetch => vi.fn(async () => new Response(data)) as unknown as typeof fetch

describe('voice typing refuses unchecked files', () => {
  it('refuses and deletes a SHA-256 mismatch, including stale cached files', async () => {
    const path = join(await folder(), 'model')
    await writeFile(path, 'old')
    await expect(downloadVoiceFile(asset, path, new AbortController().signal, () => undefined, standIn('evil'))).rejects.toThrow('SHA-256')
    await expect(readFile(path)).rejects.toMatchObject({ code: 'ENOENT' })
    await expect(readFile(path + '.part')).rejects.toMatchObject({ code: 'ENOENT' })
  })
  it('promotes only the checked file and reuses it without a connection', async () => {
    const path = join(await folder(), 'model')
    const fetcher = standIn('good')
    const progress = vi.fn()
    await downloadVoiceFile(asset, path, new AbortController().signal, progress, fetcher)
    await downloadVoiceFile(asset, path, new AbortController().signal, progress, fetcher)
    expect(await readFile(path, 'utf8')).toBe('good')
    expect(fetcher).toHaveBeenCalledTimes(1)
    expect(progress).toHaveBeenCalledWith(4)
  })
  it('restarts an interrupted partial file cleanly', async () => {
    const path = join(await folder(), 'model')
    await writeFile(path + '.part', 'interrupted')
    const cancelled = new AbortController(); cancelled.abort()
    await expect(downloadVoiceFile(asset, path, cancelled.signal, () => undefined, standIn('good'))).rejects.toBeDefined()
    await expect(readFile(path + '.part')).rejects.toMatchObject({ code: 'ENOENT' })
    await downloadVoiceFile(asset, path, new AbortController().signal, () => undefined, standIn('good'))
    expect(await readFile(path, 'utf8')).toBe('good')
  })
  it('refuses an oversized response and removes it', async () => {
    const path = join(await folder(), 'model')
    await expect(downloadVoiceFile(asset, path, new AbortController().signal, () => undefined, standIn('too large'))).rejects.toThrow('larger than expected')
    await expect(readFile(path + '.part')).rejects.toMatchObject({ code: 'ENOENT' })
  })
  it('runs a stand-in executable and returns its text', async () => {
    const script = join(await folder(), 'cli.cjs')
    await writeFile(script, 'process.stdout.write("Local words.\\n")')
    expect(await runVoiceCli(process.execPath, 'speech.wav', 'model', { prefix: [script], signal: new AbortController().signal })).toBe('Local words.')
  })
  it('stops a hung executable at its time limit and waits for termination', async () => {
    const dir = await folder()
    const pidFile = join(dir, 'pid')
    const script = join(dir, 'hang.cjs')
    // Finite safety net also makes the timeout negative control fail by assertion,
    // not by leaving a permanent child or merely timing out the test runner.
    await writeFile(script, `require('node:fs').writeFileSync(${JSON.stringify(pidFile)}, String(process.pid)); setTimeout(() => process.exit(0), 1800)`)
    await expect(runVoiceCli(process.execPath, 'speech.wav', 'model', { prefix: [script], signal: new AbortController().signal, timeoutMs: 1000 })).rejects.toThrow('took too long')
    const pid = Number(await readFile(pidFile, 'utf8'))
    expect(() => process.kill(pid, 0)).toThrow()
  })
  it('cancels a running executable without returning text', async () => {
    const dir = await folder()
    const marker = join(dir, 'started')
    const script = join(dir, 'cancel.cjs')
    await writeFile(script, `require('node:fs').writeFileSync(${JSON.stringify(marker)}, String(process.pid)); setInterval(() => {}, 1000)`)
    const controller = new AbortController()
    const run = runVoiceCli(process.execPath, '', '', { prefix: [script], signal: controller.signal })
    const result = expect(run).rejects.toThrow('cancelled')
    await vi.waitFor(async () => { expect(await readFile(marker, 'utf8')).toMatch(/^\d+$/) })
    controller.abort()
    await result
    const pid = Number(await readFile(marker, 'utf8'))
    // The cancel settles when the kill is sent. Windows ends the process then; elsewhere SIGKILL lands, and the child is
    // reaped, a moment later, so the test waits for it to be gone (vi.waitFor's second, then it fails as before).
    await vi.waitFor(() => { expect(() => process.kill(pid, 0)).toThrow() })
  })
  it('accepts only bounded canonical 16-kHz mono PCM', () => {
    const wav = voiceWav(new Float32Array(16000))
    expect(validVoiceWav(wav)).toBe(true)
    expect(validVoiceWav('a path')).toBe(false)
    const wrong = wav.slice(); new DataView(wrong.buffer).setUint16(22, 2, true)
    expect(validVoiceWav(wrong)).toBe(false)
    expect(validVoiceWav(new Uint8Array(2_000_000))).toBe(false)
  })
  it('constructs and disposes without creating a voice directory', async () => {
    const dir = join(await folder(), 'not-created')
    const host = createVoiceHost(dir)
    await host.dispose()
    expect(await host.ready()).toBe(false)
    expect((await host.transcribe(voiceWav(new Float32Array(1)))).ok).toBe(false)
    await expect(readFile(join(dir, 'whisper-cli.exe'))).rejects.toMatchObject({ code: 'ENOENT' })
  })
})

describe('only the own main frame gets microphone audio', () => {
  it('allows audio only from the owned main frame', () => { expect(voicePermission(true, 'media', true, ['audio'])).toBe(true) })
  it.each([
    [false, 'media', true, ['audio']], [true, 'media', false, ['audio']],
    [true, 'media', true, ['video']], [true, 'media', true, ['audio', 'video']],
    [true, 'media', true, ['unknown']], [true, 'media', true, []],
    [true, 'geolocation', true, ['audio']]
  ] as const)('refuses another sender, subframe, camera, unknown media, or permission (%s %s %s %s)', (own, permission, main, types) => {
    expect(voicePermission(own, permission, main, types)).toBe(false)
  })
})
