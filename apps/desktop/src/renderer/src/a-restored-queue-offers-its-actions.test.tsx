// Colin: "on reopen show it back in the box's queue with Edit/Discard. Never send it unasked after a relaunch: the person presses Send."
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { QUEUE_RESTORED_NOTE } from './conversationQueue.js'

const props: ComposerProps = {
  runtimes: [], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
  activeRoute: undefined, error: undefined, mode: 'accept-edits', onModeChange: () => undefined,
  route: { runtime: 'opencode', model: 'opencode/free' }, onRouteChange: () => undefined,
  models: [], resolvedModels: new Map(), recentRoutes: [], platform: 'win32', effort: undefined,
  onEffortChange: () => undefined, onStart: async () => true,
  onCancel: () => undefined, onOpenRoutePicker: () => undefined, onHandOff: () => undefined, handingOff: false,
  workspaceName: 'Scratch', workspacePath: 'C:/scratch', onChooseFolder: () => undefined,
  teammateName: 'Ash', busyWith: undefined, queued: 'Please review paragraph two.', queuedNote: QUEUE_RESTORED_NOTE,
  onQueue: () => undefined, onUnqueue: () => undefined, onSendQueued: () => undefined,
  continuationNote: undefined, queuedElsewhere: false
}
it('The restored queue shows the original words, its hold reason and Edit, Discard and Send.', () => {
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).toContain('Please review paragraph two.')
  expect(html).toContain('Saved before Locust closed')
  expect(html).toContain('press Send to continue')
  expect(html).toContain('>Edit</button>')
  expect(html).toContain('>Discard</button>')
  expect(html).toContain('Send now')
  expect(html).not.toContain('Sends when Ash finishes')
})
it('A queue read failure is visible even when no rows can be displayed.', () => {
  const html = renderToStaticMarkup(<Composer {...props} queued={undefined} queueError="Saved messages are unreadable; the file has been kept." />)
  expect(html).toContain('Saved messages are unreadable; the file has been kept.')
})
