/*
 * LOCUST IN A BROWSER (0.830, "Away from your desk", step 3). The whole window, in a browser, driven by this
 * computer: the browser loads the window's own code for this computer's version from Locust's relay
 * (app.locust.lol/desk), never from the computer, and makes the window's calls (desktop-api.ts) through the sealed
 * line a paired phone already uses (phone-anywhere.ts). The computer answers each as desk-calls.ts allows, tells the
 * browser what it would tell its own window, and while a browser has the window, shows "Locust is open in a browser"
 * in its own, so only one window acts at a time.
 *
 * What travels inside one sealed frame each way.
 */

/** The window's two one-off calls on this computer, from the cover it shows while a browser has the window. */
export const DESK_TAKE_BACK_CHANNEL = 'desk:take-back'
export const DESK_WHERE_CHANNEL = 'desk:where'
/** A paired device may open the full window, or may not: set in Settings on this computer only. */
export const PHONE_DESK_CHANNEL = 'phone:desk'
/**
 * The window's address while a browser has it: the cover, with no app running behind it. A query, not a #hash: a
 * window moved to a hash of its own page stays the same page, and the app went on running under it (the first drive).
 */
export const DESK_COVER_QUERY = 'elsewhere'

/** What the browser asks, by kind: who the computer is, to have the window, a call, or what is waiting already. */
export type DeskAsk =
  | { readonly kind: 'hello'; readonly id: string }
  | { readonly kind: 'take'; readonly id: string }
  | { readonly kind: 'call'; readonly id: string; readonly channel: string; readonly args: string }
  | { readonly kind: 'replay'; readonly id: string; readonly what: 'approvals' }

/** What the computer tells it: an answer, something the window is told, or that the window went elsewhere. */
export type DeskTell =
  | { readonly kind: 'res'; readonly id: string; readonly ok: true; readonly value: string }
  | { readonly kind: 'res'; readonly id: string; readonly ok: false; readonly message: string; readonly refused?: true }
  | { readonly kind: 'ev'; readonly channel: string; readonly args: string }
  | { readonly kind: 'taken'; readonly by: 'computer' | 'browser' }

/** The computer, as a browser opening the window needs to know it. */
export interface DeskHello {
  /** Locust's version there: the browser loads the window of this one. */
  readonly version: string
  /** The computer's name, as the cover and the browser's title say it. */
  readonly computer: string
  readonly theme: 'dark' | 'light'
  /** Who has the window now: this browser, the computer's own window, or another browser. */
  readonly active: 'here' | 'computer' | 'other'
}

/** A sealed frame's words, either way: one frame of a desk session, numbered within it. */
export interface DeskFrame<T> {
  readonly v: 1
  readonly kind: 'desk'
  /** The browser tab's own session, and its frames in order: one not after the last is a copy, and dropped. */
  readonly s: string
  readonly q: number
  readonly t: number
  readonly items: readonly T[]
}

/** A frame too long for one sealed box goes in parts, put back together on the other side. */
export interface DeskPart {
  readonly v: 1
  readonly kind: 'desk-part'
  readonly id: string
  readonly n: number
  readonly of: number
  readonly text: string
}

/** The most characters one part carries, inside the relay's 512 KiB box. */
export const DESK_PART_CHARS = 240_000
/** The longest frame put back together: an attachment pasted in a browser fits, a flood does not. */
export const DESK_MOST_CHARS = 48 * 1024 * 1024

/*
 * WHAT ELECTRON'S IPC KEEPS, KEPT. The window's calls go through Electron's structured clone, which keeps bytes and an
 * undefined in an argument list; JSON keeps neither. A pasted picture is bytes, and many calls take
 * `(undefined, x)`: a null there would be read as a value.
 */
export function deskEncode(value: unknown): string {
  return JSON.stringify(value, function (this: unknown, _key: string, item: unknown) {
    if (item instanceof Uint8Array) return { $bytes: bytesToBase64(item) }
    if (item instanceof ArrayBuffer) return { $bytes: bytesToBase64(new Uint8Array(item)) }
    // An answer of undefined, and an array's: an object's undefined is left out, as Electron leaves it. A null where
    // the window's code looks for undefined is read as a value: `x !== undefined ? x.title` threw on the first drive.
    if (item === undefined && (Array.isArray(this) || _key === '')) return { $undefined: true }
    return item
  }) ?? 'null'
}

export function deskDecode(text: string): unknown {
  return JSON.parse(text, (_key, item: unknown) => {
    if (typeof item === 'object' && item !== null && !Array.isArray(item)) {
      const fields = item as Record<string, unknown>
      const keys = Object.keys(fields)
      if (keys.length === 1 && typeof fields.$bytes === 'string') return base64ToBytes(fields.$bytes)
      if (keys.length === 1 && fields.$undefined === true) return undefined
    }
    return item
  })
}

function bytesToBase64(bytes: Uint8Array): string {
  let text = ''
  for (let at = 0; at < bytes.length; at += 0x8000) text += String.fromCharCode(...bytes.subarray(at, at + 0x8000))
  return btoa(text)
}

function base64ToBytes(text: string): Uint8Array {
  const raw = atob(text)
  const bytes = new Uint8Array(raw.length)
  for (let at = 0; at < raw.length; at += 1) bytes[at] = raw.charCodeAt(at)
  return bytes
}

/** A frame's text in parts small enough for the relay, each with the frame's id and its place. */
export function deskParts(id: string, text: string): readonly DeskPart[] {
  const of = Math.max(1, Math.ceil(text.length / DESK_PART_CHARS))
  return Array.from({ length: of }, (_, n) => ({ v: 1, kind: 'desk-part', id, n, of, text: text.slice(n * DESK_PART_CHARS, (n + 1) * DESK_PART_CHARS) }))
}

/** Parts put back together, a frame at a time: whole text once every part is in, undefined until then. */
export function deskJoiner(options: { readonly mostChars?: number; readonly now?: () => number; readonly waitMs?: number } = {}) {
  const most = options.mostChars ?? DESK_MOST_CHARS
  const now = options.now ?? Date.now
  const waitMs = options.waitMs ?? 120_000
  const open = new Map<string, { readonly of: number; readonly parts: (string | undefined)[]; got: number; chars: number; readonly since: number }>()
  return (part: DeskPart): string | undefined => {
    for (const [id, held] of open) if (now() - held.since > waitMs) open.delete(id)
    if (!Number.isInteger(part.of) || part.of < 1 || part.of > Math.ceil(most / DESK_PART_CHARS) + 1 || !Number.isInteger(part.n) || part.n < 0 || part.n >= part.of) return undefined
    const held = open.get(part.id) ?? { of: part.of, parts: new Array<string | undefined>(part.of).fill(undefined), got: 0, chars: 0, since: now() }
    if (held.of !== part.of || held.parts[part.n] !== undefined) return undefined
    held.parts[part.n] = part.text
    held.got += 1
    held.chars += part.text.length
    if (held.chars > most) { open.delete(part.id); return undefined }
    open.set(part.id, held)
    if (held.got < held.of) return undefined
    open.delete(part.id)
    return held.parts.join('')
  }
}

export const isDeskAsk = (value: unknown): value is DeskAsk => {
  if (typeof value !== 'object' || value === null) return false
  const ask = value as Record<string, unknown>
  if (typeof ask.id !== 'string' || ask.id.length === 0 || ask.id.length > 64) return false
  if (ask.kind === 'hello' || ask.kind === 'take') return true
  if (ask.kind === 'replay') return ask.what === 'approvals'
  return ask.kind === 'call' && typeof ask.channel === 'string' && ask.channel.length > 0 && ask.channel.length <= 120 && typeof ask.args === 'string'
}
