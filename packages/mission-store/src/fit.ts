import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

/**
 * The ledger's own limits, applied once, where events meet it.
 *
 * Ten normalizers turn ten CLIs' records into events, and each used to bound
 * its own fields -- or not, by whoever wrote that branch. The reader refuses a
 * field over its limit, and the writer refuses what the reader would refuse,
 * so one unbounded field stopped a whole run with a durable-record failure:
 * the 0.715 hunt's MU1 and CP1, and 0.777's ACP message chunk (review
 * 2026-10-15, C1 and D2). Fitting here ends the class instead of the instance.
 *
 * Only sizes are fitted. What is structural -- a type, an enum, a correlation
 * id, a sequence -- is left as it came, so the reader's strictness still
 * catches a wrong event rather than a long one.
 */

/** Must match the reader's limits in index.ts. */
export const LEDGER_TEXT_LIMIT = 16_384
/**
 * A teammate's answer: `message.delta`'s `text`, and only that field (0.869).
 *
 * At 16,384 a long answer was kept with its middle cut out, in the window and
 * for every app reading it: Colin, 2026-11-07, a Codex report that an app read
 * over Locust's MCP server "with the middle cut out", everything between two
 * of its headings missing, so the app had to ask for that part again. A record
 * may take 512 KB, and 131,072 characters fits with room to spare; `fitWeight`
 * shortens the rare answer whose bytes still do not fit.
 */
export const LEDGER_MESSAGE_TEXT_LIMIT = 131_072
export const LEDGER_NAME_LIMIT = 512
export const LEDGER_THREAD_LIMIT = 2_048
export const LEDGER_ROUTE_LIMIT = 256
const LEDGER_JSON_ENTRIES = 100
const LEDGER_JSON_DEPTH = 8
/** Under the 512 KB a record may take, with room for the record's own fields. */
const LEDGER_EVENT_BYTES = 480 * 1024
const STDERR_KEPT = 64 * 1024

const MARK = '\n…[shortened]…\n'
const TAIL_KEPT = 4_096

/** Payload fields that are names and ids, at the reader's 512. */
const NAME_FIELDS = new Set(['itemId', 'itemType', 'status', 'toolKind', 'name', 'parentItemId', 'code'])
/** Payload fields that are vocabulary: a wrong one should still be refused. */
const STRUCTURAL_FIELDS = new Set(['stepKind', 'operation', 'kind', 'level', 'phase', 'runtimeTerminal', 'final', 'terminal', 'sessionEnded', 'exitCode'])
/** Payload fields that hold JSON rather than words. */
const JSON_FIELDS = new Set(['plan', 'output', 'usage'])

/** A string the reader will take: no NUL, at most `limit` characters, both ends kept. */
export function fitText(value: string, limit: number): string {
  const clean = value.includes('\0') ? value.replaceAll('\0', '�') : value
  if (clean.length <= limit) return clean
  if (limit >= TAIL_KEPT * 2) {
    return `${clean.slice(0, limit - TAIL_KEPT - MARK.length)}${MARK}${clean.slice(-TAIL_KEPT)}`
  }
  return `${clean.slice(0, limit - 1)}…`
}

/** JSON the reader will take: every string, list, object and depth inside its limits. */
export function fitJson(value: unknown, depth = 0): unknown {
  if (value === null || typeof value === 'boolean') return value
  if (typeof value === 'number') return Number.isFinite(value) ? value : null
  if (typeof value === 'string') return fitText(value, LEDGER_TEXT_LIMIT)
  if (Array.isArray(value)) {
    if (value.length === 0) return value
    if (depth >= LEDGER_JSON_DEPTH) return '[nested deeper than the record keeps]'
    let changed = value.length > LEDGER_JSON_ENTRIES
    const kept = value.slice(0, LEDGER_JSON_ENTRIES).map((entry: unknown) => {
      const fitted = fitJson(entry, depth + 1)
      if (fitted !== entry) changed = true
      return fitted
    })
    return changed ? kept : value
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).filter(([, entry]) => entry !== undefined)
    if (entries.length === 0) return value
    if (depth >= LEDGER_JSON_DEPTH) return '[nested deeper than the record keeps]'
    let changed = entries.length > LEDGER_JSON_ENTRIES
    const kept: Record<string, unknown> = {}
    for (const [key, entry] of entries.slice(0, LEDGER_JSON_ENTRIES)) {
      const fittedKey = fitText(key, LEDGER_NAME_LIMIT)
      const fitted = fitJson(entry, depth + 1)
      if (fittedKey !== key || fitted !== entry) changed = true
      kept[fittedKey] = fitted
    }
    return changed ? kept : value
  }
  // Nothing else is JSON; the reader would refuse it, so say what it was.
  return String(value)
}

function fitEvidence(value: unknown): unknown {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return value
  const evidence = value as Record<string, unknown>
  let changed = false
  const next: Record<string, unknown> = { ...evidence }
  if (typeof evidence.runtimeEventType === 'string') {
    const fitted = fitText(evidence.runtimeEventType, LEDGER_NAME_LIMIT)
    if (fitted !== evidence.runtimeEventType) { next.runtimeEventType = fitted; changed = true }
  }
  if (evidence.raw !== undefined) {
    const fitted = fitJson(evidence.raw)
    if (fitted !== evidence.raw) { next.raw = fitted; next.redacted = true; changed = true }
  }
  return changed ? next : value
}

function fitProcess(value: unknown): unknown {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return value
  const process = value as Record<string, unknown>
  if (typeof process.signal === 'string' && process.signal.length > 64) {
    return fitProcess({ ...process, signal: fitText(process.signal, 64) })
  }
  return value
}

function fitPayload(payload: Record<string, unknown>, message = false): Record<string, unknown> {
  let changed = false
  const next: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(payload)) {
    let fitted: unknown = value
    if (key === 'evidence') fitted = fitEvidence(value)
    else if (key === 'process') fitted = fitProcess(value)
    else if (JSON_FIELDS.has(key)) fitted = fitJson(value)
    else if (typeof value === 'string' && !STRUCTURAL_FIELDS.has(key)) {
      fitted = fitText(value, key === 'runtimeThreadId' ? LEDGER_THREAD_LIMIT : NAME_FIELDS.has(key) ? LEDGER_NAME_LIMIT : message && key === 'text' ? LEDGER_MESSAGE_TEXT_LIMIT : LEDGER_TEXT_LIMIT)
    }
    if (fitted !== value) changed = true
    next[key] = fitted
  }
  return changed ? next : payload
}

/** What is still too heavy for one record, dropped in the order nobody misses. */
function fitWeight(event: NormalizedRuntimeEvent): NormalizedRuntimeEvent {
  const weight = (candidate: NormalizedRuntimeEvent): number => Buffer.byteLength(JSON.stringify(candidate), 'utf8')
  if (weight(event) <= LEDGER_EVENT_BYTES) return event
  let payload = { ...(event.payload as unknown as Record<string, unknown>) }
  const evidence = payload.evidence
  if (evidence !== null && typeof evidence === 'object' && !Array.isArray(evidence) && 'raw' in evidence) {
    const { raw: _raw, ...rest } = evidence as Record<string, unknown>
    payload = { ...payload, evidence: { ...rest, redacted: true } }
  }
  let next = { ...event, payload } as unknown as NormalizedRuntimeEvent
  if (weight(next) <= LEDGER_EVENT_BYTES) return next
  const process = payload.process
  if (process !== null && typeof process === 'object' && !Array.isArray(process)) {
    const held = process as Record<string, unknown>
    if (typeof held.stderr === 'string' && held.stderr.length > STDERR_KEPT) {
      payload = { ...payload, process: { ...held, stderr: held.stderr.slice(-STDERR_KEPT), stderrTruncated: true } }
      next = { ...event, payload } as unknown as NormalizedRuntimeEvent
      if (weight(next) <= LEDGER_EVENT_BYTES) return next
    }
  }
  for (const key of JSON_FIELDS) {
    if (payload[key] === undefined) continue
    payload = { ...payload, [key]: '[too large for one record: the run produced it, the record does not keep it]' }
    next = { ...event, payload } as unknown as NormalizedRuntimeEvent
    if (weight(next) <= LEDGER_EVENT_BYTES) return next
  }
  // An answer whose characters fit and whose bytes do not -- wide scripts,
  // escaped control characters -- is shortened, both ends kept, until it does.
  while (typeof payload.text === 'string' && payload.text.length > TAIL_KEPT * 2 && weight(next) > LEDGER_EVENT_BYTES) {
    payload = { ...payload, text: fitText(payload.text, Math.max(TAIL_KEPT * 2, Math.floor(payload.text.length * 0.7))) }
    next = { ...event, payload } as unknown as NormalizedRuntimeEvent
  }
  return next
}

/**
 * The event as the ledger can keep it. The same object back when it already
 * fits, so the common case costs one walk and no copy.
 */
export function fitEventToLedger(event: NormalizedRuntimeEvent): NormalizedRuntimeEvent {
  let next: NormalizedRuntimeEvent = event
  const record = event as unknown as Record<string, unknown>
  const top: Record<string, unknown> = {}
  for (const key of ['cliVersion', 'requestedRouteId', 'resolvedRouteId'] as const) {
    const value = record[key]
    if (typeof value === 'string' && value.length > LEDGER_ROUTE_LIMIT) top[key] = fitText(value, LEDGER_ROUTE_LIMIT)
  }
  if (typeof record.runtimeThreadId === 'string' && record.runtimeThreadId.length > LEDGER_THREAD_LIMIT) {
    top.runtimeThreadId = fitText(record.runtimeThreadId, LEDGER_THREAD_LIMIT)
  }
  const payload = event.payload as unknown
  const fittedPayload = payload !== null && typeof payload === 'object' && !Array.isArray(payload)
    ? fitPayload(payload as Record<string, unknown>, event.type === 'message.delta')
    : payload
  if (Object.keys(top).length > 0 || fittedPayload !== payload) {
    next = { ...event, ...top, payload: fittedPayload } as unknown as NormalizedRuntimeEvent
  }
  return fitWeight(next)
}

export function fitEventsToLedger(events: readonly NormalizedRuntimeEvent[]): readonly NormalizedRuntimeEvent[] {
  let changed = false
  const fitted = events.map((event) => {
    const next = fitEventToLedger(event)
    if (next !== event) changed = true
    return next
  })
  return changed ? fitted : events
}
