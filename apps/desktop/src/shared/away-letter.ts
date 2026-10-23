import type { FolderPullRequest } from './pull-request.js'
import type { ScoutFact, ScoutTone, ScoutUrgency } from './scout.js'

/**
 * SCOUT'S AWAY LETTERS (0.829). While the PC is off, a workflow in the repository Scout watches does Scout's GitHub
 * look and leaves what it found at Locust's relay as a letter; the PC leaves one too, saying what it already told the
 * person, so the next away look does not tell it again. A letter is sealed with the away key (AES-256-GCM), which
 * Locust made and keeps, gave the person's phones over their sealed line, and put in the repository's secret: the
 * relay passes letters it cannot open.
 *
 * Written against the Web Crypto every side has: Locust's main process, the workflow's Node and (in plain script, in
 * phone-page.ts) the phone. Whoever can write to the repository could read its secret and leave a letter of its own,
 * so what a letter says is read as GitHub's news only: a finding's link must be to that repository on GitHub.
 */

export const AWAY_LETTER_VERSION = 1

/** One thing an away look found: a Scout fact, cut to what a card shows. */
export interface AwayFinding {
  readonly key: string
  readonly urgency: ScoutUrgency
  readonly said: string
  readonly detail?: string
  readonly lines?: readonly string[]
  readonly chips?: readonly { readonly text: string; readonly tone: ScoutTone }[]
  readonly url?: string
}

export interface AwayLetter {
  readonly v: typeof AWAY_LETTER_VERSION
  /** A look by the workflow, or the PC saying what it told. */
  readonly kind: 'look' | 'told'
  /** When it looked, as an ISO time. */
  readonly at: string
  /** owner/name. */
  readonly repo: string
  /** Whether GitHub answered the look: a look that read nothing finds nothing. */
  readonly read: boolean
  /** What was new in this look. */
  readonly found: readonly AwayFinding[]
  /** The person's open pull requests as they stood, for the next look to compare with. */
  readonly prs: readonly FolderPullRequest[]
  /** What has been told already, by either side: the next look tells none of it again. */
  readonly told: readonly string[]
  /** The day ("2026-10-23", the person's own) the last morning brief came. */
  readonly briefDay?: string
}

export const MAX_TOLD = 500
export const MAX_FOUND = 40
const MAX_SAID = 400
const REPO = /^[\w.-]{1,100}\/[\w.-]{1,100}$/
const TONES: readonly ScoutTone[] = ['red', 'amber', 'green', 'blue', 'quiet']
const URGENCIES: readonly ScoutUrgency[] = ['now', 'soon', 'later']

const b64 = (bytes: Uint8Array): string => {
  let text = ''
  for (const byte of bytes) text += String.fromCharCode(byte)
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}
const unb64 = (text: string): Uint8Array<ArrayBuffer> => Uint8Array.from(atob(text.replaceAll('-', '+').replaceAll('_', '/')), (character) => character.charCodeAt(0))

/** What a letter is bound to: the room, and who left it, so a letter is never taken for one from the other side. */
export const awayAad = (room: string, from: 'away' | 'pc'): Uint8Array<ArrayBuffer> => new TextEncoder().encode(`locust-away/v1|${room}|${from}`) as Uint8Array<ArrayBuffer>

/** A new away key: 32 random bytes, base64url. */
export const newAwayKey = (): string => b64(crypto.getRandomValues(new Uint8Array(32)))

const keyOf = (key: string) => crypto.subtle.importKey('raw', unb64(key), 'AES-GCM', false, ['encrypt', 'decrypt'])

export async function sealAwayLetter(key: string, letter: AwayLetter, room: string, from: 'away' | 'pc'): Promise<{ readonly n: string; readonly c: string }> {
  const nonce = crypto.getRandomValues(new Uint8Array(12))
  const sealed = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce, additionalData: awayAad(room, from) }, await keyOf(key), new TextEncoder().encode(JSON.stringify(letter)) as Uint8Array<ArrayBuffer>)
  return { n: b64(nonce), c: b64(new Uint8Array(sealed)) }
}

/** A letter opened and read as one, or undefined: one that does not open with this key, or is not a letter, is nobody's. */
export async function openAwayLetter(key: string, box: { readonly n: string; readonly c: string }, room: string, from: 'away' | 'pc'): Promise<AwayLetter | undefined> {
  let text: string
  try {
    const opened = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(box.n), additionalData: awayAad(room, from) }, await keyOf(key), unb64(box.c))
    text = new TextDecoder().decode(opened)
  } catch {
    return undefined
  }
  let value: unknown
  try { value = JSON.parse(text) } catch { return undefined }
  return awayLetterFrom(value, from)
}

const clip = (text: string, most: number): string => (text.length > most ? `${text.slice(0, most - 1)}…` : text)

/** A link a finding may carry: to the letter's own repository on GitHub, and nowhere else. */
export function repoLink(repo: string, url: unknown): string | undefined {
  if (typeof url !== 'string' || url.length > 300) return undefined
  const prefix = `https://github.com/${repo}/`
  return url.toLowerCase().startsWith(prefix.toLowerCase()) && /^https:\/\/github\.com\/[\w./#?=&%-]+$/.test(url) ? url : undefined
}

function findingFrom(value: unknown, repo: string): AwayFinding | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const fields = value as Record<string, unknown>
  if (typeof fields.key !== 'string' || fields.key.length === 0 || fields.key.length > 300) return undefined
  if (typeof fields.urgency !== 'string' || !URGENCIES.includes(fields.urgency as ScoutUrgency)) return undefined
  if (typeof fields.said !== 'string' || fields.said.trim() === '') return undefined
  const lines = Array.isArray(fields.lines) ? fields.lines.filter((line): line is string => typeof line === 'string').slice(0, 8).map((line) => clip(line, 200)) : []
  const chips = Array.isArray(fields.chips)
    ? fields.chips.flatMap((chip) => {
      if (typeof chip !== 'object' || chip === null) return []
      const { text, tone } = chip as Record<string, unknown>
      return typeof text === 'string' && text !== '' && typeof tone === 'string' && TONES.includes(tone as ScoutTone) ? [{ text: clip(text, 40), tone: tone as ScoutTone }] : []
    }).slice(0, 4)
    : []
  const url = repoLink(repo, fields.url)
  return {
    key: fields.key,
    urgency: fields.urgency as ScoutUrgency,
    said: clip(fields.said, MAX_SAID),
    ...(typeof fields.detail === 'string' && fields.detail !== '' ? { detail: clip(fields.detail, MAX_SAID) } : {}),
    ...(lines.length > 0 ? { lines } : {}),
    ...(chips.length > 0 ? { chips } : {}),
    ...(url === undefined ? {} : { url })
  }
}

function pullRequestFrom(value: unknown, repo: string): FolderPullRequest | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const pr = value as Record<string, unknown>
  const url = repoLink(repo, pr.url)
  if (typeof pr.number !== 'number' || !Number.isInteger(pr.number) || pr.number <= 0 || url === undefined || typeof pr.title !== 'string') return undefined
  if (!['open', 'draft', 'merged', 'closed'].includes(String(pr.state)) || !['passing', 'failing', 'pending', 'none'].includes(String(pr.checks))) return undefined
  const count = (field: unknown): number => (typeof field === 'number' && Number.isInteger(field) && field >= 0 ? field : 0)
  return {
    number: pr.number,
    title: clip(pr.title, 300),
    url,
    state: pr.state as FolderPullRequest['state'],
    checks: pr.checks as FolderPullRequest['checks'],
    checkCount: count(pr.checkCount),
    notPassed: count(pr.notPassed),
    ...(pr.review === 'approved' || pr.review === 'changes-requested' || pr.review === 'review-required' ? { review: pr.review } : {}),
    ...(pr.conflicts === true ? { conflicts: true as const } : {}),
    ...(typeof pr.updatedAt === 'string' && !Number.isNaN(Date.parse(pr.updatedAt)) ? { updatedAt: pr.updatedAt } : {})
  }
}

export function awayLetterFrom(value: unknown, from: 'away' | 'pc'): AwayLetter | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields = value as Record<string, unknown>
  if (fields.v !== AWAY_LETTER_VERSION || (fields.kind !== 'look' && fields.kind !== 'told')) return undefined
  // The PC only ever says what it told; only the away look ever looks.
  if ((fields.kind === 'told') !== (from === 'pc')) return undefined
  if (typeof fields.at !== 'string' || Number.isNaN(Date.parse(fields.at)) || typeof fields.repo !== 'string' || !REPO.test(fields.repo)) return undefined
  const repo = fields.repo
  const found = Array.isArray(fields.found) ? fields.found.slice(0, MAX_FOUND).map((one) => findingFrom(one, repo)).filter((one): one is AwayFinding => one !== undefined) : []
  const prs = Array.isArray(fields.prs) ? fields.prs.slice(0, 50).map((one) => pullRequestFrom(one, repo)).filter((one): one is FolderPullRequest => one !== undefined) : []
  const told = Array.isArray(fields.told) ? fields.told.filter((key): key is string => typeof key === 'string' && key.length > 0 && key.length <= 300).slice(-MAX_TOLD) : []
  return {
    v: AWAY_LETTER_VERSION,
    kind: fields.kind,
    at: fields.at,
    repo,
    read: fields.read === true,
    found,
    prs,
    told,
    ...(typeof fields.briefDay === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(fields.briefDay) ? { briefDay: fields.briefDay } : {})
  }
}

/** A Scout fact as an away letter carries it. */
export function findingOf(fact: ScoutFact): AwayFinding {
  const url = fact.open?.kind === 'url' ? fact.open.url : undefined
  return {
    key: fact.key,
    urgency: fact.urgency,
    said: fact.said,
    ...(fact.detail === undefined ? {} : { detail: fact.detail }),
    ...(fact.lines === undefined ? {} : { lines: fact.lines }),
    ...(fact.chips === undefined ? {} : { chips: fact.chips }),
    ...(url === undefined ? {} : { url })
  }
}

/** And back: an away finding as the fact a card is made from on the PC. */
export function factOf(finding: AwayFinding): ScoutFact {
  return {
    key: finding.key,
    urgency: finding.urgency,
    said: finding.said,
    ...(finding.detail === undefined ? {} : { detail: finding.detail }),
    ...(finding.lines === undefined ? {} : { lines: finding.lines }),
    ...(finding.chips === undefined ? {} : { chips: finding.chips }),
    ...(finding.url === undefined ? {} : { open: { kind: 'url' as const, url: finding.url, label: 'Open on GitHub' } })
  }
}
