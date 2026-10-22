import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicModel, PublicRuntimeStatus, PublicTeammate } from '../../shared/ipc.js'
import { seedAvatar } from '../../shared/avatar.js'
import { Composer } from './components/Composer.js'
import type { ComposerProps } from './components/Composer.js'
import { HomeTeam } from './components/HomeTeam.js'
import { TeammatesScreen } from './components/Screens.js'
import { droppedFreeModel } from './status.js'

/*
 * A FREE MODEL OPENCODE DROPPED IS SAID BEFORE THE SEND (0.819). The 0.817 sweep's teammates on Ling, Space Bunny and
 * LongCat failed at their first send, "Model not found", and only then was another offered. The window knows which
 * free models OpenCode lists, so a route still on one it no longer lists is said on the teammate's card and over the
 * chat box, with the one press that moves off it.
 */
const model = (id: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
const LISTED = ['opencode/deepseek-v4.1-flash-free', 'opencode/glm-5.3-flash-free', 'opencode/big-pickle'].map(model)
const route = (model?: string, runtime = 'opencode') => ({ runtime, ...(model === undefined ? {} : { model }) })

describe('a free model OpenCode dropped', () => {
  it('is known once the catalogue is read, and only for one of OpenCode’s free models', () => {
    expect(droppedFreeModel(route('opencode/longcat-2.5-preview-free'), LISTED)).toBe(true)
    expect(droppedFreeModel(route('opencode/deepseek-v4.1-flash-free'), LISTED)).toBe(false)
    // Before the catalogue is read, nothing is said.
    expect(droppedFreeModel(route('opencode/longcat-2.5-preview-free'), [])).toBe(false)
    // A paid model, one of the person's own, another runtime's, or no model at all: theirs to keep.
    expect(droppedFreeModel(route('opencode/gone-paid'), LISTED)).toBe(false)
    expect(droppedFreeModel(route('mine/cheap-free'), LISTED)).toBe(false)
    expect(droppedFreeModel(route('opencode/longcat-2.5-preview-free', 'claude'), LISTED)).toBe(false)
    expect(droppedFreeModel(route(), LISTED)).toBe(false)
  })

  it('reads amber on the teammate’s card, and its hover says what to do', () => {
    const pip = { teammateId: 'tm_pip', name: 'Pip', hue: 'violet' as const, avatar: seedAvatar('tm_pip'), role: 'Docs & QA', route: 'OpenCode · Longcat 2.5 Preview Free', model: 'Longcat 2.5 Preview Free', runtime: 'opencode' as const, working: false }
    const dropped = renderToStaticMarkup(<HomeTeam team={[{ ...pip, dropped: true }]} onMessage={() => undefined} />)
    expect(dropped).toContain('class="lc-hometeam__route is-dropped" title="OpenCode no longer offers Longcat 2.5 Preview Free. Open Pip to switch."')
    const listed = renderToStaticMarkup(<HomeTeam team={[pip]} onMessage={() => undefined} />)
    expect(listed).toContain('class="lc-hometeam__route" title="OpenCode · Longcat 2.5 Preview Free"')
  })

  it('reads amber on the Team screen’s card too', () => {
    const pip: PublicTeammate = { teammateId: 'tm_pip', name: 'Pip', role: 'Custom', hue: 'violet', avatar: seedAvatar('tm_pip'), route: { runtime: 'opencode', model: 'opencode/longcat-2.5-preview-free', mode: 'accept-edits' }, createdAt: '2026-10-22T05:00:00.000Z' }
    const nothing = (): void => undefined
    const team = (dropped?: ReadonlySet<string>): string => renderToStaticMarkup(<TeammatesScreen teammates={[pip]} missions={[]} missionOwners={{}} spendByTeammate={{}} viewByTeammate={{}}
      titleOf={() => ''} onOpenMission={nothing} onNewTeammate={nothing} onEdit={nothing} onRemove={nothing} onMessage={nothing}
      routines={[]} routineStepByTeammate={{}} onRunRoutine={nothing} onEditRoutine={nothing} onRemoveRoutine={nothing} {...(dropped === undefined ? {} : { droppedTeammates: dropped })} />)
    expect(team(new Set(['tm_pip']))).toContain('class="lc-rostercard__model is-dropped" title="OpenCode no longer offers Longcat 2.5 Preview Free. Message Pip to switch."')
    expect(team()).toContain('class="lc-rostercard__model"')
    expect(team()).not.toContain('is-dropped')
  })

  it('is said over the chat box, with the one press off it', () => {
    const opencode = { id: 'opencode', displayName: 'OpenCode', installed: true, version: '1.18.27', auth: 'none', ready: true, status: 'ready' } as unknown as PublicRuntimeStatus
    const props: ComposerProps = {
      runtimes: [opencode], limitedRuntimes: new Map(), discoveryPhase: 'ready', running: false, cancelling: false,
      activeRoute: undefined, error: undefined, mode: 'accept-edits', onModeChange: () => undefined,
      route: { runtime: 'opencode', model: 'opencode/longcat-2.5-preview-free' }, onRouteChange: () => undefined,
      models: LISTED, resolvedModels: new Map(), recentRoutes: [], platform: 'win32', effort: undefined,
      onEffortChange: () => undefined, swarm: false, onSwarmChange: () => undefined,
      onStart: async () => true, onCancel: () => undefined, onOpenRoutePicker: () => undefined,
      onHandOff: () => undefined, handingOff: false, workspaceName: 'Test', workspacePath: 'C:/test',
      onChooseFolder: () => undefined, teammateName: 'Pip', busyWith: undefined, queued: undefined,
      queuedNote: undefined, onQueue: () => undefined, onUnqueue: () => undefined,
      onSendQueued: () => undefined, continuationNote: undefined, queuedElsewhere: false
    }
    const html = renderToStaticMarkup(<Composer {...props} modelDropped={{ text: 'OpenCode no longer offers Longcat 2.5 Preview Free.', label: 'Switch Pip to Deepseek V4.1 Flash Free', onPress: () => undefined }} />)
    expect(html).toContain('<div class="lc-notice is-plain is-offer" role="status" aria-live="polite">')
    expect(html).toContain('<span class="lc-notice__text">OpenCode no longer offers Longcat 2.5 Preview Free.</span>')
    expect(html).toContain('<button type="button" class="lc-button">Switch Pip to Deepseek V4.1 Flash Free</button>')
    expect(renderToStaticMarkup(<Composer {...props} />)).not.toContain('is-offer')
  })
})
