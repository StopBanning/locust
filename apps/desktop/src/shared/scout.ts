import { pullRequestNews, type FolderPullRequest } from './pull-request.js'

/*
 * SCOUT, A TEAMMATE THAT KEEPS WATCH (0.756). Colin, 2026-10-12, on the four ideas taken from the Grok Bot video:
 * "maybe allow the user to change the model of proactive teammate if they wish but in the meantime let's just use the
 * best free model like muse or space bunny". The design he was shown: https://claude.ai/artifact/2BMzn1KUaZNnNvJzBHgEfG.
 *
 * Locust looks first, with no model and nothing sent: the person's pull requests and their checks, the reviews asked
 * of them, the default branch's checks, a teammate waiting on them, a turn that failed. Only what is NEW goes to
 * Scout's model, in one read-only turn with no tools, and it adds its take and drafts the next step for the teammate
 * who could take it. Every card leads with Locust's own sentence, so a weak model can add to a card but never put a
 * claim in it that is not true; when the model does not answer, the cards come anyway, in Locust's words. Nothing on
 * a card happens until the person presses it.
 */

export const SCOUT_STATE = 'scout:state'
export const SCOUT_TURN_ON = 'scout:turn-on'
export const SCOUT_SET = 'scout:set'
export const SCOUT_DISMISS = 'scout:dismiss'
export const SCOUT_LOOK = 'scout:look'
/** Main to the window: what Scout holds changed. */
export const SCOUT_CHANGED = 'scout:changed'

export const SCOUT_EVERY_MINUTES = [10, 30, 60] as const
export type ScoutEvery = (typeof SCOUT_EVERY_MINUTES)[number]

/** What Scout watches; each can be switched off. */
export interface ScoutWatches {
  /** The person's pull requests and their checks, reviews asked of them, the default branch's checks. */
  readonly github: boolean
  /** A teammate waiting on them; a turn that failed. */
  readonly teammates: boolean
  /**
   * The folder's check (0.815): the command from Settings > Project folder, run when the folder has changed since the
   * last look and no turn is working in it. Off until chosen: a check can take minutes, and some reach past the folder.
   */
  readonly check: boolean
}

/**
 * Hours by this computer's clock when Scout neither looks on its own nor notifies (0.815): from `from` up to `to`,
 * over midnight when `to` comes first. The first look after them tells everything new since the last.
 */
export interface ScoutQuietHours {
  readonly from: number
  readonly to: number
}

export interface ScoutSettings {
  readonly on: boolean
  /** Scout's own teammate: its name, face and model. */
  readonly teammateId?: string
  readonly everyMinutes: ScoutEvery
  readonly watches: ScoutWatches
  /** A Windows notification for a card that needs the person now, when Locust is out of sight. */
  readonly notify: boolean
  /** What the person asked Scout to keep watch for (0.815), in their own words: read into every look. */
  readonly goal?: string
  readonly quiet?: ScoutQuietHours
  /** A look due while a turn the person started is running waits for it to end, at most SCOUT_WAIT_MOST_MS (0.815). */
  readonly waitWhileWorking: boolean
  /** The hour of the morning brief, by this computer's clock (0.816): the first look from then on brings it, once a day. */
  readonly briefAt?: number
}

export const DEFAULT_SCOUT_SETTINGS: ScoutSettings = { on: false, everyMinutes: 10, watches: { github: true, teammates: true, check: false }, notify: true, waitWhileWorking: true }

/** The longest goal kept: a few sentences, read into every look. */
export const MAX_SCOUT_GOAL = 600
/** A due look waits at most this long for the person's own turn to end, then looks anyway. */
export const SCOUT_WAIT_MOST_MS = 30 * 60_000
/** The quiet hours offered in Settings. */
export const SCOUT_QUIET_CHOICES: readonly ScoutQuietHours[] = [{ from: 21, to: 7 }, { from: 22, to: 7 }, { from: 22, to: 8 }, { from: 23, to: 8 }, { from: 0, to: 8 }]
/** The hours a morning brief may come at (0.816). */
export const SCOUT_BRIEF_HOURS = [6, 7, 8, 9, 10] as const

/** Whether `at` falls in the quiet hours, by this computer's clock. */
export function inQuietHours(quiet: ScoutQuietHours | undefined, at: Date): boolean {
  if (quiet === undefined || quiet.from === quiet.to) return false
  const hour = at.getHours()
  return quiet.from < quiet.to ? hour >= quiet.from && hour < quiet.to : hour >= quiet.from || hour < quiet.to
}

/** An hour as a person reads it: "10 PM", "midnight", "7 AM". */
export function hourWords(hour: number): string {
  if (hour === 0) return 'midnight'
  if (hour === 12) return 'noon'
  return hour < 12 ? `${String(hour)} AM` : `${String(hour - 12)} PM`
}

/** What its looks used today, by this computer's clock (0.815): the looks, and how many of them asked its model. */
export interface ScoutToday {
  readonly day: string
  readonly looks: number
  readonly asked: number
}

/** A day as `today` keys it: by this computer's clock, not UTC. */
export function scoutDay(at: Date): string {
  return `${String(at.getFullYear())}-${String(at.getMonth() + 1).padStart(2, '0')}-${String(at.getDate()).padStart(2, '0')}`
}

/** How soon the person needs it: now raises the card and, out of sight, notifies; the rest wait for Home. */
export type ScoutUrgency = 'now' | 'soon' | 'later'
export type ScoutTone = 'red' | 'amber' | 'green' | 'blue' | 'quiet'

export type ScoutOpen =
  | { readonly kind: 'url'; readonly url: string; readonly label: string }
  | { readonly kind: 'conversation'; readonly missionId: string; readonly label: string }

/** Something Locust found, in its own words. */
export interface ScoutFact {
  /** What it is about, in the state it was found in: the same news is never told twice. */
  readonly key: string
  readonly urgency: ScoutUrgency
  /** Locust's sentence: the card's first line, whatever the model says. */
  readonly said: string
  /** More for the model to read: a title, a failed turn's last line. Never shown as the card's claim. */
  readonly detail?: string
  /** What the card shows the person in mono under its sentence -- a waiting card's command -- and never gives the model. */
  readonly quote?: string
  /** Short lines under its sentence, in Locust's words: a morning brief's (0.816). */
  readonly lines?: readonly string[]
  readonly chips?: readonly { readonly text: string; readonly tone: ScoutTone }[]
  readonly open?: ScoutOpen
  /** The teammate it is about, when it is about one. */
  readonly teammateId?: string
}

export interface ScoutDraft {
  readonly teammateId: string
  readonly name: string
  readonly message: string
}

export interface ScoutCard extends ScoutFact {
  readonly cardId: string
  /** When it was found. */
  readonly at: string
  /** Scout's model is still reading it. */
  readonly reading?: true
  /** What Scout's model adds: why it matters, or what it would do. Shown as Scout's, under Locust's sentence. */
  readonly take?: string
  /** The next step, written for a teammate: one press sends it. */
  readonly draft?: ScoutDraft
}

export interface ScoutState {
  readonly settings: ScoutSettings
  /** Newest first. */
  readonly cards: readonly ScoutCard[]
  /** Its looks' turns: never listed as conversations. */
  readonly lookIds: readonly string[]
  readonly lastLookAt?: string
  /** Why its model did not answer the last time, when it did not: the cards came in Locust's words. */
  readonly modelSaid?: string
  /** What its looks used today; absent before the first look of the day. */
  readonly today?: ScoutToday
}

/**
 * A change to Scout's settings from the window: `quiet: null` turns quiet hours off, `briefAt: null` the morning brief,
 * and `goal: ''` clears the goal.
 */
export type ScoutChange = Partial<Pick<ScoutSettings, 'on' | 'everyMinutes' | 'watches' | 'notify' | 'goal' | 'waitWhileWorking'>> & {
  readonly quiet?: ScoutQuietHours | null
  readonly briefAt?: number | null
}

export interface ScoutApi {
  state(): Promise<ScoutState>
  /** Makes Scout, on this route, and starts it; on again with its teammate when it has one. */
  turnOn(route: { readonly runtime: string; readonly model: string } | undefined): Promise<ScoutState | { readonly refused: string }>
  set(change: ScoutChange): Promise<ScoutState>
  dismiss(cardId: string): Promise<ScoutState>
  look(): Promise<ScoutState>
  onChanged(listener: (state: ScoutState) => void): () => void
}

/** The most cards kept; the oldest go first. */
export const MAX_SCOUT_CARDS = 30
/** Fact keys remembered as told, so a fact is never told twice; the oldest go first. */
export const MAX_SCOUT_SEEN = 500
/** A teammate waiting this long on the person is worth a card. */
export const SCOUT_WAITING_MS = 10 * 60_000
const MAX_SAID = 300
const MAX_TAKE = 400
const MAX_DRAFT = 2_000

const clip = (text: string, max: number): string => {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length <= max ? flat : `${flat.slice(0, max - 1).trimEnd()}…`
}

/*
 * THE PULL REQUESTS' NEWS. A pull request seen for the first time is news only for what needs work now -- a failing
 * check, changes asked for, a conflict -- and its key is its state, so a lost record never tells it twice. After
 * that, whatever changed that a person would want to hear (pullRequestNews, the same words the folder's own watch
 * uses), keyed by the look that found it: the same state can come back, and coming back is news.
 */
/*
 * READY AND WAITING (0.758): checks passing, nothing asked of it, mergeable, and untouched for this long. Scout spoke
 * only of pull requests with work to do, so the person's hearth #2 -- passing, mergeable, no reviewer -- sat from
 * 2026-08-28 to the day Scout was read against Colin's own repositories without a word from it.
 */
export const READY_WAITING_DAYS = 3

export function pullRequestFacts(before: ReadonlyMap<string, FolderPullRequest>, now: readonly FolderPullRequest[], look: string, at: Date = new Date()): readonly ScoutFact[] {
  const facts: ScoutFact[] = []
  for (const pr of now) {
    // By its address, not its number: another folder's repository has a #12 too.
    const was = before.get(pr.url)
    const title = clip(pr.title, 120)
    const open: ScoutOpen = { kind: 'url', url: pr.url, label: 'Open the pull request' }
    const told = facts.length
    if (was === undefined) {
      const needs: string[] = []
      if (pr.checks === 'failing') needs.push(`${String(pr.notPassed)} of ${String(pr.checkCount)} checks did not pass`)
      if (pr.review === 'changes-requested') needs.push('changes were asked for')
      if (pr.conflicts === true) needs.push('it conflicts with its base branch')
      if (needs.length > 0) {
        facts.push({
          key: `pr:${pr.url}:${pr.checks}:${pr.review ?? ''}:${pr.conflicts === true ? 'c' : ''}`,
          urgency: pr.checks === 'failing' ? 'now' : 'soon',
          said: `Your pull request #${String(pr.number)}, ${title}: ${needs.join(', and ')}.`,
          chips: [{ text: `#${String(pr.number)}`, tone: 'quiet' }, ...(pr.checks === 'failing' ? [{ text: `${String(pr.notPassed)} of ${String(pr.checkCount)} checks not passed`, tone: 'red' as const }] : [])],
          open
        })
      }
    } else {
      const news = pullRequestNews(was, pr)
      if (news !== undefined) {
        facts.push({
          key: `pr:${pr.url}:${look}`,
          urgency: news.needsWork ? (pr.checks === 'failing' ? 'now' : 'soon') : 'later',
          said: `Your pull request #${String(pr.number)}, ${title}: ${news.said.charAt(0).toLowerCase()}${news.said.slice(1)}.`,
          chips: [{ text: `#${String(pr.number)}`, tone: 'quiet' }, { text: news.said, tone: news.needsWork ? 'red' : 'green' }],
          open
        })
      }
    }
    // Ready and waiting, whether Scout has seen it before or not -- one it has known since it was a day old turns
    // three too -- and said once (dismissed, it stays said), never beside other news of the same pull request.
    if (facts.length > told) continue
    const days = pr.updatedAt === undefined ? Number.NaN : Math.floor((at.getTime() - Date.parse(pr.updatedAt)) / 86_400_000)
    if (pr.state === 'open' && pr.checks === 'passing' && pr.review !== 'changes-requested' && pr.conflicts !== true && Number.isFinite(days) && days >= READY_WAITING_DAYS) {
      facts.push({
        key: `pr:${pr.url}:ready`,
        urgency: 'later',
        said: `Your pull request #${String(pr.number)}, ${title}, passes its checks and has waited ${String(days)} days. It can be merged, or closed.`,
        chips: [{ text: `#${String(pr.number)}`, tone: 'quiet' }, { text: 'ready', tone: 'green' }],
        open
      })
    }
  }
  return facts
}

/** A review asked of the person, from `gh pr list --search review-requested:@me --json number,title,url,author`. */
export interface ReviewAsked {
  readonly number: number
  readonly title: string
  readonly url: string
  readonly author?: string
}

export function reviewsAskedOf(json: string): readonly ReviewAsked[] {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  return parsed.flatMap((one): ReviewAsked[] => {
    if (typeof one !== 'object' || one === null) return []
    const pr = one as Record<string, unknown>
    if (typeof pr.number !== 'number' || !Number.isInteger(pr.number) || pr.number <= 0) return []
    if (typeof pr.url !== 'string' || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+$/.test(pr.url)) return []
    const author = typeof pr.author === 'object' && pr.author !== null && typeof (pr.author as Record<string, unknown>).login === 'string'
      ? clip(String((pr.author as Record<string, unknown>).login), 60) : undefined
    return [{ number: pr.number, title: typeof pr.title === 'string' ? clip(pr.title, 120) : '', url: pr.url, ...(author === undefined ? {} : { author }) }]
  })
}

export function reviewFacts(asked: readonly ReviewAsked[]): readonly ScoutFact[] {
  return asked.map((pr) => ({
    // By its address: another repository's #214 is another review.
    key: `review:${pr.url}`,
    urgency: 'soon',
    said: `${pr.author ?? 'Someone'} asked you to review pull request #${String(pr.number)}, ${pr.title}.`,
    chips: [{ text: `#${String(pr.number)}`, tone: 'quiet' }, { text: 'review asked', tone: 'amber' }],
    open: { kind: 'url', url: pr.url, label: 'Open the pull request' }
  }))
}

/** An open issue on the folder's repository, from `gh issue list --state open --json number,title,url,createdAt,author`. */
export interface IssueOpened {
  readonly number: number
  readonly title: string
  readonly url: string
  readonly createdAt: string
  readonly author?: string
}

export function issuesOf(json: string): readonly IssueOpened[] {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  return parsed.flatMap((one): IssueOpened[] => {
    if (typeof one !== 'object' || one === null) return []
    const issue = one as Record<string, unknown>
    if (typeof issue.number !== 'number' || !Number.isInteger(issue.number) || issue.number <= 0) return []
    if (typeof issue.url !== 'string' || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/issues\/\d+$/.test(issue.url)) return []
    if (typeof issue.createdAt !== 'string' || !Number.isFinite(Date.parse(issue.createdAt))) return []
    const author = typeof issue.author === 'object' && issue.author !== null && typeof (issue.author as Record<string, unknown>).login === 'string'
      ? clip(String((issue.author as Record<string, unknown>).login), 60) : undefined
    return [{ number: issue.number, title: typeof issue.title === 'string' ? clip(issue.title, 120) : '', url: issue.url, createdAt: issue.createdAt, ...(author === undefined ? {} : { author }) }]
  })
}

/*
 * NEW ISSUES (0.758): what people file on the folder's repository -- for a project with users, the first word of a
 * bug. Only those opened in the last week, three a look at most, newest first, each said once.
 */
export const NEW_ISSUE_DAYS = 7
const ISSUES_A_LOOK = 3

export function issueFacts(issues: readonly IssueOpened[], at: Date = new Date()): readonly ScoutFact[] {
  return [...issues]
    .filter((issue) => at.getTime() - Date.parse(issue.createdAt) <= NEW_ISSUE_DAYS * 86_400_000)
    .sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt))
    .slice(0, ISSUES_A_LOOK)
    .map((issue) => ({
      key: `issue:${issue.url}`,
      urgency: 'soon' as const,
      said: `${issue.author ?? 'Someone'} opened issue #${String(issue.number)}: ${issue.title}.`,
      chips: [{ text: `#${String(issue.number)}`, tone: 'quiet' as const }, { text: 'new issue', tone: 'amber' as const }],
      open: { kind: 'url' as const, url: issue.url, label: 'Open the issue' }
    }))
}

/** One run of the default branch's checks, from `gh run list --json databaseId,conclusion,status,workflowName,url`. */
export interface BranchRun {
  readonly id: number
  readonly workflow: string
  readonly url: string
  readonly failed: boolean
}

export function branchRunsOf(json: string): readonly BranchRun[] {
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  const latest = new Map<string, BranchRun>()
  for (const one of parsed) {
    if (typeof one !== 'object' || one === null) continue
    const run = one as Record<string, unknown>
    // Only a run that has finished says how the branch stands; gh lists the newest first.
    if (run.status !== 'completed' || typeof run.databaseId !== 'number' || typeof run.url !== 'string' || !/^https:\/\/github\.com\//.test(run.url)) continue
    const workflow = typeof run.workflowName === 'string' ? clip(run.workflowName, 80) : 'checks'
    if (latest.has(workflow)) continue
    latest.set(workflow, { id: run.databaseId, workflow, url: run.url, failed: run.conclusion === 'failure' || run.conclusion === 'timed_out' || run.conclusion === 'startup_failure' })
  }
  return [...latest.values()]
}

export function branchFacts(branch: string, runs: readonly BranchRun[]): readonly ScoutFact[] {
  return runs.filter((run) => run.failed).map((run) => ({
    key: `branch:${String(run.id)}`,
    urgency: 'soon',
    said: `${run.workflow} failed on ${clip(branch, 60)}.`,
    chips: [{ text: clip(branch, 60), tone: 'quiet' }, { text: `${run.workflow} · failed`, tone: 'red' }],
    open: { kind: 'url', url: run.url, label: 'Open the run' }
  }))
}

/** A teammate's approval card, waiting on the person. */
export interface WaitingCard {
  readonly approvalId: string
  readonly missionId: string
  readonly teammateId?: string
  readonly name?: string
  readonly summary: string
  readonly requestedAt: string
}

/*
 * Who waits, and how long -- never what for. A card's summary can be the exact command it would run, and a command can
 * carry a secret: the person sees it on the card, as on the approval card itself, but it stays out of a notification
 * (attention.ts) and never goes to Scout's model.
 */
export function waitingFacts(cards: readonly WaitingCard[], now: Date, waitMs = SCOUT_WAITING_MS): readonly ScoutFact[] {
  return cards.flatMap((card): ScoutFact[] => {
    const since = Date.parse(card.requestedAt)
    if (!Number.isFinite(since) || now.getTime() - since < waitMs) return []
    const minutes = Math.round((now.getTime() - since) / 60_000)
    return [{
      key: `waiting:${card.approvalId}`,
      urgency: 'now',
      said: `${card.name ?? 'A teammate'} has waited ${String(minutes)} minutes for your OK.`,
      quote: clip(card.summary, 200),
      chips: [{ text: 'waiting on you', tone: 'amber' }],
      open: { kind: 'conversation', missionId: card.missionId, label: 'Open it' },
      ...(card.teammateId === undefined ? {} : { teammateId: card.teammateId })
    }]
  })
}

/** A turn that failed, as the record keeps it. */
export interface FailedTurn {
  readonly missionId: string
  readonly teammateId?: string
  readonly name?: string
  readonly asked: string
  /** What the run said last, or the failure's own words. */
  readonly why?: string
}

/*
 * Why it failed, in the runtime's own words, leads: it is what the person needs, and without it Scout's model guessed
 * -- the first drive's Scout blamed the shell for a model that did not exist, and drafted Wren the wrong fix.
 */
export function failedFacts(turns: readonly FailedTurn[]): readonly ScoutFact[] {
  return turns.map((turn) => ({
    key: `failed:${turn.missionId}`,
    urgency: 'soon',
    said: `${turn.name ?? 'A teammate'}'s turn failed${turn.why === undefined ? '.' : `: ${clip(turn.why, 160)}`}`,
    detail: `It was asked: ${clip(turn.asked, 300)}`,
    chips: [{ text: 'failed', tone: 'red' }],
    open: { kind: 'conversation', missionId: turn.missionId, label: 'Open it' },
    ...(turn.teammateId === undefined ? {} : { teammateId: turn.teammateId })
  }))
}

/** What the look asks Scout's model: the new facts, the team, and the one form an answer takes. */
/** What the folder's check said at a look (0.815): its command, whether it passed, and the end of what it printed. */
export interface FolderCheckResult {
  readonly command: string
  readonly passed: boolean
  readonly lines: readonly string[]
}

/*
 * THE FOLDER'S CHECK (0.815): news only when it turns -- failing where it passed, or passing where it failed -- so a
 * check that stays red is told once, and one that was never run before is told only when it fails. What it printed goes
 * to Scout's model to read, never onto the card as Locust's claim.
 */
export function checkFacts(before: { readonly passed: boolean } | undefined, now: FolderCheckResult, mark: string): readonly ScoutFact[] {
  // Keyed by where the folder stood: the check runs only when that moved, so each result is its own news.
  if (now.passed) {
    if (before === undefined || before.passed) return []
    return [{ key: `check:${mark}:passes`, urgency: 'later', said: `The check ${clip(now.command, 80)} passes again.`, chips: [{ text: 'check passes', tone: 'green' }] }]
  }
  if (before !== undefined && !before.passed) return []
  const first = now.lines.find((line) => line.trim().length > 0)
  return [{
    key: `check:${mark}:fails`,
    urgency: 'now',
    said: `The check ${clip(now.command, 80)} fails${before === undefined ? '' : ' since the folder changed'}${first === undefined ? '.' : `: ${clip(first, 160)}`}`,
    ...(now.lines.length === 0 ? {} : { detail: `What it printed last:\n${now.lines.slice(-12).map((line) => clip(line, 200)).join('\n')}` }),
    chips: [{ text: 'check fails', tone: 'red' }]
  }]
}

/** What a morning brief tells, gathered by Locust with no model (0.816). */
export interface ScoutBrief {
  /** Teammates whose cards wait on the person, and for how long. */
  readonly waiting: readonly { readonly name?: string; readonly minutes: number }[]
  /** Turns that ended since the last brief, or in the last day before the first. */
  readonly finished: number
  readonly failed: number
  /** How the folder's check stood at its last run, when Scout watches it. */
  readonly check?: { readonly command: string; readonly passed: boolean }
  /** The person's open pull requests, and how many need work. */
  readonly prs?: { readonly open: number; readonly needsWork: number }
  /** Scout's other cards still on Home, and how many of them need the person now. */
  readonly cards: { readonly total: number; readonly now: number }
}

const waitedFor = (minutes: number): string => (minutes < 60 ? `${String(minutes)} min` : `${String(Math.round(minutes / 60))} h`)

/*
 * THE MORNING BRIEF (0.816). Duet's best-liked habit, from the 10/22 deep dive: one short note each morning, then quiet.
 * Every line is Locust's, counted with no model; Scout's model may add its take, as on any card. A morning with nothing
 * to tell says so in one sentence.
 */
export function briefFact(brief: ScoutBrief, day: string): ScoutFact {
  const lines: string[] = []
  if (brief.waiting.length > 0) lines.push(`Waiting on you: ${brief.waiting.slice(0, 4).map((one) => `${one.name ?? 'a teammate'}, ${waitedFor(one.minutes)}`).join('; ')}.`)
  if (brief.finished + brief.failed > 0) {
    lines.push(`Since yesterday: ${String(brief.finished)} turn${brief.finished === 1 ? '' : 's'} finished${brief.failed === 0 ? '' : `, ${String(brief.failed)} failed`}.`)
  }
  if (brief.check !== undefined) lines.push(`The check ${clip(brief.check.command, 80)} ${brief.check.passed ? 'passes' : 'fails'}.`)
  if (brief.prs !== undefined && brief.prs.open > 0) lines.push(`Your pull requests: ${String(brief.prs.open)} open${brief.prs.needsWork === 0 ? '' : `, ${String(brief.prs.needsWork)} needing work`}.`)
  if (brief.cards.total > 0) lines.push(`Scout has ${String(brief.cards.total)} more card${brief.cards.total === 1 ? '' : 's'} for you${brief.cards.now === 0 ? '' : `, ${String(brief.cards.now)} needing you now`}.`)
  const pressing = brief.waiting.length > 0 || brief.failed > 0 || brief.check?.passed === false || brief.cards.now > 0
  return {
    key: `brief:${day}`,
    urgency: pressing ? 'soon' : 'later',
    said: lines.length === 0 ? 'Good morning. Nothing waits on you, and nothing ran since yesterday.' : 'Good morning. Here is where things stand.',
    ...(lines.length === 0 ? {} : { lines, detail: lines.join('\n') }),
    chips: [{ text: 'morning brief', tone: 'blue' }]
  }
}

export function scoutPrompt(input: { readonly facts: readonly ScoutFact[]; readonly team: readonly { readonly name: string; readonly role: string }[]; readonly goal?: string }): string {
  return [
    'You are Scout, a teammate who keeps watch for the person you work with. Locust found what is below. Use only what is below: do not use any tools, open files, run commands or change anything. Read, and write.',
    // The person's own words for what to watch for (0.815): what a take weighs an item against.
    ...(input.goal === undefined ? [] : [`The person asked you to keep watch for this: ${input.goal}\nSay how each item bears on it.`]),
    'What is new:',
    input.facts.map((fact, index) => `${String(index + 1)}. ${fact.said}${fact.detail === undefined ? '' : `\n   ${fact.detail}`}`).join('\n'),
    input.team.length === 0 ? 'The team: nobody else yet.' : `The team:\n${input.team.map((mate) => `- ${mate.name}, ${mate.role}`).join('\n')}`,
    [
      'For each numbered item you have something to add to, write a block in exactly this form, and nothing else:',
      '',
      'ITEM <its number>',
      'TAKE: one or two sentences for the person: why it matters, or what you would do. No greeting.',
      'HAND TO: the teammate from the team above who should take the next step, by name, or none',
      'DRAFT: the message you would send them: what to do, and how they will know it is done',
      '',
      'Skip an item you have nothing to add to. Say nothing that is not in what is above.'
    ].join('\n')
  ].join('\n\n')
}

export interface ScoutAnswer {
  readonly take?: string
  readonly draft?: ScoutDraft
}

/**
 * Scout's model's answer, item by item. Loose about the form a small model gives it -- bold labels, a colon after the
 * number, extra blank lines -- and strict about what it may say: an item that is not in the look, a teammate who is
 * not on the team, or a draft with nobody to send it to is left out.
 */
export function scoutAnswers(text: string, factCount: number, team: readonly { readonly teammateId: string; readonly name: string }[]): ReadonlyMap<number, ScoutAnswer> {
  const answers = new Map<number, ScoutAnswer>()
  const plain = text.replace(/\r\n?/g, '\n').replace(/\*\*/g, '')
  const blocks = plain.split(/^[ \t#>-]*ITEM[ \t]+(\d+)[ \t]*:?[ \t]*$/im)
  for (let index = 1; index + 1 < blocks.length; index += 2) {
    const item = Number(blocks[index])
    if (!Number.isInteger(item) || item < 1 || item > factCount || answers.has(item)) continue
    const body = blocks[index + 1] ?? ''
    const field = (name: string): string | undefined => {
      const found = new RegExp(`^[ \\t-]*${name}[ \\t]*:[ \\t]*(.*)$`, 'im').exec(body)
      return found?.[1]?.trim()
    }
    const takeWords = field('TAKE')
    const handTo = field('HAND TO')
    // A draft runs to the end of its block.
    const draftAt = /^[ \t-]*DRAFT[ \t]*:[ \t]*/im.exec(body)
    const draftWords = draftAt === null ? undefined : body.slice(draftAt.index + draftAt[0].length).trim()
    const mate = handTo === undefined || /^none\b/i.test(handTo) ? undefined : team.find((one) => one.name.toLowerCase() === handTo.replace(/[.,;:!]+$/, '').trim().toLowerCase())
    const take = takeWords === undefined || takeWords.length === 0 ? undefined : clip(takeWords, MAX_TAKE)
    const message = draftWords === undefined || draftWords.length === 0 ? undefined : draftWords.slice(0, MAX_DRAFT).trim()
    if (take === undefined && (mate === undefined || message === undefined)) continue
    answers.set(item, {
      ...(take === undefined ? {} : { take }),
      ...(mate === undefined || message === undefined ? {} : { draft: { teammateId: mate.teammateId, name: mate.name, message } })
    })
  }
  return answers
}

/** A fact as a card, before Scout's model has read it. */
export function cardOf(fact: ScoutFact, cardId: string, at: Date, reading: boolean): ScoutCard {
  return { ...fact, said: clip(fact.said, MAX_SAID), cardId, at: at.toISOString(), ...(reading ? { reading: true as const } : {}) }
}

/** The new cards first, the oldest past MAX_SCOUT_CARDS gone. */
export function withNewCards(cards: readonly ScoutCard[], added: readonly ScoutCard[]): readonly ScoutCard[] {
  return [...added, ...cards].slice(0, MAX_SCOUT_CARDS)
}

/** Settings as a window may send them; anything else is the default, or what was there. */
export function scoutSettingsFrom(value: unknown, base: ScoutSettings = DEFAULT_SCOUT_SETTINGS): ScoutSettings {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return base
  const input = value as Record<string, unknown>
  const watches = typeof input.watches === 'object' && input.watches !== null ? input.watches as Record<string, unknown> : {}
  const goal = typeof input.goal === 'string' ? input.goal.replace(/\s+/g, ' ').trim().slice(0, MAX_SCOUT_GOAL).trim() : undefined
  const quiet = input.quiet === null ? null : quietFrom(input.quiet)
  const briefAt = input.briefAt === null ? null : (SCOUT_BRIEF_HOURS as readonly unknown[]).includes(input.briefAt) ? input.briefAt as number : undefined
  return {
    on: typeof input.on === 'boolean' ? input.on : base.on,
    ...(typeof input.teammateId === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(input.teammateId) ? { teammateId: input.teammateId } : base.teammateId === undefined ? {} : { teammateId: base.teammateId }),
    everyMinutes: (SCOUT_EVERY_MINUTES as readonly number[]).includes(input.everyMinutes as number) ? input.everyMinutes as ScoutEvery : base.everyMinutes,
    watches: {
      github: typeof watches.github === 'boolean' ? watches.github : base.watches.github,
      teammates: typeof watches.teammates === 'boolean' ? watches.teammates : base.watches.teammates,
      check: typeof watches.check === 'boolean' ? watches.check : base.watches.check
    },
    notify: typeof input.notify === 'boolean' ? input.notify : base.notify,
    waitWhileWorking: typeof input.waitWhileWorking === 'boolean' ? input.waitWhileWorking : base.waitWhileWorking,
    ...(goal === undefined ? (base.goal === undefined ? {} : { goal: base.goal }) : goal.length === 0 ? {} : { goal }),
    ...(quiet === null ? {} : quiet !== undefined ? { quiet } : base.quiet === undefined ? {} : { quiet: base.quiet }),
    ...(briefAt === null ? {} : briefAt !== undefined ? { briefAt } : base.briefAt === undefined ? {} : { briefAt: base.briefAt })
  }
}

/** Quiet hours as kept: two whole hours of the day, 0 to 23, not the same; anything else is none. */
function quietFrom(value: unknown): ScoutQuietHours | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const { from, to } = value as Record<string, unknown>
  const hour = (one: unknown): one is number => typeof one === 'number' && Number.isInteger(one) && one >= 0 && one <= 23
  return hour(from) && hour(to) && from !== to ? { from, to } : undefined
}
