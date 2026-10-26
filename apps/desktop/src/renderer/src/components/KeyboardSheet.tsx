import { useRef } from 'react'
import type { ReactElement } from 'react'

import { shortcutGroups } from '../keyboardShortcuts.js'
import { useModal } from '../useModal.js'

/** Every shortcut, grouped. Ctrl+/ opens it; Escape closes it. */
export function KeyboardSheet({ onClose }: { readonly onClose: () => void }): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-shortcuts" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Keyboard shortcuts</span>
          <button type="button" className="lc-dialog__close" aria-label="Close shortcuts" onClick={onClose}>✕</button>
        </div>
        <div className="lc-dialog__body lc-shortcuts__body">
          {shortcutGroups().map((group) => (
            <section key={group.name} className="lc-shortcuts__group">
              <h2>{group.name}</h2>
              <ul>
                {group.rows.map((row) => (
                  <li key={`${group.name}:${row.keys}:${row.does}`}>
                    <span>{row.does}</span>
                    <kbd>{row.keys}</kbd>
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
