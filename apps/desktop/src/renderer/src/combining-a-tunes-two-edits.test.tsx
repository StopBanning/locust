import { renderToStaticMarkup } from 'react-dom/server'
import type { ReactElement } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { seedAvatar } from '../../shared/avatar.js'
import type { PublicCompare } from '../../shared/compare.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { combineSetups, parseTuneCombine, TUNE_COMBINE_FALLBACK, TUNE_COMBINE_SAME, tuneRoutes } from '../../shared/tune.js'
import { KeptInstructions } from './components/KeptInstructions.js'

vi.mock('react', async (original) => ({
  ...(await original<typeof import('react')>()),
  useState: (initial: unknown) => [typeof initial === 'function' ? (initial as () => unknown)() : initial, vi.fn()],
  useRef: (initial: unknown) => ({ current: initial }),
  useEffect: vi.fn()
}))

/*
 * COMBINE BOTH EDITS (0.811). A tune keeps one draft and leaves the other, which may still have changed something
 * worth keeping; after Harness²'s composition step, one side turn writes one set of instructions with both edits.
 * The person reads it and decides: nothing is saved by the turn.
 */
const mate: PublicTeammate = {
  teammateId: 'tm_wren',
  name: 'Wren',
  role: 'Code & Migrations',
  hue: 'lime',
  avatar: seedAvatar('tm_wren'),
  createdAt: '2026-10-21T12:00:00.000Z',
  instructions: 'Be concise.',
  route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' }
}
const drafts = {
  how: { instructions: 'Be concise. Check your work before you finish.', why: 'The answer stopped before checking.' },
  knows: { instructions: 'Be concise. Name the assumption behind each conclusion.', why: 'The answer treated a guess as a fact.' }
}
const tune = (keptSlot: 'a' | 'b' | 'c', knows = drafts.knows): PublicCompare => ({
  compareId: 'cmp_tune',
  teammateId: mate.teammateId,
  prompt: 'Explain the cause.',
  createdAt: mate.createdAt,
  blind: true,
  kept: { slot: keptSlot, at: mate.createdAt },
  slots: tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free', label: 'Example Free' }, 'Be concise.', { how: drafts.how, knows }).map(
    (route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route: { ...route, why: index === 0 ? undefined : [drafts.how.why, knows.why][index - 1] }, missionIds: [`m_${String(index)}`] })
  )
})

const flatChildren = (node: unknown): ReactElement[] => {
  if (typeof node !== 'object' || node === null || !('props' in node)) return []
  const element = node as ReactElement<{ children?: unknown }>
  const kids = [element.props.children ?? []].flat(Infinity)
  return [element, ...kids.flatMap(flatChildren)]
}

describe('the two edits a tune can combine', () => {
  it('are the kept draft and the other one, each over Current', () => {
    expect(combineSetups(tune('b'))).toEqual({
      current: 'Be concise.',
      kept: { name: 'How it works', instructions: drafts.how.instructions, why: drafts.how.why },
      other: { name: 'What it knows', instructions: drafts.knows.instructions, why: drafts.knows.why }
    })
    expect(combineSetups(tune('c'))?.kept.name).toBe('What it knows')
  })

  it('are none when Current was kept, or the other draft changed nothing, or matches the kept one', () => {
    expect(combineSetups(tune('a'))).toBeUndefined()
    expect(combineSetups(tune('b', { instructions: 'Be concise.', why: 'No change was needed.' }))).toBeUndefined()
    expect(combineSetups(tune('b', drafts.how))).toBeUndefined()
    const { kept: _kept, ...unkept } = tune('b')
    expect(combineSetups(unkept)).toBeUndefined()
  })
})

describe('what the side turn is asked, and what is taken from it', () => {
  const setups = combineSetups(tune('b'))!

  it('takes one set of instructions and one line, through a fence or a think block, and nothing else', () => {
    const both = 'Be concise. Check your work before you finish. Name the assumption behind each conclusion.'
    expect(parseTuneCombine(`<think>hm</think>\n\`\`\`json\n${JSON.stringify({ instructions: both, note: 'Both edits, as written.' })}\n\`\`\``, setups)).toEqual({ ok: true, instructions: both, note: 'Both edits, as written.' })
    expect(parseTuneCombine('Here you go: {"instructions": "x"}', setups)).toEqual({ ok: false, message: TUNE_COMBINE_FALLBACK })
    expect(parseTuneCombine('not json', setups)).toEqual({ ok: false, message: TUNE_COMBINE_FALLBACK })
    expect(parseTuneCombine(JSON.stringify({ instructions: '', note: 'Nothing.' }), setups)).toEqual({ ok: false, message: TUNE_COMBINE_FALLBACK })
    // The kept setup's own words back: nothing new to offer.
    expect(parseTuneCombine(JSON.stringify({ instructions: drafts.how.instructions, note: 'Dropped the other edit.' }), setups)).toEqual({ ok: false, message: TUNE_COMBINE_SAME })
  })
})

describe('Combine both edits on the kept strip', () => {
  it('is offered beside Note what this tune showed when there are two edits to combine, and runs only when pressed', async () => {
    const onCombine = vi.fn(async () => ({ instructions: 'Combined.', note: 'Both.' }))
    const tree = KeptInstructions({ compare: tune('b'), teammate: mate, onUse: async () => undefined, onCombine })!
    const html = renderToStaticMarkup(tree)
    expect(html).toContain('Note what this tune showed')
    expect(html).toContain('Combine both edits')
    expect(onCombine).not.toHaveBeenCalled()
    const button = flatChildren(tree).find((el) => el.type === 'button' && (el.props as { children?: unknown }).children === 'Combine both edits') as ReactElement<{ onClick: () => void }> | undefined
    button!.props.onClick()
    await vi.waitFor(() => expect(onCombine).toHaveBeenCalledExactlyOnceWith(tune('b')))
  })

  it('is not offered when Current was kept or there is no second edit', () => {
    expect(renderToStaticMarkup(<KeptInstructions compare={tune('a')} teammate={mate} onUse={async () => undefined} />)).not.toContain('Combine both edits')
    expect(renderToStaticMarkup(<KeptInstructions compare={tune('b', drafts.how)} teammate={mate} onUse={async () => undefined} />)).not.toContain('Combine both edits')
  })
})
