import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import { proposedHandOffs } from '../../shared/chain-proposal.js'
import type { PublicTeammate, RoutineTemplateInfo, TeammateRole } from '../../shared/ipc.js'
import { RoutineDialog } from './components/RoutineDialog.js'
import { templateMeta } from './components/RoutineTemplates.js'
import { chainDraftFrom, routineChain } from './routines.js'

/**
 * A CHAIN TEMPLATE OPENS IN THE EDITOR (2026-10-05) with a teammate proposed
 * for each step's role and the last step marked as the checker; the person
 * changes any of it before saving, and the saved card names the chain.
 */
const mate = (teammateId: string, name: string, role: TeammateRole): PublicTeammate =>
  ({ teammateId, name, hue: 'blue', avatar: seedAvatar(teammateId), role, createdAt: '2026-10-05T00:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/free', mode: 'accept-edits' } }) as PublicTeammate
const WREN = mate('tm_wren', 'Wren', 'Code & Migrations')
const ATLAS = mate('tm_atlas', 'Atlas', 'Research & Briefs')
const SABLE = mate('tm_sable', 'Sable', 'Docs & QA')
const FIX = [{ role: 'Research & Briefs' }, { role: 'Code & Migrations' }, { role: 'Docs & QA', check: true }]

const editor = (team: readonly PublicTeammate[], inCopy = true, withRoles = true) => {
  const { owner, handOffs } = proposedHandOffs(FIX, team)
  return renderToStaticMarkup(
    <RoutineDialog
      teammate={team.find((entry) => entry.teammateId === owner)}
      initialName="Fix a bug, then check the fix"
      initialSteps={['Find the cause.', 'Fix it.', 'Check the fix.']}
      initialSchedule={undefined}
      team={team}
      initialHandOffs={handOffs}
      initialStepRoles={withRoles ? FIX.map((step) => step.role) : undefined}
      modeName="Edit"
      modeReadsOnly={false}
      initialInCopy={inCopy}
      truncated={false}
      routeLabel={undefined}
      busy={false}
      error={undefined}
      onSave={() => undefined}
      onCancel={() => undefined}
    />
  )
}

describe('the editor, opened on a chain', () => {
  it('shows who takes each step, the checker marked on the last, and the run set to change files in a copy', () => {
    const html = editor([WREN, ATLAS, SABLE])
    expect(html.match(/aria-label="Who takes step \d"/g)).toHaveLength(3)
    // Atlas runs it (the cause), Wren takes the fix, Sable checks.
    expect(html).toContain('Atlas (runs it)')
    expect(html).toMatch(/aria-label="Who takes step 2"[\s\S]*?lc-select__value">Wren</)
    expect(html).toMatch(/aria-label="Who takes step 3"[\s\S]*?lc-select__value">Sable</)
    expect(html.match(/type="checkbox" checked=""/g)).toHaveLength(1)
    expect(html).toContain('In a copy, you keep')
    expect(html).toMatch(/aria-checked="true"[^>]*>In a copy, you keep/)
  })

  it('says beside each step the role it was proposed for, and nothing when the routine has no roles', () => {
    const html = editor([WREN, ATLAS, SABLE])
    expect(html).toContain('Role: Research &amp; Briefs')
    expect(html).toContain('Role: Code &amp; Migrations')
    expect(html).toContain('Role: Docs &amp; QA')
    expect(editor([WREN, ATLAS, SABLE], true, false)).not.toContain('Role: ')
  })

  it('keeps the checker for a teammate on their own: every step is theirs and the last still has to approve', () => {
    const html = editor([WREN])
    expect(html.match(/aria-label="Who takes step \d"/g)).toHaveLength(3)
    expect(html).toContain('Wren (runs it)')
    expect(html.match(/type="checkbox" checked=""/g)).toHaveLength(1)
  })

  it('still draws no choice for one teammate on a plain routine', () => {
    expect(renderToStaticMarkup(
      <RoutineDialog teammate={WREN} initialName="n" initialSteps={['a', 'b']} initialSchedule={undefined} team={[WREN]} initialHandOffs={[{}, {}]} truncated={false} routeLabel={undefined} busy={false} error={undefined} onSave={() => undefined} onCancel={() => undefined} />
    )).not.toContain('Who takes step')
  })
})

describe('the saved card', () => {
  const routine = { teammateId: 'tm_atlas', steps: ['a', 'b', 'c'] }

  it('names the chain Atlas → Wren → Sable (checks), as any hand-off chain reads', () => {
    const { handOffs } = proposedHandOffs(FIX, [WREN, ATLAS, SABLE])
    expect(routineChain({ ...routine, handOffs }, [WREN, ATLAS, SABLE])).toBe('Atlas → Wren → Sable (checks)')
  })

  it('says the checker is the same teammate when there is only one', () => {
    const { handOffs } = proposedHandOffs(FIX, [WREN])
    expect(routineChain({ teammateId: 'tm_wren', steps: ['a', 'b', 'c'], handOffs }, [WREN])).toBe('Wren → Wren (checks)')
  })

  it('names a model once when the whole chain runs on it', () => {
    const { handOffs } = proposedHandOffs(FIX, [WREN, ATLAS, SABLE])
    expect(routineChain({ ...routine, handOffs }, [WREN, ATLAS, SABLE], true)).toMatch(/^Atlas → Wren → Sable \(checks\), all on /)
  })
})

describe('the template list', () => {
  const template = (extra: Partial<RoutineTemplateInfo> = {}): RoutineTemplateInfo => ({ id: 'fix-a-bug', name: 'Fix a bug, then check the fix', summary: 's', steps: 3, asks: ['What is going wrong?'], ...extra })

  it('says a chain hands work on, and says nothing new about the others', () => {
    expect(templateMeta(template({ chain: true }))).toBe('3 steps · hand-offs · 1 question')
    expect(templateMeta(template())).toBe('3 steps · 1 question')
  })
})

describe('the draft the editor opens on', () => {
  const preview = { name: 'Fix a bug, then check the fix', steps: ['Find the cause.', 'Fix it.', 'Check the fix.'], inputs: [], handOffRoles: ['Research & Briefs', 'Code & Migrations', 'Docs & QA'], handOffChecks: [false, false, true] }
  const fallback = { runtime: 'codex', model: 'account-default', mode: 'ask' } as const

  it('proposes the roles, marks the checker, and opens a chain that changes files set to change them, in a copy', () => {
    const draft = chainDraftFrom({ ...preview, changesFiles: true }, [WREN, ATLAS, SABLE], fallback)
    expect(draft).toMatchObject({ teammateId: 'tm_atlas', name: 'Fix a bug, then check the fix', inCopy: true, forceMode: 'accept-edits', route: { runtime: 'opencode', mode: 'accept-edits' } })
    expect(draft.handOffs).toEqual([{}, { teammateId: 'tm_wren' }, { teammateId: 'tm_sable', check: true }])
    expect(draft.stepRoles).toEqual(['Research & Briefs', 'Code & Migrations', 'Docs & QA'])
  })

  it('opens a chain that only reads as it reads: no copy, no forced mode, the own mode of the teammate', () => {
    const draft = chainDraftFrom(preview, [WREN, ATLAS, SABLE].map((entry) => ({ ...entry, route: { runtime: 'opencode', model: 'opencode/free', mode: 'ask' as const } })), fallback)
    expect(draft).not.toHaveProperty('inCopy')
    expect(draft).not.toHaveProperty('forceMode')
    expect(draft.route?.mode).toBe('ask')
  })

  it('replays a teammate with no route of their own on the one the box shows, changing files when the chain does', () => {
    const bare = { ...WREN, route: undefined } as PublicTeammate
    expect(chainDraftFrom({ ...preview, changesFiles: true }, [bare], fallback).route).toEqual({ runtime: 'codex', model: 'account-default', mode: 'accept-edits' })
  })

  it('has nobody to propose and no route when the roster is empty (the editor then says to add a teammate first)', () => {
    const draft = chainDraftFrom({ ...preview, changesFiles: true }, [], fallback)
    expect(draft.teammateId).toBeUndefined()
    expect(draft).not.toHaveProperty('route')
    expect(draft.handOffs).toEqual([{}, {}, { check: true }])
  })
  it('gives a Council review step to a teammate on another agent, as its template says (0.837)', () => {
    const council = { name: 'Council review: three AIs check a change', steps: ['Review it.', 'Review it again.', 'Settle it.'], inputs: [], handOffRoles: [undefined, undefined, undefined], handOffChecks: [false, false, true], handOffOnAnotherAgent: [false, true, true] }
    const team = [WREN, ATLAS, SABLE].map((entry, at) => ({ ...entry, route: { runtime: ['claude', 'claude', 'codex'][at]!, model: 'm', mode: 'ask' } }) as PublicTeammate)
    expect(chainDraftFrom(council, team, fallback).handOffs).toEqual([{}, { teammateId: 'tm_sable' }, { teammateId: 'tm_atlas', check: true }])
  })
})
