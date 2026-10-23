import { useEffect, useMemo, useRef, useState, type ReactElement, type ReactNode } from 'react'
import qrcode from 'qrcode-generator'

import type { PhoneAnywhereSettings, PhoneApi, PhoneMode, PhoneModeState, PhoneSettings as PhoneState } from '../../../shared/phone.js'
import { policyFeatureRefusal } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'
import { agoLabel } from '../teammateWork.js'

/** The code as one SVG path of dark squares, drawn here: no HTML from the library goes into the page. */
export function QrCode({ text, label }: { readonly text: string; readonly label: string }): ReactElement {
  const { size, path } = useMemo(() => {
    const code = qrcode(0, 'M')
    code.addData(text)
    code.make()
    const count = code.getModuleCount()
    let d = ''
    for (let row = 0; row < count; row += 1) {
      for (let col = 0; col < count; col += 1) if (code.isDark(row, col)) d += `M${String(col + 4)} ${String(row + 4)}h1v1h-1z`
    }
    return { size: count + 8, path: d }
  }, [text])
  // Dark on white whatever the theme: a phone's camera reads it, the window's colours do not matter to it.
  return (
    <svg className="lc-phone__qr" viewBox={`0 0 ${String(size)} ${String(size)}`} role="img" aria-label={label} shapeRendering="crispEdges">
      <rect width={size} height={size} fill="#ffffff" />
      <path d={path} fill="#000000" />
    </svg>
  )
}

/** When a phone was last there, in the words Settings uses elsewhere. */
function deviceState(device: PhoneAnywhereSettings['devices'][number]): string {
  const seen = device.seen === undefined ? undefined : agoLabel(device.seen)
  const when = device.online ? 'Open now' : seen === undefined ? 'Paired' : `Last open ${seen}`
  return device.alerts === true ? `${when} · alerts on` : when
}

/*
 * YOUR PHONE (0.775, shared/phone.ts). One switch, and how the phone reaches Locust: from anywhere, through Locust's
 * relay (0.826, main/phone-anywhere.ts), or only on this Wi-Fi, as before. Turning it off, or changing the way,
 * unpairs every phone of the way that goes off.
 */
export function PhoneSettings({ heading, more, api = typeof window === 'undefined' ? undefined : window.desktop?.phone, initial }: {
  readonly heading?: ReactNode
  readonly more?: ReactNode
  readonly api?: PhoneApi
  /** What to show before Locust has answered: a test draws each state from it. */
  readonly initial?: { readonly wifi?: PhoneState; readonly anywhere?: PhoneAnywhereSettings }
}): ReactElement {
  const [wifi, setWifi] = useState<PhoneState | undefined>(initial?.wifi)
  const [anywhere, setAnywhere] = useState<PhoneAnywhereSettings | undefined>(initial?.anywhere)
  // The way a phone will reach Locust when the switch goes on; from anywhere unless the person picks this Wi-Fi.
  const [picked, setPicked] = useState<Exclude<PhoneMode, 'off'>>('anywhere')
  const managed = policyFeatureRefusal(useOrganizationPolicy(), 'phone') !== undefined
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')
  const [copied, setCopied] = useState(false)
  const asked = useRef(false)

  useEffect(() => {
    let alive = true
    void Promise.all([api?.settings(), api?.anywhere()]).then(([lan, relay]) => {
      if (!alive) return
      setWifi(lan)
      setAnywhere(relay)
    }).catch(() => { if (alive) setFailure('This setting could not be read. It is unchanged.') })
    const stop = api?.onAnywhereChanged((next) => { if (alive) setAnywhere(next) })
    return () => { alive = false; stop?.() }
  }, [api])

  const mode: PhoneMode = anywhere?.enabled === true ? 'anywhere' : wifi?.enabled === true ? 'wifi' : 'off'
  const on = !managed && mode !== 'off'
  const shown: Exclude<PhoneMode, 'off'> = mode === 'off' ? picked : mode

  const run = async <T,>(work: () => Promise<T>, apply: (value: T) => void): Promise<void> => {
    if (!api || busy) return
    setBusy(true); setFailure('')
    try { apply(await work()) } catch { setFailure('This setting could not be changed. It is unchanged.') } finally { setBusy(false) }
  }
  const setMode = (next: PhoneMode): Promise<void> => run(() => api!.setMode(next), (state: PhoneModeState) => { setWifi(state.wifi); setAnywhere(state.anywhere) })
  const newCode = (): Promise<void> => run(() => api!.newCode(), setAnywhere)

  // A code runs out after ten minutes; while it is on screen a fresh one takes its place. With no phone paired yet,
  // one is shown at once: that is what the person came for.
  const code = anywhere?.code
  const devices = anywhere?.devices.length ?? 0
  useEffect(() => {
    if (!on || mode !== 'anywhere' || anywhere?.connected === undefined) return
    if (code === undefined) {
      if (devices === 0 && !asked.current) { asked.current = true; void newCode() }
      return
    }
    const timer = setTimeout(() => { void newCode() }, Math.max(1_000, Date.parse(code.until) - Date.now()))
    return () => clearTimeout(timer)
    // newCode is made fresh each render; the code and the mode are what this follows.
  }, [on, mode, code?.until, devices, anywhere?.connected])

  return (
    <>
      <div className="lc-settingline">
        <div className="lc-settingline__text">
          {heading}
          {managed && <ManagedPolicyNote />}
          <p className="lc-settings__lede">{!on
            ? 'Off. Turn it on to check on your teammates from your phone, answer approval cards and send the next message.'
            : mode === 'anywhere'
              ? 'On, from anywhere. Your phone reaches Locust through Locust’s relay. Every message is locked on this computer and opened on your phone, so the relay passes along what it cannot read.'
              : wifi?.url === undefined
                ? 'On, but this computer is not on a network Locust can find. Connect it to your Wi-Fi, then turn this off and on again.'
                : 'On. Scan the code with your phone’s camera to see your conversations, answer approval cards and send the next message. Your phone must be on the same Wi-Fi as this computer.'}</p>
          {wifi?.message !== undefined && mode !== 'anywhere' && <p className="lc-settings__note" role="status">{wifi.message}</p>}
          {failure && <p className="lc-settings__note" role="status">{failure}</p>}
        </div>
        <button type="button" className={`lc-switch${on ? ' is-on' : ''}`} role="switch" aria-label="Use Locust from your phone" aria-checked={on} disabled={managed || !wifi || !anywhere || busy} onClick={() => { void setMode(on ? 'off' : picked) }}><span /></button>
      </div>
      <div className="lc-segmented lc-phone__ways" role="radiogroup" aria-label="How your phone reaches Locust">
        {([['anywhere', 'From anywhere'], ['wifi', 'Only on this Wi-Fi']] as const).map(([value, label]) => (
          <button key={value} type="button" role="radio" aria-checked={shown === value} className={`lc-button${shown === value ? ' is-active' : ''}`} disabled={managed || busy}
            onClick={() => { if (on) { if (mode !== value) void setMode(value) } else setPicked(value) }}>
            {label}
          </button>
        ))}
      </div>

      {on && mode === 'anywhere' && anywhere !== undefined && (
        <>
          <div className="lc-phone" data-phone-anywhere="">
            {code !== undefined && <QrCode text={code.url} label="A code for your phone’s camera. It pairs your phone with Locust." />}
            <div className="lc-phone__side">
              <p className="lc-settings__note" role="status">{anywhere.message ?? (anywhere.connected ? 'Connected to Locust’s relay.' : 'Connecting to Locust’s relay…')}</p>
              {code !== undefined
                ? <>
                  <p className="lc-settings__note">Scan it with your phone’s camera. It pairs one phone, once, within ten minutes, then a new one takes its place.</p>
                  {/* A computer has no camera to scan it with (0.830): the same code, as a link to open in its browser. */}
                  <p className="lc-settings__note">On another computer, open this code as a link in its browser instead.</p>
                  <span><button type="button" className="lc-control lc-control--boxed" onClick={() => {
                    void navigator.clipboard?.writeText(code.url).then(() => { setCopied(true); setTimeout(() => setCopied(false), 2_500) }, () => undefined)
                  }}>{copied ? 'Copied' : 'Copy the link'}</button></span>
                </>
                : <span><button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void newCode() }}>Pair a phone</button></span>}
              {anywhere.devices.length > 0 && <p className="lc-settings__note">Lost a phone? Remove it below, and it stops at once.</p>}
              {anywhere.devices.some((device) => device.desk === true) && <p className="lc-settings__note">With Full window on, open app.locust.lol/desk in that device’s browser: the whole window, while this computer shows that it is open there.</p>}
            </div>
          </div>
          {anywhere.devices.length > 0 && (
            <ul className="lc-phone__devices" aria-label="Paired phones">
              {anywhere.devices.map((device) => (
                <li key={device.id} className="lc-phone__device">
                  <span className="lc-phone__who">
                    <span className="lc-phone__name">{device.name}</span>
                    <span className="lc-phone__state">{deviceState(device)}</span>
                  </span>
                  {/* Locust in a browser (0.830): the whole window, at app.locust.lol/desk, only for a device let here. */}
                  {api?.setDesk !== undefined && <label className="lc-phone__desk">
                    <span>Full window</span>
                    <button type="button" className={`lc-switch${device.desk === true ? ' is-on' : ''}`} role="switch" aria-label={`Let ${device.name} open the full window`} aria-checked={device.desk === true} disabled={busy}
                      onClick={() => { void run(() => api!.setDesk!(device.id, device.desk !== true), setAnywhere) }}><span /></button>
                  </label>}
                  <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove(device.id), setAnywhere) }}>Remove</button>
                </li>
              ))}
            </ul>
          )}
          {anywhere.devices.some((device) => device.alerts === true) && (
            <div className="lc-settingline lc-phone__night">
              <div className="lc-settingline__text">
                <p className="lc-settings__lede">Let approvals and questions buzz in quiet hours. Off, everything waits for the morning brief.</p>
              </div>
              <button type="button" className={`lc-switch${anywhere.nightApprovals === true ? ' is-on' : ''}`} role="switch" aria-label="Let approvals buzz in quiet hours" aria-checked={anywhere.nightApprovals === true} disabled={busy} onClick={() => { void run(() => api!.setNightApprovals(anywhere.nightApprovals !== true), setAnywhere) }}><span /></button>
            </div>
          )}
        </>
      )}

      {on && mode === 'wifi' && wifi?.url !== undefined && (
        <div className="lc-phone" data-phone-code="">
          <QrCode text={wifi.url} label="A code for your phone’s camera. It opens Locust on your phone." />
          <div className="lc-phone__side">
            <p className="lc-settings__note">The first time, Windows may ask whether Locust can use your network. Choose Allow, on private networks only.</p>
            <p className="lc-settings__note">Anyone who scans this code can do what you can from your phone. Lost a phone, or shared the code? Make a new one: every phone has to scan again.</p>
            <span><button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.forget(), setWifi) }}>New code</button></span>
          </div>
        </div>
      )}
      {more}
    </>
  )
}
