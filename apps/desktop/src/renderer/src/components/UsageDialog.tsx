import { accountName, useAgentAccounts } from '../agentAccounts.js'
import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { UsageReadResponse } from '../../../shared/ipc.js'
import { isMissionRuntime, runtimeDisplayName } from '../../../shared/runtimes.js'
import { totalTokens, USAGE_RANGES } from '../../../shared/usage.js'
import type { UsageRange, UsageSummary } from '../../../shared/usage.js'
import { usageReadLine, usageWindowsOf } from '../missionView.js'
import { modelDisplayName } from '../routeName.js'
import { useModal } from '../useModal.js'
import {
  cachedText,
  compactCount,
  dayLabel,
  heatColumns,
  heatLevel,
  paidBy,
  plural,
  RANGE_NAMES,
  rangePhrase,
  shareOf,
  spentSentence,
  streakText
} from '../usageView.js'
import { RuntimeMark } from './RuntimeMark.js'
import { Icon } from './Icon.js'

/**
 * USAGE, ACROSS EVERY AGENT AND MODEL (0.714).
 *
 * Claude Code's `/usage` and `/stats`, for every agent at once, because a
 * Locust person has six: how close each account is to its limit (`/usage`'s
 * bars, from the readings the agents' marks already show on hover), a
 * twelve-week grid of the days work happened and the numbers under it
 * (`/stats`), and each model by what it did and what paid for it. Ranges are
 * `/stats`'s: the last 7 days, the last 30, all time.
 *
 * Read from the host, which adds up every turn the ledger holds
 * (shared/usage.ts). Only receipts: a runtime that reports no tokens adds
 * none, and the foot says what is not here.
 */
export function UsageDialog({
  readUsage,
  usageWindows,
  onClose,
  now = () => new Date()
}: {
  readonly readUsage: (range: UsageRange) => Promise<UsageReadResponse>
  /** Each agent's latest limit reading, as the host keeps it (runtime id -> reading). */
  readonly usageWindows: ReadonlyMap<string, string>
  readonly onClose: () => void
  readonly now?: () => Date
}): ReactElement {
  const box = useRef<HTMLDivElement>(null)
  useModal(box, onClose, { focusSelf: true })
  const [range, setRange] = useState<UsageRange>('30d')
  const [summaries, setSummaries] = useState<Partial<Record<UsageRange, UsageSummary>>>({})
  const [problem, setProblem] = useState<string>()
  useEffect(() => {
    let live = true
    void readUsage(range)
      .then((answer) => {
        if (!live) return
        if (answer.ok) setSummaries((held) => ({ ...held, [range]: answer.summary }))
        else setProblem(answer.message)
      })
      .catch(() => {
        if (live) setProblem('Usage could not be read. Nothing was changed; close this and open it again.')
      })
    return () => {
      live = false
    }
  }, [range])
  const summary = summaries[range]
  const clock = now()
  useAgentAccounts()
  const limits = [...usageWindows.entries()]
    .map(([runtime, said]) => ({ key: runtime, runtime: runtime.split(':')[0]!, account: runtime.split(':')[1], said, windows: usageWindowsOf(said, clock) }))
    .filter((entry) => entry.windows.length > 0)
  return (
    <div className="lc-scrim">
      <div ref={box} className="lc-dialog lc-usage" role="dialog" aria-modal="true" aria-label="Usage" tabIndex={-1}>
        <div className="lc-dialog__head">
          <span className="lc-dialog__title">Usage</span>
          <span className="lc-dialog__sub">every agent and model, on this machine</span>
          <div className="lc-segmented lc-usage__range" role="radiogroup" aria-label="Which days">
            {USAGE_RANGES.map((value) => (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={range === value}
                className={`lc-button${range === value ? ' is-active' : ''}`}
                onClick={() => setRange(value)}
              >
                {RANGE_NAMES[value]}
              </button>
            ))}
          </div>
          <button type="button" className="lc-dialog__close" aria-label="Close" onClick={onClose}>
            <Icon name="close" size={13} />
          </button>
        </div>
        <div className="lc-dialog__body lc-usage__body">

          {problem !== undefined && summary === undefined ? (
            <p className="lc-usage__empty" role="alert">{problem}</p>
          ) : summary === undefined ? (
            <p className="lc-usage__empty">Adding it up…</p>
          ) : (
            <>
              <section className="lc-usage__section" aria-label="Activity">
                <div className="lc-usage__sectionhead">
                  <h3 className="lc-usage__heading">Activity</h3>
                  <span className="lc-usage__headnote">{rangePhrase(range)}</span>
                </div>
                <div className="lc-usage__activity">
                  <Heatmap summary={summary} />
                  <dl className="lc-usage__stats">
                    <div><dt>Turns</dt><dd>{summary.turns.toLocaleString('en-US')}</dd></div>
                    <div><dt>Conversations</dt><dd>{summary.conversations.toLocaleString('en-US')}</dd></div>
                    <div><dt>Active days</dt><dd>{`${String(summary.activeDays)} of ${String(summary.rangeDays)}`}</dd></div>
                    <div><dt>Busiest day</dt><dd>{summary.busiest === undefined ? 'none' : `${dayLabel(summary.busiest.date)} · ${plural(summary.busiest.turns, 'turn')}`}</dd></div>
                    <div><dt>Current streak</dt><dd>{streakText(summary.streak.current)}</dd></div>
                    <div><dt>Longest streak</dt><dd>{streakText(summary.streak.longest)}</dd></div>
                    <div><dt>Tokens</dt><dd>{totalTokens(summary.tokens) === 0 ? 'none reported' : compactCount(totalTokens(summary.tokens))}</dd></div>
                    <div><dt>From the cache</dt><dd>{cachedText(summary)}</dd></div>
                  </dl>
                </div>
              </section>

              <section className="lc-usage__section" aria-label="Limits">
                <div className="lc-usage__sectionhead">
                  <h3 className="lc-usage__heading">Limits</h3>
                  <span className="lc-usage__headnote">what each account said at its last reading</span>
                </div>
                {limits.length === 0 ? (
                  <p className="lc-usage__empty">No agent has reported a limit yet. Claude Code and Codex report theirs with each turn.</p>
                ) : (
                  <div className="lc-usage__limits">
                    {limits.map(({ key, runtime, account, said, windows }) => (
                      <div className="lc-usage__agent" key={key}>
                        <div className="lc-usage__agentname">
                          <RuntimeMark runtime={runtime} size={15} />
                          <span>{isMissionRuntime(runtime) ? runtimeDisplayName(runtime) : runtime}{account === undefined ? '' : ` \u00b7 ${accountName(account)}`}</span>
                        </div>
                        {/* /usage's own shape: the window and how much is used, the bar, when it resets. */}
                        {windows.map((window) => {
                          const name = window.name.charAt(0).toUpperCase() + window.name.slice(1)
                          if (window.expired === true) {
                            return (
                              <div className="lc-usage__window is-reset" key={window.name}>
                                <span className="lc-usage__windowhead">
                                  <span className="lc-usage__windowname">{name}</span>
                                  <span className="lc-usage__percent">reset since</span>
                                </span>
                                {window.resets !== undefined && <span className="lc-usage__resets">reset {window.resets}</span>}
                              </div>
                            )
                          }
                          const tone = window.percent >= 100 ? ' is-spent' : window.percent >= 80 ? ' is-pressing' : ''
                          return (
                            <div className={`lc-usage__window${tone}`} key={window.name}>
                              <span className="lc-usage__windowhead">
                                <span className="lc-usage__windowname">{name}</span>
                                <span className="lc-usage__percent">{`${String(window.remaining ?? Math.max(0, 100 - window.percent))}% left`}</span>
                              </span>
                              <span className="lc-usage__bar" role="meter" aria-label={`${name} used`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={window.percent}>
                                <span className="lc-usage__fill" style={{ width: `${String(Math.min(100, window.percent))}%` }} />
                              </span>
                              {window.resets !== undefined && <span className="lc-usage__resets">Resets {window.resets}</span>}
                            </div>
                          )
                        })}
                        {usageReadLine(said) !== undefined && <span className="lc-usage__read">{usageReadLine(said)}</span>}
                      </div>
                    ))}
                  </div>
                )}
              </section>
              <section className="lc-usage__section" aria-label="Models">
                <div className="lc-usage__sectionhead">
                  <h3 className="lc-usage__heading">Models</h3>
                  <span className="lc-usage__headnote">most turns first</span>
                </div>
                {summary.models.length === 0 ? (
                  <p className="lc-usage__empty">Nothing ran in {rangePhrase(range)}.</p>
                ) : (
                  <table className="lc-usage__models">
                    <thead>
                      <tr>
                        <th scope="col">Model</th>
                        <th scope="col">Share of turns</th>
                        <th scope="col" className="is-number">Turns</th>
                        <th scope="col" className="is-number">Tokens</th>
                        <th scope="col" className="is-number">Cached</th>
                        <th scope="col">Paid by</th>
                      </tr>
                    </thead>
                    <tbody>
                      {summary.models.map((model) => {
                        const share = shareOf(model, summary)
                        const tokens = totalTokens(model.tokens)
                        return (
                          <tr key={`${model.runtime}:${model.model}`}>
                            <td>
                              <span className="lc-usage__model">
                                <RuntimeMark runtime={model.runtime} size={14} />
                                <span className="lc-usage__modelname" title={model.model}>{modelDisplayName(model.runtime, model.model)}</span>
                                <span className="lc-usage__runtime">{isMissionRuntime(model.runtime) ? runtimeDisplayName(model.runtime) : model.runtime}</span>
                              </span>
                            </td>
                            <td>
                              <span className="lc-usage__share">
                                <span className="lc-usage__sharebar"><span className="lc-usage__sharefill" style={{ width: `${String(share)}%` }} /></span>
                                <span className="lc-usage__sharetext">{`${String(share)}%`}</span>
                              </span>
                            </td>
                            <td className="is-number">{model.turns.toLocaleString('en-US')}</td>
                            <td className="is-number">{tokens === 0 ? '—' : compactCount(tokens)}</td>
                            <td className="is-number">{cachedText(model)}</td>
                            <td>{paidBy(model)}</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                )}
              </section>
            </>
          )}
        </div>
        <div className="lc-dialog__foot">
          <span className="lc-usage__note" title="Locust's own turns, and the terminal sessions it opened or brought in. What you do in an agent's own app is not here.">
            {summary === undefined ? '' : `${spentSentence(summary)} `}
            Counted from Locust's record on this machine.
          </span>
          <button type="button" className="lc-button" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  )
}

/** Twelve weeks, a column a week, Sunday at the top: `/stats`'s grid, in the app's ivory. */
export function Heatmap({ summary }: { readonly summary: UsageSummary }): ReactElement {
  const columns = heatColumns(summary.heatmap)
  const busiest = Math.max(0, ...summary.heatmap.map((day) => day.turns))
  return (
    <div className="lc-heat" role="img" aria-label={`Turns a day over the last twelve weeks; the busiest day had ${plural(busiest, 'turn')}.`}>
      <div className="lc-heat__months" aria-hidden="true">
        {columns.map((column, index) => (
          <span key={`m${String(index)}`} className="lc-heat__month">{column.month ?? ''}</span>
        ))}
      </div>
      <div className="lc-heat__body">
        <div className="lc-heat__weekdays" aria-hidden="true">
          <span />
          <span>Mon</span>
          <span />
          <span>Wed</span>
          <span />
          <span>Fri</span>
          <span />
        </div>
        <div className="lc-heat__grid">
          {columns.map((column, index) => (
            <div className="lc-heat__week" key={`w${String(index)}`}>
              {column.days.map((day, at) =>
                day === undefined ? (
                  <span key={`d${String(at)}`} className="lc-heat__cell is-future" />
                ) : (
                  <span
                    key={day.date}
                    className={`lc-heat__cell is-${String(heatLevel(day.turns, busiest))}${day.date === summary.today ? ' is-today' : ''}`}
                    title={`${dayLabel(day.date)}: ${day.turns === 0 ? 'nothing' : plural(day.turns, 'turn')}`}
                  />
                )
              )}
            </div>
          ))}
        </div>
      </div>
      <div className="lc-heat__legend" aria-hidden="true">
        <span>Less</span>
        {[0, 1, 2, 3, 4].map((level) => (
          <span key={level} className={`lc-heat__cell is-${String(level)}`} />
        ))}
        <span>More</span>
      </div>
    </div>
  )
}
