/*
 * EACH RELEASE'S WINDOW, FROM ITS OWN ZIP (0.830, Locust in a browser). A release puts its window's built files --
 * what the installer carries in its app's renderer folder -- in one zip beside its installer on Locust's downloads
 * (window/Locust-<version>-window.zip, made by _tools/window-zip.mjs). The relay reads that zip's directory once,
 * then each file the browser asks for with one range request, so a browser only ever runs a release's own code, and
 * the relay keeps no copy of any.
 *
 * Stored or deflated entries, no zip64: a window is about 13 MB in under 200 files.
 */

export interface WindowEntry {
  readonly offset: number
  readonly compressed: number
  readonly size: number
  /** 0, stored; 8, deflated. */
  readonly method: 0 | 8
}

/** A byte range of the zip, or undefined when it cannot be read; with `end` undefined, the last `start` bytes. */
export type ZipRange = (start: number, end?: number) => Promise<{ readonly bytes: Uint8Array; readonly total: number } | undefined>

const u16 = (bytes: Uint8Array, at: number): number => bytes[at]! | (bytes[at + 1]! << 8)
const u32 = (bytes: Uint8Array, at: number): number => (bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16) | (bytes[at + 3]! << 24)) >>> 0
const MOST_ENTRIES = 5_000
const decoder = new TextDecoder()

/** The zip's directory: each file's place, by its path. Undefined when it is not a zip this reads. */
export async function windowDirectory(range: ZipRange): Promise<ReadonlyMap<string, WindowEntry> | undefined> {
  // The end record is in the last 22 bytes, unless a comment follows it: at most 64 KiB more.
  const tail = await range(22 + 65_535)
  if (tail === undefined) return undefined
  const { bytes, total } = tail
  let end = -1
  for (let at = bytes.length - 22; at >= 0; at -= 1) if (u32(bytes, at) === 0x06054b50) { end = at; break }
  if (end < 0) return undefined
  const count = u16(bytes, end + 10)
  const size = u32(bytes, end + 12)
  const start = u32(bytes, end + 16)
  if (count > MOST_ENTRIES || start + size > total) return undefined
  const directory = await range(start, start + size - 1)
  if (directory === undefined || directory.bytes.length !== size) return undefined
  const entries = new Map<string, WindowEntry>()
  let at = 0
  for (let index = 0; index < count; index += 1) {
    const d = directory.bytes
    if (at + 46 > d.length || u32(d, at) !== 0x02014b50) return undefined
    const method = u16(d, at + 10)
    const compressed = u32(d, at + 20)
    const fileSize = u32(d, at + 24)
    const nameLength = u16(d, at + 28)
    const extraLength = u16(d, at + 30)
    const commentLength = u16(d, at + 32)
    const offset = u32(d, at + 42)
    const name = decoder.decode(d.subarray(at + 46, at + 46 + nameLength))
    at += 46 + nameLength + extraLength + commentLength
    if (method !== 0 && method !== 8) continue
    if (name.endsWith('/')) continue
    entries.set(name, { offset, compressed, size: fileSize, method })
  }
  return entries
}

/** One file's bytes, from its own place in the zip; undefined when they cannot be read whole. */
export async function windowFile(range: ZipRange, entry: WindowEntry): Promise<Uint8Array<ArrayBuffer> | undefined> {
  // Its local header, then its bytes: the header's own name and extra field may be longer than the directory's,
  // so a little more is read than the directory says, and the header is read for where the bytes begin.
  const read = await range(entry.offset, entry.offset + 30 + 1_024 + entry.compressed - 1)
  if (read === undefined) return undefined
  const bytes = read.bytes
  if (bytes.length < 30 || u32(bytes, 0) !== 0x04034b50) return undefined
  const begins = 30 + u16(bytes, 26) + u16(bytes, 28)
  if (begins + entry.compressed > bytes.length) return undefined
  const data = bytes.slice(begins, begins + entry.compressed)
  if (entry.method === 0) return data
  const inflated = await new Response(new Blob([data]).stream().pipeThrough(new DecompressionStream('deflate-raw'))).arrayBuffer().catch(() => undefined)
  return inflated === undefined || inflated.byteLength !== entry.size ? undefined : new Uint8Array(inflated)
}

/** What a file is, by its name: only the kinds a built window has. */
export function windowType(path: string): string | undefined {
  const ending = /\.([a-z0-9]+)$/i.exec(path)?.[1]?.toLowerCase()
  switch (ending) {
    case 'html': return 'text/html; charset=utf-8'
    case 'js':
    case 'mjs': return 'text/javascript; charset=utf-8'
    case 'css': return 'text/css; charset=utf-8'
    case 'json': return 'application/json'
    case 'svg': return 'image/svg+xml'
    case 'png': return 'image/png'
    case 'jpg':
    case 'jpeg': return 'image/jpeg'
    case 'webp': return 'image/webp'
    case 'gif': return 'image/gif'
    case 'ico': return 'image/x-icon'
    case 'woff2': return 'font/woff2'
    case 'woff': return 'font/woff'
    case 'ttf': return 'font/ttf'
    case 'wasm': return 'application/wasm'
    case 'onnx': return 'application/octet-stream'
    case 'txt': return 'text/plain; charset=utf-8'
    default: return undefined
  }
}

/** A path a browser may ask for: inside the window, plain characters, never upwards. */
export const WINDOW_PATH = /^(?!.*\.\.)[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_.-]+)*$/
export const WINDOW_VERSION = /^\d{1,4}\.\d{1,5}\.\d{1,6}$/
