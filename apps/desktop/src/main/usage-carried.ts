import { readFile } from 'node:fs/promises'

import type { UsageTurn } from '../shared/usage.js'

/**
 * USAGE CARRIED FROM ANOTHER COMPUTER (0.849). Colin moved to a Mac and wanted his PC's Usage numbers there, and only
 * those: no teammates, no conversations, no spend per teammate. A numbers-only file in Locust's data folder, one turn
 * per line, is added to the turns the ledger holds wherever Usage is read. A line that is not a turn is left out; a
 * missing file is no turns. Nothing in it is words, a teammate or a path.
 */
export const USAGE_CARRIED_FILE = 'usage-carried.jsonl'

const count = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value) && value >= 0
const short = (value: unknown, most: number): value is string => typeof value === 'string' && value.length > 0 && value.length <= most

/** One carried turn, checked field by field, or undefined; what it does not name is not kept. */
export function carriedTurn(value: unknown): UsageTurn | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const row = value as Record<string, unknown>
  if (!short(row.at, 40) || !Number.isFinite(Date.parse(row.at))) return undefined
  if (!short(row.runtime, 40) || !/^[a-z0-9-]+$/.test(row.runtime) || !short(row.model, 200) || !short(row.conversation, 200)) return undefined
  const numbers = ['inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens', 'usd', 'premiumRequests'] as const
  if (numbers.some((key) => row[key] !== undefined && !count(row[key]))) return undefined
  if (row.plan !== undefined && row.plan !== true) return undefined
  return {
    at: row.at,
    runtime: row.runtime,
    model: row.model,
    conversation: row.conversation,
    ...Object.fromEntries(numbers.filter((key) => row[key] !== undefined).map((key) => [key, row[key]])),
    ...(row.plan === true ? { plan: true as const } : {})
  }
}

/** Every carried turn in the file at `path`; none when there is no file. */
export async function carriedUsage(path: string): Promise<readonly UsageTurn[]> {
  let text: string
  try {
    text = await readFile(path, 'utf8')
  } catch {
    return []
  }
  return text.split('\n').flatMap((line) => {
    if (line.trim().length === 0) return []
    try {
      const turn = carriedTurn(JSON.parse(line))
      return turn === undefined ? [] : [turn]
    } catch {
      return []
    }
  })
}
