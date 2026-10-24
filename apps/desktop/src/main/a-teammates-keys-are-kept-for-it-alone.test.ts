import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { keyNameRefusal, keyRequestsIn, keysSection, keyValueRefusal, scrubKeyValues, withoutKeyValues } from '../shared/teammate-keys.js'
import { createTeammateKeys, TEAMMATE_KEYS_FILE } from './teammate-keys.js'

/**
 * A TEAMMATE'S KEYS ARE KEPT FOR IT ALONE (0.833, teammate-keys.ts). Named as a variable is, never a name an agent
 * reads for its own sign-in; kept as the operating system encrypts them, so the file never holds one in the clear;
 * listed by name only; given back, decrypted, only for that teammate's runs; and gone when it is.
 */
const VALUE = 'locust-made-up-key-value-0042'
const box = { available: () => true, encrypt: (text: string) => Buffer.from([...Buffer.from(text, 'utf8')].map((byte) => byte ^ 0x5a)), decrypt: (cipher: Buffer) => Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8') }
const folders: string[] = []
afterEach(async () => { for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true }) })
const store = async () => {
  const directory = await mkdtemp(join(tmpdir(), 'locust-keys-'))
  folders.push(directory)
  return { directory, keys: createTeammateKeys({ directory, secrets: box, protectFile: async () => undefined }) }
}

describe('a teammate’s keys', () => {
  it('are named as a variable is, and never as one an agent signs in with', () => {
    expect(keyNameRefusal('STRIPE_KEY')).toBeUndefined()
    expect(keyNameRefusal('SLACK_WEBHOOK_URL')).toBeUndefined()
    for (const bad of ['stripe_key', '1KEY', 'A', 'MY-KEY', 'KEY NAME']) expect(keyNameRefusal(bad), bad).toContain('capitals, digits and underscores')
    // Given one of these, the agent would sign in with it instead of the person's plan, or start differently.
    for (const reserved of ['ANTHROPIC_API_KEY', 'OPENAI_API_KEY', 'GEMINI_API_KEY', 'GITHUB_TOKEN', 'GH_TOKEN', 'CODEX_HOME', 'PATH', 'LOCUST_TELEGRAM_API']) {
      expect(keyNameRefusal(reserved), reserved).toContain('an agent or this computer reads for itself')
    }
    expect(keyValueRefusal('')).toBe('Paste the key first.')
    expect(keyValueRefusal('two\nlines')).toContain('one line')
    expect(keyValueRefusal('x'.repeat(5_000))).toContain('longer than any key')
  })

  it('are kept encrypted, listed by name, given back only by teammate, and gone with it', async () => {
    const { directory, keys } = await store()
    expect((await keys.set('tm_mira', 'STRIPE_KEY', `  ${VALUE} `)).names).toEqual(['STRIPE_KEY'])
    expect(await keys.set('tm_mira', 'OPENAI_API_KEY', VALUE)).toMatchObject({ ok: false })
    const file = await readFile(join(directory, TEAMMATE_KEYS_FILE), 'utf8')
    expect(file).not.toContain(VALUE)
    expect(file).not.toContain(VALUE.slice(8))
    expect(await keys.names('tm_mira')).toEqual(['STRIPE_KEY'])
    expect(await keys.names('tm_ada')).toEqual([])
    expect(await keys.runKeys('tm_mira')).toEqual([{ name: 'STRIPE_KEY', value: VALUE }])
    expect(await keys.runKeys('tm_ada')).toEqual([])
    expect((await keys.remove('tm_mira', 'STRIPE_KEY')).names).toEqual([])
    await keys.set('tm_mira', 'STRIPE_KEY', VALUE)
    await keys.forget('tm_mira')
    expect(await keys.runKeys('tm_mira')).toEqual([])
  })

  it('are refused where the computer cannot encrypt them, and past twenty', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'locust-keys-'))
    folders.push(directory)
    const plain = createTeammateKeys({ directory, secrets: { ...box, available: () => false }, protectFile: async () => undefined })
    expect(await plain.set('tm_mira', 'STRIPE_KEY', VALUE)).toMatchObject({ ok: false, message: 'This computer cannot encrypt the key, so it was not kept.' })
    const { keys } = await store()
    for (let index = 0; index < 20; index += 1) expect((await keys.set('tm_mira', `KEY_${String(index)}`, VALUE)).ok).toBe(true)
    expect((await keys.set('tm_mira', 'ONE_MORE', VALUE)).ok).toBe(false)
  })

  it('are asked for by name in the answer, outside code, three at most', () => {
    const text = 'I need two keys.\n\n<locust-key name="STRIPE_KEY">To make test charges</locust-key>\n<locust-key name="SLACK_WEBHOOK_URL">\n  To post the summary\n</locust-key>\n\n```\n<locust-key name="IN_CODE">no</locust-key>\n```\n<locust-key name="STRIPE_KEY">twice</locust-key>'
    expect(keyRequestsIn(text)).toEqual([{ name: 'STRIPE_KEY', why: 'To make test charges' }, { name: 'SLACK_WEBHOOK_URL', why: 'To post the summary' }])
  })

  it('are named in the brief, never valued, with how to ask for another', () => {
    const brief = keysSection(['STRIPE_KEY'], true)
    expect(brief).toContain('STRIPE_KEY')
    expect(brief).toContain('$STRIPE_KEY')
    expect(brief).toContain('Never print, echo, log or write a key\'s value')
    expect(brief).toContain('<locust-key name="NAME">')
    expect(keysSection([], true)).not.toContain('You have these keys')
    expect(keysSection(['STRIPE_KEY'], false)).toContain('Locust does not give keys to this agent yet')
    expect(keysSection([], false)).toBe('')
  })

  it('have their values taken out of anything, longest first, deep in an event', () => {
    const keys = [{ name: 'SHORT', value: 'abc' }, { name: 'A', value: 'secret-value-1' }, { name: 'B', value: 'secret-value-12345' }]
    expect(withoutKeyValues('x secret-value-12345 y secret-value-1 abc', keys)).toBe('x [key B] y [key A] abc')
    const event = { type: 'tool.completed', payload: { output: ['secret-value-1'], evidence: { raw: { text: 'ran secret-value-12345' } }, count: 3 } }
    expect(scrubKeyValues(event, keys)).toEqual({ type: 'tool.completed', payload: { output: ['[key A]'], evidence: { raw: { text: 'ran [key B]' } }, count: 3 } })
    expect(scrubKeyValues(event, [])).toBe(event)
  })
})
