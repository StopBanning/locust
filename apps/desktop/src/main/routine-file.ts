import { isDeepStrictEqual } from 'node:util'
import { parse as parseYaml, stringify as stringifyYaml } from 'yaml'
import { MAX_INPUTS, inputsRefusal, keptInputs } from '../shared/routine-inputs.js'
import type { RoutineInput } from '../shared/routine-inputs.js'
import type { PublicRoutine, RoutineFlaggedPath } from '../shared/ipc.js'
import { stepsTooLongToSave, validRoutineName, validSteps } from './routine-store.js'

/**
 * A routine as a file (W7, docs/PLAN-2026-10-03-EXECUTE.md): `<name>.locust-routine.json`.
 *
 * What travels is what a routine IS -- its steps, what it asks for, the kind of
 * route it was made for and the connectors its steps name. What belongs to one
 * person's machine does not: no teammate, workspace or mission ids (`learnedFrom`),
 * no run counts, clock schedules, staged changes or execution state. Only the called-link kind travels, without its password.
 * A file is
 * written from an explicit list of fields, never by copying the routine, so a
 * field added to routines later does not leak into a file until someone decides
 * it should.
 */

export const ROUTINE_FILE_FORMAT = 'locust-routine'
export const ROUTINE_FILE_VERSION = 1
/** Exactly the keys a routine file may carry, and the only ones an import accepts. */
export const ROUTINE_FILE_KEYS = ['format', 'version', 'name', 'steps', 'inputs', 'handOffs', 'route', 'connectors', 'schedule'] as const
export const MAX_ROUTINE_FILE_BYTES = 512 * 1024
const MAX_CONNECTORS = 24

export interface RoutineFile {
  /** Only the called-link kind travels (2026-10-25), never its machine's password or URL. */
  readonly schedule?: { readonly kind: 'hook' }
  readonly format: typeof ROUTINE_FILE_FORMAT
  readonly version: typeof ROUTINE_FILE_VERSION
  readonly name: string
  readonly steps: readonly string[]
  readonly inputs: readonly RoutineInput[]
  /**
   * One per step. The ROLE of whoever took it where it was made -- never a
   * teammate's id -- and whether this step is the one that checks the work.
   */
  readonly handOffs: readonly { readonly role?: string; readonly check?: true }[]
  readonly route: { readonly runtime?: string }
  /** Connectors the steps name, so an import can say which this machine lacks. */
  readonly connectors: readonly string[]
}

/** A path a step names that would mean nothing -- or too much -- on another machine. */
const PATH_TOKEN = /(["'`])((?:[A-Za-z]:[\\/]|\/(?:Users|home)\/)[^"'`\r\n]+)\1|(?<=^|[\s(=[<])((?:[A-Za-z]:[\\/]|\/(?:Users|home)\/)[^\s"'`<>|*?()]*)/gm

/** The absolute paths the steps name, by step (counting from 1), trailing punctuation left off. */
export function absolutePathsIn(steps: readonly string[]): readonly RoutineFlaggedPath[] {
  return steps.flatMap((step, at) =>
    [...step.matchAll(PATH_TOKEN)].map((found) => ({ step: at + 1, path: found[2] ?? found[3]!.replace(/[.,;:!]+$/, '') }))
  )
}

/**
 * The steps and inputs with each absolute path turned into a text input
 * (`path`, `path_2`, ...): a path may name a file as well as a folder.
 * What the file carries when the person says
 * "make it an input". The same path is the same input wherever it appears.
 */
export function pathsAsInputs(steps: readonly string[], inputs: readonly RoutineInput[]):
  { readonly ok: true; readonly steps: readonly string[]; readonly inputs: readonly RoutineInput[] } | { readonly ok: false; readonly message: string } {
  const made: RoutineInput[] = [...inputs]
  const keyOf = new Map<string, string>()
  const free = (): string => {
    for (let number = 1; ; number += 1) {
      const key = number === 1 ? 'path' : `path_${String(number)}`
      if (!made.some((input) => input.key === key)) return key
    }
  }
  const next = steps.map((step) =>
    step.replace(PATH_TOKEN, (_whole: string, quote: string | undefined, quoted: string | undefined, bare: string | undefined) => {
      const path = quoted ?? bare!.replace(/[.,;:!]+$/, '')
      const tail = quoted === undefined ? bare!.slice(path.length) : ''
      let key = keyOf.get(path)
      if (key === undefined) {
        key = free()
        keyOf.set(path, key)
        made.push({ key, label: key === 'path' ? 'Path' : `Path ${key.replace('path_', '')}`, kind: 'text', required: true })
      }
      return `${quote ?? ''}{{${key}}}${quote ?? ''}${tail}`
    })
  )
  if (made.length > MAX_INPUTS) return { ok: false, message: `That would be ${String(made.length)} inputs; a routine asks for at most ${String(MAX_INPUTS)}.` }
  return { ok: true, steps: next, inputs: made }
}

/** Each connector (of those this machine has) that a step names, by its name, ignoring case. */
export function connectorsNamedIn(steps: readonly string[], names: readonly string[]): readonly string[] {
  const text = steps.join('\n').toLowerCase()
  const escaped = (name: string): string => name.toLowerCase().replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return [...new Set(names)].filter((name) => name.length > 0 && new RegExp(`(?<![a-z0-9_-])${escaped(name)}(?![a-z0-9_-])`).test(text))
}

/**
 * The file for a routine. `steps` and `inputs` are given so the caller can pass
 * the version where paths became inputs. Fields are named one by one.
 */
export function routineToFile(
  routine: PublicRoutine,
  using: {
    readonly steps: readonly string[]
    readonly inputs: readonly RoutineInput[]
    /** A teammate's role by id, for a step handed to someone else. */
    readonly roleOf: (teammateId: string) => string | undefined
    /** The connectors this machine has. */
    readonly connectorNames: readonly string[]
  }
): RoutineFile {
  return {
    format: ROUTINE_FILE_FORMAT,
    version: ROUTINE_FILE_VERSION,
    name: routine.name,
    ...(routine.schedule?.kind === 'hook' ? { schedule: { kind: 'hook' as const } } : {}),
    steps: [...using.steps],
    inputs: keptInputs(using.inputs),
    handOffs: using.steps.map((_, at) => {
      const entry = routine.handOffs?.[at]
      const who = entry?.teammateId
      // A chain says its roles for every step, the routine's own teammate's too, so another Locust can propose someone for each.
      const chain = routine.handOffs?.some((held) => held.check === true) === true
      const named = who === undefined || who === routine.teammateId ? (chain ? routine.teammateId : undefined) : who
      const role = named === undefined ? undefined : using.roleOf(named)
      return { ...(role === undefined ? {} : { role }), ...(entry?.check === true ? { check: true as const } : {}) }
    }),
    route: { runtime: routine.route.runtime },
    connectors: connectorsNamedIn(using.steps, using.connectorNames)
  }
}

/** The text of the file: the same bytes every time for the same routine. */
export function routineFileText(file: RoutineFile): string {
  return `${JSON.stringify(file, null, 2)}\n`
}

/*
 * A ROUTINE AS A MARKDOWN FILE (0.834, the Rakazo scrub's fifth take). Rakazo keeps routines as Markdown a person
 * reads, shares and edits; a `.locust-routine.json` is none of those. So a routine is exported as `<name>.routine.md`:
 * what it asks for and where it runs at the top, between `---` lines, then its name as the title and one
 * `## Step N` section per step, in the steps' own words. Edited by hand, it is imported again as any routine file is,
 * through the same checks. A routine the Markdown would not carry exactly -- a step with a `## Step` line of its own,
 * say -- is exported as the JSON file instead, so nothing is ever changed on the way out.
 */
const MARKDOWN_FIELDS = ['locust-routine', 'runs-on', 'connectors', 'inputs', 'when'] as const
const STEP_HEADING = /^## Step (\d+)((?: · [^\n]*)?)$/
const CHECKS_THE_WORK = 'checks the work'

/** The routine as Markdown: the same text every time for the same routine. */
export function routineMarkdown(file: RoutineFile): string {
  const top: Record<string, unknown> = { 'locust-routine': file.version }
  if (file.schedule?.kind === 'hook') top.when = 'hook'
  if (file.route.runtime !== undefined) top['runs-on'] = file.route.runtime
  if (file.connectors.length > 0) top.connectors = [...file.connectors]
  if (file.inputs.length > 0) top.inputs = file.inputs.map((input) => ({ ...input }))
  const steps = file.steps.map((step, at) => {
    const handOff = file.handOffs[at]
    const said = [...(handOff?.role === undefined ? [] : [handOff.role]), ...(handOff?.check === true ? [CHECKS_THE_WORK] : [])]
    return `## Step ${String(at + 1)}${said.map((part) => ` · ${part}`).join('')}\n\n${step}\n`
  })
  return `---\n${stringifyYaml(top, { lineWidth: 0 })}---\n\n# ${file.name}\n\n${steps.join('\n')}`
}

/** Whether this text is a routine written as Markdown, rather than the JSON file. */
const isMarkdown = (text: string): boolean => /^---\r?\n/.test(text)

/** The Markdown read into the fields the JSON file has, for the same checks; or why it cannot be. */
function markdownFields(text: string): { readonly ok: true; readonly value: Record<string, unknown> } | { readonly ok: false; readonly message: string } {
  const refuse = (message: string) => ({ ok: false as const, message })
  const lines = text.replace(/\r\n?/g, '\n').split('\n')
  const close = lines.indexOf('---', 1)
  if (close < 0) return refuse('That routine file opens its settings with --- and never closes them.')
  let top: unknown
  try {
    // Every value read as text, so a default of 5 is the text 5 and nothing is read as something else.
    top = parseYaml(lines.slice(1, close).join('\n'), { schema: 'failsafe', maxAliasCount: 0 })
  } catch {
    return refuse('That routine file\'s settings, between its --- lines, are not readable.')
  }
  if (!isRecord(top)) return refuse('That routine file has no settings between its --- lines.')
  if (top.when !== undefined && top.when !== 'hook') return refuse('That routine file has a trigger Locust does not read.')
  const stray = Object.keys(top).find((key) => !(MARKDOWN_FIELDS as readonly string[]).includes(key))
  if (stray !== undefined) return refuse(`That routine file has a setting Locust does not read (${stray.slice(0, 40)}), so it was not opened.`)
  const version = top['locust-routine']
  if (version === undefined) return refuse('That file is not a Locust routine: its settings do not start with locust-routine.')
  if (version !== String(ROUTINE_FILE_VERSION)) return refuse(`That routine file is version ${String(version).slice(0, 12)}; this Locust reads version ${String(ROUTINE_FILE_VERSION)}.`)

  const body = lines.slice(close + 1)
  const titleAt = body.findIndex((line) => line.trim().length > 0)
  const title = titleAt < 0 ? undefined : /^# (.+)$/.exec(body[titleAt]!)
  if (title === null || title === undefined) return refuse('That routine file needs its name as a title, a line starting with # , after its settings.')
  const steps: string[] = []
  const handOffs: Record<string, unknown>[] = []
  let current: string[] | undefined
  const finish = (): void => {
    if (current === undefined) return
    while (current.length > 0 && current[0]!.trim().length === 0) current.shift()
    while (current.length > 0 && current[current.length - 1]!.trim().length === 0) current.pop()
    steps.push(current.join('\n'))
  }
  for (const line of body.slice(titleAt + 1)) {
    const heading = STEP_HEADING.exec(line)
    // Only the next step's heading starts it, so a step may hold headings of its own.
    if (heading !== null && Number(heading[1]) === steps.length + (current === undefined ? 1 : 2)) {
      finish()
      current = []
      const said = heading[2]!.split(' · ').slice(1).map((part) => part.trim())
      const check = said[said.length - 1] === CHECKS_THE_WORK
      const role = check ? said.slice(0, -1) : said
      if (role.length > 1) return refuse(`Step ${heading[1]!}'s heading names more than one teammate.`)
      handOffs.push({ ...(role[0] === undefined ? {} : { role: role[0] }), ...(check ? { check: true } : {}) })
    } else if (current === undefined) {
      if (line.trim().length > 0) return refuse('That routine file has text between its title and ## Step 1. Put it in a step, or take it out.')
    } else {
      current.push(line)
    }
  }
  finish()
  if (steps.length === 0) return refuse('That routine file has no steps: each starts with a line ## Step 1, ## Step 2 and so on.')
  const inputs = top.inputs === undefined ? [] : top.inputs
  return {
    ok: true,
    value: {
      format: ROUTINE_FILE_FORMAT,
      version: ROUTINE_FILE_VERSION,
      name: title[1]!,
      ...(top.when === 'hook' ? { schedule: { kind: 'hook' } } : {}),
      steps,
      inputs: Array.isArray(inputs) ? inputs.map((input: unknown) => isRecord(input) && typeof input.required === 'string' && ['true', 'false'].includes(input.required) ? { ...input, required: input.required === 'true' } : input) : inputs,
      handOffs,
      route: top['runs-on'] === undefined ? {} : { runtime: top['runs-on'] },
      connectors: top.connectors === undefined ? [] : top.connectors
    }
  }
}

/** What an export writes: the Markdown, when it carries the routine exactly, and the JSON file when it would not. */
export function routineExportText(file: RoutineFile): { readonly text: string; readonly markdown: boolean } {
  const markdown = routineMarkdown(file)
  const back = parseRoutineFile(markdown)
  const plain = (one: RoutineFile): unknown => JSON.parse(JSON.stringify(one))
  return back.ok && isDeepStrictEqual(plain(back.file), plain(file)) ? { text: markdown, markdown: true } : { text: routineFileText(file), markdown: false }
}

/** `<name>.routine.md` or `<name>.locust-routine.json`, with a name any file system takes. */
export function routineFileName(name: string, markdown = false): string {
  // eslint-disable-next-line no-control-regex
  const plain = name.replace(/[<>:"/\\|?*\u0000-\u001f]/g, ' ').replace(/\s+/g, ' ').trim().replace(/[. ]+$/, '').slice(0, 60)
  return `${plain.length > 0 ? plain : 'routine'}${markdown ? '.routine.md' : '.locust-routine.json'}`
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
// eslint-disable-next-line no-control-regex
const ONE_LINE = (value: unknown, max: number): value is string => typeof value === 'string' && value.trim().length > 0 && value.length <= max && !/[\u0000-\u001f\u007f]/.test(value)

export type RoutineFileRead = { readonly ok: true; readonly file: RoutineFile } | { readonly ok: false; readonly message: string }

/**
 * Read a routine file, refusing anything that is not exactly one: another
 * format, another version, a field this Locust does not keep, a step or an
 * input it would not save. Nothing here creates a routine.
 */
export function parseRoutineFile(text: string): RoutineFileRead {
  const refuse = (message: string): RoutineFileRead => ({ ok: false, message })
  if (Buffer.byteLength(text, 'utf8') > MAX_ROUTINE_FILE_BYTES) return refuse('That file is too large to be a routine.')
  const plain = text.replace(/^\uFEFF/, '')
  let value: unknown
  if (isMarkdown(plain)) {
    const read = markdownFields(plain)
    if (!read.ok) return refuse(read.message)
    value = read.value
  } else {
    try {
      value = JSON.parse(plain)
    } catch {
      return refuse('That file is not a routine: it is neither a routine in Markdown nor readable JSON.')
    }
  }
  if (!isRecord(value)) return refuse('That file is not a routine.')
  if (value.format !== ROUTINE_FILE_FORMAT) {
    return refuse(typeof value.format === 'string' && value.format.length <= 40 ? `That file is a "${value.format}" file, not a Locust routine.` : 'That file is not a Locust routine.')
  }
  if (value.version !== ROUTINE_FILE_VERSION) {
    return refuse(typeof value.version === 'number' ? `That routine file is version ${String(value.version)}; this Locust reads version ${String(ROUTINE_FILE_VERSION)}.` : 'That routine file has no version this Locust reads.')
  }
  const stray = Object.keys(value).find((key) => !(ROUTINE_FILE_KEYS as readonly string[]).includes(key))
  if (stray !== undefined) return refuse(`That routine file has a field Locust does not read (${stray.slice(0, 40)}), so it was not opened.`)
  const missing = ROUTINE_FILE_KEYS.find((key) => key !== 'schedule' && !Object.hasOwn(value, key))
  if (missing !== undefined) return refuse(`That routine file is missing its ${missing} field.`)
  const { name, steps, inputs, handOffs, route, connectors } = value
  if (value.schedule !== undefined && (!isRecord(value.schedule) || value.schedule.kind !== 'hook' || Object.keys(value.schedule).length !== 1)) return refuse('That routine file has a trigger Locust does not read.')
  if (!validRoutineName(name)) return refuse('That routine has no usable name.')
  if (!validSteps(steps)) return refuse('That routine\'s steps are not ones Locust can keep: 1 to 12 steps, none empty.')
  const tooLong = stepsTooLongToSave(steps)
  if (tooLong !== undefined) return refuse(tooLong)
  const asked = inputs === undefined ? [] : inputs
  const saying = inputsRefusal(asked, steps)
  if (saying !== undefined) return refuse(`That routine's inputs are not ones Locust can keep. ${saying}`)
  if (!Array.isArray(handOffs) || handOffs.length !== steps.length
    || !handOffs.every((entry) => isRecord(entry) && Object.keys(entry).every((key) => key === 'role' || key === 'check') && (entry.role === undefined || ONE_LINE(entry.role, 60)) && (entry.check === undefined || entry.check === true))) {
    return refuse('That routine\'s hand-offs do not line up with its steps.')
  }
  if (!isRecord(route) || Object.keys(route).some((key) => key !== 'runtime') || (route.runtime !== undefined && !(typeof route.runtime === 'string' && /^[a-z0-9-]{1,40}$/.test(route.runtime)))) {
    return refuse('That routine\'s route is not one Locust can read.')
  }
  if (!Array.isArray(connectors) || connectors.length > MAX_CONNECTORS || !connectors.every((entry) => ONE_LINE(entry, 80))) {
    return refuse('That routine\'s list of connectors is not one Locust can read.')
  }
  return {
    ok: true,
    file: {
      format: ROUTINE_FILE_FORMAT,
      version: ROUTINE_FILE_VERSION,
      name: name.trim(),
      ...(value.schedule === undefined ? {} : { schedule: { kind: 'hook' as const } }),
      steps: [...steps],
      inputs: keptInputs(asked as readonly RoutineInput[]),
      handOffs: (handOffs as readonly { readonly role?: string; readonly check?: true }[]).map((entry) => ({ ...(entry.role === undefined ? {} : { role: entry.role }), ...(entry.check === true ? { check: true as const } : {}) })),
      route: route.runtime === undefined ? {} : { runtime: route.runtime as string },
      connectors: [...(connectors as readonly string[])]
    }
  }
}
