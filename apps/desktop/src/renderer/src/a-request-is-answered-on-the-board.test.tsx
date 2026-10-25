import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { BoardScreen, type BoardAsk } from './components/BoardScreen.js'
import type { SidebarMission } from './components/Sidebar.js'

/**
 * A REQUEST IS ANSWERED ON THE BOARD (0.835, BoardScreen.tsx). A conversation in Needs you that wants to run a command
 * shows the command whole and where it would run, with Allow once and Deny right there. Anything else -- a file change,
 * a connector, a question -- says what it needs and opens the conversation, where the change or the question is shown
 * in full. Always never appears on the Board.
 */
const row = (over: Partial<SidebarMission> & { missionId: string }): SidebarMission =>
  ({ title: over.missionId, phase: 'running', integrityIssueCount: 0, ...over })
const WREN = { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', avatar: seedAvatar('tm_wren'), role: 'Code & Migrations', createdAt: '2026-10-01T00:00:00.000Z' } as unknown as PublicTeammate
const LONG = 'python -m pytest -q tests/test_prices.py tests/test_cart.py --maxfail=1 --disable-warnings -k "total and not slow and not network"'

const board = (asks: ReadonlyMap<string, BoardAsk>): string =>
  renderToStaticMarkup(
    <BoardScreen
      missions={[row({ missionId: 'm_run', title: 'Run the price tests' }), row({ missionId: 'm_edit', title: 'Fix the cart total' })]}
      rooms={[]}
      trashed={new Set()}
      facts={{ needsYou: new Set(['m_run', 'm_edit']) }}
      waitingFor={new Map([['m_run', 'Run a command'], ['m_edit', 'Change src/cart.py']])}
      teammates={[WREN]}
      missionOwners={{ m_run: 'tm_wren', m_edit: 'tm_wren' }}
      onOpen={() => undefined}
      asks={asks}
      onAnswer={() => undefined}
    />
  )

describe('a request answered on the Board', () => {
  it('shows a command whole, where it runs, and Allow once and Deny', () => {
    const html = board(new Map([['m_run', { approvalId: 'ap_1', kind: 'command', command: LONG, where: 'C:/acme-storefront' }]]))
    expect(html).toContain('data-board-ask="ap_1"')
    // Whole, never the 90-character line the title bar's list uses.
    expect(html).toContain(LONG.replaceAll('"', '&quot;'))
    expect(html).toContain('in C:/acme-storefront')
    expect(html).toContain('>Allow once<')
    expect(html).toContain('>Deny<')
    expect(html).toContain('wants to run')
    expect(html).not.toMatch(/Always/i)
  })

  it('opens the conversation for anything it cannot show whole', () => {
    const html = board(new Map<string, BoardAsk>([
      ['m_edit', { approvalId: 'ap_2', kind: 'file-change', where: 'C:/acme-storefront' }],
      ['m_run', { approvalId: 'ap_3', kind: 'question' }]
    ]))
    expect(html).toContain('>Review<')
    expect(html).toContain('>Answer<')
    expect(html).not.toContain('Allow once')
    expect(html).toContain('Change src/cart.py')
  })

  it('leaves a conversation without a request as the card it was', () => {
    const html = board(new Map())
    expect(html).not.toContain('data-board-ask')
    expect(html).toContain('class="lc-convcard"')
  })
})
