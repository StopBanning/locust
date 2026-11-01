import type { ReactElement } from 'react'

/** A Mac's own marks for its keys, as its menus draw them; arrows read the same everywhere. */
const MAC_MARKS: Readonly<Record<string, string>> = { Cmd: '⌘', Option: '⌥', Shift: '⇧', Ctrl: '⌃', Enter: '↩', Escape: 'esc', Tab: '⇥' }
const ARROWS: Readonly<Record<string, string>> = { Up: '↑', Down: '↓' }

/** A hint that is a chord to press ("Ctrl 1", "Cmd+/"), not words about the action ("every agent and model"). */
export function isChord(hint: string): boolean {
  return /^(Ctrl|Cmd|Alt|Option|Shift)[ +]/.test(hint)
}

/**
 * A SHORTCUT AS KEYCAPS (polish pass, 2026-11-01): "Ctrl+Shift+M" is three keys and "Alt+Up / Alt+Down" two chords,
 * each key on its own cap -- the shortcuts sheet and the palette draw them alike. A Mac draws its modifiers in its own
 * marks (⌘ ⌥ ⇧ ⌃), as its menus do; a PC keeps the words. "Ctrl 1" (the palette's spelling) is two keys too.
 */
export function Keycaps({ keys, mac }: { readonly keys: string; readonly mac: boolean }): ReactElement {
  return (
    <span className="lc-keycaps">
      {keys.split(' / ').map((chord, at) => (
        <span className="lc-keycaps__chord" key={chord}>
          {at > 0 && <span className="lc-keycaps__or">or</span>}
          {(chord.length === 1 ? [chord] : chord.split(/[+ ]/)).map((key, index) => (
            <kbd key={`${key}${String(index)}`}>{ARROWS[key] ?? (mac ? MAC_MARKS[key] ?? key : key)}</kbd>
          ))}
        </span>
      ))}
    </span>
  )
}
