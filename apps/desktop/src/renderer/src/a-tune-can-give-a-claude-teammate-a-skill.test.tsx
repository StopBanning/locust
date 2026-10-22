import { renderToStaticMarkup } from 'react-dom/server'
import type { ReactElement } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { seedAvatar } from '../../shared/avatar.js'
import type { PublicCompare } from '../../shared/compare.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { TUNE_SKILL_INCOMPLETE, tuneRoutes, type PublicTeammateSkill, type TuneSkill } from '../../shared/tune.js'
import { KeptInstructions } from './components/KeptInstructions.js'
import { NewTeammateDialog } from './components/NewTeammateDialog.js'
import { TuneTeammate, tuneSkillReady, type TuneCard } from './components/TuneTeammate.js'

vi.mock('react', async (original) => ({
  ...(await original<typeof import('react')>()),
  useState: (initial: unknown) => [typeof initial === 'function' ? (initial as () => unknown)() : initial, vi.fn()],
  useRef: (initial: unknown) => ({ current: initial }),
  useEffect: vi.fn()
}))
vi.mock('./useModal.js', () => ({ useModal: vi.fn() }))

/*
 * WHAT IT KNOWS, AS A SKILL (0.811). For a Claude Code teammate the tune card shows "What it knows" as a skill -- its
 * name, when to use it, what it says -- beside Current's instructions; the kept strip offers it as the kept
 * instructions are offered, Use this skill for <name>; and Edit teammate lists the skills it keeps, each with Remove.
 */
const skill: TuneSkill = {
  name: 'check-the-cause',
  description: 'Use when explaining why something failed: name the evidence before the cause.',
  body: '- Quote the line that shows the failure.'
}
const mate: PublicTeammate = {
  teammateId: 'tm_wren',
  name: 'Wren',
  role: 'Code & Migrations',
  hue: 'lime',
  avatar: seedAvatar('tm_wren'),
  createdAt: '2026-10-21T12:00:00.000Z',
  instructions: 'Be concise.',
  route: { runtime: 'claude', model: 'claude-haiku-5-5', mode: 'accept-edits' }
}
const how = { instructions: 'Be concise. Check your work before you finish.', why: 'The answer stopped before checking.' }
const knows = { instructions: 'Be concise.', why: 'The answer named a cause with no evidence.', skill }
const card: TuneCard = { id: 1, teammateId: mate.teammateId, route: mate.route!, name: 'Wren', model: 'Haiku 5.5', free: false, prompt: 'Why did it fail?', current: 'Be concise.', drafts: { how, knows }, drafting: false }
const tune = (slot: 'a' | 'b' | 'c'): PublicCompare => ({
  compareId: 'cmp_skill',
  teammateId: mate.teammateId,
  prompt: 'Why did it fail?',
  createdAt: mate.createdAt,
  blind: true,
  kept: { slot, at: mate.createdAt },
  slots: tuneRoutes({ runtime: 'claude', model: 'claude-haiku-5-5', label: 'Haiku 5.5' }, 'Be concise.', { how, knows }).map((route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${String(index)}`] }))
})
const kept: PublicTeammateSkill = { teammateId: mate.teammateId, name: skill.name, description: skill.description, compareId: 'cmp_skill', createdAt: mate.createdAt }

const flatChildren = (node: unknown): ReactElement[] => {
  if (typeof node !== 'object' || node === null || !('props' in node)) return []
  const element = node as ReactElement<{ children?: unknown }>
  const kids = [element.props.children ?? []].flat(Infinity)
  return [element, ...kids.flatMap(flatChildren)]
}
const buttonNamed = (tree: ReactElement, name: string): ReactElement<{ onClick: () => void; disabled?: boolean }> | undefined =>
  flatChildren(tree).find((el) => el.type === 'button' && (el.props as { children?: unknown }).children === name) as ReactElement<{ onClick: () => void; disabled?: boolean }> | undefined

describe('the tune card of a Claude Code teammate', () => {
  const screen = (over: Partial<TuneCard> = {}): string => renderToStaticMarkup(<TuneTeammate card={{ ...card, ...over }} onChange={() => undefined} onCancel={() => undefined} onRun={() => undefined} />)

  it('shows What it knows as a skill beside Current’s instructions: its name, when to use it, and what it says', () => {
    const html = screen()
    expect(html).toContain('aria-label="What it knows skill"')
    expect(html).toContain('A skill beside Current’s instructions: Claude Code loads it only when the work matches when to use it.')
    expect(html).toContain('value="check-the-cause"')
    expect(html).toContain('aria-label="What it knows skill: when to use it"')
    expect(html).toContain('- Quote the line that shows the failure.')
    // Current and How it works stay instructions.
    expect(html).toContain('aria-label="Current instructions"')
    expect(html).toContain('aria-label="How it works instructions"')
    expect(html).not.toContain('aria-label="What it knows instructions"')
    expect(html).not.toContain(TUNE_SKILL_INCOMPLETE)
  })

  it('holds Run all three until the skill reads whole, and says what it needs', () => {
    const broken = { ...card, drafts: { how, knows: { ...knows, skill: { ...skill, name: 'check-' } } } }
    expect(tuneSkillReady(card)).toBe(true)
    expect(tuneSkillReady(broken)).toBe(false)
    const html = screen(broken)
    expect(html).toContain(TUNE_SKILL_INCOMPLETE)
    expect(html).toMatch(/<button type="button" class="lc-primarybutton" disabled="">Run all three<\/button>/)
  })

  it('stays instructions for a teammate whose draft brought no skill, with a switch to write one', () => {
    const html = screen({ drafts: { how, knows: { instructions: 'Be concise. Name the evidence.', why: 'x' } } })
    expect(html).toContain('aria-label="What it knows instructions"')
    expect(html).not.toContain('skill name')
    expect(html).toContain('Write a skill instead')
    // Claude Code, Codex and OpenCode take a skill (0.813); an agent that does not, such as Gemini, is not offered one.
    expect(screen({ route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' }, drafts: { how, knows: { instructions: 'x', why: 'x' } } })).toContain('Write a skill instead')
    expect(screen({ route: { runtime: 'gemini', model: 'account-default', mode: 'accept-edits' }, drafts: { how, knows: { instructions: 'x', why: 'x' } } })).not.toContain('Write a skill instead')
  })

  it('switches either way round, setting a skill aside rather than losing it', () => {
    const edits: ((current: TuneCard) => TuneCard)[] = []
    const tree = TuneTeammate({ card, onChange: (edit) => edits.push(edit), onCancel: () => undefined, onRun: () => undefined })
    buttonNamed(tree, 'Use instructions instead')!.props.onClick()
    const plain = edits.at(-1)!(card)
    expect(plain.drafts.knows).toEqual({ instructions: 'Be concise.', why: knows.why })
    expect(plain.skillAside).toEqual(skill)
    const again = TuneTeammate({ card: plain, onChange: (edit) => edits.push(edit), onCancel: () => undefined, onRun: () => undefined })
    buttonNamed(again, 'Write a skill instead')!.props.onClick()
    const back = edits.at(-1)!(plain)
    expect(back.drafts.knows.skill).toEqual(skill)
    expect(back.skillAside).toBeUndefined()
    // With none set aside, an empty one to write, and Run all three waits for it.
    const fresh = edits.at(-1)!({ ...card, drafts: { how, knows: { instructions: 'x', why: 'x' } } })
    expect(fresh.drafts.knows.skill).toEqual({ name: '', description: '', body: '' })
    expect(tuneSkillReady(fresh)).toBe(false)
  })
})

describe('the skill a kept tune offers', () => {
  it('is offered with Use this skill for the teammate, its words to review, and used only when pressed', async () => {
    const onUseSkill = vi.fn(async () => undefined)
    const tree = KeptInstructions({ compare: tune('c'), teammate: mate, onUse: async () => undefined, skills: [], onUseSkill })!
    const html = renderToStaticMarkup(tree)
    expect(html).toContain('The setup you kept adds a skill, check-the-cause.')
    expect(html).toContain('Use this skill for Wren')
    expect(html).toContain('Review the skill')
    expect(html).toContain('When to use it: Use when explaining why something failed: name the evidence before the cause.')
    // Its instructions are Wren’s own, so only the skill is offered; and a skill is added, not combined.
    expect(html).not.toContain('Use these instructions for Wren')
    expect(html).not.toContain('Combine both edits')
    expect(onUseSkill).not.toHaveBeenCalled()
    buttonNamed(tree, 'Use this skill for Wren')!.props.onClick()
    await vi.waitFor(() => expect(onUseSkill).toHaveBeenCalledExactlyOnceWith(tune('c'), 'kept'))
  })

  it('with How it works kept, offers the other setup’s skill beside the kept instructions', async () => {
    const onUseSkill = vi.fn(async () => undefined)
    const tree = KeptInstructions({ compare: tune('b'), teammate: mate, onUse: async () => undefined, skills: [], onUseSkill })!
    const html = renderToStaticMarkup(tree)
    expect(html).toContain('Use these instructions for Wren')
    expect(html).toContain('What it knows added a skill, check-the-cause, which can go beside the setup you kept.')
    buttonNamed(tree, 'Use this skill for Wren')!.props.onClick()
    await vi.waitFor(() => expect(onUseSkill).toHaveBeenCalledExactlyOnceWith(tune('b'), 'other'))
  })

  it('is not offered once used from this tune, when Current was kept, or for a teammate off Claude Code', () => {
    const offered = (compare: PublicCompare, over: Partial<Parameters<typeof KeptInstructions>[0]> = {}): string =>
      renderToStaticMarkup(<KeptInstructions compare={compare} teammate={mate} onUse={async () => undefined} skills={[]} {...over} />)
    expect(offered(tune('c'), { skills: [kept] })).not.toContain('Use this skill')
    expect(offered(tune('a'))).not.toContain('Use this skill')
    expect(offered(tune('c'), { teammate: { ...mate, route: { runtime: 'gemini', model: 'account-default', mode: 'accept-edits' } } })).not.toContain('Use this skill')
    expect(offered(tune('c'), { teammate: { ...mate, route: { runtime: 'codex', model: 'gpt-6', mode: 'accept-edits' } } })).toContain('Use this skill')
    // Kept from another tune under the same name: offered, saying it replaces that one.
    expect(offered(tune('c'), { skills: [{ ...kept, compareId: 'cmp_earlier' }] })).toContain('It replaces the skill of that name Wren has.')
  })
})

describe('Edit teammate', () => {
  const dialog = (over: Partial<Parameters<typeof NewTeammateDialog>[0]> = {}): string =>
    renderToStaticMarkup(<NewTeammateDialog initial={mate} error={undefined} mode="accept-edits" onCancel={() => undefined} onCreate={() => undefined} {...over} />)

  it('lists the skills a teammate keeps, each with Remove', () => {
    const html = dialog({ skills: [kept], onRemoveSkill: async () => undefined })
    expect(html).toContain('>Skills<')
    expect(html).toContain('aria-label="Skills for Wren"')
    expect(html).toContain('<span class="lc-mono">check-the-cause</span>')
    expect(html).toContain('Kept from tunes. A run loads one only when the work matches when to use it.')
    expect(html).toContain('>Remove<')
  })

  it('says when they wait for Claude Code, and shows nothing when there are none', () => {
    expect(dialog({ initial: { ...mate, route: { runtime: 'gemini', model: 'account-default', mode: 'accept-edits' } }, skills: [kept] })).toContain('Only a Claude Code, Codex or OpenCode run is handed these')
    expect(dialog({ initial: { ...mate, route: { runtime: 'codex', model: 'gpt-6', mode: 'accept-edits' } }, skills: [kept] })).toContain('A run loads one only when the work matches')
    expect(dialog({ skills: [] })).not.toContain('>Skills<')
  })
})
