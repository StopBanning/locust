import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { TeammateSetup as Setup } from '../../shared/teammate-setup.js'
import { TeammateSetup } from './components/TeammateSetup.js'

vi.mock('./useModal.js', () => ({ useModal: vi.fn() }))

/*
 * A TEAMMATE'S SETUP (0.811): `/setup` in a teammate's conversation -- running or not, since reading changes nothing
 * -- opens a card of what shapes its turns, with Edit teammate one press away.
 */
const setup: Setup = {
  teammateId: 'tm_ada',
  folder: { name: 'locust', kind: 'project' },
  runtimeFile: { name: 'CLAUDE.md', lines: 3 },
  instructions: 49,
  aboutYou: true,
  memory: { on: false, count: 0 },
  skills: { own: ['evidence-first'], project: 2, personal: { count: 5, lent: true }, library: 0 },
  connectors: ['Gmail'],
  lessons: 0,
  lastTurn: { version: '2.1.310', tools: 21, skills: 8 }
}
const card = (over: Partial<Parameters<typeof TeammateSetup>[0]> = {}): string =>
  renderToStaticMarkup(<TeammateSetup name="Ada" runtime="Claude Code" model="Claude Code / Haiku 5.5 · Auto" setup={setup} onEdit={() => undefined} onClose={() => undefined} {...over} />)

describe('the setup card', () => {
  it('says the model first, then each layer under its section', () => {
    const html = card()
    expect(html).toContain('aria-label="Ada’s setup"')
    expect(html.indexOf('Claude Code / Haiku 5.5 · Auto')).toBeLessThan(html.indexOf('Every turn'))
    for (const text of ['Every turn', 'When the work matches', 'Only when you tune it', 'Its last turn here', '3 lines, read by Claude Code itself', 'evidence-first', '5 skills', 'On: Gmail', '21 tools, 8 skills']) {
      expect(html).toContain(text)
    }
    expect(html).toContain('>Edit teammate</button>')
  })

  it('says it is reading, or why it could not, and nothing else', () => {
    expect(card({ setup: undefined })).toContain('Reading what shapes Ada’s turns…')
    const failed = card({ setup: undefined, problem: 'This teammate’s setup could not be read. Nothing was changed.' })
    expect(failed).toContain('role="alert"')
    expect(failed).not.toContain('Every turn')
  })
})
