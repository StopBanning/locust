/**
 * WHEN YOUR PHONE BUZZES (0.827, the spec "Scout comes to you first", release 1). Windows hears a run waiting on you
 * while the window is behind (attention.ts); a phone away from the desk hears it too, by these rules, taken from the
 * Duet design Colin shared on 10/23 ("Build Your Own Duet": only what can't wait interrupts, on a budget):
 *
 *   - only while Locust's window is behind another app or minimized (a phone page that is open shows everything
 *     itself: phone-anywhere.ts sends to no phone that is connected);
 *   - at most one buzz every MERGE_MS: what lands in between joins the next one, the most pressing first;
 *   - approvals and questions always count as pressing; anything else, past DAILY_BUDGET a day, waits for the brief;
 *   - quiet hours hold every buzz for the brief, approvals too unless the person lets them through at night;
 *   - a Scout card that can wait a while, left untouched for ESCALATE_MS, buzzes once.
 *
 * What a lock screen shows is who and what kind, never a command, a file's name or a message's words. The decision is
 * pure, so it is tested where it is written; sending and the clock are handed in.
 */

export type AlertKind = 'approval' | 'question' | 'limit' | 'scout'

export interface PhoneAlert {
  readonly kind: AlertKind
  readonly title: string
  readonly body: string
  /** Notifications with one tag replace each other on the phone. */
  readonly tag: string
}

/** What the alert said, kept for the morning brief when it was held (spec release 2). */
export interface HeldAlert {
  readonly kind: AlertKind
  readonly title: string
  readonly at: number
  readonly why: 'quiet hours' | 'daily limit'
  /** The Scout card it was about, by its key, for What Scout saw (0.828). Never sent to a phone. */
  readonly about?: string
}

export const MERGE_MS = 120_000
export const DAILY_BUDGET = 5
export const ESCALATE_MS = 4 * 60 * 60_000
const HELD_KEPT = 50

/** The first rank is said first when buzzes are merged. */
const RANK: Readonly<Record<AlertKind, number>> = { approval: 0, question: 1, limit: 2, scout: 3 }
const pressing = (kind: AlertKind): boolean => kind === 'approval' || kind === 'question'

/** A Scout card as the phone may name it: by the watch it came from, never by what the card says. */
export function scoutAlertBody(key: string): string {
  const kind = key.slice(0, Math.max(0, key.indexOf(':')))
  switch (kind) {
    case 'pr': return 'One of your pull requests needs you.'
    case 'review': return 'A review is asked of you.'
    case 'issue': return 'There is a new issue.'
    case 'branch': return 'A check failed on a branch.'
    case 'check': return 'The folder’s check fails.'
    case 'comment':
    case 'comment-answer': return 'Someone asked Locust on GitHub.'
    case 'waiting': return 'A teammate is waiting on you.'
    case 'failed': return 'A teammate’s turn failed.'
    default: return 'Scout found something for you.'
  }
}

export function approvalAlert(name: string | undefined, kind: 'approval' | 'question'): PhoneAlert {
  const who = name ?? 'A teammate'
  return kind === 'question'
    ? { kind, title: `${who} is asking you something`, body: 'The turn waits for your answer.', tag: 'waiting' }
    : { kind, title: `${who} needs your approval`, body: 'The turn is paused until you answer.', tag: 'waiting' }
}

export function limitAlert(name: string | undefined, runtimeName: string): PhoneAlert {
  return { kind: 'limit', title: `${name ?? 'A teammate'} hit the ${runtimeName} limit`, body: 'Pick where the work carries on.', tag: 'limit' }
}

export function scoutAlert(key: string): PhoneAlert {
  return { kind: 'scout', title: 'Scout', body: scoutAlertBody(key), tag: `scout:${key.slice(0, 40)}` }
}

/** Several alerts as one buzz: the most pressing says itself, and the rest are counted. */
export function mergedAlert(alerts: readonly PhoneAlert[]): PhoneAlert {
  const sorted = [...alerts].sort((a, b) => RANK[a.kind] - RANK[b.kind])
  const first = sorted[0]!
  if (sorted.length === 1) return first
  const more = sorted.length - 1
  return { ...first, body: `${first.body} And ${String(more)} more thing${more === 1 ? '' : 's'} for you in Locust.`, tag: 'locust' }
}

export interface PhoneAlertsOptions {
  /** Hands one buzz to the phones (phone-anywhere.ts). */
  readonly send: (alert: PhoneAlert) => void
  /** Whether the person is away from Locust's window: it is behind another app, or minimized. */
  readonly away: () => boolean
  /** Whether `at` is in the person's quiet hours. */
  readonly quiet: (at: Date) => boolean
  /** Whether approvals and questions may buzz in quiet hours. */
  readonly approvalsAtNight?: () => boolean
  /** Whether a Scout card is still waiting, untouched: neither started nor dismissed. */
  readonly cardOpen?: (cardId: string) => Promise<boolean>
  readonly now?: () => number
  readonly schedule?: (task: () => void, ms: number) => unknown
  readonly clear?: (handle: unknown) => void
}

export function createPhoneAlerts(options: PhoneAlertsOptions) {
  const now = options.now ?? Date.now
  const schedule = options.schedule ?? ((task: () => void, ms: number) => setTimeout(task, ms))
  const clear = options.clear ?? ((handle: unknown) => clearTimeout(handle as NodeJS.Timeout))
  let lastSent = -Infinity
  let waiting: PhoneAlert[] = []
  let flushAt: unknown
  let spent = { day: '', count: 0 }
  const held: HeldAlert[] = []
  const escalations = new Map<string, unknown>()

  const dayOf = (at: number): string => new Date(at).toDateString()
  const hold = (alert: PhoneAlert, why: HeldAlert['why'], about?: string): void => {
    held.push({ kind: alert.kind, title: alert.title, at: now(), why, ...(about === undefined ? {} : { about }) })
    if (held.length > HELD_KEPT) held.splice(0, held.length - HELD_KEPT)
  }
  const flush = (): void => {
    flushAt = undefined
    const said = waiting
    waiting = []
    if (said.length === 0) return
    lastSent = now()
    options.send(mergedAlert(said))
  }

  /** The one door every alert goes through. */
  const raise = (alert: PhoneAlert, about?: string): void => {
    if (!options.away()) return
    const at = now()
    if (options.quiet(new Date(at)) && !(pressing(alert.kind) && options.approvalsAtNight?.() === true)) { hold(alert, 'quiet hours', about); return }
    if (!pressing(alert.kind)) {
      if (spent.day !== dayOf(at)) spent = { day: dayOf(at), count: 0 }
      if (spent.count >= DAILY_BUDGET) { hold(alert, 'daily limit', about); return }
      spent = { ...spent, count: spent.count + 1 }
    }
    waiting.push(alert)
    if (flushAt !== undefined) return
    const wait = lastSent + MERGE_MS - at
    if (wait <= 0) flush()
    else flushAt = schedule(flush, wait)
  }

  return {
    approval: (name: string | undefined, kind: 'approval' | 'question'): void => raise(approvalAlert(name, kind)),
    limit: (name: string | undefined, runtimeName: string): void => raise(limitAlert(name, runtimeName)),
    /** New Scout cards: one that can't wait buzzes now; one that can wait a while buzzes once if it is left untouched. */
    scoutCards: (cards: readonly { readonly cardId: string; readonly key: string; readonly urgency: 'now' | 'soon' | 'later' }[]): void => {
      for (const card of cards) {
        if (card.urgency === 'now') raise(scoutAlert(card.key), card.key)
        else if (card.urgency === 'soon' && options.cardOpen !== undefined && !escalations.has(card.cardId)) {
          escalations.set(card.cardId, schedule(() => {
            escalations.delete(card.cardId)
            void options.cardOpen?.(card.cardId).then((open) => { if (open) raise(scoutAlert(card.key), card.key) }, () => undefined)
          }, ESCALATE_MS))
        }
      }
    },
    /** What was held since `since`, for the morning brief. */
    heldSince: (since: number): readonly HeldAlert[] => held.filter((alert) => alert.at >= since),
    dispose: (): void => {
      if (flushAt !== undefined) clear(flushAt)
      for (const handle of escalations.values()) clear(handle)
      escalations.clear()
    }
  }
}
