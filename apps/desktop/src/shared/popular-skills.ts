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
 * above: it is refreshed by hand with a release, never fetched.
 */
export const TRENDING_SKILLS_AS_OF = '2026-10-31'
export const TRENDING_SKILLS = [
  { source: 't3dotgg/tstack', name: 'tstack', description: 'The skills Theo uses every day: file, babysit and review pull requests.', stars: 2499, license: 'MIT', since: '2026-10-24' },
  { source: 'heydamir/unslop', name: 'Unslop', description: 'Turns an AI-looking interface into a designed one.', stars: 2547, license: 'CC-BY-4.0', since: '2026-10-29' },
  { source: 'morganlinton/BeautifulRust', name: 'Beautiful Rust', description: 'Write, review and port idiomatic Rust.', stars: 1598, license: 'MIT', since: '2026-10-23' },
  { source: 'daveebbelaar/bama', name: 'Build Any Mac App', description: 'Research, design, build and test native Mac apps in SwiftUI.', stars: 1015, license: 'MIT', since: '2026-10-24' }
] as const

/** A shelf click prepares every offered skill; a pasted link preserves the existing opt-in picks. */
export function skillPreviewPicks(preview: SkillLibraryPreview, all: boolean): ReadonlySet<string> {
  return new Set(preview.skills.filter((skill) => skill.held === undefined && (all || skill.keptAt !== undefined)).map((skill) => skill.name))
}
