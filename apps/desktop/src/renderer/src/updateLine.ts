import type { AppUpdateState } from '../../shared/ipc.js'

/**
 * What Settings says about a copy that does not update itself.
 *
 * It said "On a Mac this build cannot update itself yet. Download the newest
 * from the releases page and replace it." on any Mac, guessed from the user
 * agent: untrue since 0.516, when Macs began updating themselves, and the
 * first real Mac run (2026-10-30) found it on a development build and on every
 * copy a script drives. The installed Locust updates itself on every system;
 * this copy is held back, and a development build says that it never updates.
 */
export function heldBackLine(heldBack: AppUpdateState['heldBack']): string {
  const copy = "This copy of Locust doesn't update itself. The installed Locust does."
  return heldBack === 'development' ? `${copy} A development build never updates.` : copy
}
