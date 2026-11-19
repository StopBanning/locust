// Colin: "use fakes only". W6: "In the cloud start form add an optional field".
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'
import { CloudEnvironmentField } from './components/CloudEnvironmentField.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
it('The cloud form names the optional environment and explains when it is saved.', () => {
  const html = renderToStaticMarkup(<CloudEnvironmentField value="" onChange={() => undefined} />)
  expect(html).toContain('Run on your organization&#x27;s environment (ID)')
  expect(html).toContain('Optional')
  expect(html).toContain('Saved for this folder')
  expect(html).toContain('aria-invalid="false"')
  expect(html).not.toContain('required=')
})
it('The cloud form marks invalid environment ids and explains how to correct them.', () => {
  const html = renderToStaticMarkup(<CloudEnvironmentField value="bad & calc" onChange={() => undefined} />)
  expect(html).toContain('aria-invalid="true"')
  expect(html).toContain('Use ccpool_ followed by 1 to 80')
})
it('Only a Claude cloud start form offers the organization environment field.', () => {
  const props: ComposerProps = {
    runtimes: [], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
    activeRoute: undefined, error: undefined, mode: 'accept-edits', onModeChange: () => undefined,
    route: { runtime: 'claude', model: 'account-default' }, onRouteChange: () => undefined,
    models: [], resolvedModels: new Map(), recentRoutes: [], platform: 'win32', effort: undefined,
    onEffortChange: () => undefined, 
    onStart: async () => true, onCancel: () => undefined, onOpenRoutePicker: () => undefined,
    onHandOff: () => undefined, handingOff: false, workspaceName: 'Test', workspacePath: 'C:/test',
    onChooseFolder: () => undefined, teammateName: undefined, busyWith: undefined, queued: undefined,
    queuedNote: undefined, onQueue: () => undefined, onUnqueue: () => undefined,
    onSendQueued: () => undefined, continuationNote: undefined, queuedElsewhere: false
  }
  for (const [on, where, present] of [[true, 'claude', true], [false, 'claude', false], [true, 'codex', false]] as const) {
    const html = renderToStaticMarkup(<Composer {...props} cloud={{ on, where, onMode: () => undefined, environment: { value: 'ccpool_test', onChange: () => undefined } }} />)
    expect(html.includes('organization&#x27;s environment (ID)')).toBe(present)
  }
})
