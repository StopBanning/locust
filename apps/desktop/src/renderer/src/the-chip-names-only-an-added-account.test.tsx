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
 * THE CHIP NAMES AN ADDED ACCOUNT, AND ONLY THAT (0.877.1).
 *
 * Colin, 16 Nov: "How do we see what account is logged into Locust?" 0.876.2 then named both accounts on the chip, the
 * added one or "This Mac". Colin, the same day: "I don't know why this Mac has to be here. Kinda ugly." The added
 * account's name says you are on it; no name is the computer's own sign-in.
 */

const claude = {
  id: 'claude', displayName: 'Claude Code', installed: true, version: '2.1.280', auth: 'authenticated', ready: true, status: 'ready'
} as unknown as PublicRuntimeStatus

const props: ComposerProps = {
  runtimes: [claude], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
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

const max: AgentAccount = { id: `account_${'a'.repeat(32)}`, runtime: 'claude', name: 'max', state: 'ready' }
const here = `This ${machineWords(undefined).computer}`
afterEach(() => adoptAccounts([]))

it("names nothing extra while the computer's own sign-in is the only account", () => {
  adoptAccounts([])
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).not.toContain(here)
  expect(chipAccountName(undefined, [])).toBeUndefined()
})

it("names nothing for the computer's own sign-in, even beside an added account", () => {
  adoptAccounts([max])
  const html = renderToStaticMarkup(<Composer {...props} />)
  expect(html).not.toContain(here)
  expect(html).not.toContain(' · max')
  const opus = renderToStaticMarkup(<Composer {...props} route={{ runtime: 'claude', model: 'claude-opus-5-5' }} />)
  expect(opus).toContain(routeModelName('claude', 'claude-opus-5-5'))
  expect(opus).not.toContain(here)
  const known = renderToStaticMarkup(<Composer {...props} resolvedModels={new Map([['claude:account-default', 'claude-opus-5-5']])} />)
  expect(known).not.toContain(here)
  expect(chipAccountName(undefined, [max])).toBeUndefined()
})

it('names the added account when the conversation is on it, or when it is set for new conversations', () => {
  adoptAccounts([max])
  const picked = renderToStaticMarkup(<Composer {...props} route={{ runtime: 'claude', model: 'account-default', account: max.id }} />)
  expect(picked).toContain(' · max')
  const opus = renderToStaticMarkup(<Composer {...props} route={{ runtime: 'claude', model: 'claude-opus-5-5', account: max.id }} />)
  expect(opus).toContain(`${routeModelName('claude', 'claude-opus-5-5')} · max`)
  adoptAccounts([{ ...max, forNew: true }])
  const forNew = renderToStaticMarkup(<Composer {...props} />)
  expect(forNew).toContain(' · max')
  expect(forNew).not.toContain(here)
})

it('says Removed account for an id that is no longer on the list', () => {
  expect(chipAccountName(`account_${'f'.repeat(32)}`, [max])).toBe('Removed account')
})
