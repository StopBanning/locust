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

describe('a conversation waiting on the person', () => {
  it('stands above a running row with an amber mark and words, even when nested', () => {
    const html = sidebar([row('working', { phase: 'running' }), row('waiting', { needsYou: true, nestedUnder: 'working' })])
    expect(html.indexOf('Conversation waiting')).toBeLessThan(html.indexOf('Conversation working'))
    expect(html).toContain('lc-dot lc-tone-amber')
    expect(html).toContain('>needs you</span>')
    expect(html.match(/>Conversation waiting<\/span>/g)).toHaveLength(1)
  })
})
