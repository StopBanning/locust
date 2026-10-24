import { deskAccess, WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'
import { DESK_PART_CHARS, deskDecode, deskEncode, deskJoiner, deskParts, type DeskAsk, type DeskHello, type DeskTell } from '../../shared/desk.js'
import { desktopApiOver, type DesktopIpc, type IpcEvent } from '../../shared/desktop-api.js'
import type { DesktopApi } from '../../shared/ipc.js'
import { MISSION_APPROVAL_CHANNEL } from '../../shared/ipc.js'
import { snapshotKey, snapshotStore, snapshotWhen, snapshotWith, type DeskSnapshot } from './desk-snapshot.js'

/*
 * LOCUST IN A BROWSER, IN THE BROWSER (0.830, shared/desk.ts). Loaded before the window's own code on
 * app.locust.lol/desk/v/<version>/, this is the window's way into Locust there: the same calls as on the computer
 * (desktop-api.ts), each sent sealed through the relay with the keys this browser was paired with (the phone page's
 * own, kept where the browser keeps keys), and what the computer tells its window, heard here. It asks to have the
 * window as it opens; when the computer's window or another browser takes it, it says so over the window, with a way
 * to take it back.
 */

const VERSION = 'locust-relay-v1'
const enc = new TextEncoder()
const dec = new TextDecoder()

interface Link {
  readonly room: string
  readonly device: string
  readonly token: string
  readonly seal: CryptoKey
  readonly open: CryptoKey
}

const b64 = (bytes: Uint8Array): string => {
  let text = ''
  for (let at = 0; at < bytes.length; at += 0x8000) text += String.fromCharCode(...bytes.subarray(at, at + 0x8000))
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}
const unb64 = (text: string): Uint8Array<ArrayBuffer> => Uint8Array.from(atob(text.replaceAll('-', '+').replaceAll('_', '/')), (character) => character.charCodeAt(0))
const aad = (link: Link, direction: 'device-to-pc' | 'pc-to-device'): Uint8Array<ArrayBuffer> => enc.encode(`${VERSION}|${link.room}|${link.device}|${direction}`) as Uint8Array<ArrayBuffer>
const randomId = (bytes = 9): string => b64(crypto.getRandomValues(new Uint8Array(bytes)))

async function sealFor(link: Link, value: unknown): Promise<{ readonly n: string; readonly c: string }> {
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const sealed = await crypto.subtle.encrypt({ name: 'AES-GCM', iv, additionalData: aad(link, 'device-to-pc') }, link.seal, enc.encode(JSON.stringify(value)) as Uint8Array<ArrayBuffer>)
  return { n: b64(iv), c: b64(new Uint8Array(sealed)) }
}

async function openFrom(link: Link, n: string, c: string): Promise<unknown> {
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(n), additionalData: aad(link, 'pc-to-device') }, link.open, unb64(c))
  return JSON.parse(dec.decode(plain)) as unknown
}

/** The phone page's own pairing, where it keeps it (phone-page.ts): one browser, one pairing. */
function savedLink(): Promise<Link | undefined> {
  return new Promise((resolve) => {
    let request: IDBOpenDBRequest
    try { request = indexedDB.open('locust-relay', 1) } catch { resolve(undefined); return }
    request.onupgradeneeded = () => { request.result.createObjectStore('link') }
    request.onerror = () => resolve(undefined)
    request.onsuccess = () => {
      const db = request.result
      const read = db.transaction('link', 'readonly').objectStore('link').get('link')
      read.onsuccess = () => {
        db.close()
        const held = read.result as Partial<Link> | undefined
        resolve(held?.room !== undefined && held.device !== undefined && held.token !== undefined && held.seal !== undefined && held.open !== undefined ? held as Link : undefined)
      }
      read.onerror = () => { db.close(); resolve(undefined) }
    }
  })
}

/** The browser's own system, for what the window draws by it: keys, and Ctrl or ⌘. */
function browserPlatform(): string {
  const agent = `${navigator.userAgent} ${(navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? ''}`
  return /Mac|iPhone|iPad/i.test(agent) ? 'darwin' : /Win/i.test(agent) ? 'win32' : 'linux'
}

/* The words over the window, when it cannot be used here: in plain DOM, above the app, so they show whatever it does. */
function overlay() {
  let shown: HTMLElement | undefined
  return (words: string | undefined, action?: { readonly label: string; readonly run: () => void }): void => {
    if (words === undefined) { shown?.remove(); shown = undefined; return }
    if (shown === undefined) {
      shown = document.createElement('div')
      shown.setAttribute('role', 'dialog')
      shown.setAttribute('aria-label', 'Locust in a browser')
      shown.style.cssText = 'position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;background:rgba(9,10,12,.92);color:#e8e8e8;font:15px/1.5 system-ui,sans-serif;padding:24px'
      document.documentElement.appendChild(shown)
    }
    const box = document.createElement('div')
    box.style.cssText = 'max-width:440px;display:grid;gap:14px;text-align:center'
    const said = document.createElement('p')
    said.textContent = words
    said.style.margin = '0'
    box.appendChild(said)
    if (action !== undefined) {
      const button = document.createElement('button')
      button.type = 'button'
      button.textContent = action.label
      button.style.cssText = 'justify-self:center;padding:10px 18px;border-radius:10px;border:0;background:#e8e8e8;color:#0b0d0e;font:600 15px system-ui,sans-serif;cursor:pointer'
      button.addEventListener('click', action.run)
      box.appendChild(button)
    }
    shown.replaceChildren(box)
  }
}

/* The line over a window that is showing what Locust last showed (desk-snapshot.ts): thin, at the top, never in the way. */
function banner() {
  let shown: HTMLElement | undefined
  return (words: string | undefined, action?: { readonly label: string; readonly run: () => void }): void => {
    if (words === undefined) { shown?.remove(); shown = undefined; return }
    if (shown === undefined) {
      shown = document.createElement('div')
      shown.setAttribute('role', 'status')
      shown.setAttribute('aria-label', 'What Locust last showed here')
      shown.className = 'lc-deskbanner'
      shown.style.cssText = 'position:fixed;left:0;right:0;top:0;z-index:2147483646;display:flex;flex-wrap:wrap;gap:6px 12px;align-items:center;justify-content:center;padding:6px 16px;background:rgba(20,22,26,.96);color:#e8e8e8;font:13px/1.4 system-ui,sans-serif;border-bottom:1px solid rgba(255,255,255,.12)'
      document.documentElement.appendChild(shown)
    }
    const said = document.createElement('span')
    said.textContent = words
    const parts: HTMLElement[] = [said]
    if (action !== undefined) {
      const button = document.createElement('button')
      button.type = 'button'
      button.textContent = action.label
      button.style.cssText = 'padding:3px 10px;border-radius:8px;border:0;background:#e8e8e8;color:#0b0d0e;font:600 13px system-ui,sans-serif;cursor:pointer'
      button.addEventListener('click', action.run)
      parts.push(button)
    }
    shown.replaceChildren(...parts)
  }
}

/** Said while the relay cannot be reached at all from this browser, before the window ever opened. */
const RELAY_OUT_OF_REACH = 'Locust’s relay cannot be reached from this browser. Check its connection; it keeps trying.'

/** Said of a change asked for while the window shows what Locust last showed. */
const SNAPSHOT_SAYS = 'This is what Locust last showed here, so nothing can be changed from it yet. It opens live as soon as your computer answers.'

function createDeskLine() {
  const overlaid = overlay()
  const line = banner()
  const store = snapshotStore()
  let snapshot: DeskSnapshot | undefined
  /** Showing what Locust last showed, until the computer answers and the window opens live. */
  let showingSnapshot = false
  let saving: number | undefined
  let loaded: () => void = () => undefined
  const snapshotReady = new Promise<void>((resolve) => { loaded = resolve })
  const remember = (key: string, value: unknown): void => {
    snapshot = snapshotWith(snapshot, key, value, Date.now())
    if (saving !== undefined) return
    saving = window.setTimeout(() => { saving = undefined; const kept = snapshot; if (kept !== undefined) void store.save(kept).catch(() => undefined) }, 1_500)
  }
  /*
   * Over a window showing what Locust last showed, the line at the top says how things stand; the overlay is kept
   * for a window that is live, where it stops the person acting on a window that cannot answer.
   */
  const say = (words: string | undefined, action?: { readonly label: string; readonly run: () => void }): void => {
    if (!showingSnapshot) { overlaid(words, action); return }
    const when = snapshot === undefined ? '' : `, at ${snapshotWhen(snapshot.at, Date.now())}`
    line(words === undefined ? undefined : `This is what Locust last showed here${when}. ${words === 'Opening Locust…' ? 'Reaching your computer…' : words}`, action)
  }
  const listeners = new Map<string, Set<(event: IpcEvent, ...args: unknown[]) => void>>()
  const waiting = new Map<string, { readonly resolve: (value: unknown) => void; readonly reject: (error: Error) => void; readonly what: string; readonly since: number }>()
  const session = randomId()
  let sentFrames = 0
  let link: Link | undefined
  let socket: WebSocket | undefined
  let ready = false
  let pcOn = false
  /** Ours, theirs, or not yet asked: a browser takes the window as it opens, and again only when the person says. */
  let have: 'yes' | 'no' | 'asking' = 'asking'
  let outbox: DeskAsk[] = []
  let flushing: number | undefined
  let replayWanted = false
  let retry = 0
  const joiner = deskJoiner()
  // What was last taken from the computer: a frame the relay sends again, or an old one, is dropped.
  let last: { s: string; q: number; t: number } | undefined

  /** Calls made before the window is this browser's -- the window's first reads, as it opens -- go once it is. */
  let held: DeskAsk[] = []
  const fail = (message: string): void => {
    held = []
    for (const [id, wait] of waiting) { waiting.delete(id); wait.reject(new Error(message)) }
  }
  const schedule = (): void => { flushing ??= window.setTimeout(() => { void flush() }, 12) }
  const flush = async (): Promise<void> => {
    flushing = undefined
    const current = link
    if (current === undefined || socket === undefined || !ready || outbox.length === 0) return
    const items = outbox
    outbox = []
    sentFrames += 1
    const text = JSON.stringify({ v: 1, kind: 'desk', s: session, q: sentFrames, t: Date.now(), items })
    const boxes: readonly unknown[] = text.length <= DESK_PART_CHARS ? [JSON.parse(text) as unknown] : deskParts(`${session}-${String(sentFrames)}`, text)
    for (const [index, box] of boxes.entries()) {
      // A long frame's parts go a little apart: the relay counts what each connection sends.
      if (index > 0) await new Promise((done) => setTimeout(done, 180))
      const sealed = await sealFor(current, box)
      if (socket === undefined || link !== current) return
      socket.send(JSON.stringify({ t: 'send', to: 'pc', ...sealed }))
    }
  }
  const ask = (item: DeskAsk): Promise<unknown> => new Promise((resolve, reject) => {
    const opening = item.kind === 'hello' || item.kind === 'take'
    const what = item.kind === 'call' ? item.channel : item.kind
    if (opening ? ready && pcOn : have === 'yes' && ready && pcOn) {
      waiting.set(item.id, { resolve, reject, what, since: Date.now() })
      outbox.push(item)
      schedule()
      return
    }
    if (opening || have === 'no') { reject(new Error(have === 'no' ? 'Locust is open somewhere else now.' : 'Reaching Locust…')); return }
    waiting.set(item.id, { resolve, reject, what, since: Date.now() })
    held.push(item)
    // Not for ever: a computer that never answers says so.
    window.setTimeout(() => {
      if (!held.includes(item)) return
      held = held.filter((one) => one !== item)
      waiting.delete(item.id)
      reject(new Error('Locust on your computer did not answer.'))
    }, 45_000)
  })

  const take = (): void => {
    have = 'asking'
    say('Opening Locust…')
    void ask({ kind: 'take', id: randomId() }).then(() => {
      have = 'yes'
      // From what it last showed to live: the window opens again, now from the computer.
      if (showingSnapshot) {
        try { sessionStorage.setItem('locust-desk-live', '1') } catch { /* The reload is live either way. */ }
        location.reload()
        return
      }
      say(undefined)
      outbox.push(...held)
      held = []
      schedule()
      if (replayWanted) void ask({ kind: 'replay', id: randomId(), what: 'approvals' }).catch(() => undefined)
    }, (error: unknown) => {
      have = 'no'
      say(error instanceof Error ? error.message : 'Locust could not be opened here.', { label: 'Try again', run: take })
    })
  }

  const heard = (item: DeskTell): void => {
    if (item.kind === 'res') {
      const wait = waiting.get(item.id)
      if (wait === undefined) return
      waiting.delete(item.id)
      if (item.ok) {
        try { wait.resolve(deskDecode(item.value)) } catch { wait.reject(new Error('Locust’s answer could not be read. Try again.')) }
      } else wait.reject(new Error(item.message))
      return
    }
    if (item.kind === 'ev') {
      let args: unknown
      try { args = deskDecode(item.args) } catch { return }
      if (!Array.isArray(args)) return
      for (const listener of [...(listeners.get(item.channel) ?? [])]) {
        try { listener(undefined, ...args) } catch { /* One listener's mistake is its own. */ }
      }
      return
    }
    if (item.kind === 'taken') {
      have = 'no'
      fail('Locust is open somewhere else now.')
      say(item.by === 'computer'
        ? 'Locust is in use on your computer now. Use it here again, and your computer’s window shows that it is open in a browser.'
        : 'Locust is open in another browser now.', { label: 'Use Locust here', run: take })
    }
  }

  const frame = async (n: string, c: string): Promise<void> => {
    const current = link
    if (current === undefined) return
    let said: unknown
    try { said = await openFrom(current, n, c) } catch { return }
    if (typeof said !== 'object' || said === null) return
    let box = said as Record<string, unknown>
    if (box.v === 1 && box.kind === 'desk-part') {
      const whole = joiner(box as never)
      if (whole === undefined) return
      try { box = JSON.parse(whole) as Record<string, unknown> } catch { return }
    }
    if (box.v !== 1 || box.kind !== 'desk' || typeof box.s !== 'string' || typeof box.q !== 'number' || typeof box.t !== 'number' || !Array.isArray(box.items)) return
    const items = box.items as DeskTell[]
    if (last === undefined) {
      // The first frame taken is one that answers a question this page asked.
      if (!items.some((item) => item.kind === 'res' && waiting.has(item.id))) return
    } else if (box.s === last.s ? box.q <= last.q : box.t <= last.t) return
    last = { s: box.s, q: box.q, t: box.t }
    for (const item of items) heard(item)
  }

  const connect = (): void => {
    const current = link
    if (current === undefined || socket !== undefined) return
    const ws = new WebSocket(`${location.protocol === 'https:' ? 'wss://' : 'ws://'}${location.host}/v1/room/${current.room}`)
    socket = ws
    ws.onopen = () => { ws.send(JSON.stringify({ t: 'auth', as: 'device', device: current.device, token: current.token })) }
    ws.onmessage = (event) => {
      let relay: Record<string, unknown>
      try { relay = JSON.parse(String(event.data)) as Record<string, unknown> } catch { return }
      if (relay.t === 'ok') {
        ready = true
        retry = 0
        pcOn = relay.pc === true
        if (!pcOn) { say('Your computer is not connected. Check that it is on and awake, and that Locust is open there.'); return }
        if (have !== 'no') take()
        return
      }
      if (relay.t === 'presence' && typeof relay.pc === 'boolean') {
        pcOn = relay.pc
        if (!pcOn) { fail('Your computer is not connected.'); say('Your computer is not connected. Check that it is on and awake, and that Locust is open there.') }
        else if (have !== 'no') take()
        return
      }
      if (relay.t === 'removed' || (relay.t === 'err' && (relay.code === 'auth' || relay.code === 'room'))) {
        link = undefined
        // A browser that is no longer paired keeps nothing of what it was shown.
        snapshot = undefined
        void store.clear().catch(() => undefined)
        if (showingSnapshot) { showingSnapshot = false; line(undefined) }
        fail('This browser is no longer paired.')
        say('This browser is no longer paired with Locust. Pair it again from Settings › General › Your phone on your computer.', { label: 'Pair this browser', run: () => { location.href = '/' } })
        return
      }
      if (relay.t === 'err' && relay.code === 'pc-offline') { pcOn = false; fail('Your computer is not connected.'); return }
      if (relay.t === 'msg' && relay.from === 'pc' && typeof relay.n === 'string' && typeof relay.c === 'string') void frame(relay.n, relay.c)
    }
    ws.onclose = () => {
      if (socket !== ws) return
      socket = undefined
      ready = false
      fail('Locust’s relay could not be reached. Reaching it again…')
      if (link === undefined) return
      if (have === 'yes') say('Reaching Locust again…')
      // Never reached: said, rather than "Opening" for as long as it keeps trying (0.834).
      else say(RELAY_OUT_OF_REACH)
      retry = Math.min(retry + 1, 6)
      window.setTimeout(connect, 1000 * 2 ** (retry - 1))
    }
  }

  const ipc: DesktopIpc = {
    invoke: async (channel, ...args) => {
      const access = deskAccess(channel, args)
      if (access === undefined) throw new Error(WINDOW_ONLY_SAYS)
      await snapshotReady
      const key = snapshotKey(channel, args)
      if (showingSnapshot) {
        const kept = snapshot?.answers[key]
        if (access === 'read' && kept !== undefined) return structuredClone(kept.value)
        // A read it never kept waits for the live window, which takes this one's place.
        if (access === 'read') return new Promise(() => undefined)
        throw new Error(SNAPSHOT_SAYS)
      }
      const value = await ask({ kind: 'call', id: randomId(), channel, args: deskEncode(args) })
      if (access === 'read') remember(key, value)
      return value
    },
    // The window's one-way messages are its own badge and controls: nothing to send from a browser.
    send: () => undefined,
    on: (channel, listener) => {
      const held = listeners.get(channel) ?? new Set()
      held.add(listener as (event: IpcEvent, ...args: unknown[]) => void)
      listeners.set(channel, held)
      // The cards already waiting, once the window is listening for them.
      if (channel === MISSION_APPROVAL_CHANNEL && !replayWanted) {
        replayWanted = true
        if (have === 'yes') void ask({ kind: 'replay', id: randomId(), what: 'approvals' }).catch(() => undefined)
      }
      return ipc
    },
    removeListener: (channel, listener) => {
      listeners.get(channel)?.delete(listener as (event: IpcEvent, ...args: unknown[]) => void)
      return ipc
    }
  }

  return {
    ipc,
    /** What is still waiting on the computer, and for how long: for a person reporting a problem, and for drives. */
    pending: (): readonly { readonly what: string; readonly ms: number }[] => [...waiting.values()].map((wait) => ({ what: wait.what, ms: Date.now() - wait.since })),
    state: (): string => `${have}${ready ? '' : ', not connected'}${pcOn ? '' : ', computer off'}`,
    start: async (): Promise<void> => {
      link = await savedLink()
      // What it last showed, shown at once -- unless this tab already opened live, and was reloaded.
      let live = false
      // Once: a later reload of this tab, with the computer off, shows what it last showed again.
      try { live = sessionStorage.getItem('locust-desk-live') === '1'; sessionStorage.removeItem('locust-desk-live') } catch { /* Not live, then. */ }
      snapshot = link === undefined ? undefined : await store.load().catch(() => undefined)
      showingSnapshot = !live && snapshot !== undefined && Object.keys(snapshot.answers).length > 0
      loaded()
      if (showingSnapshot) say('Opening Locust…')
      if (link === undefined) {
        say('This browser is not paired with Locust yet. Pair it from Settings › General › Your phone on your computer, then open this page again.', { label: 'Pair this browser', run: () => { location.href = '/' } })
        return
      }
      connect()
    },
    hello: (): Promise<DeskHello> => ask({ kind: 'hello', id: randomId() }) as Promise<DeskHello>
  }
}

const line = createDeskLine()
const kept = ((): { computer?: string; theme?: 'dark' | 'light' } => {
  try { return JSON.parse(sessionStorage.getItem('locust-desk') ?? '{}') as { computer?: string; theme?: 'dark' | 'light' } } catch { return {} }
})()
const api: DesktopApi = desktopApiOver(line.ipc, { platform: browserPlatform(), initialTheme: kept.theme ?? 'dark' })
const desk: DesktopApi = {
  ...api,
  remote: { computer: kept.computer ?? 'your computer' },
  // A link opens in this browser, not on the computer.
  openLink: async (url: string) => {
    if (!/^https?:\/\//i.test(url)) return { ok: false, message: 'Only web links open from Locust in a browser.' }
    window.open(url, '_blank', 'noopener,noreferrer')
    return { ok: true }
  },
  minimize: () => undefined,
  toggleMaximize: () => undefined,
  close: () => undefined
}
Object.defineProperty(window, 'desktop', { value: desk, configurable: false, writable: false })
Object.defineProperty(window, 'locustDesk', { value: Object.freeze({ pending: line.pending, state: line.state }), configurable: false, writable: false })
void line.start()
