import { useEffect, useState } from 'react'
import { REMOTE_CONTROL_LABEL, REMOTE_CONTROL_SET_CHANNEL, type RemoteControlState } from '../../../shared/claude-remote-control.js'
import { OnTheComputer } from './OnTheComputer.js'
import { policyFeatureRefusal, policyUnmeteredRefusal } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'

export function RemoteControlSetting() {
  const [state, setState] = useState<RemoteControlState>()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  useEffect(() => {
    let alive = true
    let asking = false
    const read = async () => {
      if (asking) return
      asking = true
      try { const next = await window.desktop?.getRemoteControl(); if (alive) setState(next) }
      catch (cause) { if (alive) setError(cause instanceof Error ? cause.message : String(cause)) }
      finally { asking = false }
    }
    void read()
    const timer = setInterval(() => { void read() }, 1000)
    return () => { alive = false; clearInterval(timer) }
  }, [])
  // Claude Code runs on the computer, so its switch is turned there (0.849).
  return <OnTheComputer channel={REMOTE_CONTROL_SET_CHANNEL}><RemoteControlView state={state} busy={busy} error={error} onToggle={async (enabled) => {
    setBusy(true); setError(undefined)
    try { setState(await window.desktop?.setRemoteControl(enabled)) }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)) }
    finally { setBusy(false) }
  }} /></OnTheComputer>
}

/**
 * Where the switch stands, as the first words of its line -- "On.", "Off." --
 * the way every other switch in Settings says it (0.619). This sentence had a
 * row of its own under the switch, in the label's colour, and read as a
 * second setting (Colin, 2026-10-04: "whats going on here? ive never noticed
 * this").
 */
export function remoteControlSays(state: RemoteControlState): string {
  switch (state.phase) {
    case 'needs-person': return 'Complete setup in the Claude Code terminal, then enable this switch again.'
    case 'starting': return 'Starting Claude Code…'
    case 'running': return 'On. Start a session in this folder from claude.ai or the Claude app; what Claude Code prints is below. It stops when you switch it off or quit Locust.'
    case 'stopping': return 'Waiting for Claude Code to end…'
    case 'ended': return 'Claude Code ended.'
    case 'error': return `Claude Code reported a problem${state.exitCode === undefined ? '.' : ` (exit ${state.exitCode ?? 'unknown'}).`}`
    default: return 'Off. When on, you can start Claude Code sessions in this folder from claude.ai while Locust is open, until you switch it off or quit Locust.'
  }
}

export function RemoteControlView({ state, busy, error, onToggle }: {
  readonly state?: RemoteControlState
  readonly busy: boolean
  readonly error?: string
  readonly onToggle: (enabled: boolean) => Promise<void>
}) {
  const problem = error ?? state?.error
  const policy = useOrganizationPolicy()
  const managed = (policyFeatureRefusal(policy, 'phone') ?? policyUnmeteredRefusal(policy, { runtime: 'claude', mode: 'auto' })) !== undefined
  // What Claude Code printed has a row of its own, and only when there is some.
  const printed = state !== undefined && (state.stdout.length > 0 || state.stderr.length > 0 || state.truncated)
  return <div className="lc-settingrows">
    <div className="lc-settingrow">
      <div className="lc-settingline__text">
        <div>{REMOTE_CONTROL_LABEL}</div>
        {managed && <ManagedPolicyNote />}
        {/* One quiet line, the state first: the account it needs follows in the same voice, never brighter. */}
        <p className="lc-settings__lede">
          {state !== undefined && <><span role="status">{remoteControlSays(state)}</span>{' '}</>}
          It needs your claude.ai sign-in; API keys do not work. Team and Enterprise owners must allow Remote Control.
        </p>
      </div>
      <button type="button" className={`lc-switch${!managed && state?.enabled ? ' is-on' : ''}`} role="switch" aria-label={REMOTE_CONTROL_LABEL} aria-checked={!managed && (state?.enabled ?? false)} disabled={managed || busy || state === undefined || state.phase === 'stopping'} onClick={() => { void onToggle(!state?.enabled) }}><span className="lc-switch__knob" /></button>
    </div>
    {(printed || problem !== undefined) && <div className="lc-settingrow lc-remoteoutput">
      {state?.stdout && <><span className="lc-remoteoutput__label">Claude Code output</span><pre className="lc-remoteoutput__text">{state.stdout}</pre></>}
      {state?.stderr && <><span className="lc-remoteoutput__label">Claude Code errors</span><pre className="lc-remoteoutput__text">{state.stderr}</pre></>}
      {problem !== undefined && <pre role="alert" className="lc-remoteoutput__text">{problem}</pre>}
      {state?.truncated && <p className="lc-settings__note">Only the start of what Claude Code printed is shown; the rest was left out.</p>}
    </div>}
  </div>
}
