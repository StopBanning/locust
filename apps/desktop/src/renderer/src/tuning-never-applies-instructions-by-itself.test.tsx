import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ReactElement } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { seedAvatar } from '../../shared/avatar.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import type { PublicCompare } from '../../shared/compare.js'
import { compareRecord } from '../../shared/compare.js'
import { compareRecordRows } from './compareRecord.js'
import { currentDrafts, keptInstructions, parseTuneDrafts, tuneRefusal, tuneRoutes, TUNE_DRAFT_COST, TUNE_FALLBACK } from '../../shared/tune.js'
import { KeptInstructions } from './components/KeptInstructions.js'
import { TuneTeammate, type TuneCard } from './components/TuneTeammate.js'

// The offer has only local state. Inspect its actual event callback without a DOM or an agent.
vi.mock('react', async (original) => ({ ...await original<typeof import('react')>(), useState: (initial: unknown) => [initial, vi.fn()], useRef: (initial: unknown) => ({ current: initial }) }))
vi.mock('./useModal.js', () => ({ useModal: vi.fn() }))
const mate: PublicTeammate = { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations', hue: 'lime', avatar: seedAvatar('tm_wren'), createdAt: '2026-10-20T12:00:00.000Z', instructions: 'Be concise.', route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' } }
const drafts = { how: { instructions: 'Check your work.', why: 'The previous answer stopped before checking its result.' }, knows: { instructions: 'Name the assumptions behind each conclusion.', why: 'The previous answer treated an assumption as a fact.' } }
const card: TuneCard = { id: 1, teammateId: mate.teammateId, route: mate.route!, name: mate.name, model: 'Example Free', free: true, prompt: 'Explain the cause.', current: mate.instructions!, drafts, drafting: false }
const comparison: PublicCompare = { compareId: 'cmp_tune', teammateId: mate.teammateId, prompt: card.prompt, createdAt: mate.createdAt, blind: true, slots: tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free' }, card.current, drafts).map((route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${index}`] })) }
const noop = (): void => undefined
const screen = (over: Partial<TuneCard> = {}): string => renderToStaticMarkup(<TuneTeammate card={{ ...card, ...over }} onChange={noop} onCancel={noop} onRun={noop} />)

describe('tuning a teammate', () => {
  it('explains why tuning needs a finished teammate conversation and a model', () => {
    expect(tuneRefusal(undefined, 'm_1', true)).toContain('teammate')
    expect(tuneRefusal(mate, undefined, false)).toContain('Finish a turn')
    expect(tuneRefusal(mate, 'm_1', false)).toContain('Finish a turn')
    expect(tuneRefusal(mate, 'm_1', true)).toBeUndefined()
  })
  it('starts one disclosed draft turn on /tune, while only the button starts the three trials', () => {
    const html = screen({ drafting: true })
    expect(html).toContain(TUNE_DRAFT_COST)
    expect(html).toContain('Drafting the two edits')
    expect(html).toContain('disabled="">Run all three</button>')
    expect(screen()).toContain('Three runs of Example Free — free.')
    expect(screen({ free: false })).toContain('counts against your plan three times')
    const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
    expect(source).toContain("prompt.trim().toLowerCase() === '/tune'")
    const opening = source.slice(source.indexOf('const openTune ='), source.indexOf('const closeTune ='))
    expect(opening).toContain('draftTune(of)')
    expect(opening).not.toContain('startCompare(')
    expect(source).toContain('changes: true, blind: true')
  })
  it('keeps the ask and all three instruction boxes editable with the teammate cap', () => {
    const html = screen()
    for (const label of ['Ask to run again', 'Current instructions', 'How it works instructions', 'What it knows instructions']) expect(html).toContain(`aria-label="${label}"`)
    expect(html.match(/maxLength="4000"/g)).toHaveLength(3)
    expect(html).toContain(drafts.how.why)
    expect(html).toContain(drafts.knows.why)
    expect(html).toContain('role="dialog" aria-modal="true"')
    expect(html).toContain('Cancel')
  })
  it('does not lose a completed draft when the person edits the ask at the same moment', () => {
    const old = { ...card, drafting: true, drafts: currentDrafts(card.current) }
    let update: ((current: TuneCard) => TuneCard) | undefined
    const nodes = (node: unknown): ReactElement<Record<string, unknown>>[] => {
      if (Array.isArray(node)) return node.flatMap(nodes)
      if (typeof node !== 'object' || node === null || !('props' in node)) return []
      const element = node as ReactElement<Record<string, unknown>>
      return [element, ...nodes(element.props.children)]
    }
    const tree = TuneTeammate({ card: old, onChange: (edit) => { update = edit }, onCancel: noop, onRun: noop })
    const field = nodes(tree).find((node) => node.type === 'textarea' && node.props['aria-label'] === 'Ask to run again')!
    ;(field.props.onChange as (event: { target: { value: string } }) => void)({ target: { value: 'A revised ask.' } })
    const arrived = { ...old, drafting: false, drafts }
    expect(update?.(arrived)).toMatchObject({ prompt: 'A revised ask.', drafts, drafting: false })
    expect(readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')).toContain('current?.id !== tuning.id ? current : edit(current)')
  })
  it('bounds JSON drafts and leaves copies of Current on missing, malformed or untrusted shapes', () => {
    const copies = { how: { instructions: card.current, why: TUNE_FALLBACK }, knows: { instructions: card.current, why: TUNE_FALLBACK } }
    expect(currentDrafts(card.current)).toEqual(copies)
    expect(parseTuneDrafts(JSON.stringify(drafts), card.current)).toEqual(drafts)
    expect(parseTuneDrafts('```json\n' + JSON.stringify(drafts) + '\n```', card.current)).toEqual(drafts)
    for (const raw of ['', 'not JSON', 'null', '[]', '{"how":{}}', JSON.stringify({ ...drafts, knows: { instructions: 4, why: 'Wrong type.' } }), JSON.stringify({ ...drafts, how: { instructions: 'x'.repeat(4001), why: 'Long.' } }), JSON.stringify({ ...drafts, how: { instructions: 'a\0b', why: 'Null.' } }), 'x'.repeat(32_001)]) {
      expect(parseTuneDrafts(raw, card.current)).toEqual(copies)
      expect(parseTuneDrafts(raw, card.current).how.why).toBe(TUNE_FALLBACK)
    }
    expect(parseTuneDrafts(JSON.stringify(currentDrafts('')), '')).toEqual(currentDrafts(''))
  })
  it('keeps the same model and account on three explicit candidates, including Current and empty instructions', () => {
    const route = { runtime: 'codex', model: 'same', account: 'account_one', effort: 'high' }
    expect(tuneRoutes(route, '', drafts)).toEqual(['', drafts.how.instructions, drafts.knows.instructions].map((instructions, index) => ({ ...route, label: `same · ${['Current', 'How it works', 'What it knows'][index]}`, instructions })))
  })
  it('does not turn a model’s votes against its own setups into model leaderboard results', () => {
    const trial = { ...comparison, kept: { slot: 'b' as const, at: mate.createdAt } }
    expect([...compareRecord([trial])]).toEqual([])
    expect(compareRecordRows([trial], () => undefined, (route) => route.model)).toEqual([])
    const ordinary = { ...trial, slots: trial.slots.map((slot) => ({ ...slot, route: { runtime: slot.route.runtime, model: slot.slot } })) }
    expect([...compareRecord([ordinary])]).toHaveLength(3)
    expect(compareRecordRows([ordinary], () => undefined, (route) => route.model)).toHaveLength(3)
  })
  it('offers the kept draft, never an unkept column, Current or another teammate’s words', () => {
    expect(keptInstructions(comparison, mate)).toBeUndefined()
    expect(keptInstructions({ ...comparison, kept: { slot: 'a', at: mate.createdAt } }, mate)).toBeUndefined()
    expect(keptInstructions({ ...comparison, kept: { slot: 'b', at: mate.createdAt } }, mate)).toBe(drafts.how.instructions)
    expect(keptInstructions({ ...comparison, teammateId: 'tm_other', kept: { slot: 'b', at: mate.createdAt } }, mate)).toBeUndefined()
    const emptied = { ...comparison, slots: comparison.slots.map((slot) => ({ ...slot, route: { ...slot.route, instructions: '' } })), kept: { slot: 'b' as const, at: mate.createdAt } }
    expect(keptInstructions(emptied, mate)).toBe('')
  })
  it('never applies instructions on render or Not now; only the explicit button calls the save path', async () => {
    const onUse = vi.fn(async () => undefined)
    const kept = { ...comparison, kept: { slot: 'b' as const, at: mate.createdAt } }
    const element = KeptInstructions({ compare: kept, teammate: mate, onUse })!
    const buttons = (element.props as { children: unknown[] }).children.filter((child): child is ReactElement<{ onClick: () => void; children: unknown }> => typeof child === 'object' && child !== null && 'type' in child && child.type === 'button')
    expect(onUse).not.toHaveBeenCalled()
    expect(buttons).toHaveLength(2)
    buttons[1]!.props.onClick()
    expect(onUse).not.toHaveBeenCalled()
    buttons[0]!.props.onClick()
    await vi.waitFor(() => expect(onUse).toHaveBeenCalledExactlyOnceWith(mate, drafts.how.instructions))
    const html = renderToStaticMarkup(<KeptInstructions compare={kept} teammate={mate} onUse={onUse} />)
    expect(html).toContain('Use these instructions for Wren')
    expect(html).toContain('Not now')
  })
})
