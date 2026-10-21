import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { seedAvatar } from '../../shared/avatar.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { blindName, setupName, type PublicCompare } from '../../shared/compare.js'
import { tuneRoutes } from '../../shared/tune.js'
import { KeptInstructions } from './components/KeptInstructions.js'

/**
 * A TUNE NAMES ITS SETUPS (0.809). A tune is one model with three sets of instructions, so its blind columns are
 * Setup A, B and C, and once one is kept the bar names the setup that won -- the first pass read "Compared with
 * Deepseek V4.1 Flash Free and Deepseek V4.1 Flash Free", and "Model A" beside "Model B" said the models differed.
 */
const mate: PublicTeammate = { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations', hue: 'lime', avatar: seedAvatar('tm_wren'), createdAt: '2026-10-20T12:00:00.000Z', instructions: 'Be concise.', route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' } }
const drafts = { how: { instructions: 'Check your work.', why: 'It stopped before checking.' }, knows: { instructions: 'Name your assumptions.', why: 'It treated a guess as a fact.' } }
const routes = tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free', label: 'Example Free' }, 'Be concise.', drafts)
const kept = (slot: 'a' | 'b' | 'c'): PublicCompare => ({ compareId: 'cmp_tune', teammateId: mate.teammateId, prompt: 'Explain it.', createdAt: mate.createdAt, blind: true, kept: { slot, at: mate.createdAt }, slots: routes.map((route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${String(index)}`] })) })

describe('a tune names its setups', () => {
  it('as Setup A, B and C while blind, and Model A, B and C in any other comparison', () => {
    expect(blindName('a', true)).toBe('Setup A')
    expect(blindName('c')).toBe('Model C')
  })
  it('names the setup a column tried from its label, and nothing for a column of an ordinary comparison', () => {
    expect(routes.map(setupName)).toEqual(['Current', 'How it works', 'What it knows'])
    expect(setupName({ runtime: 'codex', model: 'gpt-6-luna', label: 'GPT-6-Luna · Low' })).toBeUndefined()
    expect(setupName({ runtime: 'codex', model: 'gpt-6-luna', instructions: 'x' })).toBeUndefined()
  })
  it('says which setup was kept, in the bar over the conversation', () => {
    const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
    // With a skill in the tune (0.811) its setups are not all instructions: "3 setups".
    expect(source).toContain('`You tried ${String(compare.slots.length)} ${compare.slots.some((column) => column.route.skill !== undefined) ? \'setups\' : \'sets of instructions\'} on ${tunedOn ?? \'this model\'} and kept ${keptSetup}.`')
    expect(source).toContain('blindName(column.slot, hasColumnInstructions(compare))')
    expect(source).toContain('blindName(column.slot, hasColumnInstructions(comparing))')
  })
  it('offers the kept setup’s words with its two choices side by side, the words folded beneath', () => {
    const html = renderToStaticMarkup(<KeptInstructions compare={kept('b')} teammate={mate} onUse={async () => undefined} />)
    expect(html).toContain('The setup you kept uses different instructions from Wren’s own.')
    const actions = html.slice(html.indexOf('lc-tuneoffer__actions'), html.indexOf('lc-tuneoffer__review'))
    expect(actions).toContain('Use these instructions for Wren')
    expect(actions).toContain('Not now')
    expect(html.indexOf('lc-tuneoffer__review')).toBeGreaterThan(html.indexOf('Not now'))
    expect(html).toContain('<pre>Check your work.</pre>')
  })
})
