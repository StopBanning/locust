import { renderToStaticMarkup } from 'react-dom/server'
import type { ReactElement } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { seedAvatar } from '../../shared/avatar.js'
import type { PublicCompare } from '../../shared/compare.js'
import type { PublicTeammate } from '../../shared/ipc.js'
import { tuneLessonLine, tuneRoutes, type PublicTuneLesson } from '../../shared/tune.js'
import { KeptInstructions } from './components/KeptInstructions.js'
import { NewTeammateDialog } from './components/NewTeammateDialog.js'

vi.mock('react', async (original) => ({
  ...(await original<typeof import('react')>()),
  useState: (initial: unknown) => [typeof initial === 'function' ? (initial as () => unknown)() : initial, vi.fn()],
  useRef: (initial: unknown) => ({ current: initial }),
  useEffect: vi.fn()
}))
vi.mock('./useModal.js', () => ({ useModal: vi.fn() }))

const mate: PublicTeammate = {
  teammateId: 'tm_wren',
  name: 'Wren',
  role: 'Code & Migrations',
  hue: 'lime',
  avatar: seedAvatar('tm_wren'),
  createdAt: '2026-10-20T12:00:00.000Z',
  instructions: 'Be concise.',
  route: { runtime: 'opencode', model: 'opencode/example-free', mode: 'accept-edits' }
}
const drafts = {
  how: { instructions: 'Check your work.', why: 'The previous answer stopped before checking.' },
  knows: { instructions: 'Name the assumptions behind each conclusion.', why: 'The previous answer treated a guess as a fact.' }
}
const kept: PublicCompare = {
  compareId: 'cmp_tune',
  teammateId: mate.teammateId,
  prompt: 'Explain the cause.',
  createdAt: mate.createdAt,
  blind: true,
  kept: { slot: 'b', at: mate.createdAt },
  slots: tuneRoutes({ runtime: 'opencode', model: 'opencode/example-free', label: 'Example Free' }, 'Be concise.', drafts).map(
    (route, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route, missionIds: [`m_${String(index)}`] })
  )
}
const lesson: PublicTuneLesson = {
  lessonId: 'les_1',
  teammateId: mate.teammateId,
  compareId: kept.compareId,
  createdAt: '2026-10-21T00:00:00.000Z',
  when: 'explaining a cause',
  do: 'check assumptions before concluding',
  because: 'How it works checked its assumptions and was kept',
  confidence: 'low'
}

const flatChildren = (node: unknown): ReactElement[] => {
  if (typeof node !== 'object' || node === null || !('props' in node)) return []
  const element = node as ReactElement<{ children?: unknown }>
  const kids = [element.props.children ?? []].flat(Infinity)
  return [element, ...kids.flatMap(flatChildren)]
}

describe('noting what a tune showed in the UI', () => {
  it('runs nothing before Note what this tune showed is pressed, whether or not the person used the instructions', async () => {
    const onUse = vi.fn(async () => undefined)
    const onNote = vi.fn(async () => ({ lesson }))
    const tree = KeptInstructions({ compare: kept, teammate: mate, onUse, onNote })!
    expect(onNote).not.toHaveBeenCalled()
    expect(onUse).not.toHaveBeenCalled()

    const html = renderToStaticMarkup(<KeptInstructions compare={kept} teammate={mate} onUse={onUse} onNote={onNote} />)
    expect(html).toContain('Note what this tune showed')
    expect(html).toContain('One side turn on Example Free — free.')

    const paidHtml = renderToStaticMarkup(
      <KeptInstructions compare={kept} teammate={mate} free={false} model="GPT-6-Luna" onUse={onUse} onNote={onNote} />
    )
    expect(paidHtml).toContain('One side turn on GPT-6-Luna — counts against your plan.')

    // Even after the person has already saved the kept instructions (or kept Current), Note what this tune showed is offered.
    const savedMate: PublicTeammate = { ...mate, instructions: drafts.how.instructions }
    const afterUseHtml = renderToStaticMarkup(<KeptInstructions compare={kept} teammate={savedMate} onUse={onUse} onNote={onNote} />)
    expect(afterUseHtml).toContain('Note what this tune showed')
    expect(onNote).not.toHaveBeenCalled()

    const noteButton = flatChildren(tree).find(
      (el) => el.type === 'button' && (el.props as { children?: unknown }).children === 'Note what this tune showed'
    ) as ReactElement<{ onClick: () => void }> | undefined
    expect(noteButton).toBeDefined()
    noteButton!.props.onClick()
    await vi.waitFor(() => expect(onNote).toHaveBeenCalledExactlyOnceWith(kept, false))
  })

  it('shows the lesson as When … do … with Remove in the kept strip and in Edit teammate', async () => {
    const onRemove = vi.fn(async () => undefined)
    const stripTree = KeptInstructions({ compare: kept, teammate: mate, onUse: async () => undefined, lesson, onRemoveLesson: onRemove })!
    const stripHtml = renderToStaticMarkup(stripTree)
    expect(stripHtml).toContain(tuneLessonLine(lesson))
    expect(stripHtml).toContain('When explaining a cause, do check assumptions before concluding.')
    expect(stripHtml).toContain('Remove')

    const removeInStrip = flatChildren(stripTree).find(
      (el) => el.type === 'button' && (el.props as { children?: unknown }).children === 'Remove'
    ) as ReactElement<{ onClick: () => void }> | undefined
    removeInStrip!.props.onClick()
    await vi.waitFor(() => expect(onRemove).toHaveBeenCalledExactlyOnceWith(lesson.lessonId))

    const dialogHtml = renderToStaticMarkup(
      <NewTeammateDialog
        initial={mate}
        error={undefined}
        mode="accept-edits"
        onCancel={() => undefined}
        onCreate={() => undefined}
        lessons={[lesson]}
        onRemoveLesson={onRemove}
      />
    )
    expect(dialogHtml).toContain('Tune lessons')
    expect(dialogHtml).toContain('When explaining a cause, do check assumptions before concluding.')
    expect(dialogHtml).toContain('Remove')
  })
})
