/**
 * Chords the page handles, which the window's default menu would otherwise take.
 *
 * Checked and not taken: Ctrl+W (close the window), Ctrl+R (reload in
 * development), Alt+F4 (Windows close). F12 is left for DevTools.
 *
 * Ctrl+Shift+I is Chromium's DevTools on the default menu. That chord opens
 * the model menu, so the accelerator is stopped and the key is sent to the
 * page. Ctrl+I without Shift stays the inspector. Ctrl+O is the default
 * menu's Open, and here it switches Normal and Verbose.
 */
export interface ChordInput {
  readonly type: string
  readonly control: boolean
  readonly meta: boolean
  readonly shift: boolean
  readonly alt: boolean
  readonly key: string
  readonly code: string
}

export interface HeldChord {
  readonly key: string
  readonly code: string
  readonly shift: boolean
}

export function heldChord(input: ChordInput): HeldChord | undefined {
  if (input.type !== 'keyDown' || input.alt) return undefined
  if (!input.control && !input.meta) return undefined
  const key = input.key.length === 1 ? input.key.toLowerCase() : input.key
  if (input.shift && (key === 'm' || key === 'i' || key === 'e')) {
    return { key: input.key, code: input.code, shift: true }
  }
  if (!input.shift && (key === 'o' || key === '/' || input.code === 'Slash')) {
    return { key: key === '/' || input.code === 'Slash' ? '/' : input.key, code: input.code, shift: false }
  }
  return undefined
}
