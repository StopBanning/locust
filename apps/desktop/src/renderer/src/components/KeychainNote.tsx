import type { ReactElement } from 'react'

import { Icon } from './Icon.js'

/**
 * MACOS MAY BE ASKING (0.854, main/keychain.ts). After an update a Mac asks for its password before Locust may open
 * the keys it keeps, and Locust waits for the answer: said here first, since the box can open behind this window and
 * the window does not move until it is answered. The name is the one macOS's box shows.
 */
export const KEYCHAIN_NOTE =
  'macOS may ask for your Mac’s password so Locust can open the keys it keeps (“@teammate/desktop Safe Storage”). Type it and choose Always Allow. If you don’t see the box, it may be behind this window.'

export function KeychainNote({ asking }: { readonly asking: boolean }): ReactElement | null {
  if (!asking) return null
  return (
    <div className="lc-diagnostic lc-tone-amber" role="alert" data-keychain-note="">
      <Icon name="shield" size={12} />
      <span>{KEYCHAIN_NOTE}</span>
    </div>
  )
}
