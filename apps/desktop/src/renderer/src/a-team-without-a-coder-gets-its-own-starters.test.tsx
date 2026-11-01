import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { roleLabelOf } from '../../shared/ipc.js'
import { TEAM_TEMPLATES } from '../../shared/team-templates.js'
import { StarterList, STARTERS, startersForTeam, teammateForStarter } from './components/StarterList.js'

/*
 * A TEAM WITHOUT A CODER GETS ITS OWN STARTERS (2026-11-01). Home's four rows are a coder's work, and a new person who
 * pressed Research & money was offered "Debug an issue" and "Build a landing page" with nobody on the team to do them
 * (the first-session drive on 0.845, frames 03 and 14). A team without a coder gets a row for each teammate's own work,
 * by the role on their card, and Plan before making; each goes to the teammate whose work it is.
 */
const teamOf = (templateId: string) =>
  TEAM_TEMPLATES.find((template) => template.templateId === templateId)!.teammates.map((mate) => ({ teammateId: mate.name.toLowerCase(), role: roleLabelOf(mate) }))
const labels = (team: readonly { readonly role: string }[]): string[] => startersForTeam(team).map((starter) => starter.label)

describe('Home’s starters for the team on it', () => {
  it('stay the coder’s four with nobody on the team yet, and for a team with a coder', () => {
    expect(startersForTeam([])).toBe(STARTERS)
    expect(startersForTeam(teamOf('software'))).toBe(STARTERS)
    expect(labels([{ role: 'Research & Briefs' }, { role: 'Code & Migrations' }])).toEqual(STARTERS.map((starter) => starter.label))
  })

  it('give Research & money a row for each teammate’s work, then Plan before making', () => {
    expect(labels(teamOf('money'))).toEqual(['Research a question', 'Work out the numbers', 'Ask the whole team', 'Plan before making'])
  })

  it('give Write & design rows by the titles on their custom roles', () => {
    expect(labels(teamOf('writing'))).toEqual(['Write a first draft', 'Make a design', 'Edit for clarity', 'Plan before making'])
  })

  it('offer each role once, three at most, and keep the four when no role says anything that fits', () => {
    expect(labels([{ role: 'Research & Briefs' }, { role: 'Research & Briefs' }])).toEqual(['Research a question', 'Plan before making'])
    expect(labels([{ role: 'Writer' }, { role: 'Designer' }, { role: 'Editor' }, { role: 'Research & Briefs' }])).toHaveLength(4)
    expect(startersForTeam([{ role: 'Custom' }, { role: 'Astronaut' }])).toBe(STARTERS)
  })

  it('plan without changing anything, and compare answers rather than copies', () => {
    for (const starter of startersForTeam(teamOf('money'))) {
      expect(starter.compareChanges).toBe(false)
      expect(starter.prompt.length).toBeGreaterThan(20)
    }
    expect(startersForTeam(teamOf('writing')).at(-1)).toMatchObject({ id: 'plan-make', mode: 'plan' })
  })
})

describe('who a team’s own row goes to', () => {
  it('goes to the teammate with that role, and Plan before making to the chief of staff', () => {
    const money = teamOf('money')
    expect(teammateForStarter('role:Research & Briefs', money)).toBe('sable')
    expect(teammateForStarter('role:Data & Reporting', money)).toBe('penny')
    expect(teammateForStarter('plan-make', money)).toBe('rook')
  })

  it('goes by a custom role’s title, and Plan before making to the first teammate when there is no chief of staff', () => {
    const writing = teamOf('writing')
    expect(teammateForStarter('role:Designer', writing)).toBe('iris')
    expect(teammateForStarter('plan-make', writing)).toBe('quill')
    expect(teammateForStarter('role:Astronaut', writing)).toBeUndefined()
  })
})

describe('Home, drawing them', () => {
  it('draws the rows it is given, and Home gives it the team’s', () => {
    const html = renderToStaticMarkup(<StarterList starters={startersForTeam(teamOf('money'))} onStart={() => undefined} onCompareStart={() => undefined} />)
    expect(html).toContain('Research a question')
    expect(html).toContain('Plan before making')
    expect(html).not.toContain('Debug an issue')
    expect(readFileSync(join(__dirname, 'components', 'FirstLaunch.tsx'), 'utf8')).toContain('starters={startersForTeam(team)}')
  })
})
