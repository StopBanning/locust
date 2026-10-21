import type { ReactElement } from 'react'

import type { RerunProgress } from '../../../shared/compare.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import { acrossBenchmarks, benchmarkLeader, type AcrossRow, type BenchmarkRow } from '../benchmark.js'
import { BOT_SIZE } from '../botSizes.js'
import { costCell } from '../cost.js'
import { durationText } from '../missionView.js'
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

/**
 * ACROSS YOUR BENCHMARKS (0.811, benchmark.ts acrossBenchmarks): the models that ran in two or more, one row each, in
 * the columns a benchmark's own table has -- read once, not five times.
 */
export function AcrossBenchmarks({ rows }: { readonly rows: readonly AcrossRow[] }): ReactElement | null {
  if (rows.length === 0) return null
  const scores = rows.flatMap((row) => (row.score === undefined ? [] : [row.score]))
  const best = Math.max(...scores)
  const alone = scores.filter((score) => score === best).length === 1
  const checked = rows.some((row) => row.checked > 0)
  return (
    <div className="lc-benchmarks__across lc-settingcard" aria-label="Across your benchmarks">
      <span className="lc-compare__recordlabel lc-mono">ACROSS YOUR BENCHMARKS</span>
      <div className="lc-tablewrap">
        <table className="lc-table lc-benchmark__table">
          <thead>
            <tr>
              <th scope="col">Model</th>
              <th scope="col">Benchmarks</th>
              <th scope="col">Score</th>
              <th scope="col">Met every point</th>
              {checked && <th scope="col">Check passed</th>}
              <th scope="col">Time a task</th>
              <th scope="col">Cost a task</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key}>
                <th scope="row" className="lc-benchmark__model">{row.name}</th>
                <td className="lc-compare__recordnum">{String(row.benchmarks)}</td>
                <td className={`lc-compare__recordnum lc-benchmark__score${alone && row.score === best ? ' is-best' : ''}`}>
                  {row.score === undefined ? 'not scored' : String(row.score)}
                </td>
                <td className="lc-compare__recordnum">{row.scored === 0 ? '—' : `${String(row.everyPoint)} of ${String(row.scored)}`}</td>
                {checked && (
                  <td className={`lc-compare__recordnum${row.checked === 0 ? '' : row.checkPassed === row.checked ? ' lc-tone-green' : ' lc-tone-amber'}`}>
                    {row.checked === 0 ? '—' : `${String(row.checkPassed)} of ${String(row.checked)}`}
                  </td>
                )}
                <td className="lc-compare__recordnum">{row.spanMs === undefined ? '—' : durationText(row.spanMs)}</td>
                <td className="lc-compare__recordnum">{/-free$/i.test(row.model) && (row.cost?.usd ?? 0) === 0 ? 'free' : costCell(row.cost, row.model)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="lc-compare__judgenote">
        {`A score here is the average of a model's scores in the benchmarks it ran in, each benchmark counted once, so a question run fifteen times does not outweigh one run three. Met every point${checked ? ' and Check passed count' : ' counts'} its runs in all of them. A model in fewer benchmarks was asked fewer questions.`}
      </p>
    </div>
  )
}

export function BenchmarksScreen({ entries, onOpen }: { readonly entries: readonly BenchmarkEntry[]; readonly onOpen: (compareId: string) => void }): ReactElement {
  const now = new Date()
  const ordered = orderedBenchmarks(entries)
  const runs = entries.reduce((sum, entry) => sum + entry.runs, 0)
  const across = acrossBenchmarks(entries)
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
        <>
          <AcrossBenchmarks rows={across} />
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
                      {`${entry.progress.of === 1 ? 'running once more' : `running again, ${String(entry.progress.run)} of ${String(entry.progress.of)}`} · ${entry.progress.step === 'checking' ? 'waiting for checks' : entry.progress.step === 'judging' ? 'judging' : 'answering'}`}
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
        </>
      )}
    </section>
  )
}
