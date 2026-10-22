import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

/*
 * SCOUT'S LOOK IS SEALED (0.824, Grok's review #4). What a sealed run gets is pinned where it is made
 * (codex-mission.test.ts, a-sealed-run-uses-no-tool.test.ts); this pins the one line that asks for it. Without it every
 * look would be an ordinary Ask turn again -- tools, the web and the memory brief -- and no other test would notice.
 */
const INDEX = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')

describe('Scout asks for a sealed look', () => {
  it('starts every look in its own slot, sealed', () => {
    expect(INDEX).toContain('{ key: `scout:${teammateId}:${chosen}`, sealed: true }')
  })

  it('on the mode that reads and changes nothing', () => {
    const call = INDEX.slice(INDEX.indexOf('ask: async (teammateId, prompt, model) => {'), INDEX.indexOf('sealed: true }'))
    expect(call).toContain("'ask',")
  })
})
