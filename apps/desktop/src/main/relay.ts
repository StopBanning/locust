import type { MissionStarter, WorkroomMessage } from '@teammate/mission-store'
import type { MissionRuntimeId, MissionSandbox } from '@teammate/runtime-adapters'

import type { CodexMissionStartResponse, CodexMissionUpdate, MissionMode } from '../shared/ipc.js'
import type { MissionPeerContext, PeerRosterEntry } from './workroom-briefing.js'
import { runtimeDisplayName } from '../shared/runtimes.js'
import { stripDecisionBlocks } from '../shared/decision.js'
import { stripFileBlocks } from '../shared/handover.js'
import { stripNextBlocks } from '../shared/next-steps.js'
import { stripMemoryBlocks } from '../shared/memory.js'
import { boundedShareText, stripShareBlocks } from '../shared/peer-share.js'
import { defangProtocolBlocks } from '../shared/protocolTags.js'
import { stripTaskBlocks } from '../shared/room-task.js'

/**
 * Teammates replying to each other without a person in the loop.
 *
 * Before this, a message from Wren to Booty waited in the workroom until
 * someone next messaged Booty, and Booty's answer waited until someone next
 * messaged Wren. Delivered, attributed, and not a conversation. The relay
 * closes that loop: when a run posts a share, the host may start a run for
 * the recipient with that share as its brief, and when THAT run posts back,
 * start the original sender's next turn so the answer lands in the thread
 * that asked.
 *
 * Three limits, each load-bearing:
 *
 *   - A switch in Settings, on by default. Talking to each other is the
 *     point of having more than one teammate; the cap is what bounds it.
 *   - The exchange ends when a reply carries no message: each hop is asked
 *     to write back only if that helps finish the work, and a run that
 *     posts no share starts nothing. The hop cap is a backstop for two
 *     agents thanking each other until the account is empty, not the
 *     length of a conversation.
 *   - A sender may ask to be taken NOW -- `when="now"` on the share block --
 *     and with the person's switch on, that stops the recipient's run so the
 *     message is their next one. Off by default and off is the ordinary
 *     state: it discards work in flight. The case it exists for is the one
 *     message that is useless late -- "stop, I am editing that file".
 *   - A recipient who is mid-run cannot take a second mission, so their
 *     reply is HELD and started the moment that run ends. Without this an
 *     exchange dies at its first collision: two teammates asked to argue
 *     answer at the same time, each writes to the other while the other is
 *     still working, and both messages sit undelivered forever. MEASURED
 *     2026-09-11 -- two participants, hop zero, nothing said back.
 *   - The recipient's run uses the RECIPIENT's own runtime, model and mode,
 *     the route a person last started them on. Grok answers as Grok, Fable
 *     as Fable; a teammate who has never run yet borrows the sender's route,
 *     and the thread says so. A route that cannot start is refused, not
 *     widened -- with one exception, said in the thread: Cursor lent `ask`
 *     on a machine that cannot hold it read-only runs as Accept edits,
 *     because `ask` there is a start the mission service refuses outright.
 *
 * Nothing about the record changes. A relayed run is an ordinary mission,
 * owned by the recipient, whose prompt is the host's brief; the messages it
 * was shown are recorded by id exactly as they are for a person's mission.
 */

/**
 * Automatic runs per exchange before the host stops and waits for a person.
 *
 * A backstop, not a target, and it had stopped being one: six was firing as
 * the ordinary way an exchange ended (MEASURED 2026-09-11, five runs of a
 * one-word question: 6, 6, 3, 6, 7). A limit that fires in the normal case is
 * a timer, and a teammate cut off mid-thought is the worse failure. The fix
 * was to make exchanges END -- the brief now names what a reply costs -- and
 * then to raise this out of the way. See `DEFAULT_RELAY_HOP_CAP`.
 *
 * The default only. The person's own number is read from Settings whenever a
 * share is decided; this is what the relay uses when nobody has said.
 */
export const MAX_RELAY_HOPS = 12

export interface RelayOrigin {
  /** How many automatic runs preceded this one in the exchange. 0 for a person's mission. */
  readonly hop: number
  /**
   * The person-started mission this whole exchange grew from.
   *
   * The budget is counted against THIS, because `hop` counts the depth of one
   * chain and an exchange is not one chain. MEASURED 2026-09-11: seven
   * automatic runs against a cap of six. A reply held for a busy teammate
   * (0.69.0) carries the hop decided when the message was posted, so by the
   * time it starts the exchange may have gone deeper elsewhere -- every
   * decision inside the cap, the total outside it.
   *
   * Absent on a record written before this existed; the sharing mission is
   * then the root, which is right for the exchange it is about to start and
   * merely generous for one already running.
   */
  readonly rootMissionId?: string
  /**
   * Each participant's latest mission in this exchange, so their next hop
   * continues THEIR conversation rather than starting a stranger. The
   * person-started mission is the first entry.
   */
  readonly lastMissionOf: Readonly<Record<string, string>>
  /**
   * The teammates this exchange has already briefed as a reply (A2.8): each
   * was given the reply rules in full on its first REPLY turn -- the first
   * hop is the work and is told none of them -- and its later turns continue
   * that same conversation, so they are told once and then reminded. Absent
   * on an origin made before this existed.
   */
  readonly relayedTo?: readonly string[]
  /**
   * The workroom messages this run is started to answer (A2.12), so its
   * prompt quotes them whatever else is waiting for the recipient. Absent
   * for a run no message started.
   */
  readonly answering?: readonly string[]
}

export type RelayDecision =
  | { readonly start: true; readonly hop: number }
  | { readonly start: false; readonly reason: string }

/**
 * Whether a posted message earns an automatic run for its recipient. Pure,
 * so the cap and the default can be proven without a runtime.
 */
export function decideRelay(input: {
  readonly enabled: boolean
  /**
   * The sender said `when="later"`: tell them, do not page them.
   *
   * Checked before the budget, because a message that was never going to
   * start a run should not spend an exchange's allowance on deciding not to.
   */
  readonly defer?: boolean
  /**
   * The message says its recipient needs to do nothing (A2.3,
   * `saysNothingIsNeeded`): delivered, and read on their next run, rather
   * than starting one to read it. Checked beside `defer`, for its reason.
   */
  readonly closes?: boolean
  readonly hop: number
  readonly recipientName: string
  /** The person's own budget for one exchange; the constant is only the default. */
  readonly cap?: number
  /**
   * Automatic runs this exchange has ALREADY started, across every chain.
   *
   * The budget is spent per exchange, not per chain. Absent reads as `hop`,
   * which is what this did before it could count -- and is exactly the
   * reading that let seven runs through a cap of six.
   */
  readonly spent?: number
}): RelayDecision {
  const cap = input.cap ?? MAX_RELAY_HOPS
  if (input.defer === true) {
    return {
      start: false,
      reason: `Sent to ${input.recipientName} to read on their next run, rather than starting one.`
    }
  }
  if (input.closes === true) {
    return {
      start: false,
      reason: `Sent to ${input.recipientName} to read on their next run: the message says nothing more is needed from them.`
    }
  }
  if (!input.enabled) {
    return { start: false, reason: 'Teammate replies are switched off in Settings; the message waits for their next run.' }
  }
  const spent = input.spent ?? input.hop
  if (spent >= cap) {
    return {
      start: false,
      reason: `Stopped after ${String(cap)} automatic ${cap === 1 ? 'reply' : 'replies'}. ${input.recipientName} will see this on their next run.`
    }
  }
  return { start: true, hop: input.hop + 1 }
}

/**
 * A MESSAGE THAT SAYS ITS RECIPIENT NEED DO NOTHING STARTS NOTHING (A2.3).
 *
 * The plan's "visited path" idea, read against what actually happens here.
 * Refusing every A-to-B-to-A would end every question and answer; what burns
 * the budget is the bounce with nothing in it. MEASURED in the packaged 0.312
 * relay drive (docs/beta-fixes-2026-09-24/relay-packaged-0.312): Booty gave
 * the word Wren asked for, and Wren's reply to it was a recap ending "No
 * further action is needed from you" -- a whole run of Booty's to be told to
 * do nothing, stopped there only because that drive's budget was 2.
 *
 * So a message that says, in so many words, that nothing is needed FROM THE
 * RECIPIENT, and asks nothing, is delivered without starting their run. The
 * words must name the recipient ("from you", "on your part"): Booty's own
 * "Nothing further is needed from me" was the answer, and it went back. And a
 * message that also asks for something -- a question, "please", "can you" --
 * is a request whatever else it says. Wrong in the safe direction: a message
 * held this way is read on their next run, never lost.
 */
const NOTHING_NEEDED = [
  /\bno (?:further |more |other )?(?:action|work) (?:is |will be )?(?:needed|required|necessary) (?:from you|on your (?:part|end|side))\b/i,
  /\bnothing (?:further|more|else) is (?:needed|required) (?:from you|on your (?:part|end|side))\b/i,
  /\byou (?:don't|do not|needn't|need not) (?:need to )?(?:do anything|take any action|act on this)\b/i,
  /\bno need for you to (?:do anything|take any action|act on this)\b/i
]
const ASKS_FOR_SOMETHING = /\?|\b(?:please|could you|can you|would you|will you|need you to|i need|make sure|go ahead)\b/i

export function saysNothingIsNeeded(text: string): boolean {
  return NOTHING_NEEDED.some((pattern) => pattern.test(text)) && !ASKS_FOR_SOMETHING.test(text)
}

/**
 * The brief a relayed run is started with. It is recorded as that mission's
 * prompt, so it says plainly who asked and what is expected; the message
 * itself is quoted by the ordinary inbound section, marked as a claim like
 * every other teammate message.
 */
export function relayPrompt(input: {
  readonly sender: PeerRosterEntry
  readonly recipient: PeerRosterEntry
  readonly hop: number
  /** The exchange's budget, so the brief can say where in it this reply is. */
  readonly cap?: number
  /**
   * This reply lands in the conversation the PERSON started, which they are
   * reading. Everything the brief says about nobody watching is false there.
   *
   * MEASURED 2026-09-16 in Colin's own ledger. He asked Jimothy: "can you
   * ask wembley to research aadx, assemble a report and get back to you".
   * Wembley ran seven minutes, wrote the brief, shared it back. Jimothy,
   * told "none of that reaches a person" and "nobody is watching this run",
   * ended with a memory block and not one word to Colin. The exchange
   * finished exactly as the brief asked, and the person it was for got
   * nothing.
   */
  readonly readByPerson?: boolean
  /**
   * Which automatic reply of the exchange this run is (A2.4): what it has
   * spent, plus this one. `hop` is the depth of one chain, and a held reply
   * or a second branch sits deeper in the exchange than its chain says --
   * the brief told a reply "3 of 12" in an exchange that had spent seven.
   * Absent, the chain depth, as before.
   */
  readonly replyNumber?: number
  /**
   * This teammate already had a reply turn in this exchange, so its own
   * conversation holds the rules below (A2.8): they are reminded in a line
   * instead of being told again. The count still goes every time, and the
   * person's own teammate is still told what its reply is for.
   */
  readonly briefedBefore?: boolean
}): string {
  const who = `${input.sender.name} (${input.sender.role})`
  const opening = input.hop <= 1
    ? `${who} sent you a message; it is quoted below with anything else waiting for you.`
    : `${who} replied to you; it is quoted below.`
  const readByPerson = input.readByPerson === true && input.hop > 1
  /*
   * WARNINGS ONCE (A2.8, ECC's rule): a teammate's second reply in one
   * exchange was handed the same eight sentences as its first -- the cost,
   * what a reply is worth, the share form, the rule for `when="now"`, who is
   * and is not here -- into the conversation that already held them, each
   * hop. One line carries what they come to, and the count carries where in
   * the budget it is.
   */
  if (input.briefedBefore === true && input.hop > 1) {
    return [
      opening,
      ...(readByPerson
        ? [
            `The person who started this conversation reads it: say in a line or two what ${input.sender.name}'s reply means for what they asked -- the answer, where it is, what is still open -- written to them, and then stop.`
          ]
        : []),
      `The rules for replies from your earlier turn in this exchange still hold: ${input.sender.name} is a model, a reply costs a whole run, so write back in one <locust-share to="${input.sender.name}"> block only when it moves the work, and end with no share block when nothing more is needed.`,
      budgetSentence(input.replyNumber ?? input.hop, input.cap),
      'Stay on what was asked.'
    ].join(' ')
  }
  return [
    opening,
    // Said only to a REPLY, where the whole failure lives.
    //
    // MEASURED 2026-09-11, `relay-smoke`, five runs: Wren asks Booty for one
    // word, Booty says it -- and then the pair acknowledge each other for 6,
    // 6, 3, 6 and 7 hops. Four of five ran to the budget on a question with a
    // one-word answer. An exchange is SUPPOSED to end when a reply has
    // nothing more to say; in practice it almost never did.
    //
    // The brief said "write back only if that helps finish the work", which a
    // polite model reads as permission rather than as a cost. What it never
    // said is what a reply actually COSTS: a whole mission, on a real model,
    // billed -- and read by nobody, because the thing at the other end is
    // another model that will feel the same pull to answer. Naming the cost
    // is the honest version of the instruction, and it is a fact rather than
    // a plea.
    ...(input.hop <= 1
      ? []
      : readByPerson
        ? [
            `${input.sender.name} is a MODEL, not a person, and every reply you send THEM starts another whole mission that costs money.`,
            `But the person who started this conversation reads it, and this is what they were waiting for: say in a line or two what ${input.sender.name}'s reply means for what they asked -- the answer, where it is, what is still open -- written to them, not about what you are going to write -- and then stop.`,
            `Write back to ${input.sender.name} only when the work needs it; thanks and receipts reach nobody.`
          ]
        : [
            `${input.sender.name} is a MODEL, not a person, and every reply you send starts another whole mission that costs money.`,
            // Said as what a reply IS worth, then the one case that ends it.
            //
            // This was a list: "Do not thank them, do not confirm receipt, do
            // not summarise what you both agreed". Every item was earned by a
            // measured failure and they worked -- exchanges stopped running to
            // the cap. But a brief that is mostly "do not" produces a model
            // writing to avoid mistakes rather than to be understood, which is
            // the "dumbed down" Colin hears (2026-09-17). Grok Build's rule for
            // the same reply: "Open with what is true or what to do. Do not
            // open answers or sections with negations." Same facts, opposite
            // register; the cure -- the exchange ends, and ending is normal --
            // is kept, and now named as the good outcome it is.
            'A reply is worth that only when it moves the work: an answer with its evidence, a decision, or a question they need answered before they can continue. If their message settles things or needs nothing from you, END HERE with no share block -- that is an exchange finishing, and it is the most common good outcome. Thanks, receipts and recaps reach nobody and cost a run each.'
          ]),
    'Do what it asks if that is within your role and this workspace, using what you actually know, and say plainly what is blocked or unverified rather than implying it is done; if you cannot help, say so and why.',
    `When a reply is needed, end with one <locust-share to="${input.sender.name}"> block holding it, written for a capable colleague who has not seen your turn: lead with the answer, said as what is true rather than as a negation, then the evidence by name (paths, numbers, names), then what you need from them, if anything, in complete sentences.`,
    /*
     * THE SHAPE THEY ASKED FOR WINS (0.637), as it already does for the
     * person (workroom-briefing.ts, answerSection). A relay run in Locust
     * itself, two teammates on a free model: Wren asked Booty to reply with
     * exactly the word TANGERINE, and Booty declined -- "delivery to a
     * teammate requires a locust-share block with complete sentences, so an
     * exact single-word reply ... would violate higher instructions". The
     * sentence above describes the ordinary reply; a sender who says what
     * shape they want has already decided.
     */
    `If ${input.sender.name} said what shape the reply should take -- one word, a number, a list -- give it in that shape inside the share block; that shape wins over the sentence above.`,
    // The one case where waiting is worse than interrupting, said as a rule
    // rather than as a feature -- a model told it has an urgent channel will
    // find reasons to use it.
    'Add when="now" to a share ONLY if waiting for them to finish would make it useless or cause a conflict, such as telling them to stop touching a file you are changing; an ordinary answer never needs it.',
    // MEASURED 2026-09-05, three runs of relay-smoke: the recipient sometimes
    // declined the request as a possible prompt injection and asked for
    // context -- good judgement -- but asked it with a <locust-ask> block,
    // which reaches a PERSON, and a relayed run has none. The question sat in
    // a thread nobody was watching, no share went back, and the exchange
    // ended in silence. The right recipient of that question is the
    // teammate who asked, and the share block is how to reach them.
    readByPerson
      ? `If you need the person's decision, a <locust-ask> block reaches them here; anything for ${input.sender.name} goes in the share block.`
      : noPersonHere(input.sender.name),
    'If nothing more is needed, end with no share block -- that is how an exchange finishes.',
    budgetSentence(input.replyNumber ?? input.hop, input.cap),
    'Stay on what was asked.'
  ].join(' ')
}

/**
 * Where this reply sits in the budget, said to the teammate spending it.
 *
 * MEASURED 2026-09-11, `relay-smoke`: Wren asks Booty for one word, Booty
 * says it -- and the pair then acknowledged each other until the cap fired.
 * Six relayed runs for a question with a one-word answer. The brief already
 * says "if nothing more is needed, end with no share block", and every hop
 * got that same sentence whether it was the first or the fifth.
 *
 * The host knows how much budget is left and the teammate does not. That is
 * the same shape as everything else fixed today: the app holding a fact the
 * reader needed. It is not a cure for politeness, but a model told it is on
 * the last automatic reply has a reason to stop that "if nothing more is
 * needed" does not give it.
 *
 * The last hop says the thing that is actually true and actually load-
 * bearing: a share written HERE starts nothing. `decideRelay` refuses at the
 * cap, so a teammate that writes one has written to a person who may not
 * look for hours -- and it has no way to know that unless it is told.
 */
/**
 * What a reply that runs READ-ONLY is told, after the brief (0.592).
 *
 * Handoff review finding (a), 2026-10-03: Bro, in Ask, received Codex's
 * reply, tried a command, was refused by the mode, and wrote nothing back --
 * the exchange ended in silence with the answer sitting in the quote. The
 * brief above never said the mode: it is decided later, where the route is
 * settled (`startFor`), so the sentence is added there. It names the cost
 * (a refused command ends the run with nothing written) and what to answer
 * from instead, the way the OpenCode read-only section does for a direct run.
 */
export function readOnlyReplySentence(): string {
  return 'You are replying in a read-only mode: a shell command is refused here and ends this run with nothing written. What you were sent is quoted in this message -- answer from it and from your read tools, and write back in the share block.'
}

export function budgetSentence(hop: number, cap: number | undefined): string {
  const budget = cap ?? MAX_RELAY_HOPS
  const left = budget - hop
  if (left <= 0) {
    return (
      'This is the LAST automatic reply in this exchange: anything you send back now waits for a person '
      + 'rather than reaching them, so finish what you can say here.'
    )
  }
  if (left === 1) {
    return (
      `This is automatic reply ${String(hop)} of ${String(budget)}. One more would be the last, so do not write back `
      + 'unless it genuinely needs saying.'
    )
  }
  return `This is automatic reply ${String(hop)} of ${String(budget)}.`
}

/**
 * Said in every relayed briefing. A runtime that reaches a fork on a normal
 * mission is told to stop and ask the person; on a relayed run that
 * instruction is still in its prompt, and following it strands the exchange.
 */
function noPersonHere(sender: string): string {
  // "nobody is watching this run" was true and licensed machine register: a
  // writer told nobody is reading writes like nobody is reading. The fact
  // that matters is where the question came from and where an ask would go.
  return (
    `The person is not in this exchange: ${sender} is a teammate, and the question came from them. `
    + `If you need a decision or more context before you can help, ask ${sender} inside that share block. `
    + 'A <locust-ask> block reaches only a person, so here it would strand the exchange.'
  )
}

/** What a mission that just shared looks like to the relay. */
export interface SharingMission {
  readonly runId: string
  readonly missionId: string
  readonly runtime: MissionRuntimeId
  readonly sandbox: MissionSandbox
  readonly model: string | undefined
  readonly peer: MissionPeerContext
  readonly relay: RelayOrigin | undefined
  readonly startedBy?: MissionStarter
}

export interface RelayOptions {
  readonly enabled: () => Promise<boolean>
  /** The autonomy budget for one exchange, read when a share is decided. Absent means the default. */
  readonly hopCap?: () => Promise<number>
  /**
   * The exchange a mission a PERSON started belongs to, when it is not its
   * own (A2.4): every run a room post started shares one, so the budget
   * bounds the post rather than each member's corner of it. Absent, or
   * undefined, the mission is its own exchange.
   */
  readonly exchangeOf?: (missionId: string) => Promise<string | undefined>
  readonly peerContextFor: (teammateId: string) => Promise<MissionPeerContext | undefined>
  /** A finished run's final message, for bringing back an answer it did not write back (A2.1). */
  readonly finalReplyOf?: (missionId: string) => Promise<string | undefined>
  /**
   * How a run ended when it did not complete (`howTurnEnded`), undefined when
   * it completed. Codex hit its usage limit mid-answer and Bro's thread said
   * "Codex finished without writing back" (Colin's ledger, 10/03): it had not
   * finished, and a cut-off run's last progress line is not an answer.
   */
  readonly howEnded?: (missionId: string) => Promise<string | undefined>
  /** Posts a returned answer to the asker's messages, as the answerer's (A2.1). */
  readonly post?: (input: {
    readonly from: { readonly teammateId: string; readonly name: string; readonly missionId: string }
    readonly to: { readonly teammateId: string; readonly name: string }
    readonly text: string
  }) => Promise<{ readonly messageId: string }>
  /**
   * Whether Cursor Agent can be held read-only on this machine. Absent
   * reads as yes. On Windows it cannot (its sandbox needs macOS or Linux),
   * and a relayed start in `ask` there is refused by the mission service --
   * measured 2026-09-17: "Booty could not reply on their own: Cursor Agent
   * cannot be held read-only on this system." A reply that can only run as
   * Accept edits runs as Accept edits, and the thread says so.
   */
  readonly cursorHoldsReadOnly?: () => boolean
  readonly start: (input: {
    readonly prompt: string
    readonly runtime: MissionRuntimeId
    readonly mode: MissionMode
    readonly model: string | undefined
    /** The recipient's own saved effort; a borrowed route borrows none (M10). */
    readonly effort?: string
    readonly account?: string
    readonly peer: MissionPeerContext
    readonly followUpOf: string | undefined
    readonly relay: RelayOrigin
  }) => Promise<CodexMissionStartResponse>
  readonly assignOwner: (teammateId: string, missionId: string) => Promise<void>
  /**
   * The newest turn of this teammate's hub, or nothing.
   *
   * A reply with no predecessor in its own exchange continues THIS rather
   * than starting a root of its own -- which is what put every peer message
   * after the first in Ungrouped as a conversation nobody asked for (Colin,
   * 2026-09-21). Absent means every such reply is a new conversation, the
   * way it was. The caller answers `undefined` for a turn the ledger no
   * longer has, so a deleted hub is simply begun again.
   */
  readonly hubOf?: (teammateId: string) => Promise<string | undefined>
  /** Record a run as the newest turn of the teammate's hub; `began` when it is the hub's first. */
  readonly rememberHub?: (teammateId: string, missionId: string, began: boolean) => Promise<void>
  /**
   * Whether anything is still waiting to be shown to this teammate.
   *
   * A held reply is not the only way a message gets delivered: anything a
   * teammate has not read is folded into the brief of whatever run starts
   * next, whoever started it. So if a person messaged them while the reply
   * was held, the message has already arrived and starting a second run
   * would brief them on an empty inbox. Absent means start anyway.
   */
  readonly stillWaiting?: (teammateId: string) => Promise<boolean>
  /** Whether one teammate may stop another's run to be heard now. Read per message. */
  readonly mayInterrupt?: () => Promise<boolean>
  /**
   * Stop whatever this teammate is running, and say whether anything stopped.
   *
   * Deliberately nothing more: the reply itself is started by the ordinary
   * held-message path when the run ends, so an interruption is only ever
   * "make the wait short", never a second way to start a mission.
   */
  readonly stopWorkOf?: (teammateId: string) => Promise<boolean>
  /**
   * Show this teammate's running turn a message at its next step, WITHOUT
   * stopping it (A2.10), and say whether the runtime took it. Codex's
   * app-server can (`turn/steer`), and Claude Code can (its stream-json input
   * stays open while the turn runs); every other runtime answers false.
   */
  readonly steerWorkOf?: (teammateId: string, text: string) => Promise<boolean>
  /** Reaches the window, addressed to the SENDER's run, so notices land in the thread that shared. */
  readonly notify: (update: CodexMissionUpdate) => void
  /**
   * Write a notice into a mission's own record, so it survives being looked
   * away from.
   *
   * A relay notice was a LIVE UPDATE ONLY. "Stopped after 12 automatic
   * replies" reached the window and nowhere else, so a person watching a
   * different conversation when an exchange ended never learned it had, and
   * reopening the thread later showed nothing at all -- the exchange simply
   * appeared to stop for no reason (MEASURED 2026-09-11, `relay-smoke`:
   * no thread carried the sentence).
   *
   * Only ENDINGS are written. "Still waiting on Booty" is true for a moment
   * and false after it; a record of it would be a record of something that
   * is no longer so.
   */
  readonly note?: (input: {
    readonly missionId: string
    readonly message: string
  }) => Promise<void>
}

/** A run the relay may care about ending: who it belonged to and what exchange it was in. */
export interface EndedMission {
  readonly missionId: string
  readonly peer: MissionPeerContext | undefined
  readonly relay: RelayOrigin | undefined
}

/** A message as the relay receives it: the record, plus what its sender asked for. */
export type RelayedMessage = WorkroomMessage & { readonly urgent?: boolean; readonly defer?: boolean; readonly wantsAnswer?: boolean }

export interface Relay {
  onShared(mission: SharingMission, posted: readonly RelayedMessage[]): Promise<void>
  /** Any run ending, however it ended. A meeting counts a recipient who left without answering. */
  onRunEnded(mission: EndedMission): Promise<void>
}

/**
 * A meeting: one teammate wrote to several at once, and their next turn
 * waits until every one of them has answered or given up, then starts ONCE
 * with all the replies quoted -- the minutes -- instead of once per reply,
 * which a teammate who runs one mission at a time could not take anyway.
 */
/** An ordinary one-to-one exchange, from the asker's side. */
interface OpenExchange {
  readonly askerRunId: string
  readonly askerMissionId: string
  readonly askerId: string
  readonly recipientName: string
  /** The asker wants an answer (A2.1): one not written back is brought to them. */
  readonly wantsAnswer?: boolean
  /** The asker's mission, whose route a returned answer starts on. */
  readonly asker?: SharingMission
  /** The exchange the recipient's run belongs to, which a returned answer continues. */
  readonly origin?: RelayOrigin
}

/** A reply that could not start because its recipient was already running. */
interface DeferredReply {
  readonly recipient: MissionPeerContext
  readonly prompt: string
  /**
   * The same brief for the reply number it will actually be, which is only
   * known when it starts (A2.4). Absent for a brief with no budget line.
   */
  readonly promptFor?: (replyNumber: number) => string
  readonly from: SharingMission
  readonly origin: RelayOrigin
  /** Addressed to the thread that shared, so the wait is visible where it was caused. */
  readonly notice: (message: string, level?: 'info' | 'warning') => void
  /** Registered once the held run actually starts, so the asker still hears silence. */
  readonly exchange: OpenExchange | undefined
}

interface Meeting {
  readonly askerId: string
  readonly askerName: string
  readonly askerRunId: string
  readonly askerMissionId: string
  /** The exchange the reply-back hop belongs to. */
  readonly origin: RelayOrigin
  readonly awaiting: Map<string, string>
  readonly answered: string[]
  /** Those who finished without a word to the asker; named in the minutes. */
  readonly silent: string[]
}

/**
 * The brief for the turn after a meeting. Every reply is quoted by the
 * ordinary inbound section; this only says what the person's teammate is
 * looking at.
 */
/*
 * A2.13: THE PERSON'S OWN TEAMMATE IS TOLD THE PERSON IS THERE.
 *
 * The meeting's close said "There is no person in this exchange" to every
 * asker -- including the teammate whose conversation the person started and
 * is reading, which is where the whole meeting's result is waited for (the
 * code review's reported #7). `relayPrompt` has told that teammate the truth
 * since 2026-09-16; the meeting's close and the returned answer now do too.
 */
const TO_THE_PERSON =
  'The person who started this conversation reads it and is waiting on this: say in a line or two what came back and what it means for what they asked -- the answer, where it is, what is still open -- written to them. If you need their decision, a <locust-ask> block reaches them here.'

export function meetingPrompt(input: { readonly repliers: readonly string[]; readonly silent: readonly string[]; readonly readByPerson?: boolean }): string {
  const who = input.repliers.length === 1 ? input.repliers[0] : `${input.repliers.slice(0, -1).join(', ')} and ${input.repliers[input.repliers.length - 1]}`
  return [
    `${who} replied to your message; the replies are quoted below.`,
    ...(input.silent.length === 0 ? [] : [`${input.silent.join(', ')} finished without replying.`]),
    'Take them together. Write back to anyone only if that helps finish the work: one <locust-share to="Name"> block per teammate.',
    input.readByPerson === true
      ? TO_THE_PERSON
      : 'There is no person in this exchange to answer you; a question for any of them goes in their share block, never in a <locust-ask> block, which reaches only a person.',
    'If nothing more is needed, end with no share block -- that is how an exchange finishes.',
    'Do not start unrelated work.'
  ].join(' ')
}

/** How a returned answer is labelled in the asker's messages (A2.1). */
export const RETURNED_ANSWER = '(Returned by Locust: this is how they answered in their own conversation; they did not write back.)'

/** The asker's turn when an answer is brought back to them (A2.1). */
export function returnedAnswerPrompt(recipientName: string, readByPerson = false): string {
  return [
    `${recipientName} answered your message in their own conversation and did not write back, so Locust brought their answer to you: it is quoted below as a message from them.`,
    ...(readByPerson ? [TO_THE_PERSON] : []),
    'Take it into account and carry on. Write back only if that moves the work; if nothing more is needed, end with no share block.',
    'Do not start unrelated work.'
  ].join(' ')
}

/** A final reply, as another teammate may be shown it: every protocol block out, whatever is left defanged. */
function withoutProtocolBlocks(text: string): string {
  return defangProtocolBlocks(stripNextBlocks(stripFileBlocks(stripMemoryBlocks(stripTaskBlocks(stripDecisionBlocks(stripShareBlocks(text)))))))
}

export function createRelay(options: RelayOptions): Relay {
  /** Open meetings by the asker's mission id. */
  const meetings = new Map<string, Meeting>()
  /**
   * One-to-one exchanges the host started and is still waiting on, keyed by
   * the RECIPIENT's mission. A meeting keeps minutes and says who stayed
   * silent; an ordinary exchange kept nothing, so when a recipient answered
   * in prose instead of a reply block the asker's thread simply showed
   * nothing at all -- Colin, 2026-09-05: "wren answered it but only in his
   * own chat, we never got the reply in booty's chat".
   */
  const exchanges = new Map<string, OpenExchange>()
  /*
   * The OTHER threads waiting on one held reply (2026-10-10 sweep): the first held share wins the reply, and a
   * second share to the same busy teammate used to be dropped there, so its thread heard nothing ever after.
   * Kept by recipient while held, then by the held run's mission once it starts, and told how it ended.
   */
  const heldAlso = new Map<string, OpenExchange[]>()
  const alsoWaiting = new Map<string, OpenExchange[]>()
  /** Which meeting a relayed run is answering, by that run's mission id. */
  const answering = new Map<string, Meeting>()
  /**
   * Automatic runs started per exchange, keyed by the mission a person began.
   *
   * `hop` counts the depth of ONE chain, and an exchange is not one chain --
   * a reply held for a busy teammate starts later, on its own branch, with
   * the hop it was given when the message was posted. Seven runs got through
   * a cap of six that way (MEASURED 2026-09-11). This counts what is actually
   * being spent.
   *
   * Bounded, because nothing tells the relay an exchange is over: the oldest
   * entries are dropped past a few hundred, which cannot under-count a LIVE
   * exchange -- a live one is, by definition, the most recently touched.
   */
  const spentByExchange = new Map<string, number>()
  const EXCHANGES_REMEMBERED = 400
  /**
   * What an exchange has spent, never less than the chain already proves.
   *
   * The counter is the real total when it knows -- but it cannot know about
   * an exchange that began before this code, or one whose count was lost to a
   * restart or an eviction. `hop` survives all three, because it is written
   * into the mission record, and the depth of one chain is a FLOOR on the
   * number of runs the exchange has had. Taking the larger is right in every
   * case and cannot let the budget through, which is the failure that
   * mattered.
   */
  const spendOf = (root: string | undefined, hop: number): number =>
    Math.max(hop, root === undefined ? 0 : spentByExchange.get(root) ?? 0)
  /*
   * SPENT AT THE DECISION (A2.4).
   *
   * This was counted in `startFor`, after the start returned -- and a start
   * takes seconds. Two replies decided in that window both read the same
   * count, both passed, and both ran: the budget let one through per
   * overlap. Now the count is taken in the same synchronous step as the
   * decision, before anything awaits, and given back if no run came of it
   * (refused, held for later, the recipient gone). A held reply is decided,
   * and counted, again when it starts.
   */
  const reserve = (root: string | undefined): void => {
    if (root !== undefined) recordSpend(root)
  }
  const release = (root: string | undefined): void => {
    if (root === undefined) return
    const held = spentByExchange.get(root)
    if (held === undefined) return
    if (held <= 1) spentByExchange.delete(root)
    else spentByExchange.set(root, held - 1)
  }
  const recordSpend = (root: string): void => {
    // Read BEFORE deleting. The delete-then-set is only to move the key to
    // the end of the insertion order so the eviction below drops the least
    // recently touched exchange; reading after it would reset the count to
    // one every time and the cap would never fire at all.
    const next = (spentByExchange.get(root) ?? 0) + 1
    spentByExchange.delete(root)
    spentByExchange.set(root, next)
    while (spentByExchange.size > EXCHANGES_REMEMBERED) {
      const oldest = spentByExchange.keys().next().value
      if (oldest === undefined) break
      spentByExchange.delete(oldest)
    }
  }
  /**
   * Replies whose recipient was mid-run, by that recipient's teammate id.
   *
   * One entry per teammate, not one per message: everything waiting for them
   * is folded into one brief, so a second held reply would start a second
   * run that had already been told everything.
   */
  const deferred = new Map<string, DeferredReply>()

  const notify = (runId: string, missionId: string, message: string, level: 'info' | 'warning' = 'warning'): void => {
    options.notify({ kind: 'relay-notice', runId, missionId, message, level })
  }

  /*
   * AN ANSWER NOT WRITTEN BACK IS BROUGHT BACK (A2.1).
   *
   * Colin, 2026-09-05: "wren answered it but only in his own chat, we never
   * got the reply in booty's chat". A question went out, the answer was
   * written -- in prose, in the recipient's own conversation, with no reply
   * block -- and the teammate who asked never had it. In Colin's ledger 3
   * of 12 relayed runs ended that way. The notice said where the answer was;
   * the asker still worked without it.
   *
   * When the sender WANTED an answer (wants="answer", or a message ending in
   * a question), the host brings it: the recipient's final reply, without
   * its protocol blocks and bounded like any message, is posted to the asker
   * as theirs -- labelled as returned -- and the asker's turn starts to take
   * it, through the same start and hold as every reply. Exactly once: the
   * exchange is gone the moment its run ends. It is the exchange CLOSING, so
   * it is not refused by the hop budget (it still counts against it); what
   * the asker does next is budgeted as always. An FYI ends silently, as it
   * should.
   */
  const returnAnswer = async (exchange: OpenExchange, ended: EndedMission): Promise<boolean> => {
    if (options.finalReplyOf === undefined || options.post === undefined || exchange.asker === undefined || exchange.origin === undefined) return false
    if (ended.peer === undefined) return false
    const said = await options.finalReplyOf(ended.missionId)
    const words = said === undefined ? '' : withoutProtocolBlocks(said).trim()
    if (words.length === 0) return false
    const asker = await options.peerContextFor(exchange.askerId)
    if (asker === undefined) return false
    const returned = await options.post({
      from: { teammateId: ended.peer.self.teammateId, name: ended.peer.self.name, missionId: ended.missionId },
      to: { teammateId: asker.self.teammateId, name: asker.self.name },
      text: boundedShareText(`${RETURNED_ANSWER} ${words}`)
    })
    const origin: RelayOrigin = {
      hop: exchange.origin.hop + 1,
      rootMissionId: exchange.origin.rootMissionId ?? exchange.askerMissionId,
      lastMissionOf: { ...exchange.origin.lastMissionOf, [ended.peer.self.teammateId]: ended.missionId },
      ...(exchange.origin.relayedTo === undefined ? {} : { relayedTo: exchange.origin.relayedTo }),
      answering: [returned.messageId]
    }
    const notice = (message: string, level?: 'info' | 'warning'): void => notify(exchange.askerRunId, exchange.askerMissionId, message, level)
    // The person-started mission is the first entry in `lastMissionOf`.
    const prompt = returnedAnswerPrompt(ended.peer.self.name, Object.keys(origin.lastMissionOf)[0] === asker.self.teammateId)
    // Never refused by the budget, but spent against it (A2.4).
    reserve(origin.rootMissionId)
    const result = await startFor({ recipient: asker, prompt, from: exchange.asker, origin, notice })
    if (result.kind !== 'started') release(origin.rootMissionId)
    if (result.kind === 'busy') defer({ recipient: asker, prompt, from: exchange.asker, origin, notice, exchange: undefined }, result.pool === true)
    notifyAndKeep(
      exchange.askerRunId,
      exchange.askerMissionId,
      result.kind === 'refused'
        ? `${exchange.recipientName} answered in their own conversation without writing back. The answer was brought to ${asker.self.name}'s messages, to read on their next run.`
        : `${exchange.recipientName} answered in their own conversation without writing back, so the answer was brought back to ${asker.self.name}.`
    )
    return true
  }

  /**
   * Say it, and keep it.
   *
   * For the notices a person needs AFTER the moment: why an exchange stopped,
   * and that a teammate finished without writing back. Both are facts about
   * how the work ended, and both used to exist only for as long as the window
   * was pointed at the right thread.
   */
  const notifyAndKeep = (runId: string, missionId: string, message: string): void => {
    notify(runId, missionId, message)
    void options.note?.({ missionId, message }).catch(() => undefined)
  }

  /**
   * What came of asking for a run.
   *
   * `busy` is split out from every other refusal because it is the only one
   * that resolves itself: the recipient is working, and will not be shortly.
   * Collapsing it into "could not start" is what ended exchanges at hop zero.
   */
  type StartOutcome =
    | { readonly kind: 'started'; readonly missionId: string }
    /** `pool`: every run slot was taken; the recipient may be idle. */
    | { readonly kind: 'busy'; readonly pool?: true }
    | { readonly kind: 'refused' }

  const startFor = async (input: {
    readonly recipient: MissionPeerContext
    readonly prompt: string
    readonly from: SharingMission
    readonly origin: RelayOrigin
    readonly notice: (message: string, level?: 'info' | 'warning') => void
  }): Promise<StartOutcome> => {
    const { recipient, from } = input
    // Their turn in THIS exchange first; failing that, their hub. A reply
    // inside an exchange belongs to the conversation the exchange is, and a
    // reply from outside one belongs to them.
    const inExchange = input.origin.lastMissionOf[recipient.self.teammateId]
    let hub: string | undefined
    if (inExchange === undefined && options.hubOf !== undefined) {
      try {
        hub = await options.hubOf(recipient.self.teammateId)
      } catch {
        hub = undefined
      }
    }
    const followUpOf = inExchange ?? hub
    const ontoHub = inExchange === undefined
    // Their own route, so each teammate stays the model a person made
    // them. Only a teammate who has never run borrows the sender's.
    const own = recipient.self.route
    // A borrowed route never borrows the sender's PERMISSION beyond editing
    // the workspace: a sender on Auto lends `ask`, not the run of the whole
    // machine. Conservative on purpose -- the recipient never chose Auto --
    // and the notice below now says which mode was lent, because a person
    // whose sender could edit anything and whose reply could not write at
    // all had no way to find out why (QA, 2026-09-06).
    const lent = own ?? {
      runtime: from.runtime,
      model: from.model ?? 'account-default',
      mode: from.sandbox === 'workspace-write' ? ('accept-edits' as const) : ('ask' as const)
    }
    // Cursor on a machine that cannot hold it read-only: the mission service
    // refuses `ask` and `plan` outright, so a reply lent either would never
    // run. Accept edits is the least it can be started as, said below.
    const cursorWidened =
      lent.runtime === 'cursor'
      && (lent.mode === 'ask' || lent.mode === 'plan')
      && options.cursorHoldsReadOnly?.() === false
    const route = cursorWidened ? { ...lent, mode: 'accept-edits' as const } : lent
    if (cursorWidened) {
      input.notice(
        `${recipient.self.name} replies in Edit mode rather than read-only: Cursor Agent cannot be held read-only on this system.`
      )
    }
    if (own === undefined) {
      /*
       * Said plainly, and as information (0.366). It read "Sable has not run
       * on a route of their own yet, so this reply runs on Rook's OpenCode /
       * opencode/muse-spark-1.3-contributor-free ... Message Sable once on
       * the route they should keep" -- in amber, on the first delegation of
       * every team a new person starts from Home (Rook's first starter,
       * packaged 0.365). The exact model is in About this reply.
       */
      input.notice(
        `${recipient.self.name} has no model of their own yet, so this reply uses ${from.peer.self.name}'s (${runtimeDisplayName(route.runtime)})${
          route.mode === 'accept-edits' ? ' and may edit this folder' : ', read-only'
        }. To give ${recipient.self.name} their own, edit ${recipient.self.name} on the Team screen.`,
        'info'
      )
    }
    /*
     * Say it is coming before it exists.
     *
     * A cold runtime takes seconds to boot, and `mission-started` waits for a
     * run id -- so for those seconds the window knew nothing and the room the
     * argument was happening in read as finished. The pair is emitted around
     * the start whatever it returns, including a throw.
     */
    options.notify({
      kind: 'relay-starting',
      teammateId: recipient.self.teammateId,
      name: recipient.self.name,
      answering: from.missionId,
      hop: input.origin.hop
    })
    const settled = (): void => options.notify({ kind: 'relay-start-settled', teammateId: recipient.self.teammateId })
    let response: CodexMissionStartResponse
    try {
      response = await options.start({
        // A read-only reply is told so (0.592): the mode is settled only here.
        prompt: route.mode === 'ask' || route.mode === 'plan' ? `${input.prompt} ${readOnlyReplySentence()}` : input.prompt,
        runtime: route.runtime,
        mode: route.mode,
        model: route.model === 'account-default' ? undefined : route.model,
        // M10: the effort they were saved at, as a direct message runs them.
        ...(own?.account === undefined ? {} : { account: own.account }),
        ...(own?.effort === undefined ? {} : { effort: own.effort }),
        peer: recipient,
        followUpOf,
        relay: input.origin
      })
    } catch {
      settled()
      input.notice(`${recipient.self.name} could not reply on their own: the run could not be started.`)
      return { kind: 'refused' }
    }
    settled()
    if (!response.ok) {
      // Mid-run is not a refusal. A teammate runs one mission at a time, so
      // answering while they work is impossible and answering when they
      // finish is ordinary; the caller holds the reply until then.
      if (response.error.code === 'RUN_ALREADY_ACTIVE') return response.error.busy === 'pool' ? { kind: 'busy', pool: true } : { kind: 'busy' }
      input.notice(`${recipient.self.name} could not reply on their own: ${response.error.message} The message waits for their next run.`)
      return { kind: 'refused' }
    }
    // Counted by the caller, at its decision, and given back there when no
    // run came of it (A2.4, `reserve`).
    await options.assignOwner(recipient.self.teammateId, response.data.missionId).catch(() => undefined)
    // A run that continued the hub, or began one, is the hub's newest turn
    // now. Recorded before the window is told, so a face clicked on the
    // strength of this update opens a hub the host already knows.
    if (ontoHub && options.rememberHub !== undefined) {
      await options.rememberHub(recipient.self.teammateId, response.data.missionId, hub === undefined).catch(() => undefined)
    }
    options.notify({
      kind: 'mission-started',
      runId: response.data.runId,
      missionId: response.data.missionId,
      teammateId: recipient.self.teammateId,
      prompt: input.prompt,
      data: response.data,
      startedBy: { kind: 'relay', hop: input.origin.hop },
      ...(ontoHub ? { hubMissionId: response.data.missionId } : {})
    })
    return { kind: 'started', missionId: response.data.missionId }
  }

  /**
   * Stop a teammate's run so a waiting message is their next one.
   *
   * Said in the sender's thread either way: an interruption that was asked
   * for and refused by a switch is exactly the case where silence reads as
   * the message never arriving.
   */
  /**
   * A2.10: SAFE-POINT DELIVERY. A message asked to be taken now is first
   * offered to the recipient's running turn, at its next step -- which loses
   * nothing, so it needs no switch -- and only a runtime that cannot take it
   * that way falls back to being stopped, under the switch, as before. The
   * message is still their next turn either way (the held reply is already
   * queued): steering is a heads-up that arrives in time, not a second way to
   * answer, so the hop cap and the answer's return are untouched.
   */
  const takeNowFor = async (recipient: MissionPeerContext, from: string, text: string, notice: (message: string, level?: 'info' | 'warning') => void): Promise<void> => {
    const shown = withoutProtocolBlocks(text).trim().slice(0, 2000)
    let steered = false
    try {
      steered = shown.length > 0 && (await options.steerWorkOf?.(recipient.self.teammateId, [
        `${from} sent you this while you were working, asking for it to be taken now: "${shown}"`,
        'If it changes what you are doing, act on it before you go on. You will also get it as your next turn, to answer.'
      ].join(' '))) === true
    } catch {
      steered = false
    }
    if (steered) {
      notice(`${recipient.self.name} was shown this part-way through their run, without stopping it. It is also their next turn.`, 'info')
      return
    }
    await interruptFor(recipient, notice)
  }

  const interruptFor = async (recipient: MissionPeerContext, notice: (message: string, level?: 'info' | 'warning') => void): Promise<void> => {
    let allowed = false
    try {
      allowed = (await options.mayInterrupt?.()) === true
    } catch {
      allowed = false
    }
    if (!allowed) {
      notice(
        `${recipient.self.name} was asked to take this before finishing. Teammates interrupting each other is switched off in Settings, so it waits for their run to end.`,
        'info'
      )
      return
    }
    let stopped = false
    try {
      stopped = (await options.stopWorkOf?.(recipient.self.teammateId)) === true
    } catch {
      stopped = false
    }
    if (stopped) {
      notice(`${recipient.self.name} was stopped part-way so they can take this next. Whatever they had done is in their own conversation.`)
    }
  }

  /**
   * Hold a reply for a teammate who cannot start yet. The first one held wins.
   *
   * `pool`: every run slot was taken, and the recipient may be idle. It used
   * to say they were "part-way through another mission" either way -- untrue
   * for an idle one -- and to wait for THEIR run to end, which for an idle
   * teammate never came (harness review, 2026-09-24).
   */
  const defer = (entry: DeferredReply, pool = false): void => {
    const teammateId = entry.recipient.self.teammateId
    if (deferred.has(teammateId)) {
      if (entry.exchange !== undefined) heldAlso.set(teammateId, [...(heldAlso.get(teammateId) ?? []), entry.exchange])
      return
    }
    deferred.set(teammateId, entry)
    entry.notice(
      pool
        ? `Every run slot is in use, so ${entry.recipient.self.name}'s reply starts when one frees up.`
        : `${entry.recipient.self.name} is part-way through another mission. Their reply starts when it ends.`
    )
  }

  /**
   * Start a held reply, now that its recipient's run has ended.
   *
   * The switch and the cap are read again rather than remembered: a person
   * who turned replies off while a teammate was working meant it, and a hop
   * budget they lowered should bind the hop it was lowered before.
   */
  const deliverDeferred = async (teammateId: string): Promise<void> => {
    const entry = deferred.get(teammateId)
    if (entry === undefined) return
    deferred.delete(teammateId)
    let enabled = false
    try {
      enabled = await options.enabled()
    } catch {
      enabled = false
    }
    let cap = MAX_RELAY_HOPS
    try {
      cap = (await options.hopCap?.()) ?? MAX_RELAY_HOPS
    } catch {
      cap = MAX_RELAY_HOPS
    }
    const root = entry.origin.rootMissionId
    const spent = spendOf(root, entry.origin.hop - 1)
    const decision = decideRelay({
      enabled,
      hop: entry.origin.hop - 1,
      recipientName: entry.recipient.self.name,
      cap,
      // Against what the exchange has spent NOW, not when this was held. That
      // gap is the leak: every decision was inside the cap and the total was
      // not, because a held reply carried a hop the exchange had moved past.
      spent
    })
    if (!decision.start) {
      entry.notice(decision.reason)
      for (const other of heldAlso.get(teammateId) ?? []) notifyAndKeep(other.askerRunId, other.askerMissionId, decision.reason)
      heldAlso.delete(teammateId)
      return
    }
    reserve(root)
    if (options.stillWaiting !== undefined) {
      let waiting = true
      try {
        waiting = await options.stillWaiting(teammateId)
      } catch {
        // Unknown means send: a reply shown twice is recoverable, one never
        // shown is the bug this whole path exists to fix.
        waiting = true
      }
      if (!waiting) {
        release(root)
        heldAlso.delete(teammateId)
        return
      }
    }
    const outcome = await startFor({
      recipient: entry.recipient,
      // The number it is NOW, not the one it would have been when it was held.
      prompt: entry.promptFor?.(spent + 1) ?? entry.prompt,
      from: entry.from,
      origin: entry.origin,
      notice: entry.notice
    })
    if (outcome.kind !== 'started') release(root)
    // Free for a moment and taken again -- by a person, or by another
    // teammate's reply that landed first. Wait for the next ending.
    if (outcome.kind === 'busy') {
      deferred.set(teammateId, entry)
      return
    }
    if (outcome.kind === 'started' && entry.exchange !== undefined) exchanges.set(outcome.missionId, entry.exchange)
    if (outcome.kind === 'started') {
      const others = heldAlso.get(teammateId)
      if (others !== undefined) alsoWaiting.set(outcome.missionId, others)
    }
    // Could not start: every thread waiting on it hears so, not only the first, whose notice startFor gave (0.753, bug
    // hunt 2026-10-12, RL2 -- the second thread had been told only that the reply starts when the run ends).
    if (outcome.kind === 'refused') {
      for (const other of heldAlso.get(teammateId) ?? []) {
        notifyAndKeep(other.askerRunId, other.askerMissionId, `${entry.recipient.self.name} could not reply on their own. The message waits for their next run.`)
      }
    }
    heldAlso.delete(teammateId)
  }

  /** The asker's next turn, once, briefed with everyone's reply. */
  const closeMeeting = async (meeting: Meeting, from: SharingMission): Promise<void> => {
    meetings.delete(meeting.askerMissionId)
    const asker = await options.peerContextFor(meeting.askerId)
    if (asker === undefined) return
    if (meeting.answered.length === 0) {
      notify(meeting.askerRunId, meeting.askerMissionId, 'Nobody replied. Your message waits with them for their next run.')
      return
    }
    const silent = [...meeting.silent, ...meeting.awaiting.values()]
    const origin: RelayOrigin = {
      hop: meeting.origin.hop + 1,
      ...(meeting.origin.rootMissionId === undefined ? {} : { rootMissionId: meeting.origin.rootMissionId }),
      lastMissionOf: { ...meeting.origin.lastMissionOf, [from.peer.self.teammateId]: from.missionId },
      ...(meeting.origin.relayedTo === undefined ? {} : { relayedTo: meeting.origin.relayedTo })
    }
    // The PERSON'S cap, and the exchange's actual spend -- this checked the
    // constant, so a workspace that lowered its budget still got six here.
    let cap = MAX_RELAY_HOPS
    try {
      cap = (await options.hopCap?.()) ?? MAX_RELAY_HOPS
    } catch {
      cap = MAX_RELAY_HOPS
    }
    const spent = spendOf(origin.rootMissionId, origin.hop)
    if (spent >= cap) {
      notifyAndKeep(
        meeting.askerRunId,
        meeting.askerMissionId,
        `Stopped after ${String(cap)} automatic ${cap === 1 ? 'reply' : 'replies'}. The replies wait for your next run.`
      )
      return
    }
    reserve(origin.rootMissionId)
    const notice = (message: string, level?: 'info' | 'warning'): void => notify(meeting.askerRunId, meeting.askerMissionId, message, level)
    const readByPerson = Object.keys(origin.lastMissionOf)[0] === asker.self.teammateId
    const outcome = await startFor({
      recipient: asker,
      prompt: meetingPrompt({ repliers: meeting.answered, silent, readByPerson }),
      from,
      origin,
      notice
    })
    if (outcome.kind !== 'started') release(origin.rootMissionId)
    // The asker picked up another mission while the meeting ran. The minutes
    // keep until they are free rather than being dropped on the floor.
    if (outcome.kind === 'busy') {
      defer(
        {
          recipient: asker,
          prompt: meetingPrompt({ repliers: meeting.answered, silent, readByPerson }),
          from,
          origin,
          notice,
          exchange: undefined
        },
        outcome.pool === true
      )
    }
  }

  return {
    async onShared(mission, posted) {
      if (posted.length === 0) return
      // The workroom already holds these shares. An external Ask cannot
      // spend another turn, steer a recipient, or inherit its writing mode.
      if (mission.startedBy?.kind === 'mcp') {
        const message = 'Shares from another app are visible in Locust. No teammate was started automatically.'
        await options.note?.({ missionId: mission.missionId, message }).catch(() => undefined)
        notify(mission.runId, mission.missionId, message, 'info')
        return
      }
      let enabled = false
      try {
        enabled = await options.enabled()
      } catch {
        enabled = false
      }
      let cap = MAX_RELAY_HOPS
      try {
        cap = (await options.hopCap?.()) ?? MAX_RELAY_HOPS
      } catch {
        cap = MAX_RELAY_HOPS
      }
      const hop = mission.relay?.hop ?? 0
      // The exchange this belongs to. A mission a PERSON started is its own
      // root -- unless it is one of a room post's, which share the post's
      // (A2.4) -- and every relayed one carries the root it was started under.
      let shared: string | undefined
      if (mission.relay?.rootMissionId === undefined && options.exchangeOf !== undefined) {
        try {
          shared = await options.exchangeOf(mission.missionId)
        } catch {
          shared = undefined
        }
      }
      const root = mission.relay?.rootMissionId ?? shared ?? mission.missionId
      const notice = (message: string, level?: 'info' | 'warning'): void => notify(mission.runId, mission.missionId, message, level)

      // A reply into an open meeting is held, not relayed: the asker's turn
      // starts once everyone has spoken. The message itself is already in
      // the workroom, so nothing is lost by waiting.
      const meeting = answering.get(mission.missionId)
      const held = new Set<string>()
      if (meeting !== undefined && meeting.awaiting.has(mission.peer.self.teammateId)) {
        if (posted.some((message) => message.to.teammateId === meeting.askerId)) {
          meeting.awaiting.delete(mission.peer.self.teammateId)
          meeting.answered.push(mission.peer.self.name)
          held.add(meeting.askerId)
          if (meeting.awaiting.size > 0) {
            notify(
              meeting.askerRunId,
              meeting.askerMissionId,
              `${mission.peer.self.name} replied. Still waiting on ${[...meeting.awaiting.values()].join(', ')}.`,
              'info'
            )
          } else {
            await closeMeeting(meeting, mission)
          }
        }
      }

      // One automatic run per recipient per share, whatever was posted: a
      // run that wrote to the same teammate twice gets one reply, not two.
      // This run answering an exchange it was started for: whatever else it
      // does, the asker is no longer waiting in silence.
      const waiting = exchanges.get(mission.missionId)
      if (waiting !== undefined && posted.some((message) => message.to.teammateId === waiting.askerId)) {
        exchanges.delete(mission.missionId)
      }
      const others = alsoWaiting.get(mission.missionId)
      if (others !== undefined) {
        const still = others.filter((other) => !posted.some((message) => message.to.teammateId === other.askerId))
        if (still.length === 0) alsoWaiting.delete(mission.missionId)
        else alsoWaiting.set(mission.missionId, still)
      }

      const seen = new Set<string>()
      const started: {
        readonly teammateId: string
        readonly name: string
        readonly missionId: string
        readonly wantsAnswer: boolean
        readonly origin: RelayOrigin
      }[] = []
      for (const message of posted) {
        const recipientId = message.to.teammateId
        if (seen.has(recipientId) || held.has(recipientId)) continue
        seen.add(recipientId)

        const lastMissionOf = { ...(mission.relay?.lastMissionOf ?? {}), [mission.peer.self.teammateId]: mission.missionId }
        // The person-started mission is the first entry in `lastMissionOf`
        // (documented on RelayOrigin), so its teammate is the one whose
        // conversation the person is reading.
        const readByPerson = Object.keys(lastMissionOf)[0] === recipientId
        const spent = spendOf(root, hop)
        const decision = decideRelay({
          enabled,
          hop,
          recipientName: message.to.name,
          cap,
          spent,
          ...(message.defer === true ? { defer: true } : {}),
          // Never for the person's own teammate: that run is how the person
          // hears what came of it, whatever the message says (A2.3).
          ...(!readByPerson && message.wantsAnswer !== true && saysNothingIsNeeded(message.text) ? { closes: true } : {})
        })
        if (!decision.start) {
          // Kept: this is the sentence that explains why a conversation the
          // person comes back to simply stopped.
          void options.note?.({ missionId: mission.missionId, message: decision.reason }).catch(() => undefined)
          // Every message that goes nowhere is said, off included: replies
          // are on by default now, so off is a choice the person made and
          // may have forgotten -- and a share with no answer and no word
          // was the one thing a drive could not explain (2026-09-05).
          notice(decision.reason)
          continue
        }

        // Spent in the same step as the decision, before anything awaits
        // (A2.4); given back below unless a run starts.
        reserve(root)
        let ran = false
        try {
          const recipient = await options.peerContextFor(recipientId)
          if (recipient === undefined) {
            notice(`${message.to.name} is no longer on the roster; nothing was started.`)
            continue
          }
          // Only a REPLY turn is given the reply rules (the first hop is the
          // work, and is told none of them), so only a reply turn counts as
          // having been told.
          const earlier = mission.relay?.relayedTo ?? []
          const briefedBefore = earlier.includes(recipientId)
          const origin: RelayOrigin = {
            hop: decision.hop,
            rootMissionId: root,
            lastMissionOf,
            relayedTo: decision.hop > 1 && !briefedBefore ? [...earlier, recipientId] : earlier,
            // Every message this share sent them: one run answers them all.
            answering: posted.filter((entry) => entry.to.teammateId === recipientId).map((entry) => entry.messageId)
          }
          const promptFor = (replyNumber: number): string =>
            relayPrompt({ sender: mission.peer.self, recipient: recipient.self, hop: origin.hop, cap, readByPerson, replyNumber, briefedBefore })
          const prompt = promptFor(spent + 1)
          const result = await startFor({ recipient, prompt, from: mission, origin, notice })
          ran = result.kind === 'started'
          if (result.kind === 'busy') {
            defer(
              {
                recipient,
                prompt,
                promptFor,
                from: mission,
                origin,
                notice,
                exchange: {
                  askerRunId: mission.runId,
                  askerMissionId: mission.missionId,
                  askerId: mission.peer.self.teammateId,
                  recipientName: recipient.self.name,
                  wantsAnswer: message.wantsAnswer === true,
                  asker: mission,
                  origin
                }
              },
              result.pool === true
            )
            /*
             * Asked to be taken now, and allowed to be.
             *
             * Stopping is the WHOLE action. The reply is started by the held-
             * message path when the run ends, which is the same path every
             * other waiting message takes -- so an interruption can only ever
             * shorten a wait, never become a second way to start a mission,
             * and it cannot outrun the hop cap or the relay switch because it
             * does not go near either.
             */
            if (message.urgent === true) await takeNowFor(recipient, mission.peer.self.name, message.text, notice)
            continue
          }
          if (result.kind === 'started') {
            started.push({ teammateId: recipientId, name: recipient.self.name, missionId: result.missionId, wantsAnswer: message.wantsAnswer === true, origin })
          }
        } catch (error) {
          // Whatever failed between the decision and the start, the thread
          // says so. The service that called us swallows anything thrown
          // here, which is how a failure became silence.
          notice(`${message.to.name} could not reply on their own: ${error instanceof Error ? error.message : String(error)} The message waits for their next run.`)
        } finally {
          if (!ran) release(root)
        }
      }

      // Writing to several teammates at once opens a meeting: their answers
      // are collected, and the asker's next turn starts once, with all of
      // them. One recipient is an ordinary exchange and needs no minutes.
      if (started.length >= 2) {
        const opened: Meeting = {
          askerId: mission.peer.self.teammateId,
          askerName: mission.peer.self.name,
          askerRunId: mission.runId,
          askerMissionId: mission.missionId,
          origin: {
            hop,
            rootMissionId: root,
            lastMissionOf: { ...(mission.relay?.lastMissionOf ?? {}), [mission.peer.self.teammateId]: mission.missionId },
            ...(mission.relay?.relayedTo === undefined ? {} : { relayedTo: mission.relay.relayedTo })
          },
          awaiting: new Map(started.map((entry) => [entry.teammateId, entry.name])),
          answered: [],
          silent: []
        }
        meetings.set(mission.missionId, opened)
        for (const entry of started) answering.set(entry.missionId, opened)
        notice(`Waiting on ${started.map((entry) => entry.name).join(', ')} to reply before your next turn.`, 'info')
      } else {
        for (const entry of started) {
          exchanges.set(entry.missionId, {
            askerRunId: mission.runId,
            askerMissionId: mission.missionId,
            askerId: mission.peer.self.teammateId,
            recipientName: entry.name,
            wantsAnswer: entry.wantsAnswer,
            asker: mission,
            origin: entry.origin
          })
        }
      }
    },

    async onRunEnded(mission) {
      // An ordinary exchange that ended with nothing written back. The reply
      // is not lost -- it is in the recipient's own conversation -- and the
      // person watching the thread that asked is told where to find it,
      // rather than being left to conclude the message never arrived.
      const exchange = exchanges.get(mission.missionId)
      exchanges.delete(mission.missionId)
      const others = alsoWaiting.get(mission.missionId) ?? []
      alsoWaiting.delete(mission.missionId)
      const ended = options.howEnded === undefined ? undefined : await options.howEnded(mission.missionId).catch(() => undefined)
      for (const open of [...(exchange === undefined ? [] : [exchange]), ...others]) {
        // Only a run that completed has an answer to bring back (0.572).
        const returned = ended === undefined && open.wantsAnswer === true && (await returnAnswer(open, mission).catch(() => false))
        if (!returned) {
          notifyAndKeep(
            open.askerRunId,
            open.askerMissionId,
            ended === undefined
              ? `${open.recipientName} finished without writing back. Anything they said is in their own conversation.`
              : `${open.recipientName} stopped before writing back: ${ended}. Anything they said is in their own conversation.`
          )
        }
      }
      const meeting = answering.get(mission.missionId)
      answering.delete(mission.missionId)
      const teammateId = mission.peer?.self.teammateId
      if (meeting !== undefined && mission.peer !== undefined && teammateId !== undefined && meeting.awaiting.has(teammateId)) {
        // Ended without a word to the asker: counted, not waited for forever.
        meeting.awaiting.delete(teammateId)
        meeting.silent.push(mission.peer.self.name)
        if (meeting.awaiting.size > 0) {
          notify(
            meeting.askerRunId,
            meeting.askerMissionId,
            `${mission.peer.self.name} ${ended === undefined ? 'finished without replying' : `stopped before replying: ${ended}`}. Still waiting on ${[...meeting.awaiting.values()].join(', ')}.`
          )
        } else {
          await closeMeeting(meeting, {
            runId: '',
            missionId: mission.missionId,
            runtime: 'codex',
            sandbox: 'read-only',
            model: undefined,
            peer: mission.peer,
            relay: mission.relay
          })
        }
      }
      // Whatever this run was and however it ended, its teammate is free now.
      // This is the line that makes an argument alternate: the reply that
      // arrived mid-run starts here instead of waiting for a person.
      if (teammateId !== undefined) await deliverDeferred(teammateId)
      // And a run ending frees a slot for everyone: a reply held because
      // every slot was taken, for a teammate idle all along, starts now. It
      // waited for that teammate's own run to end, which never came (harness
      // review, 2026-09-24). One still mid-run is simply held again.
      for (const other of [...deferred.keys()]) {
        if (other !== teammateId) await deliverDeferred(other)
      }
    }
  }
}
