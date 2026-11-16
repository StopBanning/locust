import { useEffect, useSyncExternalStore } from 'react'
import { accountFromRoute, type AgentAccount, type AccountAction, type AgentAccountsState } from '../../shared/agent-accounts.js'
import { windowWords } from './machineWords.js'

let accounts: readonly AgentAccount[] = []
const listeners = new Set<() => void>()
let reading: Promise<void> | undefined
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } }
const snapshot = () => accounts
export function adoptAccounts(next: readonly AgentAccount[]) {
  accounts = next
  for (const listener of listeners) listener()
}
export async function accountAction(action: AccountAction): Promise<AgentAccountsState> {
  const answer = await window.desktop?.agentAccounts?.(action)
  if (answer === undefined) return { ok: false, accounts, message: 'The desktop bridge is unavailable. Your accounts are unchanged.' }
  adoptAccounts(answer.accounts)
  return answer
}
export function refreshAccounts() {
  return reading ??= accountAction({ kind: 'list' }).then(() => undefined).catch(() => undefined).finally(() => { reading = undefined })
}
export function useAgentAccounts() {
  const list = useSyncExternalStore(subscribe, snapshot, snapshot)
  useEffect(() => { void refreshAccounts() }, [])
  return list
}
export const accountName = (id: string | undefined) => id === undefined ? `This ${windowWords().computer}'s sign-in` : accounts.find(a => a.id === id)?.name ?? 'Removed account'
export const routeAccount = (route: { readonly account?: string; readonly resolvedRouteId?: string }) => route.account ?? accountFromRoute(route.resolvedRouteId ?? '')
/**
 * The account the model chip names (0.876.2). Colin, 16 Nov: "How do we see what account is logged into Locust? It's
 * really hard to swap accounts." The chip named an account only when one was picked; on the computer's own sign-in it
 * named none, so a person with a second account could not tell which one a conversation would use. 0.876.2 named both:
 * the added one, or "This Mac". Colin, the same day, of "Opus 5.5 · This Mac · Medium": "I don't know why this Mac has
 * to be here. Kinda ugly" (0.877.1). So the chip names an added account only: its name says you are on it, and no name
 * is the computer's own sign-in, the one there always is.
 */
export function chipAccountName(account: string | undefined, list: readonly AgentAccount[]): string | undefined {
  return account === undefined ? undefined : list.find(a => a.id === account)?.name ?? 'Removed account'
}
