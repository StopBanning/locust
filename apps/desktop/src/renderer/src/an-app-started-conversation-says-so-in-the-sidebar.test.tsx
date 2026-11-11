import { appStartedConversation, conversationSourceMatches } from './conversationList.js'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { Sidebar } from './components/Sidebar.js'
import type { SidebarMission } from './components/Sidebar.js'

const wren = { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-11T05:00:00.000Z', avatar: seedAvatar('tm_wren') } as PublicTeammate
const row = (missionId: string, extra: Partial<SidebarMission> = {}): SidebarMission =>
  ({ missionId, title: `Conversation ${missionId}`, phase: 'completed', integrityIssueCount: 0, lastAt: '2026-10-11T05:00:00.000Z', ...extra }) as SidebarMission
const noop = (): void => undefined

function sidebar(missions: readonly SidebarMission[]): string {
  return renderToStaticMarkup(
    <Sidebar
      runtimes={[]} missions={missions} teammates={[wren]} viewByTeammate={{}} routineStepByTeammate={{}}
      missionOwners={Object.fromEntries(missions.map((mission) => [mission.missionId, 'tm_wren']))}
      selectedMissionId={undefined} selectedTeammateId={undefined} onSelectMission={noop} onMissionMenu={noop}
      onTeammateMenu={noop} pendingApprovals={{}} liveActivity={{}} starting={[]} recentlyDone={[]} recentlyReceived={[]}
      onSelectTeammate={noop} onNewConversationWith={noop} onOpenHub={noop} onAddMenu={noop} composerShown={false}
      onOpenSettings={noop} onOpenMissions={noop} onOpenTeammates={noop} rooms={[]} routines={[]} currentRoomId={undefined}
      onOpenRoom={noop} onOpenRooms={noop} onOpenAutomations={noop} onHome={noop} groups={[]}
    />
  )
}

describe('an app-started conversation in the sidebar', () => {
  it('has a quiet accessible origin mark and offers the source filter', () => {
    const html = sidebar([row('from_app', { appStarted: true }), row('from_person')])
    expect(html).toContain('class="lc-conv__age" role="img" aria-label="Started by another app through Locust">via app</span>')
    expect(html.match(/>via app<\/span>/g)).toHaveLength(1)
    expect(html).toContain('aria-label="Conversation source"')
    expect(html).toContain('All conversations')
    expect(html).toContain('aria-haspopup="listbox"')
  })

  it('leaves ordinary rows quiet and offers no unnecessary filter', () => {
    const html = sidebar([row('person')])
    expect(html).not.toContain('via app')
    expect(html).not.toContain('Conversation source')
  })

  it('filters app starts independently of search and teammate selection', () => {
    const app = row('app', { appStarted: true })
    const person = row('person')
    expect([app, person].filter((mission) => conversationSourceMatches(mission, 'all'))).toEqual([app, person])
    expect([app, person].filter((mission) => conversationSourceMatches(mission, 'app'))).toEqual([app])
    expect([app, person].filter((mission) => conversationSourceMatches(mission, 'person'))).toEqual([person])
  })

  it('keeps the first turn origin when either the app or person follows up, live and restored', () => {
    expect(appStartedConversation({ startedBy: { kind: 'mcp' } })).toBe(true)
    expect(appStartedConversation({})).toBe(false)
    expect(appStartedConversation({ startedBy: { kind: 'mcp' } }, {})).toBe(false)
    expect(appStartedConversation({}, { startedBy: { kind: 'mcp' } })).toBe(true)
    expect(appStartedConversation({ earlierTurns: [{ startedBy: { kind: 'mcp' } }] })).toBe(true)
    expect(appStartedConversation({ startedBy: { kind: 'mcp' }, earlierTurns: [{}] })).toBe(false)
    expect(appStartedConversation({ startedBy: { kind: 'routine' } })).toBe(false)
  })
})
