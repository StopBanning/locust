import { MAX_TEAMMATE_INSTRUCTIONS } from './ipc.js'

/** The same standing words on a teammate or a comparison column; empty explicitly means none. */
export function instructionsText(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const text = value.trim()
  return text.length <= MAX_TEAMMATE_INSTRUCTIONS && !text.includes('\0') ? text : undefined
}
