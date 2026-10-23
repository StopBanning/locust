import type { NormalizedRuntimeEvent, ToolPatch } from '@teammate/runtime-adapters'
import { providerErrorSentence } from '@teammate/runtime-adapters/provider-error'

import { cloudRelative } from '../../shared/cloud-computer.js'
import { SUBAGENT_TOOL } from './faceState.js'
import { EVENT_WINDOW, TRIMMED_TURN_LINE, windowEvents } from '../../shared/event-window.js'
import { isImagePath } from '../../shared/image-files.js'
import { codexSandboxDidNotStart } from '../../shared/codex-sandbox.js'
import { READ_TOOL_WORDS, byHelper, editToolName, isEditCommand, isShellTool } from '../../shared/tool-kinds.js'
import { LARGE_FILE_LINES, fileCounts, parseUnifiedDiff } from './diff.js'
import type { DiffCounts, DiffFile } from './diff.js'
export { fileRowCounts } from './diff.js'

import type { MissionRuntimeId } from '@teammate/runtime-adapters'

import type { PublicPeerMessage, PublicRecoveredMission } from '../../shared/ipc.js'
import { splitAttachments } from '../../shared/attachments.js'
import { stripShareBlocks } from '../../shared/peer-share.js'
import { unwrapProtocolTags } from '../../shared/protocolTags.js'
import { runtimeDisplayName } from '../../shared/runtimes.js'
import { stripTaskBlocks } from '../../shared/room-task.js'
import { stripMemoryBlocks } from '../../shared/memory.js'
import { parseDecision, stripDecisionBlocks } from '../../shared/decision.js'
import { parseFileBlocks, refusedFileLines, stripFileBlocks } from '../../shared/handover.js'
import type { HandedFile } from '../../shared/handover.js'
import type { DecisionRequest } from '../../shared/decision.js'
import { isTidyPrompt } from '../../shared/memory-tidy.js'
import { stepWordsOf } from '../../shared/hand-off.js'
import { taggedWordsOf } from '../../shared/tagging.js'

/**
 * Turns the normalized event stream into the thread the workroom renders.
 *
 * This is the layer where "logs never dump into the thread" is actually
 * enforced: raw events go to the Signal Rail, and the thread shows semantic
 * items derived here. Everything is a pure function of the events, so the UI
 * cannot show a step, a count, or a piece of text the provider did not send.
 */

export interface ActivityDetail {
  readonly kind: string
  readonly name: string
  /**
   * What the runtime called the tool. `name` is often the target -- a path,
   * a command -- so without this a read of a file it also edited renders as
   * the same path twice with nothing to tell them apart.
   */
  readonly tool?: string
  /** What the model said it was doing, on the runtimes that carry one. */
  readonly title?: string
  /**
   * The runtime was asked to run this one in the background.
   *
   * Locust has no background-task feature and this is not one: the row says
   * what the call was, and a call that was sent to the background is a
   * different thing from one that was waited on.
   */
  readonly background?: boolean
  /**
   * What became of the background work, when the runtime said.
   *
   * The call returns at once and the work goes on, so `settled` answers for
   * the call and this answers for the work. Only Claude Code reports it
   * (`task_notification`, 2026-09-22); undefined means nobody said, which is
   * a different fact from "still running" once the run is over.
   */
  readonly backgroundEnded?: BackgroundEnding
  readonly settled: boolean
  /** True only for a tool the runtime itself reported as failed. */
  readonly failed?: boolean
  readonly exitCode?: number
  /** The change the tool made, when the runtime reported one. */
  readonly patch?: ToolPatch
  /** The runtime's own status word for the call; a subagent's type, for its launcher. */
  readonly status?: string
  /** How long a reasoning step or a command took, when both ends were seen. Drawn as "Thought for 12s", or a command's "12s". */
  readonly durationMs?: number
  /** What the tool returned, when the runtime reported it in words; a subagent's summary. */
  readonly output?: string
  /** What work sent to the background is doing now, while it runs (Claude Code's `task_progress`). */
  readonly progress?: string
  /**
   * A helper's own calls, in the order it made them (ledger v22, helper
   * visibility 2026-10-05): what it read, searched, ran and changed between
   * being sent out and reporting back. Only on a helper's row, and only where
   * the runtime says which calls were the helper's (Claude Code today).
   */
  readonly children?: readonly ActivityDetail[]
}

/**
 * How background work ended: `stopped-with-run` is the run ending under it,
 * told apart from a stop somebody chose mid-run (see `backgroundEnding` in the
 * Claude adapter).
 */
export type BackgroundEnding = 'completed' | 'failed' | 'stopped' | 'stopped-with-run' | 'ended'

const BACKGROUND_ENDINGS: readonly BackgroundEnding[] = ['completed', 'failed', 'stopped', 'stopped-with-run', 'ended']

/** A ledger's word for it, or `ended` for one this build does not know. */
function backgroundEndingOf(status: string | undefined, failed: boolean): BackgroundEnding {
  if (failed) return 'failed'
  return BACKGROUND_ENDINGS.find((ending) => ending === status) ?? 'ended'
}

/**
 * An activity row, ready to draw. Files carry their parsed diff; shell rows
 * carry their result; an edit whose runtime reported no patch stays in the
 * list as an `unreported` row rather than vanishing, because a missing row
 * would understate what the teammate did.
 */
/** How many of a folded run's names the row shows before it starts counting. */
export const FOLDED_TOOL_NAMES_SHOWN = 3

/**
 * Fold runs of consecutive plain tool rows into one.
 *
 * The rule is narrow on purpose: a row folds only if it is a plain `tool`,
 * SETTLED and not failed. A command, a file change, an unreported change, a
 * helper, anything still running and anything that failed keeps its own row.
 * So the fold can never hide work that changed something or went wrong -- it
 * only ever quietens rows that were already quiet.
 *
 * A single row is left alone: folding one thing into "1 tool call" would be
 * the same row with the name taken off it.
 */
export function foldPlainToolRuns(entries: readonly ActivityEntry[]): readonly ActivityEntry[] {
  const foldable = (entry: ActivityEntry): boolean =>
    entry.kind === 'tool' && entry.settled && !entry.failed && !isImagePath(entry.name)
  const out: ActivityEntry[] = []
  let index = 0
  while (index < entries.length) {
    const entry = entries[index]
    if (entry === undefined) break
    if (!foldable(entry)) {
      out.push(entry)
      index += 1
      continue
    }
    let end = index + 1
    while (end < entries.length && foldable(entries[end]!)) end += 1
    const run = entries.slice(index, end)
    if (run.length < 2) {
      out.push(entry)
    } else {
      const names = run.map((row) => (row.kind === 'tool' ? row.name : '')).filter((name) => name.length > 0)
      const verbs = new Set(run.map((row) => (row.kind === 'tool' ? row.tool : undefined)))
      const verb = verbs.size === 1 ? [...verbs][0] : undefined
      out.push({
        kind: 'tools',
        key: `tools_${entry.key}`,
        names,
        ...(verb === undefined ? { verb: undefined } : { verb })
      })
    }
    index = end
  }
  return out
}

/**
 * The names a folded row shows, and what it says about the rest.
 *
 * grok-build's rule, which this repo did not have written down anywhere:
 * never truncate silently. An over-long list ends by saying how much of it is
 * not being shown, so a reader can tell a short list from a cut one.
 */
export function foldedToolsText(names: readonly string[], verb: string | undefined): string {
  return `${foldedToolsLead(names.length, verb)} — ${foldedToolsNames(names)}`
}

/**
 * What a folded row did, in Activity's own words (0.550, Sol on 0.546: the
 * group read "read 3", in code type, under rows that say "Read"): "Read 3
 * files", "Searched 2 times". A tool with no word of its own is counted.
 */
export function foldedToolsLead(count: number, verb: string | undefined): string {
  const said = verb === undefined ? undefined : railToolName({ name: verb, toolKind: verb })
  const one = said === undefined ? undefined : /^(Read|Ran|Changed) a (file|command)$/.exec(said)
  if (one !== null && one !== undefined) return `${one[1]!} ${String(count)} ${one[2]!}s`
  if (said === 'Searched' || said === 'Searched the web' || said === 'Opened a page') return `${said} ${String(count)} times`
  return `${String(count)} tool calls`
}

export function foldedToolsNames(names: readonly string[]): string {
  const shown = names.slice(0, FOLDED_TOOL_NAMES_SHOWN).join(', ')
  const rest = names.length - FOLDED_TOOL_NAMES_SHOWN
  return rest > 0 ? `${shown} … ${String(rest)} more` : shown
}

export type ActivityEntry =
  | {
      /** What a model thought, when its runtime reported it. Never a tool. */
      readonly kind: 'thought'
      readonly key: string
      readonly text: string
      readonly durationMs?: number
    }
  | {
      readonly kind: 'file'
      readonly key: string
      readonly file: DiffFile
      readonly counts: DiffCounts
      readonly truncated: boolean
      /** The runtime's own total, when the recorded text was cut short. */
      readonly reported: DiffCounts | undefined
      readonly large: boolean
      /** Seen changed on disk by the host, and never named by the runtime (0.491). */
      readonly observed?: true
      /** The host's look at the file after the run: its NET change, first state to last (0.494). */
      readonly net?: true
    }
  | {
      readonly kind: 'shell'
      readonly key: string
      readonly command: string
      /**
       * What the model said it was doing, where the runtime carries it.
       *
       * Claude Code's Bash tool takes a `description` on every call and the
       * model fills it in; it is the whole reason its own transcript reads
       * "Checked what the app says about the free route" rather than a shell
       * pipeline. Locust dropped it and drew the pipeline.
       *
       * The row leads with this and keeps the command underneath, because
       * the command is evidence of what ran on this machine and the
       * description is only a claim about it. Undefined on the runtimes that
       * send none -- Codex has no such field at all -- and those rows read
       * exactly as they did.
       */
      readonly title: string | undefined
      /**
       * Sent to the background rather than waited on.
       *
       * Claude Code's Bash tool takes `run_in_background`, and until
       * 2026-09-22 the adapter read the command and the description off that
       * same input and ignored this. So a call the runtime was told not to
       * wait for looked exactly like one it waited for, which is most of why
       * a finished background task reads as a turn that simply stopped.
       *
       * This says what the call WAS. Nothing re-invokes anybody on it.
       */
      readonly background?: boolean
      /** What became of that work, when the runtime said; see `ActivityDetail`. */
      readonly backgroundEnded?: BackgroundEnding
      readonly settled: boolean
      readonly failed: boolean
      readonly exitCode: number | undefined
      /**
       * How long it ran, start to finish as the runtime reported them (0.459,
       * as Devin's worklog shows it). Drawn only when it is a second or more
       * and the command was waited on: a runtime that reports both ends at
       * once would otherwise claim every command took no time at all.
       */
      readonly durationMs?: number
      /**
       * The runtime refused it before it ran, with its reason ('' when it gave
       * none). Not a failure: nothing ran (see `refusedCalls` in the Claude
       * adapter).
       */
      /** Declined by the person on a card, not refused by the mode. */
      readonly declined?: true
      readonly refused?: string
      /**
       * What the command printed, where the runtime reported it.
       *
       * Undefined is the ordinary case, not a failure: only the Codex exec
       * stream carries `aggregated_output` today. A row with nothing to show
       * simply does not offer to show anything -- which is why this is a
       * capability the row reads off the data rather than a promise made in
       * advance and then broken per runtime.
       *
       * Already redacted when it gets here: the adapter runs it through
       * `sanitizeJson`, pinned by a test that puts a bearer token in a command's
       * output and asserts it never reaches the event.
       */
      readonly output?: string
    }
  | {
      /**
       * A helper the runtime started for itself: Claude Code's Task tool,
       * OpenCode's task tool. Until now one plain tool row; what the helper
       * did inside is not reported by the runtime, so the row says what it
       * was asked and whether it reported back (Colin, 2026-09-05).
       */
      readonly kind: 'helper'
      /** Sent to the background: launched and left working while the teammate went on (0.492). */
      readonly background?: boolean
      readonly backgroundEnded?: BackgroundEnding
      /** What it is doing now, while it works in the background. */
      readonly progress?: string
      /** The subagent's type when the runtime said (Claude Code's Explore, general-purpose, ...). */
      readonly subagentType?: string
      /** Its one-line summary when it reported back. */
      readonly summary?: string
      /**
       * The helper's own calls, drawn the way the teammate's are, folded
       * under its row (helper visibility). Absent where the runtime did not
       * say what the helper did -- every ledger before v22, and every runtime
       * but Claude Code -- and the row then reads as it always has.
       */
      readonly calls?: readonly ActivityEntry[]
      readonly key: string
      readonly description: string
      readonly settled: boolean
      readonly failed: boolean
    }
  | {
      /**
       * A run of consecutive plain tool calls, folded into one row.
       *
       * grok-build's `group_tool_verbs`, read 2026-09-13: runs of read /
       * search / list fold into one line while edits keep their own, and
       * `collapsed_edit_blocks` keeps `+N/-M` visible even collapsed. A turn
       * that reads eleven files drew eleven rows of equal weight here, and
       * the one edit among them looked exactly like the ten reads.
       *
       * Only rows that changed NOTHING fold. A command, a file change, an
       * unreported change, a helper and anything that FAILED all keep their
       * own row, always -- the fold may only ever quieten what was already
       * quiet.
       */
      readonly kind: 'tools'
      readonly key: string
      /** Every name in the run, in order. The row shows some and counts the rest. */
      readonly names: readonly string[]
      /** The shared verb when they all share one, so the row can lead with it. */
      readonly verb: string | undefined
    }
  | {
      readonly kind: 'unreported' | 'tool'
      readonly key: string
      readonly name: string
      /** The runtime's own word for what it did: `read`, `search`, `list`. */
      readonly tool: string | undefined
      readonly settled: boolean
      readonly failed: boolean
      /**
       * It never ran: the person declined it on a card, or the mode refused
       * it. Said as that word, not "failed" -- a fresh-profile beta report of
       * 0.345 found a declined edit's row reading "edit failed" beside a
       * summary that said "1 refused".
       */
      readonly neverRan?: 'declined' | 'refused'
      /** Seen changed on disk by the host rather than reported by the runtime (0.364). */
      readonly observed?: true
      /**
       * The runtime did report; the record was past the cap, so the diff is
       * not here. Said as that, not as a change nobody confirmed.
       */
      readonly tooLarge?: true
    }

/**
 * Expand each activity detail into the rows the card draws. One patch can
 * touch several files, and each becomes its own row -- the same rows the
 * counts are summed from, so the card's total and the diffs beneath it are
 * two views of one array and cannot disagree.
 */
export function activityEntries(
  details: readonly ActivityDetail[],
  /** The folder this ran in, so one file reported two ways is one row. */
  workspacePath?: string,
  /** False for a helper's calls, already folded once under its row: each is its own row. */
  foldPlain = true
): readonly ActivityEntry[] {
  const entries: ActivityEntry[] = []
  // An edit some runtimes report twice is still one edit.
  //
  // MEASURED 2026-09-07, OpenCode, one append to one file: the fold drew
  // `notes.md MODIFIED +1 -0` TWICE, the line above said `1 file +2 -0`, and
  // git said `1 0 notes.md` -- one line, in one file, appearing once in the
  // file on disk. A first outside tester reported the same shape as
  // `2 files / +2 -0` against a one-line change.
  //
  // The key is the file's PATH AS DRAWN plus every hunk and row, so this only
  // ever folds a restatement of one change. The path has to go through
  // `relativePath` first for the same reason the count does: measured across
  // six runs of the same one-line append, OpenCode reported the edit twice on
  // two of them, once relatively and once absolutely, and a raw-path key saw
  // two files where the fold drew the same row twice.
  //
  // Two real edits to one file cannot collide: the second is diffed against a
  // file the first already changed, so its rows differ -- and when they do,
  // both are drawn and both are counted, which is correct.
  const seenFiles = new Set<string>()
  details.forEach((detail, index) => {
    const failed = detail.failed === true
    /*
     * A row of its own, so it is never folded into a run of tool calls.
     *
     * `foldedToolsText` gathers consecutive foldable rows and names them --
     * which is how `thought` ended up in "6 tool calls - thought, mcp, mcp".
     * Thinking sits between tool calls constantly, so left foldable it would
     * also BREAK those runs in two and stop them collapsing at all.
     */
    if (detail.kind === 'reasoning') {
      entries.push({
        kind: 'thought',
        key: `thought_${String(index)}`,
        text: detail.output ?? '',
        ...(detail.durationMs === undefined ? {} : { durationMs: detail.durationMs })
      })
      return
    }
    if (detail.kind === 'shell') {
      entries.push({
        kind: 'shell',
        key: `shell_${String(index)}`,
        command: shellCommandText(detail.name),
        title: detail.title,
        ...(detail.background === true ? { background: true } : {}),
        ...(detail.backgroundEnded === undefined ? {} : { backgroundEnded: detail.backgroundEnded }),
        settled: detail.settled,
        failed,
        exitCode: detail.exitCode,
        ...(detail.durationMs === undefined ? {} : { durationMs: detail.durationMs }),
        ...(detail.status === 'refused' || detail.status === 'declined' ? { refused: detail.output ?? '' } : {}),
        ...(detail.status === 'declined' ? { declined: true } : {}),
        /*
         * Carried whenever the runtime SAID something about output, including
         * when what it said was "none".
         *
         * It was captured by the adapter and dropped here, so a teammate could
         * run `seq 1 1200`, say "printed 1 through 1200", and leave the person
         * looking at a row that said `done` and nothing else (drive-huge-turn,
         * 2026-09-08).
         *
         * The empty string used to be dropped along with the absent case, and
         * that collapsed two different facts into one: `undefined` is a
         * runtime that reports no output at all -- five of the six -- and `''`
         * is one that ran the command and it printed nothing. The row draws
         * "no output" for the second and must stay silent for the first, so
         * they cannot arrive here as the same value. Caught by rendering the
         * card: the case was written, and unreachable.
         */
        ...(typeof detail.output === 'string' ? { output: detail.output } : {})
      })
      return
    }
    const patch = detail.patch
    const files = patch === undefined ? [] : parseUnifiedDiff(patch.text)
    if (files.length === 0) {
      // An edit whose runtime named its files but sent no diff (Codex's
      // file_change) is one row PER FILE, each saying the change was not
      // reported -- never one row named after the tool with no path at all.
      // MEASURED 2026-09-03 in a real run, correcting a first attempt: for a
      // Codex `file_change` the detail's NAME is the joined paths (they
      // arrive as the tool's command) while its TOOL is the literal string
      // `file_change`. Requiring name and tool to be equal -- which they
      // never are -- meant the card kept drawing ONE row with both paths run
      // together, labelled `file_change`, saying the change was not reported.
      // Any edit naming more than one path splits, whatever the tool is called.
      const named = detail.kind === 'edit' && detail.name.includes('\n')
        ? detail.name.split('\n').map((path) => path.trim()).filter((path) => path.length > 0)
        : undefined
      if (named !== undefined && named.length > 0) {
        named.forEach((path, fileIndex) => {
          entries.push({
            kind: 'unreported',
            key: `item_${String(index)}_${String(fileIndex)}`,
            name: path,
            tool: undefined,
            settled: detail.settled,
            failed
          })
        })
        return
      }
      if (detail.kind === 'helper') {
        entries.push({
          kind: 'helper',
          key: `helper_${String(entries.length)}`,
          // The target is what the helper was asked; without one, say so.
          description: detail.tool !== undefined && detail.tool !== detail.name ? detail.name : subagentVerb(detail.tool),
          // Claude Code's launcher carries the subagent's type in status;
          // Codex's carries the call's lifecycle word, which is not a type.
          ...(detail.status === undefined || /^(error|completed|failed|in_progress|started|cancelled)$/i.test(detail.status) ? {} : { subagentType: detail.status }),
          ...(detail.output === undefined ? {} : { summary: detail.output }),
          ...(detail.background === true ? { background: true } : {}),
          ...(detail.backgroundEnded === undefined ? {} : { backgroundEnded: detail.backgroundEnded }),
          ...(detail.progress === undefined ? {} : { progress: detail.progress }),
          ...(detail.children === undefined || detail.children.length === 0 ? {} : { calls: activityEntries(detail.children, workspacePath, false) }),
          settled: detail.settled,
          failed: detail.failed === true
        })
        return
      }
      // A plan update or a wait that names nothing reads as what it did, never
      // as the runtime's tool id: Colin saw Antigravity's bare `manage_task
      // done` (0.541), which was a check on a background command (0.542).
      const skill = detail.kind === 'edit' ? undefined : skillCalled(detail.tool ?? detail.name, detail.tool === undefined || detail.tool === detail.name ? '' : detail.name)
      if (skill !== undefined) {
        entries.push({ kind: 'tool', key: `item_${String(index)}`, name: skillWords(skill, false), tool: undefined, settled: detail.settled, failed })
        return
      }
      const looked = detail.kind === 'edit' || (detail.tool !== undefined && detail.tool !== detail.name) ? undefined : toolLooksAt(detail.tool ?? detail.name)
      if (looked === 'plan' || looked === 'wait') {
        entries.push({ kind: 'tool', key: `item_${String(index)}`, name: looked === 'plan' ? 'Updated the plan' : 'Checked on a command', tool: undefined, settled: detail.settled, failed })
        return
      }
      // A tool the app has words for reads as what it did (0.610): Codex's image tool was the row "imageGeneration done".
      const known = detail.kind === 'edit' ? undefined : toolWords(detail.tool ?? detail.name)
      if (known !== undefined) {
        entries.push({ kind: 'tool', key: `item_${String(index)}`, name: `${known.did(1).charAt(0).toUpperCase()}${known.did(1).slice(1)}`, tool: undefined, settled: detail.settled, failed })
        return
      }
      entries.push({
        kind: detail.kind === 'edit' ? 'unreported' : 'tool',
        key: `item_${String(index)}`,
        // A read's name is the path it read, and OpenCode reports it whole:
        // the fold said "read 3 — C:\Users\<home>\Documents\locust-scratch\
        // locust-walk-ws-EfOL3P, C:\Users\..." and was cut off there (Yurt's
        // beta report, #16). Relative to the folder like every file row; a
        // name that is not a path comes back as it was.
        name: relativePath(detail.name, workspacePath),
        tool: detail.tool === detail.name ? undefined : detail.tool,
        settled: detail.settled,
        failed,
        ...(detail.status === 'declined' || detail.status === 'refused' ? { neverRan: detail.status } : {}),
        // Seen changed on disk by the host, not reported by the runtime: a
        // file a command wrote, or one with no text to diff (0.364).
        ...(detail.kind === 'edit' && /on disk|from disk/.test(detail.status ?? '') ? { observed: true } : {}),
        ...(detail.status === 'result too large to keep' ? { tooLarge: true as const } : {})
      })
      return
    }
    files.forEach((file, fileIndex) => {
      const signature = JSON.stringify({
        ...file,
        path: relativePath(file.path, workspacePath).replace(/[\\/]+/g, '/').toLowerCase()
      })
      if (seenFiles.has(signature)) return
      seenFiles.add(signature)
      const counts = fileCounts(file)
      // A single-file patch can be checked against the runtime's own total;
      // across several files the total belongs to none of them, so it is
      // withheld rather than repeated on each row as if it were theirs.
      const reported =
        patch !== undefined && patch.truncated && files.length === 1
          ? { added: patch.added, removed: patch.removed }
          : undefined
      const totalLines = reported !== undefined ? reported.added + reported.removed : counts.added + counts.removed
      const large = totalLines > LARGE_FILE_LINES || patch?.truncated === true
      entries.push({
        kind: 'file',
        key: `file_${String(index)}_${String(fileIndex)}`,
        file,
        counts,
        truncated: patch?.truncated === true,
        reported,
        large,
        ...(detail.status === 'observed on disk' ? { observed: true as const } : {}),
        ...(/on disk|from disk/.test(detail.status ?? '') ? { net: true as const } : {})
      })
    })
  })
  return foldPlain ? foldPlainToolRuns(entries) : entries
}

/**
 * Whether a path is this turn's folder (or a relative name inside it).
 *
 * A resume that wrote into a scratch tree outside the compare copy still
 * named those Writes as edits of the turn; the column foot then said
 * "Edited 44 files" over a folder that only changed one (2026-10-05).
 * Outside paths are another folder's story.
 */
export function pathInWorkspace(path: string, workspacePath: string | undefined): boolean {
  if (workspacePath === undefined || workspacePath.length === 0) return true
  const normalise = (value: string): string => value.replace(/[\\/]+/g, '/').replace(/\/$/, '')
  const root = normalise(workspacePath)
  const full = normalise(path)
  if (full.toLowerCase() === root.toLowerCase()) return true
  if (full.toLowerCase().startsWith(`${root.toLowerCase()}/`)) return true
  // Already relative to the folder (a runtime naming a file by its name).
  if (!/^(?:[a-z]:)?\//i.test(full)) return true
  // A compare/routine copy of this folder reads as inside once relativePath maps it.
  const relative = relativePath(path, workspacePath)
  return relative !== path && !/^(?:[a-z]:)?[\\/]/i.test(relative)
}

/**
 * ONE ROW PER FILE, for a turn's files (0.494). Where the host looked at the
 * file after the run, its net change -- first state to last -- is the file's
 * row, and the runtime's own step diffs of it are steps, not more of it. Sol's
 * 0.492 pass: "Edited 10 files" over twelve rows, a file edited twice counted
 * twice and the total the sum of both. With no look (a folder too large to
 * walk), each step's diff stays: that is all there is to show.
 *
 * Unreported rows (a Write with a path and no diff) used to keep every call:
 * Write then Edit of the same file was two files on the card (resumed compare,
 * 2026-10-05). One path is one file, whichever call named it.
 */
export function netFileEntries(entries: readonly ActivityEntry[], workspacePath?: string): readonly ActivityEntry[] {
  const key = (path: string): string => relativePath(path, workspacePath).replace(/[\\/]+/g, '/').toLowerCase()
  const pathOf = (entry: ActivityEntry): string | undefined =>
    entry.kind === 'file' ? key(entry.file.path) : entry.kind === 'unreported' ? key(entry.name) : undefined
  const netPaths = new Set(entries.flatMap((entry) => (entry.kind === 'file' && entry.net === true ? [key(entry.file.path)] : [])))
  /*
   * A FILE THE HOST SAW CHANGE BUT COULD NOT READ (0.672): over 64 KB, or in
   * a folder past its limits. Its row said "changed · seen on disk" with no
   * counts, beside a step that had reported the change exactly. ONE step that
   * changed the file is its net change, and stands in for the host's row;
   * several are not -- each is part of it -- and the host's word stays.
   */
  const stepsOf = new Map<string, number>()
  for (const entry of entries) {
    if (entry.kind === 'file' && entry.net !== true) stepsOf.set(key(entry.file.path), (stepsOf.get(key(entry.file.path)) ?? 0) + 1)
  }
  const unreadPaths = new Set(entries.flatMap((entry) => (entry.kind === 'unreported' && entry.observed === true ? [key(entry.name)] : [])))
  const withoutSteps = entries.filter((entry) => {
    const path = pathOf(entry)
    if (path === undefined) return true
    if (netPaths.has(path)) return entry.kind === 'file' && entry.net === true
    if (unreadPaths.has(path) && (stepsOf.get(path) ?? 0) > 0) {
      return stepsOf.get(path) === 1 ? entry.kind === 'file' : entry.kind === 'unreported' && entry.observed === true
    }
    return true
  })
  const seen = new Set<string>()
  return withoutSteps.filter((entry) => {
    const path = pathOf(entry)
    if (path === undefined) return true
    if (seen.has(path)) return false
    seen.add(path)
    return true
  })
}

/** One turn that changed one file, for the viewer's history strip. */
export interface FileTurn {
  readonly missionId: string
  /** What was asked on that turn, so a version carries the reason for it. */
  readonly prompt: string
  readonly file: DiffFile
  readonly counts: DiffCounts
  readonly truncated: boolean
  readonly reported: DiffCounts | undefined
}

/**
 * Every turn in a conversation that changed one file, oldest first.
 *
 * Colin asked for artifact support and the honest answer was that the word
 * means at least four features (`docs/PLAN-2026-09-20-VIEWER-AND-ARTIFACTS.md`).
 * This is (c): *"show me what this looked like three turns ago"*, over data
 * the ledger already keeps -- every turn's diff is recorded, and the activity
 * fold draws those hunks today. Nothing new is stored.
 *
 * WHAT THIS DELIBERATELY DOES NOT DO: reconstruct the file as it stood.
 * Reverse-applying the recorded patches would produce a plausible document
 * from an incomplete record -- a patch can arrive `truncated`, and several
 * runtimes report an edit with no diff at all -- and a wrong file that looks
 * right is the worst failure available here. So a version shows **what that
 * turn changed**, and the panel says so in those words.
 *
 * It goes through `buildThread` rather than reading events itself because
 * that function already knows every runtime's quirks -- which row is the net
 * change to a file and which rows are steps inside it, chief among them. A
 * second reader of the same events would drift from the fold it sits beside.
 */
/**
 * What the replies after an edited message changed, file by file (0.502):
 * each file's recorded changes, oldest first, for the host to undo exactly
 * (shared/reverse-diff.ts) -- or the reason it cannot be, known here already:
 * a patch that arrived cut short, or an edit reported with no diff at all.
 */
export interface FilePutBack {
  readonly path: string
  readonly changes: readonly DiffFile[]
  /** Set when the record cannot put it back exactly; the host then leaves it alone. */
  readonly cannot?: string
}

export function laterFileChanges(
  turns: readonly { readonly events: readonly NormalizedRuntimeEvent[] }[],
  workspacePath: string | undefined
): readonly FilePutBack[] {
  const key = (value: string): string => relativePath(value, workspacePath).replace(/[\\/]+/g, '/')
  const plan = new Map<string, { path: string; changes: DiffFile[]; cannot?: string }>()
  const at = (path: string): { path: string; changes: DiffFile[]; cannot?: string } => {
    const id = key(path).toLowerCase()
    const held = plan.get(id)
    if (held !== undefined) return held
    const made = { path: key(path), changes: [] as DiffFile[] }
    plan.set(id, made)
    return made
  }
  for (const turn of turns) {
    for (const item of buildThread(turn.events, { running: false, ...(workspacePath === undefined ? {} : { workspacePath }) })) {
      if (item.type !== 'activity') continue
      for (const entry of netFileEntries(activityEntries(item.details, workspacePath), workspacePath)) {
        if (entry.kind === 'file') {
          const held = at(entry.file.path)
          held.changes.push(entry.file)
          if (entry.truncated) held.cannot ??= 'its recorded change was cut short'
        } else if (entry.kind === 'unreported' && !entry.failed && entry.neverRan === undefined) {
          at(entry.name).cannot ??= 'its change was recorded without the lines'
        }
      }
    }
  }
  return [...plan.values()].map((entry) => ({ path: entry.path, changes: entry.changes, ...(entry.cannot === undefined ? {} : { cannot: entry.cannot }) }))
}

/** Every file one turn's run changed, as its diff (0.395: what a review note is held against). */
export function editedFiles(events: readonly NormalizedRuntimeEvent[], workspacePath: string | undefined): readonly DiffFile[] {
  const out: DiffFile[] = []
  for (const item of buildThread(events, { running: false, ...(workspacePath === undefined ? {} : { workspacePath }) })) {
    if (item.type !== 'activity') continue
    for (const entry of netFileEntries(activityEntries(item.details, workspacePath), workspacePath)) {
      if (entry.kind === 'file') out.push(entry.file)
    }
  }
  return out
}

export function fileTurns(
  turns: readonly {
    readonly missionId: string
    readonly prompt: string
    readonly events: readonly NormalizedRuntimeEvent[]
  }[],
  path: string,
  workspacePath: string | undefined
): readonly FileTurn[] {
  const key = (value: string): string => relativePath(value, workspacePath).replace(/[\\/]+/g, '/').toLowerCase()
  const wanted = key(path)
  const out: FileTurn[] = []
  for (const turn of turns) {
    const thread = buildThread(turn.events, { running: false, ...(workspacePath === undefined ? {} : { workspacePath }) })
    for (const item of thread) {
      if (item.type !== 'activity') continue
      for (const entry of netFileEntries(activityEntries(item.details, workspacePath), workspacePath)) {
        if (entry.kind !== 'file' || key(entry.file.path) !== wanted) continue
        out.push({
          missionId: turn.missionId,
          prompt: turn.prompt,
          file: entry.file,
          counts: entry.counts,
          truncated: entry.truncated,
          reported: entry.reported
        })
      }
    }
  }
  return out
}

/**
 * The card's `+N -M`. Summed from the rendered rows, never from a runtime's
 * header: if a patch was cut short, the header would promise lines the diff
 * below cannot show.
 */
export function activityCounts(details: readonly ActivityDetail[], workspacePath?: string): DiffCounts {
  let added = 0
  let removed = 0
  for (const entry of activityEntries(details, workspacePath)) {
    if (entry.kind !== 'file') continue
    added += entry.counts.added
    removed += entry.counts.removed
  }
  return { added, removed }
}

/**
 * Which file opens by default: the first one, unless it is large enough that
 * opening it would bury everything after it.
 */
export function defaultOpenEntry(entries: readonly ActivityEntry[]): string | undefined {
  const first = entries.find((entry) => entry.kind === 'file')
  if (first === undefined || first.kind !== 'file') return undefined
  // A NEW web page opens anyway (0.446): it is shown running, in a frame of a
  // fixed height, so its length costs the thread nothing -- and a column of a
  // comparison that built the longer page showed nothing beside its rival's.
  const newPage = first.file.status === 'ADDED' && /\.html?$/i.test(first.file.path)
  return first.large && !newPage ? undefined : first.key
}

/**
 * The first NEW web page among a turn's files, to open on arrival (0.580).
 *
 * The turn's files card (one row per file, 0.494) opened nothing, so the
 * 0.446 rule above never reached it: a page a teammate made sat as a folded
 * row, in any conversation and after a comparison's Keep
 * (drive-build-and-compare, failing since at least 0.570). Only the page
 * opens there; a diff stays folded, as that card has always drawn them.
 */
export function newPageEntry(entries: readonly ActivityEntry[]): string | undefined {
  const page = entries.find((entry) => entry.kind === 'file' && entry.file.status === 'ADDED' && /\.html?$/i.test(entry.file.path))
  return page?.key
}

/** `14:44`, in the host's own timezone. */
/**
 * A usage window as the runtime worded it, with its ISO reset instants
 * turned into local clock times: "5-hour window 67% used · resets 10:10 PM".
 * The ledger keeps the instant; a person reads a time.
 */
export function usageWindowLabel(said: string): string {
  return said.replace(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z/g, (iso) => {
    const at = new Date(iso)
    if (Number.isNaN(at.getTime())) return iso
    const sameDay = at.toDateString() === new Date().toDateString()
    return sameDay ? clockTime(iso) : `${at.toLocaleDateString(undefined, { weekday: 'short' })} ${clockTime(iso)}`
  })
}

/** The fullest window's percentage in a usage-window reading, or undefined. A window that has reset since does not count. */
export function usagePercent(said: string, now: Date = new Date()): number | undefined {
  const found = usageWindowsOf(said, now).filter((window) => window.expired !== true).map((window) => window.percent)
  return found.length === 0 ? undefined : Math.max(...found)
}

/**
 * WHERE AND WHEN A READING WAS TAKEN (0.406). `run`: the usage the last run
 * in Locust reported -- use outside Locust since then is not in it. `account`:
 * read from the account itself (Codex), so it counts everything. Absent on a
 * reading from before 0.406.
 */
export function usageReadOf(said: string): { readonly kind: 'run' | 'account'; readonly at: string } | undefined {
  const run = / · from a run at (\d{4}-\d{2}-\d{2}T[\d:.]+Z)/.exec(said)
  if (run !== null) return { kind: 'run', at: run[1]! }
  const account = / · as of (\d{4}-\d{2}-\d{2}T[\d:.]+Z)/.exec(said)
  return account === null ? undefined : { kind: 'account', at: account[1]! }
}

/** "from Locust's last run, Sat 21:23" / "read from your account at 11:40". */
export function usageReadLine(said: string): string | undefined {
  const read = usageReadOf(said)
  if (read === undefined) return undefined
  const when = usageWindowLabel(read.at)
  return read.kind === 'run' ? `From Locust's last run on it, ${when}. Use outside Locust since then isn't counted.` : `Read from your account at ${when}.`
}

/**
 * The reading in the spec's words: "67% of the 5-hour window used, resets
 * 10:10 PM · 53% of the 7-day window, resets Sun 3:00 AM" (SURFACES-0.22 §3).
 */
export function usageWindowSentence(said: string, now: Date = new Date()): string {
  const windows = usageWindowsOf(said, now)
  if (windows.length === 0) return usageWindowLabel(said)
  let first = true
  const parts = windows.map((window) => {
    if (window.expired === true) return `the ${window.name} has reset since`
    const text = `${String(window.remaining ?? window.percent)}% of the ${window.name}${window.remaining === undefined ? (first ? ' used' : '') : ' left'}${window.resets === undefined ? '' : `, resets ${window.resets}`}`
    first = false
    return text
  })
  const read = usageReadOf(said)
  return parts.join(' · ') + (read === undefined ? '' : ` (${read.kind === 'run' ? "as of Locust's last run" : 'as of'} ${usageWindowLabel(read.at)})`)
}

/** One usage window, as a bar draws it: "5-hour window", 35, "22:10". */
export interface UsageWindowReading {
  readonly name: string
  readonly percent: number
  /** The runtime reported remaining quota; bars and warning tones still use consumption. */
  readonly remaining?: number
  readonly resets?: string
  /** Its reset time has passed since the reading: what it said is no longer true (0.406). */
  readonly expired?: true
}

/**
 * A reading's windows, in the runtime's order (fullest first), each with its
 * reset as a local time -- what an agent's hover card draws as bars (0.389).
 */
export function usageWindowsOf(said: string, now: Date = new Date()): readonly UsageWindowReading[] {
  // Read from the RAW reading, where a reset is still an instant: whether
  // it has passed is a comparison, and a clock time cannot be compared.
  return [...said.matchAll(/([^·]+?) window (\d{1,3})% (used|left)(?: · resets ([^·]+?))?(?= · |$)/g)].map((match) => {
    const reset = match[4]?.trim()
    const resetAt = reset === undefined ? Number.NaN : Date.parse(reset)
    const value = Math.min(100, Number(match[2]))
    return {
      name: `${match[1]!.trim()} window`,
      percent: match[3] === 'left' ? 100 - value : value,
      ...(match[3] === 'left' ? { remaining: value } : {}),
      ...(reset === undefined ? {} : { resets: usageWindowLabel(reset) }),
      ...(!Number.isNaN(resetAt) && resetAt <= now.getTime() ? { expired: true as const } : {})
    }
  })
}

export function clockTime(iso: string): string {
  const at = new Date(iso)
  if (Number.isNaN(at.getTime())) return '--:--'
  return `${String(at.getHours()).padStart(2, '0')}:${String(at.getMinutes()).padStart(2, '0')}`
}

/**
 * When a mission began, said the way a person needs it.
 *
 * The marker read `started 12:25 AM` with no date, which is unambiguous for
 * exactly as long as you keep the app open. Come back the next morning and a
 * mission from last night reads as one from five minutes ago. Today keeps the
 * bare time; anything older carries its date.
 */
export function startedLabel(iso: string, now: Date = new Date()): string | undefined {
  const at = new Date(iso)
  if (Number.isNaN(at.getTime())) return undefined
  const time = at.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
  const sameDay =
    at.getFullYear() === now.getFullYear()
    && at.getMonth() === now.getMonth()
    && at.getDate() === now.getDate()
  if (sameDay) return time
  const day = at.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    ...(at.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' })
  })
  return `${day}, ${time}`
}

/**
 * How far into a conversation a turn is, in units a person thinks in.
 *
 * It said `959 min in`, which Colin saw on a five-hour run. Two things were
 * wrong with that number and only one of them is arithmetic.
 *
 * The arithmetic: nobody reads 959 minutes. Past an hour or so a person
 * thinks in hours, and past a day, in days.
 *
 * The bigger one: it is WALL CLOCK from the first event of the whole
 * conversation, and a conversation chains across missions and across days.
 * So 959 minutes was mostly the hours he was not at the desk. The count is
 * true and it is not about work, which is what "in" sounds like it means.
 * The fix for that is not a better number -- there is no honest single
 * number for it -- it is to stop pretending a day-spanning total is a
 * duration you can feel, and to say the DATE once the turn is on a
 * different day from where the conversation started.
 */
export function elapsedInLabel(minutes: number): string {
  if (minutes < 90) return `${String(minutes)} min in`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) {
    const rest = minutes % 60
    return rest === 0 ? `${String(hours)}h in` : `${String(hours)}h ${String(rest)}m in`
  }
  const days = Math.floor(hours / 24)
  const rest = hours % 24
  return rest === 0 ? `${String(days)}d in` : `${String(days)}d ${String(rest)}h in`
}

/**
 * Whether two instants fall on the same calendar day.
 *
 * `startedLabel` learned this for the mission rule: a bare time is
 * unambiguous for exactly as long as you keep the app open, and a mission
 * from last night then reads as one from five minutes ago. The time marker
 * had the same hole, against a different reference -- not "today" but "the
 * day this conversation started".
 */
function sameCalendarDay(first: string, second: string): boolean {
  const left = new Date(first)
  const right = new Date(second)
  if (Number.isNaN(left.getTime()) || Number.isNaN(right.getTime())) return true
  return (
    left.getFullYear() === right.getFullYear()
    && left.getMonth() === right.getMonth()
    && left.getDate() === right.getDate()
  )
}

/** The day a turn happened, for a marker that has crossed one. */
function dayLabel(iso: string): string | undefined {
  const at = new Date(iso)
  if (Number.isNaN(at.getTime())) return undefined
  return at.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

/** Whole minutes between two instants, floored, never negative. */
export function minutesBetween(from: string, to: string): number {
  const start = new Date(from).getTime()
  const end = new Date(to).getTime()
  if (Number.isNaN(start) || Number.isNaN(end)) return 0
  return Math.max(0, Math.floor((end - start) / 60_000))
}

/**
 * A quiet gap worth marking. Below this a marker is noise; above it, the
 * reader deserves to know the mission sat still.
 */
export const QUIET_GAP_MINUTES = 2

export interface ThreadMarker {
  /** Index of the turn this marker sits ABOVE. */
  readonly beforeTurn: number
  readonly at: string
  readonly minutesIn: number
  /** Said only when the gap is the point, e.g. `waited 6 min`. */
  readonly note: string | undefined
  /** `45 min in`, `3h 20m in`, `2d 4h in` -- never `959 min in`. */
  readonly elapsed: string
  /**
   * The date, when this turn is on a different day from the one the
   * conversation started on. Absent within a single day, where the clock
   * time alone is unambiguous.
   */
  readonly day?: string
}

/**
 * Where a long conversation earns a time marker. A turn that follows the one
 * before it within a couple of minutes needs no marker -- it reads as the
 * same stretch of work. A gap longer than that is a fact about the mission
 * (you were away, or it was waiting on you), and stating it beats leaving
 * the reader to subtract two timestamps that are not on screen.
 */
export function threadMarkers(turns: readonly (readonly NormalizedRuntimeEvent[])[]): readonly ThreadMarker[] {
  const origin = turns.flat()[0]?.occurredAt
  if (origin === undefined) return []
  const markers: ThreadMarker[] = []
  for (let index = 1; index < turns.length; index += 1) {
    const previous = turns[index - 1]?.at(-1)?.occurredAt
    const next = turns[index]?.[0]?.occurredAt
    if (previous === undefined || next === undefined) continue
    const gap = minutesBetween(previous, next)
    if (gap < QUIET_GAP_MINUTES) continue
    const minutesIn = minutesBetween(origin, next)
    const crossedADay = !sameCalendarDay(origin, next)
    const day = crossedADay ? dayLabel(next) : undefined
    markers.push({
      beforeTurn: index,
      at: next,
      minutesIn,
      note: `waited ${String(gap)} min`,
      elapsed: elapsedInLabel(minutesIn),
      ...(day === undefined ? {} : { day })
    })
  }
  return markers
}

/**
 * Where a group's standing instructions began to apply, in a thread.
 *
 * The design agent's ruling of 2026-09-15, and the reason membership records
 * WHEN it happened: instructions brief from joining onward and never
 * retroactively, so the thread marks that boundary rather than claiming the
 * turns above it were briefed. A line at the top of a thread would claim
 * they were.
 */
export interface GroupBoundary {
  /** Index of the first turn briefed; equal to the turn count when none is yet. */
  readonly beforeTurn: number
  readonly groupName: string
  readonly instructions: string
  readonly joinedAt: string
}

/**
 * The boundary for a conversation, or nothing to draw.
 *
 * Nothing when the membership's moment is unknown (a file from before it was
 * recorded), and nothing when the group has no instructions -- a folder
 * briefs nothing, and a line saying it does would be the one thing this app
 * does not do. `turnStarts` are ISO instants, oldest first; a turn with no
 * known start is treated as before the join, which is the honest side to be
 * wrong on.
 */
export function groupBoundary(
  turnStarts: readonly (string | undefined)[],
  membership: { readonly at?: string } | undefined,
  group: { readonly name: string; readonly instructions: string } | undefined
): GroupBoundary | undefined {
  if (membership?.at === undefined || group === undefined) return undefined
  const instructions = group.instructions.trim()
  if (instructions.length === 0) return undefined
  const joined = Date.parse(membership.at)
  if (Number.isNaN(joined)) return undefined
  let beforeTurn = turnStarts.length
  for (let index = 0; index < turnStarts.length; index += 1) {
    const started = turnStarts[index]
    const at = started === undefined ? NaN : Date.parse(started)
    if (!Number.isNaN(at) && at >= joined) {
      beforeTurn = index
      break
    }
  }
  return { beforeTurn, groupName: group.name, instructions, joinedAt: membership.at }
}

/**
 * Where each ENDED membership's words began briefing, oldest first.
 *
 * Grok, passes 9, 10 and 11, the same sentence three times: after leaving, the
 * join line is gone, "turns 3 and 4 were briefed and the thread no longer
 * says so. Only the stop is marked." The current membership drew the join
 * line and a conversation that had left had none, so the thread told half the
 * story. An ended membership records when it joined, when that was recorded,
 * and this draws its join line from that -- the same rules as the current
 * one: no moment, no line; no words, no line.
 */
export function groupJoins(
  turnStarts: readonly (string | undefined)[],
  left: readonly { readonly name: string; readonly instructions: string; readonly at?: string }[]
): readonly GroupBoundary[] {
  const out: GroupBoundary[] = []
  for (const entry of left) {
    const boundary = groupBoundary(turnStarts, entry.at === undefined ? undefined : { at: entry.at }, entry)
    if (boundary !== undefined) out.push(boundary)
  }
  return out
}

/** The mirror of the join line: where a group's words STOPPED briefing. */
export interface GroupLeaving {
  /** Index of the first turn not briefed; equal to the turn count when that is the next one. */
  readonly beforeTurn: number
  readonly groupName: string
}

/**
 * Where each ended membership stops briefing, oldest first.
 *
 * Same rules as the join line, mirrored: a group with no words briefed
 * nothing, so its leaving says nothing; a turn with no known start is
 * treated as before the leave, which is the honest side to be wrong on.
 * Design agent, 2026-09-16: "Trading's instructions no longer apply from
 * here" -- the wording is theirs.
 */
export function groupLeavings(
  turnStarts: readonly (string | undefined)[],
  left: readonly { readonly name: string; readonly instructions: string; readonly until: string }[]
): readonly GroupLeaving[] {
  const out: GroupLeaving[] = []
  for (const entry of left) {
    if (entry.instructions.trim().length === 0) continue
    const until = Date.parse(entry.until)
    if (Number.isNaN(until)) continue
    let beforeTurn = turnStarts.length
    for (let index = 0; index < turnStarts.length; index += 1) {
      const started = turnStarts[index]
      const at = started === undefined ? NaN : Date.parse(started)
      if (!Number.isNaN(at) && at >= until) {
        beforeTurn = index
        break
      }
    }
    out.push({ beforeTurn, groupName: entry.name })
  }
  return out
}

export interface PlanStep {
  readonly text: string
  readonly state: 'done' | 'running' | 'pending'
}

export type ThreadItem =
  | { readonly key: string; readonly type: 'agent-message'; readonly text: string; readonly streaming: boolean }
  | {
      /**
       * THE STEPS TAKEN BETWEEN TWO THINGS SAID, as one line (0.491).
       *
       * Claude Code's turn, copied: what the teammate said, then the steps it
       * took before it next said anything, folded to one line that opens
       * onto them (`stepsLine`). Every step of the turn is in exactly one of
       * these, in the order it happened. While the turn runs, a step still
       * going is not in one yet -- the live line below names it -- and joins
       * its group the moment it ends.
       */
      readonly key: string
      readonly type: 'steps'
      readonly details: readonly ActivityDetail[]
      readonly finished: boolean
      /**
       * The group still growing at the end of a running turn (0.584): drawn
       * open, its rows arriving as the calls land, as Claude Code streams each
       * call. Colin, 2026-10-04, of a silent ten-minute Flash turn: "its
       * stacking on one line". A group opened this way is never closed by
       * the app (ActivityCard's rule); a press folds it.
       */
      readonly live?: boolean
      /**
       * The run moved on past this group (0.594): a later group is live, or the
       * model is talking after it. Drawn folded to its line unless the person
       * opened or closed it themselves. Never set once the turn is over, so
       * the last group of a finished run keeps whatever state it had.
       */
      readonly superseded?: boolean
    }
  | {
      readonly key: string
      readonly type: 'activity'
      /**
       * Notices about this turn's work, drawn at the FOOT of the fold.
       *
       * These are the diagnostics the thread's own gate drops -- the ones a
       * runtime raises before its first tool call, which used to be counted in
       * the trace line as `1 notice` and shown nowhere. The count is gone; the
       * sentence is here.
       */
      readonly notices?: readonly { readonly level: 'info' | 'warning' | 'error'; readonly message: string; readonly source: MissionRuntimeId }[]
      /** The plan this run stated, drawn as the fold's first rows. */
      readonly plan?: { readonly steps: readonly PlanStep[]; readonly doneCount: number }
      readonly summary: string
      /** The trace line, as segments, so its tones stay addressable (SURFACES-0.22 §1). */
      readonly trace: readonly TraceSegment[]
      /** True once the turn is over: an unsettled subagent then "did not report". */
      readonly finished: boolean
      readonly details: readonly ActivityDetail[]
      /**
       * Which runtime reported this work, so a row with no recorded change
       * can name it. Read off the events, never assumed.
       */
      readonly reportedBy: MissionRuntimeId | undefined
      /** The run this turn was, so its foot can offer to undo what it changed (0.674). */
      readonly runId?: string
    }
  | {
      readonly key: string
      readonly type: 'live-step'
      /**
       * WHICH REGISTER this line is, always, in one word.
       *
       * Colin, 2026-09-11: thinking, tool calls and connector calls all have
       * to be distinguishable from text the teammate actually wrote. The
       * label alone could not carry that -- it is whatever the runtime said,
       * so "Exploring the repository" and a sentence of the reply were the
       * same shape of words in the same place. This is derived from the
       * step's own kind and never from its wording.
       */
      readonly register: 'starting' | 'working' | 'thinking' | 'writing' | 'tool' | 'connector'
      /**
       * Which thinking orb this step is, or none.
       *
       * Mapped to the work, never cycled. The design agent's ruling of
       * 2026-09-20 settles why: mapped, the orb is not the sole carrier of
       * meaning -- the row already reads `shell · pnpm test billing` -- so it
       * does not need to be legible at 20px on its own. It needs to not
       * CONTRADICT. Rotation and mapping cannot coexist: a person who learns
       * `searching` means reading and then sees it during a shell command
       * distrusts every indicator in the app.
       *
       * So it is derived from the same classification the row's own text is
       * derived from, and the two cannot disagree.
       *
       * Four of the library's nine are true here, and the rest render never.
       * Inventing a Locust activity for `weaving` would be exactly the
       * failure this project keeps catching: a signal that looks like
       * information and is not. `undefined` is the honest answer for writing
       * a file and for waiting on the model, and the line keeps its dots.
       */
      readonly orb?: OrbState
      readonly label: string
      /**
       * THE STEP UNDER WAY, IN WORDS (0.569): "Reading notes.txt", "Running npm
       * test", the plan's current step. It leads the line, as Claude Code's
       * status line leads with it; absent, the register's own word does.
       * Never on a connector, which keeps its register word (it reaches off
       * this machine), or on thought.
       */
      readonly action?: string
      readonly detail: string | undefined
      /** When the step began, so the card can show elapsed time as it runs. */
      readonly startedAt: string
      /**
       * What kind of step: a tool or turn is the teammate DOING something and
       * their face works; reasoning is thought, shown as a still face with
       * staggered dots. Never a bar or a synthetic percentage.
       */
      readonly kind: 'turn' | 'reasoning' | 'item'
      /**
       * True when the teammate is waiting on the model rather than doing
       * something nameable -- the launch before the first event, and the gaps
       * between steps. Draws the dots, which is the only honest thing to show
       * for a wait: there is no step to name and no progress to claim.
       */
      readonly waiting?: boolean
      /**
       * Whether the runtime has reported ANYTHING yet, beyond the app's own
       * record of launching it. Absent on a line that names a real step,
       * because a named step is itself the answer. See `quiet.ts`.
       */
      readonly spoken?: boolean
      /**
       * When the runtime last said anything, on a line that names no command (0.758): five minutes past it the
       * line says the turn has gone quiet, with Stop beside it (`quiet.ts`). Absent while a command runs, whose
       * line already says what the turn waits on, and before the runtime has spoken (`spoken` covers that).
       */
      readonly lastHeardAt?: string
      /**
       * The plan the turn already has, set only when `detail` says "step N of M":
       * the mini plan that step opens on hover reads these, so nothing new is
       * fetched or stored.
       */
      readonly plan?: readonly PlanStep[]
    }
  | {
      readonly key: string
      readonly type: 'decision'
      /** What the runtime asked, and what it says each choice costs. */
      readonly request: DecisionRequest
    }
  | {
      readonly key: string
      readonly type: 'plan'
      /**
       * Whether this turn did nothing but plan.
       *
       * Was implicit and is now stated. The plan item only ever existed on a
       * turn with no activity, so "nothing was changed" was true by
       * construction -- and when the plan was lifted out of the fold so it
       * shows on every turn, that sentence started appearing over runs that
       * had just made thirty tool calls (Colin, 2026-09-14: "this is
       * definitely a bug", with a plan, the sentence, and `9 tool calls` in
       * one frame).
       */
      readonly touchedNothing?: boolean
      readonly steps: readonly PlanStep[]
      readonly doneCount: number
      /**
       * Whether the run this plan belongs to has ENDED.
       *
       * A plan's last state is whatever the runtime last sent. A run that
       * stops -- finished, failed, cancelled -- without a closing
       * `plan.updated` therefore leaves its unfinished steps exactly as they
       * were, and `pending` under a finished answer reads as "still to
       * come" about work that is never coming.
       *
       * The card is told, rather than the steps being rewritten. Marking
       * them done would be a lie about what happened, and dropping them
       * would hide that the run planned something it did not do -- which is
       * often the most useful thing on the card.
       */
      readonly finished?: boolean
      /**
       * The run FAILED or was STOPPED before it closed its list.
       *
       * Then an unchecked step is one it did not get to, and the card says
       * so in amber. After a run that completed, the same count is only the
       * runtime's checklist left unticked: Quill asked its five questions
       * and ended "Reply with your answers and I'll draft the page", and its
       * card read "2 not checked off" in amber over a pause that was right
       * to make (the Write & design first session, packaged 0.366).
       */
      readonly stopped?: boolean
    }
  | {
      /**
       * Files the teammate handed to the person, as a row of buttons under
       * the reply that handed them over.
       *
       * The mirror of the person's own attachments, which have drawn this way
       * since the composer took files: same row, same control, same question
       * answered -- "where is it". Pressing one reveals the file in the file
       * manager and never opens it; see `shared/handover.ts`.
       */
      readonly key: string
      readonly type: 'files'
      readonly files: readonly HandedFile[]
    }
  | {
      readonly key: string
      readonly type: 'limit'
      readonly kind: 'quota-exhausted' | 'temporary-rate-limit'
      readonly message: string
    }
  | {
      readonly key: string
      readonly type: 'diagnostic'
      readonly level: 'info' | 'warning' | 'error'
      readonly message: string
      /**
       * The model's provider is turning requests away and the runtime is
       * waiting to try again (`*.provider_busy.runtime_error`): the thread can
       * offer another model beside it (C9).
       */
      readonly busy?: boolean
      /** It says the runtime is trying again on its own: true only while the run is (0.550). */
      readonly retrying?: boolean
    }

/**
 * Read a provider plan into steps. The payload is redacted JSON of whatever
 * shape the provider sent, so every field is checked rather than assumed --
 * an unreadable plan yields no card instead of a malformed one.
 */
export function readPlan(value: unknown): readonly PlanStep[] {
  const list = Array.isArray(value)
    ? value
    : typeof value === 'object' && value !== null && Array.isArray((value as { plan?: unknown }).plan)
      ? ((value as { plan: unknown[] }).plan)
      : []
  const steps: PlanStep[] = []
  for (const entry of list) {
    if (typeof entry === 'string') {
      steps.push({ text: entry, state: 'pending' })
      continue
    }
    if (typeof entry !== 'object' || entry === null) continue
    const record = entry as Record<string, unknown>
    // `content` is OpenCode's word for the step (2026-09-13); the others are
    // Codex's and the app-server's. One plan, four spellings.
    const text = record.step ?? record.text ?? record.title ?? record.name ?? record.content
    if (typeof text !== 'string' || text.length === 0) continue
    const status = typeof record.status === 'string' ? record.status.toLowerCase() : ''
    const state: PlanStep['state'] =
      status.includes('complete') || status === 'done'
        ? 'done'
        : status.includes('progress') || status === 'running' || status === 'active'
          ? 'running'
          : 'pending'
    steps.push({ text, state })
  }
  return steps
}

/**
 * WHAT A CLI SAYS ABOUT ITS OWN SETUP, NOT ABOUT THE WORK (0.308).
 *
 * Codex opens every turn with remarks on how it is configured -- "Skill
 * descriptions were shortened to fit the skills context budget", "Codex is
 * ignoring 1 unrecognized configuration setting ... user (<the path to
 * your config.toml>)" -- and they reached the foot of every Codex turn's fold,
 * the person's own path with them: in a room of two, on the site's
 * screenshot (drive-room-two-agents, 2026-09-23). They are true and worth
 * reading once, and they are about the CLI, so they live on its row in
 * Settings > Runtimes (setupNotesOf) and nowhere in a conversation.
 */
const SETUP_NOTES: readonly RegExp[] = [
  /^Codex is ignoring \d+ unrecognized configuration settings?\b/i,
  /^Skill descriptions were shortened to fit\b/i
]

export function isSetupNote(message: string): boolean {
  return SETUP_NOTES.some((pattern) => pattern.test(message.trim()))
}

/**
 * A NOTE KEPT FOR THE RECORD, NEVER SAID (2026-10-05). A usage window is state
 * the host keeps, not a line in the thread. And how long a start took
 * ("Started in 6.1 s: looked for Codex 0.2 s, ...", main/start-timing.ts) is a
 * measurement for whoever makes starts faster, not news for the person who
 * sent the message: it sat under every answer. Colin: *"it doesnt really have
 * much purpose ... it should be removed"*. Both stay in the saved record, for
 * the drives and probes that read it; neither is drawn anywhere.
 */
export function keptForTheRecord(code: string): boolean {
  return /\.usage_window$/.test(code) || code === 'host.start-timing'
}

/** A run's setup notes, each once, in the order said -- for its runtime's row in Settings. */
export function setupNotesOf(events: readonly NormalizedRuntimeEvent[]): readonly string[] {
  const notes: string[] = []
  for (const event of events) {
    if (event.type !== 'adapter.diagnostic') continue
    const message = event.payload.message.trim()
    if (isSetupNote(message) && !notes.includes(message)) notes.push(message)
  }
  return notes
}

/**
 * Each runtime's setup notes from its newest run that said any: what its row
 * in Settings > Runtimes shows (0.308).
 */
export function latestSetupNotes(
  missions: readonly { readonly runtime: string; readonly createdAt: string; readonly events: readonly NormalizedRuntimeEvent[] }[]
): ReadonlyMap<string, readonly string[]> {
  const newest = new Map<string, { readonly at: string; readonly notes: readonly string[] }>()
  for (const mission of missions) {
    const notes = setupNotesOf(mission.events)
    if (notes.length === 0) continue
    const held = newest.get(mission.runtime)
    if (held === undefined || mission.createdAt > held.at) newest.set(mission.runtime, { at: mission.createdAt, notes })
  }
  return new Map([...newest].map(([runtime, entry]) => [runtime, entry.notes]))
}

/** A notice's words, however its spacing and case came out. */
export function noticeKey(message: string): string {
  return message.trim().replace(/\s+/g, ' ').toLowerCase()
}

/**
 * A usage-window warning: "You've used 56% of your 7-day window", "Your
 * 5-hour window is running low". Not a limit reached and not a retry -- a
 * reading, which is why it is said once a conversation.
 */
export function isUsageWarning(message: string): boolean {
  return /^You've used \d{1,3}% of your .+ window|^Your .+ window is running low/.test(message.trim())
}

/**
 * The key a turn's usage warning is remembered by: one for every reading.
 *
 * Colin, 2026-09-23, asked whether the warning belongs on every turn or once
 * a conversation: "Once per convo". A later turn's reading -- 56%, then 58%
 * -- is the same news again; a limit actually REACHED is not a warning and
 * still shows every time.
 */
export const USAGE_WARNING_KEY = 'usage-window-warning'

/** What a turn said that a later turn must not say again, keyed for `saidBefore`. */
export function foldNoticeKeys(items: readonly ThreadItem[]): readonly string[] {
  const activity = items.find((item) => item.type === 'activity')
  const notices = activity?.type === 'activity' ? activity.notices ?? [] : []
  const warned = items.some((item) => item.type === 'limit' && item.kind === 'temporary-rate-limit' && isUsageWarning(item.message))
  return [...notices.map((notice) => noticeKey(notice.message)), ...(warned ? [USAGE_WARNING_KEY] : [])]
}

function planKey(text: string): string {
  return text.trim().replace(/\s+/g, ' ').toLowerCase()
}

/** The plan a turn ended with, or none. What the next turn's card starts from. */
export function lastPlanOf(events: readonly NormalizedRuntimeEvent[]): readonly PlanStep[] {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]!
    if (event.type === 'plan.updated') return readPlan(event.payload.plan)
  }
  return []
}

/**
 * The steps that belong to this turn: all of them, less those the turn
 * before had already finished and this one did not touch again.
 */
export function thisTurnsSteps(
  plan: readonly PlanStep[],
  carried: readonly PlanStep[],
  worked: ReadonlySet<string>
): readonly PlanStep[] {
  const finishedBefore = new Set(carried.filter((step) => step.state === 'done').map((step) => planKey(step.text)))
  if (finishedBefore.size === 0) return plan
  return plan.filter((step) => !finishedBefore.has(planKey(step.text)) || worked.has(planKey(step.text)))
}


function pluralize(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`
}

/**
 * The collapsed activity line. Counts come from tool events only -- never from
 * a guess about what the model said it did.
 */
/**
 * What a person means by "the command that ran".
 *
 * Runtimes reach a shell through a host, and the host is not the work: a
 * Codex run on Windows records every command as
 * `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "npm test"`.
 * The row is one line wide, so the host name and its escaped backslashes
 * filled it and the actual command was cut off -- measured 2026-09-03 by
 * reading the card after a real run and being unable to tell what had been
 * run. The host is unwrapped for DISPLAY only; the ledger keeps the argv it
 * really used.
 */
export function shellCommandText(command: string): string {
  const host = /^\s*"?[^"]*(?:powershell|pwsh|cmd)\.exe"?\s+(?:-NoProfile\s+|-NonInteractive\s+|\/d\s+|\/s\s+)*(?:-Command|\/c)\s+([\s\S]+)$/i
  const match = host.exec(command)
  if (match === null) return command
  const inner = match[1]!.trim()
  // The host quotes the whole command; unwrap one matched layer, and undo the
  // doubling that quoting introduced.
  const doubleQuoted = inner.startsWith('"') && inner.endsWith('"')
  const unquoted =
    doubleQuoted || (inner.startsWith("'") && inner.endsWith("'"))
      ? inner.slice(1, -1)
      : inner
  /*
   * AND THE BACKSLASHES (0.610). Codex reports a Windows command with every
   * backslash escaped, its own powershell.exe path included -- the row read
   * `Get-Content 'C:\\Users\\<home>\\.codex\\skills\\...'` (Colin's screenshot,
   * 2026-10-04). When every backslash in the quoted payload is one half of an
   * escape pair, the payload was escaped as a whole and is shown as the shell
   * got it; one lone backslash means it was not, and it is shown as sent.
   */
  const unescaped = doubleQuoted ? withoutEscapes(unquoted) : undefined
  if (unescaped !== undefined) return unescaped.replace(/''/g, "'").trim()
  return unquoted.replace(/\\"/g, '"').replace(/""/g, '"').replace(/''/g, "'").trim()
}

/** A doubled backslash to one, and an escaped quote to a quote -- when every backslash in the text is half of such a pair; else undefined. */
function withoutEscapes(text: string): string | undefined {
  if (!text.includes('\\')) return undefined
  let out = ''
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index]!
    if (character !== '\\') { out += character; continue }
    const next = text[index + 1]
    if (next !== '\\' && next !== '"') return undefined
    out += next
    index += 1
  }
  return out
}

/**
 * A runtime's own tool names, said as words (0.610). Codex's image tool read
 * "Using imageGeneration" on the live line (Colin's screenshot, 2026-10-04):
 * a name written for a program, not a person.
 */
const TOOL_WORDS: readonly { readonly pattern: RegExp; readonly doing: string; readonly did: (count: number) => string }[] = [
  { pattern: /^(image_?generation|image_?gen|generate_?image)$/i, doing: 'Generating an image', did: (count) => (count === 1 ? 'made an image' : `made ${String(count)} images`) },
  // OpenCode's answer to a call of a tool it does not have, or with arguments it could not read: the 0.817 sweep's
  // question card read "Used invalid", a model having reached for the question tool Locust turns off.
  { pattern: /^invalid$/i, doing: 'Making a tool call', did: (count) => (count === 1 ? 'made a tool call that did not work' : `made ${String(count)} tool calls that did not work`) },
  ...(['status', 'diff', 'log', 'show', 'blame', 'branch'] as const).map((command) => ({
    pattern: new RegExp(`^locust_git_${command === 'branch' ? 'branches' : command}$`, 'i'),
    doing: `Running git ${command}`,
    did: (count: number) => (count === 1 ? `ran git ${command}` : `ran git ${command} ${String(count)} times`)
  }))
]

/**
 * Locust's git tool, for a run with no shell (0.818, resources/locust-git-reads.mjs), as OpenCode names its calls
 * (`locust_git_diff`): the git command it ran, which is how a shell's `git diff` reads too.
 */
export function gitToolCommand(tool: string): string | undefined {
  const command = /^locust_git_(status|diff|log|show|blame|branches)$/i.exec(tool.trim())?.[1]?.toLowerCase()
  return command === undefined ? undefined : `git ${command === 'branches' ? 'branch' : command}`
}

export function toolWords(tool: string): (typeof TOOL_WORDS)[number] | undefined {
  return TOOL_WORDS.find((entry) => entry.pattern.test(tool.trim()))
}

/**
 * The skill a Skill call ran, as the person would name it (0.679), or
 * undefined for any other tool; '' when the call named none. Locust carries
 * a folder's skills as the plugin `project` and the person's own as
 * `personal` (main/claude-skills.ts), so Claude Code calls them
 * `project:review` -- a prefix Locust added, and the row leaves it off.
 * Anyone else's plugin keeps its name: it says where the skill came from.
 */
export function skillCalled(tool: string | undefined, target: string): string | undefined {
  if (tool?.trim().toLowerCase() !== 'skill') return undefined
  return target.trim().replace(/^(project|personal):/, '')
}

/** "Used the review skill", or the live line's "Using the review skill". */
export function skillWords(skill: string, live: boolean): string {
  const verb = live ? 'Using' : 'Used'
  return skill.length === 0 ? `${verb} a skill` : `${verb} the ${skill.length > 40 ? `${skill.slice(0, 39)}…` : skill} skill`
}

/**
 * A path as the person working in this folder would write it.
 *
 * Runtimes report absolute paths, and an activity row is one line wide, so a
 * real edit read as a long temp path with the filename cut off -- the one
 * part that mattered. MEASURED 2026-09-03 while using the app. Anything
 * outside the workspace keeps its full path, because there the location IS
 * the information.
 */
// A comparison's column works in its own copy under .locust/compare/<id>/ (0.445):
// the same project, so its rows read as the folder's too -- and one file is one
// row, not its copy's path and its own name counted twice.
const WORKTREE_PATH_PREFIX = /^\.locust\/(?:worktrees|compare)\/[^/]+\//

/**
 * Cursor Agent reports the file it edited from inside its OWN copy of the
 * project, not from the folder you opened:
 *
 *   C:/Users/you/.cursor/projects/C-Users-you-code-streaks/src/streak.js
 *
 * That directory name is the workspace path with its separators and colon
 * beaten into hyphens, so it can be recognised for certain rather than
 * guessed at -- and only then, when it matches THIS workspace, is the rest
 * of it the same file you would open yourself. Measured on a real Cursor run
 * 2026-09-07: a two-line edit reported eleven file rows, every one of them
 * wearing a path like that, with the filename pushed off the end of a
 * one-line row.
 *
 * A mirror belonging to some OTHER project keeps its full path, because then
 * the location genuinely is the information.
 */
function cursorMirrorRelative(full: string, root: string): string | undefined {
  const mirror = /^(.*)\/\.cursor\/projects\/([^/]+)\/(.+)$/i.exec(full)
  if (mirror === null) return undefined
  const [, , project, rest] = mirror
  if (project === undefined || rest === undefined) return undefined
  // Colons AND both separators. The class here was one backslash short --
  // it named the forward slash and the escape, not the backslash -- so a
  // Windows root flattened to `C-Users` plus its remaining separators,
  // and never matched Cursor's own project name. The mirror
  // then went unrecognised and every Cursor file row wore the full path
  // this function exists to remove. Found 2026-09-10 by
  // `escapes-survived-the-shell`; `relativePath` below had it right all
  // along, four lines away.
  const flattened = root.replace(/[:\\/]+/g, '-').replace(/^-+|-+$/g, '')
  return project.toLowerCase() === flattened.toLowerCase() ? rest : undefined
}

export function relativePath(path: string, workspacePath: string | undefined): string {
  if (workspacePath === undefined || workspacePath.length === 0) return path
  const normalise = (value: string): string => value.replace(/[\\/]+/g, '/').replace(/\/$/, '')
  const root = normalise(workspacePath)
  const full = normalise(path)
  if (root.length === 0) return path
  // A comparison column's plain copy, under ~/.locust/compare (0.448): the
  // same project, outside the folder, so its files read as the folder's --
  // the long home path would be the one row in the column nobody can read.
  const copied = /(?:^|\/)\.locust\/compare\/cmp_[A-Za-z0-9]+-[abc]\/(.+)$/.exec(full)
  if (copied?.[1] !== undefined) return copied[1]
  // A routine's copy, under ~/.locust/routines (0.534): the same, so a run in one
  // reads `answer.txt`, not its copy's home path beside the same file counted twice.
  const routineCopy = /(?:^|\/)\.locust\/routines\/rt_[A-Za-z0-9]+\/(.+)$/.exec(full)
  if (routineCopy?.[1] !== undefined) return routineCopy[1]
  // A turn on the cloud computer (0.761): its copy there, /workspaces/.locust/t_<id>, reads as this folder.
  const inCloud = cloudRelative(full)
  if (inCloud !== undefined) return inCloud
  // A skill's own file, read from the copy Locust made for the run (0.679,
  // main/claude-skills.ts): it reads as the file it is a copy of -- the
  // folder's .claude/skills, or ~/.claude/skills for the person's own --
  // never the profile path of a copy that is gone once the run ends.
  const skillCopy = /(?:^|\/)claude-skills\/[0-9a-f-]{36}\/(project|personal)\/skills\/(.+)$/i.exec(full)
  if (skillCopy?.[2] !== undefined) return `${skillCopy[1]!.toLowerCase() === 'personal' ? '~/' : ''}.claude/skills/${skillCopy[2]}`
  // Cursor's own scratch (0.594): it writes a large connector result to a file
  // under its project folder and reads or greps it back. A UUID file name says
  // nothing to a person; what it is does.
  if (/(?:^|\/)\.cursor\/projects\/[^/]+\/agent-tools\/[^/]+$/i.test(full)) return "Cursor's saved tool result"
  // Windows paths are case-insensitive; comparing them case-sensitively is how
  // a correct prefix fails to match and the row keeps the unreadable path.
  const mirrored = cursorMirrorRelative(full, root)
  if (mirrored !== undefined) return mirrored
  // The workspace root ITSELF, which a directory-listing tool reports as the
  // thing it acted on. It fell through every branch below and kept its whole
  // absolute path -- so a stopped run's FINISHED list opened with
  // `C:\Users\...\locust-stopmid-ws-FtLao7` above three bare filenames
  // (measured 2026-09-07). The folder's own name is what a person calls it.
  if (full.toLowerCase() === root.toLowerCase()) return root.split('/').at(-1) ?? path
  if (!full.toLowerCase().startsWith(`${root.toLowerCase()}/`)) {
    // Already relative (a runtime that reports paths from its cwd): the tree
    // prefix is still noise. An absolute path elsewhere stays whole.
    const relative = /^([a-z]:)?\//i.test(full) ? full : full.replace(WORKTREE_PATH_PREFIX, '')
    return relative.length === 0 ? path : relative === full ? path : relative
  }
  const inside = full.slice(root.length + 1)
  if (inside.length === 0) return path
  // A teammate on its own branch works in <folder>/.locust/worktrees/<id>/,
  // and every path it touches carries that prefix. The tree is the same
  // project, so the row reads the way it would in the folder; the sidebar
  // already says which branch the teammate is on.
  const stripped = inside.replace(WORKTREE_PATH_PREFIX, '')
  return stripped.length === 0 ? inside : stripped
}

/**
 * The text a row shows for a path (0.596). Inside the folder it is the
 * relative path, as before. Outside it -- a teammate on Antigravity editing a
 * checkout elsewhere, in Colin's ledger 10/04 -- the whole absolute path made
 * the row unreadable; now its last two segments behind an ellipsis, and the
 * row carries the whole path on hover.
 */
export function displayPath(path: string, workspacePath: string | undefined): string {
  const relative = relativePath(path, workspacePath)
  if (relative !== path || !/^(?:[a-z]:)?[\\/]/i.test(path)) return relative
  const parts = path.split(/[\\/]+/).filter((part) => part.length > 0)
  return parts.length <= 3 ? path : `…/${parts.slice(-2).join('/')}`
}

export interface TraceSegment {
  readonly key: string
  readonly text: string
  readonly tone?: 'amber' | 'muted'
}

export type TraceOutcome = 'running' | 'completed' | 'failed' | 'cancelled'

/** How the turn ended, off its own events. */
export function traceOutcome(events: readonly NormalizedRuntimeEvent[], running: boolean): TraceOutcome {
  if (events.some((event) => event.type === 'run.failed')) return 'failed'
  if (events.some((event) => event.type === 'run.cancelled')) return 'cancelled'
  return running ? 'running' : 'completed'
}

/** "41s", "4m 20s", "1h 06m". */
/** What a finished command's row says about its time, or nothing (see the shell entry's `durationMs`). */
export function commandTook(entry: { readonly durationMs?: number; readonly settled: boolean; readonly background?: boolean; readonly refused?: string }): string | undefined {
  if (!entry.settled || entry.background === true || entry.refused !== undefined || entry.durationMs === undefined || entry.durationMs < 1000) return undefined
  return durationText(entry.durationMs)
}

export function durationText(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000))
  if (seconds < 60) return `${String(seconds)}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${String(minutes)}m ${String(seconds % 60).padStart(2, '0')}s`
  const hours = Math.floor(minutes / 60)
  return `${String(hours)}h ${String(minutes % 60).padStart(2, '0')}m`
}

/**
 * How long a run took: its first event to its last, in milliseconds, or
 * undefined with fewer than two.
 *
 * ONE measure for every surface. The Missions row timed from the mission's
 * creation to its last ledger write -- the launch before the runtime's first
 * word and the receipt after its last -- and read 43s for a turn whose fold
 * said 37s (Yurt's beta report, 2026-09-23, #14).
 */
export function runSpanMs(events: readonly NormalizedRuntimeEvent[]): number | undefined {
  const times = events.map((event) => Date.parse(event.occurredAt)).filter((t) => Number.isFinite(t))
  return times.length >= 2 ? Math.max(...times) - Math.min(...times) : undefined
}

/**
 * What a finished turn did, as one line a person reads coming back to the
 * laptop: duration, delegation, work, exceptions -- outermost fact to
 * innermost (SURFACES-0.22 §1). A segment that is zero is absent; the floor
 * is the duration alone, so the line never disappears.
 */
export function activityTrace(
  details: readonly ActivityDetail[],
  events: readonly NormalizedRuntimeEvent[],
  outcome: TraceOutcome,
  /**
   * The plan riding on this fold, when there is one.
   *
   * The reference draws the summary as `41s · 3 of 3 steps · asked 1
   * subagent · 6 tool calls · 3 files`, so the plan's progress is part of
   * what the line says the run did -- not just rows hidden inside it. Reading
   * the prose alone got the plan INTO the fold and left this out
   * (`Locust UI Review 2026-09-06.dc.html`, looked at properly 2026-09-06).
   */
  plan?: { readonly steps: readonly PlanStep[]; readonly doneCount: number },
  /**
   * Whether this run was allowed to change files.
   *
   * A run that COULD edit, finished, and edited nothing is a fact worth
   * stating. Cursor Agent said "Applying the two edits to notes.ts now",
   * reported completed, and left the file untouched (measured 2026-09-07);
   * the line above read `41s · thought 3s · asked 4 subagents · 14 tool
   * calls` and never mentioned files at all, because the count is only drawn
   * when it is greater than zero. Nothing on screen contradicted the model's
   * own account of itself.
   *
   * WHY THAT RUN CHANGED NOTHING, found later the same day and recorded
   * because the original note reads like an accusation: the workspace was
   * under `AppData`, and `~/.cursorignore` on that machine excludes it, so
   * Cursor was refused every read and write before it began. Cursor was not
   * misreporting -- it had been handed a folder it was configured to ignore.
   * The harness fix is `_tools/scratch-root.mjs`.
   *
   * The line still belongs here. A run that finished having changed nothing
   * is worth saying whatever the reason, and the reason is exactly what the
   * person cannot see: config, quota, a refusal, or a model that only said
   * it would. Silence reads as success in all four.
   *
   * Stated plainly rather than in amber: asking a question in an
   * edit-permitted session changes nothing either, and that is not a
   * problem. It is the person who asked for an edit who needs this, and for
   * them the plain fact is enough.
   */
  mayEdit?: boolean,
  /**
   * The folder this conversation is open in, so the count can tell one file
   * from two.
   *
   * A first outside tester, on OpenCode: "Alpha UI: 3s / 1 tool call / 2
   * files / +2 -0. Git: one line in one file." One append, counted twice --
   * because a runtime is free to report the same file relatively on one call
   * and absolutely on the next, and to the key below those were two files.
   *
   * `relativePath` already folds absolute-into-workspace, the worktree
   * prefix and Cursor's mirror, and the file ROWS have been drawn through it
   * since 0.38.8. The count never was. Same function on both now, so what
   * the fold lists and what the line counts cannot disagree.
   */
  workspacePath?: string,
  /**
   * The turn is longer than its display window (shared/event-window.ts), so
   * these events are not all of it. Its counts then say "at least": a
   * 600-call turn read "596 tool calls" (review 2026-10-15, C4), which is the
   * 0.627 bug the window was made to end, back for the longest turns.
   */
  trimmed?: boolean
): readonly TraceSegment[] {
  const segments: TraceSegment[] = []
  const duration = durationText(runSpanMs(events) ?? 0)
  const finished = outcome !== 'running'
  const entries = activityEntries(details, workspacePath)
  // How many FILES this run changed, which is not how many rows the card
  // drew. Two corrections from the 0.35.0 targeted QA:
  //
  // - The same path edited twice is one file. The card is right to draw both
  //   rows -- they are two things the runtime did -- but "2 files" over one
  //   file is a claim about the workspace, and it is false.
  // - A refused write changed nothing. It is already counted among the calls
  //   below and said again as "N refused", so counting it here was both a
  //   double count and the wrong noun.
  // Windows paths are case-insensitive and runtimes disagree about separators
  // and a leading `./`; the same file must key the same way whichever one
  // reported it.
  // Through `relativePath` first, because `README.md` and
  // `/home/you/proj/README.md` are one file and only the workspace root
  // knows that.
  const pathKey = (path: string): string =>
    relativePath(path, workspacePath).replace(/[\\/]+/g, '/').replace(/^\.\//, '').toLowerCase()
  const changedPaths = new Set<string>()
  for (const entry of entries) {
    // A parsed diff is a change that landed, by definition -- it is the change.
    if (entry.kind === 'file') changedPaths.add(pathKey(entry.file.path))
    // An edit the runtime named but did not diff counts only if it succeeded.
    else if (entry.kind === 'unreported' && entry.failed !== true) changedPaths.add(pathKey(entry.name))
  }
  const files = changedPaths.size
  const helpers = entries.filter((entry) => entry.kind === 'helper')
  const helpersFailed = helpers.filter((entry) => entry.failed).length
  const helpersSilent = finished ? helpers.filter((entry) => !entry.settled && !entry.failed).length : 0
  /*
   * Shell commands are counted SEPARATELY, not among the anonymous calls.
   *
   * The line already named the command when there was exactly one -- "ran seq
   * 1 300" instead of "1 tool call" -- on the reasoning that a count of a
   * thing you cannot see is not worth the room. The same is true of four of
   * them, and counting them twice ("6 tool calls · ran 4 commands") would say
   * one fact in two places, which this line does not do.
   */
  //
  // And thinking is not a call HERE either. 0.153.0 took reasoning out of
  // `activitySummary` and left this line -- the one a person reads at the
  // head of the fold -- counting it. Grok, pass 5 on 0.154.0: two reads and
  // a thought said `3 tool calls`; a run that only thought said `1 tool
  // call`. Two functions, one fact, and the visible one was the unfixed
  // one; the summary's green test is how a one-hour regression came back.
  const calls = details.filter((detail) => detail.kind !== 'helper' && detail.kind !== 'edit' && detail.kind !== 'shell' && detail.kind !== 'reasoning').length
    + details.filter((detail) => detail.kind === 'edit' && detail.failed === true).length
  const diagnostics = events.filter(
    (event): event is Extract<NormalizedRuntimeEvent, { type: 'adapter.diagnostic' }> => event.type === 'adapter.diagnostic' && !keptForTheRecord(event.payload.code)
  )
  // `no files changed` is a claim about the workspace, and it must not be made
  // in the one case where the host has ALREADY said it cannot tell what this
  // run changed. Two teammates in one folder suppress the git inference and
  // raise `host.shared_workspace` -- "what changed on disk cannot be told
  // apart ... it is not counted as this run's work" -- and the line then read
  // `no files changed - 1 notice` next to that sentence.
  //
  // MEASURED 2026-09-07, one teammate on OpenCode and one on Cursor at once:
  // Wren's card said `no files changed` while `wren-note.txt` sat on disk,
  // correct, written by that very run. Its runtime reported no edit of its
  // own and the overlap suppressed the inference, so the app knew nothing --
  // which is not the same as knowing nothing happened.
  //
  // The notice still appears and the fold still carries the whole sentence,
  // so silence is not what replaces it. What goes is the false half.
  const cannotAttribute = diagnostics.some(
    (event) => event.payload.code === 'host.shared_workspace'
  )
  // Counted by the calls that were refused, where the rows say so; by the
  // runtime's notices otherwise. One notice named two refused calls in the
  // replay of 2026-09-23 and the line said "1 refused".
  const refusedRows = details.filter((detail) => detail.status === 'refused').length
  // Declined by the person on a card: counted as its own word, never "refused".
  const declined = details.filter((detail) => detail.status === 'declined').length
  const refused = refusedRows > 0
    ? refusedRows
    : diagnostics.filter((event) => /denied|refus|permission/i.test(event.payload.code) || /not permitted|refused/i.test(event.payload.message)).length

  if (outcome === 'failed' || outcome === 'cancelled') {
    segments.push({ key: 'duration', text: `stopped at ${duration}` })
    if (outcome === 'cancelled' && files === 0) segments.push({ key: 'nothing', text: 'nothing was changed', tone: 'muted' })
  } else {
    segments.push({ key: 'duration', text: duration })
  }

  // Thought only when reasoning spans exist; most runtimes never report one.
  let thought = 0
  const openReasoning = new Map<string, number>()
  for (const event of events) {
    if (event.type === 'step.started' && event.payload.stepKind === 'reasoning') openReasoning.set(event.payload.itemId ?? 'reasoning', Date.parse(event.occurredAt))
    if ((event.type === 'step.completed' || event.type === 'step.failed') && event.payload.stepKind === 'reasoning') {
      const key = event.payload.itemId ?? 'reasoning'
      const began = openReasoning.get(key)
      if (began !== undefined) { thought += Math.max(0, Date.parse(event.occurredAt) - began); openReasoning.delete(key) }
    }
  }
  if (thought >= 1000) segments.push({ key: 'thought', text: `thought ${durationText(thought)}` })
  // Before the subagents and the tool calls: the plan is the shape of the
  // work, and the rest is how it was carried out.
  if (plan !== undefined && plan.steps.length > 0) {
    segments.push({
      key: 'steps',
      // `pluralize`, like every other count in this file. A one-step plan
      // read "0 of 1 steps" (static sweep, 2026-09-14, A3) -- and a one-step
      // plan is reachable, because this line only renders where there are
      // outcomes to count.
      text: `${String(plan.doneCount)} of ${pluralize(plan.steps.length, 'step')}`
    })
  }

  if (helpers.length > 0) {
    let text = `asked ${pluralize(helpers.length, 'subagent')}`
    let tone: TraceSegment['tone']
    if (helpersFailed > 0) { text += helpers.length === 1 ? ' · it failed' : ` · ${String(helpersFailed)} failed`; tone = 'amber' }
    else if (helpersSilent > 0) { text += helpers.length === 1 ? ' · it did not report' : ` · ${String(helpersSilent)} did not report`; tone = 'amber' }
    segments.push({ key: 'subagents', text, ...(tone === undefined ? {} : { tone }) })
  }
  /*
   * One command, and the line NAMES it: `ran seq 1 300`.
   *
   * "1 tool call" is a count of a thing the person cannot see without opening
   * the fold, which is the same complaint as the `1 notice` chip above. The
   * point of the closed state is that a finished turn reads as bubble, one
   * line, two replies -- and that only works if the one line says what
   * happened (design, 2026-09-08).
   *
   * Only when there is exactly one, and only for a shell command: naming one
   * of four would be arbitrary, and a `Read` of a path is already the file
   * rows' job. Bounded, because a command can be a paragraph and this line
   * shares its row with the duration, the file count and the cost.
   */
  // What RAN: a command the runtime refused is counted as refused, below.
  const shellCommands = details.filter((detail) => detail.kind === 'shell' && detail.status !== 'refused' && detail.status !== 'declined')
  // Through `shellCommandText`, the same unwrapping the command ROW uses. A
  // raw name on Windows begins with the whole
  // `"C:\Windows\...\powershell.exe" -NoProfile -Command` preamble, so a
  // summary built from it would spend its 60 characters on the host shell and
  // never reach the command.
  const onlyCommand =
    shellCommands.length === 1
      ? shellCommandText(shellCommands[0]?.name ?? '').split('\n')[0]?.trim()
      : undefined
  /*
   * What it RAN, and what came back.
   *
   * One command still gets its name -- that is the most useful thing the line
   * can say, and it was already true. Several get a count and their outcome,
   * because four command lines will not fit and "all exit 0" is the part a
   * reader is actually asking about.
   */
  if (shellCommands.length === 1 && onlyCommand !== undefined && onlyCommand.length > 0 && trimmed !== true) {
    const one = commandsRunText(commandsRun(details, finished))
    // Not yet RUN while it has no result and the turn is live -- running, or
    // waiting on the person's card. "ran" beside a card that says "Nothing has
    // happened yet" was the line claiming the past (drive-copilot-approve-each,
    // 0.377; every runtime's Approve each read the same).
    const verb = !finished && shellCommands[0]?.settled !== true ? 'running' : 'ran'
    const named = `${verb} ${onlyCommand.length > 60 ? `${onlyCommand.slice(0, 59)}…` : onlyCommand}`
    segments.push({
      key: 'commands',
      // The command's own exit code is on its ROW, so the line repeats it only
      // when it is the thing worth knowing: something other than success.
      text: one !== undefined && one.amber ? `${named} · ${one.text.split(' · ')[1] ?? ''}` : named,
      ...(one?.amber === true ? { tone: 'amber' as const } : {})
    })
  } else {
    const many = commandsRunText(
      commandsRun(details, finished),
      // A turn out of view is counted, not named: the names would be the window's.
      trimmed === true ? [] : shellCommands.map((detail) => detail.name)
    )
    if (many !== undefined) {
      const text = trimmed === true ? many.text.replace(/^ran /, 'ran at least ') : many.text
      segments.push({ key: 'commands', text, ...(many.amber ? { tone: 'amber' as const } : {}) })
    }
  }
  /*
   * The other calls, AFTER the commands and called other.
   *
   * Before them, "3 tool calls" read as the whole of it: Yurt counted twelve
   * command rows under "3 tool calls · ran mkdir, printf and 10 more" and
   * filed it as an undercount (beta report, #6). The three were the reads.
   */
  const atLeast = trimmed === true ? 'at least ' : ''
  if (calls > 0) segments.push({ key: 'calls', text: `${atLeast}${pluralize(calls, shellCommands.length > 0 ? 'other tool call' : 'tool call')}` })
  if (files > 0 && !(outcome === 'cancelled' && files === 0)) segments.push({ key: 'files', text: `${atLeast}${pluralize(files, 'file')}` })
  // Not said of a turn whose middle is out of view: an edit may sit there.
  else if (files === 0 && outcome === 'completed' && mayEdit === true && !cannotAttribute && trimmed !== true) {
    segments.push({ key: 'files', text: 'no files changed' })
  }
  if (refused > 0) segments.push({ key: 'refused', text: `${String(refused)} refused`, tone: 'amber' })
  if (declined > 0) segments.push({ key: 'declined', text: `${String(declined)} declined`, tone: 'amber' })
  /*
   * No `1 notice` chip. The notices themselves are drawn at the foot of the
   * fold instead -- see `activityNotices` and ActivityCard.
   *
   * The chip counted a thing the person could not read, and worse, it counted
   * a DIFFERENT set than the thread showed. This function counted every
   * diagnostic bar a usage window; the thread's own gate drops any diagnostic
   * arriving before the first tool call unless it is a run-level
   * `runtime_error` or `notification`, because Codex comments on its own setup
   * as every turn opens. So a `seq 1 300` run displayed `1 notice` with no
   * sentence anywhere on the screen -- confirmed in the shipped 0.52.0 capture,
   * whose whole recorded text is "1 tool call / no files changed / 1 notice /
   * seq 1 300 / exit 0".
   *
   * A count of an unnamed thing is not information. Naming it also makes the
   * two filters impossible to drift apart again, because there is only one.
   */
  return segments
}

/**
 * What one group of steps did, as the single line that stands for it (0.491).
 *
 * Colin, 2026-09-30, beside a frame of Claude Code's app: "ALL of our commands
 * and stuff that would appear batched on screen seem to all get rolled into
 * the bar". Claude Code draws a turn as what was said, then the steps taken
 * before the next thing said, as one line -- "Read 2 files, ran a command",
 * "Created ce3.py, ran 3 commands +150 -0", in red "Failed to add the effort
 * chip" -- and a press opens the steps. A group of one step reads as that
 * step. This is that line, from the rows the group holds.
 */
export interface StepsLine {
  /** The sentence first, then anything that went wrong, in amber. */
  readonly segments: readonly TraceSegment[]
  /**
   * The sentence in ever shorter forms (0.604), for a row too narrow for it:
   * first with names given way to counts, then with the last phrases given
   * way to "and N more". Each is shorter than the one before; the card picks
   * the first that fits. Empty when nothing shorter can be said.
   */
  readonly shorter: readonly string[]
  /** The sentence with every name whole (0.604): the line's hover title whenever what shows is not it. */
  readonly title: string
}

type Looked = 'read' | 'search' | 'list' | 'web' | 'fetch' | 'plan' | 'wait' | 'glob' | 'code'

/**
 * What a command only LOOKS at, from its first word: the Codex app's
 * "Explored" -- a `sed -n`, an `rg`, an `ls` is reading, not doing. Anything
 * else is a command. A command that edits never reaches this: it is an edit.
 */
export function commandLooksAt(command: string): 'read' | 'search' | 'list' | undefined {
  const head = commandHead(command).toLowerCase().replace(/\.exe$/, '')
  if (/^(cat|head|tail|less|more|type|get-content|gc|bat|nl|sed)$/.test(head)) return 'read'
  // `rg --files` lists files; it searches nothing (Sol, 0.492: "searched **").
  if (head === 'rg' && /\s--files\b/.test(command)) return 'list'
  if (/^(rg|grep|egrep|fgrep|findstr|select-string|sls|ag|ack)$/.test(head)) return 'search'
  if (/^(ls|dir|tree|find|fd|get-childitem|gci|ll|la)$/.test(head)) return 'list'
  return undefined
}

/** What a tool other than a command did, by the runtime's own name for it. */
function toolLooksAt(tool: string | undefined): Looked | undefined {
  const name = (tool ?? '').toLowerCase()
  if (/web_?search|search_?web/.test(name)) return 'web'
  if (/fetch|read_?url|url_?content|browser/.test(name)) return 'fetch'
  if (/todo|update_?plan|task_?boundary/.test(name)) return 'plan'
  // Antigravity CLI's check on a command it sent to the background (measured 0.542: Action "status").
  if (name === 'manage_task') return 'wait'
  // Its timer that checks back on a background command (0.570: "used schedule" on Colin's run).
  if (name === 'schedule') return 'wait'
  // Code run in a runtime's own interpreter tool: Codex's `node_repl` (0.493).
  if (/repl/.test(name)) return 'code'
  // Cursor's wait on a command it sent away.
  if (/^(await|wait|await_?shell)$/.test(name)) return 'wait'
  if (/^(ls|list|list_?dir|list_?directory|listdir|dir)$/.test(name)) return 'list'
  // Finding files by name is listing them, not searching their text (Sol, 0.491: "Searched for **/*").
  if (/^glob$|glob_?tool|find_?by_?name|file_?search/.test(name)) return 'glob'
  if (/grep|glob|search|find|codebase|semsearch|ripgrep/.test(name)) return 'search'
  if (/^(read|read_?file|readfile|view|view_?file|open_?file|notebook_?read|cat|readtoolcall)$/.test(name)) return 'read'
  return undefined
}

/** The files a reading command names, by their own names: `cat README.md LOCUST.md` reads both. */
function commandTargets(command: string): readonly string[] {
  // Every READING command of a compound line (Sol, 0.492: `Get-Content a; Get-Content b; ...`
  // read seven files and the line said five); a pipe's later half only filters what came before.
  const segments = shellCommandText(command).split('\n')[0]?.split(/\|\||&&|;/).map((part) => part.split('|')[0] ?? '') ?? []
  const names = segments
    .filter((segment) => commandLooksAt(segment.trim()) === 'read')
    .flatMap((segment) => segment.trim().split(/\s+/).slice(1))
    .map((word) => word.replace(/^["']+|["',;]+$/g, ''))
    // A path ending in a separator is a folder, not a file read.
    .filter((word) => word.length > 1 && !word.startsWith('-') && /[./\\]/.test(word) && !/[\\/]$/.test(word) && !/^\d+,\d+p?$/.test(word))
    .map((word) => word.split(/[\\/]/).filter((part) => part.length > 0).at(-1) ?? '')
    .filter((name) => name.length > 0)
  return [...new Set(names)]
}

/** The last path-looking word of a command, as its file name: `sed -n 1,80p src/a.ts` is `a.ts`. */
function commandTarget(command: string): string | undefined {
  // The first command only: `Get-Content a.log -Tail 80; Write-Output "--"` reads a.log.
  const first = shellCommandText(command).split('\n')[0]?.split(/\|\||&&|;|\|/)[0] ?? ''
  const words = first.trim().split(/\s+/).slice(1)
  const paths = words
    .map((word) => word.replace(/^["']+|["',;]+$/g, ''))
    .filter((word) => word.length > 1 && !word.startsWith('-') && /[./\\]/.test(word) && !/^\d+,\d+p?$/.test(word))
  const last = paths.at(-1)?.split(/[\\/]/).filter((part) => part.length > 0).at(-1)
  return last === undefined || last.length === 0 ? undefined : last
}

/** What a search command looked for: its first word that is not a flag, `rg port src` -> `port`. */
function commandPattern(command: string): string | undefined {
  const first = shellCommandText(command).split('\n')[0]?.split(/\|\||&&|;|\|/)[0] ?? ''
  // A bare glob (`-g '**'`) names nothing a person searched for: the first word that says something.
  const word = first.trim().split(/\s+/).slice(1).find((part) => !part.startsWith('-') && /[A-Za-z0-9]/.test(part))
  const bare = word?.replace(/^["']+|["']+$/g, '')
  return bare === undefined || bare.length === 0 ? undefined : bare
}

/**
 * A thought's headline, when its summary leads with one: Codex writes
 * "**Inspecting the config**" and then the paragraph (0.492).
 */
export function thoughtHeadline(text: string): string | undefined {
  const headline = /^\s*\*\*([^*\n]{2,80})\*\*/.exec(text)?.[1]?.trim()
  return headline === undefined || headline.length === 0 ? undefined : headline
}

/** "Run the tests" -> "run the tests", keeping a word that is capitalised on its own (`README`). */
function lowerFirst(text: string): string {
  return /^[A-Z][a-z]/.test(text) ? `${text.charAt(0).toLowerCase()}${text.slice(1)}` : text
}

/**
 * THE STEP UNDER WAY, IN WORDS, AS CLAUDE CODE'S STATUS LINE SAYS IT (0.569).
 *
 * Colin, 2026-10-03, with four frames of Claude Code's line -- "Reading pet
 * removal in main and who calls it", "Running a command", "Editing
 * pet-library.ts", each with the turn's clock -- "anything we can use from
 * this taskbar setup". Filmed (`_tools/look-live-line.mjs`), Locust's line
 * said "Working... step 2 of 3 · 15s" for a whole run that read a file, ran a
 * command and wrote another: true, and nothing a person could not have
 * guessed. This is `stepsLine` in the present tense for the one step that is
 * open: the model's own description where it gave one, else what the call
 * looks at, else what it runs. Undefined for thought, which has its own line.
 */
export function liveActionLine(detail: ActivityDetail, workspacePath?: string): string | undefined {
  const clip = (text: string, max = 56): string => (text.length > max ? `${text.slice(0, max - 1)}…` : text)
  const capital = (text: string): string => `${text.charAt(0).toUpperCase()}${text.slice(1)}`
  const fileName = (path: string): string | undefined => {
    const bare = relativePath(path, workspacePath).trim().replace(/^["']+|["']+$/g, '')
    if (bare.length === 0 || /\s/.test(bare) && !/[\\/]/.test(bare)) return undefined
    if (!/[./\\]/.test(bare)) return undefined
    return bare.split(/[\\/]/).filter((part) => part.length > 0).at(-1)
  }
  if (detail.kind === 'reasoning') return undefined
  const title = detail.title?.replace(/\s+/g, ' ').trim()
  if (title !== undefined && title.length > 0) return clip(capital(title))
  if (detail.kind === 'shell') {
    const looked = commandLooksAt(detail.name)
    if (looked === 'read') {
      const target = commandTarget(detail.name)
      return target === undefined ? 'Reading a file' : `Reading ${clip(target, 40)}`
    }
    if (looked === 'search') {
      const pattern = commandPattern(detail.name)
      return pattern === undefined ? 'Searching the files' : `Searching for ${clip(pattern, 32)}`
    }
    if (looked === 'list') return 'Listing files'
    const first = shellCommandText(detail.name).split('\n')[0]?.trim() ?? ''
    // A runtime that names its command by the model's phrase ("Running the tests").
    if (/^[A-Z][a-z]+ing\b/.test(first)) return clip(first)
    if ((first.match(/[A-Za-z]/g)?.length ?? 0) < 3) return 'Running a script'
    if (first.length > 40 && /\s-(?:e|c|-eval|-command|Command)\s+["'`]/.test(first)) return `Running a ${commandHead(first).replace(/\.exe$/i, '')} script`
    return `Running ${clip(first, 40)}`
  }
  if (detail.kind === 'helper') return `Asking a helper${detail.name.trim().length === 0 ? '' : `: ${clip(detail.name.trim(), 40)}`}`
  if (detail.kind === 'edit') {
    const entry = activityEntries([detail], workspacePath)[0]
    const path = entry === undefined ? undefined : entry.kind === 'file' ? entry.file.path : entry.kind === 'unreported' ? entry.name : undefined
    const name = path === undefined ? undefined : fileName(path) ?? path
    const verb = entry?.kind === 'file' && entry.file.status === 'ADDED' ? 'Creating' : entry?.kind === 'file' && entry.file.status === 'DELETED' ? 'Deleting' : 'Editing'
    return name === undefined ? 'Editing a file' : `${verb} ${clip(name, 40)}`
  }
  const skill = skillCalled(detail.tool ?? detail.name, detail.tool === undefined || detail.tool === detail.name ? '' : detail.name)
  if (skill !== undefined) return skillWords(skill, true)
  const tool = (detail.tool ?? detail.name).toLowerCase()
  // A runtime's own writing tool (Antigravity's write_to_file names the file it writes).
  if (/^(write|write_?to_?file|create_?file|edit|edit_?file|str_?replace\w*|apply_?patch|replace\w*)$/.test(tool)) {
    const name = fileName(detail.name)
    return name === undefined ? 'Writing a file' : `Writing ${clip(name, 40)}`
  }
  switch (toolLooksAt(detail.tool ?? detail.name)) {
    case 'read': {
      const name = fileName(detail.name)
      return name === undefined ? 'Reading a file' : `Reading ${clip(name, 40)}`
    }
    case 'list': {
      const name = fileName(detail.name)
      return name === undefined ? 'Listing a folder' : `Listing ${clip(name, 40)}`
    }
    case 'glob': return 'Finding files'
    case 'search': return /\s{2}|^[A-Z][a-z]+ing\b/.test(detail.name) || detail.name.trim().length === 0 ? 'Searching the files' : `Searching for ${clip(detail.name.trim(), 32)}`
    case 'web': return 'Searching the web'
    case 'fetch': return 'Fetching a page'
    case 'plan': return 'Updating the plan'
    case 'wait': return 'Waiting for a command'
    case 'code': return 'Running code'
    default: return toolWords(detail.tool ?? detail.name)?.doing ?? `Using ${clip(detail.tool ?? detail.name, 40)}`
  }
}

/**
 * A FILE NAME LONGER THAN THE ROW CAN HOLD is cut in the middle, its extension
 * kept (0.604): "a-full-rule-stor…very-rule.test.ts" still says what it is.
 * The row's own end-of-line ellipsis on the whole sentence -- "created
 * a-full-rule-store-keeps-every-rule.test.ts, edited ap…", Colin's screenshot
 * of 2026-10-04 -- said nothing about the second file at all. 34 characters
 * leaves a long name room beside its verb at the row's narrowest.
 */
export function shortName(name: string, max = 34): string {
  if (name.length <= max) return name
  const extension = /(?:\.[A-Za-z0-9]{1,8}){1,2}$/.exec(name)?.[0] ?? ''
  const stem = name.slice(0, name.length - extension.length)
  const room = Math.max(6, max - extension.length - 1)
  const head = Math.ceil(room * 0.64)
  const tail = room - head
  return `${stem.slice(0, head)}…${tail > 0 ? stem.slice(stem.length - tail) : ''}${extension}`
}

/**
 * The shorter forms of a steps sentence (0.604), each shorter than the last:
 * the phrases without their names, then with the last phrases given way to
 * "and N more". The card measures its row and shows the first that fits; the
 * full sentence stays as the line's title. A lone failure is said whole or
 * not at all.
 */
function shorterForms(words: readonly string[], briefs: readonly string[], sentence: string, loneFailed: boolean): string[] {
  if (loneFailed || words.length === 0) return []
  const cap = (text: string): string => `${text.charAt(0).toUpperCase()}${text.slice(1)}`
  const shorter: string[] = []
  const brief = briefs.join(', ')
  if (brief.length < sentence.length) shorter.push(cap(brief))
  for (let keep = briefs.length - 1; keep >= 1; keep -= 1) {
    const text = `${briefs.slice(0, keep).join(', ')} and ${String(briefs.length - keep)} more`
    if (text.length < (shorter.at(-1) ?? sentence).length) shorter.push(cap(text))
  }
  return shorter
}

export function stepsLine(details: readonly ActivityDetail[], finished: boolean, workspacePath?: string, options: { readonly open?: boolean } = {}): StepsLine {
  const phrases = new Map<string, { count: number; names: string[] }>()
  const note = (kind: string, name: string | undefined): void => {
    const held = phrases.get(kind) ?? { count: 0, names: [] }
    held.count += 1
    if (name !== undefined && name.length > 0 && !held.names.includes(name)) held.names.push(name)
    phrases.set(kind, held)
  }
  let failed = 0
  // Of those, commands Codex's Windows sandbox never started (0.736, shared/codex-sandbox.ts).
  let sandboxFailed = 0
  let refused = 0
  let declined = 0
  let silent = 0
  let working = 0
  let thoughtMs = 0
  let thoughts = 0
  let headline: string | undefined
  let thoughtText: string | undefined
  /*
   * A file's own name, and only when what the runtime gave IS a path:
   * Antigravity names its reads by the model's phrase ("Listing orb user
   * session d..."), which is no file name.
   */
  const fileName = (path: string): string | undefined => {
    const bare = relativePath(path, workspacePath).trim().replace(/^["']+|["']+$/g, '')
    if (bare.length === 0 || /\s/.test(bare) && !/[\\/]/.test(bare)) return undefined
    if (!/[./\\]/.test(bare)) return undefined
    return bare.split(/[\\/]/).filter((part) => part.length > 0).at(-1)
  }
  for (const detail of details) {
    if (detail.status === 'refused') { refused += 1; continue }
    if (detail.status === 'declined') { declined += 1; continue }
    if (finished && !detail.settled && detail.failed !== true) silent += 1
    // Work sent to the background goes on after its call returns (0.492).
    if (detail.background === true && detail.settled && detail.failed !== true) {
      if (detail.backgroundEnded === 'failed') failed += 1
      else if (detail.backgroundEnded === undefined) {
        if (finished) silent += 1
        else working += 1
      }
    }
    if (detail.kind === 'reasoning') {
      thoughts += 1
      thoughtMs += detail.durationMs ?? 0
      headline ??= thoughtHeadline(detail.output ?? '')
      if ((detail.output ?? '').trim().length > 0) thoughtText ??= detail.output
      continue
    }
    if (detail.kind === 'shell') {
      if (detail.failed === true || (detail.exitCode !== undefined && detail.exitCode !== 0)) failed += 1
      if (detail.failed === true && codexSandboxDidNotStart(detail.output)) sandboxFailed += 1
      const looked = commandLooksAt(detail.name)
      if (looked === 'read' && commandTargets(detail.name).length > 1) for (const name of commandTargets(detail.name)) note('read', name)
      else if (looked === 'search' && /[\\/]/.test(commandPattern(detail.name) ?? '')) note('searchedIn', commandPattern(detail.name)!.split(/[\\/]/).filter((part) => part.length > 0).at(-1))
      else if (looked !== undefined) note(looked, looked === 'read' ? commandTarget(detail.name) : looked === 'search' ? commandPattern(detail.name) : undefined)
      else note('command', detail.title ?? commandHead(detail.name))
      continue
    }
    if (detail.kind === 'helper') {
      if (detail.failed === true) failed += 1
      note('helper', detail.name)
      continue
    }
    if (detail.kind === 'edit') {
      if (detail.failed === true) { failed += 1; continue }
      const entries = activityEntries([detail], workspacePath)
      for (const entry of entries) {
        if (entry.kind === 'file') note(entry.file.status === 'ADDED' ? 'created' : entry.file.status === 'DELETED' ? 'deleted' : 'edited', fileName(entry.file.path) ?? entry.file.path)
        else if (entry.kind === 'unreported') note('edited', fileName(entry.name) ?? entry.name)
      }
      continue
    }
    if (detail.failed === true) failed += 1
    // A skill reads as the skill it was (0.679), not "used Skill".
    const skill = skillCalled(detail.tool ?? detail.name, detail.tool === undefined || detail.tool === detail.name ? '' : detail.name)
    if (skill !== undefined) { note('skill', skill.length === 0 ? undefined : skill); continue }
    const git = gitToolCommand(detail.tool ?? detail.name)
    if (git !== undefined) { note('git', git); continue }
    const looked = toolLooksAt(detail.tool)
    // A search tool may name its pattern or the folder it searched (Cursor's grep names the folder).
    const searchedIn = looked === 'search' && /[\\/]/.test(detail.name) ? fileName(detail.name) : undefined
    if (looked !== undefined) note(searchedIn !== undefined ? 'searchedIn' : looked, looked === 'read' || looked === 'list' ? fileName(detail.name) : looked === 'glob' ? detail.name : looked === 'code' ? detail.tool ?? detail.name : searchedIn ?? (looked === 'search' && !/\s{2}|^[A-Z][a-z]+ing\b/.test(detail.name) ? detail.name : undefined))
    else note('tool', detail.tool ?? detail.name)
  }
  const one = (held: { count: number; names: string[] }): string | undefined => (held.count === 1 ? held.names[0] : undefined)
  const clip = (text: string, max = 48): string => (text.length > max ? `${text.slice(0, max - 1)}…` : text)
  const times = (count: number, one: string, many: string): string => `${String(count)} ${count === 1 ? one : many}`
  const words: string[] = []
  // The same phrases without their names (0.604): what the line falls back to when its row is too narrow for them.
  const briefs: string[] = []
  // And with every name whole, for the hover: a cut name says what it is, the title says all of it.
  const longs: string[] = []
  const word = (full: string, brief = full, long = full): void => { words.push(full); briefs.push(brief); longs.push(long) }
  const files = (held: { count: number; names: string[] }): string => (held.names.length > 1 ? `read ${pluralize(held.names.length, 'file')}` : held.count === 1 ? 'read a file' : `read ${pluralize(held.count, 'file')}`)
  const searches = (held: { count: number }): string => (held.count === 1 ? 'ran a search' : `ran ${times(held.count, 'search', 'searches')}`)
  for (const [kind, held] of phrases) {
    const named = one(held)
    switch (kind) {
      case 'read': word(named !== undefined ? `read ${shortName(named)}` : files(held), files(held), named !== undefined ? `read ${named}` : files(held)); break
      case 'search': word(held.count === 1 ? (named !== undefined ? `searched for ${clip(named, 32)}` : 'searched the files') : `ran ${times(held.count, 'search', 'searches')}`, searches(held)); break
      case 'code': {
        const language = held.names.every((name) => /node|js/i.test(name)) ? 'JavaScript' : /python|py/i.test(held.names.join(' ')) ? 'Python' : 'code'
        word(held.count === 1 ? `ran ${language}` : `ran ${language} ${String(held.count)} times`)
        break
      }
      case 'glob': word(held.count > 1 ? `listed files ${String(held.count)} times` : named === undefined || /^(\*\*[\\/])?\*(\.\*)?$/.test(named) ? 'listed every file' : `listed files matching ${clip(named, 28)}`, held.count > 1 ? `listed files ${String(held.count)} times` : 'listed files'); break
      case 'searchedIn': word(held.count === 1 && named !== undefined ? `searched ${shortName(named)}` : `ran ${times(held.count, 'search', 'searches')}`, searches(held), held.count === 1 && named !== undefined ? `searched ${named}` : `ran ${times(held.count, 'search', 'searches')}`); break
      case 'wait': word(held.count === 1 ? 'waited for a command' : `waited ${String(held.count)} times`); break
      case 'list': word(held.count === 1 ? (named !== undefined ? `listed ${shortName(named)}` : 'listed a folder') : `listed ${pluralize(held.count, 'folder')}`, held.count === 1 ? 'listed a folder' : `listed ${pluralize(held.count, 'folder')}`, held.count === 1 ? (named !== undefined ? `listed ${named}` : 'listed a folder') : `listed ${pluralize(held.count, 'folder')}`); break
      case 'web': word(held.count === 1 ? 'searched the web' : `searched the web ${String(held.count)} times`); break
      case 'fetch': word(held.count === 1 ? 'fetched a page' : `fetched ${pluralize(held.count, 'page')}`); break
      case 'plan': word('updated the plan'); break
      case 'skill': {
        const only = held.names.length === 1 ? held.names[0]! : undefined
        const again = held.count === 1 ? '' : ` ${String(held.count)} times`
        const many = `used ${pluralize(Math.max(held.names.length, 1), 'skill')}`
        word(
          only !== undefined ? `used the ${clip(only, 32)} skill${again}` : many,
          held.count === 1 ? 'used a skill' : only !== undefined ? `used a skill${again}` : many,
          only !== undefined ? `used the ${only} skill${again}` : many
        )
        break
      }
      case 'helper': word(held.count === 1 ? `asked a helper${named === undefined ? '' : `: ${clip(named)}`}` : `asked ${pluralize(held.count, 'helper')}`, held.count === 1 ? 'asked a helper' : `asked ${pluralize(held.count, 'helper')}`); break
      case 'git': {
        // "ran git diff", "ran git diff 2 times", "ran git status, diff and log" -- or, past three, how many.
        const commands = held.names.map((name) => name.replace(/^git /, ''))
        const brief = held.count === 1 ? 'ran git' : `ran git ${String(held.count)} times`
        word(
          commands.length === 1 ? (held.count === 1 ? `ran git ${commands[0]!}` : `ran git ${commands[0]!} ${String(held.count)} times`)
          : commands.length <= 3 ? `ran git ${commands.slice(0, -1).join(', ')} and ${commands.at(-1)!}`
          : brief,
          brief
        )
        break
      }
      case 'created':
      case 'edited':
      case 'deleted': word(held.names.length === 1 ? `${kind} ${shortName(held.names[0]!)}` : `${kind} ${pluralize(Math.max(held.names.length, 1), 'file')}`, held.names.length === 1 ? `${kind} a file` : `${kind} ${pluralize(Math.max(held.names.length, 1), 'file')}`, held.names.length === 1 ? `${kind} ${held.names[0]!}` : `${kind} ${pluralize(Math.max(held.names.length, 1), 'file')}`); break
      case 'tool': {
        const known = held.names.length === 1 ? toolWords(held.names[0]!) : undefined
        if (known !== undefined) { word(known.did(held.count)); break }
        word(held.names.length === 1 ? (held.count === 1 ? `used ${held.names[0]!}` : `used ${held.names[0]!} ${String(held.count)} times`) : pluralize(held.count, 'other tool call'), held.names.length === 1 ? (held.count === 1 ? 'used a tool' : `used a tool ${String(held.count)} times`) : pluralize(held.count, 'other tool call'))
        break
      }
      case 'command': {
        const titled = details.filter((detail) => detail.kind === 'shell' && commandLooksAt(detail.name) === undefined)
        const only = titled.length === 1 ? titled[0] : undefined
        const first = shellCommandText(only?.name ?? '').split('\n')[0]?.trim() ?? ''
        word(
          held.count > 1 ? `ran ${pluralize(held.count, 'command')}`
          : only?.title !== undefined ? lowerFirst(only.title)
          // A runtime that names its command by the model's phrase ("Running the tests").
          : /^[A-Z][a-z]+ing\b/.test(first) ? lowerFirst(clip(first))
          // A script's opening line (PowerShell's `@'`) names nothing.
          : (first.match(/[A-Za-z]/g)?.length ?? 0) < 3 ? 'ran a script'
          // Code handed to an interpreter inline, too long to show: `node -e "import ..."`.
          : first.length > 40 && /\s-(?:e|c|-eval|-command|Command)\s+["'`]/.test(first) ? `ran a ${commandHead(first).replace(/\.exe$/i, '')} script`
          : `ran ${clip(first, 40)}`,
          held.count === 1 ? 'ran a command' : `ran ${pluralize(held.count, 'command')}`
        )
        break
      }
    }
  }
  /*
   * THE LINE SAYS WHAT WAS DONE, AS CLAUDE CODE'S DOES (0.610). Colin,
   * 2026-10-04, with Claude Code's app beside Locust's: "they are being
   * stacked on one line when claude code doesnt do that". Every Locust line
   * led with "Thought for 49s," and named things mid-line ("read main",
   * "edited storm.mjs, searched hul"); Claude Code's say only what was done,
   * in counts -- "Ran 9 commands, edited 2 files, created a file". So the
   * thought is the line only when nothing else was done (the rows say it
   * when the group is opened), and a line that mixes kinds counts and names
   * nothing; one kind alone keeps its name. Codex's headline -- what a
   * stretch of its work is for, "Inspecting the config" -- is the line
   * alone, as a described command's description is Claude Code's whole
   * line; the counts are in the rows.
   */
  if (headline !== undefined) {
    words.splice(0)
    briefs.splice(0)
    longs.splice(0)
    word(headline)
  } else if (words.length === 0 && thoughts > 0) {
    const thought = thoughtMs >= 1_000 ? `thought for ${durationText(thoughtMs)}` : 'thought'
    // Alone, closed: its first words say what about. Open, the words are underneath, once (0.594).
    const said = options.open !== true ? thoughtText?.replace(/\s+/g, ' ').trim() : undefined
    word(said !== undefined && said.length > 0 ? `${thought}: ${clip(said, 70)}` : thought, thought)
  }
  /*
   * ONE TITLED COMMAND THAT FAILED is said the way Claude Code says it:
   * "Failed to add the effort chip". Its description is an instruction to
   * itself, so the failure is the instruction not carried out.
   */
  const lone = details.length === 1 ? details[0] : undefined
  const loneFailed = lone?.kind === 'shell' && lone.title !== undefined && failed === 1 && refused === 0 && declined === 0
  const sentence = loneFailed
    ? `Failed to ${lowerFirst(lone.title!)}`
    : words.length === 0
      ? pluralize(details.length, 'step')
      // Mixed kinds count and name nothing (0.610, above); one kind alone keeps its name.
      : words.length > 1
        ? briefs.join(', ')
        : words[0]!
  const segments: TraceSegment[] = [{ key: 'what', text: `${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}`, ...(loneFailed ? { tone: 'amber' as const } : {}) }]
  if (failed > 0 && !loneFailed) segments.push({ key: 'failed', text: `${String(failed)} failed`, tone: 'amber' })
  // Every failure was the sandbox not starting: the line says so, not only the row (the look drive, 2026-10-10).
  if (sandboxFailed > 0 && sandboxFailed === failed) segments.push({ key: 'sandbox', text: 'Codex’s sandbox did not start', tone: 'amber' })
  if (working > 0) segments.push({ key: 'working', text: `${String(working)} working in the background` })
  if (silent > 0) segments.push({ key: 'silent', text: `${String(silent)} did not report`, tone: 'amber' })
  if (refused > 0) segments.push({ key: 'refused', text: `${String(refused)} refused`, tone: 'amber' })
  if (declined > 0) segments.push({ key: 'declined', text: `${String(declined)} declined`, tone: 'amber' })
  const whole = loneFailed || words.length === 0 ? sentence : longs.join(', ')
  return { segments, shorter: shorterForms(words, briefs, sentence, loneFailed), title: `${whole.charAt(0).toUpperCase()}${whole.slice(1)}` }
}

export function activitySummary(details: readonly ActivityDetail[]): string {
  // Files, not edit calls: one Codex file_change can touch several files, and
  // "Edited 1 file" over a two-file change is the wrong number.
  // An edit that FAILED edited nothing: a read-only Claude Code run whose
  // Write was refused still read "Edited 1 file" (seen driving, 2026-09-05).
  // The refused row stays in the list, marked failed; it counts as a call.
  // One file once, however many rows speak of it (0.494): a step's own change
  // and the host's look at the file after the run are one file.
  const editRows = details.filter((detail) => detail.kind === 'edit' && detail.failed !== true)
  // The same file however it was spelt: one path ends with the other at a folder boundary.
  const paths: string[] = []
  for (const path of editRows.flatMap((detail) => detail.name.split('\n').map((line) => line.trim()).filter((line) => line.length > 0))) {
    const flat = path.replace(/\\/g, '/').replace(/^\.\//, '').toLowerCase()
    // A name that is not a path (a tool's, `apply_patch`) says nothing of which file: each call counts.
    const same = !/[./]/.test(flat) ? -1 : paths.findIndex((held) => held === flat || held.endsWith(`/${flat}`) || flat.endsWith(`/${held}`))
    if (same === -1) paths.push(flat)
    else if (flat.length > paths[same]!.length) paths[same] = flat
  }
  const edits = paths.length
  const commands = details.filter((detail) => detail.kind === 'shell').length
  const helpers = details.filter((detail) => detail.kind === 'helper').length
  /*
   * REASONING IS NOT A TOOL CALL.
   *
   * It became an entry in 0.152.0 so the fold could show what a model
   * thought, and `other` is computed by subtraction -- so thinking was
   * counted as a tool call and the summary read "6 tool calls" over five,
   * with `thought` listed among their names. Colin, within the hour: "i
   * remember it being able to list all the tool calls individually."
   *
   * The count was doubly wrong, because the same line already says
   * `thought 24s` two segments earlier. A thing reported twice and counted
   * once too often.
   */
  const thinking = details.filter((detail) => detail.kind === 'reasoning').length
  const other = details.length - editRows.length - commands - helpers - thinking
  const parts: string[] = []
  if (edits > 0) parts.push(`Edited ${pluralize(edits, 'file')}`)
  if (commands > 0) parts.push(`ran ${pluralize(commands, 'command')}`)
  if (helpers > 0) parts.push(`asked ${pluralize(helpers, 'subagent')}`)
  if (other > 0) parts.push(`${pluralize(other, 'tool call')}`)
  return parts.length === 0 ? 'No tool activity' : parts.join(' · ')
}

/**
 * The commands a turn RAN, and what they returned.
 *
 * Astra's proposal, 2026-09-11, and the one part of it that needs no new
 * judgement from anybody: a turn's line says what it CHANGED -- `3 files` --
 * and never what it ran, so a run that edited three files and a run that
 * edited three files and proved them read identically. The ledger has had the
 * commands and their exit codes the whole time.
 *
 * Deliberately no inference about what a command MEANS. This does not decide
 * that `pnpm test` is a test and `ls` is not; it says what was run and what
 * came back, and the reader decides whether that is evidence. Anything else
 * would be the app guessing at proof, which is worse than staying quiet.
 *
 * An unsettled command is its own answer: a turn that is over and a command
 * that never reported is not a pass, and not a failure either.
 */
export interface CommandsRun {
  readonly ran: number
  /** Reported a non-zero exit, or failed outright. */
  readonly nonZero: number
  /** Started, and the turn ended without an outcome. Only meaningful once finished. */
  readonly unsettled: number
}

export function commandsRun(details: readonly ActivityDetail[], finished: boolean): CommandsRun {
  // A command the runtime refused never ran, so it is none of these.
  const shell = details.filter((detail) => detail.kind === 'shell' && detail.status !== 'refused' && detail.status !== 'declined')
  return {
    ran: shell.length,
    nonZero: shell.filter((detail) => detail.failed === true || (detail.exitCode !== undefined && detail.exitCode !== 0)).length,
    unsettled: finished
      ? shell.filter((detail) => detail.settled !== true && detail.failed !== true).length
      : 0
  }
}

/**
 * That, as the words the trace line uses. `undefined` when nothing ran.
 *
 * THE SUCCESS CASE NAMES THE COMMANDS RATHER THAN GRADING THEM.
 *
 * It shipped as `ran 4 commands · all exit 0`, and the design agent's ruling
 * on 2026-09-11 found the defect in it: that is the ONE segment on the line
 * where the app aggregates and grades. Every other segment names a thing that
 * happened; this collapsed four facts into a verdict, and "all" plus "0" is
 * about as close to the word PASSED as you can get without typing it. Their
 * test for whether a segment is grading -- "could it be wrong?" -- catches it
 * exactly: `all exit 0` cannot be wrong literally, but what it COMMUNICATES
 * can be, and that gap is the whole bug. A person who read it as "checked"
 * did not misread the line; the line told them.
 *
 * `ran pnpm check, tsc and 2 more` is the same derivation with no verdict in
 * it. The reader supplies the judgement, which is the division of labour this
 * whole feature committed to.
 *
 * The non-zero case is untouched, and deliberately: a command that failed IS
 * news, it IS derived, and amber is right because a person does need to act.
 */
export function commandsRunText(
  run: CommandsRun,
  names: readonly string[] = []
): { readonly text: string; readonly amber: boolean } | undefined {
  if (run.ran === 0) return undefined
  const ran = `ran ${pluralize(run.ran, 'command')}`
  if (run.nonZero > 0) {
    return {
      text: `${ran} · ${run.ran === 1 ? 'it exited non-zero' : `${String(run.nonZero)} exited non-zero`}`,
      amber: true
    }
  }
  if (run.unsettled > 0) {
    return {
      text: `${ran} · ${run.ran === 1 ? 'it did not report' : `${String(run.unsettled)} did not report`}`,
      amber: true
    }
  }
  const named = commandList(names, run.ran)
  return { text: named ?? ran, amber: false }
}

/**
 * `ran pnpm check, tsc and 2 more`, bounded to a line.
 *
 * The FIRST WORD of each command, which is the part that identifies it: the
 * rest is flags and paths, and four full command lines do not fit beside a
 * duration, a file count and a cost. `undefined` when nothing usable was
 * recorded, and then the caller falls back to the plain count -- a count is
 * not a verdict, it is just less useful.
 */
export function commandList(names: readonly string[], ran: number): string | undefined {
  /*
   * Each KIND of command once, and the count said as a count.
   *
   * It named the first word of the first two commands and counted every
   * other COMMAND as "more", so twelve commands -- one `mkdir && printf`
   * and eleven `printf` -- read "ran mkdir, printf and 10 more" over twelve
   * rows, and Claude Code, which opens most commands with `cd <folder> &&`,
   * read "ran cd, cd and 8 more" (Yurt's beta report, #6; Colin's frame of a
   * long run, 2026-09-23). The count is the commands; the names are what kind.
   */
  const heads = [...new Set(names.map(commandHead).filter((head) => head.length > 0 && head.length <= 24))]
  if (heads.length === 0) return undefined
  // Two named, then how many other kinds. Three names is already longer than
  // the rest of the line put together.
  const shown = heads.slice(0, 2)
  const kinds = heads.length > shown.length
    ? `${shown.join(', ')} and ${String(heads.length - shown.length)} more`
    : shown.join(' and ')
  // Every command a different one, and all of them named: the names are the count.
  if (ran === heads.length && heads.length <= 2) return `ran ${kinds}`
  return `ran ${pluralize(ran, 'command')}: ${kinds}`
}

/**
 * The word that says what a command is: its first, past any `cd <folder> &&`
 * in front of it -- that part says where it ran, not what ran.
 */
export function commandHead(name: string): string {
  let line = shellCommandText(name).split('\n')[0]?.trim() ?? ''
  for (;;) {
    const hop = /^(?:cd|pushd|chdir|set-location|sl)\s+(?:"[^"]*"|'[^']*'|\S+)\s*(?:&&|;)\s*/i.exec(line)
    if (hop === null) break
    line = line.slice(hop[0].length)
  }
  return line.split(/\s+/)[0] ?? ''
}

/**
 * What a subagent call without an ask of its own was doing. Codex's collab
 * tools arrive as `subagent:<verb>`; only spawn carries a prompt (2026-09-06).
 */
function subagentVerb(tool: string | undefined): string {
  const verb = tool === undefined ? undefined : /^subagent:(\w+)$/i.exec(tool)?.[1]
  switch (verb) {
    case 'wait':
    case 'wait_agent':
      return 'waiting for a subagent to report'
    case 'send_input':
      return 'a message to a subagent'
    case 'close_agent':
    case 'close':
      return 'closing a subagent'
    default:
      return 'a subagent, unnamed'
  }
}

/**
 * An MCP tool call, split into the two things a person recognises.
 *
 * A connector's tool arrives as one machine name --
 * `mcp__claude_ai_Robinhood__get_watchlists` -- and the row drew it whole.
 * Colin, seeing the first one that ever reached a teammate (2026-09-09):
 * "the mcp tool calls came out a little messy, might need to make that part
 * of the ui".
 *
 * The shape is `mcp__<server>__<tool>`, and both halves are worth having
 * separately: the SERVER is the thing a person connected and thinks in
 * ("Robinhood"), the TOOL is what was done with it ("get_watchlists"). That
 * is the same split the row already draws for every other tool -- the name
 * it acted on, and the tool that acted -- so an MCP call stops being a
 * special case and becomes an ordinary row.
 *
 * `claude_ai_` is stripped because it is a transport detail. An account
 * connector and a local server for the same product are the same product to
 * the person who connected it, and neither of them calls it "claude ai
 * Robinhood".
 *
 * Codex already sends `server.tool` for its own MCP calls, so that shape is
 * read too rather than left as the one runtime this does not help.
 */
export function mcpToolParts(name: string): { readonly server: string; readonly tool: string } | undefined {
  const doubled = /^mcp__(.+?)__(.+)$/.exec(name)
  if (doubled !== null) {
    return { server: prettyServer(doubled[1] ?? ''), tool: doubled[2] ?? '' }
  }
  /*
   * `Google_Drive__create_file` -- a connector tool with no `mcp__` prefix.
   *
   * MEASURED 2026-09-11, driving a real run: it read as `using a tool ·
   * Google_Drive__create_file`, an ordinary tool with a strange name. Every
   * connector name this app had ever been shown carried the prefix, so the
   * split required it; this one does not, and the shape is still unambiguous.
   *
   * Deliberately narrow. The left part must look like a name -- letters,
   * digits and single underscores, nothing that could be a path, a flag or a
   * sentence -- because the only thing separating this from a false positive
   * is that no ordinary tool is called `Read__something`. Read, Write, Bash,
   * Grep, Task and WebFetch carry no double underscore at all.
   */
  const bare = /^([A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)*)__([A-Za-z][A-Za-z0-9_-]*)$/.exec(name)
  if (bare !== null) {
    return { server: prettyServer(bare[1] ?? ''), tool: bare[2] ?? '' }
  }
  // Codex's own join, and only when it really is one: a dot in a file path
  // is not a server.
  const dotted = /^mcp[_.]?(?:tool)?[_.](.+)$/.exec(name)
  if (dotted !== null && dotted[1] !== undefined && dotted[1].includes('.')) {
    const at = dotted[1].indexOf('.')
    return { server: prettyServer(dotted[1].slice(0, at)), tool: dotted[1].slice(at + 1) }
  }
  return undefined
}

/**
 * A connector call whose NAME carries nothing to split.
 *
 * `mcpToolParts` reads `mcp__server__tool` and its cousins, which is every
 * shape Locust had been shown until Colin ran Antigravity against MCP on
 * 2026-09-20. That runtime names the tool `mcp` -- flatly, three rows of it
 * in his screenshot, `3 tool calls - mcp, mcp, mcp done` -- and puts the
 * server and the real tool somewhere the event does not carry. So there is
 * nothing to split, the split returned undefined, and the call was filed as
 * an ordinary local tool: no connector register, no connector orb, and no
 * permission chip saying the work left the machine.
 *
 * This says the ONE thing that is still knowable from the name: it went
 * through a connector. The server stays undefined, and the row says "using a
 * connector" without naming one rather than inventing a name for it.
 *
 * Deliberately a short closed list. `mcp` as a whole word is not a name any
 * ordinary tool has, but `mcp_server_config.json` is a file and `Read` of it
 * must not become a connector call.
 */
const BARE_CONNECTOR_TOOLS = new Set(['mcp', 'mcp_tool', 'mcptool', 'use_mcp_tool', 'call_mcp_tool', 'run_mcp_tool'])

export function isBareConnectorTool(name: string): boolean {
  return BARE_CONNECTOR_TOOLS.has(name.trim().toLowerCase())
}

/** The name a person connected, not the transport that carries it. */
function prettyServer(raw: string): string {
  return raw.replace(/^claude_ai_/, '').replace(/[_-]+/g, ' ').trim()
}

/**
 * Whether a tool call is a command, whatever the runtime calls its shell.
 *
 * This asked for the name `shell` or the kind `command_execution`, which
 * between them describe exactly ONE runtime: Codex. Claude Code names the
 * tool `Bash` and OpenCode names it `bash`, so every shell call either of
 * them made fell through to a generic tool row -- no exit-code badge,
 * nothing to expand, and counted under "other" in the turn summary rather
 * than as a command.
 *
 * Found trying to see 0.56.0's intent line on screen (2026-09-09). That
 * feature reads the model's own `description` for a Bash call, and both
 * those adapters carry it -- but the line is drawn by the COMMAND ROW, and
 * neither runtime ever produced one. So the feature was invisible on every
 * runtime: the two that send a description had no row, and the one with a
 * row sends no description. A test of the adapters passed the whole time.
 *
 * Matched case-blind, because the two spellings differ only in case and the
 * next runtime will pick one of them.
 */
/**
 * The command a person would recognise, with the shell wrapper taken off.
 *
 * A runtime does not run `dir /s /b`; it runs
 * `cmd /c "dir /s /b /o-d .locust sessions backups"`, and on this Windows box
 * every single shell call arrives wearing that prefix. The row then spends
 * its first nine characters on a fact that is true of every row, and the
 * command itself is what gets ellipsised off the end -- Colin, 2026-09-15,
 * looking at exactly that: "is this working as intended with the txt there,
 * i feel like claude code portrays it cleaner usually."
 *
 * It is right: the wrapper is transport, the same species as `mcp__` on a
 * connector name, which this file already strips for the same reason.
 *
 * Deliberately conservative. Only the wrappers this app itself spawns are
 * matched, only at the very start, and the quotes come off only when they
 * enclose the WHOLE remainder -- `cmd /c "a" && "b"` keeps its quotes,
 * because taking them off there would change what the line says. Anything
 * unrecognised is returned untouched: a command shown in full is never
 * wrong, only long.
 */
export function withoutShellWrapper(command: string): string {
  const trimmed = command.trim()
  const wrapper = /^(?:cmd(?:\.exe)?\s+\/[cCkK]|powershell(?:\.exe)?(?:\s+-\w+)*\s+-[cC]ommand|pwsh(?:\s+-\w+)*\s+-[cC]ommand|(?:\/bin\/)?(?:ba|z)?sh\s+-[lic]*c)\s+/.exec(trimmed)
  if (wrapper === null) return trimmed
  const rest = trimmed.slice(wrapper[0].length).trim()
  if (rest.length === 0) return trimmed
  for (const quote of ['"', "'"]) {
    if (rest.startsWith(quote) && rest.endsWith(quote) && rest.length > 1) {
      const inner = rest.slice(1, -1)
      // Only when the quotes wrap the whole thing: an inner quote means the
      // outer pair is punctuation inside a larger line, not a wrapper.
      if (!inner.includes(quote)) return inner.trim()
    }
  }
  return rest
}


/**
 * The orbs Locust draws — one per thing the live line can SAY.
 *
 * Seven of the library's nine. `weaving` and `shaping` are unused because
 * nothing here is honestly theirs, and both were also rejected by looking:
 * at 20px weaving is nearly invisible and shaping is a hard triangle that
 * reads as an icon rather than a state.
 */
export type OrbState =
  | 'searching'
  | 'working'
  | 'connecting'
  | 'solving'
  | 'breathing'
  | 'listening'
  | 'composing'
  | 'weaving'
  | 'shaping'

/**
 * THE FLOOR: alive, and nothing has been reported.
 *
 * Colin, 2026-09-20, with a frame of his own Cursor run — `Yurt · starting
 * ••• · 16s`, sixteen seconds and no orb: *"shouldnt it also show up for any
 * thinking or activity regardless of calls? ... the minor models should still
 * show SOMETHING no?"* He is right, and the first version's floor was wrong:
 * it drew an orb only while a tool was OPEN, so on OpenCode — which reports
 * its tools when they finish — the feature never appeared at all, and on any
 * runtime the long wait before the first tool showed nothing.
 *
 * `breathing` and not one of the four, CHOSEN BY LOOKING. Rendered at 20px,
 * every mapped state is a cloud of dots and `breathing` is a clean hollow
 * ring — the one shape in the set that cannot be mistaken for the others, and
 * the one that reads as calm rather than busy. That is exactly the claim it
 * has to make: the teammate is there, and nothing has come back yet.
 *
 * This EXTENDS the design agent's ruling from four states to five. Its rule
 * was "never invent a Locust activity for a state" and this does not: waiting
 * on the model is a real state the app already drew, with three dots. The
 * orb now says it, so the dots retire where it appears.
 */
const WAITING_ORB = 'breathing' as const

/**
 * Which orb a running step is, from the SAME facts its row's text is from.
 *
 * `detail.kind` is what `activityEntries` reads to decide whether a row is a
 * command, a subagent or a file, so an orb derived from it cannot contradict
 * the words beside it. That is the whole requirement: the orb reinforces a
 * fact the text states.
 *
 * Reading is spotted the way the fold spots it -- `READ_TOOL_WORDS` on the
 * tool's own name, which is the same set that stopped 38 Antigravity
 * `view_file` calls being counted as edits.
 */
export function orbStateFor(
  detail: { readonly kind: string; readonly name: string } | undefined,
  planMode: boolean,
  register?: 'starting' | 'working' | 'thinking' | 'writing' | 'tool' | 'connector'
): OrbState {
  // A connector call is named by its register rather than by its kind: the
  // detail carries the MCP tool, and what matters is that it left the machine.
  /*
   * A CONNECTOR GETS THE RUBIK, because Colin picked it out of a frame for
   * exactly this and because MCP had no shape of its own at all.
   *
   * 2026-09-20, with a screenshot of `solving` beside one of an Antigravity
   * turn calling MCP three times: *"working is showing the same animation as
   * grabbing an mcp tool call, i dont think we have mcp properly setup to its
   * own unique animation... this should be the one used for mcp or
   * connectors"*. He was right twice over -- see `isBareConnectorTool`, which
   * is why his MCP calls were not even being RECOGNISED as connector calls --
   * and this is the half of it that is about the picture.
   *
   * It costs the ordinary tool row its sphere: `connecting` takes that, which
   * is the shape this line used to have. The trade is deliberate. A tool call
   * is the commonest thing a run does and it already has a NAME on the row
   * beside it; a connector call is rare, reaches off the machine, and is the
   * one a person wants to spot without reading. The rarer event gets the
   * louder mark.
   *
   * It also retires the residual repeat this mapping used to admit to: a
   * Plan-mode run that opened a shell went `solving` to `solving`, and the
   * shell is `connecting` now. The new residual is Plan mode calling a
   * CONNECTOR, which is rarer still.
   */
  if (register === 'connector') return 'solving'
  // Planning is the turn's mode, not a tool, so it answers when nothing else
  // does -- a Plan-mode run with no tool open is a run that is planning.
  if (detail === undefined) {
    if (planMode) return 'solving'
    /*
     * ONE ORB PER WORD THE LINE CAN SAY (Colin, 2026-09-20): "when it
     * transitions to another word, an orb switch would be nice".
     *
     * Not a flourish — it is the difference between an indicator and an
     * ornament. The first version gave `starting` and `thinking` the same
     * ring and `working` and `writing` the same particles, so three of the
     * five transitions a run walks showed no change at all.
     *
     * CHOSEN BY RENDERING THE SEQUENCE, not by reasoning about names: the
     * candidate mappings were drawn at 20px side by side in the order a run
     * actually walks them. `weaving` for thinking is nearly invisible at that
     * size and `shaping` for writing is a hard square that reads as an icon
     * rather than a state; both were rejected on sight, twice, because the
     * names keep suggesting them and the pictures keep refusing.
     *
     * THEN COLIN NAMED THE RULE THE PICTURES WERE ALREADY MAKING.
     * 2026-09-20, after swapping two of these by eye: *"im sure you're
     * noticing a theme, the spherical ones are preffered for sure."*
     *
     * All nine were re-rendered at 20px from the library's own engine to
     * settle it with something other than an adjective, and at the size that
     * ships the set splits cleanly. THREE read as spheres — `searching` (a
     * dotted globe), `solving` (the scrambling one he picked for the plan)
     * and `listening` (a wave through latitude rings) — with `composing`, a
     * barrel of strokes, a near fourth. The rest are sparse clouds or flat
     * outlines: `working` is about four dots, `connecting` four, `breathing`
     * a hollow ring, `shaping` a square.
     *
     * NOTHING MAY REPEAT ACROSS A TRANSITION. Colin, 2026-09-20: "use
     * something different for working and using a tool, we shouldnt have the
     * same orb playing right after one another ever" -- both were
     * `listening`, so the busiest transition a run makes showed no change at
     * all, which is the same defect as the first version of this mapping
     * giving `starting` and `thinking` one ring between them.
     *
     * There are FOUR dense shapes and exactly four live registers that must
     * differ, so they go one each: `listening` to thinking, `composing` to
     * working, `solving` to every tool, `searching` to reading. `composing`
     * is on working because Colin picked that shape out of a frame and asked
     * where it would be seen most -- one drive of a free-route turn spent 7
     * samples on starting and 24 on working, since a runtime that reports
     * tools only on completion is "working" for nearly the whole turn.
     *
     * `solving` on tools cost the PLAN its rubik, which was Colin's own
     * earlier pick: the plan's step and the live line are on screen TOGETHER
     * during a run, and two identical spheres animating a few pixels apart is
     * worse than a sequential repeat. He chose the trade; the plan's marker
     * is the calm ring now and all four spheres live on the line people
     * actually watch.
     *
     * ONE RESIDUAL REPEAT, said out loud rather than hidden: a Plan-mode run
     * that opens a SHELL goes `solving` to `solving`. Plan mode is read-only,
     * so a shell there is rare; if it stops being rare, this is the thing to
     * fix.
     *
     * The older note this replaces: the allocation idea. `listening`
     * takes the working register — the busiest line in the app, and the one
     * whose four sparse dots Colin flagged twice — and `working` inherits
     * `writing`, which is the briefest word a run says and therefore the
     * cheapest place to spend the weakest asset.
     *
     * The honest-on-its-own-terms reading survives the move, for whatever it
     * is worth: a model grinding through a tool IS listening for the result,
     * and a model streaming its answer IS composing one. But the pictures
     * decided, not that sentence.
     */
    if (register === 'writing') return 'shaping'
    if (register === 'working') return 'composing'
    if (register === 'thinking') return 'listening'
    /*
     * A STEP THAT SAYS `tool` WITHOUT AN OPEN TOOL still means a tool is
     * running. Codex reports both a step and its tools, and the step can be
     * the live line while no tool detail is open — so this fell through to
     * the waiting ring and the row read **"using a tool"** beside an orb
     * whose label is "Thinking…". Measured on the first paid drive,
     * 2026-09-20: sixteen consecutive samples of exactly that.
     *
     * It is the contradiction the whole mapping exists to prevent, and it was
     * invisible on every free route because no free runtime streams steps.
     */
    if (register === 'tool') return 'connecting'
    return WAITING_ORB
  }
  /*
   * A connector reaches OFF this machine, which is the one fact the
   * permission chip exists to say — so `connecting` is the literal truth
   * about it, not an approximation. A subagent is the same shape of claim:
   * work is happening somewhere this row cannot show you.
   */
  if (detail.kind === 'helper') return 'weaving'
  if (detail.kind === 'shell') return 'connecting'
  const words = detail.name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(/[^A-Za-z]+/)
    .map((word) => word.toLowerCase())
  if (words.some((word) => READ_TOOL_WORDS.has(word))) return 'searching'
  /*
   * Any other open tool -- writing a file, or one this app does not
   * classify -- is `connecting`. A tool is running, which is all this claims;
   * the row says which tool. The specific ones claim MORE than that, so they
   * are never the fallback.
   */
  return 'connecting'
}

function toolKindOf(event: Extract<NormalizedRuntimeEvent, { type: 'tool.started' }>): string {
  const name = event.payload.name
  const command = event.payload.command
  if (isShellTool(name, event.payload.toolKind)) {
    return command !== undefined && isEditCommand(command) ? 'edit' : 'shell'
  }
  // A runtime's own sub-agent: Claude Code's `Task`, OpenCode's `task`.
  if (SUBAGENT_TOOL.test(name)) return 'helper'
  return editToolName(name) ? 'edit' : 'tool'
}


/**
 * Rebuild the assistant text the way the transcript did. Deltas carry an
 * `append` or `replace` operation against a per-item buffer, so taking the last
 * delta's text alone returns only its final fragment -- the same trap the
 * checkpoint reconciler documents.
 */
export function assistantMessages(
  events: readonly NormalizedRuntimeEvent[]
): readonly { readonly itemId: string; readonly text: string; readonly final: boolean }[] {
  const order: string[] = []
  const buffers = new Map<string, { text: string; final: boolean }>()
  for (const event of events) {
    if (event.type !== 'message.delta') continue
    const { itemId, operation, text, final } = event.payload
    const existing = buffers.get(itemId)
    if (existing === undefined) order.push(itemId)
    const next = operation === 'replace' ? text : `${existing?.text ?? ''}${text}`
    buffers.set(itemId, { text: next, final })
  }
  return order.map((itemId) => ({ itemId, ...buffers.get(itemId)! }))
}

/**
 * Everything a teammate SAID in one turn, as one piece of prose.
 *
 * For the surfaces that draw a turn as a single utterance -- a room's
 * exchange, the collapsed answers list -- where the thread itself draws each
 * message separately.
 *
 * It exists because those surfaces each took `the last message marked final`
 * and fell back to `the latest message` while none was. So a teammate's
 * progress appeared as it was written and vanished the moment the turn
 * finished, the final message replacing everything before it (Colin,
 * 2026-09-13). A turn is what was said, all of it, in order.
 */
export function turnText(events: readonly NormalizedRuntimeEvent[]): string {
  return assistantMessages(events)
    .map((message) => message.text)
    .filter((text) => text.trim().length > 0)
    .join(PARAGRAPH_GAP)
}

/** A blank line between two things that were said separately. */
const PARAGRAPH_GAP = String.fromCharCode(10, 10)

export interface MissionThreadOptions {
  /** While a run is live the last message shows a streaming caret. */
  readonly running: boolean
  /**
   * The turn has more events than are drawn (shared/event-window.ts): its first
   * steps are kept in the record but not here, and the turn says so at its top
   * rather than reading as if it began where the window does (0.627).
   */
  readonly trimmed?: boolean
  /**
   * When the run was started, so the waiting line can time the launch itself.
   * Without it a run with no events yet has no clock to show, and the line is
   * held back -- which is the lag it exists to remove.
   */
  readonly startedAt?: string
  /** What the live line says while nothing has arrived (0.602): the start's phase, else "Starting". */
  readonly startingLabel?: string
  /**
   * True when the run is stopped on an approval. A run awaiting a decision is
   * still `running`, so the waiting line would sit directly above the card
   * asking the question and say "Working" while the header says "waiting on
   * you" -- two surfaces describing one teammate differently, which is the
   * exact defect `faceState.ts` exists to prevent. The card is already saying
   * what is happening, so nothing is added above it.
   */
  readonly awaitingDecision?: boolean
  /** Whether the run was allowed to change files; see activityTrace. */
  readonly mayEdit?: boolean
  /**
   * The turn was sent in Plan mode. Only the orb reads this: planning is the
   * turn's MODE rather than a tool, so it is the one of the four mapped orbs
   * that no open tool can answer for.
   */
  readonly planMode?: boolean
  /**
   * The folder the conversation is open in. The file COUNT needs it for the
   * same reason each file ROW does -- see activityTrace.
   */
  readonly workspacePath?: string
  /**
   * The plan as the turn before this one left it (`lastPlanOf`).
   *
   * OpenCode keeps ONE to-do list for the whole session and sends all of it
   * on every turn, so turn eight's card read "8 of 8 done" over the seven
   * steps turns one to seven had already finished (Yurt's beta report, #5;
   * the same frames show it making every turn taller). A step that was done
   * before this turn began, and was not worked on again in it, is the
   * earlier turn's -- and its card already shows it.
   */
  readonly carriedPlan?: readonly PlanStep[]
  /**
   * Notices an earlier turn's fold already carried (`noticeKey`).
   *
   * Codex remarks on its own setup as every turn opens -- "Skill descriptions
   * were shortened to fit the skills context budget" -- and each turn's fold
   * said it again (Yurt's beta report, #10). A conversation hears it once.
   */
  readonly saidBefore?: ReadonlySet<string>
  /**
   * Whether this turn wrote to a teammate. Those messages are drawn beside
   * the thread rather than inside it, so a turn whose whole output was a
   * message to a colleague looked, from in here, like a turn that said
   * nothing -- and got told so, in a warning sitting directly above the
   * message it had just sent (Colin, 2026-09-06: "the this turn ended with a
   * reply intended?").
   */
  readonly spokeToPeers?: boolean
  /**
   * True for the turn at the END of the conversation. A question a runtime
   * asked is only answerable there: on an earlier turn the answer already
   * exists -- it is the next turn's prompt, visible a few lines below -- and
   * offering the buttons again would invite answering it twice.
   */
  readonly latestTurn?: boolean
}

export function buildThread(
  events: readonly NormalizedRuntimeEvent[],
  options: MissionThreadOptions
): readonly ThreadItem[] {
  // Project display text without changing the recorded events or their evidence.
  events = events.map((event) => {
    if (event.type === 'adapter.diagnostic') {
      return { ...event, payload: { ...event.payload, message: providerErrorSentence(event.payload.message, event.sourceAdapter) } }
    }
    if (event.type === 'route.limit_detected') {
      return { ...event, payload: { ...event.payload, message: providerErrorSentence(event.payload.message, event.sourceAdapter) } }
    }
    if (event.type === 'run.failed') {
      return { ...event, payload: { ...event.payload, message: failureMessage(event.payload, event.sourceAdapter) } }
    }
    return event
  })
  const endedOn = options.running ? undefined : events.find((event) => event.type === 'run.failed')
  const items: ThreadItem[] = []
  const openTools = new Map<string, ActivityDetail>()
  /**
   * When each open tool began, so the live line can be the TOOL rather than
   * the turn around it.
   *
   * Claude Code reports no step for a tool call -- only `tool.started` -- so
   * a run that spent thirty seconds reading files said "working" the whole
   * way (MEASURED 2026-09-11, `probe-live-line-says-which`: `sawATool:
   * false`). The register was honest and useless, which is the worst of both.
   */
  // `connector` is the SERVER when the name carries one; `viaConnector` is
  // whether it left the machine at all. They came apart when a runtime turned
  // up that says the second without the first -- see `isBareConnectorTool`.
  const openToolAt = new Map<string, { readonly at: string; readonly connector: string | undefined; readonly viaConnector: boolean }>()
  /**
   * Disk observations that landed on a row the runtime drew, by item id, with
   * the file's name. When the observation's patch arrives it is the NET
   * change to that file for the whole run, so every other edit row the
   * runtime drew for the same file is a step already inside it.
   */
  const observedNet = new Map<string, string>()
  /**
   * Rows for calls the runtime ran in the background, by item id, kept after
   * the call settles: the call returns at once and the work goes on, so what
   * became of it arrives after the row has closed.
   */
  const backgroundRows = new Map<string, ActivityDetail>()
  /**
   * Every row by the call it came from, once closed. Claude Code may return a
   * helper's launch before it says the helper went to the background, and
   * the news then has to find a row that is no longer open (0.492).
   */
  const closedRows = new Map<string, ActivityDetail>()
  /**
   * A HELPER'S OWN CALLS (ledger v22, helper visibility 2026-10-05), by item
   * id, with the helper row each belongs to. They are kept on that row as its
   * children and nowhere else: not in the turn's rows, its counts, its live
   * line. A call whose helper has no row here is dropped, as every helper
   * call was before v22.
   */
  const helperCalls = new Map<string, { readonly parent: string; detail: ActivityDetail }>()
  const helperRowOf = (parent: string): ActivityDetail | undefined => {
    const row = openTools.get(parent) ?? backgroundRows.get(parent) ?? closedRows.get(parent)
    return row?.kind === 'helper' ? row : undefined
  }
  /** Put the helper's row back with its children changed, wherever it is held. */
  const withHelperChildren = (parent: string, change: (children: readonly ActivityDetail[]) => readonly ActivityDetail[]): boolean => {
    const row = helperRowOf(parent)
    if (row === undefined) return false
    const next: ActivityDetail = { ...row, children: change(row.children ?? []) }
    const index = activity.indexOf(row)
    if (index >= 0) activity[index] = next
    if (openTools.get(parent) === row) openTools.set(parent, next)
    if (backgroundRows.get(parent) === row) backgroundRows.set(parent, next)
    if (closedRows.get(parent) === row) closedRows.set(parent, next)
    return true
  }
  /** The file's own name, however the runtime spelt the path to it. */
  const nameTail = (name: string): string => name.toLowerCase().replace(/\\/g, '/').split('/').at(-1) ?? name
  /*
   * H10: WHICH ROW A HOST OBSERVATION IS OF, by path, not by name. It was the
   * file's basename, so packages/a/package.json and packages/b/package.json
   * were one row, showing one diff, with one file counted and one in
   * Artifacts. The observation names a workspace-relative path and a
   * runtime's row may name it absolutely, relatively or bare: they are the
   * same file when one ends with the other at a folder boundary. A bare name
   * is trusted only when exactly one row has it.
   */
  const normalPath = (name: string): string => name.toLowerCase().replace(/\\/g, '/').replace(/\/+$/, '')
  const workspace = options.workspacePath === undefined ? undefined : normalPath(options.workspacePath)
  /** A path as the workspace knows it: its prefix stripped when it is inside. */
  const relative = (name: string): string => {
    const path = normalPath(name)
    return workspace !== undefined && path.startsWith(`${workspace}/`) ? path.slice(workspace.length + 1) : path
  }
  const isAbsolutePath = (path: string): boolean => /^[a-z]:\//.test(path) || path.startsWith('/')
  const samePath = (a: string, b: string): boolean => {
    const ra = relative(a)
    const rb = relative(b)
    if (ra === rb) return true
    // An absolute path outside a known workspace, beside a relative one:
    // the same file when the absolute one ends with it at a folder boundary.
    if (isAbsolutePath(ra) && !isAbsolutePath(rb)) return ra.endsWith(`/${rb}`)
    if (isAbsolutePath(rb) && !isAbsolutePath(ra)) return rb.endsWith(`/${ra}`)
    return false
  }
  const editRowOf = (observed: string): ActivityDetail | undefined => {
    // A failed edit changed nothing (0.672): Haiku's first two Edits of a file
    // failed, unread, before a third landed, and the host's look at that file
    // was laid on the first failure -- the row the turn's files card then kept.
    const edits = activity.filter((detail) => detail.kind === 'edit' && detail.failed !== true)
    const exact = edits.filter((detail) => relative(detail.name) === relative(observed))
    if (exact.length > 0) return exact[0]
    // Anything looser is trusted only when it names exactly one row.
    const byPath = edits.filter((detail) => samePath(detail.name, observed))
    // Several rows of ONE file (two edits to it) are one candidate; rows of
    // two different files that both end with the name are not a match.
    if (new Set(byPath.map((detail) => relative(detail.name))).size === 1) return byPath[0]
    if (byPath.length > 1) return undefined
    const tail = nameTail(observed)
    const named = edits.filter((detail) => !normalPath(detail.name).includes('/') && nameTail(detail.name) === tail)
    return named.length === 1 ? named[0] : undefined
  }
  const activity: ActivityDetail[] = []
  /**
   * The event each row of `activity` came from, kept in step with it, so
   * what the teammate SAID between its steps can be put back among them
   * (see the walk at the end). A row replaced in place keeps its origin; a row
   * removed takes its entry with it.
   */
  const activityBorn: number[] = []
  let runningStep:
    | {
        label: string
        detail: string | undefined
        startedAt: string
        kind: 'turn' | 'reasoning' | 'item'
        register: 'working' | 'thinking' | 'writing' | 'tool'
        /** The step's own words, held while the line says Writing (0.586), given back when the text is final. */
        labelBeforeWriting?: string
      }
    | undefined
  let plan: readonly PlanStep[] = []
  /** Every step this turn's plans showed unfinished at some point. */
  const workedSteps = new Set<string>()
  // Notices that arrive before the run has done anything are the runtime
  // talking about its own setup (a skills budget, a config warning), not
  // about the mission. They stay in the Signal Rail; the thread keeps only
  // notices raised while the work was under way.
  let workBegan = false
  /** Diagnostics the thread gate drops, drawn at the foot of the fold instead. */
  const foldNotices: { readonly level: 'info' | 'warning' | 'error'; readonly message: string; readonly source: MissionRuntimeId }[] = []
  /** What the host saved on the teammate's branch (0.439): the turn's last line. */
  const receipts: ThreadItem[] = []

  for (const [eventIndex, event] of events.entries()) {
    switch (event.type) {
      case 'tool.started': {
        /*
         * A HELPER'S CALL goes under the helper's row, as Claude Code folds
         * them under its helper line. It is not the teammate's work: it does
         * not start the thread's work, take the live line or count.
         */
        if (byHelper(event.payload)) {
          const parent = (event.payload as { readonly parentItemId: string }).parentItemId
          const mcpChild = mcpToolParts(event.payload.name)
          const child: ActivityDetail = {
            kind: toolKindOf(event),
            name: mcpChild?.tool ?? (event.payload.command === undefined ? undefined : withoutShellWrapper(event.payload.command)) ?? event.payload.name,
            tool: mcpChild?.server ?? event.payload.name,
            ...(typeof event.payload.title === 'string' && event.payload.title.length > 0 ? { title: event.payload.title } : {}),
            settled: false
          }
          const held = helperCalls.get(event.payload.itemId)
          if (held !== undefined) {
            const was = held.detail
            held.detail = child
            withHelperChildren(held.parent, (children) => children.map((entry) => (entry === was ? child : entry)))
          } else if (withHelperChildren(parent, (children) => [...children, child])) {
            helperCalls.set(event.payload.itemId, { parent, detail: child })
          }
          break
        }
        workBegan = true
        // The host's disk observation of a path the runtime already named:
        // its patch belongs on the runtime's row, not on a second one.
        //
        // This used to require the runtime's row to have NO patch, which
        // quietly meant "only for Codex": its `file_change` names a path and
        // sends no diff, while Cursor, OpenCode and Claude all diff their own
        // edits. So on every one of those, the observation of a change the
        // runtime had ALREADY reported became a second, identical row --
        // measured 2026-09-07 by `_tools/drive-reveal.mjs`, where one written
        // file drew `report.md ADDED +1 -0` twice under a line reading
        // `1 file`. The stated intent above was always unconditional; the
        // condition was the accident.
        if (event.payload.toolKind === 'observed_edit' && /reported by the runtime/.test(event.payload.status ?? '')) {
          const path = event.payload.command ?? ''
          const own = editRowOf(path)
          /*
           * ONLY ONTO A ROW WITH NO CHANGE OF ITS OWN (0.494). The observation
           * is the file's NET change over the run; laid onto a step that had
           * reported its own diff, it replaced that step's change with the whole
           * run's -- a +2/-2 refinement read +81/-1 once the turn ended (Sol,
           * 0.492). A step keeps what it did; the net change is its own row,
           * after the run, which the turn's files card reads instead.
           */
          if (own !== undefined && own.patch === undefined) {
            openTools.set(event.payload.itemId, own)
            openToolAt.set(event.payload.itemId, { at: event.occurredAt, connector: undefined, viaConnector: false })
            observedNet.set(event.payload.itemId, path)
            break
          }
        }
        // A connector call names the SERVER as its tool and the tool as its
        // name, which is the split every other row already uses.
        const mcp = mcpToolParts(event.payload.name)
        const detail: ActivityDetail = {
          kind: toolKindOf(event),
          name:
            mcp?.tool
            ?? (event.payload.command === undefined ? undefined : withoutShellWrapper(event.payload.command))
            ?? event.payload.name,
          tool: mcp?.server ?? event.payload.name,
          // Only Claude Code and OpenCode send one; the others leave it
          // undefined and their rows read exactly as they always have.
          ...(typeof event.payload.title === 'string' && event.payload.title.length > 0
            ? { title: event.payload.title }
            : {}),
          // Claude Code's Bash tool takes `run_in_background`, and the flag
          // rides on the tool call's own input. Carried, not interpreted.
          ...(event.payload.background === true ? { background: true } : {}),
          // A helper's type, when its launch already says it (Claude Code,
          // helper visibility): the row names it while it works.
          ...(SUBAGENT_TOOL.test(event.payload.name) && typeof event.payload.status === 'string' && event.payload.status.length > 0 ? { status: event.payload.status } : {}),
          settled: false
        }
        /*
         * A call announced again is the same call, named now.
         *
         * Claude Code's start carries the tool's name only; its adapter
         * restates the start when the call's input arrives, with the command
         * and the model's description of it (claude-events.ts). The row it
         * already drew takes the names, and keeps its place and its clock.
         */
        const already = openTools.get(event.payload.itemId)
        if (already !== undefined) {
          const at = activity.indexOf(already)
          // A helper's calls already under its row stay there.
          const named: ActivityDetail = already.children === undefined ? detail : { ...detail, children: already.children }
          openTools.set(event.payload.itemId, named)
          if (at !== -1) activity[at] = named
          break
        }
        openTools.set(event.payload.itemId, detail)
        // Whether it reaches OFF this machine, decided by the same split the
        // row uses. `tool !== name` cannot answer it: an ordinary call names
        // the tool and its target too (`Read` / `README.md`).
        openToolAt.set(event.payload.itemId, {
          at: event.occurredAt,
          connector: mcp?.server,
          viaConnector: mcp !== undefined || isBareConnectorTool(event.payload.name)
        })
        activity.push(detail)
        activityBorn.push(eventIndex)
        break
      }
      case 'tool.completed':
      case 'tool.failed': {
        if (byHelper(event.payload)) {
          const held = helperCalls.get(event.payload.itemId)
          if (held === undefined) break
          const was = held.detail
          const patch = event.payload.patch
          const closed: ActivityDetail = {
            ...was,
            settled: true,
            failed: event.type === 'tool.failed',
            ...(event.payload.exitCode === undefined ? {} : { exitCode: event.payload.exitCode }),
            ...(event.payload.status === undefined ? {} : { status: event.payload.status }),
            ...(typeof event.payload.output === 'string' ? { output: event.payload.output } : {}),
            ...(patch === undefined ? {} : { patch, kind: 'edit' }),
            ...(event.payload.command === undefined || was.name !== was.tool ? {} : { name: event.payload.command })
          }
          held.detail = closed
          withHelperChildren(held.parent, (children) => children.map((entry) => (entry === was ? closed : entry)))
          break
        }
        const open = openTools.get(event.payload.itemId)
        if (open !== undefined) {
          const index = activity.indexOf(open)
          const patch = event.payload.patch
          // How long it took (0.459): as the runtime timed it when it says
          // (OpenCode), else from its start to its end as they arrived.
          const began = Date.parse(openToolAt.get(event.payload.itemId)?.at ?? '')
          const ended = Date.parse(event.occurredAt)
          const timed = (event.payload as { readonly durationMs?: unknown }).durationMs
          const tookMs = typeof timed === 'number' && Number.isFinite(timed) && timed >= 0
            ? timed
            : Number.isNaN(began) || Number.isNaN(ended) ? undefined : Math.max(0, ended - began)
          if (index >= 0) {
            activity[index] = {
              ...open,
              settled: true,
              ...(tookMs === undefined ? {} : { durationMs: tookMs }),
              failed: event.type === 'tool.failed',
              ...(event.payload.exitCode === undefined ? {} : { exitCode: event.payload.exitCode }),
              ...(event.payload.status === undefined ? {} : { status: event.payload.status }),
              ...(typeof event.payload.output === 'string' ? { output: event.payload.output } : {}),
              // A completion that carries a patch also names what it touched:
              // the row's kind follows the evidence, not the tool's name.
              ...(patch === undefined ? {} : { patch, kind: 'edit' }),
              // Some runtimes only know what a tool acted on once it is done:
              // Claude streams a tool's input as JSON deltas AFTER the call
              // opens, so its rows read `Read done` with no file until the
              // target arrives here. A start that already named one keeps it.
              ...(event.payload.command === undefined || open.name !== open.tool
                ? {}
                : { name: event.payload.command }),
              /*
               * The same late arrival, for what the call was FOR.
               *
               * Claude Code writes a description on every Bash call and says
               * whether it was sent to the background -- both on the call's
               * input, which comes after the row opens -- so the adapter
               * carries them on the completion. This read neither from
               * there. MEASURED 2026-09-22 by running a real Claude capture
               * through the adapter and this function: the row had the
               * command, no description and no background flag, so since
               * 2026-09-09 no Claude command row has led with its
               * description, and the 0.257.0 "in the background" badge
               * never reached a Claude row. Both were tested at the adapter
               * alone, where they were true.
               */
              ...(open.title === undefined && typeof event.payload.title === 'string' && event.payload.title.length > 0
                ? { title: event.payload.title }
                : {}),
              ...(event.payload.background === true ? { background: true } : {})
            }
            const closed = activity[index]
            if (closed !== undefined) closedRows.set(event.payload.itemId, closed)
            if (closed?.background === true) backgroundRows.set(event.payload.itemId, closed)
            /*
             * The observation is the whole change to that file, so the
             * runtime's OTHER rows for it are steps inside it, not more of it.
             *
             * MEASURED 2026-09-16 by Astra on 0.154.0, OpenCode changing two
             * lines of one README in two edits: the observation attached to
             * the first row (+2 -2), the second row stayed (+1 -1), and the
             * card said `1 file +3 -3` over a file git reported as `2 2`.
             * Each runtime row was true on its own; together with the net
             * they counted the second line twice. The attach was written for
             * one edit per file and never said so.
             */
            const observedPath = observedNet.get(event.payload.itemId)
            if (observedPath !== undefined && patch !== undefined) {
              const keep = activity[index]
              for (let at = activity.length - 1; at >= 0; at -= 1) {
                const other = activity[at]
                // H10: the same FILE's other rows, never another file's with its name.
                if (other !== keep && other.kind === 'edit' && (samePath(other.name, observedPath) || (keep !== undefined && samePath(other.name, keep.name)))) {
                  activity.splice(at, 1)
                  activityBorn.splice(at, 1)
                }
              }
            }
          }
          observedNet.delete(event.payload.itemId)
          openTools.delete(event.payload.itemId)
        }
        openToolAt.delete(event.payload.itemId)
        break
      }
      case 'step.started': {
        // Background work starting is not what the teammate is doing now --
        // it sent the work away so it could do something else -- so it does
        // not take the live line. Its row already says where it went.
        if (event.payload.itemType === 'background') {
          // What that work is doing now lands on its row (0.492): a helper in
          // the background said "reported back" the moment it was launched.
          const itemId = event.payload.itemId
          const said = typeof event.payload.message === 'string' ? event.payload.message.trim() : ''
          const row = itemId === undefined ? undefined : backgroundRows.get(itemId) ?? openTools.get(itemId) ?? closedRows.get(itemId)
          const index = row === undefined ? -1 : activity.indexOf(row)
          if (itemId !== undefined && row !== undefined && index >= 0) {
            const going: ActivityDetail = { ...row, background: true, ...(said.length > 0 ? { progress: said } : {}) }
            activity[index] = going
            if (openTools.get(itemId) === row) openTools.set(itemId, going)
            else {
              backgroundRows.set(itemId, going)
              closedRows.set(itemId, going)
            }
          }
          break
        }
        // A turn opening is not work yet: Codex raises its setup notices
        // right after it, before any tool runs, and counting the turn as work
        // put "skill descriptions were shortened" back in every thread.
        if (event.payload.stepKind !== 'turn') workBegan = true
        const message = event.payload.message
        // A message item is the model writing, not a tool: "Thinking ·
        // userMessage · 5s" beside an approval card read as a tool nobody
        // had heard of (seen driving the app, 2026-09-05). The label stays;
        // the item type is only worth showing when it names real work.
        const itemType = event.payload.itemType
        // A message ITEM is the model writing; anything else a runtime opens
        // an item for is it using something. Read off the item type, which is
        // the same fact the detail is suppressed for.
        const writingNow = itemType !== undefined && /message$/i.test(itemType)
        // A reasoning ITEM is thought, not a tool (QA-2026-09-29 round 2,
        // R31): a Codex run only thinking read "Using a tool..." throughout.
        const thinkingNow = itemType !== undefined && /reasoning/i.test(itemType)
        runningStep = {
          label: message ?? (event.payload.stepKind === 'turn' ? 'Working' : 'Thinking'),
          // The item type is gone from the line. It was there to name the
          // kind of work -- `subagent`, `commandExecution` -- and the register
          // now does that in words a person uses, so printing the runtime's
          // own word for it beside them is the same fact twice, in jargon.
          detail: undefined,
          startedAt: event.occurredAt,
          kind: event.payload.stepKind,
          register:
            event.payload.stepKind === 'reasoning' || thinkingNow
              ? 'thinking'
              : event.payload.stepKind === 'turn'
                ? 'working'
                : writingNow
                  ? 'writing'
                  : 'tool'
        }
        break
      }
      case 'step.completed':
      case 'step.failed': {
        /*
         * BACKGROUND WORK ENDING lands on the row of the call that started
         * it, and leaves the live line alone: the teammate may be in the
         * middle of something else, and ending that line here would say it
         * had stopped.
         */
        if (event.payload.itemType === 'background') {
          const itemId = event.payload.itemId
          const row = itemId === undefined ? undefined : backgroundRows.get(itemId) ?? openTools.get(itemId) ?? closedRows.get(itemId)
          const index = row === undefined ? -1 : activity.indexOf(row)
          if (itemId !== undefined && row !== undefined && index >= 0) {
            const summary = typeof event.payload.message === 'string' && event.payload.message.trim().length > 0 ? event.payload.message.trim() : undefined
            const ended: ActivityDetail = {
              ...row,
              background: true,
              backgroundEnded: backgroundEndingOf(event.payload.status, event.type === 'step.failed'),
              // A helper's report, when it came back with one.
              ...(summary !== undefined && row.kind === 'helper' ? { output: summary } : {})
            }
            activity[index] = ended
            backgroundRows.set(itemId, ended)
            if (closedRows.has(itemId)) closedRows.set(itemId, ended)
            if (openTools.get(itemId) === row) openTools.set(itemId, ended)
          }
          break
        }
        /*
         * REASONING BECOMES A ROW, when the runtime sent its text.
         *
         * It has always contributed a `thought 52s` to the summary line and
         * nothing else, so a run that thought for a minute could say how
         * long and not a word about what. Colin, asked straight on
         * 2026-09-16: keep it -- "i wanted to sacrifice nothing."
         *
         * It goes in the FOLD rather than above it, because the fold is the
         * record of how the work was carried out and reasoning is exactly
         * that. Above the fold is the answer, and thinking is not an answer.
         *
         * Only when there is text. Most runtimes send none, and a row
         * reading `thought` with nothing in it would be a row that promises
         * something it does not have.
         */
        // Codex opens its thinking as a `reasoning` ITEM rather than a
        // reasoning step (0.489): the same thought, the same row.
        const reasoningItem = typeof event.payload.itemType === 'string' && /reasoning/i.test(event.payload.itemType)
        if (event.payload.stepKind === 'reasoning' || reasoningItem) {
          const said = typeof event.payload.message === 'string' ? event.payload.message.trim() : ''
          // How long, from the step that opened it. The row reads
          // "Thought for 12s" and folds the text under it -- the shape
          // Claude Code used, which Colin asked for on 2026-09-17: "it
          // would say how long they thought for ... and then you could
          // just hit a dropdown". Absent when the start was never seen.
          const began = runningStep?.register === 'thinking' ? Date.parse(runningStep.startedAt) : NaN
          const ended = Date.parse(event.occurredAt)
          // As the runtime timed it when it says (Antigravity CLI reports a step only once it ends, 0.542).
          const timed = (event.payload as { readonly durationMs?: unknown }).durationMs
          const durationMs = typeof timed === 'number' && Number.isFinite(timed) && timed >= 0
            ? timed
            : Number.isNaN(began) || Number.isNaN(ended) ? undefined : Math.max(0, ended - began)
          /*
           * THE LENGTH ALONE, when there are no words (0.489,
           * DISPLAY-COVERAGE gap 6). Claude Code and Antigravity show
           * "Thought for 12s" over every thinking block; Locust showed a row
           * only when the runtime sent text, which most never do, so a
           * minute of thinking left no mark in the thread. The row is the
           * duration and opens onto nothing. Under a second is not worth a
           * line.
           */
          if (said.length > 0 || (durationMs !== undefined && durationMs >= 1_000)) {
            workBegan = true
            activity.push({ kind: 'reasoning', name: 'thought', settled: true, output: said, ...(durationMs === undefined ? {} : { durationMs }) })
            activityBorn.push(eventIndex)
          }
        }
        runningStep = undefined
        break
      }
      /*
       * TEXT ARRIVING IS THE MODEL WRITING, and saying so is the only honest
       * orb a runtime like OpenCode can be given.
       *
       * Colin, 2026-09-21, after the finding that OpenCode shows one orb for
       * a whole run: *"you think we can atleast give them SOME orb notifiers
       * for opencode? even if they arent entirely accurately reporting a
       * tool?"* -- and the answer has to be yes WITHOUT the second half. A
       * tool orb on a runtime that reports tools only once they are finished
       * would be the app claiming something is happening that already
       * happened, which is the one thing the mapping exists to prevent.
       *
       * This claims nothing extra. A `message.delta` IS the model writing,
       * now, and `writing` is a register the app already has with an orb
       * already allocated to it. OpenCode opens exactly one step -- a `turn`,
       * which becomes `working` -- so before this it could never reach the
       * writing register at all, however much text it streamed.
       *
       * ONLY FROM `working`. A step that said something more specific
       * (`thinking`, `tool`, `connector`) keeps what it said: those are
       * claims about work this does not know better than.
       *
       * It also returns. A final delta means the text is complete, so the run
       * goes back to working rather than staying on a claim that has stopped
       * being true -- the same rule that makes the tool orb stop when its
       * tool closes.
       */
      case 'message.delta': {
        if (runningStep === undefined) break
        /*
         * FROM `working`, AND FROM A THOUGHT LEFT OPEN (0.586). A tool or a
         * connector is a claim this does not know better than and keeps its
         * line. A reasoning step the runtime never closed is different: text
         * arriving IS the model writing, and the thought is over -- Cursor
         * leaves its thinking step open while the reply streams, and the
         * Cursor leg of the cross-model pass read the thought where Claude,
         * Antigravity and OpenCode read Writing. The word follows the
         * register, and the step's own words are held and come back when the
         * text is final; a finished thought comes back as plain working.
         */
        if (runningStep.register !== 'working' && runningStep.register !== 'writing' && runningStep.register !== 'thinking') break
        const final = event.payload.final === true
        if (!final) {
          runningStep = {
            ...runningStep,
            register: 'writing',
            label: 'Writing',
            labelBeforeWriting: runningStep.labelBeforeWriting ?? (runningStep.register === 'thinking' ? 'Working' : runningStep.label)
          }
        } else {
          runningStep = {
            ...runningStep,
            register: 'working',
            // A thought that ends on a final message (no streaming before it) is over too.
            label: runningStep.labelBeforeWriting ?? (runningStep.register === 'thinking' ? 'Working' : runningStep.label),
            labelBeforeWriting: undefined
          }
        }
        break
      }
      case 'plan.updated': {
        workBegan = true
        plan = readPlan(event.payload.plan)
        // A step seen unfinished in this turn was worked on in this turn,
        // however it ends.
        for (const step of plan) if (step.state !== 'done') workedSteps.add(planKey(step.text))
        break
      }
      case 'route.limit_detected': {
        // A temporary warning that became the final failure is said by that error.
        if (event.payload.kind === 'temporary-rate-limit' && endedOn !== undefined && sameSentence(event.payload.message, endedOn.payload.message)) break
        /*
         * One usage warning a run: the newest, where the first one stood.
         *
         * Claude Code restates the window each time its figure moves, so an
         * answer wore "You've used 55% of your 7-day window" and then "...56%"
         * under it -- four amber lines over two answers in a room (drive,
         * 2026-09-23). Updated in place, so the line does not jump.
         */
        // Once a conversation: an earlier turn already gave the reading.
        if (
          event.payload.kind === 'temporary-rate-limit'
          && isUsageWarning(event.payload.message)
          && options.saidBefore?.has(USAGE_WARNING_KEY) === true
        ) {
          break
        }
        if (event.payload.kind === 'temporary-rate-limit') {
          const earlier = items.findIndex((held) => held.type === 'limit' && held.kind === 'temporary-rate-limit')
          const held = items[earlier]
          if (held?.type === 'limit') {
            items[earlier] = { ...held, message: event.payload.message }
            break
          }
        }
        items.push({
          key: event.id,
          type: 'limit',
          kind: event.payload.kind,
          message: event.payload.message
        })
        break
      }
      case 'adapter.diagnostic': {
        // About the CLI's own setup: its row in Settings, not the conversation.
        if (isSetupNote(event.payload.message)) break
        // Before any tool runs, only trouble with the RUN ITSELF gets through.
        //
        // The gate exists because Codex comments on its own setup the moment a
        // turn opens ("Skill descriptions were shortened...") and that belongs
        // nowhere near the top of a thread. But it was swallowing something
        // very different: MEASURED 2026-09-03, against a dead endpoint Codex
        // retries five times across five to eight minutes and reports each
        // attempt as `Reconnecting... 2/5`. Those arrive before the first tool
        // too, so every one was dropped and the mission sat reading "running"
        // with nothing on screen at all. The run was working; the thread
        // refused to say so.
        //
        // The adapters already separate these: a `*.runtime_error` is the run
        // in trouble, an item diagnostic is the provider talking about one
        // item. Only the former is worth interrupting an empty thread for.
        // A usage window, how long the start took: kept for the record, not lines in the thread.
        if (keptForTheRecord(event.payload.code)) break
        // What the host saved on the teammate's branch (0.439) is said when the
        // turn is over: after its work and its answer, never above them. The
        // first packaged drive drew "Saved this turn" under the ask, before
        // the work it saved.
        // And why a turn whose plan ran out was not carried on (0.774): after its error, where the offer would be.
        // And why a failing check was not sent back again (0.812): after the turn it is about.
        if (event.payload.code === 'host.turn_checkpoint' || event.payload.code === 'host.carry_on' || event.payload.code === 'host.check_fix') {
          receipts.push({ key: event.id, type: 'diagnostic', level: event.payload.level, message: event.payload.message })
          break
        }
        // A record the adapter does not know yet is this app's gap, not the
        // run's news: a runtime updates and adds record types before Locust
        // has learned them, and Claude Code 2.1.280's `tool_progress` heartbeat
        // reached the middle of a conversation as "Unhandled Claude record:
        // tool_progress" (Colin, 2026-09-22: "?"). It goes to the fold's foot
        // with the turn's other remarks -- still readable, out of the thread.
        // Copilot's word that its background tasks changed carries nothing a
        // person can read (0.571: eight in a row above one answer); it is kept
        // for the ledger, and said at the foot with the run's other remarks.
        const unknownRecord = /\.(unknown_event|background_tasks_changed)$/.test(event.payload.code)
        // SAID ONCE A TURN (0.571): the same sentence again adds nothing.
        const sentence = noticeKey(event.payload.message)
        if (foldNotices.some((notice) => noticeKey(notice.message) === sentence) || items.some((held) => held.type === 'diagnostic' && noticeKey(held.message) === sentence)) break
        // A compaction is let through too: after a `/compact` the person sent
        // (0.426) it is the whole of the turn's answer, and in the fold it
        // left the thread saying the turn "ended without a reply".
        if (unknownRecord || (!workBegan && !/\.(runtime_error|notification|context_compacted)$/.test(event.payload.code))) {
          /*
           * Not shown in the thread -- and, until now, not shown anywhere.
           *
           * The trace line counted these as `1 notice` while this gate kept
           * the sentence off the screen, so the person read a number for a
           * thing they could never open. The design's rule is that a chip
           * counting an unnamed thing becomes a sentence that names it, and
           * these belong to the turn's WORK, so the fold's foot is where they
           * go.
           *
           * Collected HERE, in the same branch that drops them, rather than
           * re-derived by a second function -- two filters over one set is
           * how the count and the sentence disagreed in the first place.
           */
          // With its source: the fold names who said it, so a runtime's
          // remark about itself is never read as a command's output.
          if (options.saidBefore?.has(noticeKey(event.payload.message)) === true) break
          foldNotices.push({ level: event.payload.level, message: event.payload.message, source: event.sourceAdapter })
          break
        }
        // Said once. A quota failure arrives as a limit event AND as the
        // runtime's error line carrying the same sentence; user session 1
        // (2026-09-05) showed "You've hit your usage limit..." three times in
        // a row for one failure. The limit card is the one that names it.
        if (items.some((held) => held.type === 'limit' && sameSentence(held.message, event.payload.message))) break
        items.push({
          key: event.id,
          type: 'diagnostic',
          level: event.payload.level,
          message: event.payload.message,
          ...(/\.provider_busy\.runtime_error$/.test(event.payload.code) ? { busy: true } : {}),
          ...(RETRYING_NOTE.test(event.payload.code) ? { retrying: true } : {})
        })
        break
      }
      default:
        break
    }
  }

  // The plan goes INSIDE the fold, as its first rows.
  //
  // A plan is the clearest possible statement of what the run DID -- it is
  // the fold's own content, not a separate object above it. Drawn as its own
  // card it put a second species on the screen for almost every Codex run,
  // saying the same kind of thing the fold below it says (design review,
  // 2026-09-06).
  //
  // Only this turn's steps: see `carriedPlan`.
  const ownPlan = thisTurnsSteps(plan, options.carriedPlan ?? [], workedSteps)
  const planSteps = ownPlan.length > 0
    ? { steps: ownPlan, doneCount: ownPlan.filter((step) => step.state === 'done').length }
    : undefined

  /*
   * THE PLAN IS NOT PROCESS, so it does not live behind the fold.
   *
   * It used to ride inside the activity card whenever there was one, and
   * only stand on its own when a turn had touched nothing -- so on exactly
   * the turns worth watching, the plan was the one thing you had to go
   * looking for. Colin, 2026-09-14: "the plan should be visible to the user
   * probably".
   *
   * The fold exists to hide HOW: tool calls, shell lines, the thousand
   * small steps. A plan is WHAT, and how far along -- which is the question
   * a person waiting actually has, and the only part of a running turn that
   * answers it. The comment on the trace already says this in so many
   * words: "the plan is the shape of the work, and the rest is how it was
   * carried out". It just was not where the code put it.
   *
   * Before the fold, so a turn reads plan first and then its workings.
   */
  if (planSteps !== undefined) {
    items.push({
      key: 'plan',
      type: 'plan',
      steps: planSteps.steps,
      doneCount: planSteps.doneCount,
      ...(options.running ? {} : { finished: true }),
      ...(['failed', 'cancelled'].includes(traceOutcome(events, options.running)) ? { stopped: true } : {}),
      // A turn that ran nothing planned and stopped. Anything else has
      // changed something, whatever its plan says.
      ...(activity.length === 0 ? { touchedNothing: true } : {})
    })
  }

  // What each message reads as on screen: every block the thread draws on
  // its own taken out, as the reply below the fold does.
  const shownText = (text: string): string =>
    unwrapProtocolTags(stripFileBlocks(stripMemoryBlocks(stripTaskBlocks(stripDecisionBlocks(stripShareBlocks(text))))))
  const messages = assistantMessages(events)
  /*
   * A MESSAGE SOMETHING CAME AFTER IS NOT STILL BEING WRITTEN (0.570).
   *
   * Colin, 2026-10-03, on an Antigravity (Claude Sonnet 5.5) run: "antigravity
   * has the _ typing animation after finished messages". Its ledger: each
   * message arrives as one delta that is never marked final, so the caret
   * stayed on "The matrix run is in progress..." through the steps after it.
   * The provider's `final` is one signal; this is the other, and it holds for
   * every runtime: a call started, or another message began, after this one.
   * A message whose own text comes again afterwards is live again.
   */
  const moved = new Set<string>()
  {
    let open: string | undefined
    for (const event of events) {
      if (event.type === 'message.delta') {
        const id = event.payload.itemId
        if (open !== undefined && open !== id) moved.add(open)
        moved.delete(id)
        open = id
      } else if (event.type === 'tool.started' && open !== undefined && !byHelper(event.payload)) {
        moved.add(open)
        open = undefined
      }
    }
  }

  /*
   * A TURN IS DRAWN IN THE ORDER IT HAPPENED (0.491): what the teammate said,
   * then the steps it took before it next said anything, as one line, then
   * what it said next -- Claude Code's turn. Colin, 2026-09-30: "compared to
   * claude code, ALL of our commands and stuff that would appear batched on
   * screen seem to all get rolled into the bar".
   *
   * It was one fold above everything said: closed while the turn ran, so a
   * forty-step run read as one bar of counts with its narration stacked
   * under it, cut off from the steps it came between; then the narration
   * moved into the fold as grey rows when the turn ended. Now nothing moves.
   * A message stands where it BEGAN, which is also where Claude Code's
   * narration belongs: its full record restates a message after the tool
   * call that followed it, and placing by the last word put narration said
   * before the last step under it (DISPLAY-COVERAGE gap 9).
   *
   * Rows the host adds after the run has ended (its look at the disk) are no
   * step the runtime took: they are in the turn's files at its foot.
   */
  const terminalAt = events.findIndex(
    (event) => event.type === 'run.completed' || event.type === 'run.failed' || event.type === 'run.cancelled'
  )
  const endAt = terminalAt === -1 ? events.length : terminalAt
  const beganAt = new Map<string, number>()
  events.forEach((event, index) => {
    if (event.type === 'message.delta' && !beganAt.has(event.payload.itemId)) beganAt.set(event.payload.itemId, index)
  })
  const thoughtsWithWords = new Set(events.flatMap((event) =>
    (event.type === 'step.started' || event.type === 'step.completed') && typeof event.payload.itemId === 'string' && typeof event.payload.message === 'string' && event.payload.message.trim().length > 0
      ? [event.payload.itemId]
      : []))
  type Piece =
    | { readonly at: number; readonly row: number }
    | { readonly at: number; readonly message: (typeof messages)[number] }
    | { readonly at: number; readonly thinks: true }
  const pieces: Piece[] = [
    ...activity.flatMap((_, row): Piece[] => {
      const at = activityBorn[row] ?? endAt
      return at < endAt ? [{ at, row }] : []
    }),
    ...messages.map((message): Piece => ({ at: beganAt.get(message.itemId) ?? endAt, message })),
    /*
     * EACH TIME THE MODEL THINKS, A NEW LINE (0.492). A model that works a
     * long stretch without a word -- an Antigravity run of 235 steps and one
     * message -- was one line with everything behind it. Codex's and
     * Antigravity's own apps start a new block each time the model plans its
     * next move, and every runtime but Claude without thinking reports that
     * moment, with words or without: the stretch reads as the moves it was.
     *
     * ONLY A THOUGHT WITH WORDS (0.571). Antigravity thinks before every call
     * and Locust keeps none of its words, so one turn of Bro's (Colin,
     * 2026-10-03) drew 29 lines, each "Thought for 4s, listed a folder". A
     * thought that says nothing is no new move to read; it folds into the
     * line it sits in, as Claude Code folds its steps, and still shows there.
     */
    ...events.flatMap((event, at): Piece[] =>
      at < endAt && event.type === 'step.started' && (event.payload.stepKind === 'reasoning' || /reasoning/i.test(event.payload.itemType ?? ''))
        && (event.payload.itemId === undefined || thoughtsWithWords.has(event.payload.itemId))
        ? [{ at, thinks: true }]
        : [])
  ].sort((a, b) => a.at - b.at)
  let group: number[] = []
  const flush = (): void => {
    if (group.length === 0) return
    const rows = group.map((row) => activity[row]!)
    // A step still going is the live line's, until it ends.
    const shown = options.running ? rows.filter((detail) => detail.settled) : rows
    // Keyed by where the group began, so it keeps its place and its state as it grows.
    if (shown.length > 0) items.push({ key: `steps_${String(activityBorn[group[0]!] ?? 0)}`, type: 'steps', details: shown, finished: !options.running })
    group = []
  }
  for (const piece of pieces) {
    if ('row' in piece) {
      group.push(piece.row)
      continue
    }
    if ('thinks' in piece) {
      flush()
      continue
    }
    const message = piece.message
    // A share block is shown in the peer card, attributed and labelled; left
    // in the bubble it would present the same claim twice, once unlabelled.
    //
    // `unwrapProtocolTags` LAST, and it is the backstop rather than a sixth
    // stripper: each of the five above deletes the blocks its own parser
    // acted on, and this takes the tags off whatever is still wearing them —
    // a block too malformed to have been acted on, or a turn cut off
    // mid-block. It keeps the body, because for those the body never reached
    // anywhere else.
    const text = shownText(message.text)
    /*
     * The files the teammate handed over, drawn UNDER the message it came
     * with rather than folded into the work.
     *
     * Parsed from the raw message, not from `text`, because `text` has just
     * had the block cut out of it. Nothing is recorded and nothing runs: the
     * file is already on disk and the card is a pointer to it.
     *
     * A reply may be nothing but a block -- "here you go" is often said in
     * the note -- so the files item is pushed even when the text is empty.
     * Not while the message is still arriving: half a block is not a file,
     * and a button appearing and vanishing mid-stream is worse than a late one.
     */
    const handed = parseFileBlocks(message.text)
    const handedNow = handed.length > 0 && (message.final || !options.running)
    // A file it handed over that is not drawn says so, rather than nothing (0.512).
    const refused = message.final || !options.running ? refusedFileLines(message.text) : []
    // Nothing on screen: it does not split the steps around it.
    if (text.length === 0 && !handedNow && refused.length === 0) continue
    flush()
    if (text.length > 0) {
      items.push({
        key: `msg_${message.itemId}`,
        type: 'agent-message',
        text,
        // A caret only where text is genuinely still arriving: the run is live,
        // the provider has not marked this message final, and nothing has come
        // after it (Antigravity never marks one final; see `moved`).
        streaming: options.running && !message.final && !moved.has(message.itemId)
      })
    }
    if (handedNow) items.push({ key: `files_${message.itemId}`, type: 'files', files: handed })
    if (refused.length > 0) {
      items.push({
        key: `files_refused_${message.itemId}`,
        type: 'diagnostic',
        level: 'warning',
        // Since 0.787 only a network or device path (or one that is no path) is refused here: a `..` or whole path goes to the host.
        message: `Not shown as a file: ${refused.map((file) => file.path).join(', ')} -- a file is handed over by its path on this computer, and ${refused.length === 1 ? 'this is not one' : 'these are not'}.`
      })
    }
  }
  flush()

  /*
   * THE TURN'S FOOT: what it came to, once it has ended -- its files, and the
   * line of totals that used to head the fold (duration, commands, what went
   * wrong). The same rows as every group above, all of them, which is what
   * the review, the viewer's history and the artifacts list read.
   */
  if (activity.length > 0) {
    items.push({
      key: 'activity',
      type: 'activity',
      summary: activitySummary(activity),
      trace: activityTrace(
        activity,
        events,
        traceOutcome(events, options.running),
        planSteps,
        options.mayEdit,
        options.workspacePath,
        options.trimmed
      ),
      finished: !options.running,
      details: activity,
      ...(foldNotices.length === 0 ? {} : { notices: foldNotices }),
      reportedBy: events.find((event) => event.type.startsWith('tool.'))?.sourceAdapter,
      // The turn's own run: the last event's, since a continuation is drawn with the run before it stitched in front.
      ...(events.at(-1)?.runId === undefined ? {} : { runId: events.at(-1)!.runId })
    })
  }

  /*
   * A TURN WITH NO WORK STILL SAYS WHAT THE RUNTIME SAID (0.586). A notice
   * that arrives before any work began goes under the fold (`foldNotices`,
   * above), and a turn with no activity has no fold -- so Copilot's "Third-
   * party MCP servers are disabled by your organization's Copilot policy"
   * was never drawn: the golden text read "(nothing in the thread)" for it.
   * Found in the cross-runtime output pass Colin asked for (2026-10-04).
   */
  if (!options.running && activity.length === 0) {
    // Once the turn has ended: while it runs the live line stands for it, and a
    // line that came and went with the reply would read as a flicker.
    foldNotices.forEach((notice, index) => {
      if (items.some((held) => held.type === 'diagnostic' && noticeKey(held.message) === noticeKey(notice.message))) return
      items.push({ key: `notice_${String(index)}`, type: 'diagnostic', level: notice.level, message: notice.message })
    })
  }
  /*
   * A RUN THAT FAILED BEFORE IT SAID ANYTHING NAMES WHY (0.586). `run.failed`
   * carries the reason -- Antigravity's "could not run it: model no-such-model
   * is not recognized" -- and it reached the rail and the foot only; with no
   * activity the thread was blank beside a sidebar saying "failed". Said once:
   * a diagnostic or a limit card that already carries the sentence is enough.
   */
  const failure = options.running ? undefined : events.find((event) => event.type === 'run.failed')
  const why = failure === undefined ? undefined : failure.payload.message
  if (failure !== undefined && typeof why === 'string' && why.trim().length > 0 && !items.some((held) => (held.type === 'diagnostic' || held.type === 'limit') && sameSentence(held.message, why))) {
    items.push({ key: `failed_${failure.id}`, type: 'diagnostic', level: 'error', message: why })
  }

  if (options.running) {
    // The group still growing at the end of the turn is live (0.584): open, its
    // rows arriving. Text after it means the model is talking, and no group is.
    // The turn's foot (above) sits after every group, so it is looked past.
    let lastSteps = -1
    for (let i = items.length - 1; i >= 0; i -= 1) {
      if (items[i]!.type === 'steps') {
        lastSteps = i
        break
      }
    }
    const liveAt = lastSteps >= 0 && !items.slice(lastSteps + 1).some((item) => item.type === 'agent-message') ? lastSteps : -1
    if (liveAt >= 0) {
      const group = items[liveAt]!
      if (group.type === 'steps') items[liveAt] = { ...group, live: true }
    }
    // Every group BEFORE the last one of a running turn is superseded (0.594):
    // the run has moved on to more steps, so it folds to its line -- Claude
    // Code's shape, one step open. Colin, 2026-10-04, of a Grok run on Cursor:
    // every group that had ever been live stayed open and the thread read as a
    // wall. The last group is not: the model talking after it is the reply,
    // and the 9/8 ask (the work stays in view when the run ends) holds for it.
    items.forEach((item, at) => {
      if (item.type === 'steps' && at !== lastSteps) items[at] = { ...item, superseded: true }
    })
    const streaming = items.some((item) => item.type === 'agent-message' && item.streaming)
    /*
     * WHICH STEP OF ITS PLAN, on the live line (0.493). Colin, 2026-09-30,
     * on a 58-minute Codex run: the plan "didn't initiate ... and just showed
     * it as completed after". Codex had sent it before its first command; the
     * card sat at the top of the turn, scrolled off, and nothing at the bottom
     * said where the run was in it. Claude Code keeps the current to-do in
     * view; this says it where the eye is.
     */
    const underway = planSteps?.steps.findIndex((step) => step.state === 'running') ?? -1
    const planAside = planSteps === undefined || underway < 0
      ? undefined
      : `step ${String(underway + 1)} of ${String(planSteps.steps.length)}`
    // The plan's step under way, when no call is open to say more (OpenCode reports a call once it ends).
    // Without its full stop (0.610): "Inspect the screenshot. · step 1 of 3" read as two sentences.
    const planAction = planSteps === undefined || underway < 0 ? undefined : planSteps.steps[underway]?.text.replace(/\s+/g, ' ').trim().replace(/(?<!\.)\.$/, '')
    /*
     * A TOOL STILL OPEN outranks everything, because it is the most specific
     * true thing about the run: more specific than the turn around it, and
     * the one register a person most wants told apart from the reply.
     *
     * Newest first -- a tool that opened inside another is what is happening
     * now. A connector is named as one: the row already splits server from
     * tool, and "using a connector - get_watchlists - Robinhood" is what a
     * person would say about it.
     */
    /*
     * ONE CLOCK FOR THE TURN, and the phase is a label on it.
     *
     * Two captures of one running turn, seconds apart, in the frame pass of
     * 2026-09-15:
     *
     *     starting ... - 3s
     *     working  -    0s
     *
     * The counter restarted when the register changed, because each branch
     * below timed from its own beginning. That is the one thing an elapsed
     * counter must never do, and it is worse here than in most apps: this
     * number is how a person tells a slow runtime from a hung one.
     *
     * The bottom branch already ran from the turn's start and said why --
     * "a number that only ever climbs cannot be mistaken for one" -- and the
     * other two did not. Now all three share this.
     *
     * A tool's own duration is not lost; the trace states it per call, where
     * it is a fact about that call rather than a clock a person is watching.
     */
    const turnStartedAt = options.startedAt ?? events[0]?.occurredAt ?? events.at(-1)?.occurredAt
    // When the runtime last said anything at all: the quiet line's clock (quiet.ts, 0.758).
    const lastHeardAt = events.at(-1)?.occurredAt
    const openToolId = [...openTools.keys()].at(-1)
    const openTool = openToolId === undefined ? undefined : openTools.get(openToolId)
    const openToolMeta = openToolId === undefined ? undefined : openToolAt.get(openToolId)
    if (options.awaitingDecision === true) {
      // Stopped on the person: no live line at all, whatever is open. The card
      // says what the run waits for, and the header already reads "waiting on
      // you". The rule was only kept when no step had been reported, so a run
      // stopped with a tool OPEN still drew "Using a tool..." straight above
      // the card asking the question -- Antigravity's `ask_question`,
      // 2026-09-23, which is exactly a tool left open until the answer comes.
    } else if (openTool !== undefined && openToolMeta !== undefined) {
      items.push({
        key: 'live-step',
        type: 'live-step',
        // What the model said the call is for, where it said: "Run the test
        // suite" says more over eight minutes than `npm test`, and far more
        // than "Bash". The command is on its row below.
        label: openTool.title ?? openTool.name,
        ...(openToolMeta.viaConnector ? {} : { action: liveActionLine(openTool, options.workspacePath) ?? planAction }),
        detail: openToolMeta.connector ?? planAside,
        startedAt: turnStartedAt ?? openToolMeta.at,
        kind: 'item',
        orb: orbStateFor(openTool, options.planMode === true, openToolMeta.viaConnector ? 'connector' : 'tool'),
        register: openToolMeta.viaConnector ? 'connector' : 'tool'
      })
    } else if (runningStep !== undefined) {
      items.push({
        key: 'live-step',
        type: 'live-step',
        label: runningStep.label,
        ...(runningStep.kind === 'reasoning' || planAction === undefined ? {} : { action: planAction }),
        detail: runningStep.detail ?? planAside,
        startedAt: turnStartedAt ?? runningStep.startedAt,
        kind: runningStep.kind,
        register: runningStep.register,
        // No tool is open, so only the turn's own mode can answer. A Plan-mode
        // run is planning; anything else keeps its dots rather than borrow an
        // orb that would be describing work nobody reported.
        orb: orbStateFor(undefined, options.planMode === true, runningStep.register),
        ...(runningStep.kind === 'reasoning' ? { waiting: true } : {}),
        ...(lastHeardAt === undefined ? {} : { lastHeardAt })
      })
    } else if (streaming) {
      /*
       * WRITING, while the reply arrives (0.581).
       *
       * This drew nothing: a "Working" line under arriving text would say the
       * opposite of what the reader sees, so there was no line at all -- and
       * the turn's clock and its Stop place went with it for as long as the
       * reply took. Claude Code keeps its status line through a reply
       * ("Writing… 12s"). This says what is visible -- writing, no waiting
       * dots -- on the same clock the other branches use.
       */
      if (turnStartedAt !== undefined) {
        items.push({
          key: 'live-step',
          type: 'live-step',
          label: 'Writing',
          register: 'writing' as const,
          ...(planAction === undefined ? {} : { action: planAction }),
          detail: planAside,
          startedAt: turnStartedAt,
          kind: 'turn',
          orb: orbStateFor(undefined, options.planMode === true, 'writing'),
          ...(lastHeardAt === undefined ? {} : { lastHeardAt })
        })
      }
    } else {
      // Nothing has begun, or the last step closed and the next has not
      // opened. The thread used to draw NOTHING here, so pressing Enter left
      // an empty page until the runtime's first event -- seconds, for a CLI
      // that has to launch a process. Colin reported it twice as the working
      // bounce lagging the send (2026-09-03, 2026-09-04); the bounce was not
      // late, there was no line for it to be on.
      //
      // A live run is always doing something, so a line always shows. What it
      // SAYS stays honest: no step has been reported, so it names the wait
      // rather than inventing a step.
      //
      // The clock runs from the START OF THE TURN, not from the last event.
      // It used to run from the last event, on the reasoning that "waiting 3s"
      // is more useful than "running 4m" -- but a runtime that reports
      // something every few seconds then resets the clock every few seconds,
      // and Colin watched one count to 10 and start over, repeatedly, which
      // reads as a stuck loop rather than a run making progress. A number that
      // only ever climbs cannot be mistaken for one.
      const since = turnStartedAt
      if (since !== undefined) {
        /*
         * WHETHER THE RUNTIME HAS SPOKEN, not whether the mission has events.
         *
         * `run.started` is the APP's own record of having launched a process.
         * It is the first event in almost every mission and it says nothing
         * whatsoever about the runtime, so `events.length === 0` -- which is
         * what the label and the register were read off -- means "we have not
         * finished spawning it yet" rather than "nothing has come back".
         *
         * One event later the line said **Working**, on the strength of the
         * app having spawned something. The ledger in `quiet.ts` is the same
         * run this is about: `mission_3848a498`, `run.started` at 3.5s and
         * then not one runtime event until 128.7s. The line read "working"
         * for over two minutes of silence, and that is the frame Colin sent.
         *
         * `spoken` -- the honest fact -- was added underneath to fix the
         * SENTENCE while the word above it kept being drawn from the proxy.
         * Both come from the same fact now.
         */
        // A runtime's remark about itself is not the run working: a provider
        // answering "Rate limit exceeded" read as "Working..." under the line
        // saying so, while OpenCode waited out the provider (drive-busy-model,
        // packaged 0.368).
        const spoken = events.some((event) => event.type !== 'run.started' && event.type !== 'adapter.diagnostic')
        items.push({
          key: 'live-step',
          type: 'live-step',
          label: spoken ? 'Working' : (options.startingLabel ?? 'Starting'),
          register: spoken ? ('working' as const) : ('starting' as const),
          ...(spoken && planAction !== undefined ? { action: planAction } : {}),
          detail: planAside,
          startedAt: since,
          kind: 'turn',
          waiting: true,
          // The same answer as the branch above, for the same reason: no tool
          // is open, so only the turn's own mode can say what is happening,
          // and in Plan mode what is happening is planning. Every other turn
          // keeps its dots, which already mean "waiting with nothing to show".
          orb: orbStateFor(undefined, options.planMode === true, spoken ? 'working' : 'starting'),
          // Past twenty seconds with nothing from the runtime the line says so
          // as well; see `quiet.ts` for the measured run above.
          spoken,
          // And once it has spoken, five quiet minutes say so too (quiet.ts, 0.758).
          ...(spoken && lastHeardAt !== undefined ? { lastHeardAt } : {})
        })
      }
    }
    // The plan rides on the live line that says "step N of M", for the card that step opens.
    if (planSteps !== undefined && planAside !== undefined) {
      const at = items.findIndex((item) => item.type === 'live-step' && item.detail === planAside)
      const found = at < 0 ? undefined : items[at]
      if (found !== undefined && found.type === 'live-step') items[at] = { ...found, plan: planSteps.steps }
    }
  }

  // A run that ended by asking. Read from the LAST message the provider marked
  // final, because that is where a runtime puts the question it stopped on --
  // and only once the run is over, since a block still streaming may not have
  // its closing tag yet and would parse as nothing or as half a question.
  if (!options.running && options.latestTurn === true) {
    const answer = assistantMessages(events).filter((message) => message.final).at(-1)
    const request = answer === undefined ? undefined : parseDecision(answer.text)
    if (request !== undefined) {
      items.push({ key: `decision_${answer!.itemId}`, type: 'decision', request })
    }
  }

  // A turn that finished and said nothing.
  //
  // MEASURED 2026-09-05, Colin watching a live test: a follow-up on Cursor
  // completed cleanly -- exit 0, tokens spent, reasoning recorded -- and
  // emitted no assistant text at all. The thread drew the person's message,
  // then blank space, and the header said `completed`. That reads as the app
  // losing the answer, or the teammate ignoring you. Neither is what
  // happened, and the record can say which: the runtime thought and then
  // ended the turn without writing a reply.
  //
  // Only for a finished run, and only when there is genuinely nothing to
  // read -- a turn that did work says so through its activity card, and a
  // failure has its own card already.
  if (!options.running && events.some((event) => event.type === 'run.completed')) {
    // M25: a plan, a file handed over or a question asked is a reply too,
    // and so is any text the runtime wrote -- a reply that was only a block
    // is stripped to nothing before it is drawn, and on an earlier turn the
    // question itself is no longer shown. Not a share block, though: one
    // that reached nobody is still a turn that told nobody anything.
    const saidSomething =
      options.spokeToPeers === true
      || items.some((item) => item.type === 'agent-message' || item.type === 'activity' || item.type === 'steps' || item.type === 'plan' || item.type === 'files' || item.type === 'decision')
      || events.some((event) => event.type === 'message.delta' && event.payload.text.replace(/<locust-share[^>]*>[^]*?<\/locust-share>/g, '').trim().length > 0)
      // A summarized conversation is what `/compact` answers with (0.426).
      || events.some((event) => event.type === 'adapter.diagnostic' && /\.context_compacted$/.test(event.payload.code))
    if (!saidSomething) {
      items.push({
        key: 'silent_turn',
        type: 'diagnostic',
        level: 'warning',
        message:
          'This turn ended without a reply: the runtime finished and wrote nothing back. Nothing was changed. Sending it again usually works.'
      })
    }
  }

  items.push(...receipts)
  /*
   * "TRYING AGAIN" ONLY WHILE IT IS TRUE (QA-2026-09-29 round 2, R17). The
   * busy provider's note -- "OpenCode is trying again on its own. To go on
   * now, press Stop and pick another model." -- stayed above the red card of
   * a run that had ended, and above the answer of a run whose model came
   * back on the third try: advice about a run with nothing left to stop.
   */
  const busyAt = events.findIndex((event) => event.type === 'adapter.diagnostic' && RETRYING_NOTE.test(event.payload.code))
  const recovered = busyAt >= 0 && events.slice(busyAt + 1).some((event) => (event.type === 'message.delta' && event.payload.text.trim().length > 0) || event.type === 'tool.started')
  if (busyAt >= 0 && recovered) {
    return items.filter((item) => !(item.type === 'diagnostic' && item.retrying === true))
  }
  // Ended without the model coming back: what the provider said is still the
  // reason, so it stays, without the claim that a retry is under way (0.551,
  // Boss on 0.550: dropping the whole note lost "Endpoint is unavailable").
  // Past the window, the turn says its start is kept but not drawn (0.627): it must not read as if it began there.
  if (options.trimmed === true) items.unshift({ key: 'trimmed', type: 'diagnostic', level: 'info', message: TRIMMED_TURN_LINE })
  if (busyAt >= 0 && options.running !== true) {
    // The red card already quoting the provider's words says them once, there
    // (0.705; sweep 7 drew the note's "Endpoint is unavailable" right above the
    // card's "OpenCode stopped: ... Endpoint is unavailable."). A card without them keeps the note.
    let failure = ''
    for (const event of events) if (event.type === 'run.failed') failure = event.payload.message
    return items.flatMap((item) => {
      if (item.type !== 'diagnostic' || item.retrying !== true) return [item]
      const said = /answered "(.+)"/.exec(item.message)?.[1]?.replace(/\.$/, '').trim()
      if (said !== undefined && said.length > 0 && failure.includes(said)) return []
      const { busy: _busy, retrying: _retrying, ...rest } = item
      return [{ ...rest, message: item.message.replace(/,? and OpenCode is trying again on its own\.(?: To go on now, press Stop and pick another model\.)?$/, '.') }]
    })
  }
  return items
}

/**
 * The notes that say a runtime is trying again on its own. OpenCode's other
 * provider error says it too (0.550, Sol on 0.546: "Endpoint is unavailable"
 * kept "OpenCode is trying again on its own" over a failed Compare column).
 */
const RETRYING_NOTE = /^opencode\.runtime_error$|\.provider_busy\.runtime_error$/

/**
 * The cancellation summary, derived from events rather than from file
 * bookkeeping the host does not have. The design's card offers KEPT / STOPPED /
 * NOT DONE; only the middle two are knowable here, so "kept" is deliberately
 * described as what the provider reported finishing, and nothing claims a
 * revert is possible.
 */
export interface CancellationSummary {
  readonly settled: readonly string[]
  readonly interrupted: readonly string[]
  readonly neverStarted: number
  /** Plan states are counted separately from tool calls and observed files. */
  readonly plan?: { readonly finished: number; readonly cutOff: number; readonly total: number }
  /**
   * The runtime reports a tool only once it has finished (OpenCode), so a
   * command it had started when the run stopped is not in either list (Sol,
   * 0.532: a running timer, and the card said no tool had run).
   */
  readonly toolsReportedWhenDone?: boolean
}

export function cancellationSummary(
  events: readonly NormalizedRuntimeEvent[],
  plannedSteps = 0,
  /** The folder this ran in, so the list reads the way a person writes paths. */
  workspacePath?: string
): CancellationSummary {
  // Through `relativePath`, and deduplicated.
  //
  // MEASURED 2026-09-07 by `_tools/drive-stopped-midedit.mjs`: a stopped run
  // listed FIVE things finished, of which one was the workspace DIRECTORY and
  // two were the same file -- an absolute path and a bare `note-1.txt` -- because
  // one tool named it absolutely and another relatively. The count of what a
  // stopped run finished is the one number on that card a person might act on,
  // and it was inflated by the same file twice.
  //
  // Same fix as the activity fold's, for the same reason: a path is not an
  // identity until the workspace root has had a say.
  const shown = (name: string): string => relativePath(name, workspacePath)
  const key = (name: string): string => shown(name).replace(/[\\/]+/g, '/').toLowerCase()
  // Sonnet's 0.569 review (#7): FINISHED listed the workspace folder. Check the raw path
  // before relativePath turns the root into its basename.
  const normal = (path: string): string => path.replace(/[\\/]+/g, '/').replace(/\/$/, '').toLowerCase()
  const root = workspacePath === undefined ? undefined : normal(workspacePath)
  const isRoot = (name: string): boolean => root !== undefined &&
    [root, root.split('/').at(-1), '.', ''].includes(normal(name))
  const settled: string[] = []
  const seen = new Set<string>()
  const open = new Map<string, string>()
  for (const event of events) {
    // What the TEAMMATE had in hand; a helper's calls are under its row.
    if ((event.type === 'tool.started' || event.type === 'tool.completed' || event.type === 'tool.failed') && byHelper(event.payload)) continue
    if (event.type === 'tool.started') {
      open.set(event.payload.itemId, event.payload.command ?? event.payload.name)
    } else if (event.type === 'tool.completed' || event.type === 'tool.failed') {
      const name = open.get(event.payload.itemId)
      if (name !== undefined) {
        if (!isRoot(name) && !seen.has(key(name))) {
          seen.add(key(name))
          settled.push(shown(name))
        }
        open.delete(event.payload.itemId)
      }
    }
  }
  const interrupted = [...open.values()].map(shown).filter((name) => {
    if (isRoot(name)) return false
    if (seen.has(key(name))) return false
    seen.add(key(name))
    return true
  })
  // Sonnet's 0.569 review (#8): the counts must add up (finished + cut off +
  // never started = the plan's steps). Reading a directory or observing a changed file cannot
  // advance the model's plan. Only its latest reported plan can do that.
  const latestPlan = events.filter((event) => event.type === 'plan.updated').at(-1)
  const steps = latestPlan?.type === 'plan.updated' ? readPlan(latestPlan.payload.plan) : undefined
  const total = steps?.length ?? plannedSteps
  const finished = steps?.filter((step) => step.state === 'done').length ?? 0
  const cutOff = steps?.filter((step) => step.state === 'running').length ?? 0
  return {
    settled,
    interrupted,
    neverStarted: Math.max(0, total - finished - cutOff),
    ...(total > 0 ? { plan: { finished, cutOff, total } } : {}),
    ...(events.some((event) => event.sourceAdapter === 'opencode') ? { toolsReportedWhenDone: true } : {})
  }
}

/** Colour class for a Signal Rail row, by what the event means. */
export type SignalTone = 'live' | 'blue' | 'violet' | 'amber' | 'red' | 'muted'

export interface SignalRow {
  readonly key: string
  readonly name: string
  readonly meta: string
  readonly tone: SignalTone
  readonly live: boolean
}

/**
 * A rail row names what happened; it is not the place for the payload. A real
 * shell invocation can be hundreds of characters (a full PowerShell line, an
 * absolute interpreter path), and pasting it whole turns one row into four and
 * pushes everything else off screen.
 */
/**
 * THE RAIL IN WORDS.
 *
 * Each row was the event's own name -- `runtime.started · claude`,
 * `tool.completed · Bash`, `step.started · reasoning`, `run.completed` --
 * which is the ledger's vocabulary, not a person's (the design review, #10:
 * "Inspector labels in words, not step.started"). The rows now say what
 * happened; the kinds a runtime reports (`temporary-rate-limit`) are spelled
 * with spaces, and a runtime is named the way Settings names it.
 */
function railWords(kind: string): string {
  return kind.replace(/[-_]+/g, ' ').trim()
}

function railRuntimeName(adapter: string): string {
  const named = runtimeDisplayName(adapter as MissionRuntimeId) as string | undefined
  return named ?? adapter
}

export function railLabel(value: string, limit = 72): string {
  const single = value.replace(/\s+/g, ' ').trim()
  return single.length <= limit ? single : `${single.slice(0, limit - 1)}…`
}

/*
 * ONE formatter, made once. `toLocaleTimeString` with options builds a new
 * one on every call -- about 74 us each -- and the Signal Rail formats every
 * event on every render while it is open: 34 ms a render at the 500-event
 * cap (renderer audit, 2026-09-22). Same options, so the same string.
 */
const RAIL_CLOCK = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' })

export function clockOf(occurredAt: string): string {
  const parsed = new Date(occurredAt)
  return Number.isFinite(parsed.getTime()) ? RAIL_CLOCK.format(parsed) : ''
}

/**
 * The Signal Rail: raw events, newest first, in the product's own vocabulary.
 *
 * This is where detail belongs -- the thread shows semantic items, the rail
 * shows what actually happened. Tone carries meaning rather than decoration:
 * tools are lime while running and neutral once settled, checkpoints blue,
 * limits amber, failures red.
 */
/**
 * A tool call as a sentence: what it did, to what.
 *
 * The rail said "shell . npm test", then "shell finished"; "file_change .
 * C:/Users/.../src/signup.ts", then "file_change finished" -- the runtimes'
 * own type names, two rows a call (first-impressions pass, 0.354). Locust is
 * for any AI user, and a person reads "Ran npm test" and "Changed signup.ts".
 * The words the thread's activity card already uses; a tool this does not
 * recognise keeps its own name, never a guess.
 */
export function railToolName(tool: { readonly name: string; readonly toolKind?: string; readonly command?: string }): string {
  const target = tool.command?.replace(/\s+/g, ' ').trim()
  const shown = target === undefined || target.length === 0 ? undefined : target
  if (isShellTool(tool.name, tool.toolKind)) return railLabel(shown === undefined ? 'Ran a command' : `Ran ${shellCommandText(shown)}`)
  const file = shown === undefined ? undefined : (shown.split(/[\\/]/).filter((part) => part.length > 0).at(-1) ?? shown)
  const words = tool.name.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase().split(/[^a-z]+/).filter((word) => word.length > 0)
  const skill = skillCalled(tool.name, shown ?? '')
  if (skill !== undefined) return railLabel(skillWords(skill, false))
  if (tool.name === 'file_change' || editToolName(tool.name)) return railLabel(file === undefined ? 'Changed a file' : `Changed ${file}`)
  // Codex's one web tool also opens pages, and then its query is the address
  // (0.711, probe-codex-web-search): that is opening a page, not a search.
  if (words.includes('websearch') || (words.includes('web') && words.includes('search'))) {
    if (shown !== undefined && /^https?:\/\/\S+$/i.test(shown)) return railLabel(`Opened ${shown}`)
    return railLabel(shown === undefined ? 'Searched the web' : `Searched the web for ${shown}`)
  }
  if (words.some((word) => word === 'search' || word === 'grep' || word === 'find' || word === 'glob')) return railLabel(shown === undefined ? 'Searched' : `Searched for ${shown}`)
  if (words.some((word) => word === 'read' || word === 'view' || word === 'cat' || word === 'open')) return railLabel(file === undefined ? 'Read a file' : `Read ${file}`)
  if (words.some((word) => word === 'fetch' || word === 'browse')) return railLabel(shown === undefined ? 'Opened a page' : `Opened ${shown}`)
  // A tool the thread has words for (TOOL_WORDS: an image, Locust's git tool) says them here too.
  const known = toolWords(tool.name)?.did(1)
  if (known !== undefined) return railLabel(`${known.charAt(0).toUpperCase()}${known.slice(1)}`)
  return railLabel(shown === undefined ? tool.name : `${tool.name} · ${shown}`)
}

/**
 * A call the mode refused, or the person declined, never ran (0.545). Sol, on
 * 0.544: Activity said "Ran git status" above "refused", for a command that
 * Antigravity's Edit mode never let run.
 */
export function railNeverRan(name: string, status: string | undefined): string {
  if (status !== 'refused' && status !== 'declined') return name
  return name.startsWith('Ran ') ? `Did not run ${name.slice('Ran '.length)}` : name === 'Ran a command' ? 'Did not run a command' : name
}

export function buildSignalRail(
  events: readonly NormalizedRuntimeEvent[],
  options: { readonly running: boolean }
): readonly SignalRow[] {
  /*
   * ONE ROW A TOOL CALL. It was two -- "shell . npm test", then "shell
   * finished . exit code 0" -- which doubled the rail and put a call's
   * outcome in a different row from the call. Now the call's row carries how
   * it ended; a runtime that reports a call only when it finishes (OpenCode)
   * still gets its one row, from that report.
   */
  const settledBy = new Map<string, Extract<NormalizedRuntimeEvent, { type: 'tool.completed' | 'tool.failed' }>>()
  const started = new Set<string>()
  for (const event of events) {
    if (event.type === 'tool.completed' || event.type === 'tool.failed') settledBy.set(event.payload.itemId, event)
    if (event.type === 'tool.started') started.add(event.payload.itemId)
  }
  const outcome = (settled: Extract<NormalizedRuntimeEvent, { type: 'tool.completed' | 'tool.failed' }>): { readonly meta: string; readonly tone: SignalTone } => {
    if (settled.type === 'tool.failed') return { meta: settled.payload.status === 'refused' ? 'refused' : 'failed', tone: 'red' }
    // Exit 0 is what "done" means; any other code is worth its number.
    const code = settled.payload.exitCode
    return { meta: code === undefined || code === 0 ? 'done' : `exit code ${String(code)}`, tone: 'muted' }
  }

  const rows: SignalRow[] = []
  for (const event of events) {
    // A helper's calls are its row's, in the Activity card; the rail is the teammate's.
    if ((event.type === 'tool.started' || event.type === 'tool.completed' || event.type === 'tool.failed') && byHelper(event.payload)) continue
    const clock = clockOf(event.occurredAt)
    switch (event.type) {
      case 'run.started':
        rows.push({
          key: event.id,
          name: `Started on ${railRuntimeName(event.sourceAdapter)}`,
          meta: clock,
          tone: 'muted',
          live: false
        })
        break
      case 'tool.started': {
        const settled = settledBy.get(event.payload.itemId)
        if (settled === undefined) {
          rows.push({
            key: event.id,
            name: railToolName(event.payload),
            meta: `${clock} · ${options.running ? 'running' : 'did not finish'}`,
            tone: options.running ? 'live' : 'muted',
            live: options.running
          })
        } else {
          const ended = outcome(settled)
          rows.push({ key: event.id, name: railNeverRan(railToolName(event.payload), settled.payload.status), meta: `${clock} · ${ended.meta}`, tone: ended.tone, live: false })
        }
        break
      }
      case 'tool.completed':
      case 'tool.failed': {
        // Its call already has its row, which says how it ended.
        if (started.has(event.payload.itemId)) break
        const ended = outcome(event)
        rows.push({ key: event.id, name: railNeverRan(railToolName(event.payload), event.payload.status), meta: `${clock} · ${ended.meta}`, tone: ended.tone, live: false })
        break
      }
      case 'step.started':
      case 'step.completed':
      case 'step.failed': {
        /*
         * A turn beginning and ending, an item opening: bookkeeping, and half
         * the rail on a busy run ("Step finished . turn"). What a person reads
         * of steps is that the model thought, and that a step failed.
         */
        const kind = railWords(event.payload.stepKind)
        if (event.type === 'step.failed') {
          rows.push({ key: event.id, name: 'A step failed', meta: `${clock} · ${kind}`, tone: 'red', live: false })
        } else if (event.type === 'step.completed' && /reasoning/.test(kind)) {
          rows.push({ key: event.id, name: 'Thought', meta: clock, tone: 'muted', live: false })
        }
        break
      }
      case 'plan.updated': {
        /*
         * WHERE THE PLAN STOOD, not that it moved (0.360). Four rows of
         * "Plan updated" between a run's reads and writes said nothing a
         * person could use (the first-session drive, packaged 0.358); the
         * same event carries the steps, so each row now says how far the
         * plan had got -- a trail of progress rather than a repeated word.
         */
        const steps = readPlan(event.payload.plan)
        const done = steps.filter((step) => step.state === 'done').length
        rows.push({
          key: event.id,
          name: `${steps.length === 0 ? 'Plan updated' : `Plan: ${String(done)} of ${String(steps.length)} done`}${event.payload.final ? ' · final' : ''}`,
          meta: clock,
          tone: 'violet',
          live: false
        })
        break
      }
      case 'route.limit_detected':
        rows.push({
          key: event.id,
          name: `Usage limit · ${railWords(event.payload.kind)}`,
          meta: railLabel(`${clock} · ${event.payload.message}`, 96),
          tone: 'amber',
          live: false
        })
        break
      case 'adapter.diagnostic':
        if (keptForTheRecord(event.payload.code)) break
        rows.push({
          key: event.id,
          name: railLabel(event.payload.message.trim().length > 0 ? event.payload.message : 'A note from the runtime', 96),
          meta: `${clock} · ${event.payload.level}`,
          tone: event.payload.level === 'error' ? 'red' : 'amber',
          live: false
        })
        break
      case 'run.completed':
        rows.push({ key: event.id, name: 'Finished', meta: clock, tone: 'blue', live: false })
        break
      case 'run.cancelled':
        rows.push({ key: event.id, name: 'Stopped', meta: `${clock} · by you`, tone: 'amber', live: false })
        break
      case 'run.failed':
        rows.push({
          key: event.id,
          name: `Failed · ${railWords(event.payload.kind)}`,
          meta: railLabel(`${clock} · ${event.payload.message}`, 96),
          tone: 'red',
          live: false
        })
        break
      default:
        break
    }
  }
  // Newest first, as drawn.
  return rows.reverse()
}

export interface PeerGroup {
  /** The OTHER party: `from` of a received message, `to` of a posted one. */
  readonly peer: { readonly teammateId: string; readonly name: string }
  /** Chronological. */
  readonly messages: readonly PublicPeerMessage[]
  /** True when any message in the group was delivered to this mission. */
  readonly received: boolean
}

/**
 * One card per peer. A group that includes a received message sits at the top
 * of the thread, where it was in time -- delivered before the work began; a
 * group of only posted messages sits after the work that produced them.
 */
export function peerGroups(messages: readonly PublicPeerMessage[]): readonly PeerGroup[] {
  const groups = new Map<string, { peer: PeerGroup['peer']; messages: PublicPeerMessage[]; received: boolean }>()
  for (const message of messages) {
    const peer = message.direction === 'received' ? message.from : message.to
    const key = peer.teammateId.length > 0 ? peer.teammateId : `name:${peer.name}`
    const group = groups.get(key) ?? { peer: { teammateId: peer.teammateId, name: peer.name }, messages: [], received: false }
    group.messages.push(message)
    if (message.direction === 'received') group.received = true
    groups.set(key, group)
  }
  return [...groups.values()].map((group) => ({
    peer: group.peer,
    messages: [...group.messages].sort((left, right) => Date.parse(left.at) - Date.parse(right.at)),
    received: group.received
  }))
}

/**
 * How many messages an exchange may hold and still be drawn open.
 *
 * The card collapses so a colleague's aside does not read as the mission's
 * own work, which is right for a long back-and-forth and wrong for the
 * common case: one teammate asks, one answers. Colin's report on 2026-09-04
 * -- *"lets try and keep the whole convo between the bots"* -- was about an
 * exchange of exactly two messages, both hidden behind a toggle he had to
 * find. Two is an exchange you read in place; more is a thread you open.
 */
export const PEER_EXCHANGE_OPEN_LIMIT = 2

/** Whether an exchange is short enough to read in place. */
export function peerExchangeStartsOpen(messageCount: number): boolean {
  return messageCount > 0 && messageCount <= PEER_EXCHANGE_OPEN_LIMIT
}

/**
 * The first line of an exchange, for the collapsed card to preview. A count
 * alone ("2 messages with Wren") says an exchange happened and nothing about
 * what it was, which is the whole reason it went unread.
 */
export function peerSnippet(text: string | null, limit = 72): string | undefined {
  if (text === null) return undefined
  const flat = text.replace(/\s+/gu, ' ').trim()
  if (flat.length === 0) return undefined
  return flat.length <= limit ? flat : `${flat.slice(0, limit - 1).trimEnd()}…`
}

/**
 * What the decision card may honestly say about the workspace.
 *
 * The design's line is "paused, nothing changed". That is a CLAIM, and in a
 * mode that permits edits it is often false -- a run can rewrite four files
 * and then reach the fork it should have asked about first. Saying "nothing
 * changed" there would be a comforting lie in the one place the product is
 * asking to be trusted, so the sentence is derived rather than written:
 *
 *   read-only        the sandbox could not write. A guarantee, not an
 *                    observation, so it is the only case that says "nothing".
 *   edits + evidence a patch came back, so work exists and is kept.
 *   edits + none     nothing was OBSERVED, which is not the same as nothing
 *                    happening -- a runtime need not report every write. So
 *                    it states the permission and claims nothing.
 */
export function decisionStanding(input: {
  readonly sandbox: 'read-only' | 'workspace-write' | 'full-access' | undefined
  readonly events: readonly NormalizedRuntimeEvent[]
}): string {
  if (input.sandbox === 'read-only') return 'stopped here · nothing was changed'
  const wrote = input.events.some(
    (event) =>
      (event.type === 'tool.completed' || event.type === 'tool.failed')
      && (event.payload as { readonly patch?: unknown }).patch !== undefined
  )
  return wrote ? 'stopped here · work already done is kept' : 'stopped here · this run could edit files'
}

export interface ThreadPeerCard {
  readonly key: string
  /** Which turn of the conversation the exchange happened in. */
  readonly turnIndex: number
  /** Received messages sit before that turn's work; posted ones after it. */
  readonly placement: 'before-work' | 'after-work'
  readonly group: PeerGroup
}

/**
 * Every peer card the thread draws, for every turn -- not just the last one.
 *
 * The thread rendered the CURRENT turn's exchange and silently dropped every
 * earlier turn's. That is invisible in the ordinary case and wrong in the one
 * that matters: a teammate who messages a peer usually does it on the turn a
 * person asked them to, and the peer's answer arrives on the NEXT turn. So
 * the message asking was always in an earlier turn and was never drawn. Colin
 * watched Booty ask Wren for a soliloquy on 2026-09-04 and saw only Wren's
 * reply -- the thread read as though Wren had answered HIM.
 *
 * Taking every turn is what makes both halves of an exchange visible. The
 * placement rule is unchanged: what a turn was handed sits above its work,
 * what it sent sits below.
 */
export function threadPeerCards(
  turns: readonly (readonly PublicPeerMessage[])[]
): readonly ThreadPeerCard[] {
  const cards: ThreadPeerCard[] = []
  turns.forEach((messages, turnIndex) => {
    for (const group of peerGroups(messages)) {
      const name = group.peer.teammateId.length > 0 ? group.peer.teammateId : group.peer.name
      cards.push({
        // Turn-scoped: the same peer appears on more than one turn of a
        // conversation, and two cards keyed alike would collapse into one.
        key: `peer_${String(turnIndex)}_${name}`,
        turnIndex,
        placement: group.received ? 'before-work' : 'after-work',
        group
      })
    }
  })
  return cards
}

/**
 * The mission a continuation chain started from. A handed-off mission's own
 * recorded prompt is the briefing the host wrote, so the words a person
 * actually typed live on the FIRST mission of the chain. Bounded walk: a
 * cycle in hand-edited ledgers must not spin forever.
 */
export function rootMission(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): PublicRecoveredMission {
  /*
   * WALKED TO THE END, GUARDED BY WHAT IT HAS SEEN.
   *
   * This stopped after 32 hops and returned whatever turn it had reached AS
   * THE ROOT. On a conversation longer than 32 turns that answer is wrong,
   * and wrong DIFFERENTLY for each turn -- the newest stops 32 back, an
   * older one reaches the real root -- so `collapseConversations`, which
   * keys on exactly this id, split one conversation into several rows.
   *
   * MEASURED IN COLIN'S OWN LEDGER, 2026-09-21, which is what settled it.
   * He reported Antigravity "spawning a new conversation after a workflow
   * finishes", then "it just did it again". Nothing spawned: his Antigravity
   * chain is ONE unbroken run of 36 turns (`mission_3bd1ccbf` back to
   * `mission_4c65cf41`), and the whole ledger holds only two true Antigravity
   * roots. The sidebar was drawing one conversation three times, and
   * `groups.json` had filed three ids of that same chain -- 4c65cf41 under
   * Locust, 0f4888ce and 9de113fa under Chief -- because he had tidied the
   * phantom rows into groups, which made the split permanent.
   *
   * Each new turn past 32 slides the window and mints another pseudo-root,
   * which is exactly the "it happened again" he saw.
   *
   * The cap was never about depth. It is cycle protection for a hand-edited
   * `continuesFrom`, and a `seen` set is that -- exactly, with no number to
   * be wrong. Four walks in this codebase carried four different numbers
   * (32, 64, 32, 64) for the same question; fewer clocks, not fewer numbers.
   */
  let current = mission
  const seen = new Set<string>([mission.missionId])
  for (;;) {
    const priorId = current.continuesFrom?.missionId
    if (priorId === undefined || seen.has(priorId)) return current
    const prior = byId.get(priorId)
    if (prior === undefined) return current
    seen.add(priorId)
    current = prior
  }
}

export interface StitchedHandoff {
  readonly from: PublicRecoveredMission['runtime']
  readonly to: PublicRecoveredMission['runtime']
  readonly fromModel?: string
  readonly toModel?: string
  readonly at: string | undefined
  readonly unsettledCount: number
  readonly omittedBriefing: readonly string[]
  /** What the person chose to leave out of the brief (0.527). */
  readonly leftOutByYou?: readonly string[]
  readonly priorEvents: readonly NormalizedRuntimeEvent[]
}

/**
 * The divider for a recovered continuation, rebuilt from the durable record.
 * The unsettled count is the prior mission's route-switch checkpoint -- the
 * same number the live divider showed -- and not a recount, so a reopened
 * thread says what it said at the time. What the briefing omitted was never
 * recorded, so it is reported as nothing rather than guessed.
 */
export function stitchedHandoff(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): StitchedHandoff | undefined {
  const link = mission.continuesFrom
  // A follow-up continues the same runtime's own conversation: there is no
  // seam to draw, and a "Codex to Codex" divider across an ordinary reply
  // would invent an event that never happened.
  if (link === undefined || link.reason !== 'route-switch') return undefined
  // A reply sent to another runtime is its own turn after the one it
  // answers, and its seam is drawn before it (`switchOf`). Stitched here, the
  // earlier turn's work was drawn UNDER the reply and its first message was
  // lost (drive-runtime-switch, packaged 0.309, opened again).
  if (isReplySwitch(mission)) return undefined
  const prior = byId.get(link.missionId)
  if (prior === undefined) return undefined
  const checkpoint = prior.checkpoints.find((entry) => entry.epoch === link.checkpointEpoch)
  return {
    from: prior.runtime,
    to: mission.runtime,
    fromModel: prior.model,
    toModel: mission.model,
    at: new Date(mission.createdAt).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
    unsettledCount: checkpoint?.unsettledActions.length ?? 0,
    // Recorded since 0.519; an older switch recorded nothing, and nothing is guessed.
    omittedBriefing: link.leftOut ?? [],
    ...(link.leftOutByYou === undefined ? {} : { leftOutByYou: link.leftOutByYou }),
    priorEvents: prior.events
  }
}

/**
 * What a route's model actually turned out to be, learned from missions that
 * already ran.
 *
 * Claude Code takes an ALIAS -- `fable`, `opus`, `sonnet` -- and resolves it
 * to whichever model is newest in that family, so the shell cannot know the
 * real name up front without asking, and asking costs a turn. It does not
 * have to: the runtime states the resolved model in its own start record, so
 * every finished mission on that alias is a free, current answer. Keyed
 * `runtime:model`, newest mission wins.
 *
 * This is earned knowledge, never a guess: an alias nobody has run yet simply
 * has no entry, and the picker says what it does know instead of inventing a
 * version number that would rot.
 */
export function resolvedModelNames(
  missions: readonly PublicRecoveredMission[]
): ReadonlyMap<string, string> {
  const byRoute = new Map<string, { readonly at: number; readonly name: string }>()
  for (const mission of missions) {
    // The runtime's own word, in either place it says it. Claude Code's
    // START record repeats the alias it was given -- `sonnet` for `sonnet`,
    // which teaches nothing -- and it is the RESULT that states the model
    // the alias turned out to mean. Reading only the start is why the picker
    // showed three bare aliases however many runs a person had done (Colin,
    // 2026-09-06: "in model list they are just listed as sonnet, fable, and
    // opus").
    const settled = mission.events.find((event) => event.type === 'run.completed')
    const resolved =
      settled?.type === 'run.completed' ? settled.payload.resolvedModel : undefined
    const started = mission.events.find((event) => event.type === 'run.started')
    if (resolved === undefined && (started === undefined || started.type !== 'run.started')) continue
    const raw = started?.type === 'run.started' ? started.payload.evidence.raw : undefined
    const fromStart =
      typeof raw === 'object' && raw !== null && !Array.isArray(raw)
        ? (raw as Record<string, unknown>).model
        : undefined
    const name = resolved ?? fromStart
    // Only a real, different name is worth showing: `fable -> fable` teaches
    // nothing, and a blank teaches less.
    if (typeof name !== 'string' || name.length === 0 || name === mission.model) continue
    const key = `${mission.runtime}:${mission.model}`
    const at = Date.parse(mission.createdAt)
    const held = byRoute.get(key)
    if (held === undefined || (Number.isFinite(at) && at > held.at)) {
      byRoute.set(key, { at: Number.isFinite(at) ? at : 0, name })
    }
  }
  return new Map([...byRoute].map(([key, held]) => [key, held.name]))
}

/**
 * WHICH MODEL A RUN UNDER WAY IS ON, from its own start record (0.827). A turn Locust carried on with Claude Code on its
 * default read "Claude  High" until it finished, and for good when no turn on that default had finished before (Colin,
 * 2026-10-23, on a turn carried on from Antigravity: "still only reading as claude and high"). Claude Code's start
 * record names the model its default is, so the run says it as it starts. Keyed as resolvedModelNames is.
 */
export function liveModelNames(
  runs: Iterable<{ readonly runtime?: string; readonly model?: string; readonly events: readonly NormalizedRuntimeEvent[] }>
): ReadonlyMap<string, string> {
  const named = new Map<string, string>()
  for (const run of runs) {
    if (run.runtime === undefined || run.model === undefined) continue
    const started = run.events.find((event) => event.type === 'run.started')
    const raw = started?.type === 'run.started' ? started.payload.evidence.raw : undefined
    const model = typeof raw === 'object' && raw !== null && !Array.isArray(raw) ? (raw as Record<string, unknown>).model : undefined
    if (typeof model === 'string' && model.length > 0 && model !== run.model) named.set(`${run.runtime}:${run.model}`, model)
  }
  return named
}

export function resolvedModelKey(runtime: MissionRuntimeId, model: string): string {
  return `${runtime}:${model}`
}

/**
 * Where a conversation moved to another runtime between two turns: the reply
 * was sent to a different runtime than the turn it answers, and the host
 * briefed that runtime from the earlier turn's record. Drawn as the handoff
 * divider BEFORE the reply -- everything above it was said on `from`,
 * everything below on `to`.
 */
export interface TurnSwitch {
  readonly from: MissionRuntimeId
  readonly to: MissionRuntimeId
  /** The models on either side (0.779), named on the divider beside each runtime's mark; absent, the runtime is named. */
  readonly fromModel?: string
  readonly toModel?: string
  readonly at: string | undefined
  readonly unsettledCount: number
  readonly omittedBriefing: readonly string[]
  /** Locust made this switch because `from`'s plan ran out (0.774), not the person. */
  readonly carriedOn?: true
}

export interface ConversationTurn {
  readonly missionId: string
  /** What the person typed for this turn. */
  readonly prompt: string
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly peerMessages: PublicRecoveredMission['peerMessages']
  /** Set when this turn was a reply sent to another runtime. */
  readonly switchedFrom?: TurnSwitch
  /** Set when the person had this exchange in that runtime's own terminal (0.391). */
  readonly inTerminal?: MissionRuntimeId
  /** The other version of this turn, when a message here was edited (0.498). */
  readonly versions?: TurnVersions
}

/**
 * THE OTHER VERSION OF AN EDITED MESSAGE (0.498).
 *
 * Editing an earlier message starts a second turn after the same one; the
 * first stays, with everything that followed it. `before` is where that
 * earlier version got to, for a turn that IS the edit; `after` is where the
 * edit got to, for a turn an edit replaced. Each is the newest turn of that
 * branch -- what opening it should show. Only a turn marked `edited` makes a
 * version: two follow-ups of one turn for any other reason are not an edit.
 */
export interface TurnVersions {
  readonly before?: string
  readonly after?: string
}

type Linked = { readonly missionId: string; readonly createdAt: string; readonly continuesFrom?: { readonly missionId: string; readonly reason: string; readonly edited?: true } }

/** The newest turn reached from `start` through the turns that continue it. */
function newestTurnFrom<T extends Linked>(start: T, children: ReadonlyMap<string, readonly T[]>): T {
  let newest = start
  const seen = new Set<string>()
  const queue = [start]
  while (queue.length > 0) {
    const at = queue.shift()!
    if (seen.has(at.missionId)) continue
    seen.add(at.missionId)
    if (Date.parse(at.createdAt) > Date.parse(newest.createdAt)) newest = at
    queue.push(...(children.get(at.missionId) ?? []))
  }
  return newest
}

export function turnVersions<T extends Linked>(missionId: string, byId: ReadonlyMap<string, T>): TurnVersions | undefined {
  const turn = byId.get(missionId)
  const parent = turn?.continuesFrom
  if (turn === undefined || parent === undefined || parent.reason !== 'follow-up') return undefined
  const children = new Map<string, T[]>()
  for (const mission of byId.values()) {
    const from = mission.continuesFrom?.missionId
    if (from === undefined) continue
    const held = children.get(from)
    if (held === undefined) children.set(from, [mission])
    else held.push(mission)
  }
  const siblings = (children.get(parent.missionId) ?? [])
    .filter((mission) => mission.continuesFrom?.reason === 'follow-up')
    .sort((left, right) => Date.parse(left.createdAt) - Date.parse(right.createdAt))
  const at = siblings.findIndex((mission) => mission.missionId === missionId)
  if (at < 0) return undefined
  const older = turn.continuesFrom?.edited === true ? siblings[at - 1] : undefined
  const newer = siblings.slice(at + 1).find((mission) => mission.continuesFrom?.edited === true)
  if (older === undefined && newer === undefined) return undefined
  return {
    ...(older === undefined ? {} : { before: newestTurnFrom(older, children).missionId }),
    ...(newer === undefined ? {} : { after: newestTurnFrom(newer, children).missionId })
  }
}

/**
 * Where a conversation went into a runtime's own terminal, or came back
 * (0.391): into it before the first exchange brought back from there, back in
 * Locust before the next turn Locust ran. Nothing between two turns that were
 * both had there.
 */
export type TerminalSeam = { readonly into: MissionRuntimeId } | { readonly back: true }

export function terminalSeamBefore(previous: MissionRuntimeId | undefined, current: MissionRuntimeId | undefined): TerminalSeam | undefined {
  if (current !== undefined) return current === previous ? undefined : { into: current }
  return previous === undefined ? undefined : { back: true }
}

/**
 * A route switch made BETWEEN turns -- a reply sent to another runtime --
 * rather than a running mission handed over. Both are recorded as
 * 'route-switch' continuations; only the reply's briefing carries the
 * person's new words under the instruction marker (`handoffInstruction`),
 * which is the same test `typedPrompt` reads the words back by.
 */
export function isReplySwitch(mission: PublicRecoveredMission): boolean {
  return mission.continuesFrom?.reason === 'route-switch' && handoffInstruction(mission.prompt) !== undefined
}

/** The seam before a reply sent to another runtime, rebuilt from the record. */
export function switchOf(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): TurnSwitch | undefined {
  const link = mission.continuesFrom
  if (link === undefined || !isReplySwitch(mission)) return undefined
  const prior = byId.get(link.missionId)
  if (prior === undefined) return undefined
  const checkpoint = prior.checkpoints.find((entry) => entry.epoch === link.checkpointEpoch)
  return {
    from: prior.runtime,
    to: mission.runtime,
    fromModel: prior.model,
    toModel: mission.model,
    at: new Date(mission.createdAt).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
    unsettledCount: checkpoint?.unsettledActions.length ?? 0,
    // Recorded since 0.519; an older switch recorded nothing, and nothing is guessed.
    omittedBriefing: link.leftOut ?? [],
    ...(link.leftOutByYou === undefined ? {} : { leftOutByYou: link.leftOutByYou }),
    ...(mission.startedBy?.kind === 'carry-on' ? { carriedOn: true as const } : {})
  }
}

/**
 * A mission and every earlier turn of its conversation, oldest first.
 *
 * Each turn is its own mission -- one mission holds one run, and a second turn
 * is a second process -- so the thread has to walk the links back to rebuild
 * what a person experienced as one exchange: follow-ups, and replies sent to
 * another runtime (each such turn carries its seam, `switchedFrom`). A
 * running mission handed over is one turn, not two. Bounded, so a
 * hand-edited cycle cannot spin.
 */
export function conversationTurns(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): readonly ConversationTurn[] {
  // Bounded by what it has already seen rather than by a count: the count
  // was cycle protection, and a 65-turn conversation would have lost its
  // earliest turns the way `rootMission`'s 32 lost Colin's.
  const seen = new Set<string>([mission.missionId])
  const turns: ConversationTurn[] = []
  let latest = mission
  for (;;) {
    // A RUNNING mission handed to another runtime is still one turn: the
    // handed-off run's work is drawn inside it (`stitchedHandoff`), so the
    // walk steps over it to the mission the turn began with. This used to
    // stop there, and every turn before a handoff was lost from a reopened
    // conversation.
    let head = latest
    for (;;) {
      const link = head.continuesFrom
      if (link === undefined || link.reason !== 'route-switch' || isReplySwitch(head)) break
      const prior = byId.get(link.missionId)
      if (prior === undefined || seen.has(prior.missionId)) break
      seen.add(prior.missionId)
      head = prior
    }
    const switchedFrom = switchOf(head, byId)
    const versions = turnVersions(head.missionId, byId)
    turns.push({
      ...(versions === undefined ? {} : { versions }),
      missionId: latest.missionId,
      prompt: latest.prompt,
      events: latest.events,
      peerMessages: latest.peerMessages,
      ...(switchedFrom === undefined ? {} : { switchedFrom }),
      ...(latest.startedBy?.kind === 'terminal' ? { inTerminal: latest.runtime } : {})
    })
    // The turn before this one: a follow-up on the same runtime, or a reply
    // sent to another runtime -- the conversation carried on either way, and
    // stopping at the switch dropped everything said before it.
    const link = head.continuesFrom
    if (link === undefined || (link.reason !== 'follow-up' && !isReplySwitch(head))) break
    const prior = byId.get(link.missionId)
    if (prior === undefined || seen.has(prior.missionId)) break
    seen.add(prior.missionId)
    latest = prior
  }
  return turns.reverse()
}

/**
 * The runtime session a reply could resume, if the run left one.
 *
 * A mission that ended is not automatically a conversation you can continue.
 * When a run fails BEFORE its runtime ever started -- the CLI was not ready,
 * the process could not launch -- no session was ever opened, and the host
 * has nothing to resume. Treating the next message as a reply there turns an
 * ordinary sentence into an error card, and the person's message is not sent
 * at all. A fresh mission is what they meant, and what they get.
 *
 * The session id is read from the events the run actually produced, so this
 * cannot claim one that was never recorded.
 */
/**
 * Capacity exhaustion, in whatever words a runtime uses for it. Cursor says
 * `RetriableError: [resource_exhausted]`, which names a real condition in
 * language nobody outside its codebase can read.
 */
const EXHAUSTION_PATTERNS = [
  /\bresource_exhausted\b/i,
  /\binsufficient_quota\b/i,
  /\bquota (?:exceeded|exhausted)\b/i,
  /\brate[_ -]?limit(?:ed| exceeded)?\b/i,
  /\btoo many requests\b/i,
  /\bhttp\s*429\b/i
] as const

/** A failure that already names its limit in plain words, needing no runtime line after it (0.823). */
const LIMIT_ALREADY_SAID = /\b(?:plan|quota) is used up\b|\bquota reached\b|\busage limit (?:has been |was )?reached\b/i

/**
 * A runtime that could not write its OWN settings file.
 *
 * Colin, 2026-09-14, 0.119.0, on a peer message between two Cursor
 * teammates -- "this def used to work":
 *
 *   Cursor Agent ended without a terminal result record. The runtime's own
 *   last word was: Error: EPERM: operation not permitted, rename
 *   'C:\Users\...\.cursor\cli-config.json.19620.<uuid>.tmp' ->
 *   'C:\Users\...\.cursor\cli-config.json'
 *
 * Which is two absolute paths, a uuid and a POSIX errno to say something a
 * person could act on in one sentence. `cursor-agent` rewrites that file on
 * startup by writing a temp beside it and renaming over the top, and on
 * WINDOWS that rename fails with EPERM while another process still has the
 * destination open. Asking one teammate to ask another starts a second
 * `cursor-agent` while the first is live, so the room features are exactly
 * what provokes it. There were thirteen abandoned `.tmp` files beside that
 * config dating back to 2026-09-02, so it is intermittent and old, not new.
 *
 * This is NOT Locust's file and not the person's workspace -- it is the
 * runtime's own config in their home folder -- and that is the fact the
 * sentence has to carry, because "operation not permitted" reads like a
 * permissions problem with THEIR project.
 *
 * Deliberately says nothing about whether the workspace was touched. The run
 * died somewhere and this cannot know where; a reassuring second clause that
 * turns out to be wrong is worse than the bare sentence it replaced.
 */
const CONFIG_LOCKED_PATTERNS = [/\bEPERM\b/i, /\boperation not permitted\b/i] as const

/** The runtime's own settings file, as each CLI names it on disk. */
const RUNTIME_CONFIG_FILES = ['cli-config.json', 'config.json', 'settings.json', 'auth.json'] as const

function isLockedRuntimeConfig(said: string): boolean {
  if (!said.toLowerCase().includes('rename')) return false
  if (!CONFIG_LOCKED_PATTERNS.some((pattern) => pattern.test(said))) return false
  return RUNTIME_CONFIG_FILES.some((name) => said.includes(name))
}

/**
 * Terminal colour codes, stripped.
 *
 * A runtime writes stderr for a terminal, so its own last word arrives
 * wrapped in escape sequences -- and this card is not a terminal. Colin,
 * 2026-09-05: OpenCode's refusal reached the screen as three boxes before
 * the sentence a person needs to read, because ESC[93m ESC[1m ESC[0m have
 * no glyphs. The codes carry no meaning here; the card has its own colour.
 *
 * Built from the character code rather than a literal escape, which is
 * invisible in a diff and easy to break silently.
 */
const ANSI = new RegExp(String.fromCharCode(27) + '\\[[0-9;?]*[ -/]*[@-~]', 'g')

/** The last line of stderr that says something, or undefined. */
function lastStderrLine(stderr: string | undefined): string | undefined {
  if (stderr === undefined) return undefined
  const lines = stderr
    .replace(ANSI, '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
  return lines.at(-1)
}

/**
 * A structured log line, said as its message (QA-2026-09-29 round 2, R17).
 * OpenCode's `--print-logs` lines ended a failure card as `timestamp=...
 * level=ERROR run=... message="stream error" providerID=... error.error=
 * "AI_APICallError: Rate limit exceeded..."`. The error, else the message,
 * is what a person needs; anything that is not such a line is kept as it is.
 */
export function messageOfLogLine(line: string): string {
  if (!/\blevel=[A-Z]+\b/.test(line) || !/\b[a-zA-Z.]+="/.test(line)) return line
  const quoted = (key: string): string | undefined => {
    const at = line.indexOf(`${key}="`)
    if (at < 0) return undefined
    let text = ''
    for (let index = at + key.length + 2; index < line.length; index += 1) {
      const character = line[index]!
      if (character === '\\' && index + 1 < line.length) {
        text += line[index + 1]
        index += 1
        continue
      }
      if (character === '"') return text.trim()
      text += character
    }
    return text.trim()
  }
  return quoted('error.error') ?? quoted('error') ?? quoted('message') ?? line
}

/**
 * Whether a failure was the runtime's own limit (QA-2026-09-29 round 2, R17):
 * the same reading `failureMessage` puts into words, for a control that
 * offers the way on.
 */
export function failedOnItsLimit(payload: {
  readonly process?: { readonly stderr?: string }
}): boolean {
  const line = lastStderrLine(payload.process?.stderr)
  return line !== undefined && EXHAUSTION_PATTERNS.some((pattern) => pattern.test(messageOfLogLine(line)))
}

/**
 * OPENCODE RETIRED THE MODEL (0.710).
 *
 * On 2026-10-08 every run on Exo Free ended "OpenCode stopped: Model exo-free
 * has been deprecated." while `opencode models` -- and models.dev, refreshed --
 * still listed it as active. Being first of the free models alphabetically, it
 * was what Home's "Use a free model" gave a new person and what Compare put
 * beside a free model: a first run that fails, and a column that never
 * answers. Such a model is left out of the window's model list from then on
 * (retiredFreeModels), and the failed run's card offers the next free one.
 *
 * AND "MODEL NOT FOUND" (0.712). A fresh OpenCode 1.18.35 lists the models its
 * binary was built with until it next reads models.dev, and Fledge Alpha was
 * still among them -- first of the free ones once Exo was left out -- while
 * OpenCode's server answered "Model not found: opencode/fledge-alpha-free. Did
 * you mean: ...". It was the first message a new person sent, on a bare Mac
 * (_smoke/mac-first-hour-smoke.mjs, 2026-10-09), and the line said why with
 * nothing to press. Retired, the same way.
 */
const RETIRED_PATTERN = /\bmodel \S+ has been deprecated\b|\bmodel not found: \S+/i

export function modelRetired(payload: { readonly message?: string }): boolean {
  return RETIRED_PATTERN.test(payload.message ?? '')
}

/**
 * THE MODEL'S PROVIDER IS DOWN (0.711). OpenCode's free models go down for
 * hours at a time: Ling 3.0 answered "Endpoint is unavailable" all of
 * 2026-09-30, and "Upstream request failed: Model is unavailable" on
 * 2026-10-09 while Ling 3.1 answered. Going on with the same model fails
 * again; the card offers the next free model instead. And refused outright
 * (2026-10-12, "Upstream request failed: Invalid credential" on every free
 * model, for everyone): the adapter's sentence for that says the free model
 * was refused by OpenCode's service (opencode-error.ts), never a sign-in.
 */
const UNAVAILABLE_PATTERN = /\b(?:model|endpoint) is unavailable\b|\brefused the free model\b/i

export function modelUnavailable(payload: { readonly message?: string }): boolean {
  return UNAVAILABLE_PATTERN.test(payload.message ?? '')
}

/** Retired before anyone here ran it: a new person's first press must not find out the hard way. */
export const KNOWN_RETIRED_MODELS: readonly string[] = ['opencode/exo-free', 'opencode/fledge-alpha-free']

/**
 * The models to leave out of the window's list: the known ones, and any whose
 * latest run here ended on OpenCode's "has been deprecated" -- unless a run on
 * it finished after that, which says it is back.
 */
export function retiredFreeModels(
  missions: readonly Pick<PublicRecoveredMission, 'runtime' | 'model' | 'phase' | 'lastUpdatedAt' | 'events'>[]
): ReadonlySet<string> {
  const retiredAt = new Map<string, number>(KNOWN_RETIRED_MODELS.map((model) => [model, 0]))
  const answeredAt = new Map<string, number>()
  for (const mission of missions) {
    if (mission.runtime !== 'opencode') continue
    const at = Date.parse(mission.lastUpdatedAt)
    const stamp = Number.isFinite(at) ? at : 0
    if (mission.phase === 'completed') {
      answeredAt.set(mission.model, Math.max(answeredAt.get(mission.model) ?? -1, stamp))
      continue
    }
    if (mission.phase !== 'failed') continue
    const failed = [...mission.events].reverse().find((event) => event.type === 'run.failed')
    if (failed === undefined || failed.type !== 'run.failed' || !modelRetired(failed.payload)) continue
    retiredAt.set(mission.model, Math.max(retiredAt.get(mission.model) ?? -1, stamp))
  }
  return new Set([...retiredAt].filter(([model, at]) => (answeredAt.get(model) ?? -1) <= at).map(([model]) => model))
}

/**
 * FREE MODELS THAT ARE DOWN RIGHT NOW (0.711). Ling 3.0 answered "Model is
 * unavailable" from 2026-10-08 into 2026-10-09 -- more than twelve hours --
 * and, as the first free model listed, it was still what "Use a free model"
 * started on, what Compare put beside a free model, and what a card offered
 * after another free model failed. A free model whose latest run here ended on
 * that is not suggested for six hours, unless a run on it has finished since.
 * It stays in the list: anyone can still pick it, and after six hours it is
 * suggested again, since down is not retired.
 */
export const DOWN_FOR_MS = 6 * 60 * 60 * 1000

export function downFreeModels(
  missions: readonly Pick<PublicRecoveredMission, 'runtime' | 'model' | 'phase' | 'lastUpdatedAt' | 'events'>[],
  now: number
): ReadonlySet<string> {
  const downAt = new Map<string, number>()
  const answeredAt = new Map<string, number>()
  for (const mission of missions) {
    if (mission.runtime !== 'opencode' || !mission.model.endsWith('-free')) continue
    const at = Date.parse(mission.lastUpdatedAt)
    if (!Number.isFinite(at)) continue
    if (mission.phase === 'completed') {
      answeredAt.set(mission.model, Math.max(answeredAt.get(mission.model) ?? -1, at))
      continue
    }
    if (mission.phase !== 'failed') continue
    const failed = [...mission.events].reverse().find((event) => event.type === 'run.failed')
    if (failed === undefined || failed.type !== 'run.failed' || !modelUnavailable(failed.payload)) continue
    downAt.set(mission.model, Math.max(downAt.get(mission.model) ?? -1, at))
  }
  return new Set([...downAt].filter(([model, at]) => now - at < DOWN_FOR_MS && (answeredAt.get(model) ?? -1) <= at).map(([model]) => model))
}

/**
 * The provider's servers were busy or dropped the turn -- not the person's
 * account, quota or folder (0.511).
 *
 * Colin, 2026-09-30, on a Codex run that stopped after 56 minutes with
 * "Selected model is at capacity. Please try a different model.": "classic
 * openai, after an hour". The card said so and offered nothing; he typed
 * "continue" himself, and the same conversation picked up. A limit of the
 * person's own (EXHAUSTION_PATTERNS) is not this: waiting for it does not help.
 */
const PROVIDER_BUSY_PATTERNS = [
  /\bat capacity\b/i,
  /\boverloaded\b/i,
  /\btemporarily unavailable\b/i,
  /\bservice unavailable\b/i,
  /\bhttp\s*(?:502|503|504|529)\b/i,
  /\binternal server error\b/i,
  /\bstream disconnected before completion\b/i
] as const

export function failedOnProviderSide(payload: {
  readonly message?: string
  readonly process?: { readonly stderr?: string }
}): boolean {
  const said = [payload.message ?? '', messageOfLogLine(lastStderrLine(payload.process?.stderr) ?? '')]
  if (said.some((text) => EXHAUSTION_PATTERNS.some((pattern) => pattern.test(text)))) return false
  return said.some((text) => PROVIDER_BUSY_PATTERNS.some((pattern) => pattern.test(text)))
}

/**
 * What to put on a failure card.
 *
 * The host's own sentence names the SHAPE of the failure ("Codex invocation
 * did not complete successfully", "Cursor Agent ended without a terminal
 * result record") and never its cause. The cause is in the process's stderr,
 * which the ledger has been recording all along and the card was throwing
 * away: two runs Colin lost on 2026-09-03 were a folder Codex would not run
 * in and a Cursor account out of capacity, and both read on screen as the
 * same shrug. So the runtime's own last word is always shown, and the one
 * condition whose wording is pure jargon is said in English instead.
 */
export function failureMessage(payload: {
  readonly message: string
  readonly process?: { readonly stderr?: string; readonly exitCode?: number | null }
}, runtime?: MissionRuntimeId): string {
  const readable = providerErrorSentence(payload.message, runtime)
  if (readable !== payload.message) return readable
  const line = lastStderrLine(payload.process?.stderr)
  if (line === undefined) return payload.message
  const said = messageOfLogLine(line)
  const readableStderr = providerErrorSentence(said, runtime)
  if (readableStderr !== said) return readableStderr
  if (sameSentence(payload.message, said)) return payload.message
  if (EXHAUSTION_PATTERNS.some((pattern) => pattern.test(said))) {
    // Already said in plain words -- Muse Code's plan used up, Antigravity's quota reached (0.823) -- the runtime's line
    // only repeated it behind "out of capacity", request id and raw JSON included. The record keeps those words.
    if (LIMIT_ALREADY_SAID.test(payload.message)) return payload.message
    return `${payload.message} The runtime reported that it is out of capacity right now — its own limit, not this machine's: ${said}`
  }
  if (isLockedRuntimeConfig(said)) {
    return `${payload.message} It could not save its own settings file, which another run of the same runtime had open — that file is the runtime's own, in your home folder, and nothing in this workspace was denied to it. Starting the run again usually works.`
  }
  return `${payload.message} The runtime's own last word was: ${said}`
}

/**
 * Two messages that say the same thing, allowing for the trim and the
 * trailing period one channel adds and another does not.
 */
export function sameSentence(a: string, b: string): boolean {
  const norm = (text: string): string => text.trim().replace(/\.\s*$/, '').toLowerCase()
  const left = norm(a)
  const right = norm(b)
  return left.length > 0 && (left === right || left.includes(right) || right.includes(left))
}

/**
 * Whether the run-level error card would only repeat an error diagnostic or
 * ending limit card already in the thread. The run's error is the runtime's own last word wrapped in the
 * host's sentence, and for a quota failure that word is the limit message --
 * so the bottom card was the third rendering of one fact (user session 1,
 * 2026-09-05). Only a limit that ENDS the run counts: a slow-down warning is
 * not why a run failed, and its presence must not hide the real reason.
 */
export function errorAlreadyShown(items: readonly ThreadItem[], error: string): boolean {
  return items.some(
    (item) => ((item.type === 'limit' && item.kind === 'quota-exhausted') || (item.type === 'diagnostic' && item.level === 'error')) && sameSentence(error, item.message)
  )
}

/**
 * SENT AGAIN (0.496): a turn stopped before it said or did anything, whose
 * same words were sent as the next turn. The thread drew the message twice
 * with nothing between -- a double-clicked Send, then Send again -- so this
 * one is not drawn: the next turn is the same message, answered.
 */
export function sentAgainBy(
  turn: { readonly prompt: string; readonly events: readonly NormalizedRuntimeEvent[] },
  nextPrompt: string | undefined
): boolean {
  if (nextPrompt === undefined || turn.prompt.trim().length === 0 || turn.prompt.trim() !== nextPrompt.trim()) return false
  return stoppedBeforeSaying(turn.events)
}

/** Stopped before it said, planned or ran anything: a turn with nothing under its message. */
export function stoppedBeforeSaying(events: readonly NormalizedRuntimeEvent[]): boolean {
  let stopped = false
  for (const event of events) {
    if (event.type === 'run.cancelled') stopped = true
    if (event.type === 'message.delta' || event.type === 'tool.started' || event.type === 'plan.updated') return false
  }
  return stopped
}

export function resumableSessionOf(
  events: readonly NormalizedRuntimeEvent[]
): string | undefined {
  for (const event of events) {
    const held = (event as { readonly runtimeThreadId?: unknown }).runtimeThreadId
    if (typeof held === 'string' && held.length > 0) return held
  }
  return undefined
}

/**
 * The words a PERSON typed for this turn.
 *
 * A mission's own prompt is not always something anybody wrote. A route
 * switch starts a new mission whose prompt is the host's briefing, so for
 * that one the typed words are the previous mission's. A follow-up is the
 * opposite: its prompt IS what the person just typed, and reaching past it to
 * the start of the conversation shows them the wrong sentence -- the reply
 * they actually sent appears nowhere, and the opening line appears twice.
 *
 * So this walks back through route switches only, and stops at the first
 * mission whose prompt was written by a person.
 */
/**
 * The person's own next words inside a handoff briefing, if it carries any.
 *
 * A route switch is usually a rescue -- nobody typed anything, and the walk
 * back below is what finds the words that started the conversation. But a
 * handoff can also be a REPLY, and `main/handoff.ts` puts those words in a
 * section of their own, last, under this exact sentence. Walking past it
 * showed the person the task they opened with and never the instruction the
 * run was actually started for (0.35.0 targeted QA).
 *
 * Matched on the host's own string, which this app writes and owns -- not on
 * something a runtime said. It is still a seam: the honest home for this is a
 * field on `continuesFrom`, which needs a ledger version, and until that
 * exists a mission recorded before it would have nothing to read anyway.
 */
export const HANDOFF_INSTRUCTION_MARKER = 'The person now asks:'

/**
 * The host writes the marker as a section of its own: a blank line, the
 * sentence, then a blank line, then the words. Matching the bare sentence
 * anywhere in any prompt found it inside a person's OWN typing -- someone
 * working on this very file typed `Rename the string "The person now asks:"
 * in handoff.ts` and the mission was titled `" in handoff.ts`, and a rescue
 * briefing that QUOTES the original task matched inside the quote and showed
 * the briefing as the person's words (QA, 2026-09-06).
 *
 * So the anchor is the section shape, not the sentence, and it is only ever
 * looked for in a continuation the host actually wrote.
 */
const HANDOFF_INSTRUCTION_SECTION = `\n\n${HANDOFF_INSTRUCTION_MARKER}\n\n`

export function handoffInstruction(prompt: string): string | undefined {
  const at = prompt.lastIndexOf(HANDOFF_INSTRUCTION_SECTION)
  if (at === -1) return undefined
  const asked = prompt.slice(at + HANDOFF_INSTRUCTION_SECTION.length).trim()
  return asked.length === 0 ? undefined : asked
}

/**
 * What a turn is CALLED, from its prompt: a routine step handed to another
 * teammate is named by the step, never by the answer quoted above it (0.435).
 * The first packaged drive found the sidebar titled "Wren did the step before
 * this one and answered:" after a restart.
 */
export function shownPrompt(mission: { readonly prompt: string; readonly startedBy?: { readonly kind: string } }): string {
  if (mission.startedBy?.kind === 'routine') return stepWordsOf(mission.prompt)
  // A teammate tagged in another conversation is named by the message, not the context after it (0.438).
  return mission.startedBy === undefined || mission.startedBy.kind === 'tag' ? taggedWordsOf(mission.prompt) : mission.prompt
}

export function typedPrompt(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): string {
  let current = mission
  // Same walk, same guard. See `rootMission`.
  const seen = new Set<string>([mission.missionId])
  for (;;) {
    const relayed = relayedTitle(current)
    if (relayed !== undefined) return relayed
    // A routine step handed to another teammate carries the answer before it
    // and, for a checker, the rule; the person's step is what follows the mark (0.435).
    if (current.startedBy?.kind === 'routine') return stepWordsOf(current.prompt)
    // Only where the host wrote the prompt. A mission a PERSON typed is their
    // words already, whatever sentences it happens to contain.
    if (current.continuesFrom?.reason !== 'route-switch') return current.startedBy === undefined ? taggedWordsOf(current.prompt) : current.prompt
    const asked = handoffInstruction(current.prompt)
    if (asked !== undefined) return asked
    const priorId = current.continuesFrom.missionId
    // The guard the removed counter was standing in for. Without it a
    // hand-edited cycle spins this loop for ever instead of ending 32 hops
    // in — which is the one thing the count was genuinely buying.
    if (seen.has(priorId)) return current.prompt
    const prior = byId.get(priorId)
    if (prior === undefined) return current.prompt
    seen.add(priorId)
    current = prior
  }
  return current.prompt
}

/**
 * What to call a run the host started for one teammate to answer another.
 *
 * Its `prompt` is a briefing written to a runtime -- *"Wren (Code & Migrations)
 * sent you a message; it is quoted below... end with one <locust-share>
 * block"* -- and that sentence was appearing as the NAME of a mission in the
 * list, beside conversations a person had actually started. Colin, 2026-09-04:
 * *"missions are like projects or whole new conversations."*
 *
 * The message that caused the run is the readable thing, and the ledger holds
 * it, so the title says who wrote and what they said. Undefined when a person
 * started the run, and undefined when the host started one but the workroom no
 * longer holds the message -- because inventing a title is worse than showing
 * the briefing, which is at least true.
 */
/**
 * The line drawn above a turn's work, or nothing.
 *
 * The thread's own rule, written when handoffs were built: a run the HOST
 * briefed must never be drawn as a person's bubble, because that attributes
 * to them something they never said. A relayed run broke that rule --
 * Colin, 2026-09-05, screenshot: the whole machine briefing ("...end with one
 * <locust-share to="Wren"> block... Do not use a <locust-ask> block here...")
 * sat in the thread as the most prominent text on screen, in the place a
 * person's message goes.
 *
 * A person's words are drawn as they were typed. A host-briefed turn is drawn
 * by the message that caused it, when the record still holds that message,
 * and otherwise not at all -- the peer card beside it already says who wrote
 * to whom, so silence here loses nothing and inventing a sentence would.
 */
/**
 * The run a peer message was DELIVERED into, so a person reading one side of
 * an exchange can open the other.
 *
 * Colin's third point on 2026-09-04, the one still unbuilt: a teammate's
 * relayed run is filed under that teammate with a readable title, but the
 * thread that asked has no way to reach it -- you go looking in the sidebar.
 * The link is already in the record: the same `messageId` appears as
 * `posted` on the run that wrote it and as `received` on the run it was
 * quoted into.
 *
 * Undefined when nothing received it, which is a real state and not an
 * error: the message may still be waiting for that teammate's next run.
 */
export function peerRunFor(
  messageId: string,
  missions: readonly PublicRecoveredMission[]
): PublicRecoveredMission | undefined {
  if (messageId.length === 0) return undefined
  return missions.find((mission) =>
    mission.peerMessages.some((message) => message.messageId === messageId && message.direction === 'received')
  )
}

/**
 * Who started a run, as the window knows it: the ledger's own kinds plus a
 * person's post to a room, which the ledger does not record (a room post's
 * missions are ordinary missions of their teammates; the room remembers
 * which).
 */
export type LiveStarter =
  | PublicRecoveredMission['startedBy']
  | { readonly kind: 'room'; readonly roomId: string; readonly postId: string }
  | { readonly kind: 'tag' }
  // A comparison's column (0.441): the person's ask, put to several models at once.
  | { readonly kind: 'compare'; readonly compareId: string; readonly slot: string }
  // A comparison's judge (0.520): the host's question about the answers, never the person's.
  | { readonly kind: 'judge'; readonly compareId: string }
  // One of Scout's looks (0.756): its answer fills in the cards on Home, never a conversation.
  | { readonly kind: 'scout' }
  // A commit message written from the changes (0.771): its answer fills the Commit box, never a conversation.
  | { readonly kind: 'commit-draft' }
  // A walk through a change (0.771): its answer is the Changes panel's, never a conversation.
  | { readonly kind: 'walkthrough' }

/**
 * What the person ASKED FOR, when a button sent a brief in their name (A1.2).
 *
 * "Tidy up" and "Ask Wren for a review" each start an ordinary turn whose
 * prompt is a page of instructions the host wrote for the runtime -- the
 * example block, the rules, the whole review material -- and the bubble drew
 * it as though the person had typed it (the 0.317 tidy drive's screenshot).
 * The person pressed a button meaning one thing; that is what their bubble
 * says, the way Claude Code shows `/review` rather than the prompt behind it.
 * The runtime still gets every word.
 */
export function briefAskedFor(prompt: string): string | undefined {
  // Any version of the brief, including the fenced one sent before 0.372.
  if (isTidyPrompt(prompt)) return "Tidy this folder's team memory."
  const review = /^(.+?) finished a piece of work and you are reviewing it\. You did not do this work/.exec(prompt)
  return review === null ? undefined : `Review ${review[1]!}'s work.`
}

/**
 * A routine's fix turn (0.534, main/routine-runner.ts goalFixPrompt): Locust's words, quoting its check, sent as the
 * turn's whole prompt. Read back from them, since the record names only the routine and its step.
 */
const ROUTINE_FIX = /^The check `([^`\n]{1,500})` failed after the work above \(fix (\d{1,2}) of (\d{1,2})\)\. The end of its output:/

/**
 * Which fix of a failing check a turn is, when Locust sent it (0.812): a check-fix turn's own starter, or a routine's
 * fix turn read from its prompt. Undefined for every other turn.
 */
export function fixOfTurn(turn: { readonly prompt: string; readonly startedBy?: LiveStarter }): { readonly command: string; readonly fix: number; readonly of: number } | undefined {
  if (turn.startedBy?.kind === 'check-fix') return { command: turn.startedBy.command, fix: turn.startedBy.fix, of: turn.startedBy.of }
  if (turn.startedBy?.kind !== 'routine') return undefined
  const found = ROUTINE_FIX.exec(turn.prompt)
  return found === null ? undefined : { command: found[1]!, fix: Number(found[2]), of: Number(found[3]) }
}

export function turnPromptLine(turn: {
  readonly prompt: string
  readonly startedBy?: LiveStarter
  readonly peerMessages?: readonly PublicPeerMessage[]
}): string | undefined {
  // Attached files are named for the RUNTIME, in a line the host writes above
  // the message. It was reaching the person too -- their own bubble opened
  // "Read this file in the workspace before you answer: - NOTES.md", words
  // they never typed, in the style that says they said them. The files are
  // still shown, as rows under the bubble; this is only about whose voice
  // the sentence is in.
  const asked = briefAskedFor(turn.prompt)
  if (asked !== undefined) return asked
  // A tagged teammate's run is recorded as an ordinary one: the bubble is the
  // message, without the context that followed it (0.438, shared/tagging.ts).
  if (turn.startedBy === undefined) return splitAttachments(taggedWordsOf(turn.prompt)).text
  // Carried on after a plan ran out (0.774): nobody typed it, and the divider above it says what happened.
  // Sent back to fix the check (0.812), or a routine's fix of its own (0.534): nobody typed those either, and their
  // own line says so (CheckFixLine).
  if (turn.startedBy.kind === 'carry-on' || fixOfTurn(turn) !== undefined) return undefined
  // A routine step is the person's own words, saved from a conversation they
  // had; it is theirs to see, even though the host pressed go. A room post
  // is the person's own words too, said to several at once, and so is what
  // they typed in the runtime's own terminal (0.391).
  if (turn.startedBy.kind === 'mcp' || turn.startedBy.kind === 'routine' || turn.startedBy.kind === 'room' || turn.startedBy.kind === 'terminal' || turn.startedBy.kind === 'tag' || turn.startedBy.kind === 'compare') {
    // A handed-off routine step carries the answer before it; the bubble is the step (0.435).
    return splitAttachments(turn.startedBy.kind === 'routine' ? stepWordsOf(turn.prompt) : turn.prompt).text
  }
  // A relayed turn's prompt IS the message that started it, and the thread
  // already draws that message as a peer card, in time, before the work. The
  // bubble therefore repeated a teammate's words verbatim -- and drew them in
  // the person's own bubble style, which reads as the person having said it
  // (Colin, 2026-09-06). The card is the better of the two: it names who sent
  // it and opens their run. So the bubble stands down whenever the card is
  // there, and only speaks when it would otherwise be the sole account of what
  // started the turn.
  const relayed = relayedTitle({ startedBy: turn.startedBy, peerMessages: turn.peerMessages ?? [] })
  const drawnAsACard = (turn.peerMessages ?? []).some((message) => message.direction === 'received')
  return drawnAsACard ? undefined : relayed
}

/**
 * The files a turn was sent with, recovered from its prompt.
 *
 * Only for a turn the person started: a relayed or handed-off turn's prompt is
 * machine-written and nothing attached files to it.
 */
/**
 * How many lines of command output a row draws before it elides the middle.
 *
 * Eight and eight. It was 200 in a box 320px tall that scrolled inside a
 * thread that also scrolls, which put a 300px black rectangle showing "1"
 * through "19" above the two lines the teammate actually said -- stdout drawn
 * louder than the person's collaborator talking (design, 2026-09-08).
 *
 * Eight each end is "enough to see a command worked, and enough to see how it
 * failed": a vitest failure puts the assertion, the file and the counts in its
 * last six lines. Anything more is asked for, and asking for it prints ALL of
 * it rather than more of a bounded window.
 */
export const SHELL_OUTPUT_HEAD_LINES = 8
export const SHELL_OUTPUT_TAIL_LINES = 8
export const MAX_SHELL_OUTPUT_LINES = SHELL_OUTPUT_HEAD_LINES + SHELL_OUTPUT_TAIL_LINES

/**
 * Command output split for the screen, keeping BOTH ends.
 *
 * The same rule `boundedMessageText` follows and for the same reason: the
 * interesting part of a long output is as often the last line as the first --
 * an error, an exit summary, the answer -- and a head-only cut throws exactly
 * that away. `seq 1 1200` is the cheerful case; `npm install` ending in a
 * permission error is the one that matters.
 *
 * Head and tail are returned SEPARATELY rather than joined around a sentence,
 * because the elision is a control now: a button between the halves that
 * prints everything, the same pattern the diff's folded context already uses.
 * A sentence in the middle of a `pre` cannot be pressed.
 */
export function boundedShellOutput(output: string): {
  readonly head: string
  readonly tail: string
  readonly omitted: number
  readonly total: number
  /** Kept for callers that want one block: head, a marker line, tail. */
  readonly text: string
} {
  // Trailing whitespace goes first. Almost every command ends with a newline,
  // and keeping it made the last visible line BLANK -- which quietly undercuts
  // the one promise this function makes. Driven on a real `seq 1 300`: the
  // tail read as an empty line rather than `300`.
  const trimmed = output.replace(/\s+$/, '')
  const lines = trimmed.split('\n')
  if (lines.length <= MAX_SHELL_OUTPUT_LINES) {
    return { head: trimmed, tail: '', omitted: 0, total: lines.length, text: trimmed }
  }
  const omitted = lines.length - SHELL_OUTPUT_HEAD_LINES - SHELL_OUTPUT_TAIL_LINES
  const head = lines.slice(0, SHELL_OUTPUT_HEAD_LINES).join('\n')
  const tail = lines.slice(-SHELL_OUTPUT_TAIL_LINES).join('\n')
  return {
    head,
    tail,
    omitted,
    total: lines.length,
    text: `${head}\n… ${String(omitted)} more lines …\n${tail}`
  }
}

export function turnAttachments(turn: {
  readonly prompt: string
  readonly startedBy?: LiveStarter
}): readonly string[] {
  if (turn.startedBy !== undefined && turn.startedBy.kind !== 'routine' && turn.startedBy.kind !== 'room') {
    return []
  }
  return splitAttachments(turn.prompt).attachments
}

export function relayedTitle(mission: {
  readonly startedBy?: LiveStarter
  readonly peerMessages: PublicRecoveredMission['peerMessages']
}): string | undefined {
  if (mission.startedBy?.kind !== 'relay') return undefined
  const asked = mission.peerMessages.find((message) => message.direction === 'received')
  if (asked === undefined || asked.text === null) return undefined
  const said = asked.text.replace(/\s+/gu, ' ').trim()
  if (said.length === 0) return undefined
  return `${asked.from.name} asked: ${said}`
}

/**
 * The routes this person has actually run, newest first, as `runtime:model`.
 *
 * The picker puts these at the top. It is a claim the app can back -- these
 * missions are in the ledger -- where "popular" or "recommended" would be a
 * judgement nothing here measured. A model whose id encodes an effort is
 * counted under the id that ran, which is the one they would pick again.
 */
/**
 * OpenCode's free models that finished a run here, newest first (0.517):
 * what "Use a free model" and a new person's first route prefer over the
 * catalogue's first, which can be down for a day (`freeStartModel`).
 */
export function freeModelsThatAnswered(
  missions: readonly Pick<PublicRecoveredMission, 'runtime' | 'model' | 'phase' | 'lastUpdatedAt'>[]
): readonly string[] {
  const seen = new Map<string, number>()
  for (const mission of missions) {
    if (mission.runtime !== 'opencode' || !mission.model.endsWith('-free') || mission.phase !== 'completed') continue
    const at = Date.parse(mission.lastUpdatedAt)
    const stamp = Number.isFinite(at) ? at : 0
    if ((seen.get(mission.model) ?? -1) < stamp) seen.set(mission.model, stamp)
  }
  return [...seen.entries()].sort((left, right) => right[1] - left[1]).map(([model]) => model)
}

export function recentlyUsedRoutes(
  missions: readonly PublicRecoveredMission[]
): readonly string[] {
  const seen = new Map<string, number>()
  for (const mission of missions) {
    const key = `${mission.runtime}:${mission.model}`
    const at = Date.parse(mission.lastUpdatedAt)
    const stamp = Number.isFinite(at) ? at : 0
    const held = seen.get(key)
    if (held === undefined || stamp > held) seen.set(key, stamp)
  }
  return [...seen.entries()]
    .sort((left, right) => right[1] - left[1])
    .map(([key]) => key)
}

/**
 * The files a mission produced, for the Inspector's Artifacts tab.
 *
 * That tab has said "artifacts appear once a run can write" since it was
 * built, and then showed nothing on runs that wrote plenty -- a promise the
 * app never kept, found by dogfooding (Colin, 2026-09-07, and again in his own
 * report: "No file listing, download trigger, or artifact delivery logic is
 * attached to this tab yet"). What was missing was not the idea, it was this
 * list.
 *
 * The rows are derived from the SAME entries the activity fold counts, through
 * the same `relativePath` key, so the tab and the fold cannot report different
 * files for one run -- the failure mode that produced `2 files` over a one-line
 * change and is worth not repeating in a second place.
 *
 * Failed edits are left out. A tool that reported a failure did not produce an
 * artifact, and a list that offered to show someone a file that was never
 * written would be worse than an empty tab.
 */
export function producedFiles(
  details: readonly ActivityDetail[],
  workspacePath: string | undefined
): readonly { readonly path: string; readonly shown: string; readonly status: string | undefined }[] {
  const entries = activityEntries(details, workspacePath)
  const key = (path: string): string =>
    // No separator normalising here: `relativePath` already collapsed both
    // kinds to `/`. This step used to repeat it, with the same one-backslash
    // -short class as the flattener above -- which cost nothing, because
    // there was nothing left for it to do. Removed rather than fixed.
    relativePath(path, workspacePath).replace(/^\.\//, '').toLowerCase()
  const seen = new Set<string>()
  const rows: { path: string; shown: string; status: string | undefined }[] = []
  for (const entry of entries) {
    const path = entry.kind === 'file' ? entry.file.path : entry.kind === 'unreported' && entry.failed !== true ? entry.name : undefined
    if (path === undefined) continue
    const id = key(path)
    if (seen.has(id)) continue
    seen.add(id)
    rows.push({
      path,
      shown: relativePath(path, workspacePath),
      status: entry.kind === 'file' ? entry.file.status : undefined
    })
  }
  return rows
}

/**
 * When a mission last DID something, rather than when it started.
 *
 * grok-build keeps `last_progress_at` and documents it as the dashboard's
 * sort key. Sorting by start time puts a conversation that has been working
 * for an hour below one that opened five minutes ago and has been idle since
 * -- the list claims to show what is happening and shows what began most
 * recently instead.
 *
 * The last event's own timestamp, falling back to when the mission was made
 * for one that has no events yet. Never the clock: a list that reorders
 * itself because time passed would move under the reader.
 */
export function lastActivityAt(mission: {
  readonly createdAt: string
  readonly events: readonly NormalizedRuntimeEvent[]
}): string {
  const last = mission.events.at(-1)?.occurredAt
  return last !== undefined && last > mission.createdAt ? last : mission.createdAt
}

/**
 * Whether this run died before its runtime did anything at all.
 *
 * The precondition for offering to run it again. A run that never emitted
 * `run.started` never opened a session, never called a tool and never
 * touched the workspace -- so starting it again cannot repeat anything,
 * because there is nothing to repeat. Any run that DID start is excluded,
 * however early it failed: at that point the honest answer is the person's,
 * not the app's, because only they know whether the half it did matters.
 *
 * Measured 2026-09-14: `cursor-agent` rewrites its own config on startup and
 * on Windows that rename fails with EPERM while a second copy of it holds
 * the file. The mission ledger for one of these holds exactly two records --
 * `mission.created` and `run.failed` -- and nothing else. That is the shape
 * this recognises. Colin hit it, and the only way forward was to retype the
 * message, which is a chore the app can spare him without guessing.
 */
export function runtimeNeverStarted(events: readonly NormalizedRuntimeEvent[]): boolean {
  /*
   * H4 (the code review): ANY SIGN OF THE RUNTIME AT WORK MEANS IT STARTED.
   * This asked only whether `run.started` was there -- and OpenCode, Muse and
   * Antigravity never emit one, so a run of theirs that had edited files and
   * then failed was offered "Run it again" under "Nothing had started, so
   * running this again cannot repeat anything". Reproduced by the review on
   * the real OpenCode normalizer: edit:notes.txt in the fold, true here.
   * Now only a record of nothing but the failure and the host's own notes
   * counts as never started.
   */
  return events.every((event) => NEVER_STARTED_SHAPES.has(event.type) && event.runtimeThreadId === undefined)
}

/** Retrying through the composer must preserve whose turn this was. */
export function runAgainKind(turn: {
  readonly prompt: string
  readonly startedBy?: LiveStarter
  readonly handoff?: unknown
  readonly restoredMission?: { readonly continuesFrom?: { readonly reason: string } }
}): 'person' | 'relay' | 'host' {
  if (turn.startedBy?.kind === 'relay') return 'relay'
  if (turn.handoff !== undefined || turn.restoredMission?.continuesFrom?.reason === 'route-switch'
    || briefAskedFor(turn.prompt) !== undefined) return 'host'
  return turn.startedBy === undefined || turn.startedBy.kind === 'tag' || turn.startedBy.kind === 'terminal'
    ? 'person' : 'host'
}

/** A host briefing cannot be retried as a newly typed person message. */
export function runAgainOffer(turn: Parameters<typeof runAgainKind>[0] & {
  readonly phase: string
  readonly events: readonly NormalizedRuntimeEvent[]
}, running: boolean, retry: () => void): { readonly onRunAgain?: () => void; readonly runAgainNote?: string } {
  if (running || turn.phase !== 'failed' || !runtimeNeverStarted(turn.events) || turn.prompt.trim().length === 0) return {}
  const kind = runAgainKind(turn)
  if (kind === 'person') return { onRunAgain: retry }
  return { runAgainNote: kind === 'relay'
    ? "The message is still waiting. It will be answered on this teammate's next run."
    : 'Locust started this run. Start it again from its original control to keep its context.' }
}

/** How many events a live run keeps in memory; the record keeps them all. The history's window too (shared/event-window.ts). */
export const LIVE_EVENT_CAP = EVENT_WINDOW

/**
 * A live run's events, capped with its opening and newest work kept, as the
 * history projection keeps them. The first event still proves it started (H4).
 */
export function cappedLiveEvents<T>(events: readonly T[], cap = LIVE_EVENT_CAP): readonly T[] {
  return windowEvents(events, cap)
}

/** What a run that never reached its runtime can hold: its failure, and diagnostics. */
const NEVER_STARTED_SHAPES: ReadonlySet<string> = new Set(['run.failed', 'adapter.diagnostic'])

/**
 * Whether a run failed because the MODE refused a tool, rather than anything
 * going wrong.
 *
 * Sol's beta review, 2026-09-21, finding 3: in Ask mode, asked for a file,
 * the run ended on a red *"The run could not continue"* card offering **Run
 * it again** beside the sentence *"Nothing had started, so running this again
 * cannot repeat anything."* Running it again would refuse identically, for
 * the same correct reason, for ever. The boundary held exactly as designed
 * and the screen read as a breakage with a useless button on it.
 *
 * The one press that IS the next thing wanted -- switch the mode and rerun --
 * already exists as `onRunWithEdits`. It was gated on the run having produced
 * a reply carrying code, which a refused run never does, so the case that
 * most needs it was the one case that never got it.
 *
 * MATCHED ON THE SENTENCE, because the sentence is all the renderer is given:
 * `run.failed` carries a message, not a reason code. That is a drift risk,
 * and the control for it lives on the other side -- runtime-adapters'
 * `a-refusal-is-not-a-crash.test.ts` pins this exact phrase against the
 * message its normalizer builds, so the adapter cannot reword it without
 * failing there. Both halves name the phrase; neither can move alone.
 */
export function modeRefusedATool(error: string | undefined): boolean {
  if (error === undefined) return false
  return /^The mode this run is in does not allow (?:bash|edit|write|patch)\b/.test(error.trim())
}

// Moved to shared/tool-kinds.ts so the host reads a tool the same way (0.364).
export { editToolName, isEditCommand, isShellTool }

/**
 * WHO ASKED, when a helper did (helper visibility, 2026-10-05). Claude Code
 * routes a helper's permission request through the same host as the
 * teammate's, naming the call that asks; that call's row says whether a
 * helper made it, and the helper's launch says what kind of helper it is.
 * Undefined when the teammate asked itself, or the call is not in the record.
 *
 * "the Explore helper of Wren"; "a helper of Wren" when the runtime gave no
 * type; "the Explore helper of this teammate" when the name is not known.
 */
export function helperAskedBy(
  toolUseId: string | undefined,
  events: readonly NormalizedRuntimeEvent[],
  teammateName: string | undefined
): string | undefined {
  if (toolUseId === undefined) return undefined
  const call = events.find((event) => event.type === 'tool.started' && event.payload.itemId === toolUseId && byHelper(event.payload))
  if (call === undefined || call.type !== 'tool.started') return undefined
  const parent = (call.payload as { readonly parentItemId: string }).parentItemId
  let helperType: string | undefined
  for (const event of events) {
    if (event.type !== 'tool.started' && event.type !== 'tool.completed' && event.type !== 'tool.failed') continue
    if (event.payload.itemId !== parent || !SUBAGENT_TOOL.test(event.payload.name)) continue
    const status = event.payload.status
    if (typeof status === 'string' && status.length > 0 && !/^(error|completed|failed|in_progress|started|cancelled|refused)$/i.test(status)) helperType = status
  }
  const whose = teammateName === undefined || teammateName.trim().length === 0 ? 'this teammate' : teammateName.trim()
  return helperType === undefined ? `a helper of ${whose}` : `the ${helperType} helper of ${whose}`
}
