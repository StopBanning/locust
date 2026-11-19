import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import type { PublicRuntimeStatus } from '../../shared/ipc.js'

/**
 * The composer's sentence about hung coding agents, counted.
 *
 * Grok, pass 16, the extra finding: the box read *"A coding agent is installed
 * but not answering"* over FIVE rows that each said so and each carried their
 * own Check again. The list was plural, the buttons were plural, and the box
 * you type into was not.
 *
 * Fixed in 0.201.0, and the plan then said to **re-verify it on Grok's next
 * pass** rather than trust my own count. That was the wrong instrument. A
 * tester noticing again is not a guarantee, it is a coincidence with good
 * habits -- and the sentence is a pure function of props, so it can be pinned
 * here and stay pinned. This is that.
 *
 * THE SECOND CLAIM IS THE HARDER ONE: the count is only the runtimes this
 * sentence is ABOUT -- installed, and still not usable after discovery gave
 * up. A machine with one working CLI and one hung one is not "2 coding agents
 * are not answering", and an off-by-one in the *other* direction would be a
 * sentence that overstates how broken the machine is, on the screen of
 * somebody deciding whether this app works at all.
 */

const runtime = (
  id: PublicRuntimeStatus['id'],
  displayName: string,
  state: 'ready' | 'hung' | 'absent'
): PublicRuntimeStatus => ({
  id,
  displayName,
  installed: state !== 'absent',
  version: state === 'ready' ? '1.18.27' : null,
  auth: state === 'ready' ? 'authenticated' : 'unknown',
  ready: state === 'ready',
  /*
   * `probe-failed` is what a hung CLI actually reports: it is on the machine,
   * and asking it for a version did not come back. There is no `checking`
   * status -- the first draft of this file invented one, vitest never
   * noticed because it does not typecheck, and `pnpm check` caught it.
   */
  status: state === 'ready' ? 'ready' : state === 'hung' ? 'probe-failed' : 'not-installed'
})

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

/** Discovery has given up, and these are the runtimes it gave up on. */
const afterGivingUp = (runtimes: readonly PublicRuntimeStatus[]): string =>
  renderToStaticMarkup(<Composer {...props({ runtimes, runtimesGaveUp: true })} />)

const hung = (n: number): readonly PublicRuntimeStatus[] =>
  Array.from({ length: n }, (_, i) => runtime(`hung${String(i)}` as PublicRuntimeStatus['id'], `Agent ${String(i)}`, 'hung'))

describe('the composer says how many agents are not answering', () => {
  it('says one in the singular', () => {
    const seen = afterGivingUp(hung(1))
    expect(seen).toContain('An AI agent is installed but not answering')
    expect(seen).not.toMatch(/\d+ AI agents/)
  })

  it('counts five as five, not as one', () => {
    // The exact shape Grok found: five rows that each said so, and a box that
    // said "A coding agent".
    const seen = afterGivingUp(hung(5))
    expect(seen).toContain('5 AI agents are installed but not answering')
    expect(seen).not.toContain('An AI agent is installed but not answering')
  })

  it('counts only what is installed, not every runtime it knows about', () => {
    /*
     * Two hung, two never installed. The sentence is about the two that are
     * HERE. Saying "4 coding agents are not answering" would overstate how
     * broken the machine is, on the screen of somebody deciding whether this
     * app works at all -- the same defect Grok found, pointed the other way.
     */
    const seen = afterGivingUp([
      runtime('codex', 'Codex CLI', 'hung'),
      runtime('claude', 'Claude Code', 'hung'),
      runtime('cursor', 'Cursor Agent', 'absent'),
      runtime('copilot', 'Copilot CLI', 'absent')
    ])
    expect(seen).toContain('2 AI agents are installed but not answering')
    expect(seen).not.toMatch(/[34] AI agents/)
  })

  it('does not complain about hung agents when one of them works', () => {
    /*
     * Found by this test file getting its own premise wrong, which is worth
     * keeping: with a usable runtime on the machine the composer stops
     * talking about the hung ones entirely and names the step that works --
     * "OpenCode is ready — switch the route to it". That is the right call
     * and nothing pinned it, so it could have been lost to a later edit of
     * the branch above.
     */
    const seen = afterGivingUp([
      runtime('opencode', 'OpenCode', 'ready'),
      runtime('codex', 'Codex CLI', 'hung'),
      runtime('claude', 'Claude Code', 'hung')
    ])
    expect(seen).not.toContain('not answering')
    expect(seen).toContain('OpenCode is ready')
  })

  it('says nothing of the kind while discovery is still asking', () => {
    // Not answering YET is not the same as not answering, and 0.197.0 was
    // spent on exactly that distinction elsewhere on the screen.
    const seen = renderToStaticMarkup(<Composer {...props({ runtimes: hung(3), runtimesGaveUp: false })} />)
    expect(seen).not.toContain('not answering')
  })
})
