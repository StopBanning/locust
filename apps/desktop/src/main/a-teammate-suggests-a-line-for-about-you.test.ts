import { describe, expect, it } from 'vitest'

import { aboutYouSection, parsedAboutYouSuggestions, withSuggestion } from '../shared/about-you.js'
import type { CodexMissionUpdate, PublicTeammate, WorkspaceSettings } from '../shared/ipc.js'
import { memorySection, parseMemoryBlocks } from '../shared/memory.js'
import { createMemoryReader } from './memory-reader.js'
import type { MemoryReaderOptions } from './memory-reader.js'

/**
 * A TEAMMATE SUGGESTS A LINE FOR ABOUT YOU; THE PERSON DECIDES (0.424).
 *
 * The second half of the Hindsight "mental model": a teammate that learns
 * something lasting about how the person works puts one sentence to them,
 * `about you :: ...` in its memory block. It waits on the Memory screen
 * whatever the memory mode -- the note is the person's -- and is added only
 * when they press Add. drive-about-you-suggestion drives it packaged.
 */
const SETTINGS: WorkspaceSettings = { relay: true, relayHopCap: 6, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' }

function harness(reply: string, mode: WorkspaceSettings['memoryMode']) {
  const added: string[] = []
  const suggested: [string, string][] = []
  const updates: CodexMissionUpdate[] = []
  const options: MemoryReaderOptions = {
    memories: {
      add: async (memory) => {
        added.push(String(memory.text))
        return { memory: { memoryId: 'mem_1', text: String(memory.text), scope: 'workspace', workspaceId: 'ws', workspaceName: 'shop', by: memory.by, createdAt: 'now', status: memory.status, enabled: true }, created: true }
      },
      forget: async () => ({ removed: [], refusal: 'nothing-matched' as const, candidates: [] }),
      proposeTidy: async () => ({ proposed: 0, refused: [] })
    },
    ledger: {
      getMission: async () =>
        ({ metadata: { missionId: 'mission_1', runId: 'run_1', workspaceId: 'ws' }, phase: 'completed', events: [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: reply, final: true } }] }) as never
    },
    teammates: {
      list: async () => [{ teammateId: 'tm_ada', name: 'Ada' } as PublicTeammate],
      missionOwners: async (): Promise<Readonly<Record<string, string>>> => ({ mission_1: 'tm_ada' }),
      readSettings: async () => ({ ...SETTINGS, memoryMode: mode })
    },
    workspaceName: 'shop',
    notify: (update) => {
      updates.push(update)
    },
    suggestAboutYou: async (text, by) => {
      suggested.push([text, by])
      return true
    }
  }
  return { added, suggested, updates, reader: createMemoryReader(options) }
}

const REPLY = 'Done.\n<locust-memory>\nabout you :: Prefers a short summary before any detail.\nremember :: Tests run with pnpm test.\n</locust-memory>'

describe('the block', () => {
  it('reads `about you ::` as a suggestion for the note, not a memory', () => {
    expect(parseMemoryBlocks(REPLY)).toEqual([
      { kind: 'about-you', text: 'Prefers a short summary before any detail.' },
      { kind: 'remember', scope: 'workspace', text: 'Tests run with pnpm test.' }
    ])
  })

  it('is taught in the note’s own paragraph and in memory’s rules', () => {
    expect(aboutYouSection('Keep it short.')).toContain('about you :: Prefers a one-line summary before any detail.')
    expect(memorySection({ workspaceName: 'shop', memories: [], askFirst: false })).toContain('about you :: ')
  })
})

describe('the reader', () => {
  it('puts the line to the person, names who suggested it, and keeps the memory as the mode says', async () => {
    const h = harness(REPLY, 'auto')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.suggested).toEqual([['Prefers a short summary before any detail.', 'Ada']])
    expect(h.added).toEqual(['Tests run with pnpm test.'])
    expect(h.updates.find((u) => u.kind === 'memory-changed')).toMatchObject({ aboutYouSuggested: ['Prefers a short summary before any detail.'] })
  })

  it('with memory OFF, still puts the line to the person, and keeps no memory', async () => {
    const h = harness(REPLY, 'off')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.suggested).toEqual([['Prefers a short summary before any detail.', 'Ada']])
    expect(h.added).toEqual([])
  })

  it('tells the window about a suggestion that came alone, with memory off', async () => {
    const h = harness('OK.\n<locust-memory>\nabout you :: Prefers tables for numbers.\n</locust-memory>', 'off')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.updates.find((u) => u.kind === 'memory-changed')).toMatchObject({ aboutYouSuggested: ['Prefers tables for numbers.'] })
  })

  it('with memory off and no suggestion, does nothing at all', async () => {
    const h = harness('Done.\n<locust-memory>\nremember :: Tests run with pnpm test.\n</locust-memory>', 'off')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.suggested).toEqual([])
    expect(h.added).toEqual([])
    expect(h.updates).toEqual([])
  })
})

describe('the stored suggestions and the note', () => {
  it('keep only well-formed entries, the newest ten', () => {
    const entries = Array.from({ length: 12 }, (_, i) => ({ id: `ays_${String(i)}`, text: `line ${String(i)}`, by: 'Ada', at: '2026-09-28T01:00:00.000Z' }))
    const kept = parsedAboutYouSuggestions([...entries, { id: 'bad', text: 'x', by: 'Ada', at: 'now' }]) ?? []
    expect(kept).toHaveLength(10)
    expect(kept[0]!.text).toBe('line 2')
  })

  it('add a line at the end of the note, or make it the note', () => {
    expect(withSuggestion('Keep it short.', 'Diffs, not prose.')).toBe('Keep it short.\nDiffs, not prose.')
    expect(withSuggestion(undefined, 'Diffs, not prose.')).toBe('Diffs, not prose.')
  })
})

describe('a suggested line that holds a key (0.433)', () => {
  // Colin, 2026-09-28: personal details help a teammate understand you;
  // "maybe just passwords and api keys/stuff like that".
  const KEYED = 'OK.\n<locust-memory>\nabout you :: Uses the key sk-proj-abcdefghijklmnopqrstuvwxyz0123 for the billing API.\n</locust-memory>'

  it('never waits for the person, and the notice says why', async () => {
    const h = harness(KEYED, 'off')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.suggested).toEqual([])
    const said = JSON.stringify(h.updates)
    expect(said).toContain('an OpenAI API key')
    expect(said).toContain('Ada')
    expect(said).not.toContain('sk-proj-abcdefghijklmnopqrstuvwxyz0123')
  })

  it('while a personal detail is put to the person as before', async () => {
    const h = harness('OK.\n<locust-memory>\nabout you :: Lives in Fairhaven, has a daughter named Juniper, and prefers texts at 352-555-0142.\n</locust-memory>', 'off')
    await h.reader.onRunEnded({ missionId: 'mission_1' })
    expect(h.suggested).toEqual([['Lives in Fairhaven, has a daughter named Juniper, and prefers texts at 352-555-0142.', 'Ada']])
  })
})
