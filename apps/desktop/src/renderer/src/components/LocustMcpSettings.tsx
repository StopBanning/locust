import { useEffect, useState, type ReactNode } from 'react'
import type { ClaudeAppConnection, LocustMcpApi, LocustMcpState } from '../../../shared/locust-mcp.js'
import { CopyButton } from './CopyButton.js'
import { policyFeatureRefusal } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'

export function LocustMcpSettings({ heading, api = typeof window === 'undefined' ? undefined : window.desktop?.locustMcp }: { readonly heading?: ReactNode; readonly api?: LocustMcpApi }) {
  const managed = policyFeatureRefusal(useOrganizationPolicy(), 'otherApps') !== undefined
  const [state, setState] = useState<LocustMcpState>()
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')
  /** Locust in the Claude app's tools (0.740): where it stands, read when this opens. */
  const [claudeApp, setClaudeApp] = useState<ClaudeAppConnection>()
  const [claudeSaid, setClaudeSaid] = useState('')
  useEffect(() => {
    let alive = true
    void api?.settings().then(value => { if (alive) setState(value) }).catch(() => { if (alive) setFailure('The server setting could not be read. Its previous choice is unchanged.') })
    void api?.claudeApp?.().then(value => { if (alive) setClaudeApp(value) }).catch(() => undefined)
    return () => { alive = false }
  }, [api])
  const change = async (next: (current: LocustMcpState) => Promise<LocustMcpState>) => {
    if (!api || !state || busy) return
    setBusy(true); setFailure('')
    try { setState(await next(state)) }
    catch { setFailure('The server setting could not be changed. Its previous choice is unchanged.') }
    finally { setBusy(false) }
  }
  const toggle = () => change(current => api!.setEnabled(!current.enabled))
  const toggleOwnMode = () => change(current => api!.setOwnMode(!current.ownMode))
  const toggleEditorAnswers = () => change(current => api!.setEditorAnswers!(current.editorAnswers !== true))
  const toggleConversations = () => change(current => api!.setConversations!(current.conversations !== true))
  /** One press: Add turns the server on and puts Locust in Claude's tools; Remove takes it out; Restart closes and reopens Claude. */
  const pressClaude = async (what: 'add' | 'remove' | 'restart') => {
    if (!api || busy) return
    setBusy(true); setClaudeSaid('')
    try {
      const result = what === 'restart' ? await api.restartClaudeApp?.() : await api.setClaudeApp?.(what === 'add')
      if (result !== undefined) {
        setClaudeApp(result.connection)
        if (result.state !== undefined) setState(result.state)
        setClaudeSaid(result.message ?? '')
      }
    } catch {
      setClaudeSaid('The Claude app was not changed.')
    } finally { setBusy(false) }
  }
  return <>
    <div className="lc-settingline">
      <div className="lc-settingline__text">
        {heading}
        {managed && <ManagedPolicyNote />}
        <p className="lc-settings__lede">{state?.enabled
          ? `On. Other apps on this computer can ask teammates, run your routines and read replies. ${state.ownMode ? "New turns use each teammate's own mode" : 'New turns are Ask mode (read only)'}, with no automatic teammate handoffs. Locust must be running; your AI account usage and monthly limits still apply.`
          : 'Off. No local server is listening.'}</p>
      </div>
      <button type="button" className={`lc-switch${!managed && state?.enabled ? ' is-on' : ''}`} role="switch" aria-label="Let your other AI apps use Locust" aria-checked={!managed && state?.enabled === true} disabled={managed || !state || busy} onClick={() => { void toggle() }}><span /></button>
    </div>
    {/* A sub-setting of the switch above (0.707): spaced and indented, so it does not read as that line's last sentence. */}
    {state?.enabled && <div className="lc-settingline lc-settingline--sub">
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">Use each teammate's own mode</h3>
        <p className="lc-settings__lede">{state.ownMode
          ? `On. A turn another app starts runs in the mode the teammate is set to here, so it can edit files. ${state.editorAnswers === true ? 'Approval cards appear in this window, and an editor can answer the ones for turns it started.' : 'Approval cards appear in this window and only you can answer them.'} Auto runs only if Auto is on in Settings.`
          : 'Off. Every turn another app starts is Ask mode (read only), whatever mode the teammate is set to.'}</p>
      </div>
      <button type="button" className={`lc-switch${state.ownMode ? ' is-on' : ''}`} role="switch" aria-label="Use each teammate's own mode" aria-checked={state.ownMode} disabled={busy} onClick={() => { void toggleOwnMode() }}><span /></button>
    </div>}
    {/* THE PERSON'S CONVERSATIONS (0.842, Colin: "can duet start or look at active chats as well?"). */}
    {state?.enabled && <div className="lc-settingline lc-settingline--sub">
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">Let apps see your conversations</h3>
        <p className="lc-settings__lede">{state.conversations === true
          ? 'On. An app can list, read, continue and stop your conversations in this folder, and carry one whose plan ran out on to another AI agent, by the same rules as its own: no higher mode, and cards answered only here.'
          : 'Off. An app sees only the conversations it started. A turn it started that runs out can still be carried on to another AI agent.'}</p>
      </div>
      <button type="button" className={`lc-switch${state.conversations === true ? ' is-on' : ''}`} role="switch" aria-label="Let apps see your conversations" aria-checked={state.conversations === true} disabled={busy || api?.setConversations === undefined} onClick={() => { void toggleConversations() }}><span /></button>
    </div>}
    {/*
      * EDITORS ANSWER ONLY WHEN YOU SAY SO (0.790). Every app connected to Locust holds the key an editor session is
      * opened with, so an editor answering its own turn's cards is the person's choice, like pairing a phone. Shown
      * once an editor has connected: nobody who has never used one reads about it.
      */}
    {state?.enabled && (state.editorSeen === true || state.editorAnswers === true) && <div className="lc-settingline lc-settingline--sub">
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">Let editors answer cards for their own turns</h3>
        <p className="lc-settings__lede">{state.editorAnswers === true
          ? 'On. An editor that starts a turn can approve or deny that turn’s cards there. Questions are still answered here, and no app can answer a card for a turn it did not start.'
          : 'Off. Cards for a turn an editor starts wait in this window, and only you answer them.'}</p>
      </div>
      <button type="button" className={`lc-switch${state.editorAnswers === true ? ' is-on' : ''}`} role="switch" aria-label="Let editors answer cards for their own turns" aria-checked={state.editorAnswers === true} disabled={busy || api?.setEditorAnswers === undefined} onClick={() => { void toggleEditorAnswers() }}><span /></button>
    </div>}
    {/*
      * THE CLAUDE APP, IN ONE PRESS (0.740). Colin: "asking me to do the pythagorean theorem for a basic feature
      * isnt it". Shown only where the Claude app is installed; Add also turns the switch above on.
      */}
    {/* On a managed computer with other apps off, nothing here can be added; Remove stays for an entry made before (0.792). */}
    {claudeApp !== undefined && claudeApp.kind !== 'no-app' && (!managed || claudeApp.kind === 'added') && <div className="lc-settingline lc-claudeapp">
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">The Claude app</h3>
        <p className="lc-settings__lede">{
          claudeApp.kind === 'unreadable' || claudeApp.kind === 'taken' ? claudeApp.message
            : claudeApp.kind === 'not-added'
              ? claudeApp.restart === true
                ? 'Removed. The Claude app keeps using Locust until it restarts; restarting closes everything running in it.'
                : 'Add Locust to the Claude app’s tools, so a Claude chat can ask your teammates and read their replies. One press; Locust keeps a copy of Claude’s settings first.'
              : claudeApp.stale === true ? 'Added, but for a copy of Locust that is not this one, or that has moved. Update points it here.'
                : claudeApp.restart === true ? 'Added. The Claude app reads it when it starts, and it is open now: restart it to finish. Restarting closes everything running in the Claude app.'
                  : 'Added. Ask a Claude chat about your Locust teammates; it asks before it uses them.'
        }</p>
        {claudeSaid && <p className="lc-settings__note" role="status">{claudeSaid}</p>}
      </div>
      <span className="lc-claudeapp__acts">
        {claudeApp.kind === 'not-added' && <button type="button" className="lc-control lc-control--boxed" disabled={managed || busy} onClick={() => { void pressClaude('add') }}>Add to the Claude app</button>}
        {claudeApp.kind === 'added' && claudeApp.stale === true && <button type="button" className="lc-control lc-control--boxed" disabled={managed || busy} onClick={() => { void pressClaude('add') }}>Update</button>}
        {(claudeApp.kind === 'added' || claudeApp.kind === 'not-added') && claudeApp.restart === true && <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void pressClaude('restart') }}>Restart Claude</button>}
        {claudeApp.kind === 'added' && <button type="button" className="lc-control" disabled={busy} onClick={() => { void pressClaude('remove') }}>Remove</button>}
      </span>
    </div>}
    {/*
      * By hand, folded (0.743): with the Claude app one press away, two blocks of paths open under the switch were
      * the loudest thing on General. They are for Claude Code and Codex, which take a command or a config entry.
      */}
    {state?.enabled && <details className="lc-settings__fold lc-mcp-byhand">
      <summary>Add Locust to Claude Code, Codex or OpenClaw by hand</summary>
      <div className="lc-settings__note">
      {/* A Copy on each (0.691): the lines run to three paths, and selecting a wrapped block by hand is fiddly. */}
      <p>Claude Code — run this in PowerShell:</p>
      <div className="lc-mcp-setupwrap"><pre className="lc-mcp-setup lc-mono">{state.claudeCommand}</pre>
        {state.claudeCommand !== undefined && <CopyButton className="lc-mcp-setup__copy" label="the Claude Code command" text={state.claudeCommand} />}</div>
      <p>Codex — add this to ~/.codex/config.toml:</p>
      <div className="lc-mcp-setupwrap"><pre className="lc-mcp-setup lc-mono">{state.codexConfig}</pre>
        {state.codexConfig !== undefined && <CopyButton className="lc-mcp-setup__copy" label="the Codex settings" text={state.codexConfig} />}</div>
      {/*
        * OpenClaw (0.754): the assistant people message from WhatsApp, Telegram, iMessage or Slack. With this line its
        * chats can ask the team here. It checks the connection before it saves; its own tool policy decides which of
        * its chats may use the tools.
        */}
      {state.openclawCommand !== undefined && <>
        <p>OpenClaw — run this in PowerShell. It checks the connection first, and its tool policy decides which of its chats can use it:</p>
        <div className="lc-mcp-setupwrap"><pre className="lc-mcp-setup lc-mono">{state.openclawCommand}</pre>
          <CopyButton className="lc-mcp-setup__copy" label="the OpenClaw command" text={state.openclawCommand} /></div>
      </>}
      <p>Locust changes another app’s settings only when you press Add to the Claude app; it keeps a copy of the file first, and Remove takes its entry out again. Turning this off stops new requests, not turns already running.</p>
      </div>
    </details>}
    {/* Managed, the host's word is the policy's, already said beside the switch: not said twice, nor before any press. */}
    {(failure || (!managed && state?.message)) && <p className="lc-settings__note" role="status">{failure || state?.message}</p>}
  </>
}
