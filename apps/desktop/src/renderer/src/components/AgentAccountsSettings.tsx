import { useEffect, useState } from 'react'
import { supportsAccounts, accountUsageKey, type AccountAction, type AgentAccount } from '../../../shared/agent-accounts.js'
import { policyFeatureRefusal } from '../../../shared/organization-policy.js'
import { accountAction, refreshAccounts, useAgentAccounts } from '../agentAccounts.js'
import { windowWords } from '../machineWords.js'
import { useOrganizationPolicy, ManagedPolicyNote } from '../organizationPolicy.js'
import { Icon } from './Icon.js'
import { UsageMeters } from './UsageMeters.js'

/** What an added account's row says about its sign-in. */
function stateSentence(state: AgentAccount['state']): string {
  return state === 'waiting'
    ? 'Finish signing in in your browser. This waits for the agent.'
    : state === 'ready'
      ? 'Signed in.'
      : state === 'failed'
        ? 'Sign-in did not finish. Your other accounts are unchanged.'
        : 'Ready to sign in.'
}

/*
 * ONE ROW PER ACCOUNT (0.797). The machine's own sign-in first, as the account a teammate uses when none is picked,
 * then each added one: its name and where its sign-in stands on the left, what can be done on the right -- the way the
 * other rows of this page read. Adding one opens its name in place.
 */
export function AgentAccountsSettings({ runtime, usage }: { readonly runtime: string; readonly usage?: ReadonlyMap<string, string> }) {
  const accounts = useAgentAccounts()
  const policy = useOrganizationPolicy()
  const blocked = policyFeatureRefusal(policy, 'accounts') !== undefined
  const [adding, setAdding] = useState(false)
  const [name, setName] = useState('')
  const [editing, setEditing] = useState<string>()
  const [removing, setRemoving] = useState<AgentAccount>()
  const [message, setMessage] = useState<string>()
  const [busy, setBusy] = useState(false)
  const own = accounts.filter(a => a.runtime === runtime)
  const waiting = own.some(a => a.state === 'waiting')
  useEffect(() => {
    if (!waiting) return
    const timer = setInterval(() => { void refreshAccounts() }, 1_000)
    return () => clearInterval(timer)
  }, [waiting])
  if (!supportsAccounts(runtime)) return null
  const act = async (action: AccountAction) => {
    setBusy(true); setMessage(undefined)
    try { const answer = await accountAction(action); if (!answer.ok) setMessage(answer.message); return answer.ok }
    catch { setMessage('The account could not be changed. Your other accounts are unchanged.'); return false }
    finally { setBusy(false) }
  }
  return <div className="lc-agentaccounts" data-runtime={runtime} aria-label={`${runtime} accounts`}>
    <div className="lc-agentaccounts__account">
      <span className="lc-agentaccounts__who">
        <span className="lc-agentaccounts__name">{`This ${windowWords().computer}'s sign-in`}</span>
        <span className="lc-agentaccounts__state">Used when a teammate has no account picked.</span>
      </span>
    </div>
    {own.map(a => <div className="lc-agentaccounts__account" key={a.id}>
      {editing === a.id ? <form className="lc-agentaccounts__form" onSubmit={event => { event.preventDefault(); void act({ kind: 'rename', id: a.id, name }).then(ok => { if (ok) setEditing(undefined) }) }}>
        <input className="lc-input" aria-label="Account name" value={name} maxLength={80} onChange={e => setName(e.target.value)} autoFocus />
        <button className="lc-button" disabled={busy || blocked}>Save name</button>
        <button type="button" className="lc-button" onClick={() => setEditing(undefined)}>Cancel</button>
      </form> : <>
        <span className="lc-agentaccounts__who">
          <span className="lc-agentaccounts__name">{a.name}</span>
          <span className="lc-agentaccounts__state" role="status">{stateSentence(a.state)}</span>
        </span>
        <span className="lc-agentaccounts__actions">
          {a.state === 'waiting'
            ? <button type="button" className="lc-button" disabled={busy} onClick={() => void act({ kind: 'cancel', id: a.id })}>Cancel sign-in</button>
            : <button type="button" className="lc-button" disabled={busy || blocked} onClick={() => void act({ kind: 'login', id: a.id })}>{a.state === 'ready' ? 'Sign in again' : 'Sign in'}</button>}
          <button type="button" className="lc-button" disabled={busy || blocked} onClick={() => { setEditing(a.id); setName(a.name) }}>Rename</button>
          <button type="button" className="lc-button" disabled={busy || a.state === 'waiting'} onClick={() => setRemoving(a)}>Remove</button>
        </span>
        {usage?.get(accountUsageKey(runtime, a.id)) !== undefined && <div className="lc-agentaccounts__usage"><UsageMeters said={usage.get(accountUsageKey(runtime, a.id))!} /></div>}
      </>}
    </div>)}
    {adding ? <form className="lc-agentaccounts__form lc-agentaccounts__adding" onSubmit={event => {
      event.preventDefault()
      void act({ kind: 'add', runtime, name }).then(ok => { if (ok) { setAdding(false); setName('') } })
    }}>
      <input className="lc-input" aria-label="Account name" value={name} placeholder="Name it, like Work" maxLength={80} autoFocus onChange={event => setName(event.target.value)} />
      <button className="lc-button" disabled={busy || blocked || !name.trim()}>Add account</button>
      <button type="button" className="lc-button" onClick={() => setAdding(false)}>Cancel</button>
      <p className="lc-agentaccounts__state">Then Sign in opens the agent&apos;s own sign-in in your browser.</p>
    </form> : <button type="button" className="lc-button lc-agentaccounts__add" disabled={blocked || busy} onClick={() => { setAdding(true); setName('') }}>
      <Icon name="plus" size={13} />
      Add another account
    </button>}
    {blocked && <ManagedPolicyNote />}
    {message !== undefined && <p className="lc-settings__note" role="alert">{message}</p>}
    {removing !== undefined && <div className="lc-agentaccounts__confirm" role="alertdialog" aria-label={`Remove ${removing.name}?`}>
      <strong>Remove {removing.name}?</strong>
      <p>This deletes only {removing.name}&apos;s agent home that Locust created: its sign-in, settings, session history and plugins. {`This ${windowWords().computer}'s sign-in and your project files stay where they are.`} Teammates using this account will need another account selected.</p>
      {removing.home !== undefined && <p className="lc-settings__note lc-mono">{removing.home}</p>}
      <span className="lc-agentaccounts__actions">
        <button type="button" className="lc-button lc-button--danger" disabled={busy} onClick={() => void act({ kind: 'remove', id: removing.id }).then(ok => { if (ok) setRemoving(undefined) })}>Delete this account</button>
        <button type="button" className="lc-button" onClick={() => setRemoving(undefined)}>Keep account</button>
      </span>
    </div>}
  </div>
}
