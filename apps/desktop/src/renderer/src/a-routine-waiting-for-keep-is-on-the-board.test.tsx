import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicRoutine, PublicTeammate } from '../../shared/ipc.js'
import { AutomationsScreen } from './components/AutomationsScreen.js'
import { BoardScreen, type BoardWait } from './components/BoardScreen.js'
import type { SidebarMission } from './components/Sidebar.js'

/**
 * A ROUTINE WAITING FOR KEEP IS ON THE BOARD. Its copy's changes sit in
 * Needs you, above the conversations, with Keep, Discard (asked first) and
 * Open the copy. The rest of the card opens Routines with that routine in view.
 */

const row = (over: Partial<SidebarMission> & { missionId: string }): SidebarMission =>
  ({ title: over.missionId, phase: 'completed', integrityIssueCount: 0, ...over })

const CEDAR = { teammateId: 'tm_cedar', name: 'Cedar', hue: 'lime', avatar: seedAvatar('tm_cedar'), role: 'Custom', createdAt: '2026-10-01T00:00:00.000Z' } as unknown as PublicTeammate

const routine = (): BoardWait => ({
  kind: 'routine',
  routineId: 'rt_inbox',
  name: 'Morning digest',
  line: 'Waiting for you',
  face: CEDAR
})

const scout = (): BoardWait => ({
  kind: 'scout',
  cardId: 'card_now',
  said: 'Atlas has waited 25 minutes for your OK.',
  who: 'Scout'
})

const screen = (waiting: readonly BoardWait[]): string =>
  renderToStaticMarkup(
    <BoardScreen
      missions={[
        row({ missionId: 'asks', title: 'Rename the invoices', phase: 'running', lastAt: '2026-10-04T05:58:00.000Z' })
      ]}
      rooms={[]}
      trashed={new Set()}
      facts={{ needsYou: new Set(['asks']) }}
      waitingFor={new Map([['asks', 'Which folder?']])}
      teammates={[CEDAR]}
      missionOwners={{ asks: 'tm_cedar' }}
      onOpen={() => undefined}
      waiting={waiting}
      onOpenRoutine={() => undefined}
      onSettleRoutine={() => undefined}
    />
  )

describe('a routine waiting for Keep or Discard, on the Board', () => {
  it('sits above the conversation, with Cedar’s face, and Keep, Discard and Open the copy', () => {
    const html = screen([scout(), routine()])
    expect(html.indexOf('data-board-scout="card_now"')).toBeLessThan(html.indexOf('data-board-routine="rt_inbox"'))
    expect(html.indexOf('Morning digest')).toBeLessThan(html.indexOf('Rename the invoices'))
    expect(html).toContain('Waiting for you')
    expect(html).toContain('Cedar')
    expect(html).toContain('>Keep<')
    expect(html).toContain('>Discard<')
    expect(html).toContain('aria-label="Discard the changes from Morning digest"')
    expect(html).toContain('>Open the copy<')
    expect(html).not.toContain('Discard for good?')
    expect(html).toContain('3 waiting on you')
  })

  it('leaves the board as it was when no routine is waiting (control)', () => {
    const html = screen([])
    expect(html).not.toContain('data-board-routine')
    expect(html).not.toContain('>Keep<')
    expect(html).toContain('Rename the invoices')
  })
})

describe('opening it lands on that routine', () => {
  const saved = {
    routineId: 'rt_inbox',
    name: 'Morning digest',
    teammateId: 'tm_cedar',
    route: { runtime: 'opencode', model: 'account-default', mode: 'ask' },
    steps: ['Write the digest.'],
    learnedFrom: [],
    createdAt: '2026-10-01T00:00:00.000Z',
    runs: 1
  } as unknown as PublicRoutine

  it('marks that row so Routines can bring it into view', () => {
    const html = renderToStaticMarkup(
      <AutomationsScreen
        routines={[saved, { ...saved, routineId: 'rt_other', name: 'Other' }]}
        teammates={[CEDAR]}
        routineStepByTeammate={{}}
        onRunRoutine={() => undefined}
        onEditRoutine={() => undefined}
        onRemoveRoutine={() => undefined}
        notice={undefined}
        onDismissNotice={() => undefined}
        focusRoutineId="rt_inbox"
      />
    )
    expect(html).toContain('data-routine="rt_inbox"')
    expect(html).toContain('data-bring-into-view')
    expect(html.indexOf('data-routine="rt_inbox"')).toBeLessThan(html.indexOf('data-bring-into-view'))
    expect(html).toContain('data-routine="rt_other"')
    expect(html.split('data-bring-into-view').length).toBe(2)
  })

  it('is the same Keep the Routines screen already uses', () => {
    const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
    expect(app).toContain("else if (place.to === 'routines') showRoutine(place.routineId)")
    expect(app).toContain("setScreen('automations')")
    expect(app).toContain('focusRoutineId: bringIntoView.routineId')
    expect(app).toContain('onSettleRoutine={settleRoutine}')
  })
})
