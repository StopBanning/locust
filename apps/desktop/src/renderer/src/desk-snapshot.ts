/*
 * LOCUST IN A BROWSER OPENS AT ONCE, FROM WHAT IT LAST SAW (0.834, the Rakazo scrub's fourth take; desk-remote.ts).
 * Every open of app.locust.lol/desk waited on the relay and the computer before anything showed, and with the
 * computer off it showed only "not connected". Rakazo's clients open on their last state. So what the window READ
 * from the computer -- the conversations, the team, a conversation's turns -- is kept in this browser, sealed with a
 * key the browser keeps and will not hand out, and shown at once the next time, under a line that says it is what
 * Locust last showed and when. Nothing can be changed from it; once the computer answers, the window opens live.
 * Forgetting the pairing forgets it too.
 */
import { deskEncode } from '../../shared/desk.js'

export interface DeskSnapshot {
  /** When it was last written, in milliseconds. */
  readonly at: number
  /** Each read's last answer, by its channel and arguments, with when it was kept. */
  readonly answers: Readonly<Record<string, { readonly value: unknown; readonly at: number }>>
}

/** A read worth keeping is at most this long, as JSON; the whole of what is kept at most this. */
export const SNAPSHOT_ENTRY_CHARS = 512_000
export const SNAPSHOT_TOTAL_CHARS = 4_000_000

export const snapshotKey = (channel: string, args: readonly unknown[]): string => `${channel}|${deskEncode(args)}`

/** The snapshot with one more answer in it: too long an answer is not kept, and the oldest go once it is too long. */
export function snapshotWith(snapshot: DeskSnapshot | undefined, key: string, value: unknown, now: number): DeskSnapshot {
  const answers: Record<string, { readonly value: unknown; readonly at: number }> = { ...(snapshot?.answers ?? {}) }
  const size = (one: unknown): number => JSON.stringify(one ?? null).length
  if (size(value) > SNAPSHOT_ENTRY_CHARS) {
    delete answers[key]
  } else {
    answers[key] = { value, at: now }
    const sizes = new Map(Object.entries(answers).map(([name, entry]) => [name, size(entry.value)]))
    let total = [...sizes.values()].reduce((sum, one) => sum + one, 0)
    for (const [name] of Object.entries(answers).sort((a, b) => a[1].at - b[1].at)) {
      if (total <= SNAPSHOT_TOTAL_CHARS) break
      if (name === key) continue
      total -= sizes.get(name) ?? 0
      delete answers[name]
    }
  }
  return { at: now, answers }
}

const enc = new TextEncoder()
const dec = new TextDecoder()
const AAD = enc.encode('locust-desk-snapshot-v1') as Uint8Array<ArrayBuffer>

/** Sealed, so what is on this computer's disk is not the conversations as text. */
export async function sealSnapshot(key: CryptoKey, snapshot: DeskSnapshot): Promise<{ readonly iv: Uint8Array<ArrayBuffer>; readonly box: ArrayBuffer }> {
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const box = await crypto.subtle.encrypt({ name: 'AES-GCM', iv, additionalData: AAD }, key, enc.encode(JSON.stringify(snapshot)) as Uint8Array<ArrayBuffer>)
  return { iv, box }
}

export async function openSnapshot(key: CryptoKey, sealed: { readonly iv: Uint8Array<ArrayBuffer>; readonly box: ArrayBuffer }): Promise<DeskSnapshot | undefined> {
  try {
    const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: sealed.iv, additionalData: AAD }, key, sealed.box)
    const parsed = JSON.parse(dec.decode(plain)) as Partial<DeskSnapshot>
    return typeof parsed.at === 'number' && typeof parsed.answers === 'object' && parsed.answers !== null ? { at: parsed.at, answers: parsed.answers } : undefined
  } catch { return undefined }
}

/** How long ago, as the line over the window says it. */
export function snapshotWhen(at: number, now: number): string {
  const time = new Date(at).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
  const day = new Date(at).toDateString() === new Date(now).toDateString() ? '' : ` on ${new Date(at).toLocaleDateString([], { weekday: 'long', month: 'short', day: 'numeric' })}`
  return `${time}${day}`
}

/** Where the browser keeps it: its own sealing key, which cannot be read back out, and the sealed snapshot. */
export function snapshotStore() {
  const open = (): Promise<IDBDatabase | undefined> => new Promise((resolve) => {
    let request: IDBOpenDBRequest
    try { request = indexedDB.open('locust-desk-snapshot', 1) } catch { resolve(undefined); return }
    request.onupgradeneeded = () => { request.result.createObjectStore('kept') }
    request.onerror = () => resolve(undefined)
    request.onsuccess = () => resolve(request.result)
  })
  const get = async (name: string): Promise<unknown> => {
    const db = await open()
    if (db === undefined) return undefined
    return new Promise((resolve) => {
      const read = db.transaction('kept', 'readonly').objectStore('kept').get(name)
      read.onsuccess = () => { db.close(); resolve(read.result) }
      read.onerror = () => { db.close(); resolve(undefined) }
    })
  }
  const put = async (name: string, value: unknown): Promise<void> => {
    const db = await open()
    if (db === undefined) return
    await new Promise<void>((resolve) => {
      const tx = db.transaction('kept', 'readwrite')
      tx.objectStore('kept').put(value, name)
      tx.oncomplete = () => { db.close(); resolve() }
      tx.onerror = () => { db.close(); resolve() }
    })
  }
  const keyOf = async (): Promise<CryptoKey | undefined> => {
    const held = await get('key')
    if (held instanceof CryptoKey) return held
    try {
      // Not extractable: the browser seals and opens with it, and gives it to no one.
      const made = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt'])
      await put('key', made)
      return made
    } catch { return undefined }
  }
  return {
    async load(): Promise<DeskSnapshot | undefined> {
      const key = await keyOf()
      const sealed = await get('snapshot') as { iv: Uint8Array<ArrayBuffer>; box: ArrayBuffer } | undefined
      return key === undefined || sealed?.iv === undefined || sealed.box === undefined ? undefined : openSnapshot(key, sealed)
    },
    async save(snapshot: DeskSnapshot): Promise<void> {
      const key = await keyOf()
      if (key !== undefined) await put('snapshot', await sealSnapshot(key, snapshot))
    },
    async clear(): Promise<void> {
      await put('snapshot', undefined)
    }
  }
}
