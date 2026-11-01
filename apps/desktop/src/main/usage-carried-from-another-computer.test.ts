import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

import { usageSummary } from '../shared/usage.js'
import { carriedTurn, carriedUsage, USAGE_CARRIED_FILE } from './usage-carried.js'

const turn = { at: '2026-10-01T10:00:00.000Z', runtime: 'claude', model: 'claude-opus-5-5', conversation: 'mission_a', inputTokens: 1200, outputTokens: 300, cacheReadTokens: 900, plan: true }

describe('usage carried from another computer', () => {
  it('keeps a turn of numbers, and nothing it does not name', () => {
    expect(carriedTurn({ ...turn, prompt: 'secret words', teammate: 'Wren' })).toEqual(turn)
  })

  it('leaves out a line that is not a turn', () => {
    expect(carriedTurn({ ...turn, at: 'yesterday' })).toBeUndefined()
    expect(carriedTurn({ ...turn, runtime: 'Claude Code' })).toBeUndefined()
    expect(carriedTurn({ ...turn, inputTokens: -1 })).toBeUndefined()
    expect(carriedTurn({ ...turn, usd: 'free' })).toBeUndefined()
    expect(carriedTurn({ ...turn, plan: 'yes' })).toBeUndefined()
    expect(carriedTurn(null)).toBeUndefined()
  })

  it('reads every good line of the file, and no file as none', async () => {
    const folder = await mkdtemp(join(tmpdir(), 'locust-usage-carried-'))
    expect(await carriedUsage(join(folder, USAGE_CARRIED_FILE))).toEqual([])
    await writeFile(join(folder, USAGE_CARRIED_FILE), [JSON.stringify(turn), 'not json', JSON.stringify({ ...turn, at: 'x' }), '', JSON.stringify({ ...turn, conversation: 'mission_b' })].join('\n'), 'utf8')
    const carried = await carriedUsage(join(folder, USAGE_CARRIED_FILE))
    expect(carried).toHaveLength(2)
    expect(usageSummary(carried, 'all', new Date('2026-10-02T00:00:00.000Z')).turns).toBe(2)
  })

  it('is added to the ledger where Usage is read', () => {
    const main = readFileSync(fileURLToPath(new URL('./index.ts', import.meta.url)), 'utf8')
    expect(main).toContain("usageSummary([...turns, ...(await carriedUsage(join(app.getPath('userData'), USAGE_CARRIED_FILE)))], range, new Date())")
  })
})
