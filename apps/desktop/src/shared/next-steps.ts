import { blocksOutsideCode, stripBlocksOutsideCode } from './protocolTags.js'

/**
 * SUGGESTED TASKS (2026-10-30): how a teammate offers the person the follow-ups its work turned up, as cards.
 *
 * Colin, of Claude Code's "2 suggested tasks" cards under a reply, each with Do it here and Start: "very cool feature
 * there, can we add that to locust as well?" A teammate ends its reply with one block:
 *
 *     <locust-next>
 *     Clean up old update files :: Delete the update images Locust keeps after installing, keeping the one waiting.
 *     </locust-next>
 *
 * One task a line, `title :: what to do and what done looks like`, at most MAX_NEXT_STEPS. The card says the title and
 * the brief; Do it here sends the brief as the next turn of this conversation, Start begins a new conversation with
 * the same teammate on it, and the cross puts it away. NOTHING RUNS until the person presses -- the block is words
 * on a card, and pressing is the person typing them -- so it needs none of the care a block that acts does. It is
 * still a protocol tag (protocolTags.ts), so a quoted reply cannot plant cards in another teammate's thread.
 *
 * Shared between main and renderer so the parser that draws the cards and the one that hides the block are one.
 */

export const NEXT_TAG = 'locust-next'
/** As Claude Code's cards: a few, or they bury the answer. */
export const MAX_NEXT_STEPS = 3
export const MAX_NEXT_TITLE = 80
export const MAX_NEXT_BRIEF = 600
/** The briefing's own placeholders, never a task: a free model copies the example's words. */
export const NEXT_PLACEHOLDERS: readonly string[] = ['<short title>', '<what to do, and what done looks like>']

const BLOCK = /<locust-next\s*>([\s\S]*?)<\/locust-next>/g

export interface NextStep {
  readonly title: string
  readonly brief: string
}

const bounded = (text: string, most: number): string => {
  const clean = text.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, '').replace(/\s+/g, ' ').trim()
  return clean.length <= most ? clean : `${clean.slice(0, most - 1).trimEnd()}…`
}

/** The tasks a reply suggests, in the order it wrote them: a title and a brief each, bounded, the placeholders left out. */
export function parseNextSteps(text: string): readonly NextStep[] {
  const steps: NextStep[] = []
  const seen = new Set<string>()
  for (const match of blocksOutsideCode(text, BLOCK)) {
    for (const line of (match[1] ?? '').split('\n')) {
      if (steps.length >= MAX_NEXT_STEPS) return steps
      const trimmed = line.trim().replace(/^[-*]\s+|^\d+[.)]\s+/, '')
      if (trimmed.length === 0) continue
      const at = trimmed.indexOf('::')
      const rawTitle = at < 0 ? trimmed : trimmed.slice(0, at)
      const rawBrief = at < 0 ? trimmed : trimmed.slice(at + 2)
      const title = bounded(rawTitle, MAX_NEXT_TITLE)
      const brief = bounded(rawBrief, MAX_NEXT_BRIEF)
      if (title.length === 0 || brief.length === 0) continue
      if (NEXT_PLACEHOLDERS.some((placeholder) => title.includes(placeholder) || brief.includes(placeholder))) continue
      const key = title.toLowerCase()
      if (seen.has(key)) continue
      seen.add(key)
      steps.push({ title, brief })
    }
  }
  return steps
}

/** The reply without its suggested tasks: they are drawn as cards under it, not as tags in the bubble. */
export function stripNextBlocks(text: string): string {
  return stripBlocksOutsideCode(text, BLOCK).replace(/\n{3,}/g, '\n\n').trimEnd()
}

/** What a started card is told: the brief, with the title it was offered under, so the turn knows its own task. */
export function nextStepPrompt(step: NextStep): string {
  return step.brief.toLowerCase().startsWith(step.title.toLowerCase()) ? step.brief : `${step.title}: ${step.brief}`
}
