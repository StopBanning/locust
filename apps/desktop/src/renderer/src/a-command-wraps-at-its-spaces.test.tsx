import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { ApprovalCard } from './components/ApprovalCard.js'
import { BoardScreen } from './components/BoardScreen.js'
import { CommandText } from './components/CommandText.js'
import type { SidebarMission } from './components/Sidebar.js'
import type { MissionApprovalRequest } from '../../shared/ipc.js'

/**
 * A COMMAND WRAPS AT ITS SPACES (0.837, CommandText.tsx). The site frames of 2026-10-27 caught Codex's command broken
 * as `powershell.exe" -` / `Command 'python -m pytest -q'` on the approval card and on the Board. A flag's dashes are
 * now held to its first letter, so no line ends in a lone dash, and the text is still the command exactly as sent.
 */
const COMMAND = String.raw`"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command 'python -m pytest -q --maxfail=1'`
const text = (html: string): string => html.replace(/<[^>]*>/g, '').replaceAll('&quot;', '"').replaceAll('&#x27;', "'")

describe('a command on a card', () => {
  it('holds each flag to its dash and changes no character', () => {
    const html = renderToStaticMarkup(<CommandText text={COMMAND} />)
    for (const flag of ['-C', '-m', '-q', '--m']) expect(html).toContain(`<span class="lc-nobreak">${flag}</span>`)
    expect(text(html)).toBe(COMMAND)
  })

  it('leaves a dash inside a word, a lone dash and a number alone', () => {
    const plain = 'git log --oneline -3 a-b - c'
    const html = renderToStaticMarkup(<CommandText text={plain} />)
    expect(html).toContain('<span class="lc-nobreak">--o</span>')
    expect(html).toContain('<span class="lc-nobreak">-3</span>')
    expect(html).not.toContain('>-b<')
    expect(html).not.toContain('>- <')
    expect(text(html)).toBe(plain)
  })

  it('is drawn so on the approval card, and a connector call is not', () => {
    const request = { approvalId: 'ap_1', kind: 'command', summary: 'Run a command', detail: COMMAND, runtimeId: 'codex' } as unknown as MissionApprovalRequest
    const card = renderToStaticMarkup(<ApprovalCard request={request} onDecide={() => undefined} onAnswer={() => undefined} busy={false} />)
    expect(card).toContain('<span class="lc-nobreak">-C</span>ommand')
    const connector = renderToStaticMarkup(<ApprovalCard request={{ ...request, kind: 'connector', detail: '{\n  "note": "see -x"\n}' }} onDecide={() => undefined} onAnswer={() => undefined} busy={false} />)
    expect(connector).not.toContain('lc-nobreak')
  })

  it('is drawn so on the Board', () => {
    const board = renderToStaticMarkup(
      <BoardScreen
        missions={[{ missionId: 'm_run', title: 'Run the tests', phase: 'running', integrityIssueCount: 0 } as SidebarMission]}
        rooms={[]}
        trashed={new Set()}
        facts={{ needsYou: new Set(['m_run']) }}
        waitingFor={new Map([['m_run', 'Run a command']])}
        teammates={[]}
        missionOwners={{}}
        onOpen={() => undefined}
        asks={new Map([['m_run', { approvalId: 'ap_1', kind: 'command', command: COMMAND, where: 'C:/acme-storefront' }]])}
        onAnswer={() => undefined}
      />
    )
    expect(board).toContain('<span class="lc-nobreak">-C</span>ommand')
  })
})
