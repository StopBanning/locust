import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { BoardScreen, type BoardWait } from './components/BoardScreen.js'
import type { SidebarMission } from './components/Sidebar.js'

/**
 * A SCOUT CARD THAT NEEDS YOU IS ON THE BOARD. Urgency `now` joins the Needs
 * you column, above the conversations, with Send, Change it first and
 * Dismiss when Home would offer them. The rest of the card is what opens Home.
 */

const row = (over: Partial<SidebarMission> & { missionId: string }): SidebarMission =>
  ({ title: over.missionId, phase: 'completed', integrityIssueCount: 0, ...over })

const SCOUT = { teammateId: 'tm_scout', name: 'Scout', hue: 'teal', avatar: seedAvatar('tm_scout'), role: 'Custom', createdAt: '2026-10-12T00:00:00.000Z' } as unknown as PublicTeammate
const WREN = { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', avatar: seedAvatar('tm_wren'), role: 'Code & Migrations', createdAt: '2026-10-01T00:00:00.000Z' } as unknown as PublicTeammate

const scout = (over: Partial<Extract<BoardWait, { kind: 'scout' }>> = {}): Extract<BoardWait, { kind: 'scout' }> => ({
  kind: 'scout',
  cardId: 'card_now',
  said: 'Atlas has waited 25 minutes for your OK.',
  who: 'Scout',
  face: SCOUT,
  sendTo: 'Atlas',
  ...over
})

const screen = (waiting: readonly BoardWait[] = [], missions: readonly SidebarMission[] = [
  row({ missionId: 'asks', title: 'Rename the invoices', phase: 'running', lastAt: '2026-10-04T05:58:00.000Z' }),
  row({ missionId: 'old', title: 'Yesterday', phase: 'completed', lastAt: '2026-10-03T05:00:00.000Z' })
]): string =>
  renderToStaticMarkup(
    <BoardScreen
      missions={missions}
      rooms={[]}
      trashed={new Set()}
      facts={{ needsYou: new Set(['asks']) }}
      waitingFor={new Map([['asks', 'Run a command: git mv invoices old-invoices']])}
      teammates={[SCOUT, WREN]}
      missionOwners={{ asks: 'tm_wren' }}
      onOpen={() => undefined}
      waiting={waiting}
      onOpenScout={() => undefined}
      onSendScout={() => undefined}
      onChangeScout={() => undefined}
      onDismissScout={() => undefined}
    />
  )

describe('a Scout card on the Board', () => {
  it('sits above the conversation that is waiting, with Scout’s face and the actions Home gives a draft', () => {
    const html = screen([scout()])
    expect(html.indexOf('data-board-scout="card_now"')).toBeLessThan(html.indexOf('Rename the invoices'))
    expect(html.indexOf('Atlas has waited 25 minutes for your OK.')).toBeLessThan(html.indexOf('Rename the invoices'))
    expect(html).toContain('Scout')
    expect(html).toContain('>Send to Atlas<')
    expect(html).toContain('>Change it first<')
    expect(html).toContain('>Dismiss<')
    expect(html).toContain('2 waiting on you')
    expect(html).toContain('>2<')
  })

  it('offers Dismiss alone when the card has no draft, which is what Home does', () => {
    const { sendTo: _sendTo, ...plain } = scout()
    const html = screen([plain])
    expect(html).toContain('>Dismiss<')
    expect(html).not.toContain('Send to')
    expect(html).not.toContain('Change it first')
  })

  it('is a Needs you column of its own when no conversation is waiting, and the board is not quiet', () => {
    const html = screen(
      [scout({ sendTo: undefined })],
      [row({ missionId: 'old', title: 'Yesterday', phase: 'completed', lastAt: '2026-10-03T05:00:00.000Z' })]
    )
    expect(html).toContain('Needs you')
    expect(html).toContain('Atlas has waited 25 minutes for your OK.')
    expect(html).toContain('1 waiting on you')
    expect(html).not.toContain('Nothing is waiting on you and nothing is running.')
    expect(html).toContain('Yesterday')
  })

  it('leaves the board as it was when nothing of Scout’s is waiting (control)', () => {
    const html = screen([])
    expect(html).not.toContain('data-board-scout')
    expect(html).toContain('1 waiting on you')
    expect(html.indexOf('Rename the invoices')).toBeGreaterThan(html.indexOf('Needs you'))
  })
})

describe('the card fits the column', () => {
  const css = readFileSync(new URL('./shell.css', import.meta.url), 'utf8')
  const tokens = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8')
  const rule = css.slice(css.indexOf('.lc-boardwait {'), css.indexOf('.lc-boardsection--needs-you .lc-boardsection__title'))

  it('wraps its actions inside the column and takes its colour from both themes', () => {
    expect(rule).toContain('flex-wrap: wrap')
    expect(rule).toContain('max-width: 100%')
    expect(rule).toContain('overflow-wrap: anywhere')
    expect(rule).toContain('border: 1px solid var(--lc-amber-tint)')
    expect(rule).not.toMatch(/#[0-9a-fA-F]{3,8}/)
    expect(tokens.match(/--lc-amber-tint:/g)).toHaveLength(2)
  })
})

describe('the title bar opens Home, and the Board uses the same presses', () => {
  const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')

  it('sends a title-bar row to Home with that card in view, and the Board’s buttons are Home’s', () => {
    expect(app).toContain('const place = needsYouPlace(item)')
    expect(app).toContain("else if (place.to === 'home') showScoutOnHome(place.cardId)")
    expect(app).toContain('setScreen(\'workroom\')')
    expect(app).toContain('focusCardId: bringIntoView.cardId')
    expect(app).toContain('if (card?.draft !== undefined) sendScoutDraft(card.draft, cardId)')
    expect(app).toContain('if (card?.draft !== undefined) draftScoutIntoBox(card.draft, cardId)')
    expect(app).toContain('window.desktop?.scout?.dismiss(cardId)')
  })
})
