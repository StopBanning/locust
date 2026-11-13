import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import { ruleSentence, whatIf, whatIfSentence } from '../../../shared/approval-rules.js'
import type { PublicApprovalRule } from '../../../shared/ipc.js'

/**
 * SAVED APPROVALS, in Settings > Teammates (0.521, shared/approval-rules.ts).
 * Every rule a card's "Yes, and don't ask again" or "Never allow this"
 * saved, as the sentence it is, with how often it answered a card, and
 * Remove. A rule that outlives the run is a Settings decision: this is
 * where it is seen and undone.
 */
export function SavedApprovalRules(): ReactElement {
  const [rules, setRules] = useState<readonly PublicApprovalRule[]>()
  const [names, setNames] = useState<Readonly<Record<string, string>>>({})
  const [problem, setProblem] = useState<string>()
  const [removing, setRemoving] = useState<string>()
  const [tried, setTried] = useState('')
  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge.listApprovalRules().then((answer) => {
      if (answer.ok) setRules(answer.rules)
      else setProblem(answer.message)
    }).catch(() => setProblem('The saved rules could not be read. Every card asks until they can.'))
    void bridge.listTeammates().then((answer) => {
      if (answer.ok) setNames(Object.fromEntries(answer.data.teammates.map((teammate) => [teammate.teammateId, teammate.name])))
    }).catch(() => undefined)
  }, [])
  const remove = (ruleId: string): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setRemoving(ruleId)
    void bridge.removeApprovalRule(ruleId).then((answer) => {
      if (answer.ok) setRules(answer.rules)
      else setProblem(answer.message)
    }).catch(() => setProblem('That rule could not be removed. It still answers cards; try again.')).finally(() => setRemoving(undefined))
  }
  /*
   * REMOVE ALL (0.587, QA's Q7). Locust's stance is that no rules means ask,
   * so the reset is "remove all": two presses, the second naming how many go
   * and that every card asks again. The host clears the file through its own
   * channel; nothing else can.
   */
  const [confirmingAll, setConfirmingAll] = useState(false)
  const [removingAll, setRemovingAll] = useState(false)
  const removeAll = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setRemovingAll(true)
    void bridge.removeAllApprovalRules().then((answer) => {
      if (answer.ok) setRules(answer.rules)
      else setProblem(answer.message)
    }).catch(() => setProblem('The rules could not be removed. They still answer cards; try again.')).finally(() => {
      setRemovingAll(false)
      setConfirmingAll(false)
    })
  }
  return (
    <>
      <p className="lc-settings__lede">
        What a teammate may do without asking, or may never do, as you said on an approval card. Each
        holds for that teammate in that folder.
      </p>
      <p className="lc-settings__note">
        A rule answers what a teammate asks you about. Some AI agents run what they judge safe without
        asking, and a rule cannot stop that: for a hard stop, use Ask mode.
      </p>
      {problem !== undefined && <p className="lc-settings__note" role="status">{problem}</p>}
      {rules !== undefined && rules.length === 0 && (
        <p className="lc-settings__note">
          None yet. On an approval card, &ldquo;Yes, and don&rsquo;t ask again&rdquo; saves one, and so does &ldquo;Never allow this&rdquo; beside Deny.
        </p>
      )}
      {rules !== undefined && rules.length > 0 && (
        <div className="lc-settingcard">
          {rules.map((rule) => (
            <div className="lc-policyrow" key={rule.ruleId}>
              <span className={`lc-tag ${rule.effect === 'allow' ? 'lc-tone-lime' : 'lc-tone-red'}`}>{rule.effect === 'allow' ? 'ALLOW' : 'NEVER'}</span>
              <span className="lc-settings__note lc-savedrule__text">
                {ruleSentence(rule, rule.teammateId === undefined ? undefined : names[rule.teammateId])}
                {rule.uses !== undefined && rule.uses > 0 && <span className="lc-savedrule__uses">{` Answered ${String(rule.uses)} ${rule.uses === 1 ? 'card' : 'cards'}.`}</span>}
              </span>
              <button type="button" className="lc-ghostbutton" disabled={removing === rule.ruleId} onClick={() => remove(rule.ruleId)}>
                {removing === rule.ruleId ? 'Removing…' : 'Remove'}
              </button>
            </div>
          ))}
          <RemoveAllRules count={rules.length} confirming={confirmingAll} removing={removingAll} onAsk={() => setConfirmingAll(true)} onConfirm={removeAll} onKeep={() => setConfirmingAll(false)} />
        </div>
      )}
      {rules !== undefined && rules.length > 0 && <TryARule rules={rules} names={names} command={tried} onCommand={setTried} />}
    </>
  )
}

/**
 * The row under the list that takes every rule back (0.587): one press asks,
 * the second names how many go and that every card asks again afterwards.
 */
export function RemoveAllRules({
  count,
  confirming,
  removing,
  onAsk,
  onConfirm,
  onKeep
}: {
  readonly count: number
  readonly confirming: boolean
  readonly removing: boolean
  readonly onAsk: () => void
  readonly onConfirm: () => void
  readonly onKeep: () => void
}): ReactElement {
  return (
    <div className="lc-policyrow lc-savedrules__all">
      {confirming ? (
        <>
          <span className="lc-settings__note lc-savedrule__text">
            Remove {count === 1 ? 'the 1 saved rule' : `all ${String(count)} saved rules`}? Every card asks again afterwards.
          </span>
          <button type="button" className="lc-denybutton" disabled={removing} onClick={onConfirm}>
            {removing ? 'Removing…' : 'Remove all'}
          </button>
          <button type="button" className="lc-ghostbutton" disabled={removing} onClick={onKeep}>
            Keep them
          </button>
        </>
      ) : (
        <>
          <span className="lc-settings__note lc-savedrule__text">Start over: take every rule back at once.</span>
          <button type="button" className="lc-ghostbutton" onClick={onAsk}>
            Remove all rules
          </button>
        </>
      )}
    </div>
  )
}

/**
 * WHAT WOULD HAPPEN IF (0.874): a command typed here is tried against the saved rules in every place they speak to,
 * and each place says what a card would do and which rule decides (shared/approval-rules.ts `whatIf`). Nothing runs.
 */
export function TryARule({
  rules,
  names,
  command,
  onCommand
}: {
  readonly rules: readonly PublicApprovalRule[]
  readonly names: Readonly<Record<string, string>>
  readonly command: string
  readonly onCommand: (command: string) => void
}): ReactElement {
  const lines = whatIf(command, rules)
  return (
    <div className="lc-settingcard lc-ruletry">
      <label className="lc-settings__note" htmlFor="lc-ruletry-command">
        What would happen if a teammate asked to run:
      </label>
      <input
        id="lc-ruletry-command"
        className="lc-input lc-mono"
        value={command}
        maxLength={500}
        placeholder="npm run test -- --watch"
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => onCommand(event.target.value)}
      />
      {lines.length > 0 && (
        <ul className="lc-ruletry__answers" aria-live="polite">
          {lines.map((line) => (
            <li key={`${line.teammateId ?? ''}:${line.folder ?? ''}`} className={`lc-settings__note${line.verdict.decision === 'deny' ? ' lc-ruletry__deny' : ''}`}>
              {whatIfSentence(line, line.teammateId === undefined ? undefined : names[line.teammateId])}
            </li>
          ))}
        </ul>
      )}
      <p className="lc-settings__note">Nothing runs here. It asks the saved rules the same question a card would.</p>
    </div>
  )
}
