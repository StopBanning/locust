/**
 * 1-9 chooses the nth row of an open menu, counting from the top.
 *
 * A search box still types the digit. Shift, Ctrl, Alt and the command key
 * are left alone: those chords belong to the window, not to a row.
 */
export function menuRowFromDigit(
  key: string,
  modifiers: { readonly ctrl: boolean; readonly meta: boolean; readonly alt: boolean; readonly shift: boolean },
  typing: boolean
): number | undefined {
  if (typing || modifiers.ctrl || modifiers.meta || modifiers.alt || modifiers.shift) return undefined
  if (!/^[1-9]$/.test(key)) return undefined
  return Number(key) - 1
}

export function isTypingTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable)
}
