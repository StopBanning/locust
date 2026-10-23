import type { CommentWatch } from './scout-comments.js'
import { randomBytes } from 'node:crypto'
import { readFile, rename, rm, writeFile } from 'node:fs/promises'

import type { FolderPullRequest } from '../shared/pull-request.js'
import {
  DEFAULT_SCOUT_SETTINGS,
  MAX_SCOUT_NOTES,
  MAX_SCOUT_SEEN,
  SCOUT_WAIT_MOST_MS,
  branchFacts,
  briefFact,
  briefSpan,
  cardOf,
  commentAnswerFact,
  checkFacts,
  daysBetween,
  emptyWeek,
  failedFacts,
  inQuietHours,
  issueFacts,
  notesFrom,
  pullRequestFacts,
  reviewFacts,
  scoutAnswers,
  scoutDay,
  scoutNoteOf,
  scoutPrompt,
  scoutSettingsFrom,
  waitingFacts,
  watchOfKey,
  weekFact,
  weekFrom,
  withNewCards,
  type BranchRun,
  type FailedTurn,
  type FolderCheckResult,
  type IssueOpened,
  type ReviewAsked,
  type ScoutCard,
  type ScoutFact,
  type ScoutOutcome,
  type ScoutSettings,
  type ScoutState,
  type ScoutToday,
  type ScoutWeek,
  type WaitingCard
} from '../shared/scout.js'
import { replaceFile } from './replace-file.js'

/**
 * SCOUT, WATCHING (0.756, shared/scout.ts). One look at a time, on a timer while it is on: Locust gathers what is
 * new, makes the cards in its own words at once, and only then asks Scout's model -- one read-only turn on Scout's
 * own route -- for its take and a drafted next step, which fill the cards in when the turn ends. A look that finds
 * nothing new asks no model. A model that does not answer leaves the cards as Locust wrote them, and says so.
 */
export interface ScoutTeammate {
  readonly teammateId: string
  readonly name: string
  readonly role: string
}

export interface ScoutGithub {
  readonly prs: readonly FolderPullRequest[]
  readonly reviews: readonly ReviewAsked[]
  readonly branch?: { readonly name: string; readonly runs: readonly BranchRun[] }
  /** The folder's repository's open issues (0.758): the ones opened this week are news. */
  readonly issues?: readonly IssueOpened[]
}

export interface ScoutDeps {
  readonly comments?: (watch: CommentWatch, team: readonly ScoutTeammate[], at: string) => Promise<{ watch: CommentWatch; facts: readonly ScoutFact[] }>
  readonly lastTeammate?: () => Promise<string | undefined>
  readonly postComment?: (repository: string, number: number, text: string) => Promise<string>
  /** Terminal answer only; undefined while a turn is still running. */
  readonly commentAnswer?: (missionId: string) => Promise<{ text: string } | { failed: string } | undefined>
  readonly file: string
  readonly now?: () => Date
  /** The person's pull requests, reviews asked of them and the default branch's checks; undefined when GitHub cannot be read here. */
  readonly github: () => Promise<ScoutGithub | undefined>
  /** Teammates' approval cards still waiting on the person. */
  readonly waiting: () => Promise<readonly WaitingCard[]>
  /** A run that ended, as a failed turn when it failed; undefined otherwise. */
  readonly failedTurn: (missionId: string) => Promise<FailedTurn | undefined>
  /** The turn this one went on from, when it continued a conversation. */
  readonly continuesFrom?: (missionId: string) => Promise<string | undefined>
  /** The team Scout may hand work to, without Scout. */
  readonly team: () => Promise<readonly ScoutTeammate[]>
  /** Starts one look on Scout's route -- or on this free model in its place: its turn, or why it did not start. */
  readonly ask: (teammateId: string, prompt: string, model?: string) => Promise<{ readonly missionId: string; readonly runId: string } | { readonly refused: string }>
  /** Stops a look's turn that will not end by itself: a provider refusing, and the runtime trying again for ever. */
  readonly stop: (runId: string) => void
  /**
   * The next free model a look may try when Scout's own free model's provider will not answer: none when Scout's
   * route is not a free one, or every one has been tried.
   */
  readonly nextFreeModel: (teammateId: string, tried: readonly string[]) => Promise<string | undefined>
  /** What a look's turn said last, once it has ended; why not, when it said nothing. */
  readonly answerOf: (missionId: string) => Promise<{ readonly text: string } | { readonly failed: string }>
  /** Makes Scout's own teammate. */
  readonly makeTeammate: (route: { readonly runtime: string; readonly model: string; readonly account?: string }) => Promise<string>
  /** Whether Scout's teammate is still on the team. */
  readonly teammateExists: (teammateId: string) => Promise<boolean>
  readonly changed: (state: ScoutState) => void
  /** A card that needs the person now, while Locust is out of sight. */
  readonly notify: (card: ScoutCard) => void
  /** Every new card, whatever its urgency or the hours: the phone's alerts keep rules of their own (phone-alerts.ts, 0.827). */
  readonly raised?: (cards: readonly ScoutCard[]) => void
  /** How long a look's turn may take before its cards stop waiting on it. */
  readonly answerWaitMs?: number
  readonly schedule?: (task: () => void, ms: number) => unknown
  readonly clear?: (handle: unknown) => void
  /** Once, after a while: the first look after a launch, and a look's turn running out of time. */
  readonly later?: (task: () => void, ms: number) => unknown
  readonly cancel?: (handle: unknown) => void
  /**
   * The window's folder as it stands -- its last commit and what is changed in it -- or undefined when it is not a
   * git folder (0.815): the check runs only when this moved since the last look's.
   */
  readonly folderMark?: () => Promise<string | undefined>
  /** Runs the folder's check from Settings > Project folder; undefined when the folder has none. */
  readonly runCheck?: () => Promise<FolderCheckResult | undefined>
  /** A turn is working in the window's folder now, so its check would read work half done (0.815). */
  readonly folderBusy?: () => boolean
  /** A turn the person started is running: a look on Scout's timer waits for it (0.815). */
  readonly personBusy?: () => boolean
  /** The turns that ended since then, for the morning brief (0.816): Scout's own looks, named, are left out. */
  readonly briefFacts?: (since: string, looks: readonly string[]) => Promise<{ readonly finished: number; readonly failed: number; readonly more?: boolean }>
  /** The alerts held back from the phone since `since` (ms), for the morning brief (0.827, phone-alerts.ts). */
  readonly heldAlerts?: (since: number) => readonly { readonly title: string; readonly why: 'quiet hours' | 'daily limit' }[]
}

/** How often a look that is waiting for the person's own turn asks again whether it has ended. */
const WAIT_STEP_MS = 60_000

interface Kept {
  readonly comments?: CommentWatch
  readonly commentTurns?: Readonly<Record<string, { readonly ask: NonNullable<ScoutFact['commentAsk']>; readonly name: string }>>
  readonly settings: ScoutSettings
  readonly cards: readonly ScoutCard[]
  readonly seen: readonly string[]
  readonly lookIds: readonly string[]
  /** The folder as the last check found it, and what the check said then (0.815). */
  readonly folderMark?: string
  readonly lastCheck?: { readonly passed: boolean; readonly command?: string }
  readonly today?: ScoutToday
  /** The day the last morning brief came, by this computer's clock, and when (0.816). */
  readonly lastBriefDay?: string
  readonly lastBriefAt?: string
  /** Scout's notes, oldest first (0.816). */
  readonly notes: readonly string[]
  /** This week's tally, for the weekly tune-up (0.817); none before the first look. */
  readonly week?: ScoutWeek
  /** Each pull request as the last look saw it, by its address. */
  readonly prs: readonly FolderPullRequest[]
  readonly lastLookAt?: string
  readonly modelSaid?: string
  /** The looks since the last brief, for what it says it checked (0.827). */
  readonly sinceBrief?: SinceBrief
}

/**
 * The looks since the last morning brief (0.827): how many, how many could not read GitHub, the cards each watch
 * raised, and what was found again and set aside, by key (each counted once).
 */
interface SinceBrief {
  readonly looks: number
  readonly githubMissed: number
  readonly raised: { readonly github: number; readonly teammates: number; readonly comments: number }
  readonly repeated: readonly string[]
}
const NO_LOOKS: SinceBrief = { looks: 0, githubMissed: 0, raised: { github: 0, teammates: 0, comments: 0 }, repeated: [] }
const MAX_REPEATED = 500

function sinceBriefFrom(value: unknown): SinceBrief | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const { looks, githubMissed, raised, repeated } = value as Record<string, unknown>
  const count = (one: unknown): one is number => typeof one === 'number' && Number.isInteger(one) && one >= 0
  const of = (raised ?? {}) as Record<string, unknown>
  if (!count(looks) || !count(githubMissed) || !count(of.github) || !count(of.teammates) || !count(of.comments) || !Array.isArray(repeated)) return undefined
  return { looks, githubMissed, raised: { github: of.github, teammates: of.teammates, comments: of.comments }, repeated: repeated.filter((key): key is string => typeof key === 'string').slice(-MAX_REPEATED) }
}

/** One more look since the brief: whether it missed GitHub, the cards it raised, and what it set aside. */
function lookTallied(was: SinceBrief | undefined, look: { readonly githubMissed: boolean; readonly raised: readonly ScoutFact[]; readonly repeated: readonly string[] }): SinceBrief {
  const base = was ?? NO_LOOKS
  const raised = { ...base.raised }
  for (const fact of look.raised) {
    const watch = watchOfKey(fact.key)
    if (watch === 'github' || watch === 'teammates') raised[watch] += 1
    else if (fact.key.startsWith('comment')) raised.comments += 1
  }
  const repeated = [...base.repeated, ...look.repeated.filter((key) => !base.repeated.includes(key))].slice(-MAX_REPEATED)
  return { looks: base.looks + 1, githubMissed: base.githubMissed + (look.githubMissed ? 1 : 0), raised, repeated }
}

const EMPTY: Kept = { settings: DEFAULT_SCOUT_SETTINGS, cards: [], seen: [], lookIds: [], prs: [], notes: [] }
/** A look's turn, past this, no longer keeps its cards waiting: a short read, on a model that answers in seconds. */
const ANSWER_WAIT_MS = 3 * 60_000
const MAX_LOOK_IDS = 400
/** Free models a look tries after Scout's own, each when the one before it would not answer. */
const MAX_FALLBACKS = 2

/** A day's count as kept; anything else is none. */
function todayFrom(value: unknown): ScoutToday | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const { day, looks, asked } = value as Record<string, unknown>
  const count = (one: unknown): one is number => typeof one === 'number' && Number.isInteger(one) && one >= 0
  return typeof day === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(day) && count(looks) && count(asked) ? { day, looks, asked } : undefined
}

/** One more look, or one more turn of its model, on the day `at` falls on (0.815). */
function counted(was: ScoutToday | undefined, at: Date, what: 'looks' | 'asked'): ScoutToday {
  const day = scoutDay(at)
  const base = was?.day === day ? was : { day, looks: 0, asked: 0 }
  return { ...base, [what]: base[what] + 1 }
}

/** The same, into this week's tally (0.817): the first look starts a week. */
function weekCounted(was: ScoutWeek | undefined, at: Date, what: 'looks' | 'asked'): ScoutWeek {
  const base = was ?? emptyWeek(scoutDay(at))
  return { ...base, [what]: base[what] + 1 }
}

/** The cards a look raised, each counted under the watch it came from. */
function weekRaised(week: ScoutWeek, facts: readonly ScoutFact[]): ScoutWeek {
  let watches = week.watches
  for (const fact of facts) {
    const watch = watchOfKey(fact.key)
    if (watch !== undefined) watches = { ...watches, [watch]: { ...watches[watch], raised: watches[watch].raised + 1 } }
  }
  return { ...week, watches }
}

/** A week-old tally brings its tune-up; one far older is stale news, and only starts the next week (0.817). */
const WEEK_DAYS = 7
const STALE_WEEK_DAYS = 14

/** A look's turn under way: what it asked about, of whom, on which run, and which free models it has tried. */
interface PendingLook {
  readonly teammateId: string
  readonly prompt: string
  readonly cardIds: readonly string[]
  readonly team: readonly ScoutTeammate[]
  readonly runId: string
  readonly tried: readonly string[]
  /** When the look began: its model turn is counted on that day, not the one its turn started on (0.824). */
  readonly at: Date
  readonly expires: unknown
}

export function createScout(deps: ScoutDeps) {
  const now = deps.now ?? (() => new Date())
  const schedule = deps.schedule ?? ((task, ms) => setInterval(task, ms))
  const clear = deps.clear ?? ((handle) => clearInterval(handle as NodeJS.Timeout))
  const later = deps.later ?? ((task, ms) => setTimeout(task, ms))
  const cancel = deps.cancel ?? ((handle) => clearTimeout(handle as NodeJS.Timeout))
  let kept: Kept | undefined
  let timer: unknown
  let firstLook: unknown
  /** A due look waiting for the person's own turn to end (0.815). */
  let deferred: unknown
  let looking = false
  /** Failed turns heard as they ended, for the next look. */
  const failedSince: FailedTurn[] = []
  /** A look's turn, by its mission id. */
  const pending = new Map<string, PendingLook>()
  let writing: Promise<unknown> = Promise.resolve()

  /*
   * WHAT SCOUT KEPT, NEVER OVERWRITTEN BECAUSE IT COULD NOT BE READ (0.823). Any failed read was an empty Scout -- off,
   * no cards, no notes, no week -- and the next save wrote that over the file, so a file busy at start was a wiped
   * Scout (Grok's review, 2026-10-22). A missing file is a first start. A busy one is read again, three times. One that
   * does not parse is kept beside it as `scout.json.unreadable-<time>` and Scout starts fresh. One that still cannot be
   * read is left alone: Scout works from memory and saves nothing over it.
   */
  let saveToDisk = true
  const readKept = async (): Promise<string | undefined> => {
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await readFile(deps.file, 'utf8')
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined
        if (attempt >= 3) {
          saveToDisk = false
          return undefined
        }
        await new Promise((done) => later(() => done(undefined), 200 * 2 ** attempt))
      }
    }
  }
  const load = async (): Promise<Kept> => {
    if (kept !== undefined) return kept
    const text = await readKept()
    if (kept !== undefined) return kept
    if (text === undefined) {
      kept = EMPTY
      return kept
    }
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>
      kept = {
        settings: scoutSettingsFrom(parsed.settings),
        ...(typeof parsed.comments === 'object' && parsed.comments !== null ? { comments: parsed.comments as CommentWatch } : {}),
        ...(typeof parsed.commentTurns === 'object' && parsed.commentTurns !== null ? { commentTurns: parsed.commentTurns as Kept['commentTurns'] } : {}),
        cards: Array.isArray(parsed.cards) ? (parsed.cards as ScoutCard[]).filter((card) => typeof card?.cardId === 'string' && typeof card.said === 'string').map(({ reading: _reading, ...card }) => card) : [],
        seen: Array.isArray(parsed.seen) ? parsed.seen.filter((key): key is string => typeof key === 'string').slice(-MAX_SCOUT_SEEN) : [],
        lookIds: Array.isArray(parsed.lookIds) ? parsed.lookIds.filter((id): id is string => typeof id === 'string').slice(-MAX_LOOK_IDS) : [],
        prs: Array.isArray(parsed.prs) ? (parsed.prs as FolderPullRequest[]).filter((pr) => typeof pr?.url === 'string' && typeof pr.number === 'number') : [],
        notes: Array.isArray(parsed.notes) ? notesFrom(parsed.notes.filter((note): note is string => typeof note === 'string').join('\n')) : [],
        ...(typeof parsed.lastLookAt === 'string' ? { lastLookAt: parsed.lastLookAt } : {}),
        ...(typeof parsed.folderMark === 'string' ? { folderMark: parsed.folderMark } : {}),
        ...(typeof (parsed.lastCheck as { passed?: unknown } | undefined)?.passed === 'boolean' ? {
          lastCheck: {
            passed: (parsed.lastCheck as { passed: boolean }).passed,
            ...(typeof (parsed.lastCheck as { command?: unknown }).command === 'string' ? { command: (parsed.lastCheck as { command: string }).command } : {})
          }
        } : {}),
        ...(todayFrom(parsed.today) === undefined ? {} : { today: todayFrom(parsed.today)! }),
        ...(typeof parsed.lastBriefDay === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(parsed.lastBriefDay) ? { lastBriefDay: parsed.lastBriefDay } : {}),
        ...(typeof parsed.lastBriefAt === 'string' && Number.isFinite(Date.parse(parsed.lastBriefAt)) ? { lastBriefAt: parsed.lastBriefAt } : {}),
        ...(weekFrom(parsed.week) === undefined ? {} : { week: weekFrom(parsed.week)! }),
        ...(sinceBriefFrom(parsed.sinceBrief) === undefined ? {} : { sinceBrief: sinceBriefFrom(parsed.sinceBrief)! })
      }
    } catch {
      // Not a file Scout can read: kept beside it, never written over. A file that cannot even be moved is left as it is.
      await rename(deps.file, `${deps.file}.unreadable-${now().toISOString().replace(/[:.]/g, '-')}`).catch(() => {
        saveToDisk = false
      })
      kept = EMPTY
    }
    return kept
  }
  const stateOf = (from: Kept): ScoutState => ({
    settings: from.settings,
    cards: from.cards,
    lookIds: from.lookIds,
    ...(from.lastLookAt === undefined ? {} : { lastLookAt: from.lastLookAt }),
    ...(from.modelSaid === undefined ? {} : { modelSaid: from.modelSaid }),
    ...(from.notes.length === 0 ? {} : { notes: from.notes }),
    // Yesterday's count is not today's: a new day starts at none.
    ...(from.today === undefined || from.today.day !== scoutDay(now()) ? {} : { today: from.today })
  })
  /** Kept in memory at once, written whole beside the file and moved over it, one write at a time. */
  const keep = async (next: Kept, mustWrite = false): Promise<ScoutState> => {
    kept = next
    const body = `${JSON.stringify({
      comments: next.comments, commentTurns: next.commentTurns, settings: next.settings, cards: next.cards, seen: next.seen, lookIds: next.lookIds, prs: next.prs,
      ...(next.notes.length === 0 ? {} : { notes: next.notes }),
      ...(next.lastLookAt === undefined ? {} : { lastLookAt: next.lastLookAt }),
      ...(next.folderMark === undefined ? {} : { folderMark: next.folderMark }),
      ...(next.lastCheck === undefined ? {} : { lastCheck: next.lastCheck }),
      ...(next.today === undefined ? {} : { today: next.today }),
      ...(next.lastBriefDay === undefined ? {} : { lastBriefDay: next.lastBriefDay }),
      ...(next.lastBriefAt === undefined ? {} : { lastBriefAt: next.lastBriefAt }),
      ...(next.week === undefined ? {} : { week: next.week }),
      ...(next.sinceBrief === undefined ? {} : { sinceBrief: next.sinceBrief })
    })}\n`
    const write = writing.then(async () => {
      // A file Scout could not read is not written over (see `load`): this run's changes live in memory. A change that
      // must be on disk first -- a post's attempt, so a restart never posts it twice -- is refused instead.
      if (!saveToDisk) {
        if (mustWrite) throw new Error('Scout could not save to its own file, so nothing was posted.')
        return
      }
      const temporary = `${deps.file}.${randomBytes(6).toString('hex')}.tmp`
      try {
        await writeFile(temporary, body, { encoding: 'utf8', flag: 'wx' })
        await replaceFile(temporary, deps.file)
      } catch (error) {
        await rm(temporary, { force: true }).catch(() => undefined)
        if (mustWrite) throw error
      }
    })
    writing = write.catch(() => undefined)
    await write
    const state = stateOf(next)
    deps.changed(state)
    return state
  }
  /*
   * EVERY CHANGE IS ONE STEP ON THE NEWEST STATE. A change read what Scout held, awaited, then wrote: a card dismissed
   * while a look was writing its own came back. Now each change is worked out from what is held at the moment it is
   * kept, with nothing awaited between the reading and the keeping.
   */
  const mutate = async (change: (from: Kept) => Kept, mustWrite = false): Promise<ScoutState> => {
    const loaded = await load()
    return keep(change(kept ?? loaded), mustWrite)
  }

  const restart = (settings: ScoutSettings): void => {
    if (timer !== undefined) clear(timer)
    timer = undefined
    if (settings.on && settings.teammateId !== undefined) timer = schedule(() => { due() }, settings.everyMinutes * 60_000)
  }

  /*
   * A LOOK ON SCOUT'S OWN TIMER (0.815). Not in quiet hours, and not beside the person's own turn: one due while a turn
   * they started is running waits for it to end, asking again every minute, and looks anyway after SCOUT_WAIT_MOST_MS
   * so a long run never silences Scout. A look the person asks for -- Look now, turning it on -- runs at once.
   */
  const due = (since = now().getTime()): void => {
    if (deferred !== undefined) return
    const waits = kept?.settings.waitWhileWorking ?? DEFAULT_SCOUT_SETTINGS.waitWhileWorking
    if (waits && deps.personBusy?.() === true && now().getTime() - since < SCOUT_WAIT_MOST_MS) {
      deferred = later(() => { deferred = undefined; due(since) }, WAIT_STEP_MS)
      return
    }
    void look(true)
  }

  /** Locust's own look: what is new since the last one, as facts. */
  const gather = async (from: Kept, lookStamp: string): Promise<{ facts: readonly ScoutFact[]; prs: readonly FolderPullRequest[]; marks: Pick<Kept, 'folderMark' | 'lastCheck' | 'comments'>; githubRead: boolean; checkStale: boolean; repeated: readonly string[] }> => {
    const facts: ScoutFact[] = []
    let prs = from.prs
    /** Whether this look read GitHub, and whether the folder moved past a check it did not run: for the brief (0.823). */
    let githubRead = false
    let checkStale = false
    /*
     * THE FOLDER'S CHECK (0.815), only when the folder moved since the check last ran -- a commit, or a change to what
     * is in it -- and never while a turn is working there, whose half-done work would read as broken. Not run, the
     * mark stays where it was, so the next look runs it.
     */
    let marks: Pick<Kept, 'folderMark' | 'lastCheck' | 'comments'> = {
      ...(from.folderMark === undefined ? {} : { folderMark: from.folderMark }),
      ...(from.lastCheck === undefined ? {} : { lastCheck: from.lastCheck })
    }
    if (from.settings.watches.check && deps.folderMark !== undefined && deps.runCheck !== undefined && deps.folderBusy?.() !== true) {
      const mark = await deps.folderMark().catch(() => undefined)
      // A turn that began while the folder was read, or while its check ran (0.823, Grok's review): its half-done work
      // would read as a failure needing the person now. The result is dropped and the mark stays: a later look runs it.
      if (mark !== undefined && mark !== from.folderMark && deps.folderBusy?.() !== true) {
        const result = await deps.runCheck().catch(() => undefined)
        if (result !== undefined && deps.folderBusy?.() !== true) {
          facts.push(...checkFacts(from.lastCheck, result, mark))
          marks = { folderMark: mark, lastCheck: { passed: result.passed, command: result.command } }
        }
      }
      checkStale = mark !== undefined && mark !== marks.folderMark
    }
    if (from.settings.watches.comments !== false && deps.comments !== undefined) {
      const watch = from.comments ?? { enabledAt: lookStamp, repositories: {} }
      const result = await deps.comments(watch, (await deps.team().catch(() => [])).filter((mate) => mate.teammateId !== from.settings.teammateId), lookStamp).catch(() => ({ watch, facts: [] }))
      marks = { ...marks, comments: result.watch }
      facts.push(...result.facts)
    }
    if (from.settings.watches.github) {
      const github = await deps.github().catch(() => undefined)
      if (github !== undefined) {
        githubRead = true
        facts.push(...pullRequestFacts(new Map(from.prs.map((pr) => [pr.url, pr] as const)), github.prs, lookStamp, now()))
        facts.push(...reviewFacts(github.reviews))
        if (github.branch !== undefined) facts.push(...branchFacts(github.branch.name, github.branch.runs))
        facts.push(...issueFacts(github.issues ?? [], now()))
        prs = github.prs
      }
    }
    if (from.settings.watches.teammates) {
      facts.push(...waitingFacts(await deps.waiting().catch(() => []), now()))
      facts.push(...failedFacts(failedSince.splice(0)))
    } else {
      failedSince.splice(0)
    }
    const seen = new Set(from.seen)
    // Found again, and told already: set aside, and counted for the brief (0.827).
    const repeated = [...new Set(facts.filter((fact) => seen.has(fact.key)).map((fact) => fact.key))]
    return { facts: facts.filter((fact, index) => !seen.has(fact.key) && facts.findIndex((other) => other.key === fact.key) === index), prs, marks, githubRead, checkStale, repeated }
  }

  /** The morning brief's fact (0.816): who waits, what ran since the last brief, the check, pull requests, the cards. */
  /*
   * What the brief says is what is true as it is said (0.823, Grok's review): the span it counts is named; GitHub's line
   * is left out when this look could not read GitHub, not filled from the last list; a check the folder moved past
   * since it ran says so; and its cards are the ones that will still be on Home after this look -- a check that turned
   * counted once, not as the old card and the new, and the weekly tune-up raised beside it counted too.
   */
  const briefOf = async (from: Kept, news: readonly ScoutFact[], prs: readonly FolderPullRequest[], marks: Pick<Kept, 'folderMark' | 'lastCheck' | 'comments'>, at: Date, read: { readonly githubRead: boolean; readonly checkStale: boolean }, replaced: (card: ScoutCard) => boolean, looks: SinceBrief): Promise<ScoutFact> => {
    const since = from.lastBriefAt ?? new Date(at.getTime() - 24 * 60 * 60_000).toISOString()
    const ran = await deps.briefFacts?.(since, from.lookIds).catch(() => undefined)
    const waiting = from.settings.watches.teammates ? await deps.waiting().catch(() => []) : []
    // The waits are told on their own line: a card for one is not counted again among the cards.
    const counts = (card: Pick<ScoutFact, 'key'>): boolean => !card.key.startsWith('brief:') && !card.key.startsWith('waiting:')
    const others = from.cards.filter((card) => counts(card) && !replaced(card))
    const told = news.filter(counts)
    const check = marks.lastCheck
    const needsWork = (pr: FolderPullRequest): boolean => pr.checks === 'failing' || pr.review === 'changes-requested' || pr.conflicts === true
    const open = prs.filter((pr) => pr.state === 'open')
    /*
     * Checked and quiet (0.827): a watch that was on, read, and raised nothing since the last brief, with nothing of it
     * waiting now. GitHub only when some look read it; the folder's check has its own line.
     */
    const quiet = [
      ...(from.settings.watches.github && looks.looks > looks.githubMissed && looks.raised.github === 0 && !open.some(needsWork) ? ['GitHub'] : []),
      ...(from.settings.watches.teammates && looks.raised.teammates === 0 && waiting.length === 0 && (ran?.failed ?? 0) === 0 ? ['your teammates'] : []),
      ...(from.settings.watches.comments !== false && deps.comments !== undefined && looks.raised.comments === 0 ? ['comments asking Locust'] : [])
    ]
    const held = deps.heldAlerts?.(Date.parse(since)) ?? []
    return briefFact({
      waiting: waiting.map((card) => ({ ...(card.name === undefined ? {} : { name: card.name }), minutes: Math.max(0, Math.round((at.getTime() - Date.parse(card.requestedAt)) / 60_000)) })),
      finished: ran?.finished ?? 0,
      failed: ran?.failed ?? 0,
      ...(ran?.more === true ? { more: true } : {}),
      span: briefSpan(new Date(since), at),
      ...(from.settings.watches.check && check?.command !== undefined ? { check: { command: check.command, passed: check.passed, ...(read.checkStale ? { stale: true } : {}) } } : {}),
      ...(from.settings.watches.github && read.githubRead && open.length > 0 ? { prs: { open: open.length, needsWork: open.filter(needsWork).length } } : {}),
      cards: { total: others.length + told.length, now: [...others, ...told].filter((one) => one.urgency === 'now').length },
      ...(quiet.length === 0 ? {} : { quiet }),
      ...(from.settings.watches.github && looks.githubMissed > 0 ? { githubMissed: { missed: looks.githubMissed, looks: looks.looks } } : {}),
      ...(looks.repeated.length === 0 ? {} : { setAside: looks.repeated.length }),
      ...(held.length === 0 ? {} : { held: held.map((one) => ({ title: one.title, why: one.why })) })
    }, scoutDay(at))
  }

  /*
   * OFF, AND QUIET HOURS, AGAIN AFTER EVERY WAIT (0.823, Grok's review). Both were read once, as a look began, and a
   * look that then waited minutes on the folder's check went on to raise cards, tell Windows and ask the model after
   * Scout was turned off or its quiet hours began. A look on Scout's own timer keeps quiet hours; one asked for does not.
   */
  const stillLooking = (scheduled: boolean): boolean => {
    const settings = (kept ?? EMPTY).settings
    return settings.on && !(scheduled && inQuietHours(settings.quiet, now()))
  }
  const OFF_SAYS = 'Scout was turned off, so these cards stay in Locust’s words.'

  /** `scheduled`: on Scout's own timer, which keeps quiet hours (0.815); a look the person asked for does not. */
  const look = async (scheduled = false): Promise<ScoutState> => {
    const from = await load()
    if (looking || !from.settings.on || from.settings.teammateId === undefined) return stateOf(from)
    if (scheduled && inQuietHours(from.settings.quiet, now())) return stateOf(from)
    looking = true
    try {
      const teammateId = from.settings.teammateId
      // Scout taken off the team is Scout turned off: nothing looks for a teammate who is gone.
      if (!(await deps.teammateExists(teammateId).catch(() => true))) {
        const state = await mutate((latest) => ({ ...latest, settings: { ...latest.settings, on: false } }))
        restart(state.settings)
        return state
      }
      const at = now()
      const stamp = at.toISOString()
      const { facts: found, prs, marks, githubRead, checkStale, repeated } = await gather(from, stamp)
      const tallied = (was: SinceBrief | undefined): SinceBrief => lookTallied(was, { githubMissed: from.settings.watches.github && !githubRead, raised: found, repeated })
      // Off, or quiet hours begun, while it gathered: nothing is kept, so the next look finds all of it again.
      if (!stillLooking(scheduled)) return stateOf(kept ?? from)
      const facts = [...found]
      /*
       * THE MORNING BRIEF (0.816): the first look from its hour on, once a day and never in quiet hours -- so a Locust
       * opened at ten brings the seven o'clock brief with its first look. It comes after the look's own news, which it
       * counts among the cards waiting.
       */
      const day = scoutDay(at)
      /*
       * THE WEEKLY TUNE-UP (0.817): the first look a week after the tally began brings it, and the next week starts with
       * this look. A tally far older -- Scout off, or Locust closed, for a week or more -- only starts the next week.
       * Worked out before the brief, which counts it among the cards (0.823), and raised after it.
       */
      const age = from.week === undefined ? 0 : daysBetween(from.week.from, day)
      const weekDue = age >= WEEK_DAYS
      const week = weekDue && age < STALE_WEEK_DAYS && from.week !== undefined && from.week.looks > 0 ? weekFact(from.week, from.settings) : undefined
      const weekOf = (latest: Kept): ScoutWeek | undefined => (weekDue ? emptyWeek(day) : latest.week)
      // A check that turned says how it stands now: the card of how it stood before goes (0.815); so do yesterday's
      // brief and last week's tune-up.
      const checkTurned = facts.some((fact) => fact.key.startsWith('check:'))
      const briefDue = from.settings.briefAt !== undefined && at.getHours() >= from.settings.briefAt && from.lastBriefDay !== day && !inQuietHours(from.settings.quiet, at)
      const outdated = (card: ScoutCard): boolean => (checkTurned && card.key.startsWith('check:')) || (briefDue && card.key.startsWith('brief:')) || (weekDue && card.key.startsWith('week:'))
      if (briefDue) facts.push(await briefOf(from, week === undefined ? facts : [...facts, week], prs, marks, at, { githubRead, checkStale }, outdated, tallied(from.sinceBrief)))
      if (briefDue && !stillLooking(scheduled)) return stateOf(kept ?? from)
      if (week !== undefined) facts.push(week)
      const briefed = briefDue ? { lastBriefDay: day, lastBriefAt: stamp } : {}
      // A brief counts this look in; the next one counts from here (0.827).
      const sinceBrief = (latest: Kept): SinceBrief => (briefDue ? NO_LOOKS : tallied(latest.sinceBrief))
      if (facts.length === 0) return await mutate((latest) => ({ ...latest, ...marks, prs, lastLookAt: stamp, today: counted(latest.today, at, 'looks'), week: weekCounted(weekOf(latest), at, 'looks'), sinceBrief: sinceBrief(latest) }))
      const team = (await deps.team().catch(() => [])).filter((mate) => mate.teammateId !== teammateId)
      /*
       * Locust's own counts and good news are not put to Scout's model. The weekly tune-up (0.817): the first packaged
       * drive's model drafted a handoff to Ada to review Scout's own dismissed cards, which no teammate can see. The
       * morning brief and a check that passes again (0.818): on the first live brief, 2026-10-22, the model's take on
       * the brief was a paragraph on capturing evidence, and the passing check got a draft asking Ada to run it again.
       * Neither has anything for a teammate to do. A look with nothing else new asks no model at all.
       */
      const forModel = (fact: ScoutFact): boolean => fact.commentAsk?.namedTeammateId === undefined && !fact.key.startsWith('comment-answer:') && !fact.key.startsWith('week:') && !fact.key.startsWith('brief:') && !/^check:.*:passes$/.test(fact.key)
      const fallback = facts.some((fact) => fact.commentAsk !== undefined && fact.commentAsk.namedTeammateId === undefined) ? await deps.lastTeammate?.().catch(() => undefined) : undefined
      const draftedFacts = facts.map((fact) => {
        if (fact.commentAsk === undefined) return fact
        const mate = team.find((mate) => mate.teammateId === (fact.commentAsk!.namedTeammateId ?? fallback)) ?? team[0]
        return mate === undefined ? fact : { ...fact, draft: { teammateId: mate.teammateId, name: mate.name, message: fact.commentAsk.message } }
      })
      const cards = draftedFacts.map((fact) => cardOf(fact, `card_${randomBytes(6).toString('hex')}`, at, forModel(fact)))
      let state = await mutate((latest) => ({
        ...latest,
        ...marks,
        ...briefed,
        cards: withNewCards(latest.cards.filter((card) => !outdated(card)), cards),
        seen: [...latest.seen, ...facts.map((fact) => fact.key)].slice(-MAX_SCOUT_SEEN),
        prs,
        lastLookAt: stamp,
        today: counted(latest.today, at, 'looks'),
        week: weekRaised(weekCounted(weekOf(latest), at, 'looks'), facts),
        sinceBrief: sinceBrief(latest)
      }))
      // Nothing in Windows in quiet hours, even from a look asked for: the cards wait on Home (0.815). Quiet hours as they
      // are now, not as the look began (0.823): a look begun at 9:59 told Windows at 10:04.
      if (state.settings.notify && state.settings.on && !inQuietHours(state.settings.quiet, now())) for (const card of cards) if (card.urgency === 'now') deps.notify(card)
      if (state.settings.on && cards.length > 0) deps.raised?.(cards)
      // Then Scout's model, for its take and the next step.
      const asked = cards.filter(forModel)
      if (asked.length === 0) return state
      if (!stillLooking(scheduled)) {
        return settle(asked.map((card) => card.cardId), undefined, state.settings.on ? 'Scout’s quiet hours began, so these cards stay in Locust’s words.' : OFF_SAYS)
      }
      const prompt = scoutPrompt({
        facts: facts.filter(forModel),
        team: team.map((mate) => ({ name: mate.name, role: mate.role })),
        ...(state.settings.goal === undefined ? {} : { goal: state.settings.goal }),
        ...(kept === undefined || kept.notes.length === 0 ? {} : { notes: kept.notes })
      })
      state = await askFor({ teammateId, prompt, cardIds: asked.map((card) => card.cardId), team, tried: [], at }) ?? state
      return state
    } finally {
      looking = false
    }
  }

  /**
   * One turn for a look, on Scout's model or on the free one given in its place. A turn that does not start settles
   * its cards with why; one that starts is waited on, and stops waiting -- and is stopped -- if it runs out of time.
   */
  const askFor = async (look: Omit<PendingLook, 'runId' | 'expires'>, model?: string): Promise<ScoutState | undefined> => {
    const asked = await deps.ask(look.teammateId, look.prompt, model)
      .catch((error: unknown) => ({ refused: error instanceof Error ? error.message : 'The look could not start.' }))
    if ('refused' in asked) return settle(look.cardIds, undefined, asked.refused)
    // Turned off while its turn was starting (0.823): off found nothing waiting then, so the turn is stopped here.
    if (!(kept ?? EMPTY).settings.on) {
      try { deps.stop(asked.runId) } catch { /* it has ended already */ }
      return settle(look.cardIds, undefined, OFF_SAYS)
    }
    const expires = later(() => { void gaveUp(asked.missionId, 'Scout’s model took too long to answer.') }, deps.answerWaitMs ?? ANSWER_WAIT_MS)
    pending.set(asked.missionId, { ...look, runId: asked.runId, expires })
    // On the day the look began (Grok's review): a look from 11:59 PM that asks after midnight counted a turn of its
    // model on a day with no look.
    return mutate((latest) => ({ ...latest, lookIds: [...latest.lookIds, asked.missionId].slice(-MAX_LOOK_IDS), today: counted(latest.today, look.at, 'asked'), week: weekCounted(latest.week, look.at, 'asked') }))
  }

  /*
   * OFF MEANS OFF (0.815). Turned off, the timer stopping was all: a look's turn already started ran on, and a look
   * waiting for the person's turn still came. Now each is stopped, and its cards stay as Locust wrote them.
   */
  const stopLooks = async (why: string): Promise<void> => {
    if (firstLook !== undefined) cancel(firstLook)
    firstLook = undefined
    if (deferred !== undefined) cancel(deferred)
    deferred = undefined
    const cardIds: string[] = []
    for (const look of pending.values()) {
      cancel(look.expires)
      try { deps.stop(look.runId) } catch { /* it has ended already */ }
      cardIds.push(...look.cardIds)
    }
    pending.clear()
    if (cardIds.length > 0) await settle(cardIds, undefined, why)
  }

  /*
   * A LOOK WHOSE MODEL WILL NOT ANSWER (2026-10-12, the packaged 0.756 drive): Space Bunny's provider answered "Free
   * usage exceeded", OpenCode tried again on its own for as long as it ran, and the cards said "Scout is reading it"
   * until the look's time ran out -- its turn still running, so every later look found Scout busy. Now the turn is
   * stopped, and the look tries the next free model, as the design said it would; past MAX_FALLBACKS, or on a route
   * that is not a free one, the cards stay in Locust's words and say why.
   */
  const gaveUp = async (missionId: string, why: string): Promise<void> => {
    const look = pending.get(missionId)
    if (look === undefined) return
    pending.delete(missionId)
    cancel(look.expires)
    try { deps.stop(look.runId) } catch { /* it has ended already */ }
    // No other model once Scout is off (0.823): giving up on one used to start the next whatever Scout's state.
    const next = look.tried.length >= MAX_FALLBACKS || !(kept ?? EMPTY).settings.on ? undefined : await deps.nextFreeModel(look.teammateId, look.tried).catch(() => undefined)
    if (next === undefined) {
      await settle(look.cardIds, undefined, `${why} These cards are in Locust’s words.`)
      return
    }
    const { runId: _runId, expires: _expires, ...rest } = look
    await askFor({ ...rest, tried: [...look.tried, next] }, next)
  }

  /** The cards a look asked about, no longer waiting: with what the model added, or why it added nothing. */
  const settle = (cardIds: readonly string[], answers: ReadonlyMap<number, { readonly take?: string; readonly draft?: ScoutCard['draft'] }> | undefined, why?: string): Promise<ScoutState> =>
    mutate((from) => {
      const cards = from.cards.map((card) => {
        const at = cardIds.indexOf(card.cardId)
        if (at < 0) return card
        const { reading: _reading, ...rest } = card
        const answer = answers?.get(at + 1)
        return { ...rest, ...(answer?.take === undefined ? {} : { take: answer.take }), ...(answer?.draft === undefined ? {} : { draft: card.commentAsk === undefined ? answer.draft : { ...answer.draft, message: card.commentAsk.message } }) }
      })
      // An answer that came clears the last one's excuse; one that did not says why.
      const { modelSaid: _was, ...base } = from
      return { ...base, cards, ...(why === undefined ? {} : { modelSaid: why }) }
    })

  const ended = async (missionId: string): Promise<void> => {
    const look = pending.get(missionId)
    if (look === undefined) return
    const answer = await deps.answerOf(missionId).catch(() => ({ failed: 'Scout’s model’s answer could not be read.' }))
    // A turn that ended without an answer is a model that would not answer: the next free one is tried.
    if ('failed' in answer) {
      await gaveUp(missionId, answer.failed)
      return
    }
    pending.delete(missionId)
    cancel(look.expires)
    await settle(look.cardIds, scoutAnswers(answer.text, look.cardIds.length, look.team))
    // A note the look kept (0.816): dated by Locust, said once, the oldest going past the most kept.
    const note = scoutNoteOf(answer.text)
    if (note !== undefined) {
      await mutate((latest) => latest.notes.some((kept) => kept.endsWith(note))
        ? latest
        : { ...latest, notes: [...latest.notes, `${scoutDay(now())}: ${note}`].slice(-MAX_SCOUT_NOTES) })
    }
  }

  const applySettings = async (change: unknown): Promise<ScoutState> => {
    let before: ScoutSettings | undefined
    const state = await mutate((latest) => {
      before = latest.settings
      // The window may not name Scout's teammate: Scout made it, and keeps it.
      return { ...latest, settings: scoutSettingsFrom({ ...(typeof change === 'object' && change !== null ? change : {}), teammateId: latest.settings.teammateId }, latest.settings) }
    })
    if (state.settings.on && state.settings.watches.comments !== false && (await load()).comments === undefined) await mutate((latest) => ({ ...latest, comments: { enabledAt: now().toISOString(), repositories: {} } }))
    if (state.settings.everyMinutes !== before?.everyMinutes || state.settings.on !== before.on) restart(state.settings)
    if (before?.on === true && !state.settings.on) {
      await stopLooks(OFF_SAYS)
      return stateOf(await load())
    }
    return state
  }

  const answerComment = async (missionId: string): Promise<void> => {
    const turn = (await load()).commentTurns?.[missionId]
    if (turn === undefined) return
    const answer = await deps.commentAnswer?.(missionId)
    if (answer === undefined) return
    const fact = commentAnswerFact(turn.ask, missionId, turn.name, answer)
    await mutate((latest) => {
      if (latest.commentTurns?.[missionId] === undefined) return latest
      const turns = { ...latest.commentTurns }
      delete turns[missionId]
      return { ...latest, commentTurns: turns, cards: withNewCards(latest.cards, [cardOf(fact, `card_${randomBytes(6).toString('hex')}`, now(), false)]) }
    })
  }

  return {
    /** Bound by the host only after a successful person-origin start. Reconcile fast completions too. */
    async sentComment(cardId: unknown, missionId: string, teammateId: string | undefined): Promise<void> {
      const card = (await load()).cards.find((one) => one.cardId === cardId)
      if (card?.commentAsk === undefined) return
      const mate = (await deps.team()).find((one) => one.teammateId === teammateId)
      if (mate === undefined) return
      await mutate((latest) => ({ ...latest, cards: latest.cards.filter((one) => one.cardId !== cardId), commentTurns: { ...latest.commentTurns, [missionId]: { ask: card.commentAsk!, name: mate.name } } }))
      await answerComment(missionId)
    },
    async editPost(cardId: unknown, text: unknown): Promise<ScoutState> {
      return mutate((latest) => ({ ...latest, cards: latest.cards.map((card) => card.cardId === cardId && card.post !== undefined && !card.post.attempted && typeof text === 'string' && text.trim().length > 0 && text.length <= 200_000 ? { ...card, post: { ...card.post, message: text } } : card) }))
    },
    async post(cardId: unknown): Promise<ScoutState> {
      let claimed: ScoutCard | undefined
      await mutate((latest) => ({ ...latest, cards: latest.cards.map((card) => {
        if (card.cardId !== cardId || card.post === undefined || card.post.attempted || deps.postComment === undefined) return card
        claimed = card
        return { ...card, post: { ...card.post, attempted: true, failed: 'Posting has not been confirmed. Check GitHub before posting again.' } }
      }) }), true)
      if (claimed?.post === undefined) return stateOf(await load())
      const post = claimed.post
      let result: { postedUrl: string } | { failed: string }
      try { result = { postedUrl: await deps.postComment!(post.repository, post.number, post.message) } }
      catch (error) { result = { failed: `GitHub did not confirm the post: ${error instanceof Error ? error.message : 'The request failed.'} Nothing will be retried on its own.` } }
      return mutate((latest) => ({ ...latest, cards: latest.cards.map((card) => card.cardId === cardId ? { ...card, urgency: 'later', post: { ...post, attempted: true, ...result } } : card) }))
    },
    state: async (): Promise<ScoutState> => stateOf(await load()),
    /** Picks up where the last launch left off: on, it looks again shortly and on its timer after. */
    async start(firstLookMs = 30_000): Promise<void> {
      const from = await load()
      // A look's cards from before a restart wait on nothing now.
      if (from.cards.some((card) => card.reading === true)) await mutate((latest) => ({ ...latest, cards: latest.cards.map(({ reading: _reading, ...card }) => card) }))
      for (const missionId of Object.keys(from.commentTurns ?? {})) await answerComment(missionId)
      if (from.settings.on && from.settings.watches.comments !== false && from.comments === undefined) await mutate((latest) => ({ ...latest, comments: { enabledAt: now().toISOString(), repositories: {} } }))
      restart(from.settings)
      if (from.settings.on) firstLook = later(() => { firstLook = undefined; due() }, firstLookMs)
    },
    /** On again with its own teammate; a new Scout, on this route, when it has none. */
    async turnOn(route: { readonly runtime: string; readonly model: string; readonly account?: string } | undefined): Promise<ScoutState | { readonly refused: string }> {
      const from = await load()
      let teammateId = from.settings.teammateId
      if (teammateId === undefined || !(await deps.teammateExists(teammateId).catch(() => false))) {
        if (route === undefined) return { refused: 'Scout needs a model to read with: install OpenCode for a free one, then turn it on.' }
        try {
          teammateId = await deps.makeTeammate(route)
        } catch (error) {
          return { refused: error instanceof Error ? error.message : 'Scout could not be added to the team.' }
        }
      }
      const madeId = teammateId
      const state = await mutate((latest) => ({ ...latest, settings: { ...latest.settings, on: true, teammateId: madeId }, ...(latest.comments === undefined && latest.settings.watches.comments !== false ? { comments: { enabledAt: now().toISOString(), repositories: {} } } : {}) }))
      restart(state.settings)
      return look()
    },
    set: applySettings,
    /**
     * The card goes; what the person did with it -- sent its draft on, changed it first, or dismissed it -- counts toward
     * this week's tune-up when the card came this week (0.817).
     */
    dismiss: (cardId: unknown, how?: unknown): Promise<ScoutState> => mutate((latest) => {
      const card = latest.cards.find((one) => one.cardId === cardId)
      const cards = latest.cards.filter((one) => one.cardId !== cardId)
      const watch = card === undefined ? undefined : watchOfKey(card.key)
      const week = latest.week
      if (card === undefined || watch === undefined || week === undefined || scoutDay(new Date(card.at)) < week.from) return { ...latest, cards }
      const outcome: ScoutOutcome = how === 'sent' || how === 'changed' ? how : 'dismissed'
      const was = week.watches[watch]
      const counts = outcome === 'dismissed' ? { ...was, dismissed: was.dismissed + 1 } : { ...was, used: was.used + 1 }
      return { ...latest, cards, week: { ...week, watches: { ...week.watches, [watch]: counts }, [outcome]: week[outcome] + 1 } }
    }),
    /** A weekly tune-up's suggestion, applied to the settings as they stand now (0.817); its card goes. */
    async tune(cardId: unknown): Promise<ScoutState> {
      const suggested = (await load()).cards.find((card) => card.cardId === cardId)?.tune
      if (suggested?.watchOff !== undefined) await applySettings({ watches: { [suggested.watchOff]: false } })
      else if (suggested?.everyMinutes !== undefined) await applySettings({ everyMinutes: suggested.everyMinutes })
      return mutate((latest) => ({ ...latest, cards: latest.cards.filter((card) => card.cardId !== cardId) }))
    },
    /** The notes as the person wrote them in Settings (0.816): what they keep is what the next look reads. */
    saveNotes: (text: unknown): Promise<ScoutState> => mutate((latest) => ({ ...latest, notes: notesFrom(typeof text === 'string' ? text : '') })),
    look,
    /** A look's turn whose provider will not answer while the runtime keeps trying: stopped, and the next model tried. */
    async providerRefused(missionId: string, why: string): Promise<void> {
      if (pending.has(missionId)) await gaveUp(missionId, why)
    },
    /** Every run that ends: a look's turn fills its cards in; a teammate's failed turn is news for the next look. */
    async onRunEnded(mission: { readonly missionId: string }): Promise<void> {
      const commentState = await load()
      const isCommentTurn = commentState.commentTurns?.[mission.missionId] !== undefined || commentState.cards.some((card) => card.key.startsWith('comment-answer:') && card.open?.kind === 'conversation' && card.open.missionId === mission.missionId)
      await answerComment(mission.missionId)
      if (isCommentTurn) return
      if (pending.has(mission.missionId)) {
        await ended(mission.missionId)
        return
      }
      const from = await load()
      if (from.lookIds.includes(mission.missionId)) return
      /*
       * A CONVERSATION THAT WENT ON IS NOT STUCK (0.797). When a later turn of it ends -- the person's reply, or Locust
       * carrying the work on when a plan ran out -- the failure before it is no longer news: it leaves the next look,
       * and a card already showing it goes. Colin, 2026-10-19, on a card drafting a handoff to redo work Claude Code
       * had already finished after Codex's limit: "outdated?"
       */
      const earlier = await deps.continuesFrom?.(mission.missionId).catch(() => undefined)
      if (earlier !== undefined) {
        for (let index = failedSince.length - 1; index >= 0; index -= 1) if (failedSince[index]!.missionId === earlier) failedSince.splice(index, 1)
        const key = `failed:${earlier}`
        if (from.cards.some((card) => card.key === key)) await mutate((latest) => ({ ...latest, cards: latest.cards.filter((card) => card.key !== key) }))
      }
      if (!from.settings.on || !from.settings.watches.teammates) return
      const failed = await deps.failedTurn(mission.missionId).catch(() => undefined)
      if (failed !== undefined && failed.teammateId !== from.settings.teammateId) failedSince.push(failed)
    },
    dispose(): void {
      if (timer !== undefined) clear(timer)
      timer = undefined
      if (firstLook !== undefined) cancel(firstLook)
      firstLook = undefined
      if (deferred !== undefined) cancel(deferred)
      deferred = undefined
      for (const look of pending.values()) {
        cancel(look.expires)
        try { deps.stop(look.runId) } catch { /* ended already */ }
      }
      pending.clear()
    }
  }
}

export type Scout = ReturnType<typeof createScout>
