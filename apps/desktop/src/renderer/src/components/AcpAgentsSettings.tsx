import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { AcpAgent, AcpAgentAnswer, AcpAgentsApi } from '../../../shared/acp-agents.js'
import { ArmedButton } from './ArmedButton.js'

/**
 * YOUR OWN AGENTS, in Settings (0.777, shared/acp-agents.ts).
 *
 * Any program that speaks the Agent Client Protocol, added by the command that starts it. Test opens a session with
 * it and closes it again -- no prompt, so nothing is spent -- and says what it answered: its name, its version, the
 * models it offers. Adding tests first, and keeps only an agent that answered.
 */
export const ACP_AGENT_TESTING = 'Testing… An agent that is starting for the first time can take half a minute.'

/** What a Test found, in one sentence. */
export function acpAnswerSentence(answer: AcpAgentAnswer): string {
  const who = [answer.agentName, answer.version].filter((part): part is string => part !== undefined).join(' ')
  const models = answer.modelOption?.models.length ?? 0
  const offers = models === 0 ? 'It offers no models to pick from, so it runs on its own default.' : `It offers ${String(models)} model${models === 1 ? '' : 's'} to pick from.`
  return `${who.length > 0 ? `${who} answered.` : 'It answered.'} ${offers}`
}

export function AcpAgentsSettings({ onChanged, api = typeof window === 'undefined' ? undefined : window.desktop?.acpAgents }: {
  readonly onChanged: () => void
  readonly api?: AcpAgentsApi
}): ReactElement {
  const [agents, setAgents] = useState<readonly AcpAgent[]>()
  const [name, setName] = useState('')
  const [commandLine, setCommandLine] = useState('')
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<{ readonly text: string; readonly good: boolean }>()
  const load = (): void => {
    void api?.list().then((listed) => { if (listed.ok) setAgents(listed.agents) }).catch(() => undefined)
  }
  useEffect(load, [api])
  const filled = name.trim().length > 0 && commandLine.trim().length > 0

  return (
    <div className="lc-ownmodels" data-acp-agents="">
      {agents !== undefined && agents.length > 0 && (
        <div className="lc-settingrows">
          {agents.map((agent) => (
            <div className="lc-settingrow lc-ownmodel" key={agent.id} data-acp-agent={agent.id}>
              <span className="lc-ownmodel__text">
                <span className="lc-ownmodel__name">{agent.name}</span>
                <span className="lc-ownmodel__meta lc-mono">
                  {agent.commandLine}
                  {agent.answer?.agentName === undefined ? '' : ` · ${agent.answer.agentName}${agent.answer.version === undefined ? '' : ` ${agent.answer.version}`}`}
                  {` · ${String(agent.answer?.modelOption?.models.length ?? 0)} models`}
                </span>
              </span>
              <ArmedButton
                className="lc-button"
                ariaLabel={`Remove ${agent.name}`}
                armedLabel={`Remove ${agent.name}?`}
                onConfirm={() => {
                  void api?.remove(agent.id).then(() => {
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
          if (!filled || busy || api === undefined) return
          setBusy(true)
          setSaid({ text: ACP_AGENT_TESTING, good: true })
          void api
            .save(name.trim(), commandLine.trim())
            .then((saved) => {
              if (!saved.ok) {
                setSaid({ text: saved.message, good: false })
                return
              }
              setSaid({ text: `${name.trim()} is in every teammate's model list, in a group of its own. It runs in Approve each.`, good: true })
              setName('')
              setCommandLine('')
              setAgents(saved.agents)
              onChanged()
            })
            .catch(() => setSaid({ text: 'That agent could not be added. Your list is as it was.', good: false }))
            .finally(() => setBusy(false))
        }}
      >
        <label className="lc-ownmodel__field">
          <span className="lc-fieldlabel">Name</span>
          <input className="lc-input" value={name} maxLength={40} placeholder="Goose" autoComplete="off" onChange={(event) => setName(event.target.value)} />
        </label>
        <label className="lc-ownmodel__field lc-ownmodel__field--wide">
          <span className="lc-fieldlabel">Command</span>
          <input
            className="lc-input lc-mono"
            value={commandLine}
            maxLength={1000}
            placeholder="goose acp"
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setCommandLine(event.target.value)}
          />
        </label>
        <div className="lc-ownmodel__actions">
          <button
            type="button"
            className="lc-button"
            disabled={commandLine.trim().length === 0 || busy || api === undefined}
            onClick={() => {
              setBusy(true)
              setSaid({ text: ACP_AGENT_TESTING, good: true })
              void api!.test(commandLine.trim())
                .then((tested) => setSaid(tested.ok ? { text: acpAnswerSentence(tested.answer), good: true } : { text: tested.message, good: false }))
                .catch(() => setSaid({ text: 'That agent could not be tested. Nothing was added.', good: false }))
                .finally(() => setBusy(false))
            }}
          >
            Test
          </button>
          <button type="submit" className="lc-primarybutton" disabled={!filled || busy}>
            Add agent
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
