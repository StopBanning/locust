import { CheckFixLine } from './CheckFixLine.js'
import { EditCheckCard } from './EditCheckCard.js'
import { ThreadImagesContext } from '../threadImages.js'
import { FilesElsewhere, RanOnTheCloud } from '../filesElsewhere.js'
import type { EditCheckShown } from './EditCheckCard.js'
import { Fragment, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { ReactElement, ReactNode } from 'react'

import type { MissionRuntimeId, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { providerErrorSentence } from '@teammate/runtime-adapters/provider-error'

import type {
  MissionApprovalDecision,
  MissionApprovalRequest,
  PublicPeerMessage,
  PublicRecoveredMission,
  PublicTeammate
} from '../../../shared/ipc.js'
import type { TurnVersions } from '../missionView.js'
import type { MissionMode, RewindPutBackResponse } from '../../../shared/ipc.js'
import { activityEntries, buildThread, netFileEntries, cancellationSummary, editedFiles, decisionStanding, errorAlreadyShown, fixOfTurn, foldNoticeKeys, helperAskedBy, lastPlanOf, modeRefusedATool, pathInWorkspace, readPlan, relativePath, sentAgainBy, stepsLine, stoppedBeforeSaying, terminalSeamBefore, threadMarkers, threadPeerCards, turnAttachments, turnPromptLine, usageWindowLabel } from '../missionView.js'
import { handedFileOnDisk, isWholePath } from '../../../shared/handover.js'
import type { GroupBoundary, GroupLeaving, LiveStarter, TurnSwitch } from '../missionView.js'
import { parseAgentText } from '../agentText.js'
import { folderName, ranOnLine } from '../ranOn.js'
import { useFollowBottom } from '../useFollowBottom.js'
import { JumpToBottom } from './JumpToBottom.js'
import { ThreadFind } from './ThreadFind.js'
import { PromptRail } from './PromptRail.js'
import { promptRailLine } from '../promptRail.js'
import { CopyButton } from './CopyButton.js'
import { PublishedCards } from './PublishedCards.js'
import { TurnUndo } from './TurnUndo.js'
import { ledgerFailureRows, ledgerFailureSentence } from '../ledgerFailure.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import { liveActivityOf } from '../faceState.js'
import { costLabel, costLineOrWhyNot, runCostOf } from '../cost.js'
import type { FaceActivity } from '../faceState.js'
import { checkpointLabel, ledgerVerificationLabel, missionPhaseView, modeFacts, shortMissionId } from '../status.js'
import { ActivityCard } from './ActivityCard.js'
import { MemoryCard } from './MemoryCard.js'
import { memoriesOfTurn } from '../conversationMemories.js'
import type { MemoryCardLine } from './MemoryCard.js'
import { AttachedImage } from './AttachedImage.js'
import { isImagePath } from '../../../shared/image-files.js'
import { Icon } from './Icon.js'
import { ApprovalCard } from './ApprovalCard.js'
import { CancellationCard, stoppedBeforeAnyTool } from './CancellationCard.js'
import { SignInButton } from './SignInButton.js'
import { AgentAvatar, AgentText, DiagnosticLine, LinkedWords, LiveStepCard, PlanSteps } from './ThreadItems.js'
import { CloudChangeCard, CloudMarker } from './CloudTurn.js'
import { cloudFilesHere, type PublicCloudTurn } from '../../../shared/cloud-computer.js'
import { splitAttachments } from '../../../shared/attachments.js'
import { DecisionCard } from './DecisionCard.js'
import { ResumeCard } from './ResumeCard.js'
import { resumeOffer } from '../resume.js'
import { HandoffDivider } from './HandoffDivider.js'
import { TerminalDivider } from './TerminalDivider.js'
import { SentNotes } from './SentNotes.js'
import { splitDiffNotes } from '../diffNotes.js'
import type { DiffFile } from '../diff.js'
import { TimeMarker } from './TimeMarker.js'
import { PeerThread } from './PeerThread.js'
import { PdfTile } from './PdfTile.js'
import { isPdf } from '../pdfPages.js'
import type { ThreadItem, ThreadPeerCard } from '../missionView.js'
import type { DecisionOption } from '../../../shared/decision.js'
import { callPlacements, type CallPlace, type SavedCall } from '../../../shared/calls.js'
import type { VoiceCallRole } from '../../../shared/voice-call.js'
import { CallWords } from './CallWords.js'
import { InstructionsCard } from './InstructionsCard.js'
import { KeyCard } from './KeyCard.js'
import { KEYS_REACH, keyRequestsInEvents } from '../../../shared/teammate-keys.js'
import { instructionsIn, isInterview } from '../../../shared/interview.js'
import type { LiveCallAsk } from '../liveCall.js'

/**
 * The files a teammate handed over, and what the host said when a press
 * could not be honoured.
 *
 * Its own component because it holds state, and it holds state for the
 * reason every other control in this app that talks to the host does: the
 * answer has to be shown. `revealFile` returns `{ok, message}` for a path
 * outside the workspace or a file that is not there, and the first version of
 * this dropped it on the floor -- `void bridge.revealFile(...).catch(...)`.
 *
 * That is the shape this project keeps paying for. Grok found it on outbound
 * links (pass 2, finding 3): a refused press and a press that worked looked
 * identical, and nothing whatsoever appeared on screen. The handover drive
 * found the same thing here on 2026-09-20 -- a reply that said it had written
 * `notes/summary.md`, a card that drew a button for it, and no such file on
 * disk. A model can name a file it never wrote, so this card must be able to
 * say so.
 *
 * The refusal clears on the next press: a stale message beside a button that
 * now works is the same lie pointing the other way.
 */
function HandedFiles({
  files,
  workspacePath,
  onOpenFile
}: {
  readonly files: readonly { readonly path: string; readonly note?: string }[]
  readonly workspacePath: string | undefined
  /** Open it in the panel beside the conversation. Absent: the name reveals instead. */
  readonly onOpenFile?: (path: string, folder?: string) => void
}): ReactElement {
  const [refused, setRefused] = useState<string>()
  // Not in this folder (0.762, filesElsewhere.ts): named, with nothing offered that would act on this folder's file.
  const elsewhere = useContext(FilesElsewhere)
  const ask = (action: 'revealFile' | 'saveCopy', path: string): void => {
    const bridge = window.desktop
    setRefused(undefined)
    if (bridge === undefined || workspacePath === undefined) return
    void bridge[action](handedFileOnDisk(workspacePath, path))
      .then((answer) => setRefused(answer.ok ? undefined : answer.message))
      // The host never answered. Say what is still true: nothing moved.
      .catch(() => setRefused('Locust could not reach that file. Nothing was changed.'))
  }
  return (
    <div className="lc-handedfiles">
      {files.map((file) => elsewhere !== undefined ? (
        <span key={file.path} className="lc-handedfile is-elsewhere">
          <span className="lc-handedfile__open" title={file.note === undefined ? `${file.path}\n\n${elsewhere}` : `${file.note}\n\n${elsewhere}`}>
            <Icon name="file" size={12} />
            <span className="lc-handedfile__path">{file.path}</span>
            {file.note !== undefined && <span className="lc-handedfile__note">{file.note}</span>}
          </span>
        </span>
      ) : (
        <span key={file.path} className="lc-handedfile">
          <button
            type="button"
            className="lc-handedfile__open"
            /*
              * THE NOTE IS IN THE TOOLTIP, because the note is what truncates.
              *
              * The row is a pill and the note is the teammate's own words
              * about the file, so it ellipsises -- and the title said only
              * "Show <path> in the file manager", which left the cut-off half
              * unrecoverable. Colin saw exactly that on a real handover
              * (2026-09-20: "evening continuation: workroom/relay/q…").
              */
            title={(() => {
              const what = onOpenFile === undefined ? `Show ${file.path} in the file manager` : `Open ${file.path}`
              return file.note === undefined ? what : `${file.note}\n\n${what}`
            })()}
            onClick={() => (onOpenFile === undefined ? ask('revealFile', file.path) : onOpenFile(file.path))}
          >
            <Icon name="file" size={12} />
            {/* A whole path (0.787) reads as its file's name: its folders cut the name short in the pill. The title has all of it. */}
            <span className="lc-handedfile__path">{isWholePath(file.path) ? file.path.split('/').pop() : file.path}</span>
            {file.note !== undefined && <span className="lc-handedfile__note">{file.note}</span>}
          </button>
          {/*
            * SAVE A COPY, the way every other client offers a download
            * (Colin, 2026-09-20: "give it the little download icon ... in
            * case the user wants to easily move it to another folder").
            *
            * On a desktop app the file is already on disk, so this is a copy
            * to a place the person picks in a native save dialog rather than
            * a download. Reveal answers "where is it"; this answers "I want
            * it somewhere else". Neither one ever opens it.
            */}
          {workspacePath !== undefined && (
            <CopyButton
              className="lc-handedfile__save"
              label={file.path}
              text={async () => {
                const read = await window.desktop?.readTextFile(handedFileOnDisk(workspacePath, file.path))
                if (read === undefined) return { refused: 'Locust could not reach that file.' }
                if (!read.ok) return { refused: read.message }
                // A spreadsheet or a Word file has no text to copy as it is: saving a copy keeps it whole.
                if (read.mode === 'table' || read.mode === 'document') return { refused: 'Only a text file can be copied. Save a copy instead.' }
                return read.text
              }}
            />
          )}
          <button
            type="button"
            className="lc-handedfile__save"
            aria-label={`Save a copy of ${file.path}`}
            title="Save a copy…"
            onClick={() => ask('saveCopy', file.path)}
          >
            <Icon name="download" size={12} />
          </button>
        </span>
      ))}
      {refused !== undefined && (
        <span className="lc-handedfile__refusal" role="status">
          {refused}
        </span>
      )}
    </div>
  )
}

/**
 * One transcript's worth of items. Extracted so a handed-off mission can render
 * TWO of them -- what the first runtime did, the divider, then what the second
 * one did -- without either half being re-derived differently from the other.
 *
 * EXPORTED for the same reason, one step further: a room renders N of them,
 * one per teammate answering a post. Colin, 2026-09-12: "we already have this
 * exact same system we just need to be able to have it work with multiple in
 * one chat." The room used to draw its own little card showing a teammate's
 * LAST message and nothing else -- no thinking, no tool calls, no plan, and
 * none of the earlier messages of the same turn, which is what he saw going
 * missing. Rendering the real thing is both less code and more of the truth.
 */
export function ThreadItems({
  items,
  owner,
  activity,
  workspacePath,
  decision,
  onOpenFile,
  onDraft,
  planMode = false,
  faces = true,
  busyModel,
  onStop
}: {
  /** Stops the running turn from its live line's quiet note (0.758); only on the turn that runs. */
  readonly onStop?: () => void
  readonly items: readonly ThreadItem[]
  /**
   * Another model to go on with, when this turn's model is busy (C9): drawn
   * under the latest busy notice. Absent on earlier turns and on a turn that
   * is over, and whenever there is no other free model to offer.
   */
  readonly busyModel?: { readonly label: string; readonly onPress: () => void }
  /**
   * Draw the teammate's face beside what they said. Off where something
   * around these items already shows it -- a room's answer card has the face
   * in its own header, and drawing it again one row down was the same
   * teammate announced twice (design pass, 2026-09-22). The gutter stays, so
   * the text keeps its indent.
   */
  readonly faces?: boolean
  /** Open a handed file in the panel beside the conversation. */
  readonly onOpenFile?: (path: string, folder?: string) => void
  /** Words for the conversation's box, for the person to send (0.733: a status chip changed). */
  readonly onDraft?: (text: string) => void
  /** The turn was sent in Plan mode; see `Thread`'s prop of the same name. */
  readonly planMode?: boolean
  readonly owner: PublicTeammate | undefined
  /** What the live run is doing; only the working line draws it. */
  readonly activity: FaceActivity
  readonly workspacePath: string | undefined
  /** The folder now open, so the receipt names one only for its own missions. */
  readonly workspaceId?: string | undefined
  /** How to answer a question the run ended on. Absent on earlier turns. */
  readonly decision:
    | { readonly onChoose: (option: DecisionOption) => void; readonly busy: boolean; readonly standing: string }
    | undefined
}): ReactElement {
  /*
   * The orb the live line is showing, if this turn is still going.
   *
   * The plan's running step and the live line make the SAME claim — this is
   * what is happening now — so they show the same mark rather than two
   * opinions about one turn. Read off the items, which is the only place both
   * of them can agree by construction.
   */
  // The busy notice that gets the way on: the latest, so a second busy model
  // offers the one after it rather than the first offer again.
  const lastBusy = [...items].reverse().find((entry) => entry.type === 'diagnostic' && entry.busy === true)?.key
  const live = items.find((entry) => entry.type === 'live-step')
  const runningOrb = live !== undefined && live.type === 'live-step' ? live.orb : undefined
  /*
   * ONE FACE PER RUN OF SPEECH, not one per paragraph.
   *
   * A finished turn drew the teammate's face three times in a row down the
   * left edge -- once for "Making your HELLO file", once for "File created",
   * once for "DONE" (design pass, 2026-09-22). Colin, the same day: the face
   * is already one of "so many indicators". Consecutive things the teammate
   * SAID share the face of the first; anything between them -- a tool fold,
   * a plan with outcomes, the live line -- starts a new run.
   */
  /*
   * A turn's steps do not start a new run (0.491): they sit between the
   * things said, as Claude Code draws them, so one face heads the turn's
   * speech however many groups of steps come between.
   */
  const continuesSpeech = (index: number): boolean => {
    for (let at = index - 1; at >= 0; at -= 1) {
      const before = items[at]!
      if (before.type === 'agent-message' || before.type === 'plan') return true
      if (before.type !== 'steps' && before.type !== 'files') return false
    }
    return false
  }
  const face = (index: number): ReactElement =>
    !faces || continuesSpeech(index) ? <span className="lc-agentline__gutter" /> : <AgentAvatar teammate={owner} />
  // Copy sits under the turn's last reply once nothing is still going, as Claude Code's does (0.545).
  const lastSpeech = items.some((one) => one.type === 'live-step') ? -1 : items.map((one) => one.type).lastIndexOf('agent-message')
  const lastReply = lastSpeech < 0 ? undefined : items[lastSpeech]
  const replyText = lastReply?.type === 'agent-message' && lastReply.streaming !== true && lastReply.text.trim().length > 0 ? lastReply.text : undefined
  // A finished footer line takes Copy at its end (it measured 66px under DONE as its own row, 0.545).
  const footTakesCopy = items.some((one) => one.type === 'activity' && one.finished)
  return (
    <ThreadImagesContext.Provider value={{ folder: workspacePath, onOpenFile: onOpenFile === undefined ? undefined : (path) => onOpenFile(path, workspacePath), onDraft }}>
      {items.map((item, index) => {
        if (item.type === 'agent-message') {
          return (
            <div className="lc-agentline" key={item.key}>
              {face(index)}
              <div className="lc-agentline__body">
                <AgentText text={item.text} streaming={item.streaming === true} />
                {/* What it published elsewhere, as cards, once the reply is whole (0.727). */}
                {item.streaming !== true && <PublishedCards text={item.text} />}
                {index === lastSpeech && replyText !== undefined && !footTakesCopy && (
                  <div className="lc-agentline__tools">
                    <CopyButton className="lc-agentline__copy" label="this reply" text={item.text} />
                  </div>
                )}
              </div>
            </div>
          )
        }
        if (item.type === 'plan') {
          /*
           * A Plan-mode turn is the teammate's ANSWER, so it sits where their
           * prose sits: beside the face, no box, at reading size.
           *
           * It was an `lc-card` -- the register this app reserves for
           * something holding a control -- carrying a PLAN label, a
           * `0 of 5 done` counter and five identical dots. Nothing here holds
           * a control and nothing here is standing; it is the reply. Drawing
           * it as a box was the defect, and the census goes to thirteen by
           * deleting a species rather than adding one (design, 2026-09-09).
           *
           * The plan INSIDE a fold is untouched. One is a record of work, the
           * other is an answer, which is exactly why the same markup was wrong
           * in one of the two places.
           */
          return (
            <div className="lc-agentline" key={item.key}>
              {face(index)}
              <div className="lc-agentline__body">
                {/*
                  * OUTCOMES WHEN THE PLAN WAS ACTUALLY CARRIED OUT.
                  *
                  * This passed `false` unconditionally, which draws the
                  * plain numbered list above -- no markers, no `N of M
                  * done`. That is right for a Plan-MODE turn, where the plan
                  * IS the answer and nothing ran, and it is what the comment
                  * above describes. It was wrong for every other turn.
                  *
                  * Measured in Colin's own ledger, 2026-09-15: 29
                  * `plan.updated` events across his Cursor missions, and the
                  * last update of each one all `TODO_STATUS_COMPLETED`. So
                  * the app knew three of three steps were done and drew
                  * three identical lines that looked exactly like a plan
                  * nothing had happened to -- while the fold underneath
                  * quietly said "3 of 3 steps". "plans are still bugged and
                  * not showing in the UI" is the fair reading of that.
                  *
                  * `touchedNothing` was the distinction the code already
                  * made and already carried, so it decided this too. It is
                  * the wrong one on its own, and in the same way the
                  * "Plan mode -- nothing was changed" sentence beside it was
                  * wrong (Fable, pass 1, finding 4): a turn in ACCEPT EDITS
                  * that answers a question without editing anything also
                  * touched nothing, and its working to-do list was then
                  * drawn as though the plan were the deliverable -- ordinals,
                  * reading size, no PLAN header -- which lands in the middle
                  * of a conversation as a stray numbered line. Colin, seeing
                  * it in his own app, 2026-09-19: "the task/plan looked like
                  * it was showing up glitchy and not our usual ui".
                  *
                  * The plan is the ANSWER only when the person asked for a
                  * plan: Plan mode, and nothing done. Everything else is a
                  * list of work with states to report.
                  */}
                <PlanSteps
                  steps={item.steps}
                  doneCount={item.doneCount}
                  outcomes={!(planMode && item.touchedNothing === true)}
                  underway={runningOrb !== undefined}
                  finished={item.finished === true}
                  stopped={item.stopped === true}
                />
                {/*
                  * Derived, and true: this run changed nothing, and the mode is
                  * why. The same class of fact the trace line carries, said in
                  * the standing register.
                  *
                  * It points at the mode control on the composer rather than
                  * offering a "run this plan" button, because that button does
                  * not exist -- and the question behind it is what a turn IS,
                  * which is a product decision to answer before anything is
                  * drawn.
                  */}
                {planMode && item.steps.length > 0 && item.touchedNothing === true && (
                  <p className="lc-planmode">
                    Plan mode — nothing was changed. Switch the mode below and send again to have{' '}
                    {owner?.name ?? 'your teammate'} do it.
                  </p>
                )}
              </div>
            </div>
          )
        }
        if (item.type === 'files') {
          // Drawn below, from the host's own answer: see `HandedFiles`.
          /*
           * A file the teammate handed over, drawn as the mirror of a file
           * the PERSON attached: the same row of buttons, the same control,
           * the same answer to "where is it". Colin, 2026-09-19: "the user
           * should have the ability to receive files ... just like Claude",
           * sent with a screenshot where he asked for a file and got a path
           * as a sentence.
           *
           * REVEAL, NEVER OPEN. `window.desktop.revealFile` shows the file in
           * the file manager; the host refuses `shell.openPath` on purpose,
           * because opening would RUN a `.bat` or a `.ps1` the model had just
           * written. Do not add an Open button here.
           *
           * Indented to the body, not to the page: these belong to the reply
           * above them, which is why they are inside `lc-agentline`.
           */
          return (
            <div className="lc-agentline" key={item.key}>
              <span className="lc-agentline__gutter" />
              <div className="lc-agentline__body">
                <HandedFiles files={item.files} workspacePath={workspacePath} {...(onOpenFile === undefined ? {} : { onOpenFile })} />
              </div>
            </div>
          )
        }
        if (item.type === 'steps') {
          // Under the words, at their indent: the steps are part of the reply.
          const closedLine = stepsLine(item.details, item.finished, workspacePath)
          // Open, a lone thought's line drops its preview: the words are underneath (0.594).
          const openLine = stepsLine(item.details, item.finished, workspacePath, { open: true })
          return (
            <div className="lc-agentline" key={item.key}>
              <span className="lc-agentline__gutter" />
              <div className="lc-agentline__body">
                <ActivityCard
                  variant="steps"
                  summary=""
                  trace={closedLine.segments}
                  // The sentence in shorter forms, for a row too narrow for it (0.604).
                  traceShorter={closedLine.shorter}
                  traceTitle={closedLine.title}
                  traceOpen={openLine.segments}
                  traceOpenShorter={openLine.shorter}
                  traceOpenTitle={openLine.title}
                  finished={item.finished}
                  details={item.details}
                  runtimeName={undefined}
                  workspacePath={workspacePath}
                  // The group still growing showed its rows as they landed (0.584, `live`), and one the run had
                  // moved past folded back (0.594, `superseded`, still honoured for one opened by hand).
                  // Folded until pressed, the newest too, as Claude Code's groups are (0.610). Colin, 2026-10-04:
                  // "because that recent row stays open, it give it, its own look ... wont be familiar to claude users".
                  openByDefault={false}
                  fold={item.superseded === true}
                  {...(onOpenFile === undefined ? {} : { onOpenFile })}
                />
              </div>
            </div>
          )
        }
        if (item.type === 'activity') {
          // The foot is what the turn came to; a turn still going has not come to it.
          if (!item.finished) return null
          return (
            <TurnFoot
              key={item.key}
              item={item}
              workspacePath={workspacePath}
              {...(replyText === undefined ? {} : { replyText })}
              {...(onOpenFile === undefined ? {} : { onOpenFile })}
            />
          )
        }
        if (item.type === 'live-step') {
          return (
            <LiveStepCard
              key={item.key}
              label={item.label}
              {...(item.action === undefined ? {} : { action: item.action })}
              detail={item.detail}
              startedAt={item.startedAt}
              kind={item.kind}
              register={item.register}
              waiting={item.waiting ?? false}
              {...(item.orb === undefined ? {} : { orb: item.orb })}
              owner={owner}
              activity={activity}
              face={faces}
              {...(item.plan === undefined ? {} : { plan: item.plan })}
              {...(item.lastHeardAt === undefined ? {} : { lastHeardAt: item.lastHeardAt })}
              {...(onStop === undefined ? {} : { onStop })}
            />
          )
        }
        if (item.type === 'limit') {
          // Red is for a run that cannot continue. An approaching-limit
          // warning is not that: the mission ran fine, and a full-width red
          // card on every turn trains a person to ignore the colour that is
          // supposed to mean "stopped".
          // The reset instant is the ledger's; a person reads a local time.
          if (item.kind === 'temporary-rate-limit') {
            return <DiagnosticLine key={item.key} level="warning" message={usageWindowLabel(item.message)} />
          }
          return (
            <div className="lc-card is-terminal is-red" key={item.key}>
              <div className="lc-card__head">
                <span>Usage limit reached</span>
                <span className="lc-tag is-red">{item.kind}</span>
              </div>
              <div className="lc-card__body">{usageWindowLabel(item.message)}</div>
            </div>
          )
        }
        if (item.type === 'decision') {
          // Only where an answer can actually be given. buildThread already
          // withholds the item on earlier turns; this is the second half of
          // the same rule, so a card can never appear with no way to answer.
          if (decision === undefined) return null
          return (
            <DecisionCard
              key={item.key}
              request={item.request}
              teammateName={owner?.name}
              standing={decision.standing}
              busy={decision.busy}
              onChoose={decision.onChoose}
            />
          )
        }
        return (
          <DiagnosticLine
            key={item.key}
            level={item.level}
            message={item.message}
            {...(busyModel !== undefined && item.key === lastBusy ? { action: busyModel } : {})}
          />
        )
      })}
    </ThreadImagesContext.Provider>
  )
}

/**
 * THE FOOT OF A FINISHED TURN (0.491): the files it changed, as Claude Code
 * closes a turn with "Edited 2 files", and the line of totals that used to
 * head the fold -- how long, what ran, what went wrong. The steps themselves
 * are in the groups above it, where they happened.
 */
function TurnFoot({
  item,
  workspacePath,
  onOpenFile,
  replyText
}: {
  readonly item: Extract<ThreadItem, { type: 'activity' }>
  readonly workspacePath: string | undefined
  /** The turn's last reply, for the Copy at the end of this line (0.545). */
  readonly replyText?: string
  readonly onOpenFile?: (path: string) => void
}): ReactElement {
  // What changed on disk, not every edit call: one refused was no change.
  // Paths outside this folder (a scratch tree a resume wrote into) are not
  // this turn's files -- they inflated "Edited N files" beside a footer that
  // counted one file in the compare copy (2026-10-05). When EVERY named edit
  // is outside (a recording drawn against a stub folder), keep them: that is
  // all there is to show of the turn.
  const named = item.details.filter((detail) => detail.kind === 'edit' && detail.failed !== true)
  const inFolder = named.filter((detail) => pathInWorkspace(detail.name, workspacePath))
  const edits = inFolder.length > 0 ? inFolder : named
  // Counted over the rows the card draws (0.672): a step standing in for a file the host could not read
  // was one row under a heading that still counted the host's word for it as "1 more changed".
  const entries = netFileEntries(activityEntries(edits, workspacePath), workspacePath)
  const pathsOf = (seen: boolean): number => new Set(
    entries.flatMap((entry) =>
      entry.kind === 'file' && (entry.observed === true) === seen ? [relativePath(entry.file.path, workspacePath).toLowerCase()]
      : entry.kind === 'unreported' && (entry.observed === true) === seen ? [relativePath(entry.name, workspacePath).toLowerCase()]
      : []
    )
  ).size
  /*
   * WHAT THE RUN SAID IT CHANGED, apart from what only changed while it ran.
   *
   * The host looks at the folder after a run, and a file the runtime never
   * named is "seen on disk". In a folder another program also writes -- Colin
   * works in his `.claude`, where Claude Code keeps backups and file history
   * -- that was twenty files headed "Edited 20 files", as if the teammate had
   * written them (0.491).
   */
  const edited = pathsOf(false)
  const seen = pathsOf(true)
  const heading = edited > 0
    ? `Edited ${edited === 1 ? '1 file' : `${String(edited)} files`}${seen > 0 ? ` · ${String(seen)} more changed in the folder` : ''}`
    : `${seen === 1 ? '1 file' : `${String(seen)} files`} changed in the folder while it ran`
  const changed = edited + seen
  // The files card says how many; the line need not say it again.
  const trace = changed > 0 ? item.trace.filter((seg) => seg.key !== 'files') : item.trace
  const notices = item.notices ?? []
  return (
    <div className="lc-turnfoot">
      {changed > 0 && (
        <ActivityCard
          summary=""
          variant="files"
          oneRowPerFile
          trace={[{ key: 'edited', text: heading }]}
          finished
          details={edits}
          runtimeName={item.reportedBy === undefined ? undefined : runtimeDisplayName(item.reportedBy)}
          workspacePath={workspacePath}
          openByDefault
          {...(onOpenFile === undefined ? {} : { onOpenFile })}
        />
      )}
      <div className="lc-turnfoot__trace lc-trace">
        {trace.map((seg) => (
          <span className={`lc-trace__seg${seg.tone === undefined ? '' : ` is-${seg.tone}`}`} key={seg.key}>
            {seg.text}
          </span>
        ))}
        {replyText !== undefined && <CopyButton className="lc-agentline__copy" label="this reply" text={replyText} />}
      </div>
      {/* Undo a turn (0.674): offered only when Locust kept the folder from before it, and only once it is over. */}
      {item.finished && item.runId !== undefined && changed > 0 && <TurnUndo runId={item.runId} />}
      {/*
        * What the runtime said about this turn before its work began, under
        * the totals, each naming who said it. Muted whatever level the runtime
        * sent: nothing here asks the reader to act (RULINGS 2026-09-10).
        */}
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
  )
}

/**
 * The durable receipt. Every value is read from the recovered mission -- the
 * runtime it really ran on, the checkpoints really written, the actions the
 * ledger could not settle, and the ledger's own path. `verified` is a claim
 * about durability, so it is withheld whenever recovery reported an issue.
 */
function ReceiptCard({
  mission,
  workspacePath,
  workspaceId
}: {
  readonly mission: PublicRecoveredMission
  readonly workspacePath?: string | undefined
  readonly workspaceId?: string | undefined
}): ReactElement {
  const view = missionPhaseView(mission.phase, mission.integrityIssueCount > 0)
  const verification = ledgerVerificationLabel(mission.integrityIssueCount)
  const checkpoints = mission.checkpoints ?? []
  const last = checkpoints.at(-1)
  const unsettled = last?.unsettledActions ?? []
  const [open, setOpen] = useState(false)
  /*
   * A RECEIPT WITH NO NEWS IS A LINK, NOT A CARD.
   *
   * Under every reopened answer sat "DURABLE RECEIPT . COMPLETED . codex
   * 0.156.1 / account-default . no checkpoints . ledger verified" -- a boxed
   * card of engineering words telling the person nothing they needed, on the
   * conversations where nothing had gone wrong (first-impressions drive,
   * packaged 0.349; Colin 2026-09-26: Claude's UI "and beyond" is the bar).
   * Claude puts nothing there. The record still exists and still opens: a
   * quiet "Details" under the answer. A receipt that DOES carry news -- a
   * run that failed or was cut off, a ledger that could not be verified, an
   * action that never reported back -- keeps the card, at rest, as before.
   */
  const routine = view.tone === 'blue' && verification === 'record readable' && unsettled.length === 0
  if (routine && !open) {
    return (
      <div className="lc-receipt__quiet">
        <button type="button" className="lc-receipt__details" aria-expanded={false} onClick={() => setOpen(true)}>
          Details
          <Icon name="chevron-right" size={11} />
        </button>
      </div>
    )
  }
  return (
    <div className={`lc-card is-terminal is-${view.tone === 'blue' ? 'blue' : view.tone === 'red' ? 'red' : 'amber'}`}>
      <div className="lc-card__head">
        <span>
          <span className="lc-mono lc-rail__meta">DURABLE RECEIPT</span>{' '}
          <span className={`lc-tag is-${view.tone}`}>{view.tag}</span>
        </span>
        <span className="lc-rail__meta">restored from local ledger</span>
      </div>
      {/*
        * Summarised, not spread out. It is the app's proof of durability and
        * it should exist; it should not be OPEN, because nobody reads "Events:
        * 41 recorded" twice (design review, 2026-09-06). Same gesture as the
        * activity fold, so there is one way to open a detail in this app
        * rather than two.
        *
        * Two things stay outside the disclosure: the verification word, and
        * the unverified count when it is not zero. An action that started and
        * never reported an outcome is the one row here a person needs without
        * asking for it.
        */}
      <button
        type="button"
        className="lc-receipt__summary"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <span className="lc-mono lc-receipt__line">
          {mission.runtime} {mission.cliVersion ?? ''} / {mission.model}
          <span className="lc-separator">·</span>
          {checkpoints.length === 0 ? 'no checkpoints' : `${String(checkpoints.length)} checkpoints`}
          <span className="lc-separator">·</span>
          <span className={verification === 'record readable' ? 'lc-tone-green' : 'lc-tone-amber'}>{verification}</span>
          {unsettled.length > 0 && (
            <>
              <span className="lc-separator">·</span>
              <span className="lc-tone-amber">
                {unsettled.length === 1 ? '1 unverified' : `${String(unsettled.length)} unverified`}
              </span>
            </>
          )}
        </span>
        <Icon name={open ? 'chevron-down' : 'chevron-right'} size={12} />
      </button>
      {open && (
      <dl className="lc-receipt">
        <dt>Runtime</dt>
        <dd className="lc-mono">
          {mission.runtime} {mission.cliVersion ?? ''} · {mission.model}
        </dd>
        {/*
          * SCOPE, stated positively -- the design agent's ruling, 2026-09-11,
          * on whether to say what a run could not have checked. It should
          * not: choosing which absences matter is a judgement about what the
          * diff means, and without that judgement the list is true of every
          * run and becomes furniture. What IS knowable with certainty is that
          * a run happened on one machine in one environment, and that is
          * architecture rather than a guess.
          *
          * "Ran on Windows" tells a person who changed a path handler
          * everything a macOS warning would have, and tells a person who
          * changed a copy string nothing -- correctly.
          */}
        <dt>Ran on</dt>
        <dd className="lc-mono">
          {ranOnLine({
            platform: window.desktop?.platform ?? '',
            // Named only when this mission is one of the open folder's own: a
            // recovered mission records its workspace as an id, not a path,
            // so printing today's folder beside an older run would be a claim
            // nothing supports.
            ...(workspaceId !== undefined && mission.workspaceId === workspaceId
              ? { folder: folderName(workspacePath) ?? '' }
              : {})
          })}
        </dd>
        <dt>Run</dt>
        <dd className="lc-mono">{shortMissionId(mission.missionId)}</dd>
        <dt>Checkpoints</dt>
        <dd>
          {checkpoints.length === 0
            ? 'none written'
            : `${checkpoints.length} written · last ${checkpointLabel(last!.epoch)}`}
        </dd>
        {unsettled.length > 0 && (
          <>
            <dt>Unverified</dt>
            <dd className="lc-tone-amber">
              {unsettled.length === 1
                ? '1 action started and never reported an outcome'
                : `${unsettled.length} actions started and never reported an outcome`}
            </dd>
          </>
        )}
        <dt>Events</dt>
        <dd>
          {mission.eventCount} recorded{mission.eventsTruncated ? ' · window truncated for display' : ''}
        </dd>
        <dt>{costLabel(runCostOf(mission.events))}</dt>
        <dd className="lc-mono">{costLineOrWhyNot(runCostOf(mission.events))}</dd>
        <dt>Ledger</dt>
        <dd className={verification === 'record readable' ? 'lc-tone-green' : 'lc-tone-amber'}>{verification}</dd>
      </dl>
      )}
    </div>
  )
}

/** The thread's line for a turn another app started -- or a voice call's Codex handed over (0.739). */
export function otherAppMarker(mode: MissionMode | undefined, via?: 'voice'): string {
  const from = via === 'voice' ? 'Started from a voice call' : 'Started from another app'
  if (mode === undefined) return from
  if (mode === 'ask') return `${from} · Ask mode (read only)`
  return `${from} · ${modeFacts(mode).name}`
}

export interface ThreadProps {
  readonly prompt: string
  /**
   * YOUR CLOUD COMPUTER (0.761): the turns of this conversation that ran there, by mission, and what to do with a
   * change waiting there. Each answers with what went wrong, or undefined when it was done.
   */
  readonly cloud?: {
    readonly turns: ReadonlyMap<string, PublicCloudTurn>
    readonly onPatch: (missionId: string) => Promise<string | undefined>
    readonly onApply: (missionId: string) => Promise<string | undefined>
    readonly onDiscard: (missionId: string) => Promise<string | undefined>
    /** Sends the change waiting there to a pull request on GitHub (0.762). */
    readonly onPullRequest?: (missionId: string, message: string) => Promise<string | undefined>
    /** The conversation's teammate, named in a pull request's drafted message. */
    readonly teammate?: string
  }
  /** The folder now open, so the receipt names one only for its own missions. */
  readonly workspaceId?: string | undefined
  /** Who started the current turn; a host-briefed one is not the person's words. */
  readonly startedBy?: LiveStarter
  /** The mode this turn ran in, as its run recorded it: what the "another app" marker names (0.703). */
  readonly runMode?: MissionMode
  /** Open the run a peer message reached; undefined for one nothing received yet. */
  /** Open a handed file in the panel beside the conversation. */
  readonly onOpenFile?: (path: string) => void
  /** Words for the box, for the person to send (0.733). */
  readonly onDraft?: (text: string) => void
  /**
   * FIND (0.769, ThreadFind.tsx): Ctrl+F, counted -- each new number opens this conversation's find bar. Only the
   * conversation on screen is given it, so a thread in a side panel never opens a second bar.
   */
  readonly findRequest?: number
  readonly onOpenPeerRun: (messageId: string) => (() => void) | undefined
  /**
   * Earlier turns of the same conversation, oldest first, each with the words
   * the person typed for it. Empty for a first turn. They render above this
   * turn so the exchange reads as one, which is what it was.
   */
  readonly coldStart?: boolean
  /**
   * EDIT AN EARLIER MESSAGE (0.498, Claude Code's Esc Esc rewind). Offered on
   * each message the person typed, when nothing is running: its words go back
   * in the box, and sending starts the conversation again from there.
   */
  readonly onEditMessage?: (missionId: string, words: string) => void
  /** This turn started again from an edited message (0.498): said above it. */
  readonly rewound?: boolean
  /** What putting the later replies' files back did (0.502). */
  readonly putBack?: RewindPutBackResponse
  /** The other version of THIS turn, when it is an edit or was replaced by one (0.498). */
  readonly versions?: TurnVersions
  /** Open another version of an edited message: the newest turn of that branch. */
  readonly onOpenVersion?: (missionId: string) => void
  /** The folder missions run in; paths render relative to it. */
  readonly workspacePath?: string
  /** Present only when re-running with edits allowed is possible; see App. */
  readonly onRunWithEdits?: () => void
  /** Offered only where the runtime never started, so nothing can repeat. */
  readonly onRunAgain?: () => void
  /** A host-started turn cannot be repeated through the person's composer. */
  readonly runAgainNote?: string
  /** A stopped run's message, sent again; drawn only when no tool had run. */
  readonly onSendAgain?: () => void
  /**
   * A free model that gave up on its limit, whose provider is down, or that
   * OpenCode retired: another one, with the message handed back. `why` picks
   * the sentence over the button; absent, it is the limit's.
   */
  readonly limitModel?: { readonly label: string; readonly onPress: () => void; readonly why?: 'retired' | 'down' | 'limit' }
  /** The provider's servers were busy (0.511): carry the same conversation on. */
  readonly onContinueAfterBusy?: () => void
  /** The agent's plan ran out (0.774): carry the work on with another agent, briefed with what was done. */
  readonly carryOn?: { readonly label: string; readonly from: MissionRuntimeId; readonly busy: boolean; readonly refused?: string; readonly onPress: () => void }
  /** A run that failed because its runtime is signed out: that runtime's sign-in. */
  readonly signInRuntime?: MissionRuntimeId
  /** Open the conversation a received message was written in. */
  readonly onOpenSenderRun?: (missionId: string) => () => void
  /** Whether that run was asked to PLAN rather than do; the offer then reads as the build step. */
  readonly wasPlan?: boolean
  /**
   * How a person answers a question the run ended on. Absent when this thread
   * cannot take a next turn at all, which is what keeps a card from appearing
   * with no way to answer it.
   */
  readonly onAnswer?: (option: DecisionOption) => void
  /**
   * Go on with another free model when this turn's is busy (C9): App names
   * it and does the switch. Drawn only while the turn runs, under the latest
   * busy notice.
   */
  readonly busyModel?: { readonly label: string; readonly onPress: () => void }
  /** Stop the running turn: offered on its live line once it has gone quiet (0.758, quiet.ts). */
  readonly onStopRun?: () => void
  /**
   * Pick up an interrupted mission from its last checkpoint. Absent when this
   * thread cannot start a run at all, so the offer can never appear without a
   * way to accept it.
   */
  readonly onResume?: (epoch: number) => void
  /** M28: why the last Resume from checkpoint did not start, shown on the card. */
  readonly resumeRefusal?: string
  /**
   * What THIS run was permitted to do. Taken from the live run rather than
   * from `restoredMission`, which is set only for a mission recovered from
   * the ledger -- so reading it there made every live read-only run say it
   * could edit files, which is the one claim on the card that must not be
   * loose. Caught on screen 2026-09-05, not by a test.
   */
  readonly sandbox?: 'read-only' | 'workspace-write' | 'full-access'
  /**
   * The run was sent in Plan mode. The "Plan mode — nothing was changed"
   * sentence hangs on this, not on the shape of the reply: a run in Accept
   * edits that answered a question has steps and touched nothing too, and
   * was being told to switch modes two inches above a composer reading
   * Accept edits (Fable, pass 1, finding 4, with the ledger).
   */
  readonly planMode?: boolean
  readonly earlierTurns: readonly {
    readonly missionId: string
    readonly prompt: string
    readonly events: readonly NormalizedRuntimeEvent[]
    /** What that turn exchanged with peers. Drawn with the turn, not with the last one. */
    readonly peerMessages?: readonly PublicPeerMessage[]
    /** Who started it. A host-briefed turn is not drawn as the person's words. */
    readonly startedBy?: LiveStarter
    /** Set when that turn was a reply sent to another runtime: its divider goes above it. */
    readonly switchedFrom?: TurnSwitch
    /** Set when the person had that exchange in the runtime's own terminal (0.391). */
    readonly inTerminal?: MissionRuntimeId
    /** The other version of that turn, when a message there was edited (0.498). */
    readonly versions?: TurnVersions
  }[]
  /**
   * Where this conversation's group began briefing it, if it is in one with
   * instructions and the join was recorded. Drawn at that point in the
   * thread -- after the last turn that ran without them -- never at the top.
   */
  readonly groupBoundary?: GroupBoundary
  /**
   * Join lines for memberships that have ENDED, so the turns they briefed
   * still say so. Grok, three passes running: after leaving, only the stop
   * was marked. Drawn with the same note as the current join, above its own
   * leaving line.
   */
  readonly pastBoundaries?: readonly GroupBoundary[]
  /** Where earlier groups' words stopped briefing this conversation; drawn as the join line's mirror. */
  readonly groupLeavings?: readonly GroupLeaving[]
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly eventsTruncated?: boolean
  /** The live line's words while nothing has arrived (0.602): the start's phase. */
  readonly startingLabel?: string
  readonly running: boolean
  readonly restoredMission: PublicRecoveredMission | undefined
  /** The turn being shown, so its own memories are drawn under it. */
  readonly shownMissionId?: string
  /**
   * What was said on calls with this conversation's teammate (0.747), kept and live: drawn among the turns each
   * call started (callPlacements). A live one grows as you talk; an ended one folds to a line.
   */
  readonly calls?: readonly (SavedCall & { readonly live?: boolean; readonly partial?: Readonly<Record<VoiceCallRole, string>>; readonly asks?: readonly LiveCallAsk[] })[]
  readonly error: string | undefined
  readonly errorIsPersistence: boolean
  /** Where receipts are written; the ledger-failure card offers to open it. */
  readonly ledgerPath?: string
  /** Local wall-clock label for when the mission began. */
  readonly startedAt: string | undefined
  /** The same moment as an ISO string, for the waiting line's clock. */
  readonly startedAtIso?: string
  /** Approvals waiting on the user, oldest first. */
  readonly approvals: readonly MissionApprovalRequest[]
  readonly onDecide: (approvalId: string, decision: MissionApprovalDecision, reason?: string) => void
  /** A QUESTION's answers, keyed by question id. Distinct from `onAnswer`, which
   *  answers a decision block in the transcript -- different surface, different act. */
  readonly onAnswerQuestion: (approvalId: string, answers: Readonly<Record<string, readonly string[]>>) => void
  readonly decidingIds: readonly string[]
  /** True once the run has been stopped by the user. */
  readonly cancelled: boolean
  /**
   * Set when this run continues one that was stopped for a route switch. The
   * prior run's events are rendered above the divider so the transcript reads
   * as one piece of work, while the divider keeps the two runtimes' authorship
   * distinguishable -- which the durable record insists on.
   */
  /**
   * Set when this turn is a reply sent to another runtime than the turn
   * before it. The divider goes ABOVE the reply: the turns before it were
   * said on one runtime, this one and what follows on the other. (A running
   * mission handed over is `handoff`, drawn after the prompt instead.)
   */
  readonly switchedFrom?: TurnSwitch
  /** Set when THIS turn was had in the runtime's own terminal and brought back (0.391). */
  readonly inTerminal?: MissionRuntimeId
  /** Turns that came back from Claude Code's background sessions (W10): their divider says so, not "terminal". */
  readonly backgroundTurns?: ReadonlySet<string>
  readonly handoff:
    | {
        readonly from: MissionRuntimeId
        readonly to: MissionRuntimeId
        readonly fromModel?: string
        readonly toModel?: string
        readonly at: string | undefined
        readonly unsettledCount: number
        readonly omittedBriefing: readonly string[]
        readonly leftOutByYou?: readonly string[]
        readonly priorEvents: readonly NormalizedRuntimeEvent[]
      }
    | undefined
  /**
   * The workroom exchange around this mission. `self` is the teammate the
   * mission belongs to; `notices` are shares the host could not honour, said
   * in the thread rather than dropped.
   */
  /** A3.3: the person's check after this turn, and sending its findings back. */
  readonly editCheck?: EditCheckShown
  /** The person pressed Stop on this run; otherwise the card does not say "You". */
  readonly stoppedByPerson?: boolean
  readonly onSendEditCheck?: (text: string) => void
  /** An interview's instructions saved as the teammate's own (0.831): resolves with why not, or nothing. */
  readonly onSaveInstructions?: (teammateId: string, instructions: string) => Promise<string | undefined>
  readonly peers: {
    readonly self: PublicTeammate | undefined
    readonly teammates: readonly PublicTeammate[]
    readonly messages: readonly PublicPeerMessage[]
    readonly notices: readonly { readonly message: string; readonly level: 'info' | 'warning' }[]
    /** What this conversation taught the team, read from the memory list. */
    readonly memories?: readonly MemoryCardLine[]
  }
}

/**
 * HOW MUCH OF A LONG CONVERSATION IS PUT ON THE PAGE AT ONCE (0.573).
 *
 * Colin, 10/03: "hitching/lagging when clicking between two working
 * sessions". Measured on a copy of his ledger (`_tools/profile-real-switch.mjs`):
 * his Codex conversation is 1,538 messages and 1,032 step lines, 23,635
 * elements, every one of them built and laid out on every click into it --
 * 0.6-0.8 s with the window frozen, each time. Nobody reads 1,500 messages
 * at once: the conversation opens on its newest turns, up to this many rows,
 * and the older ones go on the page as the person scrolls up to them, the
 * way a chat app loads its history. The newest earlier turn is always shown,
 * however long; the turn in progress is never held back.
 */
export const EARLIER_ROWS_SHOWN = 400

/** The index of the oldest earlier turn on the page: the newest turns whose rows fit in `budget`, at least one. */
export function firstEarlierTurnShown(sizes: readonly number[], budget: number = EARLIER_ROWS_SHOWN): number {
  let rows = 0
  let first = sizes.length
  for (let index = sizes.length - 1; index >= 0; index -= 1) {
    const size = sizes[index] ?? 0
    if (first < sizes.length && rows + size > budget) break
    rows += size
    first = index
  }
  return first
}

export function Thread({
  prompt,
  findRequest,
  editCheck,
  onSendEditCheck,
  onSaveInstructions,
  stoppedByPerson = false,
  startedBy,
  runMode,
  onOpenFile,
  onDraft,
  onOpenPeerRun,
  earlierTurns,
  planMode = false,
  startingLabel,
  groupBoundary,
  pastBoundaries = [],
  groupLeavings = [],
  coldStart = false,
  onEditMessage,
  rewound = false,
  putBack,
  versions,
  onOpenVersion,
  onRunWithEdits,
  onRunAgain,
  runAgainNote,
  onSendAgain,
  limitModel,
  onContinueAfterBusy,
  carryOn,
  signInRuntime,
  onOpenSenderRun,
  wasPlan,
  onAnswer,
  busyModel,
  onStopRun,
onResume,
  resumeRefusal,
  sandbox,
  workspacePath,
  workspaceId,
  events,
  eventsTruncated,
  running,
  restoredMission,
  shownMissionId,
  calls,
  error: recordedError,
  errorIsPersistence,
  ledgerPath,
  startedAt,
  startedAtIso,
  approvals,
  onDecide,
  onAnswerQuestion,
  decidingIds,
  cancelled,
  switchedFrom,
  inTerminal,
  backgroundTurns,
  handoff,
  peers,
  cloud
}: ThreadProps): ReactElement {
  // Where a turn ran, under the person's words, and what became of its change, under its work (0.761).
  const cloudMarkerFor = (missionId: string | undefined): ReactElement | null => {
    const turn = missionId === undefined ? undefined : cloud?.turns.get(missionId)
    return turn === undefined ? null : <CloudMarker turn={turn} />
  }
  /**
   * What the person asked for the turns a change there holds (0.762), oldest first: the turn's own words, and those
   * of the earlier turns carried into it -- a pull request's message is drafted from them, as Commit's is.
   */
  const cloudAsksFor = (turn: PublicCloudTurn): readonly string[] => {
    const all = [...earlierTurns.map((one) => ({ missionId: one.missionId, prompt: one.prompt })), ...(shownMissionId === undefined ? [] : [{ missionId: shownMissionId, prompt }])]
    const upTo = all.findIndex((one) => one.missionId === turn.missionId)
    return all
      .slice(0, upTo + 1)
      .filter((one) => {
        const there = cloud?.turns.get(one.missionId)
        return one.missionId === turn.missionId || (there?.taskId === turn.taskId && there.state === 'carried' && there.outcome === undefined)
      })
      .map((one) => splitAttachments(one.prompt).text)
      .slice(-5)
  }
  const cloudCardFor = (missionId: string | undefined): ReactElement | null => {
    const turn = missionId === undefined ? undefined : cloud?.turns.get(missionId)
    if (turn === undefined || cloud === undefined) return null
    return (
      <CloudChangeCard
        key={`cloud-${turn.missionId}`}
        turn={turn}
        onPatch={cloud.onPatch}
        onApply={cloud.onApply}
        onDiscard={cloud.onDiscard}
        {...(cloud.onPullRequest === undefined ? {} : { onPullRequest: cloud.onPullRequest })}
        asks={turn.state === 'there' ? cloudAsksFor(turn) : []}
        {...(cloud.teammate === undefined ? {} : { teammate: cloud.teammate })}
      />
    )
  }
  /** Where a turn's files are, when not in this folder (filesElsewhere.ts): said on its rows' titles. */
  const elsewhereFor = (missionId: string | undefined): string | undefined => {
    const turn = missionId === undefined ? undefined : cloud?.turns.get(missionId)
    if (cloudFilesHere(turn)) return undefined
    const went = turn?.state === 'carried' ? turn.outcome : turn?.state
    return went === 'pr' ? 'In a pull request on GitHub, not in this folder'
      : went === 'discarded' ? 'Put back on your cloud computer, not in this folder'
        : 'On your cloud computer, not in this folder yet'
  }
  /** A turn's work, told whether it ran on the cloud computer and where its files are now (filesElsewhere.ts). */
  const inItsPlace = (missionId: string | undefined, work: ReactNode): ReactElement => (
    <RanOnTheCloud.Provider value={missionId !== undefined && cloud?.turns.has(missionId) === true}>
      <FilesElsewhere.Provider value={elsewhereFor(missionId)}>{work}</FilesElsewhere.Provider>
    </RanOnTheCloud.Provider>
  )
  const error = recordedError === undefined ? undefined : providerErrorSentence(recordedError, events[0]?.sourceAdapter ?? restoredMission?.runtime)
  // What the run was allowed, hoisted so EVERY turn can be told -- not just
  // the newest. `no files changed` existed only on the last turn, so scrolling
  // up in a conversation showed the silence the line exists to break.
  const mayEdit = sandbox !== undefined && sandbox !== 'read-only'
  /*
   * AN EARLIER TURN'S WORK IS BUILT ONCE, not on every render.
   *
   * It depends on that turn's events, whether this run may edit, the folder
   * and the teammate -- none of which move while somebody reads, or while a
   * teammate streams anywhere in the app. Rebuilding it anyway re-derived
   * every fold, diff and file row of the whole conversation on every commit:
   * measured on Colin's largest conversation (30 turns, 11.7k nodes) at 34 ms
   * a re-render with nothing changed, more than two frames (2026-09-22).
   *
   * The SAME element object comes back while its inputs hold, and React skips
   * a subtree whose element has not changed -- the folds keep their state,
   * because nothing about them was touched.
   *
   * In order, because each turn is told what the ones before it said: the
   * plan it started from (`carriedPlan`) and the notices already carried
   * (`saidBefore`).
   */
  // A turn on the cloud computer changed nothing HERE (0.761): what it changed is its card's to say, so the line
  // never says "no files changed" over a change waiting there.
  const cloudTurnIds = cloud?.turns
  const earlier = useMemo(() => {
    const said = new Set<string>()
    /** Each turn's rows, which is what decides how many turns go on the page (EARLIER_ROWS_SHOWN). */
    const sizes: number[] = []
    const elements = earlierTurns.map((turn, index) => {
      const built = buildThread(turn.events, {
        running: false,
        mayEdit: mayEdit && cloudTurnIds?.has(turn.missionId) !== true,
        carriedPlan: index === 0 ? [] : lastPlanOf(earlierTurns[index - 1]!.events),
        saidBefore: new Set(said),
        // Said for the latest turn and never for these, so a turn whose whole
        // reply was a message to a teammate read "ended without a reply ...
        // Sending it again usually works" the moment another turn followed
        // it -- over the message it had sent, inviting a second copy
        // (drive-brief-once, 2026-09-24, the same on 0.320).
        spokeToPeers: (turn.peerMessages ?? []).length > 0,
        ...(workspacePath === undefined ? {} : { workspacePath })
      })
      for (const key of foldNoticeKeys(built)) said.add(key)
      sizes.push(built.length)
      return (
        <ThreadItems
          items={built}
          owner={peers.self}
          activity="idle"
          workspacePath={workspacePath}
          decision={undefined}
        />
      )
    })
    return { elements, said, sizes }
  }, [earlierTurns, mayEdit, workspacePath, peers.self, cloudTurnIds])
  const earlierWork = earlier.elements
  // A call's words among the turns it started (0.747); opened by the person once it has ended.
  const callPlaces = useMemo(
    () => callPlacements(calls ?? [], [...earlierTurns.map((turn) => turn.missionId), ...(shownMissionId === undefined ? [] : [shownMissionId])]),
    [calls, earlierTurns, shownMissionId]
  )
  // The turns on screen, for a call's summary (0.755): a to-do among them is right here, with nothing to open.
  const turnsHere = useMemo(
    () => new Set([...earlierTurns.map((turn) => turn.missionId), ...(shownMissionId === undefined ? [] : [shownMissionId])]),
    [earlierTurns, shownMissionId]
  )
  const [openCalls, setOpenCalls] = useState<ReadonlySet<string>>(() => new Set())
  const callsAt = (place: CallPlace) => (callPlaces.get(place) ?? []).map((piece) => {
    const source = (calls ?? []).find((call) => call.callId === piece.call.callId)
    const id = piece.call.callId
    return <CallWords key={`${id}-${place}`} piece={{ ...piece, live: source?.live === true, ...(source?.partial === undefined ? {} : { partial: source.partial }), ...(source?.asks === undefined ? {} : { asks: source.asks }) }}
      teammate={peers.self} open={openCalls.has(id)} here={turnsHere}
      onToggle={() => setOpenCalls((was) => { const next = new Set(was); if (next.has(id)) next.delete(id); else next.add(id); return next })} />
  })
  /*
   * A LONG CONVERSATION OPENS ON ITS NEWEST TURNS (0.573). The rest wait
   * above, put on the page as the person scrolls up to them (or presses
   * "Show earlier turns"). See EARLIER_ROWS_SHOWN. Each "show earlier" adds
   * another budget's worth; opening another conversation starts over.
   */
  const shownIdentity = shownMissionId ?? restoredMission?.missionId ?? startedAtIso ?? ''
  // Who began the conversation: its first turn's starter (0.748), for the marker at its top. A first turn the person
  // typed has none, and that is the answer (0.750): 0.748 fell through to the newest turn's, a call's, and said so.
  const firstStarter = earlierTurns.length > 0 ? earlierTurns[0]?.startedBy : startedBy
  const [earlierBudget, setEarlierBudget] = useState({ identity: shownIdentity, rows: EARLIER_ROWS_SHOWN })
  let earlierRows = earlierBudget.rows
  if (earlierBudget.identity !== shownIdentity) {
    earlierRows = EARLIER_ROWS_SHOWN
    setEarlierBudget({ identity: shownIdentity, rows: EARLIER_ROWS_SHOWN })
  }
  const firstShown = firstEarlierTurnShown(earlier.sizes, earlierRows)
  const showEarlier = (): void => setEarlierBudget((current) => ({ identity: shownIdentity, rows: (current.identity === shownIdentity ? current.rows : EARLIER_ROWS_SHOWN) + EARLIER_ROWS_SHOWN }))
  // An actual eviction, not merely reaching the window's size.
  const trimmed = restoredMission?.eventsTruncated === true || eventsTruncated === true
  const items = buildThread(events, {
    running,
    ...(trimmed ? { trimmed: true } : {}),
    latestTurn: true,
    // What the turn before left the plan at: see `carriedPlan`.
    carriedPlan: lastPlanOf(earlierTurns.at(-1)?.events ?? []),
    saidBefore: earlier.said,
    awaitingDecision: approvals.length > 0,
    spokeToPeers: peers.messages.length > 0,
    mayEdit: mayEdit && (shownMissionId === undefined || cloud?.turns.has(shownMissionId) !== true),
    // Only the orb reads this: `solving` is the one of the four mapped states
    // no open tool can answer for, because planning is the turn's mode.
    planMode,
    ...(startingLabel === undefined ? {} : { startingLabel }),
    ...(workspacePath === undefined ? {} : { workspacePath }),
    ...(startedAtIso === undefined ? {} : { startedAt: startedAtIso })
  })
  /*
   * A read-only run whose answer carries code is where "run it again, with
   * edits allowed" is worth offering -- and that is ALL it is.
   *
   * The sentence beside it used to say the runtime "could not write to the
   * workspace", on the reasoning quoted here for years: that it wrote the
   * change and was not permitted to apply it. Grok measured what that costs
   * (2026-09-14, finding 2): a plain Ask-mode question -- give me a markdown
   * link, a fenced javascript block and a table -- answered correctly, with a
   * banner underneath saying the run could not write to the workspace.
   *
   * Nothing tried to write. The person asked for an example and was told the
   * app had attempted to edit their folder and failed. The offer is still
   * right; the claim about what happened was invented. It says what is true
   * of the mode now and makes no claim about an attempt.
   *
   * Asked of the parsed reply rather than the prose, so a stray backtick
   * cannot fake it.
   */
  /*
   * A run the MODE stopped is not a run that went wrong, and the two need
   * different offers. See `modeRefusedATool`.
   */
  const refusedByMode = modeRefusedATool(error)
  const answeredWithCode = items.some(
    (item) => item.type === 'agent-message' && parseAgentText(item.text).some((block) => block.kind === 'code')
  )
  // The current turn is the last in the sequence, so its own marker is the
  // one whose index is past every earlier turn.
  const markers = threadMarkers([...earlierTurns.map((turn) => turn.events), events])
  /*
   * The group's line, and its instructions on request -- shown AS THE
   * GROUP'S: read-only here, with the way to edit going to the group's own
   * header, so nobody edits shared text believing it is their own.
   */
  // Which join notes are opened to their words, by the moment they joined:
  // one per line, because two groups' words are two different things.
  const [viewingJoins, setViewingJoins] = useState<ReadonlySet<string>>(new Set())
  const joinNote = (boundary: GroupBoundary, index: number): ReactElement => {
    const open = viewingJoins.has(boundary.joinedAt)
    return (
      <div key={`join-${boundary.joinedAt}-${String(index)}`} className="lc-thread__note lc-thread__groupnote">
        <span>
          {boundary.groupName}&apos;s instructions brief every turn from here
          {' · '}
          <button
            type="button"
            className="lc-linkbutton"
            onClick={() =>
              setViewingJoins((held) => {
                const next = new Set(held)
                if (next.has(boundary.joinedAt)) next.delete(boundary.joinedAt)
                else next.add(boundary.joinedAt)
                return next
              })
            }
          >
            {open ? 'hide' : 'view'}
          </button>
        </span>
        {open && (
          <blockquote className="lc-thread__groupwords">
            {boundary.instructions}
            <span className="lc-thread__groupedit lc-mono">edit in Project settings</span>
          </blockquote>
        )}
      </div>
    )
  }
  // Every join line, oldest first, the current membership's last: it is the
  // one still in force, so it sits nearest the turns it governs.
  const joins: readonly GroupBoundary[] = [...pastBoundaries, ...(groupBoundary === undefined ? [] : [groupBoundary])]
  const joinNotes = (test: (beforeTurn: number) => boolean) =>
    joins.filter((boundary) => test(boundary.beforeTurn)).map((boundary, index) => joinNote(boundary, index))
  // The mirror line. A leave and a join at the same index are drawn in that
  // order, which is the order they happened in.
  const leavingNotes = (test: (beforeTurn: number) => boolean) =>
    groupLeavings.filter((leaving) => test(leaving.beforeTurn)).map((leaving, index) => (
      <div key={`left-${String(leaving.beforeTurn)}-${String(index)}`} className="lc-thread__note lc-thread__groupnote lc-thread__groupnote--left">
        <span>{leaving.groupName}&apos;s instructions no longer apply from here</span>
      </div>
    ))
  const currentMarker = markers.find((marker) => marker.beforeTurn === earlierTurns.length)
  // Every turn's exchange, not only the last one: the message a teammate SENT
  // was written on an earlier turn than the reply it drew, so a thread that
  // only drew the current turn showed the answer and never the question.
  // What the current turn is called: the person's words, or -- for a turn the
  // host briefed -- the message that caused it, or nothing at all.
  const currentLine = turnPromptLine({ prompt, startedBy, peerMessages: peers.messages })
  // The turn on screen as Locust's fix of a failing check (0.812), drawn as its line rather than a message.
  const currentFix = fixOfTurn({ prompt, ...(startedBy === undefined ? {} : { startedBy }) })
  const currentSeam = terminalSeamBefore(earlierTurns.at(-1)?.inTerminal, inTerminal)
  /*
   * The person's turn: what they typed, and under it the files they attached.
   *
   * The files used to be inside the bubble, because the host names them in a
   * line above the message for the runtime to act on and that whole string was
   * what the bubble drew. So the person read an instruction they had not
   * written, and the files -- the thing they actually did -- were a sentence
   * rather than something to look at or open.
   *
   * Rows, and the same reveal control the activity fold uses on a file a
   * teammate wrote: "where is it" is the question a file attracts, and it
   * should have the same answer wherever the file appears.
   */
  /*
   * `edited` is asked for only when the message carried review notes, and
   * only once its turn is over: the files that turn changed, which is what
   * each note is held against (SentNotes.tsx, 0.395).
   */
  const userTurn = (line: string | undefined, attached: readonly string[], edited?: () => readonly DiffFile[], editId?: string): ReactElement | undefined => {
    const split = line === undefined ? undefined : splitDiffNotes(line)
    const words = split === undefined || split.text.trim().length === 0 ? undefined : split.text
    const notes = split?.notes ?? []
    if (words === undefined && attached.length === 0 && notes.length === 0) return undefined
    /*
     * Inside the bubble, drawn outside its left edge (0.498): a wrapper would
     * break the column's `> .lc-bubble` spacing rules, and an icon with no
     * text leaves the bubble's words exactly what they were.
     */
    const edit = editId === undefined || onEditMessage === undefined || words === undefined || notes.length > 0 || attached.length > 0
      ? undefined
      : (
          <button
            type="button"
            className="lc-bubble__edit"
            aria-label="Edit this message"
            title="Edit this message and start again from here"
            onClick={() => onEditMessage(editId, words)}
          >
            <Icon name="pencil" size={13} />
          </button>
        )
    const marked = words === undefined ? '' : promptRailLine(words)
    return (
      <>
        {words !== undefined && (
          <div className="lc-bubble" {...(marked.length > 0 ? { 'data-prompt-mark': marked } : {})}>
            <LinkedWords text={words} />
            {edit}
          </div>
        )}
        {notes.length > 0 && <SentNotes notes={notes} edited={edited?.()} workspacePath={workspacePath} />}
        {attached.length > 0 && (
          <div className="lc-sentfiles">
            {attached.map((path) => isPdf(path) ? (
              // A PDF is a card with its first page, and opens in the viewer (0.714, PdfTile).
              <PdfTile
                key={path}
                path={path}
                folder={workspacePath}
                onOpen={() => {
                  if (onOpenFile !== undefined) {
                    onOpenFile(path)
                    return
                  }
                  const bridge = window.desktop
                  if (bridge === undefined || workspacePath === undefined) return
                  void bridge.revealFile(`${workspacePath}/${path}`).catch(() => undefined)
                }}
              />
            ) : (
              <button
                key={path}
                type="button"
                className="lc-sentfile"
                title={`Show ${path} in the file manager`}
                onClick={() => {
                  const bridge = window.desktop
                  if (bridge === undefined || workspacePath === undefined) return
                  void bridge.revealFile(`${workspacePath}/${path}`).catch(() => undefined)
                }}
              >
                {isImagePath(path) ? <AttachedImage path={path} /> : <Icon name="file" size={12} />}
                <span className="lc-sentfile__path">{path}</span>
              </button>
            ))}
          </div>
        )}
      </>
    )
  }
  /*
   * AN EDITED MESSAGE SAYS SO, AND WHERE THE OTHER VERSION IS (0.498). The
   * version before an edit is not gone -- it is the turns the edit set aside,
   * and a conversation reopened on either one names the other, as claude.ai
   * does with its arrows.
   */
  const versionLinks = (other: TurnVersions | undefined): ReactElement[] => {
    if (other === undefined || onOpenVersion === undefined) return []
    return [
      ...(other.before === undefined ? [] : [
        <button key="before" type="button" className="lc-thread__versionlink" onClick={() => onOpenVersion(other.before!)}>
          Show the version before
        </button>
      ]),
      ...(other.after === undefined ? [] : [
        <button key="after" type="button" className="lc-thread__versionlink" onClick={() => onOpenVersion(other.after!)}>
          Show the edited version
        </button>
      ])
    ]
  }
  const versionNote = (other: TurnVersions | undefined): ReactElement | null => {
    const links = versionLinks(other)
    if (links.length === 0) return null
    return (
      <div className="lc-thread__note">
        {other?.before !== undefined ? 'Edited.' : 'This message was edited later.'} {links}
      </div>
    )
  }
  const exchanges = threadPeerCards([...earlierTurns.map((turn) => turn.peerMessages ?? []), peers.messages])
  const peerCard = (card: ThreadPeerCard): ReactElement => (
    <PeerThread
      key={card.key}
      self={peers.self}
      peer={card.group.peer}
      messages={card.group.messages}
      teammates={peers.teammates}
      onOpenPeerRun={onOpenPeerRun}
      {...(onOpenSenderRun === undefined ? {} : { onOpenSenderRun })}
    />
  )
  const cardsFor = (turnIndex: number, placement: ThreadPeerCard['placement']): readonly ThreadPeerCard[] =>
    exchanges.filter((card) => card.turnIndex === turnIndex && card.placement === placement)
  // Planned-step count comes from the last plan the provider sent, so
  // "never started" is measured against what it said it would do.
  const plannedSteps = events
    .filter((event) => event.type === 'plan.updated')
    .map((event) => (event.type === 'plan.updated' ? readPlan(event.payload.plan).length : 0))
    .at(-1) ?? 0
  const stopped = cancelled ? cancellationSummary(events, plannedSteps, workspacePath) : undefined
  const stoppedAt = cancelled
    ? new Date(events.at(-1)?.occurredAt ?? Date.now()).toLocaleTimeString(undefined, {
        hour: '2-digit',
        minute: '2-digit'
      })
    : undefined
  // Follow the newest line while the person is at the bottom, and stop the
  // moment they scroll up to read something. See `stickToBottom.ts` for why
  // the "am I at the bottom" question has to be asked BEFORE the content
  // grows rather than after.
  // The whole of it -- following, the eased walk, and the way back down --
  // now lives in `useFollowBottom`, because the room and the peer thread need
  // the same behaviour and had none of it (Colin, 2026-09-13: "lets have all
  // chats auto scroll the same way").
  const follow = useFollowBottom()
  /** The turns themselves, which find searches -- never its own bar, which sits outside them. */
  const column = useRef<HTMLDivElement>(null)

  // A mission the person just opened starts at its newest line, wherever the
  // previous one had been left. Opening a conversation JUMPS -- walking would
  // make a person watch the whole of somebody else's finished answer scroll
  // past.
  useEffect(() => {
    follow.jumpNow()
    // Identity of the conversation on screen: the recovered mission when there
    // is one, else this run's start time. Either changes exactly when the
    // person opens a different conversation, which is the moment to jump.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [restoredMission?.missionId, startedAtIso])

  /*
   * Older turns going on the page above keep the reader where they were.
   * The browser's own scroll anchoring does not do it at the very top (the
   * real app, scrolled to 0: every older turn came in at once, one batch
   * after another, the view pinned to the top), so the distance from the
   * bottom is kept across the render and put back before paint. That also
   * puts the button back out of sight, so batches come one scroll at a time.
   */
  const keepFromBottom = useRef<number | undefined>(undefined)
  const showEarlierHere = (): void => {
    const box = follow.ref.current
    if (box !== null) keepFromBottom.current = box.scrollHeight - box.scrollTop
    showEarlier()
  }
  /** Every earlier turn on the page at once, for a find that should reach them all (0.769). */
  const showAllEarlier = (): void => {
    const box = follow.ref.current
    if (box !== null) keepFromBottom.current = box.scrollHeight - box.scrollTop
    setEarlierBudget({ identity: shownIdentity, rows: Number.MAX_SAFE_INTEGER })
  }
  useLayoutEffect(() => {
    const box = follow.ref.current
    const kept = keepFromBottom.current
    keepFromBottom.current = undefined
    if (box === null || kept === undefined) return
    box.scrollTop = box.scrollHeight - kept
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstShown])

  // Scrolled up to the window's top edge, the next older turns go on the page.
  const earlierButton = useRef<HTMLButtonElement | null>(null)
  useEffect(() => {
    const button = earlierButton.current
    const box = follow.ref.current
    if (button === null || box === null || typeof IntersectionObserver === 'undefined') return undefined
    const watch = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) showEarlierHere()
    }, { root: box, rootMargin: '600px 0px 0px 0px' })
    watch.observe(button)
    return () => watch.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstShown, shownIdentity])

  return (
    <div className="lc-thread" ref={follow.ref} onScroll={follow.onScroll}>
      <PromptRail scroller={follow.ref} column={column} />
      {findRequest !== undefined && <ThreadFind request={findRequest} root={column} scroller={follow.ref} earlier={{ count: firstShown, onShowAll: showAllEarlier }} />}
      <div className="lc-thread__column" ref={column}>
        {/*
          * The mission id is PROVENANCE, and provenance lives on the workroom
          * header band where the 0.20 pass put it -- the header already reads
          * "Mission · 78243d5d · <model> · restored from the local ledger".
          * Drawing it here as well said the same thing twice, thirty pixels
          * apart (design review, 2026-09-06).
          *
          * The START TIME is not on the header, so it stays. What is left is
          * a time marker, which the thread already has a species for, rather
          * than a species of its own.
          */}
        {startedAt !== undefined && (
          <div className="lc-thread__marker lc-mono">Started {startedAt}</div>
        )}
        {firstStarter?.kind === 'mcp' && (
          // Another app's turn runs in Ask unless the person turned on each
          // teammate's own mode (0.703); the marker names the run's own mode,
          // and claims none it does not know. A voice call's hand-off says so (0.739).
          // Only when the CONVERSATION began there (0.748): a turn a call asked for in a conversation the person
          // started is under its call's own line, and this marker read as if the whole conversation came from it.
          <div className="lc-thread__marker lc-mono">{otherAppMarker(runMode, firstStarter.via)}</div>
        )}

        {/*
          Every turn the person actually typed, in order. A handed-off run is
          launched with a machine-written briefing instead, and drawing THAT as
          a user bubble would attribute to them something they never said --
          which is why a handoff contributes no bubble of its own here.
        */}
        {firstShown > 0 && (
          <button type="button" className="lc-thread__earlier" ref={earlierButton} onClick={showEarlierHere}>
            {`Show ${String(firstShown)} earlier ${firstShown === 1 ? 'turn' : 'turns'}`}
          </button>
        )}
        {earlierTurns.map((turn, index) => {
          // Not on the page yet: above the window (firstEarlierTurnShown).
          if (index < firstShown) return null
          const marker = markers.find((candidate) => candidate.beforeTurn === index)
          const seam = terminalSeamBefore(earlierTurns[index - 1]?.inTerminal, turn.inTerminal)
          // Stopped before it said anything, then sent again: one message (0.496).
          const again = sentAgainBy(turn, earlierTurns[index + 1]?.prompt ?? prompt)
          return (
            <Fragment key={turn.missionId}>
              {marker !== undefined && (
                <TimeMarker at={marker.at} minutesIn={marker.minutesIn} elapsed={marker.elapsed} note={marker.note} {...(marker.day === undefined ? {} : { day: marker.day })} />
              )}
              {leavingNotes((beforeTurn) => beforeTurn === index)}
              {joinNotes((beforeTurn) => beforeTurn === index)}
              {turn.switchedFrom !== undefined && <HandoffDivider {...turn.switchedFrom} />}
              {seam !== undefined && <TerminalDivider seam={seam} {...(backgroundTurns?.has(turn.missionId) === true ? { background: true } : {})} />}
              {!again && versionNote(turn.versions)}
              {callsAt(`before:${turn.missionId}`)}
              {!again && userTurn(turnPromptLine(turn), turnAttachments(turn), () => editedFiles(turn.events, workspacePath), turn.startedBy === undefined && turn.inTerminal === undefined ? turn.missionId : undefined)}
              {(() => {
                const sentBack = fixOfTurn(turn)
                return sentBack === undefined ? null : <CheckFixLine {...sentBack} prompt={turn.prompt} {...(peers.self === undefined ? {} : { teammateName: peers.self.name })} />
              })()}
              {!again && cloudMarkerFor(turn.missionId)}
              {cardsFor(index, 'before-work').map(peerCard)}
              {!again && inItsPlace(turn.missionId, earlierWork[index])}
              {/* An earlier turn stopped before it said anything had nothing under it, as if still waiting (0.496; Claude Code: "Interrupted"). */}
              {!again && stoppedBeforeSaying(turn.events) && <div className="lc-thread__note">Stopped before it replied</div>}
              {cloudCardFor(turn.missionId)}
              {cardsFor(index, 'after-work').map(peerCard)}
              {/* What that turn taught the team, under that turn. */}
              <MemoryCard lines={memoriesOfTurn(peers.memories ?? [], turn.missionId)} />
              {callsAt(`after:${turn.missionId}`)}
            </Fragment>
          )
        })}

        {currentMarker !== undefined && (
          <TimeMarker at={currentMarker.at} minutesIn={currentMarker.minutesIn} elapsed={currentMarker.elapsed} note={currentMarker.note} {...(currentMarker.day === undefined ? {} : { day: currentMarker.day })} />
        )}
        {leavingNotes((beforeTurn) => beforeTurn === earlierTurns.length)}
        {joinNotes((beforeTurn) => beforeTurn === earlierTurns.length)}
        {rewound && (
          <div className="lc-thread__note">
            {putBack === undefined
              ? 'Started again from an edited message. The replies after it were set aside; files they changed are as they left them. '
              : (putBack.heldBack ?? []).length > 0
                /*
                 * None put back, because some could not be (0.512): all of
                 * them or none, so the project is never half put back.
                 */
                ? `Started again from an edited message. The replies after it were set aside; files they changed are as they left them. Nothing was put back: ${putBack.leftAlone.map((file) => `${file.path} (${file.why})`).join('; ')} could not be, and putting back only the other ${String((putBack.heldBack ?? []).length)} could leave the project half changed. `
                : `Started again from an edited message. The replies after it were set aside${putBack.putBack.length === 0 ? '' : `, and ${putBack.putBack.length === 1 ? '1 file they changed was' : `${String(putBack.putBack.length)} files they changed were`} put back`}. `}
            {putBack !== undefined && putBack.leftAlone.length > 0 && (putBack.heldBack ?? []).length === 0 && (
              <>{`Left as ${putBack.leftAlone.length === 1 ? 'it is' : 'they are'}: ${putBack.leftAlone.map((file) => `${file.path} (${file.why})`).join('; ')}. `}</>
            )}
            {versionLinks(versions)}
          </div>
        )}
        {!rewound && versionNote(versions)}
        {!rewound && coldStart && earlierTurns.some((turn, index) => !sentAgainBy(turn, earlierTurns[index + 1]?.prompt ?? prompt)) && (
          /*
           * WHERE THIS TURN BEGAN, said where the turn begins.
           *
           * Said plainly because the alternative is a person assuming the
           * model read the turn above it. The conversation is one thread; the
           * runtime's memory of it is not.
           *
           * It used to render at the very END of the thread -- after the
           * approvals, hard against the composer, which is where LIVE and
           * PENDING things live. Colin, 2026-09-14, looking at one: "is that
           * alert at the bottom about memory bc of the update, also its not
           * disappearing". Both readings were the placement's fault. It is
           * not an alert and there is nothing to dismiss: it is a durable
           * fact about where this turn started, and it belongs beside the
           * time marker that opens the turn, above the work it describes.
           */
          <div className="lc-thread__note">
            {/* 0.495: said as what happens. "Started without the earlier messages" read
                as the history gone (Grok's 0.489 pass); the teammate is now told it. */}
            A fresh session: the turn before could not be picked up again, so the teammate was given the conversation so far
          </div>
        )}

        {/*
          The seam of a reply sent to another runtime, above the reply. The
          window had no word of the switch until 0.310, so the thread ran on
          as if one runtime had answered both turns.
        */}
        {switchedFrom !== undefined && <HandoffDivider {...switchedFrom} />}
        {currentSeam !== undefined && <TerminalDivider seam={currentSeam} {...(shownMissionId !== undefined && backgroundTurns?.has(shownMissionId) === true ? { background: true } : {})} />}
        {shownMissionId !== undefined && callsAt(`before:${shownMissionId}`)}
        {userTurn(currentLine, turnAttachments({ prompt, ...(startedBy === undefined ? {} : { startedBy }) }), running ? undefined : () => editedFiles(events, workspacePath), !running && startedBy === undefined && inTerminal === undefined ? shownMissionId : undefined)}
        {currentFix !== undefined && <CheckFixLine {...currentFix} prompt={prompt} {...(peers.self === undefined ? {} : { teammateName: peers.self.name })} />}
        {cloudMarkerFor(shownMissionId)}

        {handoff !== undefined && (
          <>
            <ThreadItems items={buildThread(handoff.priorEvents, { running: false, mayEdit, ...(workspacePath === undefined ? {} : { workspacePath }) })} owner={peers.self} activity="idle" workspacePath={workspacePath} decision={undefined} />
            <HandoffDivider
              from={handoff.from}
              to={handoff.to}
              {...(handoff.fromModel === undefined ? {} : { fromModel: handoff.fromModel })}
              {...(handoff.toModel === undefined ? {} : { toModel: handoff.toModel })}
              at={handoff.at}
              unsettledCount={handoff.unsettledCount}
              omittedBriefing={handoff.omittedBriefing}
              leftOutByYou={handoff.leftOutByYou ?? []}
            />
          </>
        )}

        {/*
          Exchanges that were delivered to this run sit where they were in
          time: before the work. Ones this run only sent follow the work.
        */}
        {cardsFor(earlierTurns.length, 'before-work').map(peerCard)}

        {inItsPlace(shownMissionId, (
        <ThreadItems
          /*
           * Keyed by the turn's mission (code review B4, renderer-thread (b)).
           * The window draws one Thread and swaps what it shows, so without a
           * key this list -- and the activity card inside it, open, toggled,
           * "show all" -- was the SAME component from one conversation to the
           * next, and a card opened in one arrived open in another.
           */
          key={restoredMission?.missionId ?? events.find((event) => event.missionId !== undefined)?.missionId ?? 'starting'}
          {...(onOpenFile === undefined ? {} : { onOpenFile })}
          {...(onDraft === undefined ? {} : { onDraft })}
          items={items}
          owner={peers.self}
          activity={liveActivityOf(events, running)}
          workspacePath={workspacePath}
          planMode={planMode}
          decision={
            onAnswer === undefined
              ? undefined
              : { onChoose: onAnswer, busy: running, standing: decisionStanding({ sandbox: sandbox ?? restoredMission?.sandbox, events }) }
          }
          {...(busyModel === undefined || !running ? {} : { busyModel })}
          {...(onStopRun === undefined || !running ? {} : { onStop: onStopRun })}
        />
        ))}

        {!running && cloudCardFor(shownMissionId)}
        {!running && onSaveInstructions !== undefined && peers.self !== undefined && isInterview(earlierTurns[0]?.prompt ?? prompt) && (() => {
          const written = instructionsIn(events)
          const self = peers.self
          return written === undefined ? null : (
            <InstructionsCard key={`instructions_${shownMissionId ?? ''}`} name={self.name} written={written}
              {...(self.instructions === undefined ? {} : { current: self.instructions })}
              onSave={(text) => onSaveInstructions(self.teammateId, text)} />
          )
        })()}
        {/* A key the teammate asked for (0.833): its name, a masked box, and Keep, under the answer that asked. */}
        {!running && peers.self !== undefined && (() => {
          const requests = keyRequestsInEvents(events)
          const self = peers.self
          const runtime = self.route?.runtime
          return requests.length === 0 ? null : (
            <KeyCard key={`keys_${shownMissionId ?? ''}`} teammateId={self.teammateId} name={self.name} requests={requests} reaches={runtime !== undefined && KEYS_REACH.has(runtime)} />
          )
        })()}
        {cardsFor(earlierTurns.length, 'after-work').map(peerCard)}
        {/* The turn's own check first, then what came after it -- a routine's "finished" reads as its end (0.814). */}
        {editCheck !== undefined && (
          <EditCheckCard
            check={editCheck}
            teammateName={peers.self?.name}
            {...(currentFix === undefined ? {} : { fix: currentFix.fix })}
            {...(onSendEditCheck === undefined ? {} : { onSend: onSendEditCheck })}
          />
        )}
        {peers.notices.map((notice, index) => (
          <DiagnosticLine key={`peer_notice_${index}`} level={notice.level} message={notice.message} />
        ))}
        {/*
          * The CURRENT turn's memories, under the current turn -- and the
          * card is per turn now, not one at the foot of the conversation.
          * A memory learned on turn one used to be drawn under turn five,
          * which read as something the last reply had just done (Colin,
          * 2026-09-11).
          */}
        <MemoryCard lines={memoriesOfTurn(peers.memories ?? [], shownMissionId)} />
        {shownMissionId !== undefined && callsAt(`after:${shownMissionId}`)}
        {callsAt('end')}
        {/* Joined after the last turn started: the NEXT turn is the first briefed, so the line sits below this one. */}
        {leavingNotes((beforeTurn) => beforeTurn > earlierTurns.length)}
        {joinNotes((beforeTurn) => beforeTurn > earlierTurns.length)}

        {/*
          Approvals sit at the END of the thread, after everything that has
          happened. They are what the run is waiting on, so they belong where
          the reader's eye already is rather than buried in the transcript.
        */}
        {approvals.map((request) => (
          <ApprovalCard
            key={request.approvalId}
            request={request}
            {...(() => {
              // A helper's request names the helper (helper visibility).
              const askedBy = helperAskedBy(request.toolUseId, events, peers.self?.name)
              return askedBy === undefined ? {} : { askedBy }
            })()}
            busy={decidingIds.includes(request.approvalId)}
            onDecide={(decision, reason) => onDecide(request.approvalId, decision, reason)}
            onAnswer={(answers) => onAnswerQuestion(request.approvalId, answers)}
          />
        ))}

        {stopped !== undefined && <CancellationCard summary={stopped} stoppedAt={stoppedAt} byPerson={stoppedByPerson} />}
        {stopped !== undefined && stoppedBeforeAnyTool(stopped) && onSendAgain !== undefined && (
          <div className="lc-rerun">
            {/*
              * Said once (0.696): on OpenCode the card above already says to check the folder, and this
              * line said it again in other words, stacked under it (the 0.695 sweep's stop drive).
              */}
            {stopped.toolsReportedWhenDone !== true && <span>No tool had run, so sending it again cannot repeat anything.</span>}
            <button type="button" className="lc-button" onClick={onSendAgain}>
              <Icon name="play" size={13} /> Send again
            </button>
          </div>
        )}

        {/*
          * A MODE REFUSAL IS ONE CARD, NOT TWO (beta review of 0.255.0, #8).
          *
          * It drew a "Run again with edits allowed" line and then, under it,
          * a red "The run could not continue" card that said the same thing
          * again and ended on the runtime's raw `permission requested: bash
          * ... auto-rejecting`. A safety boundary doing its job read as two
          * failures, with the remedy above the problem. The refusal below
          * now carries the runtime's own words in a fold instead.
          */}
        {error !== undefined && !refusedByMode && !errorAlreadyShown(items, error) && (
          errorIsPersistence ? (
            /*
              The one moment the product's central claim breaks, drawn rather
              than shrugged at (design pass section: "a persistence-failure
              state"). It used to title itself "Mission held" and then say in
              the next sentence that the run had been stopped -- and stopped is
              what the host does. Both halves matter, so the card says which
              happened, splits what is safe from what is at risk, and offers
              the folder.
            */
            <div className="lc-card is-terminal is-red">
              <div className="lc-card__head">
                <span>
                  {/* The title leads with what HAPPENED, not with the fault.
                      "Can't write the mission ledger" describes the app's
                      problem; "Stopped" is the fact about the person's run
                      (design, 2026-09-08). */}
                  <Icon name="shield" size={13} /> Stopped — the mission ledger could not be written
                </span>
              </div>
              <div className="lc-card__body">
                <p className="lc-ledgerfail__why">{ledgerFailureSentence(error)}</p>
                <dl className="lc-ledgerfail">
                  {/*
                    * Nothing written at all: no recovered record AND no events
                    * on screen. A mid-run failure has events either way, and a
                    * failure to CREATE the mission has neither -- which is the
                    * case where "this mission can be reopened" is false.
                    */}
                  {ledgerFailureRows(
                    restoredMission?.checkpoints?.length,
                    restoredMission === undefined && events.length === 0
                  ).map((row) => (
                    <Fragment key={row.label}>
                      <dt className={`lc-ledgerfail__label is-${row.tone}`}>{row.label}</dt>
                      <dd className="lc-ledgerfail__text">{row.text}</dd>
                    </Fragment>
                  ))}
                </dl>
                {/*
                  * One action, and the path beside it.
                  *
                  * The design draws two -- Retry and "Change where the ledger
                  * lives" -- and neither exists to be wired. The ledger
                  * directory is a fixed `join(userData, 'mission-ledger')`
                  * with no setting behind it, so changing it is a feature; and
                  * the run is already dead, so retrying is what ResumeCard
                  * does. Drawing either would be a dead control, which is the
                  * rule this app has paid for more than once and the same
                  * discipline the design praised on the slash menu.
                  *
                  * The path is shown regardless, which is the half of that
                  * idea that costs nothing: someone told their ledger cannot
                  * be written and not told where it lives has been informed
                  * and not helped.
                  */}
                {ledgerPath !== undefined && ledgerPath.length > 0 && (
                  <div className="lc-ledgerfail__actions">
                    <button
                      type="button"
                      className="lc-button"
                      onClick={() => {
                        const bridge = window.desktop
                        if (bridge === undefined) return
                        void bridge.revealFile(ledgerPath).catch(() => undefined)
                      }}
                    >
                      Show the ledger folder
                    </button>
                    <span className="lc-ledgerfail__path lc-mono">{ledgerPath}</span>
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="lc-card is-terminal is-red">
              <div className="lc-card__head">
                <span>
                  <Icon name="shield" size={13} /> The run could not continue
                </span>
              </div>
              <div className="lc-card__body">{error}</div>
            </div>
          )
        )}

        {/* After the card that says what happened: the problem, then the remedy. */}
        {onRunWithEdits !== undefined && (wasPlan === true || answeredWithCode || refusedByMode) && (
          // Deliberately not an error: the run did exactly what its mode
          // allows. This is the one click that would otherwise be a mode
          // change and a retyped prompt. A plan run says so in its own
          // words -- the point of planning is that carrying it out is the
          // next, separate decision.
          <div className="lc-rerun">
            <span>
              {wasPlan === true
                ? 'This is the plan, not the work: nothing in the workspace has changed.'
                : refusedByMode
                  ? 'Ask mode does not change files, so this run stopped rather than write one. Nothing in the workspace has changed.'
                  : 'Ask mode answers in the conversation, so this stayed in the reply. Nothing in the workspace has changed.'}
            </span>
            <button type="button" className="lc-button" onClick={onRunWithEdits}>
              <Icon name="diff" size={13} /> {wasPlan === true ? 'Build this plan' : 'Run again with edits allowed'}
            </button>
            {refusedByMode && error !== undefined && (
              <details className="lc-rerun__said">
                <summary>What the runtime said</summary>
                <p>{error}</p>
              </details>
            )}
          </div>
        )}

        {/*
          * NOT offered when the mode is what refused it. Sol's beta finding
          * 3: pressing this would hit the same boundary, correctly, for
          * ever -- and the sentence beside it ("running this again cannot
          * repeat anything") is a button admitting it does nothing. The
          * offer that belongs there is the mode switch above.
          */}
        {limitModel !== undefined && error !== undefined && (
          <div className="lc-rerun">
            <span>
              {limitModel.why === 'retired'
                ? 'OpenCode no longer offers this model.'
                : limitModel.why === 'down'
                  ? 'This model’s provider is down for now.'
                  : 'This model is at its limit.'}{' '}
              Switching puts your message back in the chat box, to send there.
            </span>
            <button type="button" className="lc-button" onClick={limitModel.onPress}>
              {limitModel.label}
            </button>
          </div>
        )}

        {onContinueAfterBusy !== undefined && error !== undefined && (
          <div className="lc-rerun">
            <span>The model's servers were busy: not your account, and nothing you did. What it finished is kept; Continue picks up where it stopped.</span>
            <button type="button" className="lc-button" onClick={onContinueAfterBusy}>
              <Icon name="play" size={13} /> Continue
            </button>
          </div>
        )}

        {carryOn !== undefined && error !== undefined && (
          <div className="lc-rerun" data-carry-on="">
            <span>
              {`${runtimeDisplayName(carryOn.from)} ran out of its plan. What it did is kept, and the next agent is told what was done and what is left.`}
            </span>
            <button type="button" className="lc-button" onClick={carryOn.onPress} disabled={carryOn.busy}>
              <Icon name="play" size={13} /> {carryOn.busy ? 'Starting…' : carryOn.label}
            </button>
            {carryOn.refused !== undefined && <span className="lc-thread__note" role="note">{carryOn.refused}</span>}
          </div>
        )}

        {signInRuntime !== undefined && error !== undefined && (
          <div className="lc-rerun">
            <span>{runtimeDisplayName(signInRuntime)} is signed out. Sign in again, then send your message.</span>
            <SignInButton runtime={signInRuntime} />
          </div>
        )}

        {runAgainNote !== undefined && <div className="lc-thread__note" role="note">{runAgainNote}</div>}

        {onRunAgain !== undefined && !refusedByMode && (
          /*
           * One press, where retyping was the only way forward.
           *
           * Offered ONLY where the runtime never started -- no session, no
           * tool, nothing touched -- so pressing this cannot repeat work.
           * A run that got as far as doing something is deliberately not
           * offered it: whether the half it did matters is the person's
           * call, and the app does not get to make it for them.
           *
           * The case that produced this is a Cursor start failing on its own
           * config file while a second copy of it held that file open
           * (Colin, 2026-09-14). It is transient, it is not his fault, and
           * the message he had typed was still right.
           */
          <div className="lc-rerun">
            <span>Nothing had started, so running this again cannot repeat anything.</span>
            <button type="button" className="lc-button" onClick={onRunAgain}>
              <Icon name="play" size={13} /> Run it again
            </button>
          </div>
        )}

        {/*
          An interrupted mission kept everything needed to carry on and, until
          now, offered no way to. The offer sits ABOVE the receipt: the receipt
          is the record, this is the thing to do about it.
        */}
        {/*
          A DAMAGED RECORD SAYS SO WHERE IT IS READ (QA-2026-09-29 round 2,
          N7). A bad line in the middle of a mission's file cuts off what
          follows it, and the reply above stopped mid-sentence with nothing
          here to say why -- only the Missions screen's count hinted at it.
        */}
        {restoredMission !== undefined && restoredMission.integrityIssueCount > 0 && (
          <div className="lc-thread__note" role="note">
            Part of this turn&apos;s record could not be read, so what came after it is missing here. The file is kept as it is.
          </div>
        )}
        {restoredMission !== undefined && onResume !== undefined && (
          <ResumeCard offer={resumeOffer(restoredMission)} onResume={onResume} busy={running} {...(resumeRefusal === undefined ? {} : { refusal: resumeRefusal })} />
        )}
        {restoredMission !== undefined && (
          <ReceiptCard
            mission={restoredMission}
            workspacePath={workspacePath}
            {...(workspaceId === undefined ? {} : { workspaceId })}
          />
        )}
      </div>
      <JumpToBottom shown={follow.away} onClick={follow.toBottom} working={running} />
    </div>
  )
}
