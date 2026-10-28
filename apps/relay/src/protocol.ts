/**
 * LOCUST'S RELAY (0.826): what a PC, a paired phone and a phone being paired say to the relay, and what it says back.
 *
 * Phone messages are sealed: the relay sees who talks to whom and when, never what. Their words are sealed on one of the person's
 * devices and opened on another (apps/desktop/src/main/relay-crypto.ts, and the same steps in the phone's page), so
 * `n` and `c` below are a nonce and a sealed box it only passes on. What it does keep is small: a hash of the PC's
 * token, a hash of each paired device's token, and while a code is shown, a hash of that code's token.
 * Called links (2026-10-25) are the explicit exception: their bodies are readable data, for the PC alone, at most a week.
 */

/** 16 random bytes, base64url: a room is one PC's. */
export const ROOM_ID = /^[A-Za-z0-9_-]{22}$/
/** A paired device's id, cut from a hash of its public key. */
export const DEVICE_ID = /^[A-Za-z0-9_-]{22}$/
/** 32 random bytes, base64url: the PC's token, a device's token, a pairing token. */
export const TOKEN = /^[A-Za-z0-9_-]{43}$/
/** SHA-256, base64url. */
export const HASH = /^[A-Za-z0-9_-]{43}$/
/** An X25519 public key, base64url: sent only by a phone being paired, so the PC can open what it sealed. */
export const PUBLIC_KEY = /^[A-Za-z0-9_-]{43}$/
/** AES-GCM's 12-byte nonce, base64url. */
export const NONCE = /^[A-Za-z0-9_-]{16}$/
/** A sealed box: at least AES-GCM's 16-byte tag, at most 512 KiB. A conversation's last turns fit in one. */
export const MAX_SEALED_BYTES = 512 * 1024
const SEALED = new RegExp(`^[A-Za-z0-9_-]{22,${String(Math.ceil((MAX_SEALED_BYTES * 4) / 3))}}$`)
/** A phone being paired, until it is: the relay names its connection, and the PC answers it by that name. */
export const PAIRING_TAG = /^[A-Za-z0-9_-]{12}$/
/** The longest frame read at all. Cloudflare's own limit is 1 MiB. */
export const MAX_FRAME = 720 * 1024
/** A request id the PC may put on a command, so it knows which one the relay has done. */
const COMMAND_ID = /^[A-Za-z0-9_-]{1,32}$/

/*
 * SIGNED IN WITH GITHUB (0.838). A PC may tie its room to the person's GitHub account; then a browser where they sign
 * in to GitHub is given a one-time ticket into the room, the way a code's token lets a phone in, and the PC checks the
 * account with GitHub itself before it pairs that browser. The relay keeps the account's number and name, the PC's
 * public key and its name, and hashes of the tickets it hands out.
 */
/** A GitHub account's number. */
export const GITHUB_USER = /^[1-9]\d{0,15}$/
/** A GitHub login: letters, digits and single hyphens, at most 39. */
export const GITHUB_LOGIN = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/
/** What a computer is called where a browser chooses between them: plain words, at most 60. */
export const COMPUTER_NAME = /^[\p{L}\p{N} '.,()_-]{1,60}$/u
/** The account a room is tied to, as the PC is told it. */
export interface GithubAccount { readonly user: number; readonly login: string }

/**
 * SCOUT'S AWAY LOOKS (0.829). While the PC is off, a workflow in the repository Scout watches looks for it and leaves
 * a letter here, sealed with a key the relay never sees; the PC and its phones read the letters when they come. A
 * letter's id is when it came, then random, so ids sort by time.
 */
export const LETTER_ID = /^\d{13}-[A-Za-z0-9_-]{8}$/
/** A letter's sealed box: one look's findings fit in far less. */
export const MAX_LETTER_BYTES = 64 * 1024
const LETTER = new RegExp(`^[A-Za-z0-9_-]{22,${String(Math.ceil((MAX_LETTER_BYTES * 4) / 3))}}$`)
/** Who left a letter: the away look, or the PC saying what it already told the person. */
export type LetterFrom = 'away' | 'pc'
export interface Letter { readonly id: string; readonly at: number; readonly from: LetterFrom; readonly n: string; readonly c: string }
/** A called link (2026-10-25): readable data for the PC alone, never a sealed phone message. */
export interface HookLetter { readonly id: string; readonly at: number; readonly hook: string; readonly body: string; readonly contentType: string }
export const HOOK_ID = /^[A-Za-z0-9_-]{1,80}$/
/** One routine brief's data budget; one extra character makes the PC's cut line explicit. */
export const HOOK_BODY_CHARS = 4_000
/** Which of a phone's two kept alerts a letter rings: something found, or the morning brief. */
export type AwayAlertKind = 'found' | 'brief'
/** One phone's two alerts, sealed by the PC for its browser ahead of time, so the relay rings it with no PC on. */
export interface AwayAlert { readonly device: string; readonly endpoint: string; readonly found: string; readonly brief: string }

/**
 * WRITTEN WHILE THE COMPUTER IS OFF (0.837). A paired phone leaves a message here for its PC, sealed with the key the
 * two of them share, and the PC collects it when it is back. The relay keeps who left it and when, and, once the PC has
 * looked, a one-word reason it did not send it; never the words.
 */
export const MAX_NOTES = 20
/** Why the PC left a note unsent: it waited too long to send unasked, its conversation is gone, or the send failed. */
export type NoteHeld = 'old' | 'gone' | 'failed'
export const NOTE_HELD: readonly NoteHeld[] = ['old', 'gone', 'failed']
export interface Note { readonly id: string; readonly at: number; readonly device: string; readonly n: string; readonly c: string; readonly held?: NoteHeld }
/** What a phone is told of its own notes: which still wait, and why one was held. Its words it keeps itself. */
export interface NoteState { readonly id: string; readonly at: number; readonly held?: NoteHeld }

/** Who a message goes to: the PC, a paired device, or a phone being paired. */
export type Address = 'pc' | `pairing:${string}` | string

export type ClientFrame =
  | { readonly t: 'hook-open'; readonly hook: string; readonly hash: string; readonly id?: string }
  | { readonly t: 'hook-close'; readonly hook: string; readonly id?: string }
  | { readonly t: 'hook-letters'; readonly hook: string; readonly id?: string }
  | { readonly t: 'hook-collected'; readonly hook: string; readonly letters: readonly string[]; readonly id?: string }
  | { readonly t: 'auth'; readonly as: 'pc'; readonly token: string }
  | { readonly t: 'auth'; readonly as: 'device'; readonly device: string; readonly token: string }
  | { readonly t: 'auth'; readonly as: 'pairing'; readonly token: string }
  | { readonly t: 'send'; readonly to: Address; readonly n: string; readonly c: string; readonly k?: string }
  | { readonly t: 'allow'; readonly device: string; readonly hash: string; readonly id?: string }
  | { readonly t: 'remove'; readonly device: string; readonly id?: string }
  | { readonly t: 'pair-open'; readonly hash: string; readonly until: number; readonly id?: string }
  | { readonly t: 'pair-close'; readonly id?: string }
  | { readonly t: 'reset'; readonly id?: string }
  | { readonly t: 'push'; readonly endpoint: string; readonly body: string; readonly urgency: 'normal' | 'high'; readonly id?: string }
  | { readonly t: 'away-open'; readonly hash: string; readonly id?: string }
  | { readonly t: 'away-close'; readonly id?: string }
  | { readonly t: 'away-alerts'; readonly alerts: readonly AwayAlert[]; readonly id?: string }
  | { readonly t: 'post'; readonly n: string; readonly c: string; readonly id?: string }
  | { readonly t: 'letters'; readonly after?: string; readonly newest?: number; readonly id?: string }
  | { readonly t: 'note'; readonly n: string; readonly c: string; readonly id?: string }
  | { readonly t: 'unnote'; readonly note: string; readonly id?: string }
  | { readonly t: 'notes'; readonly id?: string }
  | { readonly t: 'notes-collected'; readonly notes: readonly string[]; readonly held: readonly { readonly note: string; readonly why: NoteHeld }[]; readonly id?: string }
  /** The PC is about to send the person to GitHub: the link's hash, the PC's public key, and what it is called. */
  | { readonly t: 'github-open'; readonly hash: string; readonly key: string; readonly name: string; readonly id?: string }
  /** No browser signs in with GitHub any more, and the account is let go. */
  | { readonly t: 'github-close'; readonly id?: string }

/** Where a browser's subscription may point. The relay posts to nothing else. */
const PUSH_HOSTS = [/^fcm\.googleapis\.com$/, /^android\.googleapis\.com$/, /^updates\.push\.services\.mozilla\.com$/, /^push\.services\.mozilla\.com$/, /^web\.push\.apple\.com$/, /^[a-z0-9-]+\.notify\.windows\.com$/]

export function isPushEndpoint(endpoint: string): boolean {
  let url: URL
  try { url = new URL(endpoint) } catch { return false }
  return url.protocol === 'https:' && url.port === '' && url.username === '' && PUSH_HOSTS.some((host) => host.test(url.hostname))
}

/** A push's sealed body, at most one 4 KiB record (RFC 8291), base64url. */
const PUSH_BODY = /^[A-Za-z0-9_-]{120,5462}$/

export type ErrorCode =
  /** The token was wrong, or the device is not paired (any more). */
  | 'auth'
  /** No PC has made this room. */
  | 'room'
  /** No code is open, or this one was used or ran out. */
  | 'pairing-closed'
  /** The PC is not connected now. */
  | 'pc-offline'
  /** The one being written to is not connected now. */
  | 'peer-offline'
  | 'bad-frame'
  | 'too-big'
  | 'too-fast'
  /** Only the PC may do that. */
  | 'not-owner'
  /** The room has as many devices as it may. */
  | 'full'
  /** The push service refused the alert, or could not be reached. */
  | 'push-failed'
  /** The push service says the phone's subscription is gone: stop sending to it. */
  | 'push-gone'
  /** This relay has no key to sign pushes with. */
  | 'push-off'
  /** Scout's away looks are not on for this room. */
  | 'away-off'
  /** That note was collected, taken back, or ran out. */
  | 'note-gone'
  /** This relay does not offer signing in with GitHub. */
  | 'github-off'

export type RelayFrame =
  | { readonly t: 'hook-letter'; readonly hook: string }
  | { readonly t: 'hook-letters'; readonly items: readonly HookLetter[]; readonly lost: number; readonly id?: string }
  /** `github`, from a relay that offers it: the account the room is tied to, or false for none. */
  | { readonly t: 'ok'; readonly as: 'pc'; readonly devices: readonly string[]; readonly github?: GithubAccount | false }
  | { readonly t: 'ok'; readonly as: 'device' | 'pairing'; readonly pc: boolean }
  | { readonly t: 'msg'; readonly from: Address; readonly n: string; readonly c: string; readonly k?: string }
  | { readonly t: 'presence'; readonly pc: boolean }
  | { readonly t: 'presence'; readonly device: string; readonly on: boolean }
  | { readonly t: 'done'; readonly of: 'allow' | 'remove' | 'pair-open' | 'pair-close' | 'reset' | 'push' | 'away-open' | 'away-close' | 'away-alerts' | 'post' | 'hook-open' | 'hook-close' | 'hook-collected' | 'notes-collected' | 'github-open' | 'github-close'; readonly id?: string }
  /** To the PC: the person signed in to GitHub, and the room is tied to this account now. */
  | { readonly t: 'github'; readonly account: GithubAccount }
  | { readonly t: 'letters'; readonly items: readonly Letter[]; readonly more: boolean; readonly id?: string }
  /** A letter that came while this connection was open. */
  | { readonly t: 'letter'; readonly item: Letter }
  | { readonly t: 'removed' }
  | { readonly t: 'err'; readonly code: ErrorCode; readonly id?: string }
  /** A phone's note was kept, under this id. */
  | { readonly t: 'noted'; readonly note: string; readonly id?: string }
  | { readonly t: 'unnoted'; readonly note: string; readonly id?: string }
  /** The PC is told every note waiting, with its sealed box; a phone only its own, without. */
  | { readonly t: 'notes'; readonly items: readonly Note[]; readonly id?: string }
  | { readonly t: 'my-notes'; readonly items: readonly NoteState[]; readonly id?: string }
  /** To the PC: a phone left a note while it was connected. To a phone: what became of its notes changed. */
  | { readonly t: 'notes-changed' }

const isAddress = (value: unknown): value is Address =>
  typeof value === 'string' && (value === 'pc' || DEVICE_ID.test(value) || (value.startsWith('pairing:') && PAIRING_TAG.test(value.slice(8))))

const withId = (fields: Record<string, unknown>): { readonly id?: string } | undefined => {
  if (fields.id === undefined) return {}
  return typeof fields.id === 'string' && COMMAND_ID.test(fields.id) ? { id: fields.id } : undefined
}

/** A frame as the relay reads it, or undefined for anything else: a field it does not know is refused, not kept. */
export function parseClientFrame(text: string): ClientFrame | undefined {
  if (text.length > MAX_FRAME) return undefined
  let value: unknown
  try { value = JSON.parse(text) } catch { return undefined }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields = value as Record<string, unknown>
  const keys = Object.keys(fields)
  const only = (...allowed: readonly string[]): boolean => keys.every((key) => allowed.includes(key))
  const id = withId(fields)
  switch (fields.t) {
    case 'hook-open':
      if (id === undefined || !only('t', 'hook', 'hash', 'id') || typeof fields.hook !== 'string' || !HOOK_ID.test(fields.hook) || typeof fields.hash !== 'string' || !HASH.test(fields.hash)) return undefined
      return { t: 'hook-open', hook: fields.hook, hash: fields.hash, ...id }
    case 'hook-close':
    case 'hook-letters':
      if (id === undefined || !only('t', 'hook', 'id') || typeof fields.hook !== 'string' || !HOOK_ID.test(fields.hook)) return undefined
      return { t: fields.t, hook: fields.hook, ...id }
    case 'hook-collected':
      if (id === undefined || !only('t', 'hook', 'letters', 'id') || typeof fields.hook !== 'string' || !HOOK_ID.test(fields.hook)
        || !Array.isArray(fields.letters) || fields.letters.length > LETTERS_AT_ONCE || !fields.letters.every((item) => typeof item === 'string' && LETTER_ID.test(item))) return undefined
      return { t: 'hook-collected', hook: fields.hook, letters: fields.letters as string[], ...id }
    case 'auth':
      if (fields.as === 'pc' && only('t', 'as', 'token') && typeof fields.token === 'string' && TOKEN.test(fields.token)) return { t: 'auth', as: 'pc', token: fields.token }
      if (fields.as === 'device' && only('t', 'as', 'device', 'token') && typeof fields.device === 'string' && DEVICE_ID.test(fields.device) && typeof fields.token === 'string' && TOKEN.test(fields.token)) {
        return { t: 'auth', as: 'device', device: fields.device, token: fields.token }
      }
      if (fields.as === 'pairing' && only('t', 'as', 'token') && typeof fields.token === 'string' && TOKEN.test(fields.token)) return { t: 'auth', as: 'pairing', token: fields.token }
      return undefined
    case 'send': {
      if (!only('t', 'to', 'n', 'c', 'k') || !isAddress(fields.to) || typeof fields.n !== 'string' || !NONCE.test(fields.n) || typeof fields.c !== 'string' || !SEALED.test(fields.c)) return undefined
      if (fields.k !== undefined && (typeof fields.k !== 'string' || !PUBLIC_KEY.test(fields.k))) return undefined
      return { t: 'send', to: fields.to, n: fields.n, c: fields.c, ...(typeof fields.k === 'string' ? { k: fields.k } : {}) }
    }
    case 'allow':
      if (id === undefined || !only('t', 'device', 'hash', 'id') || typeof fields.device !== 'string' || !DEVICE_ID.test(fields.device) || typeof fields.hash !== 'string' || !HASH.test(fields.hash)) return undefined
      return { t: 'allow', device: fields.device, hash: fields.hash, ...id }
    case 'remove':
      if (id === undefined || !only('t', 'device', 'id') || typeof fields.device !== 'string' || !DEVICE_ID.test(fields.device)) return undefined
      return { t: 'remove', device: fields.device, ...id }
    case 'pair-open':
      if (id === undefined || !only('t', 'hash', 'until', 'id') || typeof fields.hash !== 'string' || !HASH.test(fields.hash) || typeof fields.until !== 'number' || !Number.isSafeInteger(fields.until)) return undefined
      return { t: 'pair-open', hash: fields.hash, until: fields.until, ...id }
    case 'pair-close':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'pair-close', ...id }
    case 'reset':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'reset', ...id }
    case 'push':
      if (id === undefined || !only('t', 'endpoint', 'body', 'urgency', 'id') || typeof fields.endpoint !== 'string' || fields.endpoint.length > 1024
        || typeof fields.body !== 'string' || !PUSH_BODY.test(fields.body) || (fields.urgency !== 'normal' && fields.urgency !== 'high')) return undefined
      return { t: 'push', endpoint: fields.endpoint, body: fields.body, urgency: fields.urgency, ...id }
    case 'away-open':
      if (id === undefined || !only('t', 'hash', 'id') || typeof fields.hash !== 'string' || !HASH.test(fields.hash)) return undefined
      return { t: 'away-open', hash: fields.hash, ...id }
    case 'away-close':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'away-close', ...id }
    case 'away-alerts': {
      if (id === undefined || !only('t', 'alerts', 'id') || !Array.isArray(fields.alerts) || fields.alerts.length > MAX_AWAY_ALERTS) return undefined
      const alerts = fields.alerts.map(awayAlertOf)
      if (alerts.some((alert) => alert === undefined)) return undefined
      return { t: 'away-alerts', alerts: alerts as AwayAlert[], ...id }
    }
    case 'post':
      if (id === undefined || !only('t', 'n', 'c', 'id') || typeof fields.n !== 'string' || !NONCE.test(fields.n) || typeof fields.c !== 'string' || !LETTER.test(fields.c)) return undefined
      return { t: 'post', n: fields.n, c: fields.c, ...id }
    case 'letters':
      if (id === undefined || !only('t', 'after', 'newest', 'id')) return undefined
      if (fields.after !== undefined && (typeof fields.after !== 'string' || !LETTER_ID.test(fields.after))) return undefined
      if (fields.newest !== undefined && (typeof fields.newest !== 'number' || !Number.isInteger(fields.newest) || fields.newest < 1 || fields.newest > LETTERS_AT_ONCE)) return undefined
      if (fields.after !== undefined && fields.newest !== undefined) return undefined
      return { t: 'letters', ...(fields.after === undefined ? {} : { after: fields.after }), ...(fields.newest === undefined ? {} : { newest: fields.newest }), ...id }
    case 'note':
      if (id === undefined || !only('t', 'n', 'c', 'id') || typeof fields.n !== 'string' || !NONCE.test(fields.n) || typeof fields.c !== 'string' || !LETTER.test(fields.c)) return undefined
      return { t: 'note', n: fields.n, c: fields.c, ...id }
    case 'unnote':
      if (id === undefined || !only('t', 'note', 'id') || typeof fields.note !== 'string' || !LETTER_ID.test(fields.note)) return undefined
      return { t: 'unnote', note: fields.note, ...id }
    case 'notes':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'notes', ...id }
    case 'notes-collected': {
      if (id === undefined || !only('t', 'notes', 'held', 'id') || !Array.isArray(fields.notes) || fields.notes.length > MAX_NOTES
        || !fields.notes.every((item) => typeof item === 'string' && LETTER_ID.test(item)) || !Array.isArray(fields.held) || fields.held.length > MAX_NOTES) return undefined
      const held = fields.held.map((value) => {
        if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
        const item = value as Record<string, unknown>
        if (!Object.keys(item).every((key) => key === 'note' || key === 'why') || typeof item.note !== 'string' || !LETTER_ID.test(item.note) || !NOTE_HELD.includes(item.why as NoteHeld)) return undefined
        return { note: item.note, why: item.why as NoteHeld }
      })
      if (held.some((item) => item === undefined)) return undefined
      return { t: 'notes-collected', notes: fields.notes as string[], held: held as { note: string; why: NoteHeld }[], ...id }
    }
    case 'github-open':
      if (id === undefined || !only('t', 'hash', 'key', 'name', 'id') || typeof fields.hash !== 'string' || !HASH.test(fields.hash)
        || typeof fields.key !== 'string' || !PUBLIC_KEY.test(fields.key) || typeof fields.name !== 'string' || !COMPUTER_NAME.test(fields.name)) return undefined
      return { t: 'github-open', hash: fields.hash, key: fields.key, name: fields.name, ...id }
    case 'github-close':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'github-close', ...id }
    default:
      return undefined
  }
}

/** The most phones one PC keeps away alerts for: as many as it may pair. */
export const MAX_AWAY_ALERTS = 20
/** The most letters one answer carries; a PC catching up asks again after the last. */
export const LETTERS_AT_ONCE = 50

function awayAlertOf(value: unknown): AwayAlert | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields = value as Record<string, unknown>
  if (!Object.keys(fields).every((key) => ['device', 'endpoint', 'found', 'brief'].includes(key))) return undefined
  const { device, endpoint, found, brief } = fields
  if (typeof device !== 'string' || !DEVICE_ID.test(device) || typeof endpoint !== 'string' || endpoint.length > 1024 || !isPushEndpoint(endpoint)) return undefined
  if (typeof found !== 'string' || !PUSH_BODY.test(found) || typeof brief !== 'string' || !PUSH_BODY.test(brief)) return undefined
  return { device, endpoint, found, brief }
}

/** What an away look posts: one sealed letter, and which alert it rings, if any. */
export interface AwayPost { readonly n: string; readonly c: string; readonly alert?: AwayAlertKind }

export function parseAwayPost(text: string): AwayPost | undefined {
  if (text.length > MAX_FRAME) return undefined
  let value: unknown
  try { value = JSON.parse(text) } catch { return undefined }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields = value as Record<string, unknown>
  if (!Object.keys(fields).every((key) => ['n', 'c', 'alert'].includes(key))) return undefined
  if (typeof fields.n !== 'string' || !NONCE.test(fields.n) || typeof fields.c !== 'string' || !LETTER.test(fields.c)) return undefined
  if (fields.alert !== undefined && fields.alert !== 'found' && fields.alert !== 'brief') return undefined
  return { n: fields.n, c: fields.c, ...(fields.alert === undefined ? {} : { alert: fields.alert }) }
}
