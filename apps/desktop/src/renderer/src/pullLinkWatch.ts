import type { LinkedIssue, LinkedPullRequest, PullLinkAddress } from '../../shared/pull-request.js'

/**
 * When a shown link asks, and when it stops.
 *
 * One pull request or issue is read once, kept for a minute, and read again
 * when the window comes back after that. A pull request is also read every few
 * minutes while its checks are still running; an issue has no checks, so it is
 * never polled (0.816). A link that is not on screen has no listener, so it
 * has no timer and is not read.
 */
const MINUTE_MS = 60_000
const AGAIN_MS = 3 * 60_000

interface Slot<T> {
  readonly address: PullLinkAddress
  value: T | undefined
  at: number
  pending: Promise<void> | undefined
  readonly listeners: Set<() => void>
}

interface LinkWatch<T> {
  current(address: PullLinkAddress): T | undefined
  watch(address: PullLinkAddress, listener: () => void): () => void
}

function createLinkWatch<T>(options: {
  readonly read: (address: PullLinkAddress) => Promise<T | undefined>
  /** Whether a value shown is still moving, and is read again every `againMs` while it is. */
  readonly moving?: (value: T) => boolean
  readonly now?: () => number
  readonly minuteMs?: number
  readonly againMs?: number
  readonly every?: (ms: number, fn: () => void) => () => void
  readonly onFocus?: (fn: () => void) => () => void
}): LinkWatch<T> {
  const now = options.now ?? (() => Date.now())
  const minuteMs = options.minuteMs ?? MINUTE_MS
  const againMs = options.againMs ?? AGAIN_MS
  const slots = new Map<string, Slot<T>>()
  let stopFocus: (() => void) | undefined
  let stopTimer: (() => void) | undefined

  const keyOf = (address: PullLinkAddress): string => `${address.owner}/${address.repo}#${String(address.number)}`
  const moving = (slot: Slot<T>): boolean => slot.listeners.size > 0 && slot.value !== undefined && options.moving?.(slot.value) === true

  const anyListeners = (): boolean => {
    for (const slot of slots.values()) if (slot.listeners.size > 0) return true
    return false
  }

  const watchingMoving = (): boolean => {
    for (const slot of slots.values()) if (moving(slot)) return true
    return false
  }

  const notify = (slot: Slot<T>): void => {
    for (const listener of [...slot.listeners]) listener()
  }

  const load = (slot: Slot<T>, force: boolean): void => {
    if (slot.pending !== undefined) return
    if (!force && slot.at > 0 && now() - slot.at < minuteMs) return
    slot.pending = options.read(slot.address).then(
      (value) => {
        slot.value = value
        slot.at = now()
        slot.pending = undefined
        notify(slot)
        ensureTimer()
      },
      () => {
        slot.value = undefined
        slot.at = now()
        slot.pending = undefined
        notify(slot)
        ensureTimer()
      }
    )
  }

  const tick = (): void => {
    for (const slot of slots.values()) if (moving(slot)) load(slot, true)
  }

  const ensureTimer = (): void => {
    if (options.every === undefined) return
    if (watchingMoving()) {
      if (stopTimer === undefined) stopTimer = options.every(againMs, tick)
      return
    }
    stopTimer?.()
    stopTimer = undefined
  }

  const ensureFocus = (): void => {
    if (stopFocus !== undefined || options.onFocus === undefined) return
    stopFocus = options.onFocus(() => {
      for (const slot of slots.values()) {
        if (slot.listeners.size === 0) continue
        if (slot.at === 0 || now() - slot.at >= minuteMs) load(slot, false)
      }
    })
  }

  const prune = (): void => {
    for (const [key, slot] of slots) {
      if (slot.listeners.size === 0 && slot.pending === undefined && slot.at > 0 && now() - slot.at >= minuteMs) slots.delete(key)
    }
  }

  return {
    current(address) {
      const slot = slots.get(keyOf(address))
      if (slot === undefined || slot.at === 0) return undefined
      return slot.value
    },
    watch(address, listener) {
      prune()
      const key = keyOf(address)
      let slot = slots.get(key)
      if (slot === undefined) {
        slot = { address, value: undefined, at: 0, pending: undefined, listeners: new Set() }
        slots.set(key, slot)
      }
      slot.listeners.add(listener)
      ensureFocus()
      if (slot.pending === undefined && (slot.at === 0 || now() - slot.at >= minuteMs)) load(slot, false)
      else ensureTimer()
      return () => {
        slot.listeners.delete(listener)
        if (!anyListeners()) {
          stopFocus?.()
          stopFocus = undefined
          stopTimer?.()
          stopTimer = undefined
          return
        }
        ensureTimer()
      }
    }
  }
}

export function createPullLinkWatch(options: {
  readonly read: (address: PullLinkAddress) => Promise<LinkedPullRequest | undefined>
  readonly now?: () => number
  readonly minuteMs?: number
  readonly againMs?: number
  readonly every?: (ms: number, fn: () => void) => () => void
  readonly onFocus?: (fn: () => void) => () => void
}): LinkWatch<LinkedPullRequest> {
  return createLinkWatch({ ...options, moving: (value) => (value.state === 'open' || value.state === 'draft') && value.checks === 'running' })
}

/** An issue's watch: the same reading, never on a timer. */
export function createIssueLinkWatch(options: {
  readonly read: (address: PullLinkAddress) => Promise<LinkedIssue | undefined>
  readonly now?: () => number
  readonly minuteMs?: number
  readonly onFocus?: (fn: () => void) => () => void
}): LinkWatch<LinkedIssue> {
  return createLinkWatch(options)
}

const onWindowFocus = (fn: () => void): (() => void) => {
  if (typeof window === 'undefined') return () => undefined
  window.addEventListener('focus', fn)
  return () => window.removeEventListener('focus', fn)
}

let sharedIssues: LinkWatch<LinkedIssue> | undefined

/** The window's one issue watch: every chip of one issue shares it. */
export function sharedIssueLinks(): LinkWatch<LinkedIssue> {
  if (sharedIssues === undefined) {
    sharedIssues = createIssueLinkWatch({
      read: (address) => {
        const bridge = typeof window === 'undefined' ? undefined : window.desktop
        if (bridge === undefined) return Promise.resolve(undefined)
        return bridge.issueLink({ owner: address.owner, repo: address.repo, number: address.number })
      },
      now: () => Date.now(),
      onFocus: onWindowFocus
    })
  }
  return sharedIssues
}

let shared: LinkWatch<LinkedPullRequest> | undefined

/** The window's one watch: every chip of one pull request shares it. */
export function sharedPullLinks(): LinkWatch<LinkedPullRequest> {
  if (shared === undefined) {
    shared = createPullLinkWatch({
      read: (address) => {
        const bridge = typeof window === 'undefined' ? undefined : window.desktop
        if (bridge === undefined) return Promise.resolve(undefined)
        return bridge.pullLink({ owner: address.owner, repo: address.repo, number: address.number })
      },
      now: () => Date.now(),
      every: (ms, fn) => {
        const id = setInterval(fn, ms)
        return () => clearInterval(id)
      },
      onFocus: onWindowFocus
    })
  }
  return shared
}
