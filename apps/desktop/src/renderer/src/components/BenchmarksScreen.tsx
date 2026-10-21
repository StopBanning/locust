import type { ReactElement } from 'react'

import type { RerunProgress } from '../../../shared/compare.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import { benchmarkLeader, type BenchmarkRow } from '../benchmark.js'
import { BOT_SIZE } from '../botSizes.js'
import { shortAgo } from '../railFlyout.js'
import { Icon } from './Icon.js'
import { ScreenHeader } from './Screens.js'
import { TeammateBot } from './TeammateBot.js'
import { WorkingSpark } from './WorkingSpark.js'

/**
 * YOUR BENCHMARKS, IN ONE PLACE (0.764). A benchmark is a comparison its judge scored, run again (0.763,
 * main/compare-reruns.ts) -- and it lived only inside that comparison, so a person with three of them had to remember
 * which conversation each began in. This is the list: each one's question, its models, how many runs, who leads, and
 * when it last ran, the row opening it. It shows up in the sidebar once there is one, as Cloud tasks does.
 */
export interface BenchmarkEntry {
  /** The comparison it runs again: the row opens it. */
  readonly compareId: string
  readonly question: string
  readonly owner?: PublicTeammate
  readonly models: readonly string[]
  /** Every run, the first one too. */
  readonly runs: number
  readonly judgeName?: string
  /** When its newest run began. */
  readonly lastAt: string
  readonly rows: readonly BenchmarkRow[]
  /** Where its runs again are, while they run. */
  readonly progress?: RerunProgress
  readonly changes: boolean
}

/** Running ones first, then the newest. */
export function orderedBenchmarks(entries: readonly BenchmarkEntry[]): readonly BenchmarkEntry[] {
  return [...entries].sort((first, second) => {
    const running = (entry: BenchmarkEntry): number => (entry.progress === undefined ? 1 : 0)
    return running(first) - running(second) || Date.parse(second.lastAt) - Date.parse(first.lastAt)
  })
}

/** "Space Bunny vs Muse Spark", or "3 models" past three: the meta line is one line. */
export function benchmarkModelsLine(models: readonly string[]): string {
  return models.length <= 3 ? models.join(' vs ') : `${String(models.length)} models`
}

/** Who leads, as the chip says it: one name, a tie of two by name, a tie of more by count. */
export function leaderParts(leader: { readonly names: readonly string[] }): { readonly who: string; readonly tie: boolean } {
  if (leader.names.length === 1) return { who: leader.names[0]!, tie: false }
  if (leader.names.length === 2) return { who: `${leader.names[0]!} and ${leader.names[1]!}`, tie: true }
  return { who: `${String(leader.names.length)} models`, tie: true }
}

export function leaderWords(leader: { readonly names: readonly string[] }): string {
  const { who, tie } = leaderParts(leader)
  return tie ? `${who} tie` : who
}

export function BenchmarksScreen({ entries, onOpen }: { readonly entries: readonly BenchmarkEntry[]; readonly onOpen: (compareId: string) => void }): ReactElement {
  const now = new Date()
  const ordered = orderedBenchmarks(entries)
  const runs = entries.reduce((sum, entry) => sum + entry.runs, 0)
  return (
    <section className="lc-screen lc-benchmarks" aria-label="Benchmarks">
      <ScreenHeader
        title="Benchmarks"
        meta={entries.length === 0 ? 'none yet' : `${entries.length === 1 ? '1 benchmark' : `${String(entries.length)} benchmarks`} · ${String(runs)} runs`}
      />
      {entries.length === 0 ? (
        <div className="lc-empty">
          <p>
            A benchmark is a comparison its judge scored, run again: the same models on the same question, scored on the
            same points, so one lucky run does not decide which model a teammate uses.
          </p>
          <p className="lc-empty__how">Compare two models, ask a judge to score them, then press Run 3 more times.</p>
        </div>
      ) : (
        <div className="lc-benchmarks__list lc-settingcard">
          {ordered.map((entry) => {
            const leader = benchmarkLeader(entry.rows)
            const ago = shortAgo(entry.lastAt, now)
            return (
              <button key={entry.compareId} type="button" className="lc-benchmarks__row" onClick={() => onOpen(entry.compareId)}>
                <span className="lc-benchmarks__face">
                  {entry.owner === undefined ? (
                    <Icon name="target" size={14} />
                  ) : (
                    <TeammateBot hue={entry.owner.hue} avatar={entry.owner.avatar} size={BOT_SIZE.routineRow} teammateId={entry.owner.teammateId} />
                  )}
                </span>
                <span className="lc-benchmarks__name">
                  <span className="lc-benchmarks__question" title={entry.question}>{entry.question}</span>
                  <span className="lc-benchmarks__meta lc-mono">
                    {`${benchmarkModelsLine(entry.models)} · ${String(entry.runs)} runs${entry.judgeName === undefined ? '' : ` · judged by ${entry.judgeName}`}${entry.changes ? ' · changes files' : ''}`}
                  </span>
                </span>
                {entry.progress !== undefined ? (
                  <span className="lc-benchmarks__chip is-running lc-mono" role="status">
                    <WorkingSpark />
                    {`run ${String(entry.progress.run)} of ${String(entry.progress.of)} · ${entry.progress.step === 'checking' ? 'waiting for checks' : entry.progress.step === 'judging' ? 'judging' : 'answering'}`}
                  </span>
                ) : leader === undefined ? (
                  <span className="lc-benchmarks__chip lc-mono">not scored yet</span>
                ) : (
                  <span className="lc-benchmarks__chip" title={`${leader.names.join(', ')}: the best score, out of 100, averaged over the runs the judge scored`}>
                    {/* The names may be cut short; "tie" never is -- a cut tie read as one model leading (the 0.764 drive's frame). */}
                    <span className="lc-benchmarks__who">
                      <span className="lc-benchmarks__leader">{leaderParts(leader).who}</span>
                      {leaderParts(leader).tie && <span className="lc-benchmarks__tie">tie</span>}
                    </span>
                    <span className="lc-benchmarks__meter" aria-hidden="true">
                      <span className="lc-benchmarks__fill" style={{ width: `${String(leader.score)}%` }} />
                    </span>
                    <span className="lc-benchmarks__score lc-mono">{String(leader.score)}</span>
                  </span>
                )}
                <span className="lc-benchmarks__at lc-mono">{ago ?? ''}</span>
                <Icon name="chevron-right" size={14} />
              </button>
            )
          })}
        </div>
      )}
    </section>
  )
}
