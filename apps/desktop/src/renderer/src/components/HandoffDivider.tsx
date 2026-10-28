import type { ReactElement } from 'react'

import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { leftOutInWords } from '../handoffPreview.js'
import { modelDisplayName } from '../routeName.js'
import { RuntimeMark } from './RuntimeMark.js'

const RUNTIME_LABEL: Readonly<Record<string, string>> = {
  codex: 'Codex',
  claude: 'Claude Code',
  omniroute: 'OmniRoute'
}

/**
 * A side of the seam by the model it ran (0.779), as a past turn is named -- its own id spelled, never today's
 * meaning of an alias. The account's own default, or no model recorded, is the runtime's name.
 */
function sideName(runtime: MissionRuntimeId, model: string | undefined, runtimeLabel: string): string {
  if (model === undefined || model.trim().length === 0 || model === 'account-default') return runtimeLabel
  return modelDisplayName(runtime, model)
}

/**
 * The seam between two runtimes working the same problem.
 *
 * This is the one place the thread shows a boundary the durable record insists
 * on: a mission holds ONE runtime, so a switch is genuinely two missions with a
 * reconciliation between them. The divider is where that fact stops being an
 * implementation detail and becomes the honest answer to "who did what".
 *
 * It states the number of actions left in doubt rather than a reassuring
 * summary, because that is the number a person needs in order to decide how
 * much to trust what comes after the line.
 */
export function HandoffDivider({
  from,
  to,
  at,
  unsettledCount,
  omittedBriefing,
  leftOutByYou = [],
  carriedOn,
  carriedFor,
  fromModel,
  toModel
}: {
  readonly from: MissionRuntimeId
  readonly to: MissionRuntimeId
  /**
   * The models either side (0.779). Colin, 2026-10-16, over another app's "Context handoff, GPT-6.1-Sol -> Claude
   * Opus 5.5" line: "would be cool to have our ui do something similar to this w/ the logos when handoffs happen".
   */
  readonly fromModel?: string
  readonly toModel?: string
  readonly at: string | undefined
  readonly unsettledCount: number
  readonly omittedBriefing: readonly string[]
  /** What the person chose to leave out of it (0.527): their choice, so said plainly, not as a warning. */
  readonly leftOutByYou?: readonly string[]
  /** Locust made the switch because `from`'s plan ran out (0.774): said first, since nobody typed the turn below. */
  readonly carriedOn?: true
  /** And it was `from`'s provider failing before it did anything, not its plan (0.839). */
  readonly carriedFor?: 'provider'
}): ReactElement {
  // Every runtime by its name. Only three were listed, so a switch to
  // OpenCode read "Codex -> opencode" (drive-runtime-switch, packaged 0.309).
  const fromLabel = RUNTIME_LABEL[from] ?? runtimeDisplayName(from) ?? from
  const toLabel = RUNTIME_LABEL[to] ?? runtimeDisplayName(to) ?? to
  const fromName = sideName(from, fromModel, fromLabel)
  const toName = sideName(to, toModel, toLabel)
  // Said whole for a screen reader: the model, and the runtime it ran on where that is not already its name.
  const spoken = (name: string, label: string): string => (name === label ? label : `${name} on ${label}`)

  return (
    <div className="lc-handoff" role="separator" aria-label={`Handed off from ${spoken(fromName, fromLabel)} to ${spoken(toName, toLabel)}`}>
      <span className="lc-handoff__rule lc-handoff__rule--start" />
      <div className="lc-handoff__body">
        <span className="lc-handoff__title">
          <span className="lc-handoff__label">Handed off</span>
          <span className="lc-handoff__side lc-handoff__from" title={fromName === fromLabel ? undefined : fromLabel}>
            <RuntimeMark runtime={from} size={13} />
            {fromName}
          </span>
          <span className="lc-handoff__arrow" aria-hidden="true">
            →
          </span>
          <span className="lc-handoff__side lc-handoff__to" data-runtime={to} title={toName === toLabel ? undefined : toLabel}>
            <RuntimeMark runtime={to} size={13} />
            {toName}
          </span>
          {at !== undefined && <span className="lc-handoff__time">· {at}</span>}
        </span>
        {carriedOn === true && (
          <span className="lc-handoff__note">
            {carriedFor === 'provider'
              ? `${fromLabel}’s provider failed before it did anything, so Locust carried the work on with ${toLabel}.`
              : `${fromLabel} ran out of its plan, so Locust carried the work on with ${toLabel}.`}
          </span>
        )}
        <span className="lc-handoff__note">
          {unsettledCount === 0
            ? `Nothing was in flight, so ${toLabel} picked up from a clean stop.`
            : `${unsettledCount} action${unsettledCount === 1 ? '' : 's'} had started and never reported back. ${toLabel} was told to check ${unsettledCount === 1 ? 'it' : 'them'} before building on ${unsettledCount === 1 ? 'it' : 'them'}.`}
        </span>
        {omittedBriefing.length > 0 && (
          /*
            Said out loud rather than hidden: the next runtime is working from
            less than the whole story, and only the person can tell whether the
            missing part mattered.
          */
          <span className="lc-handoff__note lc-tone-amber">
            {`Left out of the summary to fit: ${leftOutInWords(omittedBriefing)}.`}
          </span>
        )}
        {leftOutByYou.length > 0 && (
          <span className="lc-handoff__note">{`You left out ${leftOutInWords(leftOutByYou)}.`}</span>
        )}
      </div>
      <span className="lc-handoff__rule lc-handoff__rule--end" />
    </div>
  )
}
