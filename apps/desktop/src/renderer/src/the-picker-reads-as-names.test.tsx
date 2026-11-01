import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { RoutePicker } from './components/RoutePicker.js'
import type { PublicModel, PublicRuntimeStatus } from '../../shared/ipc.js'

/**
 * The model picker is the differentiator, so it has to read like one.
 *
 * The catalogue falls back to a model's ID when the runtime reports no
 * display name for it, and OpenCode's free models report none. So the picker
 * listed `muse-spark-1.3-contributor-free` in a column where every other row
 * read as a proper name -- with the composer chip an inch below it reading
 * "Muse Spark 1.3 Contributor Free", because chips and mission rows have gone
 * through `modelDisplayName` since Grok's first pass found the same
 * inconsistency there. The picker was the last untreated spot.
 *
 * Both halves matter and the second is the one that could go wrong: this
 * spells out IDENTIFIERS. A name the runtime actually gave is the runtime's
 * own product name and is printed exactly as it wrote it.
 */

const OPENCODE: PublicRuntimeStatus = {
  id: 'opencode',
  displayName: 'OpenCode',
  installed: true,
  version: '1.0.0',
  auth: 'not-applicable',
  ready: true,
  status: 'ready'
}

const model = (id: string, displayName: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName, description: '', supportedEfforts: [] }) as PublicModel

const drawn = (models: readonly PublicModel[]): string =>
  renderToStaticMarkup(
    <RoutePicker
      runtimes={[OPENCODE]}
      models={models}
      active={{ runtime: 'opencode', model: 'account-default' }}
      resolvedModels={new Map()}
      recentRoutes={[]}
      limitedRuntimes={new Map()}
      onSelect={() => undefined}
      onClose={() => undefined}
    />
  )

describe('a model row whose runtime never named it', () => {
  it('reads as a name rather than an identifier', () => {
    // The row and the composer chip below it now say the same words.
    const html = drawn([model('muse-spark-1.3-contributor-free', 'muse-spark-1.3-contributor-free')])
    expect(html).toContain('Muse Spark 1.3 Contributor Free')
    expect(html).not.toContain('>muse-spark-1.3-contributor-free<')
  })

  it('leaves a name the runtime really gave exactly as it wrote it', () => {
    /*
     * THE CONTROL, and the half this could break. `Cursor Grok 4.6` is the
     * runtime's own product name; running it through the speller would strip
     * the word Cursor because the group header already says it -- a defensible
     * choice for a chip and the wrong one here, where it would mean the app
     * quietly renaming somebody's product in a list of products.
     */
    const html = drawn([model('cursor-grok-4.6-high', 'Cursor Grok 4.6')])
    expect(html).toContain('Cursor Grok 4.6')
  })
})

describe("the account's own default", () => {
  it('reads "Account Default" in every group, as Claude Code\'s row does', () => {
    // First impressions, 0.848: Antigravity's group listed "Account default" a group above Claude Code's "Account Default".
    const html = drawn([model('account-default', 'Account default')])
    expect(html).toContain('>Account Default<')
    expect(html).not.toContain('>Account default<')
  })
})
