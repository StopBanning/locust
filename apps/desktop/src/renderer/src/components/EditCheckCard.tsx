import type { ReactElement } from 'react'

import { Icon } from './Icon.js'
import { WorkingSpark } from './WorkingSpark.js'

/**
 * A3.3: what the person's check said after this turn changed files.
 *
 * Only what is NEW since the last check in the folder is listed; a failure
 * that was already there is said once as the same failure, not re-listed as
 * this teammate's doing. Nothing goes back to the teammate unless the person
 * presses Send (Colin, 2026-09-25: the button, not an automatic loop).
 */
export interface EditCheckShown {
  readonly command: string
  /** `running` from the moment it starts until it says how it ended (0.811); never recorded. */
  readonly outcome: 'passed' | 'failed' | 'timed-out' | 'could-not-run' | 'running'
  readonly newLines: readonly string[]
  readonly unchanged: boolean
  readonly first: boolean
}

/** The check as it starts (0.811): its command, and nothing said about the turn yet. */
export const runningCheck = (command: string): EditCheckShown => ({ command, outcome: 'running', newLines: [], unchanged: false, first: false })

/** What the teammate is sent when the person presses Send. */
export function editCheckFollowUp(check: EditCheckShown): string {
  return [
    `After your last turn, the project's check \`${check.command}\` failed${check.first ? '' : ' with problems it did not have before'}:`,
    '',
    ...check.newLines.map((line) => `    ${line}`),
    '',
    // Not "run the check again": Locust runs it after the turn, and asking
    // the teammate to made it ask permission for a shell command (drive,
    // 2026-09-25).
    'Fix what your change caused and say what you changed. Locust runs the check again after your turn.'
  ].join('\n')
}

export function EditCheckCard({
  check,
  teammateName,
  onSend
}: {
  readonly check: EditCheckShown
  readonly teammateName: string | undefined
  /** Absent while a run is going: a follow-up waits for it. */
  readonly onSend?: (text: string) => void
}): ReactElement {
  const command = <code className="lc-mono">{check.command}</code>
  if (check.outcome === 'running') {
    return (
      <div className="lc-diagnostic lc-tone-muted lc-editcheck" role="status">
        <WorkingSpark />
        <span>Running the check {command} after this turn…</span>
      </div>
    )
  }
  if (check.outcome === 'passed') {
    return (
      <div className="lc-diagnostic lc-tone-muted lc-editcheck">
        <Icon name="shield" size={12} />
        <span>The check {command} passed after this turn.</span>
      </div>
    )
  }
  if (check.outcome === 'timed-out' || check.outcome === 'could-not-run') {
    return (
      <div className="lc-diagnostic lc-tone-amber lc-editcheck">
        <Icon name="shield" size={12} />
        <span>
          {check.outcome === 'timed-out'
            ? <>The check {command} ran past three minutes and was stopped, so it said nothing about this turn.</>
            : <>The check {command} could not be run{check.newLines[0] === undefined ? '' : `: ${check.newLines[0]}`}. Change it in Settings.</>}
        </span>
      </div>
    )
  }
  if (check.unchanged || check.newLines.length === 0) {
    return (
      <div className="lc-diagnostic lc-tone-amber lc-editcheck">
        <Icon name="shield" size={12} />
        <span>The check {command} still fails the same way it did before this turn; nothing in it is new.</span>
      </div>
    )
  }
  return (
    <div className="lc-diagnostic lc-tone-amber lc-editcheck" role="group" aria-label="Check after this turn">
      <Icon name="shield" size={12} />
      <div className="lc-editcheck__body">
        <span>
          The check {command} failed after this turn
          {check.first ? '' : ` with ${String(check.newLines.length)} line${check.newLines.length === 1 ? '' : 's'} that were not there before`}:
        </span>
        <pre className="lc-editcheck__lines lc-mono">{check.newLines.join('\n')}</pre>
        {onSend !== undefined && (
          <button type="button" className="lc-button" onClick={() => onSend(editCheckFollowUp(check))}>
            Send to {teammateName ?? 'the teammate'}
          </button>
        )}
      </div>
    </div>
  )
}
