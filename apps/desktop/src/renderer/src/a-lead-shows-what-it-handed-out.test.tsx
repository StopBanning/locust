import type { ReactElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicPeerMessage } from '../../shared/ipc.js'
import { CallTodosContext, callTodoState, type CallTodoState } from './callTodos.js'
import { Thread } from './components/Thread.js'
import { peerRunFor } from './missionView.js'

/*
 * A LEAD SHOWS WHAT IT HANDED OUT (2026-10-25). The full peer exchange stays where it was; the card under each turn
 * says who has its asks and how they are going, using the call card's state and delivery's actual mission id.
 */
const share = (messageId: string, name: string, text: string): PublicPeerMessage => ({
  messageId, direction: 'posted', from: { teammateId: 'tm_bro', name: 'Bro', missionId: 'mis_lead' },
  to: { teammateId: `tm_${name.toLowerCase()}`, name }, text, at: '2026-10-25T12:00:00.000Z'
})
const fix = share('msg_fix', 'Casper', 'Fix the login test.\nKeep the existing sign-in behavior.')
const review = share('msg_review', 'Toaster', 'Review the migration')
const delivered = (message: PublicPeerMessage, missionId: string) => ({
  missionId, peerMessages: [{ ...message, direction: 'received' as const }]
})
const targets = [delivered(fix, 'mis_fix'), delivered(review, 'mis_review')]
const receivedIn = new Map([fix, review].map((message) => [message.messageId, peerRunFor(message.messageId, targets)!.missionId]))

function draw(messages: readonly PublicPeerMessage[], states: ReadonlyMap<string, CallTodoState> = new Map(), earlier = false, links = receivedIn): string {
  const thread: ReactElement = <Thread
    prompt={earlier ? 'What comes next?' : 'Please hand out the work'}
    onOpenPeerRun={(messageId) => links.has(messageId) ? () => undefined : undefined}
    earlierTurns={earlier ? [{ missionId: 'mis_lead', prompt: 'Please hand out the work', events: [], peerMessages: messages }] : []}
    events={[]} running={false} restoredMission={undefined} error={undefined} errorIsPersistence={false} startedAt={undefined}
    approvals={[]} onDecide={() => undefined} onAnswerQuestion={() => undefined} decidingIds={[]} cancelled={false} handoff={undefined}
    peers={{ self: undefined, teammates: [], messages: earlier ? [] : messages, notices: [] }}
  />
  return renderToStaticMarkup(<CallTodosContext.Provider value={{ states, receivedIn: links, open: () => undefined }}>{thread}</CallTodosContext.Provider>)
}

describe('a lead shows what it handed out', () => {
  it('lists two delegations with their live states and Open, retaining the full exchange', () => {
    const html = draw([fix, review], new Map([
      ['mis_fix', callTodoState({ runPhase: 'running', needsYou: false })],
      ['mis_review', callTodoState({ runPhase: 'running', needsYou: true })]
    ]))
    const card = html.match(/<section class="lc-calltodos"[\s\S]*?<\/section>/)?.[0] ?? ''
    expect(card).toContain('Handed out · 2 to-dos')
    expect(card).toContain('Casper')
    expect(card).toContain('Toaster')
    expect(card).toContain('title="Fix the login test.\nKeep the existing sign-in behavior."')
    expect(card).toContain('<span class="lc-status lc-status--blue">Working</span>')
    expect(card).toContain('<span class="lc-status lc-status--amber">Needs you</span>')
    expect(card.match(/>Open<\/button>/g)).toHaveLength(2)
    expect(card).toContain('data-call-todo="mis_fix"')
    expect(card).toContain('data-call-todo="mis_review"')
    expect(html.match(/class="lc-peer is-open"/g)).toHaveLength(2)
  })

  it('adds no card when the turn handed nothing out, including an incoming message', () => {
    expect(draw([])).not.toContain('lc-calltodos')
    expect(draw([{ ...fix, direction: 'received' }])).not.toContain('lc-calltodos')
  })

  it('reads Done when the receiving turn is completed, from the same state as a call', () => {
    const html = draw([fix], new Map([['mis_fix', callTodoState({ recordPhase: 'completed', needsYou: false })]]))
    expect(html).toContain('Handed out · 1 to-do')
    expect(html).toContain('<span class="lc-status lc-status--green">Done</span>')
  })

  it('keeps an earlier turn’s card under that turn, without repeating it under the newest', () => {
    const html = draw([fix], new Map([['mis_fix', 'done']]), true)
    expect(html.match(/<section class="lc-calltodos"/g)).toHaveLength(1)
    expect(html.indexOf('Handed out')).toBeLessThan(html.indexOf('What comes next?'))
  })

  it('shows an undelivered share as Starting without an Open button', () => {
    const html = draw([fix], new Map(), false, new Map())
    expect(html).toContain('Handed out · 1 to-do')
    expect(html).toContain('<span class="lc-status lc-status--quiet">Starting</span>')
    expect(html).not.toContain('lc-calltodo__open')
  })

  it('keeps separate asks delivered to the same turn, rather than losing one row', () => {
    const html = draw([fix, { ...fix, messageId: 'msg_more', text: 'Also check the signed-out case' }], new Map([['mis_fix', 'working']]), false,
      new Map([['msg_fix', 'mis_fix'], ['msg_more', 'mis_fix']]))
    expect(html).toContain('Handed out · 2 to-dos')
    expect(html.match(/data-call-todo="mis_fix"/g)).toHaveLength(2)
    expect(html).toContain('Also check the signed-out case')
  })

  it('uses the receiving live mission before a history refresh, never the sender', () => {
    const sender = { missionId: 'mis_lead', peerMessages: [fix] }
    expect(peerRunFor(fix.messageId, [sender, targets[0]!])?.missionId).toBe('mis_fix')
    expect(peerRunFor(fix.messageId, [sender])).toBeUndefined()
  })
})
