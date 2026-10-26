import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicModel, PublicRuntimeStatus } from '../../shared/ipc.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { routeLabel } from './components/GroupSettingsDialog.js'
import { isOwnRoute, rememberOwnModels, routeChrome } from './routeName.js'

/**
 * A MODEL OF YOUR OWN IS NAMED AS YOURS (0.361).
 *
 * Every model a person adds under Your own models runs through OpenCode, so
 * the chat bar read "OpenCode / Acme Chat" and the header "Code & Migrations
 * · OpenCode" (drive-own-model, packaged 0.358): a company that brought its
 * own model met another product's name in front of it. On chrome the name
 * the person gave it is now the whole name; the exact route stays in the
 * chip's tooltip.
 */
const ACME: PublicModel = { id: 'own-a1b2c3d0/acme-70b', runtime: 'opencode', displayName: 'Acme Chat', description: 'Your model · llm.acme.example', supportedEfforts: [], own: true }
const opencode: PublicRuntimeStatus = { id: 'opencode', displayName: 'OpenCode', installed: true, version: '1.18.27', auth: 'authenticated', ready: true, status: 'ready' }

const composer = (route: ComposerProps['route']): string =>
  renderToStaticMarkup(
    <Composer
      runtimes={[opencode]}
      limitedRuntimes={new Map()}
      discoveryPhase="ready"
      running={false}
      cancelling={false}
      activeRoute={undefined}
      error={undefined}
      mode="accept-edits"
      onModeChange={() => undefined}
      route={route}
      onRouteChange={() => undefined}
      models={[ACME]}
      resolvedModels={new Map()}
      recentRoutes={[]}
      platform="win32"
      effort={undefined}
      onEffortChange={() => undefined}
      swarm={false}
      onSwarmChange={() => undefined}
      onStart={async () => true}
      onCancel={() => undefined}
      onOpenRoutePicker={() => undefined}
      onHandOff={() => undefined}
      handingOff={false}
      workspaceName="shop"
      workspacePath="C:\\work\\shop"
      onChooseFolder={() => undefined}
      teammateName={undefined}
      busyWith={undefined}
      queued={undefined}
      queuedNote={undefined}
      onQueue={() => undefined}
      onUnqueue={() => undefined}
      onSendQueued={() => undefined}
      continuationNote={undefined}
      queuedElsewhere={false}
    />
  )

/** The route chip's visible words, tags and all. */
const chipWords = (html: string): string => {
  const chip = /<button[^>]*aria-haspopup="listbox"[^>]*>([\s\S]*?)<\/button>/.exec(html)?.[1] ?? ''
  return chip.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim()
}

describe('a model of your own', () => {
  it('is known by its route', () => {
    expect(isOwnRoute('own-a1b2c3d0/acme-70b')).toBe(true)
    expect(isOwnRoute('opencode/muse-spark-1.3-contributor-free')).toBe(false)
    expect(isOwnRoute('gpt-6-astra')).toBe(false)
  })

  it('is named by its own name on every surface that names a route', () => {
    expect(routeChrome('opencode', 'own-a1b2c3d0/acme-70b', 'Acme Chat')).toBe('Acme Chat')
    expect(routeChrome('opencode', 'own-a1b2c3d0/acme-70b', 'Acme Chat', ' · ')).toBe('Acme Chat')
    // Every other route keeps its runtime in front, as before.
    expect(routeChrome('codex', 'gpt-6-astra', 'GPT-6 Astra')).toBe('Codex / GPT-6 Astra')
    expect(routeChrome('opencode', 'opencode/ling-3.0-flash-fin-free', 'Ling 3.0 Flash Fin Free', ' · ')).toBe('OpenCode · Ling 3.0 Flash Fin Free')
    rememberOwnModels([ACME])
    expect(routeLabel({ runtime: 'opencode', model: 'own-a1b2c3d0/acme-70b', mode: 'accept-edits' })).toBe('Acme Chat')
  })

  it('reads as itself in the chat bar, with the exact route one hover away', () => {
    rememberOwnModels([ACME])
    const html = composer({ runtime: 'opencode', model: 'own-a1b2c3d0/acme-70b' })
    // Effort sits on the same control. This model has none to choose, so the words are its name and Fixed.
    expect(chipWords(html)).toBe('Acme Chat · Fixed')
    expect(chipWords(html)).not.toContain('OpenCode')
    expect(html).toContain('OpenCode / own-a1b2c3d0/acme-70b')
    // The agent's name stays in the tooltip. The face is the model and its effort, with the mark beside them.
    const ordinary = composer({ runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free' })
    expect(chipWords(ordinary)).not.toMatch(/^OpenCode \//)
    expect(ordinary).toContain('OpenCode / opencode/ling-3.0-flash-fin-free')
  })
})
