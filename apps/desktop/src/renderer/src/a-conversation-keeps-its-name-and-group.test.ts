import { describe, expect, it } from 'vitest'

import { conversationKeys, conversationTitleFor, heldFor } from './conversationList.js'
import type { SidebarMission } from './components/Sidebar.js'

/**
 * A conversation keeps its name and its group across its own turns.
 *
 * Colin, 2026-09-16: *"ive named a conversation 'code' and moved it to a
 * group named locust twice and it keeps disappearing"* — then, precisely:
 * *"it actually didnt disappear it just renamed itself and left the group."*
 *
 * Two symptoms, one cause, and his ledger showed it plainly:
 * `mission_77ef58ad` carries `continuesFrom`, so it is a FOLLOW-UP TURN.
 *
 * Which id the app uses for a conversation depends on when you ask. A live
 * run does not always know its earlier turns, so its row is keyed by the turn
 * itself; once the ledger is re-read the same conversation is keyed by its
 * ROOT. So a name typed — or a group chosen — during the live turn was stored
 * against that turn's id and then looked for under the root's, where nothing
 * was. The name fell back to the derived title and the membership became
 * invisible.
 *
 * Writing still uses the root when it is known, because that is the stable
 * identity. Reading tolerates whichever id was current when the person acted.
 */

const row = (over: Partial<SidebarMission> & { missionId: string }): SidebarMission =>
  ({ title: 't', phase: 'completed', integrityIssueCount: 0, ...over }) as SidebarMission

describe('the ids a conversation has worn', () => {
  it('includes the root, the row and every turn', () => {
    const keys = conversationKeys(row({ missionId: 'leaf', rootId: 'root', memberIds: ['root', 'mid', 'leaf'] }))
    expect(keys).toContain('root')
    expect(keys).toContain('mid')
    expect(keys).toContain('leaf')
  })

  it('lists each one once', () => {
    expect(conversationKeys(row({ missionId: 'a', rootId: 'a', memberIds: ['a'] }))).toEqual(['a'])
  })

  it('prefers the root, which is the stable identity', () => {
    expect(conversationKeys(row({ missionId: 'leaf', rootId: 'root' }))[0]).toBe('root')
  })
})

describe('finding what was saved', () => {
  it('finds a name stored against a follow-up turn', () => {
    // Exactly Colin's case: typed while the row was the live turn.
    const held = heldFor(row({ missionId: 'leaf', rootId: 'root', memberIds: ['root', 'leaf'] }), { leaf: 'code' })
    expect(held).toBe('code')
  })

  it('finds a group stored against the root', () => {
    const held = heldFor(row({ missionId: 'leaf', rootId: 'root', memberIds: ['root', 'leaf'] }), {
      root: { groupId: 'grp_locust' }
    })
    expect(held?.groupId).toBe('grp_locust')
  })

  it('prefers the root when both were written, so the newer identity wins', () => {
    const held = heldFor(row({ missionId: 'leaf', rootId: 'root', memberIds: ['root', 'leaf'] }), {
      root: 'from the root',
      leaf: 'from the leaf'
    })
    expect(held).toBe('from the root')
  })

  it('answers undefined when nothing was saved for any of them', () => {
    expect(heldFor(row({ missionId: 'leaf', rootId: 'root' }), { other: 'x' })).toBeUndefined()
  })

  it('works for a single-turn conversation with no root recorded', () => {
    expect(heldFor(row({ missionId: 'only' }), { only: 'named' })).toBe('named')
  })
})

describe("a conversation's header says the name its sidebar row says (0.864)", () => {
  // Colin's 2026-11-03: the sidebar read "handoff-export-polish.md" and the header of the same conversation, which
  // belongs to no teammate, read "continue mb" -- the newest turn's prompt.
  const rows = [
    row({ missionId: 'leaf', rootId: 'root', memberIds: ['root', 'mid', 'leaf'], title: 'handoff-export-polish.md' }),
    row({ missionId: 'other', title: 'Something else' })
  ]

  it('finds the row by any turn the conversation has had', () => {
    expect(conversationTitleFor(rows, 'leaf')).toBe('handoff-export-polish.md')
    expect(conversationTitleFor(rows, 'mid')).toBe('handoff-export-polish.md')
    expect(conversationTitleFor(rows, 'root')).toBe('handoff-export-polish.md')
  })

  it('says nothing for a run no row holds, so the header falls back to the prompt', () => {
    expect(conversationTitleFor(rows, 'pending:3')).toBeUndefined()
    expect(conversationTitleFor(rows, undefined)).toBeUndefined()
  })

  it('the header reads it before the prompt, after a teammate', async () => {
    const { readFileSync } = await import('node:fs')
    const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
    expect(app).toContain('{missionOwner?.name ?? shownConversationTitle ?? missionTitle(shownPrompt(liveRun))}')
  })
})
