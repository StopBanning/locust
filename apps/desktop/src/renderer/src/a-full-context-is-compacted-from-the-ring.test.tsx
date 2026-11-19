import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { compactAfterTurn, compactConsequence, compactOffer } from './compactNow.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { ContextRingMenu } from './components/ContextRing.js'
import { latestContext } from './cost.js'

/**
 * A FULL CONTEXT IS COMPACTED FROM THE RING (0.797).
 *
 * The OpenCode 2 docs review, 2026-10-19 (gap J): OpenCode offers "Compact
 * session" and Claude Code says to run /compact as its context fills. Locust
 * had /compact only in the `/` menu. The ring is now a button: it opens what
 * it says, with Compact now beneath, which sends /compact as the next message.
 */

const claudeCommands = { claude: [{ name: 'compact', description: 'Clear conversation history but keep a summary', argumentHint: '' }] }

const ready = {
  runtime: 'claude',
  nextRuntime: 'claude',
  runtimeName: 'Claude Code',
  commands: claudeCommands,
  running: false,
  queued: false,
  elsewhere: false
} as const

describe('Compact now', () => {
  it('is offered for a conversation whose runtime lists /compact', () => {
    expect(compactOffer(ready)).toEqual({ kind: 'ready' })
    expect(compactOffer({ ...ready, runtime: 'codex', nextRuntime: 'codex', commands: { codex: [{ name: 'compact', description: '', argumentHint: '' }] } })).toEqual({ kind: 'ready' })
  })

  it('is not offered where the runtime lists no /compact, or the message would go elsewhere', () => {
    expect(compactOffer({ ...ready, commands: { claude: [{ name: 'init', description: '', argumentHint: '' }] } })).toBeUndefined()
    expect(compactOffer({ ...ready, commands: undefined })).toBeUndefined()
    expect(compactOffer({ ...ready, runtime: 'cursor', nextRuntime: 'cursor' })).toBeUndefined()
    expect(compactOffer({ ...ready, runtime: undefined })).toBeUndefined()
    // A comparison, a cloud or background turn, an earlier message being edited.
    expect(compactOffer({ ...ready, elsewhere: true })).toBeUndefined()
  })

  it('waits in line behind a running turn, as a message sent then does (Orca does the same)', () => {
    expect(compactOffer({ ...ready, running: true })).toEqual({ kind: 'after-turn' })
    const html = renderToStaticMarkup(
      <ContextRingMenu reading={{ usedTokens: 164_000, windowTokens: 200_000, percent: 82 }} offer={{ kind: 'after-turn' }} runtimeName="Claude Code" onCompact={() => undefined} />
    )
    expect(html).toContain('Compact when this turn ends')
    expect(html).toContain(compactAfterTurn('Claude Code'))
    expect(html).not.toContain('disabled')
  })

  it('says why it cannot, with a message already waiting or another model picked', () => {
    expect(compactOffer({ ...ready, running: true, queued: true })).toEqual({
      kind: 'refused',
      reason: 'A message is already waiting for this turn to end. Compact after it goes.'
    })
    expect(compactOffer({ ...ready, nextRuntime: 'codex' })).toEqual({
      kind: 'refused',
      reason: 'Your next message goes to another model. Pick Claude Code again to compact this conversation.'
    })
  })

  it('opens under the reading, the way the hover says it, with what it does', () => {
    const html = renderToStaticMarkup(
      <ContextRingMenu
        reading={{ usedTokens: 164_000, windowTokens: 200_000, percent: 82 }}
        conversationCost="$1.20"
        offer={{ kind: 'ready' }}
        runtimeName="Claude Code"
        onCompact={() => undefined}
      />
    )
    expect(html).toContain('role="menu"')
    expect(html).toContain('Context: 164k of 200k used, 82%')
    expect(html).toContain('This conversation: $1.20')
    expect(html).toContain('Compact now')
    expect(html).toContain(compactConsequence('Claude Code'))
    expect(html).not.toContain('disabled')
  })

  it('shows the reason in place of what it does when it waits', () => {
    const html = renderToStaticMarkup(
      <ContextRingMenu
        reading={{ usedTokens: 164_000, windowTokens: 200_000, percent: 82 }}
        offer={{ kind: 'refused', reason: 'A message is already waiting for this turn to end. Compact after it goes.' }}
        runtimeName="Claude Code"
        onCompact={() => undefined}
      />
    )
    expect(html).toContain('disabled=""')
    expect(html).toContain('A message is already waiting for this turn to end. Compact after it goes.')
    expect(html).not.toContain('This conversation:')
  })
})

describe('the ring in the box', () => {
  const props: ComposerProps = {
    runtimes: [], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
    activeRoute: { runId: 'r', missionId: 'm', runtime: 'claude', model: 'haiku', resolvedRouteId: 'claude:haiku', cliVersion: null } as ComposerProps['activeRoute'],
    error: undefined, mode: 'accept-edits', onModeChange: () => undefined,
    route: { runtime: 'claude', model: 'haiku' }, onRouteChange: () => undefined,
    models: [], resolvedModels: new Map(), recentRoutes: [], platform: 'win32', effort: undefined,
    onEffortChange: () => undefined, 
    onStart: async () => true, onCancel: () => undefined, onOpenRoutePicker: () => undefined,
    onHandOff: () => undefined, handingOff: false, workspaceName: 'Test', workspacePath: 'C:/test',
    onChooseFolder: () => undefined, teammateName: undefined, busyWith: undefined, queued: undefined,
    queuedNote: undefined, onQueue: () => undefined, onUnqueue: () => undefined,
    onSendQueued: () => undefined, continuationNote: undefined, queuedElsewhere: false,
    context: { usedTokens: 164_000, windowTokens: 200_000, percent: 82 }
  }

  it('is a button that opens a menu when Compact now can be offered', () => {
    const html = renderToStaticMarkup(<Composer {...props} runtimeCommands={claudeCommands} />)
    expect(html).toMatch(/<button[^>]*class="lc-control lc-contextring__button"[^>]*aria-haspopup="menu"/)
    expect(html).toContain('Context: 164k of 200k used, 82%')
  })

  it('is a button while a turn is still starting, before it has a route of its own', () => {
    // The compact drive pressed the ring two seconds into "Starting Claude Code..." and nothing opened.
    const html = renderToStaticMarkup(<Composer {...props} running activeRoute={undefined} runtimeCommands={claudeCommands} />)
    expect(html).toContain('lc-contextring__button')
  })

  it('stays a picture where it cannot', () => {
    const html = renderToStaticMarkup(<Composer {...props} runtimeCommands={{}} />)
    expect(html).toContain('lc-contextring')
    expect(html).not.toContain('lc-contextring__button')
  })
})

describe('after a compaction', () => {
  const event = (type: string, payload: Record<string, unknown>): NormalizedRuntimeEvent =>
    ({ id: 'e', runId: 'r', missionId: 'm', sequence: 1, occurredAt: '2026-10-19T06:00:00.000Z', sourceAdapter: 'claude', type, payload } as unknown as NormalizedRuntimeEvent)
  const turn = (used: number) => ({ events: [event('run.completed', { usage: { contextTokens: used, contextWindow: 200_000 }, process: {} })] })
  const compacted = [
    event('adapter.diagnostic', { code: 'claude.context_compacted', level: 'info', terminal: false, message: 'Claude Code summarized the conversation so far, as asked, and carries on from the summary.' }),
    event('run.completed', { usage: { inputTokens: 10, outputTokens: 400 }, process: {} })
  ]

  it('the ring no longer reads the conversation as it was before', () => {
    expect(latestContext([turn(164_000)], compacted)).toBeUndefined()
    expect(latestContext([turn(164_000), { events: compacted }], [])).toBeUndefined()
  })

  it('and reads again from the next turn that reports', () => {
    expect(latestContext([turn(164_000), { events: compacted }], turn(18_000).events)?.usedTokens).toBe(18_000)
  })
})
