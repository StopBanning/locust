import { useEffect, useState } from 'react'
import type { ReactElement, ReactNode } from 'react'

import type { PhoneApi, YourAccountStatus } from '../../../shared/phone.js'
import { ArmedButton } from './ArmedButton.js'

/**
 * SETTINGS › YOUR ACCOUNT (accounts slice 3, main/your-account.ts).
 *
 * The account is the GitHub sign-in Use from anywhere ties this computer to. Sync keeps a sealed copy of this Locust
 * at the relay, which cannot open it. The recovery code is shown once, the first time sync goes on for an account,
 * and the card says plainly what losing it costs before anything is sent. Delete my copy asks twice.
 */

export function sizeOf(bytes: number): string {
  if (bytes < 1024 * 1024) return `${String(Math.max(1, Math.round(bytes / 1024)))} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** "just now", "4 min ago", "3 h ago", or the day: when the copy was last brought up to date. */
export function agoOf(at: string, now: number = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - Date.parse(at)) / 1000))
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${String(Math.round(seconds / 60))} min ago`
  if (seconds < 86_400) return `${String(Math.round(seconds / 3600))} h ago`
  return new Date(at).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** How often an open card looks again while signed in. */
export const ACCOUNT_LOOK_EVERY_MS = 5_000

export function YourAccountSettings({ heading, api = typeof window === 'undefined' ? undefined : window.desktop?.phone, initial, initialCode }: {
  readonly heading?: ReactNode
  readonly api?: PhoneApi
  /** What to show before Locust has answered: a test draws each state from it. */
  readonly initial?: YourAccountStatus
  readonly initialCode?: string
}): ReactElement | null {
  const [status, setStatus] = useState<YourAccountStatus | undefined>(initial)
  const [code, setCode] = useState<string | undefined>(initialCode)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')
  const [typed, setTyped] = useState('')
  const [bringing, setBringing] = useState<string>()

  useEffect(() => {
    let alive = true
    void api?.account?.().then((next) => { if (alive) setStatus(next) }, () => { if (alive) setFailure('Your account could not be read. Nothing changed.') })
    const stop = api?.onAccountChanged?.((next) => { if (alive) setStatus(next) })
    // Signing in with GitHub happens under Your phone; the card follows it.
    const stopPhone = api?.onAnywhereChanged(() => { void api.account?.().then((next) => { if (alive) setStatus(next) }, () => undefined) })
    return () => { alive = false; stop?.(); stopPhone?.() }
  }, [api])

  // While signed in, the card looks again every few seconds: a new computer's ask, or the grant it waits for, shows up
  // without anyone pressing anything. Main asks the relay at most once a look (your-account.ts COPIES_FRESH_MS).
  const signedIn = status?.signedIn !== undefined
  useEffect(() => {
    if (!signedIn || api?.account === undefined) return undefined
    let alive = true
    const timer = setInterval(() => { void api.account!().then((next) => { if (alive) setStatus(next) }, () => undefined) }, ACCOUNT_LOOK_EVERY_MS)
    return () => { alive = false; clearInterval(timer) }
  }, [api, signedIn])

  if (api?.account === undefined && initial === undefined) return null
  const run = async <T,>(work: () => Promise<T>, take: (value: T) => void): Promise<void> => {
    setBusy(true)
    setFailure('')
    try {
      take(await work())
    } catch {
      setFailure('That did not finish. Nothing changed.')
    } finally {
      setBusy(false)
    }
  }
  const on = status?.sync === 'on'
  const said = status?.message

  return (
    <>
      {heading}
      {status?.signedIn === undefined ? (
        <div className="lc-settingline">
          <div className="lc-settingline__text">
            <p className="lc-settings__lede">Sign in once, and your teammates, routines, memories and conversations follow you to any computer where you sign in.</p>
            <p className="lc-settings__note">Your account is your GitHub sign-in. Turn on From anywhere under Your phone, then press Sign in with GitHub there. Locust stays whole without an account.</p>
          </div>
        </div>
      ) : (
        <>
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <p className="lc-settings__lede">Signed in with GitHub as {status.signedIn.login}. Keep a copy of this Locust on Locust’s relay.</p>
              <p className="lc-settings__note">{on
                ? `${status.syncing ? 'Syncing now.' : status.lastSynced === undefined ? 'Not synced yet.' : `Last synced ${agoOf(status.lastSynced.at)}: ${String(status.lastSynced.files)} files, ${sizeOf(status.lastSynced.bytes)}.`}${status.storage === undefined ? '' : ` Using ${sizeOf(status.storage.used)} of ${sizeOf(status.storage.quota)}.`}`
                : status.hasKey
                  ? 'Off. Your copy stays as it was; turning sync on again needs no new code.'
                  : 'Each file is sealed on this computer before it leaves, with a key only your computers hold, so the relay cannot read it. Agent sign-ins and keys are never sent.'}</p>
            </div>
            <button type="button" className={`lc-switch${on ? ' is-on' : ''}`} role="switch" aria-label="Keep a copy on Locust’s relay" aria-checked={on} disabled={busy}
              onClick={() => { void run(async () => (on ? { status: await api!.accountSyncOff!() } : api!.accountSyncOn!()), (answer: { status: YourAccountStatus; code?: string }) => { setStatus(answer.status); if (answer.code !== undefined) setCode(answer.code) }) }}><span /></button>
          </div>
          {code !== undefined && (
            <div className="lc-settingline" role="alert">
              <div className="lc-settingline__text">
                <p className="lc-settings__lede">Write down your recovery code. It is shown only now.</p>
                <p className="lc-account__codetext lc-mono">{code}</p>
                <p className="lc-settings__note">A new computer opens your copy with this code, or with an OK from a computer already signed in. Lose every computer and this code, and the copy cannot be opened by anyone, Locust included.</p>
              </div>
              <button type="button" className="lc-control lc-control--boxed" onClick={() => { setCode(undefined) }}>I wrote it down</button>
            </div>
          )}
          {!on && status.elsewhere !== undefined && (
            <div className="lc-settingline lc-account__bring">
              <div className="lc-settingline__text">
                <p className="lc-settings__lede">{status.elsewhere.computers === 1 ? 'Another computer keeps' : `${String(status.elsewhere.computers)} other computers keep`} a copy of your Locust, last changed {agoOf(status.elsewhere.at)}. Bring it here with your recovery code.</p>
                <p className="lc-settings__note">Locust restarts once. What it replaces on this computer is moved aside first, not deleted.</p>
                <input className="lc-input lc-mono lc-account__codeinput" aria-label="Recovery code" placeholder="XXXXX-XXXXX-…" spellCheck={false} autoComplete="off" value={typed} disabled={busy || bringing !== undefined} onChange={(event) => { setTyped(event.target.value) }} />
              </div>
              <ArmedButton className="lc-control lc-control--boxed" ariaLabel="Bring your Locust here" armedLabel="Restart and bring it" disabled={busy || bringing !== undefined || typed.trim().length < 10}
                onConfirm={() => {
                  void run(() => api!.accountBringHere!(typed), (answer) => {
                    if (answer.ok) { setBringing(answer.computer); setTyped('') } else setFailure(answer.reason)
                  })
                }}>Bring it here</ArmedButton>
            </div>
          )}
          {!on && status.elsewhere !== undefined && bringing === undefined && (
            <div className="lc-settingline lc-account__bring">
              <div className="lc-settingline__text">
                {status.asking === undefined
                  ? <p className="lc-settings__lede">No code to hand? Ask the computer that has your Locust, and approve this one there.</p>
                  : <>
                    <p className="lc-settings__lede">Asking. On your other computer, open Settings › General › Your account, and approve there only if it shows this code:</p>
                    <p className="lc-account__codetext lc-mono">{status.asking.match}</p>
                    <p className="lc-settings__note">If the codes differ, choose Not mine there. Locust restarts here once it is approved.</p>
                  </>}
              </div>
              {status.asking === undefined
                ? <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.accountAsk!(), setStatus) }}>Ask that computer</button>
                : <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.accountAskStop!(), setStatus) }}>Stop asking</button>}
            </div>
          )}
          {bringing !== undefined && <p className="lc-settings__note lc-tone-green" role="status">Your Locust from {bringing} is ready. Restarting to bring it in…</p>}
          {(on || status.hasKey) && (
            <div className="lc-settingline lc-account__acts">
              {on && <button type="button" className="lc-control lc-control--boxed" disabled={busy || status.syncing} onClick={() => { void run(() => api!.accountSyncNow!(), setStatus) }}>Sync now</button>}
              <ArmedButton className="lc-control lc-control--boxed" ariaLabel="Delete my copy on Locust’s relay" armedLabel="Delete it" disabled={busy}
                onConfirm={() => { void run(() => api!.accountDelete!(), setStatus) }}>Delete my copy</ArmedButton>
            </div>
          )}
          {(status.asks ?? []).map((ask) => (
            <div className="lc-settingline lc-account__bring lc-account__ask" key={ask.room} role="alert">
              <div className="lc-settingline__text">
                <p className="lc-settings__lede">{ask.name} asks for your Locust. Approve only if its screen shows this code:</p>
                <p className="lc-account__codetext lc-mono">{ask.match}</p>
                <p className="lc-settings__note">Approving hands that computer the key to your copy, sealed so only it can open it. If the codes differ, or it is not your computer, choose Not mine.</p>
              </div>
              <div className="lc-account__acts">
                <ArmedButton className="lc-control lc-control--boxed" ariaLabel={`Approve ${ask.name}`} armedLabel="Approve it" disabled={busy}
                  onConfirm={() => { void run(() => api!.accountApprove!(ask.room, ask.match), setStatus) }}>Approve</ArmedButton>
                <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.accountIgnore!(ask.room), setStatus) }}>Not mine</button>
              </div>
            </div>
          ))}
        </>
      )}
      {(failure !== '' || said !== undefined) && <p className={`lc-settings__note ${failure !== '' || status?.good !== true ? 'lc-tone-amber' : 'lc-tone-green'}`}>{failure !== '' ? failure : said}</p>}
    </>
  )
}
