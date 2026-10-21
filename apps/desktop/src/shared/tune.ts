import { instructionsText } from './teammate-instructions.js'
import { hasColumnInstructions, type CompareRoute, type PublicCompare } from './compare.js'
import type { PublicTeammate } from './ipc.js'

export const TUNE_DRAFT_CHANNEL = 'teammate:tune-drafts'
export const TUNE_LESSON_NOTE_CHANNEL = 'teammate:tune-lesson-note'
export const TUNE_LESSON_LIST_CHANNEL = 'teammate:tune-lesson-list'
export const TUNE_LESSON_REMOVE_CHANNEL = 'teammate:tune-lesson-remove'

export interface TuneEdit { readonly instructions: string; readonly why: string }
export interface TuneDrafts { readonly how: TuneEdit; readonly knows: TuneEdit }
export const TUNE_FALLBACK = 'The drafts could not be written. These are copies of Current; you can write your own.'
export const TUNE_DRAFT_COST = 'Opening /tune starts one side turn on this model to draft the edits. The three comparison runs start only when you press Run all three.'
export const TUNE_LESSON_COST = (model: string, free: boolean): string =>
  `One side turn on ${model} — ${free ? 'free' : 'counts against your plan'}.`
export const TUNE_LESSON_FALLBACK = 'That reflection could not be read as JSON. Nothing was saved.'
export const TUNE_LESSON_CANNOT_TELL = 'One run was not enough to tell the edits apart. No lesson was saved.'
export const TUNE_LESSON_NONE = 'This tune did not point to a general lesson. Nothing was saved.'

export const TUNE_EDIT_VERDICTS = ['helped', 'hurt', 'no clear effect', 'cannot tell'] as const
export type TuneEditVerdict = (typeof TUNE_EDIT_VERDICTS)[number]
export const TUNE_LESSON_CONFIDENCES = ['high', 'medium', 'low'] as const
export type TuneLessonConfidence = (typeof TUNE_LESSON_CONFIDENCES)[number]

export interface TuneEditOutcome {
  readonly draft?: string
  readonly changed: string
  readonly did: string
  readonly verdict: TuneEditVerdict
}

export interface TuneLessonDraft {
  readonly when: string
  readonly do: string
  readonly because: string
  readonly confidence: TuneLessonConfidence
}

export interface PublicTuneLesson extends TuneLessonDraft {
  readonly lessonId: string
  readonly teammateId: string
  readonly compareId: string
  readonly createdAt: string
}

export type ParsedTuneLesson =
  | { readonly ok: true; readonly edits: readonly TuneEditOutcome[]; readonly lesson: TuneLessonDraft | null; readonly message?: string }
  | { readonly ok: false; readonly edits: readonly []; readonly lesson: null; readonly message: string }

export type TuneLessonNoteResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly runId: string
        readonly missionId: string
        readonly edits: readonly TuneEditOutcome[]
        readonly lesson: PublicTuneLesson | null
        readonly message?: string
      }
    }
  | { readonly ok: false; readonly message: string }

export type TuneLessonListResponse =
  | { readonly ok: true; readonly data: { readonly lessons: readonly PublicTuneLesson[] } }
  | { readonly ok: false; readonly message: string }

export function currentDrafts(current: string, why = TUNE_FALLBACK): TuneDrafts {
  return { how: { instructions: current, why }, knows: { instructions: current, why } }
}

/** Only the two bounded edits cross into the card, never arbitrary fields or executable output. */
export function parseTuneDrafts(raw: string, current: string): TuneDrafts {
  try {
    if (raw.length > 32_000) return currentDrafts(current)
    const body = raw.trim().replace(/^```(?:json)?\s*\n([\s\S]*?)\n```$/i, '$1')
    const value: unknown = JSON.parse(body)
    if (typeof value !== 'object' || value === null) return currentDrafts(current)
    const edit = (input: unknown): TuneEdit | undefined => {
      if (typeof input !== 'object' || input === null) return undefined
      const row = input as Record<string, unknown>
      const instructions = instructionsText(row.instructions)
      const why = typeof row.why === 'string' ? row.why.replace(/\s+/g, ' ').trim() : ''
      return instructions === undefined || why.length === 0 || why.length > 400 || /[\u0000-\u001f\u007f]/.test(why)
        ? undefined : { instructions, why }
    }
    const row = value as Record<string, unknown>
    const how = edit(row.how)
    const knows = edit(row.knows)
    return how === undefined || knows === undefined ? currentDrafts(current) : { how, knows }
  } catch { return currentDrafts(current) }
}

export function tuneRefusal(teammate: PublicTeammate | undefined, missionId: string | undefined, finished: boolean): string | undefined {
  if (teammate === undefined) return 'Open a teammate’s conversation to tune its instructions. Nothing was started.'
  if (missionId === undefined || !finished) return 'Finish a turn in this teammate’s conversation first. Nothing was started.'
  if (teammate.route === undefined) return 'Choose this teammate’s model in Edit teammate first. Nothing was started.'
  return undefined
}

/** Keep is not permission to change standing instructions: this is only an offer. */
export function keptInstructions(compare: PublicCompare, teammate: PublicTeammate | undefined): string | undefined {
  if (compare.kept === undefined || teammate === undefined || compare.teammateId !== teammate.teammateId) return undefined
  const instructions = compare.slots.find((column) => column.slot === compare.kept?.slot)?.route.instructions
  return instructions === undefined || instructions === (teammate.instructions ?? '') ? undefined : instructions
}

/** Whether a comparison is a kept tune belonging to this teammate (0.810). */
export function isKeptTune(compare: PublicCompare, teammate: PublicTeammate | undefined): teammate is PublicTeammate {
  return compare.kept !== undefined && teammate !== undefined && compare.teammateId === teammate.teammateId && hasColumnInstructions(compare)
}

/** One line in the kept strip and Edit teammate ("When … do …", 0.810). */
export function tuneLessonLine(lesson: Pick<TuneLessonDraft, 'when' | 'do'>): string {
  const cue = lesson.when.trim().replace(/^when\s+/i, '').replace(/[.,;:]+$/, '')
  const action = lesson.do.trim().replace(/^do[:\s]+/i, '').replace(/\.$/, '')
  return `When ${cue}, do ${action}.`
}

const cleanLine = (value: unknown): string | undefined => {
  if (typeof value !== 'string') return undefined
  const line = value.replace(/\s+/g, ' ').trim()
  return line.length === 0 || /[\u0000-\u001f\u007f]/.test(line) ? undefined : line.slice(0, 400)
}

const normalizeVerdict = (value: unknown): TuneEditVerdict | undefined => {
  if (typeof value !== 'string') return undefined
  const lower = value.trim().toLowerCase()
  return (TUNE_EDIT_VERDICTS as readonly string[]).includes(lower) ? (lower as TuneEditVerdict) : undefined
}

const normalizeConfidence = (value: unknown): TuneLessonConfidence | undefined => {
  if (typeof value !== 'string') return undefined
  const lower = value.trim().toLowerCase()
  return (TUNE_LESSON_CONFIDENCES as readonly string[]).includes(lower) ? (lower as TuneLessonConfidence) : undefined
}

const mentionsEditRow = (because: string, row: TuneEditOutcome): boolean => {
  const haystack = because.toLowerCase()
  const draft = row.draft?.toLowerCase()
  if (draft === 'how' && (haystack.includes('how it works') || /\bhow\b/.test(haystack))) return true
  if (draft === 'knows' && (haystack.includes('what it knows') || /\bknows\b/.test(haystack))) return true
  if (draft !== undefined && draft.length > 0 && haystack.includes(draft)) return true
  const changed = row.changed.toLowerCase()
  return changed.length >= 4 && haystack.includes(changed)
}

/**
 * DEFENSIVE PARSE OF WHAT A TUNE SHOWED (0.810). A reply that will not parse saves nothing and says so in one line;
 * a lesson resting only on `cannot tell` rows is dropped so one noisy run never enters the teammate's lesson list.
 */
export function parseTuneLesson(raw: string): ParsedTuneLesson {
  const fail = (message = TUNE_LESSON_FALLBACK): ParsedTuneLesson => ({ ok: false, edits: [], lesson: null, message })
  try {
    if (typeof raw !== 'string' || raw.length === 0 || raw.length > 32_000) return fail()
    const stripped = raw.replace(/<think>[\s\S]*?<\/think>/gi, '').trim()
    const fenced = /```(?:json)?\s*([\s\S]*?)```/i.exec(stripped)
    const candidate = (fenced !== null ? fenced[1]! : stripped).trim()
    const firstBrace = candidate.indexOf('{')
    const lastBrace = candidate.lastIndexOf('}')
    const body = firstBrace >= 0 && lastBrace > firstBrace ? candidate.slice(firstBrace, lastBrace + 1) : candidate
    const value: unknown = JSON.parse(body)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return fail()
    const record = value as Record<string, unknown>
    if ('lessons' in record && record.lessons !== undefined) return fail()
    const parseEdit = (input: unknown, fallbackDraft?: string): TuneEditOutcome | undefined => {
      if (typeof input !== 'object' || input === null || Array.isArray(input)) return undefined
      const row = input as Record<string, unknown>
      const changed = cleanLine(row.changed)
      const did = cleanLine(row.did)
      const verdict = normalizeVerdict(row.verdict)
      const draft = cleanLine(row.draft ?? row.setup ?? fallbackDraft)
      if (changed === undefined || did === undefined || verdict === undefined) return undefined
      return { ...(draft === undefined ? {} : { draft }), changed, did, verdict }
    }
    let rawEdits: readonly TuneEditOutcome[] = []
    if (Array.isArray(record.edits)) {
      const nonCurrent = record.edits.filter((entry) => {
        if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) return true
        const tag = String((entry as Record<string, unknown>).draft ?? (entry as Record<string, unknown>).setup ?? '').trim().toLowerCase()
        return tag !== 'current'
      })
      const source = nonCurrent.length > 0 ? nonCurrent : record.edits
      if (source.length === 0 || source.length > 4) return fail()
      const parsed = source.map((entry) => parseEdit(entry))
      if (parsed.some((entry) => entry === undefined)) return fail()
      rawEdits = parsed.filter((entry): entry is TuneEditOutcome => entry !== undefined)
    } else if (typeof record.edits === 'object' && record.edits !== null) {
      const map = record.edits as Record<string, unknown>
      const how = parseEdit(map.how, 'how')
      const knows = parseEdit(map.knows, 'knows')
      if (how === undefined || knows === undefined) return fail()
      rawEdits = [how, knows]
    } else {
      return fail()
    }
    if (record.lesson === null || record.lesson === undefined) {
      return { ok: true, edits: rawEdits, lesson: null, message: TUNE_LESSON_NONE }
    }
    if (typeof record.lesson !== 'object' || Array.isArray(record.lesson)) return fail()
    const lessonRecord = record.lesson as Record<string, unknown>
    const when = cleanLine(lessonRecord.when)
    const action = cleanLine(lessonRecord.do)
    const because = Array.isArray(lessonRecord.because)
      ? cleanLine(lessonRecord.because.filter((item): item is string => typeof item === 'string').join('; '))
      : cleanLine(lessonRecord.because)
    const confidence = normalizeConfidence(lessonRecord.confidence)
    if (when === undefined || action === undefined || because === undefined || confidence === undefined) return fail()
    const allCannotTell = rawEdits.every((entry) => entry.verdict === 'cannot tell')
    const namedCannotTell = rawEdits.some((entry) => entry.verdict === 'cannot tell' && mentionsEditRow(because, entry))
    const namedOther = rawEdits.some((entry) => entry.verdict !== 'cannot tell' && mentionsEditRow(because, entry))
    const onlyCannotTell = allCannotTell || (namedCannotTell && !namedOther) || (/\bcannot tell\b/i.test(because) && !namedOther)
    if (onlyCannotTell) {
      return { ok: true, edits: rawEdits, lesson: null, message: TUNE_LESSON_CANNOT_TELL }
    }
    return {
      ok: true,
      edits: rawEdits,
      lesson: { when, do: action, because, confidence }
    }
  } catch {
    return fail()
  }
}

/**
 * A SETUP THAT REPEATS ONE BEFORE IT (0.809). An angle may keep the instructions as they are, and drafts that could
 * not be written start as copies of Current, so two columns can run the same words. That stays allowed -- after
 * Harness², the same setup run twice shows how much one run changes by chance -- but the card says so before the
 * runs start, since each run counts against a paid plan. Each draft names the first setup it matches; `distinct`
 * counts the different setups among the three.
 */
export function repeatsOf(current: string, drafts: TuneDrafts): { readonly how?: string; readonly knows?: string; readonly distinct: number } {
  const setups = [['Current', current.trim()], ['How it works', drafts.how.instructions.trim()], ['What it knows', drafts.knows.instructions.trim()]] as const
  const earlier = (index: number): string | undefined => setups.slice(0, index).find(([, words]) => words === setups[index]![1])?.[0]
  const how = earlier(1)
  const knows = earlier(2)
  return { ...(how === undefined ? {} : { how }), ...(knows === undefined ? {} : { knows }), distinct: new Set(setups.map(([, words]) => words)).size }
}

/** Each trial keeps the same route. Identical copies remain valid when an angle recommends no change. */
export function tuneRoutes<R extends CompareRoute>(route: R, current: string, drafts: TuneDrafts): readonly (Omit<R, 'label'> & { readonly label: string; readonly instructions: string })[] {
  const angles = ['Current', 'How it works', 'What it knows']
  return [current, drafts.how.instructions, drafts.knows.instructions].map((instructions, index) => ({ ...route, label: `${route.label ?? route.model} · ${angles[index]}`, instructions }))
}
