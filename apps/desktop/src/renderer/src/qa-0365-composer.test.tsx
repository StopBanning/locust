import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { Composer } from './components/Composer.js'
import { RoutePicker } from './components/RoutePicker.js'
import type { ComposerProps } from './components/Composer.js'
import type { PublicModel, PublicRuntimeStatus } from '../../shared/ipc.js'

/**
 * The wall a person hits after doing exactly what the app told them to.
 *
 * From the independent QA pass on 0.36.5 (2026-09-06), reproduced live on
 * Linux: install nothing, read Settings, run `npm install -g opencode-ai`,
 * come back to the window -- discovery finds it, the Settings row goes READY,
 * home says "1 runtime connected" -- then type a message and press Enter.
 * Nothing happens. The box still says "Install a coding agent and sign in to
 * start a mission…", and no part of the screen says why.
 *
 * The cause is not the placeholder alone. `App.tsx` seeds the route as
 * `codex/account-default` and never moves it, and the composer decides
 * everything from THAT route's runtime rather than from whether any runtime
 * on the machine can run at all.
 *
 * Rendered rather than unit-tested because the placeholder is the whole
 * finding: it is the sentence the person reads. There is no DOM environment
 * in this repo, so this renders to static markup and asserts on the string a
 * person would see. The "Enter does nothing" half is proven by a drive, not
 * here -- no click is simulated.
 */

const runtime = (
  id: PublicRuntimeStatus['id'],
  displayName: string,
  usable: boolean
): PublicRuntimeStatus => ({
  id,
  displayName,
  installed: usable,
  version: usable ? '1.18.27' : null,
  auth: usable ? 'authenticated' : 'unknown',
  ready: usable,
  status: usable ? 'ready' : 'not-installed'
})

/** Every prop the composer needs, with nothing in it that decides this case. */
const props = (over: Partial<ComposerProps>): ComposerProps => ({
  runtimes: [],
  limitedRuntimes: new Map(),
  discoveryPhase: 'ready',
  running: false,
  cancelling: false,
  activeRoute: undefined,
  error: undefined,
  mode: 'accept-edits',
  onModeChange: () => undefined,
  route: { runtime: 'codex', model: 'account-default' },
  onRouteChange: () => undefined,
  models: [],
  resolvedModels: new Map(),
  recentRoutes: [],
  platform: 'win32',
  effort: undefined,
  onEffortChange: () => undefined,
  onStart: async () => true,
  onCancel: () => undefined,
  onOpenRoutePicker: () => undefined,
  onHandOff: () => undefined,
  handingOff: false,
  workspaceName: 'shop',
  workspacePath: 'C:\\work\\shop',
  onChooseFolder: () => undefined,
  teammateName: undefined,
  busyWith: undefined,
  queued: undefined,
  queuedNote: undefined,
  onQueue: () => undefined,
  onUnqueue: () => undefined,
  onSendQueued: () => undefined,
  continuationNote: undefined,
  queuedElsewhere: false,
  ...over
})

const markup = (over: Partial<ComposerProps>): string =>
  renderToStaticMarkup(<Composer {...props(over)} />)

describe('the composer on a machine that just got its first runtime', () => {
  // The state the QA reproduced: OpenCode installed and ready, the seeded
  // route still pointing at Codex, which is not installed.
  const justInstalledOpenCode = {
    runtimes: [
      runtime('codex', 'Codex CLI', false),
      runtime('opencode', 'OpenCode', true)
    ],
    route: { runtime: 'codex' as const, model: 'account-default' }
  }

  it('does not tell a person who just installed OpenCode to install an AI agent', () => {
    expect(markup(justInstalledOpenCode)).not.toContain('Install an AI agent')
  })

  it('invites a message, since one runtime can run it', () => {
    // Either the route has moved to the runtime that works, or the box says
    // which step is left. What it must not do is repeat the sentence for a
    // machine with nothing on it.
    const seen = markup(justInstalledOpenCode)
    const invites = seen.includes('Write a message') || seen.includes('Message ')
    const namesTheStep = /OpenCode/.test(seen) && /switch|route|pick it/i.test(seen)
    expect(invites || namesTheStep).toBe(true)
  })

  // ---- controls, so the two above cannot pass on a broken render ----

  it('still says to install one when nothing is installed', () => {
    expect(markup({ runtimes: [runtime('codex', 'Codex CLI', false)] })).toContain(
      'Install an AI agent'
    )
  })

  it('invites a message when the route itself is the ready runtime', () => {
    expect(
      markup({
        runtimes: [runtime('opencode', 'OpenCode', true)],
        route: { runtime: 'opencode', model: 'account-default' }
      })
    ).toContain('Write a message')
  })
})

describe('the picker and the one model that costs nothing', () => {
  const free: PublicModel = {
    id: 'opencode/muse-spark-1.2-contributor-free',
    runtime: 'opencode',
    displayName: 'Muse Spark 1.2 (contributor, free)',
    description: 'Free · no sign-in · listed by opencode models',
    supportedEfforts: []
  }
  const paid: PublicModel = {
    id: 'opencode/big-pickle',
    runtime: 'opencode',
    displayName: 'Big Pickle',
    description: 'Listed by opencode models',
    supportedEfforts: []
  }

  const picker = (models: readonly PublicModel[]): string =>
    renderToStaticMarkup(
      <RoutePicker
        runtimes={[runtime('opencode', 'OpenCode', true)]}
        limitedRuntimes={new Map()}
        models={models}
        resolvedModels={new Map()}
        recentRoutes={[]}
        active={{ runtime: 'opencode', model: 'account-default' }}
        onSelect={() => undefined}
        onClose={() => undefined}
      />
    )

  it('says which OpenCode model is free', () => {
    // The catalogue already marks it -- `model-catalog.ts` writes
    // "Free · no sign-in" and its comment calls that "the whole reason this
    // runtime is here" -- and `RoutePicker` builds each row's detail from the
    // effort count and the resolved name only, so the description never
    // reaches anyone. A person with no account reads a list of names and
    // cannot tell which of them cost nothing.
    //
    // Rendered as the PICKER, not through the composer: the composer draws
    // the picker only when it is open, so asserting on the closed composer
    // would go red because nothing rendered at all -- a red that does not
    // name the behaviour is no better than a green that cannot fail.
    expect(picker([free, paid])).toMatch(/Free|no sign-in/)
  })

  it('draws the rows it was given', () => {
    // The control for the assertion above: proves the picker really rendered
    // both models, so "no mention of free" is about the DETAIL line and not
    // about an empty render.
    const seen = picker([free, paid])
    expect(seen).toContain('Muse Spark 1.2 (contributor, free)')
    expect(seen).toContain('Big Pickle')
  })
})

describe('the placeholder on a machine with nothing installed', () => {
  it('does not tell the person to sign in, because there is nothing to sign in to', () => {
    // Grok, pass 5 on 0.154.0, bare PATH: the roster stopped saying it in
    // 0.138; the box you type into still did, under a welcome saying
    // OpenCode needs no account.
    const bare = markup({ runtimes: [runtime('opencode', 'OpenCode', false), runtime('codex', 'Codex CLI', false)] })
    expect(bare).toContain('Install an AI agent to start…')
    expect(bare).not.toContain('sign in')
  })
  it('still says sign in when a runtime is installed but not usable', () => {
    const unsigned = { ...runtime('codex', 'Codex CLI', false), installed: true, version: '0.153.0', auth: 'unauthenticated' as const, status: 'auth-required' as const }
    expect(markup({ runtimes: [unsigned] })).toContain('sign in to start…')
  })
})
