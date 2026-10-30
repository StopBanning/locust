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
