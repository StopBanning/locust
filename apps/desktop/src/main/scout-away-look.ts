import { findingOf, MAX_FOUND, MAX_TOLD, openAwayLetter, sealAwayLetter, type AwayLetter } from '../shared/away-letter.js'
import type { FolderPullRequest } from '../shared/pull-request.js'
import { branchFacts, briefFact, newIssues, pullRequestFacts, reviewFacts, type ScoutFact } from '../shared/scout.js'
import { readScoutGithub } from './scout-github.js'

export { AWAY_WORKFLOW_NAME } from './scout-github.js'

/**
 * SCOUT'S AWAY LOOK (0.829, "Away from your desk", step 2). While the person's PC is off, a scheduled workflow in the
 * repository Scout watches runs this, built into one file (resources/scout-away.mjs) that Locust's pull request puts
 * in that repository, so the person can read what runs. It does Scout's own GitHub look -- the same reads and the same
 * facts as the PC's (scout-github.ts, shared/scout.ts) -- with the workflow's read-only token, for the person's login.
 *
 * It asks the relay first: with Locust open on the PC, the PC looks itself and this does nothing more. Otherwise it
 * opens the newest letter, from the last away look or from the PC, to know what the person was told already; tells
 * only what is new; brings the morning brief once a day from its hour on; and leaves its letter, sealed, asking the
 * relay to ring the phones for something that can't wait. Never in quiet hours. Nothing it prints names a pull
 * request, an issue or a person: a workflow's log is read by whoever can read the repository.
 */

/** What Locust put in the repository's LOCUST_SCOUT secret when the person turned the away look on. */
export interface AwayConfig {
  readonly v: 1
  readonly relay: string
  readonly room: string
  readonly token: string
  readonly key: string
  /** The person's GitHub login: whose pull requests, and whose reviews. */
  readonly login: string
  /** Their time zone, for quiet hours and the brief's hour. */
  readonly tz: string
  readonly quiet?: { readonly from: number; readonly to: number }
  readonly briefAt?: number
}

export function awayConfigOf(text: string | undefined): AwayConfig | undefined {
  if (text === undefined || text.trim() === '') return undefined
  let value: unknown
  try { value = JSON.parse(text) } catch { return undefined }
  if (typeof value !== 'object' || value === null) return undefined
  const fields = value as Record<string, unknown>
  const hour = (field: unknown): field is number => typeof field === 'number' && Number.isInteger(field) && field >= 0 && field <= 23
  if (fields.v !== 1 || typeof fields.relay !== 'string' || !/^https?:\/\/[^\s/]+$/.test(fields.relay)) return undefined
  if (typeof fields.room !== 'string' || !/^[A-Za-z0-9_-]{22}$/.test(fields.room) || typeof fields.token !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(fields.token)) return undefined
  if (typeof fields.key !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(fields.key) || typeof fields.login !== 'string' || !/^[A-Za-z0-9-]{1,39}$/.test(fields.login)) return undefined
  if (typeof fields.tz !== 'string' || fields.tz.length > 64) return undefined
  const quiet = typeof fields.quiet === 'object' && fields.quiet !== null ? fields.quiet as Record<string, unknown> : undefined
  return {
    v: 1, relay: fields.relay, room: fields.room, token: fields.token, key: fields.key, login: fields.login, tz: fields.tz,
    ...(quiet !== undefined && hour(quiet.from) && hour(quiet.to) ? { quiet: { from: quiet.from, to: quiet.to } } : {}),
    ...(hour(fields.briefAt) ? { briefAt: fields.briefAt } : {})
  }
}

/** The person's own day and hour at `at`, in their time zone (the runner's clock is UTC). */
export function localTime(at: Date, tz: string): { readonly day: string; readonly hour: number } {
  let parts: Intl.DateTimeFormatPart[]
  try {
    parts = new Intl.DateTimeFormat('en-CA', { timeZone: tz, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', hourCycle: 'h23' }).formatToParts(at)
  } catch {
    parts = new Intl.DateTimeFormat('en-CA', { timeZone: 'UTC', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', hourCycle: 'h23' }).formatToParts(at)
  }
  const part = (type: string): string => parts.find((one) => one.type === type)?.value ?? '0'
  return { day: `${part('year')}-${part('month')}-${part('day')}`, hour: Number(part('hour')) % 24 }
}

const quietAt = (quiet: AwayConfig['quiet'], hour: number): boolean =>
  quiet !== undefined && quiet.from !== quiet.to && (quiet.from < quiet.to ? hour >= quiet.from && hour < quiet.to : hour >= quiet.from || hour < quiet.to)

export interface AwayLookDeps {
  readonly config: AwayConfig
  /** owner/name. */
  readonly repo: string
  readonly gh: (args: readonly string[]) => Promise<string>
  readonly fetch: typeof fetch
  readonly now?: () => Date
}

/** What a run says in the workflow's log, and whether it failed. */
export interface AwayLookResult { readonly said: string; readonly ok: boolean }

interface Box { readonly n: string; readonly c: string; readonly from?: string }

export async function awayLook(deps: AwayLookDeps): Promise<AwayLookResult> {
  const { config } = deps
  const at = (deps.now ?? (() => new Date()))()
  const base = `${config.relay}/v1/room/${config.room}/away`
  const headers = { authorization: `Bearer ${config.token}`, 'content-type': 'application/json' }
  let asked: Response
  try { asked = await deps.fetch(base, { method: 'GET', headers }) } catch { return { said: 'Locust’s relay could not be reached. The next look tries again.', ok: false } }
  if (asked.status === 403) return { said: 'Scout’s away look is off in Locust. Delete .github/workflows/locust-scout.yml and the LOCUST_SCOUT secret to stop these runs.', ok: true }
  if (asked.status === 401 || asked.status === 404) return { said: 'Locust’s relay did not know this look’s token. Turn “Look while my computer is off” off and on again in Locust.', ok: false }
  if (!asked.ok) return { said: `Locust’s relay answered ${String(asked.status)}. The next look tries again.`, ok: false }
  const answer = await asked.json().catch(() => ({})) as { pc?: unknown; latest?: Box }
  if (answer.pc === true) return { said: 'Locust is open on your computer, so Scout looks from there.', ok: true }

  const from = answer.latest?.from === 'pc' ? 'pc' : 'away'
  const before: AwayLetter | undefined = answer.latest === undefined ? undefined : await openAwayLetter(config.key, answer.latest, config.room, from)
  const told = new Set(before?.told ?? [])
  const local = localTime(at, config.tz)
  const quiet = quietAt(config.quiet, local.hour)

  const github = await readScoutGithub('.', (args) => deps.gh(args), config.login).catch(() => undefined)
  const stamp = at.toISOString()
  let found: ScoutFact[] = []
  let prs: readonly FolderPullRequest[] = before?.prs ?? []
  if (github !== undefined) {
    const facts = [
      ...pullRequestFacts(new Map(prs.map((pr) => [pr.url, pr] as const)), github.prs, stamp, at),
      ...reviewFacts(github.reviews),
      ...(github.branch === undefined ? [] : branchFacts(github.branch.name, github.branch.runs)),
      ...newIssues(github.issues ?? [], at, told).facts
    ]
    found = facts.filter((fact, index) => !told.has(fact.key) && facts.findIndex((other) => other.key === fact.key) === index).slice(0, MAX_FOUND)
    prs = github.prs
  }
  // With no letter to go by (the first look, or a key changed since), what is there now is taken as known: told
  // all at once, a repository's open work would ring the phone for things the person has seen for weeks.
  const first = before === undefined
  if (first) {
    for (const fact of found) told.add(fact.key)
    found = []
  }

  // The morning brief, from its hour on, once a day and never in quiet hours: from GitHub only, the PC being off.
  const briefDue = config.briefAt !== undefined && local.hour >= config.briefAt && before?.briefDay !== local.day && !quiet
  if (briefDue) {
    const needsWork = (pr: FolderPullRequest): boolean => pr.checks === 'failing' || pr.review === 'changes-requested' || pr.conflicts === true
    const open = prs.filter((pr) => pr.state === 'open')
    const brief = briefFact({
      waiting: [], finished: 0, failed: 0,
      ...(github !== undefined && open.length > 0 ? { prs: { open: open.length, needsWork: open.filter(needsWork).length } } : {}),
      cards: { total: found.length, now: found.filter((fact) => fact.urgency === 'now').length },
      ...(github === undefined ? { githubMissed: { missed: 1, looks: 1 } } : found.length === 0 && !open.some(needsWork) ? { quiet: ['GitHub'] } : {})
    }, local.day)
    const lines = [...(brief.lines ?? []), 'Your computer is off, so Scout looked at GitHub only.']
    found = [...found, { ...brief, said: brief.said.replace(/, and nothing ran since yesterday\./, '.'), lines, detail: lines.join('\n') }]
  }

  const letter: AwayLetter = {
    v: 1,
    kind: 'look',
    at: stamp,
    repo: deps.repo,
    read: github !== undefined,
    found: found.map(findingOf),
    prs,
    told: [...told, ...found.map((fact) => fact.key)].slice(-MAX_TOLD),
    ...(briefDue ? { briefDay: local.day } : before?.briefDay === undefined ? {} : { briefDay: before.briefDay })
  }
  const pressing = found.some((fact) => fact.urgency === 'now')
  const alert = quiet ? undefined : briefDue ? 'brief' : pressing ? 'found' : undefined
  const box = await sealAwayLetter(config.key, letter, config.room, 'away')
  let posted: Response
  try {
    posted = await deps.fetch(base, { method: 'POST', headers, body: JSON.stringify({ ...box, ...(alert === undefined ? {} : { alert }) }) })
  } catch {
    return { said: 'Scout looked, but Locust’s relay could not be reached to leave what it found.', ok: false }
  }
  if (!posted.ok) return { said: `Scout looked, but Locust’s relay answered ${String(posted.status)}.`, ok: false }
  const rung = Number(((await posted.json().catch(() => ({}))) as { rung?: unknown }).rung ?? 0)
  const news = found.filter((fact) => !fact.key.startsWith('brief:')).length
  return {
    said: [
      github === undefined ? 'Scout could not read GitHub this time.' : first ? 'Scout’s first away look: it noted what is open now, and tells only what changes.' : news === 0 ? 'Scout looked: nothing new.' : `Scout looked: ${String(news)} new thing${news === 1 ? '' : 's'}.`,
      ...(briefDue ? ['It brought the morning brief.'] : []),
      ...(rung > 0 ? [`It rang ${String(rung)} phone${rung === 1 ? '' : 's'}.`] : quiet && (pressing || briefDue) ? ['Quiet hours: no phone was rung.'] : [])
    ].join(' '),
    ok: true
  }
}
