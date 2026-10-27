import { roleLabelOf } from './ipc.js'
import type { PublicTeammate, RoutineHandOff } from './ipc.js'

/**
 * WHO TAKES EACH STEP OF A CHAIN TEMPLATE (2026-10-05).
 *
 * A chain template names a ROLE for each step ("Research & Briefs", "Code &
 * Migrations", "Docs & QA"), because a template cannot know anyone's roster.
 * When a person starts one, this proposes a teammate for each role from the
 * roster they have. It is only a proposal: the editor shows it, step by step,
 * and the person changes any of it before the routine is saved.
 *
 * - A role with a teammate on the roster goes to that teammate (the first one,
 *   in roster order). A checker prefers a teammate who has not taken an earlier
 *   step, so the work is not checked by whoever did it, when there is another.
 * - A role nobody on the roster has goes to whoever took the step before it, so
 *   a roster of one -- or of teammates with other jobs -- still works: the
 *   steps go to them. A checker with no match goes to someone who has not
 *   taken a step yet, if there is such a teammate.
 * - A step marked ANOTHER AGENT (0.837, Council review) goes first to a
 *   teammate on an agent no earlier step ran on, so the same change is read by
 *   different AIs; with nobody on another agent, to a teammate who has not
 *   taken a step yet, and only then as above.
 * - An empty roster proposes nobody.
 */
export interface ChainStep {
  /** The role the template names for this step, as the file wrote it. */
  readonly role?: string | undefined
  /** This step checks the work. */
  readonly check?: boolean | undefined
  /** This step should go to a teammate on an agent no earlier step ran on, where the roster has one. */
  readonly another?: boolean | undefined
}

type Proposable = Pick<PublicTeammate, 'teammateId' | 'role' | 'roleTitle' | 'route'>

const same = (left: string, right: string): boolean => left.trim().toLowerCase() === right.trim().toLowerCase()

/** One teammate id per step, in step order; empty when there is no one to propose. */
export function proposeTeammates(steps: readonly ChainStep[], team: readonly Proposable[]): readonly string[] {
  if (team.length === 0) return []
  const picked: string[] = []
  for (const step of steps) {
    const checks = step.check === true
    const matching = step.role === undefined ? [] : team.filter((mate) => same(roleLabelOf(mate), step.role!))
    const fresh = (mates: readonly Proposable[]): Proposable | undefined => mates.find((mate) => !picked.includes(mate.teammateId))
    const used = new Set(picked.map((id) => team.find((mate) => mate.teammateId === id)?.route?.runtime))
    const elsewhere = (mates: readonly Proposable[]): Proposable | undefined => mates.find((mate) => !used.has(mate.route?.runtime))
    let chosen: Proposable | undefined = step.another === true ? elsewhere(matching) ?? elsewhere(team) ?? fresh(matching) ?? fresh(team) : undefined
    if (chosen === undefined && matching.length > 0) chosen = (checks ? fresh(matching) : undefined) ?? matching[0]
    else if (chosen === undefined && checks) chosen = fresh(team)
    if (chosen === undefined) {
      const before = picked[picked.length - 1]
      chosen = team.find((mate) => mate.teammateId === before) ?? team[0]
    }
    picked.push(chosen!.teammateId)
  }
  return picked
}

/**
 * The proposal as a routine stores it: the first step's teammate runs the
 * routine, every other step names its teammate only where it differs, and the
 * checking step is marked. `undefined` owner when there is no roster.
 */
export function proposedHandOffs(
  steps: readonly ChainStep[],
  team: readonly Proposable[]
): { readonly owner: string | undefined; readonly handOffs: readonly RoutineHandOff[] } {
  const picked = proposeTeammates(steps, team)
  const owner = picked[0]
  return {
    owner,
    handOffs: steps.map((step, at) => ({
      ...(picked[at] === undefined || picked[at] === owner ? {} : { teammateId: picked[at]! }),
      ...(step.check === true ? { check: true as const } : {})
    }))
  }
}
