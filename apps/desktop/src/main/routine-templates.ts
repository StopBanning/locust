/**
 * STARTER ROUTINES (0.615; the PRD's R16 and its section 7).
 *
 * "Ten starter routines shipped as files, 'Start from a template'": a new
 * person's Routines screen had nothing in it and no way to see what a routine
 * is for. The templates are ordinary routine files (routine-file.ts) shipped
 * beside the app in `resources/routines/`, read by the same reader an import
 * uses and previewed by the same preview, so a template is exactly what a file
 * from a colleague would be: nothing here can create a routine an import could
 * not. Every one is written for Ask -- it reads and answers, it changes no file
 * -- and names no route, no path and no connector; the import makes it an Ask
 * routine of the teammate chosen. The eleventh is the PRD's "Challenge an idea
 * before building it": an architect, then a checker.
 *
 * Three more are CHAINS (2026-10-05): fix a bug, build a feature, make it
 * faster. Each step names a ROLE, not a teammate -- a template cannot know
 * anyone's roster -- and the last step is marked as the checker. Locust
 * proposes a teammate for each role (shared/chain-proposal.ts) in the editor,
 * which is where a chain is opened: the person sees who takes which step and
 * can change it before anything is saved. They are the ones that change files,
 * so the editor opens them changing files, in a copy of the folder that waits
 * for Keep or Discard (`templateChangesFiles`).
 */
import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { RoutineTemplateInfo } from '../shared/ipc.js'
import { MAX_ROUTINE_FILE_BYTES, parseRoutineFile } from './routine-file.js'
import type { RoutineFileRead } from './routine-file.js'

export const TEMPLATE_SUFFIX = '.locust-routine.json'

/** The order they are offered in: the first thing a new person might ask first. */
export const TEMPLATE_ORDER = [
  'explain-this-project',
  'find-what-is-unfinished',
  'review-a-file',
  'council-review',
  'fix-a-bug',
  'build-a-feature',
  'make-it-faster',
  'polish-overnight',
  'plan-a-change',
  'challenge-an-idea',
  'write-a-status-update',
  'weekly-report-from-notes',
  'summarize-a-long-text',
  'draft-a-reply',
  'check-a-spreadsheet',
  'find-the-gaps-in-a-document'
] as const

/** One line each, in the person's words, for the list a template is chosen from. */
const WHAT_IT_DOES: Readonly<Record<string, string>> = {
  'explain-this-project': 'What this folder is, how it is laid out and how to run it, for someone new.',
  'find-what-is-unfinished': 'Every TODO and half-done part, grouped by how much it matters.',
  'review-a-file': 'A careful review of one file: bugs, unclear names, missing checks.',
  'council-review': 'Three teammates, each on a different AI where your team has one, review the same change, and the third keeps what two would stand behind.',
  'fix-a-bug': 'Find the cause, fix it with the smallest change, and have the fix checked where it broke.',
  'build-a-feature': 'A plan, the build, and a review of the build against the plan.',
  'make-it-faster': 'Measure where the time goes, change the slowest part, then measure again.',
  // The overnight polish teammate (2026-10-12), from Grok Bot's nightly team bot: set it to run each night.
  'polish-overnight': 'Set it to run each night: it runs the tests, fixes what fails in a copy for you to keep or discard, and leaves a report for the morning.',
  'plan-a-change': 'A step-by-step plan for a change, then the same plan read back sceptically.',
  'challenge-an-idea': 'An architect lists what the idea is missing, plans it, and a checker judges the plan.',
  'write-a-status-update': 'A short update on this project for your team, manager or client.',
  'weekly-report-from-notes': 'Your notes as Done, In progress, Blocked and Next week.',
  'summarize-a-long-text': 'A two-minute summary of a long text, with its decisions and open questions.',
  'draft-a-reply': 'A short reply to a message, in the tone you choose.',
  'check-a-spreadsheet': 'Missing values, duplicates and odd numbers in a CSV, and whether to trust it.',
  'find-the-gaps-in-a-document': 'Unsupported claims, missing steps and places a reader would get stuck.'
}

/** The chains that work on the folder's files rather than only reading it: they open set to change files, in a copy. */
const CHANGES_FILES: ReadonlySet<string> = new Set(['fix-a-bug', 'build-a-feature', 'make-it-faster', 'polish-overnight'])

export const templateChangesFiles = (id: string): boolean => CHANGES_FILES.has(id)

/*
 * COUNCIL REVIEW (0.837), from tstack's council (github.com/t3dotgg/tstack, MIT, read 2026-10-27): models from
 * different providers review one change until they agree. Locust's take is a chain: the steps marked here go to a
 * teammate on an agent no earlier step ran on, where the roster has one (shared/chain-proposal.ts), so the same change
 * is read by different AIs and the last of them settles it. A hint for the proposal only, never part of the file.
 */
const ON_ANOTHER_AGENT: Readonly<Record<string, readonly boolean[]>> = { 'council-review': [false, true, true] }

/** Which steps of a template go to a teammate on an agent no earlier step used; undefined when none do. */
export const templateOnAnotherAgent = (id: string): readonly boolean[] | undefined => ON_ANOTHER_AGENT[id]

/** As the window lists it (shared/ipc.ts). */
export type RoutineTemplate = RoutineTemplateInfo

const rank = (id: string): number => {
  const at = (TEMPLATE_ORDER as readonly string[]).indexOf(id)
  return at < 0 ? TEMPLATE_ORDER.length : at
}

/** Every template in `directory` that reads as a routine file, in TEMPLATE_ORDER; one that does not is left out and named. */
export async function listRoutineTemplates(directory: string): Promise<{ readonly templates: readonly RoutineTemplate[]; readonly refused: readonly string[] }> {
  const names = (await readdir(directory).catch(() => [] as string[])).filter((name) => name.endsWith(TEMPLATE_SUFFIX))
  const templates: RoutineTemplate[] = []
  const refused: string[] = []
  for (const name of names) {
    const id = name.slice(0, -TEMPLATE_SUFFIX.length)
    const read = await readTemplateFile(directory, name)
    if (!read.ok) {
      refused.push(`${name}: ${read.message}`)
      continue
    }
    templates.push({
      id,
      name: read.file.name,
      summary: WHAT_IT_DOES[id] ?? read.file.steps[0]!.split(/(?<=[.!?])\s/)[0]!.slice(0, 160),
      steps: read.file.steps.length,
      asks: read.file.inputs.map((input) => input.label),
      ...(read.file.handOffs.some((entry) => entry.check === true) ? { chain: true as const } : {})
    })
  }
  templates.sort((a, b) => rank(a.id) - rank(b.id) || a.name.localeCompare(b.name))
  return { templates, refused }
}

async function readTemplateFile(directory: string, name: string): Promise<RoutineFileRead> {
  try {
    const text = await readFile(join(directory, name), 'utf8')
    if (Buffer.byteLength(text, 'utf8') > MAX_ROUTINE_FILE_BYTES) return { ok: false, message: 'That template is too large to be a routine.' }
    return parseRoutineFile(text)
  } catch {
    return { ok: false, message: 'That template could not be read.' }
  }
}

/** One template by its id, only if it is one of those listed: an id is never a path. */
export async function readRoutineTemplate(directory: string, id: unknown): Promise<RoutineFileRead> {
  if (typeof id !== 'string' || !/^[a-z0-9-]{1,80}$/.test(id)) return { ok: false, message: 'That template is not one Locust ships.' }
  const names = (await readdir(directory).catch(() => [] as string[])).filter((name) => name.endsWith(TEMPLATE_SUFFIX))
  if (!names.includes(`${id}${TEMPLATE_SUFFIX}`)) return { ok: false, message: 'That template is not one Locust ships.' }
  return readTemplateFile(directory, `${id}${TEMPLATE_SUFFIX}`)
}
