import { createContext, useContext, type ReactNode } from 'react'
import { policySettingWords, type OrganizationPolicyState } from '../../shared/organization-policy.js'

export const OrganizationPolicyContext = createContext<OrganizationPolicyState | undefined>(undefined)
export const useOrganizationPolicy = (): OrganizationPolicyState | undefined => useContext(OrganizationPolicyContext)

export function ManagedPolicyNote() {
  return <span className="lc-settings__note">Set by your organization</span>
}

export function OrganizationPolicyNotice() {
  const state = useOrganizationPolicy()
  return state?.error === undefined ? null : <p className="lc-settings__note lc-tone-amber" role="alert">{state.error}</p>
}

const NAMES: Readonly<Record<string, string>> = {
  allowedAgents: 'Allowed AI agents', allowedModels: 'Allowed models', highestMode: 'Highest permission mode',
  monthlyLimitUsd: 'Maximum monthly limit per teammate (USD)', otherApps: 'Access from other AI apps, the locust command and editors',
  phone: 'Phone pairing', cloud: 'Cloud computers and tasks', terminal: 'Terminal panel'
}

export function OrganizationPolicySettings({ heading }: { readonly heading?: ReactNode }) {
  const state = useOrganizationPolicy()
  if (state?.present !== true) return null
  const settings = Object.entries(state.policy).filter(([key]) => key !== 'version')
  return <section className="lc-settings__section">
    {heading ?? <h2 className="lc-settings__heading">Managed by your organization</h2>}
    <p className="lc-settings__note">{state.path}</p>
    {state.error !== undefined ? <p className="lc-settings__note lc-tone-amber" role="alert">{state.error}</p> : <>
      <ul>{settings.map(([key, value]) => <li key={key}><strong>{NAMES[key] ?? key}</strong>: {policySettingWords(key, value)}</li>)}</ul>
      {settings.length === 0 && <p className="lc-settings__note">This policy sets no restrictions.</p>}
      <p className="lc-settings__note">Your choices can be more restrictive. Policy changes take effect when Locust restarts.</p>
    </>}
  </section>
}
