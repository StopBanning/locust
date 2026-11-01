import { useRef } from 'react'
import type { ReactElement } from 'react'

import { keysOn, shortcutGroups } from '../keyboardShortcuts.js'
import { useModal } from '../useModal.js'
import { Icon } from './Icon.js'
import { Keycaps } from './Keycaps.js'

/**
 * Every shortcut, grouped. Ctrl+/ opens it; Escape closes it.
 *
 * It opens holding focus itself, not on its close (polish pass, 2026-11-01): a sheet with nothing to type in has no
 * first field, and the X it fell to wore a heavy focus ring at the top right whenever the sheet was opened from the
 * keyboard -- on a Mac, a stray frame. Tab still walks to the X.
 */
export function KeyboardSheet({
  onClose,
  platform = typeof window === 'undefined' ? undefined : window.desktop?.platform
}: {
  readonly onClose: () => void
  /** Whose keys to name: a Mac says Cmd and Option (keysOn). */
  readonly platform?: string
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose, { focusSelf: true })
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-shortcuts" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" tabIndex={-1}>
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
                    <Keycaps keys={keysOn(row.keys, platform, row.controlOnMac === true)} mac={platform === 'darwin'} />
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
