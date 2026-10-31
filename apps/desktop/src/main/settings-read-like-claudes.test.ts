import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * SETTINGS READ LIKE CLAUDE'S (0.393).
 *
 * Colin, 2026-09-27, with a screenshot of Claude's own settings: "we should
 * just make our settings like claude codes, way cleaner and more organized
 * than ours and has icons". Three things carry that look, and each is held
 * here: pages under group labels with an icon each; no card around a setting,
 * rows on hairlines; and a setting that is one switch drawn as one line.
 * Read as text, like the other guards on this screen (settings-pages-are-real).
 */
const source = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const PAGES = source('../renderer/src/settingsPages.ts')
const SCREENS = source('../renderer/src/components/Screens.tsx')
const CSS = source('../renderer/src/shell.css')
const ICONS = source('../renderer/src/components/Icon.tsx')

const entries = [...PAGES.matchAll(/id: '([^']+)',\s*label: (?:'([^']+)'|"([^"]+)"),\s*group: '([^']+)',\s*icon: '([^']+)'/g)].map((match) => ({
  id: match[1],
  label: match[2] ?? match[3],
  group: match[4],
  icon: match[5]
}))

describe('the settings list', () => {
  it('files every page under a group, with an icon the set can draw', () => {
    expect(entries.map((entry) => entry.id)).toEqual(['app', 'chat', 'appearance', 'privacy', 'teammates', 'memory', 'relay', 'runtimes', 'models', 'connectors', 'cloud', 'workspace', 'whatsnew'])
    for (const entry of entries) expect(ICONS, entry.icon ?? '').toMatch(new RegExp(`\\n    '?${entry.icon ?? ''}'?: `))
  })

  it('keeps each group together, in order, so every label is said once', () => {
    const runs = entries.map((entry) => entry.group).filter((group, index, all) => group !== all[index - 1])
    expect(runs).toEqual(['Locust', 'Team', 'Agents', 'This folder', 'About'])
  })

  it('draws the group label above its first page and the icon beside each name', () => {
    expect(SCREENS).toContain("{shownPages[index - 1]?.group !== entry.group && <span className=\"lc-settings__navgroup\">{entry.group}</span>}")
    expect(SCREENS).toContain('<span className="lc-settings__navicon" aria-hidden="true"><Icon name={entry.icon} size={16} /></span>')
  })
})

describe('the settings pane', () => {
  it('puts no card around a setting: rows sit on the pane with a hairline between', () => {
    const flat = CSS.slice(CSS.indexOf('.lc-settings__pane .lc-settingrows,'))
    expect(flat.slice(0, 260)).toContain('border: 0;')
    expect(flat.slice(0, 260)).toContain('background: transparent;')
    expect(CSS).toMatch(/\.lc-settings__pane \.lc-settingrow:hover \{\s*background: transparent;/)
  })

  it('draws a setting that is one control as one line: name and description left, the control right (0.393, Appearance 0.394)', () => {
    // Six again since 0.510: Finances (0.501) was shelved. Seven with Terminal faces (0.561). Nine with What a face
    // says (2026-10-05): a line whose right-hand side is a face to click, not a control. Ten with Your skills (0.679), eleven with Voice typing. Thirteen with Your phone (0.775). Fourteen with Theme (0.789). Fifteen with Telegram (0.831). Sixteen with Your account (accounts slice 3).
    expect(SCREENS.split('lc-settings__section lc-settings__section--line').length - 1).toBe(16)
    for (const name of ['Swarm', 'Auto mode', 'Plans', 'Your skills', 'Reply text size', 'Terminal faces', 'What a face says', 'Sidebar', 'Boot screen', 'Voice typing']) {
      const at = SCREENS.indexOf(`<h2 className="lc-settings__heading">${name}</h2>`)
      const line = SCREENS.lastIndexOf('<div className="lc-settingline">', at)
      expect(at - line, name).toBeLessThan(120)
    }
    expect(CSS).toMatch(/\.lc-settingline \{\s*display: flex;\s*align-items: center;\s*justify-content: space-between;/)
  })

  it('says "How it works" in sentence case', () => {
    expect(CSS).toMatch(/\.lc-settings__pane \.lc-settings__more > summary \{[^}]*text-transform: none;/)
  })
})
