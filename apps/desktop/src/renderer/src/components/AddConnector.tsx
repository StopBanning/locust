import { useState } from 'react'
import type { ReactElement } from 'react'

import { connectorFrom, CONNECTOR_AGENTS } from '../../../shared/connector-add.js'
import type { ConnectorAgent, ConnectorAgentResult } from '../../../shared/connector-add.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { RuntimeMark } from './RuntimeMark.js'

/**
 * ADD A CONNECTOR, TO EVERY AGENT YOU CHOOSE (0.716; shared/connector-add.ts).
 *
 * Settings > Connectors: a name, how it runs -- a command on this computer or
 * a web address -- and which agents get it. Locust runs each agent's own
 * `mcp add` and says what each answered, in the rows Connectors already
 * draws: added, had one by that name already (kept as it was), or why not.
 * Undo, right after, runs each one's own `mcp remove`. Only agents installed
 * here are offered, and all of them are chosen until one is turned off.
 */

/** A row of what came back; `undo` marks one that Undo's answer replaced. */
type ResultRow = ConnectorAgentResult & { readonly undo?: true }

type RowWords = { readonly state: string; readonly tone: 'good' | 'amber' | 'muted'; readonly detail?: string }

const ROW_WORDS: Readonly<Record<ConnectorAgentResult['outcome'], RowWords>> = {
  added: { state: 'Added', tone: 'good' },
  had: { state: 'Had one', tone: 'muted', detail: 'It already has a connector by this name, and keeps its own.' },
  failed: { state: 'Not added', tone: 'amber' },
  removed: { state: 'Taken back', tone: 'muted' },
  kept: { state: 'Still has it', tone: 'amber' }
}

export function rowWords(row: ResultRow): RowWords {
  const words = ROW_WORDS[row.outcome]
  const state = row.outcome === 'failed' && row.undo === true ? 'Not taken back' : words.state
  const detail = row.said ?? words.detail
  return { state, tone: words.tone, ...(detail === undefined ? {} : { detail }) }
}

export function AddConnector({ installed, onChanged }: { readonly installed: readonly ConnectorAgent[]; readonly onChanged?: () => void }): ReactElement {
  const offered = CONNECTOR_AGENTS.filter((agent) => installed.includes(agent))
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [kind, setKind] = useState<'command' | 'url'>('command')
  const [value, setValue] = useState('')
  // Turned OFF, not on: an agent whose check lands after this opened is chosen too.
  const [left, setLeft] = useState<readonly ConnectorAgent[]>([])
  const [busy, setBusy] = useState<'adding' | 'undoing'>()
  const [problem, setProblem] = useState<string>()
  const [results, setResults] = useState<{ readonly name: string; readonly rows: readonly ResultRow[] }>()

  const picked = offered.filter((agent) => !left.includes(agent))
  const add = (): void => {
    const read = connectorFrom({ name, kind, value })
    if (!read.ok) {
      setProblem(read.problem)
      return
    }
    if (picked.length === 0) {
      setProblem('Choose at least one agent.')
      return
    }
    const bridge = window.desktop
    if (bridge === undefined) return
    setProblem(undefined)
    setBusy('adding')
    void bridge
      .addConnector({ name: read.connector.name, kind, value, agents: picked })
      .then((answer) => {
        if (!answer.ok) {
          setProblem(answer.message)
          return
        }
        setResults({ name: read.connector.name, rows: answer.results })
        if (answer.results.some((one) => one.outcome === 'added')) onChanged?.()
        // All of it done: the form goes, and what each said stays. Anything
        // not done keeps the form as typed, to change and try again.
        if (answer.results.every((one) => one.outcome === 'added' || one.outcome === 'had')) {
          setOpen(false)
          setName('')
          setValue('')
        }
      })
      .catch(() => setProblem('The connector could not be added. Nothing was changed.'))
      .finally(() => setBusy(undefined))
  }
  const undo = (): void => {
    const bridge = window.desktop
    if (bridge === undefined || results === undefined) return
    const given = results.rows.filter((row) => row.outcome === 'added').map((row) => row.agent)
    if (given.length === 0) return
    setProblem(undefined)
    setBusy('undoing')
    void bridge
      .removeConnector(results.name, given)
      .then((answer) => {
        if (!answer.ok) {
          setProblem(answer.message)
          return
        }
        // What came back replaces the added rows; the rest stay as they were said.
        const back = new Map(answer.results.map((one) => [one.agent, one]))
        setResults({
          name: results.name,
          rows: results.rows.map((row) => {
            const now = back.get(row.agent)
            return now === undefined ? row : { ...now, undo: true }
          })
        })
        onChanged?.()
      })
      .catch(() => setProblem('It could not be taken back. Check each agent’s connectors.'))
      .finally(() => setBusy(undefined))
  }

  const addedCount = results?.rows.filter((row) => row.outcome === 'added').length ?? 0
  return (
    <div className="lc-addconnector">
      {!open && (
        <div className="lc-addconnector__start">
          <button
            type="button"
            className="lc-button"
            disabled={offered.length === 0 || busy !== undefined}
            onClick={() => {
              setProblem(undefined)
              setOpen(true)
            }}
          >
            {results === undefined ? 'Add a connector…' : 'Add another…'}
          </button>
          {offered.length === 0 && <span className="lc-settings__note">None of the agents that can take one is installed here.</span>}
        </div>
      )}
      {open && (
        <form
          className="lc-addconnector__form"
          aria-label="Add a connector"
          onSubmit={(event) => {
            event.preventDefault()
            add()
          }}
        >
          <label className="lc-addconnector__field">
            <span className="lc-fieldlabel">Name</span>
            <input
              className="lc-input lc-mono"
              value={name}
              maxLength={64}
              placeholder="rea"
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <div className="lc-addconnector__field">
            <span className="lc-fieldlabel" id="lc-addconnector-runs">
              It runs as
            </span>
            <div className="lc-segmented" role="radiogroup" aria-labelledby="lc-addconnector-runs">
              <button type="button" role="radio" aria-checked={kind === 'command'} className={`lc-button${kind === 'command' ? ' is-active' : ''}`} onClick={() => setKind('command')}>
                A command
              </button>
              <button type="button" role="radio" aria-checked={kind === 'url'} className={`lc-button${kind === 'url' ? ' is-active' : ''}`} onClick={() => setKind('url')}>
                A web address
              </button>
            </div>
          </div>
          <label className="lc-addconnector__field lc-addconnector__field--wide">
            <span className="lc-fieldlabel">{kind === 'command' ? 'Command' : 'Address'}</span>
            <input
              className="lc-input lc-mono"
              value={value}
              maxLength={2000}
              placeholder={kind === 'command' ? 'npx -y rea-agents@6.3.0 mcp' : 'https://example.com/mcp'}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setValue(event.target.value)}
            />
            <span className="lc-field__hint">
              {kind === 'command'
                ? 'The command that starts it, as its instructions give it. Each agent starts it itself, on this computer.'
                : 'Where it answers, starting https://. One that needs a key or a sign-in is added in the agent itself.'}
            </span>
          </label>
          <div className="lc-addconnector__field lc-addconnector__field--wide">
            <span className="lc-fieldlabel" id="lc-addconnector-agents">
              Give it to
            </span>
            <div className="lc-addconnector__agents" role="group" aria-labelledby="lc-addconnector-agents">
              {offered.map((agent) => {
                const on = !left.includes(agent)
                return (
                  <button
                    key={agent}
                    type="button"
                    className={`lc-addconnector__agent${on ? ' is-on' : ''}`}
                    aria-pressed={on}
                    onClick={() => setLeft((held) => (held.includes(agent) ? held.filter((one) => one !== agent) : [...held, agent]))}
                  >
                    <RuntimeMark runtime={agent} size={13} muted={!on} />
                    {runtimeDisplayName(agent)}
                  </button>
                )
              })}
            </div>
            <span className="lc-field__hint">Cursor Agent has no command for this yet: add it in Cursor.</span>
          </div>
          <div className="lc-addconnector__actions">
            <button
              type="button"
              className="lc-button"
              disabled={busy !== undefined}
              onClick={() => {
                setProblem(undefined)
                setOpen(false)
              }}
            >
              Cancel
            </button>
            <button type="submit" className="lc-primarybutton" disabled={busy !== undefined || picked.length === 0}>
              {busy === 'adding' ? 'Adding…' : `Add to ${String(picked.length)} ${picked.length === 1 ? 'agent' : 'agents'}`}
            </button>
          </div>
        </form>
      )}
      {problem !== undefined && (
        <p className="lc-addconnector__problem lc-tone-amber" role="alert">
          {problem}
        </p>
      )}
      {results !== undefined && (
        <div className="lc-addconnector__results" role="status" aria-label={`What each agent said about ${results.name}`}>
          <span className="lc-fieldlabel">{results.name}</span>
          <ul className="lc-connectorhealth__list">
            {results.rows.map((row) => {
              const words = rowWords(row)
              return (
                <li key={row.agent} className="lc-connectorhealth__row" data-outcome={row.outcome}>
                  <span className={`lc-connectorhealth__state lc-connectorhealth__state--${words.tone}`}>{words.state}</span>
                  <span className="lc-addconnector__who">
                    <RuntimeMark runtime={row.agent} size={13} />
                    {runtimeDisplayName(row.agent)}
                  </span>
                  {words.detail !== undefined && <span className="lc-connectorhealth__detail">{words.detail}</span>}
                </li>
              )
            })}
          </ul>
          {addedCount > 0 && (
            <div className="lc-addconnector__after">
              <span className="lc-settings__note">{addedCount === 1 ? 'That agent has it' : 'Those agents have it'} from their next turn.</span>
              <button type="button" className="lc-button" disabled={busy !== undefined} onClick={undo}>
                {busy === 'undoing' ? 'Taking it back…' : 'Undo'}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
