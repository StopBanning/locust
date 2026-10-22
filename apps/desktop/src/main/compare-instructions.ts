import type { CompareRoute } from '../shared/compare.js'
import { SKILL_RUNTIME_ANY, takesSkills } from '../shared/skill-runtimes.js'
import { instructionsText } from '../shared/teammate-instructions.js'

/** Missing keeps the teammate's words; empty removes them for this column only. */
export function columnInstructions(current: string | undefined, override: string | undefined): string | undefined {
  return override === undefined ? current : override
}

/** Blind letters describe outputs, not the order the setups were written in. */
export function shuffledColumns<T>(routes: readonly T[], pick: (upper: number) => number): T[] {
  const columns = [...routes]
  for (let index = columns.length - 1; index > 0; index -= 1) {
    const other = pick(index + 1)
    ;[columns[index], columns[other]] = [columns[other]!, columns[index]!]
  }
  return columns
}

export function compareInstructionsRefusal(routes: readonly CompareRoute[], teammateId: string | undefined): string | undefined {
  if (routes.some((route) => route.instructions !== undefined && instructionsText(route.instructions) === undefined)) return 'Column instructions must be text of at most 4,000 characters, without a null character. Nothing was started.'
  if (routes.some((route) => route.instructions !== undefined) && teammateId === undefined) return 'Choose a teammate: column instructions replace that teammate’s own instructions. Nothing was started.'
  // Explicit trials may retain Current more than once: either drafting angle may recommend no change.
  const seen = new Set<string>()
  for (const route of routes) {
    const key = JSON.stringify([route.runtime, route.model, route.instructions])
    if (route.instructions === undefined && seen.has(key)) return 'Pick different models, or give each column its own teammate instructions. Nothing was started.'
    seen.add(key)
  }
  return undefined
}

/**
 * WHAT IT KNOWS, AS A SKILL (0.811): a skill is handed to a run of an agent that takes one (shared/skill-runtimes.ts:
 * Claude Code since 0.811, Codex and OpenCode since 0.813) and goes with a tune's column, which carries its own
 * instructions.
 */
export function compareSkillRefusal(routes: readonly CompareRoute[]): string | undefined {
  if (routes.some((route) => route.skill !== undefined && !takesSkills(route.runtime))) return `A skill reaches only a ${SKILL_RUNTIME_ANY} teammate for now. Nothing was started.`
  if (routes.some((route) => route.skill !== undefined && route.instructions === undefined)) return 'A skill goes with a tune’s column, beside its instructions. Nothing was started.'
  return undefined
}
