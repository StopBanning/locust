import { deflateRawSync } from 'node:zlib'

import { describe, expect, it } from 'vitest'

import { crc32, writeZip } from '../../../_tools/window-zip.mjs'
import { WINDOW_PATH, WINDOW_VERSION, windowDirectory, windowFile, windowType, type ZipRange } from '../src/window-zip.js'

/**
 * A RELEASE'S WINDOW COMES FROM ITS OWN ZIP (0.830, Locust in a browser). The release puts its window's files in one
 * zip beside its installer (_tools/window-zip.mjs); the relay reads the directory once and each file by a range of
 * its own (src/window-zip.ts). Only files of the kinds a window has, at plain paths inside it, are ever served.
 */
const ranged = (zip: Uint8Array): ZipRange & { calls: number } => {
  const range = (async (start: number, end?: number) => {
    range.calls += 1
    if (end === undefined) return { bytes: zip.subarray(Math.max(0, zip.length - start)), total: zip.length }
    return { bytes: zip.subarray(start, end + 1), total: zip.length }
  }) as ZipRange & { calls: number }
  range.calls = 0
  return range
}

/** One deflated entry, as other zip tools write a window: the relay reads those too. */
function deflatedZip(path: string, text: string): Uint8Array {
  const data = Buffer.from(text, 'utf8')
  const packed = deflateRawSync(data)
  const name = Buffer.from(path, 'utf8')
  const local = Buffer.alloc(30)
  local.writeUInt32LE(0x04034b50, 0); local.writeUInt16LE(20, 4); local.writeUInt16LE(8, 8)
  local.writeUInt32LE(crc32(data), 14); local.writeUInt32LE(packed.length, 18); local.writeUInt32LE(data.length, 22); local.writeUInt16LE(name.length, 26)
  // An extra field in the local header the directory does not repeat: where the bytes begin is read from the header.
  const extra = Buffer.from([0x55, 0x54, 0x05, 0x00, 0x01, 0, 0, 0, 0])
  local.writeUInt16LE(extra.length, 28)
  const central = Buffer.alloc(46)
  central.writeUInt32LE(0x02014b50, 0); central.writeUInt16LE(20, 4); central.writeUInt16LE(20, 6); central.writeUInt16LE(8, 10)
  central.writeUInt32LE(crc32(data), 16); central.writeUInt32LE(packed.length, 20); central.writeUInt32LE(data.length, 24); central.writeUInt16LE(name.length, 28)
  central.writeUInt32LE(0, 42)
  const body = Buffer.concat([local, name, extra, packed])
  const directory = Buffer.concat([central, name])
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(1, 8); end.writeUInt16LE(1, 10); end.writeUInt32LE(directory.length, 12); end.writeUInt32LE(body.length, 16)
  return new Uint8Array(Buffer.concat([body, directory, end]))
}

describe('a release’s window comes from its own zip', () => {
  it('reads the zip the release makes: each file whole, by its own range', async () => {
    const files = [
      { path: 'desk.html', bytes: Buffer.from('<!doctype html><title>Locust</title>') },
      { path: 'desk-remote.js', bytes: Buffer.from('window.desktop = {}') },
      { path: 'assets/main-abc.js', bytes: Buffer.from('x'.repeat(5000)) },
      { path: 'assets/font.woff2', bytes: Buffer.from([0, 1, 2, 3, 255]) }
    ]
    const zip = new Uint8Array(writeZip(files))
    const range = ranged(zip)
    const directory = await windowDirectory(range)
    expect([...(directory?.keys() ?? [])]).toEqual(files.map((file) => file.path))
    expect(range.calls).toBe(2)
    for (const file of files) {
      const before = range.calls
      expect(Buffer.from((await windowFile(range, directory!.get(file.path)!))!)).toEqual(file.bytes)
      expect(range.calls - before).toBe(1)
    }
  })

  it('makes the same zip of the same files, and writes each file’s CRC as zip tools check it', () => {
    const files = [{ path: 'a.txt', bytes: Buffer.from('hello') }]
    expect(Buffer.from(writeZip(files)).equals(Buffer.from(writeZip(files)))).toBe(true)
    expect(crc32(Buffer.from('hello'))).toBe(0x3610a686)
  })

  it('reads a deflated entry with a local field of its own', async () => {
    const zip = deflatedZip('assets/app.css', 'body { color: red }'.repeat(50))
    const range = ranged(zip)
    const directory = await windowDirectory(range)
    const bytes = await windowFile(range, directory!.get('assets/app.css')!)
    expect(new TextDecoder().decode(bytes)).toBe('body { color: red }'.repeat(50))
  })

  it('reads nothing that is not a zip, and nothing cut short', async () => {
    expect(await windowDirectory(ranged(new Uint8Array(400)))).toBeUndefined()
    const zip = new Uint8Array(writeZip([{ path: 'a.js', bytes: Buffer.from('1') }]))
    expect(await windowDirectory(async () => undefined)).toBeUndefined()
    const directory = await windowDirectory(ranged(zip))
    expect(await windowFile(async () => ({ bytes: zip.subarray(0, 10), total: zip.length }), directory!.get('a.js')!)).toBeUndefined()
  })

  it('serves only a window’s own kinds of file, at plain paths inside it, for a version that looks like one', () => {
    for (const ok of ['desk.html', 'assets/main-abc.js', 'assets/x.woff2', 'assets/onig.wasm']) expect(WINDOW_PATH.test(ok) && windowType(ok) !== undefined, ok).toBe(true)
    for (const no of ['../secret.js', 'assets/../../x.js', '/etc/passwd', 'a\\b.js', 'assets//x.js', '.']) expect(WINDOW_PATH.test(no) && windowType(no) !== undefined, no).toBe(false)
    expect(windowType('notes.exe')).toBeUndefined()
    expect(WINDOW_VERSION.test('0.830.0')).toBe(true)
    for (const no of ['latest', '0.830', '../0.830.0', '0.830.0-beta']) expect(WINDOW_VERSION.test(no), no).toBe(false)
  })
})
