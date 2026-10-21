import { instructionsText } from './teammate-instructions.js'
import { hasColumnInstructions, setupName, type CompareRoute, type PublicCompare } from './compare.js'
import type { PublicTeammate } from './ipc.js'

export const TUNE_DRAFT_CHANNEL = 'teammate:tune-drafts'
export const TUNE_LESSON_NOTE_CHANNEL = 'teammate:tune-lesson-note'
export const TUNE_LESSON_LIST_CHANNEL = 'teammate:tune-lesson-list'
export const TUNE_LESSON_REMOVE_CHANNEL = 'teammate:tune-lesson-remove'
export const TEAMMATE_SKILL_LIST_CHANNEL = 'teammate:skills-list'
export const TEAMMATE_SKILL_USE_CHANNEL = 'teammate:skills-use'
export const TEAMMATE_SKILL_REMOVE_CHANNEL = 'teammate:skills-remove'

/** A skill a teammate keeps (0.811, main/teammate-skills.ts): what Edit teammate lists, and what its Remove names. */
export interface PublicTeammateSkill {
  readonly teammateId: string
  readonly name: string
  readonly description: string
  readonly compareId?: string
  readonly createdAt: string
}

export type TeammateSkillListResponse =
  | { readonly ok: true; readonly data: { readonly skills: readonly PublicTeammateSkill[] } }
  | { readonly ok: false; readonly message: string }

/**
 * WHAT IT KNOWS, AS A SKILL (0.811). Harness² finds a teammate's skills the strongest of its levers, and standing
 * instructions the weakest; a practitioner's checks belong in a skill, loaded when the work matches its description,
 * not in words read on every turn. For a Claude Code teammate -- the one runtime Locust can hand a skill to for a single
 * run (claude-skills.ts) -- a tune's "What it knows" is drafted as one.
 */
export type TuneSkill = NonNullable<CompareRoute['skill']>
/** A name Claude Code gives a skill: lower case words and digits, joined by single hyphens. */
export const TUNE_SKILL_NAME = /^[a-z0-9]+(?:-[a-z0-9]+)*$/
export const MAX_SKILL_NAME = 64
export const MAX_SKILL_DESCRIPTION = 1_024
export const MAX_SKILL_BODY = 8_000

/** A skill read defensively: a name Claude Code takes, a one-line description of when to use it, and what it says. */
export function tuneSkillOf(value: unknown): TuneSkill | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const row = value as Record<string, unknown>
  const name = typeof row.name === 'string' ? row.name.trim() : ''
  const description = typeof row.description === 'string' ? row.description.replace(/\s+/g, ' ').trim() : ''
  const body = typeof row.body === 'string' ? row.body.replace(/\r\n?/g, '\n').trim() : ''
  if (name.length > MAX_SKILL_NAME || !TUNE_SKILL_NAME.test(name)) return undefined
  if (description.length === 0 || description.length > MAX_SKILL_DESCRIPTION || /[\u0000-\u001f\u007f]/.test(description)) return undefined
  if (body.length === 0 || body.length > MAX_SKILL_BODY || body.includes('\0')) return undefined
  return { name, description, body }
}

/** Its SKILL.md as Claude Code reads one: the name and description up top -- a JSON string is a YAML one -- then the rest. */
export function skillMarkdown(skill: TuneSkill): string {
  return `---\nname: ${skill.name}\ndescription: ${JSON.stringify(skill.description)}\n---\n\n${skill.body}\n`
}

/** A draft's edit: its instructions, why it was written, and -- for "What it knows" on Claude Code -- the skill it adds. */
export interface TuneEdit { readonly instructions: string; readonly why: string; readonly skill?: TuneSkill }
/** What the card says when a skill will not read whole, and Run all three waits for it. */
export const TUNE_SKILL_INCOMPLETE = 'A skill needs a name of lower-case words joined by hyphens, one sentence on when to use it, and what it says.'

/** A setup's standing instructions: a skill's setup keeps Current's, as Current reads on the card when it runs. */
export function setupInstructions(edit: TuneEdit, current: string): string {
  return edit.skill === undefined ? edit.instructions : current
}

/** What a skill says, for the person to review before keeping it: when it applies, then its checks. */
export function skillReview(skill: TuneSkill): string {
  return `When to use it: ${skill.description}\n\n${skill.body}`
}
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

/**
 * A SETUP, MEASURED (0.811): what its tune's benchmark counted for it over every run -- the first and each run again,
 * scored by the same judge on the same points -- as the table shows it. Handed to Note what this tune showed, which
 * otherwise has one run of each setup to stand on and rightly finds nothing in it.
 */
export interface TuneMeasured {
  readonly slot: 'a' | 'b' | 'c'
  readonly runs: number
  readonly scored: number
  /** 0-100, over its scored runs. */
  readonly score?: number
  readonly everyPoint: number
  readonly checked: number
  readonly checkPassed: number
}

/** The measured records the window sends, read defensively: whole counts that add up, one a column, or none at all. */
export function tuneMeasuredOf(value: unknown): readonly TuneMeasured[] | undefined {
  if (!Array.isArray(value) || value.length === 0 || value.length > 3) return undefined
  const count = (input: unknown): number | undefined => (typeof input === 'number' && Number.isInteger(input) && input >= 0 && input <= 50 ? input : undefined)
  const records: TuneMeasured[] = []
  for (const entry of value) {
    if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) return undefined
    const row = entry as Record<string, unknown>
    const slot = row.slot === 'a' || row.slot === 'b' || row.slot === 'c' ? row.slot : undefined
    const runs = count(row.runs)
    const scored = count(row.scored)
    const everyPoint = count(row.everyPoint)
    const checked = count(row.checked)
    const checkPassed = count(row.checkPassed)
    const score = row.score === undefined ? undefined : typeof row.score === 'number' && Number.isFinite(row.score) && row.score >= 0 && row.score <= 100 ? Math.round(row.score) : null
    if (slot === undefined || runs === undefined || scored === undefined || everyPoint === undefined || checked === undefined || checkPassed === undefined || score === null) return undefined
    if (scored > runs || everyPoint > scored || checked > runs || checkPassed > checked || records.some((one) => one.slot === slot)) return undefined
    records.push({ slot, runs, scored, ...(score === undefined ? {} : { score }), everyPoint, checked, checkPassed })
  }
  return records
}

export function currentDrafts(current: string, why = TUNE_FALLBACK): TuneDrafts {
  return { how: { instructions: current, why }, knows: { instructions: current, why } }
}

/**
 * Only the two bounded edits cross into the card, never arbitrary fields or executable output. With `skills` (a Claude
 * Code teammate, 0.811) "knows" may be a skill instead: its setup keeps Current's instructions and adds the skill. A
 * "knows" that keeps Current and drafts no skill stays a valid outcome, as either angle's always was.
 */
export function parseTuneDrafts(raw: string, current: string, options: { readonly skills?: boolean } = {}): TuneDrafts {
  try {
    if (raw.length > 32_000) return currentDrafts(current)
    const body = raw.trim().replace(/^```(?:json)?\s*\n([\s\S]*?)\n```$/i, '$1')
    const value: unknown = JSON.parse(body)
    if (typeof value !== 'object' || value === null) return currentDrafts(current)
    const whyOf = (row: Record<string, unknown>): string | undefined => {
      const why = typeof row.why === 'string' ? row.why.replace(/\s+/g, ' ').trim() : ''
      return why.length === 0 || why.length > 400 || /[\u0000-\u001f\u007f]/.test(why) ? undefined : why
    }
    const edit = (input: unknown): TuneEdit | undefined => {
      if (typeof input !== 'object' || input === null) return undefined
      const row = input as Record<string, unknown>
      const why = whyOf(row)
      if (why === undefined) return undefined
      if (options.skills === true && row.skill !== undefined && row.skill !== null) {
        const skill = tuneSkillOf(row.skill)
        return skill === undefined ? undefined : { instructions: current, why, skill }
      }
      const instructions = instructionsText(row.instructions)
      return instructions === undefined ? undefined : { instructions, why }
    }
    const row = value as Record<string, unknown>
    const how = edit(row.how)
    const knows = edit(row.knows)
    // A skill is only ever "What it knows": a "how" that drafted one is read as no edit at all.
    return how === undefined || knows === undefined || how.skill !== undefined ? currentDrafts(current) : { how, knows }
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

/** A field's first word in lower case, unless it is an acronym such as "API": two fields become one sentence. */
const midSentence = (text: string): string => (/^[A-Z](?=[a-z\s'’])/.test(text) ? text.charAt(0).toLowerCase() + text.slice(1) : text)

/**
 * One line in the kept strip and Edit teammate ("When …, …", 0.810). The model writes each field as a sentence of its
 * own, so the 0.810 drive read "When A teammate's standing instructions …, do Add a line …"; joined, it reads as one.
 */
export function tuneLessonLine(lesson: Pick<TuneLessonDraft, 'when' | 'do'>): string {
  const cue = midSentence(lesson.when.trim().replace(/^when\s+/i, '').replace(/[.,;:]+$/, ''))
  const action = midSentence(lesson.do.trim().replace(/^do[:\s]+/i, '').replace(/\.$/, ''))
  return `When ${cue}, ${action}.`
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
  // A setup is its instructions and the skill it adds (0.811): Current's words with a skill are not Current.
  const of = (edit: TuneEdit): string => (edit.skill === undefined ? edit.instructions.trim() : JSON.stringify([current.trim(), edit.skill]))
  const setups = [['Current', current.trim()], ['How it works', of(drafts.how)], ['What it knows', of(drafts.knows)]] as const
  const earlier = (index: number): string | undefined => setups.slice(0, index).find(([, words]) => words === setups[index]![1])?.[0]
  const how = earlier(1)
  const knows = earlier(2)
  return { ...(how === undefined ? {} : { how }), ...(knows === undefined ? {} : { knows }), distinct: new Set(setups.map(([, words]) => words)).size }
}

/** Each trial keeps the same route. Identical copies remain valid when an angle recommends no change. */
export function tuneRoutes<R extends CompareRoute>(route: R, current: string, drafts: TuneDrafts): readonly (Omit<R, 'label'> & { readonly label: string; readonly instructions: string; readonly skill?: TuneSkill })[] {
  const angles = ['Current', 'How it works', 'What it knows']
  return [{ instructions: current, why: '' }, drafts.how, drafts.knows].map((edit: TuneEdit, index) => ({
    ...route,
    label: `${route.label ?? route.model} · ${angles[index]}`,
    instructions: setupInstructions(edit, current),
    // What it knows, as a skill (0.811): its column's run alone is handed it.
    ...(edit.skill === undefined ? {} : { skill: edit.skill })
  }))
}

/** The skill the kept setup adds (0.811), to offer for the teammate as the kept instructions are offered. */
export function keptSkill(compare: PublicCompare): TuneSkill | undefined {
  if (compare.kept === undefined || !hasColumnInstructions(compare)) return undefined
  return compare.slots.find((column) => column.slot === compare.kept?.slot)?.route.skill
}

/**
 * The skill a setup not kept adds (0.811): with How it works kept, What it knows' skill may be added beside it -- a
 * skill is added, not merged. Not when Current was kept: then the person chose the run without it.
 */
export function otherSkill(compare: PublicCompare): { readonly setup: string; readonly skill: TuneSkill } | undefined {
  if (compare.kept === undefined || !hasColumnInstructions(compare) || keptSkill(compare) !== undefined) return undefined
  const kept = compare.slots.find((column) => column.slot === compare.kept?.slot)
  if (kept === undefined || setupName(kept.route) === 'Current') return undefined
  const column = compare.slots.find((one) => one.slot !== compare.kept?.slot && one.route.skill !== undefined)
  const setup = column === undefined ? undefined : setupName(column.route)
  return column?.route.skill === undefined || setup === undefined ? undefined : { setup, skill: column.route.skill }
}

export const TUNE_COMBINE_CHANNEL = 'teammate:tune-combine'
export const TUNE_COMBINE_FALLBACK = 'The combined instructions could not be read. Nothing was saved.'
export const TUNE_COMBINE_SAME = 'The combined instructions came back the same as the kept setup’s, so there is nothing new to use.'

export type TuneCombineResponse =
  | { readonly ok: true; readonly data: { readonly runId: string; readonly missionId: string; readonly instructions: string; readonly note: string } }
  | { readonly ok: false; readonly message: string }

/** A kept tune's two drafts as they are combined (0.811): the kept one and the other, each an edit of Current. */
export interface TuneCombineSetups {
  readonly current: string
  readonly kept: { readonly name: string; readonly instructions: string; readonly why?: string }
  readonly other: { readonly name: string; readonly instructions: string; readonly why?: string }
}

/**
 * COMBINE BOTH EDITS (0.811), after Harness²'s composition step: a tune keeps one draft and leaves the other, which
 * may still have changed something worth keeping. Only when a draft was kept and the other draft is an edit of its
 * own -- different from Current and from the kept one; the kept one must itself have changed something.
 */
export function combineSetups(compare: PublicCompare): TuneCombineSetups | undefined {
  // A skill is added, not merged (0.811): the kept strip offers the other setup's skill as it is, so no side turn.
  if (compare.kept === undefined || !hasColumnInstructions(compare) || compare.slots.some((column) => column.route.skill !== undefined)) return undefined
  const named = compare.slots.map((column) => ({ slot: column.slot, name: setupName(column.route), words: (column.route.instructions ?? '').trim(), why: column.route.why }))
  const current = named.find((column) => column.name === 'Current')
  const kept = named.find((column) => column.slot === compare.kept?.slot)
  const other = named.find((column) => column.slot !== compare.kept?.slot && column.name !== undefined && column.name !== 'Current')
  if (current === undefined || kept?.name === undefined || kept.name === 'Current' || other?.name === undefined) return undefined
  if (kept.words === current.words || other.words === current.words || other.words === kept.words) return undefined
  const setup = (column: { readonly name: string; readonly words: string; readonly why: string | undefined }) => ({ name: column.name, instructions: column.words, ...(column.why === undefined ? {} : { why: column.why }) })
  return { current: current.words, kept: setup({ ...kept, name: kept.name }), other: setup({ ...other, name: other.name }) }
}

/** What the side turn wrote, read defensively: one set of instructions and one line saying what came from each. */
export function parseTuneCombine(raw: string, setups: TuneCombineSetups): { readonly ok: true; readonly instructions: string; readonly note: string } | { readonly ok: false; readonly message: string } {
  const fail = (message = TUNE_COMBINE_FALLBACK) => ({ ok: false, message }) as const
  try {
    if (typeof raw !== 'string' || raw.length === 0 || raw.length > 32_000) return fail()
    const stripped = raw.replace(/<think>[\s\S]*?<\/think>/gi, '').trim()
    const fenced = /```(?:json)?\s*([\s\S]*?)```/i.exec(stripped)
    const candidate = (fenced !== null ? fenced[1]! : stripped).trim()
    const first = candidate.indexOf('{')
    const last = candidate.lastIndexOf('}')
    const value: unknown = JSON.parse(first >= 0 && last > first ? candidate.slice(first, last + 1) : candidate)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return fail()
    const record = value as Record<string, unknown>
    const instructions = instructionsText(record.instructions)
    const note = cleanLine(record.note)
    if (instructions === undefined || instructions.length === 0 || note === undefined) return fail()
    if (instructions === setups.kept.instructions) return fail(TUNE_COMBINE_SAME)
    return { ok: true, instructions, note }
  } catch {
    return fail()
  }
}
