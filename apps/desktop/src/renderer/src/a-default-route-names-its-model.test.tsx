import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'

import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import type { PublicRuntimeStatus } from '../../shared/ipc.js'
import { routeModelName } from './routeName.js'
import { liveModelNames } from './missionView.js'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

/**
 * A ROUTE ON THE ACCOUNT'S DEFAULT NAMES THE MODEL IT TURNED OUT TO BE (0.797).
 *
 * Colin, 2026-10-19, on a turn Locust carried on with Claude Code when Codex's plan ran out: "why does it just say
 * claude high when i do a handoff?" The default reads as the runtime alone since 0.723; once a run has said which
 * model the default is, the chip names it too.
 */

const claude = {
  id: 'claude', displayName: 'Claude Code', installed: true, version: '2.1.280', auth: 'authenticated', ready: true, status: 'ready'
} as unknown as PublicRuntimeStatus

const props: ComposerProps = {
  runtimes: [claude], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
  activeRoute: undefined, error: undefined, mode: 'accept-edits', onModeChange: () => undefined,
  route: { runtime: 'claude', model: 'account-default' }, onRouteChange: () => undefined,
  models: [], resolvedModels: new Map(), recentRoutes: [], platform: 'win32', effort: undefined,
  onEffortChange: () => undefined, swarm: false, onSwarmChange: () => undefined,
  onStart: async () => true, onCancel: () => undefined, onOpenRoutePicker: () => undefined,
  onHandOff: () => undefined, handingOff: false, workspaceName: 'Test', workspacePath: 'C:/test',
  onChooseFolder: () => undefined, teammateName: undefined, busyWith: undefined, queued: undefined,
  queuedNote: undefined, onQueue: () => undefined, onUnqueue: () => undefined,
  onSendQueued: () => undefined, continuationNote: undefined, queuedElsewhere: false
}

it('names the model the default turned out to be, once a run has said', () => {
  const name = routeModelName('claude', 'claude-opus-5-5')
  expect(name).toMatch(/Opus 5\.5/)
  const html = renderToStaticMarkup(<Composer {...props} resolvedModels={new Map([['claude:account-default', 'claude-opus-5-5']])} />)
  expect(html).toContain(`<span class="lc-control__mono lc-control__model">${name}</span>`)
  expect(html).toContain(`its own default model, ${name} on its last run`)
  expect(html).not.toContain('account-default')
})

it('reads as the runtime alone until then, and never as its id', () => {
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).not.toContain('lc-control__mono lc-control__model')
  expect(html).not.toContain('No AI agent')
  expect(html).toContain('its own default model')
  expect(html).not.toContain('account-default')
})

/*
 * 0.827: a turn carried on with Claude Code read "Claude  High" all the way through, since only finished runs were
 * read (Colin, 2026-10-23: "still only reading as claude and high"). Claude Code's start record names the model.
 */
it('names the model of a run under way, from its start record', () => {
  const started = { type: 'run.started', payload: { runtimeThreadId: 's1', evidence: { redacted: true, raw: { model: 'claude-sonnet-5-5' } } } } as unknown as NormalizedRuntimeEvent
  const live = liveModelNames([
    { runtime: 'claude', model: 'account-default', events: [started] },
    // An alias the start only repeats teaches nothing; a run not yet started says nothing.
    { runtime: 'claude', model: 'claude-sonnet-5-5', events: [started] },
    { runtime: 'codex', model: 'account-default', events: [] }
  ])
  expect([...live]).toEqual([['claude:account-default', 'claude-sonnet-5-5']])
  const html = renderToStaticMarkup(<Composer {...props} resolvedModels={live} />)
  expect(html).toContain(`<span class="lc-control__mono lc-control__model">${routeModelName('claude', 'claude-sonnet-5-5')}</span>`)
})
