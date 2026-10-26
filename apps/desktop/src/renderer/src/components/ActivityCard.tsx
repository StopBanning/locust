import { Fragment, useEffect, useLayoutEffect, useRef, useState, useContext } from 'react'
import { fileMenuEvent } from '../fileMenuOpen.js'
import { looksLikeFilePath } from '../../../shared/file-menu.js'

/** "Thought for 12s", or just "Thought" when the runtime never said when it began. */
/**
 * The word a file row's status shows for the tool that touched it.
 *
 * The runtime's own tool name, which is a word for most of them (`read`,
 * `edit`) -- but Codex reports every change as a record TYPE, `file_change`,
 * and a row reading "src/signup.ts  file_change" was an internal name on the
 * screen (first-impressions drive, 0.349). That one becomes a word.
 */
/*
 * The word beside a row, in Claude Code's own tool words (0.545). Cursor's
 * 0.544 pass: the same read said `Read` on Claude, `read` on Cursor and
 * `view_file` on Antigravity. Every runtime's read, search, listing and fetch
 * now says what Claude Code's would; a word with no counterpart stays its own.
 */
const CLAUDE_TOOL_WORDS: readonly (readonly [RegExp, string])[] = [
  [/^(read|read_?file|readfile|view|view_?file|view_?file_?outline|view_?code_?item|open_?file|readtoolcall)$/i, 'Read'],
  // Antigravity's own names (0.596): rows in Colin's ledger read "write_to_file" and "replace_file_content".
  [/^(write_?to_?file)$/i, 'Write'],
  [/^(replace_file_content|multi_replace_file_content)$/i, 'Edit'],
  [/^(grep|grep_?search|ripgrep|codebase_?search|semsearch)$/i, 'Grep'],
  [/^(glob|find_?by_?name|file_?search|list_?dir|list_?directory|listdir|ls)$/i, 'Glob'],
  [/^(web_?search|search_?web|websearch)$/i, 'WebSearch'],
  [/^(fetch|web_?fetch|read_?url(_?content)?|url_?content)$/i, 'WebFetch'],
  // Codex's image tool (0.610): "imageGeneration" on a row read as a program's name.
  [/^(image_?generation|image_?gen|generate_?image)$/i, 'Image']
]

export function fileToolWord(tool: string): string {
  if (tool === 'file_change') return 'changed'
  return CLAUDE_TOOL_WORDS.find(([pattern]) => pattern.test(tool))?.[1] ?? tool
}

export function thoughtLine(durationMs: number | undefined): string {
  if (durationMs === undefined || durationMs < 500) return 'Thought'
  return `Thought for ${durationText(durationMs)}`
}
import type { ReactElement } from 'react'

import { activityEntries, netFileEntries, boundedShellOutput, commandTook, defaultOpenEntry, fileRowCounts, newPageEntry, foldedToolsLead, foldedToolsNames, relativePath, displayPath, durationText, thoughtHeadline } from '../missionView.js'
import type { TraceSegment, ActivityDetail, ActivityEntry, PlanStep } from '../missionView.js'
import type { DiffCounts } from '../diff.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { CopyButton } from './CopyButton.js'
import { DiffView } from './DiffView.js'
import { ThreadImage } from './ThreadImage.js'
import { DocPreview, isNewDocument } from './DocPreview.js'
import { FilesElsewhere, RanOnTheCloud } from '../filesElsewhere.js'
import { InComparisonCell, PinnedPagesContext } from '../pinnedPages.js'
import { Icon } from './Icon.js'
import { FileTypeIcon, fileName, folderName, statusWord } from './FileTypeIcon.js'
import { AgentText, PlanSteps } from './ThreadItems.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { commandReach } from '../../../shared/command-reach.js'
import { CODEX_SANDBOX_DID_NOT_START, codexSandboxDidNotStart } from '../../../shared/codex-sandbox.js'

/** A changed file's whole text, for its row's Copy; refused in words when it cannot be had as text. */
async function readWholeFile(path: string, workspacePath: string | undefined): Promise<string | { readonly refused: string }> {
  const absolute = /^(?:[A-Za-z]:[\\/]|[\\/])/.test(path) || workspacePath === undefined ? path : `${workspacePath}/${path}`
  const read = await window.desktop?.readTextFile(absolute)
  if (read === undefined) return { refused: 'Locust could not reach that file.' }
  if (!read.ok) return { refused: read.message }
  // A spreadsheet or a Word file has no text to copy as it is.
  if (read.mode === 'table' || read.mode === 'document') return { refused: 'Only a text file can be copied. Open it instead.' }
  return read.text
}

/**
 * The disclosure chain for what a teammate did, three rungs deep:
 *
 *   Edited 3 files · ran 2 commands  +254 −16     <- this card, collapsed
 *     src/billing/v3.ts  MODIFIED  +184 −12       <- a file row
 *       the unified diff                          <- DiffView
 *
 * One list, two row kinds: files and shell commands sit in the order they
 * happened, so "what it did" reads top to bottom. Every count on the card
 * is derived from the same rows the diff draws -- the header total is the
 * sum of the file rows, and each file row is the sum of its hunks.
 */
/**
 * What a command printed, bounded and kept at both ends.
 *
 * `pre` rather than a diff view: this is output, not a change, and the shape
 * of it — columns, indentation, a stack trace — is often the information.
 */
function ShellOutput({ output, failed = false }: { readonly output: string; readonly failed?: boolean }): ReactElement {
  const { head, tail, omitted, total } = boundedShellOutput(output)
  /*
   * On a failing run the TAIL is the answer, so it is drawn brighter.
   *
   * The whole reason both ends are kept is that an error is as often the last
   * line as the first -- a vitest failure puts the assertion, the file and the
   * counts in its last six. When the command failed, those lines are what the
   * person opened the row for, and drawing them in the same muted grey as 300
   * lines of `seq` output makes them work to find it (design, 2026-09-08).
   *
   * Only on failure. Emphasising the tail of every successful command would
   * make the emphasis mean nothing, which is how `1 notice` happened.
   */
  const tailClass = `lc-shellout__text${failed ? ' is-answer' : ''}`
  /**
   * Everything, once asked for.
   *
   * Deliberately unbounded from here: at that point the person has pressed a
   * button that says how many lines they are asking for, and a second cut
   * after an explicit request is the app deciding it knows better.
   */
  const [showAll, setShowAll] = useState(false)
  if (showAll || omitted === 0) {
    /*
     * Expanded: nothing is emphasised. A short output IS its own tail, so a
     * failing one is drawn as the answer -- but once the person has asked for
     * all 300 lines, marking all 300 as "the answer" is emphasis that means
     * nothing, which is the rule stated three lines above this one and broken
     * one line below it in the first version.
     */
    return (
      <div className="lc-shellout">
        <pre className={showAll ? 'lc-shellout__text' : tailClass}>
          {showAll ? output.replace(/\s+$/, '') : head}
        </pre>
        <ShellOutputFoot output={output} total={total} />
      </div>
    )
  }
  return (
    <div className="lc-shellout">
      <pre className="lc-shellout__text">{head}</pre>
      {/*
        * The elision is a CONTROL, not a sentence.
        *
        * It used to be a line of prose in the middle of the `pre` explaining
        * that lines were missing, inside a box 320px tall that scrolled --
        * so the fact was unpressable and the box shouted. Same pattern as the
        * diff's folded context, which this app already ships.
        */}
      <button type="button" className="lc-shellout__more" onClick={() => setShowAll(true)}>
        {omitted} more lines
      </button>
      <pre className={tailClass}>{tail}</pre>
      <ShellOutputFoot output={output} total={total} />
    </div>
  )
}

/**
 * The foot of the output: take all of it, and how much there is.
 *
 * Copying is the honest answer to a bounded view -- the whole point of cutting
 * the middle is that the screen is the wrong place for 300 lines, and the
 * right place is wherever the person was going to put them.
 */
function ShellOutputFoot({ output, total }: { readonly output: string; readonly total: number }): ReactElement {
  const [copied, setCopied] = useState(false)
  const copy = (): void => {
    const text = output.replace(/\s+$/, '')
    const done = (): void => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1_600)
    }
    // Same fallback as `InstallCommand`: `navigator.clipboard` is the right
    // call and is not always available to a packaged page.
    navigator.clipboard?.writeText(text).then(done).catch(() => {
      const field = document.createElement('textarea')
      field.value = text
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
  return (
    <div className="lc-shellout__foot">
      <button type="button" className="lc-shellout__copy" onClick={copy}>
        <Icon name="copy" size={12} />
        {copied ? 'Copied' : `Copy all ${String(total)} ${total === 1 ? 'line' : 'lines'}`}
      </button>
    </div>
  )
}

/** How many rows a finished fold shows before "Show N more". */
export const FOLD_ROWS_SHOWN = 10
/** A finished turn's changed files show their first five, as Claude Code's list does (0.835); the rest are a press away. */
export const FILE_ROWS_SHOWN = 5

export function ActivityCard({
  summary,
  details,
  runtimeName,
  trace,
  finished = false,
  workspacePath,
  plan,
  notices = [],
  onOpenFile,
  planUnderway = false,
  openByDefault = false,
  verbose = false,
  fold = false,
  traceOpen,
  traceShorter,
  traceOpenShorter,
  traceTitle,
  traceOpenTitle,
  variant = 'card',
  oneRowPerFile = false
}: {
  /** A turn's files (0.494): one row per file, its net change where the host looked. */
  readonly oneRowPerFile?: boolean
  /**
   * `steps`: one group of a turn's steps, drawn as Claude Code draws one --
   * a plain line saying what was done, opening onto the steps (0.491). The
   * `trace` is that line (`stepsLine`). No box, no cap, nothing open inside
   * until asked. `card`: the boxed fold. `files`: the box as a finished
   * turn's files use it, every file closed until pressed, as Claude Code
   * closes a turn with "Edited 2 files".
   */
  readonly variant?: 'card' | 'steps' | 'files'
  /** Whether the turn is still running, so the plan's step gets its orb. */
  readonly planUnderway?: boolean
  readonly summary: string
  /**
   * Open a file this turn touched in the panel beside the conversation.
   *
   * The fold is the COMMON way a person meets a file -- a handed file is a
   * teammate choosing to give you one, and most files are not handed. Until
   * this, the fold's only offer was the file manager, so reading what was
   * written meant leaving the app. Absent: the row reveals, as before.
   */
  readonly onOpenFile?: (path: string) => void
  /** The trace line; when absent the summary string is drawn (older callers). */
  readonly trace?: readonly TraceSegment[]
  /** True once the turn is over: an unsettled subagent then reads "did not report". */
  readonly finished?: boolean
  readonly details: readonly ActivityDetail[]
  readonly runtimeName: string | undefined
  /** The folder this mission ran in, so paths read the way a person writes them. */
  readonly workspacePath: string | undefined
  /**
   * The plan this run stated, drawn as the fold's FIRST rows.
   *
   * A plan is the clearest possible statement of what the run did, so it is
   * the fold's own content rather than a card sitting above it -- which is
   * what it used to be, on almost every Codex run (design review,
   * 2026-09-06).
   */
  readonly plan?: { readonly steps: readonly PlanStep[]; readonly doneCount: number }
  /**
   * What the runtime said about this turn, drawn at the foot of the fold.
   *
   * The diagnostics the thread's gate drops. They were counted in the trace
   * line as `1 notice` and shown nowhere -- see missionView's collection
   * point, which is inside the branch that drops them so the two can never
   * describe different sets again.
   */
  readonly notices?: readonly {
    readonly level: 'info' | 'warning' | 'error'
    readonly message: string
    /** Which runtime said it: the source the footer names. */
    readonly source: MissionRuntimeId
  }[]
  /**
   * Open on arrival, for the newest finished turn. See `openByDefault` on the
   * activity item for why: the live narration disappears when a run ends, and
   * a person looking back at it found one collapsed line.
   */
  readonly openByDefault?: boolean
  /**
   * Verbose (Ctrl+O): this group starts open, and every tool call's detail
   * is shown. The person's own open or close still wins.
   */
  readonly verbose?: boolean
  /**
   * The run moved on past this group (0.594): fold it to its line, unless
   * the person opened or closed it by hand. The one way the app closes a
   * fold, and only while the turn is running -- see the effect below.
   */
  readonly fold?: boolean
  /** The line to draw while open, when it differs (a lone thought keeps its preview for the closed line only). */
  readonly traceOpen?: readonly TraceSegment[]
  /** The steps sentence in ever shorter forms (0.604), closed and open; the line shows the first that fits its row. */
  readonly traceShorter?: readonly string[]
  readonly traceOpenShorter?: readonly string[]
  /** The sentence with every name whole (0.604): the hover title whenever what shows is not it. */
  readonly traceTitle?: string
  readonly traceOpenTitle?: string
}): ReactElement {
  const [open, setOpen] = useState(openByDefault || verbose)
  /*
   * THE LINE FITS ITS ROW (0.604). The sentence is measured after it paints;
   * while it still overflows its box, the next shorter form from the builder
   * (`shorter`: names to counts, then "and N more") is tried, and the row's
   * widening starts the fitting over. The row used to cut the sentence at its
   * edge mid-word ("edited ap…", Colin's screenshot, 2026-10-04). The full
   * sentence is the line's title whenever a shorter form is shown. A card
   * that is not the steps line does none of this.
   */
  const shownTrace = open && traceOpen ? traceOpen : trace
  const shorter = (open && traceOpen ? traceOpenShorter : traceShorter) ?? []
  const fullText = shownTrace?.[0]?.text ?? summary
  const boxRef = useRef<HTMLDivElement>(null)
  const textRef = useRef<HTMLSpanElement>(null)
  const [fitted, setFitted] = useState<{ readonly of: string; readonly level: number } | undefined>(undefined)
  const [boxWidth, setBoxWidth] = useState(0)
  const fitLevel = fitted?.of === fullText ? Math.min(fitted.level, shorter.length) : 0
  const shownText = fitLevel === 0 ? fullText : shorter[fitLevel - 1]!
  // Whole names on hover whenever the line shows anything less than them: a cut name, a count, "and N more".
  const wholeText = (open && traceOpen ? traceOpenTitle : traceTitle) ?? fullText
  const hoverTitle = shownText === wholeText ? undefined : wholeText
  useLayoutEffect(() => {
    const el = textRef.current
    if (variant !== 'steps' || el === null) return
    if (el.scrollWidth > el.clientWidth + 1 && fitLevel < shorter.length) setFitted({ of: fullText, level: fitLevel + 1 })
  }, [variant, fullText, fitLevel, shorter.length, boxWidth, open])
  useEffect(() => {
    const box = boxRef.current
    if (variant !== 'steps' || box === null || typeof ResizeObserver === 'undefined') return
    let last = box.clientWidth
    const observer = new ResizeObserver(() => {
      const width = box.clientWidth
      if (width > last) setFitted(undefined)
      last = width
      setBoxWidth(width)
    })
    observer.observe(box)
    return () => observer.disconnect()
  }, [variant])
  /**
   * Whether the person has decided for themselves.
   *
   * `openByDefault` flips from false to true when the run finishes, and this
   * component does not remount -- so without remembering a deliberate press,
   * closing the fold mid-run would be silently undone a moment later. A choice
   * made by hand outranks the default, always.
   */
  const decided = useRef(false)
  /*
   * FOLDED WHEN THE RUN MOVES ON (0.594). The rule below -- a default may
   * open a fold and never close it -- was written for a fold closing under
   * the person's eyes the moment they sent a follow-up (2026-09-08). It also
   * meant every group that had ever been live stayed open, and a long Grok
   * run on Cursor read as a wall (Colin, 2026-10-04). Claude Code keeps one
   * step open: the one under way. So when the host says the run has moved
   * past this group, it folds to its line -- one press reopens it, nothing is
   * lost -- unless the person opened or closed it themselves. `fold` is never
   * true once the turn is over, so the last group stays as it was.
   */
  useEffect(() => {
    if (decided.current || fold !== true || verbose) return
    setOpen(false)
  }, [fold, verbose])
  const verboseWas = useRef(verbose)
  useEffect(() => {
    if (!decided.current && verbose) setOpen(true)
    if (!decided.current && !verbose && verboseWas.current) setOpen(openByDefault)
    verboseWas.current = verbose
  }, [verbose, openByDefault])
  useEffect(() => {
    if (decided.current) return
    /*
     * A default may OPEN this fold. It may never close it.
     *
     * `openByDefault` is true only for the newest finished turn, so sending a
     * follow-up flipped it back to false on the turn before -- and this effect
     * dutifully closed a fold the person was looking at. Driven and measured
     * (docs/user-session/2026-09-08T14-07-32-earlier-turn-work): six tool rows
     * on screen, then `expanded: false, rowsVisible: 0` the moment the next
     * message was sent, with nothing having been pressed.
     *
     * That is Colin's report, 2026-09-08 -- "the thoughts and tool calls
     * disappear after an agent is done ... we want that to stay so they can
     * see after the fact or if they missed it". The work was never lost, but
     * it was taken off the screen by the app rather than by them.
     *
     * Opening is a default; closing is an action, and the app does not get to
     * take it. A turn arriving from history still starts closed, because its
     * initial state was closed and nothing here opens it.
     */
    if (openByDefault === true) setOpen(true)
  }, [openByDefault])
  // Only ever set when the host refuses. A reveal that works needs no words:
  // the file manager comes to the front and that is the whole feedback.
  const [revealNotice, setRevealNotice] = useState<string | undefined>(undefined)
  const reveal = (path: string): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setRevealNotice(undefined)
    void bridge
      .revealFile(path)
      .then((response) => {
        if (!response.ok) setRevealNotice(response.message)
      })
      .catch(() => setRevealNotice('That file could not be shown. It is still where it was written.'))
  }
  // A turn's files: one row per file, its net change where the host looked (0.494).
  const entries = oneRowPerFile ? netFileEntries(activityEntries(details, workspacePath), workspacePath) : activityEntries(details, workspacePath)
  const fileEntries = entries.filter((entry): entry is ActivityEntry & { readonly kind: 'file' } => entry.kind === 'file')
  const rowCountsList = fileEntries.map(fileRowCounts)
  let counts: DiffCounts | undefined
  if (fileEntries.length > 0 && rowCountsList.every((c): c is DiffCounts => c !== undefined)) {
    let added = 0
    let removed = 0
    for (const c of rowCountsList) {
      if (c !== undefined) {
        added += c.added
        removed += c.removed
      }
    }
    counts = { added, removed }
  }
  const anyPatch = fileEntries.length > 0
  const [toggled, setToggled] = useState<ReadonlyMap<string, boolean>>(() => new Map())
  // A page shown running above, in a comparison's cell, stays folded here (0.450).
  const pinned = useContext(PinnedPagesContext)
  // A turn whose files are not in this folder (0.762, filesElsewhere.ts): its rows act on nothing here.
  const elsewhere = useContext(FilesElsewhere)
  const openHere = elsewhere === undefined ? onOpenFile : undefined
  const onTheCloud = useContext(RanOnTheCloud)
  const inComparison = useContext(InComparisonCell)
  const steps = variant === 'steps'
  // The files card opens a new page only (0.580, `newPageEntry`) -- not in a
  // comparison's cell, whose page runs above it: after Keep the kept page's
  // path is the folder's, not the copy's, so the pinned check below missed it
  // and the reopened column ran the page twice.
  const firstOpen = variant === 'card' ? defaultOpenEntry(entries) : variant === 'files' && !inComparison ? newPageEntry(entries) : undefined
  const initiallyOpen = entries.some((entry) => entry.key === firstOpen && entry.kind === 'file' && pinned.has(entry.file.path)) ? undefined : firstOpen
  const isOpen = (entry: ActivityEntry): boolean => {
    const chosen = toggled.get(entry.key)
    if (chosen !== undefined) return chosen
    if (verbose) return true
    return entry.key === initiallyOpen
  }
  const decide = (next: boolean): void => {
    decided.current = true
    setOpen(next)
  }
  const toggle = (entry: ActivityEntry): void => {
    const next = new Map(toggled)
    next.set(entry.key, !isOpen(entry))
    setToggled(next)
  }
  /*
   * A LONG FINISHED FOLD SHOWS ITS FIRST ROWS, AND SAYS HOW MANY MORE.
   *
   * Folds stay open when a turn finishes -- Colin, 2026-09-08: "we want that
   * to stay so they can see after the fact". But a twelve-file turn opened a
   * twenty-five-row fold 969px tall in an 800px window, and the answer it
   * led to was off the screen (Yurt's beta report, #2 / B3; the 0.271 design
   * recheck's "make the final answer the focal point", 9a). So a finished
   * fold keeps its first rows and one press shows the rest: open, as he
   * asked, and the answer within reach. A run still going shows every row --
   * that is the work happening.
   */
  const [showAll, setShowAll] = useState(false)
  /*
   * A FILE BY ITS NAME (0.835), as Claude Code lists a turn's files: the name, and its folder only when two of the
   * rows share that name. The whole path is the row's hover, as before.
   */
  const named = new Map<string, number>()
  for (const entry of entries) {
    const path = entry.kind === 'file' ? entry.file.path : entry.kind === 'unreported' ? entry.name : undefined
    if (path !== undefined) named.set(fileName(path).toLowerCase(), (named.get(fileName(path).toLowerCase()) ?? 0) + 1)
  }
  const twice = (path: string): string | undefined => ((named.get(fileName(path).toLowerCase()) ?? 0) > 1 ? folderName(relativePath(path, workspacePath)) : undefined)
  const rowsShown = variant === 'files' ? FILE_ROWS_SHOWN : FOLD_ROWS_SHOWN
  const capped = !verbose && !steps && finished && !showAll && entries.length > rowsShown + 2
  const shown = capped ? entries.slice(0, rowsShown) : entries

  return (
    <div className={steps ? 'lc-steps' : 'lc-card'} ref={boxRef}>
      {steps ? (
        <button type="button" className="lc-steps__line" onClick={() => decide(!open)} aria-expanded={open}>
          {/* The words are shortened to fit, never cut (0.604); what went wrong never gives way. Open, the line may read differently (0.594). */}
          <span ref={textRef} className={`lc-steps__text${shownTrace?.[0]?.tone === undefined ? '' : ` is-${shownTrace[0]!.tone}`}`} {...(hoverTitle === undefined ? {} : { title: hoverTitle })}>{shownText}</span>
          {(shownTrace ?? []).slice(1).map((seg) => (
            <span className={`lc-steps__extra${seg.tone === undefined ? '' : ` is-${seg.tone}`}`} key={seg.key}>
              {seg.text}
            </span>
          ))}
          {counts !== undefined && !inComparison && (
            <span className="lc-steps__counts lc-mono">
              <span className="lc-diff__addmark">+{counts.added}</span>
              <span className="lc-diff__delmark">−{counts.removed}</span>
            </span>
          )}
          <span className="lc-steps__chev" aria-hidden="true">
            <Icon name={open ? 'chevron-down' : 'chevron-right'} size={12} />
          </span>
        </button>
      ) : (
      <button type="button" className="lc-activity" onClick={() => decide(!open)} aria-expanded={open}>
        <Icon name="diff" size={14} />
        {trace === undefined || trace.length === 0 ? (
          <span>{summary}</span>
        ) : (
          <span className="lc-trace">
            {trace.map((seg) => (
              <span className={`lc-trace__seg${seg.tone === undefined ? '' : ` is-${seg.tone}`}`} key={seg.key}>
                {seg.text}
              </span>
            ))}
          </span>
        )}
        {counts !== undefined && !inComparison && (
          <span className="lc-activity__counts lc-mono">
            <span className="lc-diff__addmark">+{counts.added}</span>
            <span className="lc-diff__delmark">−{counts.removed}</span>
          </span>
        )}
        <span className={`lc-activity__chev${anyPatch ? '' : ' is-alone'}`} aria-hidden="true">
          <Icon name={open ? 'chevron-down' : 'chevron-right'} size={12} />
        </span>
      </button>
      )}
      {/*
        * A LINE THAT IS ONE THOUGHT opens straight onto its words (0.493), as
        * Claude Code's thinking does. It opened onto a row repeating the line
        * ("Thought for 7s · Considering fallback options"), which opened in
        * turn onto the headline a third time (Colin's frame, 2026-09-30).
        */}
      {open && steps && entries.length === 1 && entries[0]?.kind === 'thought' && entries[0].text.trim().length > 0 ? (
        <div className="lc-steps__list lc-steps__thought">
          <AgentText text={withoutHeadline(entries[0].text)} streaming={false} />
        </div>
      ) : open && (
        <div className={steps ? 'lc-steps__list' : 'lc-activity__list'}>
          {plan !== undefined && (
            <div className="lc-activity__plan">
              {/* The same component the thread uses for a Plan-mode answer,
                  with the one prop that separates the two jobs. Inside a fold
                  a run happened, so the steps have outcomes and the counter is
                  a fact about them. */}
              <PlanSteps steps={plan.steps} doneCount={plan.doneCount} outcomes underway={planUnderway} />
            </div>
          )}
          {shown.map((entry) => (
            <Fragment key={entry.key}>
              {entry.kind === 'file' ? (
                <>
                  <div className="lc-filerow__line">
                    <button type="button" className="lc-filerow" onClick={() => toggle(entry)} onContextMenu={fileMenuEvent(entry.file.path)} aria-expanded={isOpen(entry)}>
                      <FileTypeIcon path={entry.file.path} />
                      <span className="lc-filerow__path lc-filerow__name" title={elsewhere === undefined ? displayPath(entry.file.path, workspacePath) : `${displayPath(entry.file.path, workspacePath)}\n${elsewhere}`}>{fileName(entry.file.path)}</span>
                      {twice(entry.file.path) !== undefined && <span className="lc-filerow__dir">{twice(entry.file.path)}</span>}
                      {statusWord(entry.file.status) !== undefined && <span className="lc-filerow__status">{statusWord(entry.file.status)}</span>}
                      {/* Said when its counts could not be known: the reason there are none. */}
                      {entry.large && <span className="lc-filerow__status is-large">large</span>}
                      {(() => {
                        const counts = fileRowCounts(entry)
                        if (counts === undefined) return null
                        return (
                          <span className="lc-filerow__result">
                            <span className="lc-diff__addmark">+{counts.added}</span>
                            <span className="lc-diff__delmark">−{counts.removed}</span>
                          </span>
                        )
                      })()}
                      <span className="lc-activity__chev" aria-hidden="true">
                        <Icon name={isOpen(entry) ? 'chevron-down' : 'chevron-right'} size={12} />
                      </span>
                    </button>
                    {/*
                      The file is on this disk and, until now, nothing on screen
                      would take you to it -- found by dogfooding (Colin,
                      2026-09-07): a teammate wrote a report, named it, and the
                      name was not clickable. This is the row's own control
                      rather than a menu item, because "where is it" is the
                      first thing asked about a file and a fold two levels deep
                      is not where the answer belongs.
                    */}
                    {/*
                      Read it here, rather than going to find it. The diff
                      above answers "what changed"; this answers "what does
                      the file SAY", which for a report or a brief is the
                      whole question and which a patch of three hunks cannot
                      answer. It opens the same panel a handed file opens --
                      one viewer, not two -- and like every other file control
                      in this app it never hands the file to the operating
                      system to run.
                    */}
                    {openHere !== undefined && (
                      <button
                        type="button"
                        className="lc-filerow__view"
                        title={`Open ${relativePath(entry.file.path, workspacePath)}`}
                        aria-label={`Open ${relativePath(entry.file.path, workspacePath)}`}
                        onClick={() => openHere(entry.file.path)}
                      >
                        {/* `maximize`, not `file`: the row's left edge already
                            carries a file glyph, and two of the same mark on
                            one row says the two controls do the same thing. */}
                        <Icon name="maximize" size={13} />
                      </button>
                    )}
                    {/*
                      COPY THE FILE (0.649), as a handed file offers it. Colin,
                      2026-10-05: "if its a file change shouldnt it copy the
                      file". The whole file as it is now, read when asked; the
                      opened change's own Copy gives just the changed code.
                    */}
                    {entry.file.status !== 'DELETED' && elsewhere === undefined && (
                      <CopyButton
                        className="lc-filerow__copy"
                        label={relativePath(entry.file.path, workspacePath)}
                        text={() => readWholeFile(entry.file.path, workspacePath)}
                      />
                    )}
                    {elsewhere === undefined && (
                      <button
                        type="button"
                        className="lc-filerow__reveal"
                        title={`Show ${relativePath(entry.file.path, workspacePath)} in the file manager`}
                        aria-label={`Show ${relativePath(entry.file.path, workspacePath)} in the file manager`}
                        onClick={() => reveal(entry.file.path)}
                      >
                        <Icon name="folder" size={13} />
                      </button>
                    )}
                  </div>
                  {isOpen(entry) &&
                    (isNewDocument(entry.file) ? (
                      <DocPreview
                        file={entry.file}
                        truncated={entry.truncated}
                        reported={entry.reported}
                        {...(openHere === undefined ? {} : { onOpen: () => openHere(entry.file.path) })}
                      />
                    ) : (
                      <DiffView file={entry.file} truncated={entry.truncated} reported={entry.reported} workspacePath={workspacePath} />
                    ))}
                </>
              ) : entry.kind === 'helper' && entry.calls !== undefined && entry.calls.length > 0 ? (
                /*
                 * A HELPER THAT SAYS WHAT IT DID (helper visibility,
                 * 2026-10-05). Colin: "users are probably going to want to
                 * inspect when one of the 'helpers/agents' is sent out." Its
                 * calls are counted on its row and fold under it, closed until
                 * pressed, one step in -- Claude Code's helper line. Closed by
                 * default whatever the card opens first: they are the helper's
                 * working, not the teammate's.
                 */
                <>
                  <button type="button" className="lc-filerow is-helper" onClick={() => toggle(entry)} aria-expanded={toggled.get(entry.key) === true}>
                    <Icon name="users" size={14} />
                    <span className="lc-filerow__path">{entry.description}</span>
                    <span className="lc-filerow__status">{entry.subagentType === undefined ? 'subagent' : `${entry.subagentType} subagent`}</span>
                    <span className="lc-filerow__status lc-helper__count">{helperCallCount(entry.calls)}</span>
                    <span
                      className={`lc-filerow__result ${helperTone(entry, finished)}`}
                      {...(entry.summary === undefined ? {} : { title: entry.summary })}
                    >
                      {helperResult(entry, finished)}
                    </span>
                    <span className="lc-activity__chev" aria-hidden="true">
                      <Icon name={toggled.get(entry.key) === true ? 'chevron-down' : 'chevron-right'} size={12} />
                    </span>
                  </button>
                  {toggled.get(entry.key) === true && <HelperCalls calls={entry.calls} finished={finished} workspacePath={workspacePath} />}
                </>
              ) : entry.kind === 'helper' ? (
                // A helper the runtime started for itself. Where the runtime
                // does not say what it did inside, what it was asked, and
                // whether it came back, is all there is to show.
                <div className="lc-filerow is-static is-helper">
                  <Icon name="users" size={14} />
                  <span className="lc-filerow__path">{entry.description}</span>
                  <span className="lc-filerow__status">{entry.subagentType === undefined ? 'subagent' : `${entry.subagentType} subagent`}</span>
                  {/* Four states, not three: a helper the run ended without settling is not working on it -- it did not report (SURFACES-0.22 §2). Amber, not red: nothing said it failed. */}
                  <span
                    className={`lc-filerow__result ${helperTone(entry, finished)}`}
                    {...(entry.summary === undefined ? {} : { title: entry.summary })}
                  >
                    {helperResult(entry, finished)}
                  </span>
                </div>
              ) : entry.kind === 'shell' ? (
                /*
                 * A command row opens onto what the command PRINTED, when the
                 * runtime reported it.
                 *
                 * It was `is-static`: the command, a result word, and no way
                 * to see the output at all. A teammate ran `seq 1 1200`, said
                 * "printed 1 through 1200, one per line", and the row said
                 * `done` — the person could not see one of those lines
                 * (drive-huge-turn, 2026-09-08). The adapter had captured the
                 * output and the entry threw it away.
                 *
                 * Static still, where there is nothing to show. Only the Codex
                 * exec stream reports command output today, so a row that
                 * offered to expand everywhere would be a dead control on the
                 * runtimes that do not — the rule this composer has already
                 * paid for twice. The chevron follows the evidence.
                 */
                /*
                 * The exit code sits at the right end, as a badge.
                 *
                 * It used to lead the row, so `rg …` began about a badge's
                 * width to the right of the file names under it and the eye
                 * could not run down one edge. A read row already says
                 * `done` at that end. The badge's colour, wording and hover
                 * are the same; only its place changed.
                 *
                 * A command that printed NOTHING is a static row saying so.
                 * It used to open onto an empty box, which reads as "the app
                 * lost it" rather than "there was none" -- and `undefined`
                 * (the runtime never reported output) and `''` (it reported
                 * none) are different facts, so they get different words.
                 */
                /*
                 * A DESCRIBED command opens too, onto the command.
                 *
                 * The row leads with the description, so the command itself
                 * is the thing to open onto -- and on a row that printed
                 * nothing, the static branch drew the description IN PLACE
                 * of the command with no way back to it. That was unreachable
                 * on Claude Code only because its descriptions never arrived
                 * here (fixed 2026-09-22); Claude reports no output at all,
                 * so every Claude command would have lost its command.
                 */
                (entry.output === undefined || entry.output.trim() === '') && entry.title === undefined ? (
                  <div className="lc-filerow is-shell is-static">
                    <Icon name="terminal" size={14} />
                    {/* Nothing printed and nothing said about it, so nothing
                      * to open onto -- the command is the row. */}
                    <span
                      className={`lc-filerow__path${shellSweeping(entry, finished) ? ' lc-sweep' : ''}`}
                      // The highlight is drawn from this, not from the child
                      // text -- see `.lc-sweep`. Harmless when not sweeping.
                      data-text={entry.command}
                    >
                      {entry.command}
                    </span>
                    <BackgroundBadge entry={entry} />
                    <ReachBadge entry={entry} />
                    <span className={`lc-shellbadge ${shellResultClass(entry, finished)}`} title={refusedWhy(entry)}>{shellResult(entry, finished)}</span>
                    {entry.output !== undefined && entry.settled && (
                      <span className="lc-filerow__result is-muted">no output</span>
                    )}
                    {commandTook(entry) !== undefined && <span className="lc-filerow__took">{commandTook(entry)}</span>}
                  </div>
                ) : (
                  <>
                    <button type="button" className="lc-filerow is-shell" onClick={() => toggle(entry)} aria-expanded={isOpen(entry)}>
                      <Icon name="terminal" size={14} />
                      {/*
                        * What it was DOING, where the runtime says so.
                        *
                        * Claude Code's Bash tool takes a description on every
                        * call and the model writes it; that is the whole
                        * reason its own transcript reads "Checked what the
                        * app says about the free route" instead of a shell
                        * pipeline. Locust had the field and drew the pipeline.
                        *
                        * The command does not go away -- it moves under the
                        * fold. It is the evidence of what ran on this machine
                        * and the sentence is only a claim about it, so the row
                        * may lead with the claim and must not lose the
                        * evidence.
                        */}
                      <span
                        className={`lc-filerow__path${shellSweeping(entry, finished) ? ' lc-sweep' : ''}`}
                        data-text={entry.title ?? entry.command}
                      >
                        {entry.title ?? entry.command}
                      </span>
                      {/*
                        * SENT TO THE BACKGROUND, said on the row.
                        *
                        * Claude Code's Bash tool takes `run_in_background`
                        * and the flag rides on the same input this row
                        * already reads for the command and the description.
                        * Without it, a call the runtime was told not to wait
                        * for looked exactly like one it waited for -- which
                        * is most of why a finished background task reads as
                        * a turn that just stopped (Colin, 2026-09-21).
                        *
                        * It says what the CALL was, and what became of the
                        * work once the runtime says.
                        */}
                      <BackgroundBadge entry={entry} />
                      <ReachBadge entry={entry} />
                      <SandboxBadge entry={entry} />
                      <span className={`lc-shellbadge ${shellResultClass(entry, finished)}`} title={refusedWhy(entry)}>{shellResult(entry, finished)}</span>
                      {entry.output !== undefined && entry.output.trim() === '' && entry.settled && (
                        <span className="lc-filerow__result is-muted">no output</span>
                      )}
                      {commandTook(entry) !== undefined && <span className="lc-filerow__took">{commandTook(entry)}</span>}
                      <span className="lc-activity__chev" aria-hidden="true">
                        <Icon name={isOpen(entry) ? 'chevron-down' : 'chevron-right'} size={12} />
                      </span>
                    </button>
                    {isOpen(entry) && (
                      <>
                        {entry.title !== undefined && <pre className="lc-shellcommand lc-mono">{entry.command}</pre>}
                        {entry.failed === true && codexSandboxDidNotStart(entry.output) && <div className="lc-shellwhy">{CODEX_SANDBOX_DID_NOT_START}</div>}
                        {entry.output !== undefined && entry.output.trim() !== '' && (
                          <ShellOutput output={entry.output} failed={entry.failed === true} />
                        )}
                      </>
                    )}
                  </>
                )
              ) : entry.kind === 'thought' ? (
                /*
                 * What the model thought, in the fold with the rest of the
                 * work. Not a tool row: it has no verb, no outcome and
                 * nothing to succeed or fail at, so it borrows none of that
                 * furniture. Muted and quoted, because it is the model
                 * talking to itself rather than reporting to anyone.
                 */
                <>
                  {/*
                    * One line, folded. Colin, 2026-09-17: "do you remember ...
                    * how claude code did it? it would say how long they
                    * thought for ... and then you could just hit a dropdown
                    * and the thoughts would show if needed, i just worry in
                    * its current state it takes up so much real estate."
                    * The duration is the line; the words are under it.
                    */}
                  {entry.text.trim().length === 0 ? (
                    // The length alone (0.489): nothing to open, so no chevron
                    // promising something it does not have.
                    <div className="lc-filerow is-static lc-filerow--thought">
                      <Icon name="thought" size={14} />
                      <span className="lc-filerow__path">{thoughtLine(entry.durationMs)}</span>
                    </div>
                  ) : steps ? (
                    /*
                     * ONE WAY A THOUGHT READS (0.515). Colin, 2026-10-01, with
                     * two frames: a line that is one thought opened straight
                     * onto its words; a line of a thought and two reads opened
                     * onto "Thought for 3s · <headline>", which opened onto the
                     * headline again in bold -- "i think the first is cleaner,
                     * wanna stick with that?". Inside a step line a thought is
                     * its words, quiet, as the one-thought line draws them.
                     */
                    <div className="lc-steps__thought">
                      <AgentText text={withoutHeadline(entry.text)} streaming={false} />
                    </div>
                  ) : (
                    <>
                      <button type="button" className="lc-filerow lc-filerow--thought" onClick={() => toggle(entry)} aria-expanded={isOpen(entry)}>
                        <Icon name="thought" size={14} />
                        {/* Codex leads its summary with a headline: it names the thought (0.492). */}
                        <span className="lc-filerow__path">
                          {thoughtLine(entry.durationMs)}
                          {thoughtHeadline(entry.text) === undefined ? '' : ` · ${thoughtHeadline(entry.text) ?? ''}`}
                        </span>
                        <span className="lc-activity__chev" aria-hidden="true">
                          <Icon name={isOpen(entry) ? 'chevron-down' : 'chevron-right'} size={12} />
                        </span>
                      </button>
                      {isOpen(entry) && (
                        <div className="lc-filerow__thought">
                          <AgentText text={entry.text} streaming={false} />
                        </div>
                      )}
                    </>
                  )}
                </>
              ) : entry.kind === 'tools' ? (
                /*
                 * A run of plain tool calls, as one row.
                 *
                 * Nothing here changed a file or ran a command -- the fold
                 * refuses to absorb either, and refuses anything that failed
                 * -- so this row is allowed to be quiet. It still NAMES what
                 * it covers and counts what it does not show, because a list
                 * that stops without saying it stopped is the thing this is
                 * copying grok-build to avoid.
                 */
                <div className="lc-filerow is-static lc-filerow--folded">
                  <Icon name="activity" size={14} />
                  <span className="lc-filerow__lead">{foldedToolsLead(entry.names.length, entry.verb)}</span>
                  <span className="lc-filerow__path">{foldedToolsNames(entry.names)}</span>
                  <span className="lc-filerow__result is-muted">done</span>
                </div>
              ) : entry.kind === 'unreported' && entry.observed === true ? (
                /*
                 * A FILE THE HOST SAW CHANGE (0.364): written by a command, or
                 * with no text to diff -- Penny's budget workbook, made by a
                 * Python command. It is a changed file, said as one, and it
                 * opens like one; "OpenCode did not report the change" was
                 * true and told the person nothing they could use.
                 */
                <div className="lc-filerow__line">
                  <div className="lc-filerow is-static">
                    <FileTypeIcon path={entry.name} />
                    <span className="lc-filerow__path lc-filerow__name" title={displayPath(entry.name, workspacePath)} onContextMenu={fileMenuEvent(entry.name)}>{fileName(entry.name)}</span>
                    {twice(entry.name) !== undefined && <span className="lc-filerow__dir">{twice(entry.name)}</span>}
                    {/* The runtime's own word (Write, Edit); not Codex's "changed", which the status says already. */}
                    {entry.tool !== undefined && fileToolWord(entry.tool) !== 'changed' && <span className="lc-filerow__status">{fileToolWord(entry.tool)}</span>}
                    <span className="lc-filerow__result is-muted">changed · seen on disk</span>
                  </div>
                  {openHere !== undefined && (
                    <button
                      type="button"
                      className="lc-filerow__view"
                      title={`Open ${relativePath(entry.name, workspacePath)}`}
                      aria-label={`Open ${relativePath(entry.name, workspacePath)}`}
                      onClick={() => openHere(entry.name)}
                    >
                      <Icon name="maximize" size={13} />
                    </button>
                  )}
                  {elsewhere === undefined && (
                    <button
                      type="button"
                      className="lc-filerow__reveal"
                      title={`Show ${relativePath(entry.name, workspacePath)} in the file manager`}
                      aria-label={`Show ${relativePath(entry.name, workspacePath)} in the file manager`}
                      onClick={() => reveal(entry.name)}
                    >
                      <Icon name="folder" size={13} />
                    </button>
                  )}
                </div>
              ) : (
                // An edit the runtime recorded without the change itself. The
                // row says so, in words: silence here would read as "nothing
                // to see", which is the opposite of what happened.
                <div className="lc-filerow is-static">
                  <Icon name={entry.kind === 'tool' ? 'activity' : 'file'} size={14} />
                  {/*
                    * The sweep on the row that is actually going, and only on
                    * that one. Colin asked for it on "using a tool" as well as
                    * the live line (2026-09-20).
                    *
                    * `finished` is in the condition deliberately: on a turn
                    * that has ended, this row reads "did not report", and a
                    * travelling highlight over it would be the same lie the
                    * word beside it just stopped telling.
                    */}
                  <span
                    className={`lc-filerow__path${entry.settled || finished ? '' : ' lc-sweep'}`}
                    data-text={displayPath(entry.name, workspacePath)}
                    title={entry.name}
                    {...(looksLikeFilePath(entry.name) ? { onContextMenu: fileMenuEvent(entry.name) } : {})}
                  >
                    {displayPath(entry.name, workspacePath)}
                  </span>
                  {entry.tool !== undefined && <span className="lc-filerow__status">{fileToolWord(entry.tool)}</span>}
                  {/*
                    * A FINISHED RUN HAS NOTHING STILL RUNNING IN IT.
                    *
                    * Colin, 2026-09-20, on a completed Antigravity turn whose
                    * rows read `running` and `still running`: "kind of
                    * confused whats happening here". The header said
                    * completed, the model said it was waiting for `pnpm test`,
                    * and two rows claimed to be going — on a turn that had
                    * ended minutes earlier.
                    *
                    * The fold's own trace line had it right the whole time:
                    * `ran 7 commands · 1 did not report`. So the app knew, and
                    * only the rows lied. The helper row has said this
                    * correctly since SURFACES-0.22 §2; shell and tool rows
                    * were never given `finished` to say it with.
                    *
                    * Amber, not red: a tool that never reported is not a tool
                    * that failed, and the difference matters — the command may
                    * well have run.
                    */}
                  <span
                    className={`lc-filerow__result ${entry.settled ? (entry.neverRan !== undefined ? 'is-stalled' : entry.failed ? 'is-failed' : 'is-muted') : finished ? 'is-stalled' : 'is-running'}`}
                    {...(!entry.settled && finished && entry.kind === 'unreported' ? { title: `${runtimeName ?? 'The runtime'} started this change and never said it finished. The file may still have changed.` } : {})}
                  >
                    {!entry.settled
                      // A file call the turn ended without settling was stopped
                      // before it could report -- not a Claude Code failure
                      // (resumed compare file card, 2026-10-05). "Not confirmed", not
                      // "stopped" (0.644): a Cursor turn that completed left four edits
                      // unclosed in its stream, and the edits had landed.
                      ? finished ? (entry.kind === 'unreported' ? 'not confirmed' : 'did not report') : 'still running'
                      : entry.neverRan !== undefined
                        ? entry.neverRan
                        : entry.failed
                        ? 'failed'
                        : 'tooLarge' in entry && entry.tooLarge === true
                          ? 'result too large to keep'
                          : entry.kind === 'tool'
                            ? 'done'
                            : onTheCloud
                              ? 'changed on your cloud computer'
                              : `${runtimeName ?? 'the runtime'} did not report the change`}
                  </span>
                </div>
              )}
              {entry.kind === 'file' && entry.file.status !== 'DELETED' && <ThreadImage path={entry.file.path} />}
              {entry.kind === 'unreported' && !entry.failed && entry.neverRan === undefined && <ThreadImage path={entry.name} />}
              {entry.kind === 'tool' && !entry.failed && entry.neverRan === undefined && fileToolWord(entry.tool ?? '') === 'Read' && <ThreadImage path={entry.name} />}
            </Fragment>
          ))}
          {capped && (
            <button type="button" className="lc-activity__more" onClick={() => setShowAll(true)}>
              Show {entries.length - rowsShown} more
            </button>
          )}
          {revealNotice !== undefined && <p className="lc-filerow__notice">{revealNotice}</p>}
          {/*
            * What the runtime said about this turn, in the fold's FOOTER --
            * behind the fold's own hairline, so it cannot read as the last
            * command's output, and each line naming who said it.
            *
            * These arrive before the first tool call, so the thread's own gate
            * drops them -- Codex comments on its own setup as every turn opens
            * and that belongs nowhere near the top of a conversation. The
            * trace line counted them anyway, as `1 notice`, which meant a
            * number for a sentence that was on no screen at all.
            *
            * THE TONE IS NOT THE RUNTIME'S LEVEL. Amber in this app means "a
            * person may need to act" -- the pending register's own rule, that
            * pending must contain a control -- and a notice here contains
            * none. So a notice is amber only if the reader has something to
            * decide, which none of these do, whatever level the runtime sent
            * (design agent, RULINGS 2026-09-10). Codex's `warning` is a true
            * statement about Codex's situation; amber would be a statement
            * about the reader's. The level is kept on the title -- this
            * changes presentation, not provenance.
            */}
          {notices.length > 0 && (
            <div className="lc-activity__foot">
              {notices.map((notice, index) => (
                <p
                  key={`notice_${String(index)}`}
                  className="lc-shellnotice lc-tone-muted"
                  title={`${runtimeDisplayName(notice.source)} called this ${notice.level === 'info' ? 'a note' : `a ${notice.level}`}`}
                >
                  <span className="lc-shellnotice__source lc-mono">{runtimeDisplayName(notice.source)}</span>
                  {notice.message}
                </p>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * What became of a helper, in the row's words.
 *
 * A helper SENT TO THE BACKGROUND (0.492) returns at once -- the call's result
 * is only the runtime saying it launched -- so "reported back" on it was the
 * app claiming a report nobody had made. It works in the background, saying
 * what it is doing, until the runtime says it came back.
 */
/** "1 call", "7 calls": how much a helper did, on its row. */
export function helperCallCount(calls: readonly ActivityEntry[]): string {
  return calls.length === 1 ? '1 call' : `${String(calls.length)} calls`
}

/**
 * A helper's own calls, under its row, one step in (helper visibility,
 * 2026-10-05). The rows the teammate's calls use, in their words: what was
 * read, searched, run or changed, and how it ended. A turn that has ended has
 * nothing still running in it, so an unsettled call there "did not report",
 * as the teammate's own rows say.
 */
export function HelperCalls({
  calls,
  finished,
  workspacePath
}: {
  readonly calls: readonly ActivityEntry[]
  readonly finished: boolean
  readonly workspacePath: string | undefined
}): ReactElement {
  const ended = (settled: boolean, failed: boolean, neverRan?: string): { readonly word: string; readonly tone: string } =>
    !settled
      ? finished ? { word: 'did not report', tone: 'is-stalled' } : { word: 'running', tone: 'is-running' }
      : neverRan !== undefined ? { word: neverRan, tone: 'is-stalled' } : failed ? { word: 'failed', tone: 'is-failed' } : { word: 'done', tone: 'is-muted' }
  return (
    <div className="lc-helper__calls" role="list" aria-label="What the helper did">
      {calls.map((call) => {
        if (call.kind === 'file') {
          return (
            <div key={call.key} role="listitem" className="lc-filerow is-static">
              <Icon name="file" size={14} />
              <span className="lc-filerow__path" title={call.file.path} onContextMenu={fileMenuEvent(call.file.path)}>{displayPath(call.file.path, workspacePath)}</span>
              <span className="lc-filerow__status">{call.file.status}</span>
              {call.large && <span className="lc-filerow__status is-large">LARGE</span>}
              {(() => {
                const counts = fileRowCounts(call)
                if (counts === undefined) return null
                return (
                  <span className="lc-filerow__result">
                    <span className="lc-diff__addmark">+{counts.added}</span>
                    <span className="lc-diff__delmark">−{counts.removed}</span>
                  </span>
                )
              })()}
            </div>
          )
        }
        if (call.kind === 'shell') {
          const result = ended(call.settled, call.failed, call.refused === undefined ? undefined : call.declined === true ? 'declined' : 'refused')
          return (
            <div key={call.key} role="listitem" className="lc-filerow is-shell is-static">
              <Icon name="terminal" size={14} />
              <span className="lc-filerow__path lc-mono" title={call.command}>{call.title ?? call.command}</span>
              <span className={`lc-filerow__result ${result.tone}`}>{result.word}</span>
            </div>
          )
        }
        if (call.kind === 'tool' || call.kind === 'unreported') {
          const result = ended(call.settled, call.failed, call.neverRan)
          return (
            <div key={call.key} role="listitem" className="lc-filerow is-static">
              <Icon name={call.kind === 'tool' ? 'activity' : 'file'} size={14} />
              <span className="lc-filerow__path" title={call.name} {...(looksLikeFilePath(call.name) ? { onContextMenu: fileMenuEvent(call.name) } : {})}>{displayPath(call.name, workspacePath)}</span>
              {call.tool !== undefined && <span className="lc-filerow__status">{fileToolWord(call.tool)}</span>}
              <span className={`lc-filerow__result ${result.tone}`}>{result.word}</span>
            </div>
          )
        }
        if (call.kind === 'helper') {
          const result = ended(call.settled, call.failed)
          return (
            <div key={call.key} role="listitem" className="lc-filerow is-static is-helper">
              <Icon name="users" size={14} />
              <span className="lc-filerow__path">{call.description}</span>
              <span className="lc-filerow__status">{call.subagentType === undefined ? 'subagent' : `${call.subagentType} subagent`}</span>
              <span className={`lc-filerow__result ${result.tone}`}>{result.word}</span>
            </div>
          )
        }
        return null
      })}
    </div>
  )
}

/** A thought's words without the headline its line already shows (0.493). */
export function withoutHeadline(text: string): string {
  return thoughtHeadline(text) === undefined ? text : text.replace(/^\s*\*\*[^*\n]{2,80}\*\*\s*/, '')
}

/**
 * A helper's one-line summary as words (0.625). It is the helper's own text,
 * often Markdown, set on one line beside its row -- where "**33** files total"
 * read with its asterisks (the helper drive's frame, 2026-10-05). The marks
 * go; the words, and code spans' contents, stay.
 */
export function plainSummary(text: string): string {
  return text
    .replace(/^\s*#{1,6}\s+/, '')
    .replace(/\*\*([^*\n]+)\*\*/g, '$1')
    .replace(/__([^_\n]+)__/g, '$1')
    .replace(/`([^`\n]+)`/g, '$1')
    .replace(/(^|[\s(])\*([^*\s][^*\n]*?)\*(?=$|[\s).,!?:;])/g, '$1$2')
    .replace(/\s+/g, ' ')
    .trim()
}

export function helperResult(entry: Extract<ActivityEntry, { kind: 'helper' }>, finished: boolean): string {
  if (!entry.settled) return finished ? 'did not report' : 'working on it'
  if (entry.failed) return 'failed'
  const back = entry.summary === undefined ? 'reported back' : `reported back · ${plainSummary(entry.summary)}`
  if (entry.background !== true) return back
  switch (entry.backgroundEnded) {
    case 'completed':
      return back
    case 'failed':
      return 'failed'
    case 'stopped':
      return 'stopped'
    case 'stopped-with-run':
      return 'stopped when the run ended'
    case 'ended':
      return 'ended'
    case undefined:
      return finished ? 'did not report' : `working in the background${entry.progress === undefined ? '' : ` · ${entry.progress}`}`
  }
}

function helperTone(entry: Extract<ActivityEntry, { kind: 'helper' }>, finished: boolean): string {
  if (!entry.settled) return finished ? 'is-stalled' : 'is-running'
  if (entry.failed || entry.backgroundEnded === 'failed') return 'is-failed'
  if (entry.background === true && entry.backgroundEnded === undefined) return finished ? 'is-stalled' : 'is-running'
  if (entry.background === true && entry.backgroundEnded !== 'completed') return 'is-stalled'
  return 'is-muted'
}

function shellResult(entry: Extract<ActivityEntry, { kind: 'shell' }>, finished = false): string {
  // See the tool row above: a command the run ended without settling did not
  // report, and saying `running` about it contradicts the header beside it.
  if (!entry.settled) return finished ? 'did not report' : 'running'
  // Refused is not failed: the command never ran.
  if (entry.refused !== undefined) return entry.declined === true ? 'declined' : 'refused'
  if (entry.failed) return entry.exitCode === undefined ? 'failed' : `failed · exit ${String(entry.exitCode)}`
  /*
   * A call sent to the background returns at once, so `done` on it was a
   * claim about the COMMAND that nobody had made: the call was done, the
   * work had not started finishing. Green `done` beside "in the background"
   * read as finished work, which is the misreading Colin reported on
   * 2026-09-21. The word follows the work.
   */
  if (entry.background === true) {
    switch (entry.backgroundEnded) {
      case 'completed':
        return 'done'
      case 'failed':
        return 'failed'
      case 'stopped':
      case 'stopped-with-run':
        return 'stopped'
      case 'ended':
        return 'ended'
      case undefined:
        return finished ? 'did not report' : 'running'
    }
  }
  return entry.exitCode === undefined ? 'done' : `exit ${String(entry.exitCode)}`
}

/**
 * Whether the command is still running, for the sweep across its text: the
 * call is still open, or it went to the background and nothing has said the
 * work ended. Never once the run is over -- a sweep then would be the row
 * claiming something is happening after everything stopped.
 */
function shellSweeping(entry: Extract<ActivityEntry, { kind: 'shell' }>, finished: boolean): boolean {
  if (finished) return false
  return !entry.settled || (entry.background === true && entry.backgroundEnded === undefined && !entry.failed)
}

/** Why the runtime refused a command, for the badge's hover. */
function refusedWhy(entry: Extract<ActivityEntry, { kind: 'shell' }>): string | undefined {
  if (entry.refused === undefined) return undefined
  return entry.refused.length > 0 ? `Refused before it ran: ${entry.refused}` : 'Refused before it ran'
}

function shellResultClass(entry: Extract<ActivityEntry, { kind: 'shell' }>, finished = false): string {
  if (!entry.settled) return finished ? 'is-stalled' : 'is-running'
  if (entry.refused !== undefined) return 'is-stalled'
  if (entry.failed) return 'is-failed'
  if (entry.background === true) {
    switch (entry.backgroundEnded) {
      case 'completed':
        return 'is-ok'
      case 'failed':
        return 'is-failed'
      case undefined:
        return finished ? 'is-stalled' : 'is-running'
      default:
        return 'is-stalled'
    }
  }
  return 'is-ok'
}

/**
 * Where the work went, and for the one ending that needs it, why it stopped.
 *
 * `stopped` alone would leave the person to guess who stopped it. Claude Code
 * stops its background work when its run ends -- measured 2026-09-22, the
 * command killed right after the answer -- and that is the case a person can
 * do something about: ask for it again, run in the foreground.
 */
/*
 * A command that acted on programs this run did not start (0.578), said on
 * its row. In Auto no card is asked, so the row is the only place left to
 * say that `taskkill //F //IM python.exe` stopped every Python on the
 * computer. Not on a refused command: it never ran.
 */
function ReachBadge({ entry }: { readonly entry: Extract<ActivityEntry, { kind: 'shell' }> }) {
  if (entry.refused !== undefined) return null
  const reach = commandReach(entry.command)
  if (reach === undefined) return null
  return (
    <span className="lc-shellbadge is-reach lc-mono" title={reach.said} data-reach={reach.kind}>
      {reach.short}
    </span>
  )
}

/* Codex's Windows sandbox did not start, so the command never ran (0.736, shared/codex-sandbox.ts). */
function SandboxBadge({ entry }: { readonly entry: Extract<ActivityEntry, { kind: 'shell' }> }) {
  if (entry.failed !== true || !codexSandboxDidNotStart(entry.output)) return null
  return (
    <span className="lc-shellbadge is-reach lc-mono" title={CODEX_SANDBOX_DID_NOT_START}>
      sandbox did not start
    </span>
  )
}

function BackgroundBadge({ entry }: { readonly entry: Extract<ActivityEntry, { kind: 'shell' }> }) {
  if (entry.background !== true) return null
  if (entry.backgroundEnded === 'stopped-with-run') {
    return (
      <span
        className="lc-shellbadge is-background lc-mono"
        title="The teammate's run ended while this was still running in the background, and the work stopped with it. Ask for it again and it can run in the foreground."
      >
        when the run ended
      </span>
    )
  }
  return <span className="lc-shellbadge is-background lc-mono">in the background</span>
}
