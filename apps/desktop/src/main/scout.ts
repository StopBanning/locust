import { randomBytes } from 'node:crypto'
import { readFile, rm, writeFile } from 'node:fs/promises'

import type { FolderPullRequest } from '../shared/pull-request.js'
import {
  DEFAULT_SCOUT_SETTINGS,
  MAX_SCOUT_NOTES,
  MAX_SCOUT_SEEN,
  SCOUT_WAIT_MOST_MS,
  branchFacts,
  briefFact,
  cardOf,
  checkFacts,
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
  withNewCards,
  type BranchRun,
  type FailedTurn,
  type FolderCheckResult,
  type IssueOpened,
  type ReviewAsked,
  type ScoutCard,
  type ScoutFact,
  type ScoutSettings,
  type ScoutState,
  type ScoutToday,
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
  readonly briefFacts?: (since: string, looks: readonly string[]) => Promise<{ readonly finished: number; readonly failed: number }>
}

/** How often a look that is waiting for the person's own turn asks again whether it has ended. */
const WAIT_STEP_MS = 60_000

interface Kept {
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
  /** Each pull request as the last look saw it, by its address. */
  readonly prs: readonly FolderPullRequest[]
  readonly lastLookAt?: string
  readonly modelSaid?: string
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

/** A look's turn under way: what it asked about, of whom, on which run, and which free models it has tried. */
interface PendingLook {
  readonly teammateId: string
  readonly prompt: string
  readonly cardIds: readonly string[]
  readonly team: readonly ScoutTeammate[]
  readonly runId: string
  readonly tried: readonly string[]
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

  const load = async (): Promise<Kept> => {
    if (kept !== undefined) return kept
    try {
      const parsed = JSON.parse(await readFile(deps.file, 'utf8')) as Record<string, unknown>
      kept = {
        settings: scoutSettingsFrom(parsed.settings),
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
        ...(typeof parsed.lastBriefAt === 'string' && Number.isFinite(Date.parse(parsed.lastBriefAt)) ? { lastBriefAt: parsed.lastBriefAt } : {})
      }
    } catch {
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
  const keep = async (next: Kept): Promise<ScoutState> => {
    kept = next
    const body = `${JSON.stringify({
      settings: next.settings, cards: next.cards, seen: next.seen, lookIds: next.lookIds, prs: next.prs,
      ...(next.notes.length === 0 ? {} : { notes: next.notes }),
      ...(next.lastLookAt === undefined ? {} : { lastLookAt: next.lastLookAt }),
      ...(next.folderMark === undefined ? {} : { folderMark: next.folderMark }),
      ...(next.lastCheck === undefined ? {} : { lastCheck: next.lastCheck }),
      ...(next.today === undefined ? {} : { today: next.today }),
      ...(next.lastBriefDay === undefined ? {} : { lastBriefDay: next.lastBriefDay }),
      ...(next.lastBriefAt === undefined ? {} : { lastBriefAt: next.lastBriefAt })
    })}\n`
    const write = writing.then(async () => {
      const temporary = `${deps.file}.${randomBytes(6).toString('hex')}.tmp`
      try {
        await writeFile(temporary, body, { encoding: 'utf8', flag: 'wx' })
        await replaceFile(temporary, deps.file)
      } catch {
        await rm(temporary, { force: true }).catch(() => undefined)
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
  const mutate = async (change: (from: Kept) => Kept): Promise<ScoutState> => {
    const loaded = await load()
    return keep(change(kept ?? loaded))
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
  const gather = async (from: Kept, lookStamp: string): Promise<{ facts: readonly ScoutFact[]; prs: readonly FolderPullRequest[]; marks: Pick<Kept, 'folderMark' | 'lastCheck'> }> => {
    const facts: ScoutFact[] = []
    let prs = from.prs
    /*
     * THE FOLDER'S CHECK (0.815), only when the folder moved since the check last ran -- a commit, or a change to what
     * is in it -- and never while a turn is working there, whose half-done work would read as broken. Not run, the
     * mark stays where it was, so the next look runs it.
     */
    let marks: Pick<Kept, 'folderMark' | 'lastCheck'> = {
      ...(from.folderMark === undefined ? {} : { folderMark: from.folderMark }),
      ...(from.lastCheck === undefined ? {} : { lastCheck: from.lastCheck })
    }
    if (from.settings.watches.check && deps.folderMark !== undefined && deps.runCheck !== undefined && deps.folderBusy?.() !== true) {
      const mark = await deps.folderMark().catch(() => undefined)
      if (mark !== undefined && mark !== from.folderMark) {
        const result = await deps.runCheck().catch(() => undefined)
        if (result !== undefined) {
          facts.push(...checkFacts(from.lastCheck, result, mark))
          marks = { folderMark: mark, lastCheck: { passed: result.passed, command: result.command } }
        }
      }
    }
    if (from.settings.watches.github) {
      const github = await deps.github().catch(() => undefined)
      if (github !== undefined) {
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
    return { facts: facts.filter((fact, index) => !seen.has(fact.key) && facts.findIndex((other) => other.key === fact.key) === index), prs, marks }
  }

  /** The morning brief's fact (0.816): who waits, what ran since the last brief, the check, pull requests, the cards. */
  const briefOf = async (from: Kept, news: readonly ScoutFact[], prs: readonly FolderPullRequest[], marks: Pick<Kept, 'folderMark' | 'lastCheck'>, at: Date): Promise<ScoutFact> => {
    const since = from.lastBriefAt ?? new Date(at.getTime() - 24 * 60 * 60_000).toISOString()
    const ran = await deps.briefFacts?.(since, from.lookIds).catch(() => undefined)
    const waiting = from.settings.watches.teammates ? await deps.waiting().catch(() => []) : []
    // The waits are told on their own line: a card for one is not counted again among the cards.
    const counts = (card: Pick<ScoutFact, 'key'>): boolean => !card.key.startsWith('brief:') && !card.key.startsWith('waiting:')
    const others = from.cards.filter(counts)
    const told = news.filter(counts)
    const check = marks.lastCheck
    const needsWork = (pr: FolderPullRequest): boolean => pr.checks === 'failing' || pr.review === 'changes-requested' || pr.conflicts === true
    const open = prs.filter((pr) => pr.state === 'open')
    return briefFact({
      waiting: waiting.map((card) => ({ ...(card.name === undefined ? {} : { name: card.name }), minutes: Math.max(0, Math.round((at.getTime() - Date.parse(card.requestedAt)) / 60_000)) })),
      finished: ran?.finished ?? 0,
      failed: ran?.failed ?? 0,
      ...(from.settings.watches.check && check?.command !== undefined ? { check: { command: check.command, passed: check.passed } } : {}),
      ...(from.settings.watches.github && open.length > 0 ? { prs: { open: open.length, needsWork: open.filter(needsWork).length } } : {}),
      cards: { total: others.length + told.length, now: [...others, ...told].filter((one) => one.urgency === 'now').length }
    }, scoutDay(at))
  }

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
      const { facts: found, prs, marks } = await gather(from, stamp)
      const facts = [...found]
      /*
       * THE MORNING BRIEF (0.816): the first look from its hour on, once a day and never in quiet hours -- so a Locust
       * opened at ten brings the seven o'clock brief with its first look. It comes after the look's own news, which it
       * counts among the cards waiting.
       */
      const day = scoutDay(at)
      const briefDue = from.settings.briefAt !== undefined && at.getHours() >= from.settings.briefAt && from.lastBriefDay !== day && !inQuietHours(from.settings.quiet, at)
      if (briefDue) facts.push(await briefOf(from, facts, prs, marks, at))
      const briefed = briefDue ? { lastBriefDay: day, lastBriefAt: stamp } : {}
      if (facts.length === 0) return await mutate((latest) => ({ ...latest, ...marks, prs, lastLookAt: stamp, today: counted(latest.today, at, 'looks') }))
      const team = (await deps.team().catch(() => [])).filter((mate) => mate.teammateId !== teammateId)
      const cards = facts.map((fact) => cardOf(fact, `card_${randomBytes(6).toString('hex')}`, at, true))
      // A check that turned says how it stands now: the card of how it stood before goes (0.815); so does yesterday's brief.
      const checkTurned = facts.some((fact) => fact.key.startsWith('check:'))
      const outdated = (card: ScoutCard): boolean => (checkTurned && card.key.startsWith('check:')) || (briefDue && card.key.startsWith('brief:'))
      let state = await mutate((latest) => ({
        ...latest,
        ...marks,
        ...briefed,
        cards: withNewCards(latest.cards.filter((card) => !outdated(card)), cards),
        seen: [...latest.seen, ...facts.map((fact) => fact.key)].slice(-MAX_SCOUT_SEEN),
        prs,
        lastLookAt: stamp,
        today: counted(latest.today, at, 'looks')
      }))
      // Nothing in Windows in quiet hours, even from a look asked for: the cards wait on Home (0.815).
      if (state.settings.notify && !inQuietHours(state.settings.quiet, at)) for (const card of cards) if (card.urgency === 'now') deps.notify(card)
      // Then Scout's model, for its take and the next step.
      const prompt = scoutPrompt({
        facts,
        team: team.map((mate) => ({ name: mate.name, role: mate.role })),
        ...(state.settings.goal === undefined ? {} : { goal: state.settings.goal }),
        ...(kept === undefined || kept.notes.length === 0 ? {} : { notes: kept.notes })
      })
      state = await askFor({ teammateId, prompt, cardIds: cards.map((card) => card.cardId), team, tried: [] }) ?? state
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
    const expires = later(() => { void gaveUp(asked.missionId, 'Scout’s model took too long to answer.') }, deps.answerWaitMs ?? ANSWER_WAIT_MS)
    pending.set(asked.missionId, { ...look, runId: asked.runId, expires })
    return mutate((latest) => ({ ...latest, lookIds: [...latest.lookIds, asked.missionId].slice(-MAX_LOOK_IDS), today: counted(latest.today, now(), 'asked') }))
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
    const next = look.tried.length >= MAX_FALLBACKS ? undefined : await deps.nextFreeModel(look.teammateId, look.tried).catch(() => undefined)
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
        return { ...rest, ...(answer?.take === undefined ? {} : { take: answer.take }), ...(answer?.draft === undefined ? {} : { draft: answer.draft }) }
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

  return {
    state: async (): Promise<ScoutState> => stateOf(await load()),
    /** Picks up where the last launch left off: on, it looks again shortly and on its timer after. */
    async start(firstLookMs = 30_000): Promise<void> {
      const from = await load()
      // A look's cards from before a restart wait on nothing now.
      if (from.cards.some((card) => card.reading === true)) await mutate((latest) => ({ ...latest, cards: latest.cards.map(({ reading: _reading, ...card }) => card) }))
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
      const state = await mutate((latest) => ({ ...latest, settings: { ...latest.settings, on: true, teammateId: madeId } }))
      restart(state.settings)
      return look()
    },
    async set(change: unknown): Promise<ScoutState> {
      let before: ScoutSettings | undefined
      const state = await mutate((latest) => {
        before = latest.settings
        // The window may not name Scout's teammate: Scout made it, and keeps it.
        return { ...latest, settings: scoutSettingsFrom({ ...(typeof change === 'object' && change !== null ? change : {}), teammateId: latest.settings.teammateId }, latest.settings) }
      })
      if (state.settings.everyMinutes !== before?.everyMinutes || state.settings.on !== before.on) restart(state.settings)
      if (before?.on === true && !state.settings.on) {
        await stopLooks('Scout was turned off, so these cards stay in Locust’s words.')
        return stateOf(await load())
      }
      return state
    },
    dismiss: (cardId: unknown): Promise<ScoutState> => mutate((latest) => ({ ...latest, cards: latest.cards.filter((card) => card.cardId !== cardId) })),
    /** The notes as the person wrote them in Settings (0.816): what they keep is what the next look reads. */
    saveNotes: (text: unknown): Promise<ScoutState> => mutate((latest) => ({ ...latest, notes: notesFrom(typeof text === 'string' ? text : '') })),
    look,
    /** A look's turn whose provider will not answer while the runtime keeps trying: stopped, and the next model tried. */
    async providerRefused(missionId: string, why: string): Promise<void> {
      if (pending.has(missionId)) await gaveUp(missionId, why)
    },
    /** Every run that ends: a look's turn fills its cards in; a teammate's failed turn is news for the next look. */
    async onRunEnded(mission: { readonly missionId: string }): Promise<void> {
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
