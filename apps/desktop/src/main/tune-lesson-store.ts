import { randomUUID } from 'node:crypto'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readFile, unlink } from 'node:fs/promises'
import { join } from 'node:path'

import { replaceFile } from './replace-file.js'
import { TUNE_LESSON_CONFIDENCES, type PublicTuneLesson, type TuneLessonConfidence } from '../shared/tune.js'

/**
 * TUNE LESSONS PER TEAMMATE (0.810).
 *
 * At most 20 lessons per teammate, newest last, each with its compareId and createdAt date. Read only by the next
 * /tune draft prompt (`tuneDraftPrompt`) and Edit teammate; never read by an ordinary turn's brief. Removing a
 * teammate removes its lessons.
 */
export const MAX_LESSONS_PER_TEAMMATE = 20

const SCHEMA_VERSION = 1 as const
const MAX_FILE_BYTES = 1024 * 1024
const UNREADABLE = 'The tune lesson record on disk could not be read.'

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly lessons: readonly PublicTuneLesson[]
}

const EMPTY: StoredFile = { schemaVersion: SCHEMA_VERSION, lessons: [] }

export interface CreateTuneLessonStoreOptions {
  readonly rootDirectory: string
  readonly now?: () => Date
  readonly createId?: () => string
}

export interface AddTuneLessonInput {
  readonly teammateId: string
  readonly compareId: string
  readonly when: string
  readonly do: string
  readonly because: string
  readonly confidence: TuneLessonConfidence
}

export interface TuneLessonStore {
  list(teammateId?: unknown): Promise<readonly PublicTuneLesson[]>
  add(input: AddTuneLessonInput): Promise<PublicTuneLesson>
  remove(lessonId: unknown): Promise<void>
  removeForTeammate(teammateId: unknown): Promise<void>
}

const cleanField = (value: unknown, max: number): string | undefined => {
  if (typeof value !== 'string') return undefined
  const text = value.replace(/\s+/g, ' ').trim()
  return text.length === 0 || text.length > max || /[\u0000-\u001f\u007f]/.test(text) ? undefined : text
}

const isConfidence = (value: unknown): value is TuneLessonConfidence =>
  typeof value === 'string' && (TUNE_LESSON_CONFIDENCES as readonly string[]).includes(value)

function parsedLesson(value: unknown): PublicTuneLesson | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const row = value as Record<string, unknown>
  const lessonId = cleanField(row.lessonId, 80)
  const teammateId = cleanField(row.teammateId, 200)
  const compareId = cleanField(row.compareId, 80)
  const createdAt = cleanField(row.createdAt, 40)
  const when = cleanField(row.when, 400)
  const action = cleanField(row.do, 400)
  const because = cleanField(row.because, 400)
  const confidence = row.confidence
  if (
    lessonId === undefined ||
    teammateId === undefined ||
    compareId === undefined ||
    createdAt === undefined ||
    when === undefined ||
    action === undefined ||
    because === undefined ||
    !isConfidence(confidence)
  ) {
    return undefined
  }
  return { lessonId, teammateId, compareId, createdAt, when, do: action, because, confidence }
}

function capPerTeammate(lessons: readonly PublicTuneLesson[]): readonly PublicTuneLesson[] {
  const counts = new Map<string, number>()
  const keptReversed: PublicTuneLesson[] = []
  const seenCompare = new Set<string>()
  for (let index = lessons.length - 1; index >= 0; index -= 1) {
    const lesson = lessons[index]!
    const tuneKey = `${lesson.teammateId}:${lesson.compareId}`
    if (seenCompare.has(tuneKey)) continue
    seenCompare.add(tuneKey)
    const count = counts.get(lesson.teammateId) ?? 0
    if (count >= MAX_LESSONS_PER_TEAMMATE) continue
    counts.set(lesson.teammateId, count + 1)
    keptReversed.push(lesson)
  }
  return keptReversed.reverse()
}

function parsedLessonFile(raw: string): StoredFile {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    throw new Error(UNREADABLE)
  }
  if (typeof parsed !== 'object' || parsed === null) throw new Error(UNREADABLE)
  const record = parsed as Record<string, unknown>
  if (record.schemaVersion !== SCHEMA_VERSION || !Array.isArray(record.lessons)) throw new Error(UNREADABLE)
  const lessons = record.lessons.map(parsedLesson).filter((entry): entry is PublicTuneLesson => entry !== undefined)
  return { schemaVersion: SCHEMA_VERSION, lessons: capPerTeammate(lessons) }
}

export function createTuneLessonStore(options: CreateTuneLessonStoreOptions): TuneLessonStore {
  const rootDirectory = options.rootDirectory
  const path = join(rootDirectory, 'tune-lessons.json')
  const now = options.now ?? (() => new Date())
  const createId = options.createId ?? (() => `les_${randomUUID().replace(/-/g, '').slice(0, 16)}`)

  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(work: () => Promise<T>): Promise<T> => {
    const next = queue.then(work, work)
    queue = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }

  const read = async (): Promise<StoredFile> => {
    let raw: string
    try {
      raw = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return EMPTY
      throw new Error(UNREADABLE)
    }
    if (Buffer.byteLength(raw, 'utf8') > MAX_FILE_BYTES) throw new Error(UNREADABLE)
    return parsedLessonFile(raw)
  }

  const write = async (file: StoredFile): Promise<void> => {
    await mkdir(rootDirectory, { recursive: true, mode: 0o700 })
    const temporary = `${path}.${randomUUID()}.tmp`
    const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600)
    try {
      await handle.writeFile(`${JSON.stringify(file, null, 2)}\n`, 'utf8')
      await handle.sync()
    } finally {
      await handle.close()
    }
    try {
      await replaceFile(temporary, path)
    } catch (error) {
      await unlink(temporary).catch(() => undefined)
      throw error
    }
  }

  return {
    list: (teammateId) =>
      serialize(async () => {
        const lessons = (await read()).lessons
        if (typeof teammateId !== 'string' || teammateId.trim().length === 0) return lessons
        return lessons.filter((lesson) => lesson.teammateId === teammateId)
      }),

    add: (input) =>
      serialize(async () => {
        const teammateId = cleanField(input.teammateId, 200)
        const compareId = cleanField(input.compareId, 80)
        const when = cleanField(input.when, 400)
        const action = cleanField(input.do, 400)
        const because = cleanField(input.because, 400)
        if (
          teammateId === undefined ||
          compareId === undefined ||
          when === undefined ||
          action === undefined ||
          because === undefined ||
          !isConfidence(input.confidence)
        ) {
          throw new Error('Write a valid tune lesson before saving.')
        }
        const file = await read()
        const lesson: PublicTuneLesson = {
          lessonId: createId(),
          teammateId,
          compareId,
          createdAt: now().toISOString(),
          when,
          do: action,
          because,
          confidence: input.confidence
        }
        const withoutSameTune = file.lessons.filter(
          (entry) => !(entry.teammateId === teammateId && entry.compareId === compareId)
        )
        const next = capPerTeammate([...withoutSameTune, lesson])
        await write({ schemaVersion: SCHEMA_VERSION, lessons: next })
        return lesson
      }),

    remove: (lessonId) =>
      serialize(async () => {
        if (typeof lessonId !== 'string' || lessonId.trim().length === 0) return
        const file = await read()
        const next = file.lessons.filter((lesson) => lesson.lessonId !== lessonId)
        if (next.length === file.lessons.length) return
        await write({ schemaVersion: SCHEMA_VERSION, lessons: next })
      }),

    removeForTeammate: (teammateId) =>
      serialize(async () => {
        if (typeof teammateId !== 'string' || teammateId.trim().length === 0) return
        const file = await read()
        const next = file.lessons.filter((lesson) => lesson.teammateId !== teammateId)
        if (next.length === file.lessons.length) return
        await write({ schemaVersion: SCHEMA_VERSION, lessons: next })
      })
  }
}
