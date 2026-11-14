import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { ActivityCard } from './components/ActivityCard.js'
import { modalityAfter, watchInputModality } from './inputModality.js'

/**
 * TWO THINGS IN COLIN'S FRAME OF 2026-11-13 (0.875), a long Codex run with every step opened:
 * - "Waiting for script output" and "Clarifying control 18 source" opened onto an empty quote bar. Each was one
 *   thought that was only its headline, and the headline is already the line.
 * - The step he had clicked wore the keyboard's focus ring.
 */
const card = (details: unknown[]): string =>
  renderToStaticMarkup(
    <ActivityCard
      variant="steps"
      summary=""
      trace={[{ key: 'what', text: 'Waiting for script output' }]}
      finished
      openByDefault
      details={details as never}
      runtimeName={undefined}
      workspacePath={undefined}
    />
  )

describe('a step whose thought is only its headline', () => {
  it('is a line with no chevron and nothing to open', () => {
    const html = card([{ kind: 'reasoning', name: 'thought', settled: true, output: '**Waiting for script output**', durationMs: 2_000 }])
    expect(html).toContain('lc-steps__line is-static')
    expect(html).toContain('Waiting for script output')
    expect(html).not.toContain('aria-expanded')
    expect(html).not.toContain('lc-steps__chev')
    expect(html).not.toContain('lc-steps__thought')
    expect(html).not.toContain('lc-steps__list')
  })

  it('beside other steps draws no empty quote, and the step still opens onto them', () => {
    const html = card([
      { kind: 'reasoning', name: 'thought', settled: true, output: '**Waiting for script output**', durationMs: 2_000 },
      { kind: 'tool', id: 't1', name: 'grep_search', tool: 'grep_search', settled: true, failed: false }
    ])
    expect(html).toContain('aria-expanded="true"')
    expect(html).not.toContain('lc-steps__thought')
    expect(html).not.toContain('lc-steps__line is-static')
  })

  it('a thought with words under its headline still opens onto them', () => {
    const html = card([{ kind: 'reasoning', name: 'thought', settled: true, output: '**Waiting for script output**\n\nThe fixture prints only at the end.', durationMs: 2_000 }])
    expect(html).toContain('aria-expanded="true"')
    expect(html).toContain('lc-steps__thought')
    expect(html).toContain('The fixture prints only at the end.')
  })
})

describe('the focus ring after a pointer press', () => {
  it('a press is the pointer; a key that moves focus or presses is the keyboard; a letter or a shortcut says nothing', () => {
    expect(modalityAfter({ type: 'pointerdown' })).toBe('pointer')
    expect(modalityAfter({ type: 'mousedown' })).toBe('pointer')
    for (const key of ['Tab', 'ArrowDown', 'Enter', ' ', 'Escape']) expect(modalityAfter({ type: 'keydown', key }), key).toBe('keyboard')
    expect(modalityAfter({ type: 'keydown', key: 'a' })).toBeUndefined()
    expect(modalityAfter({ type: 'keydown', key: 'Tab', metaKey: true })).toBeUndefined()
    expect(modalityAfter({ type: 'focus' })).toBeUndefined()
  })

  it('keeps the root marked with the hand that moved last, listening in the capture phase', () => {
    const listeners: { type: string; capture: boolean; handle: (event: Event) => void }[] = []
    const target = { addEventListener: (type: string, handle: (event: Event) => void, options?: AddEventListenerOptions | boolean) => { listeners.push({ type, capture: typeof options === 'object' && options.capture === true, handle }) } }
    const root = { dataset: {} as DOMStringMap }
    watchInputModality(target as never, root)
    expect(listeners.every((one) => one.capture)).toBe(true)
    const fire = (type: string, key?: string): void => { for (const one of listeners.filter((l) => l.type === type)) one.handle({ type, key } as unknown as Event) }
    fire('pointerdown')
    expect(root.dataset.input).toBe('pointer')
    fire('keydown', 'a')
    expect(root.dataset.input).toBe('pointer')
    fire('keydown', 'Tab')
    expect(root.dataset.input).toBe('keyboard')
  })

  it('hides a button ring after the pointer and keeps a field ring; the window installs it', () => {
    const tokens = readFileSync(join(__dirname, 'tokens.css'), 'utf8')
    expect(tokens).toMatch(/:root\[data-input='pointer'\] button:focus-visible,[\s\S]*?outline: none;/)
    expect(tokens).not.toMatch(/:root\[data-input='pointer'\] (input|textarea):focus-visible/)
    expect(readFileSync(join(__dirname, 'main.tsx'), 'utf8')).toContain('watchInputModality(window, document.documentElement)')
  })
})
