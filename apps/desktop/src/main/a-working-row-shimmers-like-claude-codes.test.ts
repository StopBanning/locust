import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * A WORKING ROW SHIMMERS, AS CLAUDE CODE'S DOES (0.839). Colin, 2026-10-28, of Claude Code's sidebar: "this rolling
 * shimmer to show that the convo/chat is working and it looks really nice its like a monochrome gradient ... now i
 * want you to make ours like that, i have lots of chats going now and its too distracting". The turning spark on every
 * running row is gone from the sidebar; a light band passes slowly through the title instead. Pure CSS, so these read
 * the stylesheet (renderer tests cannot: a `?raw` CSS import is empty there).
 */
const SRC = fileURLToPath(new URL('../renderer/src/', import.meta.url))
const css = readFileSync(`${SRC}shell.css`, 'utf8')
const sidebar = readFileSync(`${SRC}components/Sidebar.tsx`, 'utf8')
const rule = (selector: string): string => {
  const at = css.indexOf(`${selector} {`)
  return at < 0 ? '' : css.slice(at, css.indexOf('\n}', at) + 2)
}

describe('a working row shimmers like Claude Code’s', () => {
  it('is the grey title with one white band passing through it, from the theme’s own tokens', () => {
    const shimmer = rule('.lc-conv__title.is-working')
    expect(shimmer).toContain('linear-gradient(90deg, var(--lc-text-muted) 0%, var(--lc-text-muted) 38%, var(--lc-text-primary) 50%')
    expect(shimmer).toContain('background-clip: text;')
    expect(shimmer).toContain('color: transparent;')
    // No colour of its own: only the theme's greys, so light mode flips with them.
    expect(shimmer).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|lime/i)
  })

  it('passes slowly, left to right, and stands still for a person who asked for less motion', () => {
    expect(rule('.lc-conv__title.is-working')).toContain('animation: lc-shimmer 2.4s linear infinite;')
    const keyframes = css.slice(css.indexOf('@keyframes lc-shimmer {'), css.indexOf('\n}', css.indexOf('@keyframes lc-shimmer {')) + 2)
    expect(keyframes).toContain('from { background-position: 100% 0; }')
    expect(keyframes).toContain('to { background-position: 0% 0; }')
    const still = css.slice(css.indexOf('.lc-conv__title.is-working {', css.indexOf('@media (prefers-reduced-motion: reduce)', css.indexOf('@keyframes lc-shimmer'))))
    expect(still.slice(0, 160)).toContain('animation: none;')
  })

  it('is worn by a running conversation and a running room only, and keeps the dot’s footprint empty', () => {
    expect(sidebar).toContain("className={`lc-conv__title${mission.phase === 'running' ? ' is-working' : ''}`}")
    expect(sidebar).toContain("className={`lc-conv__title${running ? ' is-working' : ''}`}")
    expect(sidebar.split('<span className="lc-conv__working" aria-hidden="true" data-orb="shimmer" />').length - 1).toBe(2)
    const slot = rule('.lc-conv__working')
    expect(slot).toContain('width: 6px;')
    expect(slot).toContain('height: 6px;')
  })
})
