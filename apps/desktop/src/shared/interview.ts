import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { MAX_TEAMMATE_INSTRUCTIONS } from './ipc.js'
import { joinMessageFragments } from './messageFragments.js'
import { blocksOutsideCode } from './protocolTags.js'

/**
 * A NEW TEAMMATE INTERVIEWS YOU (0.831, the Rakazo scrub's second take). An empty Instructions box asks a person to
 * know, before a teammate has done anything, how to brief it. Rakazo's bots ask instead. So the dialog offers "Let it
 * interview you": the teammate's first conversation opens with the person's ask to be interviewed, the teammate asks a
 * few short questions one at a time, and then writes its own standing instructions in a `locust-instructions` block.
 * Locust draws a card under that answer with the words in a box the person can change, and Save -- nothing is kept
 * until they press it, and a block anywhere but an interview of the person's own making is only words.
 */
export const INSTRUCTIONS_TAG = 'locust-instructions'

/** How every interview begins: the person's own ask, as the dialog sends it. A conversation that begins so is one. */
export const INTERVIEW_OPENING = 'Interview me so you can write your own standing instructions.'

/** The person's first message of an interview: what is asked, and how the answer must come back. */
export function interviewPrompt(name: string, role: string): string {
  return [
    INTERVIEW_OPENING,
    `You are ${name}, ${role}, a teammate in Locust. Ask me one short question at a time and wait for my answer: what I want from you, the work you will do, how I like answers, and what you must never do. Five questions at most.`,
    // MEASURED 2026-10-23 (drive-a-new-teammate-interviews-you, the free OpenCode model): told only to ask, it made a
    // plan, read the folder and began the work itself, and one turn ran past four minutes. Said outright, it asks.
    'Only ask: use no tool, read no file and make no plan while you interview me, and do not start the work itself.',
    `When you know enough, or when I say I am done, write your standing instructions to yourself, in the second person, under ${String(Math.floor(MAX_TEAMMATE_INSTRUCTIONS / 2))} characters, inside <${INSTRUCTIONS_TAG}> and </${INSTRUCTIONS_TAG}> on lines of their own. Put no password, key or other secret in them.`
  ].join('\n\n')
}

/** Whether a conversation that began with these words is an interview. */
export function isInterview(firstPrompt: string | undefined): boolean {
  return firstPrompt !== undefined && firstPrompt.trimStart().startsWith(INTERVIEW_OPENING)
}

const BLOCK = new RegExp(`<${INSTRUCTIONS_TAG}\\s*>([\\s\\S]*?)</${INSTRUCTIONS_TAG}>`, 'g')

/** The last instructions block outside code in a message, trimmed and bounded; nothing when there is none. */
export function instructionsBlock(text: string): string | undefined {
  let found: string | undefined
  for (const block of blocksOutsideCode(text, BLOCK)) found = block[1]
  const words = found?.trim()
  if (words === undefined || words.length === 0) return undefined
  return words.length > MAX_TEAMMATE_INSTRUCTIONS ? words.slice(0, MAX_TEAMMATE_INSTRUCTIONS).trimEnd() : words
}

/** The instructions a finished turn wrote, read from its last whole message. */
export function instructionsIn(events: readonly NormalizedRuntimeEvent[]): string | undefined {
  const final = joinMessageFragments(events).findLast((event) => event.type === 'message.delta' && event.payload.final)
  return final?.type === 'message.delta' ? instructionsBlock(final.payload.text) : undefined
}
