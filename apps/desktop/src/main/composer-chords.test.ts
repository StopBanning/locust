import { describe, expect, it } from 'vitest'

import { heldChord } from './composer-chords.js'

const down = { type: 'keyDown', control: true, meta: false, shift: false, alt: false, key: 'o', code: 'KeyO' }

describe('chords the page handles are held back from the default menu', () => {
  it('holds the model, mode and effort chords, and Normal or Verbose, and the shortcuts sheet', () => {
    expect(heldChord({ ...down, shift: true, key: 'I', code: 'KeyI' })).toEqual({ key: 'I', code: 'KeyI', shift: true })
    expect(heldChord({ ...down, shift: true, key: 'M', code: 'KeyM' })?.shift).toBe(true)
    expect(heldChord({ ...down, shift: true, key: 'E', code: 'KeyE' })?.shift).toBe(true)
    expect(heldChord(down)).toEqual({ key: 'o', code: 'KeyO', shift: false })
    expect(heldChord({ ...down, key: '/', code: 'Slash' })).toEqual({ key: '/', code: 'Slash', shift: false })
  })

  it('does not take close, reload, or a key the page does not use', () => {
    expect(heldChord({ ...down, key: 'w', code: 'KeyW' })).toBeUndefined()
    expect(heldChord({ ...down, key: 'r', code: 'KeyR' })).toBeUndefined()
    expect(heldChord({ ...down, control: false, alt: true, key: 'F4', code: 'F4' })).toBeUndefined()
    expect(heldChord({ ...down, shift: false, key: 'i', code: 'KeyI' })).toBeUndefined()
    expect(heldChord({ ...down, type: 'keyUp' })).toBeUndefined()
  })
})
