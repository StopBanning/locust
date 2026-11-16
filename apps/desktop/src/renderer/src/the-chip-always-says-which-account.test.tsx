import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, expect, it } from 'vitest'

import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import type { PublicRuntimeStatus } from '../../shared/ipc.js'
import { adoptAccounts, chipAccountName } from './agentAccounts.js'
import type { AgentAccount } from '../../shared/agent-accounts.js'
import { machineWords } from '../../shared/machine-words.js'
import { routeModelName } from './routeName.js'

/**
 * THE CHIP ALWAYS SAYS WHICH ACCOUNT, ONCE THERE IS MORE THAN ONE (0.876.2).
 *
 * Colin, 16 Nov: "How do we see what account is logged into Locust? It's really hard to swap accounts even though we
 * claim to have the ability." On the computer's own sign-in the chip named no account, so beside an added one there was
 * no telling which a conversation would use.
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

const max: AgentAccount = { id: `account_${'a'.repeat(32)}`, runtime: 'claude', name: 'max', state: 'ready' }
const here = `This ${machineWords(undefined).computer}`
afterEach(() => adoptAccounts([]))

it("names nothing extra while the computer's own sign-in is the only account", () => {
  adoptAccounts([])
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).not.toContain(here)
  expect(chipAccountName('claude', undefined, [])).toBeUndefined()
})

it("says This Mac on the computer's own sign-in once another account is added", () => {
  adoptAccounts([max])
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).toContain(` · ${here}`)
  expect(html).not.toContain(' · max')
})

it('says it beside a named model too, and a default whose model is known', () => {
  adoptAccounts([max])
  const opus = renderToStaticMarkup(<Composer {...props} route={{ runtime: 'claude', model: 'claude-opus-5-5' }} />)
  expect(opus).toContain(`${routeModelName('claude', 'claude-opus-5-5')} \u00b7 ${here}`)
  const known = renderToStaticMarkup(<Composer {...props} resolvedModels={new Map([['claude:account-default', 'claude-opus-5-5']])} />)
  expect(known).toContain(`${routeModelName('claude', 'claude-opus-5-5')} \u00b7 ${here}`)
})

it('names the added account when the conversation is on it, or when it is set for new conversations', () => {
  adoptAccounts([max])
  const picked = renderToStaticMarkup(<Composer {...props} route={{ runtime: 'claude', model: 'account-default', account: max.id }} />)
  expect(picked).toContain(' · max')
  expect(picked).not.toContain(here)
  adoptAccounts([{ ...max, forNew: true }])
  const forNew = renderToStaticMarkup(<Composer {...props} />)
  expect(forNew).toContain(' · max')
  expect(forNew).not.toContain(here)
})

it('leaves an agent without accounts alone, and an account of another agent does not count', () => {
  expect(chipAccountName('opencode', undefined, [max])).toBeUndefined()
  expect(chipAccountName('codex', undefined, [max])).toBeUndefined()
  expect(chipAccountName('claude', undefined, [{ ...max, state: 'new' }])).toBe(here)
  expect(chipAccountName('claude', `account_${'f'.repeat(32)}`, [max])).toBe('Removed account')
})
