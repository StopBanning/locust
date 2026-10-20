import { useCallback, useLayoutEffect, useRef, useState, type CSSProperties, type ReactElement } from 'react'

import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { CompareSlotId, PublicCompare } from '../../../shared/compare.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import { activityEntries } from '../missionView.js'
import type { ThreadItem } from '../missionView.js'
import { InComparisonCell, PinnedPagesContext } from '../pinnedPages.js'
import { PagePreview } from './DocPreview.js'
import { ApprovalCard } from './ApprovalCard.js'
import type { MissionApprovalDecision, MissionApprovalRequest, MissionMode } from '../../../shared/ipc.js'
import { Icon } from './Icon.js'
import { Select } from './Select.js'
import type { SelectOption } from './Select.js'
import { RuntimeMark } from './RuntimeMark.js'
import { ThreadItems } from './Thread.js'
import { WorkingSpark } from './WorkingSpark.js'
import type { CompareRecordRow } from '../compareRecord.js'
import type { RunCost } from '../cost.js'
import { CompareBenchmark, type BenchmarkView } from './CompareBenchmark.js'
import { scoreOf } from '../../../shared/compare-score.js'
import type { Scorecard } from '../../../shared/compare-score.js'
import { keptSentence } from '../status.js'
import { KeptInstructions } from './KeptInstructions.js'

/**
 * COMPARE, SIDE BY SIDE (0.441, shared/compare.ts,
 * docs/PLAN-2026-09-28-COMPARE.md).
 *
 * One column per model, in place of the thread. Each ask runs across the
 * whole width, and the answers to it sit side by side beneath it, so a
 * follow-up lines up with the answers it follows. The column heads stay at
 * the top; the feet say how long each took and what it cost, and hold the
 * one control that decides: Keep this one.
 */
export interface CompareColumnView {
  readonly slot: CompareSlotId
  readonly name: string
  readonly runtime: MissionRuntimeId
  readonly runtimeName: string
  /** It answers in a copy of the folder, because it cannot be held read-only here (0.443). */
  readonly copy?: true
  /** Why it could not start, when it could not. */
  readonly refused?: string
  /** Where it works, when not the person's folder (0.555): its files are read there. */
  readonly folder?: string
  /** One cell per ask, oldest first; a column that could not take an ask has none for it. */
  readonly turns: readonly { readonly missionId: string; readonly items: readonly ThreadItem[]; readonly running: boolean }[]
  readonly running: boolean
  /** Its newest answer finished: a failed or empty column has nothing to keep. */
  readonly keepable: boolean
  /** Its newest ask did not get an answer (failed, stopped, or never started): it can be asked again. */
  readonly retryable: boolean
  /** Its newest answer's words, for Copy; empty when there are none. */
  readonly answer: string
  /** "done", "working", "stopped", "failed" -- what the head says. */
  readonly state: string
  /** "41s", summed over its turns, when known. */
  readonly span?: string
  /** The same, in milliseconds, for a benchmark's time a task (0.763). */
  readonly spanMs?: number
  /** What it cost, as its runtime reported it. */
  readonly cost?: string
  /** The same, as the receipt has it, for a benchmark's cost a task (0.763). */
  readonly runCost?: RunCost
  /**
   * What its run is waiting on the person for (0.451). The 0.450 three-way
   * comparison sat twenty minutes on "Using a tool..." while four approvals
   * waited under the title bar's "4 need you" -- a column never drew its card.
   */
  readonly approvals?: readonly MissionApprovalRequest[]
}

/** The web page a turn created, if it made one: what its column shows running at the top (0.450). */
const NO_PAGES: ReadonlySet<string> = new Set()

export function builtPageOf(items: readonly ThreadItem[], workspacePath: string | undefined): string | undefined {
  for (const item of items) {
    if (item.type !== 'activity') continue
    for (const entry of activityEntries(item.details, workspacePath)) {
      if (entry.kind === 'file' && entry.file.status === 'ADDED' && /\.html?$/i.test(entry.file.path)) return entry.file.path
    }
  }
  return undefined
}

/**
 * WHERE A COLUMN'S WORDS HOLD (2026-10-05). The columns share one scroll, so
 * a short answer used to leave its column empty for the whole length of a
 * long one: Colin's three-way compare showed Model A blank beside Model B's
 * fortieth line. Each cell's body is sticky at this offset from the top of
 * the scroll: under the heads, when it fits; otherwise far enough up that it
 * stops with its end at the bottom edge, so its last words stay in view while
 * a longer column goes on. In pixels; `head` is the sticky heads' height.
 */
export function stickAt(head: number, port: number, body: number): number {
  const GAP = 8
  return Math.round(Math.min(head + GAP, port - body - GAP))
}

export function CompareView({
  compare,
  changeLines,
  prompts,
  columns,
  onOpenFile,
  owner,
  workspacePath,
  keeping,
  conversationMode,
  retrying,
  problem,
  onKeep,
  onRetry,
  onBack,
  onUseInstructions,
  decidingIds,
  onDecide,
  onAnswer,
  record,
  judge,
  judgeChoices = [],
  judging = false,
  onJudge,
  onDraftPoints,
  addChoices = [],
  adding = false,
  onAddModel,
  benchmark,
  backLabel = 'Back to the conversation'
}: {
  readonly compare: PublicCompare
  /** In a comparison that edits (0.445): what each column has changed, "+12 -3 in 2 files". */
  readonly changeLines: Partial<Record<CompareSlotId, string>>
  /** Each ask, oldest first. */
  readonly prompts: readonly string[]
  readonly columns: readonly CompareColumnView[]
  readonly onOpenFile?: (path: string, folder?: string) => void
  readonly owner: PublicTeammate | undefined
  readonly workspacePath: string | undefined
  /** A keep is under way: no second one. */
  readonly keeping: boolean
  /** The mode the conversation is in (the chat box's own): what the kept sentence says it carries on in. */
  readonly conversationMode?: MissionMode
  /** The column being asked again, while it is being started. */
  readonly retrying: CompareSlotId | undefined
  /** What the last keep could not do, said in the bar. */
  readonly problem?: string
  readonly onKeep: (slot: CompareSlotId) => void
  readonly onRetry: (slot: CompareSlotId) => void
  /** Answering a column's approval, as the thread answers one. */
  readonly decidingIds?: readonly string[]
  readonly onDecide?: (approvalId: string, decision: MissionApprovalDecision, reason?: string) => void
  readonly onAnswer?: (approvalId: string, answers: Readonly<Record<string, readonly string[]>>) => void
  /** Kept already: back to the conversation it carries on in. */
  readonly onBack: (() => void) | undefined
  readonly onUseInstructions?: (teammate: PublicTeammate, instructions: string) => Promise<string | undefined>
  /** Once one is kept: every model's record across your decided comparisons (0.519). */
  readonly record?: readonly CompareRecordRow[]
  /** The judge's view, when one was asked (0.520): its model, and what it said or is saying. */
  readonly judge?: JudgeView
  /** The models a judge can be, the first the one offered (0.520). */
  readonly judgeChoices?: readonly ModelChoice[]
  /** A judge is being asked. */
  readonly judging?: boolean
  readonly onJudge?: (choiceKey: string, criteria: string, others?: readonly string[]) => void
  /** Draft the points from the question (0.766): the points, or why there are none. */
  readonly onDraftPoints?: (choiceKey: string) => Promise<readonly string[] | string>
  /** Models not yet in it, that the same question can be put to (0.523). */
  readonly addChoices?: readonly ModelChoice[]
  readonly adding?: boolean
  readonly onAddModel?: (choiceKey: string) => void
  /** Your own benchmark (0.763): this comparison's runs again, read as one table. */
  readonly benchmark?: BenchmarkView
  /** What the back button says: a run again goes back to the comparison it runs again (0.763). */
  readonly backLabel?: string
}): ReactElement {
  const kept = compare.kept?.slot
  // A blind comparison hides what would name the model -- its mark, its runtime, its cost -- until one is kept (0.449).
  const veiled = compare.blind === true && kept === undefined
  const keptName = columns.find((column) => column.slot === kept)?.name
  /*
   * FOCUS (0.444, Arena's Expand): one column wide, the others narrowed to
   * rails that still say who they are and how they are doing. A rail's head
   * focuses it; the wide column's own button shows them all again.
   */
  const [focusedSlot, setFocusedSlot] = useState<CompareSlotId | undefined>(undefined)
  const focused = columns.some((column) => column.slot === focusedSlot) ? focusedSlot : undefined
  const railed = (slot: CompareSlotId): boolean => focused !== undefined && slot !== focused
  const [copied, setCopied] = useState<CompareSlotId | undefined>(undefined)
  /*
   * WHERE EACH COLUMN'S BODY HOLDS, KEPT CURRENT (0.632).
   *
   * One ResizeObserver for as long as the scroll is on screen, watching the
   * scroll, the heads and every cell's body. A running column grows with each
   * event -- that is a body resizing, which it hears -- and so is a window
   * resize. A draw only hands it a head or body it has not seen, and lets go
   * of one that has left. It measures in its own callback, at most once a
   * frame, reading every size before writing any offset, and writes only an
   * offset that changed. It was a new observer and a forced layout on every
   * draw, which in a streaming comparison is every batch of every column.
   */
  const stick = useRef<{ readonly port: HTMLDivElement; readonly observer: ResizeObserver; readonly watched: Set<Element> } | undefined>(undefined)
  const scrollRef = useCallback((port: HTMLDivElement | null) => {
    stick.current?.observer.disconnect()
    stick.current = undefined
    if (port === null || typeof ResizeObserver === 'undefined') return
    const place = (): void => {
      const head = port.querySelector('.lc-compare__head')?.getBoundingClientRect().height ?? 0
      const height = port.clientHeight
      const bodies = [...port.querySelectorAll<HTMLElement>('.lc-compare__cellbody')]
      const offsets = bodies.map((body) => `${String(stickAt(head, height, body.offsetHeight))}px`)
      bodies.forEach((body, index) => {
        const offset = offsets[index]
        if (offset !== undefined && body.style.getPropertyValue('--lc-compare-stick') !== offset) body.style.setProperty('--lc-compare-stick', offset)
      })
    }
    const observer = new ResizeObserver(place)
    observer.observe(port)
    stick.current = { port, observer, watched: new Set<Element>([port]) }
  }, [])
  useLayoutEffect(() => {
    const held = stick.current
    if (held === undefined) return
    for (const element of [...held.watched]) {
      if (element === held.port || element.isConnected) continue
      held.observer.unobserve(element)
      held.watched.delete(element)
    }
    for (const element of held.port.querySelectorAll('.lc-compare__head, .lc-compare__cellbody')) {
      if (held.watched.has(element)) continue
      held.watched.add(element)
      held.observer.observe(element)
    }
  })
  const copy = (column: CompareColumnView): void => {
    const done = (): void => {
      setCopied(column.slot)
      window.setTimeout(() => setCopied((now) => (now === column.slot ? undefined : now)), 1_600)
    }
    // Same fallback as the shell output's Copy: `navigator.clipboard` is not always there for a packaged page.
    navigator.clipboard?.writeText(column.answer).then(done).catch(() => {
      const field = document.createElement('textarea')
      field.value = column.answer
      document.body.appendChild(field)
      field.select()
      try {
        document.execCommand('copy')
        done()
      } finally {
        field.remove()
      }
    })
  }
  const style = {
    '--lc-compare-columns': String(columns.length),
    ...(focused === undefined ? {} : { '--lc-compare-template': columns.map((column) => (column.slot === focused ? 'minmax(var(--lc-compare-min), 1fr)' : 'var(--lc-compare-rail)')).join(' ') })
  } as CSSProperties
  return (
    <section className="lc-compare" aria-label="Comparison" style={style}>
      <div className="lc-compare__bar">
        <span>
          {kept === undefined
            ? `${compare.rerunOf === undefined ? 'Comparing' : 'A run of your benchmark, comparing'} ${columns.map((column) => column.name).join(columns.length === 2 ? ' and ' : ', ')}${veiled ? ', names hidden until you keep one' : ''}. ${
                compare.changes !== true
                  ? 'They answer without changing files.'
                  : compare.changesIn === 'folder'
                    ? 'This folder is too big to give each its own copy, so they all work in your folder itself.'
                    : `Each changes its own copy of your project; only the one you keep comes into your folder.${
                        // Said, not promised (0.555): Auto runs with the whole disk, and Sol, told to, wrote outside its copy.
                        compare.slots.some((column) => column.route.mode === 'auto') ? ' On Auto, a model can still change files outside its copy if it is asked to.' : ''
                      }`
              }`
            : keptSentence(keptName, conversationMode, compare.slots.find((column) => column.slot === kept)?.route.mode)}
        </span>
        {problem !== undefined && <span className="lc-compare__problem" role="status">{problem}</span>}
        {onBack !== undefined && (
          <button type="button" className="lc-button" onClick={onBack}>
            {backLabel}
          </button>
        )}
      </div>
      {onUseInstructions !== undefined && <KeptInstructions key={`${compare.compareId}:${compare.kept?.slot ?? ''}`} compare={compare} teammate={owner} onUse={onUseInstructions} />}
      <div className="lc-compare__scroll" ref={scrollRef}>
        <div className="lc-compare__grid">
          {columns.map((column) => railed(column.slot) ? (
            <button
              key={`head:${column.slot}`}
              type="button"
              className={`lc-compare__head is-rail${column.slot === kept ? ' is-kept' : ''}`}
              title={`${column.name}: ${column.state}. Focus on it.`}
              onClick={() => setFocusedSlot(column.slot)}
            >
              {!veiled && <RuntimeMark runtime={column.runtime} size={13} />}
              <span className="lc-compare__name">{column.name}</span>
              {column.running && <WorkingSpark />}
            </button>
          ) : (
            <div key={`head:${column.slot}`} className={`lc-compare__head${column.slot === kept ? ' is-kept' : ''}`}>
              {!veiled && <RuntimeMark runtime={column.runtime} size={13} />}
              {/* Its runtime is in the hover too: a narrow head gives that word up first (2026-10-05). */}
              <span className="lc-compare__name" title={veiled ? column.name : `${column.name} · ${column.runtimeName}`}>{column.name}</span>
              {judge?.pick === column.slot && (judge.panel === undefined
                ? <span className="lc-compare__judgebadge" title="The judge would keep this answer. Its view, not a decision.">Judge's pick</span>
                : <span className="lc-compare__judgebadge" title="Most of the judges would keep this answer. Their view, not a decision.">Judges' pick</span>)}
              {!veiled && (
              <span
                className={`lc-compare__runtime lc-mono${column.copy === true ? ' is-copy' : ''}`}
                {...(column.copy === true ? { title: `${column.runtimeName} cannot be held read-only on this computer, so it answers in a copy of your folder. Nothing in your folder changes.` } : {})}
              >
                {column.copy === true ? `${column.runtimeName} · in a copy` : column.runtimeName}
              </span>
              )}
              <span className="lc-compare__state lc-mono">
                {column.running && <WorkingSpark />}
                {column.state}
              </span>
              <button
                type="button"
                className="lc-compare__tool"
                aria-label={`Copy ${column.name}'s answer`}
                title={column.answer.length === 0 ? 'No answer to copy yet.' : copied === column.slot ? 'Copied.' : 'Copy its answer.'}
                disabled={column.answer.length === 0}
                onClick={() => copy(column)}
              >
                <Icon name={copied === column.slot ? 'check' : 'copy'} size={14} />
              </button>
              {columns.length > 1 && (
                <button
                  type="button"
                  className="lc-compare__tool"
                  aria-label={focused === column.slot ? 'Show every answer' : `Focus on ${column.name}`}
                  title={focused === column.slot ? 'Show every answer side by side.' : 'Give this answer the width. The others stay at the side.'}
                  aria-pressed={focused === column.slot}
                  onClick={() => setFocusedSlot(focused === column.slot ? undefined : column.slot)}
                >
                  <Icon name={focused === column.slot ? 'collapse' : 'expand'} size={14} />
                </button>
              )}
            </div>
          ))}
          {prompts.map((prompt, turn) => (
            <div key={`turn:${String(turn)}`} className="lc-compare__turn">
              <div className="lc-compare__ask">{prompt}</div>
              <div className="lc-compare__cells">
                {columns.map((column) => {
                  const cell = column.turns[turn]
                  if (railed(column.slot)) return <div key={`cell:${column.slot}:${String(turn)}`} className="lc-compare__cell is-rail" />
                  const built = cell === undefined || cell.running ? undefined : builtPageOf(cell.items, workspacePath)
                  /*
                   * A COLUMN NOT KEPT HAS NO PAGE LEFT (0.571). Keep removes the
                   * other copies, and their page's name then reads from the
                   * folder -- which holds the KEPT page: the blind arena run
                   * showed Sonnet's title screen in Opus's column, under Opus's name.
                   */
                  const copyGone = built !== undefined && compare.changes === true && compare.changesIn !== 'folder' && compare.kept !== undefined && compare.kept.slot !== column.slot
                  const page = copyGone ? undefined : built
                  return (
                    <div key={`cell:${column.slot}:${String(turn)}`} className="lc-compare__cell" aria-label={`${column.name}'s answer`}>
                      <div className="lc-compare__cellbody">
                        {cell === undefined ? (
                          <p className="lc-compare__quiet">{turn === 0 && column.refused !== undefined ? column.refused : 'Not asked this.'}</p>
                        ) : cell.items.length === 0 ? (
                          <p className="lc-compare__quiet">{cell.running ? 'Starting…' : 'No answer was recorded.'}</p>
                        ) : (
                          <>
                            {copyGone && (
                              <p className="lc-compare__quiet">{`Its copy was removed when you kept ${columns.find((other) => other.slot === compare.kept?.slot)?.name ?? 'another model'}, so the page it made is no longer on this computer. What it said about it is below.`}</p>
                            )}
                            {page !== undefined && (
                              <div className="lc-compare__page" aria-label={`The page ${column.name} made, running`}>
                                <PagePreview path={page} name={page.replace(/\\/g, '/').split('/').pop() ?? page} column={{ compareId: compare.compareId, slot: column.slot }} />
                              </div>
                            )}
                            <PinnedPagesContext.Provider value={page === undefined ? NO_PAGES : new Set([page])}>
                              <InComparisonCell.Provider value={true}>
                              <ThreadItems
                                items={cell.items}
                                owner={owner}
                                faces={false}
                                activity={cell.running ? 'thinking' : 'idle'}
                                workspacePath={column.folder ?? workspacePath}
                                {...(onOpenFile === undefined ? {} : { onOpenFile: (path: string) => onOpenFile(path, column.folder ?? workspacePath) })}
                                decision={undefined}
                              />
                              </InComparisonCell.Provider>
                            </PinnedPagesContext.Provider>
                          </>
                        )}
                        {/* What this column waits on you for, where you are looking: its newest turn. */}
                        {turn === column.turns.length - 1 && onDecide !== undefined && onAnswer !== undefined &&
                          (column.approvals ?? []).map((request) => (
                            <ApprovalCard
                              key={request.approvalId}
                              request={request}
                              busy={(decidingIds ?? []).includes(request.approvalId)}
                              onDecide={(decision, reason) => onDecide(request.approvalId, decision, reason)}
                              onAnswer={(answers) => onAnswer(request.approvalId, answers)}
                            />
                          ))}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ))}
        </div>
        {judge !== undefined && (
          /*
           * THE JUDGE'S VIEW (0.520), after Optima's judge models: a model
           * the person picked read the answers under their letters. Its
           * view, marked as one; the person still keeps an answer.
           */
          <div className="lc-compare__judge" aria-label="The judge's view">
            <span className="lc-compare__recordlabel lc-mono">
              {judge.running && <WorkingSpark />}
              {`${judge.panel === undefined ? "THE JUDGE'S VIEW" : "THE JUDGES' VIEW"} · ${judge.name}${judge.running ? ' · reading' : ''}`}
            </span>
            {/*
             * Drawn as every other reply (0.554). Colin, on 0.553: "very
             * unresponsive... not even sure myself if its doing anything" --
             * it said "Reading the answers…" until the judge was done, and
             * hid its steps and words while they came.
             */}
            {/* Its scorecard first, the summary before the words (0.759): every answer on every point, then score, time and cost. */}
            {judge.scorecard !== undefined && <JudgeScorecard scorecard={judge.scorecard} columns={columns} veiled={veiled} />}
            {/* Three judges (0.765): what the table is, and whose words follow it. */}
            {judge.panel !== undefined && !judge.running && judge.failed === undefined && (
              <p className="lc-compare__judgenote">
                {`${judge.panel.silent.length === 0 ? `The ${String(judge.panel.judges.length)} judges` : `${joinedNames(judge.panel.silent)} did not answer, so the other ${judge.panel.judges.length - judge.panel.silent.length === 1 ? 'judge' : `${String(judge.panel.judges.length - judge.panel.silent.length)} judges`}`} scored every point on their own; a point counts when more of them say met than not. The dots are their votes. The words below are ${judge.panel.wordsOf}'s.`}
              </p>
            )}
            {judge.failed !== undefined ? (
              <p className="lc-compare__quiet">{judge.failed}</p>
            ) : judge.items.length === 0 ? (
              <p className="lc-compare__quiet">{judge.running ? 'Starting the judge…' : 'No answer was recorded.'}</p>
            ) : (
              <div className="lc-compare__judgesaid">
                <InComparisonCell.Provider value={true}>
                  <ThreadItems
                    items={judge.items}
                    owner={undefined}
                    faces={false}
                    activity={judge.running ? 'thinking' : 'idle'}
                    workspacePath={workspacePath}
                    decision={undefined}
                  />
                </InComparisonCell.Provider>
              </div>
            )}
            <p className="lc-compare__judgenote">{judge.panel === undefined ? 'It read the answers as Answer A, Answer B and so on, not by model. Its view, not a decision: you keep the answer.' : 'They read the answers as Answer A, Answer B and so on, not by model. Their view, not a decision: you keep the answer.'}</p>
          </div>
        )}
        {/* Your own benchmark (0.763): under the judge's view, once it has scored the answers. */}
        {benchmark !== undefined && <CompareBenchmark view={benchmark} />}
        {onJudge !== undefined && compare.rerunOf === undefined && (compare.reruns ?? []).length === 0 && judgeChoices.length > 0 && judge?.running !== true && columns.filter((column) => column.keepable).length >= 2 && !columns.some((column) => column.running) && (
          <JudgeAsk choices={judgeChoices} compared={compare.slots.map((column) => `${column.route.runtime}:${column.route.model}`)} again={judge !== undefined} busy={judging} onJudge={onJudge} {...(onDraftPoints === undefined ? {} : { onDraftPoints })} />
        )}
        {onAddModel !== undefined && compare.rerunOf === undefined && addChoices.length > 0 && columns.length < 3 && compare.changes !== true && !columns.some((column) => column.running) && columns.every((column) => column.turns.length <= 1) && (
          <AddModelAsk choices={addChoices} busy={adding} onAdd={onAddModel} />
        )}
        {/*
          * WHY IT WENT (0.539). Sol, on 0.532, confirmed by Cursor on 0.537:
          * after a follow-up the block was gone with nothing said. A new
          * column is asked the first question only, so after a follow-up it
          * would be answering a different conversation.
          */}
        {onAddModel !== undefined && compare.rerunOf === undefined && addChoices.length > 0 && columns.length < 3 && compare.changes !== true && !columns.some((column) => column.running) && columns.some((column) => column.turns.length > 1) && (
          <p className="lc-compare__judgenote">Another model can join only before the first follow-up: it would be asked the first question and miss the turns since. Start a new comparison to add one.</p>
        )}
        {record !== undefined && record.length > 0 && (
          /*
           * YOUR RECORD (0.519), as Optima's results table: every model you
           * have compared, how often you kept it, and what its answers
           * typically took. Under the answers, once one is kept.
           */
          <div className="lc-compare__record">
            <span className="lc-compare__recordlabel lc-mono">YOUR RECORD</span>
            <div className="lc-tablewrap">
              <table className="lc-table">
                <thead>
                  <tr>
                    <th>Model</th>
                    <th>Kept</th>
                    <th>Typical time</th>
                    <th>Typical tokens</th>
                  </tr>
                </thead>
                <tbody>
                  {record.map((row) => (
                    <tr key={row.key}>
                      <td>{row.name}</td>
                      <td className="lc-compare__recordnum">{`${String(row.kept)} of ${String(row.compared)}`}</td>
                      <td className="lc-compare__recordnum">{row.time ?? '—'}</td>
                      <td className="lc-compare__recordnum">{row.cost ?? '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
      <div className="lc-compare__feet">
        {columns.map((column) => (
          <div key={`foot:${column.slot}`} className={`lc-compare__foot${railed(column.slot) ? ' is-rail' : ''}`}>
            {!railed(column.slot) && (
              /*
               * Each number whole (2026-10-05): in a third of the window the
               * line was cut to "+1273 -0 in 1 file · 5..." beside Keep. The
               * parts wrap between them; none is cut.
               */
              <span className="lc-compare__numbers lc-mono">
                {[changeLines[column.slot], column.span, veiled ? undefined : column.cost]
                  .filter((part): part is string => part !== undefined && part.length > 0)
                  .map((part) => <span key={part} className="lc-compare__part">{part}</span>)}
              </span>
            )}
            {/* A run of a benchmark is a measurement (0.763): nothing in it is tried again or kept. */}
            {railed(column.slot) || compare.rerunOf !== undefined ? null : kept === undefined && column.retryable ? (
              <button
                type="button"
                className="lc-button"
                disabled={retrying !== undefined || keeping}
                title="Ask it again, on the same model. The answer it gives takes this one's place."
                onClick={() => onRetry(column.slot)}
              >
                {retrying === column.slot ? 'Starting…' : 'Try again'}
              </button>
            ) : kept === undefined ? (
              <button
                type="button"
                className="lc-primarybutton"
                disabled={keeping || !column.keepable}
                title={
                  column.keepable
                    ? compare.changes === true && compare.changesIn === 'folder'
                      ? 'Keep this one: the conversation carries on with it. They all worked in your folder, so its changes are there already, with the others’.'
                      : compare.changes === true
                      ? 'Keep this one. Its changes come into your folder, not committed; the others stop, and their copies are removed.'
                      : 'Keep this one. The others stop, and the conversation carries on with it.'
                    : column.running ? 'Still answering.' : 'Nothing to keep: its answer did not finish.'
                }
                onClick={() => onKeep(column.slot)}
              >
                Keep this one
              </button>
            ) : column.slot === kept ? (
              <span className="lc-compare__kept lc-mono">Kept</span>
            ) : null}
          </div>
        ))}
      </div>
    </section>
  )
}

/** A model a judge or a new column can be: grouped under its agent when `group` is given. */
export interface ModelChoice {
  readonly key: string
  readonly label: string
  /** "Claude Code", "OpenCode": the agent it runs on, the menu's heading. */
  readonly group?: string
  /** Its name alone, under that heading. */
  readonly short?: string
}

/**
 * THE MODEL PICKED, OR THE ONE OFFERED (0.554). The menu is first drawn
 * before the agents are found, when only the compared models can judge; a
 * pick held from then stayed on one of them, and the judge ran on a model it
 * was judging. Until the person picks, it follows the one offered first.
 */
function useChosenModel(choices: readonly ModelChoice[]): readonly [string, (key: string) => void] {
  const [picked, setPicked] = useState<string>()
  const held = picked !== undefined && choices.some((one) => one.key === picked) ? picked : undefined
  return [held ?? choices[0]?.key ?? '', setPicked] as const
}

/**
 * EVERY MODEL, UNDER ITS AGENT (0.554), as the composer's model menu lists
 * them. Colin, on 0.553: "not all of our models showing in judge category"
 * -- sixteen fit, and the free OpenCode ones filled most of them. The one
 * offered stays first; the rest follow under their agent's name.
 */
function modelMenuOptions(choices: readonly ModelChoice[]): readonly SelectOption[] {
  const [first, ...rest] = choices
  const options: SelectOption[] = []
  if (first !== undefined) options.push({ value: first.key, label: first.label })
  const groups: { readonly name: string; readonly members: ModelChoice[] }[] = []
  for (const one of rest) {
    const name = one.group ?? ''
    const group = groups.find((candidate) => candidate.name === name)
    if (group === undefined) groups.push({ name, members: [one] })
    else group.members.push(one)
  }
  for (const group of groups) {
    for (const one of group.members) {
      options.push(group.name.length === 0
        ? { value: one.key, label: one.label }
        : { value: one.key, label: one.short ?? one.label, group: group.name })
    }
  }
  return options
}

/**
 * ASK A JUDGE (0.520): which model reads the answers, and what a good answer
 * does if the person wants to say. One line under the answers, once at
 * least two have finished; nothing runs until Judge is pressed.
 */
function JudgeAsk({ choices, compared = [], again, busy, onJudge, onDraftPoints }: {
  /** Draft the points from the question on this model (0.766): the points, or why there are none. */
  readonly onDraftPoints?: (choiceKey: string) => Promise<readonly string[] | string>
  readonly choices: readonly ModelChoice[]
  /** The compared models' keys: a judge is never one of them while another model can judge. */
  readonly compared?: readonly string[]
  readonly again: boolean
  readonly busy: boolean
  readonly onJudge: (choiceKey: string, criteria: string, others?: readonly string[]) => void
}): ReactElement {
  const [picked, setChoice] = useChosenModel(choices)
  const [criteria, setCriteria] = useState('')
  /*
   * THREE JUDGES (0.765), the canvas's call -- "three cheap models voting, or one stronger judge" -- left to the
   * person: one row more, two pickers, each offered a model the others are not. They vote on the points written
   * here, so Judge waits for one.
   */
  const [three, setThree] = useState(false)
  const [pickedOthers, setOthers] = useState<readonly (string | undefined)[]>([undefined, undefined])
  // The two beside the first are offered free models first -- the canvas's "three cheap models voting" -- and the
  // compared ones last of all. The menu's own order is no guard: before an agent's catalogue arrives its models sort
  // to the top, and the 0.765 drive had the two compared models offered as their own judges.
  const ranked = [...choices.filter((one) => !compared.includes(one.key)), ...choices.filter((one) => compared.includes(one.key))]
  const judges = threeJudges(ranked, [picked, ...pickedOthers], three ? 3 : 1, (key) => key.endsWith('-free') && !compared.includes(key))
  const choice = judges[0] ?? ''
  const others = judges.slice(1)
  const pointsWritten = criteria.split(/\r?\n/).some((line) => line.trim().length > 0)
  /*
   * DRAFT THE POINTS (0.766), the canvas's "Locust drafts that list from the task with a cheap model. You edit it."
   * On the first judge's model, offered while the box is empty; what comes back goes in the box, to be edited.
   */
  const [drafting, setDrafting] = useState(false)
  const [draftProblem, setDraftProblem] = useState<string>()
  const draft = async (): Promise<void> => {
    if (onDraftPoints === undefined || choice.length === 0) return
    setDrafting(true)
    setDraftProblem(undefined)
    try {
      const drafted = await onDraftPoints(choice)
      if (typeof drafted === 'string') setDraftProblem(drafted)
      else setCriteria((current) => (current.trim().length === 0 ? drafted.join('\n') : current))
    } finally {
      setDrafting(false)
    }
  }
  // By its model -- or, for an account's default, its agent: "with Account Default" said nothing (the 0.766 drive).
  const drafter = choices.find((one) => one.key === choice)
  const draftName = drafter === undefined ? '' : choice.endsWith(':account-default') ? drafter.group ?? drafter.label.split(' / ')[0] ?? drafter.label : drafter.short ?? drafter.label
  const ready = !drafting && choice.length > 0 && (!three || (others.length === 2 && pointsWritten))
  // Each label written out where it is used: the harness check reads labels from the attribute itself.
  const pickerOf = (index: number, value: string, onPick: (key: string) => void): ReactElement => (
    <span className="lc-compare__select" key={`judge:${String(index)}`}>
      <Select className="lc-input lc-compare__judgepick" aria-label={index === 0 ? (three ? 'The first judge' : 'The model that judges') : index === 1 ? 'The second judge' : 'The third judge'} value={value} onChange={onPick} options={modelMenuOptions(choices.filter((one) => one.key === value || !judges.includes(one.key)))} />
    </span>
  )
  return (
    <div className="lc-compare__judgeask">
      <span className="lc-compare__recordlabel lc-mono">{three ? (again ? 'ASK THREE OTHER JUDGES' : 'ASK THREE JUDGES') : again ? 'ASK ANOTHER JUDGE' : 'ASK A JUDGE'}</span>
      <div className="lc-compare__judgerow">
        {pickerOf(0, choice, setChoice)}
        {/* One point a line, each a row of the judge's scorecard (0.759); it grows with them. */}
        <textarea
          className="lc-input lc-compare__judgecriteria"
          aria-label="What a good answer does, one point a line"
          placeholder="What a good answer does, one point a line (optional)"
          maxLength={1000}
          rows={1}
          value={criteria}
          onChange={(event) => setCriteria(event.target.value)}
        />
        <button type="button" className="lc-button" disabled={busy || !ready} title={three && !pointsWritten ? 'Write the points they vote on first' : undefined} onClick={() => onJudge(choice, criteria, three ? others : [])}>
          {busy ? 'Asking…' : 'Judge'}
        </button>
      </div>
      {three && (
        <div className="lc-compare__judgerow">
          <span className="lc-compare__panelword">with</span>
          {others.map((key, index) => pickerOf(index + 1, key, (next) => setOthers((current) => current.map((one, at) => (at === index ? next : one)))))}
          <button type="button" className="lc-linkbutton" onClick={() => setThree(false)}>One judge</button>
        </div>
      )}
      <p className="lc-compare__judgenote">
        {three
          ? 'Each reads the answers without the models’ names and scores every point on its own; a point counts when two of the three agree. They vote on the points you write. Nothing is kept.'
          : 'It reads the answers without the models’ names, scores each on every point, and says which it would keep. With no points written, it sets three from your request. It keeps nothing itself.'}
        {pointsWritten && ' End a point with ×2 to count it twice.'}
        {!three && choices.length >= 3 && (
          <>
            {' '}
            <button type="button" className="lc-linkbutton" onClick={() => setThree(true)}>Ask three judges to vote</button>
          </>
        )}
      </p>
      {onDraftPoints !== undefined && !pointsWritten && (
        <p className="lc-compare__judgenote lc-compare__draft" {...(drafting ? { role: 'status' } : {})}>
          {drafting ? (
            <>
              <WorkingSpark />
              {`${draftName} is drafting the points from your question…`}
            </>
          ) : (
            <>
              <button type="button" className="lc-linkbutton" disabled={choice.length === 0} onClick={() => void draft()}>Draft the points</button>
              {` from your question with ${draftName}, then edit them.`}
            </>
          )}
        </p>
      )}
      {draftProblem !== undefined && !drafting && <p className="lc-compare__problem" role="status">{draftProblem}</p>}
    </div>
  )
}

/**
 * The judges asked, in order: each the one the person picked when it is still offered and not another's, else the
 * first offered that no other judge is -- for the judges after the first, one `preferred` first when there is one.
 * Fewer than `count` when there are not that many models.
 */
export function threeJudges(choices: readonly ModelChoice[], picked: readonly (string | undefined)[], count: number, preferred: (key: string) => boolean = () => false): readonly string[] {
  const chosen: string[] = []
  for (let index = 0; index < count; index += 1) {
    const want = picked[index]
    const free = (one: ModelChoice): boolean => !chosen.includes(one.key) && !picked.slice(index + 1).includes(one.key)
    const offered = (index > 0 ? choices.find((one) => free(one) && preferred(one.key)) : undefined) ?? choices.find(free) ?? choices.find((one) => !chosen.includes(one.key))
    const held = want !== undefined && choices.some((one) => one.key === want) && !chosen.includes(want) ? want : offered?.key
    if (held !== undefined) chosen.push(held)
  }
  return chosen
}

/** "Codex", "Codex and Ling 3.1", "Codex, Ling 3.1 and Muse Spark". */
export function joinedNames(names: readonly string[]): string {
  if (names.length <= 1) return names[0] ?? ''
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1)!}`
}

/** What the comparison shows of its judge, or of its three (0.765). */
export interface JudgeView {
  readonly name: string
  readonly running: boolean
  readonly answer: string
  readonly items: readonly ThreadItem[]
  readonly pick?: CompareSlotId
  readonly failed?: string
  readonly scorecard?: Scorecard
  /** Three judges voting: who they are, whose words are drawn, and any that did not answer. */
  readonly panel?: { readonly judges: readonly string[]; readonly wordsOf: string; readonly silent: readonly string[] }
}

/**
 * THE JUDGE'S SCORECARD (0.759, shared/compare-score.ts), after Artificial Analysis's Optima: every answer on every
 * point of what a good answer does -- met or not, and the judge's why in a line -- under the columns' own names, then
 * each one's score, time and what it spent. A blind comparison keeps that row out until one is kept, as its feet do. The
 * best score is marked only when one answer has it alone: a tie names no winner.
 */
function JudgeScorecard({ scorecard, columns, veiled }: {
  readonly scorecard: Scorecard
  readonly columns: readonly CompareColumnView[]
  readonly veiled: boolean
}): ReactElement {
  const shown = columns.filter((column) => scorecard.verdicts[column.slot] !== undefined)
  const scores = shown.map((column) => scoreOf(scorecard.verdicts[column.slot], scorecard.weights))
  const best = Math.max(...scores.map((score) => score.met))
  const alone = scores.filter((score) => score.met === best).length === 1
  return (
    <div className="lc-tablewrap lc-compare__scorecard">
      <table className="lc-table">
        <thead>
          <tr>
            <th scope="col">{scorecard.byJudge ? 'The judge’s points' : 'What a good answer does'}</th>
            {shown.map((column) => <th key={column.slot} scope="col">{column.name}</th>)}
          </tr>
        </thead>
        <tbody>
          {scorecard.points.map((point, index) => (
            <tr key={`point:${String(index)}`}>
              <th scope="row" className="lc-compare__point">
                {point}
                {(scorecard.weights?.[index] ?? 1) > 1 && (
                  <span className="lc-compare__weight lc-mono" title={`Counts ${String(scorecard.weights![index])} times in the score`}>{`×${String(scorecard.weights![index])}`}</span>
                )}
              </th>
              {shown.map((column) => {
                const verdict = scorecard.verdicts[column.slot]?.[index]
                return (
                  <td key={column.slot} className="lc-compare__verdictcell">
                    {verdict === undefined ? (
                      <span className="lc-compare__verdict is-unscored">not scored</span>
                    ) : (
                      <>
                        <span className={`lc-compare__verdict ${verdict.met ? 'is-met' : 'is-missed'}`}>
                          {verdict.met ? 'met' : 'not met'}
                          {verdict.votes !== undefined && verdict.votes.of > 1 && (
                            <span className="lc-compare__votes" role="img" aria-label={`${String(verdict.votes.met)} of ${String(verdict.votes.of)} judges said met`} title={`${String(verdict.votes.met)} of ${String(verdict.votes.of)} judges said met`}>
                              {Array.from({ length: verdict.votes.of }, (_unused, at) => <span key={at} className={`lc-compare__vote${at < verdict.votes!.met ? ' is-met' : ''}`} />)}
                            </span>
                          )}
                        </span>
                        {verdict.why.length > 0 && <span className="lc-compare__why">{verdict.why}</span>}
                      </>
                    )}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">Score</th>
            {shown.map((column, index) => (
              <td key={column.slot} className={`lc-compare__recordnum lc-compare__score${alone && scores[index]?.met === best ? ' is-best' : ''}`}>
                {`${String(scores[index]?.met ?? 0)} of ${String(scores[index]?.of ?? 0)}`}
              </td>
            ))}
          </tr>
          {/* A row no column has a figure for says nothing: it is left out, not drawn as a line of dashes. */}
          {shown.some((column) => column.span !== undefined) && (
            <tr>
              <th scope="row">Time</th>
              {shown.map((column) => <td key={column.slot} className="lc-compare__recordnum">{column.span ?? '—'}</td>)}
            </tr>
          )}
          {/* "Spent": dollars, premium requests, "in your plan" or tokens -- whatever each runtime reported (cost.ts). */}
          {!veiled && shown.some((column) => column.cost !== undefined) && (
            <tr>
              <th scope="row">Spent</th>
              {shown.map((column) => <td key={column.slot} className="lc-compare__recordnum">{column.cost ?? '—'}</td>)}
            </tr>
          )}
        </tfoot>
      </table>
      {scorecard.byJudge && <p className="lc-compare__judgenote">No points were written, so the judge set these from your request.</p>}
    </div>
  )
}

/**
 * ANOTHER MODEL, THE SAME QUESTION (0.523): a third column, asked what the
 * others were. After Optima, which runs a benchmark again on each new model.
 */
function AddModelAsk({ choices, busy, onAdd }: {
  readonly choices: readonly ModelChoice[]
  readonly busy: boolean
  readonly onAdd: (choiceKey: string) => void
}): ReactElement {
  const [choice, setChoice] = useChosenModel(choices)
  return (
    <div className="lc-compare__judgeask">
      <span className="lc-compare__recordlabel lc-mono">ASK ANOTHER MODEL</span>
      <div className="lc-compare__judgerow">
        <Select className="lc-input lc-compare__judgepick" aria-label="The model to ask too" value={choice} onChange={setChoice} options={modelMenuOptions(choices)} />
        <button type="button" className="lc-button" disabled={busy || choice.length === 0} onClick={() => onAdd(choice)}>
          {busy ? 'Asking…' : 'Ask it too'}
        </button>
      </div>
      <p className="lc-compare__judgenote">It is asked the same question, as a new column. What you kept stays kept.</p>
    </div>
  )
}
