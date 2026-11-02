import type { SecretBox } from './own-models.js'

/**
 * THE KEYCHAIN IS ASKED ONLY FOR A KEY, AND THE WINDOW SAYS SO FIRST (0.854).
 *
 * Every Mac build is signed with an identity of its own, so after an update the first read of the keys Locust keeps
 * makes macOS ask for the person's password ("@teammate/desktop Safe Storage"). Electron's read waits for that answer
 * on the main thread: the window froze, often with the box behind it, and nothing said why (the first Mac run, F12).
 *
 * - Nothing asks the keychain without a key to open. A store checks what it keeps first (routine-hooks.ts,
 *   teammate-keys.ts), so someone who keeps no keys is never asked. Asking whether a key CAN be locked reads the
 *   keychain on a Mac too: a routine-hooks sync asked it on every launch, and created the item on a fresh one.
 * - On a Mac's first launch of a build, when a key is kept, the window says it first: once the window is showing and
 *   listening, the note goes up, then the keychain is opened once, then the note comes down. Electron keeps what it
 *   read for the rest of the run, so nothing after that waits on macOS. A launch of a build that opened it before
 *   (Always Allow, given to this build) says nothing.
 */

/** Electron's safeStorage, as far as Locust uses it. */
export interface KeychainBackend {
  isEncryptionAvailable(): boolean
  encryptString(text: string): Buffer
  decryptString(cipher: Buffer): string
}

export interface Keychain {
  /** What every store is given: the same three calls, straight through. */
  readonly secrets: SecretBox
  /** Whether the window should be saying, now, that macOS may ask. */
  readonly asking: boolean
  /** Start the launch's step: once every store that keeps keys exists. Again is the same step. */
  begin(): Promise<void>
  /** Resolves once the launch's step is over, whether or not anything was asked. For readers that open keys early. */
  settled(): Promise<void>
}

export function createKeychain(options: {
  readonly backend: KeychainBackend
  /** A Mac reading its own keychain. Elsewhere, or with a mock keychain, nothing asks, and nothing is said. */
  readonly mayAsk: boolean
  /** This launch's build, and the one the keychain was last opened by here. */
  readonly version: string
  readonly openedBy: () => Promise<string | undefined>
  readonly rememberOpened: (version: string) => Promise<void>
  /** Whether any store keeps a key. */
  readonly keepsKey: () => Promise<boolean>
  /** Resolves once the window is showing and can draw the note. */
  readonly windowListening: () => Promise<void>
  /** Tells the window whether macOS may be asking now. */
  readonly say: (asking: boolean) => void
  /** Time for the note to be drawn before reading the keychain may freeze the window. */
  readonly drawMs?: number
  /** How long a window that never listens is waited for. The keys are opened then anyway: the readers need them. */
  readonly listenMs?: number
  readonly wait?: (ms: number) => Promise<void>
}): Keychain {
  const wait = options.wait ?? ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)))
  const { backend } = options
  let opened = false
  let asking = false
  let step: Promise<void> | undefined
  let over: () => void = () => undefined
  const done = new Promise<void>((resolve) => { over = resolve })
  const touched = <T>(read: () => T): T => {
    opened = true
    return read()
  }
  const run = async (): Promise<void> => {
    if (!options.mayAsk || opened) return
    if ((await options.openedBy().catch(() => undefined)) === options.version) return
    // Unknown counts as kept: a note said for nothing is better than a window frozen without one.
    if (!(await options.keepsKey().catch(() => true))) return
    await Promise.race([options.windowListening(), wait(options.listenMs ?? 20_000)])
    if (opened) return
    let available = false
    asking = true
    options.say(true)
    try {
      await wait(options.drawMs ?? 400)
      available = touched(() => backend.isEncryptionAvailable())
    } catch {
      available = false
    } finally {
      asking = false
      options.say(false)
    }
    // Refused, or not there: not remembered, so the next launch says it again.
    if (available) await options.rememberOpened(options.version).catch(() => undefined)
  }
  return {
    secrets: {
      available: () => touched(() => backend.isEncryptionAvailable()),
      encrypt: (text) => touched(() => backend.encryptString(text)),
      decrypt: (cipher) => touched(() => backend.decryptString(cipher))
    },
    get asking() { return asking },
    begin() {
      step ??= run().catch(() => undefined).finally(over)
      return step
    },
    settled: () => done
  }
}
