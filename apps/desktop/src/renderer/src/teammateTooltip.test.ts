import { describe, expect, it } from 'vitest'

import { teammateTooltip } from './teammateTooltip.js'

describe('what hovering a teammate says', () => {
  it('gives the name, the role and the model', () => {
    expect(
      teammateTooltip({ name: 'Wren', role: 'Code & Migrations', route: { runtime: 'cursor', model: 'cursor-grok-4.6-medium' } })
    ).toBe('Message Wren · Code & Migrations · Cursor / Grok 4.6 Medium')
  })

  it('names the model as the chip does, never by its id', () => {
    // The rail at 1024x684 (macOS 15 runners, 2026-11-01) said "OpenCode / opencode/nemotron-3.5-lightning-free".
    expect(teammateTooltip({ name: 'Ash', role: 'Custom', route: { runtime: 'opencode', model: 'opencode/nemotron-3.5-lightning-free' } }))
      .toBe('Message Ash · Custom · OpenCode / Nemotron 3.5 Lightning Free')
    // The account's own default is the runtime alone, as every chip says it.
    expect(teammateTooltip({ name: 'Wren', role: 'Custom', route: { runtime: 'codex', model: 'account-default' } })).toBe('Message Wren · Custom · Codex')
  })

  it('stops after the role when the teammate has never run', () => {
    // THE test. A tooltip that named a model for a teammate whose route
    // nothing has resolved would be inventing the exact fact people open this
    // app to compare.
    const said = teammateTooltip({ name: 'Gem', role: 'Research & Briefs' })
    expect(said).toBe('Message Gem · Research & Briefs')
    expect(said).not.toMatch(/\//)
  })

  it('still leads with the action, so a click is predictable', () => {
    // Every drive in the harness finds a teammate by this prefix, and more to
    // the point a tooltip on a button should say what pressing it does.
    expect(teammateTooltip({ name: 'Wren', role: 'Custom' }).startsWith('Message Wren')).toBe(true)
  })

  it('names the runtime the way the rest of the app does', () => {
    // `cursor` is an id, "Cursor Agent" is the product. A tooltip is not the
    // place to start showing people internal ids.
    expect(teammateTooltip({ name: 'A', role: 'Custom', route: { runtime: 'opencode', model: 'x' } })).toContain('OpenCode')
  })
})
