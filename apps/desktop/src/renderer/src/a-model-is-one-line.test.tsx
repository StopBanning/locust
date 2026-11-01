import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { RoutePicker } from './components/RoutePicker.js'
import type { PublicModel, PublicRuntimeStatus } from '../../shared/ipc.js'

/**
 * A MODEL IS ONE LINE IN THE PICKER, the way Claude Code's and Codex's own
 * pickers draw them.
 *
 * Each row's second line carried the catalogue's description, the resolved
 * name and every effort level -- "Newest fable model · resolved by Claude
 * Code at launch · 5 effort levels · low, medium, high, xhigh, max" -- and
 * wrapped to three lines at the picker's width, so about four models fitted
 * in view (frames, 2026-09-22). The effort levels are what nobody chooses a
 * model by; they move to the row's hover, and the row keeps the rest.
 */

const claude: PublicRuntimeStatus = {
  id: 'claude',
  displayName: 'Claude Code',
  installed: true,
  version: '2.1.280',
  auth: 'authenticated',
  ready: true,
  status: 'ready'
}

const fable: PublicModel = {
  id: 'fable',
  runtime: 'claude',
  displayName: 'Fable 5.1',
  description: 'Always the newest Fable',
  supportedEfforts: ['low', 'medium', 'high', 'xhigh', 'max']
}
const bare: PublicModel = {
  id: 'sonnet',
  runtime: 'claude',
  displayName: 'Sonnet 5.5',
  // A catalogue with nothing to say about it.
  description: '',
  supportedEfforts: ['low', 'high']
}

const draw = (models: readonly PublicModel[]): string =>
  renderToStaticMarkup(
    <RoutePicker
      runtimes={[claude]}
      limitedRuntimes={new Map()}
      models={models}
      resolvedModels={new Map()}
      recentRoutes={[]}
      active={{ runtime: 'claude', model: 'fable' }}
      onSelect={() => undefined}
      onClose={() => undefined}
    />
  )

/** The visible detail line of the row whose label is `label`. */
const detailOf = (html: string, label: string): string => {
  const row = html.slice(html.indexOf(`>${label}<`))
  // The line is in the reading face since the polish pass (2026-11-01): words about the model, not machine output.
  return /class="lc-picker__detail">([^<]*)</.exec(row)?.[1] ?? ''
}

describe('a model is one line in the picker', () => {
  it('keeps what the catalogue says, and leaves the effort levels to the hover', () => {
    const html = draw([fable])
    expect(detailOf(html, 'Fable 5.1')).toBe('Always the newest Fable')
    // Nothing lost: the hover carries all of it, levels included.
    expect(html).toContain('title="Fable 5.1 · Always the newest Fable · 5 effort levels · low, medium, high, xhigh, max"')
  })

  it('a model the catalogue says nothing about keeps its levels as its line, in words', () => {
    expect(detailOf(draw([bare]), 'Sonnet 5.5')).toBe('Low, High')
    // Many levels read as a range, and a fast variant is said once.
    const cursorish = { ...bare, id: 'opus', displayName: 'Claude Opus 5.5 1M', supportedEfforts: ['low', 'low-fast', 'medium', 'high', 'xhigh', 'max', 'max-fast'] }
    // A Claude row is named from Claude Code's alias table: `opus` reads Opus 5.5.
    expect(detailOf(draw([cursorish]), 'Opus 5.5')).toBe('Low to Max · Fast')
  })

  // The layout half -- that the line is DRAWN to stay one line -- is checked
  // against shell.css in src/main/picker-rows-stay-one-line.test.ts, where
  // the stylesheet can be read as text.
})
