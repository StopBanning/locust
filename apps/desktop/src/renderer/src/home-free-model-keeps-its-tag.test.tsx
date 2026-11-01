import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import { HomeTeam, type HomeTeammate } from './components/HomeTeam.js'
import { homeRouteOf } from './routeName.js'

/**
 * HOME'S CARD, AS THE CHIP SAYS IT (0.848). First impressions on 2026-11-01: two of three cards cut "Nemotron 3.5
 * Lightning Free" to "Nemotron 3.5 Lightning Fr...", and Wren's read "Account Default" beside Codex's mark, where
 * the chip and the Team screen both said "Codex".
 */
const mate = (model: string, runtime: HomeTeammate['runtime']): HomeTeammate => ({
  teammateId: 'tm_penny', name: 'Penny', hue: 'butter', avatar: seedAvatar('tm_penny'), role: 'Money', route: `OpenCode · ${model}`, model, runtime, working: false
})
const draw = (team: readonly HomeTeammate[]): string => renderToStaticMarkup(<HomeTeam team={team} onMessage={() => undefined} />)

describe("a teammate's model on Home", () => {
  it('keeps the Free tag whole and lets the name give way', () => {
    const html = draw([mate('Nemotron 3.5 Lightning Free', 'opencode')])
    expect(html).toContain('<span class="lc-hometeam__model">Nemotron 3.5 Lightning</span><span class="lc-hometeam__free">Free</span>')
    expect(html).toContain('lc-hometeam__route has-free')
  })

  it('draws a paid model as the line it was', () => {
    const html = draw([mate('Opus 5.5', 'claude')])
    expect(html).not.toContain('lc-hometeam__free')
    expect(html).not.toContain('has-free')
    expect(html).toContain('Opus 5.5</span>')
  })

  it("names a teammate on its account's default by the runtime, as the chip and Team do", () => {
    const line = homeRouteOf({ runtime: 'codex', model: 'account-default' }, new Map())
    expect(line.model).toBe('Codex')
    expect(line.route).toBe('Codex')
    expect(homeRouteOf({ runtime: 'antigravity', model: 'account-default' }, new Map()).model).toBe('Antigravity')
  })
})
