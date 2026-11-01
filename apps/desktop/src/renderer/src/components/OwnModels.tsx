import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { PublicOwnModel } from '../../../shared/ipc.js'
import { ArmedButton } from './ArmedButton.js'

/**
 * YOUR OWN MODELS, in Settings (0.357).
 *
 * A company with a model of its own -- Colin's father's builds one -- adds it
 * here once and it is in every teammate's model list, under its own name.
 * Anything that speaks the OpenAI chat API works: a company gateway, vLLM,
 * Ollama's /v1, LM Studio. The key is handed to the host and kept only as the
 * operating system encrypts it; this screen never sees it again, only
 * whether one is kept.
 *
 * Test asks the endpoint what it serves, before or after adding -- the
 * mistake worth catching early is a model name the endpoint does not know.
 */

/**
 * Said while Test waits (0.640): the model gets up to a minute to answer
 * (own-models.ts), and a minute of nothing reads as a click that did not take.
 */
export const OWN_MODEL_TESTING = 'Testing… A model that is starting up can take a minute to answer.'

export function OwnModels({ onChanged }: { readonly onChanged: () => void }): ReactElement {
  const [models, setModels] = useState<readonly PublicOwnModel[]>()
  const [name, setName] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [model, setModel] = useState('')
  const [key, setKey] = useState('')
  /*
   * CHAT ONLY (0.358): a model that cannot take tools -- OpenCode sends it
   * ten, and it answers 400 "does not support tools" -- still talks when its
   * runs are given none. Test finds out and sets this; so can the person.
   */
  const [chatOnly, setChatOnly] = useState(false)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<{ readonly text: string; readonly good: boolean }>()
  const [testedKept, setTestedKept] = useState<Readonly<Record<string, { readonly text: string; readonly good: boolean }>>>({})

  const load = (): void => {
    void window.desktop?.listOwnModels().then((response) => {
      if (response.ok) setModels(response.data.models)
    }).catch(() => undefined)
  }
  useEffect(load, [])

  const filled = name.trim().length > 0 && baseUrl.trim().length > 0 && model.trim().length > 0
  const typed = { baseUrl: baseUrl.trim(), model: model.trim(), ...(key.trim().length === 0 ? {} : { key: key.trim() }) }

  return (
    <div className="lc-ownmodels">
      {models !== undefined && models.length > 0 && (
        <div className="lc-settingrows">
          {models.map((kept) => (
            <div className="lc-settingrow lc-ownmodel" key={kept.ownId}>
              <span className="lc-ownmodel__text">
                <span className="lc-ownmodel__name">{kept.name}</span>
                <span className="lc-ownmodel__meta lc-mono">
                  {kept.model} · {hostOf(kept.baseUrl)} · {kept.hasKey ? 'key kept' : 'no key'}
                  {kept.chatOnly ? ' · chat only' : ''}
                </span>
                {testedKept[kept.ownId] !== undefined && (
                  <span className={`lc-ownmodel__said${testedKept[kept.ownId]!.good ? '' : ' lc-tone-amber'}`}>{testedKept[kept.ownId]!.text}</span>
                )}
              </span>
              <button
                type="button"
                className={`lc-switch${kept.chatOnly ? '' : ' is-on'}`}
                role="switch"
                aria-checked={!kept.chatOnly}
                aria-label={`${kept.name} uses tools`}
                title={kept.chatOnly ? 'Chat only: its runs get no tools' : 'Uses tools: it may read and change files'}
                onClick={() => {
                  void window.desktop?.setOwnModelChatOnly(kept.ownId, !kept.chatOnly).then(() => {
                    load()
                    onChanged()
                  })
                }}
              >
                <span className="lc-switch__knob" />
              </button>
              <button
                type="button"
                className="lc-button"
                disabled={testedKept[kept.ownId]?.text === OWN_MODEL_TESTING}
                onClick={() => {
                  const tested = (said: { readonly text: string; readonly good: boolean }): void => setTestedKept((held) => ({ ...held, [kept.ownId]: said }))
                  tested({ text: OWN_MODEL_TESTING, good: true })
                  void window.desktop?.testOwnModel({ ownId: kept.ownId }).then((response) => {
                    tested(response.ok ? { text: response.data.said, good: response.data.reached } : { text: response.error.message, good: false })
                    // A Test that found out whether it takes tools has set it.
                    if (response.ok && response.data.tools !== undefined) {
                      load()
                      onChanged()
                    }
                  }).catch(() => tested({ text: 'That model could not be tested. It is kept as it was.', good: false }))
                }}
              >
                Test
              </button>
              <ArmedButton
                className="lc-button"
                ariaLabel={`Remove ${kept.name}`}
                armedLabel={`Remove ${kept.name}?`}
                onConfirm={() => {
                  void window.desktop?.removeOwnModel(kept.ownId).then(() => {
                    load()
                    onChanged()
                  })
                }}
              >
                Remove
              </ArmedButton>
            </div>
          ))}
        </div>
      )}

      <form
        className="lc-ownmodel__form"
        onSubmit={(event) => {
          event.preventDefault()
          if (!filled || busy) return
          setBusy(true)
          setSaid(undefined)
          void window.desktop
            ?.addOwnModel({ name: name.trim(), ...typed, ...(chatOnly ? { chatOnly: true } : {}) })
            .then((response) => {
              if (!response.ok) {
                setSaid({ text: response.error.message, good: false })
                return
              }
              setSaid({ text: `${name.trim()} is in every teammate's model list, under Your models.`, good: true })
              setName('')
              setBaseUrl('')
              setModel('')
              setKey('')
              setChatOnly(false)
              load()
              onChanged()
            })
            .finally(() => setBusy(false))
        }}
      >
        <label className="lc-ownmodel__field">
          <span className="lc-fieldlabel">Name</span>
          <input className="lc-input" value={name} maxLength={60} placeholder="Acme Chat" autoComplete="off" onChange={(event) => setName(event.target.value)} />
        </label>
        <label className="lc-ownmodel__field">
          <span className="lc-fieldlabel">Model</span>
          <input
            className="lc-input lc-mono"
            value={model}
            maxLength={200}
            placeholder="acme-70b"
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setModel(event.target.value)}
          />
        </label>
        <label className="lc-ownmodel__field lc-ownmodel__field--wide">
          <span className="lc-fieldlabel">Address</span>
          <input
            className="lc-input lc-mono"
            value={baseUrl}
            maxLength={500}
            placeholder="https://llm.example.com/v1"
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setBaseUrl(event.target.value)}
          />
        </label>
        <label className="lc-ownmodel__field lc-ownmodel__field--wide">
          <span className="lc-fieldlabel">Key</span>
          <input
            className="lc-input lc-mono"
            type="password"
            value={key}
            maxLength={4000}
            placeholder="Leave empty if it asks for none"
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setKey(event.target.value)}
          />
        </label>
        <div className="lc-ownmodel__tools">
          <button
            type="button"
            className={`lc-switch${chatOnly ? '' : ' is-on'}`}
            role="switch"
            aria-checked={!chatOnly}
            aria-label="Uses tools"
            onClick={() => setChatOnly(!chatOnly)}
          >
            <span className="lc-switch__knob" />
          </button>
          <span className="lc-ownmodel__toolstext">
            <span className="lc-ownmodel__name">{chatOnly ? 'Chat only' : 'Uses tools'}</span>
            <span className="lc-field__hint">
              {chatOnly
                ? 'Its teammates talk with it, and it reads and changes no files. For a model that cannot take tools.'
                : 'It may read and change files in the project, as other models do. Test turns this off if it cannot.'}
            </span>
          </span>
        </div>
        <div className="lc-ownmodel__actions">
          <button
            type="button"
            className="lc-button"
            disabled={baseUrl.trim().length === 0 || model.trim().length === 0 || busy || said?.text === OWN_MODEL_TESTING}
            onClick={() => {
              setSaid({ text: OWN_MODEL_TESTING, good: true })
              void window.desktop?.testOwnModel(typed).then((response) => {
                setSaid(response.ok ? { text: response.data.said, good: response.data.reached } : { text: response.error.message, good: false })
                if (response.ok && response.data.tools !== undefined) setChatOnly(!response.data.tools)
              }).catch(() => setSaid({ text: 'That model could not be tested. Nothing was added.', good: false }))
            }}
          >
            Test
          </button>
          <button type="submit" className="lc-primarybutton" disabled={!filled || busy}>
            Add model
          </button>
        </div>
        {said !== undefined && (
          <p className={`lc-ownmodel__said${said.good ? '' : ' lc-tone-amber'}`} role="status">
            {said.text}
          </p>
        )}
      </form>
    </div>
  )
}

function hostOf(baseUrl: string): string {
  try {
    return new URL(baseUrl).host
  } catch {
    return baseUrl
  }
}
