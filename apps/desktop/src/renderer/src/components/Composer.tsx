import { accountUsageKey } from '../../../shared/agent-accounts.js'
import { accountName, routeAccount, useAgentAccounts } from '../agentAccounts.js'
import mark from '../assets/locust-mark.svg'
import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { usagePercent, usageWindowSentence } from '../missionView.js'
import { useDismissOnOutsidePress } from '../useDismissOnOutsidePress.js'
import type { ClipboardEvent, FormEvent, KeyboardEvent, MouseEvent, ReactElement } from 'react'

import type {
  MissionMode,
  MissionRouteSummary,
  PublicModel,
  PublicRuntimeStatus
} from '../../../shared/ipc.js'
import type { PublicTeammate, RuntimeCommandsResponse } from '../../../shared/ipc.js'
import { hostCanRunMission, isMissionRuntime, runtimeDisplayName } from '../../../shared/runtimes.js'
import type { ModeFacts } from '../status.js'
import {
  MODE_FACTS,
  handoffAvailability,
  handoffTitle,
  modeRunsOn,
  modesFor,
  modeUnavailableReason,
  modelFamily,
  runtimeIsUsable
} from '../status.js'
import { defaultEffort, sendBlockedReason } from '../status.js'
import { freeTagOf, isOwnRoute, routeFamilyName, routeModelName, shortRuntimeName } from '../routeName.js'
import { routeChipReading } from '../routeChipReading.js'
import { isTypingTarget, menuRowFromDigit } from '../menuDigit.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { handoffPreviewParts, joinerBefore } from '../handoffPreview.js'
import type { HandoffPreviewParts } from '../handoffPreview.js'
import { AttachedImage } from './AttachedImage.js'
import { PagePointChips } from './PagePointChips.js'
import { ReferenceChips } from './ReferenceChips.js'
import { MetalSend } from './MetalSend.js'
import type { MetalMotion, MetalPreset, MetalStrength } from '../../../shared/ipc.js'
import { isImagePath } from '../../../shared/image-files.js'
import { ContextRing, ContextRingMenu } from './ContextRing.js'
import type { ClaudeContextBreakdown } from '../../../shared/context-breakdown.js'
import { compactOffer } from '../compactNow.js'
import { connectorMentioned, connectorsOffHint } from '../connectorsOff.js'
import { ArrowUpGlyph, ChevronGlyph } from './ChatGlyphs.js'
import { PlusMenu } from './PlusMenu.js'
import type { ContextReading } from '../cost.js'
import { Icon } from './Icon.js'
import { Beam } from './Beam.js'
import { effortFooter, effortName } from '../effortLevels.js'
import { EffortSlider } from './EffortSlider.js'
import { CloudEnvironmentField } from './CloudEnvironmentField.js'
import { effortScale, joinEffort, splitEffort } from '../effortScale.js'
import { ATTACHMENT_DIR, attachmentLabel, MAX_ATTACHMENTS, withAttachments } from '../../../shared/attachments.js'
import { messageOfPagePoints, type PagePointChip } from '../../../shared/browser-point.js'
import { diffNotesTile, withDiffNotes } from '../diffNotes.js'
import type { DiffNote } from '../diffNotes.js'
import { availableCommands, matchingCommands, runtimeSlashCommands, slashQuery, SLASH_COMMANDS } from '../slashCommands.js'
import { argumentsOfOwnCommand, bangDraft, leaveCommandMode, ownCommandChoice } from '../../../shared/own-commands.js'
import type { ListedCommand } from '../../../shared/own-commands.js'
import { atQuery, fileMatches, withoutAtQuery } from '../fileMentions.js'
import type { SlashCommand } from '../slashCommands.js'
import { RoutePicker } from './RoutePicker.js'
import { policyFeatureRefusal, policyModeAllowed, policyTerminalRefusal } from '../../../shared/organization-policy.js'
import { CommandAsk, CommandModeNote } from './OwnCommand.js'
import { IssueList } from './IssueList.js'
import type { ListedIssue } from '../../../shared/github-issue.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'
import type { ComparePicking } from './RoutePicker.js'
import { MAX_COMPARE_SLOTS, MIN_COMPARE_SLOTS, versusLabel } from '../../../shared/compare.js'
import type { RouteChoice } from './RoutePicker.js'
import { RuntimeMark } from './RuntimeMark.js'
import { noteTyping } from '../faceLife.js'
import { VoiceButton } from './VoiceButton.js'
import { VoiceCallButton, type VoiceCallTalkTo } from './VoiceCall.js'
import { VoiceBeam } from 'voice-glow'
import { voiceLevel, voiceState } from '../voiceLevel.js'
import type { VoiceState } from '../voiceLevel.js'
/**
 * The microphone's level as the glow reads it. The meter is raw RMS, and ordinary speech sits around 0.05-0.2 of it;
 * the glow is drawn for 0..1 with speech near the top. A square root (loudness as heard) brings speech up without
 * making silence glow. Measured 2026-10-07 on the drive's spoken clip: a faint haze at raw RMS.
 * 0.689: x2.5, not x2. Colin's raised voice reached only half the box. The ceiling is unchanged, so it
 * still never climbs higher over the chips; it gets there with less voice.
 */
const glowLevel = (): number => Math.min(1, Math.sqrt(voiceLevel.get()) * 2.5)
import { insertVoiceText } from '../../../shared/voice-pcm.js'

const MAX_PROMPT_LENGTH = 8_000
/*
 * What Auto in a comparison does, said once for the menu and the chip's title.
 * "Its own copy" read as a wall, and it is not one: a model running without
 * asking can still write outside its copy -- by an absolute path, or through a
 * link in the project that points elsewhere (QA-2026-09-29, Q5). Keep is still
 * exactly what it says; the sentence now says the rest.
 */
const AUTO_COMPARE_CONSEQUENCE = 'Each model works in its own copy and runs what it needs without asking; only the one you keep comes into your folder. A copy is not a wall: like Auto anywhere, a model can still change files outside it.'
/*
 * A folder too big to copy works in place (0.555) -- and Auto's line still promised each model a copy of its own
 * there (the 2026-10-06 sweep, drive-compare-big-folder). Said as it is, before anything starts.
 */
const AUTO_COMPARE_IN_PLACE = 'This folder is too big to copy, so each model works in the folder itself and runs what it needs without asking: every model\u2019s changes land in your folder as it makes them, whichever one you keep.'
/** How long after a run starts a Stop press is taken as the second click of a double-click on Start (Q6). */
const STOP_SETTLE_MS = 600

/**
 * Each mode states its consequence, not just its name. There are two because
 * there are two the runtime can actually honour: `codex exec` has no
 * interactive approval channel, so "plan first" and "automatic" would be
 * labels over behaviour that does not differ.
 */
/*
 * The picker's rows, from the one table. They used to be written out here,
 * which is how the same mode came to be `Ask` in this list and `ask` in a
 * receipt -- see `MODE_FACTS` for the rest of that history.
 */
const MODES: readonly ModeFacts[] = MODE_FACTS

/**
 * What this mode does to a connector, in one sentence, measured.
 *
 * THREE wrong versions of this shipped on 2026-09-09 before the facts were
 * all in, and the history is kept because each was wrong in an instructive
 * way.
 *
 *  1. "Only Auto can reach an MCP server." The reason given was
 *     `--restricted`, which never blocked MCP at all -- its own help names
 *     `--strict-mcp-config` as the flag that would.
 *  2. "Your connectors are available in every mode." Removing our
 *     `--disallowedTools mcp__*` made them OFFERED, and driving the built app
 *     in Ask showed the rest: Claude Code asks before using one, and a
 *     printed run has nowhere to put that question, so the call was denied.
 *  3. "A connector call will be refused in this mode." True at the time, and
 *     wrong the moment Locust began passing an allow rule per connector.
 *
 * What is true now, measured by running the argv by hand and then by driving
 * the built app: every connector the person's own Claude Code can reach is
 * named in an allow rule on every run, so it is used without asking. Colin,
 * 2026-09-10: "honestly just let them have access to the mcp tools if the
 * client have access to it -- it only makes sense and is way less muddy."
 *
 * Which leaves the one thing a person genuinely cannot infer, and it is the
 * thing worth the line: a connector acts somewhere that is not this machine.
 * "Ask -- reads and explains, every write is refused" is a promise about
 * DISK. A teammate in Ask can still send mail or place an order through a
 * connector, because no sandbox on this machine reaches the far end of one.
 *
 * Claude Code only, for the MODE sentence: it is the only runtime whose
 * command builder mentions MCP at all, so the same words elsewhere would be a
 * claim nothing behind them makes.
 *
 * ELSEWHERE, THE SILENCE WAS THE PROBLEM. Locust reads connectors from the
 * person's Claude Code (`connector-reader.ts` -- `claude mcp list` is the
 * only source that has the claude.ai ones) and builds allow rules only for
 * Claude Code runs. On any other route this said nothing at all, so a person
 * with connectors set up had no way to learn that the route they had chosen
 * was not where they live.
 *
 * MEASURED 2026-09-11, and it cost Colin an evening: Robinhood is signed in
 * and `Connected` on his Claude Code, while `cursor-agent mcp list` reports
 * `requires_authentication` for the same URL and its `mcp login` persists
 * nothing at all -- so a teammate on Cursor failed every call, and, having
 * nothing else to go on, explained the failure by inventing reasons about
 * desktop OAuth. One sentence here would have ended it in a minute.
 *
 * The sentence must not overclaim in the other direction either. A teammate
 * on Cursor is NOT without connectors -- Cursor's own plugins were ready in
 * that same session -- it is a DIFFERENT SET, signed in separately. That is
 * the fact, and it is the one a person cannot see from here.
 */
export function connectorsNote(
  runtime: string,
  mode: MissionMode,
  /** Whether this machine's Claude Code has any, from `listConnectors`. */
  hasConnectors = false
): string | undefined {
  /*
   * Cursor's print mode has nobody to answer an approval, so a connector
   * call outside Auto resolves to "no". MEASURED in Colin's own ledger,
   * 2026-09-15/16: 17, 5, 3 and 2 `user rejected MCP` per run in Accept
   * edits (`--approve-mcps` approves the SERVER, not each call); the same
   * connector, same teammate, on Auto: 0. His question -- "wouldnt accept
   * edits hypothetically give you a prompt for the mcp call" -- yes in
   * Cursor's own app, where someone is there to answer; here there is not.
   */
  // 0.168.0: Locust writes the answer down where Cursor reads it -- an allow
  // rule per configured server in this workspace's `.cursor/cli.json` -- so
  // the question is never asked. The sentence says what a person cannot
  // infer from the mode: the rule is a file in their workspace.
  if (runtime === 'cursor' && mode !== 'auto') {
    return 'Your connectors work in this mode too: Locust allows each server you configured in this workspace\'s Cursor settings, since nobody is here to answer Cursor\'s per-call prompt. What this mode limits is this machine.'
  }
  if (runtime !== 'claude') {
    if (!hasConnectors) return undefined
    return `Your connectors are signed in on Claude Code. A teammate on ${shortRuntimeName(runtime as MissionRuntimeId)} uses that program's own instead — a different set, signed in separately.`
  }
  // Ask and Plan ask before each connector call, as Claude Code's plan mode does (0.545, measured).
  if (mode === 'ask' || mode === 'plan') {
    return 'Your connectors work in this mode, and each call asks you first, as Claude Code does. A connector acts on the service it reaches, not this machine.'
  }
  return mode === 'auto'
    ? 'Your connectors work here, and so does everything else on this machine.'
    : 'Your connectors work in this mode too. This mode only limits what happens on this machine — a connector acts on the service it reaches.'
}

export interface ComposerProps {
  readonly referenceMissionId?: string
  readonly referenceCwd?: string
  readonly referencesOnCloud?: boolean
  /** The teammate whose conversation this is, for Talk (0.746): a call with them, into this conversation. */
  readonly talkTo?: VoiceCallTalkTo
  /**
   * Words handed back to the box (C9): a busy model's message, with its
   * attachments, when the person goes on with another model -- so Enter
   * sends it there. A new object hands it back again.
   */
  readonly handBack?: { readonly text: string; readonly attachments: readonly string[] }
  /**
   * A part of a reply to ask about (SelectionAsk): quoted at the top of the
   * box, above whatever is typed there. A new object quotes again.
   */
  readonly quoteIn?: { readonly quote: string; readonly attachments?: readonly string[] }
  /** Files to attach, and nothing else. A new id attaches again. */
  readonly attachIn?: { readonly id: number; readonly paths: readonly string[] }
  /** A sentence from the window, shown once per id. */
  readonly told?: { readonly id: number; readonly text: string }
  /** Ctrl+Shift+M, I or E: open the mode, model or effort menu of this conversation. */
  readonly menuChord?: { readonly id: number; readonly which: 'mode' | 'model' | 'effort' }
  /**
   * Parts of the browser page waiting to be sent. Each picture is attached
   * with the message; removing a chip removes its picture. Nothing is sent
   * until Send.
   */
  readonly pagePoints?: readonly PagePointChip[]
  readonly onPagePointNote?: (id: string, note: string) => void
  readonly onPagePointRemove?: (id: string) => void
  readonly onPagePointsSent?: () => void
  /** Each runtime's own slash commands, as its CLI last listed them (0.426). */
  readonly runtimeCommands?: RuntimeCommandsResponse
  /**
   * Whether this machine's Claude Code has any connectors, so the mode menu
   * can say where they live when the route is not Claude. See
   * `connectorsNote`.
   */
  readonly hasConnectors?: boolean
  /** The teammate's connectors are off (0.798): a draft naming one says so. Absent where they are on, or not Claude Code. */
  readonly connectorsOff?: { readonly teammate: string; readonly names: readonly string[] }
  /** A free model OpenCode no longer lists (0.819, App's droppedOffer): said over the box, with the press off it. */
  readonly modelDropped?: { readonly text: string; readonly label: string; readonly onPress: () => void }
  readonly runtimes: readonly PublicRuntimeStatus[]
  /** Discovery asked its four times and nobody answered; the rows say NOT ANSWERING. */
  readonly runtimesGaveUp?: boolean
  /** Runtimes whose last run ended on the account's usage limit, with its own words. */
  readonly limitedRuntimes: ReadonlyMap<string, string>
  /** The latest still-allowed rate-limit reading per runtime, for the route chip's tooltip. */
  readonly usageWindows?: ReadonlyMap<string, string>
  /**
   * How full this conversation's context is, drawn left of the route.
   *
   * It lived here as a bare glyph beside the swarm mark, moved to the mission
   * header on the design agent's read (2026-09-10) -- "a fact about THIS
   * conversation, and that line is where the conversation's other facts are"
   * -- and is back, at Colin's word on 2026-09-11: "theres no more
   * context/usage circle, we can just put it on the left of the model
   * picker." Left of the ROUTE, not loose on the row, which is the part that
   * answers the original objection: it now belongs to the control it is about.
   */
  readonly context?: ContextReading
  /** The conversation's total spend, for the ring's hover or its slot. */
  readonly conversationCost?: string
  readonly discoveryPhase: 'loading' | 'ready' | 'error'
  readonly running: boolean
  readonly cancelling: boolean
  readonly activeRoute: MissionRouteSummary | undefined
  readonly error: string | undefined
  readonly mode: MissionMode
  readonly onModeChange: (mode: MissionMode) => void
  /** How full the model's context is, when the runtime reported its size. */
  /** Whether this workspace has Auto switched on. Picking Auto here switches it on. */
  readonly autoMode?: boolean
  /** Turn Auto on for the workspace, because the person just chose it. */
  readonly onEnableAutoMode?: () => void
  readonly route: RouteChoice
  readonly onRouteChange: (route: RouteChoice) => void
  readonly models: readonly PublicModel[]
  /** What each route's model resolved to last time, keyed `runtime:model`. */
  readonly resolvedModels: ReadonlyMap<string, string>
  /** Routes this person has run, newest first, as `runtime:model`. */
  readonly recentRoutes: readonly string[]
  /** Where this build is running; some containment is platform specific. */
  readonly platform: string | undefined
  /** The send button's metal; see `MetalSend` and Settings. */
  readonly metal?: MetalPreset
  readonly metalStrength?: MetalStrength
  readonly metalMotion?: MetalMotion
  readonly metalBend?: boolean
  readonly effort: string | undefined
  readonly onEffortChange: (effort: string | undefined) => void
  readonly swarm: boolean
  readonly onSwarmChange: (swarm: boolean) => void
  /** True when it started; false, or the words saying why not (M27), when it did not. */
  /** `leaveOut`: a reply on another runtime, the brief sections the person left out (0.527). */
  readonly onStart: (prompt: string, options?: { readonly leaveOut?: readonly string[]; readonly referenceIds?: readonly string[] }) => Promise<boolean | string>
  /**
   * TAG A TEAMMATE FROM ANY CONVERSATION (0.438, shared/tagging.ts): the team
   * `@` offers, who is on screen (never offered), and what happens to the
   * tagged -- sent the message in a conversation of their own. Called BEFORE
   * the send, so the conversation on screen is still the one they came from.
   */
  readonly team?: readonly PublicTeammate[]
  readonly currentTeammateId?: string
  /** Answers what to say under the box: who it went to, or who could not take it. */
  readonly onTag?: (teammateIds: readonly string[], prompt: string) => Promise<{ readonly text: string; readonly sent: boolean } | undefined>
  /**
   * COMPARE (0.441, shared/compare.ts): the picker's One / Compare switch and
   * the models ticked; absent where a comparison cannot start. `asking` is a
   * comparison on screen: the box then asks every column.
   */
  readonly compare?: ComparePicking
  /**
   * CLOUD (0.503): the chat-type menu's fourth choice, a task for Codex Cloud.
   * `refusal` says why it cannot be picked here (not a Codex route, no GitHub).
   */
  readonly cloud?: {
    readonly on: boolean
    readonly onMode: (on: boolean) => void
    readonly refusal?: string
    /**
     * Which cloud the picked model goes to (0.538): Codex Cloud, or Claude's. Absent: Codex. `computer` (0.761): the
     * person's own cloud computer, a GitHub Codespace, for a teammate whose maker has no cloud.
     */
    readonly where?: 'codex' | 'claude' | 'computer'
    readonly environment?: { readonly value: string; readonly onChange: (value: string) => void }
    /**
     * The person's own cloud computer for a Claude Code or Codex teammate (0.768), offered beside its maker's cloud:
     * that agent, signed in there by the person, runs the turn there as a turn of this conversation.
     */
    readonly computer?: { readonly on: boolean; readonly onMode: (on: boolean) => void; readonly agentName: string }
  }
  /**
   * BACKGROUND (W10, 0.683): the chat-type menu's choice to hand a Claude turn to Claude Code's own background
   * session, which keeps going if Locust closes. Only where the window passes it: a Claude route.
   */
  readonly background?: { readonly on: boolean; readonly onMode: (on: boolean) => void }
  readonly asking?: { readonly label: string; readonly columns: number; readonly changes?: boolean; readonly blind?: boolean }
  /** Opens the model picker when it changes: Home's Compare models (0.442). */
  readonly pickerRequest?: number
  /**
   * Words put in the box from outside it, each time `seq` moves (0.448: a Build and compare starter). From a link
   * (0.770, shared/locust-link.ts) the box takes the cursor and says where they came from, until they are sent or
   * cleared.
   */
  readonly fill?: { readonly text: string; readonly seq: number; readonly from?: 'link' }
  /**
   * An earlier message is being edited (0.498): its words are in the box, and
   * sending starts the conversation again from there. Cancel leaves it as it was.
   */
  readonly editingEarlier?: {
    readonly onCancel: () => void
    /** The files the replies after it changed (0.502): put back too, if ticked. */
    readonly files?: { readonly count: number; readonly on: boolean; readonly onToggle: () => void }
    /** Some of those cannot go back exactly, so none are offered (0.512). */
    readonly filesHeld?: { readonly total: number; readonly cannot: number }
  }
  /**
   * Notes the person pinned to lines of a diff in this conversation
   * (diffNotes.ts, 0.376). They go with the next message, as one block after
   * its words, and are cleared once it has gone.
   */
  readonly diffNotes?: readonly DiffNote[]
  readonly onClearDiffNotes?: () => void
  readonly onCancel: () => void
  readonly onOpenRoutePicker: () => void
  /**
   * Move the RUNNING mission to another route. Distinct from `onRouteChange`,
   * which only decides what the next mission starts on: this one stops the
   * current run, reconciles it, and starts a briefed continuation. Two very
   * different consequences, so they are two different callbacks rather than one
   * that quietly means something else while a run is live.
   */
  /** Resolves to the reason when the handoff was refused with nothing stopped (M9). */
  readonly onHandOff: (route: RouteChoice) => void | Promise<string | undefined>
  readonly handingOff: boolean
  /** The folder the next mission runs in, by its last segment; undefined when none is chosen. */
  readonly workspaceName: string | undefined
  /** The whole path, for the chip's tooltip -- a name alone is ambiguous across projects. */
  readonly workspacePath: string | undefined
  readonly workspaceMade?: boolean
  /** Why the folder is no place for teammates, when it is not (0.790): "your home folder", ".claude, where ...". */
  readonly workspaceCaution?: string
  /**
   * The question before the first conversation in such a folder (0.790): the send came back unstarted, and the box
   * asks over itself. Start here keeps the folder (onConfirmFolder) and sends what is in the box.
   */
  readonly folderQuestion?: { readonly name: string; readonly why: string }
  readonly onConfirmFolder?: () => Promise<boolean>
  readonly onChooseFolder: () => void
  /**
   * Folders already worked in, newest first, for the chip's menu (0.458, as
   * Claude Code's folder picker): one press starts a new conversation there.
   */
  readonly folders?: readonly { readonly id: string; readonly name: string; readonly path: string }[]
  readonly currentFolderId?: string
  readonly onSwitchFolder?: (id: string) => void
  /** Who the next mission is messaged to; the placeholder says so. */
  readonly teammateName: string | undefined
  /**
   * Set when the addressed teammate already has a live mission somewhere.
   * Starting is refused for THEM, not for the workspace: another teammate's
   * run being on screen does not block this one.
   */
  readonly busyWith: string | undefined
  /**
   * Said above the box when the next message continues a stopped run on a
   * DIFFERENT runtime: the person is told, before sending, that the new
   * runtime starts from the old one's checkpoint rather than its memory.
   */
  readonly continuationNote: string | undefined
  /**
   * The stopped run a reply on another runtime continues, and that runtime
   * (0.517): the box asks the host what the brief would carry, as it is
   * typed, and says it under the note.
   */
  readonly continuation?: { readonly followUpOf: string; readonly runtime: MissionRuntimeId }
  /**
   * What is waiting to be sent when the running mission finishes, if anything.
   *
   * Already FOLDED: the strip shows the one instruction that will actually go,
   * not the pieces it was typed in, because that is what the teammate will
   * receive and therefore what a person is deciding about.
   */
  readonly queued: string | undefined
  /** How many separate things are waiting, so the strip can say so. */
  readonly queuedCount?: number
  readonly queuedAttachments?: readonly string[]
  /** Why a queued message has not gone yet, when it is not simply still running. */
  readonly queuedNote: string | undefined
  readonly queueError?: string
  /** With the files attached to it, which ride on the row and go when it goes (L20). */
  /** `now`: Ctrl+Enter -- stop the run on screen and send this at once (0.485). */
  readonly onQueue: (text: string, attachments: readonly string[], now?: boolean) => void | Promise<boolean>
  /** Stop the run on screen and send the queued message now; absent where that is not the run on screen. */
  readonly onSendQueuedNow?: () => void
  readonly onUnqueue: () => void | Promise<boolean>
  readonly onSendQueued: () => void
  /** The queued message belongs to a conversation that is NOT the one on screen. */
  readonly queuedElsewhere: boolean
  /**
   * Which conversation the box is writing in (0.551). A message typed and not
   * sent stays with its conversation, as Claude Code's does; Colin, on 0.549:
   * "my message is staying in the chatbox for all chats if unsent".
   */
  readonly draftKey?: string
  /** Markdown commands from the person's folder and this conversation's folder. */
  readonly ownCommands?: readonly ListedCommand[]
  /** Puts the expanded prompt in the box. Choosing does not send. */
  readonly onChooseOwnCommand?: (name: string, source: 'global' | 'project', args: string) => void
  /** A `!` command. The box does not send it to a teammate. */
  readonly onRunCommand?: (command: string) => void
  /** A project command's `!` lines, waiting for a yes. */
  readonly commandAsk?: { readonly lines: readonly string[]; readonly onAllow: () => void; readonly onNotNow: () => void }
  /** The folder is a GitHub repository, so `/issue` is in the menu. */
  readonly issues?: boolean
  /** Open the issue list. Choosing one fills the box, and nothing is sent. */
  readonly onPickIssue?: () => void
  readonly canTune?: boolean
  readonly onTune?: () => void
  /** `/setup` (0.811): the open conversation is a teammate's. */
  readonly canSetup?: boolean
  readonly onSetup?: () => void
  /** The open list, drawn above the field. */
  readonly issueList?: { readonly issues: readonly ListedIssue[]; readonly empty?: string; readonly onChoose: (number: number) => void; readonly onClose: () => void }
}

/**
 * The composer: one field, one border, controls underneath.
 *
 * The design puts mode, add, route, effort and swarm on this row. Mode, add,
 * effort and swarm are drawn as disabled here on purpose -- each needs a
 * capability that does not exist yet, and a control that silently does nothing
 * is worse than one that says it is not ready. The route control is real: it
 * opens the picker, and what it displays comes from the running mission or from
 * discovery, never from a constant.
 */
/**
 * The mode Shift+Tab moves to, or undefined when there is nowhere to go.
 *
 * M33 (the code review): Auto is in the cycle only when the workspace's Auto
 * switch is already on. Landing on it used to turn that saved safety switch
 * on, and nothing turned it off when the next press moved on -- so going
 * from Accept edits round to Ask left Auto allowed for good. Turning it on is
 * the menu's choice, with its warning, never a side effect of a key.
 */
export function shiftTabMode<M extends string>(current: M, usable: readonly M[], autoAllowed: boolean): M | undefined {
  const cycle = usable.filter((mode) => mode !== 'auto' || autoAllowed)
  if (cycle.length < 2) return undefined
  const at = cycle.indexOf(current)
  return cycle[(at + 1) % cycle.length]
}

/** The chat mode chip's three choices, in Arena's words where they fit (0.451). */
const CHAT_MODES: readonly { readonly id: 'direct' | 'compare' | 'blind' | 'cloud' | 'computer' | 'background'; readonly name: string; readonly desc: string; readonly icon: 'message' | 'columns' | 'eye-off' | 'cloud' | 'server' | 'clock' }[] = [
  { id: 'direct', name: 'Direct', desc: 'Chat with one model at a time', icon: 'message' },
  { id: 'compare', name: 'Compare', desc: 'Two or three models of your choice, side by side', icon: 'columns' },
  { id: 'blind', name: 'Blind', desc: 'Compare with the names hidden until you keep one', icon: 'eye-off' },
  // 0.503: offered only where the window passes `cloud` (main/cloud-tasks.ts). Codex Cloud takes no model from
  // Locust (`codex cloud exec` 0.161.0 has no --model), so the line says whose model it runs on (10/08).
  { id: 'cloud', name: 'Cloud', desc: 'Runs in Codex Cloud on this repository, on the model Codex Cloud chooses; bring the change home when it is done', icon: 'cloud' },
  // 0.768: offered only where the window passes `cloud.computer` (a Claude Code or Codex route); its words name the agent.
  { id: 'computer', name: 'Cloud computer', desc: 'Runs on your cloud computer, a GitHub Codespace; watch it here, bring the change home when it is done', icon: 'server' },
  // W10: offered only where the window passes `background` (a Claude route).
  { id: 'background', name: 'Background', desc: 'Keeps working on this computer even if you close Locust. Claude Code runs it, and you answer its questions there', icon: 'clock' }
]
/**
 * The Cloud choice when a Claude model is picked (0.538). Since 0.556 no
 * window opens unless Claude Code has a question for the person.
 */
const CLAUDE_CLOUD_DESC = 'Runs in Claude’s cloud; follow it on claude.ai, tell it more from here, bring it home when it is done'
/** The Cloud choice for a teammate whose maker has no cloud (0.761): the person's own, a GitHub Codespace. */
const COMPUTER_CLOUD_DESC = 'Runs on your cloud computer, a GitHub Codespace; watch it here, bring the change home when it is done'
/** The Cloud computer choice for a Claude Code or Codex teammate (0.768): its own copy there runs it. */
const agentComputerDesc = (agentName: string): string => `${agentName} runs it on your cloud computer, a GitHub Codespace; watch it here, bring the change home when it is done`

export function Composer({
  referenceMissionId,
  referenceCwd,
  referencesOnCloud,
  talkTo,
  handBack,
  quoteIn,
  attachIn,
  told,
  menuChord,
  pagePoints,
  onPagePointNote,
  onPagePointRemove,
  onPagePointsSent,
  runtimeCommands,
  hasConnectors = false,
  connectorsOff,
  modelDropped,
  continuationNote,
  continuation,
  workspaceName,
  workspacePath,
  workspaceMade = false,
  workspaceCaution,
  folderQuestion,
  onConfirmFolder,
  onChooseFolder,
  folders = [],
  currentFolderId,
  onSwitchFolder,
  runtimes,
  runtimesGaveUp = false,
  limitedRuntimes,
  usageWindows,
  context,
  conversationCost,
  discoveryPhase,
  running,
  cancelling,
  activeRoute,
  error,
  mode,
  onModeChange,
  autoMode,
  onEnableAutoMode,
  route,
  onRouteChange,
  models,
  resolvedModels,
  recentRoutes,
  platform,
  metal,
  metalStrength,
  metalMotion,
  metalBend,
  effort,
  onEffortChange,
  swarm,
  onSwarmChange,
  onStart,
  team,
  currentTeammateId,
  onTag,
  compare,
  asking,
  pickerRequest,
  fill,
  editingEarlier,
  cloud,
  background,
  diffNotes,
  onClearDiffNotes,
  onCancel,
  onOpenRoutePicker,
  onHandOff,
  handingOff,
  teammateName,
  busyWith,
  queued,
  queuedCount,
  queuedAttachments,
  queuedNote,
  queueError,
  onQueue,
  onUnqueue,
  onSendQueued,
  onSendQueuedNow,
  queuedElsewhere,
  draftKey,
  ownCommands,
  onChooseOwnCommand,
  onRunCommand,
  commandAsk,
  issues,
  onPickIssue,
  canTune,
  onTune,
  canSetup,
  onSetup,
  issueList
}: ComposerProps): ReactElement {
  const policy = useOrganizationPolicy()
  const cloudManaged = policyFeatureRefusal(policy, 'cloud') !== undefined
  const terminalOff = policyTerminalRefusal(policy) !== undefined
  const [value, setValue] = useState('')
  const drafts = useRef(new Map<string, string>())
  const valueNow = useRef(value)
  valueNow.current = value
  const draftFor = useRef(draftKey)
  useEffect(() => {
    const before = draftFor.current
    if (before === draftKey) return
    if (before !== undefined) {
      if (valueNow.current.trim().length > 0) drafts.current.set(before, valueNow.current)
      else drafts.current.delete(before)
    }
    draftFor.current = draftKey
    // A new conversation's first run takes its id as it starts: still the same conversation.
    if (before !== undefined && before.startsWith('pending')) return
    setValue(draftKey === undefined ? '' : drafts.current.get(draftKey) ?? '')
  }, [draftKey])
  // What a reply on another runtime would carry, worked out by the host as it is typed (0.517).
  const [carried, setCarried] = useState<HandoffPreviewParts>()
  // What the person chose to leave out of that brief (0.527), for this conversation and runtime only.
  const [leaveOut, setLeaveOut] = useState<readonly string[]>([])
  const continuationKey = continuation === undefined ? undefined : `${continuation.followUpOf}:${continuation.runtime}`
  useEffect(() => {
    setLeaveOut([])
  }, [continuationKey])
  const leaveOutKey = leaveOut.join(',')
  // A click answers at once; typing waits for a pause, as it always has.
  const askedWith = useRef(leaveOutKey)
  useEffect(() => {
    const clicked = askedWith.current !== leaveOutKey
    askedWith.current = leaveOutKey
    if (continuation === undefined || window.desktop?.previewHandoff === undefined) {
      setCarried(undefined)
      return
    }
    let live = true
    const timer = setTimeout(() => {
      void window.desktop
        ?.previewHandoff({ followUpOf: continuation.followUpOf, runtime: continuation.runtime, prompt: value, ...(leaveOut.length === 0 ? {} : { leaveOut }) })
        .then((preview) => {
          if (live) setCarried(handoffPreviewParts(preview))
        })
        .catch(() => undefined)
    }, clicked ? 0 : 350)
    return () => {
      live = false
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the keys stand for the objects
  }, [continuationKey, value, leaveOutKey])
  /*
   * Start and Stop are one button in one place, so the second click of a
   * double-click on Start landed on Stop: "You stopped this run", and nothing
   * had run (QA-2026-09-29, Q6). A Stop pressed within STOP_SETTLE_MS of the
   * run starting is taken as that second click and ignored; a person who
   * means Stop presses it again a moment later.
   */
  const runningSince = useRef<number | undefined>(undefined)
  useEffect(() => {
    runningSince.current = running ? Date.now() : undefined
  }, [running])
  const stopClicked = (): void => {
    if (runningSince.current !== undefined && Date.now() - runningSince.current < STOP_SETTLE_MS) return
    onCancel()
  }
  /*
   * The box's words came off the queue by Edit. Claude Code does the same --
   * a queued message pulled back into the box is no longer queued -- but it
   * said nothing, and the outside recheck of 0.303 read Edit as "keep it
   * queued until I save": a run that finished mid-edit sent nothing, and the
   * words sat in the box. Said now, where the queued card was.
   */
  const [offTheQueue, setOffTheQueue] = useState(false)
  // Emptied -- sent, or cleared -- it is just the box again.
  if (offTheQueue && value.trim().length === 0) setOffTheQueue(false)
  /** Why the last press of Enter did nothing. Cleared as soon as one lands. */
  /**
   * What the box has to say about the last press, and in which register.
   *
   * `plain` is the difference between "that did not work" and "here is what I
   * did": a file copied in from outside the workspace is reported, not
   * refused, and drawing it in the red of a failure taught the eye to read a
   * success as a problem (seen in a drive, 2026-09-08).
   */
  const [refusal, setRefusal] = useState<{ readonly text: string; readonly plain?: boolean; readonly icon?: 'users' }>()
  const type = (next: string): void => {
    setValue(next)
    // The face you are typing to listens (faceLife.ts).
    noteTyping()
    // Typing leaves the history. Without this, editing a recalled message and
    // then pressing up again would walk further back and throw the edit away.
    if (recallAt >= 0) setRecallAt(-1)
    // The refusal was about the press, not about the text. Typing again is
    // the person trying something; leaving the old sentence up implies it
    // still applies.
    if (refusal !== undefined) setRefusal(undefined)
  }
  /** Say something, or -- with no text -- say nothing. */
  const setNote = (text: string | undefined, plain = false): void => {
    setRefusal(text === undefined ? undefined : { text, ...(plain ? { plain: true } : {}) })
  }
  const [modeOpen, setModeOpen] = useState(false)
  /** Direct, Compare or Blind (0.451, as Arena draws it): the chip left of the mode. */
  const [chatModeOpen, setChatModeOpen] = useState(false)
  /** The folder chip's menu (0.458). */
  const [folderOpen, setFolderOpen] = useState(false)
  const [referenceOpen, setReferenceOpen] = useState(0)
  const [referenceIds, setReferenceIds] = useState<readonly string[]>([])
  const [referenceBusy, setReferenceBusy] = useState(false)
  const [referencesSent, setReferencesSent] = useState(0)
  // The context ring's menu (0.797): what it says, and Compact now.
  const [contextOpen, setContextOpen] = useState(false)
  const [contextBreakdown, setContextBreakdown] = useState<ClaudeContextBreakdown>()
  /** Which comparison column's picker is open, or the + for a third (0.460). */
  const [slotPicker, setSlotPicker] = useState<number | 'add' | undefined>(undefined)
  /** Which comparison column's effort panel is open (0.490). */
  const [slotEffort, setSlotEffort] = useState<number | undefined>(undefined)
  /**
   * Files this message will point the runtime at, workspace-relative.
   *
   * Cleared on send, like the box itself: an attachment belongs to the message
   * it was chosen for, and carrying it silently into the next one would attach
   * a file nobody asked for.
   */
  const [attached, setAttached] = useState<readonly string[]>([])
  /**
   * Which attached files were brought in from outside the workspace.
   *
   * The mark appears only where something actually happened: a file already
   * inside the folder gets a plain tile, because marking it would claim a copy
   * that never took place.
   */
  const [copiedIn, setCopiedIn] = useState<ReadonlySet<string>>(new Set())
  const [attaching, setAttaching] = useState(false)
  /**
   * Which slash command the arrow keys are on.
   *
   * Typing narrows the list under it, so this index can outlive the entry it
   * pointed at. Every read clamps to the last row rather than resetting on
   * every keystroke: resetting would drag the highlight back to the top mid-typing,
   * and clamping means the index can never name a command that is not there.
   */
  const [slashAt, setSlashAt] = useState(0)
  // With a runtime's commands the list is long: the row the arrows reach stays in view (0.426).
  const slashList = useRef<HTMLDivElement>(null)
  useEffect(() => {
    slashList.current?.querySelector('.lc-slash__item.is-active')?.scrollIntoView({ block: 'nearest' })
  }, [slashAt])
  const [pickerOpen, setPickerOpen] = useState(false)
  const [pickerFocus, setPickerFocus] = useState<'model' | 'effort'>('model')
  useEffect(() => {
    if (pickerRequest !== undefined && pickerRequest > 0) setPickerOpen(true)
  }, [pickerRequest])
  const [fromLink, setFromLink] = useState(false)
  useEffect(() => {
    if (fill === undefined) return
    setValue(fill.text)
    setFromLink(fill.from === 'link')
    if (fill.from === 'link') field.current?.focus()
  }, [fill?.seq])
  useEffect(() => {
    // Sent or emptied. Only once it was said: a box drawn afresh reads its first, empty value after the fill.
    if (fromLink && value.trim().length === 0) setFromLink(false)
  }, [value])

  /*
   * These three panels closed only by pressing their own control again,
   * which is not how a menu behaves anywhere else in this app or on the
   * machine. Reported by the first outside tester on 0.55.0: "you to click
   * out of it you have to click the button again instead of just clicking
   * anywhere on the page."
   *
   * The ref is on the ANCHOR, which holds the panel and the control that
   * opens it, so a press on the control still toggles rather than being
   * swallowed by the close.
   */
  /*
   * What you have sent, so the up arrow can bring it back.
   *
   * Claude Code recalls the previous message on an empty prompt and it is one
   * of the highest-frequency things in the whole program -- resend, tweak a
   * word, resend. Locust's composer bound the arrows only while the slash
   * menu was open, so on an empty box they did nothing at all.
   *
   * Kept in the composer rather than the record because it is a property of
   * this box in this session, the same as it is there: it recalls what YOU
   * typed, not what the mission holds.
   */
  const sent = useRef<string[]>([])
  const [recallAt, setRecallAt] = useState(-1)

  const modeAnchor = useRef<HTMLSpanElement>(null)
  const chatModeAnchor = useRef<HTMLSpanElement>(null)
  const folderAnchor = useRef<HTMLSpanElement>(null)
  const slotsAnchor = useRef<HTMLSpanElement>(null)
  const closeSlots = useCallback(() => setSlotPicker(undefined), [])
  useDismissOnOutsidePress(slotPicker !== undefined, closeSlots, slotsAnchor)
  const closeSlotEffort = useCallback(() => setSlotEffort(undefined), [])
  useDismissOnOutsidePress(slotEffort !== undefined, closeSlotEffort, slotsAnchor)
  const closeFolder = useCallback(() => setFolderOpen(false), [])
  useDismissOnOutsidePress(folderOpen, closeFolder, folderAnchor)
  const pickerAnchor = useRef<HTMLSpanElement>(null)
  const routeChip = useRef<HTMLButtonElement>(null)
  const closeMode = useCallback(() => setModeOpen(false), [])
  const closePicker = useCallback(() => setPickerOpen(false), [])
  useDismissOnOutsidePress(modeOpen, closeMode, modeAnchor)
  const contextAnchor = useRef<HTMLSpanElement>(null)
  const closeContext = useCallback(() => setContextOpen(false), [])
  useDismissOnOutsidePress(contextOpen, closeContext, contextAnchor)
  const closeChatMode = useCallback(() => setChatModeOpen(false), [])
  useDismissOnOutsidePress(chatModeOpen, closeChatMode, chatModeAnchor)
  useDismissOnOutsidePress(pickerOpen, closePicker, pickerAnchor)
  useEffect(() => {
    if (!modeOpen && !chatModeOpen) return undefined
    const onKey = (event: globalThis.KeyboardEvent): void => {
      const index = menuRowFromDigit(event.key, { ctrl: event.ctrlKey, meta: event.metaKey, alt: event.altKey, shift: event.shiftKey }, isTypingTarget(event.target))
      if (index === undefined) return
      const root = modeOpen ? modeAnchor.current : chatModeAnchor.current
      const row = root?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]:not([disabled])')[index]
      if (row === undefined) return
      event.preventDefault()
      row.click()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [modeOpen, chatModeOpen])

  // A mode the chosen route cannot run is not the mode a mission would start
  // in, so it is not the mode the control shows either. Switching route used
  // to leave "Approve each action" selected against a runtime that refuses
  // it, and every message was then rejected before it began.
  const effectiveMode: MissionMode = modeRunsOn(mode, route.runtime, platform)
    ? mode
    : modesFor(route.runtime, platform)[0] ?? 'accept-edits'
  /*
   * Slash commands. Every one maps to a control already on this row, so the
   * menu is a keyboard path to things that exist rather than a new capability
   * -- and the list is filtered by what is actually possible, so a command can
   * never be offered and then refused.
   */
  const slashing = slashQuery(value)
  const commandMode = !terminalOff && bangDraft(value) !== undefined
  const ownRows: readonly SlashCommand[] = (ownCommands ?? []).map((command) => ({
    name: command.name,
    detail: command.description ?? (command.source === 'project' ? 'A command in this folder.' : 'A command of yours.'),
    action: { kind: 'own', source: command.source },
    group: command.source === 'project' ? 'This folder' : 'Your commands'
  }))
  const takenNames = new Set(SLASH_COMMANDS.map((command) => command.name))
  const yours = ownRows.filter((command) => !takenNames.has(command.name.toLowerCase()))
  const runtimeRows = !running && (route.runtime === 'claude' || route.runtime === 'opencode' || route.runtime === 'codex') && runtimeCommands?.[route.runtime] !== undefined
    ? runtimeSlashCommands(runtimeDisplayName(route.runtime), runtimeCommands[route.runtime]!).filter((command) => !takenNames.has(command.name.toLowerCase()))
    : []
  // Yours sit after Locust's own and before a runtime's long list, so `/` shows them.
  const slashMatches = commandMode || slashing === undefined
    ? []
    : matchingCommands(slashing, [
        ...availableCommands({
          modes: modesFor(route.runtime, platform).filter((value) => policyModeAllowed(policy, value)),
          running,
          canSwarm: !running,
          issues,
          tune: canTune,
          setup: canSetup
        }),
        ...yours,
        ...runtimeRows
      ])
  // A runtime's command followed by a space is complete: what comes next is
  // its arguments, and Enter sends it. Left open, the menu took that Enter
  // and wrote the same command back into the box, forever (0.426).
  const writingArguments = /\s$/.test(value) &&
    slashMatches.some((command) => command.action.kind === 'runtime' && command.name.toLowerCase() === slashing)
  // `/name the rest` is still that command: Enter fills the box with the rest as its arguments.
  const typedOwn = commandMode || commandAsk !== undefined ? undefined : argumentsOfOwnCommand(value, ownRows.map((command) => command.name))
  const ownWithArgs = typedOwn === undefined ? [] : ownRows.filter((command) => command.name.toLowerCase() === typedOwn.name.toLowerCase())
  const slashChoices = commandAsk !== undefined || writingArguments ? [] : ownWithArgs.length > 0 ? ownWithArgs : slashMatches
  /*
   * `@` FOR A FILE OF THE PROJECT (0.436, fileMentions.ts). The folder's
   * files, asked of the host the first time `@` is typed and again at most
   * every half minute; picking one attaches it as the + menu does and takes
   * the `@word` out of the message. Escape closes the list for what is typed.
   */
  const [projectFiles, setProjectFiles] = useState<readonly string[] | undefined>(undefined)
  // The host listed only the first files it found (R16): the menu says so.
  const [filesPartial, setFilesPartial] = useState(false)
  const filesAskedAt = useRef(0)
  const [atAt, setAtAt] = useState(0)
  const [atDismissed, setAtDismissed] = useState<string | undefined>(undefined)
  const typedAt = slashing === undefined ? atQuery(value) : undefined
  const wantsFiles = typedAt !== undefined
  useEffect(() => {
    if (!wantsFiles || Date.now() - filesAskedAt.current < 30_000) return
    filesAskedAt.current = Date.now()
    void window.desktop?.workspaceFiles().then((answer) => {
      if (!answer.ok) return
      setProjectFiles(answer.paths)
      setFilesPartial(answer.truncated)
    }).catch(() => undefined)
  }, [wantsFiles])
  // Memoized: a folder can list 50,000 files, and this ran on every render.
  const fileChoices = useMemo(
    () => typedAt === undefined || projectFiles === undefined || atDismissed === value
      ? []
      : fileMatches(projectFiles.filter((path) => !attached.includes(path)), typedAt),
    [typedAt, projectFiles, atDismissed, value, attached]
  )
  const filesHeading = filesPartial && projectFiles !== undefined
    ? `Files in this folder, the first ${projectFiles.length.toLocaleString('en-US')} found`
    : 'Files in this folder'
  // Nothing found in a partial list is not "not there" (R16): said, with the way on.
  const noMatchInPartial = filesPartial && projectFiles !== undefined && typedAt !== undefined && typedAt.length > 0 && atDismissed !== value && fileChoices.length === 0
  // Teammates first, by the start of their name; never the one on screen, never one tagged already.
  const [tagged, setTagged] = useState<readonly string[]>([])
  const teammateChoices = typedAt === undefined || atDismissed === value || team === undefined || onTag === undefined
    ? []
    : team.filter((mate) => mate.teammateId !== currentTeammateId && !tagged.includes(mate.teammateId) && mate.name.toLowerCase().startsWith(typedAt.toLowerCase())).slice(0, 6)
  type AtChoice = { readonly kind: 'teammate'; readonly mate: PublicTeammate } | { readonly kind: 'file'; readonly path: string }
  const atChoices: readonly AtChoice[] = [
    ...teammateChoices.map((mate) => ({ kind: 'teammate' as const, mate })),
    ...fileChoices.map((path) => ({ kind: 'file' as const, path }))
  ]
  const pickTeammate = (teammateId: string): void => {
    setAtAt(0)
    setValue(withoutAtQuery(value))
    setTagged((current) => [...new Set([...current, teammateId])].slice(0, 5))
    requestAnimationFrame(() => field.current?.focus())
  }
  const pickChoice = (choice: AtChoice): void => {
    if (choice.kind === 'teammate') pickTeammate(choice.mate.teammateId)
    else pickFile(choice.path)
  }
  /*
   * Attached, deduplicated and capped -- and the cap SAID (QA-2026-09-29
   * round 2, R19): ten files chosen kept eight, and nothing said two were
   * left out.
   */
  const attachPaths = (paths: readonly string[]): void => {
    setAttached((current) => {
      const all = [...new Set([...current, ...paths])]
      const left = all.length - MAX_ATTACHMENTS
      if (left > 0) setNote(`A message carries ${String(MAX_ATTACHMENTS)} files at most, so ${left === 1 ? 'one was' : `${String(left)} were`} not attached: ${all.slice(MAX_ATTACHMENTS).map((path) => path.slice(path.lastIndexOf('/') + 1)).join(', ')}.`)
      return all.slice(0, MAX_ATTACHMENTS)
    })
  }
  const pickFile = (path: string): void => {
    setAtAt(0)
    setValue(withoutAtQuery(value))
    if (attached.length >= MAX_ATTACHMENTS) {
      setNote(`A message can carry ${String(MAX_ATTACHMENTS)} files at most.`)
      return
    }
    setAttached((current) => [...new Set([...current, path])].slice(0, MAX_ATTACHMENTS))
    requestAnimationFrame(() => field.current?.focus())
  }
  const runSlash = (command: SlashCommand): void => {
    setSlashAt(0)
    // The person's own command fills the box with its prompt. Nothing is sent.
    if (command.action.kind === 'own') {
      const choice = ownCommandChoice(command.name, command.action.source, argumentsOfOwnCommand(value, [command.name])?.args ?? '')
      onChooseOwnCommand?.(choice.fill.name, choice.fill.source, choice.fill.args)
      return
    }
    if (command.action.kind === 'tune') {
      setValue('')
      onTune?.()
      return
    }
    if (command.action.kind === 'setup') {
      setValue('')
      onSetup?.()
      return
    }
    // The issue list. Choosing one fills the box. Nothing is sent.
    if (command.action.kind === 'issues') {
      setValue('')
      onPickIssue?.()
      return
    }
    // A runtime's own command is written into the box, where its arguments
    // go; sending it sends that command (0.426).
    if (command.action.kind === 'runtime') {
      setValue(`/${command.name} `)
      requestAnimationFrame(() => field.current?.focus())
      return
    }
    setValue('')
    switch (command.action.kind) {
      case 'mode':
        // Exactly what choosing the mode in the menu does, including the part
        // that is easy to miss: Auto is refused unless the workspace switch is
        // on, and App turns it straight back to Accept edits. A drive on
        // 2026-09-08 typed `/auto`, watched the box clear, and watched the
        // mode not change -- a command offered and then quietly undone, which
        // is the one thing this menu promises cannot happen.
        if (command.action.mode === 'auto' && autoMode !== true) onEnableAutoMode?.()
        onModeChange(command.action.mode)
        break
      case 'route':
        onOpenRoutePicker()
        break
      case 'stop':
        onCancel()
        break
      case 'swarm':
        onSwarmChange(!swarm)
        break
    }
  }
  const accounts = useAgentAccounts()
  const selected = runtimes.find((runtime) => runtime.id === route.runtime)
  // The account's window, from the latest reading (SURFACES-0.22 §3): a
  // sentence in the tooltip always; a dot on the chip only from 80%, when a
  // long run might not finish.
  const usageReading = usageWindows?.get(accountUsageKey(route.runtime, route.account))
  const usagePercentNow = usageReading === undefined ? undefined : usagePercent(usageReading)
  const usageSentence = usageReading === undefined
    ? undefined
    : usagePercentNow !== undefined && usagePercentNow >= 100
      ? `${usageWindowSentence(usageReading)}. This window's limit is used up.`
      : usagePercentNow !== undefined && usagePercentNow >= 80
        ? `${usageWindowSentence(usageReading)}. Long runs may be cut short.`
        : usageWindowSentence(usageReading)
  const usagePressing = usagePercentNow !== undefined && usagePercentNow >= 80
  // An agent the person added (0.777) is never discovered: the host finds it when the message goes, and says in the
  // thread, in words, when it cannot start it -- the chat box does not hold the message back on a check it cannot make.
  const selectedReady = route.account !== undefined ? accounts.some(a => a.id === route.account && a.runtime === route.runtime && a.state === 'ready') : route.runtime === 'acp' || (selected !== undefined && runtimeIsUsable(selected))
  // A runtime that could run this message even though the route does not
  // point at it. Only consulted when the selected one cannot.
  const readyElsewhere = selectedReady
    ? undefined
    : runtimes.find(
        (runtime) => runtimeIsUsable(runtime) && isMissionRuntime(runtime.id) && hostCanRunMission(runtime.id)
      )
  // A runtime can own a mission once the host can read its events. Readiness
  // still comes from discovery, so a route that is installed but signed out
  // cannot be started.
  const routeCanRun = hostCanRunMission(route.runtime)
  const canStart = selectedReady && routeCanRun && busyWith === undefined && value.trim().length > 0 && !referenceBusy
  // While a mission works, what you type is not lost and not sent into a run
  // that cannot hear it: it waits and goes as the next turn when this one
  // finishes. Before 0.20.0 the box was simply disabled, so the only way to
  // say the next thing was to stop the work first.
  //
  // Keyed on the RUN being live, not on a teammate being busy: a mission
  // that belongs to nobody -- which is every mission until someone makes a
  // teammate -- has no busy teammate, and the first version of this could
  // not be used at all on a fresh profile (steering smoke, 2026-09-05).
  const workingNow = running || busyWith !== undefined
  // An edited earlier message is never queued (0.500): the queue sends a plain reply, at the end.
  const canQueue = workingNow && queued === undefined && value.trim().length > 0 && editingEarlier === undefined
  const workingName = busyWith ?? 'this mission'

  // With a message queued the box is shut, and the caption under that
  // message already says when it goes: the box says how to change it.
  // Compare (0.441): the ticked models, or the comparison on screen, speak for the route chip.
  const comparing = asking !== undefined || (compare?.on === true && compare.picks.length > 0)
  // Compact now, from the ring (0.797, compactNow.ts): named for the conversation's runtime, not the chip's.
  const compactName = runtimeDisplayName(activeRoute?.runtime ?? route.runtime)
  const compact = context === undefined ? undefined : compactOffer({
    // A turn that is still starting has no route of its own yet, and it is on the route chip's (the compact drive
    // pressed the ring two seconds into "Starting Claude Code..." and nothing opened).
    runtime: activeRoute?.runtime ?? (workingNow ? route.runtime : undefined),
    nextRuntime: route.runtime,
    runtimeName: compactName,
    commands: runtimeCommands,
    running: workingNow,
    queued: queued !== undefined,
    elsewhere: comparing || cloud?.on === true || cloud?.computer?.on === true || background?.on === true || editingEarlier !== undefined
  })
  // The ring menu's breakdown (0.806): Claude Code's own /context for the last finished turn, read when the menu opens.
  useEffect(() => {
    let live = true
    setContextBreakdown(undefined)
    if (!contextOpen || workingNow || activeRoute?.runtime !== 'claude' || referenceMissionId === undefined) return
    void window.desktop?.readContextBreakdown?.(referenceMissionId).then((result) => { if (live) setContextBreakdown(result) }).catch(() => undefined)
    return () => { live = false }
  }, [contextOpen, workingNow, activeRoute?.runtime, referenceMissionId])
  /** Setting up a comparison (not asking one on screen): a chip per model, as Arena (0.460). */
  /*
   * Not while the picker is open on fewer than two (0.678): the chips took its place at the first tick, the
   * picker it was anchored in went with them, and "Pick two or three models ... Done" closed after one -- on a
   * first launch, with nothing recent to start from, the second model was then only to be had through "+".
   */
  const slotsShown = asking === undefined && compare?.on === true && compare.picks.length > 0 && !running
    && !(pickerOpen && compare.picks.length < MIN_COMPARE_SLOTS)
  const chordSeen = useRef(0)
  useEffect(() => {
    if (menuChord === undefined || menuChord.id === 0 || menuChord.id === chordSeen.current) return
    chordSeen.current = menuChord.id
    if (menuChord.which === 'mode') {
      setPickerOpen(false)
      setChatModeOpen(false)
      setSlotPicker(undefined)
      setModeOpen(true)
      return
    }
    setModeOpen(false)
    setChatModeOpen(false)
    if (menuChord.which === 'effort' && slotsShown) {
      setPickerOpen(false)
      setSlotEffort(0)
      return
    }
    if (menuChord.which === 'model' && slotsShown) {
      setPickerOpen(false)
      setSlotPicker(0)
      return
    }
    // which === 'model' opens the list; which === 'effort' opens the same list at its effort.
    setPickerFocus(menuChord.which === 'effort' ? 'effort' : 'model')
    setPickerOpen(true)
  }, [menuChord, slotsShown])
  /*
   * COMPARE CHANGES (0.445): while models are being picked, the mode chip
   * chooses between answers (Ask) and changes (Edit, each model in its own
   * copy). Once a comparison is on screen it is that comparison's, and fixed.
   */
  const compareEdits = asking !== undefined ? asking.changes === true : compare?.changes === true
  const compareModeOpen = comparing && asking === undefined && compare?.onChanges !== undefined
  // Direct, Compare or Blind: what the chat mode chip says (0.451).
  const chatMode: 'direct' | 'compare' | 'blind' | 'cloud' | 'computer' | 'background' =
    asking !== undefined ? (asking.blind === true ? 'blind' : 'compare') : background?.on === true ? 'background' : cloud?.computer?.on === true ? 'computer' : cloud?.on === true ? 'cloud' : compare?.on === true ? (compare.blind === true ? 'blind' : 'compare') : 'direct'
  const versus = asking?.label ?? versusLabel((compare?.picks ?? []).map((pick) => pick.label))
  // One column left answering (the others could not start) is asked as one.
  const placeholder = background?.on === true && asking === undefined ? 'Say what Claude should keep working on…' : cloud?.computer?.on === true && asking === undefined ? 'Describe a task for your cloud computer…' : cloud?.on === true && asking === undefined ? (cloud.where === 'claude' ? 'Describe a task for Claude’s cloud…' : cloud.where === 'computer' ? 'Describe a task for your cloud computer…' : 'Describe a task for Codex Cloud…') : asking !== undefined ? (asking.columns <= 1 ? 'Ask a follow-up…' : `Ask ${asking.columns === 2 ? 'both' : `all ${String(asking.columns)}`}…`) : workingNow
    ? queued === undefined
      ? `Say what is next — it goes to ${workingName} when this finishes…`
      : 'Edit the queued message to change it…'
    : !routeCanRun
      ? `Locust cannot run ${selected?.displayName ?? 'that AI agent'} fully yet — pick another model to start…`
      : selectedReady
        ? // Just the invitation. Three of these used to restate the
          // permission mode -- "it may edit files in this workspace" -- which
          // the mode control says in two words directly underneath, so the
          // box was the second voice saying the same thing (Colin,
          // 2026-09-05). Every OTHER line here survives, because each says
          // something no other part of the screen does.
          // Asking several says so; asking one is the sentence it always was.
          teammateName === undefined ? 'Write a message…' : `Message ${teammateName}…`
        : discoveryPhase === 'loading'
          ? 'Looking for the AI agents on this machine…'
          : discoveryPhase === 'error'
            ? 'Locust could not look for AI agents on this machine…'
            : // Something on this machine CAN run; it just is not the route
              // this box is pointing at. Telling the person to install a
              // coding agent when they have just installed one -- and been
              // told by Settings to -- is the wall the whole first run hits
              // (QA, 2026-09-06, reproduced live). The route normally moves
              // itself; this covers the case where the person has chosen one
              // deliberately and it has since stopped being usable.
              readyElsewhere !== undefined
              ? `${readyElsewhere.displayName} is ready — pick it in the model menu…`
              : // "and sign in" only when there is something to sign in to.
                // Pass 4 took this instruction off the roster; pass 5 found
                // it still in the box you type into, two inches under a
                // welcome saying OpenCode needs no account (Grok, 0.154.0).
                // And "installed" was not enough either: five CLIs that
                // hang on PATH count as installed, and the box then asked
                // for a sign-in beside a row saying no account is needed
                // (Grok, pass 11). Only a runtime that has SAID it wants a
                // sign-in earns the words.
                runtimes.some((runtime) => runtime.status === 'auth-required' || runtime.auth === 'unauthenticated')
                ? 'Install an AI agent and sign in to start…'
                : // And "install" is wrong under five rows that say installed
                  // and not answering (Fable, pass 1, finding 9): the box
                  // names what the screen above it offers, Check again.
                  runtimes.some((runtime) => runtime.installed)
                  ? runtimesGaveUp
                    ? /*
                       * COUNTED, because the rest of the screen is plural.
                       *
                       * This said "A coding agent is installed but not
                       * answering" over FIVE rows that each said so and each
                       * carried their own Check again (Grok, pass 16, the
                       * extra finding). The list was plural, the buttons were
                       * plural, and the box you type into was not.
                       *
                       * Only the ones this sentence is ABOUT: installed, and
                       * still not usable after discovery gave up. A machine
                       * with one working CLI and one hung one is not
                       * "2 coding agents are not answering".
                       */
                      (() => {
                        const silent = runtimes.filter(
                          (runtime) => runtime.installed && !runtimeIsUsable(runtime)
                        ).length
                        return silent === 1
                          ? 'An AI agent is installed but not answering — Check again above…'
                          : `${String(silent)} AI agents are installed but not answering — Check again above…`
                      })()
                    : // Installed and still being asked: say that, not "install".
                      'Checking the AI agents on this machine…'
                  : 'Install an AI agent to start…'

  /** Set by Ctrl+Enter for the submit it starts: stop the run and send this now (0.485). */
  const sendNow = useRef(false)
  const queueing = useRef(false)
  const submit = (submitEvent: FormEvent<HTMLFormElement>): void => {
    submitEvent.preventDefault()
    if (queueing.current || commandAsk !== undefined || issueList !== undefined) return
    if (referenceBusy) { setNote('Wait for the references to be ready. Your message is still in the box.'); return }
    const drafted = bangDraft(value)
    if (!terminalOff && drafted !== undefined) {
      if (drafted.command.length > 0) onRunCommand?.(drafted.command)
      setValue('')
      return
    }
    const now = sendNow.current
    sendNow.current = false
    const typed = value.trim()
    // Notes alone are a message: the person may have nothing to add to them.
    // A pointed part of the page is one too, notes and all.
    const notes = diffNotes ?? []
    const pointed = messageOfPagePoints(pagePoints ?? [])
    const body = [pointed.text, typed].filter((part) => part.length > 0).join('\n\n')
    const prompt = withDiffNotes(body, notes)
    if (prompt.length === 0) return
    const files = [...pointed.attachments, ...attached].slice(0, MAX_ATTACHMENTS)
    // The tagged are sent it first, while the conversation on screen is still
    // the one they came from; with nobody on screen they are who it goes to.
    if (tagged.length > 0 && onTag !== undefined) {
      // Who it went to is a report, not a refusal: plain, with the team's icon.
      void onTag(tagged, withAttachments(prompt, files)).then((said) => {
        if (said !== undefined) setRefusal({ text: said.text, ...(said.sent ? { plain: true, icon: 'users' as const } : {}) })
      })
      setTagged([])
      if (currentTeammateId === undefined) {
        setValue('')
        setAttached([])
        onPagePointsSent?.()
        if (notes.length > 0) onClearDiffNotes?.()
        return
      }
    }
    if (editingEarlier !== undefined && workingNow) {
      setNote(`Not sent: ${workingName === 'this mission' ? 'this teammate' : workingName} is still working. Your edit stays in the box -- send it once they finish.`)
      return
    }
    if (canQueue) {
      // With its files, and the tiles cleared, as a send does. L20 (the code
      // review): the queued row dropped them, and the tiles then rode along
      // on the next, unrelated message.
      queueing.current = true
      void Promise.resolve(onQueue(prompt, files, now && onSendQueuedNow !== undefined)).then((saved) => {
        if (saved === false) return
        // A disk acknowledgement must not erase a newer thought typed meanwhile.
        if (valueNow.current === value) setValue('')
        setAttached([])
        onPagePointsSent?.()
        if (notes.length > 0) onClearDiffNotes?.()
      }).finally(() => { queueing.current = false })
      return
    }
    if (!canStart) {
      // Silence here is the single worst thing this box can do: the person
      // types, presses Enter, and the app neither sends nor explains. It cost
      // the whole first run in the QA pass. Every refusal now names itself.
      setNote(
        !routeCanRun
          ? `Locust cannot run ${selected?.displayName ?? 'that AI agent'} fully yet, so it cannot start here. Pick another model.`
          : readyElsewhere !== undefined
            ? `This message would go to ${selected?.displayName ?? 'the AI agent picked'}, which is not ready. ${readyElsewhere.displayName} is — pick it in the model menu.`
            : 'No AI agent on this machine can start yet. Settings lists what to install.'
      )
      return
    }
    setRefusal(undefined)
    // Newest last, and never the same line twice running -- pressing up
    // after sending the same thing twice should go back one message, not
    // one keystroke.
    if (sent.current[sent.current.length - 1] !== typed && typed.length > 0) sent.current = [...sent.current, typed].slice(-50)
    setRecallAt(-1)
    // Cleared NOW, not when the host answers. The turn is already on screen as
    // a bubble the instant it is sent, so waiting for the round trip left the
    // same sentence in two places for the whole "Starting..." window and read
    // as lag (Colin, 2026-09-05, screenshot). If the send never happened at
    // all, the words come straight back rather than being lost.
    setValue('')
    const sending = attached
    setAttached([])
    const handoffOptions = continuation === undefined || leaveOut.length === 0 ? undefined : { leaveOut }
    void onStart(withAttachments(prompt, files), referenceIds.length === 0 ? handoffOptions : { ...handoffOptions, referenceIds }).then((started) => {
      if (started !== true) {
        setValue(typed)
        setAttached(sending)
        // M27: and say why, when the host said.
        if (typeof started === 'string') setNote(started)
        return
      }
      // Gone with the message; kept, for another try, when it did not go.
      setReferencesSent((value) => value + 1)
      onPagePointsSent?.()
      if (notes.length > 0) onClearDiffNotes?.()
    })
  }

  /**
   * Ctrl+V a file or a screenshot straight into the message.
   *
   * Colin, 2026-09-10: "lets add the ability to ctrl+v a file or photo into
   * the chat."
   *
   * A pasted SCREENSHOT is what decides the shape. The clipboard holds a
   * bitmap, not a file, so there is no path anywhere for the + button's route
   * to take -- the bytes have to travel. Electron also stopped exposing
   * `File.path` some versions ago, so a real file copied out of Explorer
   * arrives with no path either. One route serves both: read the bytes, let
   * the host name and place them.
   *
   * Text pastes fall through untouched. `clipboardData.files` is empty for
   * those, so pasting a paragraph still just types it.
   */
  /**
   * THE WHOLE BOX TAKES A CLICK.
   *
   * Colin, 2026-09-23: "only a very small portion of the chat window is
   * actually clickable to begin chatting". The field is one line at the top
   * of a box six times its height -- measured, 728x16 inside 760x108 -- and a
   * press on the padding or between the chips did nothing: 61 of 468 points
   * off the controls started a message (probe-composer-click). Claude Code's
   * input takes a click anywhere in its frame. So a press on the box that is
   * not on one of its controls puts the caret in the field, at the end; a
   * control keeps its press, and a press on the field itself is the field's.
   */
  const field = useRef<HTMLTextAreaElement>(null)
  // Start here on the folder question sends through the form itself, so it goes exactly as Send would (0.790).
  const formRef = useRef<HTMLFormElement>(null)
  // Handed back (C9): the words and files return to the box, ready to send.
  useEffect(() => {
    if (handBack === undefined) return
    setValue(handBack.text)
    setAttached(handBack.attachments)
    field.current?.focus()
  }, [handBack])
  // A quoted part of a reply: above what is typed, the caret after it all.
  useEffect(() => {
    if (quoteIn === undefined) return
    setValue((current) => (current.trim().length === 0 ? `${quoteIn.quote}\n\n` : `${quoteIn.quote}\n\n${current}`))
    // A part of a page comes with its picture (0.484).
    if (quoteIn.attachments !== undefined && quoteIn.attachments.length > 0) attachPaths(quoteIn.attachments)
    requestAnimationFrame(() => {
      const input = field.current
      if (input === null) return
      input.focus()
      input.setSelectionRange(input.value.length, input.value.length)
    })
  }, [quoteIn])
  useEffect(() => {
    if (attachIn === undefined || attachIn.paths.length === 0) return
    attachPaths(attachIn.paths)
    field.current?.focus()
  }, [attachIn])
  useEffect(() => {
    if (told === undefined) return
    setNote(told.text)
  }, [told])
  const pressBox = (event: MouseEvent<HTMLDivElement>): void => {
    if (event.button !== 0) return
    const target = event.target as HTMLElement
    const input = field.current
    if (input === null || input.disabled || target === input) return
    if (target.closest('button, a, input, select, textarea, [role="button"], [role="menu"], [role="menuitem"], [role="listbox"], [role="option"]') !== null) return
    // Kept from landing on the box, where it would only leave again.
    event.preventDefault()
    input.focus()
    input.setSelectionRange(input.value.length, input.value.length)
  }

  const paste = (event: ClipboardEvent<HTMLTextAreaElement>): void => {
    const bridge = window.desktop
    const files = [...(event.clipboardData?.files ?? [])]
    if (bridge === undefined || files.length === 0 || running) return
    // Only once there is something to attach: calling this on every text
    // paste would swallow the paste.
    event.preventDefault()
    setAttaching(true)
    void (async () => {
      for (const file of files.slice(0, MAX_ATTACHMENTS)) {
        const bytes = new Uint8Array(await file.arrayBuffer())
        // A pasted bitmap has no name of its own -- the browser calls it
        // `image.png` at best -- so it gets one that says where it came from
        // and cannot collide with a file the person chose.
        const named = file.name.length > 0 ? file.name : `pasted-${String(Date.now())}.png`
        const answer = await bridge.attachPasted(named, bytes)
        if (answer.ok) {
          attachPaths(answer.paths)
          if (answer.copied !== undefined && answer.copied.length > 0) {
            setCopiedIn((current) => new Set([...current, ...(answer.copied ?? [])]))
          }
        } else if (answer.message.length > 0) {
          setNote(answer.message)
        }
      }
    })()
      .catch(() => setNote('That could not be attached. Your message is untouched.'))
      .finally(() => setAttaching(false))
  }

  const keyDown = (keyEvent: KeyboardEvent<HTMLTextAreaElement>): void => {
    // Every key belongs to an input method while it is composing (code review
    // B4, renderer-thread (a)): Enter there confirms a Korean or Japanese
    // word, and was sending the half-typed message; Escape cancels the
    // composition, and was stopping the run. 229 is the key code an IME
    // reports where `isComposing` is not set.
    if (keyEvent.nativeEvent.isComposing || keyEvent.keyCode === 229) return
    // The allow card is the decision. Enter must not send the command's name.
    if (commandAsk !== undefined) {
      if (keyEvent.key === 'Escape') {
        keyEvent.preventDefault()
        commandAsk.onNotNow()
      } else if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
        keyEvent.preventDefault()
      }
      return
    }
    // The issue list has the keys. Enter here must not send what was already in the box.
    if (issueList !== undefined) {
      if (keyEvent.key === 'Escape') {
        keyEvent.preventDefault()
        issueList.onClose()
      } else if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
        keyEvent.preventDefault()
      }
      return
    }
    // `!` is a command of the person's. Escape leaves that mode. Enter runs it
    // here, and does not send a message.
    if (commandMode) {
      if (keyEvent.key === 'Escape') {
        keyEvent.preventDefault()
        setValue(leaveCommandMode(value))
        return
      }
      if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
        keyEvent.preventDefault()
        const drafted = bangDraft(value)
        if (drafted !== undefined && drafted.command.length > 0) onRunCommand?.(drafted.command)
        setValue('')
        return
      }
    }
    // While the slash menu is open the arrows and Enter belong to it. Enter
    // must NOT fall through to submit: sending "/plan" as a message to a
    // teammate is the one outcome this feature exists to prevent.
    if (slashChoices.length > 0) {
      if (keyEvent.key === 'ArrowDown') {
        keyEvent.preventDefault()
        setSlashAt((at) => (at + 1) % slashChoices.length)
        return
      }
      if (keyEvent.key === 'ArrowUp') {
        keyEvent.preventDefault()
        setSlashAt((at) => (at - 1 + slashChoices.length) % slashChoices.length)
        return
      }
      if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
        keyEvent.preventDefault()
        const chosen = slashChoices[Math.min(slashAt, slashChoices.length - 1)]
        if (chosen !== undefined) runSlash(chosen)
        return
      }
      if (keyEvent.key === 'Escape') {
        keyEvent.preventDefault()
        setValue('')
        return
      }
    }
    // The `@` list, the same way (0.436): Enter or Tab attaches the file.
    if (atChoices.length > 0) {
      if (keyEvent.key === 'ArrowDown') {
        keyEvent.preventDefault()
        setAtAt((at) => (at + 1) % atChoices.length)
        return
      }
      if (keyEvent.key === 'ArrowUp') {
        keyEvent.preventDefault()
        setAtAt((at) => (at - 1 + atChoices.length) % atChoices.length)
        return
      }
      if ((keyEvent.key === 'Enter' && !keyEvent.shiftKey) || keyEvent.key === 'Tab') {
        keyEvent.preventDefault()
        const chosen = atChoices[Math.min(atAt, atChoices.length - 1)]
        if (chosen !== undefined) pickChoice(chosen)
        return
      }
      if (keyEvent.key === 'Escape') {
        keyEvent.preventDefault()
        setAtDismissed(value)
        return
      }
    }
    /*
     * Shift+Tab cycles the permission mode, the way it does in Claude Code.
     *
     * The mode is the highest-consequence control on this screen -- it is
     * the difference between a teammate that explains and one that writes --
     * and reaching it meant a menu every time. Tab alone is left alone,
     * because moving focus out of a text box is what Tab is for everywhere.
     *
     * Only through modes this route can ACTUALLY run. `modeUnavailableReason`
     * is what the menu greys an option out with, so cycling past those keeps
     * the key from landing somewhere the menu would have refused -- offered
     * and then refused is the pattern this app keeps paying for.
     */
    if (keyEvent.key === 'Tab' && keyEvent.shiftKey) {
      const usable = MODES.filter(
        (option) => policyModeAllowed(policy, option.mode) && modeUnavailableReason(option.mode, route.runtime, platform) === undefined
      ).map((option) => option.mode)
      const next = shiftTabMode(mode, usable, autoMode === true)
      if (next !== undefined) {
        keyEvent.preventDefault()
        onModeChange(next)
        return
      }
    }
    /*
     * Escape stops the run, the way it does in Claude Code.
     *
     * Only while this box has focus and no panel is open. A panel's own
     * Escape closes it -- and a keystroke that both dismissed a menu and
     * killed a mission would be the worst kind of surprise, since one of
     * those is free and the other is not.
     */
    if (keyEvent.key === 'Escape' && running && !modeOpen && !pickerOpen) {
      keyEvent.preventDefault()
      onCancel()
      return
    }
    /*
     * The up arrow brings back what you sent.
     *
     * Only from an empty box, or once already walking the history. This is a
     * TEXTAREA and messages here are often several lines, so an up arrow in
     * the middle of one has to keep moving the caret -- which is also what
     * Claude Code does.
     */
    if (keyEvent.key === 'ArrowUp' && sent.current.length > 0 && (value.length === 0 || recallAt >= 0)) {
      keyEvent.preventDefault()
      const next = recallAt < 0 ? sent.current.length - 1 : Math.max(0, recallAt - 1)
      setRecallAt(next)
      setValue(sent.current[next] ?? '')
      return
    }
    if (keyEvent.key === 'ArrowDown' && recallAt >= 0) {
      keyEvent.preventDefault()
      const next = recallAt + 1
      // Past the newest is the empty box you started from, not a wrap.
      if (next >= sent.current.length) {
        setRecallAt(-1)
        setValue('')
        return
      }
      setRecallAt(next)
      setValue(sent.current[next] ?? '')
      return
    }
    if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
      // shortcut:newline Shift+Enter is the new line, so it is not taken here.
      keyEvent.preventDefault()
      // Ctrl+Enter while a teammate works: Claude Code's send-now. It queues
      // like Enter, then stops the run and sends at once (0.485).
      sendNow.current = (keyEvent.ctrlKey || keyEvent.metaKey) && canQueue && onSendQueuedNow !== undefined
      keyEvent.currentTarget.form?.requestSubmit()
    }
  }

  // Effort is offered ONLY where the chosen model says it is supported. The
  // design's rule is that an unsupported effort must show as unsupported
  // rather than be sent as a silent no-op.

  // While a mission runs, the control states what IT is on. Otherwise it
  // states what the next mission will use -- which is what the person just
  // picked. Reading the live run's model when nothing is running left a
  // finished mission's `account-default` on screen over a chosen model.
  const shownRuntime = running ? activeRoute?.runtime ?? route.runtime : route.runtime
  const shownModel = running ? activeRoute?.model ?? route.model : route.model
  // Keyed to the route the chip is SHOWING, which while a mission runs is the
  // live one rather than the next one. It used to read the next route's
  // levels under the live route's name -- two models on one chip. The control
  // is disabled during a run, so this is display only and has to agree with
  // the label beside it; when nothing is running the two are the same route.
  // Through `modelFamily`, not a bare id match: once a run resolves a Cursor
  // route to a variant, an id match finds no family and the chip vanishes.
  /*
   * What this model reports -- and, separately, whether we have been told yet.
   *
   * These were one value, and an empty list meant both "this model has no
   * levels" and "the catalog has not arrived". So a composer opened before the
   * model list loaded stated "effort · fixed", which reads as a fact about the
   * runtime ("it uses its own") when nothing was known at all. Caught by the
   * approve-each drive on 2026-09-08: three runs of the same seed showed
   * `medium`, then `low`, then `effort · fixed`.
   */
  const shownFamily = modelFamily(models, shownRuntime, shownModel)
  const supportedEfforts = shownFamily?.supportedEfforts ?? []
  /** Only true once the catalog has actually answered for THIS model. */
  const effortIsGenuinelyFixed = shownFamily !== undefined && shownFamily.supportedEfforts.length === 0
  // Swarm means "this model's maximum", and the catalog orders efforts lowest
  // to highest, so the maximum is the last one THIS model reported -- not a
  // fixed name that some models do not have.
  const swarmEffort = supportedEfforts[supportedEfforts.length - 1]
  // What the chip states. Swarm overrides it, and otherwise an unchosen
  // level shows the model's default rather than nothing -- the same value
  // App.tsx sets on a route change, so the chip says what the run gets.
  /*
   * The effort a VARIANT ID already encodes, when no separate one was chosen.
   *
   * Cursor states its effort in the model name -- `cursor-grok-4.6-high` and
   * `cursor-grok-4.6-low` are two ids for one model -- so a route carrying
   * only that id already says which level it is. Falling straight through to
   * `defaultEffort` made a teammate stored on `cursor-grok-4.6-high` read as
   * `medium` and, worse, run as medium (MEASURED 2026-09-11: seeded `-high`,
   * the panel said `medium`). The catalog's own variant map is the answer;
   * nothing is guessed from the string.
   */
  const effortOfShownId = Object.entries(shownFamily?.variants ?? {})
    .find(([, id]) => id === shownModel)?.[0]
  const shownEffort = swarm ? swarmEffort : effort ?? effortOfShownId ?? defaultEffort(supportedEfforts, shownFamily?.defaultEffort)
  // The scale this model actually offers, and where the current level sits on
  // it. Four stops and a switch rather than eight rows; see `effortScale.ts`.
  const { bases: effortBases, hasFast: effortHasFast } = effortScale(supportedEfforts)
  const { base: effortBase, fast: effortIsFast } = splitEffort(shownEffort ?? effortBases[0] ?? '')
  const effortIndex = Math.max(0, effortBases.indexOf(effortBase))
  const effortPhrase = comparing
    ? undefined
    : effortIsGenuinelyFixed
      ? 'Fixed'
      : shownEffort !== undefined && supportedEfforts.length > 0
        ? `${effortName(effortBase)}${effortIsFast ? ' · Fast' : ''}`
        : undefined
  const shownRuntimeStatus = runtimes.find((runtime) => runtime.id === shownRuntime)
  /** Nothing on this machine can take work, so no route is a real answer. */
  const nothingConnected = !runtimes.some(runtimeIsUsable) && !accounts.some(a => a.state === 'ready')
  // The chip is a label on a control, not the Settings row that tells you
  // which program this is -- so `Cursor`, not `Cursor Agent`. See `routeName`.
  const runtimeLabel = shortRuntimeName(shownRuntime)
  // The whole thing, for the tooltip: a chip drops repetition, and the exact
  // string you would type somewhere else must still be reachable.
  /*
   * WHICH MODEL "ITS DEFAULT" IS, once a run has said (0.797). A route on the account's default reads as the runtime
   * alone, "Claude" (0.723) -- and a turn Locust carried on when Codex's plan ran out read "Claude  High" with nothing
   * to say what it ran on (Colin, 2026-10-19: "why does it just say claude high when i do a handoff?"). The runtime
   * reports the model its default turned out to be on every run (resolvedModels), so the chip names it.
   */
  const defaultResolvedId = shownModel === 'account-default' ? resolvedModels.get(`${shownRuntime}:account-default`) : undefined
  const defaultModelName = defaultResolvedId === undefined ? undefined : routeModelName(shownRuntime, defaultResolvedId)
  const exactRoute = shownModel === 'account-default'
    ? `${shownRuntimeStatus?.displayName ?? runtimeDisplayName(shownRuntime)} / its own default model${defaultModelName === undefined ? '' : `, ${defaultModelName} on its last run`}`
    : `${shownRuntimeStatus?.displayName ?? runtimeDisplayName(shownRuntime)} / ${shownModel}`
  // A family known only through its effort variants is listed under its
  // family name; showing the stand-in variant's id ("cursor-grok-4.6-high-fast")
  // beside "effort · low" read as two different answers (2026-09-06). Found
  // through its variants as well, so a route on a sibling id -- a teammate
  // saved on `claude-opus-5-5-high` -- reads "Claude Opus 5.5 1M" too, not
  // its own id spelled out.
  const namedModel = routeModelName(
    shownRuntime,
    shownFamily?.variants !== undefined ? shownFamily.displayName : shownModel,
    resolvedModels.get(`${shownRuntime}:${shownModel}`)
  )
  /*
   * Not twice on one row. Cursor carries the effort INSIDE the id
   * (`cursor-grok-4.6-medium`), and the control immediately to the right of
   * this chip is the effort. Normally the catalog resolves the family and the
   * question never arises; when it has not loaded yet, the id is all there is
   * and the row read `Grok 4.6 Medium` beside a slider saying `medium`.
   */
  const effortWord = (shownEffort ?? '').split('-')[0] ?? ''
  const trailing = effortWord.length === 0 ? '' : ` ${effortWord.charAt(0).toUpperCase()}${effortWord.slice(1)}`
  const modelLabel =
    trailing.length > 0 && namedModel.endsWith(trailing) && namedModel.length > trailing.length
      ? namedModel.slice(0, -trailing.length)
      : namedModel
  /*
   * "Free" is its own tag, outside the part that truncates.
   *
   * The name ends in it -- "Muse Spark 1.3 Contributor Free" -- and the chip
   * cuts names at 18 characters, so the one word that says this route costs
   * nothing was the word cut: "OpenCode / Muse Spark 1.3 C..." on every free
   * run (Yurt's beta report, #12; the 0.268 design recheck asked for this).
   */
  const { name: modelName, free: freeModel } = freeTagOf(modelLabel)
  // A model of the person's own is named by the name they gave it, with no
  // "OpenCode /" in front (routeChrome, 0.361); the tooltip still says both.
  const ownModel = isOwnRoute(shownModel)
  // What the RUNNING mission is actually on, which is not always what the
  // composer's next-run route says. A handoff has to be measured against the
  // live run, or picking "the same" route would still stop it.
  const handoff = handoffAvailability(running, activeRoute !== undefined, handingOff)
  const activeChoice: RouteChoice =
    activeRoute === undefined
      ? route
      : { runtime: activeRoute.runtime, model: activeRoute.model, ...(routeAccount(activeRoute) === undefined ? {} : { account: routeAccount(activeRoute) }) }

  /** Attaching, as the + menu's first satellite does it. */
  const attachFiles = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setAttaching(true)
    void bridge
      .attachFiles()
      .then((answer) => {
        if (answer.ok) {
          // Deduplicated and capped: the same file twice is one
          // reference, and the cap is what keeps a stray
          // multi-select out of the prompt budget.
          attachPaths(answer.paths)
          /*
           * The "copied in" fact goes on the TILE, not in a note.
           *
           * It was a full-width bordered box above the composer,
           * the same shape as a text input and directly above
           * one -- two objects for one event, three stacked rows
           * above the box, and a transient object carrying a
           * permanent fact. The file stays copied for as long as
           * the tile exists, so dismissing the note lost
           * something still true (design, 2026-09-08).
           *
           * Remembered across picks rather than replaced: two
           * separate attaches each copying one file must leave
           * both tiles marked.
           */
          if (answer.copied !== undefined && answer.copied.length > 0) {
            setCopiedIn((current) => new Set([...current, ...(answer.copied ?? [])]))
          }
        } else if (answer.message.length > 0) {
          setNote(answer.message)
        }
      })
      .catch(() => setNote('Those files could not be attached. Your message is untouched.'))
      .finally(() => setAttaching(false))
  }

  // Listening, typing your words, or off: the glow's state (voiceLevel.ts).
  const [voice, setVoice] = useState<VoiceState>(voiceState.get())
  useEffect(() => voiceState.subscribe(setVoice), [])
  return (
    /*
     * THE CHAT BOX GLOWS WITH YOUR VOICE (0.686, voice-glow, MIT) -- around the visible box (0.687), below. Mounted always, so starting to listen never
     * remounts the box under your cursor; `active` only while listening (the button says "Typing…" after), and an inactive
     * beam registers no animation at all (read in its source), so the box at rest costs nothing. The level is
     * voice typing's own meter: the glow never opens the microphone a second time. `reach` 1.5: the box sits ~15 px
     * above the window's edge, and the glow is centered on its bottom edge, so only what rises into the box shows.
     */
    <div className="lc-composer">
      <div className="lc-composer__inner">
        {error !== undefined && (
          <div className="lc-notice" role="status" aria-live="polite">
            <Icon name="shield" size={13} />
            {error}
          </div>
        )}
        {/*
          * Why the last Enter did nothing. Same surface as the host's own
          * errors: one place the box speaks, rather than a second voice.
          */}
        {/* A connector named while this teammate's are off (0.798, connectorsOff.ts): said before the message goes. */}
        {refusal === undefined && connectorsOff !== undefined && (() => {
          const named = value.trim().length === 0 ? undefined : connectorMentioned(value, connectorsOff.names)
          return named === undefined ? null : (
            <div className="lc-notice is-plain" role="status" aria-live="polite">
              <Icon name="plug" size={13} />
              {connectorsOffHint(connectorsOff.teammate, named)}
            </div>
          )
        })()}
        {/* A free model OpenCode dropped (0.819): said before the send, with the one press that moves off it. */}
        {refusal === undefined && modelDropped !== undefined && (
          <div className="lc-notice is-plain is-offer" role="status" aria-live="polite">
            <Icon name="route" size={13} />
            <span className="lc-notice__text">{modelDropped.text}</span>
            <button type="button" className="lc-button" onClick={modelDropped.onPress}>{modelDropped.label}</button>
          </div>
        )}
        {refusal !== undefined && (
          <div className={`lc-notice${refusal.plain === true ? ' is-plain' : ''}`} role="status" aria-live="polite">
            {/* The shield says "refused". A note about what the host DID is
                not a refusal, so it gets the icon for the thing it is. */}
            <Icon name={refusal.icon ?? (refusal.plain === true ? 'file' : 'shield')} size={13} />
            {refusal.text}
          </div>
        )}
        {/*
          * A QUEUED MESSAGE IS YOUR NEXT MESSAGE, WAITING.
          *
          * Colin, 2026-09-23, with a frame of it: "lets clean this up, looks
          * a bit trashy compared to the rest of the app". It was a label in
          * the mono face at body size, the message at full strength beside
          * it, a caption in mono under that, a lone rule down the side and
          * two buttons floating at the far right -- five voices for one
          * fact, over a box saying the same "when Pip finishes" again.
          *
          * Claude Code shows a queued message as the person's own message,
          * dimmed, just over the box. Here that is the thread's own bubble,
          * on the person's side and outlined where a sent one is filled --
          * what you will see in the thread once it goes -- and one quiet line
          * under it: when it goes, or why it is held, and what can be done.
          */}
        {fromLink && editingEarlier === undefined && (
          <div className="lc-queued is-editing" role="status" aria-live="polite">
            <div className="lc-queued__meta">
              <Icon name="link" size={12} />
              <span className="lc-queued__note">From a link — read it before you send it.</span>
              <button type="button" className="lc-queued__action" onClick={() => { setValue(''); setFromLink(false) }}>
                Clear
              </button>
            </div>
          </div>
        )}
        {/*
          * BEFORE THE FIRST CONVERSATION IN A FOLDER LIKE .claude (0.790): asked here, over the box, once per folder.
          * Start here keeps the folder and sends the words still in the box, as Send would; Choose another folder
          * opens the picker, and the words wait.
          */}
        {folderQuestion !== undefined && (
          <div className="lc-queued is-editing lc-folderask" role="group" aria-label={`Start this conversation in ${folderQuestion.name}?`}>
            <div className="lc-queued__meta">
              <Icon name="folder" size={12} />
              <span className="lc-queued__note">
                <span className="lc-folderask__question">Start this conversation in {folderQuestion.name}?</span>{' '}
                This is {folderQuestion.why}, and a teammate here reads it as its project, sign-in files included.
              </span>
              <button type="button" className="lc-queued__action" onClick={onChooseFolder}>
                Choose another folder
              </button>
              <button type="button" className="lc-queued__action" onClick={() => {
                void onConfirmFolder?.().then((kept) => { if (kept) formRef.current?.requestSubmit() })
              }}>
                Start here
              </button>
            </div>
          </div>
        )}
        {editingEarlier !== undefined && (
          <div className="lc-queued is-editing" role="status" aria-live="polite">
            <div className="lc-queued__meta">
              <Icon name="pencil" size={12} />
              <span className="lc-queued__note">
                {editingEarlier.filesHeld !== undefined
                  ? `Editing an earlier message — sending starts again from there. Files stay as they are: ${editingEarlier.filesHeld.cannot === 1 ? '1' : String(editingEarlier.filesHeld.cannot)} of the ${String(editingEarlier.filesHeld.total)} files the replies changed cannot be put back exactly, and putting back only the others could leave the project half changed.`
                  : editingEarlier.files === undefined
                    ? 'Editing an earlier message — sending starts again from there. Files stay as they are.'
                    : 'Editing an earlier message — sending starts again from there.'}
              </span>
              {editingEarlier.files !== undefined && (
                <label className="lc-queued__check">
                  <input type="checkbox" checked={editingEarlier.files.on} onChange={editingEarlier.files.onToggle} />
                  {`Also put back the ${String(editingEarlier.files.count)} ${editingEarlier.files.count === 1 ? 'file' : 'files'} the replies after it changed`}
                </label>
              )}
              <button type="button" className="lc-queued__action" onClick={editingEarlier.onCancel}>
                Cancel
              </button>
            </div>
          </div>
        )}
        {queued === undefined && offTheQueue && editingEarlier === undefined && (
          <div className="lc-queued is-editing" role="status" aria-live="polite">
            <div className="lc-queued__meta">
              <Icon name="clock" size={12} />
              <span className="lc-queued__note">
                {workingNow
                  ? `Off the queue while you edit — Enter queues it again for when ${workingName} finishes`
                  : 'Off the queue — Enter sends it'}
              </span>
            </div>
          </div>
        )}
        {queueError !== undefined && queued === undefined && <div className="lc-queued is-editing" role="status">{queueError}</div>}
        {queued !== undefined && (
          <div className="lc-queued" role="status" aria-live="polite">
            <div className="lc-queued__bubble">
              <span className="lc-queued__text">{queued}</span>
            </div>
            <div className="lc-queued__meta">
              <Icon name="clock" size={12} />
              {/*
                * The reason, or what will happen -- never a description of
                * the button. "ready to send" was the one state a person could
                * not act on intelligently (design pass, objection 1), and it
                * is unreachable: a queue that is ready has already gone. The
                * count only once there is more than one: several lines typed
                * during one run go as one message.
                */}
              <span className={`lc-queued__note${queuedNote === undefined ? '' : ' is-held'}`}>
                {queuedCount !== undefined && queuedCount > 1 ? `${String(queuedCount)} messages, sent as one · ` : ''}
                {queuedNote === undefined
                  ? `Sends when ${workingName} finishes`
                  : `${queuedNote.charAt(0).toUpperCase()}${queuedNote.slice(1)}`}
              </span>
              {workingNow && onSendQueuedNow !== undefined && (
                /* Claude Code's ctrl+enter, as a button: the run stops and
                   this goes now, rather than when it finishes (0.485). */
                <button type="button" className="lc-queued__action" title={`Stops ${workingName} and sends this now (Ctrl+Enter while typing)`} onClick={onSendQueuedNow}>
                  Stop and send now
                </button>
              )}
              {!workingNow && (
                <button type="button" className="lc-queued__action" onClick={onSendQueued}>
                  {/* It always goes into the conversation ON SCREEN, so where
                      that is not the one it was typed at, the button says so
                      rather than reading as "send it where it was going". */}
                  {queuedElsewhere ? 'Send here' : 'Send now'}
                </button>
              )}
              {/*
                * Fixing one word meant discarding the sentence and retyping
                * it from memory: while a message is queued the box is
                * disabled, so Discard was the only way back to the text
                * (design pass, gap 2). Edit is those same two operations in
                * the order people want them.
                */}
              <button
                type="button"
                className="lc-queued__action"
                onClick={() => {
                  void Promise.resolve(onUnqueue()).then((saved) => {
                    if (saved === false) return
                    setValue(queued)
                    setAttached(queuedAttachments ?? [])
                    setOffTheQueue(true)
                  })
                }}
              >
                Edit
              </button>
              <button type="button" className="lc-queued__action" onClick={onUnqueue} aria-label="Discard the queued message">
                Discard
              </button>
            </div>
          </div>
        )}
        {continuationNote !== undefined && (
          <div className="lc-continuation lc-mono" role="status">
            <Icon name="route" size={12} />
            <span>
              {continuationNote}
              {carried?.kind === 'refused' && <span className="lc-continuation__carried">{` ${carried.line}`}</span>}
              {carried?.kind === 'parts' && (
                /*
                  A WAY TO DROP A SECTION (0.527, product ideas round four).
                  The earlier messages, the finished steps and the last reply
                  each carry their own × : the person may know one is stale
                  and would mislead. The task, their own words and the steps
                  that never reported back are not offered: those are what the
                  run is for, and the warning that stops a redo.
                */
                <span className="lc-continuation__carried">
                  {' It carries '}
                  {carried.carried.map((entry, index) => (
                    <Fragment key={entry.name}>
                      {joinerBefore(index, carried.carried.length)}
                      {entry.droppable ? (
                        <button
                          type="button"
                          className="lc-continuation__part"
                          aria-label={`Leave out ${entry.words}`}
                          title={`Leave out ${entry.words}: it is not sent with this message`}
                          onClick={() => setLeaveOut((current) => (current.includes(entry.name) ? current : [...current, entry.name]))}
                        >
                          {entry.words}
                          <Icon name="close" size={9} />
                        </button>
                      ) : (
                        entry.words
                      )}
                    </Fragment>
                  ))}
                  .
                  {carried.leftToFit.length > 0 && ` Left out to fit: ${carried.leftToFit.map((words, index) => `${joinerBefore(index, carried.leftToFit.length)}${words}`).join('')}.`}
                  {carried.leftByYou.length > 0 && (
                    <>
                      {` You are leaving out ${carried.leftByYou.map((entry, index) => `${joinerBefore(index, carried.leftByYou.length)}${entry.words}`).join('')}. `}
                      <button type="button" className="lc-continuation__putback" onClick={() => setLeaveOut([])}>
                        Put back
                      </button>
                    </>
                  )}
                </span>
              )}
            </span>
          </div>
        )}
        {/*
          * The command menu, above the box, reading what is being typed.
          *
          * Every entry maps to a control already on the row below it, so this
          * is a keyboard path to what exists rather than a second way to do
          * something new -- and `availableCommands` drops any command that
          * would be refused, so nothing here can be chosen and then fail.
          */}
        {atChoices.length > 0 && (
          <div className="lc-slash" role="listbox" aria-label="Files">
            {atChoices.map((choice, index) => {
              const active = index === Math.min(atAt, atChoices.length - 1)
              const heading = index === 0 || atChoices[index - 1]?.kind !== choice.kind
              if (choice.kind === 'teammate') {
                return (
                  <Fragment key={`mate:${choice.mate.teammateId}`}>
                    {heading && <div className="lc-slash__group lc-mono" role="presentation">Teammates</div>}
                    <button
                      type="button"
                      role="option"
                      aria-selected={active}
                      className={`lc-slash__item${active ? ' is-active' : ''}`}
                      title={`Tag ${choice.mate.name}: they are sent this message too`}
                      onMouseEnter={() => setAtAt(index)}
                      onClick={() => pickChoice(choice)}
                    >
                      <span className="lc-slash__name lc-mono">@{choice.mate.name}</span>
                      <span className="lc-slash__detail">{choice.mate.role}: sent this message too</span>
                    </button>
                  </Fragment>
                )
              }
              const path = choice.path
              const cut = path.lastIndexOf('/')
              return (
                <Fragment key={`file:${path}`}>
                {heading && <div className="lc-slash__group lc-mono" role="presentation">{filesHeading}</div>}
                <button
                  key={path}
                  type="button"
                  role="option"
                  aria-selected={active}
                  className={`lc-slash__item${active ? ' is-active' : ''}`}
                  title={path}
                  onMouseEnter={() => setAtAt(index)}
                  onClick={() => pickChoice(choice)}
                >
                  <span className="lc-slash__name lc-mono">{path.slice(cut + 1)}</span>
                  <span className="lc-slash__detail">{cut < 0 ? 'the folder itself' : path.slice(0, cut)}</span>
                </button>
                </Fragment>
              )
            })}
          </div>
        )}
        {atChoices.length === 0 && noMatchInPartial && (
          <div className="lc-slash" role="status">
            <div className="lc-slash__group lc-mono">{filesHeading}</div>
            <p className="lc-slash__detail lc-slash__empty">No match among these. This folder has more files than @ lists; + attaches any of them.</p>
          </div>
        )}
        {slashChoices.length > 0 && (
          <div className="lc-slash" role="listbox" aria-label="Commands" ref={slashList}>
            {slashChoices.map((command, index) => (
              <Fragment key={`${command.group ?? 'locust'}:${command.name}`}>
              {command.group !== undefined && slashChoices[index - 1]?.group !== command.group && (
                <div className="lc-slash__group lc-mono" role="presentation">{command.group}</div>
              )}
              <button
                key={command.name}
                type="button"
                role="option"
                aria-selected={index === Math.min(slashAt, slashChoices.length - 1)}
                /*
                 * A weighted command gets its own section rather than only its
                 * own colour: hairline above, tinted ground, shield glyph.
                 * Amber text alone announced "this one is different" only once
                 * the row had already been read.
                 */
                className={`lc-slash__item${command.weighted === true ? ' is-weighted' : ''}${index === Math.min(slashAt, slashChoices.length - 1) ? ' is-active' : ''}`}
                onMouseEnter={() => setSlashAt(index)}
                onClick={() => runSlash(command)}
                // The whole of it, where a long hint or description is cut short (0.432).
                title={`/${command.name}${command.hint === undefined ? '' : ` ${command.hint}`} — ${command.detail}`}
              >
                <span className="lc-slash__name lc-mono">
                  {command.weighted === true && <Icon name="shield" size={11} />}/{command.name}
                  {command.hint !== undefined && <span className="lc-slash__hint"> {command.hint}</span>}
                </span>
                <span className={`lc-slash__detail${command.weighted === true ? ' lc-tone-amber' : ''}`}>
                  {command.detail}
                </span>
              </button>
              </Fragment>
            ))}
          </div>
        )}
            {/*
          * Attached files, above the message rather than on the button row.
          *
          * They were a `1 file` chip beside the mode and folder controls,
          * and that row does not wrap on purpose -- so a new fixed-width
          * control on it had nowhere to go. Measured: 14px over, with the
          * effort chip running under the swarm mark. Colin, 2026-09-08:
          * "maybe just copy or use your own UI for inspiration, they just
          * put the file square in the chat."
          *
          * So: a tile per file, inside the box, where what is being sent
          * belongs -- and each one removes itself, because attaching four
          * files and wanting three was previously all-or-nothing.
          */}
        {/* The folders this conversation reads (0.804), inside the box with what else goes with the message. */}
        <ReferenceChips draftKey={draftKey ?? 'new'} {...(referenceMissionId === undefined ? {} : { missionId: referenceMissionId })} openRequest={referenceOpen} sent={referencesSent} runtime={route.runtime} mode={mode} {...(referenceCwd === undefined ? {} : { cwd: referenceCwd })} {...(referencesOnCloud === undefined ? {} : { cloud: referencesOnCloud })} onChange={setReferenceIds} onBusy={setReferenceBusy} />
        {/* Who is tagged (0.438): where the message goes besides the one on screen, removable. */}
        {tagged.length > 0 && (
          <div className="lc-tagged" aria-label="Tagged">
            {tagged.map((id) => {
              const name = team?.find((mate) => mate.teammateId === id)?.name ?? 'Someone'
              return (
                <button
                  key={id}
                  type="button"
                  className="lc-tagged__chip lc-mono"
                  aria-label={`Untag ${name}`}
                  title={`Untag ${name}`}
                  onClick={() => setTagged(tagged.filter((held) => held !== id))}
                >
                  @{name} ×
                </button>
              )
            })}
            <span className="lc-tagged__note">
              {currentTeammateId === undefined ? 'this goes to them' : 'they are sent this too, each in a conversation of their own'}
            </span>
          </div>
        )}
        {(pagePoints ?? []).length > 0 && (
          <PagePointChips
            points={pagePoints ?? []}
            onNote={(id, note) => onPagePointNote?.(id, note)}
            onRemove={(id) => onPagePointRemove?.(id)}
          />
        )}
        {(attached.length > 0 || (diffNotes ?? []).length > 0) && (
          <div className="lc-attached" aria-label={`${attachmentLabel(attached.length)} attached`}>
            {/* Notes on the diff, as one tile: they go with the next message. */}
            {(diffNotes ?? []).length > 0 && onClearDiffNotes !== undefined && (
              <button
                type="button"
                className="lc-attached__tile lc-notestile"
                title={`${diffNotesTile((diffNotes ?? []).length)} — they go with your next message. Click to remove them all.`}
                aria-label={`Remove ${diffNotesTile((diffNotes ?? []).length)}`}
                onClick={onClearDiffNotes}
              >
                <Icon name="message" size={13} />
                <span className="lc-attached__name">{diffNotesTile((diffNotes ?? []).length)}</span>
                <Icon name="close" size={11} />
              </button>
            )}
            {attached.map((path) => {
              const wasCopied = copiedIn.has(path)
              return (
                <button
                  key={path}
                  type="button"
                  className="lc-attached__tile"
                  title={
                    wasCopied
                      ? `${path} — copied into ${ATTACHMENT_DIR} so your teammate can read it. Click to remove.`
                      : `${path} — click to remove`
                  }
                  aria-label={wasCopied ? `Remove ${path}, copied into the workspace` : `Remove ${path}`}
                  onClick={() => setAttached((current) => current.filter((entry) => entry !== path))}
                >
                  {/* The picture where there is one, the file icon where there
                      is not -- never both, and never a broken image in place of
                      either. */}
                  {isImagePath(path) ? <AttachedImage path={path} /> : <Icon name="file" size={13} />}
                  <span className="lc-attached__name">{path.split('/').pop() ?? path}</span>
                  {/* The fact, as a segment of the tile behind a hairline. Two
                      words on screen; the whole sentence on the title, and only
                      if asked. */}
                  {wasCopied && (
                    <span className="lc-attached__copied">
                      <Icon name="copy" size={10} />
                      copied in
                    </span>
                  )}
                  <Icon name="close" size={11} />
                </button>
              )
            })}
          </div>
        )}
        <form ref={formRef} className="command-dock lc-composer__form" onSubmit={submit}>
          {commandAsk !== undefined && <CommandAsk lines={commandAsk.lines} onAllow={commandAsk.onAllow} onNotNow={commandAsk.onNotNow} />}
          {issueList !== undefined && <IssueList issues={issueList.issues} onChoose={issueList.onChoose} onClose={issueList.onClose} {...(issueList.empty === undefined ? {} : { empty: issueList.empty })} />}
          {cloud?.on === true && cloud.where === 'claude' && cloud.environment !== undefined && <CloudEnvironmentField {...cloud.environment} />}
          <VoiceBeam className={`lc-composerbeam${voice === 'listening' ? ' is-listening' : ''}`} active={voice === 'listening'} level={glowLevel} theme="auto" colorVariant="colorful" reach={1.5}>
          <div className="lc-composer__box" onMouseDown={pressBox}>
            {commandMode && <CommandModeNote />}
            <textarea
              ref={field}
              value={value}
              onChange={(changeEvent) => type(changeEvent.target.value)}
              onKeyDown={keyDown}
              onPaste={paste}
              placeholder={commandMode ? 'A command in this folder…' : placeholder}
              aria-label="Message"
              rows={1}
              maxLength={MAX_PROMPT_LENGTH}
              disabled={workingNow && queued !== undefined}
            />
            {/*
              * The "Shift+Enter for a new line" hint is GONE, on Colin's word
              * (2026-09-07): "everyone knows that and it takes up half the
              * chat box." It was added the same week because an outside tester
              * lost their opening mission to it -- a two-line prompt whose
              * first Enter submitted the first line -- so do not restore it
              * citing that tester without asking him again.
              *
              * The fact itself is not lost: it is on the send button's title,
              * where it costs no space in the box.
              */}
          <div className="lc-composer__controls">
            <div className="lc-composer__group">
              {/*
                * The `+`, back as the real thing.
                *
                * It was removed in the 0906 design review because it was a
                * permanently disabled control titled "not built yet" on the
                * most-visited surface in the app -- "a dead plus costs more
                * than a missing one". It only returns now because attaching
                * actually works: the picker is limited to the workspace and
                * the chosen paths are named in the message, which every
                * runtime can act on (docs/ATTACHMENTS-INTAKE-2026-09-08.md).
                */}
              {/*
                * The + is libraries.dev's gooey plus menu since the metal
                * composer (PlusMenu.tsx): Attach files, and Choose a folder,
                * which left the row with it.
                */}
              <PlusMenu
                disabled={running || attaching}
                folderMissing={workspacePath === undefined}
                onAttach={attachFiles}
                onChooseFolder={onChooseFolder}
              />
            </div>
            <div className="lc-composer__group">
<VoiceButton key={draftKey ?? ''} platform={platform ?? 'unknown'} disabled={workingNow && queued !== undefined} onText={(text) => {
                const input = field.current
                const inserted = insertVoiceText(valueNow.current, text, input?.selectionStart ?? valueNow.current.length, input?.selectionEnd ?? valueNow.current.length, MAX_PROMPT_LENGTH)
                type(inserted.text)
                requestAnimationFrame(() => { input?.focus(); input?.setSelectionRange(inserted.cursor, inserted.cursor) })
              }} />
              {/*
                * A voice call (0.737): talks back, where the microphone beside it only types. Leaving the conversation
                * ends it -- except a call WITH a teammate (0.746), which lasts while you stay with them: its first ask
                * opens their new conversation, and that move must not hang up the call that made it.
                */}
              <VoiceCallButton key={`call-${talkTo?.teammateId ?? draftKey ?? ''}`} platform={platform ?? 'unknown'} {...(talkTo === undefined ? {} : { talkTo })} onText={(text) => {
                const input = field.current
                const inserted = insertVoiceText(valueNow.current, text, input?.selectionStart ?? valueNow.current.length, input?.selectionEnd ?? valueNow.current.length, MAX_PROMPT_LENGTH)
                type(inserted.text)
                requestAnimationFrame(() => { input?.focus(); input?.setSelectionRange(inserted.cursor, inserted.cursor) })
              }} />
              {running && !canQueue ? (
                // A mono beam travels the stop button while the run goes --
                // Colin: "a loading hue for their stop button ... make it mono
                // instead to make it subtle". It stops with the stopping.
                // Colin, 2026-09-23: "slightly raise the intensity for the current
                // mono beam, its not too visible rn" -- full strength here, and
                // the layers lifted in shell.css (.lc-stopbeam).
                <Beam size="sm" strength={1} active={!cancelling} className="lc-stopbeam">
                  <button
                    type="button"
                    className="send-button lc-send is-stop"
                    onClick={stopClicked}
                    disabled={cancelling}
                    aria-label="Stop the running reply"
                  >
                    {/* A small rounded square, as drawn -- not a pause icon. */}
                    <span className="lc-stopsquare" />
                  </button>
                </Beam>
              ) : canQueue ? (
                // Typed text turns the control into "queue this", so the stop
                // button is still one click away with an empty box. What the
                // button does is what the placeholder just promised.
                <button
                  type="submit"
                  className="send-button lc-send is-queue"
                  aria-label="Send this when the reply finishes"
                  title="Send this when the reply finishes"
                >
                  <ArrowUpGlyph />
                </button>
              ) : (
                /*
                 * ONE send control, which grows a label when it is about to
                 * do more than send.
                 *
                 * The drawing gives the multi-teammate case a button reading
                 */
                /*
                 * METAL, AND ONLY ON THIS ONE.
                 *
                 * The design agent's answer to "use it more widely" is no, and
                 * the reason is worth keeping next to the one place it is used:
                 * three metal buttons in a view and metal stops meaning
                 * anything — it becomes the button style, which is decoration.
                 * This button earns it by being the only control in the app
                 * that is purely an invitation rather than a state.
                 *
                 * And two of the candidates are worse than redundant: `Deny` is
                 * destructive and `Approve once` is consequential. Making
                 * either delightful to hover is the wrong nudge on a card whose
                 * whole job is to slow a person down.
                 *
                 * The stop and queue variants above stay plain for the same
                 * reason: one is a state, the other is a deferral.
                 *
                 * Nothing beside it for its metal to reflect onto (0.836): the
                 * route chip it was cast on moved under the box, where the
                 * reflection drew a pale smudge across the chip's top.
                 */
                <MetalSend
                  /*
                   * As the page's composer draws it: a rim of light along the
                   * ring's top inside edge, and the metal cast onto the chip
                   * beside it (there the Auto chip; here whichever sits next
                   * to the send -- the effort chip, or the route when a model
                   * has no effort to choose).
                   */
                  innerShadow
                  {...(metal === undefined ? {} : { preset: metal })}
                  {...(metalStrength === undefined ? {} : { strength: metalStrength })}
                  {...(metalMotion === undefined ? {} : { motion: metalMotion })}
                  {...(metalBend === undefined ? {} : { bend: metalBend })}
                  type="submit"
                  className="send-button lc-send"
                  disabled={!canStart}
                  aria-label="Send"
                  /*
                    * A disabled control says why. Sol's beta finding 6: with
                    * nothing installed, Send was grey and its only word about
                    * itself was the thing it would not do.
                    */
                  title={
                    sendBlockedReason({
                      nothingInstalled: nothingConnected,
                      runtimeReady: selectedReady,
                      routeCanRun,
                      busy: busyWith !== undefined,
                      // Notes on the diff are a message on their own.
                      empty: value.trim().length === 0 && (diffNotes ?? []).length === 0 && (pagePoints ?? []).length === 0
                    }) ?? 'Send — Shift+Enter for a new line'
                  }
                >
                  <ArrowUpGlyph />
                </MetalSend>
              )}
            </div>
          </div>
          </div>
          </VoiceBeam>
          <div className="lc-composer__under">
            <div className="lc-composer__group">
              {/*
                * DIRECT, COMPARE OR BLIND (0.451). Colin, 2026-09-28, sending
                * Arena's menu: "maybe let's just do what they do in arena. We
                * can have direct and compare mode." One chip, where the ask
                * is written, instead of a One / Compare switch inside the
                * model picker and a "Hide the names" box in its foot. Picking
                * Compare or Blind opens the picker to tick the models.
                */}
              {compare !== undefined && (
                <span className="lc-control__anchor" ref={chatModeAnchor}>
                  {chatModeOpen && (
                    <div className="lc-menu" role="menu" aria-label="Direct or compare">
                      {cloudManaged && <ManagedPolicyNote />}
                      {CHAT_MODES.filter((option) => (option.id !== 'cloud' || cloud !== undefined) && (option.id !== 'computer' || cloud?.computer !== undefined) && (option.id !== 'background' || background !== undefined)).map((option) => (
                        <button
                          key={option.id}
                          type="button"
                          role="menuitemradio"
                          aria-checked={chatMode === option.id}
                          className="lc-menu__item"
                          disabled={cloudManaged && (option.id === 'cloud' || option.id === 'computer')}
                          {...(option.id === 'cloud' && cloud?.refusal !== undefined ? { 'aria-disabled': true, title: cloud.refusal } : {})}
                          onClick={() => {
                            setChatModeOpen(false)
                            if (option.id === 'background') {
                              compare.onMode(false)
                              compare.onBlind?.(false)
                              cloud?.onMode(false)
                              cloud?.computer?.onMode(false)
                              background?.onMode(true)
                              return
                            }
                            background?.onMode(false)
                            if (option.id === 'computer') {
                              compare.onMode(false)
                              compare.onBlind?.(false)
                              cloud?.onMode(false)
                              cloud?.computer?.onMode(true)
                              return
                            }
                            cloud?.computer?.onMode(false)
                            if (option.id === 'cloud') {
                              // Said, not silently refused: why Cloud cannot be picked here.
                              if (cloud?.refusal !== undefined) {
                                setNote(cloud.refusal)
                                return
                              }
                              compare.onMode(false)
                              compare.onBlind?.(false)
                              cloud?.onMode(true)
                              return
                            }
                            cloud?.onMode(false)
                            compare.onMode(option.id !== 'direct')
                            compare.onBlind?.(option.id === 'blind')
                            // Two models already chosen, as Arena opens Side by Side (0.460);
                            // only when there are not two to choose does the picker open.
                            if (option.id !== 'direct' && compare.picks.length < 2 && compare.prefills !== true) {
                              onOpenRoutePicker()
                              setPickerOpen(true)
                            }
                          }}
                        >
                          <Icon name={option.icon} size={15} />
                          <span className="lc-menu__text">
                            <span className="lc-menu__name">{option.name}</span>
                            <span className="lc-menu__desc">{option.id === 'computer' && cloud?.computer !== undefined ? agentComputerDesc(cloud.computer.agentName) : option.id === 'cloud' && cloud?.refusal !== undefined ? cloud.refusal : option.id === 'cloud' && cloud?.where === 'claude' ? CLAUDE_CLOUD_DESC : option.id === 'cloud' && cloud?.where === 'computer' ? COMPUTER_CLOUD_DESC : option.desc}</span>
                          </span>
                          {chatMode === option.id && <Icon name="check" size={13} />}
                        </button>
                      ))}
                    </div>
                  )}
                  <button
                    type="button"
                    className={`lc-control lc-control--boxed lc-control--chatmode${chatMode === 'direct' ? '' : ' is-on'}`}
                    aria-haspopup="menu"
                    aria-expanded={chatModeOpen}
                    aria-label={`Chat mode: ${CHAT_MODES.find((option) => option.id === chatMode)?.name ?? 'Direct'}`}
                    title={chatMode === 'computer' && cloud?.computer !== undefined ? agentComputerDesc(cloud.computer.agentName) : chatMode === 'cloud' && cloud?.where === 'computer' ? COMPUTER_CLOUD_DESC : chatMode === 'cloud' && cloud?.where === 'claude' ? CLAUDE_CLOUD_DESC : CHAT_MODES.find((option) => option.id === chatMode)?.desc}
                    disabled={running || asking !== undefined}
                    onClick={() => setChatModeOpen(!chatModeOpen)}
                  >
                    <Icon name={CHAT_MODES.find((option) => option.id === chatMode)?.icon ?? 'message'} size={14} />
                    {chatMode !== 'direct' && <span>{CHAT_MODES.find((option) => option.id === chatMode)?.name}</span>}
                    <ChevronGlyph />
                  </button>
                </span>
              )}
              <span className="lc-control__anchor" ref={modeAnchor}>
                {modeOpen && compareModeOpen && (
                  <div className="lc-menu" role="menu" aria-label="What the comparison does">
                    {([
                      [false, 'Ask', 'Each model answers. Nothing in your folder changes.', undefined],
                      [true, 'Auto', compare?.inPlace === true ? AUTO_COMPARE_IN_PLACE : AUTO_COMPARE_CONSEQUENCE, compare?.changesRefusal]
                    ] as const).map(([edits, name, consequence, refusal]) => (
                      <button
                        key={name}
                        type="button"
                        role="menuitemradio"
                        aria-checked={compareEdits === edits}
                        className="lc-menu__item"
                        disabled={refusal !== undefined}
                        title={refusal}
                        onClick={() => {
                          // Choosing Auto here IS switching it on, as in the mode menu (0.451).
                          if (edits && autoMode !== true) onEnableAutoMode?.()
                          compare?.onChanges?.(edits)
                          setModeOpen(false)
                        }}
                      >
                        <span className="lc-menu__text">
                          <span className="lc-menu__name">{name}</span>
                          <span className="lc-menu__desc">{refusal ?? consequence}</span>
                        </span>
                        {compareEdits === edits && <Icon name="check" size={13} />}
                      </button>
                    ))}
                  </div>
                )}
                {modeOpen && !comparing && (
                  <div className="lc-menu" role="menu" aria-label="Permission mode">
                    {policy?.policy.highestMode !== undefined && <ManagedPolicyNote />}
                    {MODES.filter((option) => policyModeAllowed(policy, option.mode)).map((option) => {
                      const unavailable = modeUnavailableReason(option.mode, route.runtime, platform)
                      return (
                        <button
                          key={option.mode}
                          type="button"
                          role="menuitemradio"
                          aria-checked={mode === option.mode}
                          className="lc-menu__item"
                          disabled={unavailable !== undefined}
                          title={unavailable}
                          onClick={() => {
                            // Choosing Auto is what turns it on. Settings shows
                            // the same state and takes it back; this is that
                            // decision made where it is needed, rather than a
                            // trip to another screen before the mode works.
                            if (option.mode === 'auto' && autoMode !== true) onEnableAutoMode?.()
                            onModeChange(option.mode)
                            setModeOpen(false)
                          }}
                        >
                          <span className="lc-menu__text">
                            <span className="lc-menu__name">{option.name}</span>
                            <span
                              className={`lc-menu__desc${unavailable === undefined && option.mode === 'auto' ? ' lc-tone-amber' : ''}`}
                            >
                              {unavailable ?? option.consequence}
                            </span>
                          </span>
                          {mode === option.mode && <Icon name="check" size={13} />}
                        </button>
                      )
                    })}
                    {/*
                      * Say when a mode has taken the person's connectors away.
                      *
                      * `--restricted` is what keeps a person's own Claude Code
                      * settings out of a mission, and Auto is the one mode that
                      * drops it -- it cannot be combined with
                      * `bypassPermissions`. So in every other mode the MCP
                      * servers are never loaded, and the run then also denies
                      * `mcp__*` outright.
                      *
                      * Nothing said so. Colin asked a teammate in Accept edits
                      * to check Robinhood twice on 2026-09-09; it answered,
                      * correctly and unhelpfully, that it had no connection --
                      * and the only way to learn why was to read the launcher.
                      * A refusal a person cannot act on is the app's fault, not
                      * the model's.
                      *
                      * Claude Code only, because it is the only runtime whose
                      * command builder touches MCP at all. Said whatever the
                      * settings hold: an account connector from claude.ai never
                      * appears in `~/.claude.json`, so counting configured
                      * servers would have hidden this in exactly the case that
                      * raised it.
                      */}
                    {connectorsNote(route.runtime, mode, hasConnectors) !== undefined && (
                      <p className="lc-menu__foot">{connectorsNote(route.runtime, mode, hasConnectors)}</p>
                    )}
                  </div>
                )}
                {/* The permission mode is the most consequential control on
                  * the bar, and read as a label (design review, 2026-09-05).
                  * Boxed like the route and effort chips, with a chevron. */}
                <button
                  type="button"
                  className="lc-control lc-control--boxed"
                  aria-haspopup="menu"
                  aria-expanded={modeOpen}
                  aria-label="Permission mode"
                  title={
                    comparing
                      ? compareEdits
                        ? `Auto: ${compare?.inPlace === true ? AUTO_COMPARE_IN_PLACE : AUTO_COMPARE_CONSEQUENCE}`
                        : 'Each model answers. Nothing in your folder changes.'
                      : connectorsNote(route.runtime, effectiveMode, hasConnectors) ?? 'Permission mode'
                  }
                  disabled={running || (comparing && !compareModeOpen)}
                  onClick={() => setModeOpen(!modeOpen)}
                >
                  {/* Auto only when it is on: otherwise the columns run in Edit, and the chip says so. */}
                  {comparing ? (compareEdits ? (autoMode === true ? 'Auto' : 'Edit') : 'Ask') : MODES.find((option) => option.mode === effectiveMode)?.chip ?? 'Ask'}
                  <ChevronGlyph />
                </button>
              </span>
              {/*
                * NO SWARM BUTTON ON THE ROW, since the metal composer.
                *
                * Colin put it back once (2026-09-07: "its a good indicator ...
                * it just looks cool, its our logo"), and let it go on
                * 2026-09-23: "its going to cause a cache reread for most
                * models, and is very very niche. we can leave it as a
                * command maybe?" So it is `/swarm` and Settings, and while
                * it is on, the effort chip it holds wears the mark.
                */}
              {/*
                * Which folder this message runs in. Stated on the bar that
                * says what the message will do, because it is the same kind
                * of fact as the mode and the model -- and because an app
                * launched from the Start menu had no folder at all and no
                * surface said so (Colin, 2026-09-05).
                *
                * The metal composer (2026-09-23) kept it on the row only when
                * there was none -- the chosen folder's name in the title bar,
                * choosing another on the + menu. BACK ON THE ROW (0.306),
                * Colin: "our workspace folder asset is gone ... we just can
                * use what we used to have until we find a better idea".
                */}
              <span className="lc-control__anchor lc-control__anchor--folder" ref={folderAnchor}>
                {folderOpen && (
                  <div className="lc-menu lc-menu--folders" role="menu" aria-label="Work in folder">
                    <button type="button" role="menuitem" className="lc-menu__item" onClick={() => { setFolderOpen(false); setReferenceOpen((value) => value + 1) }}><span className="lc-menu__text"><span className="lc-menu__name">Add a reference...</span><span className="lc-menu__desc">Read another folder or GitHub repository</span></span></button>
                    {folders.slice(0, 8).map((folder) => (
                      <button
                        key={folder.id}
                        type="button"
                        role="menuitemradio"
                        aria-checked={folder.id === currentFolderId}
                        className="lc-menu__item"
                        title={folder.path}
                        onClick={() => {
                          setFolderOpen(false)
                          if (folder.id !== currentFolderId) onSwitchFolder?.(folder.id)
                        }}
                      >
                        <span className="lc-menu__text">
                          <span className="lc-menu__name">{folder.name}</span>
                          <span className="lc-menu__desc lc-mono">{folder.path}</span>
                        </span>
                        {folder.id === currentFolderId && <Icon name="check" size={13} />}
                      </button>
                    ))}
                    <button
                      type="button"
                      role="menuitem"
                      className="lc-menu__item"
                      onClick={() => {
                        setFolderOpen(false)
                        onChooseFolder()
                      }}
                    >
                      <span className="lc-menu__text">
                        <span className="lc-menu__name">Choose a folder...</span>
                      </span>
                    </button>
                  </div>
                )}
              <button
                type="button"
                className={`lc-control lc-control--folder${workspacePath === undefined ? ' is-missing' : ''}`}
                aria-haspopup="menu"
                aria-expanded={folderOpen}
                title={
                  workspacePath === undefined
                    ? 'No folder chosen. Every teammate works inside one project folder.'
                    : workspaceCaution !== undefined
                      // Said in the hover (0.790), in the chip's own grey since 0.803: a folder kept for its old conversations is not
                      // a standing alarm (Colin: "having the .claude folder in yellow is triggering me a bit"). The question before
                      // its first conversation stays.
                      ? `New conversations work in ${workspacePath}. This is ${workspaceCaution}: a teammate here reads it as its project, sign-in files included. A project folder of its own is the safer place.`
                      : workspaceMade
                        ? `Teammates work in ${workspacePath}. Locust made this folder; pick any other to work there instead.`
                        : `New conversations work in ${workspacePath}`
                }
                onClick={() => setFolderOpen(!folderOpen)}
              >
                <Icon name="folder" size={13} />
                <span className="lc-control__folder">{workspaceName ?? 'No folder'}</span>
              </button>
              </span>
            </div>
            <div className="lc-composer__group">
              {context !== undefined && compact === undefined ? (
                <span className="lc-composer__context">
                  <ContextRing reading={context} {...(conversationCost === undefined ? {} : { conversationCost })} />
                </span>
              ) : context !== undefined && compact !== undefined ? (
                <span className="lc-control__anchor lc-composer__context" ref={contextAnchor}>
                  {contextOpen && (
                    <ContextRingMenu
                      reading={context}
                      {...(conversationCost === undefined ? {} : { conversationCost })}
                      offer={compact}
                      runtimeName={compactName}
                      {...(contextBreakdown === undefined ? {} : { breakdown: contextBreakdown })}
                      onCompact={() => {
                        setContextOpen(false)
                        // As typing it would send it, with the box left as it is; in line behind a running turn.
                        if (compact.kind === 'after-turn') {
                          void Promise.resolve(onQueue('/compact', [], false))
                          return
                        }
                        void onStart('/compact').then((started) => {
                          if (typeof started === 'string') setNote(started)
                        })
                      }}
                    />
                  )}
                  <button
                    type="button"
                    className="lc-control lc-contextring__button"
                    aria-haspopup="menu"
                    aria-expanded={contextOpen}
                    onClick={() => setContextOpen(!contextOpen)}
                  >
                    <ContextRing reading={context} {...(conversationCost === undefined ? {} : { conversationCost })} />
                  </button>
                </span>
              ) : null}
              {slotsShown && compare !== undefined ? (
                /*
                 * A DROPDOWN PER MODEL (0.460), as Arena draws Side by Side:
                 * each chip names one column's model and opens the picker for
                 * that column alone. It was one "A vs B" chip opening a
                 * checklist -- Colin, 2026-09-29: "select the models with two
                 * dropdowns ... will help with clarity". A third column is one
                 * press on the +; a column is removed from its own picker.
                 */
                <span className="lc-control__anchor lc-compare-slots" ref={slotsAnchor}>
                  {slotPicker !== undefined && (
                    <RoutePicker
                      runtimes={runtimes}
                      limitedRuntimes={limitedRuntimes}
                      models={models}
                      resolvedModels={resolvedModels}
                      recentRoutes={recentRoutes}
                      active={slotPicker === 'add' ? route : compare.picks[slotPicker] ?? route}
                      onSelect={(choice) => {
                        const pick = { ...choice, label: routeFamilyName(choice, resolvedModels, models) }
                        if (slotPicker === 'add') compare.onToggle(pick)
                        else compare.onReplace?.(slotPicker, pick)
                        setSlotPicker(undefined)
                      }}
                      onClose={() => setSlotPicker(undefined)}
                      slot={{
                        refusal: compare.refusal,
                        taken: compare.picks.filter((_, index) => index !== slotPicker),
                        ...(slotPicker !== 'add' && compare.picks.length > MIN_COMPARE_SLOTS
                          ? { onRemove: () => compare.onToggle(compare.picks[slotPicker as number]!) }
                          : {})
                      }}
                    />
                  )}
                  {compare.picks.map((pick, index) => {
                    /*
                     * EACH COLUMN'S OWN EFFORT (0.490). Colin, 2026-09-30:
                     * "still no effort control for compare". The one effort
                     * chip cannot serve two or three models -- their levels
                     * differ -- so each model's chip carries its own, joined
                     * to it, the same slider the single chip opens. Shown the
                     * way that chip shows it: the chosen level, else the one
                     * the model's id already names, else its default; absent
                     * where the model lists no levels.
                     */
                    const letter = String.fromCharCode(65 + index)
                    const family = modelFamily(models, pick.runtime, pick.model)
                    const levels = family?.supportedEfforts ?? []
                    const idLevel = Object.entries(family?.variants ?? {}).find(([, id]) => id === pick.model)?.[0]
                    const level = pick.effort ?? idLevel ?? defaultEffort(levels, family?.defaultEffort)
                    const { bases, hasFast } = effortScale(levels)
                    const { base, fast } = splitEffort(level ?? bases[0] ?? '')
                    const effortable = level !== undefined && levels.length > 0 && compare.onEffort !== undefined
                    return (
                      <span key={`${pick.runtime}:${pick.model}`} className={`lc-slotgroup${effortable ? ' has-effort' : ''}`}>
                        <button
                          type="button"
                          className="lc-control lc-control--boxed lc-control--slot"
                          aria-haspopup="listbox"
                          aria-expanded={slotPicker === index}
                          aria-label={`Model ${letter}: ${pick.label}`}
                          title={`${runtimeDisplayName(pick.runtime)} / ${pick.label}`}
                          onClick={() => {
                            setSlotEffort(undefined)
                            setSlotPicker(slotPicker === index ? undefined : index)
                          }}
                        >
                          <RuntimeMark runtime={pick.runtime} size={13} />
                          <span className="lc-control__model">{pick.label}</span>
                          <ChevronGlyph />
                        </button>
                        {effortable && (
                          <button
                            type="button"
                            className="lc-control lc-control--boxed lc-control--sloteffort"
                            aria-haspopup="menu"
                            aria-expanded={slotEffort === index}
                            aria-label={`Effort for model ${letter}: ${effortName(base)}${fast ? ', fast' : ''}`}
                            title={`How hard ${pick.label} thinks`}
                            onClick={() => {
                              setSlotPicker(undefined)
                              setSlotEffort(slotEffort === index ? undefined : index)
                            }}
                          >
                            {/*
                              * A METER, not the word (0.553). Colin, 10/02, of
                              * three models each beside "Medium": "we need to
                              * find a cleaner way to portray this". The word took
                              * the names' room ("Claude...", "C"); four bars say
                              * the level at a glance, and its name is the label.
                              */}
                            <EffortMeter filled={meterBars(bases, base)} fast={fast} />
                          </button>
                        )}
                        {effortable && slotEffort === index && (
                          <div className="lc-menu lc-menu--right lc-effortpanel" role="group" aria-label={`Reasoning effort for model ${letter}`}>
                            <EffortSlider
                              bases={bases}
                              index={Math.max(0, bases.indexOf(base))}
                              fast={fast}
                              hasFast={hasFast}
                              footer={effortFooter(pick.runtime)}
                              onPick={(next) => {
                                const chosen = joinEffort(next, fast, levels)
                                if (chosen !== undefined) compare.onEffort?.(index, chosen)
                              }}
                              onFast={(next) => {
                                const chosen = joinEffort(base, next, levels)
                                if (chosen !== undefined) compare.onEffort?.(index, chosen)
                              }}
                            />
                          </div>
                        )}
                      </span>
                    )
                  })}
                  {compare.picks.length < MAX_COMPARE_SLOTS && (
                    <button
                      type="button"
                      className="lc-control lc-control--boxed lc-control--addslot"
                      // The one after those picked (0.678): with one picked it said "a third".
                      aria-label={`Add a ${compare.picks.length === 1 ? 'second' : 'third'} model`}
                      title={`Add a ${compare.picks.length === 1 ? 'second' : 'third'} model`}
                      onClick={() => setSlotPicker(slotPicker === 'add' ? undefined : 'add')}
                    >
                      <Icon name="plus" size={13} />
                    </button>
                  )}
                </span>
              ) : (
              <span className="lc-control__anchor" ref={pickerAnchor}>
                {pickerOpen && (
                  <RoutePicker
                    runtimes={runtimes}
                    limitedRuntimes={limitedRuntimes}
                    models={models}
                    resolvedModels={resolvedModels}
                    recentRoutes={recentRoutes}
                    active={running ? activeChoice : route}
                    onSelect={(choice) => {
                      setPickerOpen(false)
                      if (handoff !== 'available') {
                        onRouteChange(choice)
                        return
                      }
                      // Picking the route the run is already on would stop it
                      // and buy nothing, so it is not an action here.
                      if (choice.runtime === activeChoice.runtime && choice.account === activeChoice.account) return
                      void Promise.resolve(onHandOff(choice)).then((why) => {
                        if (typeof why === 'string') setNote(why)
                      })
                    }}
                    onClose={() => setPickerOpen(false)}
                    openAt={pickerFocus}
                    {...(comparing || effortIsGenuinelyFixed || supportedEfforts.length === 0 ? {} : {
                      effort: (
                        <div data-effort-focus="true">
                          {swarm ? (
                            <p className="lc-picker__effortnote">Swarm holds this at {effortName(effortBase)}.</p>
                          ) : (
                            <EffortSlider
                              bases={effortBases}
                              index={effortIndex}
                              fast={effortIsFast}
                              hasFast={effortHasFast}
                              footer={effortFooter(route.runtime)}
                              onPick={(base) => {
                                const level = joinEffort(base, effortIsFast, supportedEfforts)
                                if (level !== undefined) onEffortChange(level)
                              }}
                              onFast={(next) => {
                                const level = joinEffort(effortBase, next, supportedEfforts)
                                if (level !== undefined) onEffortChange(level)
                              }}
                            />
                          )}
                        </div>
                      )
                    })}
                    // Compare (0.441): only where one can start -- nothing running, no comparison on screen.
                    {...(compare === undefined || asking !== undefined || running ? {} : { compare })}
                    {...(handoff === 'available'
                      ? {
                          notice:
                            'This is running. Choosing another AI agent stops it here and hands the work over with a summary of what was done — it cannot be undone.'
                        }
                      : {})}
                  />
                )}
                {/*
                  A running mission CAN change route now: the host stops it,
                  reconciles it, and briefs a continuation. Only an in-flight
                  handoff locks this control, because a second switch would
                  race the first.
                */}
                <button
                  ref={routeChip}
                  type="button"
                  className={`lc-control lc-control--boxed${usagePressing ? ' is-pressing' : ''}`}
                  /*
                   * Hover said "OpenCode / account-default" while the chip
                   * itself read "No runtime" -- the label was fixed in pass 2
                   * and the tooltip was not (outside tester, 0.164.0). What
                   * the mouse says and what the eye reads are now one thing.
                   */
                  title={[
                    comparing ? versus : nothingConnected ? 'No AI agent on this machine can start yet.' : exactRoute,
                    handoffTitle(handoff),
                    usageSentence
                  ]
                    .filter((part) => part !== undefined && part.length > 0)
                    .join('\n')}
                  onClick={() => {
                    onOpenRoutePicker()
                    setPickerOpen(!pickerOpen)
                  }}
                  disabled={handoff === 'starting' || handoff === 'switching'}
                  aria-haspopup="listbox"
                  aria-expanded={pickerOpen}
                  aria-label={comparing ? versus : nothingConnected ? 'No AI agent' : routeChipReading(modelName, effortPhrase)}
                >
                  {/*
                    * THE RUNTIME'S MARK, where the dot was (0.383).
                    *
                    * Colin, 2026-09-26, on Orca: logos "to signify which model
                    * is being used by what". The lime dot said only "ready",
                    * the normal state, beside words that already named the
                    * runtime; the mark names it at a glance and greys when
                    * the runtime cannot take work, which is the one thing the
                    * dot was there to say. A model of the person's own shows
                    * no maker's mark -- it is named as theirs (routeChrome) --
                    * and nothing connected names no runtime at all, so both
                    * keep the dot.
                    */}
                  {comparing ? (
                    <span className="lc-control__model lc-control__versus">{versus}</span>
                  ) : nothingConnected || ownModel ? (
                    <span className={`lc-dot ${selectedReady ? 'lc-tone-lime' : 'lc-tone-muted'}`} />
                  ) : (
                    <RuntimeMark runtime={shownRuntime} size={13} muted={!selectedReady} />
                  )}
                  {/*
                    * With NOTHING connected, this chip names no route.
                    *
                    * Grok's beta drive, 2026-09-14, finding 6: on a machine
                    * with every coding CLI off PATH, the footer said "0
                    * runtimes connected", the banner said to install one, the
                    * send button was disabled, the placeholder said "Install
                    * a coding agent..." -- and this chip said `OpenCode /
                    * Account Default`. First contact, and the one control on
                    * the screen that cannot work was the one claiming a route
                    * was already chosen.
                    *
                    * The muted dot was carrying that distinction alone, which
                    * is far too much weight for a 6px circle beside two
                    * confident words. The picker still opens, because that is
                    * where a person goes to see what could be installed.
                    */}
                  {comparing ? null : nothingConnected ? (
                    <span className="lc-control__model">No AI agent</span>
                  ) : shownModel === 'account-default' && !ownModel ? (
                    // The account's own default is the runtime alone, "Claude", as every other surface says it (0.723) -- with
                    // the account when one was picked (0.797), and otherwise with the model it turned out to be once a run
                    // has said (0.797, above). That reading is the machine's own sign-in's, so never beside another account.
                    (() => {
                      const account = running ? routeAccount(activeRoute ?? {}) : route.account
                      if (account !== undefined) return <>{runtimeLabel}{` · ${accountName(account)}`}</>
                      return defaultModelName === undefined ? runtimeLabel : (
                        <>
                          <span className="lc-control__runtime">{runtimeLabel}</span>
                          <span className="lc-separator">/</span>
                          <span className="lc-control__mono lc-control__model">{defaultModelName}</span>
                        </>
                      )
                    })()
                  ) : (
                    <>
                      <span className="lc-control__mono lc-control__model">{modelName}{(running ? routeAccount(activeRoute ?? {}) : route.account) !== undefined ? ` \u00b7 ${accountName(running ? routeAccount(activeRoute ?? {}) : route.account)}` : ''}</span>
                      {freeModel && <span className="lc-control__free">Free</span>}
                    </>
                  )}
                  {!comparing && effortPhrase !== undefined && <span className="lc-control__effort">· {effortPhrase}</span>}
                  {swarm && !comparing && <img className="lc-control__swarmmark" src={mark} alt="" aria-hidden="true" />}
                  {/*
                    * One control: the model and its effort, as `Haiku 5.5 · Medium`,
                    * with the agent's mark in front. The list and the effort
                    * slider open together.
                    */}
                  <ChevronGlyph />
                </button>
              </span>
              )}
              
            </div>
          </div>
        </form>
      </div>
    </div>
  )
}

/** Where a level sits on four bars: the lowest one bar, the highest four. */
export function meterBars(bases: readonly string[], base: string): number {
  const at = bases.indexOf(base)
  if (at < 0 || bases.length === 0) return 2
  if (bases.length === 1) return 4
  return Math.max(1, Math.min(4, Math.round(1 + (at * 3) / (bases.length - 1))))
}

function EffortMeter({ filled, fast }: { readonly filled: number; readonly fast: boolean }) {
  return (
    <span className={`lc-effortmeter${fast ? ' is-fast' : ''}`} aria-hidden="true">
      {[1, 2, 3, 4].map((bar) => (
        <i key={bar} className={bar <= filled ? 'is-on' : undefined} style={{ height: `${String(4 + bar * 2)}px` }} />
      ))}
    </span>
  )
}
