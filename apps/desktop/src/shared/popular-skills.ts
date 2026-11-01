import type { SkillLibraryPreview } from './ipc.js'

// GitHub repository metadata checked 2026-10-30. Stars are a dated snapshot, never a live request.
export const POPULAR_SKILLS_AS_OF = '2026-10-30'
export const POPULAR_SKILLS = [
  { source: 'mattpocock/skills', name: "Matt Pocock's skills", description: 'Practical workflows for building and improving software.', stars: 307555, license: 'MIT' },
  { source: 'obra/superpowers', name: 'Superpowers', description: 'Plan, test and debug with a structured development workflow.', stars: 309493, license: 'MIT' },
  { source: 'anthropics/skills', name: "Anthropic's skills", description: 'Create documents, designs and other working artifacts.', stars: 185396, license: 'Apache-2.0 + source-available' },
  { source: 'pbakaus/impeccable', name: 'Impeccable', description: 'Design vocabulary for polished interfaces and useful critiques.', stars: 97191, license: 'Apache-2.0' },
  { source: 'thedotmack/claude-mem', name: 'Claude-Mem', description: 'Memory workflows. Needs its own install to work.', stars: 106430, license: 'Apache-2.0' }
] as const

/**
 * NEW THIS WEEK (2026-10-31): skill packs made in the last fortnight that climbed fastest, found by a GitHub search of
 * new repositories by stars, each listed by Locust's own skill finder before it went here. A dated snapshot like the row
 * above: it is refreshed by hand with a release, never fetched. 2026-11-01: Unslop had gained 1,357 stars in a day,
 * and pixelfx (made 2026-10-30) and Dimensional Drawing had passed Beautiful Rust and Build Any Mac App, which slowed.
 */
export const TRENDING_SKILLS_AS_OF = '2026-11-01'
export const TRENDING_SKILLS = [
  { source: 'heydamir/unslop', name: 'Unslop', description: 'Turns an AI-looking interface into a designed one.', stars: 3904, license: 'CC-BY-4.0', since: '2026-10-29' },
  { source: 't3dotgg/tstack', name: 'tstack', description: 'The skills Theo uses every day: file, babysit and review pull requests.', stars: 2549, license: 'MIT', since: '2026-10-24' },
  { source: 'patch-gg/pixelfx', name: 'pixelfx', description: '244 pixel-art effects to play, recolor and export, and a skill that makes new ones.', stars: 1797, license: 'MIT', since: '2026-10-30' },
  { source: 'enesozturk/dimensional-drawing', name: 'Dimensional Drawing', description: 'Draws your interface parts as Apple-style spec sheets, still or moving.', stars: 1692, license: 'MIT', since: '2026-10-23' }
] as const

/** A shelf click prepares every offered skill; a pasted link preserves the existing opt-in picks. */
export function skillPreviewPicks(preview: SkillLibraryPreview, all: boolean): ReadonlySet<string> {
  return new Set(preview.skills.filter((skill) => skill.held === undefined && (all || skill.keptAt !== undefined)).map((skill) => skill.name))
}
