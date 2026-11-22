import { useSyncExternalStore } from 'react'

/**
 * WHETHER THE MODEL CHIP NAMES THE ACCOUNT (0.882). "Opus 5.5 · freev2 · Medium" put a third thing on the chip for
 * every message; Colin, 22 Nov: "have account name as a togglable setting. Otherwise it's just bloat... have it in a
 * place where people would find it if they were adding another account." Off unless chosen; the switch is in each
 * agent's accounts list (Settings > AI agents). The chip's hover and the model menu still name the account.
 */
let shown = false
const listeners = new Set<() => void>()

export function setAccountOnChip(on: boolean): void {
  if (on === shown) return
  shown = on
  for (const listener of listeners) listener()
}

export function accountOnChip(): boolean {
  return shown
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}

export function useAccountOnChip(): boolean {
  return useSyncExternalStore(subscribe, accountOnChip, accountOnChip)
}

/** Chosen in Settings: shown at once, put back if the disk did not take it. */
export async function chooseAccountOnChip(next: boolean): Promise<void> {
  const before = shown
  setAccountOnChip(next)
  try {
    const settings = await window.desktop?.changeWorkspaceSettings({ accountOnChip: next })
    if (settings !== undefined) setAccountOnChip(settings.accountOnChip === true)
  } catch {
    setAccountOnChip(before)
  }
}
