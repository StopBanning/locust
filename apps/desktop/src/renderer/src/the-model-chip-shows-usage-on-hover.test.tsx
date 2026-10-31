import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { AgentCard } from './components/AgentMark.js'

/**
 * THE MODEL CHIP SHOWS ITS USAGE ON HOVER (2026-10-30). Colin, on the Mac: "hovering isnt showing usage window for
 * codex ... model picker usually does it". The chat box's model chip carried the account's usage only in its `title`,
 * the system's own tooltip, which a Mac shows only while Locust is the app in front and after a pause, where Windows
 * shows it whenever the pointer rests. It now opens the card the agents' marks open on Home (AgentMark.tsx, 0.389):
 * drawn by the page, on hover or focus, above the chip and aligned to its right.
 */
const source = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const reading = 'weekly window 64% used · resets 2026-11-04T06:36:18.000Z · 5-hour window 12% used · resets 2026-10-31T05:51:36.000Z · from a run at 2026-10-31T02:22:14.418Z'

describe('the model chip', () => {
  it("opens the agent's card, with a bar for each window, placed for the chip", () => {
    const html = renderToStaticMarkup(<AgentCard className="lc-agentcard--chip" runtime="codex" name="Codex CLI" state="Ready · 0.166.0" usage={reading} now={new Date('2026-10-31T03:20:00Z')} />)
    expect(html).toContain('<span class="lc-agentcard lc-agentcard--chip" aria-hidden="true">')
    expect(html).toContain('<span class="lc-agentcard__label">Weekly window</span><span class="lc-agentcard__percent">64%</span>')
    expect(html).toContain('<span class="lc-agentcard__label">5-hour window</span><span class="lc-agentcard__percent">12%</span>')
  })

  it('draws it on hover or focus, above and to the right, rather than leaving it to the system tooltip', () => {
    const composer = source('./components/Composer.tsx')
    expect(composer).toContain('className="lc-agentcard--chip"')
    expect(composer).toContain('usage={usageReading}')
    // The usage is no longer a line of the chip's title; it is read aloud as the chip's description.
    expect(composer).not.toMatch(/handoffTitle\(handoff\),\s*usageSentence/)
    expect(composer).toContain("'aria-description': usageSentence")
    const css = source('./shell.css')
    expect(css).toContain('.lc-control__anchor:hover > .lc-agentcard--chip,\n.lc-control__anchor:focus-within > .lc-agentcard--chip {\n  display: grid;\n}')
    expect(css).toContain('.lc-agentcard--chip {\n  left: auto;\n  right: 0;\n  transform: none;\n}')
  })
})
