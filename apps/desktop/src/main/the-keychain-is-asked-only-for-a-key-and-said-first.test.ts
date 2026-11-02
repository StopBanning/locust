import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { createKeychain, type KeychainBackend } from './keychain.js'
import { createRoutineHooks, ROUTINE_HOOKS_FILE } from './routine-hooks.js'
import { createRoutineStore } from './routine-store.js'
import { createTeammateKeys } from './teammate-keys.js'
import { createTelegramLink, TELEGRAM_FILE } from './telegram.js'
import { ACCOUNT_SYNC_FILE, createYourAccount } from './your-account.js'

/*
 * THE KEYCHAIN IS ASKED ONLY FOR A KEY, AND THE WINDOW SAYS SO FIRST (0.854).
 *
 * Every Mac build is signed apart, so after an update macOS asks for the person's password at the first read of the
 * keys Locust keeps -- and Electron waits on that answer with the window frozen (the first Mac run, F12). A routine-hooks
 * sync asked on every launch, keys or none, and every run start asked whether a teammate's keys could be opened.
 */

const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })
const folder = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), 'locust-keychain-'))
  roots.push(root)
  return root
}

/** A stand-in for the operating system's lock that counts what asked it. */
const counted = () => {
  const asked: string[] = []
  return {
    asked,
    box: {
      available: () => { asked.push('available'); return true },
      encrypt: (text: string) => { asked.push('encrypt'); return Buffer.from([...Buffer.from(text)].map((byte) => byte ^ 0x5a)) },
      decrypt: (cipher: Buffer) => { asked.push('decrypt'); return Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8') }
    }
  }
}

describe('nothing asks the keychain without a key to open', () => {
  it('a called-links sync with no link kept and no routine called by one asks nothing', async () => {
    const root = await folder()
    const lock = counted()
    const store = createRoutineStore({ rootDirectory: root })
    const hooks = createRoutineHooks({ file: join(root, ROUTINE_HOOKS_FILE), secrets: lock.box, relay: () => undefined, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined })
    await hooks.sync()
    expect(lock.asked).toEqual([])
    expect(await hooks.keepsKey()).toBe(false)
  })

  it('a routine called by a link still has its link kept, as before', async () => {
    const root = await folder()
    const lock = counted()
    const store = createRoutineStore({ rootDirectory: root })
    await store.create({ name: 'Event summary', teammateId: 'tm_a', route: { runtime: 'codex', model: 'account-default', mode: 'ask' }, steps: ['Summarize the event.'], learnedFrom: [], schedule: { kind: 'hook' }, workspaceId: 'ws_a' })
    const hooks = createRoutineHooks({ file: join(root, ROUTINE_HOOKS_FILE), secrets: lock.box, relay: () => undefined, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined })
    await hooks.sync()
    expect(lock.asked).toContain('available')
  })

  it('a kept-links file that cannot be read still counts as kept, so its error is still said', async () => {
    const root = await folder()
    const lock = counted()
    await writeFile(join(root, ROUTINE_HOOKS_FILE), '{')
    const store = createRoutineStore({ rootDirectory: root })
    const hooks = createRoutineHooks({ file: join(root, ROUTINE_HOOKS_FILE), secrets: lock.box, relay: () => undefined, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined })
    expect(await hooks.keepsKey()).toBe(true)
    await expect(hooks.sync()).rejects.toThrow('The kept links could not be read')
  })

  it("a run for a teammate with no keys asks nothing; one with a key opens it", async () => {
    const root = await folder()
    const lock = counted()
    const keys = createTeammateKeys({ directory: root, secrets: lock.box, protectFile: async () => undefined })
    expect(await keys.runKeys('tm_wren')).toEqual([])
    expect(await keys.keepsKey()).toBe(false)
    expect(lock.asked).toEqual([])
    expect((await keys.set('tm_wren', 'STRIPE_KEY', 'kept-for-wren-alone')).ok).toBe(true)
    expect(await keys.keepsKey()).toBe(true)
    lock.asked.length = 0
    expect(await keys.runKeys('tm_booty')).toEqual([])
    expect(lock.asked).toEqual([])
    expect(await keys.runKeys('tm_wren')).toEqual([{ name: 'STRIPE_KEY', value: 'kept-for-wren-alone' }])
    expect(lock.asked).toContain('decrypt')
  })

  it('a chat app and your account say whether they keep a key without opening it', async () => {
    const root = await folder()
    const lock = counted()
    const telegram = createTelegramLink({
      directory: root, secrets: lock.box, refusal: () => undefined, computer: () => 'DESK-PC', team: async () => [],
      start: async () => ({ ok: true, missionId: 'mission_1' }), read: async () => undefined, live: () => false, waiting: () => [], decide: async () => false,
      api: () => 'http://127.0.0.1:1', protectFile: async () => undefined
    })
    expect(await telegram.keepsKey()).toBe(false)
    await writeFile(join(root, TELEGRAM_FILE), JSON.stringify({ schemaVersion: 1, token: 'c2VhbGVk', bot: { username: 'locust_bot' } }))
    expect(await telegram.keepsKey()).toBe(true)
    const account = createYourAccount({ profile: root, link: async () => undefined, secrets: lock.box, computer: () => 'Mac', appVersion: '0.854.0', busy: () => false, remote: {} as never })
    expect(await account.keepsKey()).toBe(false)
    await writeFile(join(root, ACCOUNT_SYNC_FILE), JSON.stringify({ v: 1, on: false, account: 7, key: 'c2VhbGVk' }))
    expect(await account.keepsKey()).toBe(true)
    expect(lock.asked).toEqual([])
  })
})

/** A Mac's keychain, with every step written down in order. */
function mac(input: { readonly openedBy?: string; readonly keepsKey?: boolean; readonly mayAsk?: boolean; readonly available?: boolean | 'throws'; readonly listens?: boolean } = {}) {
  const steps: string[] = []
  let listen: () => void = () => undefined
  const listening = new Promise<void>((resolve) => { listen = resolve })
  const backend: KeychainBackend = {
    isEncryptionAvailable: () => {
      steps.push('keychain')
      if (input.available === 'throws') throw new Error('User interaction is not allowed.')
      return input.available ?? true
    },
    encryptString: (text) => { steps.push('keychain'); return Buffer.from(text) },
    decryptString: (cipher) => { steps.push('keychain'); return cipher.toString() }
  }
  const keychain = createKeychain({
    backend,
    mayAsk: input.mayAsk ?? true,
    version: '0.854.0',
    openedBy: async () => input.openedBy,
    rememberOpened: async (version) => { steps.push(`remember ${version}`) },
    keepsKey: async () => input.keepsKey ?? true,
    windowListening: () => { steps.push('waits for the window'); return listening },
    say: (asking) => { steps.push(asking ? 'note up' : 'note down') },
    wait: async (ms) => {
      steps.push(`wait ${String(ms)}`)
      // The window that never listens: its wait is the one that ends.
      if (ms === 20_000 && input.listens === false) return
      if (ms === 20_000) await new Promise(() => undefined)
    }
  })
  if (input.listens !== false) listen()
  return { keychain, steps }
}

describe("a Mac's first launch of a build, with a key kept", () => {
  it('says it in the window first, then opens the keychain once, then takes the note down and remembers the build', async () => {
    const { keychain, steps } = mac()
    await keychain.begin()
    expect(steps).toEqual(['waits for the window', 'wait 20000', 'note up', 'wait 400', 'keychain', 'note down', 'remember 0.854.0'])
    expect(keychain.asking).toBe(false)
  })

  it('a build that opened it before, nothing kept, or not a Mac: nothing said and nothing read', async () => {
    for (const quiet of [mac({ openedBy: '0.854.0' }), mac({ keepsKey: false }), mac({ mayAsk: false })]) {
      await quiet.keychain.begin()
      expect(quiet.steps).toEqual([])
    }
  })

  it('refused, or failing: the note still comes down, and the build is not remembered, so the next launch says it again', async () => {
    for (const refused of [mac({ available: false }), mac({ available: 'throws' })]) {
      await refused.keychain.begin()
      expect(refused.steps).toEqual(['waits for the window', 'wait 20000', 'note up', 'wait 400', 'keychain', 'note down'])
    }
  })

  it('a window that never listens is not waited on forever: the keys are opened anyway, for the readers that need them', async () => {
    const { keychain, steps } = mac({ listens: false })
    await keychain.begin()
    expect(steps).toEqual(['waits for the window', 'wait 20000', 'note up', 'wait 400', 'keychain', 'note down', 'remember 0.854.0'])
  })

  it('a read before the step leaves nothing to say', async () => {
    const { keychain, steps } = mac()
    keychain.secrets.decrypt(Buffer.from('sealed'))
    await keychain.begin()
    expect(steps).toEqual(['keychain'])
  })

  it('is one step however often it is begun, and settled once it is over', async () => {
    const { keychain, steps } = mac()
    let settled = false
    void keychain.settled().then(() => { settled = true })
    await Promise.all([keychain.begin(), keychain.begin()])
    await Promise.resolve()
    expect(settled).toBe(true)
    expect(steps.filter((step) => step === 'keychain')).toHaveLength(1)
  })
})
