import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { usageSummary } from '../../shared/usage.js'
import type { UsageTurn } from '../../shared/usage.js'
import { TEAM_TEMPLATES } from '../../shared/team-templates.js'
import { startersFor } from './components/IdleTeammate.js'
import { teammateForStarter } from './components/StarterList.js'
import { usageLineFacts } from './usageView.js'

/**
 * A NEW PERSON IS MET WHERE THEY ARE (the new-user walk on 0.843, 2026-10-30): an empty profile, opened as Finder
 * opens it. Four things met them wrong:
 * - Wren offered "Read this project" in the empty folder Locust had just made;
 * - "Use a free model" went the moment Build software was pressed, with the chat box on Claude;
 * - Home's "Build a landing page" went to the agent alone, beside the team they had just made;
 * - one small free question read "1 turn · 59k tokens", and nothing said it cost nothing.
 */
const source = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const mate = (team: 'software' | 'money', name: string) => TEAM_TEMPLATES.find((one) => one.templateId === team)!.teammates.find((one) => one.name === name)!

describe('a teammate in an empty folder', () => {
  it('offers to make something, not to read a project that is not there', () => {
    const wren = mate('software', 'Wren')
    const fresh = startersFor({ role: wren.role, starters: wren.starters }, true)
    expect(fresh).toEqual(wren.freshStarters)
    expect(fresh.join(' ')).not.toMatch(/this project|this repo|riskiest/i)
    // A teammate made by hand takes its role's.
    expect(startersFor({ role: 'Code & Migrations' }, true).join(' ')).toMatch(/Make a small web page here/)
  })

  it('offers its own starters once the folder holds something, and a template teammate without fresh ones keeps its own', () => {
    const wren = mate('software', 'Wren')
    expect(startersFor({ role: wren.role, starters: wren.starters }, false)).toEqual(wren.starters)
    const sable = mate('money', 'Sable')
    expect(startersFor({ role: sable.role, starters: sable.starters }, true)).toEqual(sable.starters)
  })

  it('is told the folder is empty by the window, which asks again after each turn', () => {
    const app = source('./App.tsx')
    expect(app).toContain('folderEmpty={workspaceEmpty}')
    expect(app).toContain('setWorkspaceEmpty(info.workspaceEmpty === true)')
  })
})

describe('Home with a team just made', () => {
  it('keeps the first-run line and its free model until the first conversation', () => {
    const home = source('./components/FirstLaunch.tsx')
    expect(home).toContain("discoveryPhase === 'ready' && (teammateCount === 0 || noConversationsYet) && (")
    expect(source('./App.tsx')).toContain('noConversationsYet={history.length === 0}')
  })

  it('sends a starter row to the teammate whose work it is', () => {
    const team = [
      { teammateId: 'wren', role: 'Code & Migrations' as const },
      { teammateId: 'juno', role: 'Docs & QA' as const },
      { teammateId: 'atlas', role: 'Research & Briefs' as const }
    ]
    expect(teammateForStarter('page', team)).toBe('wren')
    expect(teammateForStarter('debug', team)).toBe('wren')
    expect(teammateForStarter('review', team)).toBe('juno')
    expect(teammateForStarter('review', [team[0]!])).toBe('wren')
    // A team with no coder: the words wait in the box as before.
    expect(teammateForStarter('page', [{ teammateId: 'sable', role: 'Research & Briefs' }])).toBeUndefined()
    expect(source('./App.tsx')).toContain('const suited = teammateForStarter(starter.id, teammates)')
  })
})

describe('the usage line', () => {
  const now = new Date(2026, 9, 30, 21, 0, 0)
  const turn = (fields: Partial<UsageTurn>): UsageTurn => ({ at: new Date(2026, 9, 30, 20, 0, 0).toISOString(), runtime: 'opencode', model: 'deepseek-v4.1-flash-free', conversation: 'c1', inputTokens: 58_000, outputTokens: 1_000, ...fields })

  it('says "all free" when every turn was on a free model', () => {
    expect(usageLineFacts(usageSummary([turn({})], '30d', now))).toBe('1 turn · 59k tokens · all free')
  })

  it('says nothing of free when any turn was not', () => {
    expect(usageLineFacts(usageSummary([turn({}), turn({ runtime: 'claude', model: 'claude-opus-4-6', plan: true, conversation: 'c2' })], '30d', now))).not.toContain('free')
  })
})
