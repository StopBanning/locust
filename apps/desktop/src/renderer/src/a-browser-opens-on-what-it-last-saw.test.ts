import { describe, expect, it } from 'vitest'

import { openSnapshot, sealSnapshot, SNAPSHOT_ENTRY_CHARS, SNAPSHOT_TOTAL_CHARS, snapshotKey, snapshotWhen, snapshotWith } from './desk-snapshot.js'

/**
 * A BROWSER OPENS ON WHAT IT LAST SAW (0.834, desk-snapshot.ts). What the window read from the computer is kept, sealed,
 * and shown at once the next time: each read by its call and its arguments, the newest kept when there is too much,
 * one too long not kept at all, and never readable without the browser's own key.
 */
const big = (chars: number): string => 'x'.repeat(chars)

describe('what a browser keeps of the window', () => {
  it('keeps each read by its call and its arguments', () => {
    expect(snapshotKey('mission:history', [])).not.toBe(snapshotKey('mission:get', ['m1']))
    expect(snapshotKey('mission:get', ['m1'])).not.toBe(snapshotKey('mission:get', ['m2']))
    const kept = snapshotWith(snapshotWith(undefined, snapshotKey('mission:get', ['m1']), { turns: 3 }, 1_000), snapshotKey('mission:get', ['m1']), { turns: 4 }, 2_000)
    expect(kept.answers[snapshotKey('mission:get', ['m1'])]).toEqual({ value: { turns: 4 }, at: 2_000 })
    expect(kept.at).toBe(2_000)
  })

  it('keeps the newest when there is too much, and does not keep one too long', () => {
    const third = Math.floor(SNAPSHOT_TOTAL_CHARS / 3)
    const step = Math.min(third, SNAPSHOT_ENTRY_CHARS - 100)
    let kept = snapshotWith(undefined, 'a', big(step), 1)
    for (let index = 0; index < 12; index += 1) kept = snapshotWith(kept, `k${String(index)}`, big(step), 10 + index)
    expect(kept.answers.a).toBeUndefined()
    expect(kept.answers.k11).toBeDefined()
    expect(JSON.stringify(kept.answers).length).toBeLessThanOrEqual(SNAPSHOT_TOTAL_CHARS + 2_000)
    const before = snapshotWith(undefined, 'huge', 'small', 1)
    expect(snapshotWith(before, 'huge', big(SNAPSHOT_ENTRY_CHARS + 1), 2).answers.huge).toBeUndefined()
  })

  it('is sealed: opened with its own key, and with no other', async () => {
    const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt'])
    const other = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt'])
    const snapshot = snapshotWith(undefined, snapshotKey('teammates:list', []), [{ name: 'Ada' }], 5)
    const sealed = await sealSnapshot(key, snapshot)
    expect(new TextDecoder().decode(sealed.box)).not.toContain('Ada')
    expect(await openSnapshot(key, sealed)).toEqual(snapshot)
    expect(await openSnapshot(other, sealed)).toBeUndefined()
  })

  it('says when it is from, with the day when it was not today', () => {
    const now = new Date(2026, 9, 24, 15, 0).getTime()
    expect(snapshotWhen(new Date(2026, 9, 24, 10, 32).getTime(), now)).not.toContain(' on ')
    expect(snapshotWhen(new Date(2026, 9, 22, 10, 32).getTime(), now)).toContain(' on ')
  })
})
