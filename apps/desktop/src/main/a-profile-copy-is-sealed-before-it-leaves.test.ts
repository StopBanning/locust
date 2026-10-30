import { describe, expect, it } from 'vitest'

import { CHUNK_BYTES, SEAL_OVERHEAD, chunkIdOf, chunksOf, fileIdOf, manifestIdOf, newProfileKey, openFile, profileKeyFromCode, recoveryCode, sealFile, syncKeys } from './profile-sync-crypto.js'

/**
 * A PROFILE COPY IS SEALED BEFORE IT LEAVES (docs/PLAN-2026-10-30-ACCOUNTS.md). What the relay is given of a
 * person's Locust: boxes it cannot open, under ids it cannot read back to names. A box opens only with the same profile
 * key, for the same account, under the same id; ids are the same on every computer of the account and say nothing of
 * the name; a file goes in fixed pieces, so an appended conversation changes only its last; and the recovery code
 * gives the key back, and refuses a code copied wrong.
 */
const text = (value: string): Uint8Array => new TextEncoder().encode(value)

describe('a sealed copy', () => {
  it('opens with the same key, account and id, and holds what was sealed', async () => {
    const keys = await syncKeys(newProfileKey(), 7)
    const id = await fileIdOf(keys, 'teammates.json')
    const box = await sealFile(keys, id, text('{"teammates":[]}'))
    expect(new TextDecoder().decode(await openFile(keys, id, box))).toBe('{"teammates":[]}')
    expect(box.length).toBe(text('{"teammates":[]}').length + SEAL_OVERHEAD)
  })

  it('holds none of the words it was given', async () => {
    const keys = await syncKeys(newProfileKey(), 7)
    const secret = 'Colin asked Codex to refactor the billing page'
    const box = await sealFile(keys, await fileIdOf(keys, 'x'), text(secret.repeat(4)))
    expect(Buffer.from(box).toString('latin1')).not.toContain('billing')
    // Sealed twice, the same words make two different boxes: a fresh nonce each time.
    const again = await sealFile(keys, await fileIdOf(keys, 'x'), text(secret.repeat(4)))
    expect(Buffer.from(again).equals(Buffer.from(box))).toBe(false)
  })

  it('does not open with another key, for another account, or under another id', async () => {
    const profile = newProfileKey()
    const keys = await syncKeys(profile, 7)
    const id = await fileIdOf(keys, 'memories.json')
    const box = await sealFile(keys, id, text('remember this'))
    await expect(openFile(await syncKeys(newProfileKey(), 7), id, box)).rejects.toThrow()
    await expect(openFile(await syncKeys(profile, 8), id, box)).rejects.toThrow()
    await expect(openFile(keys, await fileIdOf(keys, 'routines.json'), box)).rejects.toThrow()
    // Nor once a byte of it has changed.
    const changed = Uint8Array.from(box)
    changed[changed.length - 1] ^= 1
    await expect(openFile(keys, id, changed)).rejects.toThrow()
  })
})

describe('a file’s id', () => {
  it('is the same on every computer of the account, and differs by name and by key', async () => {
    const profile = newProfileKey()
    const pc = await syncKeys(profile, 7)
    const mac = await syncKeys(profile, 7)
    expect(await chunkIdOf(pc, 'mission-ledger/mission_a.jsonl', 0)).toBe(await chunkIdOf(mac, 'mission-ledger/mission_a.jsonl', 0))
    expect(await chunkIdOf(pc, 'mission-ledger/mission_a.jsonl', 0)).not.toBe(await chunkIdOf(pc, 'mission-ledger/mission_a.jsonl', 1))
    expect(await manifestIdOf(pc)).not.toBe(await manifestIdOf(await syncKeys(newProfileKey(), 7)))
  })

  it('is a relay file id that says nothing of the name', async () => {
    const keys = await syncKeys(newProfileKey(), 7)
    const id = await chunkIdOf(keys, 'mission-ledger/mission_secret.jsonl', 0)
    expect(id).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(id).not.toContain('mission')
  })
})

describe('the pieces of a file', () => {
  it('fall at fixed offsets, so an appended file changes only its last piece', () => {
    expect(chunksOf(0)).toEqual([[0, 0]])
    expect(chunksOf(10)).toEqual([[0, 10]])
    expect(chunksOf(CHUNK_BYTES)).toEqual([[0, CHUNK_BYTES]])
    const before = chunksOf(CHUNK_BYTES * 2 + 5)
    const after = chunksOf(CHUNK_BYTES * 2 + 900)
    expect(after.slice(0, 2)).toEqual(before.slice(0, 2))
    expect(after[2]).toEqual([CHUNK_BYTES * 2, CHUNK_BYTES * 2 + 900])
  })
})

describe('the recovery code', () => {
  it('gives back the profile key, however it is typed', async () => {
    const key = newProfileKey()
    const code = await recoveryCode(key)
    expect(code).toMatch(/^([0-9A-HJKMNP-TV-Z]{5}-){10}[0-9A-HJKMNP-TV-Z]{5}$/)
    expect(await profileKeyFromCode(code)).toEqual(key)
    expect(await profileKeyFromCode(code.toLowerCase().replaceAll('-', ' '))).toEqual(key)
  })

  it('refuses a code copied wrong', async () => {
    const code = await recoveryCode(newProfileKey())
    const swapped = (code[0] === '0' ? '1' : '0') + code.slice(1)
    expect(await profileKeyFromCode(swapped)).toBeUndefined()
    expect(await profileKeyFromCode(code.slice(0, -1))).toBeUndefined()
    expect(await profileKeyFromCode('')).toBeUndefined()
  })
})
