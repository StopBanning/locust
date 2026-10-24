import { useEffect, useState, type ReactElement } from 'react'

import { keyNameRefusal, KEYS_REACH_SAID, type KeyRequest, type TeammateKeysApi } from '../../../shared/teammate-keys.js'
import { Icon } from './Icon.js'

/*
 * A KEY A TEAMMATE ASKED FOR (0.833, shared/teammate-keys.ts): under the answer that asked, each key by its name and
 * what it is for, a masked box, and Save. The value goes to Locust on this computer once, kept encrypted for that
 * teammate alone, and never comes back to the window -- only its name does.
 */
export function KeyCard({ teammateId, name, requests, reaches, api = typeof window === 'undefined' ? undefined : window.desktop?.teammateKeys, kept: initial }: {
  readonly teammateId: string
  readonly name: string
  readonly requests: readonly KeyRequest[]
  /** Whether the teammate's agent passes keys into its commands (KEYS_REACH). */
  readonly reaches: boolean
  readonly api?: TeammateKeysApi
  /** The names it has, before Locust has answered: a test draws each state from it. */
  readonly kept?: readonly string[]
}): ReactElement {
  const [kept, setKept] = useState<readonly string[]>(initial ?? [])
  useEffect(() => {
    let alive = true
    void api?.list(teammateId).then((result) => { if (alive) setKept(result.names) }, () => undefined)
    return () => { alive = false }
  }, [api, teammateId])
  return (
    <div className="lc-diagnostic lc-tone-muted lc-keycard" role="group" aria-label={`Keys ${name} asked for`}>
      <Icon name="shield" size={12} />
      <div className="lc-editcheck__body lc-keycard__body">
        {!reaches && <span>{`${name} asked for a key, but Locust does not give keys to its agent yet, so it could not use one. Keys reach ${KEYS_REACH_SAID} teammates for now: give ${name} one of those in its settings to use a key.`}</span>}
        {reaches && requests.map((request) => (
          <KeyRequestRow key={request.name} teammateId={teammateId} name={name} request={request} has={kept.includes(request.name)} api={api} onKept={setKept} />
        ))}
      </div>
    </div>
  )
}

function KeyRequestRow({ teammateId, name, request, has, api, onKept }: {
  readonly teammateId: string
  readonly name: string
  readonly request: KeyRequest
  readonly has: boolean
  readonly api: TeammateKeysApi | undefined
  readonly onKept: (names: readonly string[]) => void
}): ReactElement {
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string>()
  const refused = keyNameRefusal(request.name)
  const why = request.why.length > 0 ? ` -- ${request.why}` : ''
  if (refused !== undefined) {
    return <span className="lc-keycard__row">{`${name} asked for a key named ${request.name}${why}. ${refused} Ask ${name} for it under another name.`}</span>
  }
  if (has) {
    return (
      <span className="lc-keycard__row" role="status">
        <Icon name="check" size={12} />{` ${name} has ${request.name}. It can use it from its next turn; remove it any time in ${name}’s settings.`}
      </span>
    )
  }
  return (
    <form className="lc-keycard__row lc-keycard__form" onSubmit={(event) => {
      event.preventDefault()
      if (api === undefined || busy) return
      setBusy(true)
      setProblem(undefined)
      void api.set(teammateId, request.name, value)
        .then((result) => {
          if (result.ok) { setValue(''); onKept(result.names) } else setProblem(result.message)
        })
        .catch(() => setProblem('The key could not be kept. Nothing changed.'))
        .finally(() => setBusy(false))
    }}>
      <span>{`${name} asks for a key: `}<code>{request.name}</code>{why}</span>
      <span className="lc-keycard__entry">
        <input className="lc-input" type="password" aria-label={`The key ${request.name} for ${name}`} autoComplete="off" spellCheck={false} value={value} disabled={busy}
          onChange={(event) => { setValue(event.target.value); setProblem(undefined) }} placeholder="Paste the key" />
        <button type="submit" className="lc-button" disabled={busy || value.trim().length === 0}>{`Keep for ${name}`}</button>
      </span>
      <span className="lc-interview__note">{`Kept encrypted on this computer, for ${name} alone. Its runs get it as ${request.name}, and Locust takes the value out of anything it records or shows.`}</span>
      {problem !== undefined && <span className="lc-interview__note lc-tone-red" role="alert">{problem}</span>}
    </form>
  )
}

/** A teammate's keys in its settings (0.833): each by name, with Remove, and one added by name and masked value. */
export function TeammateKeysField({ teammateId, name, api = typeof window === 'undefined' ? undefined : window.desktop?.teammateKeys, kept: initial }: {
  readonly teammateId: string
  readonly name: string
  readonly api?: TeammateKeysApi
  readonly kept?: readonly string[]
}): ReactElement {
  const [kept, setKept] = useState<readonly string[]>(initial ?? [])
  const [keyName, setKeyName] = useState('')
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string>()
  useEffect(() => {
    let alive = true
    void api?.list(teammateId).then((result) => { if (alive) setKept(result.names) }, () => undefined)
    return () => { alive = false }
  }, [api, teammateId])
  const change = (work: () => Promise<{ readonly ok: boolean; readonly names: readonly string[]; readonly message?: string }>, after?: () => void): void => {
    if (api === undefined || busy) return
    setBusy(true)
    setProblem(undefined)
    void work()
      .then((result) => { setKept(result.names); if (result.ok) after?.(); else setProblem(result.message) })
      .catch(() => setProblem('The keys could not be changed. They are unchanged.'))
      .finally(() => setBusy(false))
  }
  return (
    <div className="lc-field lc-field--lessons">
      <span className="lc-fieldlabel lc-mono">Keys</span>
      {kept.length > 0 && (
        <ul className="lc-tunelessons" aria-label={`Keys for ${name}`}>
          {kept.map((one) => (
            <li key={one} className="lc-tunelessons__row">
              <span className="lc-tunelessons__text"><code>{one}</code></span>
              <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => change(() => api!.remove(teammateId, one))}>Remove</button>
            </li>
          ))}
        </ul>
      )}
      <span className="lc-keycard__entry">
        <input className="lc-input" aria-label="The key’s name" autoComplete="off" spellCheck={false} value={keyName} disabled={busy} placeholder="STRIPE_KEY"
          onChange={(event) => { setKeyName(event.target.value.toUpperCase().replace(/[^A-Z0-9_]/g, '_')); setProblem(undefined) }} />
        <input className="lc-input" type="password" aria-label="The key" autoComplete="off" spellCheck={false} value={value} disabled={busy} placeholder="Paste the key"
          onChange={(event) => { setValue(event.target.value); setProblem(undefined) }} />
        <button type="button" className="lc-button" disabled={busy || keyName.length === 0 || value.trim().length === 0}
          onClick={() => change(() => api!.set(teammateId, keyName, value), () => { setKeyName(''); setValue('') })}>Add</button>
      </span>
      {problem !== undefined && <span className="lc-field__hint lc-tone-red" role="alert">{problem}</span>}
      <span className="lc-field__hint">{`Kept encrypted on this computer, for ${name} alone. Its runs get each as an environment variable of that name; its brief names them, and Locust takes their values out of anything it records or shows. Keys reach ${KEYS_REACH_SAID} teammates for now.`}</span>
    </div>
  )
}
