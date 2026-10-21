import { instructionsText } from './teammate-instructions.js'
import type { CompareRoute, PublicCompare } from './compare.js'
import type { PublicTeammate } from './ipc.js'

export const TUNE_DRAFT_CHANNEL = 'teammate:tune-drafts'
export interface TuneEdit { readonly instructions: string; readonly why: string }
export interface TuneDrafts { readonly how: TuneEdit; readonly knows: TuneEdit }
export const TUNE_FALLBACK = 'The drafts could not be written. These are copies of Current; you can write your own.'
export const TUNE_DRAFT_COST = 'Opening /tune starts one side turn on this model to draft the edits. The three comparison runs start only when you press Run all three.'

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
