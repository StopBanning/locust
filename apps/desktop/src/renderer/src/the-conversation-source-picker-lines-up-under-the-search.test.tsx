import { readFileSync } from 'node:fs'
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

const css = readFileSync(new URL('./shell.css', import.meta.url), 'utf8')
const rule = (selector: string): string => {
  const start = css.indexOf('\n' + selector + ' ' + String.fromCharCode(123))
  return start < 0 ? '' : css.slice(start, css.indexOf(String.fromCharCode(125), start))
}

describe('the conversation source picker', () => {
  // case: Colin, 22 Nov, on a screenshot of the sidebar's top: "This is kind of messy." The picker sat in the
  // faces' narrowing slab, a grey band wider than the search box and pressed against it.
  it('sits in its own row, not in the faces narrowing slab', () => {
    const html = sidebar([row('from_app', { appStarted: true })])
    const at = html.indexOf('aria-label="Conversation source"')
    expect(at).toBeGreaterThan(0)
    const opened = html.lastIndexOf('<div class="', at)
    expect(html.slice(opened, at)).toContain('<div class="lc-sidebar__source">')
    expect(html.slice(opened, at)).not.toContain('lc-faces__clear')
  })

  it('has no slab of its own and lines its words up with the search box', () => {
    const own = rule('.lc-sidebar__source')
    expect(own).not.toMatch(/background/)
    // 14px, the search box's inset, less the control's own 6px padding.
    expect(own).toMatch(/margin: var\(--lc-space-2\) 14px 0 8px;/)
    expect(rule('.lc-search')).toMatch(/margin: 0 14px;/)
  })
})
