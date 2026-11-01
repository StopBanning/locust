import { useRef } from 'react'
import type { ReactElement } from 'react'

import { keysOn, shortcutGroups } from '../keyboardShortcuts.js'
import { useModal } from '../useModal.js'
import { Icon } from './Icon.js'

/** A Mac's own marks for its keys, as its menus draw them; arrows read the same everywhere. */
const MAC_MARKS: Readonly<Record<string, string>> = { Cmd: '⌘', Option: '⌥', Shift: '⇧', Ctrl: '⌃', Enter: '↩', Escape: 'esc', Tab: '⇥' }
const ARROWS: Readonly<Record<string, string>> = { Up: '↑', Down: '↓' }

/**
 * A SHORTCUT AS KEYCAPS (polish pass, 2026-11-01): "Ctrl+Shift+M" is three keys and "Alt+Up / Alt+Down" two chords,
 * each key on its own cap. A Mac draws its modifiers in its own marks (⌘ ⌥ ⇧ ⌃), as its menus do; a PC keeps the words.
 */
function Keys({ keys, mac }: { readonly keys: string; readonly mac: boolean }): ReactElement {
  return (
    <span className="lc-shortcuts__keys">
      {keys.split(' / ').map((chord, at) => (
        <span className="lc-shortcuts__chord" key={chord}>
          {at > 0 && <span className="lc-shortcuts__or">or</span>}
          {(chord.length === 1 ? [chord] : chord.split('+')).map((key, index) => (
            <kbd key={`${key}${String(index)}`}>{ARROWS[key] ?? (mac ? MAC_MARKS[key] ?? key : key)}</kbd>
          ))}
        </span>
      ))}
    </span>
  )
}

/** Every shortcut, grouped. Ctrl+/ opens it; Escape closes it. */
export function KeyboardSheet({
  onClose,
  platform = typeof window === 'undefined' ? undefined : window.desktop?.platform
}: {
  readonly onClose: () => void
  /** Whose keys to name: a Mac says Cmd and Option (keysOn). */
  readonly platform?: string
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-shortcuts" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Keyboard shortcuts</span>
          <button type="button" className="lc-dialog__close" aria-label="Close shortcuts" onClick={onClose}><Icon name="close" size={13} /></button>
        </div>
        <div className="lc-dialog__body lc-shortcuts__body">
          {shortcutGroups().map((group) => (
            <section key={group.name} className="lc-shortcuts__group">
              <h2>{group.name}</h2>
              <ul>
                {group.rows.map((row) => (
                  <li key={`${group.name}:${row.keys}:${row.does}`}>
                    <span>{row.does}</span>
                    <Keys keys={keysOn(row.keys, platform, row.controlOnMac === true)} mac={platform === 'darwin'} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </div>
    </div>
  )
}
