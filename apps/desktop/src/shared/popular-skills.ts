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

/** A shelf click prepares every offered skill; a pasted link preserves the existing opt-in picks. */
export function skillPreviewPicks(preview: SkillLibraryPreview, all: boolean): ReadonlySet<string> {
  return new Set(preview.skills.filter((skill) => skill.held === undefined && (all || skill.keptAt !== undefined)).map((skill) => skill.name))
}
